---
title: "ADR 0004: Native plugins live in the repo and use the public bus"
status: accepted
version: 1.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["plugins/", "crates/config/src/native.rs"]
owner: "@rfluid"
tags: [adr, plugins]
---

# ADR 0004: Native plugins live in the repo and use the public bus

**Context.** Detent should be useful the moment it's installed, and the plugin API should be proven by real use.

**Decision.** First-party "native" plugins (`kde`, `radial`, more later) live in `plugins/`, build with the workspace, install next to `detentd`, and are registered in `crates/config/src/native.rs` so `[plugins.<name>]` enables them with no `exec` and sensible default capabilities. They **must** use only the public bus and SDK.

**Consequences.** Zero-setup experience; the SDK and protocol are exercised by every release. Native plugins can't take shortcuts: any new need extends the public protocol. The radial plugin brings GPUI into the workspace build (mitigated by `just check-core`).
