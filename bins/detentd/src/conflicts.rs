//! Detect other programs that control Logitech devices. Policy: warn (configurable).

use std::fs;

/// Process names (`/proc/<pid>/comm`, max 15 chars) of known conflicting tools.
const CONFLICTING: &[(&str, &str)] = &[
    ("solaar", "Solaar"),
    ("logid", "logiops (logid)"),
    ("juhradiald", "JuhRadial MX"),
];

/// How often the background watch rescans `/proc`.
const WATCH_EVERY: std::time::Duration = std::time::Duration::from_secs(10);

/// Keep checking after startup: login autostart can start a conflicting tool a moment
/// after Detent (seen 2026-11-24: JuhRadial started in the same second, after the startup
/// check, and both acted on every gesture). Warns once each time a tool appears.
pub fn watch(already: Vec<&'static str>) {
    let spawned = std::thread::Builder::new()
        .name("conflicts".into())
        .spawn(move || {
            let mut known = already;
            loop {
                std::thread::sleep(WATCH_EVERY);
                let now = running_conflicts();
                for label in now.iter().filter(|l| !known.contains(l)) {
                    tracing::warn!(
                        "{label} started and may fight Detent over the mouse (diversion, \
                         gestures: both act on every press). Stop and disable it."
                    );
                }
                known = now;
            }
        });
    if let Err(e) = spawned {
        tracing::debug!("conflict watch not started: {e}");
    }
}

pub fn running_conflicts() -> Vec<&'static str> {
    let Ok(entries) = fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut found: Vec<&'static str> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(comm) = fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        let comm = comm.trim();
        for (proc_name, label) in CONFLICTING {
            if comm == *proc_name && !found.contains(label) {
                found.push(label);
            }
        }
    }
    found
}
