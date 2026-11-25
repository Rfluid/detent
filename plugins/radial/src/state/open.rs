//! The open ring: hover, keys, clicks and the trigger while it is shown; selecting an item,
//! entering and leaving folders, closing.

use super::fan::Fan;
use super::{Effect, Input, Key, Radial, Ring, State, Target};
use crate::menu::{Item, ItemAction};

/// What the adjustable item made of an input (see `Radial::adjustable`).
enum Pre {
    /// Handled; the ring stays as it is.
    Keep,
    /// Handled with the held trigger: the hold is over.
    EndHold,
    /// Select this item (the pill of a gauge with nothing to switch to).
    Select(usize),
    /// Not the adjustable item's: the ring handles it.
    Go(Input),
}

impl Radial {
    #[allow(clippy::too_many_arguments)] // the fields of State::Open, destructured
    pub(super) fn open(
        &mut self,
        stack: Vec<Vec<Item>>,
        hl: Option<usize>,
        holding: bool,
        leased: bool,
        control: Option<String>,
        input: Input,
        fx: &mut Vec<Effect>,
    ) -> State {
        let n = stack.last().map_or(0, Vec::len);
        let keep = |stack, hl, holding, leased, control| State::Open {
            stack,
            hl,
            holding,
            leased,
            control,
        };
        let is_trigger = |c: &str| control.as_deref() == Some(c);
        let on_trigger = matches!(&input, Input::Button { control: c, .. } if is_trigger(c));
        let input = match self.adjustable(input, hl, holding, on_trigger, fx) {
            Pre::Keep => return keep(stack, hl, holding, leased, control),
            Pre::EndHold => {
                if leased {
                    fx.push(Effect::ReleaseLease);
                }
                return keep(stack, hl, false, false, control);
            }
            Pre::Select(i) => return self.select(i, stack, leased, control, fx),
            Pre::Go(i) => i,
        };
        // The trigger again, or `radial.open` again: select what is highlighted, else
        // close. On an adjustable item it closes (it has nothing to run).
        let runnable = hl.filter(|i| !self.is_gauge(*i));
        match input {
            // A pinned gauge keeps the highlight: the wheel goes to it wherever the
            // cursor is.
            Input::Hover(_) if self.pinned() => keep(stack, hl, holding, leased, control),
            Input::Hover(h) => {
                let h = h.filter(|i| *i < n);
                self.highlight(hl, h, fx);
                if h != hl {
                    self.gauge_hover(h, fx);
                }
                keep(stack, h, holding, leased, control)
            }
            Input::Key(Key::Tab) | Input::Key(Key::BackTab) if n > 0 => {
                let next = match (input, hl) {
                    (Input::Key(Key::Tab), None) => 0,
                    (Input::Key(Key::Tab), Some(i)) => (i + 1) % n,
                    (_, None) => n - 1,
                    (_, Some(i)) => (i + n - 1) % n,
                };
                if let Some(g) = &mut self.gauge {
                    g.view.pinned = false;
                }
                self.highlight(hl, Some(next), fx);
                self.gauge_hover(Some(next), fx);
                keep(stack, Some(next), holding, leased, control)
            }
            Input::Key(Key::Select) => match hl {
                Some(i) => self.select(i, stack, leased, control, fx),
                None => keep(stack, hl, holding, leased, control),
            },
            Input::Key(Key::Escape)
            | Input::Click { left: false, .. }
            | Input::Click {
                target: Target::Center,
                ..
            } => self.back(stack, leased, control, fx),
            Input::Click {
                target: Target::Item(i),
                ..
            } if i < n => self.select(i, stack, leased, control, fx),
            Input::Click { .. } => self.close(leased, fx),
            // Hold style: releasing the trigger on a bubble selects it; anywhere else the
            // ring stays open for a click (that is also the press-and-release style).
            Input::Button {
                control: c,
                press: false,
            } if holding && is_trigger(&c) => match hl {
                Some(i) => self.select(i, stack, leased, control, fx),
                None => {
                    if leased {
                        fx.push(Effect::ReleaseLease);
                    }
                    keep(stack, hl, false, false, control)
                }
            },
            // Trigger pressed again while open (through the lease, or the binding firing
            // `radial.open` again): select what is highlighted, else close.
            Input::Button {
                control: c,
                press: true,
            } if is_trigger(&c) => match runnable {
                Some(i) => self.select(i, stack, leased, control, fx),
                None => self.close(leased, fx),
            },
            Input::Open { .. } => match runnable {
                Some(i) => self.select(i, stack, leased, control, fx),
                None => self.close(leased, fx),
            },
            Input::Revoked { close: true } => {
                self.gauge = None;
                fx.push(Effect::Hide);
                State::Closed
            }
            Input::Revoked { close: false } | Input::LeaseDenied => {
                keep(stack, hl, false, false, control)
            }
            _ => keep(stack, hl, holding, leased, control),
        }
    }

