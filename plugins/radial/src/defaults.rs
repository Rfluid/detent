//! Default icons: an item without `icon` / `image` / `text` gets an icon-theme name chosen
//! from what it does (Copy → `edit-copy`, a URL → `globe`, ...). Pure; the names are
//! freedesktop names, all present in Breeze (checked 2026-11-24), resolved like `icon = ...`
//! (the `-symbolic` variant is preferred, drawn as a glyph). Spec: docs/plugins/radial.md.

use serde_json::Value;

use crate::icon::{desktop_entry, expand_home};
use crate::menu::{ItemAction, Step};

/// Fallbacks when nothing more specific matches.
const KEYS: &str = "input-keyboard";
const RUN: &str = "system-run";

/// `kde.*` and other known plugin actions.
fn for_action(action: &str) -> Option<&'static str> {
    Some(match action {
        "kde.overview" | "kde.overview.up" | "kde.overview.down" | "kde.grid" => "view-grid",
        "kde.windows" | "kde.windows.all" => "window-duplicate",
        "kde.desktop.next" | "kde.desktop.right" => "go-next",
        "kde.desktop.prev" | "kde.desktop.left" => "go-previous",
        "kde.window.maximize" => "window-maximize",
        "kde.window.minimize" => "window-minimize",
        "kde.window.close" => "window-close",
        "kde.tile.left" => "align-horizontal-left",
        "kde.tile.right" => "align-horizontal-right",
        "kde.volume.up" => "audio-volume-high",
        "kde.volume.down" => "audio-volume-low",
        "kde.volume.mute" => "audio-volume-muted",
        "kde.volume" => "audio-volume-medium",
        "kde.brightness" => "brightness-high",
        "kde.zoom" => "zoom-in",
        "kde.shortcut" => "configure-shortcuts",
        "kde.terminal" => "utilities-terminal",
        "radial.open" => "view-more",
        _ => return None,
    })
}

/// A key combo in a canonical spelling: lowercase, modifiers ordered ctrl, alt, shift, super,
/// key aliases folded (`volup` → `volumeup`, `prtsc` → `print`, ...).
fn canonical_keys(keys: &str) -> String {
    let mut mods = [false; 4];
    let mut key = String::new();
    for part in keys.split('+').map(|p| p.trim().to_ascii_lowercase()) {
        match part.as_str() {
            "ctrl" | "control" | "lctrl" | "rctrl" => mods[0] = true,
            "alt" | "lalt" | "ralt" | "altgr" => mods[1] = true,
            "shift" | "lshift" | "rshift" => mods[2] = true,
            "super" | "meta" | "win" | "cmd" => mods[3] = true,
            _ => key = part,
        }
    }
    let key = match key.trim_start_matches("key_") {
        "play" | "playpause" => "playpause",
        "next" | "nextsong" => "nextsong",
        "prev" | "previous" | "previoussong" => "previoussong",
        "volup" | "volumeup" => "volumeup",
        "voldown" | "volumedown" => "volumedown",
        "print" | "printscreen" | "prtsc" | "sysrq" => "print",
        "equal" | "plus" => "plus",
        "ins" | "insert" => "insert",
        k => k,
    }
    .to_string();
    let names = ["ctrl", "alt", "shift", "super"];
    let mut out: Vec<&str> = names
        .iter()
        .zip(mods)
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect();
    out.push(&key);
    out.join("+")
}

fn for_keys(keys: &str) -> &'static str {
    match canonical_keys(keys).as_str() {
        "ctrl+c" | "ctrl+insert" => "edit-copy",
        "ctrl+v" | "shift+insert" => "edit-paste",
        "ctrl+x" => "edit-cut",
        "ctrl+z" => "edit-undo",
        "ctrl+shift+z" | "ctrl+y" => "edit-redo",
        "ctrl+s" => "document-save",
        "ctrl+a" => "edit-select-all",
        "ctrl+f" => "edit-find",
        "ctrl+p" => "document-print",
        "ctrl+t" => "tab-new",
        "ctrl+w" | "ctrl+f4" => "tab-close",
        "ctrl+n" => "document-new",
        "ctrl+o" => "document-open",
        "alt+left" => "go-previous",
        "alt+right" => "go-next",
        "f5" | "ctrl+r" => "view-refresh",
        "alt+f4" => "window-close",
        "ctrl+plus" => "zoom-in",
        "ctrl+minus" => "zoom-out",
        "playpause" => "media-playback-start",
        "nextsong" => "media-skip-forward",
        "previoussong" => "media-skip-backward",
        "mute" => "audio-volume-muted",
        "volumeup" => "audio-volume-high",
        "volumedown" => "audio-volume-low",
        "print" => "camera-photo",
        _ => KEYS,
    }
}

