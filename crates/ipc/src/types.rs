//! Result schemas of the IPC methods (docs/specs/ipc.md). These are also what
//! `detentctl --json` prints, so they are a stable, additive-only contract: new fields get
//! `#[serde(default)]`, nothing is renamed or removed without a major version.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// `status`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Status {
    /// Daemon version (`CARGO_PKG_VERSION`).
    pub version: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub uptime_s: u64,
    pub config_path: String,
    /// Why the file was rejected; the previous config stays active. `None` = file in use.
    #[serde(default)]
    pub config_error: Option<String>,
    /// Bindings in the loaded config (base `[[bind]]` + base layers).
    #[serde(default)]
    pub bindings: usize,
    #[serde(default)]
    pub profiles: Vec<String>,
    pub active_profile: String,
    /// Context published by plugins (`app`, ...).
    #[serde(default)]
    pub context: BTreeMap<String, String>,
    /// `false` while remapping is paused (`detentctl disable`).
    #[serde(default = "yes")]
    pub remap_enabled: bool,
    #[serde(default)]
    pub devices: Vec<DeviceSummary>,
    #[serde(default)]
    pub plugins: Vec<PluginInfo>,
}

fn yes() -> bool {
    true
}

/// One attached device (`status.devices`, `devices.list`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceSummary {
    /// Stable key: `PPPP:serial` (or the node when there is no serial).
    pub id: String,
    pub name: String,
    pub node: String,
    /// Receiver slot (1–6) when the device is reached through a receiver at `node`;
    /// absent for direct (Bluetooth / USB cable) devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    /// HID++ frames read by the data plane since attach.
    #[serde(default)]
    pub frames: u64,
    /// Messages the data plane dropped because a consumer was slow (should stay 0).
    #[serde(default)]
    pub dropped: u64,
}

/// `remap.get`, `remap.set`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemapState {
    pub enabled: bool,
    /// `remap.set` only: whether this call changed the state.
    #[serde(default)]
    pub changed: bool,
}

/// One plugin in `plugins.list` / `status.plugins`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    /// `connected` | `starting` | `stopped`.
    pub state: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub restarts: u32,
    #[serde(default)]
    pub last_error: Option<String>,
    /// Bus messages dropped because the plugin was slow.
    #[serde(default)]
    pub dropped: u64,
    /// Granted capabilities (`None` until connected).
    #[serde(default)]
    pub caps: Option<Vec<String>>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub uptime_s: Option<u64>,
}

/// One entry of `actions.list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionInfo {
    pub name: String,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub continuous: bool,
    /// Providing plugin (`detent` for built-ins).
    #[serde(default)]
    pub plugin: String,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn old_daemon_status_still_parses() {
        // A 0.0.1 daemon's `status` (no pid/uptime/remap/stats): defaults fill in.
        let v = json!({
            "version": "0.0.1", "config_path": "/c", "config_error": null, "bindings": 2,
            "profiles": [], "active_profile": "base", "context": {},
            "devices": [{"id": "k", "name": "n", "node": "/dev/hidraw1"}],
            "plugins": [{"name": "kde", "state": "connected", "pid": 1, "restarts": 0,
                         "last_error": null, "dropped": 0, "caps": ["context"],
                         "version": "0.0.1", "uptime_s": 5}]
        });
        let s: Status = serde_json::from_value(v).unwrap();
        assert!(s.remap_enabled);
        assert_eq!(
            s.plugins[0].caps.as_deref(),
            Some(&["context".to_string()][..])
        );
    }

    #[test]
    fn status_schema_keys_are_stable() {
        let s = Status {
            version: "1".into(),
            pid: 1,
            uptime_s: 2,
            config_path: "/c".into(),
            config_error: None,
            bindings: 0,
            profiles: vec![],
            active_profile: "base".into(),
            context: BTreeMap::new(),
            remap_enabled: true,
            devices: vec![],
            plugins: vec![],
        };
        let v = serde_json::to_value(&s).unwrap();
        let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "active_profile",
                "bindings",
                "config_error",
                "config_path",
                "context",
                "devices",
                "pid",
                "plugins",
                "profiles",
                "remap_enabled",
                "uptime_s",
                "version"
            ]
        );
    }
}
