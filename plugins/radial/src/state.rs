//! The radial menu's interaction state machine, modelled on the Logitech Actions Ring.
//! Pure: inputs in, effects out (the GPUI layer and the bus connection perform them).
//! Spec: docs/plugins/radial.md ("Interaction").
//!
//! - The ring opens around the cursor; the cursor moves freely; the bubble under it is
//!   highlighted (haptic tick); clicking it selects.
//! - Both opening styles work at once, as on the Actions Ring: press-and-release the
//!   trigger, then click a bubble; or hold the trigger and release it on a bubble.
//! - Esc / right click: back out of a folder, else close. Click on the centre: same. Click
//!   outside the ring: close. Tab / Shift+Tab rotate the highlight, Enter / Space select.
//! - An adjustable item (`adjust = "kde.volume"`) shows a gauge after a short hover; the
//!   wheel (coarse) and thumbwheel (fine) change its value, middle click mutes, selecting it
//!   pins the gauge (the wheel then works anywhere) instead of running anything.
//! - When its provider offers targets (the output devices), clicking the gauge's pill opens
//!   the fan: one small bubble per target, outside the ring; choosing one switches to it.
//!
//! Layout: `open` handles the shown ring, `gauge` the adjustable item's value, `fan` its
//! targets; tests in `tests.rs` and `tests/`.

mod fan;
mod gauge;
mod open;
#[cfg(test)]
mod tests;

use crate::icon::Icon;
use crate::menu::{Item, Settings, Step};
use gauge::GaugeState;

/// Hover time on an adjustable item before its gauge opens (a click or scroll opens it
/// at once).
pub const GAUGE_HOVER_MS: u32 = 120;
/// After the cursor leaves an unpinned gauge: it closes this much later (moving back in
/// time keeps it, so crossing the gap to the card doesn't flicker).
pub const GAUGE_LEAVE_MS: u32 = 300;
/// No answer from the provider in this time: "unavailable".
pub const LEVEL_TIMEOUT_MS: u32 = 1000;
/// The action providers answer to (`radial.level`, declared in the manifest).
pub const LEVEL_REPLY: &str = "radial.level";
/// Where providers send their target list (`radial.targets`).
pub const TARGETS_REPLY: &str = "radial.targets";
/// Fan bubbles at most (the ring's own limit).
pub const MAX_TARGETS: usize = 8;

/// Something the adjustable item can act on instead (an output device), from the provider.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    /// The provider's id, passed back in `<source>.target`.
    pub id: String,
    pub name: String,
    /// The provider's full name (raw device name), shown in the fan's label; the short
    /// `name` when it has none.
    pub description: String,
    pub icon: Icon,
    /// Its own value, when the provider knows it.
    pub percent: Option<u32>,
    pub muted: bool,
    /// The one adjusted now.
    pub current: bool,
}

