//! Running what the menu asks for, on the action thread (never on the D-Bus thread).
//! Daemon control goes through the IPC socket or `systemctl --user`, like detentctl.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

use detent_ipc::{Client, ClientError, code, method};
use serde_json::json;

use crate::menu::Action;
use crate::model::UpdateView;
use crate::poll::{Kick, UNIT};
use crate::sni::{Applied, Handle};
use crate::{notify, platform};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const COMMIT: &str = env!("DETENT_GIT_COMMIT");

/// Spawn a GUI helper detached from the tray; reap it on a thread so it never zombies.
fn launch(mut c: Command) -> std::io::Result<()> {
    let mut child = c
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// The terminal command for following the journal: konsole, then `$TERMINAL -e`, then
/// `xdg-terminal-exec`. Pure (the lookups are injected) for tests.
pub fn terminal_command(
    has: impl Fn(&str) -> bool,
    terminal_env: Option<String>,
) -> Option<Vec<String>> {
    let journal = ["journalctl", "--user", "-u", UNIT, "-f"].map(String::from);
    let with = |prefix: Vec<String>| Some(prefix.into_iter().chain(journal.clone()).collect());
    if has("konsole") {
        return with(vec!["konsole".into(), "-e".into()]);
    }
    if let Some(t) = terminal_env.filter(|t| !t.trim().is_empty()) {
        return with(vec![t, "-e".into()]);
    }
    if has("xdg-terminal-exec") {
        return with(vec!["xdg-terminal-exec".into()]);
    }
    None
}

fn on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).is_file()))
}

fn view_logs() {
    let Some(argv) = terminal_command(on_path, std::env::var("TERMINAL").ok()) else {
        notify::show(
            "No terminal found",
            &format!("Install konsole or set $TERMINAL. Logs: journalctl --user -u {UNIT} -f"),
            None,
        );
        return;
    };
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..]);
    if let Err(e) = launch(c) {
        notify::show("Couldn't open the logs", &format!("{}: {e}", argv[0]), None);
    }
}

fn open_config(path: &Path) {
    if path.exists() {
        platform::open_path(path.to_path_buf());
    } else {
        notify::show(
            "No config file yet",
            &format!(
                "{} doesn't exist. Create it with `detentctl config edit`.",
                path.display()
            ),
            None,
        );
    }
}

fn ipc_error(what: &str, e: &ClientError) {
    let body = match e.rpc_code() {
        Some(code::METHOD_NOT_FOUND) => {
            "The running detentd is older than this tray. Restart it on the new build.".to_string()
        }
        _ => e.to_string(),
    };
    notify::show(&format!("{what} failed"), &body, None);
}

fn set_remap(socket: &Path, enabled: bool) {
    let r = Client::connect_to(socket).and_then(|mut c| c.set_remap(enabled));
    if let Err(e) = r {
        ipc_error(if enabled { "Resume" } else { "Pause" }, &e);
    }
}

fn reload(socket: &Path) {
    let r = Client::connect_to(socket).and_then(|mut c| c.call(method::CONFIG_RELOAD, json!({})));
    if let Err(e) = r {
        if e.rpc_code() == Some(code::CONFIG) {
            notify::show(
                "Config rejected",
                &format!("The previous config is still active.\n{e}"),
                None,
            );
        } else {
            ipc_error("Reload", &e);
        }
    }
}

fn systemctl(verb: &str) {
    let r = Command::new("systemctl")
        .args(["--user", verb, UNIT])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match r {
        Ok(o) if o.status.success() => {}
        Ok(o) => notify::show(
            &format!("Couldn't {verb} Detent"),
            String::from_utf8_lossy(&o.stderr).trim(),
            None,
        ),
        Err(e) => notify::show(
            &format!("Couldn't {verb} Detent"),
            &format!("systemctl: {e}"),
            None,
        ),
    }
}

