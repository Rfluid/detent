//! How frames reach the device.
//!
//! The data plane owns one [`Transport`] per device and is the only reader/writer.
//! Tests use [`mock::MockTransport`]; Phase 1 adds a record/replay transport fed by
//! captures from `detentctl capture`.
//!
//! Backends (ADR 0010, docs/specs/transport.md): [`hidraw`] on Linux, [`hid`] (hidapi) on
//! macOS and Windows. [`OsTransport`] names the one this build uses, and
//! [`discover::logitech`] lists devices for it, so device-side code (`detentctl`'s direct
//! path) is written once. Callers stay generic over [`Transport`]: static dispatch, no
//! `dyn`, nothing allocated per frame.

pub mod discover;
#[cfg(not(target_os = "linux"))]
pub mod hid;
#[cfg(target_os = "linux")]
pub mod hidraw;
pub mod mock;

use std::io;
use std::time::Duration;

use detent_hidpp_proto::Frame;

/// A bidirectional HID++ frame channel.
pub trait Transport {
    fn send(&mut self, frame: &Frame) -> io::Result<()>;

    /// Wait up to `timeout` for the next inbound frame. `None` timeout blocks.
    /// Returns `Ok(None)` on timeout.
    fn recv(&mut self, timeout: Option<Duration>) -> io::Result<Option<Frame>>;
}

/// This OS's HID++ transport: opened from [`discover::DeviceInfo::node`].
#[cfg(target_os = "linux")]
pub type OsTransport = hidraw::HidrawTransport;

/// This OS's HID++ transport: opened from [`discover::DeviceInfo::node`].
#[cfg(not(target_os = "linux"))]
pub type OsTransport = hid::HidapiTransport;
