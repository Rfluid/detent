---
title: Easy-Switch (0x1814/0x1815) in prior art, and what is still unverified
date: 2026-11-25
source_task: Easy-Switch hosts list/switch/rename
tags: [hidpp, easy-switch, prior-art, mx-master-4]
---

Read from source (Solaar `hidpp20.py` / `settings_templates.py`, logiops `ChangeHost.cpp`,
`ChangeHostAction.cpp`) on 2026-11-25:

- 0x1814: fn0 `[count, current, flags]`, fn1 setCurrentHost `[slot]` gets **no reply**.
  Solaar: `no_reply: True` and `persist = False` ("persisting this setting is harmful").
  logiops: `callFunctionNoResponse`, accepts `next`/`prev`/`N`. Its `prev` from slot 0
  computes `-1`, which becomes 255 and is rejected, so wrap explicitly.
- 0x1815 (Solaar only): fn0 `[caps, ?, count, current]` (bit0 read names, bit1 write
  names), fn1 `[slot, status, ?, ?, nameLen, nameMaxLen]`, fn3 `[slot, offset]` returns 14
  name bytes from reply byte 2, fn4 `[slot, offset, ≤14 bytes]` writes. Solaar writes the
  local hostname into the current slot on its own; Detent rejects that (owner decision).
- Solaar's `Host` diversion rule is a **condition** (`socket.getfqdn()`), not a host switch.
- Logitech's `x1815_hostsinfo_v2.pdf` exists in their HID++ Drive folder but couldn't be
  fetched, so the full 0x1815 function list is unknown.

**Not yet checked on the owner's MX4** (the mouse was on the Bolt receiver during this
task): caps bits, fn1 bytes 2-3, whether a rename of a non-current slot is accepted,
whether a shorter name truncates the old one, and that setCurrentHost moves the mouse.
Run `detentctl hosts`, `hosts rename 1 --this-host`, `hosts switch next` and record results here.
