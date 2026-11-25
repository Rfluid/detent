---
title: Testing strategy
status: draft
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["crates/hidpp-transport/src/mock.rs", "crates/config/src/lib.rs"]
owner: "@rfluid"
tags: [dev, testing]
---

# Testing strategy

| Layer | How | Where |
|---|---|---|
| Protocol codecs | unit tests with literal byte frames; `cargo-fuzz` on `Frame::parse`, `classify` and every notification decoder | `crates/hidpp-proto` |
| Features | `MockTransport` scripted with request → response pairs taken from real captures | `crates/hidpp-features`, `crates/device` |
| Device sessions | **record/replay**: `detentctl capture` writes a JSONL trace (timestamped frames, both directions) to `captures/` (gitignored, may contain serials). Scrubbed traces go in `tests/fixtures/` | phase 1+ |
| Engine | table/modifier/lease/gesture unit tests; a **zero-allocation test** with a counting global allocator over 10k replayed events | `crates/engine` |
| Config | the example config is parsed by a test; trigger grammar tests; invalid-config tests | `crates/config` |
| Plugins | SDK mock daemon: feed messages, assert on the outbox; pure logic unit tests (`radial/geometry.rs`) | `crates/plugin-sdk`, `plugins/*` |
| Benchmarks | criterion: parse+classify, dispatch, plugin send | `benches/` in `hidpp-proto`, `engine` |
| On device | the acceptance checks in docs/product/roadmap.md, run manually on the MX Master 4 | per phase |

Rules:
- Every bug fix comes with a test that fails before the fix.
- Hardware facts used in tests must come from real captures, not guesses.
- `just verify` (fmt, clippy `-D warnings`, tests) must pass before work is called done.
