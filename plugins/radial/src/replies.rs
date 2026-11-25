//! Providers' replies (`radial.level`, `radial.targets`) → state machine inputs.
//! `targets_input` resolves icons (file system lookups), so it runs on the bus thread.

use serde_json::Value;

use crate::icon;
use crate::state::{At, Choice, Input, Level, MAX_TARGETS};

fn seq_of(p: &Value) -> u64 {
    p.get("seq").and_then(Value::as_u64).unwrap_or(0)
}

fn str_of<'a>(p: &'a Value, key: &str) -> &'a str {
    p.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// `radial.cursor` params `{ x, y, output, width?, height? }` (from `kde.cursor`) → `At`.
pub fn cursor_at(p: &Value) -> Option<At> {
    let num = |k: &str| p.get(k).and_then(Value::as_f64).map(|v| v as f32);
    let size = match (num("width"), num("height")) {
        (Some(w), Some(h)) if w >= 1. && h >= 1. => Some((w, h)),
        _ => None,
    };
    Some(At {
        x: num("x")?,
        y: num("y")?,
        output: str_of(p, "output").to_string(),
        size,
    })
}

/// `radial.level` params → `Input::Level`.
pub fn level_input(p: &Value) -> Option<Input> {
    let source = p.get("source")?.as_str()?.to_string();
    let seq = seq_of(p);
    let level = match p.get("error").and_then(Value::as_str) {
        Some(e) => Err(e.to_string()),
        None => {
            let num = |k: &str| p.get(k).and_then(Value::as_u64);
            Ok(Level {
                percent: num("percent")? as u32,
                max: num("max").unwrap_or(100) as u32,
                muted: p.get("muted").and_then(Value::as_bool) == Some(true),
                label: str_of(p, "label").to_string(),
            })
        }
    };
    Some(Input::Level { source, seq, level })
}

/// One entry of `targets`: `{ id, name, description?, icon?, percent?, muted?, current? }`.
fn choice(t: &Value) -> Option<Choice> {
    let id = match t.get("id")? {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    let name = match str_of(t, "name") {
        "" => "Output",
        n => n,
    };
    let description = match str_of(t, "description") {
        "" => name,
        d => d,
    };
    let icon = match str_of(t, "icon") {
        "" => icon::Icon::None,
        spec => icon::resolve_quiet(spec, Some(true)),
    };
    Some(Choice {
        id,
        name: name.to_string(),
        description: description.to_string(),
        icon,
        percent: t.get("percent").and_then(Value::as_u64).map(|v| v as u32),
        muted: t.get("muted").and_then(Value::as_bool) == Some(true),
        current: t.get("current").and_then(Value::as_bool) == Some(true),
    })
}

/// `radial.targets` params → `Input::Targets` (at most `MAX_TARGETS`).
pub fn targets_input(p: &Value) -> Option<Input> {
    let source = p.get("source")?.as_str()?.to_string();
    let seq = seq_of(p);
    let targets = match p.get("error").and_then(Value::as_str) {
        Some(e) => Err(e.to_string()),
        None => Ok(p
            .get("targets")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(choice).take(MAX_TARGETS).collect())
            .unwrap_or_default()),
    };
    Some(Input::Targets {
        source,
        seq,
        targets,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn cursor() {
        let at = cursor_at(&json!({ "x": 900.5, "y": 560, "output": "eDP-1",
            "width": 1800, "height": 1125 }))
        .unwrap();
        assert_eq!((at.x, at.y, at.output.as_str()), (900.5, 560., "eDP-1"));
        assert_eq!(at.size, Some((1800., 1125.)));
        // An older detent-kde (no size) or a zero size: open as before.
        let at = cursor_at(&json!({ "x": 1, "y": 2, "output": "DP-6" })).unwrap();
        assert_eq!(at.size, None);
        let at = cursor_at(&json!({ "x": 1, "y": 2, "width": 0, "height": 0 })).unwrap();
        assert_eq!(at.size, None);
        assert!(cursor_at(&json!({ "y": 2 })).is_none());
    }

    #[test]
    fn levels() {
        let i = level_input(&json!({ "source": "kde.volume", "seq": 3, "percent": 40,
            "muted": true, "max": 150, "label": "HDMI 1" }));
        assert_eq!(
            i,
            Some(Input::Level {
                source: "kde.volume".into(),
                seq: 3,
                level: Ok(Level {
                    percent: 40,
                    max: 150,
                    muted: true,
                    label: "HDMI 1".into()
                })
            })
        );
        let e = level_input(&json!({ "source": "kde.volume", "error": "no wpctl" }));
        assert!(matches!(e, Some(Input::Level { level: Err(_), .. })));
        assert_eq!(level_input(&json!({ "seq": 1 })), None);
    }

    #[test]
    fn targets() {
        let i = targets_input(&json!({ "source": "kde.volume", "seq": 9, "targets": [
            { "id": "71", "name": "HDMI 1", "percent": 40, "current": false,
              "description": "HD Audio HDMI / DisplayPort 1 Output (Stereo)" },
            { "id": 76, "name": "AB13X USB Audio", "muted": true, "current": true },
            { "name": "no id" },
        ]}));
        let Some(Input::Targets {
            source,
            seq,
            targets: Ok(t),
        }) = i
        else {
            panic!("not parsed: {i:?}");
        };
        assert_eq!((source.as_str(), seq, t.len()), ("kde.volume", 9, 2));
        assert_eq!(
            (t[0].id.as_str(), t[0].percent, t[0].current),
            ("71", Some(40), false)
        );
        assert_eq!(
            (t[1].id.as_str(), t[1].muted, t[1].current),
            ("76", true, true)
        );
        assert_eq!(t[1].icon, icon::Icon::None);
        assert_eq!(
            t[0].description,
            "HD Audio HDMI / DisplayPort 1 Output (Stereo)"
        );
        // no description: the short name stands in
        assert_eq!(t[1].description, "AB13X USB Audio");
        let e = targets_input(&json!({ "source": "kde.volume", "seq": 1, "error": "no pw-dump" }));
        assert!(matches!(
            e,
            Some(Input::Targets {
                targets: Err(_),
                ..
            })
        ));
    }
}
