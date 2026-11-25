use std::collections::BTreeMap;

use detent_hidpp_features::{Feature, RequestError};
use detent_hidpp_features::{
    FeatureTable, Requester, adjustable_dpi, device_info, haptics, hosts, reprog_controls, root,
    smartshift, unified_battery, wheel,
};
use detent_hidpp_proto::FeatureId;
use serde::Serialize;

/// Stable identity: model + serial, so per-device config survives reconnects and host switches.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct DeviceId {
    pub product: u32,
    pub serial: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Link {
    Connected,
    /// Asleep, out of range or on another host. Reported state is unknown.
    Disconnected,
}

/// Desired vs reported value of one feature, versioned so subscribers receive diffs.
#[derive(Debug, Clone, Serialize)]
pub struct Section<T> {
    pub desired: Option<T>,
    pub reported: Option<T>,
    /// Bumped on every change to either side.
    pub generation: u64,
}

impl<T> Default for Section<T> {
    fn default() -> Self {
        Self {
            desired: None,
            reported: None,
            generation: 0,
        }
    }
}

impl<T: PartialEq> Section<T> {
    pub fn in_sync(&self) -> bool {
        match (&self.desired, &self.reported) {
            (Some(d), Some(r)) => d == r,
            (None, _) => true,
            (Some(_), None) => false,
        }
    }

    pub fn set_desired(&mut self, v: Option<T>) {
        if self.desired != v {
            self.desired = v;
            self.generation += 1;
        }
    }

    pub fn set_reported(&mut self, v: Option<T>) {
        if self.reported != v {
            self.reported = v;
            self.generation += 1;
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Identity {
    pub name: String,
    pub kind: Option<String>,
    pub firmware: Vec<device_info::Firmware>,
    pub protocol: Option<(u8, u8)>,
    /// hidraw node currently used (changes between connections; never an identity key).
    pub node: String,
}

/// Everything the user can see (and mostly edit) about one device.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceState {
    pub id: DeviceId,
    pub identity: Identity,
    pub link: Link,
    /// Every advertised feature, including ones Detent has no module for.
    pub features: FeatureTable,
    pub battery: Section<unified_battery::State>,
    pub dpi: Section<adjustable_dpi::State>,
    pub smartshift: Section<smartshift::State>,
    pub wheel: Section<wheel::WheelState>,
    pub thumbwheel: Section<wheel::ThumbwheelState>,
    /// Easy-Switch slots. Reported only: switching is a command, never desired state.
    pub hosts: Section<hosts::HostsState>,
    pub controls: Section<reprog_controls::State>,
    pub haptics: Section<haptics::State>,
    /// Last read error per section (cleared on success), so `get` can explain gaps.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub errors: BTreeMap<&'static str, String>,
}

impl DeviceState {
    pub fn new(id: DeviceId, identity: Identity, features: FeatureTable) -> Self {
        Self {
            id,
            identity,
            link: Link::Connected,
            features,
            battery: Section::default(),
            dpi: Section::default(),
            smartshift: Section::default(),
            wheel: Section::default(),
            thumbwheel: Section::default(),
            hosts: Section::default(),
            controls: Section::default(),
            haptics: Section::default(),
            errors: BTreeMap::new(),
        }
    }

    /// Called when the link drops: the device forgets diversion and settings.
    pub fn mark_disconnected(&mut self) {
        self.link = Link::Disconnected;
        self.battery.set_reported(None);
        self.dpi.set_reported(None);
        self.smartshift.set_reported(None);
        self.wheel.set_reported(None);
        self.thumbwheel.set_reported(None);
        self.hosts.set_reported(None);
        self.controls.set_reported(None);
        self.haptics.set_reported(None);
    }

    fn record<T>(&mut self, key: &'static str, r: Result<T, RequestError>) -> Option<T> {
        match r {
            Ok(v) => {
                self.errors.remove(key);
                Some(v)
            }
            Err(RequestError::Unsupported(_)) => None,
            Err(e) => {
                self.errors.insert(key, e.to_string());
                None
            }
        }
    }

    /// Read every supported section from the device into `reported`.
    pub fn read_all(&mut self, dev: &mut dyn Requester) {
        let has = |s: &Self, id: FeatureId| s.features.index_of(id).is_some();
        if has(self, FeatureId::UNIFIED_BATTERY) {
            let v = self.record("battery", unified_battery::UnifiedBattery::read(dev));
            self.battery.set_reported(v);
        }
        if has(self, FeatureId::ADJUSTABLE_DPI) {
            let v = self.record("dpi", adjustable_dpi::AdjustableDpi::read(dev));
            self.dpi.set_reported(v);
        }
        if has(self, FeatureId::SMART_SHIFT_ENHANCED) {
            let v = self.record("smartshift", smartshift::SmartShift::read(dev));
            self.smartshift.set_reported(v);
        }
        if has(self, FeatureId::HIRES_WHEEL) {
            let v = self.record("wheel", wheel::HiResWheel::read(dev));
            self.wheel.set_reported(v);
        }
        if has(self, FeatureId::THUMBWHEEL) {
            let v = self.record("thumbwheel", wheel::Thumbwheel::read(dev));
            self.thumbwheel.set_reported(v);
        }
        if has(self, FeatureId::CHANGE_HOST) {
            let v = self.record("hosts", hosts::read(dev));
            self.hosts.set_reported(v);
        }
        if has(self, FeatureId::REPROG_CONTROLS_V4) {
            let v = self.record("controls", reprog_controls::ReprogControls::read(dev));
            self.controls.set_reported(v);
        }
        if has(self, haptics::Haptics::ID) {
            let v = self.record("haptics", haptics::Haptics::read(dev));
            self.haptics.set_reported(v);
        }
    }
}

/// Identify a freshly opened device (features must already be loaded on `dev`).
pub fn identify(dev: &mut dyn Requester, node: &str) -> Identity {
    Identity {
        name: device_info::name(dev).unwrap_or_default(),
        kind: device_info::device_type(dev).ok().map(str::to_string),
        firmware: device_info::firmware(dev).unwrap_or_default(),
        protocol: root::protocol_version(dev).ok(),
        node: node.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_sync_tracking() {
        let mut s: Section<u16> = Section::default();
        assert!(s.in_sync());
        s.set_desired(Some(1600));
        assert!(!s.in_sync());
        s.set_reported(Some(1600));
        assert!(s.in_sync());
        assert_eq!(s.generation, 2);
        s.set_reported(Some(1600)); // no change, no bump
        assert_eq!(s.generation, 2);
    }
}
