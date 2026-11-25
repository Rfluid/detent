//! detent-kde: KDE Plasma integration. Spec: docs/plugins/kde.md
//!
//! Linux only (KDE Plasma, session D-Bus; ADR 0010). On other OSes the binary still builds
//! and exits at once with where to follow platform support.
//!
//! Layout: `app.rs` is the plugin (actions, context, `main`); `dbus.rs` the zbus thread and
//! KWin script, `audio.rs` PipeWire volume, `actions.rs` shortcut names, `terminal.rs`.

#[cfg(target_os = "linux")]
mod actions;
#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod audio;
#[cfg(target_os = "linux")]
mod dbus;
#[cfg(target_os = "linux")]
mod terminal;

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    app::main()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("{}", detent_ipc::platform::unsupported("detent-kde"));
    std::process::ExitCode::from(69)
}
