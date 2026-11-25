---
title: GPUI layer-shell overlay on KWin 6.8.3 (measured)
date: 2026-11-23
source_task: phase-5
tags: [fact, gpui, kwin, wayland, latency]
---

- `gpui-pre =0.3.14` + `gpui-pre-platform` (feature `wayland`): full-output overlay layer surface
  (anchors all edges, exclusive zone -1, KeyboardInteractivity::None, transparent) renders on
  KWin 6.8.3. ~~Viewport on the owner's laptop: 1536×1095 logical px.~~ Correction
  2026-11-24: 1536×1095 is GPUI's default window size, not the screen; the surface wasn't
  covering the output (see 2026-11-23-gpui-layer-shell-size).
- First `open_window` in a process: ~0.64–0.76 s (Vulkan/wgpu init; a dzn ICD warning is harmless).
  Subsequent windows: ~150 ms each (per-window surface setup). Keep this in mind for overlays.
- Pointer position: `window.mouse_position()` became valid after the compositor's `enter`
  (~50 ms after map) in one run without moving the mouse, but not in another → treat as
  best-effort. (Likely the undersized surface: no `enter` when the cursor was outside it.)
  Since 2026-11-24 the radial asks KWin (`kde.cursor`) and never draws at a guessed place.
- KWin has no D-Bus method for the cursor position (checked `/KWin`).
