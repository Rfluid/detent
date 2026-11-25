---
title: Screenshot GPUI overlays in an isolated virtual KWin
date: 2026-11-23
source_task: radial menu Actions Ring redesign
tags: [testing, kwin, gpui, screenshots]
---

To see what a layer-shell overlay really renders without touching the owner's session (works
even when the screen is locked, where overlays never show):

```sh
timeout -k 5 30 dbus-run-session -- kwin_wayland --virtual --no-lockscreen \
  --width 900 --height 640 --socket wayland-detent-x --exit-with-session "/path/shot.sh out.png" \
  </dev/null >kwin.log 2>&1
# shot.sh: start `detent-radial --preview … 2>err &`, sleep 3.5, `spectacle -b -n -f -o out.png`
```

- The private D-Bus session lets spectacle use that KWin's ScreenShot2.
- The virtual output has no pointer: `enter` never arrives and the ring draws nothing until it
  knows the cursor, so pass `--at X,Y` (checked 2026-11-24). There is no portal either: pass
  `--theme light|dark` (`system` falls back to light). `--highlight 0 --pin` shows a gauge.
- Redirect stdin/stdout to files; piping kwin's output keeps the pipeline alive after timeout.
- Never `pkill -x kwin_wayland` (kills the owner's compositor) or `pkill -f <socket>` from a
  `bash -c` (matches itself, see lessons/2026-11-23-pkill-self).
