//! Budget: dispatch (modifiers → layer → table lookup → output) < 200 ns
//! (docs/architecture/performance.md). Measured on the example config.

#![allow(clippy::unwrap_used)]

use std::time::Instant;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use detent_engine::action::{CmdId, KeyChord};
use detent_engine::out::code::*;
use detent_engine::{
    Action, Compiled, Edge, Engine, LayerId, ModMask, Out, SlotInfo, Tables, slot,
};

fn cfg() -> Compiled {
    let mut c = Compiled::default();
    c.slots[slot::GESTURE.0 as usize] = SlotInfo {
        cid: 0xC3,
        modifier: Some(0),
        ..Default::default()
    };
    c.slots[slot::THUMB.0 as usize] = SlotInfo {
        cid: 0x1A0,
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
        .set(LayerId(0), slot::THUMB, Edge::Press, Action::Exec(CmdId(1)));
    t.bindings.set(
        LayerId(1),
        slot::SWIPE,
        Edge::SwipeUp,
        Action::Exec(CmdId(2)),
    );
    t.bindings.set(
        LayerId(1),
        slot::WHEEL,
        Edge::RotatePos,
        Action::Keys(KeyChord::new(&[29, 15]).unwrap()),
    );
    c.profiles = vec![t];
    c
}

fn bench(c: &mut Criterion) {
    let cfg = cfg();
    let now = Instant::now();

    c.bench_function("dispatch: button press+release (exec)", |b| {
        let mut e = Engine::default();
        let mut out = Out::default();
        b.iter(|| {
            out.clear();
            e.on_diverted_buttons(&cfg, black_box([0x1A0, 0, 0, 0]), now, &mut out);
            e.on_diverted_buttons(&cfg, black_box([0, 0, 0, 0]), now, &mut out);
            black_box(out.controls().count())
        })
    });

    c.bench_function("dispatch: raw XY frame during gesture", |b| {
        let mut e = Engine::default();
        let mut out = Out::default();
        e.on_diverted_buttons(&cfg, [0xC3, 0, 0, 0], now, &mut out);
        b.iter(|| {
            out.clear();
            e.on_raw_xy(&cfg, black_box(1), black_box(0), &mut out);
            black_box(out.events().len())
        })
    });

    c.bench_function("dispatch: wheel in layer → keys", |b| {
        let mut e = Engine::default();
        let mut out = Out::default();
        e.on_diverted_buttons(&cfg, [0xC3, 0, 0, 0], now, &mut out);
        e.set_grabbed(true);
        b.iter(|| {
            out.clear();
            e.on_evdev(&cfg, EV_REL, REL_WHEEL, black_box(1), now, &mut out);
            black_box(out.events().len())
        })
    });

    c.bench_function("dispatch: pass-through motion while grabbed", |b| {
        let mut e = Engine::default();
        let mut out = Out::default();
        e.on_diverted_buttons(&cfg, [0xC3, 0, 0, 0], now, &mut out);
        e.set_grabbed(true);
        b.iter(|| {
            out.clear();
            e.on_evdev(&cfg, EV_REL, REL_X, black_box(3), now, &mut out);
            e.on_evdev(&cfg, EV_SYN, SYN_REPORT, 0, now, &mut out);
            black_box(out.events().len())
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
