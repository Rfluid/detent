//! Adjustable DPI (0x2201).
//!
//! - fn 0 getSensorCount → `count u8`
//! - fn 1 getSensorDpiList(sensor) → `sensor u8, (dpi u16 BE)*, 0 terminator`; a value
//!   `0xE000 | step` between two values means "every `step` from the previous to the next"
//! - fn 2 getSensorDpi(sensor) → `sensor u8, dpi u16, default u16`
//! - fn 3 setSensorDpi(sensor, dpi u16) → echo

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{Feature, FieldKind, FieldSchema, RequestError, Requester};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub dpi: u16,
    /// Device default (read-only).
    pub default: u16,
    pub min: u16,
    pub max: u16,
    /// Granularity when the device reports a range; 0 when it reports a list.
    pub step: u16,
    /// Explicit list when the device reports one (empty for ranges).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub list: Vec<u16>,
}

impl State {
    /// Is `dpi` a value the device accepts?
    pub fn accepts(&self, dpi: u16) -> bool {
        if !self.list.is_empty() {
            return self.list.contains(&dpi);
        }
        dpi >= self.min
            && dpi <= self.max
            && (self.step == 0 || (dpi - self.min).is_multiple_of(self.step))
    }
}

/// Parse a DPI list payload (after the sensor byte). Returns (min, max, step, list).
pub fn parse_dpi_list(p: &[u8]) -> Result<(u16, u16, u16, Vec<u16>), RequestError> {
    let mut values = Vec::new();
    let mut step = 0u16;
    let mut ranged = false;
    for pair in p.as_chunks::<2>().0 {
        let v = u16::from_be_bytes([pair[0], pair[1]]);
        if v == 0 {
            break;
        }
        if v >= 0xE000 {
            step = v & 0x1FFF;
            ranged = true;
        } else {
            values.push(v);
        }
    }
    let (Some(&min), Some(&max)) = (values.iter().min(), values.iter().max()) else {
        return Err(RequestError::Malformed);
    };
    Ok(if ranged {
        (min, max, step, Vec::new())
    } else {
        (min, max, 0, values)
    })
}

pub struct AdjustableDpi;

fn index(dev: &dyn Requester) -> Result<u8, RequestError> {
    dev.feature_index(FeatureId::ADJUSTABLE_DPI)
        .ok_or(RequestError::Unsupported(FeatureId::ADJUSTABLE_DPI))
}

pub fn set_dpi(dev: &mut dyn Requester, dpi: u16) -> Result<(), RequestError> {
    let idx = index(dev)?;
    let b = dpi.to_be_bytes();
    dev.request(idx, 3, &[0, b[0], b[1]])?;
    Ok(())
}

impl Feature for AdjustableDpi {
    const ID: FeatureId = FeatureId::ADJUSTABLE_DPI;
    type State = State;

    fn read(dev: &mut dyn Requester) -> Result<State, RequestError> {
        let idx = index(dev)?;
        let list = dev.request(idx, 1, &[0])?;
        let (min, max, step, list) = parse_dpi_list(list.params().get(1..).unwrap_or(&[]))?;
        let cur = dev.request(idx, 2, &[0])?;
        let p = cur.params();
        if p.len() < 5 {
            return Err(RequestError::Malformed);
        }
        Ok(State {
            dpi: u16::from_be_bytes([p[1], p[2]]),
            default: u16::from_be_bytes([p[3], p[4]]),
            min,
            max,
            step,
            list,
        })
    }

    fn apply(
        dev: &mut dyn Requester,
        desired: &State,
        current: &State,
    ) -> Result<(), RequestError> {
        if desired.dpi != current.dpi {
            if !current.accepts(desired.dpi) {
                return Err(RequestError::OutOfRange(format!(
                    "dpi {} (device accepts {}..={} step {})",
                    desired.dpi, current.min, current.max, current.step
                )));
            }
            set_dpi(dev, desired.dpi)?;
        }
        Ok(())
    }

    fn schema() -> &'static [FieldSchema] {
        // Real bounds come from the device (`min`, `max`, `step` in the state).
        &[FieldSchema {
            path: "dpi",
            kind: FieldKind::Range {
                min: 200,
                max: 8000,
                step: 50,
            },
            doc: "Sensor DPI (device bounds in min/max/step)",
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_range_and_list() {
        // 200, step 50, 8000
        let p = [0x00, 0xC8, 0xE0, 0x32, 0x1F, 0x40, 0, 0];
        assert_eq!(parse_dpi_list(&p).unwrap(), (200, 8000, 50, vec![]));
        let p = [0x01, 0x90, 0x03, 0x20, 0x06, 0x40, 0, 0];
        assert_eq!(
            parse_dpi_list(&p).unwrap(),
            (400, 1600, 0, vec![400, 800, 1600])
        );
        assert!(parse_dpi_list(&[0, 0]).is_err());
    }

    #[test]
    fn accepts() {
        let s = State {
            dpi: 1000,
            default: 1000,
            min: 200,
            max: 8000,
            step: 50,
            list: vec![],
        };
        assert!(s.accepts(1600));
        assert!(!s.accepts(1625));
        assert!(!s.accepts(9000));
    }
}
