//! Parsing `[plugins.radial]` (as JSON) into [`Settings`]: items, actions, folders, macros,
//! adjustable items, menus and per-app rings.

use serde_json::{Map, Value};

use super::{
    Adjust, Item, ItemAction, Labels, Layout, MAX_FOLDER_ITEMS, MAX_RING_ITEMS, Menu, Settings,
    Step, Theme, parse_color,
};
use crate::defaults;
use crate::icon::{self, Icon};

const ITEM_KEYS: &[&str] = &[
    "label",
    "icon",
    "image",
    "text",
    "tint",
    "color",
    "accent",
    "action",
    "exec",
    "keys",
    "app",
    "open",
    "steps",
    "items",
    "adjust",
    "step",
    "fine_step",
    "max",
    "invert_fine",
];

fn parse_adjust(source: &str, v: &Value, ctx: &str) -> Result<Adjust, String> {
    if !source.contains('.') || source.contains(char::is_whitespace) {
        return Err(format!(
            "{ctx}: `adjust` names a provider like \"kde.volume\""
        ));
    }
    let pct = |k: &str, default: u32, max: u32| -> Result<u32, String> {
        match v.get(k) {
            None => Ok(default),
            Some(n) => n
                .as_u64()
                .filter(|n| (1..=u64::from(max)).contains(n))
                .map(|n| n as u32)
                .ok_or_else(|| format!("{ctx}: `{k}` is a whole number from 1 to {max}")),
        }
    };
    // Muted glyph for the known providers (theme icons, like the defaults).
    let muted_icon = match source {
        "kde.volume" => match icon::resolve_quiet("audio-volume-muted", None) {
            Icon::None => None,
            i => Some(i),
        },
        _ => None,
    };
    Ok(Adjust {
        source: source.to_string(),
        step: pct("step", 2, 25)?,
        fine_step: pct("fine_step", 1, 25)?,
        max: pct("max", 100, Adjust::MAX_LIMIT)?,
        invert_fine: v.get("invert_fine").and_then(Value::as_bool) == Some(true),
        muted_icon,
    })
}

fn str_of<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}

/// The `action = "..."` params: every key that isn't an item key.
fn action_params(o: &Map<String, Value>, reserved: &[&str]) -> String {
    let params: Map<String, Value> = o
        .iter()
        .filter(|(k, _)| !reserved.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Value::Object(params).to_string()
}

fn parse_step(v: &Value, ctx: &str) -> Result<Step, String> {
    if let Some(ms) = v.get("delay_ms").and_then(Value::as_u64) {
        return Ok(Step::DelayMs(ms.min(10_000) as u32));
    }
    let empty = Map::new();
    let o = v.as_object().unwrap_or(&empty);
    let action = parse_action(v, o, ctx, 0)?;
    match action {
        ItemAction::Folder(_) | ItemAction::Macro(_) | ItemAction::Adjust(_) => Err(format!(
            "{ctx}: a macro step can't be a folder, an adjustable item or another macro"
        )),
        a => a
            .steps()
            .into_iter()
            .next()
            .ok_or_else(|| format!("{ctx}: empty step")),
    }
}

fn parse_action(
    v: &Value,
    o: &Map<String, Value>,
    ctx: &str,
    depth: usize,
) -> Result<ItemAction, String> {
    if let Some(a) = str_of(v, "adjust") {
        Ok(ItemAction::Adjust(parse_adjust(a, v, ctx)?))
    } else if let Some(a) = str_of(v, "action") {
        Ok(ItemAction::Action(
            a.to_string(),
            action_params(o, ITEM_KEYS),
        ))
    } else if let Some(e) = str_of(v, "exec") {
        Ok(ItemAction::Exec(e.to_string()))
    } else if let Some(k) = str_of(v, "keys") {
        Ok(ItemAction::Keys(k.to_string()))
    } else if let Some(a) = str_of(v, "app") {
        Ok(ItemAction::App(a.trim_end_matches(".desktop").to_string()))
    } else if let Some(t) = str_of(v, "open") {
        Ok(ItemAction::Open(t.to_string()))
    } else if let Some(steps) = v.get("steps").and_then(Value::as_array) {
        if steps.is_empty() {
            return Err(format!("{ctx}: `steps` is empty"));
        }
        let steps = steps
            .iter()
            .map(|s| parse_step(s, ctx))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ItemAction::Macro(steps))
    } else if let Some(items) = v.get("items").and_then(Value::as_array) {
        if depth > 0 {
            return Err(format!("{ctx}: folders can't contain folders"));
        }
        let items = parse_items(items, ctx, MAX_FOLDER_ITEMS, depth + 1)?;
        Ok(ItemAction::Folder(items))
    } else {
        Err(format!(
            "{ctx} needs one of `action`, `exec`, `keys`, `app`, `open`, `steps`, `items` or \
             `adjust`"
        ))
    }
}

