//! Unified Battery (0x1004). Read-only state + change notifications.
//!
//! - fn 0 getCapabilities → `supported_levels u8, flags u8` (flag bit 1: state of charge in %)
//! - fn 1 getStatus → `soc% u8, level u8 (1 critical, 2 low, 4 good, 8 full), charging u8, external u8`
//! - event 0: same layout as getStatus

use detent_hidpp_proto::{FeatureId, Frame};
use serde::{Deserialize, Serialize};

use crate::{Change, Feature, FieldKind, FieldSchema, RequestError, Requester};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Charging {
    Discharging,
    Charging,
    ChargingSlow,
    Full,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    Critical,
    Low,
    Good,
    Full,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub percent: Option<u8>,
    pub level: Level,
    pub charging: Charging,
    pub external_power: bool,
}

impl State {
    pub fn decode(p: &[u8]) -> Result<Self, RequestError> {
        if p.len() < 4 {
            return Err(RequestError::Malformed);
        }
        let level = match p[1] {
            l if l & 0x08 != 0 => Level::Full,
            l if l & 0x04 != 0 => Level::Good,
            l if l & 0x02 != 0 => Level::Low,
            l if l & 0x01 != 0 => Level::Critical,
            _ => Level::Unknown,
        };
        let charging = match p[2] {
            0 => Charging::Discharging,
            1 => Charging::Charging,
            2 => Charging::ChargingSlow,
            3 => Charging::Full,
            _ => Charging::Error,
        };
        Ok(Self {
            percent: (p[0] > 0 && p[0] <= 100).then_some(p[0]),
            level,
            charging,
            external_power: p[3] != 0,
        })
    }
}

pub struct UnifiedBattery;

impl Feature for UnifiedBattery {
    const ID: FeatureId = FeatureId::UNIFIED_BATTERY;
    type State = State;

    fn read(dev: &mut dyn Requester) -> Result<State, RequestError> {
        let idx = dev
            .feature_index(Self::ID)
            .ok_or(RequestError::Unsupported(Self::ID))?;
        State::decode(dev.request(idx, 1, &[])?.params())
    }

    fn apply(
        _dev: &mut dyn Requester,
        _desired: &State,
        _current: &State,
    ) -> Result<(), RequestError> {
        Ok(()) // read-only
    }

    fn on_notification(frame: &Frame, state: &mut State) -> Option<Change> {
        if frame.function() != 0 {
            return None;
        }
        let new = State::decode(frame.params()).ok()?;
        (new != *state).then(|| {
            *state = new;
            Change::StateChanged
        })
    }

    fn schema() -> &'static [FieldSchema] {
        &[
            FieldSchema {
                path: "percent",
                kind: FieldKind::ReadOnly,
                doc: "Battery level",
            },
            FieldSchema {
                path: "charging",
                kind: FieldKind::ReadOnly,
                doc: "Charging status",
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_status() {
        let s = State::decode(&[80, 0x04, 0, 0]).unwrap();
        assert_eq!(s.percent, Some(80));
        assert_eq!(s.level, Level::Good);
        assert_eq!(s.charging, Charging::Discharging);
        let s = State::decode(&[0, 0x08, 3, 1]).unwrap();
        assert_eq!(s.percent, None);
        assert_eq!(s.charging, Charging::Full);
        assert!(s.external_power);
    }
}
