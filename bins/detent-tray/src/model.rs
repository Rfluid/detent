//! What the tray knows (a [`Snapshot`]) and everything derived from it that the panel
//! shows: the state, the icon name, the SNI status and the tooltip. Pure, so it is tested
//! without D-Bus or a daemon.

use std::path::PathBuf;

/// One Easy-Switch slot, for the "Switch to" rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostView {
    /// 1-based, as printed on the mouse.
    pub number: u8,
    pub name: Option<String>,
    /// `Some(false)`: nothing paired there (the row is disabled).
    pub paired: Option<bool>,
    /// The slot this computer uses.
    pub current: bool,
}

/// One attached device as the header and tooltip show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceView {
    /// The daemon's device id (`hosts.switch` targets it).
    pub id: String,
    /// Easy-Switch slots; empty when the device has none.
    pub hosts: Vec<HostView>,
    pub name: String,
    /// `Bluetooth`, `USB`, ... (from sysfs); `None` when unknown.
    pub link: Option<String>,
    /// Percent as the device reports it; `None` when it reports no battery.
    pub battery: Option<u8>,
}

/// A running daemon, from `status` (plus `device.get` per device for the battery).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonView {
    pub version: String,
    pub config_path: PathBuf,
    pub config_error: Option<String>,
    pub remap_enabled: bool,
    pub devices: Vec<DeviceView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Daemon {
    /// Nothing answers on the socket. `failed`: systemd reports `detent.service` failed.
    Down {
        failed: bool,
    },
    Up(DaemonView),
}

/// A newer release than this tray (from the update check).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateView {
    /// `v1.2.3`
    pub tag: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub daemon: Daemon,
    pub update: Option<UpdateView>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            daemon: Daemon::Down { failed: false },
            update: None,
        }
    }
}

/// The icon's state. Priority when several hold: error, stopped, paused, no mouse, then
/// running (an update only decorates running and no mouse).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Error,
    Stopped,
    Paused,
    NoMouse,
    Running,
}

impl Snapshot {
    pub fn state(&self) -> State {
        match &self.daemon {
            Daemon::Down { failed: true } => State::Error,
            Daemon::Down { failed: false } => State::Stopped,
            Daemon::Up(d) if d.config_error.is_some() => State::Error,
            Daemon::Up(d) if !d.remap_enabled => State::Paused,
            Daemon::Up(d) if d.devices.is_empty() => State::NoMouse,
            Daemon::Up(_) => State::Running,
        }
    }

    pub fn running(&self) -> Option<&DaemonView> {
        match &self.daemon {
            Daemon::Up(d) => Some(d),
            Daemon::Down { .. } => None,
        }
    }

    /// Named icon in the hicolor theme (packaging/icons/).
    pub fn icon_name(&self) -> &'static str {
        match (self.state(), self.update.is_some()) {
            (State::Error, _) => "detent-tray-error-symbolic",
            (State::Stopped, _) => "detent-tray-stopped-symbolic",
            (State::Paused, _) => "detent-tray-paused-symbolic",
            (State::NoMouse | State::Running, true) => "detent-tray-update-symbolic",
            (State::NoMouse, false) => "detent-tray-nomouse-symbolic",
            (State::Running, false) => "detent-tray-running-symbolic",
        }
    }

    /// SNI `Status`: only errors ask for attention.
    pub fn needs_attention(&self) -> bool {
        self.state() == State::Error
    }

    /// SNI tooltip `(title, description)`.
    pub fn tooltip(&self) -> (String, String) {
        let update = self
            .update
            .as_ref()
            .map(|u| format!(" Update {} available.", u.tag))
            .unwrap_or_default();
        let (title, desc) = match (&self.daemon, self.state()) {
            (Daemon::Down { .. }, State::Error) => (
                "Detent failed",
                "detent.service failed. Choose Start daemon, or View logs for why.".to_string(),
            ),
            (Daemon::Down { .. }, _) => (
                "Detent is not running",
                "Open the menu and choose Start daemon.".to_string(),
            ),
            (Daemon::Up(d), State::Error) => (
                "Detent, config error",
                format!(
                    "Reload failed: the previous config is still active. {}",
                    first_line(d.config_error.as_deref().unwrap_or(""))
                ),
            ),
            (Daemon::Up(_), State::Paused) => (
                "Detent, paused",
                "Remapping is off. Middle-click to resume.".to_string(),
            ),
            (Daemon::Up(_), State::NoMouse) => ("Detent", format!("No mouse connected.{update}")),
            (Daemon::Up(d), _) => (
                "Detent",
                format!("{}. Remapping on.{update}", devices_line(&d.devices)),
            ),
        };
        (title.to_string(), desc.trim_end().to_string())
    }

    /// Middle click: toggle pause when the daemon runs; `None` otherwise.
    pub fn middle_click(&self) -> Option<bool> {
        self.running().map(|d| !d.remap_enabled)
    }
}

