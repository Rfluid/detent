---
title: "ADR 0006: Two Unix-socket protocols: binary plugin bus, JSON client IPC"
status: accepted
version: 1.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/bus/", "crates/ipc/"]
owner: "@rfluid"
tags: [adr, ipc]
---

# ADR 0006: Two Unix-socket protocols: binary plugin bus, JSON client IPC

**Context.** Plugins receive events at high rates and are latency-sensitive; clients (CLI, GUI, scripts) are not and benefit from debuggability.

**Decision.** Plugin bus: `SOCK_SEQPACKET` + postcard (JSON negotiable). Client IPC: newline-delimited JSON-RPC 2.0 over a stream socket. Both under `$XDG_RUNTIME_DIR/detent/`. D-Bus may be added later as a facade for desktop integration, not as the primary transport.

**Consequences.** Fast plugin path, trivially scriptable control path, no D-Bus dependency in the daemon core. Two protocols to document (docs/specs/plugin-bus.md, docs/specs/ipc.md).

**Amendment (2026-11-25, ADR 0010: other OSes).** The daemon is Linux-only for v0.1.0, so
neither protocol gets a non-Unix transport yet. `detent-ipc` keeps its Unix stream socket
(builds on Linux and macOS); on Windows the client compiles against an uninhabited stand-in
stream and `connect_to` returns `ClientError::Unsupported`, because `detentctl` needs the
crate's types and settings catalogue there but has no daemon to reach. The plugin bus
(`SOCK_SEQPACKET`, believed unsupported for `AF_UNIX` on macOS: ⚠ to verify) and the SDK are Linux-only. When a
Windows daemon exists, the client IPC moves to a named pipe (`\\.\pipe\detent-ctl`, e.g. via
the `interprocess` crate) with the same newline-delimited JSON-RPC; the plugin bus will need
a message-preserving transport of its own on macOS and Windows.
