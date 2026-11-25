//! The tray proper (Linux: StatusNotifierItem over D-Bus): argument handling, the poll
//! loop and `--preview`. `main.rs` declares the modules and picks this or the
//! unsupported-OS exit (ADR 0010).

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use ksni::blocking::TrayMethods;

use crate::model::Daemon;
use crate::poll::{Backoff, Kick};
use crate::sni::{Applied, DetentTray};
use crate::{actions, icons, menu, model, platform, poll, sponsor};

/// Re-read status (battery) this often while the daemon is up and quiet.
const IDLE_REFRESH: Duration = Duration::from_secs(60);

const HELP: &str = "\
detent-tray: system tray icon for Detent (KDE Plasma StatusNotifierItem)

Usage: detent-tray [--version] [--help] [--write-icons DIR]

Shows whether detentd runs, the mouse is connected and remapping is on. Left click opens
the menu; middle click pauses or resumes remapping. Normally started by
detent-tray.service with the graphical session.

Environment:
  DETENT_SOCKET           daemon socket (default $XDG_RUNTIME_DIR/detent/ctl.sock)
  DETENT_TRAY_ICON_PATH   extra icon theme root holding hicolor/scalable/status/
                          (to run from a checkout: packaging/icons)
Config: ~/.config/detent/tray.toml
  [update]  check = true|false, token_file = \"PATH\"
  [sponsor] nudge = true|false   (the one-time \"Enjoying Detent?\" notification)";

/// `$XDG_CONFIG_HOME/detent/config.toml`, the daemon's default (crates/config).
fn default_config() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".config")
        })
        .join("detent")
        .join("config.toml")
}

pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => {}
        Some("--version" | "-V") => {
            println!("detent-tray {} ({})", actions::VERSION, actions::COMMIT);
            return;
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            return;
        }
        Some("--write-icons") => {
            let Some(dir) = args.get(1) else {
                eprintln!("usage: detent-tray --write-icons DIR");
                std::process::exit(2);
            };
            if let Err(e) = icons::write_all(std::path::Path::new(dir)) {
                eprintln!("detent-tray: {e}");
                std::process::exit(1);
            }
            return;
        }
        Some("--preview") => {
            // Hidden: a fixed state for screenshots and panel checks (no daemon involved).
            let Some(snap) = args.get(1).and_then(|s| preview(s)) else {
                eprintln!(
                    "usage: detent-tray --preview running|nomouse|paused|stopped|error|update"
                );
                std::process::exit(2);
            };
            if let Err(e) = run_preview(&args[1], snap) {
                eprintln!("detent-tray: {e}");
                std::process::exit(1);
            }
            return;
        }
        Some(other) => {
            eprintln!("detent-tray: unknown argument {other:?}\n\n{HELP}");
            std::process::exit(2);
        }
    }
    if !platform::single_instance() {
        return; // another tray is already running: exit 0 silently
    }
    if let Err(e) = run() {
        eprintln!("detent-tray: {e}");
        std::process::exit(1);
    }
}

/// A made-up snapshot per state, for `--preview`.
fn preview(state: &str) -> Option<model::Snapshot> {
    use model::{DaemonView, DeviceView, Snapshot, UpdateView};
    let mx = DeviceView {
        name: "MX Master 4".into(),
        link: Some("Bluetooth".into()),
        battery: Some(85),
    };
    let up = |devices: Vec<DeviceView>, remap_enabled, err: Option<&str>| Snapshot {
        daemon: Daemon::Up(DaemonView {
            version: actions::VERSION.into(),
            config_path: default_config(),
            config_error: err.map(str::to_string),
            remap_enabled,
            devices,
        }),
        update: None,
    };
    Some(match state {
        "running" => up(vec![mx], true, None),
        "nomouse" => up(vec![], true, None),
        "paused" => up(vec![mx], false, None),
        "stopped" => Snapshot::default(),
        "error" => up(
            vec![mx],
            true,
            Some("line 12: unknown action `kde.overveiw`"),
        ),
        "update" => Snapshot {
            update: Some(UpdateView {
                tag: "v9.9.9".into(),
                url: detent_update::RELEASES_URL.into(),
            }),
            ..up(vec![mx], true, None)
        },
        _ => return None,
    })
}

fn run_preview(state: &str, snap: model::Snapshot) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();
    let tray = DetentTray {
        title: format!("Detent ({state})"),
        snap,
        actions: tx,
        default_config: default_config(),
        icon_theme_path: platform::icon_theme_path(),
    };
    let handle = tray.assume_sni_available(true).spawn()?;
    for a in rx {
        if a == menu::Action::Quit {
            handle.shutdown().wait();
            return Ok(());
        }
        eprintln!("detent-tray --preview: would run {a:?}");
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let socket = detent_ipc::socket_path();
    sponsor::record_first_run();
    let (action_tx, action_rx) = mpsc::channel();
    let (kick_tx, kick_rx) = mpsc::channel();

    let first = poll::read(&socket);
    let tray = DetentTray {
        title: "Detent".into(),
        snap: model::Snapshot {
            daemon: first.clone(),
            update: None,
        },
        actions: action_tx,
        default_config: default_config(),
        icon_theme_path: platform::icon_theme_path(),
    };
    // graphical-session.target doesn't wait for Plasma to claim the StatusNotifierWatcher:
    // keep running and register when a host appears (watcher_offline returns true).
    let handle = tray.assume_sni_available(true).spawn()?;

    {
        let (h, s, k) = (handle.clone(), socket.clone(), kick_tx.clone());
        std::thread::Builder::new()
            .name("actions".into())
            .spawn(move || actions::run(action_rx, s, k, h))?;
    }
    {
        let (s, k) = (socket.clone(), kick_tx);
        std::thread::Builder::new()
            .name("events".into())
            .spawn(move || poll::watch_events(s, k))?;
    }
    {
        let h = handle.clone();
        std::thread::Builder::new()
            .name("update".into())
            .spawn(move || actions::update_loop(h))?;
    }
    {
        let h = handle.clone();
        std::thread::Builder::new()
            .name("sponsor".into())
            .spawn(move || sponsor::run(h))?;
    }

    let mut backoff = Backoff::default();
    let mut shown = Applied::default();
    shown.changed(&first);
    loop {
        if handle.is_closed() {
            return Ok(()); // the tray service shut down
        }
        let daemon = poll::read(&socket);
        let wait = match daemon {
            Daemon::Up(_) => {
                backoff.reset();
                IDLE_REFRESH
            }
            Daemon::Down { .. } => backoff.next(),
        };
        if shown.changed(&daemon) && handle.update(|t| t.snap.daemon = daemon).is_none() {
            return Ok(());
        }
        match kick_rx.recv_timeout(wait) {
            Ok(Kick::Refresh) => {
                // Coalesce a burst of events into one refresh.
                std::thread::sleep(Duration::from_millis(150));
                while kick_rx.try_recv().is_ok() {}
                // A kick (an action, the daemon appearing) restarts the fast polling.
                backoff.reset();
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => std::thread::sleep(wait),
        }
    }
}
