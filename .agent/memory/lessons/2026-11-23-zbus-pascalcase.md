---
title: zbus exports interface methods in PascalCase
date: 2026-11-23
source_task: phase-4
tags: [lesson, dbus, zbus, kde]
---

`#[zbus::interface] impl X { fn activated(&self, ...) }` is exported as `Activated`. A KWin script
calling `callDBus(..., "activated", ...)` fails silently (KWin doesn't report the error). Call the
PascalCase name (or set `#[zbus(name = "...")]`). Debug with
`busctl --user introspect <service> <path> <iface>` and a manual `busctl --user call`.
