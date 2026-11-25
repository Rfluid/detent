//! Per-device worker: the control plane's side of one device. Owns the `DeviceState`,
//! applies config (settings + diversion), resyncs after reconnects, runs exec and device
//! commands the data plane hands over. All HID++ requests go through the data plane's
//! ring, so the data plane stays the only reader/writer of the hidraw fd.

use std::collections::VecDeque;
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use detent_config::CompiledConfig;
use detent_config::schema::DeviceSettings;
use detent_device::DeviceState;
use detent_device::requester::error_name;
use detent_engine::ControlOut;
use detent_engine::action::DeviceCmd;
use detent_hidpp_features::{
    Feature, RequestError, Requester, adjustable_dpi, haptics, reprog_controls, smartshift,
    unified_battery,
};
use detent_hidpp_proto::{DeviceIndex, FeatureId, Frame};
use tracing::{debug, info, warn};

use crate::msg::{Cmd, FromDp, ToDp, WorkerMsg};
use crate::shared::Shared;
use ring::RingRequester;

mod hosts;
mod ring;
mod settings;

/// Wait after a reconnect before re-applying, so the kernel's own setup runs first.
const RESYNC_DELAY: Duration = Duration::from_millis(300);
/// Cheap check that diversion is still in place (catches missed reconnect events).
const VERIFY_EVERY: Duration = Duration::from_secs(30);

pub struct Link {
    pub tx: rtrb::Producer<ToDp>,
    pub wake: OwnedFd,
    /// Where requests go: 0xFF direct, 1–6 a receiver slot.
    pub device: DeviceIndex,
}

impl Link {
    pub fn send(&mut self, m: ToDp) {
        if self.tx.push(m).is_err() {
            warn!("data-plane command ring full; dropped {m:?}");
            return;
        }
        let _ = rustix::io::write(&self.wake, &1u64.to_ne_bytes());
    }
}

pub struct Worker {
    pub key: String,
    pub link: Link,
    pub rx: Receiver<WorkerMsg>,
    pub state: DeviceState,
    pub shared: Arc<Shared>,
    pub dp: Option<JoinHandle<&'static str>>,
    backlog: VecDeque<WorkerMsg>,
    gone: Option<&'static str>,
    /// What we diverted: (cid, raw XY).
    diverted: Vec<(u16, bool)>,
    resync_at: Option<Instant>,
    next_verify: Instant,
    /// Live edits (`detentctl set` / plugins) layered on top of the config until reload.
    overrides: serde_json::Map<String, serde_json::Value>,
}

fn fmt_err(e: &RequestError) -> String {
    match e {
        RequestError::Device(c) => format!("device error 0x{c:02X} ({})", error_name(*c)),
        other => other.to_string(),
    }
}

impl Worker {
    pub fn new(
        key: String,
        link: Link,
        rx: Receiver<WorkerMsg>,
        state: DeviceState,
        shared: Arc<Shared>,
        dp: JoinHandle<&'static str>,
    ) -> Self {
        Self {
            key,
            link,
            rx,
            state,
            shared,
            dp: Some(dp),
            backlog: VecDeque::new(),
            gone: None,
            diverted: Vec::new(),
            resync_at: None,
            next_verify: Instant::now() + VERIFY_EVERY,
            overrides: serde_json::Map::new(),
        }
    }

    /// Run `f` with a requester over the ring.
    fn with_dev<R>(&mut self, f: impl FnOnce(&mut dyn Requester, &mut DeviceState) -> R) -> R {
        let features = self.state.features.clone();
        let mut req = RingRequester {
            link: &mut self.link,
            rx: &self.rx,
            backlog: &mut self.backlog,
            features: &features,
            gone: &mut self.gone,
        };
        f(&mut req, &mut self.state)
    }

    fn config(&self) -> Arc<CompiledConfig> {
        self.shared.config.load_full()
    }

    /// `[device."<name or serial>"]` for this device (exact name, "MX Master 4" style
    /// substring, serial, or `"*"`).
    fn device_settings(&self, cfg: &CompiledConfig) -> Option<DeviceSettings> {
        let name = self.state.identity.name.to_lowercase();
        let serial = self.state.id.serial.to_lowercase();
        cfg.source
            .device
            .iter()
            .find(|(k, _)| {
                let k = k.to_lowercase();
                k == name || k == serial || name.contains(&k)
            })
            .or_else(|| cfg.source.device.iter().find(|(k, _)| k.as_str() == "*"))
            .map(|(_, v)| v.clone())
    }

