//! detent-radial: on-screen radial menu, modelled on the Logitech Actions Ring.
//! Spec: docs/plugins/radial.md
//!
//! Linux only for now (GPUI with Wayland layer-shell; ADR 0010). On other OSes the binary
//! still builds and exits at once with where to follow platform support.
//!
//! Layout: `app.rs` holds the manifest, bus thread, `Ui` driver and `main`; `state.rs` the
//! pure state machine, `view.rs` the GPUI overlay, `menu.rs` the menu config.
//!
//! Threads:
//! - GPUI main thread: owns the layer-shell overlay window and the state machine
//! - bus thread: `detent_plugin::connect`, forwards daemon messages into GPUI through an
//!   async channel; GPUI sends leases/haptics/invokes back with a `Sender`
//!
//! The overlay's hover / clicks / keys come back through the same channel (`Msg::Input`),
//! so every input reaches the state machine in order on one thread.

#[cfg(target_os = "linux")]
mod app;
#[cfg(target_os = "linux")]
mod defaults;
#[cfg(target_os = "linux")]
mod geometry;
#[cfg(target_os = "linux")]
mod icon;
#[cfg(target_os = "linux")]
mod menu;
#[cfg(target_os = "linux")]
mod preview;
#[cfg(target_os = "linux")]
mod replies;
#[cfg(target_os = "linux")]
mod state;
#[cfg(target_os = "linux")]
mod view;

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    app::main()
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("{}", detent_ipc::platform::unsupported("detent-radial"));
    std::process::ExitCode::from(69)
}
