//! Messages from a connected plugin: capability checks, requests, input leases.

use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::{Duration, Instant};

use detent_bus::{
    Capability, DaemonToPlugin, DeviceCommand, LeaseRevokeReason, LeaseTarget, PluginToDaemon,
};
use detent_engine::engine::Lease;
use serde_json::Value;
use tracing::{debug, info, warn};

use super::{ActiveLease, Bus};
use crate::msg::{Cmd, WorkerMsg};
use crate::shared::{Shared, lock};

/// Tell every device's data plane about the lease (inactive = end it).
pub(super) fn send_lease(shared: &Shared, lease: Lease) {
    for d in lock(&shared.devices).values() {
        let _ = d.tx.try_send(WorkerMsg::Cmd(Cmd::Lease(lease)));
    }
}

impl Bus {
    #[expect(
        clippy::too_many_lines,
        reason = "predates the lint; split it when next changed (ADR 0008)"
    )]
    pub(super) fn handle(
        &self,
        shared: &Arc<Shared>,
        name: &str,
        fd: &Arc<OwnedFd>,
        m: PluginToDaemon,
    ) {
        let has = |c: Capability| {
            lock(&self.state)
                .conns
                .get(name)
                .is_some_and(|x| Arc::ptr_eq(&x.fd, fd) && x.has(&c))
        };
        let deny = |what: &str, cap: Capability| {
            warn!(
                "plugin `{name}`: {what} needs capability `{}`; ignored",
                cap.name()
            );
        };
        match m {
            PluginToDaemon::Hello(_) => {}
            PluginToDaemon::Pong { .. } => {
                if let Some(c) = lock(&self.state).conns.get_mut(name) {
                    c.missed_pings = 0;
                }
            }
            PluginToDaemon::Subscribe(topics) => {
                if !has(Capability::EventsSubscribe) {
                    return deny("Subscribe", Capability::EventsSubscribe);
                }
                if let Some(c) = lock(&self.state).conns.get_mut(name) {
                    c.topics = topics;
                }
            }
            PluginToDaemon::SetContext { key, value } => {
                if !has(Capability::ContextPublish) {
                    return deny("SetContext", Capability::ContextPublish);
                }
                shared.set_context(&key, &value);
            }
            PluginToDaemon::Invoke {
                action,
                params_json,
            } => {
                if !has(Capability::ActionsInvoke) {
                    return deny("Invoke", Capability::ActionsInvoke);
                }
                if action == detent_bus::builtin::KEYS && !has(Capability::InputEmit) {
                    return deny("the `keys` action", Capability::InputEmit);
                }
                let trigger = format!("plugin:{name}");
                if let Err(e) = shared.invoke_named(&action, &params_json, &trigger) {
                    warn!("plugin `{name}`: {e}");
                }
            }
            PluginToDaemon::Device(cmd) => {
                let cap = match cmd {
                    DeviceCommand::HapticPulse { .. } => Capability::DeviceHaptics,
                    _ => Capability::DeviceControl,
                };
                if !has(cap.clone()) {
                    return deny("device command", cap);
                }
                let c = match cmd {
                    DeviceCommand::HapticPulse { waveform, .. } => Cmd::Haptic(waveform),
                    DeviceCommand::SetDpi(d) => {
                        Cmd::Device(detent_engine::action::DeviceCmd::SetDpi(d))
                    }
                    DeviceCommand::CycleDpi => {
                        Cmd::Device(detent_engine::action::DeviceCmd::CycleDpi)
                    }
                    DeviceCommand::ToggleSmartShift => {
                        Cmd::Device(detent_engine::action::DeviceCmd::ToggleSmartShift)
                    }
                };
                if let Some(d) = lock(&shared.devices).values().next() {
                    let _ = d.tx.try_send(WorkerMsg::Cmd(c));
                }
            }
            PluginToDaemon::SetState { path, value_json } => {
                if !has(Capability::StateWrite) {
                    return deny("SetState", Capability::StateWrite);
                }
                let value = serde_json::from_str::<Value>(&value_json)
                    .map(|v| match v {
                        Value::String(s) => s,
                        other => other.to_string(),
                    })
                    .unwrap_or(value_json);
                if let Some(d) = lock(&shared.devices).values().next() {
                    let (tx, _rx) = std::sync::mpsc::sync_channel(1);
                    let _ = d.tx.try_send(WorkerMsg::Cmd(Cmd::Set {
                        path,
                        value,
                        reply: tx,
                    }));
                }
            }
            PluginToDaemon::LeaseRequest {
                id,
                targets,
                max_ms,
            } => {
                let r = self.grant_lease(shared, name, id, &targets, max_ms, &has);
                let msg = match r {
                    Ok(()) => DaemonToPlugin::LeaseGranted { id },
                    Err(reason) => {
                        debug!("plugin `{name}`: lease {id} denied: {reason}");
                        DaemonToPlugin::LeaseDenied { id, reason }
                    }
                };
                self.send_to(name, &msg);
            }
            PluginToDaemon::LeaseRelease { id } => {
                let mine = {
                    let mut st = lock(&self.state);
                    let m = st
                        .lease
                        .as_ref()
                        .is_some_and(|l| l.plugin == name && l.id == id);
                    if m {
                        st.lease = None;
                    }
                    m
                };
                if mine {
                    send_lease(shared, Lease::default());
                }
            }
        }
    }

    fn grant_lease(
        &self,
        shared: &Arc<Shared>,
        name: &str,
        id: u32,
        targets: &[LeaseTarget],
        max_ms: u32,
        has: &dyn Fn(Capability) -> bool,
    ) -> Result<(), String> {
        let cfg = shared.config.load();
        let mut lease = Lease::default();
        for t in targets {
            match t {
                LeaseTarget::Pointer => {
                    if !has(Capability::LeasePointer) {
                        return Err("needs capability lease.pointer".into());
                    }
                    lease.pointer = true;
                }
                LeaseTarget::Buttons => {
                    if !has(Capability::LeaseButtons) {
                        return Err("needs capability lease.buttons".into());
                    }
                    lease.buttons = true;
                }
                LeaseTarget::Control(n) => {
                    if !has(Capability::LeaseButtons) {
                        return Err("needs capability lease.buttons".into());
                    }
                    let end = n.0.iter().position(|b| *b == 0).unwrap_or(n.0.len());
                    let cname = String::from_utf8_lossy(&n.0[..end]).to_string();
                    let slot = detent_hidpp_features::cid::from_config_name(&cname)
                        .and_then(|c| cfg.engine.slot_for_cid(c))
                        .ok_or_else(|| {
                            format!("control `{cname}` is not diverted (bind it to something)")
                        })?;
                    lease.controls |= 1 << slot.0;
                }
                LeaseTarget::Wheel | LeaseTarget::Thumbwheel => {
                    return Err("wheel leases are not supported yet".into());
                }
            }
        }
        let max = cfg.source.daemon.max_lease_ms.min(u64::from(max_ms.max(1)));
        lease.until = Some(Instant::now() + Duration::from_millis(max));
        {
            let mut st = lock(&self.state);
            if let Some(l) = &st.lease
                && l.plugin != name
            {
                return Err(format!("input is leased by `{}`", l.plugin));
            }
            st.lease = Some(ActiveLease {
                plugin: name.to_string(),
                id,
            });
        }
        if lock(&shared.devices).is_empty() {
            lock(&self.state).lease = None;
            return Err("no device attached".into());
        }
        send_lease(shared, lease);
        Ok(())
    }

    /// Revoke the active lease (plugin gone, config reload, device disconnect).
    pub fn revoke(&self, shared: &Shared, reason: LeaseRevokeReason) {
        let Some(l) = lock(&self.state).lease.take() else {
            return;
        };
        send_lease(shared, Lease::default());
        self.send_to(
            &l.plugin,
            &DaemonToPlugin::LeaseRevoked { id: l.id, reason },
        );
        info!("lease of `{}` revoked ({reason:?})", l.plugin);
    }
}
