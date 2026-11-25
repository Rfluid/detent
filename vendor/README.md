---
title: Vendored dependencies
status: current
version: 0.2.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["Cargo.toml"]
owner: "@rfluid"
tags: [vendor, gpui]
---

# Vendored dependencies

Nothing is vendored at the moment.

## History: `gpui/` (removed 2026-11-23)

GPUI 0.2.2 was vendored here to add a Wayland layer-shell patch for the radial overlay
(ADR 0005 rev. 1). Phase R found that upstream Zed already ships layer-shell, available on
crates.io as the `gpui-pre` snapshot crates, so the workspace now depends on
`gpui-pre =0.3.14` / `gpui-pre-platform =0.3.14` (renamed `gpui` / `gpui_platform`) and no
patch is needed (ADR 0005 rev. 2). The old copy remains in git history.

Rules if something must be vendored again:
- Each patch minimal, commented `// DETENT PATCH (#n): ...`, listed in a table here.
- Wire it with `[patch.crates-io]` and keep `Cargo.lock` changes to what the patch needs.
