//! Desktop notifications (org.freedesktop.Notifications via notify-rust on zbus) and the
//! "already told the owner about this version" memo in `$XDG_STATE_HOME/detent/`.

use std::path::PathBuf;

use notify_rust::{Notification, Timeout};

pub const APP: &str = "Detent";
pub const ICON: &str = "detent-tray-running-symbolic";

/// Show a notification; with `action = Some((label, url))` clicking the action opens `url`.
/// Never blocks the caller: waiting for the action happens on its own thread.
pub fn show(summary: &str, body: &str, action: Option<(&str, String)>) {
    let mut n = Notification::new();
    n.appname(APP)
        .summary(summary)
        .body(body)
        .icon(ICON)
        .timeout(Timeout::Milliseconds(10_000));
    if let Some((label, _)) = &action {
        n.action("open", label);
    }
    match n.show() {
        Ok(handle) => {
            if let Some((_, url)) = action {
                std::thread::spawn(move || {
                    handle.wait_for_action(|a| {
                        if a == "open" {
                            crate::platform::open_url(url.clone());
                        }
                    });
                });
            }
        }
        Err(e) => eprintln!("detent-tray: notification failed: {e}: {summary}: {body}"),
    }
}

/// `$XDG_STATE_HOME/detent/tray-update-notified` (default `~/.local/state/...`).
fn memo_path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/"))
                .join(".local/state")
        })
        .join("detent")
        .join("tray-update-notified")
}

/// Was `tag` already announced? Records it if not. Notify once per version.
pub fn first_time_for(tag: &str) -> bool {
    first_time_at(&memo_path(), tag)
}

fn first_time_at(path: &std::path::Path, tag: &str) -> bool {
    if std::fs::read_to_string(path).is_ok_and(|s| s.trim() == tag) {
        return false;
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(path, format!("{tag}\n")) {
        eprintln!("detent-tray: can't record {}: {e}", path.display());
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_per_version() {
        let p = std::env::temp_dir()
            .join(format!("detent-tray-memo-{}", std::process::id()))
            .join("tray-update-notified");
        let _ = std::fs::remove_file(&p);
        assert!(first_time_at(&p, "v0.2.0"));
        assert!(!first_time_at(&p, "v0.2.0"));
        assert!(first_time_at(&p, "v0.3.0"));
        assert!(!first_time_at(&p, "v0.3.0"));
    }
}
