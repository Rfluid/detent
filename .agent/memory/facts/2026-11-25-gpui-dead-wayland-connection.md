---
title: A 0×0 GPUI layer surface on a fractional-scale output is a protocol error; KWin drops the client
date: 2026-11-25
source_task: radial menu not opening on the built-in monitor, then on none (owner's 3-monitor laptop)
tags: [gpui, wayland, kwin, mesa, radial, fractional-scale]
---

Owner's setup: eDP-1 2880×1800 at 160% (logical 1800×1125), DP-5 1600×900 and DP-6
2560×1080 at 100%, single Intel ARL GPU, Plasma 6.8.

Cause, confirmed with `WAYLAND_DEBUG=1 detent-radial --preview --warm-up --at 900,560,eDP-1`:
- The overlay was opened with 0×0 bounds (the fix from 2026-11-23-gpui-layer-shell-size).
- On a fractional-scale output, KWin sends `wp_fractional_scale_v1.preferred_scale` before
  the first layer `configure`. gpui-pre-linux 0.3.14 `set_size_and_scale` then calls
  `wp_viewport.set_destination(width, height)` with the still-0×0 size.
- KWin: `wl_display.error(wp_viewport, 0, "invalid destination size")`, and the journal of
  `plasma-kwin_wayland.service` says `error in client communication (pid N)`.
- At 100% the scale never changes, so `set_destination` isn't called early: no error there.

What follows: GPUI doesn't exit on a dead connection. Every later `open_window` fails with
`Adapter "Intel(R) Graphics (ARL)" … is not compatible with the display surface for this
window.` (wgpu `get_capabilities` gets no formats, because Mesa's WSI round trip fails on the
dead connection). So the ring breaks on every monitor until the plugin restarts.

Fix: open at the output's logical size, which `kde.cursor` now reports (`width`, `height`
from KWin `screenAt(p).geometry`). Verified: `set_destination(1800, 1125)` followed by
`configure(1800, 1125)`, no error. The other two outputs give 1600×900 and 2560×1080.
GPUI's own display bounds are not usable: they are the mode size divided by the integer
`wl_output.scale`, which gives 1440×900 for eDP-1.

Safety net: any open failure other than `LayerShellNotSupportedError` makes detent-radial
exit 1, and the supervisor restarts it. Diagnose with `detentctl logs --plugin radial`.
The real fix belongs in GPUI (skip `set_destination` while the size is 0); upstream it.
