---
title: gpui-pre silently runs headless unless gpui_platform's `wayland` feature is on
date: 2026-11-23
source_task: phase-5
tags: [lesson, gpui, wayland]
---

`gpui-pre-platform` depends on `gpui-pre-linux` with `default-features = false`. Without
`features = ["wayland"]` on `gpui_platform`, `open_window` returns Ok, `render` runs once with a
fake viewport, `mouse_position()` stays (0,0), and nothing appears on screen. No log line says so.
Symptom to recognise: exactly one render, no further frames after `cx.notify()`.

Also: GPUI's default `QuitMode` on Linux quits when the last window closes; a resident plugin
needs `.with_quit_mode(QuitMode::Explicit)`.
