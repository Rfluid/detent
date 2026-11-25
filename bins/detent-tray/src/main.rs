//! `detent-tray`: the Detent system tray icon (KDE StatusNotifierItem + dbusmenu via ksni).
//!
//! A separate process on its own user unit, not a bus plugin, so it can show "stopped" and
//! offer Start while detentd is down (ADR 0009). It only uses the public IPC socket, like
//! detentctl. User guide: docs/user/tray.md.
//!
//! Linux only for now (StatusNotifierItem over D-Bus; ADR 0010). On other OSes the binary
//! still builds and exits at once with where to follow platform support.
//!
//! Layout: `app.rs` holds argument handling, the refresh loop and `--preview`.
//! Threads: ksni's D-Bus thread (menu callbacks only queue actions), the action thread,
//! the event watcher (events.subscribe → refresh), the update checker, the one-time
//! sponsor notification (`sponsor.rs`), and main, which refreshes the snapshot on every
//! kick, every minute while up, and with backoff while down.

#[cfg(target_os = "linux")]
mod actions;
#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod icons;
#[cfg(target_os = "linux")]
mod menu;
#[cfg(target_os = "linux")]
mod model;
#[cfg(target_os = "linux")]
mod notify;
#[cfg(target_os = "linux")]
mod platform;
#[cfg(target_os = "linux")]
mod poll;
#[cfg(target_os = "linux")]
mod sni;
#[cfg(target_os = "linux")]
mod sponsor;

#[cfg(target_os = "linux")]
fn main() {
    app::main();
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("{}", detent_ipc::platform::unsupported("detent-tray"));
    std::process::ExitCode::from(69)
}
