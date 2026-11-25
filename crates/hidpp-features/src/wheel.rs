//! Hi-res wheel (0x2121) and thumbwheel (0x2150).
//!
//! 0x2121:
//! - fn 0 getWheelCapability → `multiplier u8, flags u8 (0x08 has ratchet switch, 0x04 has invert)`
//! - fn 1 getWheelMode → `mode u8: bit0 target (1 = HID++ diverted), bit1 hi-res, bit2 inverted`
//! - fn 2 setWheelMode(mode)
//! - fn 3 getRatchetSwitchState → `state u8 (0 free, 1 ratchet)`
//! - event 0 wheelMovement, event 1 ratchetSwitch
//!
//! The kernel driver (`hid-logitech-hidpp`) enables hi-res mode itself on supported
//! devices. Detent shows the mode but never changes `hires` unless the user asks
//! (docs/specs/hidpp.md, "Coexistence with the kernel driver").
//!
//! 0x2150:
//! - fn 0 getInfo → `native_res u16, diverted_res u16, default_dir u8 (bit0 inverted), caps u8, ...`
//! - fn 1 getStatus → `diverted u8, inverted u8, ...`
//! - fn 2 setReporting(diverted u8, inverted u8)
//! - event 0: `rotation i16, timestamp u16, status u8, flags u8`

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{Feature, FieldKind, FieldSchema, RequestError, Requester};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WheelState {
    pub hires: bool,
    pub invert: bool,
    /// Wheel events go to software (HID++) instead of the OS.
    pub diverted: bool,
    /// Hi-res events per ratchet detent.
    pub multiplier: u8,
    /// Ratchet switch currently engaged (None if the wheel has no switch).
    pub ratchet: Option<bool>,
}

pub mod wheel_mode {
    pub const TARGET: u8 = 0x01;
    pub const HIRES: u8 = 0x02;
    pub const INVERT: u8 = 0x04;
}

pub struct HiResWheel;

fn idx(dev: &dyn Requester, id: FeatureId) -> Result<u8, RequestError> {
    dev.feature_index(id).ok_or(RequestError::Unsupported(id))
}

pub fn set_wheel_mode(
    dev: &mut dyn Requester,
    hires: bool,
    invert: bool,
    diverted: bool,
) -> Result<(), RequestError> {
    let i = idx(dev, FeatureId::HIRES_WHEEL)?;
    let mut m = 0;
    if diverted {
        m |= wheel_mode::TARGET;
    }
    if hires {
        m |= wheel_mode::HIRES;
    }
    if invert {
        m |= wheel_mode::INVERT;
    }
    dev.request(i, 2, &[m])?;
    Ok(())
}

impl Feature for HiResWheel {
    const ID: FeatureId = FeatureId::HIRES_WHEEL;
    type State = WheelState;

    fn read(dev: &mut dyn Requester) -> Result<WheelState, RequestError> {
        let i = idx(dev, Self::ID)?;
        let cap = dev.request(i, 0, &[])?;
        let cp = cap.params();
        let multiplier = cp.first().copied().unwrap_or(1);
        let has_switch = cp.get(1).is_some_and(|f| f & 0x08 != 0);
        let mode = *dev
            .request(i, 1, &[])?
            .params()
            .first()
            .ok_or(RequestError::Malformed)?;
        let ratchet = if has_switch {
            dev.request(i, 3, &[])?
                .params()
                .first()
                .map(|s| s & 0x01 != 0)
        } else {
            None
        };
        Ok(WheelState {
            hires: mode & wheel_mode::HIRES != 0,
            invert: mode & wheel_mode::INVERT != 0,
            diverted: mode & wheel_mode::TARGET != 0,
            multiplier,
            ratchet,
        })
    }

    fn apply(
        dev: &mut dyn Requester,
        desired: &WheelState,
        current: &WheelState,
    ) -> Result<(), RequestError> {
        if (desired.hires, desired.invert, desired.diverted)
            != (current.hires, current.invert, current.diverted)
        {
            set_wheel_mode(dev, desired.hires, desired.invert, desired.diverted)?;
        }
        Ok(())
    }

    fn schema() -> &'static [FieldSchema] {
        &[
            FieldSchema {
                path: "invert",
                kind: FieldKind::Bool,
                doc: "Invert scroll direction (natural scrolling)",
            },
            FieldSchema {
                path: "hires",
                kind: FieldKind::Bool,
                doc: "High-resolution scrolling (managed by the kernel driver)",
            },
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThumbwheelState {
    pub invert: bool,
    pub diverted: bool,
    pub native_res: u16,
    pub diverted_res: u16,
}

pub struct Thumbwheel;

pub fn set_thumbwheel(
    dev: &mut dyn Requester,
    diverted: bool,
    invert: bool,
) -> Result<(), RequestError> {
    let i = idx(dev, FeatureId::THUMBWHEEL)?;
    dev.request(i, 2, &[u8::from(diverted), u8::from(invert)])?;
    Ok(())
}

impl Feature for Thumbwheel {
    const ID: FeatureId = FeatureId::THUMBWHEEL;
    type State = ThumbwheelState;

    fn read(dev: &mut dyn Requester) -> Result<ThumbwheelState, RequestError> {
        let i = idx(dev, Self::ID)?;
        let info = dev.request(i, 0, &[])?;
        let ip = info.params();
        if ip.len() < 4 {
            return Err(RequestError::Malformed);
        }
        let st = dev.request(i, 1, &[])?;
        let sp = st.params();
        if sp.len() < 2 {
            return Err(RequestError::Malformed);
        }
        Ok(ThumbwheelState {
            native_res: u16::from_be_bytes([ip[0], ip[1]]),
            diverted_res: u16::from_be_bytes([ip[2], ip[3]]),
            diverted: sp[0] & 0x01 != 0,
            invert: sp[1] & 0x01 != 0,
        })
    }

    fn apply(
        dev: &mut dyn Requester,
        desired: &ThumbwheelState,
        current: &ThumbwheelState,
    ) -> Result<(), RequestError> {
        if (desired.diverted, desired.invert) != (current.diverted, current.invert) {
            set_thumbwheel(dev, desired.diverted, desired.invert)?;
        }
        Ok(())
    }

    fn schema() -> &'static [FieldSchema] {
        &[FieldSchema {
            path: "invert",
            kind: FieldKind::Bool,
            doc: "Invert thumbwheel direction",
        }]
    }
}

/// Thumbwheel event 0 (diverted rotation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbwheelEvent {
    pub rotation: i16,
    pub status: u8,
    pub touch: bool,
}

impl ThumbwheelEvent {
    #[inline]
    pub fn decode(p: &[u8]) -> Option<Self> {
        if p.len() < 6 {
            return None;
        }
        Some(Self {
            rotation: i16::from_be_bytes([p[0], p[1]]),
            status: p[4],
            touch: p[5] & 0x01 != 0,
        })
    }
}
