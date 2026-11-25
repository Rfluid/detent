//! Find Logitech HID++ candidates.
//!
//! - Linux: a cheap sysfs scan of `/sys/class/hidraw` (every Logitech hidraw node; the
//!   caller finds out whether it speaks HID++). Used by `detentctl devices` and at daemon
//!   start; hotplug (udev monitor) is layered on top in the daemon.
//! - macOS / Windows: hidapi enumeration, keeping only the vendor collection that carries
//!   HID++ long reports ([`is_hidpp_long_collection`], docs/specs/transport.md).

use std::io;
use std::path::PathBuf;

pub const LOGITECH_VENDOR: u32 = 0x046D;

/// HID++ vendor usage page on receivers and USB devices: usage 0x0001 is the short-report
/// (0x10) collection, 0x0002 the long-report (0x11) one.
pub const USAGE_PAGE_HIDPP: u16 = 0xFF00;
/// HID++ vendor usage page on Bluetooth LE devices: usage 0x0202 carries report 0x11 (the
/// only HID++ collection; verified on the MX Master 4 (B042) report descriptor). Wired
/// G-series keyboards use usage 0x0602 (prior art: openlogi-hid; unverified here).
pub const USAGE_PAGE_HIDPP_BT: u16 = 0xFF43;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    Usb,
    Bluetooth,
    Other(u16),
}

