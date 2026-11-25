//! Plugin bus protocol. Normative spec: `docs/specs/plugin-bus.md`.
//!
//! Transport: Unix `SOCK_SEQPACKET` at `$XDG_RUNTIME_DIR/detent/bus.sock`
//! (one message per packet, no framing). Codec: postcard by default; JSON if the
//! plugin asks for it in `Hello`. Every message type here is part of the public API:
//! changes need a `PROTOCOL_VERSION` bump and a docs update.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;
pub const SOCKET_NAME: &str = "bus.sock";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    Postcard,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// Receive device events (`Subscribe`).
    EventsSubscribe,
    /// Publish context keys used for profile matching (`SetContext`).
    ContextPublish,
    /// Lease pointer motion (cursor freezes, deltas go to the plugin).
    LeasePointer,
    /// Lease buttons (uses evdev grab for non-divertable buttons).
    LeaseButtons,
    /// Send haptic pulses.
    DeviceHaptics,
    /// Send other device commands (DPI, SmartShift...).
    DeviceControl,
    /// Edit device state through the reconciler (`SetState`).
    StateWrite,
    /// Synthesize input through detentd's uinput device.
    InputEmit,
    /// Call other actions (built-ins and other plugins').
    ActionsInvoke,
}

/// An action a plugin adds to the shared namespace, e.g. `radial.open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionDecl {
    /// Local name; the full name is `<plugin>.<name>`.
    pub name: String,
    pub doc: String,
    /// Continuous actions receive rotation/motion deltas (e.g. `kde.volume`).
    pub continuous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub protocol: u16,
    pub codec: Codec,
    pub actions: Vec<ActionDecl>,
    pub caps: Vec<Capability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Topic {
    Button,
    Gesture,
    Wheel,
    Thumbwheel,
    Motion,
    Battery,
    StateDiff,
    Link,
    Context,
}

/// Controls a plugin can lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseTarget {
    Pointer,
    Buttons,
    Wheel,
    Thumbwheel,
    /// A named control, e.g. "thumb".
    Control(ControlName),
}

/// Short control name, fixed-size to keep messages small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlName(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceCommand {
    HapticPulse { waveform: u8, intensity: u8 },
    SetDpi(u16),
    CycleDpi,
    ToggleSmartShift,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonEdge {
    Press,
    Release,
}

