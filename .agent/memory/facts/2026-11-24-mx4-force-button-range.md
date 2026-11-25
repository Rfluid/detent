---
title: MX Master 4 Force Sensing Button (0x19C0) on the owner's unit
date: 2026-11-24
source_task: click-force design
tags: [fact, device, mx-master-4, hidpp, force-button]
---

- Tested over BT through `device.raw`, firmware RBM 27.03 B0019.
- fn0 → `01`: one force sensor (the Haptic Sense Panel, `thumb`).
- fn1(0) → `00 01 | 15 68 | 1C 78 | 11 20`: flags u16 = 0x0001 (changeable),
  **default 5480, max 7288, min 4384**. No index echo in the reply.
- fn2(0) → `14 DE`: owner's value **5342** (not the default; probably set by Options+ once).
- fn3 `[0, hi, lo]` sets the value and echoes `[0, hi, lo]`. fn2 reads the new value at once.
- **Writes are stored in the mouse's memory**: 5480 was still there after the power switch was turned off and on
  (owner-approved test, 2026-11-24). Restored to 5342 afterwards.
- The range differs between units (familiar: 3704..6158, OpenLogi fixture: 4071..6768), so
  always read it at runtime. The unit is unknown; treat raw values as opaque.