fn about(socket: &Path) {
    let daemon = Client::connect_to(socket)
        .and_then(|mut c| c.status())
        .map(|s| match s.pid {
            0 => format!("detentd {}", s.version), // older daemons don't report it
            pid => format!("detentd {} (pid {pid})", s.version),
        })
        .unwrap_or_else(|_| "detentd is not running".into());
    notify::show(
        "About Detent",
        &format!("Detent {VERSION} (commit {COMMIT})\n{daemon}\nMIT licensed."),
        Some(("Open repository", detent_update::REPO_URL.to_string())),
    );
}

/// Run one update check. `manual`: always say the result; otherwise stay quiet unless a
/// newer version appears for the first time. Returns the update to show, if any.
pub fn check_updates(manual: bool, settings: &detent_update::Settings) -> Option<UpdateView> {
    match detent_update::check(VERSION, settings) {
        Ok(r) if r.update_available => {
            let tag = r.latest.tag.clone();
            if manual || notify::first_time_for(&tag) {
                notify::show(
                    &format!("Detent {tag} is available"),
                    &format!("You have {VERSION}. The tray never installs anything itself."),
                    Some(("Release notes", r.latest.url.clone())),
                );
            }
            Some(UpdateView {
                tag,
                url: r.latest.url,
            })
        }
        Ok(r) => {
            if manual {
                notify::show(
                    "Detent is up to date",
                    &format!("{VERSION} is the newest release ({}).", r.latest.tag),
                    None,
                );
            }
            None
        }
        Err(e) => {
            if manual {
                let summary = match e {
                    detent_update::Error::NotFound => "No Detent release found",
                    _ => "Couldn't check for updates",
                };
                notify::show(summary, &e.to_string(), None);
            } else {
                eprintln!("detent-tray: update check: {e}");
            }
            None
        }
    }
}

fn load_settings() -> detent_update::Settings {
    detent_update::Settings::load().unwrap_or_else(|e| {
        eprintln!("detent-tray: {e}; using defaults");
        detent_update::Settings::default()
    })
}

/// The update thread: check at start, then daily, when enabled.
pub fn update_loop(handle: Handle) {
    const DAY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);
    let mut shown = Applied::default();
    loop {
        if handle.is_closed() {
            return;
        }
        let s = load_settings();
        if s.check {
            let u = check_updates(false, &s);
            if shown.changed(&u) && handle.update(|t| t.snap.update = u).is_none() {
                return;
            }
        }
        std::thread::sleep(DAY);
    }
}

/// The action thread. Ends when every sender (the tray) is gone.
pub fn run(rx: Receiver<Action>, socket: PathBuf, kick: Sender<Kick>, handle: Handle) {
    for a in rx {
        match a {
            Action::SetRemap(on) => set_remap(&socket, on),
            Action::Reload => reload(&socket),
            Action::StartDaemon { restart } => systemctl(if restart { "restart" } else { "start" }),
            Action::OpenConfig(p) => open_config(&p),
            Action::ViewLogs => view_logs(),
            Action::OpenUrl(u) => platform::open_url(u),
            Action::CheckUpdates => {
                let u = check_updates(true, &load_settings());
                handle.update(|t| t.snap.update = u);
            }
            Action::About => about(&socket),
            Action::Quit => {
                handle.shutdown().wait();
                std::process::exit(0);
            }
        }
        let _ = kick.send(Kick::Refresh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_preference() {
        let cmd = |has: &[&str], env: Option<&str>| {
            let has: Vec<String> = has.iter().map(|s| s.to_string()).collect();
            terminal_command(|b| has.iter().any(|h| h == b), env.map(String::from))
        };
        let k = cmd(&["konsole", "xdg-terminal-exec"], Some("foot")).unwrap();
        assert_eq!(&k[..3], ["konsole", "-e", "journalctl"]);
        assert!(k.ends_with(&["detent.service".into(), "-f".into()]));
        assert_eq!(
            &cmd(&["xdg-terminal-exec"], Some("foot")).unwrap()[..2],
            ["foot", "-e"]
        );
        assert_eq!(
            cmd(&["xdg-terminal-exec"], Some(" ")).unwrap()[0],
            "xdg-terminal-exec"
        );
        assert_eq!(cmd(&[], None), None);
    }
}
