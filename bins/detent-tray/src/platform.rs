//! Desktop plumbing, following Rfluid/aura's proven tray setup (ADR 0009): the
//! single-instance lock, opening files and URLs without blocking, and where the icons live.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rustix::fs::{FlockOperation, flock};

use crate::notify;

/// `$XDG_RUNTIME_DIR/detent-tray.lock`, else `$TMPDIR/detent-tray-<uid>.lock`.
fn lock_path() -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d).join("detent-tray.lock"),
        None => std::env::temp_dir().join(format!(
            "detent-tray-{}.lock",
            rustix::process::getuid().as_raw()
        )),
    }
}

/// Take the per-user single-instance lock (`flock(LOCK_EX | LOCK_NB)`). `false`: another
/// tray holds it. The descriptor is leaked on purpose so the kernel, not a destructor,
/// releases the lock when the process ends (also on SIGKILL or a panic).
pub fn single_instance() -> bool {
    single_instance_at(&lock_path())
}

fn single_instance_at(path: &Path) -> bool {
    let file = match OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    {
        Ok(f) => f,
        Err(e) => {
            // Can't lock: run anyway rather than show no icon at all.
            eprintln!(
                "detent-tray: {}: {e}; skipping the single-instance check",
                path.display()
            );
            return true;
        }
    };
    if flock(&file, FlockOperation::NonBlockingLockExclusive).is_err() {
        return false;
    }
    std::mem::forget(file);
    true
}

/// Run `cmd`, wait for it, and report whether it succeeded. Output is discarded.
fn ok(mut cmd: Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn xdg_open(target: &str) -> bool {
    let mut c = Command::new("xdg-open");
    c.arg(target);
    ok(c)
}

/// `org.freedesktop.FileManager1.ShowItems([file://path], "")`: highlight it in Dolphin.
fn show_in_file_manager(path: &Path) -> zbus::Result<()> {
    let uri = format!("file://{}", path.display());
    let conn = zbus::blocking::Connection::session()?;
    conn.call_method(
        Some("org.freedesktop.FileManager1"),
        "/org/freedesktop/FileManager1",
        Some("org.freedesktop.FileManager1"),
        "ShowItems",
        &(vec![uri], ""),
    )?;
    Ok(())
}

/// Open a file in its default app on a detached thread. Falls back to showing it in the
/// file manager, then to opening its directory (a system without a TOML editor still
/// lands somewhere useful).
pub fn open_path(path: PathBuf) {
    std::thread::spawn(move || {
        if xdg_open(&path.display().to_string()) || show_in_file_manager(&path).is_ok() {
            return;
        }
        let dir = path.parent().unwrap_or(Path::new("/"));
        if !xdg_open(&dir.display().to_string()) {
            notify::show(
                "Couldn't open",
                &format!("No application opened {}", path.display()),
                None,
            );
        }
    });
}

/// Open a URL in the default browser on a detached thread.
pub fn open_url(url: String) {
    std::thread::spawn(move || {
        if !xdg_open(&url) {
            notify::show("Couldn't open the browser", &url, None);
        }
    });
}

/// SNI `IconThemePath`: `$DETENT_TRAY_ICON_PATH`, else the user icon dir install.sh fills,
/// so the named icons resolve even before an icon cache notices the new files.
pub fn icon_theme_path() -> String {
    if let Ok(p) = std::env::var("DETENT_TRAY_ICON_PATH")
        && !p.is_empty()
    {
        return p;
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".local/share")
        });
    let root = data.join("icons");
    let probe = root.join("hicolor/scalable/status/detent-tray-running-symbolic.svg");
    if probe.is_file() {
        root.display().to_string()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_instance_loses_the_lock() {
        let p = std::env::temp_dir().join(format!("detent-tray-lock-test-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        assert!(single_instance_at(&p));
        // flock locks belong to the open file description: a second open in the same
        // process conflicts just like another process would.
        assert!(!single_instance_at(&p));
    }
}
