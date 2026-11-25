//! SmartShift Enhanced (0x2111): ratchet vs free-spin wheel, auto-shift and torque.
//!
//! - fn 0 getCapabilities → `caps u8 (bit0 tunable torque), auto_default u8, torque_default u8, max_force u8`
//! - fn 1 getRatchetControlMode → `mode u8 (1 free-spin, 2 ratchet), auto_disengage u8, torque u8`
//! - fn 2 setRatchetControlMode(mode, auto_disengage, torque); 0 in a field = "don't change"
//!
//! `auto_disengage`: wheel speed above which ratchet disengages (1..=254, 255 = never).
//! The legacy 0x2110 has the same fn 0/1 layout as 0x2111's fn 1/2 without torque; the
//! MX Master 4 exposes only 0x2111.

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{Feature, FieldKind, FieldSchema, RequestError, Requester};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WheelMode {
    Ratchet,
    FreeSpin,
}

impl WheelMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "ratchet" => Some(Self::Ratchet),
            "free-spin" | "freespin" | "free_spin" => Some(Self::FreeSpin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub mode: WheelMode,
    /// Auto-shift threshold: speed at which ratchet disengages. 255 = never auto-shift.
    pub auto_threshold: u8,
    /// Ratchet force in percent (if the device supports tunable torque).
    pub torque: Option<u8>,
}

fn index(dev: &dyn Requester) -> Result<u8, RequestError> {
    dev.feature_index(FeatureId::SMART_SHIFT_ENHANCED)
        .ok_or(RequestError::Unsupported(FeatureId::SMART_SHIFT_ENHANCED))
}

pub fn set(
    dev: &mut dyn Requester,
    mode: Option<WheelMode>,
    auto_threshold: Option<u8>,
    torque: Option<u8>,
) -> Result<(), RequestError> {
    let idx = index(dev)?;
    let m = match mode {
        None => 0,
        Some(WheelMode::FreeSpin) => 1,
        Some(WheelMode::Ratchet) => 2,
    };
    dev.request(
        idx,
        2,
        &[m, auto_threshold.unwrap_or(0), torque.unwrap_or(0)],
    )?;
    Ok(())
}

pub struct SmartShift;

impl Feature for SmartShift {
    const ID: FeatureId = FeatureId::SMART_SHIFT_ENHANCED;
    type State = State;

    fn read(dev: &mut dyn Requester) -> Result<State, RequestError> {
        let idx = index(dev)?;
        let caps = dev.request(idx, 0, &[])?;
        let tunable = caps.params().first().is_some_and(|c| c & 0x01 != 0);
        let resp = dev.request(idx, 1, &[])?;
        let p = resp.params();
        if p.len() < 3 {
            return Err(RequestError::Malformed);
        }
        Ok(State {
            mode: if p[0] == 2 {
                WheelMode::Ratchet
            } else {
                WheelMode::FreeSpin
            },
            auto_threshold: p[1],
            torque: tunable.then_some(p[2]),
        })
    }

    fn apply(
        dev: &mut dyn Requester,
        desired: &State,
        current: &State,
    ) -> Result<(), RequestError> {
        if desired == current {
            return Ok(());
        }
        let torque = desired.torque.filter(|_| desired.torque != current.torque);
        // MX Master 4 quirk: a torque change only takes effect if the mode byte is written
        // in the same request (docs/research/prior-art.md, "SmartShift").
        let mode = (desired.mode != current.mode || torque.is_some()).then_some(desired.mode);
        set(
            dev,
            mode,
            (desired.auto_threshold != current.auto_threshold).then_some(desired.auto_threshold),
            torque,
        )
    }

    fn schema() -> &'static [FieldSchema] {
        &[
            FieldSchema {
                path: "mode",
                kind: FieldKind::Choice(&["ratchet", "free-spin"]),
                doc: "Wheel mode",
            },
            FieldSchema {
                path: "auto_threshold",
                kind: FieldKind::Range {
                    min: 1,
                    max: 255,
                    step: 1,
                },
                doc: "Auto shift speed (255 = never)",
            },
            FieldSchema {
                path: "torque",
                kind: FieldKind::Range {
                    min: 1,
                    max: 100,
                    step: 1,
                },
                doc: "Ratchet force (%)",
            },
        ]
    }
}
