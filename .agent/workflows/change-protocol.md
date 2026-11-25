---
title: Change the plugin bus or client IPC protocol
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["crates/bus/src/lib.rs", "crates/ipc/src/lib.rs", "docs/specs/plugin-bus.md", "docs/specs/ipc.md"]
owner: "@rfluid"
tags: [workflow, protocol]
---

# Change the plugin bus or client IPC protocol

These are public APIs: third-party plugins and scripts depend on them.

1. **Justify:** what plugin or client needs this, and why the existing messages can't do it.
2. **Design additively** if possible (new variant or optional field). Breaking change → bump `PROTOCOL_VERSION` and document migration.
3. **Capabilities:** new powers need a `Capability` (and native defaults in `crates/config/src/native.rs` if native plugins use it).
4. **Same commit:** types (`crates/bus` or `crates/ipc`) + spec doc table + SDK helper (`crates/plugin-sdk`) + daemon handling + tests (both codecs: postcard and JSON roundtrip).
5. **Safety review:** can this let a plugin block input, hold input forever, or escalate beyond its caps? If so, add deadlines/limits.
6. `just verify`.
