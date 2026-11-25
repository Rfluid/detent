---
title: Desktop facts behind the radial volume gauge, kde.terminal and theme = "system"
date: 2026-11-24
source_task: radial Volume/Terminal defaults + dark theme
tags: [fact, kde, pipewire, gpui, theme]
---

Checked on the owner's Fedora 44 + Plasma 6.8 (Breeze Dark):

- `wpctl get-volume @DEFAULT_AUDIO_SINK@` → `Volume: 0.80` (` [MUTED]` appended when muted).
  `wpctl inspect @DEFAULT_AUDIO_SINK@` → `  * node.description = "AB13X USB Audio Analog Stereo"`.
- Plasma OSD: `org.kde.plasmashell /org/kde/osdService org.kde.osdService.volumeChanged(ii)`
  (percent, maximumPercent) exists (busctl introspect).
- `~/.config/kdeglobals` has **no** `TerminalApplication` / `TerminalService` while the default
  terminal is kept → fall back to `konsole` (`/usr/bin/konsole`). No `ColorScheme` key either;
  the scheme comes from `[KDE] LookAndFeelPackage=org.kde.breezedark.desktop`.
- XDG portal `org.freedesktop.appearance color-scheme` = `1` (prefer dark) under Breeze Dark.
  GPUI (gpui-pre 0.3.14) reads it at start and follows its change signal:
  `cx.window_appearance()` → `Dark`. In a nested `kwin_wayland --virtual` there is no portal, so
  previews must pass `--theme`.
- GPUI on Wayland: a wheel notch = `ScrollDelta::Lines` of 3.0 (`SCROLL_LINES`), sign flipped
  (`vertical_modifier = -1`): wheel up → `y > 0`. Horizontal (thumbwheel) is flipped too
  (`horizontal_modifier = -1`); which thumbwheel direction is "up" on the MX4 is **unverified**.
  `ScrollWheelEvent` / `on_scroll_wheel` are deprecated aliases of `ScrollEvent` / `on_scroll`.
- `gpui::Path` has no type parameter in this GPUI; `PathBuilder::stroke(px(w))` + `paint_path`.
