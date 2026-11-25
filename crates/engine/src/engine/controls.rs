//! Press/release of a bound control and one-shot action output: what a press holds
//! down, what its release undoes, tap/hold resolution, and [`Action`] → [`OutEvent`]s.
//! **Hot path.**

use std::time::Instant;

use crate::action::{Action, Axis, KeyChord};
use crate::compiled::Compiled;
use crate::out::{ControlOut, Out, OutEvent, code};
use crate::{ControlSlot, Edge, ModMask};

use super::swipe::{Swipe, gesture_haptic};
use super::{Engine, Held, SlotState};

impl Engine {
    pub(super) fn mask_without(&self, cfg: &Compiled, s: ControlSlot) -> ModMask {
        let mut m = self.mask;
        if let Some(i) = cfg.slots[s.0 as usize].modifier {
            m.release(i);
        }
        m
    }

    /// Another control was used while modifiers are held: they become chords.
    pub(super) fn mark_chord(&mut self, except: ControlSlot) {
        for (i, st) in self.slots.iter_mut().enumerate() {
            if st.down && i != except.0 as usize {
                st.used = true;
            }
        }
    }

    pub(super) fn press(
        &mut self,
        cfg: &Compiled,
        s: ControlSlot,
        now: Instant,
        out: &mut Out,
        from_evdev: bool,
    ) {
        let i = s.0 as usize;
        self.mark_chord(s);
        let mask_before = self.mask;
        if let Some(m) = cfg.slots[i].modifier {
            self.mask.press(m);
            self.swipe[m as usize] = Swipe::default();
        }
        self.slots[i] = SlotState {
            down: true,
            used: false,
            hold_fired: false,
            pressed_at: Some(now),
            held: Held::Nothing,
        };
        let a = self.lookup(cfg, mask_before, s, Edge::Press);
        let held = match a {
            Action::Keys(k) => {
                emit_keys(k, true, out);
                Held::Keys(k)
            }
            Action::Button(b) => {
                out.push(OutEvent::key(b, true));
                Held::Button(b)
            }
            Action::None => {
                let is_mod = cfg.slots[i].modifier.is_some();
                let bound_somewhere = is_mod
                    || !self.lookup(cfg, mask_before, s, Edge::Release).is_none()
                    || self.has_deferred_edges(cfg, mask_before, s);
                if is_mod || bound_somewhere {
                    Held::Swallowed
                } else if from_evdev {
                    out.push(OutEvent::key(cfg.slots[i].evdev_button, true));
                    Held::PassThrough
                } else if cfg.slots[i].native_button != 0 {
                    out.push(OutEvent::key(cfg.slots[i].native_button, true));
                    Held::Button(cfg.slots[i].native_button)
                } else {
                    Held::Swallowed
                }
            }
            other => {
                self.fire(other, 0, out);
                Held::Swallowed
            }
        };
        self.slots[i].held = held;
    }

    pub(super) fn release(&mut self, cfg: &Compiled, s: ControlSlot, _now: Instant, out: &mut Out) {
        let i = s.0 as usize;
        let st = self.slots[i];
        if !st.down {
            return;
        }
        match st.held {
            Held::PassThrough => out.push(OutEvent::key(cfg.slots[i].evdev_button, false)),
            h => Self::undo(h, out),
        }
        if let Some(m) = cfg.slots[i].modifier {
            self.mask.release(m);
        }
        let mask = self.mask;
        let rel = self.lookup(cfg, mask, s, Edge::Release);
        if !rel.is_none() {
            self.fire(rel, 0, out);
        }
        if !st.used && !st.hold_fired {
            // A swipe modifier released without moving: the "no direction" gesture.
            if cfg.slots[i]
                .modifier
                .is_some_and(|m| cfg.swipe_mods.0 & (1 << m) != 0)
            {
                gesture_haptic(cfg, out);
            }
            let tap = self.lookup(cfg, mask, s, Edge::Tap);
            if !tap.is_none() {
                self.fire(tap, 0, out);
            } else if st.held == Held::Swallowed
                && cfg.slots[i].modifier.is_none()
                && self.has_deferred_edges(cfg, mask, s)
            {
                // Only a hold edge is bound and it didn't fire: behave natively (click).
                let native = if cfg.slots[i].cid != 0 {
                    cfg.slots[i].native_button
                } else {
                    cfg.slots[i].evdev_button
                };
                if native != 0 {
                    out.push(OutEvent::key(native, true));
                    out.push(OutEvent::SYN);
                    out.push(OutEvent::key(native, false));
                }
            }
        }
        self.slots[i] = SlotState::default();
    }

    pub(super) fn undo(held: Held, out: &mut Out) {
        match held {
            Held::Button(b) => out.push(OutEvent::key(b, false)),
            Held::Keys(k) => emit_keys(k, false, out),
            _ => {}
        }
    }

    /// Run a one-shot action.
    pub(super) fn fire(&mut self, a: Action, delta: i16, out: &mut Out) {
        match a {
            Action::None => {}
            Action::Keys(k) => {
                emit_keys(k, true, out);
                out.push(OutEvent::SYN);
                emit_keys(k, false, out);
                out.push(OutEvent::SYN);
            }
            Action::Button(b) => {
                out.push(OutEvent::key(b, true));
                out.push(OutEvent::SYN);
                out.push(OutEvent::key(b, false));
                out.push(OutEvent::SYN);
            }
            Action::Scroll { axis, amount } => {
                let (lo, hi) = match axis {
                    Axis::Vertical => (code::REL_WHEEL, code::REL_WHEEL_HI_RES),
                    Axis::Horizontal => (code::REL_HWHEEL, code::REL_HWHEEL_HI_RES),
                };
                out.push(OutEvent::rel(lo, amount.into()));
                out.push(OutEvent::rel(
                    hi,
                    i32::from(amount) * code::HI_RES_PER_DETENT,
                ));
                out.push(OutEvent::SYN);
            }
            Action::Device(cmd) => out.control(ControlOut::Device(cmd)),
            Action::Profile(p) => out.control(ControlOut::Profile(p)),
            Action::Exec(c) => out.control(ControlOut::Exec(c)),
            Action::Plugin { action, params, .. } => out.control(ControlOut::Action {
                action,
                params,
                delta,
            }),
        }
    }

    /// Tap a chord on request of the control plane (plugin `keys` built-in).
    pub fn emit_keys(&mut self, k: KeyChord, out: &mut Out) {
        self.fire(Action::Keys(k), 0, out);
    }
}

#[inline]
fn emit_keys(k: KeyChord, down: bool, out: &mut Out) {
    let n = (k.len as usize).min(k.codes.len());
    if down {
        for &c in &k.codes[..n] {
            out.push(OutEvent::key(c, true));
        }
    } else {
        for &c in k.codes[..n].iter().rev() {
            out.push(OutEvent::key(c, false));
        }
    }
}
