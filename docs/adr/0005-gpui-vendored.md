---
title: "ADR 0005: GPUI for plugin UI (rev. 2: upstream layer-shell, no vendoring)"
status: accepted
version: 2.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["vendor/README.md", "plugins/radial/", "Cargo.toml"]
owner: "@rfluid"
tags: [adr, ui, gpui]
---

# ADR 0005: GPUI for plugin UI (rev. 2)

**Context.** The radial menu needs a GPU-rendered, low-latency overlay. The author prefers GPUI
(already used in `aura`). crates.io `gpui` 0.2.2 has no Wayland layer-shell support, which an
overlay needs.

**Original decision (rev. 1).** Vendor GPUI 0.2.2 in `vendor/gpui` and add layer-shell as a
minimal patch, *unless a newer upstream GPUI already provides it*.

**What Phase R found.** Upstream Zed merged layer-shell (PR #35610, merge `b92664c5`,
2025-10-29): `WindowKind::LayerShell(LayerShellOptions { namespace, layer, anchor,
exclusive_zone, exclusive_edge, margin, keyboard_interactivity })`. crates.io `gpui` 0.2.2
predates it, but the `gpui-pre` crates (Apache-2.0 snapshots of zed@f126950) include it.

**Decision (rev. 2).** Depend on `gpui-pre` / `gpui-pre-platform` **=0.3.14** from crates.io
(feature `wayland`), renamed to `gpui` / `gpui_platform` in the workspace. No vendored copy, no
patch. `vendor/gpui` (0.2.2) is removed.

**Verified on the owner's Plasma 6.8.3:** a full-output overlay layer surface renders
transparently; `window.mouse_position()` reports the pointer after the compositor's `enter`;
the first window costs ~0.7 s (Vulkan/wgpu init, once per process), later ones ~150 ms each.

**Consequences.**
- Must enable `gpui_platform`'s `wayland` feature; without it GPUI silently falls back to a
  headless platform (windows "open" but nothing is shown). Recorded in memory.
- Use `QuitMode::Explicit`: the plugin outlives its windows.
- Use `KeyboardInteractivity::None`: KWin focuses keyboard-interactive overlay surfaces at once.
- Heavy dependency tree (slow first build; `just check-core` skips it). Exact pin; upgrades are
  deliberate (re-test the overlay on the device).
