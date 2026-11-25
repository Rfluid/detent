---
title: Data plane
status: current
version: 0.2.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["crates/engine/", "crates/hidpp-proto/", "crates/hidpp-transport/src/hidraw.rs", "bins/detentd/src/dataplane.rs", "bins/detentd/src/uinput.rs", "bins/detentd/src/msg.rs"]
owner: "@rfluid"
tags: [architecture, performance, hot-path]
---

# Data plane

One thread per device (`bins/detentd/src/dataplane.rs`). It owns the hidraw fd, the mouse's
evdev fd and the two uinput devices, and is their **only** reader and writer.

## Loop

```text
loop {
    poll([hidraw, wake eventfd, evdev], timeout = next hold deadline)   // only place it sleeps
    if wake:   drain the command ring (rtrb): Send(frame) → hidraw write,
               SetDpi, SetProfile, ReleaseAll, Stop
    if hidraw: read every pending report → Frame::parse → classify(swid 0xE)
                 Response / Error   → try_send to the worker
                 Notification 0x1B04 fn0 → engine.on_diverted_buttons(4 CIDs)
                 Notification 0x1B04 fn1 → engine.on_raw_xy(dx, dy)
                 other Notification → try_send to the worker (battery, 0x1D4B link)
                 Foreign swid 1     → try_send KernelTraffic (kernel reset the wheel)
    if evdev:  fetch events → engine.on_evdev (tracks buttons; translates only while grabbed)
    hold deadline passed → engine.tick
    engine.grab_change → EVIOCGRAB / ungrab
    flush: uinput write(s) + control messages (try_send)
}
```

## Engine (`crates/engine`)

Pure state machine: `Engine` turns events into an `Out` (fixed buffers: 64 input events,
8 control messages). The device thread performs the I/O. Semantics in
[bindings.md](bindings.md).

## Channels

| Direction | Mechanism | Full → |
|---|---|---|
| worker → data plane | `rtrb` SPSC ring (64) + `eventfd` wake | worker logs and drops |
| data plane → worker | bounded `std::sync::mpsc::sync_channel(256)`, `try_send` only | dropped, counted (`Stats`) |

The data plane never blocks on the control plane (ADR 0007).

## Rules

1. **No heap allocation per event.** Verified by `crates/engine/tests/zero_alloc.rs` (counting
   global allocator, 10 000 mixed iterations, 0 allocations).
2. **No locks.** `CompiledConfig` is read with one `ArcSwap::load` per loop iteration.
3. **No blocking.** Every outbound path is `try_send` / ring push.
4. **Syscalls on the path:** hidraw read, evdev read, uinput write(s), at most one `try_send`.
5. **Logging:** `debug!` only, for failures; info and above on the control plane.

## Request/response

Requests (reconciler, IPC, plugins, device actions) are built by the worker and pushed on
the ring; the data plane writes them; responses come back over the channel and are matched
by the worker (feature index, function, swid). Long reports, 1 s timeout, 3 attempts,
BUSY retried (docs/specs/hidpp.md).

## uinput

Two virtual devices, created at attach: **Detent virtual keyboard** (KEY_ESC..KEY_MICMUTE)
and **Detent virtual pointer** (BTN_LEFT..BTN_TASK, REL_X/Y, wheel and hwheel incl. hi-res),
`BUS_VIRTUAL 1209:DE71/DE72`. A batch is split per device and per SYN frame; each frame is
one `emit` (events + SYN_REPORT). `UI_SET_PHYS` fails with EINVAL through evdev 0.13 on
this kernel, so devices are identified by name/ID instead of `phys`.

## Grab while armed

The mouse's evdev node is always open (non-grabbed) so the engine knows which physical
buttons are down. `EVIOCGRAB` happens when a modifier whose layer needs evdev events is
held **and no button is down**; events queued before the grab are consumed in tracking
mode. While grabbed, unbound events are forwarded to the virtual pointer. Ungrab happens
when the modifier is released **and no button is down**, so a press/release pair is never
split between the real and the virtual device.

## Reconnect

- hidraw read error / HUP → release everything, thread ends (`Gone`), worker detaches; the
  udev monitor re-attaches when the node comes back (the node number may change).
- 0x1D4B event with status/request = 1 → worker waits 300 ms (kernel setup first), then
  full resync: read all, apply settings, re-divert.
- Every 30 s the worker checks one diverted CID; if diversion was lost, it resyncs.
