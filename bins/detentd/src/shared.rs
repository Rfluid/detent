//! State shared by the control-plane threads (never touched by the data plane except
//! the `config` ArcSwap, which it only loads).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};

use arc_swap::ArcSwap;
use detent_config::{CompiledConfig, NamedAction};
use serde_json::{Value, json};
use tracing::info;

use crate::msg::{Cmd, WorkerMsg};

pub struct DeviceHandle {
    pub tx: SyncSender<WorkerMsg>,
    pub name: String,
    pub node: String,
    /// Receiver slot (1–6); `None` for a direct device.
    pub slot: Option<u8>,
    /// The data plane's counters (read-only here).
    pub stats: Arc<crate::dataplane::Stats>,
}

impl DeviceHandle {
    pub fn summary(&self, id: &str) -> detent_ipc::types::DeviceSummary {
        detent_ipc::types::DeviceSummary {
            id: id.to_string(),
            name: self.name.clone(),
            node: self.node.clone(),
            slot: self.slot,
            frames: self.stats.frames.load(Ordering::Relaxed),
            dropped: self.stats.dropped_to_worker.load(Ordering::Relaxed)
                + self.stats.dropped_out.load(Ordering::Relaxed),
        }
    }
}

/// Where named actions (`kde.overview`, `radial.open`, ...) go: the plugin bus.
pub trait ActionSink: Send + Sync {
    /// `Ok(true)` handled, `Ok(false)` no plugin provides it.
    fn invoke(&self, action: &str, params_json: &str, trigger: &str, delta: i16) -> bool;
    fn summary(&self) -> Value;
    /// Every action plugins declared: `[{name, doc, continuous, plugin}]`.
    fn actions(&self) -> Value;
    fn restart(&self, name: &str) -> Result<(), String>;
    /// A daemon event (`link`, `context`, ...) for subscribed plugins.
    fn event(&self, topic: &str, ev: &Value);
    fn config_changed(&self, cfg: &CompiledConfig);
    fn shutdown(&self);
    /// Leased input or lease end from a data plane.
    fn lease_input(&self, c: detent_engine::ControlOut);
}

struct Watcher {
    topics: Vec<String>,
    tx: SyncSender<Value>,
}

pub struct Shared {
    /// The loaded config file (control plane: settings, plugins, profiles, actions).
    pub config: Arc<ArcSwap<CompiledConfig>>,
    /// What the data planes run: `config`, or the same config with no bindings while
    /// remapping is paused (`remap.set`). Only ever swapped, never locked.
    pub active: Arc<ArcSwap<CompiledConfig>>,
    /// Remapping on (`true`) or paused (`detentctl disable`, tray). Not persisted.
    pub remap: AtomicBool,
    pub started: std::time::Instant,
    pub devices: Mutex<BTreeMap<String, DeviceHandle>>,
    pub shutdown: AtomicBool,
    pub actions: Mutex<Option<Arc<dyn ActionSink>>>,
    /// Last config error (shown by `detentctl`), `None` if the active config is the file's.
    pub config_error: Mutex<Option<String>>,
    pub config_path: std::path::PathBuf,
    /// Context published by plugins (`app`, `desktop`, ...) for profile matching.
    pub context: Mutex<BTreeMap<String, String>>,
    pub profile: AtomicU8,
    /// Data planes hand exec / named actions / profile switches to the dispatch thread.
    pub dispatch: SyncSender<detent_engine::ControlOut>,
    watchers: Mutex<Vec<Watcher>>,
    /// Serializes config installs and remap switches (control plane only).
    remap_lock: Mutex<()>,
}

/// The config the data planes run: `c` itself, or `c` without bindings while paused.
fn engine_view(c: Arc<CompiledConfig>, remap: bool) -> Arc<CompiledConfig> {
    if remap {
        return c;
    }
    Arc::new(CompiledConfig {
        engine: detent_engine::Compiled::default(),
        ..(*c).clone()
    })
}

