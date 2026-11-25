//! `detent-radial --preview`: run the real ring (same state machine, window and input
//! handling) from a config file, without the daemon: no lease, no haptics; a selection
//! prints what it would run instead of running it. For checking a customised ring and for
//! screenshots.
//!
//! ```text
//! detent-radial --preview [CONFIG] [--menu NAME] [--app APP] [--at X,Y[,OUTPUT[,W,H]]]
//!               [--folder N] [--highlight N [--pin [--fan]]] [--seconds S] [--backdrop "#rrggbb"]
//!               [--theme system|light|dark] [--warm-up]
//! ```
//! `--warm-up` starts like the plugin (1×1 warm-up surface first, ring 2.5 s later), to
//! reproduce problems on its window path.
//! Adjustable items (`adjust = "kde.volume"`) get a stand-in provider with three outputs
//! (Speaker 42%, Headphones 25%, HDMI 1 80%), so the dial, wheel, middle click and the
//! output fan work without detent-kde. `--pin` pins the gauge of the `--highlight`ed item;
//! `--fan` then opens its fan.
//! CONFIG defaults to `~/.config/detent/config.toml`. `--at` stands in for the desktop's
//! cursor position (logical px on OUTPUT; W,H = the output's logical size, as `kde.cursor`
//! reports it); without it the ring appears where the compositor reports the pointer. Hover, click, Esc, Tab and Enter work as in real use.

use std::process::ExitCode;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedSender, unbounded};
use gpui::{App, QuitMode};

use crate::app::{Backend, Msg, Ui, drive};
use crate::icon::expand_home;
use crate::menu::{Settings, Theme, parse_color};
use crate::state::{At, Input, Key, Radial, Target};

pub struct Opts {
    path: String,
    menu: String,
    app: Option<String>,
    at: Option<At>,
    folder: Option<usize>,
    highlight: Option<usize>,
    seconds: u64,
    backdrop: Option<u32>,
    theme: Option<Theme>,
    /// After opening: pin the adjustable item's gauge (screenshots).
    pin: bool,
    /// After pinning: open the output fan (screenshots).
    fan: bool,
    /// Warm up the GPU like the plugin does before opening (reproduces its window path).
    warm_up: bool,
}

fn parse_at(s: &str) -> Result<At, String> {
    let mut parts = s.split(',');
    let num = |p: Option<&str>| {
        p.and_then(|v| v.trim().parse::<f32>().ok())
            .ok_or_else(|| format!("--at {s}: use X,Y or X,Y,OUTPUT or X,Y,OUTPUT,W,H"))
    };
    let x = num(parts.next())?;
    let y = num(parts.next())?;
    let output = parts.next().unwrap_or_default().trim().to_string();
    let size = match (parts.next(), parts.next()) {
        (None, _) => None,
        (w, h) => Some((num(w)?, num(h)?)),
    };
    if parts.next().is_some() {
        return Err(format!("--at {s}: too many values"));
    }
    Ok(At { x, y, output, size })
}

impl Opts {
    /// `Some` when `--preview` is among the arguments.
    pub fn from_args(args: &[String]) -> Result<Option<Self>, String> {
        let Some(at) = args.iter().position(|a| a == "--preview") else {
            return Ok(None);
        };
        let mut o = Self {
            path: "~/.config/detent/config.toml".into(),
            menu: "main".into(),
            app: None,
            at: None,
            folder: None,
            highlight: None,
            seconds: 30,
            backdrop: None,
            theme: None,
            pin: false,
            fan: false,
            warm_up: false,
        };
        let mut it = args[at + 1..].iter();
        while let Some(a) = it.next() {
            let mut val = |name: &str| {
                it.next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a value"))
            };
            let num = |s: String, name: &str| {
                s.parse::<u64>()
                    .map_err(|_| format!("{name}: not a number: {s}"))
            };
            match a.as_str() {
                "--menu" => o.menu = val("--menu")?,
                "--app" => o.app = Some(val("--app")?),
                "--at" => o.at = Some(parse_at(&val("--at")?)?),
                "--folder" => o.folder = Some(num(val("--folder")?, "--folder")? as usize),
                "--highlight" => {
                    o.highlight = Some(num(val("--highlight")?, "--highlight")? as usize)
                }
                "--seconds" => o.seconds = num(val("--seconds")?, "--seconds")?.clamp(1, 600),
                "--backdrop" => o.backdrop = Some(parse_color(&val("--backdrop")?)?),
                "--theme" => {
                    o.theme = Some(match val("--theme")?.as_str() {
                        "system" => Theme::System,
                        "light" => Theme::Light,
                        "dark" => Theme::Dark,
                        t => return Err(format!("--theme {t}: use system, light or dark")),
                    })
                }
                "--pin" => o.pin = true,
                "--fan" => o.fan = true,
                "--warm-up" => o.warm_up = true,
                p if !p.starts_with("--") => o.path = p.to_string(),
                other => return Err(format!("unknown option {other}")),
            }
        }
        Ok(Some(o))
    }
}

