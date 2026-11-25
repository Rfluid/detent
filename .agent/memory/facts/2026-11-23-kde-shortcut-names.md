---
title: KDE Plasma 6.8.3 global shortcut names (verified)
date: 2026-11-23
source_task: phase-r
tags: [fact, kde, dbus]
---

- Call `invokeShortcut(name)` on `org.kde.kglobalaccel /component/<c>`, iface `org.kde.kglobalaccel.Component`.
- `kwin`: `Overview`, `Grid View`, `Expose`, `ExposeAll`, `Switch to Next Desktop`, `Switch to Previous Desktop`, `Switch One Desktop to the Left/Right`, `Window Maximize`, `Window Minimize`, `Window Close`, `Window Quick Tile Left/Right`, `view_zoom_in`, `view_zoom_out`, `view_actual_size`.
- `kwin` also has `Cycle Overview` / `Cycle Overview Opposite` (no default keys; present on 6.8.3, checked 2026-11-24). They follow the same state machine as the 4-finger touchpad swipes: up = desktop → Overview → Grid, down = desktop → Grid, Grid → Overview, Overview → closed (KWin Plasma/6.8 `overvieweffect.cpp`). Window View (`Expose`) has no touchpad gesture.
- Volume: component **`kmix`** (plasma-pa): `increase_volume`, `decrease_volume`, `mute` (shows OSD; default step 5%).
- Brightness: `org_kde_powerdevil`: `Increase Screen Brightness`, `Decrease Screen Brightness`.
- No `qdbus`/`qdbus6` on Fedora; use `busctl --user` (or `qdbus-qt6`).
- KWin script: `org.kde.KWin /Scripting loadScript(path,name)` → id → `/Scripting/Script<id>` `run()`; JS: `workspace.windowActivated`, `window.resourceClass`, `callDBus`.
