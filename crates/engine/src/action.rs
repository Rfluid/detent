//! What a binding does. A closed enum with inline data: matching it is free and
//! dispatch never touches the heap.

/// Up to 6 pre-resolved evdev key codes pressed together, e.g. Ctrl+Alt+Left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub codes: [u16; 6],
    pub len: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// Commands sent to the device through the request ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceCmd {
    SetDpi(u16),
    CycleDpi,
    ToggleSmartShift,
    HapticPulse { waveform: u8, intensity: u8 },
}

/// Index into the control plane's table of exec commands (spawning never happens here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CmdId(pub u16);

/// A plugin connection slot, assigned when the plugin says hello.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSlot(pub u8);

/// Action ID within the plugin's manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionId(pub u16);

/// Index into the compiled config's interned parameter blobs (e.g. `menu = "main"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamsRef(pub u16);

// Hot-path rule: actions stay small and Copy (docs/architecture/performance.md).
const _: () = assert!(core::mem::size_of::<Action>() <= 32);

impl KeyChord {
    pub fn new(codes: &[u16]) -> Option<Self> {
        if codes.is_empty() || codes.len() > 6 {
            return None;
        }
        let mut c = [0u16; 6];
        c[..codes.len()].copy_from_slice(codes);
        Some(Self {
            codes: c,
            len: codes.len() as u8,
        })
    }
}

impl Action {
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Action::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    #[default]
    None,
    Keys(KeyChord),
    Button(u16),
    Scroll {
        axis: Axis,
        amount: i16,
    },
    Device(DeviceCmd),
    Profile(u8),
    Exec(CmdId),
    Plugin {
        plugin: PluginSlot,
        action: ActionId,
        params: ParamsRef,
    },
}