/// Poison-tolerant lock: a panicked control-plane thread must not take the daemon down.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    pub fn new(
        config: CompiledConfig,
        path: std::path::PathBuf,
        dispatch: SyncSender<detent_engine::ControlOut>,
    ) -> Self {
        let config = Arc::new(config);
        Self {
            dispatch,
            active: Arc::new(ArcSwap::new(config.clone())),
            config: Arc::new(ArcSwap::new(config)),
            remap: AtomicBool::new(true),
            started: std::time::Instant::now(),
            devices: Mutex::new(BTreeMap::new()),
            shutdown: AtomicBool::new(false),
            actions: Mutex::new(None),
            config_error: Mutex::new(None),
            config_path: path,
            context: Mutex::new(BTreeMap::new()),
            profile: AtomicU8::new(0),
            watchers: Mutex::new(Vec::new()),
            remap_lock: Mutex::new(()),
        }
    }

    /// Install a newly loaded config. The data planes get it too unless remapping is paused.
    pub fn install_config(&self, c: Arc<CompiledConfig>) {
        let _g = lock(&self.remap_lock);
        self.config.store(c.clone());
        self.active
            .store(engine_view(c, self.remap.load(Ordering::Relaxed)));
    }

    /// Pause (`false`) or resume (`true`) remapping. Pausing swaps the data planes to the
    /// same config without bindings: workers undivert every control, the engines release
    /// held keys and drop the evdev grab, and the mouse behaves as without Detent. Device
    /// settings (DPI, SmartShift, ...) and plugins keep running. Returns whether it changed.
    pub fn set_remap(&self, enabled: bool) -> bool {
        {
            let _g = lock(&self.remap_lock);
            if self.remap.swap(enabled, Ordering::Relaxed) == enabled {
                return false;
            }
            self.active
                .store(engine_view(self.config.load_full(), enabled));
        }
        info!("remapping {}", if enabled { "resumed" } else { "paused" });
        self.broadcast(|| Cmd::Rebind);
        self.update_profile();
        self.publish("remap", json!({ "enabled": enabled }));
        true
    }

    pub fn remap_enabled(&self) -> bool {
        self.remap.load(Ordering::Relaxed)
    }

    fn sink(&self) -> Option<Arc<dyn ActionSink>> {
        lock(&self.actions).clone()
    }

    /// A binding fired a named action.
    pub fn invoke_action(&self, a: &NamedAction, delta: i16) {
        self.publish(
            "action",
            json!({"action": a.name, "trigger": a.trigger, "delta": delta}),
        );
        let handled = self
            .sink()
            .is_some_and(|s| s.invoke(&a.name, &a.params_json, &a.trigger, delta));
        if !handled {
            info!(
                "action `{}` (from `{}`): no running plugin provides it",
                a.name, a.trigger
            );
        }
    }

    /// `actions.invoke` from IPC or a plugin. Built-ins: `exec {cmd}`, `keys {keys}`.
    pub fn invoke_named(&self, name: &str, params_json: &str, trigger: &str) -> Result<(), String> {
        let params: Value = serde_json::from_str(params_json).unwrap_or(Value::Null);
        match name {
            detent_bus::builtin::EXEC => {
                let cmd = params["cmd"]
                    .as_str()
                    .ok_or("`exec` needs {\"cmd\": \"...\"}")?;
                self.publish("exec", json!({ "cmd": cmd, "trigger": trigger }));
                crate::exec::spawn(cmd);
                return Ok(());
            }
            detent_bus::builtin::KEYS => {
                let keys = params["keys"]
                    .as_str()
                    .ok_or("`keys` needs {\"keys\": \"Ctrl+C\"}")?;
                let codes = detent_config::keys::parse_chord(keys)?;
                let chord = detent_engine::action::KeyChord::new(&codes).ok_or("1 to 6 keys")?;
                let devs = lock(&self.devices);
                let d = devs.values().next().ok_or("no device attached")?;
                d.tx.try_send(WorkerMsg::Cmd(Cmd::EmitKeys(chord)))
                    .map_err(|_| "device busy".to_string())?;
                return Ok(());
            }
            detent_bus::builtin::DEVICE_HOST => {
                let to = match &params["to"] {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => return Err("`device.host` needs {\"to\": 2 | \"next\" | \"prev\"}".into()),
                };
                let t = crate::ipc::handlers::hosts::parse_target(&to).map_err(|e| e.1)?;
                let devs = lock(&self.devices);
                let d = devs.values().next().ok_or("no device attached")?;
                d.tx.try_send(WorkerMsg::Cmd(Cmd::Device(
                    detent_engine::action::DeviceCmd::ChangeHost(t),
                )))
                .map_err(|_| "device busy".to_string())?;
                return Ok(());
            }
            _ => {}
        }
        self.publish("action", json!({"action": name, "trigger": trigger}));
        if self
            .sink()
            .is_some_and(|s| s.invoke(name, params_json, trigger, 0))
        {
            Ok(())
        } else {
            Err(format!("no running plugin provides `{name}`"))
        }
    }

    pub fn plugins_summary(&self) -> Value {
        self.sink().map_or(json!([]), |s| s.summary())
    }

    pub fn actions_list(&self) -> Value {
        self.sink().map_or(json!([]), |s| s.actions())
    }

    pub fn restart_plugin(&self, name: &str) -> Result<(), String> {
        self.sink()
            .ok_or_else(|| "plugin bus not running".to_string())?
            .restart(name)
    }

    pub fn detached(&self, key: &str) {
        lock(&self.devices).remove(key);
        self.publish("link", json!({"device": key, "connected": false}));
    }

    pub fn broadcast(&self, mk: impl Fn() -> Cmd) {
        for d in lock(&self.devices).values() {
            let _ = d.tx.send(WorkerMsg::Cmd(mk()));
        }
    }

    pub fn active_profile_name(&self) -> String {
        let p = self.profile.load(Ordering::Relaxed);
        if p == 0 {
            return "base".into();
        }
        self.config
            .load()
            .profiles
            .get(p as usize - 1)
            .map_or_else(|| "base".into(), |m| m.name.clone())
    }

    /// Re-evaluate profiles after a context or config change; tell devices if it changed.
    pub fn update_profile(&self) {
        let ctx = lock(&self.context).clone();
        let p = self.config.load().profile_for(&ctx);
        let old = self.profile.swap(p, Ordering::Relaxed);
        if old != p {
            info!("profile: {}", self.active_profile_name());
            self.publish("profile", json!({"profile": self.active_profile_name()}));
        }
        // Always re-send: devices attached later or a reload reset needs it.
        self.broadcast(|| Cmd::Profile(p));
    }

    pub fn set_context(&self, key: &str, value: &str) {
        let changed = {
            let mut c = lock(&self.context);
            c.get(key).map(String::as_str) != Some(value) && {
                c.insert(key.to_string(), value.to_string());
                true
            }
        };
        if changed {
            self.publish("context", json!({"key": key, "value": value}));
            self.update_profile();
        }
    }

    /// `events.subscribe`: topics empty = everything.
    pub fn subscribe(&self, topics: Vec<String>) -> Receiver<Value> {
        let (tx, rx) = sync_channel(256);
        lock(&self.watchers).push(Watcher { topics, tx });
        rx
    }

    /// Fan an event out to plugins and `detentctl watch` clients (non-blocking; slow ones
    /// lose events).
    pub fn publish(&self, topic: &str, mut ev: Value) {
        if let Some(s) = self.sink() {
            s.event(topic, &ev);
        }
        let mut w = lock(&self.watchers);
        if w.is_empty() {
            return;
        }
        if let Value::Object(m) = &mut ev {
            m.insert("topic".into(), Value::from(topic));
        }
        w.retain(|x| {
            if !x.topics.is_empty() && !x.topics.iter().any(|t| t == topic) {
                return true;
            }
            !matches!(
                x.tx.try_send(ev.clone()),
                Err(std::sync::mpsc::TrySendError::Disconnected(_))
            )
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> CompiledConfig {
        detent_config::load(include_str!("../../../config/detent.example.toml")).unwrap()
    }

    #[test]
    fn pause_strips_bindings_only_from_the_data_plane_view() {
        let (tx, _rx) = sync_channel(1);
        let s = Shared::new(example(), "/x.toml".into(), tx);
        let events = s.subscribe(vec!["remap".into()]);
        let diverted = |c: &ArcSwap<CompiledConfig>| c.load().engine.diversion().count();
        assert!(diverted(&s.active) > 0);

        assert!(s.set_remap(false));
        assert!(!s.set_remap(false), "second pause is a no-op");
        assert_eq!(diverted(&s.active), 0);
        // plugins, settings and profiles keep reading the real config
        assert!(diverted(&s.config) > 0);
        assert_eq!(events.try_recv().unwrap()["enabled"], false);

        // a reload while paused stays paused
        s.install_config(Arc::new(example()));
        assert_eq!(diverted(&s.active), 0);

        assert!(s.set_remap(true));
        assert!(diverted(&s.active) > 0);
        assert_eq!(events.try_recv().unwrap()["enabled"], true);
    }
}
