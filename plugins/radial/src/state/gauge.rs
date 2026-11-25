//! The adjustable item's gauge (`adjust = "kde.volume"`): asking the provider for its value,
//! the hover / leave timers, wheel and mute changes, and the provider's replies.

use serde_json::json;

use super::{
    Effect, GAUGE_HOVER_MS, GAUGE_LEAVE_MS, Gauge, Input, LEVEL_REPLY, LEVEL_TIMEOUT_MS, Level,
    LevelState, Radial, TARGETS_REPLY, Target, Timer,
};
use crate::menu::{Adjust, Item, ItemAction, Step};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct GaugeState {
    pub(super) adjust: Adjust,
    pub(super) view: Gauge,
    /// The latest request: only its reply is taken (older ones are superseded).
    pub(super) seq: u64,
    /// The latest `.targets` request.
    pub(super) targets_seq: u64,
}

pub(super) fn invoke(action: String, params: serde_json::Value) -> Effect {
    Effect::Run(vec![Step::Invoke(action, params.to_string())])
}

/// Did going from `old` to `new` reach or cross a multiple of ten?
pub(super) fn crosses_ten(old: u32, new: u32) -> bool {
    if new > old {
        new / 10 > old / 10
    } else {
        old.div_ceil(10) > new.div_ceil(10)
    }
}

impl Radial {
    pub(super) fn emit_gauge(&self, fx: &mut Vec<Effect>) {
        fx.push(Effect::Gauge(self.gauge.as_ref().map(|g| g.view.clone())));
    }

    pub(super) fn is_gauge(&self, i: usize) -> bool {
        self.gauge.as_ref().is_some_and(|g| g.view.item == i)
    }

    pub(super) fn pinned(&self) -> bool {
        self.gauge.as_ref().is_some_and(|g| g.view.pinned)
    }

    /// A ring was shown: find its adjustable item and ask its provider for the value.
    pub(super) fn start_gauge(&mut self, items: &[Item], fx: &mut Vec<Effect>) {
        let had = self.gauge.is_some();
        self.gauge = items
            .iter()
            .enumerate()
            .find_map(|(i, it)| match &it.action {
                ItemAction::Adjust(a) => Some(GaugeState {
                    adjust: a.clone(),
                    view: Gauge {
                        item: i,
                        open: false,
                        pinned: false,
                        level: LevelState::Loading,
                        targets: Vec::new(),
                        fan: false,
                        fan_hl: None,
                    },
                    seq: 0,
                    targets_seq: 0,
                }),
                _ => None,
            });
        self.gauge_timer += 1;
        if let Some(mut g) = self.gauge.take() {
            self.level_seq += 1;
            g.seq = self.level_seq;
            fx.push(invoke(
                format!("{}.get", g.adjust.source),
                json!({ "reply": LEVEL_REPLY, "seq": g.seq, "max": g.adjust.max }),
            ));
            // Asked now, so the fan opens at once when the pill is clicked.
            self.level_seq += 1;
            g.targets_seq = self.level_seq;
            fx.push(invoke(
                format!("{}.targets", g.adjust.source),
                json!({ "reply": TARGETS_REPLY, "seq": g.targets_seq }),
            ));
            fx.push(Effect::After {
                ms: LEVEL_TIMEOUT_MS,
                input: Box::new(Input::Due(Timer::LevelTimeout, g.seq)),
            });
            self.gauge = Some(g);
        }
        if had || self.gauge.is_some() {
            self.emit_gauge(fx);
        }
    }

    /// The highlight moved: open the gauge after a hover, or close it after leaving.
    pub(super) fn gauge_hover(&mut self, h: Option<usize>, fx: &mut Vec<Effect>) {
        let Some(g) = &self.gauge else { return };
        self.gauge_timer += 1;
        let on = h == Some(g.view.item);
        if on && !g.view.open {
            fx.push(Effect::After {
                ms: GAUGE_HOVER_MS,
                input: Box::new(Input::Due(Timer::GaugeHover, self.gauge_timer)),
            });
        } else if !on && g.view.open && !g.view.pinned {
            fx.push(Effect::After {
                ms: GAUGE_LEAVE_MS,
                input: Box::new(Input::Due(Timer::GaugeLeave, self.gauge_timer)),
            });
        }
    }

