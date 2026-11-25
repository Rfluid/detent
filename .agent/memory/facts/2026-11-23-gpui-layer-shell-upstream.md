---
title: GPUI layer-shell exists upstream (not on crates.io gpui 0.2.2)
date: 2026-11-23
source_task: phase-r
tags: [fact, gpui, wayland]
---

- Zed PR #35610 (merge `b92664c52d7c49bb49acd06d2d40eb23b4e8c601`, 2025-10-29) added `WindowKind::LayerShell(LayerShellOptions { namespace, layer, anchor, exclusive_zone, exclusive_edge, margin, keyboard_interactivity })`.
- crates.io `gpui` 0.2.2 predates it; `gpui-pre` 0.3.14 (Longbridge snapshot, Apache-2.0) and `gpui-ce` 0.3.0 include it.
- KWin focuses keyboard-interactive overlay layer surfaces immediately. The radial menu used
  `None` until 2026-11-24; it now uses `Exclusive` on purpose (Esc/Tab like the Actions Ring)
  and KWin restores the previous window's focus on close (see 2026-11-24-kwin-cursor-and-overlay-focus).
- Supersedes the "write our own patch" plan in ADR 0005 / docs/plugins/radial.md.
