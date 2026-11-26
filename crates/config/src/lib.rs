//! User configuration. Reference: `docs/specs/config.md`.
//!
//! Pipeline: TOML text → [`schema::Config`] (serde) → validate → [`compile::compile`] into
//! `detent_engine::Compiled` + exec/action tables. An invalid file never replaces a
//! working config.

pub mod compile;
pub mod keys;
pub mod listing;
pub mod native;
pub mod schema;
pub mod trigger;

use std::path::PathBuf;

pub use compile::{CompiledConfig, NamedAction, compile};
pub use native::NativePlugin;
pub use schema::Config;
pub use trigger::{Trigger, TriggerError};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("TOML: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("binding `{when}`: {source}")]
    Trigger { when: String, source: TriggerError },
    #[error(
        "binding `{0}`: exactly one of `keys`, `exec`, `action`, `button`, `device` is required"
    )]
    AmbiguousAction(String),
    #[error("`{when}`: {msg}")]
    Compile { when: String, msg: String },
}

/// Parse and validate a config file body (syntax + triggers).
pub fn parse(text: &str) -> Result<Config, ConfigError> {
    let cfg: Config = toml::from_str(text)?;
    cfg.validate()?;
    Ok(cfg)
}

/// Parse, validate and compile: what the daemon loads and `detentctl check` runs.
pub fn load(text: &str) -> Result<CompiledConfig, ConfigError> {
    compile(&parse(text)?)
}

/// `$XDG_CONFIG_HOME/detent/config.toml` (default `~/.config/detent/config.toml`).
pub fn default_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("detent").join("config.toml")
}

#[cfg(test)]
mod tests {
    use detent_engine::{Action, Edge, LayerId, slot};

    #[test]
    fn example_config_is_valid() {
        let text = include_str!("../../../config/detent.example.toml");
        let cfg = super::parse(text).expect("example config must stay valid");
        assert!(cfg.plugins.contains_key("kde"));
        assert!(cfg.plugins.contains_key("radial"));
        assert!(cfg.layer.contains_key("gesture"));
        let c = super::load(text).expect("example config must compile");
        // gesture is modifier 0 with a layer; left chord + wheel need the grab
        assert_eq!(c.engine.slots[slot::GESTURE.0 as usize].cid, 0x00C3);
        assert_eq!(c.engine.grab_mods.0, 1);
        assert_eq!(c.engine.swipe_mods.0, 1);
        assert_eq!(
            c.engine.swipe_haptic,
            Some(1),
            "gesture haptic on (damp-state-change)"
        );
        assert_eq!(c.profiles.len(), 1);
        let base = &c.engine.profiles[0];
        assert!(matches!(
            base.bindings
                .lookup(LayerId::BASE, slot::GESTURE, Edge::Tap),
            Action::Plugin { .. }
        ));
        assert!(matches!(
            base.bindings.lookup(LayerId(1), slot::LEFT, Edge::Press),
            Action::Exec(_)
        ));
        // firefox profile overrides gesture + left with Ctrl+W
        let ff = &c.engine.profiles[1];
        assert!(matches!(
            ff.bindings.lookup(LayerId(1), slot::LEFT, Edge::Press),
            Action::Keys(_)
        ));
        let ctx = [("app".to_string(), "Firefox".to_string())].into();
        assert_eq!(c.profile_for(&ctx), 1);
        assert_eq!(c.profile_for(&Default::default()), 0);
    }

    #[test]
    fn rejects_meaningful_mistakes() {
        let bad = [
            (
                "[[bind]]\nwhen = \"left\"\ndo = { keys = \"a\" }\n",
                "chord",
            ),
            (
                "[[bind]]\nwhen = \"left + middle\"\ndo = { keys = \"a\" }\n",
                "modifier",
            ),
            (
                "[[bind]]\nwhen = \"gesture + swipe\"\ndo = { keys = \"a\" }\n",
                "direction",
            ),
            (
                "[[bind]]\nwhen = \"thumb\"\ndo = { keys = \"Ctrl+Nope\" }\n",
                "unknown key",
            ),
            (
                "[[bind]]\nwhen = \"thumb\"\ndo = { action = \"overview\" }\n",
                "<plugin>",
            ),
            (
                "[[bind]]\nwhen = \"pinky\"\ndo = { keys = \"a\" }\n",
                "unknown control",
            ),
            (
                "[[bind]]\nwhen = \"gesture + wheel\"\ndo = { keys = \"a\" }\n",
                "continuous",
            ),
        ];
        for (text, needle) in bad {
            let e = super::load(text).expect_err(text).to_string();
            assert!(e.contains(needle), "{text}: {e}");
        }
    }

