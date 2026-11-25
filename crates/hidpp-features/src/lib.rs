//! One module per HID++ feature. Each implements [`Feature`].
//!
//! Adding a feature: follow `.agent/skills/add-hidpp-feature.md`.

pub mod adjustable_dpi;
pub mod cid;
pub mod device_info;
pub mod feature_set;
pub mod haptics;
pub mod hosts;
pub mod reprog_controls;
pub mod root;
pub mod smartshift;
pub mod table;
pub mod unified_battery;
pub mod wheel;

use detent_hidpp_proto::{FeatureId, Frame};
use serde::{Serialize, de::DeserializeOwned};

pub use table::FeatureTable;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("device returned HID++ error code 0x{0:02X}")]
    Device(u8),
    #[error("request timed out")]
    Timeout,
    #[error("feature {0:?} not supported by this device")]
    Unsupported(FeatureId),
    #[error("malformed response")]
    Malformed,
    #[error("value out of range: {0}")]
    OutOfRange(String),
    #[error("transport: {0}")]
    Io(#[from] std::io::Error),
}

/// Serialized request/response channel to one device (implemented in `detent-device`).
pub trait Requester {
    /// Resolve a feature ID to this device's feature index (cached).
    fn feature_index(&self, id: FeatureId) -> Option<u8>;

    /// Send a request and wait for its matching response.
    fn request(
        &mut self,
        feature_index: u8,
        function: u8,
        params: &[u8],
    ) -> Result<Frame, RequestError>;

    /// Send a request that gets no reply (ChangeHost setCurrentHost: the link drops at
    /// once). Written once, never retried.
    fn send(&mut self, feature_index: u8, function: u8, params: &[u8]) -> Result<(), RequestError>;
}

/// Something a feature can report from an unsolicited notification.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Change {
    StateChanged,
}

/// Editable-field description, used by `detentctl`, plugins and a future GUI
/// to build editors without per-feature code.
#[derive(Debug, Clone, Serialize)]
pub struct FieldSchema {
    pub path: &'static str,
    pub kind: FieldKind,
    pub doc: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub enum FieldKind {
    Bool,
    Range { min: i64, max: i64, step: i64 },
    Choice(&'static [&'static str]),
    ReadOnly,
}

pub trait Feature {
    const ID: FeatureId;
    /// The user-visible, editable shape of this feature's state.
    type State: Serialize + DeserializeOwned + PartialEq + Clone;

    fn read(dev: &mut dyn Requester) -> Result<Self::State, RequestError>;

    /// Bring the device from `current` to `desired`. Only send what differs.
    fn apply(
        dev: &mut dyn Requester,
        desired: &Self::State,
        current: &Self::State,
    ) -> Result<(), RequestError>;

    /// Update state from a notification for this feature. Called on the control plane.
    fn on_notification(_frame: &Frame, _state: &mut Self::State) -> Option<Change> {
        None
    }

    fn schema() -> &'static [FieldSchema];
}
