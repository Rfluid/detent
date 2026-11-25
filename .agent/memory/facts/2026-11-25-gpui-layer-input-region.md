---
title: GPUI layer surfaces can drop pointer input while mapped, not keyboard focus
date: 2026-11-25
source_task: radial centre-ward close transitions (design pag_OkXg1xaVPSlQ)
tags: [gpui, wayland, layer-shell, radial]
---

In `gpui-pre` 0.3.14:

- `Window::set_input_region(Some(&[]))` empties the surface's input region and commits
  at once, so pointer input goes to the window below while the overlay keeps drawing.
  `None` restores whole-surface input.
- `LayerShellOptions::keyboard_interactivity` is only applied when the window is created
  (`linux/wayland/window.rs`); there's no setter. A mapped `Exclusive` overlay keeps the
  keyboard until it is removed.

Consequence: the radial ring's close animation runs on a click-through surface, but any
item whose steps send `keys` must remove the surface at once (`Close::Now`), because KWin
only gives focus back to the app when the overlay goes away (`FOCUS_RETURN`, 60 ms).
