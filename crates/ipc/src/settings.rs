//! The device settings Detent can change (`device.set` paths). One catalogue for the daemon
//! (validation), `detentctl settings list` and the direct no-daemon path, so they can't
//! drift. Only settings whose HID++ features are verified on hardware belong here
//! (AGENTS.md); `click_force.*` (0x19C0, stored on the mouse) waits for the layered
//! settings work in docs/architecture/settings-layers.md.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Integer in `min..=max` (DPI: further limited by the device's range and step).
    Int {
        min: u64,
        max: u64,
    },
    Bool,
    Choice(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy)]
pub struct Setting {
    /// Dotted path: `dpi`, `smartshift.mode`.
    pub path: &'static str,
    /// `DeviceState` section the value lives in.
    pub section: &'static str,
    /// Field inside the section's `reported` value.
    pub field: &'static str,
    pub kind: Kind,
    pub doc: &'static str,
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        path: "dpi",
        section: "dpi",
        field: "dpi",
        kind: Kind::Int {
            min: 0,
            max: u16::MAX as u64,
        },
        doc: "Pointer resolution (dots per inch)",
    },
    Setting {
        path: "smartshift.mode",
        section: "smartshift",
        field: "mode",
        kind: Kind::Choice(&["ratchet", "free-spin"]),
        doc: "Main wheel mode",
    },
    Setting {
        path: "smartshift.auto_threshold",
        section: "smartshift",
        field: "auto_threshold",
        kind: Kind::Int { min: 0, max: 255 },
        doc: "SmartShift: scroll speed that switches to free-spin (0 = off)",
    },
    Setting {
        path: "smartshift.torque",
        section: "smartshift",
        field: "torque",
        kind: Kind::Int { min: 0, max: 100 },
        doc: "Ratchet force (%)",
    },
    Setting {
        path: "haptics.enabled",
        section: "haptics",
        field: "enabled",
        kind: Kind::Bool,
        doc: "Haptic feedback on/off",
    },
    Setting {
        path: "haptics.intensity",
        section: "haptics",
        field: "intensity",
        kind: Kind::Int { min: 0, max: 100 },
        doc: "Haptic feedback strength (%)",
    },
    Setting {
        path: "wheel.invert",
        section: "wheel",
        field: "invert",
        kind: Kind::Bool,
        doc: "Invert the main wheel",
    },
    Setting {
        path: "thumbwheel.invert",
        section: "thumbwheel",
        field: "invert",
        kind: Kind::Bool,
        doc: "Invert the thumbwheel (Detent default: natural, thumb down = left; on over Bluetooth, off through a receiver)",
    },
];

pub fn find(path: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.path == path)
}

/// `"`x` is not settable (dpi, smartshift.mode, ...)"`.
pub fn unknown(path: &str) -> String {
    let all: Vec<&str> = SETTINGS.iter().map(|s| s.path).collect();
    format!("`{path}` is not settable ({})", all.join(", "))
}

impl Setting {
    /// Parse a CLI/IPC value (always a string at the edges) into its JSON value.
    pub fn parse(&self, value: &str) -> Result<Value, String> {
        match self.kind {
            Kind::Int { min, max } => {
                let n: u64 = value
                    .trim()
                    .parse()
                    .map_err(|_| format!("`{value}` is not a number"))?;
                if n < min || n > max {
                    return Err(format!("{} must be {min}..={max}", self.path));
                }
                Ok(Value::from(n))
            }
            Kind::Bool => match value.trim() {
                "true" | "on" | "yes" | "1" => Ok(Value::Bool(true)),
                "false" | "off" | "no" | "0" => Ok(Value::Bool(false)),
                _ => Err(format!("`{value}` is not on/off")),
            },
            Kind::Choice(c) => {
                let v = value.trim();
                if c.contains(&v) {
                    Ok(Value::from(v))
                } else {
                    Err(format!("{} is one of: {}", self.path, c.join(", ")))
                }
            }
        }
    }
}