    /// What the adjustable item takes before the ring: the fan's inputs while it is open,
    /// the value and the provider's replies, Tab on the pinned gauge and the pill (both open
    /// the fan; with nothing to switch to, the pill acts like its bubble).
    fn adjustable(
        &mut self,
        input: Input,
        hl: Option<usize>,
        holding: bool,
        on_trigger: bool,
        fx: &mut Vec<Effect>,
    ) -> Pre {
        let input = if self.fan_open() {
            match self.fan_input(input, holding, on_trigger, fx) {
                Fan::Done => return Pre::Keep,
                Fan::DoneEndHold => return Pre::EndHold,
                Fan::Pass(i) => i,
            }
        } else {
            input
        };
        let input = match self.value_input(input, hl, fx) {
            Ok(()) => return Pre::Keep,
            Err(i) => i,
        };
        match input {
            Input::Key(Key::Tab) if self.pinned() && self.open_fan(fx) => Pre::Keep,
            Input::Click {
                left: true,
                target: Target::Pill,
            } => {
                if self.open_fan(fx) {
                    return Pre::Keep;
                }
                match self.gauge.as_ref().map(|g| g.view.item) {
                    Some(i) => Pre::Select(i),
                    None => Pre::Keep,
                }
            }
            other => Pre::Go(other),
        }
    }

    /// The adjustable item's value and the provider's replies: handled here without
    /// changing the ring. `Err` = not one of these.
    fn value_input(
        &mut self,
        input: Input,
        hl: Option<usize>,
        fx: &mut Vec<Effect>,
    ) -> Result<(), Input> {
        match input {
            Input::Scroll { coarse, fine } => self.scroll(coarse, fine, hl, fx),
            Input::Middle(target) => self.mute(target, fx),
            Input::Level { source, seq, level } => self.on_level(&source, seq, level, fx),
            Input::Targets {
                source,
                seq,
                targets,
            } => self.on_targets(&source, seq, targets, fx),
            Input::Due(t, id) => self.on_due(t, id, hl, fx),
            other => return Err(other),
        }
        Ok(())
    }

    fn highlight(&self, old: Option<usize>, new: Option<usize>, fx: &mut Vec<Effect>) {
        if new != old {
            fx.push(Effect::Highlight(new));
            if new.is_some() && self.settings.haptic_ticks {
                fx.push(Effect::Haptic(self.settings.tick_waveform));
            }
        }
    }

    fn select(
        &mut self,
        i: usize,
        mut stack: Vec<Vec<Item>>,
        leased: bool,
        control: Option<String>,
        fx: &mut Vec<Effect>,
    ) -> State {
        let Some(item) = stack.last().and_then(|r| r.get(i)).cloned() else {
            return self.close(leased, fx);
        };
        if self.settings.haptic_ticks {
            fx.push(Effect::Haptic(self.settings.select_waveform));
        }
        match item.action {
            // Selecting the adjustable item pins (or unpins) its gauge; the ring stays open
            // for the wheel, as click-to-select (the trigger may still be down).
            ItemAction::Adjust(_) => {
                if let Some(g) = &mut self.gauge {
                    g.view.pinned = !g.view.pinned;
                    g.view.open = true;
                }
                self.gauge_timer += 1;
                self.emit_gauge(fx);
                if leased {
                    fx.push(Effect::ReleaseLease);
                }
                State::Open {
                    stack,
                    hl: Some(i),
                    holding: false,
                    leased: false,
                    control,
                }
            }
            ItemAction::Folder(items) => {
                fx.push(Effect::Navigate(Ring {
                    items: items.clone(),
                    back: true,
                }));
                // A folder continues as click-to-select (the trigger may still be down).
                if leased {
                    fx.push(Effect::ReleaseLease);
                }
                self.start_gauge(&items, fx);
                stack.push(items);
                State::Open {
                    stack,
                    hl: None,
                    holding: false,
                    leased: false,
                    control,
                }
            }
            action => {
                self.gauge = None;
                fx.push(Effect::Hide);
                if leased {
                    fx.push(Effect::ReleaseLease);
                }
                fx.push(Effect::Run(action.steps()));
                State::Closed
            }
        }
    }

    /// Esc / right click / centre: leave the folder, or close at the top level.
    fn back(
        &mut self,
        mut stack: Vec<Vec<Item>>,
        leased: bool,
        control: Option<String>,
        fx: &mut Vec<Effect>,
    ) -> State {
        if stack.len() <= 1 {
            return self.close(leased, fx);
        }
        stack.pop();
        let items = stack.last().cloned().unwrap_or_default();
        fx.push(Effect::Navigate(Ring {
            items: items.clone(),
            back: stack.len() > 1,
        }));
        self.start_gauge(&items, fx);
        State::Open {
            stack,
            hl: None,
            holding: false,
            leased,
            control,
        }
    }

    fn close(&mut self, leased: bool, fx: &mut Vec<Effect>) -> State {
        self.gauge = None;
        fx.push(Effect::Hide);
        if leased {
            fx.push(Effect::ReleaseLease);
        }
        State::Closed
    }
}