/// `MX Master 4, connected, 85%` per device, joined with `; `.
pub fn devices_line(devices: &[DeviceView]) -> String {
    devices
        .iter()
        .map(|d| match d.battery {
            Some(b) => format!("{}, connected, {b}%", d.name),
            None => format!("{}, connected", d.name),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// `Connected, Bluetooth, 85%` (the header's second row).
pub fn device_detail(d: &DeviceView) -> String {
    let mut s = String::from("Connected");
    if let Some(l) = &d.link {
        s.push_str(", ");
        s.push_str(l);
    }
    if let Some(b) = d.battery {
        s.push_str(&format!(", {b}%"));
    }
    s
}

/// The first line of an error, cut to a tooltip-friendly length.
pub fn first_line(s: &str) -> String {
    const MAX: usize = 160;
    let l = s.lines().next().unwrap_or("").trim();
    if l.chars().count() <= MAX {
        return l.to_string();
    }
    let mut out: String = l.chars().take(MAX - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn mx(battery: Option<u8>) -> DeviceView {
        DeviceView {
            id: "B042:serial".into(),
            hosts: Vec::new(),
            name: "MX Master 4".into(),
            link: Some("Bluetooth".into()),
            battery,
        }
    }

    pub fn up(devices: Vec<DeviceView>, remap: bool, err: Option<&str>) -> Snapshot {
        Snapshot {
            daemon: Daemon::Up(DaemonView {
                version: "0.0.1".into(),
                config_path: "/home/u/.config/detent/config.toml".into(),
                config_error: err.map(str::to_string),
                remap_enabled: remap,
                devices,
            }),
            update: None,
        }
    }

    pub fn with_update(mut s: Snapshot) -> Snapshot {
        s.update = Some(UpdateView {
            tag: "v0.2.0".into(),
            url: "https://github.com/Rfluid/detent/releases/tag/v0.2.0".into(),
        });
        s
    }

    #[test]
    fn priority_error_stopped_paused_nomouse_running() {
        let failed = Snapshot {
            daemon: Daemon::Down { failed: true },
            update: None,
        };
        assert_eq!(failed.state(), State::Error);
        assert_eq!(Snapshot::default().state(), State::Stopped);
        // A config error beats paused and no mouse.
        assert_eq!(up(vec![], false, Some("bad")).state(), State::Error);
        // Paused beats no mouse.
        assert_eq!(up(vec![], false, None).state(), State::Paused);
        assert_eq!(up(vec![], true, None).state(), State::NoMouse);
        assert_eq!(up(vec![mx(Some(85))], true, None).state(), State::Running);
        // An update never changes the state, only the running / no-mouse icon.
        assert_eq!(with_update(up(vec![], false, None)).state(), State::Paused);
    }

    #[test]
    fn icon_names() {
        let n = |s: Snapshot| s.icon_name();
        assert_eq!(
            n(up(vec![mx(None)], true, None)),
            "detent-tray-running-symbolic"
        );
        assert_eq!(n(up(vec![], true, None)), "detent-tray-nomouse-symbolic");
        assert_eq!(
            n(up(vec![mx(None)], false, None)),
            "detent-tray-paused-symbolic"
        );
        assert_eq!(n(Snapshot::default()), "detent-tray-stopped-symbolic");
        assert_eq!(n(up(vec![], true, Some("x"))), "detent-tray-error-symbolic");
        assert_eq!(
            n(with_update(up(vec![mx(None)], true, None))),
            "detent-tray-update-symbolic"
        );
        assert_eq!(
            n(with_update(up(vec![], true, None))),
            "detent-tray-update-symbolic"
        );
        assert_eq!(
            n(with_update(up(vec![mx(None)], false, None))),
            "detent-tray-paused-symbolic"
        );
        assert_eq!(
            n(with_update(Snapshot::default())),
            "detent-tray-stopped-symbolic"
        );
    }

    #[test]
    fn attention_only_on_error() {
        assert!(up(vec![], true, Some("x")).needs_attention());
        assert!(!Snapshot::default().needs_attention());
        assert!(!up(vec![], false, None).needs_attention());
    }

    #[test]
    fn tooltips() {
        let t = |s: Snapshot| s.tooltip();
        assert_eq!(
            t(up(vec![mx(Some(85))], true, None)),
            (
                "Detent".into(),
                "MX Master 4, connected, 85%. Remapping on.".into()
            )
        );
        assert_eq!(
            t(up(vec![], true, None)),
            ("Detent".into(), "No mouse connected.".into())
        );
        assert_eq!(
            t(up(vec![mx(Some(85))], false, None)),
            (
                "Detent, paused".into(),
                "Remapping is off. Middle-click to resume.".into()
            )
        );
        assert_eq!(
            t(Snapshot::default()),
            (
                "Detent is not running".into(),
                "Open the menu and choose Start daemon.".into()
            )
        );
        let (title, desc) = t(up(vec![], true, Some("line 3: unknown key `x`\nmore")));
        assert_eq!(title, "Detent, config error");
        assert_eq!(
            desc,
            "Reload failed: the previous config is still active. line 3: unknown key `x`"
        );
        assert_eq!(
            t(with_update(up(vec![mx(Some(85))], true, None))).1,
            "MX Master 4, connected, 85%. Remapping on. Update v0.2.0 available."
        );
        assert_eq!(
            t(with_update(up(vec![], true, None))).1,
            "No mouse connected. Update v0.2.0 available."
        );
        let failed = Snapshot {
            daemon: Daemon::Down { failed: true },
            update: None,
        };
        assert_eq!(t(failed).0, "Detent failed");
        // Without a battery the percentage is left out.
        assert_eq!(
            t(up(vec![mx(None)], true, None)).1,
            "MX Master 4, connected. Remapping on."
        );
    }

    #[test]
    fn middle_click_toggles_only_when_running() {
        assert_eq!(up(vec![], true, None).middle_click(), Some(false));
        assert_eq!(up(vec![], false, None).middle_click(), Some(true));
        assert_eq!(Snapshot::default().middle_click(), None);
    }

    #[test]
    fn details_and_first_line() {
        assert_eq!(device_detail(&mx(Some(85))), "Connected, Bluetooth, 85%");
        let bare = DeviceView {
            id: "x".into(),
            hosts: Vec::new(),
            name: "x".into(),
            link: None,
            battery: None,
        };
        assert_eq!(device_detail(&bare), "Connected");
        assert_eq!(first_line(&"a".repeat(500)).chars().count(), 160);
    }
}
