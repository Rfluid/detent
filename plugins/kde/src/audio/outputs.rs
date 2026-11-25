//! Output devices for `kde.volume.targets` / `kde.volume.target`: the PipeWire sinks, listed
//! from `pw-dump` (JSON), switched with `wpctl set-default`.
//!
//! Switching moves the audio that is already playing: every playback stream gets
//! `target.object = -1` and `target.node = -1` in the `default` metadata, which tells
//! WirePlumber "follow the default output". Streams that only followed the default move by
//! themselves; this also moves streams an app or a mixer had pinned to one output.
//!
//! Verified on Fedora 44 / PipeWire 1.6 / WirePlumber 0.5 (2026-11-24), see
//! .agent/memory/facts/2026-11-24-pipewire-outputs.md:
//! - `node.nick` is the short name ("HDMI 1", "Speaker", "AB13X USB Audio"); the
//!   description of built-in outputs repeats the card name ("Core Ultra … HDMI / DisplayPort 1
//!   Output (Stereo)"), so three HDMI outputs look alike once shortened.
//! - Unplugged ports keep their sink: HDMI 2 / 3 without a monitor are still `Audio/Sink`
//!   nodes; their device's route says `available: "no"` (Plasma hides them, so do we).
//! - `channelVolumes` are cubic: `wpctl` shows 0.71 for 0.357921.
//! - `device.icon-name` is `audio-card-analog` for all of them and `device.form-factor` is
//!   unset, so the icon is chosen from the node name.
//! - A stream pinned to an output (`pw-cat --target`) moves to the default once its
//!   `target.object` metadata is -1; deleting the key sends it back to its pinned output.

use serde_json::{Value, json};

use super::{SINK, wpctl};

/// One output the user can switch to.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// PipeWire global id (what `wpctl set-default` takes).
    pub id: u32,
    pub name: String,
    /// PipeWire's full name (`node.description`), for the fan's hover label.
    pub description: String,
    /// Freedesktop icon name.
    pub icon: &'static str,
    pub percent: Option<u32>,
    pub muted: bool,
    /// The current default output.
    pub current: bool,
}

impl Output {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id.to_string(), "name": self.name, "description": self.description,
            "icon": self.icon,
            "percent": self.percent, "muted": self.muted, "current": self.current,
        })
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn objects(dump: &Value) -> &[Value] {
    dump.as_array().map_or(&[], Vec::as_slice)
}

fn is_type(o: &Value, t: &str) -> bool {
    o.get("type").and_then(Value::as_str) == Some(t)
}

/// The short name: `node.nick`, else `node.description`, else `node.name`.
pub fn display_name(props: &Value) -> String {
    ["node.nick", "node.description", "node.name"]
        .iter()
        .map(|k| str_of(props, k))
        .find(|s| !s.is_empty())
        .unwrap_or("Output")
        .to_string()
}

/// A theme icon for the output: its form factor if PipeWire knows it, else its name.
fn icon_for(props: &Value) -> &'static str {
    let lower = |k: &str| str_of(props, k).to_ascii_lowercase();
    match lower("device.form-factor").as_str() {
        "headphone" => return "audio-headphones",
        "headset" | "hands-free" | "handset" => return "audio-headset",
        "speaker" | "hifi" | "portable" | "car" => return "audio-speakers",
        "tv" => return "video-display",
        _ => {}
    }
    let name = lower("node.name");
    if name.contains("hdmi") || name.contains("displayport") {
        "video-display"
    } else if name.contains("headphone") || lower("device.api") == "bluez5" {
        "audio-headphones"
    } else if name.contains("speaker") {
        "audio-speakers"
    } else {
        "audio-card"
    }
}

/// The default sink's `node.name` (metadata `default`, key `default.audio.sink`).
fn default_sink(objs: &[Value]) -> Option<&str> {
    objs.iter()
        .filter(|o| is_type(o, "PipeWire:Interface:Metadata"))
        .filter(|o| o.pointer("/props/metadata.name").and_then(Value::as_str) == Some("default"))
        .filter_map(|o| o.get("metadata").and_then(Value::as_array))
        .flatten()
        .find(|m| str_of(m, "key") == "default.audio.sink")
        .and_then(|m| m.pointer("/value/name"))
        .and_then(Value::as_str)
}

