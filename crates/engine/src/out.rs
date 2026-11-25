//! What one engine step produces: input events for the virtual device and messages for
//! the control plane. Fixed-capacity buffers on the stack; overflow drops and counts.

use crate::action::{ActionId, CmdId, DeviceCmd, ParamsRef};

/// Linux input event codes used by the engine (from `linux/input-event-codes.h`).
pub mod code {
    pub const EV_SYN: u16 = 0x00;
    pub const EV_KEY: u16 = 0x01;
    pub const EV_REL: u16 = 0x02;
    pub const EV_MSC: u16 = 0x04;

    pub const SYN_REPORT: u16 = 0;
    pub const SYN_DROPPED: u16 = 3;

    pub const REL_X: u16 = 0x00;
    pub const REL_Y: u16 = 0x01;
    pub const REL_HWHEEL: u16 = 0x06;
    pub const REL_WHEEL: u16 = 0x08;
    pub const REL_WHEEL_HI_RES: u16 = 0x0B;
    pub const REL_HWHEEL_HI_RES: u16 = 0x0C;

    pub const BTN_LEFT: u16 = 0x110;
    pub const BTN_RIGHT: u16 = 0x111;
    pub const BTN_MIDDLE: u16 = 0x112;
    pub const BTN_SIDE: u16 = 0x113;
    pub const BTN_EXTRA: u16 = 0x114;
    pub const BTN_FORWARD: u16 = 0x115;
    pub const BTN_BACK: u16 = 0x116;
    pub const BTN_TASK: u16 = 0x117;

    /// Hi-res wheel units per detent.
    pub const HI_RES_PER_DETENT: i32 = 120;
}

/// One `input_event` without the timestamp (the kernel stamps uinput events).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutEvent {
    pub ty: u16,
    pub code: u16,
    pub value: i32,
}

impl OutEvent {
    pub const SYN: Self = Self {
        ty: code::EV_SYN,
        code: code::SYN_REPORT,
        value: 0,
    };

    pub const fn key(code: u16, down: bool) -> Self {
        Self {
            ty: code::EV_KEY,
            code,
            value: down as i32,
        }
    }

    pub const fn rel(code: u16, value: i32) -> Self {
        Self {
            ty: code::EV_REL,
            code,
            value,
        }
    }
}

/// Work for the control plane. `Copy`, no heap: indices into the compiled config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlOut {
    Exec(CmdId),
    /// A named action (plugin or built-in), with the rotation delta for continuous ones.
    Action {
        action: ActionId,
        params: ParamsRef,
        delta: i16,
    },
    Device(DeviceCmd),
    Profile(u8),
    /// Leased pointer motion accumulated over one input frame.
    LeaseMotion {
        dx: i16,
        dy: i16,
    },
    /// A leased button (`code` = evdev BTN_*) or diverted control (`code` = 0, `slot`).
    LeaseButton {
        code: u16,
        slot: u8,
        down: bool,
    },
    /// The lease ended inside the data plane (expired or escape chord).
    LeaseEnded {
        escape: bool,
    },
}

pub const MAX_OUT_EVENTS: usize = 64;
pub const MAX_CONTROL_OUT: usize = 8;

/// Output of one engine step.
#[derive(Debug, Clone)]
pub struct Out {
    events: [OutEvent; MAX_OUT_EVENTS],
    n_events: usize,
    control: [Option<ControlOut>; MAX_CONTROL_OUT],
    n_control: usize,
    /// Events or messages that didn't fit (should stay 0; exported as a counter).
    pub dropped: u32,
}

impl Default for Out {
    fn default() -> Self {
        Self {
            events: [OutEvent::default(); MAX_OUT_EVENTS],
            n_events: 0,
            control: [None; MAX_CONTROL_OUT],
            n_control: 0,
            dropped: 0,
        }
    }
}

impl Out {
    #[inline]
    pub fn clear(&mut self) {
        self.n_events = 0;
        self.n_control = 0;
    }

    #[inline]
    pub fn push(&mut self, ev: OutEvent) {
        if self.n_events < MAX_OUT_EVENTS {
            self.events[self.n_events] = ev;
            self.n_events += 1;
        } else {
            self.dropped += 1;
        }
    }

    /// Push a SYN_REPORT unless the batch is empty or already ends with one.
    #[inline]
    pub fn syn(&mut self) {
        if self.n_events > 0 && self.events[self.n_events - 1] != OutEvent::SYN {
            self.push(OutEvent::SYN);
        }
    }

    #[inline]
    pub fn control(&mut self, msg: ControlOut) {
        if self.n_control < MAX_CONTROL_OUT {
            self.control[self.n_control] = Some(msg);
            self.n_control += 1;
        } else {
            self.dropped += 1;
        }
    }

    #[inline]
    pub fn events(&self) -> &[OutEvent] {
        &self.events[..self.n_events]
    }

    #[inline]
    pub fn controls(&self) -> impl Iterator<Item = ControlOut> + '_ {
        self.control[..self.n_control].iter().flatten().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.n_events == 0 && self.n_control == 0
    }
}
