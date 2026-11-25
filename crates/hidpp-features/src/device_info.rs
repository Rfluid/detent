//! Identity features, read-only:
//! - DeviceName/Type (0x0005): fn 0 getCount → name length, fn 1 getName(offset) → chunk,
//!   fn 2 getType → device type
//! - DeviceFwVersion (0x0003): fn 0 getEntityCount, fn 1 getFwInfo(entity) →
//!   `[type, prefix×3, number(BCD), revision(BCD), build(u16 BCD), ...]`

use detent_hidpp_proto::FeatureId;
use serde::{Deserialize, Serialize};

use crate::{RequestError, Requester};

fn index(dev: &dyn Requester, id: FeatureId) -> Result<u8, RequestError> {
    dev.feature_index(id).ok_or(RequestError::Unsupported(id))
}

/// Marketing name, e.g. "MX Master 4".
pub fn name(dev: &mut dyn Requester) -> Result<String, RequestError> {
    let idx = index(dev, FeatureId::DEVICE_NAME_TYPE)?;
    let len = *dev
        .request(idx, 0, &[])?
        .params()
        .first()
        .ok_or(RequestError::Malformed)? as usize;
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let resp = dev.request(idx, 1, &[out.len() as u8])?;
        let chunk = resp.params();
        let take = chunk.len().min(len - out.len());
        if take == 0 {
            return Err(RequestError::Malformed);
        }
        out.extend_from_slice(&chunk[..take]);
    }
    let s = String::from_utf8_lossy(&out);
    Ok(s.trim_end_matches('\0').to_string())
}

pub fn device_type(dev: &mut dyn Requester) -> Result<&'static str, RequestError> {
    let idx = index(dev, FeatureId::DEVICE_NAME_TYPE)?;
    let t = *dev
        .request(idx, 2, &[])?
        .params()
        .first()
        .ok_or(RequestError::Malformed)?;
    Ok(match t {
        0 => "keyboard",
        1 => "remote control",
        2 => "numpad",
        3 => "mouse",
        4 => "trackpad",
        5 => "trackball",
        6 => "presenter",
        7 => "receiver",
        8 => "headset",
        9 => "webcam",
        10 => "steering wheel",
        11 => "joystick",
        12 => "gamepad",
        13 => "dock",
        14 => "speaker",
        15 => "microphone",
        16 => "illumination light",
        17 => "programmable controller",
        18 => "car sim pedals",
        19 => "adapter",
        _ => "unknown",
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Firmware {
    pub kind: String,
    /// e.g. "RBM 17.02 B0013"
    pub version: String,
}

fn fw_kind(t: u8) -> &'static str {
    match t & 0x0F {
        0 => "main application",
        1 => "bootloader",
        2 => "hardware",
        3 => "touchpad",
        4 => "optical sensor",
        5 => "softdevice",
        6 => "RF companion MCU",
        7 => "factory application",
        8 => "RGB custom effect",
        9 => "motor drive",
        _ => "other",
    }
}

/// Firmware entities (application, bootloader, hardware revision...).
pub fn firmware(dev: &mut dyn Requester) -> Result<Vec<Firmware>, RequestError> {
    let idx = index(dev, FeatureId::DEVICE_FW_INFO)?;
    let count = *dev
        .request(idx, 0, &[])?
        .params()
        .first()
        .ok_or(RequestError::Malformed)?;
    let mut out = Vec::new();
    for entity in 0..count {
        let resp = dev.request(idx, 1, &[entity])?;
        let p = resp.params();
        if p.len() < 8 {
            return Err(RequestError::Malformed);
        }
        let prefix: String = p[1..4]
            .iter()
            .filter(|b| b.is_ascii_graphic())
            .map(|&b| b as char)
            .collect();
        let version = if p[0] & 0x0F == 2 {
            // hardware revision: only the number is meaningful
            format!("rev {}", p[4])
        } else {
            format!(
                "{prefix} {:02X}.{:02X} B{:02X}{:02X}",
                p[4], p[5], p[6], p[7]
            )
            .trim()
            .to_string()
        };
        out.push(Firmware {
            kind: fw_kind(p[0]).to_string(),
            version,
        });
    }
    Ok(out)
}