/// Is the port behind this sink known to be unplugged? The device's active route for the
/// sink decides; without one, every route that could serve it must say `available: "no"`.
fn unplugged(objs: &[Value], props: &Value) -> bool {
    let num = |k: &str| {
        props.get(k).and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
    };
    let (Some(device), Some(profile_device)) = (num("device.id"), num("card.profile.device"))
    else {
        return false;
    };
    let Some(dev) = objs
        .iter()
        .find(|o| is_type(o, "PipeWire:Interface:Device") && o["id"].as_u64() == Some(device))
    else {
        return false;
    };
    let routes = |key: &str| {
        dev.pointer(&format!("/info/params/{key}"))
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter(|r| str_of(r, "direction") == "Output")
            .filter(move |r| {
                r.get("devices")
                    .and_then(Value::as_array)
                    .is_some_and(|d| d.iter().any(|x| x.as_u64() == Some(profile_device)))
            })
    };
    if let Some(active) = routes("Route").next() {
        return str_of(active, "available") == "no";
    }
    let mut any = false;
    for r in routes("EnumRoute") {
        any = true;
        if str_of(r, "available") != "no" {
            return false;
        }
    }
    any
}

/// `(percent, muted)` from the node's `Props` (volumes are cubic).
fn level(node: &Value) -> (Option<u32>, bool) {
    let props = node.pointer("/info/params/Props/0");
    let muted = props
        .and_then(|p| p.get("mute"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let vols: Vec<f64> = props
        .and_then(|p| p.get("channelVolumes"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let percent = (!vols.is_empty()).then(|| {
        let avg = vols.iter().sum::<f64>() / vols.len() as f64;
        (avg.max(0.).cbrt() * 100.).round() as u32
    });
    (percent, muted)
}

/// The outputs to offer, by id: every `Audio/Sink` except unplugged ones.
pub fn parse_outputs(dump: &Value) -> Vec<Output> {
    let objs = objects(dump);
    let current = default_sink(objs);
    let mut out: Vec<Output> = objs
        .iter()
        .filter(|o| is_type(o, "PipeWire:Interface:Node"))
        .filter_map(|o| {
            let props = o.pointer("/info/props")?;
            if str_of(props, "media.class") != "Audio/Sink" || unplugged(objs, props) {
                return None;
            }
            let (percent, muted) = level(o);
            Some(Output {
                id: u32::try_from(o.get("id")?.as_u64()?).ok()?,
                name: display_name(props),
                description: match str_of(props, "node.description") {
                    "" => display_name(props),
                    d => d.to_string(),
                },
                icon: icon_for(props),
                percent,
                muted,
                current: current.is_some_and(|c| c == str_of(props, "node.name")),
            })
        })
        .collect();
    out.sort_by_key(|o| o.id);
    out
}

/// Playback streams to move: apps' output streams, not the inner halves of loopbacks and
/// filter chains (`node.link-group`), not passive or monitor streams.
pub fn streams_to_move(dump: &Value) -> Vec<u32> {
    let truthy = |v: Option<&Value>| match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => s == "true",
        _ => false,
    };
    objects(dump)
        .iter()
        .filter(|o| is_type(o, "PipeWire:Interface:Node"))
        .filter_map(|o| {
            let p = o.pointer("/info/props")?;
            let keep = str_of(p, "media.class") == "Stream/Output/Audio"
                && p.get("node.link-group").is_none()
                && !truthy(p.get("node.passive"))
                && !truthy(p.get("stream.monitor"));
            if !keep {
                return None;
            }
            u32::try_from(o.get("id")?.as_u64()?).ok()
        })
        .collect()
}

fn pw_dump() -> Result<Value, String> {
    let out = std::process::Command::new("pw-dump")
        .output()
        .map_err(|e| format!("pw-dump: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "pw-dump: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("pw-dump: {e}"))
}

pub fn list() -> Result<Vec<Output>, String> {
    pw_dump().map(|d| parse_outputs(&d))
}

/// `wpctl inspect` → the node id on its first line (`id 76, type PipeWire:Interface:Node`).
pub fn parse_inspect_id(out: &str) -> Option<u32> {
    out.trim_start()
        .strip_prefix("id ")?
        .split(',')
        .next()?
        .trim()
        .parse()
        .ok()
}

/// Make `id` the default output and move what is playing to it. Returns once
/// `@DEFAULT_AUDIO_SINK@` resolves to it (at most ~300 ms), so the volume read next is the
/// new output's.
pub fn switch(id: u32) -> Result<(), String> {
    wpctl(&["set-default", &id.to_string()])?;
    for _ in 0..15 {
        let now = wpctl(&["inspect", SINK])
            .ok()
            .and_then(|o| parse_inspect_id(&o));
        if now == Some(id) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let dump = pw_dump()?;
    for stream in streams_to_move(&dump) {
        for key in ["target.object", "target.node"] {
            let res = std::process::Command::new("pw-metadata")
                .args(["--", &stream.to_string(), key, "-1"])
                .output();
            if let Err(e) = res {
                return Err(format!("pw-metadata: {e}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