/// Device → plugin events. Timestamps are monotonic microseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Button {
        control: String,
        edge: ButtonEdge,
        ts_us: u64,
    },
    Motion {
        dx: i16,
        dy: i16,
        ts_us: u64,
    },
    Rotate {
        control: String,
        delta: i16,
        ts_us: u64,
    },
    Gesture {
        direction: String,
        ts_us: u64,
    },
    Battery {
        percent: Option<u8>,
        charging: bool,
    },
    Link {
        connected: bool,
    },
    Context {
        key: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginToDaemon {
    Hello(Manifest),
    Subscribe(Vec<Topic>),
    LeaseRequest {
        id: u32,
        targets: Vec<LeaseTarget>,
        max_ms: u32,
    },
    LeaseRelease {
        id: u32,
    },
    Device(DeviceCommand),
    /// Invoke any action by full name, with TOML-like params encoded as JSON text.
    Invoke {
        action: String,
        params_json: String,
    },
    SetContext {
        key: String,
        value: String,
    },
    SetState {
        path: String,
        value_json: String,
    },
    Pong {
        nonce: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DaemonToPlugin {
    Welcome {
        protocol: u16,
        granted: Vec<Capability>,
    },
    /// The plugin's `[plugins.<name>]` settings as JSON, re-sent on every config reload.
    Config {
        settings_json: String,
    },
    ActionInvoked {
        /// Local action name (without the `<plugin>.` prefix).
        action: String,
        params_json: String,
        trigger: String,
        /// Rotation delta for continuous actions (wheel detents); 0 for discrete ones.
        delta: i16,
        ts_us: u64,
    },
    Event(Event),
    StateDiff {
        path: String,
        value_json: String,
        generation: u64,
    },
    LeaseGranted {
        id: u32,
    },
    LeaseDenied {
        id: u32,
        reason: String,
    },
    LeaseRevoked {
        id: u32,
        reason: LeaseRevokeReason,
    },
    Ping {
        nonce: u64,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseRevokeReason {
    Expired,
    EscapeChord,
    ConfigReload,
    DeviceDisconnected,
    /// The plugin disconnected or stopped answering pings.
    PluginGone,
}

/// Largest packet either side sends.
pub const MAX_PACKET: usize = 64 * 1024;

/// Built-in actions any plugin with `ActionsInvoke` can call (no plugin prefix).
pub mod builtin {
    /// `{ "cmd": "spectacle -r" }`
    pub const EXEC: &str = "exec";
    /// `{ "keys": "Ctrl+C" }` (needs `InputEmit`)
    pub const KEYS: &str = "keys";
    /// Easy-Switch: `{ "to": 2 }` (1-based), `{ "to": "next" }` or `{ "to": "prev" }`.
    /// The mouse disconnects from this computer. Needs `DeviceControl`.
    pub const DEVICE_HOST: &str = "device.host";
}

/// Encoding of bus packets. One message per `SOCK_SEQPACKET` packet.
pub mod codec {
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    use crate::Codec;

    #[derive(Debug)]
    pub struct CodecError(pub String);

    impl core::fmt::Display for CodecError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl std::error::Error for CodecError {}

    pub fn encode<T: Serialize>(codec: Codec, msg: &T) -> Result<Vec<u8>, CodecError> {
        match codec {
            Codec::Postcard => postcard::to_allocvec(msg).map_err(|e| CodecError(e.to_string())),
            Codec::Json => serde_json::to_vec(msg).map_err(|e| CodecError(e.to_string())),
        }
    }

    pub fn decode<T: DeserializeOwned>(codec: Codec, bytes: &[u8]) -> Result<T, CodecError> {
        match codec {
            Codec::Postcard => postcard::from_bytes(bytes).map_err(|e| CodecError(e.to_string())),
            Codec::Json => serde_json::from_slice(bytes).map_err(|e| CodecError(e.to_string())),
        }
    }

    /// The first packet (`Hello`) is self-describing: JSON if it starts with `{`,
    /// otherwise postcard.
    pub fn sniff(first_packet: &[u8]) -> Codec {
        if first_packet.first() == Some(&b'{') {
            Codec::Json
        } else {
            Codec::Postcard
        }
    }
}

/// `$XDG_RUNTIME_DIR/detent/bus.sock` unless `$DETENT_BUS` says otherwise.
pub fn socket_path() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("DETENT_BUS").filter(|p| !p.is_empty()) {
        return p.into();
    }
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("detent").join(SOCKET_NAME)
}

impl Capability {
    /// Config spelling (`events.subscribe`, ...).
    pub fn name(&self) -> &'static str {
        match self {
            Self::EventsSubscribe => "events.subscribe",
            Self::ContextPublish => "context.publish",
            Self::LeasePointer => "lease.pointer",
            Self::LeaseButtons => "lease.buttons",
            Self::DeviceHaptics => "device.haptics",
            Self::DeviceControl => "device.control",
            Self::StateWrite => "state.write",
            Self::InputEmit => "input.emit",
            Self::ActionsInvoke => "actions.invoke",
        }
    }

    pub const ALL: &'static [Capability] = &[
        Self::EventsSubscribe,
        Self::ContextPublish,
        Self::LeasePointer,
        Self::LeaseButtons,
        Self::DeviceHaptics,
        Self::DeviceControl,
        Self::StateWrite,
        Self::InputEmit,
        Self::ActionsInvoke,
    ];

    pub fn from_name(s: &str) -> Option<Self> {
        Self::ALL.iter().find(|c| c.name() == s).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_both_codecs() {
        let m = DaemonToPlugin::ActionInvoked {
            action: "overview".into(),
            params_json: "{}".into(),
            trigger: "gesture.tap".into(),
            delta: -2,
            ts_us: 42,
        };
        for c in [Codec::Postcard, Codec::Json] {
            let b = codec::encode(c, &m).unwrap_or_default();
            let back: DaemonToPlugin = codec::decode(c, &b).unwrap_or(DaemonToPlugin::Shutdown);
            assert_eq!(back, m);
        }
        let hello = PluginToDaemon::Hello(Manifest {
            name: "x".into(),
            version: "1".into(),
            protocol: PROTOCOL_VERSION,
            codec: Codec::Json,
            actions: vec![],
            caps: vec![Capability::ActionsInvoke],
        });
        let j = codec::encode(Codec::Json, &hello).unwrap_or_default();
        assert_eq!(codec::sniff(&j), Codec::Json);
        let p = codec::encode(Codec::Postcard, &hello).unwrap_or_default();
        assert_eq!(codec::sniff(&p), Codec::Postcard);
        assert_eq!(
            Capability::from_name("lease.pointer"),
            Some(Capability::LeasePointer)
        );
    }
}
