//! Config → engine tables (docs/architecture/bindings.md). Runs on the control plane at
//! load/reload; every error here is reported to the user and the previous config stays.

use std::collections::BTreeMap;

use detent_engine::action::{ActionId, CmdId, DeviceCmd, KeyChord, ParamsRef, PluginSlot};
use detent_engine::out::code;
use detent_engine::{Action, Compiled, ControlSlot, Edge, LayerId, ModMask, Tables, slot};
use detent_hidpp_features::cid;

use crate::schema::{Bind, Config, Do, GestureHaptic, GestureSettings, Layer};
use crate::trigger::{EdgeSpec, Trigger};
use crate::{ConfigError, keys};

/// A named action (plugin or built-in) with its parameters, referenced by index from
/// `Action::Plugin`.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedAction {
    pub name: String,
    pub params_json: String,
    /// The trigger text, for `ActionInvoked.trigger`.
    pub trigger: String,
}

#[derive(Debug, Clone)]
pub struct ProfileMatch {
    pub name: String,
    pub matches: BTreeMap<String, String>,
}

/// Everything the daemon needs from a config file.
#[derive(Debug, Clone)]
pub struct CompiledConfig {
    pub engine: Compiled,
    pub exec: Vec<String>,
    pub actions: Vec<NamedAction>,
    /// `profiles[i]` ↔ `engine.profiles[i + 1]`, in file order.
    pub profiles: Vec<ProfileMatch>,
    pub source: Config,
}

impl CompiledConfig {
    /// First profile whose `match` is satisfied by `context` (index into engine profiles).
    pub fn profile_for(&self, context: &BTreeMap<String, String>) -> u8 {
        self.profiles
            .iter()
            .position(|p| {
                p.matches
                    .iter()
                    .all(|(k, v)| context.get(k).is_some_and(|c| c.eq_ignore_ascii_case(v)))
            })
            .map_or(0, |i| (i + 1).min(u8::MAX as usize) as u8)
    }
}

fn err(when: &str, msg: impl Into<String>) -> ConfigError {
    ConfigError::Compile {
        when: when.to_string(),
        msg: msg.into(),
    }
}

/// Where a control name lands.
#[derive(Clone, Copy)]
enum Ctl {
    /// Diverted HID++ control.
    Cid(ControlSlot, u16),
    /// Read from evdev during a grab (left, right).
    Evdev(ControlSlot),
    Wheel,
    Thumbwheel,
    Swipe,
}

fn control(name: &str, extras: &mut Vec<u16>, when: &str) -> Result<Ctl, ConfigError> {
    Ok(match name {
        "left" => Ctl::Evdev(slot::LEFT),
        "right" => Ctl::Evdev(slot::RIGHT),
        "wheel" => Ctl::Wheel,
        "thumbwheel" => Ctl::Thumbwheel,
        "swipe" => Ctl::Swipe,
        other => {
            let c = cid::from_config_name(other).ok_or_else(|| {
                err(
                    when,
                    format!(
                        "unknown control `{other}` (known: left, right, middle, back, forward, \
                         gesture, smartshift, thumb, wheel, thumbwheel, swipe)"
                    ),
                )
            })?;
            let s = match c {
                cid::MIDDLE => slot::MIDDLE,
                cid::BACK => slot::BACK,
                cid::FORWARD => slot::FORWARD,
                cid::GESTURE => slot::GESTURE,
                cid::SMART_SHIFT => slot::SMARTSHIFT,
                cid::HAPTIC_PANEL => slot::THUMB,
                cid::LEFT | cid::RIGHT => {
                    return Err(err(when, "left/right are not divertable"));
                }
                other => {
                    let i = match extras.iter().position(|&e| e == other) {
                        Some(i) => i,
                        None => {
                            extras.push(other);
                            extras.len() - 1
                        }
                    };
                    let n = slot::FIRST_EXTRA as usize + i;
                    if n >= slot::COUNT {
                        return Err(err(when, "too many extra controls"));
                    }
                    ControlSlot(n as u8)
                }
            };
            Ctl::Cid(s, c)
        }
    })
}

fn native_button(c: u16) -> u16 {
    match c {
        cid::MIDDLE => code::BTN_MIDDLE,
        cid::BACK => code::BTN_SIDE,
        cid::FORWARD => code::BTN_EXTRA,
        _ => 0,
    }
}

struct Builder<'a> {
    cfg: &'a Config,
    out: Compiled,
    modifiers: Vec<u16>, // modifier index → CID
    layers: BTreeMap<u8, LayerId>,
    extras: Vec<u16>,
    exec: Vec<String>,
    actions: Vec<NamedAction>,
}

/// One binding after parsing.
struct Parsed {
    when: String,
    mask: ModMask,
    ctl: Ctl,
    edges: Vec<Edge>,
    hold_ms: Option<u16>,
    action: Action,
}

