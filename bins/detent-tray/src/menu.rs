//! The tray menu as plain data (labels use `&` for the mnemonic, as Qt does), built from a
//! [`Snapshot`]. `sni.rs` turns it into dbusmenu items. Pure, so tested without D-Bus.

use std::path::{Path, PathBuf};

use crate::model::{Daemon, Snapshot, device_detail};

/// What a menu row (or a click on the icon) asks the action thread to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// `remap.set {enabled}`.
    SetRemap(bool),
    Reload,
    /// `systemctl --user start` (daemon down) or `restart` (daemon up).
    StartDaemon {
        restart: bool,
    },
    OpenConfig(PathBuf),
    ViewLogs,
    OpenUrl(String),
    CheckUpdates,
    About,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// A disabled informational row (no mnemonic processing).
    Header(String),
    Separator,
    Item {
        label: String,
        enabled: bool,
        action: Action,
    },
}

fn item(label: &str, enabled: bool, action: Action) -> Entry {
    Entry::Item {
        label: label.to_string(),
        enabled,
        action,
    }
}

/// Header rows: one name + detail pair per device, or the daemon's state.
fn headers(s: &Snapshot) -> Vec<Entry> {
    let mut out = Vec::new();
    match &s.daemon {
        Daemon::Down { failed } => {
            out.push(Entry::Header("Detent".into()));
            out.push(Entry::Header(
                if *failed {
                    "Daemon failed"
                } else {
                    "Daemon stopped"
                }
                .into(),
            ));
        }
        Daemon::Up(d) => {
            let paused = if d.remap_enabled { "" } else { ". Paused" };
            if d.devices.is_empty() {
                out.push(Entry::Header("No mouse".into()));
                out.push(Entry::Header(format!("Not connected{paused}")));
            }
            for dev in &d.devices {
                out.push(Entry::Header(dev.name.clone()));
                out.push(Entry::Header(format!("{}{paused}", device_detail(dev))));
            }
            if d.config_error.is_some() {
                out.push(Entry::Header("Config error: previous config active".into()));
            }
        }
    }
    out
}

/// The whole menu. `default_config` is used when the daemon can't tell its config path.
pub fn build(s: &Snapshot, default_config: &Path) -> Vec<Entry> {
    let up = s.running();
    let mut m = headers(s);
    m.push(Entry::Separator);
    match up {
        Some(d) if !d.remap_enabled => {
            m.push(item("Re&sume remapping", true, Action::SetRemap(true)))
        }
        Some(_) => m.push(item("&Pause remapping", true, Action::SetRemap(false))),
        None => m.push(item("&Pause remapping", false, Action::SetRemap(false))),
    }
    m.push(Entry::Separator);
    m.push(item("&Reload config", up.is_some(), Action::Reload));
    m.push(match up {
        Some(_) => item(
            "Res&tart daemon",
            true,
            Action::StartDaemon { restart: true },
        ),
        None => item(
            "S&tart daemon",
            true,
            Action::StartDaemon { restart: false },
        ),
    });
    m.push(Entry::Separator);
    let config = up
        .map(|d| d.config_path.clone())
        .unwrap_or_else(|| default_config.to_path_buf());
    m.push(item("&Open config…", true, Action::OpenConfig(config)));
    m.push(item("View &logs…", true, Action::ViewLogs));
    m.push(Entry::Separator);
    if let Some(u) = &s.update {
        // No mnemonic: the other letters stay put whether or not this row is there.
        m.push(item(
            &format!("Update available: {}…", u.tag),
            true,
            Action::OpenUrl(u.url.clone()),
        ));
    }
    // The current version rides along, as in aura's update button.
    m.push(item(&check_label(), true, Action::CheckUpdates));
    m.push(item("&About Detent", true, Action::About));
    m.push(item(
        SPONSOR_LABEL,
        true,
        Action::OpenUrl(detent_update::sponsor::SPONSOR_URL.into()),
    ));
    m.push(Entry::Separator);
    m.push(item("&Quit tray", true, Action::Quit));
    m
}

/// Always present; opens GitHub Sponsors. `D`: `S` is taken by Resume/Start.
pub const SPONSOR_LABEL: &str = "Sponsor &Detent…";

/// `Check for &updates · v0.1.0` (this tray's version).
pub fn check_label() -> String {
    format!("Check for &updates · v{}", env!("CARGO_PKG_VERSION"))
}

/// Qt-style `&` mnemonics → dbusmenu's `_` (Plasma's importer maps them back), escaping
/// literal underscores. `&&` is a literal `&`.
pub fn to_dbusmenu(label: &str) -> String {
    let mut out = String::with_capacity(label.len() + 2);
    let mut it = label.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '&' if it.peek() == Some(&'&') => {
                it.next();
                out.push('&');
            }
            '&' => out.push('_'),
            '_' => out.push_str("__"),
            c => out.push(c),
        }
    }
    out
}

