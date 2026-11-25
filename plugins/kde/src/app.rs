//! The plugin proper (Linux: session D-Bus, KWin, PipeWire): actions, context and `main`.
//! `main.rs` declares the modules and picks this or the unsupported-OS exit (ADR 0010).

use std::process::ExitCode;
use std::sync::mpsc::Sender;

use detent_plugin::{ActionCall, Capability, Context, Manifest, Plugin};
use serde_json::Value;

use crate::{actions, audio, dbus, terminal};

struct Kde {
    jobs: Option<Sender<dbus::Job>>,
    audio: Option<Sender<audio::Job>>,
    context: bool,
    /// `osd = true`: show Plasma's OSD for volume changes (callers can override per call).
    osd: bool,
}

impl Default for Kde {
    fn default() -> Self {
        Self {
            jobs: None,
            audio: None,
            context: true,
            osd: true,
        }
    }
}

impl Kde {
    fn job(&self, j: dbus::Job) {
        if let Some(tx) = &self.jobs {
            let _ = tx.send(j);
        }
    }

    fn audio(&self, j: audio::Job) {
        if let Some(tx) = &self.audio {
            let _ = tx.send(j);
        }
    }

    /// The PipeWire volume actions (docs/plugins/kde.md, "Volume"). `false` = not one of
    /// them (`volume.mute` without a reply is the kmix shortcut).
    fn volume(&self, action: &str, params: &Value) -> bool {
        let needs_reply = |a: &str| {
            eprintln!("detent-kde: kde.{a} needs {{ reply = \"plugin.action\" }}");
        };
        match action {
            "volume.get" => match reply_of(params) {
                Some(reply) => self.audio(audio::Job::Get {
                    reply,
                    max: max_of(params),
                }),
                None => needs_reply(action),
            },
            "volume.set" => match params.get("percent").and_then(Value::as_f64) {
                Some(p) => self.audio(audio::Job::Set {
                    percent: p.round().max(0.0) as u32,
                    max: max_of(params),
                    unmute: params.get("unmute").and_then(Value::as_bool) == Some(true),
                    osd: params
                        .get("osd")
                        .and_then(Value::as_bool)
                        .unwrap_or(self.osd),
                    reply: reply_of(params),
                }),
                None => eprintln!("detent-kde: kde.volume.set needs {{ percent }}"),
            },
            // With a reply: toggle through PipeWire and report the new state (no OSD, the
            // caller shows it). Without: the kmix shortcut, with Plasma's OSD.
            "volume.mute" if params.get("reply").is_some() => {
                self.audio(audio::Job::ToggleMute {
                    reply: reply_of(params),
                    max: max_of(params),
                });
            }
            "volume.targets" => match reply_of(params) {
                Some(reply) => self.audio(audio::Job::Targets { reply }),
                None => needs_reply(action),
            },
            "volume.target" => match target_id(params) {
                Some(id) => self.audio(audio::Job::Target {
                    id,
                    reply: reply_of(params),
                    max: max_of(params),
                }),
                None => eprintln!("detent-kde: kde.volume.target needs {{ id }}"),
            },
            _ => return false,
        }
        true
    }
}

/// The output id from `volume.targets` (a string; a number works too).
fn target_id(params: &Value) -> Option<u32> {
    let v = params.get("id")?;
    let n = v
        .as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))?;
    u32::try_from(n).ok()
}

/// `{ reply, seq }` from an action's params, if `reply` is a valid action name.
fn reply_of(params: &Value) -> Option<audio::Reply> {
    let action = params.get("reply").and_then(Value::as_str)?;
    if !actions::valid_reply(action) {
        eprintln!("detent-kde: invalid reply action `{action}`");
        return None;
    }
    Some(audio::Reply {
        action: action.to_string(),
        seq: params.get("seq").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn max_of(params: &Value) -> u32 {
    params
        .get("max")
        .and_then(Value::as_u64)
        .map_or(100, |m| m.clamp(1, u64::from(audio::MAX_LIMIT)) as u32)
}

impl Plugin for Kde {
    fn manifest(&self) -> Manifest {
        detent_plugin::manifest(
            "kde",
            actions::DECLS,
            &[
                Capability::EventsSubscribe,
                Capability::ContextPublish,
                Capability::DeviceHaptics,
                Capability::ActionsInvoke,
            ],
        )
    }

    fn on_config(&mut self, ctx: &mut Context, settings_json: &str) {
        let s: Value = serde_json::from_str(settings_json).unwrap_or(Value::Null);
        let context = s.get("context").and_then(Value::as_bool).unwrap_or(true);
        self.osd = s.get("osd").and_then(Value::as_bool).unwrap_or(true);
        if self.jobs.is_none() {
            if let Some(bus) = ctx.sender() {
                self.jobs = dbus::spawn(bus.clone(), context);
                if let Some(osd) = &self.jobs {
                    self.audio = audio::spawn(bus, osd.clone());
                }
            }
        } else if context != self.context {
            self.job(dbus::Job::SetContextEnabled(context));
        }
        self.context = context;
    }

    fn on_action(&mut self, _ctx: &mut Context, call: &ActionCall) {
        let params: Value = serde_json::from_str(&call.params_json).unwrap_or(Value::Null);
        if let Some(target) = actions::continuous_for(&call.action) {
            // Bound to a discrete trigger (delta 0) → one step up.
            let mut steps = if call.delta == 0 {
                1
            } else {
                i32::from(call.delta)
            };
            if params.get("invert").and_then(Value::as_bool) == Some(true) {
                steps = -steps;
            }
            self.job(dbus::Job::Steps { target, steps });
            return;
        }
        if call.action == "cursor" {
            match params.get("reply").and_then(Value::as_str) {
                Some(r) if actions::valid_reply(r) => self.job(dbus::Job::Cursor {
                    reply: r.to_string(),
                }),
                _ => eprintln!("detent-kde: kde.cursor needs {{ reply = \"plugin.action\" }}"),
            }
            return;
        }
        if self.volume(&call.action, &params) {
            return;
        }
        if call.action == "terminal" {
            terminal::launch();
            return;
        }
        if call.action == "shortcut" {
            match (
                params.get("component").and_then(Value::as_str),
                params.get("name").and_then(Value::as_str),
            ) {
                (Some(c), Some(n)) => self.job(dbus::Job::Shortcut {
                    component: c.to_string(),
                    name: n.to_string(),
                }),
                _ => eprintln!("detent-kde: kde.shortcut needs {{ component, name }}"),
            }
            return;
        }
        match actions::shortcut_for(&call.action) {
            Some((component, name)) => self.job(dbus::Job::Shortcut {
                component: component.to_string(),
                name: name.to_string(),
            }),
            None => eprintln!("detent-kde: unknown action kde.{}", call.action),
        }
    }

    fn on_shutdown(&mut self) {
        self.audio(audio::Job::Stop);
        self.job(dbus::Job::Stop);
        // Give the D-Bus thread a moment to unload the KWin script.
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

pub fn main() -> ExitCode {
    match detent_plugin::run(Kde::default()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("detent-kde: {e}");
            ExitCode::FAILURE
        }
    }
}
