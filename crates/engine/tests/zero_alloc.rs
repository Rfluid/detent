//! N8: zero heap allocations per event on the data plane. Runs a realistic event mix
//! through the engine under a counting global allocator.

#![allow(clippy::unwrap_used)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use detent_engine::action::{CmdId, KeyChord};
use detent_engine::out::code::*;
use detent_engine::{
    Action, Compiled, Edge, Engine, LayerId, ModMask, Out, SlotInfo, Tables, slot,
};
use detent_hidpp_proto::{Frame, SoftwareId, classify};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarding to the system allocator with the same layout.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: `p` was allocated by `System` with layout `l`.
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn no_allocation_per_event() {
    let mut c = Compiled::default();
    c.slots[slot::GESTURE.0 as usize] = SlotInfo {
        cid: 0xC3,
        modifier: Some(0),
        ..Default::default()
    };
    c.slots[slot::LEFT.0 as usize] = SlotInfo {
        evdev_button: BTN_LEFT,
        ..Default::default()
    };
    c.modifier_slots[0] = Some(slot::GESTURE);
    c.grab_mods = ModMask(1);
    c.swipe_mods = ModMask(1);
    let mut t = Tables::new(2);
    t.layers.set(ModMask(1), LayerId(1));
    t.bindings
        .set(LayerId(0), slot::GESTURE, Edge::Tap, Action::Exec(CmdId(1)));
    t.bindings.set(
        LayerId(1),
        slot::SWIPE,
        Edge::SwipeUp,
        Action::Exec(CmdId(2)),
    );
    t.bindings.set(
        LayerId(1),
        slot::LEFT,
        Edge::Press,
        Action::Keys(KeyChord::new(&[29, 46]).unwrap()),
    );
    c.profiles = vec![t];

    let mut e = Engine::default();
    let mut out = Out::default();
    let now = Instant::now();
    let raw = [
        0x11u8, 0xFF, 0x0D, 0x00, 0x00, 0xC3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    let before = ALLOCS.load(Ordering::Relaxed);
    for i in 0..10_000 {
        let f = Frame::parse(&raw).unwrap();
        let _ = classify(&f, SoftwareId::DETENT);
        out.clear();
        e.on_diverted_buttons(&c, [0xC3, 0, 0, 0], now, &mut out);
        e.set_grabbed(true);
        e.on_raw_xy(&c, 3, -40, &mut out);
        e.on_evdev(&c, EV_KEY, BTN_LEFT, 1, now, &mut out);
        e.on_evdev(&c, EV_REL, REL_X, i % 7, now, &mut out);
        e.on_evdev(&c, EV_SYN, SYN_REPORT, 0, now, &mut out);
        e.on_evdev(&c, EV_KEY, BTN_LEFT, 0, now, &mut out);
        e.on_diverted_buttons(&c, [0, 0, 0, 0], now, &mut out);
        let _ = e.next_deadline(&c);
        let _ = e.grab_change(&c);
        e.set_grabbed(false);
        std::hint::black_box(out.events().len());
    }
    let after = ALLOCS.load(Ordering::Relaxed);
    assert_eq!(after - before, 0, "engine allocated on the hot path");
}
