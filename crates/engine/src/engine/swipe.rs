//! Raw XY (HID++ 0x1B04 event 1) while a swipe modifier is held: accumulate the motion
//! and classify it into one of 8 swipe edges once it passes the physical thresholds.
//! **Hot path.**

use crate::Edge;
use crate::action::DeviceCmd;
use crate::compiled::{Compiled, SwipeThresholds, slot};
use crate::modifiers::MAX_MODIFIERS;
use crate::out::{ControlOut, Out};

use super::Engine;

/// Motion accumulated since a swipe modifier was pressed.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Swipe {
    dx: i32,
    dy: i32,
    frames: u32,
}

impl Engine {
    pub fn on_raw_xy(&mut self, cfg: &Compiled, dx: i16, dy: i16, out: &mut Out) {
        for m in 0..MAX_MODIFIERS {
            let bit = 1u8 << m;
            if self.mask.0 & bit == 0 || cfg.swipe_mods.0 & bit == 0 {
                continue;
            }
            let Some(s) = cfg.modifier_slots[m] else {
                continue;
            };
            if self.slots[s.0 as usize].used {
                continue;
            }
            let sw = &mut self.swipe[m];
            sw.frames += 1;
            if sw.frames == 1 {
                // The first raw-XY report after a press can be garbage (prior art).
                continue;
            }
            sw.dx += i32::from(dx);
            sw.dy += i32::from(dy);
            match swipe_direction(sw.dx, sw.dy, cfg.swipe, self.dpi) {
                SwipeDir::Pending => {}
                // Any direction, bound or not, decides the hold (no tap on release);
                // an unbound one looks up `Action::None` and fires nothing.
                SwipeDir::Edge(edge) => {
                    self.slots[s.0 as usize].used = true;
                    gesture_haptic(cfg, out);
                    let a = self.lookup(cfg, self.mask, slot::SWIPE, edge);
                    self.fire(a, 0, out);
                }
            }
        }
        out.syn();
    }
}

/// Pulse the configured gesture waveform (if any): the hold resolved.
#[inline]
pub(super) fn gesture_haptic(cfg: &Compiled, out: &mut Out) {
    if let Some(waveform) = cfg.swipe_haptic {
        out.control(ControlOut::Device(DeviceCmd::HapticPulse {
            waveform,
            intensity: 0,
        }));
    }
}

/// Result of classifying the movement accumulated during a hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDir {
    /// Not far enough yet for the sector the movement is in.
    Pending,
    /// Decided: one of the 8 swipe edges.
    Edge(Edge),
}

/// Classify the accumulated movement (device counts, screen coordinates: +y is down).
///
/// Each axis is scaled by its own threshold, which turns the threshold ellipse
/// `(x/x_um)² + (y/y_um)² = 1` into the unit circle. Once the movement leaves it:
/// - one scaled axis ≥ 1.3× the other → up/down/left/right;
/// - otherwise → a diagonal (at 45° in the scaled plane: `x_um/√2` across, `y_um/√2`
///   up or down).
///
/// With `x_um == y_um` this is a plain distance threshold on the raw movement.
#[inline]
pub fn swipe_direction(dx: i32, dy: i32, t: SwipeThresholds, dpi: u16) -> SwipeDir {
    let (ax, ay) = (i128::from(dx).abs(), i128::from(dy).abs());
    let (tx, ty) = (i128::from(t.x_um.max(1)), i128::from(t.y_um.max(1)));
    // Multiply through by tx·ty instead of dividing: nx/(tx·ty) = ax/tx.
    let (nx, ny) = (ax * ty, ay * tx);
    // counts * 25400 / dpi = micrometres; compare squared to avoid sqrt.
    let r = tx * ty * i128::from(dpi.max(1));
    if (nx * nx + ny * ny) * 25_400 * 25_400 < r * r {
        return SwipeDir::Pending;
    }
    SwipeDir::Edge(if nx * 10 >= ny * 13 {
        if dx > 0 {
            Edge::SwipeRight
        } else {
            Edge::SwipeLeft
        }
    } else if ny * 10 >= nx * 13 {
        if dy > 0 {
            Edge::SwipeDown
        } else {
            Edge::SwipeUp
        }
    } else {
        match (dx > 0, dy > 0) {
            (true, true) => Edge::SwipeDownRight,
            (true, false) => Edge::SwipeUpRight,
            (false, true) => Edge::SwipeDownLeft,
            (false, false) => Edge::SwipeUpLeft,
        }
    })
}