impl<'a> Builder<'a> {
    fn modifier(&mut self, name: &str, when: &str) -> Result<u8, ConfigError> {
        let c = cid::from_config_name(name)
            .filter(|c| *c != cid::LEFT && *c != cid::RIGHT)
            .ok_or_else(|| {
                err(
                    when,
                    format!(
                        "`{name}` can't be a modifier: modifiers must be divertable buttons \
                         (gesture, thumb, middle, back, forward, smartshift)"
                    ),
                )
            })?;
        if let Some(i) = self.modifiers.iter().position(|&m| m == c) {
            return Ok(i as u8);
        }
        if self.modifiers.len() >= detent_engine::modifiers::MAX_MODIFIERS {
            return Err(err(when, "at most 8 different modifiers"));
        }
        self.modifiers.push(c);
        Ok((self.modifiers.len() - 1) as u8)
    }

    fn layer_for(&mut self, mask: ModMask) -> LayerId {
        if mask.is_empty() {
            return LayerId::BASE;
        }
        let next = LayerId(self.layers.len() as u8 + 1);
        *self.layers.entry(mask.0).or_insert(next)
    }

    fn action(&mut self, d: &Do, when: &str, continuous: bool) -> Result<Action, ConfigError> {
        if let Some(k) = &d.keys {
            let codes = keys::parse_chord(k).map_err(|m| err(when, m))?;
            return KeyChord::new(&codes)
                .map(Action::Keys)
                .ok_or_else(|| err(when, "1 to 6 keys"));
        }
        if let Some(b) = &d.button {
            return keys::button_code(b)
                .map(Action::Button)
                .ok_or_else(|| err(when, format!("unknown button `{b}`")));
        }
        if let Some(e) = &d.exec {
            if e.trim().is_empty() {
                return Err(err(when, "empty exec"));
            }
            let id = match self.exec.iter().position(|x| x == e) {
                Some(i) => i,
                None => {
                    self.exec.push(e.clone());
                    self.exec.len() - 1
                }
            };
            return Ok(Action::Exec(CmdId(id as u16)));
        }
        if let Some(dev) = &d.device {
            let cmd = match dev.as_str() {
                "cycle-dpi" => DeviceCmd::CycleDpi,
                "toggle-smartshift" => DeviceCmd::ToggleSmartShift,
                "haptic" => {
                    let w = d
                        .params
                        .get("waveform")
                        .and_then(|v| v.as_str())
                        .unwrap_or("damp-state-change");
                    let id = detent_hidpp_features::haptics::waveform::by_name(w)
                        .ok_or_else(|| err(when, format!("unknown waveform `{w}`")))?;
                    DeviceCmd::HapticPulse {
                        waveform: id,
                        intensity: 0,
                    }
                }
                "dpi" => {
                    let v = d
                        .params
                        .get("value")
                        .and_then(|v| v.as_integer())
                        .ok_or_else(|| err(when, "`device = \"dpi\"` needs `value = <dpi>`"))?;
                    DeviceCmd::SetDpi(u16::try_from(v).map_err(|_| err(when, "dpi out of range"))?)
                }
                other => {
                    return Err(err(
                        when,
                        format!(
                            "unknown device action `{other}` (cycle-dpi, toggle-smartshift, \
                             haptic, dpi)"
                        ),
                    ));
                }
            };
            return Ok(Action::Device(cmd));
        }
        if let Some(name) = &d.action {
            if !name.contains('.') {
                return Err(err(
                    when,
                    format!("action `{name}` must be `<plugin>.<action>`, e.g. `kde.overview`"),
                ));
            }
            let params_json = serde_json::to_string(&d.params).unwrap_or_else(|_| "{}".into());
            let _ = continuous;
            self.actions.push(NamedAction {
                name: name.clone(),
                params_json,
                trigger: when.to_string(),
            });
            let id = (self.actions.len() - 1) as u16;
            return Ok(Action::Plugin {
                plugin: PluginSlot(0),
                action: ActionId(id),
                params: ParamsRef(id),
            });
        }
        Err(ConfigError::AmbiguousAction(when.to_string()))
    }

