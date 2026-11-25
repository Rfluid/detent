//! HID++ 2.0 wire format.
//!
//! This crate is on the **data plane hot path**: everything here is `no_std`,
//! allocation-free and `Copy`. Frames live on the stack.
//!
//! Spec notes: `docs/specs/hidpp.md`.

#![cfg_attr(not(test), no_std)]

pub mod classify;
pub mod error;
pub mod feature_id;
pub mod frame;

pub use classify::{Kind, classify};
pub use error::ProtoError;
pub use feature_id::FeatureId;
pub use frame::{DeviceIndex, Frame, MAX_FRAME_LEN, ReportId, SoftwareId};
