//! The per-device event state machine: diverted buttons, raw XY swipes, evdev events
//! during a grab, hold timers. **Hot path**: no allocation, no locks, no syscalls; all
//! output goes into an [`Out`] the device thread flushes.
//!
//! Semantics (docs/architecture/bindings.md):
//! - press on a modifier starts "undecided"; another press → chord; raw XY past the
//!   threshold → swipe (once per hold); hold timer → hold; release while undecided → tap
//! - lookup order: current layer, then base layer, then the control's native behaviour
//!   (diverted: re-emit its button; evdev during a grab: pass the event through)
//! - `keys`/`button` bound to a press are held while the control is held
//!
//! Layout: this file holds the state, layer lookup, diverted buttons, leases, timers and
//! grab policy; `controls` press/release and action output; `evdev` the physical mouse's
//! events; `swipe` raw XY classification.

mod controls;
mod evdev;
mod swipe;

use std::time::{Duration, Instant};

use crate::action::{Action, KeyChord};
use crate::compiled::{Compiled, slot};
use crate::modifiers::MAX_MODIFIERS;
use crate::out::{ControlOut, Out};
use crate::{ControlSlot, Edge, LayerId, ModMask};

use swipe::Swipe;
pub use swipe::{SwipeDir, swipe_direction};

/// What a pressed control is currently emitting, so its release can undo it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Held {
    #[default]
    Nothing,
    Button(u16),
    Keys(KeyChord),
    /// Press went to the desktop unchanged (evdev pass-through during a grab).
    PassThrough,
    /// Press was swallowed (bound action, or chord); swallow the release too.
    Swallowed,
}

#[derive(Debug, Clone, Copy, Default)]
struct SlotState {
    down: bool,
    /// Decided as chord / swipe / hold: no tap on release.
    used: bool,
    hold_fired: bool,
    pressed_at: Option<Instant>,
    held: Held,
}

/// Grab transition the device thread should perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrabChange {
    Grab,
    Ungrab,
}

/// What a plugin currently leases (one lease at a time).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Lease {
    pub pointer: bool,
    pub buttons: bool,
    /// Bit per control slot (diverted controls routed to the plugin).
    pub controls: u32,
    pub until: Option<Instant>,
}

impl Lease {
    pub fn is_active(&self) -> bool {
        self.pointer || self.buttons || self.controls != 0
    }
}

/// Left + right held this long revokes every lease.
pub const ESCAPE_CHORD: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct Engine {
    lease: Lease,
    lease_dx: i32,
    lease_dy: i32,
    escape_since: Option<Instant>,
    held_cids: [u16; 4],
    mask: ModMask,
    slots: [SlotState; slot::COUNT],
    swipe: [Swipe; MAX_MODIFIERS],
    profile: u8,
    dpi: u16,
    /// Physical buttons held on the evdev node (bit n = BTN_LEFT + n).
    phys_buttons: u16,
    grabbed: bool,
}

impl Default for Engine {
    fn default() -> Self {
        Self {
            lease: Lease::default(),
            lease_dx: 0,
            lease_dy: 0,
            escape_since: None,
            held_cids: [0; 4],
            mask: ModMask::default(),
            slots: [SlotState::default(); slot::COUNT],
            swipe: [Swipe::default(); MAX_MODIFIERS],
            profile: 0,
            dpi: 1000,
            phys_buttons: 0,
            grabbed: false,
        }
    }
}

impl Engine {
    pub fn set_dpi(&mut self, dpi: u16) {
        self.dpi = dpi.max(1);
    }

    pub fn set_profile(&mut self, profile: u8) {
        self.profile = profile;
    }

    pub fn profile(&self) -> u8 {
        self.profile
    }

    pub fn mask(&self) -> ModMask {
        self.mask
    }

    pub fn is_grabbed(&self) -> bool {
        self.grabbed
    }

    #[inline]
    fn layer(&self, cfg: &Compiled, mask: ModMask) -> LayerId {
        cfg.tables(self.profile).layers.layer(mask)
    }

    /// Layer lookup with base fallback.
    #[inline]
    fn lookup(&self, cfg: &Compiled, mask: ModMask, s: ControlSlot, edge: Edge) -> Action {
        let t = cfg.tables(self.profile);
        let layer = t.layers.layer(mask);
        let a = t.bindings.lookup(layer, s, edge);
        if a.is_none() && layer != LayerId::BASE {
            t.bindings.lookup(LayerId::BASE, s, edge)
        } else {
            a
        }
    }

    /// Does `slot` have a tap/hold edge bound for this mask (then native output is
    /// deferred to release instead of held through)?
    #[inline]
    fn has_deferred_edges(&self, cfg: &Compiled, mask: ModMask, s: ControlSlot) -> bool {
        !self.lookup(cfg, mask, s, Edge::Tap).is_none()
            || (cfg.slots[s.0 as usize].hold_ms > 0
                && !self.lookup(cfg, mask, s, Edge::Hold).is_none())
    }

    // ── diverted buttons (HID++ 0x1B04 event 0) ────────────────────────────────

