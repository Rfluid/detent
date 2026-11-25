---
title: "ADR 0003: Plugins are out-of-process programs on a plugin bus"
status: accepted
version: 1.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/bus/", "crates/plugin-sdk/"]
owner: "@rfluid"
tags: [adr, plugins]
---

# ADR 0003: Plugins are out-of-process programs on a plugin bus

**Context.** The reference plugins (KDE integration, radial menu) need D-Bus, KWin scripting, Wayland surfaces and UI toolkits. Their latency floors (D-Bus round trip, compositor frame) are far above the cost of a process boundary.

**Options.** Native dylibs (no stable Rust ABI, a crash kills input), in-process WASM (sandboxed but no I/O or UI), embedded Lua (no sandbox, no UI), out-of-process over IPC.

**Decision.** Plugins are supervised processes speaking a versioned protocol over Unix `SOCK_SEQPACKET` (postcard default, JSON optional). Plugins add named actions, subscribe to events, lease input with deadlines, and send device commands within granted capabilities. In-process WASM stays an optional later extension for pure per-frame logic only.

**Consequences.** Crash isolation, any language or toolkit, composable action namespace. Costs: ~10–30 µs per delivered event, a supervisor, a public protocol to maintain.
