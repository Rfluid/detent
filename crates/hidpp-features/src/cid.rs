//! Control IDs (CIDs) reported by ReprogControlsV4, with display and config names.
//!
//! Names follow the public CID catalogue used across prior art
//! (docs/research/prior-art.md, "Controls"). MX Master 4 values verified on the device
//! are listed in docs/specs/devices/mx-master-4.md.

pub const LEFT: u16 = 0x0050;
pub const RIGHT: u16 = 0x0051;
pub const MIDDLE: u16 = 0x0052;
pub const BACK: u16 = 0x0053;
pub const FORWARD: u16 = 0x0056;
pub const GESTURE: u16 = 0x00C3;
pub const SMART_SHIFT: u16 = 0x00C4;
pub const VIRTUAL_GESTURE: u16 = 0x00D7;
/// MX Master 4 Haptic Sense Panel (thumb rest). Config name: `thumb`.
pub const HAPTIC_PANEL: u16 = 0x01A0;

/// Display name for a CID, or `"?"` if not in the catalogue.
pub fn name(cid: u16) -> &'static str {
    match cid {
        LEFT => "Left Button",
        RIGHT => "Right Button",
        MIDDLE => "Middle Button",
        BACK => "Back",
        0x0054 => "Back (alt)",
        FORWARD => "Forward",
        0x0057 => "Forward (alt)",
        0x005B => "Left Tilt",
        0x005D => "Right Tilt",
        GESTURE => "Gesture Button",
        SMART_SHIFT => "Smart Shift",
        VIRTUAL_GESTURE => "Virtual Gesture Button",
        0x00D8 => "Thumbwheel Press",
        HAPTIC_PANEL => "Haptic Sense Panel",
        _ => "?",
    }
}

/// Names used in config triggers (docs/specs/config.md, "Controls").
pub const CONFIG_NAMES: &[(&str, u16)] = &[
    ("left", LEFT),
    ("right", RIGHT),
    ("middle", MIDDLE),
    ("back", BACK),
    ("forward", FORWARD),
    ("gesture", GESTURE),
    ("smartshift", SMART_SHIFT),
    ("thumb", HAPTIC_PANEL),
];

pub fn config_name(cid: u16) -> Option<&'static str> {
    CONFIG_NAMES
        .iter()
        .find(|(_, c)| *c == cid)
        .map(|(n, _)| *n)
}

/// Resolve a config control name. Also accepts `cid:0x1a0` for unnamed controls.
pub fn from_config_name(name: &str) -> Option<u16> {
    if let Some(hex) = name
        .strip_prefix("cid:0x")
        .or_else(|| name.strip_prefix("cid_0x"))
    {
        return u16::from_str_radix(hex, 16).ok();
    }
    let name = match name {
        "haptic-panel" | "panel" => "thumb",
        "shift" => "smartshift",
        n => n,
    };
    CONFIG_NAMES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_roundtrip() {
        for (n, c) in CONFIG_NAMES {
            assert_eq!(from_config_name(n), Some(*c));
            assert_eq!(config_name(*c), Some(*n));
        }
        assert_eq!(from_config_name("panel"), Some(HAPTIC_PANEL));
        assert_eq!(from_config_name("cid:0x1a0"), Some(HAPTIC_PANEL));
        assert_eq!(from_config_name("nope"), None);
    }
}