    fn parse(&mut self, when: &str, d: &Do) -> Result<Parsed, ConfigError> {
        let t = Trigger::parse(when).map_err(|source| ConfigError::Trigger {
            when: when.to_string(),
            source,
        })?;
        let mut mask = ModMask::default();
        for m in &t.modifiers {
            let i = self.modifier(m, when)?;
            mask.press(i);
        }
        let ctl = control(&t.control, &mut self.extras, when)?;
        let mut hold_ms = None;
        let edges = match (ctl, &t.edge) {
            (Ctl::Swipe, Some(EdgeSpec::Up)) => vec![Edge::SwipeUp],
            (Ctl::Swipe, Some(EdgeSpec::Down)) => vec![Edge::SwipeDown],
            (Ctl::Swipe, Some(EdgeSpec::Left)) => vec![Edge::SwipeLeft],
            (Ctl::Swipe, Some(EdgeSpec::Right)) => vec![Edge::SwipeRight],
            (Ctl::Swipe, _) => {
                return Err(err(
                    when,
                    "swipe needs a direction: swipe.up/down/left/right",
                ));
            }
            (Ctl::Wheel | Ctl::Thumbwheel, Some(EdgeSpec::Up | EdgeSpec::Right)) => {
                vec![Edge::RotatePos]
            }
            (Ctl::Wheel | Ctl::Thumbwheel, Some(EdgeSpec::Down | EdgeSpec::Left)) => {
                vec![Edge::RotateNeg]
            }
            (Ctl::Wheel | Ctl::Thumbwheel, None) => {
                if d.action.is_none() {
                    return Err(err(
                        when,
                        "a wheel without a direction is continuous and needs a plugin `action` \
                         (e.g. kde.volume); use `.up`/`.down` for keys, exec or buttons",
                    ));
                }
                vec![Edge::RotatePos, Edge::RotateNeg]
            }
            (Ctl::Wheel | Ctl::Thumbwheel, Some(_)) => {
                return Err(err(
                    when,
                    "wheels take .up/.down (thumbwheel also .left/.right)",
                ));
            }
            (_, None | Some(EdgeSpec::Press)) => vec![Edge::Press],
            (_, Some(EdgeSpec::Release)) => vec![Edge::Release],
            (_, Some(EdgeSpec::Tap)) => vec![Edge::Tap],
            (_, Some(EdgeSpec::Hold(d))) => {
                hold_ms = Some(u16::try_from(d.as_millis()).unwrap_or(u16::MAX).max(1));
                vec![Edge::Hold]
            }
            (_, Some(EdgeSpec::DoubleTap)) => {
                return Err(err(
                    when,
                    "double_tap is not supported yet (planned, phase 6)",
                ));
            }
            (_, Some(_)) => {
                return Err(err(when, "buttons take press, release, tap or hold(<N>ms)"));
            }
        };
        match ctl {
            Ctl::Evdev(_) | Ctl::Wheel | Ctl::Thumbwheel if mask.is_empty() => {
                return Err(err(
                    when,
                    format!(
                        "`{}` can only be bound in a chord with a divertable modifier \
                         (e.g. `gesture + {}`): Detent doesn't intercept it otherwise",
                        t.control, t.control
                    ),
                ));
            }
            Ctl::Swipe if mask.is_empty() => {
                return Err(err(
                    when,
                    "swipes need a held modifier (`gesture + swipe.up`)",
                ));
            }
            _ => {}
        }
        if let Ctl::Evdev(_) = ctl
            && !matches!(edges[0], Edge::Press | Edge::Release)
        {
            return Err(err(
                when,
                "left/right chords support press/release only (tap/hold need diversion)",
            ));
        }
        let action = self.action(d, when, edges.len() == 2)?;
        Ok(Parsed {
            when: when.to_string(),
            mask,
            ctl,
            edges,
            hold_ms,
            action,
        })
    }

    fn collect(
        &mut self,
        binds: &[Bind],
        layers: &BTreeMap<String, Layer>,
    ) -> Result<Vec<Parsed>, ConfigError> {
        let mut v = Vec::new();
        for b in binds {
            v.push(self.parse(&b.when, &b.action)?);
        }
        for layer in layers.values() {
            for (target, d) in &layer.bindings {
                v.push(self.parse(&format!("{} + {}", layer.hold, target), d)?);
            }
        }
        Ok(v)
    }

    fn register(&mut self, p: &Parsed) -> Result<(), ConfigError> {
        let s = match p.ctl {
            Ctl::Cid(s, c) => {
                let info = &mut self.out.slots[s.0 as usize];
                info.cid = c;
                info.native_button = native_button(c);
                s
            }
            Ctl::Evdev(s) => {
                self.out.slots[s.0 as usize].evdev_button = if s == slot::LEFT {
                    code::BTN_LEFT
                } else {
                    code::BTN_RIGHT
                };
                self.out.grab_mods.0 |= p.mask.0;
                s
            }
            Ctl::Wheel => {
                self.out.grab_mods.0 |= p.mask.0;
                slot::WHEEL
            }
            Ctl::Thumbwheel => {
                self.out.grab_mods.0 |= p.mask.0;
                slot::THUMBWHEEL
            }
            Ctl::Swipe => {
                self.out.swipe_mods.0 |= p.mask.0;
                slot::SWIPE
            }
        };
        if let Some(ms) = p.hold_ms {
            let cur = &mut self.out.slots[s.0 as usize].hold_ms;
            if *cur != 0 && *cur != ms {
                return Err(err(
                    &p.when,
                    format!("a control has one hold time; {} ms is already used", *cur),
                ));
            }
            *cur = ms;
        }
        let _ = self.layer_for(p.mask);
        Ok(())
    }

