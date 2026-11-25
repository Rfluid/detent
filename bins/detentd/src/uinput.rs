//! Virtual output devices. Two uinput devices (keyboard + pointer) so libinput classifies
//! each correctly; every capability is declared up front (never recreated to add keys).
//! Identified by name ("Detent virtual ...") and ID `1209:DE71/DE72` on `BUS_VIRTUAL`, so
//! nothing (including Detent) mistakes them for the real mouse. (`UI_SET_PHYS` through
//! evdev 0.13 fails with EINVAL on kernel 7.3, so no `phys` tag; see memory.)

use std::io;

use detent_engine::OutEvent;
use detent_engine::out::code;
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, InputEvent, InputId, KeyCode, RelativeAxisCode};

const MAX_FRAME: usize = 64;

pub struct Uinput {
    kbd: VirtualDevice,
    ptr: VirtualDevice,
}

fn id(product: u16) -> InputId {
    InputId::new(BusType::BUS_VIRTUAL, 0x1209, product, 1)
}

impl Uinput {
    pub fn create() -> io::Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();
        // KEY_ESC (1) ..= KEY_MICMUTE (248): every regular key.
        for c in 1..=248u16 {
            keys.insert(KeyCode::new(c));
        }
        let kbd = VirtualDevice::builder()?
            .name("Detent virtual keyboard")
            .input_id(id(0xDE71))
            .with_keys(&keys)?
            .build()?;

        let mut buttons = AttributeSet::<KeyCode>::new();
        for c in code::BTN_LEFT..=code::BTN_TASK {
            buttons.insert(KeyCode::new(c));
        }
        let mut rel = AttributeSet::<RelativeAxisCode>::new();
        for a in [
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_HWHEEL,
            RelativeAxisCode::REL_WHEEL_HI_RES,
            RelativeAxisCode::REL_HWHEEL_HI_RES,
        ] {
            rel.insert(a);
        }
        let ptr = VirtualDevice::builder()?
            .name("Detent virtual pointer")
            .input_id(id(0xDE72))
            .with_keys(&buttons)?
            .with_relative_axes(&rel)?
            .build()?;
        Ok(Self { kbd, ptr })
    }

    /// Write an engine batch. Events are split per device and per SYN frame; each frame
    /// is one `emit` (events + SYN_REPORT). No heap allocation.
    pub fn write(&mut self, events: &[OutEvent]) -> io::Result<()> {
        let mut kbd = [InputEvent::new(0, 0, 0); MAX_FRAME];
        let mut ptr = [InputEvent::new(0, 0, 0); MAX_FRAME];
        let (mut nk, mut np) = (0usize, 0usize);
        for e in events {
            if e.ty == code::EV_SYN {
                if nk > 0 {
                    self.kbd.emit(&kbd[..nk])?;
                    nk = 0;
                }
                if np > 0 {
                    self.ptr.emit(&ptr[..np])?;
                    np = 0;
                }
                continue;
            }
            let ev = InputEvent::new(e.ty, e.code, e.value);
            let to_kbd = e.ty == code::EV_KEY && e.code < 0x100;
            if to_kbd && nk < MAX_FRAME {
                kbd[nk] = ev;
                nk += 1;
            } else if !to_kbd && np < MAX_FRAME {
                ptr[np] = ev;
                np += 1;
            }
        }
        if nk > 0 {
            self.kbd.emit(&kbd[..nk])?;
        }
        if np > 0 {
            self.ptr.emit(&ptr[..np])?;
        }
        Ok(())
    }
}
