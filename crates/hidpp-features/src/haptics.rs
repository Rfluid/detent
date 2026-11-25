//! Haptics (0x19B0), MX Master 4. No public Logitech spec; layout from prior art
//! (Solaar, OpenLogi, familiar; see docs/research/prior-art.md, "Haptics") and verified
//! read-only on the owner's device (docs/specs/devices/mx-master-4.md).
//!
//! - fn 0 getCapabilities → bytes 4..8: u32 BE bitmap of supported waveforms (bit N = id N)
//! - fn 1 getConfig → `flags u8 (bit0 enabled, bit1 battery saving), intensity u8 (0..=100), levels u8`
//! - fn 2 setConfig(flags, intensity, 0) → the applied config
//! - fn 4 play(waveform, 0, 0): one-shot, not stored on the device; safe to repeat every ≥ 12 ms

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{Feature, FieldKind, FieldSchema, RequestError, Requester};

pub const ID: FeatureId = FeatureId::HAPTICS;

/// Waveform IDs (names as published by prior art).
pub mod waveform {
    pub const SHARP_STATE_CHANGE: u8 = 0x00;
    pub const DAMP_STATE_CHANGE: u8 = 0x01;
    pub const SHARP_COLLISION: u8 = 0x02;
    pub const DAMP_COLLISION: u8 = 0x03;
    pub const SUBTLE_COLLISION: u8 = 0x04;
    pub const HAPPY_ALERT: u8 = 0x05;
    pub const ANGRY_ALERT: u8 = 0x06;
    pub const COMPLETED: u8 = 0x07;
    pub const SQUARE: u8 = 0x08;
    pub const WAVE: u8 = 0x09;
    pub const FIREWORK: u8 = 0x0A;
    pub const MAD: u8 = 0x0B;
    pub const KNOCK: u8 = 0x0C;
    pub const JINGLE: u8 = 0x0D;
    pub const RINGING: u8 = 0x0E;
    pub const WHISPER_COLLISION: u8 = 0x1B;

    pub const NAMES: &[(u8, &str)] = &[
        (SHARP_STATE_CHANGE, "sharp-state-change"),
        (DAMP_STATE_CHANGE, "damp-state-change"),
        (SHARP_COLLISION, "sharp-collision"),
        (DAMP_COLLISION, "damp-collision"),
        (SUBTLE_COLLISION, "subtle-collision"),
        (HAPPY_ALERT, "happy-alert"),
        (ANGRY_ALERT, "angry-alert"),
        (COMPLETED, "completed"),
        (SQUARE, "square"),
        (WAVE, "wave"),
        (FIREWORK, "firework"),
        (MAD, "mad"),
        (KNOCK, "knock"),
        (JINGLE, "jingle"),
        (RINGING, "ringing"),
        (WHISPER_COLLISION, "whisper-collision"),
    ];

    pub fn by_name(name: &str) -> Option<u8> {
        NAMES.iter().find(|(_, n)| *n == name).map(|(id, _)| *id)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub enabled: bool,
    /// 0..=100
    pub intensity: u8,
    #[serde(default)]
    pub battery_saving: bool,
    /// Waveform IDs this unit supports (read-only).
    #[serde(default)]
    pub waveforms: Vec<u8>,
}

/// A one-shot effect sent on demand (e.g. radial menu segment tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pulse {
    pub waveform: u8,
}

fn index(dev: &dyn Requester) -> Result<u8, RequestError> {
    dev.feature_index(ID).ok_or(RequestError::Unsupported(ID))
}

pub fn decode_waveforms(caps: &[u8]) -> Vec<u8> {
    let Some(b) = caps.get(4..8) else {
        return Vec::new();
    };
    let mask = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    (0..32u8).filter(|i| mask & (1 << i) != 0).collect()
}

pub fn play(dev: &mut dyn Requester, waveform: u8) -> Result<(), RequestError> {
    let i = index(dev)?;
    dev.request(i, 4, &[waveform, 0, 0])?;
    Ok(())
}

pub struct Haptics;

impl Feature for Haptics {
    const ID: FeatureId = ID;
    type State = State;

    fn read(dev: &mut dyn Requester) -> Result<State, RequestError> {
        let i = index(dev)?;
        let caps = dev.request(i, 0, &[])?;
        let cfg = dev.request(i, 1, &[])?;
        let p = cfg.params();
        if p.len() < 2 {
            return Err(RequestError::Malformed);
        }
        Ok(State {
            enabled: p[0] & 0x01 != 0,
            battery_saving: p[0] & 0x02 != 0,
            intensity: p[1].min(100),
            waveforms: decode_waveforms(caps.params()),
        })
    }

    fn apply(
        dev: &mut dyn Requester,
        desired: &State,
        current: &State,
    ) -> Result<(), RequestError> {
        if (desired.enabled, desired.intensity, desired.battery_saving)
            == (current.enabled, current.intensity, current.battery_saving)
        {
            return Ok(());
        }
        if desired.intensity > 100 {
            return Err(RequestError::OutOfRange(format!(
                "haptic intensity {} (0..=100)",
                desired.intensity
            )));
        }
        let i = index(dev)?;
        let flags = u8::from(desired.enabled) | (u8::from(desired.battery_saving) << 1);
        dev.request(i, 2, &[flags, desired.intensity, 0])?;
        Ok(())
    }

    fn schema() -> &'static [FieldSchema] {
        &[
            FieldSchema {
                path: "enabled",
                kind: FieldKind::Bool,
                doc: "Haptic feedback",
            },
            FieldSchema {
                path: "intensity",
                kind: FieldKind::Range {
                    min: 0,
                    max: 100,
                    step: 1,
                },
                doc: "Haptic strength",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_bitmap() {
        assert_eq!(
            decode_waveforms(&[0, 0, 0, 0, 0, 0, 0x00, 0x13]),
            vec![0, 1, 4]
        );
        assert!(decode_waveforms(&[0; 3]).is_empty());
        assert_eq!(waveform::by_name("subtle-collision"), Some(4));
    }
}