    pub(super) fn on_due(&mut self, t: Timer, id: u64, hl: Option<usize>, fx: &mut Vec<Effect>) {
        let Some(mut g) = self.gauge.take() else {
            return;
        };
        let on = hl == Some(g.view.item);
        let changed = match t {
            Timer::GaugeHover if id == self.gauge_timer && on && !g.view.open => {
                g.view.open = true;
                true
            }
            Timer::GaugeLeave if id == self.gauge_timer && !on && !g.view.pinned && !g.view.fan => {
                std::mem::replace(&mut g.view.open, false)
            }
            Timer::LevelTimeout if id == g.seq && g.view.level == LevelState::Loading => {
                g.view.level = LevelState::Unavailable(format!(
                    "no answer from {} (is its plugin running?)",
                    g.adjust.source
                ));
                true
            }
            _ => false,
        };
        self.gauge = Some(g);
        if changed {
            self.emit_gauge(fx);
        }
    }

    /// Wheel notches: change the value while the gauge is hovered or pinned.
    pub(super) fn scroll(
        &mut self,
        coarse: i32,
        fine: i32,
        hl: Option<usize>,
        fx: &mut Vec<Effect>,
    ) {
        let Some(mut g) = self.gauge.take() else {
            return;
        };
        if !(g.view.pinned || hl == Some(g.view.item)) {
            self.gauge = Some(g);
            return;
        }
        // Scrolling on the bubble opens the gauge at once.
        let mut changed = !std::mem::replace(&mut g.view.open, true);
        let a = &g.adjust;
        let fine = if a.invert_fine { -fine } else { fine };
        let delta = coarse * a.step as i32 + fine * a.fine_step as i32;
        if let (LevelState::Ready(level), true) = (&mut g.view.level, delta != 0) {
            let old = level.percent.min(a.max);
            let new = (old as i32 + delta).clamp(0, a.max as i32) as u32;
            let unmute = level.muted && delta > 0;
            if new != old && self.settings.haptic_ticks {
                if new == 0 || new == a.max {
                    fx.push(Effect::Haptic(self.settings.select_waveform));
                } else if crosses_ten(old, new) {
                    fx.push(Effect::Haptic(self.settings.tick_waveform));
                }
            }
            if new != old || unmute {
                level.percent = new;
                level.muted &= !unmute;
                self.level_seq += 1;
                g.seq = self.level_seq;
                // Our gauge replaces Plasma's OSD.
                fx.push(invoke(
                    format!("{}.set", a.source),
                    json!({ "percent": new, "max": a.max, "unmute": unmute, "osd": false,
                            "reply": LEVEL_REPLY, "seq": g.seq }),
                ));
                changed = true;
            }
        }
        self.gauge = Some(g);
        if changed {
            self.emit_gauge(fx);
        }
    }

    /// Middle click on the bubble or its pill (or anywhere while pinned): toggle mute.
    pub(super) fn mute(&mut self, target: Target, fx: &mut Vec<Effect>) {
        let Some(mut g) = self.gauge.take() else {
            return;
        };
        let here = g.view.pinned || target == Target::Item(g.view.item) || target == Target::Pill;
        if let (LevelState::Ready(level), true) = (&mut g.view.level, here) {
            level.muted = !level.muted;
            g.view.open = true;
            self.level_seq += 1;
            g.seq = self.level_seq;
            fx.push(invoke(
                format!("{}.mute", g.adjust.source),
                json!({ "reply": LEVEL_REPLY, "seq": g.seq, "max": g.adjust.max }),
            ));
            if self.settings.haptic_ticks {
                fx.push(Effect::Haptic(self.settings.select_waveform));
            }
            self.gauge = Some(g);
            self.emit_gauge(fx);
        } else {
            self.gauge = Some(g);
        }
    }

    pub(super) fn on_level(
        &mut self,
        source: &str,
        seq: u64,
        level: Result<Level, String>,
        fx: &mut Vec<Effect>,
    ) {
        let Some(g) = &mut self.gauge else { return };
        // Replies to superseded requests would make the gauge jump back while scrolling.
        if g.adjust.source != source || seq != g.seq {
            return;
        }
        g.view.level = match level {
            Ok(l) => LevelState::Ready(l),
            Err(e) => LevelState::Unavailable(e),
        };
        self.emit_gauge(fx);
    }
}