fn load(path: &str) -> Result<Settings, String> {
    let path = expand_home(path);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
    let doc: toml::Value = toml::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
    let radial = doc
        .get("plugins")
        .and_then(|p| p.get("radial"))
        .ok_or_else(|| format!("{path}: no [plugins.radial]"))?;
    let json = serde_json::to_string(radial).map_err(|e| e.to_string())?;
    Settings::parse(&json)
}

/// A stand-in output: short and full name, theme icon, (percent, muted).
struct FakeOutput {
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    level: (u32, bool),
}

struct Preview {
    at: Option<At>,
    tx: UnboundedSender<Msg>,
    /// Stand-in for an adjustable item's provider (`kde.volume`) and its outputs.
    outputs: Vec<FakeOutput>,
    current: usize,
}

impl Preview {
    fn send(&self, input: Option<Input>) {
        if let Some(i) = input {
            let _ = self.tx.unbounded_send(Msg::Input(i));
        }
    }

    fn targets_reply(&self, source: &str, seq: &serde_json::Value) {
        let targets: Vec<_> = self
            .outputs
            .iter()
            .enumerate()
            .map(|(k, o)| {
                serde_json::json!({ "id": k.to_string(), "name": o.name,
                    "description": o.description, "icon": o.icon,
                    "percent": o.level.0, "muted": o.level.1, "current": k == self.current })
            })
            .collect();
        let reply = serde_json::json!({ "source": source, "seq": seq, "targets": targets });
        self.send(crate::replies::targets_input(&reply));
    }

    /// Answer `<source>.get` / `.set` / `.mute` / `.targets` / `.target` like detent-kde.
    fn fake_provider(&mut self, action: &str, params_json: &str) -> bool {
        let Some((source, op)) = action.rsplit_once('.') else {
            return false;
        };
        let p: serde_json::Value = serde_json::from_str(params_json).unwrap_or_default();
        let num = |k: &str| p.get(k).and_then(serde_json::Value::as_u64);
        let max = num("max").unwrap_or(100);
        let seq = p.get("seq").cloned().unwrap_or_default();
        let cur = self.current;
        match op {
            "get" => {}
            "set" => {
                let level = &mut self.outputs[cur].level;
                level.0 = num("percent").unwrap_or(0).min(max) as u32;
                if p.get("unmute").and_then(serde_json::Value::as_bool) == Some(true) {
                    level.1 = false;
                }
            }
            "mute" => self.outputs[cur].level.1 = !self.outputs[cur].level.1,
            "target" => {
                let id = p.get("id").and_then(|v| v.as_str()?.parse::<usize>().ok());
                if let Some(k) = id.filter(|k| *k < self.outputs.len()) {
                    self.current = k;
                }
            }
            "targets" => {
                self.targets_reply(source, &seq);
                return true;
            }
            _ => return false,
        }
        let o = &self.outputs[self.current];
        let reply = serde_json::json!({
            "source": source, "seq": seq,
            "percent": o.level.0, "muted": o.level.1, "max": max, "label": o.name,
        });
        self.send(crate::replies::level_input(&reply));
        true
    }
}

