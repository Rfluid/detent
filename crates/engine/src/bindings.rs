//! The compiled binding table: `table[layer][slot][edge] → Action`, one flat array.

use crate::Action;

/// Dense index assigned at config-compile time to each bindable control
/// (diverted CIDs, wheel, thumbwheel, gesture directions, evdev buttons during a grab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlSlot(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerId(pub u8);

impl LayerId {
    pub const BASE: Self = Self(0);
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Press = 0,
    Release,
    Tap,
    Hold,
    RotatePos,
    RotateNeg,
    SwipeUp,
    SwipeDown,
    SwipeLeft,
    SwipeRight,
}

impl Edge {
    pub const COUNT: usize = 10;
}

/// Immutable after compile; shared with the data plane through `ArcSwap`.
#[derive(Debug, Clone)]
pub struct BindingTable {
    slots: usize,
    layers: usize,
    actions: Box<[Action]>,
}

impl BindingTable {
    pub fn new(layers: usize, slots: usize) -> Self {
        Self {
            slots,
            layers,
            actions: vec![Action::None; layers * slots * Edge::COUNT].into(),
        }
    }

    #[inline]
    fn index(&self, layer: LayerId, slot: ControlSlot, edge: Edge) -> Option<usize> {
        let (l, s) = (layer.0 as usize, slot.0 as usize);
        (l < self.layers && s < self.slots)
            .then(|| (l * self.slots + s) * Edge::COUNT + edge as usize)
    }

    /// Hot path. O(1), no allocation.
    #[inline]
    pub fn lookup(&self, layer: LayerId, slot: ControlSlot, edge: Edge) -> Action {
        self.index(layer, slot, edge)
            .map_or(Action::None, |i| self.actions[i])
    }

    /// Compile time only.
    pub fn set(&mut self, layer: LayerId, slot: ControlSlot, edge: Edge, action: Action) -> bool {
        match self.index(layer, slot, edge) {
            Some(i) => {
                self.actions[i] = action;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{CmdId, DeviceCmd};

    #[test]
    fn layered_lookup() {
        let mut t = BindingTable::new(2, 4);
        let left = ControlSlot(1);
        t.set(LayerId::BASE, left, Edge::Press, Action::Button(0x110));
        t.set(LayerId(1), left, Edge::Press, Action::Exec(CmdId(7)));
        assert_eq!(
            t.lookup(LayerId::BASE, left, Edge::Press),
            Action::Button(0x110)
        );
        assert_eq!(
            t.lookup(LayerId(1), left, Edge::Press),
            Action::Exec(CmdId(7))
        );
        assert_eq!(t.lookup(LayerId(1), left, Edge::Release), Action::None);
        // Out of range is a no-op, never a panic.
        assert_eq!(t.lookup(LayerId(9), left, Edge::Press), Action::None);
        assert!(!t.set(
            LayerId(9),
            left,
            Edge::Press,
            Action::Device(DeviceCmd::CycleDpi)
        ));
    }
}
