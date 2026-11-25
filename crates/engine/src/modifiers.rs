//! Held modifiers → active layer. See `docs/architecture/bindings.md`.

use crate::LayerId;

pub const MAX_MODIFIERS: usize = 8;

/// Bitset of currently held modifier controls (bit i = modifier i).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModMask(pub u8);

impl ModMask {
    #[inline]
    pub fn press(&mut self, modifier: u8) {
        self.0 |= 1 << modifier;
    }

    #[inline]
    pub fn release(&mut self, modifier: u8) {
        self.0 &= !(1 << modifier);
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Precomputed `mask → layer` map (256 entries, 256 bytes). Unmapped masks fall back to BASE.
#[derive(Debug, Clone)]
pub struct LayerMap {
    by_mask: [LayerId; 256],
}

impl Default for LayerMap {
    fn default() -> Self {
        Self {
            by_mask: [LayerId::BASE; 256],
        }
    }
}

impl LayerMap {
    pub fn set(&mut self, mask: ModMask, layer: LayerId) {
        self.by_mask[mask.0 as usize] = layer;
    }

    #[inline]
    pub fn layer(&self, mask: ModMask) -> LayerId {
        self.by_mask[mask.0 as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_selects_layer() {
        let (gesture, thumb) = (0, 1);
        let mut map = LayerMap::default();
        let mut m = ModMask::default();
        m.press(gesture);
        map.set(m, LayerId(1));
        m.press(thumb);
        map.set(m, LayerId(2));

        let mut held = ModMask::default();
        assert_eq!(map.layer(held), LayerId::BASE);
        held.press(gesture);
        assert_eq!(map.layer(held), LayerId(1));
        held.press(thumb);
        assert_eq!(map.layer(held), LayerId(2));
        held.release(gesture);
        assert_eq!(map.layer(held), LayerId::BASE); // thumb alone: unmapped
    }
}
