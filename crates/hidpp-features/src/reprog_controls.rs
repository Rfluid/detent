//! Reprogrammable Controls v4 (0x1B04): the core of the event path.
//!
//! Functions (layouts in docs/specs/hidpp.md and docs/research/prior-art.md):
//! - fn 0 getCount → n
//! - fn 1 getCidInfo(i) → `cid u16, tid u16, flags u8, pos u8, group u8, gmask u8, flags2 u8`
//! - fn 2 getCidReporting(cid) → `cid u16, report u8, remap u16, report2 u8`
//! - fn 3 setCidReporting(cid, report, remap, report2) → echo
//!
//! Notifications: event 0 divertedButtons (up to 4 CIDs currently held, u16 BE each;
//! a release shows up as a CID missing from the list), event 1 divertedRawXY
//! (dx i16 BE, dy i16 BE).
//!
//! Diversion is volatile: the device forgets it on sleep/reconnect/host switch.
//! The reconciler re-applies [`State::diverted`] when the link comes back.

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{Feature, FieldKind, FieldSchema, RequestError, Requester};

/// Control ID as reported by the device (e.g. gesture button).
pub type Cid = u16;

/// getCidInfo `flags` byte.
pub mod flag {
    pub const MOUSE: u8 = 0x01;
    pub const FKEY: u8 = 0x02;
    pub const HOTKEY: u8 = 0x04;
    pub const FN_TOGGLE: u8 = 0x08;
    pub const REPROG_HINT: u8 = 0x10;
    pub const DIVERTABLE: u8 = 0x20;
    pub const PERSIST_DIVERTABLE: u8 = 0x40;
    pub const VIRTUAL: u8 = 0x80;
}

/// getCidInfo `flags2` byte (additional flags).
pub mod flag2 {
    pub const RAW_XY: u8 = 0x01;
    pub const FORCE_RAW_XY: u8 = 0x02;
    pub const ANALYTICS: u8 = 0x04;
    pub const RAW_WHEEL: u8 = 0x08;
}

/// getCidReporting / setCidReporting `report` byte: each setting has a "valid" bit that
/// must be set for the device to apply it.
pub mod report {
    pub const DIVERT: u8 = 0x01;
    pub const DIVERT_VALID: u8 = 0x02;
    pub const PERSIST: u8 = 0x04;
    pub const PERSIST_VALID: u8 = 0x08;
    pub const RAW_XY: u8 = 0x10;
    pub const RAW_XY_VALID: u8 = 0x20;
    pub const FORCE_RAW_XY: u8 = 0x40;
    pub const FORCE_RAW_XY_VALID: u8 = 0x80;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlInfo {
    pub cid: Cid,
    pub task_id: u16,
    pub flags: u8,
    pub flags2: u8,
    pub pos: u8,
    pub group: u8,
    pub gmask: u8,
    /// Can be diverted to software. Left/right click usually cannot.
    pub divertable: bool,
    pub persistently_divertable: bool,
    pub virtual_: bool,
    pub raw_xy_capable: bool,
    pub raw_wheel_capable: bool,
}

impl ControlInfo {
    pub fn decode(p: &[u8]) -> Result<Self, RequestError> {
        if p.len() < 9 {
            return Err(RequestError::Malformed);
        }
        let flags = p[4];
        let flags2 = p[8];
        Ok(Self {
            cid: u16::from_be_bytes([p[0], p[1]]),
            task_id: u16::from_be_bytes([p[2], p[3]]),
            flags,
            flags2,
            pos: p[5],
            group: p[6],
            gmask: p[7],
            divertable: flags & flag::DIVERTABLE != 0,
            persistently_divertable: flags & flag::PERSIST_DIVERTABLE != 0,
            virtual_: flags & flag::VIRTUAL != 0,
            raw_xy_capable: flags2 & flag2::RAW_XY != 0,
            raw_wheel_capable: flags2 & flag2::RAW_WHEEL != 0,
        })
    }
}

/// Current reporting of one control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reporting {
    pub cid: Cid,
    pub diverted: bool,
    pub persist: bool,
    pub raw_xy: bool,
    pub force_raw_xy: bool,
    /// CID this control is remapped to (0 or own CID = not remapped).
    pub remap: Cid,
}

