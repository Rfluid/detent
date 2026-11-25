//! The dispatch thread: runs `exec`, named (plugin) actions and profile switches handed
//! over by the data planes. Separate from the device workers so an action never waits
//! behind a slow HID++ round trip.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use detent_engine::ControlOut;
use serde_json::json;
use tracing::warn;

use crate::exec;
use crate::shared::Shared;

pub fn run(shared: Arc<Shared>, rx: Receiver<ControlOut>) {
    while let Ok(msg) = rx.recv() {
        let cfg = shared.config.load();
        match msg {
            ControlOut::Exec(id) => match cfg.exec.get(id.0 as usize) {
                Some(cmd) => {
                    shared.publish("exec", json!({ "cmd": cmd }));
                    exec::spawn(cmd);
                }
                None => warn!("exec #{} not in the current config", id.0),
            },
            ControlOut::Action { action, delta, .. } => match cfg.actions.get(action.0 as usize) {
                Some(a) => shared.invoke_action(a, delta),
                None => warn!("action #{} not in the current config", action.0),
            },
            ControlOut::Profile(p) => {
                shared
                    .profile
                    .store(p, std::sync::atomic::Ordering::Relaxed);
                shared.broadcast(|| crate::msg::Cmd::Profile(p));
            }
            c @ (ControlOut::LeaseMotion { .. }
            | ControlOut::LeaseButton { .. }
            | ControlOut::LeaseEnded { .. }) => {
                if let Some(s) = crate::shared::lock(&shared.actions).clone() {
                    s.lease_input(c);
                }
            }
            // Device commands go to the device's own worker.
            ControlOut::Device(_) => {}
        }
    }
}