/// A provider's value (`{ percent, muted, max, label }`).
#[derive(Debug, Clone, PartialEq)]
pub struct Level {
    pub percent: u32,
    pub max: u32,
    pub muted: bool,
    /// What is being adjusted (the output device's name).
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LevelState {
    Loading,
    Ready(Level),
    Unavailable(String),
}

/// What the overlay draws for the ring's adjustable item.
#[derive(Debug, Clone, PartialEq)]
pub struct Gauge {
    /// Index of the item in the current ring.
    pub item: usize,
    /// The dial is open: number in the bubble, ticks, the pill (else only the level arc).
    pub open: bool,
    pub pinned: bool,
    pub level: LevelState,
    /// The provider's targets (empty when it has none, or only one).
    pub targets: Vec<Choice>,
    /// The fan of targets is shown.
    pub fan: bool,
    /// The fan bubble under the cursor / picked with the wheel or Tab.
    pub fan_hl: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {
    GaugeHover,
    GaugeLeave,
    LevelTimeout,
}

/// Where a click landed (from the overlay's hit test).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Item(usize),
    Center,
    Outside,
    /// The adjustable item's pill (it names what is adjusted; opens the fan).
    Pill,
    /// A fan bubble.
    Choice(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Escape,
    Tab,
    BackTab,
    /// Enter or Space.
    Select,
}

/// Pointer position from the desktop (`kde.cursor`): logical px relative to `output`.
#[derive(Debug, Clone, PartialEq)]
pub struct At {
    pub x: f32,
    pub y: f32,
    pub output: String,
    /// The output's logical size, when the desktop reports it: the overlay opens at it.
    pub size: Option<(f32, f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// `radial.open` fired: menu name and the control that triggered it (`thumb`).
    Open {
        menu: String,
        control: Option<String>,
    },
    /// The focused application changed (`app` context from a desktop plugin).
    App(String),
    /// Reply to `LocateCursor`.
    Cursor(At),
    /// `LocateCursor` got no reply in time (number = the request's sequence).
    LocateTimeout(u32),
    LeaseGranted,
    LeaseDenied,
    /// The daemon took the lease back. `close`: escape chord, reload, disconnect (close
    /// the ring); expiry only ends the hold.
    Revoked {
        close: bool,
    },
    /// The trigger control (leased while held).
    Button {
        control: String,
        press: bool,
    },
    /// From the overlay: the bubble under the cursor changed.
    Hover(Option<usize>),
    /// From the overlay: the fan bubble under the cursor changed.
    HoverChoice(Option<usize>),
    /// From the overlay: a click. `left` = primary button.
    Click {
        left: bool,
        target: Target,
    },
    /// From the overlay: a middle click (mutes the gauge).
    Middle(Target),
    /// From the overlay: whole wheel notches, positive = up (louder). `coarse` = main
    /// wheel / arrow keys, `fine` = thumbwheel (horizontal axis).
    Scroll {
        coarse: i32,
        fine: i32,
    },
    Key(Key),
    /// A provider's answer to `<source>.get` / `.set` / `.mute` (via `radial.level`).
    Level {
        source: String,
        seq: u64,
        level: Result<Level, String>,
    },
    /// A provider's answer to `<source>.targets` (via `radial.targets`).
    Targets {
        source: String,
        seq: u64,
        targets: Result<Vec<Choice>, String>,
    },
    /// A timer set with `Effect::After` is due.
    Due(Timer, u64),
}

/// What the overlay shows: a ring of items around a centre that closes (top level) or
/// goes back (inside a folder).
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub items: Vec<Item>,
    pub back: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Lease the trigger control so its release (hold-and-release) reaches us.
    RequestLease {
        control: String,
    },
    ReleaseLease,
    /// Ask the desktop where the pointer is; answer with `Input::Cursor`, or
    /// `Input::LocateTimeout(seq)` if nothing comes back.
    LocateCursor {
        seq: u32,
    },
    /// Open the overlay with this ring, centred on `at` (or, without it, on the pointer
    /// position the compositor reports; never at a guessed place).
    Show {
        ring: Ring,
        at: Option<At>,
    },
    /// Replace the ring in the open overlay (entering / leaving a folder).
    Navigate(Ring),
    Highlight(Option<usize>),
    Hide,
    Haptic(u8),
    /// Run these steps in order (invokes and delays).
    Run(Vec<Step>),
    /// Draw (or clear) the adjustable item's gauge.
    Gauge(Option<Gauge>),
    /// Feed `input` back in after `ms`.
    After {
        ms: u32,
        input: Box<Input>,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum State {
    Closed,
    /// Waiting for the pointer position before showing.
    Locating {
        items: Vec<Item>,
        seq: u32,
        holding: bool,
        leased: bool,
        control: Option<String>,
    },
    Open {
        /// Rings from the top level down to the current folder (never empty).
        stack: Vec<Vec<Item>>,
        hl: Option<usize>,
        /// The trigger is still down: releasing it on a bubble selects.
        holding: bool,
        /// We hold (or requested) the trigger lease.
        leased: bool,
        control: Option<String>,
    },
}

pub struct Radial {
    pub settings: Settings,
    /// Focused application, for per-app rings.
    app: Option<String>,
    seq: u32,
    state: State,
    /// The current ring's adjustable item, while open.
    gauge: Option<GaugeState>,
    /// Request numbers for provider calls (echoed in replies).
    level_seq: u64,
    /// Generation of the hover/leave timers: bumping it cancels the pending one.
    gauge_timer: u64,
}

impl Radial {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            app: None,
            seq: 0,
            state: State::Closed,
            gauge: None,
            level_seq: 0,
            gauge_timer: 0,
        }
    }