impl Backend for Preview {
    fn lease(&mut self, _control: &str) {}
    fn release(&mut self) {}
    fn locate(&mut self, tx: &UnboundedSender<Msg>) {
        // Stand-in for kde.cursor; without --at, the timeout shows it at the pointer.
        if let Some(at) = self.at.clone() {
            let _ = tx.unbounded_send(Msg::Input(Input::Cursor(at)));
        }
    }
    fn haptic(&mut self, _waveform: u8) {}
    fn invoke(&mut self, action: &str, params_json: &str) {
        println!("would run: {action} {params_json}");
        if params_json.contains(crate::state::LEVEL_REPLY)
            || params_json.contains(crate::state::TARGETS_REPLY)
        {
            self.fake_provider(action, params_json);
        }
    }
    fn closed(&mut self, cx: &mut App) {
        // Let a selection's steps print, then quit.
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

/// The stand-in provider's outputs: Speaker 42%, Headphones 25%, HDMI 1 80%.
fn fake_outputs() -> Vec<FakeOutput> {
    let fake = |name, description, icon, percent| FakeOutput {
        name,
        description,
        icon,
        level: (percent, false),
    };
    vec![
        fake(
            "Speaker",
            "Built-in Audio Analog Stereo Speaker",
            "audio-speakers",
            42,
        ),
        fake(
            "Headphones",
            "Bluetooth Headphones (A2DP Sink)",
            "audio-headphones",
            25,
        ),
        fake(
            "HDMI 1",
            "Built-in Audio HDMI / DisplayPort 1 Output (Stereo)",
            "video-display",
            80,
        ),
    ]
}

pub fn run(o: Opts) -> ExitCode {
    let settings = match load(&o.path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("detent-radial --preview: {e}");
            return ExitCode::FAILURE;
        }
    };
    if settings.menu(&o.menu).is_none() {
        eprintln!("detent-radial --preview: no menu `{}`", o.menu);
        return ExitCode::FAILURE;
    }
    let mut settings = settings;
    if let Some(t) = o.theme {
        settings.theme = t;
    }
    let (tx, rx) = unbounded();
    let preview = Preview {
        at: o.at,
        tx: tx.clone(),
        outputs: fake_outputs(),
        current: 0,
    };
    let mut ui = Ui::new(Radial::new(settings), preview, tx.clone());
    ui.backdrop = o.backdrop;
    let mut first = Vec::new();
    if let Some(app) = o.app {
        first.push(Input::App(app));
    }
    first.push(Input::Open {
        menu: o.menu,
        control: None,
    });
    // After the ring is up: enter a folder / highlight an item (screenshots).
    let mut later = Vec::new();
    if let Some(f) = o.folder {
        later.push(Input::Click {
            left: true,
            target: Target::Item(f),
        });
    }
    if let Some(h) = o.highlight {
        later.push(Input::Hover(Some(h)));
        if o.pin {
            later.push(Input::Click {
                left: true,
                target: Target::Item(h),
            });
            if o.fan {
                later.push(Input::Key(Key::Tab));
            }
        }
    }
    let seconds = o.seconds;
    gpui_platform::application()
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx: &mut App| {
            // `--warm-up`: start like the plugin does (warm-up surface first, so the GPU
            // context is shared with it) and open once that surface is gone.
            let warm = o.warm_up;
            if warm {
                crate::view::warm_up(cx);
            }
            drive(ui, rx, cx);
            cx.spawn(async move |cx| {
                if warm {
                    cx.background_executor()
                        .timer(Duration::from_millis(2500))
                        .await;
                }
                for i in first {
                    let _ = tx.unbounded_send(Msg::Input(i));
                }
                cx.background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
                for i in later {
                    let _ = tx.unbounded_send(Msg::Input(i));
                }
                cx.background_executor()
                    .timer(Duration::from_secs(seconds))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        });
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_option() {
        assert_eq!(
            parse_at("450,320,DP-1"),
            Ok(At {
                x: 450.,
                y: 320.,
                output: "DP-1".into(),
                size: None,
            })
        );
        assert_eq!(
            parse_at("900,560,eDP-1,1800,1125").map(|a| a.size),
            Ok(Some((1800., 1125.)))
        );
        assert!(parse_at("1,2,eDP-1,1800").is_err());
        assert!(parse_at("1,2,eDP-1,1800,1125,9").is_err());
        assert_eq!(parse_at("1.5, 2").map(|a| (a.x, a.y)), Ok((1.5, 2.)));
        assert!(parse_at("x,2").is_err());
    }
}
