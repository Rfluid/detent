//! Budget: `Frame::parse` + `classify` < 100 ns (docs/architecture/performance.md).

#![allow(clippy::unwrap_used)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use detent_hidpp_proto::{Frame, SoftwareId, classify};

fn bench(c: &mut Criterion) {
    // A diverted-buttons notification from the MX Master 4 (reprog index 13).
    let raw = [
        0x11u8, 0xFF, 0x0D, 0x00, 0x00, 0xC3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    c.bench_function("parse+classify", |b| {
        b.iter(|| {
            let f = Frame::parse(black_box(&raw)).unwrap();
            black_box(classify(&f, SoftwareId::DETENT))
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
