---
title: "ADR 0007: control plane on std threads, not tokio"
status: current
version: 1.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["bins/detentd/src/"]
owner: "@rfluid"
tags: [adr, architecture]
---

# ADR 0007: control plane on std threads, not tokio

## Context

The architecture overview planned a small tokio runtime for the control plane. Building
Phase 2–3, the control plane turned out to be: one worker per device (blocking HID++
round trips through the data plane, ~20 ms each), a udev monitor, a config watcher, a
JSON-RPC server with a handful of clients, and later a plugin supervisor.

## Decision

Use plain `std::thread`s and bounded `std::sync::mpsc` channels for the control plane.
The data plane is unchanged: one thread per device, `poll(2)`, `rtrb` ring + `eventfd`
for control → data, non-blocking `try_send` for data → control.

## Why

- Every control-plane task is naturally blocking and low-rate; async buys nothing and costs
  a large dependency, compile time and a second concurrency model next to the data plane.
- The data plane can't use async anyway (hot-path rules); one model is easier to reason about.
- Prior art daemons of this kind (logiops, keyd) are thread- or loop-based.

## Consequences

- `tokio` stays out of `detentd`. A plugin may use any runtime it likes (the bus is a socket).
- IPC is one thread per client (expected: a few). If that ever matters, revisit.
