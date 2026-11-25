//! Input leases: a plugin temporarily owns some controls (e.g. the radial menu owns
//! pointer motion and buttons while open). See `docs/specs/plugin-bus.md#leases`.
//!
//! Safety invariants:
//! - every lease has a deadline; expired leases are revoked by the control plane tick
//! - plugin disconnect revokes all its leases
//! - the escape chord (left + right held 2 s) revokes every lease

use std::time::Instant;

use crate::ControlSlot;
use crate::action::PluginSlot;

pub const MAX_SLOTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Owner {
    #[default]
    Engine,
    Plugin(PluginSlot),
}

#[derive(Debug, Clone)]
pub struct LeaseTable {
    owner: [Owner; MAX_SLOTS],
    deadline: [Option<Instant>; MAX_SLOTS],
}

impl Default for LeaseTable {
    fn default() -> Self {
        Self {
            owner: [Owner::Engine; MAX_SLOTS],
            deadline: [None; MAX_SLOTS],
        }
    }
}

impl LeaseTable {
    /// Hot path: who receives events from this control right now.
    #[inline]
    pub fn owner(&self, slot: ControlSlot) -> Owner {
        self.owner
            .get(slot.0 as usize)
            .copied()
            .unwrap_or(Owner::Engine)
    }

    /// Grant fails if another plugin holds any requested slot.
    pub fn grant(&mut self, plugin: PluginSlot, slots: &[ControlSlot], until: Instant) -> bool {
        let free = slots.iter().all(|s| {
            matches!(self.owner(*s), Owner::Engine) || self.owner(*s) == Owner::Plugin(plugin)
        });
        if !free || slots.iter().any(|s| s.0 as usize >= MAX_SLOTS) {
            return false;
        }
        for s in slots {
            self.owner[s.0 as usize] = Owner::Plugin(plugin);
            self.deadline[s.0 as usize] = Some(until);
        }
        true
    }

    pub fn revoke_plugin(&mut self, plugin: PluginSlot) {
        for i in 0..MAX_SLOTS {
            if self.owner[i] == Owner::Plugin(plugin) {
                self.owner[i] = Owner::Engine;
                self.deadline[i] = None;
            }
        }
    }

    pub fn revoke_all(&mut self) {
        *self = Self::default();
    }

    /// Revoke expired leases; returns true if anything changed.
    pub fn expire(&mut self, now: Instant) -> bool {
        let mut changed = false;
        for i in 0..MAX_SLOTS {
            if self.deadline[i].is_some_and(|d| d <= now) {
                self.owner[i] = Owner::Engine;
                self.deadline[i] = None;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn lease_lifecycle() {
        let mut t = LeaseTable::default();
        let (radial, other) = (PluginSlot(1), PluginSlot(2));
        let now = Instant::now();
        let slots = [ControlSlot(3), ControlSlot(4)];
        assert!(t.grant(radial, &slots, now + Duration::from_secs(10)));
        assert_eq!(t.owner(ControlSlot(3)), Owner::Plugin(radial));
        assert!(!t.grant(other, &[ControlSlot(4)], now + Duration::from_secs(1)));
        assert!(t.expire(now + Duration::from_secs(11)));
        assert_eq!(t.owner(ControlSlot(3)), Owner::Engine);
        assert!(t.grant(other, &[ControlSlot(4)], now + Duration::from_secs(1)));
        t.revoke_plugin(other);
        assert_eq!(t.owner(ControlSlot(4)), Owner::Engine);
    }
}
