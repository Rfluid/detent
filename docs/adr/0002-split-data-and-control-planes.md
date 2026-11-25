---
title: "ADR 0002: Split data plane and control plane"
status: accepted
version: 1.0.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["crates/engine/"]
owner: "@rfluid"
tags: [adr, performance]
---

# ADR 0002: Split data plane and control plane

**Context.** Performance is the first requirement. Mixing event handling with config, IPC and plugin management in one async runtime adds task wake-ups, allocation and lock contention to every click.

**Decision.** One dedicated, synchronous thread per device runs the whole event path (read → decode → dispatch → uinput write) without allocation, locks or async. Everything else runs on a tokio control plane. They communicate only through `ArcSwap` (config snapshots) and bounded SPSC rings (requests, telemetry). Plugin sends from the data plane are non-blocking.

**Consequences.** Predictable latency and isolation from slow consumers. Costs: config must be compiled into flat tables; some state is mirrored across the boundary; hot-path code follows strict rules (docs/architecture/performance.md).
