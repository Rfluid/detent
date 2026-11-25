//! Output volume through WirePlumber's `wpctl` (PipeWire's default sink), on its own thread:
//! each call spawns a short-lived process (~5 ms), so bursts from a spinning wheel are
//! merged (only the latest target is applied) and never block the plugin's bus loop.
//!
//! `wpctl` output verified on Fedora 44 / PipeWire 1.4 (2026-11-24):
//! `get-volume` → `Volume: 0.80` (` [MUTED]` appended when muted);
//! `inspect` → `  * node.description = "AB13X USB Audio Analog Stereo"`.
//! Spec: docs/plugins/kde.md ("Volume").
//!
//! Layout: this file holds the jobs, the worker and `wpctl`; `outputs` lists the output
//! devices and switches between them (`kde.volume.targets` / `.target`).

mod outputs;

use std::process::Command;
use std::sync::mpsc::{Receiver, Sender};

use detent_plugin::Sender as BusSender;
use serde_json::json;

const SINK: &str = "@DEFAULT_AUDIO_SINK@";
/// Plasma's "raise maximum volume" goes to 150%.
pub const MAX_LIMIT: u32 = 150;

/// Where a reply goes: the action to invoke and the caller's sequence number (echoed so
/// the caller can drop replies to requests it has since superseded).
#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub action: String,
    pub seq: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Job {
    Get {
        reply: Reply,
        max: u32,
    },
    Set {
        percent: u32,
        max: u32,
        unmute: bool,
        osd: bool,
        reply: Option<Reply>,
    },
    ToggleMute {
        reply: Option<Reply>,
        max: u32,
    },
    /// List the output devices (`kde.volume.targets`).
    Targets {
        reply: Reply,
    },
    /// Switch the default output and move what is playing (`kde.volume.target`); replies
    /// with the new output's level.
    Target {
        id: u32,
        reply: Option<Reply>,
        max: u32,
    },
    Stop,
}

/// `wpctl get-volume` → (percent, muted).
pub fn parse_volume(out: &str) -> Option<(u32, bool)> {
    let rest = out.trim().strip_prefix("Volume:")?.trim();
    let num = rest.split_whitespace().next()?;
    let v: f64 = num.parse().ok()?;
    Some((
        (v * 100.0).round().max(0.0) as u32,
        rest.contains("[MUTED]"),
    ))
}

/// `wpctl inspect` → the output's short name: `node.nick`, else `node.description` (the same
/// name the output list uses, see `outputs::display_name`).
pub fn parse_name(out: &str) -> Option<String> {
    let field = |key: &str| {
        out.lines().find_map(|l| {
            let l = l.trim().trim_start_matches('*').trim();
            let v = l.strip_prefix(key)?.trim().strip_prefix('=')?.trim();
            Some(v.trim_matches('"').to_string()).filter(|s| !s.is_empty())
        })
    };
    field("node.nick").or_else(|| field("node.description"))
}

/// Merge a burst: keep every `Get` / `ToggleMute` in order, but only the last `Set`
/// (placed where the last one was). A `Stop` ends the batch.
pub fn coalesce(batch: Vec<Job>) -> Vec<Job> {
    let last_set = batch.iter().rposition(|j| matches!(j, Job::Set { .. }));
    batch
        .into_iter()
        .enumerate()
        .filter(|(i, j)| !matches!(j, Job::Set { .. }) || Some(*i) == last_set)
        .map(|(_, j)| j)
        .collect()
}