    #[cfg(test)]
    pub fn is_open(&self) -> bool {
        !matches!(self.state, State::Closed)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "predates the lint; split it when next changed (ADR 0008)"
    )]
    pub fn handle(&mut self, input: Input) -> Vec<Effect> {
        let mut fx = Vec::new();
        let state = std::mem::replace(&mut self.state, State::Closed);
        self.state = match (state, input) {
            (s, Input::App(app)) => {
                self.app = Some(app);
                s
            }

            // ── opening ─────────────────────────────────────────────────────────
            (State::Closed, Input::Open { menu, control }) => match self.settings.menu(&menu) {
                Some(m) => {
                    self.seq = self.seq.wrapping_add(1);
                    if let Some(c) = &control {
                        fx.push(Effect::RequestLease { control: c.clone() });
                    }
                    fx.push(Effect::LocateCursor { seq: self.seq });
                    State::Locating {
                        items: m.items_for(self.app.as_deref()).to_vec(),
                        seq: self.seq,
                        holding: control.is_some(),
                        leased: control.is_some(),
                        control,
                    }
                }
                None => {
                    eprintln!("detent-radial: no menu `{menu}` in [plugins.radial.menus]");
                    State::Closed
                }
            },
            (
                State::Locating {
                    items,
                    holding,
                    leased,
                    control,
                    ..
                },
                Input::Cursor(at),
            ) => self.show(items, Some(at), holding, leased, control, &mut fx),
            (
                State::Locating {
                    items,
                    seq,
                    holding,
                    leased,
                    control,
                },
                Input::LocateTimeout(s),
            ) if s == seq => self.show(items, None, holding, leased, control, &mut fx),
            // The trigger was released before the ring appeared: it opens and stays
            // open for a click (press-and-release style).
            (
                State::Locating {
                    items,
                    seq,
                    holding: true,
                    leased,
                    control,
                },
                Input::Button {
                    control: c,
                    press: false,
                },
            ) if control.as_deref() == Some(c.as_str()) => {
                if leased {
                    fx.push(Effect::ReleaseLease);
                }
                State::Locating {
                    items,
                    seq,
                    holding: false,
                    leased: false,
                    control,
                }
            }
            // Pressed again (or escape chord / reload) before it appeared: cancel.
            (
                State::Locating { leased, .. },
                Input::Open { .. } | Input::Revoked { close: true },
            ) => {
                if leased {
                    fx.push(Effect::ReleaseLease);
                }
                State::Closed
            }
            (
                State::Locating {
                    items,
                    seq,
                    control,
                    ..
                },
                Input::LeaseDenied | Input::Revoked { close: false },
            ) => State::Locating {
                items,
                seq,
                holding: false,
                leased: false,
                control,
            },

            // ── open ────────────────────────────────────────────────────────────
            (
                State::Open {
                    stack,
                    hl,
                    holding,
                    leased,
                    control,
                },
                input,
            ) => self.open(stack, hl, holding, leased, control, input, &mut fx),

            (s, _) => s,
        };
        fx
    }

    fn show(
        &mut self,
        items: Vec<Item>,
        at: Option<At>,
        holding: bool,
        leased: bool,
        control: Option<String>,
        fx: &mut Vec<Effect>,
    ) -> State {
        fx.push(Effect::Show {
            ring: Ring {
                items: items.clone(),
                back: false,
            },
            at,
        });
        self.gauge = None;
        self.start_gauge(&items, fx);
        State::Open {
            stack: vec![items],
            hl: None,
            holding,
            leased,
            control,
        }
    }
}

/// The trigger's control name from a binding trigger like `thumb.press` or `gesture + thumb`.
/// Only divertable buttons qualify (they can be leased); anything else (a plugin or
/// `detentctl` invoking the action) yields `None`.
pub fn trigger_control(trigger: &str) -> Option<String> {
    const BUTTONS: &[&str] = &[
        "thumb",
        "gesture",
        "middle",
        "back",
        "forward",
        "smartshift",
    ];
    let last = trigger.rsplit('+').next()?.trim();
    let control = last.split('.').next()?.trim();
    (BUTTONS.contains(&control) || control.starts_with("cid_0x")).then(|| control.to_string())
}
