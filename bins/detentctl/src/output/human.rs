//! Human printers for each command group. Input: the same typed values `--json` prints.

use detent_config::listing::BindingRow;
use detent_ipc::settings::SettingInfo;
use detent_ipc::types::{ActionInfo, PluginInfo, RemapState, Status};
use serde_json::Value;

use super::{Style, table};
use crate::cmd::devices::{BatteryOut, DeviceRow};
use crate::cmd::status::VersionOut;

fn v(x: &Value) -> String {
    match x {
        Value::Null => "-".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => if *b { "on" } else { "off" }.into(),
        other => other.to_string(),
    }
}

fn duration(s: u64) -> String {
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m {}s", s / 60, s % 60),
        3600..86400 => format!("{}h {}m", s / 3600, (s % 3600) / 60),
        _ => format!("{}d {}h", s / 86400, (s % 86400) / 3600),
    }
}

pub fn status(st: &Style, s: &Status) {
    let line = |k: &str, val: String| println!("{}{val}", st.bold(&format!("{k:<10}")));
    // pid/uptime are 0 from a daemon older than these fields.
    let detail = if s.pid == 0 {
        format!("(v{})", s.version)
    } else {
        format!(
            "(v{}, pid {}, up {})",
            s.version,
            s.pid,
            duration(s.uptime_s)
        )
    };
    line(
        "detentd",
        format!("{} {}", st.green("running"), st.dim(&detail)),
    );
    line(
        "remapping",
        if s.remap_enabled {
            st.green("enabled")
        } else {
            st.yellow("paused (detentctl enable to resume)")
        },
    );
    line("config", s.config_path.clone());
    match &s.config_error {
        Some(e) => line(
            "",
            st.red(&format!("! rejected, previous config active: {e}")),
        ),
        None => line("", format!("ok ({} base bindings)", s.bindings)),
    }
    let profiles = if s.profiles.is_empty() {
        String::new()
    } else {
        st.dim(&format!("  (profiles: {})", s.profiles.join(", ")))
    };
    line("profile", format!("{}{profiles}", s.active_profile));
    if !s.context.is_empty() {
        let parts: Vec<String> = s.context.iter().map(|(k, v)| format!("{k}={v}")).collect();
        line("context", parts.join(" "));
    }
    if s.devices.is_empty() {
        line("device", st.yellow("none attached"));
    }
    for d in &s.devices {
        let drop = if d.dropped > 0 {
            st.yellow(&format!("  {} dropped", d.dropped))
        } else {
            String::new()
        };
        line("device", format!("{} [{}]{drop}", d.name, d.node));
    }
    for p in &s.plugins {
        line(
            "plugin",
            format!("{} {}", p.name, plugin_state(st, &p.state)),
        );
    }
}

fn plugin_state(st: &Style, s: &str) -> String {
    match s {
        "connected" => st.green(s),
        "starting" => st.yellow(s),
        _ => st.red(s),
    }
}

pub fn not_running(st: &Style, socket: &str, service: &str) {
    println!(
        "{}{} {}",
        st.bold(&format!("{:<10}", "detentd")),
        st.red("not running"),
        st.dim(&format!("(service: {service}; no daemon at {socket})"))
    );
}

pub fn remap(st: &Style, r: &RemapState) {
    let what = if r.enabled {
        st.green("remapping enabled")
    } else {
        st.yellow("remapping paused") + " (buttons act natively; settings and plugins keep running)"
    };
    let note = if r.changed { "" } else { " (unchanged)" };
    println!("{what}{note}");
}

pub fn version(st: &Style, v: &VersionOut) {
    println!(
        "{} {} {}",
        st.bold("detentctl"),
        v.detentctl.version,
        st.dim(&format!(
            "(commit {}, {}, {})",
            v.detentctl.commit, v.detentctl.target, v.detentctl.profile
        ))
    );
    match &v.detentd {
        Some(d) => println!(
            "{}   {} {}",
            st.bold("detentd"),
            d.version,
            st.dim(&format!("(running, pid {})", d.pid))
        ),
        None => println!("{}   {}", st.bold("detentd"), st.dim("not running")),
    }
}

pub fn devices(st: &Style, rows: &[DeviceRow]) {
    if rows.is_empty() {
        println!("no Logitech HID++ devices found");
        return;
    }
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|d| {
            vec![
                d.node.clone(),
                d.bus.clone(),
                format!("{:04X}:{:04X}", d.vendor, d.product),
                match &d.attached {
                    Some(id) => format!("attached ({id})"),
                    None => "-".into(),
                },
                d.name.clone(),
            ]
        })
        .collect();
    table(&["NODE", "BUS", "ID", "DAEMON", "NAME"], &body, st);
}

pub fn battery(st: &Style, b: &BatteryOut) {
    let pct = b.percent.map_or("?".into(), |p| format!("{p}%"));
    let pct = match b.percent {
        Some(p) if p <= 10 => st.red(&pct),
        Some(p) if p <= 25 => st.yellow(&pct),
        _ => st.green(&pct),
    };
    println!(
        "{}  {pct}  {}",
        b.device,
        st.dim(&format!("({}, {})", v(&b.level), v(&b.charging)))
    );
}

