---
title: Other tools on the owner's machine that drive the mouse
date: 2026-11-23
source_task: phase-0
tags: [fact, environment, conflicts]
---

- The owner had **Solaar** (XDG autostart `/etc/xdg/autostart/solaar.desktop`) and **JuhRadial MX** (`~/.config/autostart/juhradial-mx.desktop` + user unit `juhradialmx-daemon.service` + `ydotoold.service`) running. Both divert 0xC3 and 0x1A0.
- With the owner's approval they were stopped and disabled on 2026-11-23: `~/.config/autostart/solaar.desktop` (Hidden=true override), `juhradial-mx.desktop` gets `Hidden=true` (backup `.detent-backup`), `systemctl --user disable juhradialmx-daemon`. `ydotoold` left running.
- **Mistake found 2026-11-24:** the backup `~/.config/autostart/juhradial-mx.desktop.detent-backup` was itself autostarted: `systemd-xdg-autostart-generator` turned it into `app-juhradial\x2dmx.desktop.detent\x2dbackup@autostart.service`, so JuhRadial ran at every login and both it and Detent acted on every gesture (its `gesture_directions` switch desktops / maximize / minimize). **Never leave backups (any file) in an autostart directory**; the backup now lives in `~/.config/detent/backup/juhradial-mx.desktop`. Check with `systemctl --user list-units --all 'app-*@autostart.service'`.
- Detentd's startup conflict check raced login autostart (JuhRadial started in the same second, after it); detentd now rescans every 10 s and warns when a tool appears (`bins/detentd/src/conflicts.rs`).
- Diversion left behind by such tools survives until the mouse sleeps: `detentctl reset` clears it.
- `detentd` conflict detection must include `juhradiald` besides `solaar` and `logid`.
- Access today comes from Solaar's `42-logitech-unify-permissions.rules` and group membership (`input`, `uinput`); Detent still ships its own `70-detent.rules`.