    /// Full resync: read everything, apply desired settings and diversion.
    pub fn resync(&mut self, why: &str) {
        info!("{}: applying configuration ({why})", self.key);
        self.with_dev(|dev, st| st.read_all(dev));
        if let Some(d) = self.state.dpi.reported.as_ref().map(|d| d.dpi) {
            self.link.send(ToDp::SetDpi(d));
        }
        self.link.send(ToDp::SetProfile(
            self.shared
                .profile
                .load(std::sync::atomic::Ordering::Relaxed),
        ));
        self.apply_settings();
        self.apply_diversion(true);
        self.next_verify = Instant::now() + VERIFY_EVERY;
        self.shared.publish(
            "link",
            serde_json::json!({"device": self.key, "connected": true, "why": why}),
        );
    }

    /// Divert what the compiled config needs; undivert what it no longer needs.
    /// `force`: re-send even if we believe it's already set (after reconnect).
    pub fn apply_diversion(&mut self, force: bool) {
        // The data planes' view: no bindings (nothing to divert) while remapping is paused.
        let cfg = self.shared.active.load_full();
        let want: Vec<(u16, bool)> = cfg.engine.diversion().collect();
        let controls = self
            .state
            .controls
            .reported
            .as_ref()
            .map(|c| c.controls.clone())
            .unwrap_or_default();
        let mut plan: Vec<(u16, bool, bool)> = Vec::new(); // cid, divert, raw
        for &(c, raw) in &want {
            match controls.iter().find(|x| x.cid == c) {
                Some(info) if info.divertable => {
                    let raw = raw && info.raw_xy_capable;
                    if force || !self.diverted.contains(&(c, raw)) {
                        plan.push((c, true, raw));
                    }
                }
                Some(_) => warn!(
                    "{}: control 0x{c:04X} is not divertable on this device; bindings on it won't work",
                    self.key
                ),
                None => warn!(
                    "{}: control 0x{c:04X} doesn't exist on this device; bindings on it won't work",
                    self.key
                ),
            }
        }
        for &(c, _) in &self.diverted {
            if !want.iter().any(|(w, _)| *w == c) {
                plan.push((c, false, false));
            }
        }
        if plan.is_empty() {
            return;
        }
        let key = self.key.clone();
        let mut done = Vec::new();
        self.with_dev(|dev, _| {
            for (c, divert, raw) in plan {
                match reprog_controls::set_reporting(dev, c, divert, raw) {
                    Ok(_) => done.push((c, divert, raw)),
                    Err(e) => warn!("{key}: diverting 0x{c:04X} failed: {}", fmt_err(&e)),
                }
            }
        });
        for (c, divert, raw) in done {
            self.diverted.retain(|(d, _)| *d != c);
            if divert {
                self.diverted.push((c, raw));
            }
        }
        self.sync_controls_section();
        debug!("{}: diverted {:04X?}", self.key, self.diverted);
    }

    /// Reflect our diversion in the state shown by `detentctl get`.
    fn sync_controls_section(&mut self) {
        let diverted: Vec<u16> = self.diverted.iter().map(|(c, _)| *c).collect();
        let raw: Vec<u16> = self
            .diverted
            .iter()
            .filter(|(_, r)| *r)
            .map(|(c, _)| *c)
            .collect();
        if let Some(r) = self.state.controls.reported.as_mut() {
            r.diverted = diverted;
            r.raw_xy = raw;
            self.state.controls.generation += 1;
        }
    }

    /// Undo our diversion (shutdown): the mouse behaves as without Detent.
    fn undivert_all(&mut self) {
        let list = std::mem::take(&mut self.diverted);
        if list.is_empty() {
            return;
        }
        let key = self.key.clone();
        self.with_dev(|dev, _| {
            for (c, _) in list {
                if let Err(e) = reprog_controls::set_reporting(dev, c, false, false) {
                    warn!("{key}: restoring 0x{c:04X} failed: {}", fmt_err(&e));
                }
            }
        });
    }

    fn verify(&mut self) {
        self.next_verify = Instant::now() + VERIFY_EVERY;
        let Some(&(cid, _)) = self.diverted.first() else {
            return;
        };
        let r = self.with_dev(|dev, _| reprog_controls::reporting(dev, cid));
        match r {
            Ok(rep) if !rep.diverted => {
                info!("{}: diversion was lost (sleep/reconnect)", self.key);
                self.resync("diversion lost");
            }
            Ok(_) => {}
            Err(e) => debug!("{}: verify failed: {}", self.key, fmt_err(&e)),
        }
    }

