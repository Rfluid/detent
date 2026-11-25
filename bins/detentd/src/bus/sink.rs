//! The daemon-facing side of the bus: [`ActionSink`] (dispatch thread, IPC) routes
//! named actions, leased input and events to plugins and reports their state.

use std::process::Child;
use std::sync::atomic::Ordering;
use std::time::Instant;

use detent_bus::{ButtonEdge, Capability, DaemonToPlugin, Event, LeaseRevokeReason};
use detent_config::CompiledConfig;
use detent_engine::{ControlOut, ControlSlot, slot};
use serde_json::{Value, json};
use tracing::{info, warn};

use super::Bus;
use super::supervisor::stop_child;
use crate::shared::{ActionSink, lock};

fn now_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as u64)
}

fn button_name(code: u16, slot: u8) -> &'static str {
    use detent_engine::out::code as c;
    match (code, slot) {
        (c::BTN_LEFT, _) => "left",
        (c::BTN_RIGHT, _) => "right",
        (c::BTN_MIDDLE, _) => "middle",
        (c::BTN_SIDE, _) => "back",
        (c::BTN_EXTRA, _) => "forward",
        (0, s) => match ControlSlot(s) {
            slot::GESTURE => "gesture",
            slot::THUMB => "thumb",
            slot::MIDDLE => "middle",
            slot::BACK => "back",
            slot::FORWARD => "forward",
            slot::SMARTSHIFT => "smartshift",
            _ => "control",
        },
        _ => "button",
    }
}

impl ActionSink for Bus {
    fn invoke(&self, action: &str, params_json: &str, trigger: &str, delta: i16) -> bool {
        let Some((plugin, local)) = action.split_once('.') else {
            return false;
        };
        let st = lock(&self.state);
        let Some(c) = st.conns.get(plugin) else {
            return false;
        };
        if !c.manifest.actions.iter().any(|a| a.name == local) {
            warn!("plugin `{plugin}` has no action `{local}`");
            return false;
        }
        c.send(&DaemonToPlugin::ActionInvoked {
            action: local.to_string(),
            params_json: params_json.to_string(),
            trigger: trigger.to_string(),
            delta,
            ts_us: now_us(),
        });
        true
    }

    fn summary(&self) -> Value {
        let st = lock(&self.state);
        let mut names: Vec<&String> = st.procs.keys().chain(st.conns.keys()).collect();
        names.sort();
        names.dedup();
        Value::Array(
            names
                .into_iter()
                .map(|n| {
                    let c = st.conns.get(n);
                    let p = st.procs.get(n);
                    let state = match (c, p.and_then(|p| p.child.as_ref())) {
                        (Some(_), _) => "connected",
                        (None, Some(_)) => "starting",
                        (None, None) => "stopped",
                    };
                    json!({
                        "name": n,
                        "state": state,
                        "pid": p.and_then(|p| p.child.as_ref()).map(Child::id),
                        "restarts": p.map_or(0, |p| p.restarts),
                        "last_error": p.and_then(|p| p.last_error.clone()),
                        "dropped": c.map_or(0, |c| c.dropped.load(Ordering::Relaxed)),
                        "caps": c.map(|c| c.granted.iter().map(Capability::name).collect::<Vec<_>>()),
                        "version": c.map(|c| c.manifest.version.clone()),
                        "uptime_s": c.map(|c| c.connected_at.elapsed().as_secs()),
                    })
                })
                .collect(),
        )
    }

    fn actions(&self) -> Value {
        let st = lock(&self.state);
        let mut v = vec![
            json!({"name": "exec", "doc": "Run a command: {\"cmd\": \"...\"}", "continuous": false, "plugin": "detent"}),
            json!({"name": "keys", "doc": "Tap a key chord: {\"keys\": \"Ctrl+C\"}", "continuous": false, "plugin": "detent"}),
        ];
        for (p, c) in &st.conns {
            for a in &c.manifest.actions {
                v.push(json!({
                    "name": format!("{p}.{}", a.name),
                    "doc": a.doc,
                    "continuous": a.continuous,
                    "plugin": p,
                }));
            }
        }
        Value::Array(v)
    }

    fn restart(&self, name: &str) -> Result<(), String> {
        let mut st = lock(&self.state);
        let p = st
            .procs
            .get_mut(name)
            .ok_or_else(|| format!("no supervised plugin `{name}`"))?;
        p.restarts = 0;
        p.next_start = Instant::now();
        if let Some(mut ch) = p.child.take() {
            stop_child(&mut ch);
        }
        Ok(())
    }

    fn event(&self, topic: &str, ev: &Value) {
        self.publish(topic, ev);
    }

    fn config_changed(&self, cfg: &CompiledConfig) {
        Bus::config_changed(self, cfg);
    }

    fn shutdown(&self) {
        Bus::shutdown(self);
    }

    fn lease_input(&self, c: ControlOut) {
        let holder = lock(&self.state)
            .lease
            .as_ref()
            .map(|l| (l.plugin.clone(), l.id));
        let Some((plugin, id)) = holder else { return };
        let ts_us = now_us();
        let msg = match c {
            ControlOut::LeaseMotion { dx, dy } => {
                DaemonToPlugin::Event(Event::Motion { dx, dy, ts_us })
            }
            ControlOut::LeaseButton { code, slot, down } => DaemonToPlugin::Event(Event::Button {
                control: button_name(code, slot).to_string(),
                edge: if down {
                    ButtonEdge::Press
                } else {
                    ButtonEdge::Release
                },
                ts_us,
            }),
            ControlOut::LeaseEnded { escape } => {
                lock(&self.state).lease = None;
                let reason = if escape {
                    LeaseRevokeReason::EscapeChord
                } else {
                    LeaseRevokeReason::Expired
                };
                info!("lease of `{plugin}` ended ({reason:?})");
                DaemonToPlugin::LeaseRevoked { id, reason }
            }
            _ => return,
        };
        self.send_to(&plugin, &msg);
    }
}
