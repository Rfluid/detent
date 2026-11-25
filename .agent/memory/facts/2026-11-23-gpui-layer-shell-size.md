---
title: GPUI layer-shell surfaces need 0×0 bounds to fill the output
date: 2026-11-23
source_task: radial menu Actions Ring redesign
tags: [gpui, wayland, layer-shell, kwin]
---

gpui-pre 0.3.14 sends `zwlr_layer_surface_v1.set_size(bounds)` using the window bounds, which
default to 1536×1095 when `window_bounds` is `None`. KWin honours a non-zero size even when the
surface is anchored to all four edges, so a "full-output" overlay is actually 1536×1095,
centred, and its `viewport_size()` is not the output's.

Fix: pass `window_bounds: Windowed(Bounds { size: 0×0 })` → `set_size(0, 0)` → the compositor
sizes it to the anchored edges; GPUI applies the size from the layer `configure`.
Side effect: the first frame can render at 0×0 before configure, so never latch anything
computed from `viewport_size()` on the first frame.

**Superseded on fractional-scale outputs (2026-11-25):** 0×0 makes GPUI send
`wp_viewport.set_destination(0, 0)`, which is a protocol error. Pass the output's logical size
instead (see 2026-11-25-gpui-dead-wayland-connection).

Measured in a nested `kwin_wayland --virtual --width 900 --height 640`: viewport 1536×1095
before, 900×640 after.