impl Reporting {
    pub fn decode(p: &[u8]) -> Result<Self, RequestError> {
        if p.len() < 5 {
            return Err(RequestError::Malformed);
        }
        let r = p[2];
        Ok(Self {
            cid: u16::from_be_bytes([p[0], p[1]]),
            diverted: r & report::DIVERT != 0,
            persist: r & report::PERSIST != 0,
            raw_xy: r & report::RAW_XY != 0,
            force_raw_xy: r & report::FORCE_RAW_XY != 0,
            remap: u16::from_be_bytes([p[3], p[4]]),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub controls: Vec<ControlInfo>,
    pub diverted: Vec<Cid>,
    pub raw_xy: Vec<Cid>,
}

fn index(dev: &dyn Requester) -> Result<u8, RequestError> {
    dev.feature_index(FeatureId::REPROG_CONTROLS_V4)
        .ok_or(RequestError::Unsupported(FeatureId::REPROG_CONTROLS_V4))
}

/// getCount + getCidInfo for every control.
pub fn controls(dev: &mut dyn Requester) -> Result<Vec<ControlInfo>, RequestError> {
    let idx = index(dev)?;
    let count = *dev
        .request(idx, 0, &[])?
        .params()
        .first()
        .ok_or(RequestError::Malformed)?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        out.push(ControlInfo::decode(dev.request(idx, 1, &[i])?.params())?);
    }
    Ok(out)
}

/// getCidReporting(cid). Read-only.
pub fn reporting(dev: &mut dyn Requester, cid: Cid) -> Result<Reporting, RequestError> {
    let idx = index(dev)?;
    let c = cid.to_be_bytes();
    Reporting::decode(dev.request(idx, 2, &[c[0], c[1]])?.params())
}

/// setCidReporting(cid): set (or clear) temporary diversion and raw XY.
/// Persistent diversion is never touched (we only set its valid bit to 0).
pub fn set_reporting(
    dev: &mut dyn Requester,
    cid: Cid,
    divert: bool,
    raw_xy: bool,
) -> Result<Reporting, RequestError> {
    let idx = index(dev)?;
    let c = cid.to_be_bytes();
    let mut r = report::DIVERT_VALID | report::RAW_XY_VALID;
    if divert {
        r |= report::DIVERT;
    }
    if raw_xy {
        r |= report::RAW_XY;
    }
    // remap = 0 means "leave unchanged"
    Reporting::decode(dev.request(idx, 3, &[c[0], c[1], r, 0, 0])?.params())
}

pub struct ReprogControls;

impl Feature for ReprogControls {
    const ID: FeatureId = FeatureId::REPROG_CONTROLS_V4;
    type State = State;

    fn read(dev: &mut dyn Requester) -> Result<State, RequestError> {
        let controls = controls(dev)?;
        let mut diverted = Vec::new();
        let mut raw_xy = Vec::new();
        for c in controls.iter().filter(|c| c.divertable) {
            let r = reporting(dev, c.cid)?;
            if r.diverted {
                diverted.push(c.cid);
            }
            if r.raw_xy {
                raw_xy.push(c.cid);
            }
        }
        Ok(State {
            controls,
            diverted,
            raw_xy,
        })
    }

    fn apply(
        dev: &mut dyn Requester,
        desired: &State,
        current: &State,
    ) -> Result<(), RequestError> {
        for c in desired.controls.iter().filter(|c| c.divertable) {
            let want = (
                desired.diverted.contains(&c.cid),
                desired.raw_xy.contains(&c.cid),
            );
            let have = (
                current.diverted.contains(&c.cid),
                current.raw_xy.contains(&c.cid),
            );
            if want != have {
                set_reporting(dev, c.cid, want.0, want.1 && c.raw_xy_capable)?;
            }
        }
        Ok(())
    }

    fn schema() -> &'static [FieldSchema] {
        &[
            FieldSchema {
                path: "controls",
                kind: FieldKind::ReadOnly,
                doc: "Controls and capabilities",
            },
            FieldSchema {
                path: "diverted",
                kind: FieldKind::ReadOnly,
                doc: "Managed by bindings",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_cid_info() {
        // gesture-button-like control: divertable + persist + virtual? no; raw XY capable
        let p = [0x00, 0xC3, 0x00, 0xA9, 0x30, 0x00, 0x03, 0x1F, 0x01, 0, 0];
        let c = ControlInfo::decode(&p).unwrap();
        assert_eq!(c.cid, 0x00C3);
        assert_eq!(c.task_id, 0x00A9);
        assert!(c.divertable && !c.persistently_divertable);
        assert!(c.raw_xy_capable);
        assert!(!c.virtual_);
        assert!(ControlInfo::decode(&p[..5]).is_err());
    }

    #[test]
    fn decodes_reporting() {
        let r = Reporting::decode(&[0x00, 0xC3, 0x11, 0x00, 0x00, 0]).unwrap();
        assert!(r.diverted && r.raw_xy && !r.persist);
        assert_eq!(r.remap, 0);
    }
}