impl Bus {
    /// Linux `BUS_*` codes (`HID_ID=0005:...`).
    fn from_code(code: u16) -> Self {
        match code {
            0x0003 => Self::Usb,
            0x0005 => Self::Bluetooth,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// What [`crate::OsTransport::open`] takes: `/dev/hidraw9` on Linux, the hidapi path
    /// (IOService path on macOS, the collection's device interface path on Windows).
    pub node: PathBuf,
    pub bus: Bus,
    pub vendor: u32,
    pub product: u32,
    pub name: String,
    /// Linux `HID_UNIQ` (Bluetooth address / serial), stable across reconnects; elsewhere
    /// the HID serial number string, if the OS reports one.
    pub uniq: Option<String>,
    /// This node is a receiver's HID++ interface: paired devices answer at indexes 1–6
    /// ([`RECEIVER_SLOTS`]), not at the direct index 0xFF.
    pub receiver: bool,
}

/// Device indexes a receiver gives its paired devices.
pub const RECEIVER_SLOTS: std::ops::RangeInclusive<u8> = 1..=6;

/// Logi Bolt receiver. Verified on the owner's receiver (2026-11-25): USB `046D:C548`,
/// bound to `hid-generic` on Linux 7.3 (no `hid-logitech-dj` alias), HID++ on interface 2.
pub const PRODUCT_BOLT: u32 = 0xC548;

/// Receivers Detent talks to by slot. Only Bolt is verified; Unifying (C52B/C532) is left
/// out until someone checks it on hardware (issue #5).
pub fn is_receiver(product: u32) -> bool {
    product == PRODUCT_BOLT
}

/// Does a report descriptor declare the 0xFF00/0x0002 (HID++ long report) collection?
/// Short items: `06 00 FF` (usage page, 2 bytes) then `09 02` (usage, 1 byte).
pub fn has_hidpp_long_collection(desc: &[u8]) -> bool {
    desc.windows(5).any(|w| w == [0x06, 0x00, 0xFF, 0x09, 0x02])
}

/// Does this (usage page, usage) pair name a collection that carries HID++ long reports?
/// That is the one to open where the OS splits a device into one handle per top-level
/// collection (Windows) or reports one entry per collection (macOS).
pub fn is_hidpp_long_collection(usage_page: u16, usage: u16) -> bool {
    matches!(
        (usage_page, usage),
        (USAGE_PAGE_HIDPP, 0x0002) | (USAGE_PAGE_HIDPP_BT, 0x0202 | 0x0602)
    )
}

/// Parse `HID_ID=0005:0000046D:0000B042` and `HID_NAME=...` out of a uevent file body.
pub fn parse_uevent(body: &str) -> Option<(Bus, u32, u32, String)> {
    let mut id = None;
    let mut name = String::new();
    for line in body.lines() {
        if let Some(v) = line.strip_prefix("HID_ID=") {
            let mut parts = v.split(':');
            let bus = u16::from_str_radix(parts.next()?, 16).ok()?;
            let vendor = u32::from_str_radix(parts.next()?, 16).ok()?;
            let product = u32::from_str_radix(parts.next()?, 16).ok()?;
            id = Some((Bus::from_code(bus), vendor, product));
        } else if let Some(v) = line.strip_prefix("HID_NAME=") {
            name = v.to_string();
        }
    }
    let (bus, vendor, product) = id?;
    Some((bus, vendor, product, name))
}

/// List Logitech devices this OS's transport can open: every Logitech hidraw node on
/// Linux; the HID++ long-report collection of each Logitech device elsewhere.
pub fn logitech() -> io::Result<Vec<DeviceInfo>> {
    #[cfg(target_os = "linux")]
    {
        scan(
            std::path::Path::new("/sys/class/hidraw"),
            std::path::Path::new("/dev"),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        crate::hid::enumerate()
    }
}

#[cfg(target_os = "linux")]
fn scan(sys_root: &std::path::Path, dev_root: &std::path::Path) -> io::Result<Vec<DeviceInfo>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(sys_root)? {
        let entry = entry?;
        let uevent = entry.path().join("device/uevent");
        let Ok(body) = std::fs::read_to_string(&uevent) else {
            continue;
        };
        let Some((bus, vendor, product, name)) = parse_uevent(&body) else {
            continue;
        };
        if vendor != LOGITECH_VENDOR {
            continue;
        }
        let uniq = body
            .lines()
            .find_map(|l| l.strip_prefix("HID_UNIQ="))
            .filter(|u| !u.is_empty())
            .map(str::to_string);
        let receiver = is_receiver_node(&entry.path(), product, &body);
        // A receiver's keyboard / mouse / touch interfaces never speak HID++.
        if is_receiver(product) && !receiver {
            continue;
        }
        out.push(DeviceInfo {
            node: dev_root.join(entry.file_name()),
            bus,
            vendor,
            product,
            name,
            uniq,
            receiver,
        });
    }
    out.sort_by(|a, b| a.node.cmp(&b.node));
    Ok(out)
}

/// A receiver's HID++ interface that Detent must address by slot. A receiver splits into
/// several hidraw nodes (keyboard, mouse, HID++, ...); only the one whose descriptor has
/// the HID++ long collection counts. When `hid-logitech-dj` owns the receiver, the kernel
/// gives every paired device its own node (reached at 0xFF), so the receiver is skipped.
/// ⚠ The dj case is unverified (Linux 7.3 doesn't bind dj to the Bolt receiver).
#[cfg(target_os = "linux")]
fn is_receiver_node(sys_node: &std::path::Path, product: u32, uevent: &str) -> bool {
    if !is_receiver(product) || uevent.lines().any(|l| l == "DRIVER=logitech-djreceiver") {
        return false;
    }
    std::fs::read(sys_node.join("device/report_descriptor"))
        .is_ok_and(|d| has_hidpp_long_collection(&d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mx_master_4_bluetooth() {
        let body =
            "DRIVER=hid-generic\nHID_ID=0005:0000046D:0000B042\nHID_NAME=Logitech MX Master 4\n";
        let (bus, vendor, product, name) = parse_uevent(body).unwrap();
        assert_eq!(bus, Bus::Bluetooth);
        assert_eq!(vendor, LOGITECH_VENDOR);
        assert_eq!(product, 0xB042);
        assert_eq!(name, "Logitech MX Master 4");
    }

    #[test]
    fn ignores_garbage() {
        assert!(parse_uevent("HID_NAME=x\n").is_none());
        assert!(parse_uevent("HID_ID=zz:yy\n").is_none());
    }

    #[test]
    fn finds_the_hidpp_interface_of_a_bolt_receiver() {
        // Interface 2 of the owner's Bolt receiver (hidraw10), verbatim.
        let hidpp = [
            0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0x85, 0x10, 0x95, 0x06, 0x75, 0x08, 0x15,
            0x00, 0x26, 0xFF, 0x00, 0x09, 0x01, 0x81, 0x00, 0x09, 0x01, 0x91, 0x00, 0xC0, 0x06,
            0x00, 0xFF, 0x09, 0x02, 0xA1, 0x01, 0x85, 0x11, 0x95, 0x13, 0x75, 0x08, 0x15, 0x00,
            0x26, 0xFF, 0x00, 0x09, 0x02, 0x81, 0x00, 0x09, 0x02, 0x91, 0x00, 0xC0,
        ];
        // Interface 0 (boot keyboard).
        let keyboard = [0x05, 0x01, 0x09, 0x06, 0xA1, 0x01, 0x95, 0x08];
        assert!(has_hidpp_long_collection(&hidpp));
        assert!(!has_hidpp_long_collection(&keyboard));
        assert!(is_receiver(0xC548));
        assert!(!is_receiver(0xB042));
    }

    #[test]
    fn picks_the_long_report_collections() {
        assert!(is_hidpp_long_collection(0xFF00, 0x0002));
        assert!(is_hidpp_long_collection(0xFF43, 0x0202));
        // the short-report collection, the mouse itself, the haptics collection
        assert!(!is_hidpp_long_collection(0xFF00, 0x0001));
        assert!(!is_hidpp_long_collection(0x0001, 0x0002));
        assert!(!is_hidpp_long_collection(0x000E, 0x0001));
    }
}
