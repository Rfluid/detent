//! Reading the daemon: one `status` (+ `device.get` per device for the battery) per
//! refresh, an `events.subscribe` connection that triggers refreshes, and `systemctl`
//! for "failed" when nothing answers. Only the public IPC socket, like detentctl.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Duration;

use detent_hidpp_transport::discover::{self, Bus};
use detent_ipc::types::Status;
use detent_ipc::{Client, method};
use serde_json::{Value, json};

use crate::model::{Daemon, DaemonView, DeviceView};

pub const UNIT: &str = "detent.service";

/// Events that can change what the tray shows.
const TOPICS: [&str; 4] = ["remap", "link", "config", "plugin"];

/// Wake-ups for the refresh loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kick {
    /// Something changed (an event, an action finished): refresh now.
    Refresh,
}

/// Reconnect delays while the socket is absent: 1, 2, 4, 8, then 15 s.
#[derive(Debug, Default)]
pub struct Backoff(u32);

impl Backoff {
    pub fn next(&mut self) -> Duration {
        let s = [1, 2, 4, 8, 15][self.0.min(4) as usize];
        self.0 = self.0.saturating_add(1);
        Duration::from_secs(s)
    }

    pub fn reset(&mut self) {
        self.0 = 0;
    }
}

/// `systemctl --user is-failed detent.service` → true only for `failed`.
pub fn service_failed() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-failed", "--quiet", UNIT])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// hidraw node → link name, from the sysfs scan detentctl also uses.
fn links() -> HashMap<String, String> {
    discover::logitech()
        .unwrap_or_default()
        .into_iter()
        .map(|d| {
            let link = match d.bus {
                Bus::Bluetooth => "Bluetooth".to_string(),
                Bus::Usb => "USB".to_string(),
                Bus::Other(c) => format!("bus {c:#06x}"),
            };
            (d.node.display().to_string(), link)
        })
        .collect()
}

/// `battery.reported.percent` out of a `device.get` result.
pub fn battery_of(state: &Value) -> Option<u8> {
    state["battery"]["reported"]["percent"]
        .as_u64()
        .and_then(|p| u8::try_from(p).ok())
}

/// One full read of the daemon. `Down` when nothing answers.
pub fn read(socket: &Path) -> Daemon {
    let Ok(mut c) = Client::connect_to(socket) else {
        return Daemon::Down {
            failed: service_failed(),
        };
    };
    let Ok(s) = c.status() else {
        return Daemon::Down { failed: false };
    };
    Daemon::Up(view(&mut c, s))
}

fn view(c: &mut Client, s: Status) -> DaemonView {
    let links = links();
    let devices = s
        .devices
        .iter()
        .map(|d| DeviceView {
            name: d.name.clone(),
            link: links.get(&d.node).cloned(),
            battery: c
                .call(method::DEVICE_GET, json!({ "device": d.id }))
                .ok()
                .as_ref()
                .and_then(battery_of),
        })
        .collect();
    DaemonView {
        version: s.version,
        config_path: PathBuf::from(s.config_path),
        config_error: s.config_error,
        remap_enabled: s.remap_enabled,
        devices,
    }
}

/// Subscribe to events and kick the refresh loop on each one; reconnect with backoff
/// while the daemon is away. Runs forever on its own thread; ends when `kick` is closed.
pub fn watch_events(socket: PathBuf, kick: Sender<Kick>) {
    let topics: Vec<String> = TOPICS.iter().map(|t| t.to_string()).collect();
    let mut backoff = Backoff::default();
    loop {
        if let Ok(c) = Client::connect_to(&socket) {
            backoff.reset();
            // Connected: the daemon may have just started.
            if kick.send(Kick::Refresh).is_err() {
                return;
            }
            let _ = c.watch(&topics, |_| kick.send(Kick::Refresh).is_ok());
            // The daemon went away (or restarted): show it.
            if kick.send(Kick::Refresh).is_err() {
                return;
            }
        }
        std::thread::sleep(backoff.next());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_caps() {
        let mut b = Backoff::default();
        let v: Vec<u64> = (0..7).map(|_| b.next().as_secs()).collect();
        assert_eq!(v, [1, 2, 4, 8, 15, 15, 15]);
        b.reset();
        assert_eq!(b.next().as_secs(), 1);
    }

    #[test]
    fn battery_from_device_state() {
        let st = json!({"battery": {"reported": {"percent": 85, "level": "good"}}});
        assert_eq!(battery_of(&st), Some(85));
        assert_eq!(battery_of(&json!({"battery": {"reported": null}})), None);
        assert_eq!(battery_of(&json!({})), None);
    }

    #[test]
    fn absent_socket_reads_as_down() {
        let p = std::env::temp_dir().join(format!("detent-tray-none-{}.sock", std::process::id()));
        assert!(matches!(read(&p), Daemon::Down { .. }));
    }
}
