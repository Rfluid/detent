//! Immutable tables the engine reads per event. Built by `detent-config`'s compiler,
//! shared with the data plane through `ArcSwap`.

use crate::{BindingTable, ControlSlot, Edge, LayerId, LayerMap, ModMask};

/// Fixed control slots. Divertable CIDs without a name get slots from `FIRST_EXTRA`.
pub mod slot {
    use crate::ControlSlot;
    pub const LEFT: ControlSlot = ControlSlot(0);
    pub const RIGHT: ControlSlot = ControlSlot(1);
    pub const MIDDLE: ControlSlot = ControlSlot(2);
    pub const BACK: ControlSlot = ControlSlot(3);
    pub const FORWARD: ControlSlot = ControlSlot(4);
    pub const GESTURE: ControlSlot = ControlSlot(5);
    pub const SMARTSHIFT: ControlSlot = ControlSlot(6);
    pub const THUMB: ControlSlot = ControlSlot(7);
    pub const WHEEL: ControlSlot = ControlSlot(8);
    pub const THUMBWHEEL: ControlSlot = ControlSlot(9);
    pub const SWIPE: ControlSlot = ControlSlot(10);
    pub const FIRST_EXTRA: u8 = 11;
    pub const COUNT: usize = 24;
}

pub const MAX_CIDS: usize = 16;

/// How a slot's control reaches the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SlotInfo {
    /// HID++ CID if the control is diverted (0 = not diverted).
    pub cid: u16,
    /// evdev button code used when the control is not diverted (0 = none).
    pub evdev_button: u16,
    /// Button emitted when a diverted control has no binding (keeps its native function).
    pub native_button: u16,
    /// Modifier index if this control is a modifier.
    pub modifier: Option<u8>,
    /// Hold threshold in ms if a `.hold` edge is bound anywhere (0 = no hold edge).
    pub hold_ms: u16,
}

/// One binding set: the base plus any profile overrides already merged in.
#[derive(Debug, Clone)]
pub struct Tables {
    pub bindings: BindingTable,
    pub layers: LayerMap,
}

impl Tables {
    pub fn new(layers: usize) -> Self {
        Self {
            bindings: BindingTable::new(layers.max(1), slot::COUNT),
            layers: LayerMap::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Compiled {
    pub slots: [SlotInfo; slot::COUNT],
    /// Slot of each modifier index.
    pub modifier_slots: [Option<ControlSlot>; crate::modifiers::MAX_MODIFIERS],
    /// Modifiers whose layer needs evdev events (left/right/wheel/thumbwheel/...):
    /// pressing them grabs the mouse's evdev node until everything is released.
    pub grab_mods: ModMask,
    /// Modifiers whose layer has swipe bindings (diverted with raw XY).
    pub swipe_mods: ModMask,
    /// Swipe threshold in micrometres of physical movement.
    pub swipe_threshold_um: u32,
    /// Haptic waveform played when a swipe modifier's hold resolves: a swipe in any
    /// direction, a diagonal, or a tap (`[gesture] haptic`). `None` = no pulse.
    pub swipe_haptic: Option<u8>,
    /// `profiles[0]` is the base; others are base + overrides.
    pub profiles: Vec<Tables>,
}

impl Default for Compiled {
    fn default() -> Self {
        Self {
            slots: [SlotInfo::default(); slot::COUNT],
            modifier_slots: [None; crate::modifiers::MAX_MODIFIERS],
            grab_mods: ModMask::default(),
            swipe_mods: ModMask::default(),
            swipe_threshold_um: 6000,
            swipe_haptic: None,
            profiles: vec![Tables::new(1)],
        }
    }
}

impl Compiled {
    /// Slot for a diverted CID (linear scan over ≤ 24 entries).
    #[inline]
    pub fn slot_for_cid(&self, cid: u16) -> Option<ControlSlot> {
        if cid == 0 {
            return None;
        }
        self.slots
            .iter()
            .position(|s| s.cid == cid)
            .map(|i| ControlSlot(i as u8))
    }

    /// Slot for an evdev button that is *not* diverted.
    #[inline]
    pub fn slot_for_button(&self, code: u16) -> Option<ControlSlot> {
        self.slots
            .iter()
            .position(|s| s.evdev_button == code && s.cid == 0)
            .map(|i| ControlSlot(i as u8))
    }

    #[inline]
    pub fn tables(&self, profile: u8) -> &Tables {
        self.profiles
            .get(profile as usize)
            .or_else(|| self.profiles.first())
            .unwrap_or_else(|| unreachable_tables())
    }

    /// Every diverted CID, and whether it needs raw XY.
    pub fn diversion(&self) -> impl Iterator<Item = (u16, bool)> + '_ {
        self.slots.iter().filter(|s| s.cid != 0).map(|s| {
            let raw = s
                .modifier
                .is_some_and(|m| self.swipe_mods.0 & (1 << m) != 0);
            (s.cid, raw)
        })
    }

    /// Does any table bind `edge` on `slot` in any layer? (compile-time helper)
    pub fn binds(&self, slot: ControlSlot, edge: Edge) -> bool {
        self.profiles
            .iter()
            .any(|t| (0..=255u8).any(|l| !t.bindings.lookup(LayerId(l), slot, edge).is_none()))
    }
}

/// `profiles` is never empty (constructors guarantee one entry); this keeps `tables()`
/// panic-free without `unwrap`.
fn unreachable_tables() -> &'static Tables {
    static EMPTY: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    EMPTY.get_or_init(|| Tables::new(1))
}
