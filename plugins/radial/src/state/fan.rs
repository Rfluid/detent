//! The fan: the adjustable item's targets (output devices) as small bubbles outside the
//! ring. Opened from the gauge's pill (or Tab while pinned); while it is open the ring
//! itself is inactive. Spec: docs/plugins/radial.md ("Switching the output").

use serde_json::json;

use super::gauge::invoke;
use super::{
    Choice, Effect, Input, Key, LEVEL_REPLY, LEVEL_TIMEOUT_MS, Level, LevelState, MAX_TARGETS,
    Radial, Target, Timer,
};

/// What the fan made of an input.
pub(super) enum Fan {
    /// Handled; the ring stays as it is.
    Done,
    /// Handled by choosing with the held trigger: the hold is over.
    DoneEndHold,
    /// Not the fan's: the ring handles it as usual.
    Pass(Input),
}

impl Radial {
    pub(super) fn fan_open(&self) -> bool {
        self.gauge.as_ref().is_some_and(|g| g.view.fan)
    }

    /// Two or more targets: the pill is a switch.
    pub(super) fn can_fan(&self) -> bool {
        self.gauge
            .as_ref()
            .is_some_and(|g| g.view.targets.len() >= 2)
    }

    pub(super) fn on_targets(
        &mut self,
        source: &str,
        seq: u64,
        targets: Result<Vec<Choice>, String>,
        fx: &mut Vec<Effect>,
    ) {
        let Some(g) = &mut self.gauge else { return };
        if g.adjust.source != source || seq != g.targets_seq {
            return;
        }
        g.view.targets = match targets {
            Ok(mut t) => {
                t.truncate(MAX_TARGETS);
                t
            }
            Err(e) => {
                eprintln!("detent-radial: {source}.targets: {e}");
                Vec::new()
            }
        };
        if g.view.targets.len() < 2 {
            g.view.fan = false;
            g.view.fan_hl = None;
        }
        self.emit_gauge(fx);
    }

    /// Show the fan (two or more targets). `false` = nothing to show.
    pub(super) fn open_fan(&mut self, fx: &mut Vec<Effect>) -> bool {
        if !self.can_fan() {
            return false;
        }
        let Some(g) = &mut self.gauge else {
            return false;
        };
        g.view.fan = true;
        g.view.open = true;
        g.view.fan_hl = None;
        // Cancel a pending leave: the dial stays open under the fan.
        self.gauge_timer += 1;
        if self.settings.haptic_ticks {
            fx.push(Effect::Haptic(self.settings.tick_waveform));
        }
        self.emit_gauge(fx);
        true
    }

    fn close_fan(&mut self, fx: &mut Vec<Effect>) {
        if let Some(g) = &mut self.gauge {
            g.view.fan = false;
            g.view.fan_hl = None;
        }
        self.emit_gauge(fx);
    }

    fn fan_highlight(&mut self, h: Option<usize>, fx: &mut Vec<Effect>) {
        let Some(g) = &mut self.gauge else { return };
        let h = h.filter(|j| *j < g.view.targets.len());
        if h == g.view.fan_hl {
            return;
        }
        g.view.fan_hl = h;
        if h.is_some() && self.settings.haptic_ticks {
            fx.push(Effect::Haptic(self.settings.tick_waveform));
        }
        self.emit_gauge(fx);
    }

    /// Wheel (clamped at the ends) or Tab (wraps): move the highlight, starting from the
    /// current target when nothing is highlighted yet.
    fn fan_step(&mut self, delta: i32, wrap: bool, fx: &mut Vec<Effect>) {
        let Some(g) = &self.gauge else { return };
        let n = g.view.targets.len() as i32;
        if n == 0 || delta == 0 {
            return;
        }
        let next = match g.view.fan_hl {
            Some(j) if wrap => (j as i32 + delta).rem_euclid(n),
            Some(j) => (j as i32 + delta).clamp(0, n - 1),
            None => g.view.targets.iter().position(|t| t.current).unwrap_or(0) as i32,
        };
        self.fan_highlight(Some(next as usize), fx);
    }

    /// Switch to target `j` (nothing to do for the current one) and fold the fan.
    fn choose(&mut self, j: usize, fx: &mut Vec<Effect>) {
        let Some(mut g) = self.gauge.take() else {
            return;
        };
        g.view.fan = false;
        g.view.fan_hl = None;
        if let Some(t) = g.view.targets.get(j).cloned()
            && !t.current
        {
            for (k, c) in g.view.targets.iter_mut().enumerate() {
                c.current = k == j;
            }
            // Show the new output's level right away; the reply sets the real one.
            let old = match &g.view.level {
                LevelState::Ready(l) => l.percent,
                _ => 0,
            };
            g.view.level = LevelState::Ready(Level {
                percent: t.percent.unwrap_or(old).min(g.adjust.max),
                max: g.adjust.max,
                muted: t.muted,
                label: t.name.clone(),
            });
            self.level_seq += 1;
            g.seq = self.level_seq;
            fx.push(invoke(
                format!("{}.target", g.adjust.source),
                json!({ "id": t.id, "max": g.adjust.max, "reply": LEVEL_REPLY, "seq": g.seq }),
            ));
            fx.push(Effect::After {
                ms: LEVEL_TIMEOUT_MS,
                input: Box::new(Input::Due(Timer::LevelTimeout, g.seq)),
            });
        }
        if self.settings.haptic_ticks {
            fx.push(Effect::Haptic(self.settings.select_waveform));
        }
        self.gauge = Some(g);
        self.emit_gauge(fx);
    }

    /// An input while the fan is open. `holding`: the trigger is still down;
    /// `on_trigger`: the input is the trigger control's button.
    pub(super) fn fan_input(
        &mut self,
        input: Input,
        holding: bool,
        on_trigger: bool,
        fx: &mut Vec<Effect>,
    ) -> Fan {
        let hl = self.gauge.as_ref().and_then(|g| g.view.fan_hl);
        match input {
            // The ring under the fan doesn't react.
            Input::Hover(_) | Input::Middle(_) => {}
            Input::HoverChoice(h) => self.fan_highlight(h, fx),
            Input::Scroll { coarse, fine } => self.fan_step(-(coarse + fine).signum(), false, fx),
            Input::Key(Key::Tab) => self.fan_step(1, true, fx),
            Input::Key(Key::BackTab) => self.fan_step(-1, true, fx),
            Input::Key(Key::Select) => match hl {
                Some(j) => self.choose(j, fx),
                None => self.close_fan(fx),
            },
            Input::Click {
                left: true,
                target: Target::Choice(j),
            } => self.choose(j, fx),
            // A click outside everything closes the ring, as usual.
            Input::Click {
                left: true,
                target: Target::Outside,
            } => return Fan::Pass(input),
            Input::Key(Key::Escape) | Input::Click { .. } => self.close_fan(fx),
            // Releasing the held trigger on a fan bubble chooses it.
            Input::Button { press: false, .. } if holding && on_trigger => {
                if let Some(j) = hl {
                    self.choose(j, fx);
                }
                return Fan::DoneEndHold;
            }
            // The trigger again: choose what is highlighted, else close the ring.
            Input::Button { press: true, .. } if on_trigger && hl.is_some() => {
                if let Some(j) = hl {
                    self.choose(j, fx);
                }
            }
            Input::Open { .. } if hl.is_some() => {
                if let Some(j) = hl {
                    self.choose(j, fx);
                }
            }
            other => return Fan::Pass(other),
        }
        Fan::Done
    }
}
