//! Raw XY (HID++ 0x1B04 event 1) while a swipe modifier is held: accumulate the motion
//! and classify it into a swipe edge once it passes the physical threshold. **Hot path.**

use crate::Edge;
use crate::action::DeviceCmd;
use crate::compiled::{Compiled, slot};
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
            match swipe_direction(sw.dx, sw.dy, cfg.swipe_threshold_um, self.dpi) {
                SwipeDir::Pending => {}
                // Past the threshold but no dominant axis: a decided move that does
                // nothing (and is no tap on release).
                SwipeDir::Diagonal => {
                    self.slots[s.0 as usize].used = true;
                    gesture_haptic(cfg, out);
                }
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
    /// Not yet `threshold_um` of physical distance.
    Pending,
    /// Past the threshold with no axis dominating by 1.3×: decided, fires nothing.
    Diagonal,
    /// Past the threshold along a dominant axis.
    Edge(Edge),
}

/// Classify the accumulated movement once it passes `threshold_um` of physical distance:
/// a swipe edge when one axis dominates the other by ≥ 1.3×, else a diagonal.
/// Screen coordinates: +y is down.
#[inline]
pub fn swipe_direction(dx: i32, dy: i32, threshold_um: u32, dpi: u16) -> SwipeDir {
    let (ax, ay) = (i64::from(dx).abs(), i64::from(dy).abs());
    // counts * 25400 / dpi = micrometres; compare squared to avoid sqrt.
    let dist2_um = (ax * ax + ay * ay) * 25_400 * 25_400;
    let thr = i64::from(threshold_um) * i64::from(dpi.max(1));
    if dist2_um < thr * thr {
        return SwipeDir::Pending;
    }
    if ax * 10 >= ay * 13 {
        SwipeDir::Edge(if dx > 0 {
            Edge::SwipeRight
        } else {
            Edge::SwipeLeft
        })
    } else if ay * 10 >= ax * 13 {
        SwipeDir::Edge(if dy > 0 {
            Edge::SwipeDown
        } else {
            Edge::SwipeUp
        })
    } else {
        SwipeDir::Diagonal
    }
}