/// One row of `device.schema` / `detentctl settings list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettingInfo {
    pub path: String,
    /// `int` | `bool` | `choice`.
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u64>,
    /// Allowed values (choices, or a DPI list on devices with discrete steps).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<Value>,
    /// What the device reports now.
    pub value: Value,
    /// What Detent wants (config + live edits); `null` = hands off.
    #[serde(default)]
    pub desired: Value,
    /// `desired` is set and differs from `value`.
    #[serde(default)]
    pub pending: bool,
    pub doc: String,
}

/// The settable paths this device supports, with current and desired values, from a
/// serialized `DeviceState`. Settings whose feature is missing are left out.
pub fn describe(state: &Value) -> Vec<SettingInfo> {
    let mut out = Vec::new();
    for s in SETTINGS {
        let sec = &state[s.section];
        let reported = &sec["reported"];
        let value = &reported[s.field];
        if reported.is_null() || value.is_null() {
            continue;
        }
        let desired = sec["desired"][s.field].clone();
        let (kind, mut min, mut max, mut step, mut values) = match s.kind {
            Kind::Int { min, max } => ("int", Some(min), Some(max), None, Vec::new()),
            Kind::Bool => ("bool", None, None, None, Vec::new()),
            Kind::Choice(c) => (
                "choice",
                None,
                None,
                None,
                c.iter().map(|&v| v.into()).collect(),
            ),
        };
        if s.path == "dpi" {
            min = reported["min"].as_u64().or(min);
            max = reported["max"].as_u64().or(max);
            step = reported["step"].as_u64().filter(|&v| v > 0);
            if let Some(list) = reported["list"].as_array().filter(|l| !l.is_empty()) {
                values = list.clone();
            }
        }
        out.push(SettingInfo {
            path: s.path.to_string(),
            kind: kind.to_string(),
            min,
            max,
            step,
            values,
            pending: !desired.is_null() && desired != *value,
            value: value.clone(),
            desired,
            doc: s.doc.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn state() -> Value {
        json!({
            "dpi": {"reported": {"dpi": 1000, "default": 1000, "min": 200, "max": 8000, "step": 50, "list": []},
                    "desired": {"dpi": 1600, "default": 1000, "min": 200, "max": 8000, "step": 50, "list": []},
                    "generation": 1},
            "smartshift": {"reported": {"mode": "ratchet", "auto_threshold": 10, "torque": null},
                           "desired": null, "generation": 1},
            "haptics": {"reported": null, "desired": null, "generation": 0},
            "thumbwheel": {"reported": {"invert": true, "diverted": false, "native_res": 18, "diverted_res": 120},
                           "desired": {"invert": true, "diverted": false, "native_res": 18, "diverted_res": 120},
                           "generation": 2}
        })
    }

    #[test]
    fn describe_lists_present_settings_with_ranges() {
        let rows = describe(&state());
        let paths: Vec<&str> = rows.iter().map(|r| r.path.as_str()).collect();
        // no torque (null), no haptics (feature absent), no wheel section
        assert_eq!(
            paths,
            [
                "dpi",
                "smartshift.mode",
                "smartshift.auto_threshold",
                "thumbwheel.invert"
            ]
        );
        let dpi = &rows[0];
        assert_eq!(
            (dpi.min, dpi.max, dpi.step),
            (Some(200), Some(8000), Some(50))
        );
        assert!(dpi.pending);
        assert_eq!(dpi.desired, json!(1600));
        assert!(!rows[3].pending);
        let v = serde_json::to_value(&rows[1]).unwrap();
        assert_eq!(v["type"], "choice");
        assert_eq!(v["values"], json!(["ratchet", "free-spin"]));
        assert!(v.get("min").is_none());
    }

    #[test]
    fn parse_validates() {
        let p = |path: &str, v: &str| find(path).unwrap().parse(v);
        assert_eq!(p("dpi", "1600"), Ok(json!(1600)));
        assert!(p("dpi", "fast").is_err());
        assert!(p("haptics.intensity", "101").is_err());
        assert_eq!(p("wheel.invert", "on"), Ok(json!(true)));
        assert!(p("wheel.invert", "maybe").is_err());
        assert_eq!(p("smartshift.mode", "free-spin"), Ok(json!("free-spin")));
        assert!(p("smartshift.mode", "freespin").is_err());
        assert!(find("click_force.thumb").is_none());
        assert!(unknown("x").contains("thumbwheel.invert"));
    }
}
