//! evdev events from the physical mouse: physical button tracking (escape chord, grab
//! policy) always; while grabbed, lease routing, binding lookup and wheel rotation.
//! **Hot path.**

use std::time::Instant;

use crate::action::Action;
use crate::compiled::{Compiled, slot};
use crate::out::{ControlOut, Out, OutEvent, code};
use crate::{ControlSlot, Edge};

use super::Engine;

const BTN_BITS: core::ops::RangeInclusive<u16> = code::BTN_LEFT..=code::BTN_TASK;

impl Engine {
    /// Feed one evdev event. Tracks physical button state always; translates and
    /// passes through only while grabbed.
    pub fn on_evdev(
        &mut self,
        cfg: &Compiled,
        ty: u16,
        code_: u16,
        value: i32,
        now: Instant,
        out: &mut Out,
    ) {
        if ty == code::EV_KEY && BTN_BITS.contains(&code_) {
            let bit = 1u16 << (code_ - code::BTN_LEFT);
            match value {
                0 => self.phys_buttons &= !bit,
                1 => self.phys_buttons |= bit,
                _ => {} // autorepeat
            }
            const BOTH: u16 = 0b11; // BTN_LEFT | BTN_RIGHT
            self.escape_since = if self.phys_buttons & BOTH == BOTH {
                self.escape_since.or(Some(now))
            } else {
                None
            };
        }
        if !self.grabbed {
            return;
        }
        // Leased input goes to the plugin, not the desktop.
        if self.lease.pointer
            && ty == code::EV_REL
            && (code_ == code::REL_X || code_ == code::REL_Y)
        {
            if code_ == code::REL_X {
                self.lease_dx += value;
            } else {
                self.lease_dy += value;
            }
            return;
        }
        if self.lease.pointer
            && ty == code::EV_SYN
            && code_ == code::SYN_REPORT
            && (self.lease_dx != 0 || self.lease_dy != 0)
        {
            let c = |v: i32| v.clamp(i16::MIN.into(), i16::MAX.into()) as i16;
            out.control(ControlOut::LeaseMotion {
                dx: c(self.lease_dx),
                dy: c(self.lease_dy),
            });
            self.lease_dx = 0;
            self.lease_dy = 0;
        }
        if self.lease.buttons && ty == code::EV_KEY && BTN_BITS.contains(&code_) {
            if value <= 1 {
                out.control(ControlOut::LeaseButton {
                    code: code_,
                    slot: 0,
                    down: value == 1,
                });
            }
            return;
        }
        match (ty, code_) {
            (code::EV_SYN, code::SYN_REPORT) => out.syn(),
            (code::EV_SYN, _) => {}
            (code::EV_KEY, c) => match cfg.slot_for_button(c) {
                Some(s) if value == 1 => self.press(cfg, s, now, out, true),
                Some(s) if value == 0 => self.release(cfg, s, now, out),
                Some(_) => {}
                None => out.push(OutEvent { ty, code: c, value }),
            },
            (code::EV_REL, code::REL_WHEEL) => self.rotate(cfg, slot::WHEEL, ty, code_, value, out),
            (code::EV_REL, code::REL_HWHEEL) => {
                self.rotate(cfg, slot::THUMBWHEEL, ty, code_, value, out)
            }
            (code::EV_REL, code::REL_WHEEL_HI_RES) => {
                self.rotate_hires(cfg, slot::WHEEL, ty, code_, value, out)
            }
            (code::EV_REL, code::REL_HWHEEL_HI_RES) => {
                self.rotate_hires(cfg, slot::THUMBWHEEL, ty, code_, value, out)
            }
            _ => out.push(OutEvent {
                ty,
                code: code_,
                value,
            }),
        }
    }

    /// The kernel dropped events (SYN_DROPPED): reset tracked button state to `held`.
    pub fn resync_buttons(&mut self, held: u16) {
        self.phys_buttons = held;
    }

    #[inline]
    fn rotation_action(&self, cfg: &Compiled, s: ControlSlot, value: i32) -> Action {
        let edge = if value > 0 {
            Edge::RotatePos
        } else {
            Edge::RotateNeg
        };
        self.lookup(cfg, self.mask, s, edge)
    }

    fn rotate(
        &mut self,
        cfg: &Compiled,
        s: ControlSlot,
        ty: u16,
        c: u16,
        value: i32,
        out: &mut Out,
    ) {
        let a = self.rotation_action(cfg, s, value);
        if a.is_none() {
            out.push(OutEvent { ty, code: c, value });
            return;
        }
        self.mark_chord(s);
        let delta = value.clamp(i16::MIN.into(), i16::MAX.into()) as i16;
        match a {
            // Discrete actions repeat once per detent; named actions get the delta.
            Action::Plugin { .. } => self.fire(a, delta, out),
            _ => {
                for _ in 0..value.unsigned_abs().min(8) {
                    self.fire(a, delta.signum(), out);
                }
            }
        }
    }

    fn rotate_hires(
        &mut self,
        cfg: &Compiled,
        s: ControlSlot,
        ty: u16,
        c: u16,
        value: i32,
        out: &mut Out,
    ) {
        if self.rotation_action(cfg, s, value).is_none() {
            out.push(OutEvent { ty, code: c, value });
        }
    }
}