fn wpctl(args: &[&str]) -> Result<String, String> {
    let out = Command::new("wpctl")
        .args(args)
        .output()
        .map_err(|e| format!("wpctl: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "wpctl {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn read() -> Result<(u32, bool), String> {
    let out = wpctl(&["get-volume", SINK])?;
    parse_volume(&out).ok_or_else(|| format!("unexpected wpctl output: {}", out.trim()))
}

struct Worker {
    bus: BusSender,
    /// For the OSD (`org.kde.osdService.volumeChanged`), owned by the D-Bus thread.
    osd: Sender<crate::dbus::Job>,
    sink: Option<String>,
}

impl Worker {
    fn reply(&self, r: &Reply, max: u32, level: Result<(u32, bool), String>) {
        let params = match level {
            Ok((percent, muted)) => json!({
                "source": "kde.volume", "seq": r.seq, "percent": percent, "muted": muted,
                "max": max, "label": self.sink.clone().unwrap_or_default(),
            }),
            Err(e) => json!({ "source": "kde.volume", "seq": r.seq, "error": e }),
        };
        let _ = self.bus.invoke(&r.action, &params.to_string());
    }

    /// The default output can change between calls (headphones, a switch): read its name.
    fn refresh_name(&mut self) {
        self.sink = wpctl(&["inspect", SINK]).ok().and_then(|o| parse_name(&o));
    }

    fn targets(&self, r: &Reply) {
        let params = match outputs::list() {
            Ok(list) => json!({
                "source": "kde.volume", "seq": r.seq,
                "targets": list.iter().map(outputs::Output::to_json).collect::<Vec<_>>(),
            }),
            Err(e) => {
                eprintln!("detent-kde: volume: {e}");
                json!({ "source": "kde.volume", "seq": r.seq, "error": e })
            }
        };
        let _ = self.bus.invoke(&r.action, &params.to_string());
    }

    fn run(&mut self, job: Job) {
        match job {
            Job::Targets { reply } => self.targets(&reply),
            Job::Target { id, reply, max } => {
                let level = outputs::switch(id).and_then(|()| {
                    self.refresh_name();
                    read()
                });
                if let Err(e) = &level {
                    eprintln!("detent-kde: volume: {e}");
                }
                if let Some(r) = reply {
                    self.reply(&r, max, level);
                }
            }
            Job::Get { reply, max } => {
                self.refresh_name();
                let level = read();
                if let Err(e) = &level {
                    eprintln!("detent-kde: volume: {e}");
                }
                self.reply(&reply, max, level);
            }
            Job::Set {
                percent,
                max,
                unmute,
                osd,
                reply,
            } => {
                let max = max.clamp(1, MAX_LIMIT);
                let p = percent.min(max);
                let value = format!("{:.2}", f64::from(p) / 100.0);
                let limit = format!("{:.2}", f64::from(max) / 100.0);
                let mut res = wpctl(&["set-volume", "-l", &limit, SINK, &value]).map(|_| ());
                if res.is_ok() && unmute {
                    res = wpctl(&["set-mute", SINK, "0"]).map(|_| ());
                }
                let level = res.and_then(|()| read());
                match &level {
                    Ok((now, muted)) if osd => {
                        let shown = if *muted { 0 } else { *now };
                        let _ = self.osd.send(crate::dbus::Job::VolumeOsd {
                            percent: shown as i32,
                            max: max as i32,
                        });
                    }
                    Err(e) => eprintln!("detent-kde: volume: {e}"),
                    _ => {}
                }
                if let Some(r) = reply {
                    self.reply(&r, max, level);
                }
            }
            Job::ToggleMute { reply, max } => {
                let level = wpctl(&["set-mute", SINK, "toggle"]).and_then(|_| read());
                if let Err(e) = &level {
                    eprintln!("detent-kde: volume: {e}");
                }
                if let Some(r) = reply {
                    self.reply(&r, max, level);
                }
            }
            Job::Stop => {}
        }
    }
}

fn run(bus: BusSender, osd: Sender<crate::dbus::Job>, rx: Receiver<Job>) {
    let mut w = Worker {
        bus,
        osd,
        sink: None,
    };
    while let Ok(first) = rx.recv() {
        let mut batch = vec![first];
        batch.extend(rx.try_iter());
        let stop = batch.iter().position(|j| matches!(j, Job::Stop));
        if let Some(i) = stop {
            batch.truncate(i);
        }
        for job in coalesce(batch) {
            w.run(job);
        }
        if stop.is_some() {
            return;
        }
    }
}

pub fn spawn(bus: BusSender, osd: Sender<crate::dbus::Job>) -> Option<Sender<Job>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("kde-audio".into())
        .spawn(move || run(bus, osd, rx))
        .ok()
        .map(|_| tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_wpctl() {
        assert_eq!(parse_volume("Volume: 0.80\n"), Some((80, false)));
        assert_eq!(parse_volume("Volume: 0.42 [MUTED]\n"), Some((42, true)));
        assert_eq!(parse_volume("Volume: 1.25"), Some((125, false)));
        assert_eq!(parse_volume("nope"), None);
        let inspect = "id 56, type PipeWire:Interface:Node\n    node.name = \"alsa_output.x\"\n  * node.description = \"AB13X USB Audio Analog Stereo\"\n  * node.nick = \"AB13X USB Audio\"\n";
        assert_eq!(parse_name(inspect).as_deref(), Some("AB13X USB Audio"));
        assert_eq!(
            parse_name("  * node.description = \"Desc\"").as_deref(),
            Some("Desc")
        );
    }

    #[test]
    fn bursts_keep_only_the_last_set() {
        let set = |p| Job::Set {
            percent: p,
            max: 100,
            unmute: false,
            osd: false,
            reply: None,
        };
        let get = Job::Get {
            reply: Reply {
                action: "radial.level".into(),
                seq: 1,
            },
            max: 100,
        };
        let out = coalesce(vec![set(10), get.clone(), set(12), set(14)]);
        assert_eq!(out, vec![get, set(14)]);
    }
}
