---
title: Performance budget
status: current
version: 0.2.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["crates/engine/", "crates/hidpp-proto/"]
owner: "@rfluid"
tags: [architecture, performance]
---

# Performance budget

Performance is the first design rule. A Bluetooth mouse already has milliseconds of link latency, and Detent must add an amount you can't measure next to that.

## Targets (to be measured, not assumed)

| Path | Budget (p99) | How measured |
|---|---|---|
| `Frame::parse` + `classify` | < 100 ns | criterion (`detent-hidpp-proto`) |
| dispatch: modifiers → lease → table lookup | < 200 ns | criterion (`detent-engine`) |
| hidraw read → uinput write, built-in action | < 50 µs | e2e harness: replay transport + uinput timestamp |
| hidraw read → plugin `recv` | < 100 µs | e2e harness with SDK test plugin |
| Thumb press → radial overlay visible | next compositor frame after the action arrives | manual + compositor timing |
| Idle CPU | ~0 | `perf stat` over 60 s idle |

## Measured (2026-11-23, owner's laptop, `just bench`, release + LTO)

| Path | Result | Budget |
|---|---|---|
| `Frame::parse` + `classify` | **4.7 ns** | < 100 ns |
| dispatch: diverted button press + release → exec | **26 ns** | < 200 ns |
| dispatch: wheel in a layer → key chord (6 output events) | **15 ns** | < 200 ns |
| dispatch: raw-XY frame during a gesture | **4.7 ns** | < 200 ns |
| dispatch: pass-through motion while grabbed | **3.5 ns** | < 200 ns |
| allocations per event (`tests/zero_alloc.rs`, 10 000 mixed iterations) | **0** | 0 |
| daemon RSS with the example config | ≈ 4–5 MB | < 15 MB |
| HID++ round trip over Bluetooth | ≈ 20 ms | (link, not ours) |
| radial menu: plugin action → overlay mapped | ≈ 130–150 ms (GPUI per-window surface) | next frame (**not met**, see radial.md) |

End to end (hidraw read → uinput write) was not instrumented separately; it is the dispatch
above plus one `read(2)` and one or two `write(2)` to uinput (single-digit µs).

## Rules (enforced in review; see `.agent/skills/hot-path-change.md`)

1. No heap allocation per event; a test runs N events under a counting global allocator and asserts 0 allocations.
2. No locks or blocking queues on the data plane: `ArcSwap` for config, `rtrb` SPSC rings, non-blocking sockets.
3. No `format!`, `String`, `Vec::push`, `Box::new` on the path.
4. Bounded work per event: arrays sized at compile time (≤ 64 slots, ≤ 8 modifiers, ≤ 256 layer masks).
5. Exec, D-Bus, file I/O and info+ logging happen on the control plane only.
6. Hot functions are `#[inline]` and branch-light; `Action` stays `Copy` and ≤ 32 bytes (assert with `const _: () = assert!(size_of::<Action>() <= 32);`).

## Regression gate

`just bench` stores a criterion baseline; CI (later) fails if a hot-path bench regresses by more than 10%.

## Tuning knobs (later, only if measured to help)

- `SCHED_FIFO` or `nice -5` for data-plane threads (needs `CAP_SYS_NICE` or rtkit; optional)
- CPU pinning of the device thread
- `mlockall` to avoid page faults on the path
