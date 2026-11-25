---
title: Background children outlive a nested kwin_wayland session
date: 2026-11-24
source_task: detent-tray system tray icon
tags: [testing, kwin, screenshots]
---

Processes a nested-session script starts with `&` (plasmashell, kded6, test binaries) are
not all reaped when `kwin_wayland --exit-with-session` exits; several `detent-tray` copies
kept running afterwards. Record their pids (`$!`) and kill exactly those at the end of the
script. Never `pkill -x plasmashell`/`kded6`: that also hits the owner's session.
