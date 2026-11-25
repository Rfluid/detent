//! Native plugins: first-party plugins built in this repo (`plugins/`), installed
//! next to `detentd`, enabled with just `[plugins.<name>]` (no `exec`).
//!
//! They use the same plugin bus and SDK as third-party plugins: no private APIs.
//! Adding one: `.agent/skills/add-native-plugin.md`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePlugin {
    Kde,
    Radial,
}

impl NativePlugin {
    pub const ALL: &'static [Self] = &[Self::Kde, Self::Radial];

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.name() == name)
    }

    /// Config key and action namespace (`kde.*`, `radial.*`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Kde => "kde",
            Self::Radial => "radial",
        }
    }

    pub fn binary(self) -> &'static str {
        match self {
            Self::Kde => "detent-kde",
            Self::Radial => "detent-radial",
        }
    }

    /// Capabilities granted when the user doesn't list `caps` explicitly.
    pub fn default_caps(self) -> &'static [&'static str] {
        match self {
            Self::Kde => &[
                "events.subscribe",
                "context.publish",
                "device.haptics",
                "actions.invoke",
            ],
            Self::Radial => &[
                // the trigger control while held (hold-and-release); the pointer stays free
                "lease.buttons",
                "device.haptics",
                "actions.invoke",
                // menu items with `keys = "..."`
                "input.emit",
                // menu items with `action = "device.host"` (Easy-Switch)
                "device.control",
                // `app` context for per-application rings
                "events.subscribe",
            ],
        }
    }

    /// Resolve the executable: next to `detentd` first, then `$PATH` lookup by the supervisor.
    pub fn resolve(self, daemon_exe: &Path) -> PathBuf {
        let sibling = daemon_exe.with_file_name(self.binary());
        if sibling.exists() {
            sibling
        } else {
            PathBuf::from(self.binary())
        }
    }
}