    fn on_device_cmd(&mut self, cmd: DeviceCmd) {
        if let DeviceCmd::ChangeHost(t) = cmd {
            // From a binding: the result only goes to the log.
            let _ = self.switch_host(t);
            return;
        }
        let cfg = self.config();
        let cycle = self
            .device_settings(&cfg)
            .and_then(|d| d.dpi_cycle)
            .unwrap_or_else(|| vec![800, 1600, 3200]);
        let key = self.key.clone();
        let r = self.with_dev(|dev, st| -> Result<(), RequestError> {
            match cmd {
                DeviceCmd::SetDpi(d) => adjustable_dpi::set_dpi(dev, d)?,
                DeviceCmd::CycleDpi => {
                    let cur = st.dpi.reported.as_ref().map_or(0, |d| d.dpi);
                    let next = cycle
                        .iter()
                        .copied()
                        .find(|&d| d > cur)
                        .or_else(|| cycle.first().copied())
                        .unwrap_or(cur);
                    adjustable_dpi::set_dpi(dev, next)?;
                    info!("{key}: dpi {next}");
                }
                DeviceCmd::ToggleSmartShift => {
                    let cur = smartshift::SmartShift::read(dev)?;
                    let next = match cur.mode {
                        smartshift::WheelMode::Ratchet => smartshift::WheelMode::FreeSpin,
                        smartshift::WheelMode::FreeSpin => smartshift::WheelMode::Ratchet,
                    };
                    smartshift::set(dev, Some(next), None, None)?;
                }
                DeviceCmd::HapticPulse { waveform, .. } => haptics::play(dev, waveform)?,
                DeviceCmd::ChangeHost(_) => {} // handled above
            }
            if matches!(cmd, DeviceCmd::SetDpi(_) | DeviceCmd::CycleDpi) {
                st.dpi
                    .set_reported(adjustable_dpi::AdjustableDpi::read(dev).ok());
            }
            if matches!(cmd, DeviceCmd::ToggleSmartShift) {
                st.smartshift
                    .set_reported(smartshift::SmartShift::read(dev).ok());
            }
            Ok(())
        });
        if let Err(e) = r {
            warn!(
                "{}: device command {cmd:?} failed: {}",
                self.key,
                fmt_err(&e)
            );
        }
        if let Some(d) = self.state.dpi.reported.as_ref().map(|d| d.dpi) {
            self.link.send(ToDp::SetDpi(d));
        }
    }

    /// Only device commands reach the worker; the data plane sends everything else to
    /// the dispatch thread (`dispatch.rs`).
    fn on_control(&mut self, c: ControlOut) {
        if let ControlOut::Device(cmd) = c {
            self.on_device_cmd(cmd);
        }
    }

    fn on_notification(&mut self, f: Frame) {
        let fi = f.feature_index();
        let idx = |id| self.state.features.index_of(id);
        if Some(fi) == idx(FeatureId::WIRELESS_DEVICE_STATUS) && f.function() == 0 {
            let p = f.params();
            // [status, request, reason]: request 1 = software reconfiguration needed
            if p.first() == Some(&1) || p.get(1) == Some(&1) {
                info!(
                    "{}: reconnected (0x1D4B {:02X?})",
                    self.key,
                    &p[..3.min(p.len())]
                );
                self.resync_at = Some(Instant::now() + RESYNC_DELAY);
            }
        } else if Some(fi) == idx(FeatureId::UNIFIED_BATTERY)
            && let Some(cur) = self.state.battery.reported.as_mut()
            && <unified_battery::UnifiedBattery as Feature>::on_notification(&f, cur).is_some()
        {
            self.state.battery.generation += 1;
            debug!("{}: battery {:?}", self.key, self.state.battery.reported);
        }
    }

    fn on_kernel_traffic(&mut self, f: Frame) {
        // The kernel re-sets the wheel mode (fn 2 setWheelMode) on connect; if we manage
        // the wheel's invert, re-apply after it.
        if Some(f.feature_index()) == self.state.features.index_of(FeatureId::HIRES_WHEEL)
            && f.function() == 2
            && self.state.wheel.desired.is_some()
        {
            self.resync_at.get_or_insert(Instant::now() + RESYNC_DELAY);
        }
    }

