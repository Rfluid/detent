//! detentd: the Detent daemon. Architecture: docs/architecture/overview.md
//!
//! Linux only for now (hidraw, evdev grab, uinput, udev hotplug; ADR 0010). On other OSes
//! the binary still builds and exits at once with where to follow platform support.
//!
//! Layout: `app.rs` is startup, hotplug and shutdown; the other modules one concern each.
//! Threads:
//! - main: signals (SIGTERM/SIGINT → orderly shutdown)
//! - hotplug: udev monitor on `hidraw`, attaches Logitech HID++ devices
//! - per device: data plane (`dataplane.rs`) + worker (`worker.rs`)
//! - config watcher and client IPC (`config_watch.rs`, `ipc.rs`)

#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod bus;
#[cfg(target_os = "linux")]
mod config_watch;
#[cfg(target_os = "linux")]
mod conflicts;
#[cfg(target_os = "linux")]
mod dataplane;
#[cfg(target_os = "linux")]
mod device;
#[cfg(target_os = "linux")]
mod dispatch;
#[cfg(target_os = "linux")]
mod exec;
#[cfg(target_os = "linux")]
mod ipc;
#[cfg(target_os = "linux")]
mod msg;
#[cfg(target_os = "linux")]
mod persist;
#[cfg(target_os = "linux")]
mod shared;
#[cfg(target_os = "linux")]
mod uinput;
#[cfg(target_os = "linux")]
mod worker;

#[cfg(target_os = "linux")]
fn main() {
    app::main();
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("{}", detent_ipc::platform::unsupported("detentd"));
    std::process::ExitCode::from(69) // EX_UNAVAILABLE, as detentctl uses
}