    #[test]
    fn host_switch_bindings() {
        use detent_engine::action::{DeviceCmd, HostTarget};
        let host = |v: &str| {
            let text = format!("[[bind]]\nwhen = \"thumb\"\ndo = {{ device = \"host\"{v} }}\n");
            super::load(&text)
                .map(|c| {
                    c.engine.profiles[0]
                        .bindings
                        .lookup(LayerId::BASE, slot::THUMB, Edge::Press)
                })
                .map_err(|e| e.to_string())
        };
        let cmd = |t| Ok(Action::Device(DeviceCmd::ChangeHost(t)));
        assert_eq!(host(", value = 2"), cmd(HostTarget::Slot(1)));
        assert_eq!(host(", value = \"next\""), cmd(HostTarget::Next));
        assert_eq!(host(", value = \"prev\""), cmd(HostTarget::Prev));
        for bad in ["", ", value = 0", ", value = \"left\"", ", value = true"] {
            assert!(
                host(bad).unwrap_err().contains("device = \"host\""),
                "{bad}"
            );
        }
    }

    #[test]
    fn gesture_haptic_values() {
        let haptic = |v: &str| {
            super::load(&format!("[gesture]\nhaptic = {v}\n"))
                .map(|c| c.engine.swipe_haptic)
                .map_err(|e| e.to_string())
        };
        assert_eq!(
            super::load("").unwrap().engine.swipe_haptic,
            None,
            "default off"
        );
        assert_eq!(haptic("false"), Ok(None));
        assert_eq!(haptic("true"), Ok(Some(1)));
        assert_eq!(haptic("\"subtle-collision\""), Ok(Some(4)));
        assert!(haptic("\"nope\"").unwrap_err().contains("unknown waveform"));
    }

    #[test]
    fn diagonal_swipes_and_axis_thresholds() {
        use detent_engine::SwipeThresholds;
        let swipe = |g: &str| {
            super::load(&format!("[gesture]\n{g}\n"))
                .map(|c| c.engine.swipe)
                .map_err(|e| e.to_string())
        };
        let default = SwipeThresholds {
            x_um: 6000,
            y_um: 3000,
        };
        assert_eq!(super::load("").unwrap().engine.swipe, default);
        assert_eq!(swipe("haptic = true"), Ok(default));
        assert_eq!(
            swipe("threshold_mm = 8"),
            Ok(SwipeThresholds::uniform(8000))
        );
        assert_eq!(
            swipe("threshold_x_mm = 10\nthreshold_y_mm = 3.5"),
            Ok(SwipeThresholds {
                x_um: 10_000,
                y_um: 3500
            })
        );
        assert_eq!(
            swipe("threshold_mm = 8\nthreshold_y_mm = 4"),
            Ok(SwipeThresholds {
                x_um: 8000,
                y_um: 4000
            }),
            "an unset axis falls back to threshold_mm"
        );
        assert_eq!(
            swipe("threshold_y_mm = 4"),
            Ok(SwipeThresholds {
                x_um: 6000,
                y_um: 4000
            }),
            "...or to its default"
        );
        assert!(
            swipe("threshold_y_mm = 0.5")
                .unwrap_err()
                .contains("threshold_y_mm")
        );

        let text = "[layer.gesture]\nhold = \"gesture\"\n\
                    \"swipe.up_left\" = { keys = \"a\" }\n\
                    \"swipe.down_right\" = { keys = \"b\" }\n";
        let c = super::load(text).unwrap();
        let t = &c.engine.profiles[0].bindings;
        assert!(matches!(
            t.lookup(LayerId(1), slot::SWIPE, Edge::SwipeUpLeft),
            Action::Keys(_)
        ));
        assert!(matches!(
            t.lookup(LayerId(1), slot::SWIPE, Edge::SwipeDownRight),
            Action::Keys(_)
        ));
        assert!(
            t.lookup(LayerId(1), slot::SWIPE, Edge::SwipeUpRight)
                .is_none()
        );
        assert_eq!(c.engine.swipe_mods.0, 1);
    }
}
