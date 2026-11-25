//! Client IPC. Spec: `docs/specs/ipc.md`.
//!
//! JSON-RPC 2.0, newline-delimited, over `$XDG_RUNTIME_DIR/detent/ctl.sock`.
//! Not latency-critical: optimized for debuggability (`socat - UNIX:...`).
//!
//! - [`Client`]: blocking client used by `detentctl` and reusable by GUIs (tray, settings).
//! - [`types`]: the stable result schemas (`status`, `remap.*`, `plugins.list`, ...).
//! - [`settings`]: the catalogue of settable device paths, shared by the daemon (validation),
//!   the CLI (`settings list`) and the direct no-daemon path.
//! - [`platform`]: which OS runs the daemon, and the message Linux-only programs print
//!   elsewhere (ADR 0010). The socket is a Unix socket; on Windows [`Client::connect_to`]
//!   returns [`ClientError::Unsupported`] (no daemon there yet).

use std::path::PathBuf;

mod client;
pub mod platform;
pub mod settings;
pub mod types;

pub use client::{Client, ClientError, RpcError};

pub const SOCKET_NAME: &str = "ctl.sock";

/// Overrides the socket path for both the daemon and clients (tests, a second daemon).
pub const SOCKET_ENV: &str = "DETENT_SOCKET";

/// Method names. Keep in sync with docs/specs/ipc.md.
pub mod method {
    pub const STATUS: &str = "status";
    pub const DEVICES_LIST: &str = "devices.list";
    pub const DEVICE_GET: &str = "device.get";
    pub const DEVICE_SCHEMA: &str = "device.schema";
    pub const DEVICE_SET: &str = "device.set";
    pub const DEVICE_UNSET: &str = "device.unset";
    pub const DEVICE_FEATURES: &str = "device.features";
    pub const DEVICE_RAW: &str = "device.raw";
    pub const HAPTIC_PLAY: &str = "haptic.play";
    pub const EVENTS_SUBSCRIBE: &str = "events.subscribe";
    pub const CONFIG_RELOAD: &str = "config.reload";
    pub const CONFIG_VALIDATE: &str = "config.validate";
    pub const REMAP_GET: &str = "remap.get";
    pub const REMAP_SET: &str = "remap.set";
    pub const BINDINGS_LIST: &str = "bindings.list";
    pub const PLUGINS_LIST: &str = "plugins.list";
    pub const PLUGINS_RESTART: &str = "plugins.restart";
    pub const ACTIONS_LIST: &str = "actions.list";
    pub const ACTIONS_INVOKE: &str = "actions.invoke";
}

/// JSON-RPC error codes. Keep in sync with docs/specs/ipc.md ("Errors").
pub mod code {
    pub const PARSE: i32 = -32700;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    /// Device not found, disconnected or not answering.
    pub const DEVICE: i32 = 1001;
    /// HID++ error reported by the device.
    pub const HIDPP: i32 = 1002;
    /// Config invalid or could not be written.
    pub const CONFIG: i32 = 1003;
}

/// Event topics published on `events.subscribe`.
pub const TOPICS: &[&str] = &[
    "action", "exec", "link", "config", "profile", "context", "plugin", "remap", "setting",
];

/// `$XDG_RUNTIME_DIR/detent` (falls back to `/run/user/<uid>/detent`).
pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", rustix_uid())))
        .join("detent")
}

/// `$DETENT_SOCKET`, else `$XDG_RUNTIME_DIR/detent/ctl.sock`.
pub fn socket_path() -> PathBuf {
    std::env::var_os(SOCKET_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| runtime_dir().join(SOCKET_NAME))
}

fn rustix_uid() -> u32 {
    // Avoid a libc dependency for one call: /proc/self status has it.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1))
                .and_then(|u| u.parse().ok())
        })
        .unwrap_or(1000)
}
