//! Device settings on the worker: config values + live edits (`device.set`) → `desired`,
//! then the reconciler applies what differs from `reported`. Validation comes from the
//! shared catalogue (`detent_ipc::settings`), so the CLI, the daemon and the direct path
//! accept exactly the same paths and values.

use detent_hidpp_features::{Feature, adjustable_dpi, haptics, smartshift, wheel};
use detent_hidpp_proto::DeviceIndex;
use serde_json::Value;
use tracing::{info, warn};

use super::{Worker, fmt_err};
use crate::msg::ToDp;

impl Worker {
    pub(super) fn desired_from_config(&mut self) {
        let cfg = self.config();
        let ds = self.device_settings(&cfg).unwrap_or_default();
        let st = &mut self.state;
        let ov = &self.overrides;
        let num = |k: &str| ov.get(k).and_then(|v| v.as_u64());
        let flag = |k: &str| ov.get(k).and_then(|v| v.as_bool());

        let dpi = num("dpi").map(|v| v as u16).or(ds.dpi);
        st.dpi.set_desired(match (dpi, st.dpi.reported.clone()) {
            (Some(d), Some(mut r)) => {
                r.dpi = d;
                Some(r)
            }
            _ => None,
        });

        let ss_mode = ov
            .get("smartshift.mode")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or(ds.smartshift.as_ref().and_then(|s| s.mode.clone()));
        let ss_auto = num("smartshift.auto_threshold")
            .map(|v| v as u8)
            .or(ds.smartshift.as_ref().and_then(|s| s.auto_threshold));
        let ss_torque = num("smartshift.torque")
            .map(|v| v as u8)
            .or(ds.smartshift.as_ref().and_then(|s| s.torque));
        st.smartshift
            .set_desired(match st.smartshift.reported.clone() {
                Some(mut r) if ss_mode.is_some() || ss_auto.is_some() || ss_torque.is_some() => {
                    if let Some(m) = ss_mode.as_deref().and_then(smartshift::WheelMode::parse) {
                        r.mode = m;
                    }
                    if let Some(a) = ss_auto {
                        r.auto_threshold = a;
                    }
                    if ss_torque.is_some() && r.torque.is_some() {
                        r.torque = ss_torque;
                    }
                    Some(r)
                }
                _ => None,
            });

        let h_en = flag("haptics.enabled").or(ds.haptics.as_ref().and_then(|h| h.enabled));
        let h_int = num("haptics.intensity")
            .map(|v| v as u8)
            .or(ds.haptics.as_ref().and_then(|h| h.intensity));
        st.haptics.set_desired(match st.haptics.reported.clone() {
            Some(mut r) if h_en.is_some() || h_int.is_some() => {
                if let Some(e) = h_en {
                    r.enabled = e;
                }
                if let Some(i) = h_int {
                    r.intensity = i.min(100);
                }
                Some(r)
            }
            _ => None,
        });

        // Default: the natural direction (thumb down → left, up → right). The device's own
        // direction differs by link: over Bluetooth it needs the invert bit, through a Logi
        // Bolt receiver it is already natural (owner-reported, docs/specs/devices/mx-master-4.md).
        let natural_invert = self.link.device == DeviceIndex::DIRECT;
        let tw_inv = flag("thumbwheel.invert")
            .or(ds.thumbwheel.as_ref().map(|t| t.invert))
            .or(Some(natural_invert));
        st.thumbwheel
            .set_desired(match (tw_inv, st.thumbwheel.reported.clone()) {
                (Some(i), Some(mut r)) => {
                    r.invert = i;
                    Some(r)
                }
                _ => None,
            });

        let w_inv = flag("wheel.invert").or(ds.wheel.as_ref().map(|w| w.invert));
        st.wheel
            .set_desired(match (w_inv, st.wheel.reported.clone()) {
                (Some(i), Some(mut r)) => {
                    r.invert = i;
                    Some(r)
                }
                _ => None,
            });
    }

    /// Apply out-of-sync settings and read them back.
    pub fn apply_settings(&mut self) {
        self.desired_from_config();
        let key = self.key.clone();
        self.with_dev(|dev, st| {
            macro_rules! apply {
                ($sec:ident, $feat:ty, $name:literal) => {
                    if let (Some(d), Some(r)) = (st.$sec.desired.clone(), st.$sec.reported.clone())
                        && d != r
                    {
                        match <$feat>::apply(dev, &d, &r) {
                            Ok(()) => {
                                info!("{key}: {} set", $name);
                                st.$sec.set_reported(<$feat>::read(dev).ok());
                                st.errors.remove($name);
                            }
                            Err(e) => {
                                warn!("{key}: setting {} failed: {}", $name, fmt_err(&e));
                                st.errors.insert($name, fmt_err(&e));
                            }
                        }
                    }
                };
            }
            apply!(dpi, adjustable_dpi::AdjustableDpi, "dpi");
            apply!(smartshift, smartshift::SmartShift, "smartshift");
            apply!(haptics, haptics::Haptics, "haptics");
            apply!(thumbwheel, wheel::Thumbwheel, "thumbwheel");
            apply!(wheel, wheel::HiResWheel, "wheel");
        });
        if let Some(d) = self.state.dpi.reported.as_ref().map(|d| d.dpi) {
            self.link.send(ToDp::SetDpi(d));
        }
    }

    /// Live edit of one setting (session only unless persisted by the caller).
    pub(super) fn set(&mut self, path: &str, value: &str) -> Result<Value, String> {
        let s =
            detent_ipc::settings::find(path).ok_or_else(|| detent_ipc::settings::unknown(path))?;
        let v = s.parse(value)?;
        if path == "dpi"
            && let Some(r) = &self.state.dpi.reported
            && !r.accepts(v.as_u64().unwrap_or(0) as u16)
        {
            return Err(format!(
                "dpi must be {}..={} in steps of {}",
                r.min, r.max, r.step
            ));
        }
        self.overrides.insert(path.to_string(), v);
        self.apply_settings();
        self.section_result(s.section)
    }

    /// Drop a live edit: the config value (or "hands off") applies again.
    pub(super) fn unset(&mut self, path: &str) -> Result<Value, String> {
        let s =
            detent_ipc::settings::find(path).ok_or_else(|| detent_ipc::settings::unknown(path))?;
        self.overrides.remove(path);
        self.apply_settings();
        self.section_result(s.section)
    }

    /// The section's reported value after a change, or the error that stopped it.
    fn section_result(&self, section: &str) -> Result<Value, String> {
        if let Some(e) = self.state.errors.get(section) {
            return Err(e.clone());
        }
        let state = serde_json::to_value(&self.state).map_err(|e| e.to_string())?;
        Ok(state
            .get(section)
            .and_then(|s| s.get("reported"))
            .cloned()
            .unwrap_or(Value::Null))
    }
}