/// `hosts list`: one row per Easy-Switch slot, from the `hosts` section.
pub fn hosts(st: &Style, h: &Value) {
    let current = h["current"].as_u64();
    let slots = h["slots"].as_array().map_or(&[][..], Vec::as_slice);
    let body: Vec<Vec<String>> = slots
        .iter()
        .map(|s| {
            let n = s["slot"].as_u64().unwrap_or(0);
            let here = current == Some(n);
            let mark = if here { st.green("●") } else { " ".into() };
            let state = match (here, s["paired"].as_bool()) {
                (true, _) => st.green("this computer"),
                (false, Some(true)) => "paired".into(),
                (false, Some(false)) => st.dim("empty"),
                (false, None) => st.dim("?"),
            };
            let name = s["name"].as_str().filter(|n| !n.is_empty());
            let name = name.map_or_else(|| st.dim("-"), str::to_string);
            vec![format!("{mark} {}", n + 1), name, state]
        })
        .collect();
    table(&["HOST", "NAME", "STATE"], &body, st);
    if h["can_rename"].as_bool() == Some(false) && slots.iter().any(|s| !s["name"].is_null()) {
        println!("{}", st.dim("(this device doesn't accept new host names)"));
    }
}

pub fn settings(st: &Style, rows: &[SettingInfo]) {
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let range = if !r.values.is_empty() && r.kind == "choice" {
                r.values.iter().map(v).collect::<Vec<_>>().join(" | ")
            } else if let (Some(a), Some(b)) = (r.min, r.max) {
                match r.step {
                    Some(s) => format!("{a}..{b} step {s}"),
                    None => format!("{a}..{b}"),
                }
            } else {
                "on | off".into()
            };
            let value = if r.pending {
                format!("{} (desired {})", v(&r.value), v(&r.desired))
            } else {
                v(&r.value)
            };
            vec![r.path.clone(), value, range, r.doc.clone()]
        })
        .collect();
    if body.is_empty() {
        println!("this device exposes no settable settings");
        return;
    }
    table(&["SETTING", "VALUE", "RANGE", "DESCRIPTION"], &body, st);
}

pub fn bindings(st: &Style, rows: &[BindingRow]) {
    if rows.is_empty() {
        println!("no bindings");
        return;
    }
    let body: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let action = if r.params.is_empty() {
                format!("{}: {}", r.kind, r.action)
            } else {
                format!("{}: {} ({})", r.kind, r.action, r.params)
            };
            vec![
                r.profile.clone(),
                r.layer.clone().unwrap_or_else(|| "-".into()),
                r.trigger.clone(),
                action,
            ]
        })
        .collect();
    table(&["PROFILE", "LAYER", "TRIGGER", "ACTION"], &body, st);
}

pub fn actions(st: &Style, list: &[ActionInfo]) {
    let body: Vec<Vec<String>> = list
        .iter()
        .map(|a| {
            let doc = if a.continuous && !a.doc.contains("(continuous)") {
                format!("{} (continuous)", a.doc)
            } else {
                a.doc.clone()
            };
            vec![a.name.clone(), a.plugin.clone(), doc]
        })
        .collect();
    table(&["ACTION", "PLUGIN", "DESCRIPTION"], &body, st);
}

pub fn plugins(st: &Style, list: &[PluginInfo]) {
    if list.is_empty() {
        println!("no plugins running");
        return;
    }
    let body: Vec<Vec<String>> = list
        .iter()
        .map(|p| {
            vec![
                p.name.clone(),
                p.state.clone(),
                p.pid.map_or("-".into(), |x| x.to_string()),
                p.restarts.to_string(),
                p.dropped.to_string(),
                p.caps.as_ref().map_or("-".into(), |c| c.join(",")),
            ]
        })
        .collect();
    table(
        &["PLUGIN", "STATE", "PID", "RESTARTS", "DROPPED", "CAPS"],
        &body,
        st,
    );
}

pub fn plugin(st: &Style, p: &PluginInfo) {
    let line = |k: &str, val: String| println!("{}{val}", st.bold(&format!("{k:<10}")));
    line("plugin", p.name.clone());
    line("state", plugin_state(st, &p.state));
    line("version", p.version.clone().unwrap_or_else(|| "-".into()));
    line("pid", p.pid.map_or("-".into(), |x| x.to_string()));
    line("uptime", p.uptime_s.map_or("-".into(), duration));
    line("restarts", p.restarts.to_string());
    line("dropped", p.dropped.to_string());
    line("caps", p.caps.as_ref().map_or("-".into(), |c| c.join(", ")));
    if let Some(e) = &p.last_error {
        line("error", st.red(e));
    }
}

/// One event line: `topic  key=value ...`.
pub fn event(st: &Style, ev: &Value) {
    let topic = ev["topic"].as_str().unwrap_or("?");
    let rest: Vec<String> = ev
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k.as_str() != "topic")
        .map(|(k, x)| format!("{k}={}", v(x)))
        .collect();
    println!("{} {}", st.cyan(&format!("{topic:<8}")), rest.join(" "));
}

#[cfg(test)]
mod tests {
    #[test]
    fn durations() {
        assert_eq!(super::duration(5), "5s");
        assert_eq!(super::duration(125), "2m 5s");
        assert_eq!(super::duration(7260), "2h 1m");
        assert_eq!(super::duration(90000), "1d 1h");
    }
}
