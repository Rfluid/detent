//! Actions this plugin adds to the `kde.*` namespace.
//!
//! Shortcut names are kglobalaccel action names, verified on Plasma 6.8.3 with
//! `busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component
//! shortcutNames` (docs/plugins/kde.md, .agent/memory/facts/2026-11-23-kde-shortcut-names.md).

/// `(name, doc, continuous)`
pub const DECLS: &[(&str, &str, bool)] = &[
    ("overview", "Toggle KWin Overview", false),
    (
        "overview.up",
        "Like a 4-finger touchpad swipe up: desktop → Overview → Grid",
        false,
    ),
    (
        "overview.down",
        "Like a 4-finger touchpad swipe down: desktop → Grid, Grid → Overview, Overview → closed",
        false,
    ),
    ("grid", "Toggle the desktop grid", false),
    ("windows", "Present windows (current desktop)", false),
    ("windows.all", "Present windows (all desktops)", false),
    ("desktop.next", "Switch to the next virtual desktop", false),
    (
        "desktop.prev",
        "Switch to the previous virtual desktop",
        false,
    ),
    ("desktop.left", "Switch one desktop to the left", false),
    ("desktop.right", "Switch one desktop to the right", false),
    (
        "window.maximize",
        "Maximize/restore the active window",
        false,
    ),
    ("window.minimize", "Minimize the active window", false),
    ("window.close", "Close the active window", false),
    ("tile.left", "Quick-tile the window left", false),
    ("tile.right", "Quick-tile the window right", false),
    (
        "volume",
        "Volume up/down with the Plasma OSD (continuous)",
        true,
    ),
    ("volume.up", "Volume up (OSD)", false),
    ("volume.down", "Volume down (OSD)", false),
    (
        "volume.mute",
        "Mute/unmute (OSD); with params { reply, seq? } toggles through PipeWire and replies \
         like volume.get",
        false,
    ),
    (
        "volume.get",
        "Report the output volume: params { reply, seq?, max? } → invokes `reply` with \
         { source: \"kde.volume\", seq, percent, muted, max, label } or { source, seq, error }",
        false,
    ),
    (
        "volume.set",
        "Set the output volume: params { percent, max? (100, ≤150), unmute?, osd?, reply?, \
         seq? }; bursts are merged (the last target wins)",
        false,
    ),
    (
        "volume.targets",
        "List the output devices: params { reply, seq? } → invokes `reply` with { source: \
         \"kde.volume\", seq, targets: [{ id, name, icon, percent, muted, current }] } or \
         { source, seq, error }",
        false,
    ),
    (
        "volume.target",
        "Switch the default output and move what is playing to it: params { id, max?, \
         reply?, seq? }; replies like volume.get for the new output",
        false,
    ),
    (
        "terminal",
        "Open the default terminal (System Settings → Default Applications; Konsole if unset)",
        false,
    ),
    (
        "brightness",
        "Screen brightness up/down with OSD (continuous)",
        true,
    ),
    ("zoom", "KWin zoom in/out (continuous)", true),
    (
        "shortcut",
        "Invoke any global shortcut: params { component, name }",
        false,
    ),
    (
        "cursor",
        "Report the pointer position: params { reply } → invokes `reply` with { x, y, output } \
         (logical px relative to that output)",
        false,
    ),
];

/// Action names a `reply` may use: `plugin.action`, lowercase, no quotes (they are
/// embedded in a KWin script).
pub fn valid_reply(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.contains('.')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// kglobalaccel `(component, shortcut name)` for discrete actions.
pub fn shortcut_for(action: &str) -> Option<(&'static str, &'static str)> {
    Some(match action {
        "overview" => ("kwin", "Overview"),
        // KWin's overview plugin (Plasma/6.8 overvieweffect.cpp): `cycle()` / `reverseCycle()`
        // walk the same states as its 4-finger touchpad up / down gestures.
        "overview.up" => ("kwin", "Cycle Overview"),
        "overview.down" => ("kwin", "Cycle Overview Opposite"),
        "grid" => ("kwin", "Grid View"),
        "windows" => ("kwin", "Expose"),
        "windows.all" => ("kwin", "ExposeAll"),
        "desktop.next" => ("kwin", "Switch to Next Desktop"),
        "desktop.prev" => ("kwin", "Switch to Previous Desktop"),
        "desktop.left" => ("kwin", "Switch One Desktop to the Left"),
        "desktop.right" => ("kwin", "Switch One Desktop to the Right"),
        "window.maximize" => ("kwin", "Window Maximize"),
        "window.minimize" => ("kwin", "Window Minimize"),
        "window.close" => ("kwin", "Window Close"),
        "tile.left" => ("kwin", "Window Quick Tile Left"),
        "tile.right" => ("kwin", "Window Quick Tile Right"),
        "volume.up" => ("kmix", "increase_volume"),
        "volume.down" => ("kmix", "decrease_volume"),
        "volume.mute" => ("kmix", "mute"),
        _ => return None,
    })
}

/// Continuous actions: `(component, up, down)` invoked once per detent.
pub fn continuous_for(action: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match action {
        "volume" => ("kmix", "increase_volume", "decrease_volume"),
        "brightness" => (
            "org_kde_powerdevil",
            "Increase Screen Brightness",
            "Decrease Screen Brightness",
        ),
        "zoom" => ("kwin", "view_zoom_in", "view_zoom_out"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_names() {
        assert!(valid_reply("radial.cursor"));
        assert!(valid_reply("my-osd.pos_2"));
        assert!(!valid_reply("cursor"));
        assert!(!valid_reply("radial.cursor\"); evil(\""));
        assert!(!valid_reply("Radial.Cursor"));
        assert!(!valid_reply(""));
    }

    #[test]
    fn every_action_has_a_mapping() {
        for (name, _, continuous) in DECLS {
            if matches!(
                *name,
                "shortcut"
                    | "cursor"
                    | "volume.get"
                    | "volume.set"
                    | "volume.targets"
                    | "volume.target"
                    | "terminal"
            ) {
                continue;
            }
            if *continuous {
                assert!(continuous_for(name).is_some(), "no mapping for kde.{name}");
            } else {
                assert!(shortcut_for(name).is_some(), "no mapping for kde.{name}");
            }
        }
    }
}
