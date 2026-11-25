//! Key and button names → Detent key codes. `"Ctrl+Shift+Tab"` → `[29, 42, 15]`.
//!
//! Names are case-insensitive. Accepted: aliases below, single letters/digits, and any
//! kernel name with or without the `KEY_` prefix (`"VolumeUp"`, `"KEY_VOLUMEUP"`, `"f5"`).
//!
//! Layout: this file owns parsing; `keys/names.rs` holds the name → code table. Codes are
//! the Linux input-event-codes numbers, Detent's OS-independent key vocabulary (ADR 0010),
//! so nothing here depends on an OS crate.

mod names;

const ALIASES: &[(&str, &str)] = &[
    ("ctrl", "KEY_LEFTCTRL"),
    ("control", "KEY_LEFTCTRL"),
    ("lctrl", "KEY_LEFTCTRL"),
    ("rctrl", "KEY_RIGHTCTRL"),
    ("shift", "KEY_LEFTSHIFT"),
    ("lshift", "KEY_LEFTSHIFT"),
    ("rshift", "KEY_RIGHTSHIFT"),
    ("alt", "KEY_LEFTALT"),
    ("lalt", "KEY_LEFTALT"),
    ("altgr", "KEY_RIGHTALT"),
    ("ralt", "KEY_RIGHTALT"),
    ("super", "KEY_LEFTMETA"),
    ("meta", "KEY_LEFTMETA"),
    ("win", "KEY_LEFTMETA"),
    ("cmd", "KEY_LEFTMETA"),
    ("esc", "KEY_ESC"),
    ("escape", "KEY_ESC"),
    ("enter", "KEY_ENTER"),
    ("return", "KEY_ENTER"),
    ("del", "KEY_DELETE"),
    ("ins", "KEY_INSERT"),
    ("pgup", "KEY_PAGEUP"),
    ("pgdn", "KEY_PAGEDOWN"),
    ("pagedown", "KEY_PAGEDOWN"),
    ("pageup", "KEY_PAGEUP"),
    ("bs", "KEY_BACKSPACE"),
    ("print", "KEY_SYSRQ"),
    ("printscreen", "KEY_SYSRQ"),
    ("prtsc", "KEY_SYSRQ"),
    ("caps", "KEY_CAPSLOCK"),
    ("plus", "KEY_EQUAL"),
    ("minus", "KEY_MINUS"),
    ("comma", "KEY_COMMA"),
    ("period", "KEY_DOT"),
    ("dot", "KEY_DOT"),
    ("slash", "KEY_SLASH"),
    ("backslash", "KEY_BACKSLASH"),
    ("semicolon", "KEY_SEMICOLON"),
    ("quote", "KEY_APOSTROPHE"),
    ("grave", "KEY_GRAVE"),
    ("backtick", "KEY_GRAVE"),
    ("play", "KEY_PLAYPAUSE"),
    ("playpause", "KEY_PLAYPAUSE"),
    ("next", "KEY_NEXTSONG"),
    ("prev", "KEY_PREVIOUSSONG"),
    ("previous", "KEY_PREVIOUSSONG"),
    ("mute", "KEY_MUTE"),
    ("volup", "KEY_VOLUMEUP"),
    ("voldown", "KEY_VOLUMEDOWN"),
];

/// One key name → code.
pub fn key_code(name: &str) -> Option<u16> {
    let lower = name.trim().to_ascii_lowercase();
    if lower.is_empty() {
        return None;
    }
    if let Some((_, k)) = ALIASES.iter().find(|(a, _)| *a == lower) {
        return by_name(k);
    }
    let upper = lower.to_ascii_uppercase();
    let full = if upper.starts_with("KEY_") || upper.starts_with("BTN_") {
        upper
    } else {
        format!("KEY_{upper}")
    };
    by_name(&full)
}

/// Exact (upper-case) `KEY_*` / `BTN_*` name → code.
fn by_name(full: &str) -> Option<u16> {
    names::NAMES
        .iter()
        .find(|(n, _)| *n == full)
        .map(|&(_, code)| code)
}

/// `"Ctrl+Shift+Tab"` → codes in press order (≤ 6).
pub fn parse_chord(s: &str) -> Result<Vec<u16>, String> {
    let parts: Vec<&str> = if s.trim() == "+" {
        vec!["plus"]
    } else {
        s.split('+').collect()
    };
    if parts.len() > 6 {
        return Err(format!("`{s}`: at most 6 keys"));
    }
    parts
        .iter()
        .map(|p| key_code(p).ok_or_else(|| format!("unknown key `{}` in `{s}`", p.trim())))
        .collect()
}

/// Mouse button names for `button = "..."`.
pub fn button_code(name: &str) -> Option<u16> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "left" => 0x110,
        "right" => 0x111,
        "middle" => 0x112,
        "back" | "side" => 0x113,
        "forward" | "extra" => 0x114,
        "button6" => 0x115,
        "button7" => 0x116,
        "task" => 0x117,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chords() {
        assert_eq!(parse_chord("Ctrl+Shift+Tab").unwrap(), vec![29, 42, 15]);
        assert_eq!(parse_chord("Alt+Left").unwrap(), vec![56, 105]);
        assert_eq!(parse_chord("ctrl+c").unwrap(), vec![29, 46]);
        assert_eq!(parse_chord("Super").unwrap(), vec![125]);
        assert_eq!(parse_chord("KEY_VOLUMEUP").unwrap(), vec![115]);
        assert_eq!(parse_chord("F5").unwrap(), vec![63]);
        assert!(parse_chord("Ctrl+Nope").is_err());
        assert_eq!(button_code("back"), Some(0x113));
    }

    #[test]
    fn every_alias_resolves() {
        for (alias, name) in ALIASES {
            assert!(
                by_name(name).is_some(),
                "alias `{alias}` → unknown `{name}`"
            );
        }
    }

    /// The table must parse exactly like `evdev::KeyCode::from_str` did before this crate
    /// dropped the dependency (ADR 0010): same names, same codes, nothing missing.
    #[cfg(target_os = "linux")]
    #[test]
    fn table_matches_evdev() {
        use std::str::FromStr;
        for &(name, code) in names::NAMES {
            assert_eq!(
                evdev::KeyCode::from_str(name).map(|c| c.code()),
                Ok(code),
                "{name}"
            );
        }
        // Every code evdev can name has an entry (its Debug prints the first name per code).
        for code in 0..0x300u16 {
            let dbg = format!("{:?}", evdev::KeyCode::new(code));
            if !dbg.starts_with("unknown") {
                assert_eq!(by_name(&dbg), Some(code), "{dbg} missing");
            }
        }
    }
}
