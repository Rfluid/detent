//! Receivers (Logi Bolt): which devices are paired, at which index, and what they are.
//!
//! A receiver speaks HID++ 1.0 registers at index 0xFF; its paired devices speak HID++ 2.0
//! at indexes 1–6 through the same hidraw node (docs/specs/hidpp.md, "Receivers").
//! Everything here is a read: register reads (sub-id 0x81 / 0x83) and nothing else.
//!
//! Register reads have no software id, so they don't go through [`crate::DirectRequester`]:
//! a reply is matched on (device index 0xFF, sub-id, register; plus the echoed sub-address
//! for 0xB5) and an error on `8F <sub-id> <register> <code>`.

use std::time::{Duration, Instant};

use detent_hidpp_features::RequestError;
use detent_hidpp_proto::{DeviceIndex, Frame};
use detent_hidpp_transport::Transport;
use detent_hidpp_transport::discover::RECEIVER_SLOTS;
use serde::Serialize;

/// HID++ 1.0 sub-ids.
const GET_REGISTER: u8 = 0x81;
const GET_LONG_REGISTER: u8 = 0x83;
const ERROR: u8 = 0x8F;
/// Receiver info register; Bolt keeps per-slot pairing info at 0x50+n, names at 0x60+n.
const RECEIVER_INFO: u8 = 0xB5;
const BOLT_PAIRING: u8 = 0x50;
const BOLT_NAME: u8 = 0x60;
/// Receiver replies take a few ms; Solaar waits 0.9 s (docs/research/hidpp-layouts-protocol.md).
const TIMEOUT: Duration = Duration::from_millis(900);

/// One device paired to a receiver (online or not).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Paired {
    /// 1–6.
    pub slot: u8,
    /// Wireless product id, e.g. 0xB042 for the MX Master 4 (same as its Bluetooth PID).
    pub wpid: u16,
    /// The device's unit id (equals `DeviceInformation.getDeviceInfo` unitId), uppercase hex.
    pub serial: String,
    /// 1 keyboard, 2 mouse, ... (low nibble of the pairing info's kind byte).
    pub kind: u8,
    pub name: Option<String>,
}

impl Paired {
    pub fn index(&self) -> DeviceIndex {
        DeviceIndex(self.slot)
    }
}

/// Read a receiver register. `long` selects 0x83 (16-byte reply) over 0x81 (3 bytes).
/// Returns the reply's bytes after the register address.
///
/// The owner's Bolt receiver sometimes answers the first request after a quiet spell with
/// a spurious error (`8F 83 B5 08` for a slot that is paired) and gets it right on the
/// next try (2026-11-25), so a device error is retried once.
pub fn read_register<T: Transport>(
    t: &mut T,
    long: bool,
    register: u8,
    params: [u8; 3],
) -> Result<Vec<u8>, RequestError> {
    match read_register_once(t, long, register, params) {
        Err(RequestError::Device(_)) => {
            std::thread::sleep(Duration::from_millis(20));
            read_register_once(t, long, register, params)
        }
        r => r,
    }
}

fn read_register_once<T: Transport>(
    t: &mut T,
    long: bool,
    register: u8,
    params: [u8; 3],
) -> Result<Vec<u8>, RequestError> {
    let sub = if long {
        GET_LONG_REGISTER
    } else {
        GET_REGISTER
    };
    let req = [0x10, 0xFF, sub, register, params[0], params[1], params[2]];
    let req = Frame::parse(&req).map_err(|_| RequestError::Malformed)?;
    send(t, &req)?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(RequestError::Timeout);
        }
        let Some(f) = t.recv(Some(deadline - now))? else {
            return Err(RequestError::Timeout);
        };
        let b = f.as_bytes();
        if b.len() < 6 || b[1] != 0xFF {
            continue;
        }
        // 0xB5 replies echo the sub-address (first param); other registers carry data there.
        if b[2] == sub && b[3] == register && (register != RECEIVER_INFO || b[4] == params[0]) {
            return Ok(b[4..].to_vec());
        }
        if b[2] == ERROR && b[3] == sub && b[4] == register {
            return Err(RequestError::Device(b[5]));
        }
    }
}

fn send<T: Transport>(t: &mut T, f: &Frame) -> Result<(), RequestError> {
    Ok(crate::requester::send_retrying(t, f)?)
}

