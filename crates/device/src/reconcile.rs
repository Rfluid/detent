//! Reconciler: desired → device → read back → reported → publish diff.
//!
//! Triggers (docs/architecture/device-state.md):
//! - config reload or live edit (IPC / plugin `SetState`)
//! - link up after sleep, reconnect or host switch (diversion is volatile)
//! - periodic verify (cheap reads, catches drift)
//!
//! Runs on the control plane. Requests go through the data plane's request ring,
//! so the data plane stays the only owner of the hidraw fd.

use crate::DeviceState;

/// What the reconciler should do next for a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    Nothing,
    /// Re-read everything, then apply all desired sections (after reconnect).
    FullResync,
    /// Apply only sections that are out of sync.
    ApplyDirty,
}

pub fn plan(state: &DeviceState) -> Plan {
    use crate::state::Link;
    if state.link == Link::Disconnected {
        return Plan::Nothing;
    }
    let dirty = !state.dpi.in_sync()
        || !state.smartshift.in_sync()
        || !state.wheel.in_sync()
        || !state.thumbwheel.in_sync()
        || !state.controls.in_sync()
        || !state.haptics.in_sync();
    if dirty {
        Plan::ApplyDirty
    } else {
        Plan::Nothing
    }
}