/// An icon for one bus invocation (`exec`, `keys`, or a named action).
fn for_invoke(action: &str, params_json: &str) -> String {
    let params: Value = serde_json::from_str(params_json).unwrap_or(Value::Null);
    let param = |k: &str| params.get(k).and_then(Value::as_str).unwrap_or_default();
    match action {
        "keys" => for_keys(param("keys")).to_string(),
        "exec" => for_command(param("cmd")),
        a => for_action(a).unwrap_or(RUN).to_string(),
    }
}

/// `exec`: the program's own icon when it has a desktop file (`spectacle -r` → Spectacle).
fn for_command(cmd: &str) -> String {
    let prog = cmd.split_whitespace().next().unwrap_or_default();
    let name = prog.rsplit('/').next().unwrap_or(prog);
    if name.is_empty() {
        return RUN.into();
    }
    desktop_entry(name)
        .and_then(|d| d.icon)
        .unwrap_or_else(|| RUN.into())
}

fn for_open(target: &str) -> &'static str {
    let t = target.trim();
    if t.contains("://") || t.starts_with("www.") || t.starts_with("mailto:") {
        "globe"
    } else if std::path::Path::new(&expand_home(t)).is_dir() {
        "folder"
    } else {
        "document-open"
    }
}

/// The default icon name for an item that names none, and whether to keep its colours
/// (app icons from desktop files) instead of drawing it as a glyph.
pub fn icon_for(action: &ItemAction) -> (String, Option<bool>) {
    match action {
        ItemAction::App(id) => match desktop_entry(id).and_then(|d| d.icon) {
            Some(i) => (i, Some(false)),
            None => (RUN.into(), None),
        },
        ItemAction::Open(t) => (for_open(t).into(), None),
        ItemAction::Folder(_) => ("folder".into(), None),
        ItemAction::Adjust(a) => (for_action(&a.source).unwrap_or("view-more").into(), None),
        ItemAction::Exec(c) => {
            let i = for_command(c);
            let colour = (i != RUN).then_some(false);
            (i, colour)
        }
        other => match other.steps().into_iter().find_map(|s| match s {
            Step::Invoke(a, p) => Some((a, p)),
            Step::DelayMs(_) => None,
        }) {
            Some((a, p)) => (for_invoke(&a, &p), None),
            None => (RUN.into(), None),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_canonical() {
        assert_eq!(canonical_keys("Ctrl+C"), "ctrl+c");
        assert_eq!(canonical_keys("shift + control + Z"), "ctrl+shift+z");
        assert_eq!(canonical_keys("VolUp"), "volumeup");
        assert_eq!(canonical_keys("KEY_PLAYPAUSE"), "playpause");
    }

    #[test]
    fn icons_for_common_actions() {
        let keys = |k: &str| icon_for(&ItemAction::Keys(k.into())).0;
        assert_eq!(keys("Ctrl+C"), "edit-copy");
        assert_eq!(keys("ctrl+v"), "edit-paste");
        assert_eq!(keys("Ctrl+Shift+Z"), "edit-redo");
        assert_eq!(keys("PlayPause"), "media-playback-start");
        assert_eq!(keys("Alt+Left"), "go-previous");
        assert_eq!(keys("Ctrl+Alt+K"), "input-keyboard");
        let act = |a: &str| icon_for(&ItemAction::Action(a.into(), "{}".into())).0;
        assert_eq!(act("kde.overview"), "view-grid");
        assert_eq!(act("kde.desktop.next"), "go-next");
        assert_eq!(act("my-plugin.thing"), "system-run");
        assert_eq!(act("kde.terminal"), "utilities-terminal");
        assert_eq!(
            icon_for(&ItemAction::Open("https://x.org".into())).0,
            "globe"
        );
        assert_eq!(icon_for(&ItemAction::Open("/".into())).0, "folder");
        assert_eq!(icon_for(&ItemAction::Folder(vec![])).0, "folder");
        assert_eq!(
            icon_for(&ItemAction::Exec("/nonexistent/detent-test-tool -x".into())),
            ("system-run".into(), None)
        );
        // a macro takes its first step's icon
        let m = ItemAction::Macro(vec![
            Step::DelayMs(10),
            Step::Invoke("keys".into(), r#"{"keys":"Ctrl+A"}"#.into()),
        ]);
        assert_eq!(icon_for(&m).0, "edit-select-all");
    }
}
