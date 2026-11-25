//! Easy-Switch on the worker: list, switch and rename hosts (`detent_hidpp_features::hosts`).
//! Switching is a command, never desired state: nothing here is re-applied on reconnect.
//! A switch drops the link, so the data plane ends and the worker detaches right after.

use detent_engine::action::HostTarget;
use detent_hidpp_features::RequestError;
use detent_hidpp_features::hosts::{self, Target};
use detent_ipc::code;
use serde_json::{Value, json};
use tracing::{info, warn};

use super::{Worker, fmt_err};
use crate::msg::HostOp;

type HostResult = Result<Value, (i32, String)>;

fn target(t: HostTarget) -> Target {
    match t {
        HostTarget::Slot(s) => Target::Slot(s),
        HostTarget::Next => Target::Next,
        HostTarget::Prev => Target::Prev,
    }
}

/// Bad host numbers and names are the caller's input; the rest is the device.
fn rpc_err(e: &RequestError) -> (i32, String) {
    match e {
        RequestError::OutOfRange(m) => (code::INVALID_PARAMS, m.clone()),
        RequestError::Unsupported(_) => (
            code::DEVICE,
            "this device has no Easy-Switch hosts".to_string(),
        ),
        other => (code::HIDPP, fmt_err(other)),
    }
}

impl Worker {
    pub(super) fn on_host(&mut self, op: HostOp) -> HostResult {
        match op {
            HostOp::List => {
                let st = self
                    .with_dev(|dev, _| hosts::read(dev))
                    .map_err(|e| rpc_err(&e))?;
                self.state.hosts.set_reported(Some(st));
                Ok(self.hosts_json())
            }
            HostOp::Switch(t) => self.switch_host(t),
            HostOp::Rename { slot, name } => {
                let r = self.with_dev(|dev, _| hosts::rename(dev, slot, &name));
                let st = r.map_err(|e| rpc_err(&e))?;
                info!("{}: host {} renamed to {name:?}", self.key, slot + 1);
                self.state.hosts.set_reported(Some(st));
                self.shared.publish(
                    "host",
                    json!({"device": self.key, "renamed": slot + 1, "name": name}),
                );
                Ok(self.hosts_json())
            }
        }
    }

    /// `{"switched": bool, "from": n, "to": n}` with 1-based hosts.
    pub(super) fn switch_host(&mut self, t: HostTarget) -> HostResult {
        let from = self.state.hosts.reported.as_ref().map(|h| h.current + 1);
        match self.with_dev(|dev, _| hosts::switch(dev, target(t))) {
            Ok(Some(to)) => {
                info!("{}: switching to host {} (Easy-Switch)", self.key, to + 1);
                let v = json!({"switched": true, "from": from, "to": to + 1});
                let mut ev = v.clone();
                ev["device"] = json!(self.key);
                self.shared.publish("host", ev);
                Ok(v)
            }
            Ok(None) => {
                let cur = from.map_or_else(|| "?".to_string(), |f| f.to_string());
                info!("{}: already on host {cur}", self.key);
                Ok(json!({"switched": false, "from": from, "to": from}))
            }
            // A host number the device doesn't have is the caller's mistake, not ours.
            Err(e @ RequestError::OutOfRange(_)) => Err(rpc_err(&e)),
            Err(e) => {
                warn!("{}: host switch failed: {}", self.key, fmt_err(&e));
                Err(rpc_err(&e))
            }
        }
    }

    fn hosts_json(&self) -> Value {
        serde_json::to_value(&self.state.hosts.reported).unwrap_or_default()
    }
}