    fn fill(&self, tables: &mut Tables, list: &[Parsed]) {
        for p in list {
            let s = match p.ctl {
                Ctl::Cid(s, _) | Ctl::Evdev(s) => s,
                Ctl::Wheel => slot::WHEEL,
                Ctl::Thumbwheel => slot::THUMBWHEEL,
                Ctl::Swipe => slot::SWIPE,
            };
            let layer = if p.mask.is_empty() {
                LayerId::BASE
            } else {
                self.layers.get(&p.mask.0).copied().unwrap_or(LayerId::BASE)
            };
            for &e in &p.edges {
                tables.bindings.set(layer, s, e, p.action);
            }
        }
    }
}

/// `[gesture]`: swipe threshold and haptic confirmation.
fn gesture(g: &GestureSettings, out: &mut Compiled) -> Result<(), ConfigError> {
    use detent_hidpp_features::haptics::waveform;
    if !(1.0..=50.0).contains(&g.threshold_mm) {
        return Err(err("gesture.threshold_mm", "must be between 1 and 50 mm"));
    }
    out.swipe_threshold_um = (g.threshold_mm * 1000.0) as u32;
    out.swipe_haptic = match &g.haptic {
        GestureHaptic::Enabled(false) => None,
        GestureHaptic::Enabled(true) => Some(waveform::DAMP_STATE_CHANGE),
        GestureHaptic::Waveform(w) => Some(
            waveform::by_name(w)
                .ok_or_else(|| err("gesture.haptic", format!("unknown waveform `{w}`")))?,
        ),
    };
    Ok(())
}

/// Compile a validated config.
pub fn compile(cfg: &Config) -> Result<CompiledConfig, ConfigError> {
    let mut b = Builder {
        cfg,
        out: Compiled::default(),
        modifiers: Vec::new(),
        layers: BTreeMap::new(),
        extras: Vec::new(),
        exec: Vec::new(),
        actions: Vec::new(),
    };
    let base = b.collect(&cfg.bind, &cfg.layer)?;
    let mut profiles = Vec::new();
    for (name, p) in &cfg.profile {
        if p.matches.is_empty() {
            return Err(err(&format!("profile.{name}"), "`match` must not be empty"));
        }
        profiles.push((
            name.clone(),
            p.matches.clone(),
            b.collect(&p.bind, &p.layer)?,
        ));
    }
    for p in base
        .iter()
        .chain(profiles.iter().flat_map(|(_, _, v)| v.iter()))
    {
        b.register(p)?;
    }
    // Modifier slots.
    for (i, &c) in b.modifiers.clone().iter().enumerate() {
        let s = match control(
            cid::config_name(c).unwrap_or("?"),
            &mut b.extras,
            "modifier",
        )? {
            Ctl::Cid(s, _) => s,
            _ => continue,
        };
        let info = &mut b.out.slots[s.0 as usize];
        info.cid = c;
        info.modifier = Some(i as u8);
        info.native_button = 0;
        b.out.modifier_slots[i] = Some(s);
    }
    if let Some(g) = &cfg.gesture {
        gesture(g, &mut b.out)?;
    }
    let n_layers = b.layers.len() + 1;
    let mut base_tables = Tables::new(n_layers);
    for (&mask, &layer) in &b.layers {
        base_tables.layers.set(ModMask(mask), layer);
    }
    // A diverted SmartShift button keeps its native job (toggle ratchet) unless rebound.
    let ss = slot::SMARTSHIFT;
    if b.out.slots[ss.0 as usize].cid != 0
        && base_tables
            .bindings
            .lookup(LayerId::BASE, ss, Edge::Press)
            .is_none()
    {
        base_tables.bindings.set(
            LayerId::BASE,
            ss,
            Edge::Press,
            Action::Device(DeviceCmd::ToggleSmartShift),
        );
    }
    b.fill(&mut base_tables, &base);
    let mut engine_profiles = vec![base_tables.clone()];
    let mut matches = Vec::new();
    for (name, m, list) in &profiles {
        let mut t = base_tables.clone();
        b.fill(&mut t, list);
        engine_profiles.push(t);
        matches.push(ProfileMatch {
            name: name.clone(),
            matches: m.clone(),
        });
    }
    if engine_profiles.len() > 255 {
        return Err(err("profile", "at most 254 profiles"));
    }
    b.out.profiles = engine_profiles;
    let _ = b.cfg;
    Ok(CompiledConfig {
        engine: b.out,
        exec: b.exec,
        actions: b.actions,
        profiles: matches,
        source: cfg.clone(),
    })
}
