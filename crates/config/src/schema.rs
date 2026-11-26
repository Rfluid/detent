use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{ConfigError, trigger::Trigger};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub daemon: Daemon,
    /// Per-device settings, keyed by device name or serial.
    #[serde(default)]
    pub device: BTreeMap<String, DeviceSettings>,
    #[serde(default)]
    pub bind: Vec<Bind>,
    #[serde(default)]
    pub layer: BTreeMap<String, Layer>,
    #[serde(default)]
    pub plugins: BTreeMap<String, PluginConfig>,
    #[serde(default)]
    pub profile: BTreeMap<String, Profile>,
    /// Gesture tuning (`[gesture]`).
    pub gesture: Option<GestureSettings>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GestureSettings {
    /// Shorthand: the same threshold (mm) on both axes. Overridden per axis below.
    #[serde(default)]
    pub threshold_mm: Option<f32>,
    /// Horizontal threshold (mm): how far left/right a swipe must go. Default 6.
    /// Diagonals are derived from the X/Y ellipse.
    #[serde(default)]
    pub threshold_x_mm: Option<f32>,
    /// Vertical threshold (mm): how far up/down a swipe must go. Default 3.
    #[serde(default)]
    pub threshold_y_mm: Option<f32>,
    /// Haptic pulse when a gesture resolves (swipe in any direction, diagonal or tap):
    /// `true` (damp-state-change), `false`, or a waveform name. Default off.
    #[serde(default)]
    pub haptic: GestureHaptic,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum GestureHaptic {
    Enabled(bool),
    Waveform(String),
}

impl Default for GestureHaptic {
    fn default() -> Self {
        Self::Enabled(false)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Daemon {
    #[serde(default)]
    pub conflict_policy: ConflictPolicy,
    /// Maximum lease duration a plugin may request, in milliseconds.
    #[serde(default = "default_max_lease_ms")]
    pub max_lease_ms: u64,
}

impl Default for Daemon {
    fn default() -> Self {
        Self {
            conflict_policy: ConflictPolicy::default(),
            max_lease_ms: default_max_lease_ms(),
        }
    }
}

fn default_max_lease_ms() -> u64 {
    10_000
}

/// What to do when Solaar or logiops is running.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictPolicy {
    #[default]
    Warn,
    Refuse,
    Ignore,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceSettings {
    pub dpi: Option<u16>,
    pub smartshift: Option<SmartShift>,
    pub haptics: Option<Haptics>,
    pub thumbwheel: Option<Thumbwheel>,
    pub wheel: Option<Thumbwheel>,
    /// DPI values for `device = "cycle-dpi"` (default 800, 1600, 3200).
    pub dpi_cycle: Option<Vec<u16>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartShift {
    /// `ratchet` | `free-spin`; omitted = leave as is.
    pub mode: Option<String>,
    pub auto_threshold: Option<u8>,
    pub torque: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Haptics {
    pub enabled: Option<bool>,
    pub intensity: Option<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thumbwheel {
    #[serde(default)]
    pub invert: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bind {
    pub when: String,
    #[serde(rename = "do")]
    pub action: Do,
}

/// One action. Exactly one of the action fields is set; extra keys are parameters
/// passed to plugin actions (e.g. `menu = "main"`).
#[derive(Debug, Clone, Deserialize)]
pub struct Do {
    pub keys: Option<String>,
    pub exec: Option<String>,
    pub action: Option<String>,
    pub button: Option<String>,
    pub device: Option<String>,
    #[serde(flatten)]
    pub params: toml::Table,
}

impl Do {
    fn kinds(&self) -> usize {
        [
            self.keys.is_some(),
            self.exec.is_some(),
            self.action.is_some(),
            self.button.is_some(),
            self.device.is_some(),
        ]
        .into_iter()
        .filter(|b| *b)
        .count()
    }
}

/// A layer: active while `hold` (a modifier, or `a + b`) is held.
/// Keys are trigger targets within the layer, quoted when dotted: `"swipe.up" = { ... }`.
#[derive(Debug, Clone, Deserialize)]
pub struct Layer {
    pub hold: String,
    #[serde(flatten)]
    pub bindings: BTreeMap<String, Do>,
}

/// `[plugins.<name>]`. Native plugins (kde, radial) need no `exec`.
/// Every key other than the reserved ones is plugin-owned settings, validated by the plugin.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    #[serde(default = "yes")]
    pub enable: bool,
    pub exec: Option<String>,
    pub caps: Option<Vec<String>>,
    #[serde(flatten)]
    pub settings: toml::Table,
}

fn yes() -> bool {
    true
}

/// Context-matched overrides (context comes from plugins, e.g. `app` from detent-kde).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    #[serde(rename = "match")]
    pub matches: BTreeMap<String, String>,
    #[serde(default)]
    pub bind: Vec<Bind>,
    #[serde(default)]
    pub layer: BTreeMap<String, Layer>,
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let check = |when: &str, d: &Do| -> Result<(), ConfigError> {
            Trigger::parse(when).map_err(|source| ConfigError::Trigger {
                when: when.to_string(),
                source,
            })?;
            if d.kinds() != 1 {
                return Err(ConfigError::AmbiguousAction(when.to_string()));
            }
            Ok(())
        };
        let check_layers = |layers: &BTreeMap<String, Layer>| -> Result<(), ConfigError> {
            for layer in layers.values() {
                for (target, d) in &layer.bindings {
                    check(&format!("{} + {}", layer.hold, target), d)?;
                }
            }
            Ok(())
        };
        for b in &self.bind {
            check(&b.when, &b.action)?;
        }
        check_layers(&self.layer)?;
        for p in self.profile.values() {
            for b in &p.bind {
                check(&b.when, &b.action)?;
            }
            check_layers(&p.layer)?;
        }
        Ok(())
    }
}
