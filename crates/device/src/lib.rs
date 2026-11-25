//! The device model. See `docs/architecture/device-state.md`.
//!
//! - [`state::DeviceState`]: typed, serializable view of everything about a device
//! - [`state::Section`]: per-feature desired + reported values with a generation counter
//! - [`reconcile`]: brings reported → desired; re-runs on reconnect
//! - [`requester::DirectRequester`]: blocking, serialized request/response over a transport
//! - [`receiver`]: what is paired to a Bolt receiver, at which slot (HID++ 1.0 register reads)

pub mod receiver;
pub mod reconcile;
pub mod requester;
pub mod state;

pub use requester::DirectRequester;

pub use state::{DeviceId, DeviceState, Section};