    fn on_cmd(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Reload => {
                self.link.send(ToDp::ReleaseAll);
                self.overrides.clear();
                self.apply_settings();
                self.apply_diversion(false);
            }
            Cmd::Rebind => {
                // Remapping paused/resumed: settings and live edits stay as they are.
                self.link.send(ToDp::ReleaseAll);
                self.apply_diversion(false);
            }
            Cmd::Profile(p) => self.link.send(ToDp::SetProfile(p)),
            Cmd::Get(reply) => {
                // Battery is cheap to refresh and the one value that changes on its own.
                let fresh = self.with_dev(|dev, _| unified_battery::UnifiedBattery::read(dev));
                if let Ok(b) = fresh {
                    self.state.battery.set_reported(Some(b));
                }
                let _ = reply.try_send(serde_json::to_value(&self.state).unwrap_or_default());
            }
            Cmd::Set { path, value, reply } => {
                let r = self.set(&path, &value);
                let _ = reply.try_send(r);
            }
            Cmd::Unset { path, reply } => {
                let r = self.unset(&path);
                let _ = reply.try_send(r);
            }
            Cmd::Raw {
                feature,
                function,
                params,
                reply,
            } => {
                let r = self.with_dev(|dev, st| {
                    let fi = st
                        .features
                        .index_of(FeatureId(feature))
                        .ok_or_else(|| format!("feature 0x{feature:04X} not on this device"))?;
                    if feature == FeatureId::WIRELESS_DEVICE_STATUS.0 {
                        return Err(
                            "refusing to send requests to 0x1D4B (kernel reconnect logic)"
                                .to_string(),
                        );
                    }
                    dev.request(fi, function, &params)
                        .map(|f| f.params().to_vec())
                        .map_err(|e| fmt_err(&e))
                });
                let _ = reply.try_send(r);
            }
            Cmd::Haptic(w) => self.on_device_cmd(DeviceCmd::HapticPulse {
                waveform: w,
                intensity: 0,
            }),
            Cmd::Device(c) => self.on_device_cmd(c),
            Cmd::Host { op, reply } => {
                let r = self.on_host(op);
                let _ = reply.try_send(r);
            }
            Cmd::EmitKeys(k) => self.link.send(ToDp::EmitKeys(k)),
            Cmd::Lease(l) => self.link.send(ToDp::Lease(l)),
            Cmd::Shutdown => return false,
        }
        true
    }

    fn next_timeout(&self) -> Duration {
        let now = Instant::now();
        let mut t = self.next_verify;
        if let Some(r) = self.resync_at {
            t = t.min(r);
        }
        t.saturating_duration_since(now)
    }

    fn handle(&mut self, m: WorkerMsg) -> bool {
        match m {
            WorkerMsg::Dp(FromDp::Control(c)) => self.on_control(c),
            WorkerMsg::Dp(FromDp::Notification(f)) => self.on_notification(f),
            WorkerMsg::Dp(FromDp::KernelTraffic(f)) => self.on_kernel_traffic(f),
            WorkerMsg::Dp(FromDp::Grabbed(g)) => debug!("{}: evdev grab {g}", self.key),
            WorkerMsg::Dp(FromDp::Response(_)) => {}
            WorkerMsg::Dp(FromDp::Gone(r)) => {
                self.gone = Some(r);
                return false;
            }
            WorkerMsg::Cmd(c) => return self.on_cmd(c),
        }
        true
    }

    /// Worker main loop. Returns when the device is gone or on shutdown.
    pub fn run(mut self) {
        self.resync("attached");
        loop {
            if self.gone.is_some() {
                break;
            }
            let m = match self.backlog.pop_front() {
                Some(m) => Some(m),
                None => match self.rx.recv_timeout(self.next_timeout()) {
                    Ok(m) => Some(m),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
            };
            if let Some(m) = m
                && !self.handle(m)
            {
                break;
            }
            let now = Instant::now();
            if self.resync_at.is_some_and(|t| t <= now) {
                self.resync_at = None;
                self.resync("reconnected");
            } else if self.next_verify <= now {
                self.verify();
            }
        }
        if self.gone.is_none() {
            // Orderly shutdown: give the buttons back, then stop the data plane.
            self.undivert_all();
            self.link.send(ToDp::Stop);
        }
        let reason = self.dp.take().and_then(|h| h.join().ok()).unwrap_or("?");
        info!("{}: detached ({reason})", self.key);
        self.shared.detached(&self.key);
    }
}
