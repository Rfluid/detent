//! hidapi transport for macOS and Windows (ADR 0010, docs/specs/transport.md).
//!
//! - one handle on the device's HID++ long-report collection (usage page 0xFF00 usage
//!   0x0002, or 0xFF43 / 0x0202 over Bluetooth): Windows opens each top-level collection
//!   separately, and Bluetooth devices such as the MX Master 4 have no short-report
//!   collection at all, so requests always go out as long reports (the requester already
//!   sends long ones)
//! - macOS: hidapi's `macos-shared-device` opens without seizing the device, so the OS
//!   keeps delivering pointer input (⚠ to verify on a Mac)
//! - Windows: the pure-Rust `windows-native` hidapi backend (no C toolchain)
//! - reads into a stack buffer; no allocation per frame
//!
//! Not used by the data plane (the daemon is Linux-only for now): this serves
//! `detentctl`'s direct device commands.

use std::ffi::CString;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use detent_hidpp_proto::{Frame, MAX_FRAME_LEN, ReportId};
use hidapi::{BusType, HidApi, HidDevice};

use crate::Transport;
use crate::discover::{Bus, DeviceInfo, LOGITECH_VENDOR, is_hidpp_long_collection};

pub struct HidapiTransport {
    dev: HidDevice,
}

impl std::fmt::Debug for HidapiTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HidapiTransport").finish_non_exhaustive()
    }
}

fn to_io(e: hidapi::HidError) -> io::Error {
    io::Error::other(e.to_string())
}

impl HidapiTransport {
    /// Open a path from [`crate::discover::logitech`].
    pub fn open(path: &Path) -> io::Result<Self> {
        let c = CString::new(path.as_os_str().to_string_lossy().into_owned())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let api = HidApi::new().map_err(to_io)?;
        let dev = api.open_path(&c).map_err(to_io)?;
        Ok(Self { dev })
    }
}

/// One input report → a frame. Some OSes pad a report to the collection's longest input
/// report; cut it back to the length its report ID implies. Non-HID++ reports → `None`.
fn frame_of(report: &[u8]) -> Option<Frame> {
    let rid = ReportId::from_u8(*report.first()?)?;
    let len = rid.frame_len();
    Frame::parse(report.get(..len)?).ok()
}

impl Transport for HidapiTransport {
    fn send(&mut self, frame: &Frame) -> io::Result<()> {
        self.dev.write(frame.as_bytes()).map_err(to_io)?;
        Ok(())
    }

    fn recv(&mut self, timeout: Option<Duration>) -> io::Result<Option<Frame>> {
        let deadline = timeout.map(|t| Instant::now() + t);
        loop {
            let ms = match deadline {
                Some(d) => {
                    let left = d.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(None);
                    }
                    // Round up so a sub-millisecond remainder still waits.
                    i32::try_from(left.as_micros().div_ceil(1000)).unwrap_or(i32::MAX)
                }
                None => -1,
            };
            let mut buf = [0u8; MAX_FRAME_LEN];
            let n = self.dev.read_timeout(&mut buf, ms).map_err(to_io)?;
            if n == 0 {
                return Ok(None);
            }
            if let Some(frame) = frame_of(&buf[..n]) {
                return Ok(Some(frame));
            }
        }
    }
}

fn bus_of(b: BusType) -> Bus {
    match b {
        BusType::Usb => Bus::Usb,
        BusType::Bluetooth => Bus::Bluetooth,
        // Linux BUS_* codes, so `Other` means the same on every OS.
        BusType::I2c => Bus::Other(0x18),
        BusType::Spi => Bus::Other(0x1C),
        BusType::Unknown => Bus::Other(0),
    }
}

/// Every Logitech HID++ long-report collection hidapi can see, sorted by path.
pub fn enumerate() -> io::Result<Vec<DeviceInfo>> {
    let api = HidApi::new().map_err(to_io)?;
    let mut out: Vec<DeviceInfo> = api
        .device_list()
        .filter(|d| u32::from(d.vendor_id()) == LOGITECH_VENDOR)
        .filter(|d| is_hidpp_long_collection(d.usage_page(), d.usage()))
        .map(|d| DeviceInfo {
            node: PathBuf::from(d.path().to_string_lossy().into_owned()),
            bus: bus_of(d.bus_type()),
            vendor: u32::from(d.vendor_id()),
            product: u32::from(d.product_id()),
            name: d.product_string().unwrap_or_default().to_string(),
            uniq: d
                .serial_number()
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        })
        .collect();
    out.sort_by(|a, b| a.node.cmp(&b.node));
    out.dedup_by(|a, b| a.node == b.node);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::frame_of;

    #[test]
    fn trims_padded_reports() {
        let mut r = [0u8; 64];
        r[..4].copy_from_slice(&[0x11, 0xFF, 0x00, 0x1E]);
        let f = frame_of(&r).unwrap();
        assert_eq!(f.as_bytes().len(), 20);
        assert!(frame_of(&[0x02, 0, 0]).is_none(), "mouse report, not HID++");
        assert!(frame_of(&[0x11, 0xFF]).is_none(), "short read");
    }
}
