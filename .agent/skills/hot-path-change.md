---
title: Change hot-path code
status: current
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["crates/engine/", "crates/hidpp-proto/", "crates/hidpp-transport/src/hidraw.rs", "docs/architecture/performance.md"]
owner: "@rfluid"
tags: [skill, performance, hot-path]
---

# Change hot-path code

Use this skill whenever you touch `crates/hidpp-proto`, `crates/engine`, the device thread in `detentd`, or `HidrawTransport`.

## Checklist before writing code
- [ ] Which per-event function(s) does this change? Name them.
- [ ] Does any of them now allocate, lock, block, format strings or log at info+? If yes, redesign: precompute at config-compile time, use a fixed array, or move it to the control plane via a ring.
- [ ] Is the work per event O(1) or bounded by a compile-time constant?
- [ ] No new runtime dependency in `crates/engine` or `crates/hidpp-proto`: their `[dependencies]` stay empty (ADR 0008); test-only crates go in `[dev-dependencies]`.

## Allowed on the hot path
Stack values, fixed arrays, `ArcSwap::load`, `rtrb` `push/pop` (non-blocking), hidraw `read`/`write`, uinput `write`, evdev `read`/`EVIOCGRAB`, non-blocking `send` on plugin sockets, `eventfd` write, `trace!`/`debug!` behind a static level check.

## Forbidden
`Vec::push`, `String`, `format!`, `Box::new`, `HashMap` inserts, `Mutex`/`RwLock`, blocking channels, `.await`, spawning processes, D-Bus, file I/O, `info!`+ logging, unbounded loops over user data.

## After writing code
1. Unit tests for the logic.
2. Keep `Action` `Copy` and ≤ 32 bytes (const assert).
3. Run `just bench`; compare with the baseline. A regression > 10% needs a justification in the commit body or a fix.
4. Zero-allocation test still passes (once it exists, phase 2).
5. Update docs/architecture/data-plane.md if the flow changed.