fn parse_item(v: &Value, depth: usize) -> Result<Item, String> {
    let o = v.as_object().ok_or("menu items are tables `{ ... }`")?;
    let named = str_of(v, "label").map(str::to_string);
    let ctx = format!("menu item `{}`", named.as_deref().unwrap_or("?"));
    let action = parse_action(v, o, &ctx, depth)?;
    // Apps bring their own name and icon from their desktop file (else their id, if not installed).
    let (desktop, app_id) = match &action {
        ItemAction::App(id) => (icon::desktop_entry(id), Some(id.clone())),
        _ => (None, None),
    };
    let label = named
        .or_else(|| desktop.as_ref().map(|d| d.name.clone()).or(app_id))
        .ok_or_else(|| format!("{ctx}: needs `label`"))?;
    let tint = v.get("tint").and_then(Value::as_bool);
    // Every item gets an icon: the one named, else one chosen from what it does (app icon,
    // `edit-copy` for Ctrl+C, ...); the label's initials only if the theme has neither.
    let default_icon = || {
        let (name, keep_colours) = defaults::icon_for(&action);
        icon::resolve_quiet(&name, tint.or(keep_colours))
    };
    let icon = if let Some(p) = str_of(v, "image") {
        match Icon::photo(p) {
            Icon::None => default_icon(),
            i => i,
        }
    } else if let Some(t) = str_of(v, "text") {
        Icon::Text(t.to_string())
    } else if let Some(i) = str_of(v, "icon") {
        match icon::resolve(i, tint) {
            Icon::None => default_icon(),
            i => i,
        }
    } else {
        default_icon()
    };
    let color = str_of(v, "color").map(parse_color).transpose()?;
    let accent = str_of(v, "accent").map(parse_color).transpose()?;
    Ok(Item {
        label,
        icon,
        action,
        color,
        accent,
    })
}

fn parse_items(items: &[Value], ctx: &str, max: usize, depth: usize) -> Result<Vec<Item>, String> {
    if items.is_empty() || items.len() > max {
        return Err(format!("{ctx}: 1 to {max} items"));
    }
    let items = items
        .iter()
        .map(|v| parse_item(v, depth))
        .collect::<Result<Vec<_>, _>>()?;
    // The wheel drives one gauge at a time.
    let adjustable = items
        .iter()
        .filter(|i| matches!(i.action, ItemAction::Adjust(_)))
        .count();
    if adjustable > 1 {
        return Err(format!("{ctx}: at most one `adjust` item per ring"));
    }
    Ok(items)
}

fn parse_menu(name: &str, m: &Value) -> Result<Menu, String> {
    let ctx = format!("menu `{name}`");
    let items = m
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{ctx} needs `items = [...]`"))?;
    let items = parse_items(items, &ctx, MAX_RING_ITEMS, 0)?;
    let mut apps = Vec::new();
    if let Some(a) = m.get("apps").and_then(Value::as_object) {
        for (app, v) in a {
            let actx = format!("{ctx}, app `{app}`");
            let list = v
                .get("items")
                .and_then(Value::as_array)
                .ok_or_else(|| format!("{actx} needs `items = [...]`"))?;
            apps.push((app.clone(), parse_items(list, &actx, MAX_RING_ITEMS, 0)?));
        }
    }
    Ok(Menu { items, apps })
}

impl Settings {
    pub fn parse(json: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let mut s = Self::default();
        // Older configs: the ring now behaves like the Actions Ring, where both styles
        // (press-release + click, hold + release on a bubble) always work, and the bubble
        // under the cursor is selected (no dead zone). Accept the keys, say they're unused.
        for old in ["mode", "dead_zone"] {
            if v.get(old).is_some() {
                eprintln!(
                    "detent-radial: `{old}` is no longer used (click and hold both work; \
                     the bubble under the cursor is selected)"
                );
            }
        }
        let mut ring = match str_of(&v, "size").unwrap_or("medium") {
            "small" => 68.0,
            "medium" => 84.0,
            "large" => 104.0,
            other => return Err(format!("size `{other}`: use small, medium or large")),
        };
        if let Some(r) = v.get("radius").and_then(Value::as_f64) {
            ring = (r as f32).clamp(40.0, 300.0);
        }
        s.layout = Layout::for_ring(ring);
        if let Some(h) = v.get("haptic_ticks").and_then(Value::as_bool) {
            s.haptic_ticks = h;
        }
        if let Some(l) = str_of(&v, "labels") {
            s.labels = match l {
                "never" => Labels::Never,
                "hover" => Labels::Hover,
                "always" => Labels::Always,
                other => return Err(format!("labels `{other}`: use never, hover or always")),
            };
        }
        if let Some(t) = v.get("tooltip_ms").and_then(Value::as_u64) {
            s.tooltip_ms = t.min(5000) as u32;
        }
        if let Some(a) = v.get("animation_ms").and_then(Value::as_u64) {
            s.animation_ms = a.min(1000) as u32;
        }
        s.theme = match str_of(&v, "theme").unwrap_or("system") {
            "system" => Theme::System,
            "light" => Theme::Light,
            "dark" => Theme::Dark,
            other => return Err(format!("theme `{other}`: use system, light or dark")),
        };
        if let Some(c) = v.get("colors").and_then(Value::as_object) {
            for (k, val) in c {
                let col = val
                    .as_str()
                    .ok_or_else(|| format!("colors.{k}: a \"#rrggbb\" string"))
                    .and_then(parse_color)?;
                for palette in [&mut s.light, &mut s.dark] {
                    *palette
                        .slot(k)
                        .ok_or_else(|| format!("colors.{k}: unknown colour"))? = col;
                }
            }
        }
        if let Some(menus) = v.get("menus").and_then(Value::as_object) {
            for (name, m) in menus {
                s.menus.push((name.clone(), parse_menu(name, m)?));
            }
        }
        Ok(s)
    }
}
