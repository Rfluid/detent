//! The data plane. **Hot path rules** (enforced in review, see `.agent/skills/hot-path-change.md`):
//!
//! 1. No heap allocation per event. No `Vec::push`, `String`, `Box::new`, `format!`.
//! 2. No locks, no blocking channels, no async. Config arrives via `ArcSwap` load.
//! 3. No syscalls except: hidraw read/write, evdev grab/read, uinput write, non-blocking
//!    socket send to plugins, eventfd.
//! 4. Every per-event operation is O(1) or bounded by a small constant.
//! 5. Anything slow (exec, D-Bus, logging at info+) is handed to the control plane via a ring.
//!
//! The engine itself is pure: [`engine::Engine`] turns events into an [`out::Out`]; the
//! device thread in `detentd` performs the I/O.

pub mod action;
pub mod bindings;
pub mod compiled;
pub mod engine;
pub mod leases;
pub mod modifiers;
pub mod out;

pub use action::Action;
pub use bindings::{BindingTable, ControlSlot, Edge, LayerId};
pub use compiled::{Compiled, SlotInfo, Tables, slot};
pub use engine::{Engine, GrabChange};
pub use leases::{LeaseTable, Owner};
pub use modifiers::{LayerMap, ModMask};
pub use out::{ControlOut, Out, OutEvent};
