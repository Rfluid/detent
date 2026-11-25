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

**Checked on the owner's MX4 on 2026-11-25** (through the Bolt receiver, device index 2):
3 slots, current = slot 3 (the receiver), `enhanced` flag 0, caps allow read and write,
`nameMaxLen` 24. Renaming works on the current slot and on another slot; a shorter name
replaces a longer one completely (no leftover tail). Switching to the current slot sends
nothing. Slot names were restored afterwards (`chico`, `R phone`, `Logitech Bolt
receiver`; `chico` is also this computer's hostname, its Bluetooth pairing).

**Still unverified:** an actual setCurrentHost to another slot (it takes the mouse away;
the owner presses Easy-Switch to come back), and 0x1815 fn1 bytes 2-3.