    /// The device reports the set of up to 4 diverted CIDs currently held.
    pub fn on_diverted_buttons(
        &mut self,
        cfg: &Compiled,
        cids: [u16; 4],
        now: Instant,
        out: &mut Out,
    ) {
        let prev = self.held_cids;
        self.held_cids = cids;
        for &c in prev.iter().filter(|&&c| c != 0 && !cids.contains(&c)) {
            if let Some(s) = cfg.slot_for_cid(c) {
                if self.leased(s) {
                    // A press from before the lease must not fire its release/tap now.
                    self.slots[s.0 as usize] = SlotState::default();
                    if let Some(m) = cfg.slots[s.0 as usize].modifier {
                        self.mask.release(m);
                    }
                    out.control(ControlOut::LeaseButton {
                        code: 0,
                        slot: s.0,
                        down: false,
                    });
                } else {
                    self.release(cfg, s, now, out);
                }
            }
        }
        for &c in cids.iter().filter(|&&c| c != 0 && !prev.contains(&c)) {
            if let Some(s) = cfg.slot_for_cid(c) {
                if self.leased(s) {
                    out.control(ControlOut::LeaseButton {
                        code: 0,
                        slot: s.0,
                        down: true,
                    });
                } else {
                    self.press(cfg, s, now, out, false);
                }
            }
        }
        out.syn();
    }

    // ── leases ─────────────────────────────────────────────────────────────────

    #[inline]
    fn leased(&self, s: ControlSlot) -> bool {
        self.lease.controls & (1u32 << (s.0 as u32 & 31)) != 0
    }

    pub fn lease(&self) -> Lease {
        self.lease
    }

    pub fn set_lease(&mut self, lease: Lease) {
        self.lease = lease;
        self.lease_dx = 0;
        self.lease_dy = 0;
    }

    fn end_lease(&mut self, escape: bool, out: &mut Out) {
        if self.lease.is_active() {
            self.lease = Lease::default();
            self.escape_since = None;
            out.control(ControlOut::LeaseEnded { escape });
        }
    }

    // ── timers ─────────────────────────────────────────────────────────────────

    /// Earliest pending hold deadline (the device thread's poll timeout).
    pub fn next_deadline(&self, cfg: &Compiled) -> Option<Instant> {
        let mut best: Option<Instant> = None;
        if self.lease.is_active() {
            best = self.lease.until;
            if let Some(t) = self.escape_since {
                let d = t + ESCAPE_CHORD;
                best = Some(best.map_or(d, |b| b.min(d)));
            }
        }
        for (i, st) in self.slots.iter().enumerate() {
            let hold = cfg.slots[i].hold_ms;
            if !st.down || st.used || st.hold_fired || hold == 0 {
                continue;
            }
            if let Some(t) = st.pressed_at {
                let d = t + Duration::from_millis(hold.into());
                best = Some(best.map_or(d, |b| b.min(d)));
            }
        }
        best
    }

    pub fn tick(&mut self, cfg: &Compiled, now: Instant, out: &mut Out) {
        if self.lease.is_active() {
            if self.escape_since.is_some_and(|t| now >= t + ESCAPE_CHORD) {
                self.end_lease(true, out);
            } else if self.lease.until.is_some_and(|t| now >= t) {
                self.end_lease(false, out);
            }
        }
        for i in 0..slot::COUNT {
            let hold = cfg.slots[i].hold_ms;
            let st = self.slots[i];
            if !st.down || st.used || st.hold_fired || hold == 0 {
                continue;
            }
            let due = st
                .pressed_at
                .is_some_and(|t| now >= t + Duration::from_millis(hold.into()));
            if !due {
                continue;
            }
            let s = ControlSlot(i as u8);
            let mask = self.mask_without(cfg, s);
            let a = self.lookup(cfg, mask, s, Edge::Hold);
            self.slots[i].hold_fired = true;
            self.slots[i].used = true;
            if !a.is_none() {
                self.fire(a, 0, out);
            }
        }
        out.syn();
    }

    // ── grab policy ────────────────────────────────────────────────────────────

    /// Grab while a modifier whose layer needs evdev is held; ungrab once it's released
    /// and no physical button is down (so no press/release pair is ever split between
    /// the real and the virtual device). Never grab while a button is down.
    pub fn grab_change(&self, cfg: &Compiled) -> Option<GrabChange> {
        let want = self.mask.0 & cfg.grab_mods.0 != 0 || self.lease.pointer || self.lease.buttons;
        match (self.grabbed, want, self.phys_buttons == 0) {
            (false, true, true) => Some(GrabChange::Grab),
            (true, false, true) => Some(GrabChange::Ungrab),
            _ => None,
        }
    }

    pub fn set_grabbed(&mut self, grabbed: bool) {
        self.grabbed = grabbed;
    }

    /// Release everything this engine is holding down on the virtual device
    /// (shutdown, disconnect, config swap).
    pub fn release_all(&mut self, out: &mut Out) {
        for s in &mut self.slots {
            Self::undo(s.held, out);
            *s = SlotState::default();
        }
        self.mask = ModMask::default();
        self.held_cids = [0; 4];
        self.swipe = [Swipe::default(); MAX_MODIFIERS];
        out.syn();
    }

    /// Current layer (for diagnostics and tests).
    pub fn current_layer(&self, cfg: &Compiled) -> LayerId {
        self.layer(cfg, self.mask)
    }
}