/// A plain (header) label for dbusmenu: only underscores need escaping.
pub fn plain_dbusmenu(label: &str) -> String {
    label.replace('_', "__")
}

/// The mnemonic letter of a label, if any (tests).
#[cfg(test)]
fn mnemonic(label: &str) -> Option<char> {
    let i = label.find('&')?;
    label[i + 1..]
        .chars()
        .next()
        .map(|c| c.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::tests::{mx, up, with_update};

    const CFG: &str = "/home/u/.config/detent/config.toml";

    fn labels(m: &[Entry]) -> Vec<String> {
        m.iter()
            .map(|e| match e {
                Entry::Header(h) => format!("# {h}"),
                Entry::Separator => "-".into(),
                Entry::Item { label, enabled, .. } => {
                    format!("{label}{}", if *enabled { "" } else { " (disabled)" })
                }
            })
            .collect()
    }

    #[test]
    fn running_menu() {
        let m = build(&up(vec![mx(Some(85))], true, None), Path::new("/d"));
        let check = check_label();
        assert!(check.starts_with("Check for &updates · v"));
        assert_eq!(
            labels(&m),
            [
                "# MX Master 4",
                "# Connected, Bluetooth, 85%",
                "-",
                "&Pause remapping",
                "-",
                "&Reload config",
                "Res&tart daemon",
                "-",
                "&Open config…",
                "View &logs…",
                "-",
                check.as_str(),
                "&About Detent",
                "Sponsor &Detent…",
                "-",
                "&Quit tray",
            ]
        );
        assert!(m.contains(&item(
            SPONSOR_LABEL,
            true,
            Action::OpenUrl("https://github.com/sponsors/Rfluid".into())
        )));
        assert!(m.contains(&item("&Open config…", true, Action::OpenConfig(CFG.into()))));
        assert!(m.contains(&item("&Pause remapping", true, Action::SetRemap(false))));
    }

    #[test]
    fn paused_with_update() {
        let m = build(
            &with_update(up(vec![mx(Some(85))], false, None)),
            Path::new("/d"),
        );
        let l = labels(&m);
        assert_eq!(l[1], "# Connected, Bluetooth, 85%. Paused");
        assert_eq!(l[3], "Re&sume remapping");
        assert!(m.contains(&item("Re&sume remapping", true, Action::SetRemap(true))));
        let at = l
            .iter()
            .position(|x| x.starts_with("Update available"))
            .unwrap();
        assert_eq!(l[at], "Update available: v0.2.0…");
        assert_eq!(l[at + 1], check_label());
        assert!(
            matches!(&m[at], Entry::Item { action: Action::OpenUrl(u), .. } if u.ends_with("v0.2.0"))
        );
    }

    #[test]
    fn stopped_menu_offers_start_and_disables_daemon_items() {
        let m = build(&Default::default(), Path::new("/d/config.toml"));
        let l = labels(&m);
        assert_eq!(&l[..2], ["# Detent", "# Daemon stopped"]);
        assert!(l.contains(&"&Pause remapping (disabled)".to_string()));
        assert!(l.contains(&"&Reload config (disabled)".to_string()));
        assert!(m.contains(&item(
            "S&tart daemon",
            true,
            Action::StartDaemon { restart: false }
        )));
        // Open config falls back to the XDG default path.
        assert!(m.contains(&item(
            "&Open config…",
            true,
            Action::OpenConfig("/d/config.toml".into())
        )));
    }

    #[test]
    fn no_mouse_and_config_error_headers() {
        let l = labels(&build(&up(vec![], true, Some("bad")), Path::new("/d")));
        assert_eq!(
            &l[..3],
            [
                "# No mouse",
                "# Not connected",
                "# Config error: previous config active"
            ]
        );
        let two = up(vec![mx(Some(85)), mx(None)], true, None);
        let l = labels(&build(&two, Path::new("/d")));
        assert_eq!(
            l.iter().filter(|x| x.starts_with("# ")).count(),
            4,
            "two rows per device"
        );
    }

    #[test]
    fn mnemonics_are_unique_in_every_state() {
        for s in [
            up(vec![mx(None)], true, None),
            with_update(up(vec![], false, None)),
            Default::default(),
        ] {
            let mut seen: Vec<char> = build(&s, Path::new("/d"))
                .iter()
                .filter_map(|e| match e {
                    Entry::Item { label, .. } => mnemonic(label),
                    _ => None,
                })
                .collect();
            let n = seen.len();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), n, "duplicate mnemonic in {s:?}");
        }
    }

    #[test]
    fn dbusmenu_label_encoding() {
        assert_eq!(to_dbusmenu("&Pause remapping"), "_Pause remapping");
        assert_eq!(to_dbusmenu("Res&tart daemon"), "Res_tart daemon");
        assert_eq!(to_dbusmenu("a_b && c"), "a__b & c");
        assert_eq!(plain_dbusmenu("MX_Master"), "MX__Master");
    }
}
