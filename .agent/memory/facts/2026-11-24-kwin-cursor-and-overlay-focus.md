---
title: KWin 6.8 — cursor position via script; keyboard-exclusive overlay focus
date: 2026-11-24
source_task: radial menu behaves like the Actions Ring
tags: [fact, kwin, scripting, wayland, focus]
---

Verified in a nested `kwin_wayland --virtual` (KWin 6.8, private `dbus-run-session`):

- KWin scripts can read the pointer: `workspace.cursorPos` (QPointF) and
  `workspace.screenAt(p)` → Output with `.name` ("Virtual-0") and `.geometry` (x, y, w, h).
  A one-shot script reporting back with `callDBus(...)` arrives in a few ms. (There is still
  no D-Bus method for the cursor on `/KWin`.)
- Pass numbers to `callDBus` as `String(...)`: JS numbers may marshal as int or double.
- `dbus-send` sends a *signal* unless given `--type=method_call` (`Script.run` silently did
  nothing). Script `print()` output is not visible by default; report through D-Bus instead.
- A layer-shell overlay with `KeyboardInteractivity::Exclusive` is *activated* by KWin
  (`workspace.windowActivated` fires with `resourceClass = "detent-radial"`); when its surface
  is removed, KWin re-activates the previous window (Konsole in the test). Anything that
  publishes the active app must skip the overlay.
- GPUI display UUID = `Uuid::new_v5(NAMESPACE_DNS, wl_output name)`, the same name KWin
  scripts report, so a KWin output can be mapped to a GPUI `display_id`.
