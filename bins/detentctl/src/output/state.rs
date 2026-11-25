//! Human-readable rendering of device state (`detentctl get`). `--json` prints the raw
//! serialized `DeviceState` (or the subtree at `path`).

use anyhow::{Result, bail};
use serde_json::Value;

use crate::names;

/// `path` is dotted (`dpi`, `smartshift.mode`); it selects inside each section's
/// `reported` value unless it names a top-level key (`identity`, `features`, ...).
pub fn select<'a>(state: &'a Value, path: &str) -> Option<&'a Value> {
    let mut parts = path.split('.');
    let first = parts.next()?;
    let mut cur = state.get(first)?;
    if cur.get("reported").is_some() && cur.get("generation").is_some() {
        cur = cur.get("reported")?;
    }
    let mut deeper = false;
    for p in parts {
        cur = cur.get(p)?;
        deeper = true;
    }
    // `get dpi` → the number, not the whole section.
    if !deeper && let Some(v) = cur.get(first) {
        return Some(v);
    }
    Some(cur)
}

pub fn print(state: &Value, path: Option<&str>, json: bool) -> Result<()> {
    if let Some(p) = path {
        let Some(v) = select(state, p) else {
            bail!("no such path `{p}` (try `detentctl devices info --json` to see all paths)");
        };
        if json {
            println!("{}", serde_json::to_string_pretty(v)?);
        } else {
            match v {
                Value::String(s) => println!("{s}"),
                other => println!("{}", serde_json::to_string_pretty(other)?),
            }
        }
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(state)?);
        return Ok(());
    }
    summary(state);
    Ok(())
}

fn rep<'a>(state: &'a Value, section: &str) -> Option<&'a Value> {
    state.get(section)?.get("reported").filter(|v| !v.is_null())
}

fn des<'a>(state: &'a Value, section: &str) -> Option<&'a Value> {
    state.get(section)?.get("desired").filter(|v| !v.is_null())
}

fn s(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => "?".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => if *b { "on" } else { "off" }.into(),
        Some(other) => other.to_string(),
    }
}

fn pending(state: &Value, section: &str, field: &str) -> String {
    let (Some(d), Some(r)) = (des(state, section), rep(state, section)) else {
        return match des(state, section) {
            Some(d) => format!("  (desired {}, not applied)", s(d.get(field))),
            None => String::new(),
        };
    };
    if d.get(field) != r.get(field) {
        format!("  (desired {})", s(d.get(field)))
    } else {
        String::new()
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "predates the lint; split it when next changed (ADR 0008)"
)]
fn summary(st: &Value) {
    let id = &st["identity"];
    println!(
        "{}  [{}]  {}  link: {}",
        s(id.get("name")),
        s(id.get("node")),
        st["id"]["serial"].as_str().unwrap_or(""),
        s(st.get("link"))
    );
    if let Some(fw) = id.get("firmware").and_then(Value::as_array) {
        let app: Vec<String> = fw
            .iter()
            .filter(|f| f["kind"] == "main application")
            .map(|f| s(f.get("version")))
            .collect();
        if let Some(v) = app.first() {
            println!("  firmware      {v}");
        }
    }
    if let Some(b) = rep(st, "battery") {
        println!(
            "  battery       {}% ({}, {})",
            s(b.get("percent")),
            s(b.get("level")),
            s(b.get("charging"))
        );
    }
    if let Some(d) = rep(st, "dpi") {
        println!(
            "  dpi           {}{}   (range {}..{} step {}, default {})",
            s(d.get("dpi")),
            pending(st, "dpi", "dpi"),
            s(d.get("min")),
            s(d.get("max")),
            s(d.get("step")),
            s(d.get("default"))
        );
    }
    if let Some(m) = rep(st, "smartshift") {
        println!(
            "  smartshift    {}{}   auto-shift {}{}   torque {}{}",
            s(m.get("mode")),
            pending(st, "smartshift", "mode"),
            s(m.get("auto_threshold")),
            pending(st, "smartshift", "auto_threshold"),
            s(m.get("torque")),
            pending(st, "smartshift", "torque"),
        );
    }
    if let Some(w) = rep(st, "wheel") {
        println!(
            "  wheel         hires {}  invert {}{}  diverted {}  ratchet {}",
            s(w.get("hires")),
            s(w.get("invert")),
            pending(st, "wheel", "invert"),
            s(w.get("diverted")),
            s(w.get("ratchet"))
        );
    }
    if let Some(t) = rep(st, "thumbwheel") {
        println!(
            "  thumbwheel    invert {}{}  diverted {}",
            s(t.get("invert")),
            pending(st, "thumbwheel", "invert"),
            s(t.get("diverted"))
        );
    }
    if let Some(h) = rep(st, "haptics") {
        println!(
            "  haptics       {}{}  intensity {}{}  ({} waveforms)",
            s(h.get("enabled")),
            pending(st, "haptics", "enabled"),
            s(h.get("intensity")),
            pending(st, "haptics", "intensity"),
            h.get("waveforms")
                .and_then(Value::as_array)
                .map_or(0, Vec::len)
        );
    }
    if let Some(h) = rep(st, "hosts") {
        let cur = h.get("current").and_then(Value::as_u64).unwrap_or(0);
        let name = h
            .pointer(&format!("/slots/{cur}/name"))
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty())
            .map(|n| format!(" ({n})"))
            .unwrap_or_default();
        println!(
            "  easy-switch   host {}{name} of {}  (detentctl hosts)",
            cur + 1,
            s(h.get("count"))
        );
    }
    if let Some(c) = rep(st, "controls") {
        let list = |key: &str| -> String {
            let v: Vec<String> = c
                .get(key)
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_u64)
                        .map(|cid| names::control_label(cid as u16))
                        .collect()
                })
                .unwrap_or_default();
            if v.is_empty() {
                "none".into()
            } else {
                v.join(", ")
            }
        };
        println!("  diverted      {}", list("diverted"));
    }
    if let Some(e) = st.get("errors").and_then(Value::as_object) {
        for (k, v) in e {
            println!("  ! {k}: {}", s(Some(v)));
        }
    }
}