/// Bolt pairing info for one slot. `Ok(None)`: nothing paired there.
pub fn bolt_paired<T: Transport>(t: &mut T, slot: u8) -> Result<Option<Paired>, RequestError> {
    let info = match read_register(t, true, RECEIVER_INFO, [BOLT_PAIRING + slot, 0, 0]) {
        Ok(v) => v,
        // The receiver answers an empty slot with an error (verified: 0x08 unknown device).
        Err(RequestError::Device(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    let Some(p) = parse_bolt_pairing(slot, &info) else {
        return Ok(None);
    };
    let name = read_register(t, true, RECEIVER_INFO, [BOLT_NAME + slot, 1, 0])
        .ok()
        .and_then(|v| parse_bolt_name(&v));
    Ok(Some(Paired { name, ..p }))
}

/// Every device paired to a Bolt receiver, by slot.
pub fn bolt_paired_all<T: Transport>(t: &mut T) -> Result<Vec<Paired>, RequestError> {
    let mut out = Vec::new();
    for slot in RECEIVER_SLOTS {
        if let Some(p) = bolt_paired(t, slot)? {
            out.push(p);
        }
    }
    Ok(out)
}

/// `[0x50+n, kind, wpid lo, wpid hi, unit id ×4, ...]` (bytes after the register address).
/// Verified on the owner's receiver: `52 02 42 B0 A2 04 BE 6C ...` = slot 2, mouse, B042,
/// unit id A204BE6C (the mouse's own 0x0003 unitId).
fn parse_bolt_pairing(slot: u8, v: &[u8]) -> Option<Paired> {
    if v.len() < 8 {
        return None;
    }
    let wpid = u16::from_le_bytes([v[2], v[3]]);
    if wpid == 0 {
        return None;
    }
    Some(Paired {
        slot,
        wpid,
        serial: v[4..8].iter().map(|b| format!("{b:02X}")).collect(),
        kind: v[1] & 0x0F,
        name: None,
    })
}

/// `[0x60+n, page, len, utf-8...]`. Verified: `62 01 0B "MX Master 4"`.
fn parse_bolt_name(v: &[u8]) -> Option<String> {
    let len = usize::from(*v.get(2)?);
    let bytes = v.get(3..3 + len).or_else(|| v.get(3..))?;
    let s = String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_string();
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use detent_hidpp_transport::mock::MockTransport;

    fn frame(bytes: &[u8]) -> Frame {
        let len = if bytes[0] == 0x10 { 7 } else { 20 };
        let mut b = vec![0u8; len];
        b[..bytes.len()].copy_from_slice(bytes);
        Frame::parse(&b).unwrap()
    }

    #[test]
    fn reads_the_owners_bolt_slot() {
        let mut t = MockTransport::new();
        // stray traffic first: a device notification and another slot's register reply
        t.push_inbound(frame(&[0x11, 0x02, 0x09, 0x00, 0x55, 0x08]));
        t.push_inbound(frame(&[
            0x11, 0xFF, 0x83, 0xB5, 0x52, 0x02, 0x42, 0xB0, 0xA2, 0x04, 0xBE, 0x6C, 0x02, 0x01,
            0x80, 0x14, 0x01,
        ]));
        t.push_inbound(frame(&[
            0x11, 0xFF, 0x83, 0xB5, 0x62, 0x01, 0x0B, b'M', b'X', b' ', b'M', b'a', b's', b't',
            b'e', b'r', b' ', b'4',
        ]));
        let p = bolt_paired(&mut t, 2).unwrap().unwrap();
        assert_eq!(
            p,
            Paired {
                slot: 2,
                wpid: 0xB042,
                serial: "A204BE6C".into(),
                kind: 2,
                name: Some("MX Master 4".into()),
            }
        );
        assert_eq!(
            t.sent[0].as_bytes(),
            &[0x10, 0xFF, 0x83, 0xB5, 0x52, 0x00, 0x00]
        );
        assert_eq!(
            t.sent[1].as_bytes(),
            &[0x10, 0xFF, 0x83, 0xB5, 0x62, 0x01, 0x00]
        );
    }

    #[test]
    fn empty_slot_is_none() {
        let mut t = MockTransport::new();
        // verbatim from the owner's receiver (slot 1 empty); asked twice (one retry)
        t.push_inbound(frame(&[0x10, 0xFF, 0x8F, 0x83, 0xB5, 0x08]));
        t.push_inbound(frame(&[0x10, 0xFF, 0x8F, 0x83, 0xB5, 0x08]));
        assert_eq!(bolt_paired(&mut t, 1).unwrap(), None);
        assert_eq!(t.sent.len(), 2);
    }

    #[test]
    fn spurious_error_is_retried() {
        let mut t = MockTransport::new();
        t.push_inbound(frame(&[0x10, 0xFF, 0x8F, 0x83, 0xB5, 0x08]));
        t.push_inbound(frame(&[
            0x11, 0xFF, 0x83, 0xB5, 0x52, 0x02, 0x42, 0xB0, 0xA2, 0x04, 0xBE, 0x6C,
        ]));
        let v = read_register(&mut t, true, 0xB5, [0x52, 0, 0]).unwrap();
        assert_eq!(&v[..4], &[0x52, 0x02, 0x42, 0xB0]);
    }

    #[test]
    fn short_register_read() {
        let mut t = MockTransport::new();
        t.push_inbound(frame(&[0x10, 0xFF, 0x81, 0x02, 0x00, 0x01, 0x00]));
        assert_eq!(
            read_register(&mut t, false, 0x02, [0, 0, 0]).unwrap(),
            vec![0x00, 0x01, 0x00]
        );
    }
}
