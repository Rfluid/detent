//! Messages between a device's data plane, its worker, and the rest of the daemon.
//! Data-plane-facing variants are `Copy`-sized values: no allocation to send them.

use std::sync::mpsc::SyncSender;

use detent_engine::ControlOut;
use detent_hidpp_proto::Frame;

/// Worker → data plane (rtrb ring + eventfd wake).
#[derive(Debug, Clone, Copy)]
pub enum ToDp {
    /// Write this HID++ request.
    Send(Frame),
    /// Current sensor DPI (swipe thresholds are in mm).
    SetDpi(u16),
    SetProfile(u8),
    /// Release every key/button the engine holds down (config swap).
    ReleaseAll,
    /// Tap a key chord on the virtual keyboard (plugin `keys` built-in).
    EmitKeys(detent_engine::action::KeyChord),
    /// Start (or end, with an inactive lease) a plugin lease.
    Lease(detent_engine::engine::Lease),
    /// Release everything, ungrab, exit.
    Stop,
}

/// Data plane → worker.
#[derive(Debug, Clone, Copy)]
pub enum FromDp {
    /// Response or error report to one of our requests.
    Response(Frame),
    /// Notification the data plane doesn't handle itself (battery, link, ...).
    Notification(Frame),
    /// HID++ traffic of the kernel driver (swid 1).
    KernelTraffic(Frame),
    Control(ControlOut),
    Grabbed(bool),
    /// The data-plane thread ended.
    Gone(&'static str),
}

/// Control plane → worker.
#[derive(Debug)]
pub enum Cmd {
    /// A new config was installed: re-apply diversion and device settings.
    Reload,
    /// Remapping was paused or resumed: re-apply diversion from `Shared::active`.
    Rebind,
    Profile(u8),
    /// Read the current state (reply with JSON).
    Get(SyncSender<serde_json::Value>),
    /// Set one setting: `(path, value)`; reply with the result.
    Set {
        path: String,
        value: String,
        reply: SyncSender<Result<serde_json::Value, String>>,
    },
    /// Drop the live edit of `path`; reply with the section's new value.
    Unset {
        path: String,
        reply: SyncSender<Result<serde_json::Value, String>>,
    },
    /// Raw HID++ request (expert): feature index, function, params.
    Raw {
        feature: u16,
        function: u8,
        params: Vec<u8>,
        reply: SyncSender<Result<Vec<u8>, String>>,
    },
    /// Play a haptic waveform.
    Haptic(u8),
    /// A device command from a plugin (DPI, SmartShift).
    Device(detent_engine::action::DeviceCmd),
    /// Tap a key chord on the virtual keyboard.
    EmitKeys(detent_engine::action::KeyChord),
    Lease(detent_engine::engine::Lease),
    Shutdown,
}

#[derive(Debug)]
pub enum WorkerMsg {
    Dp(FromDp),
    Cmd(Cmd),
}
