---
title: MX Master 4 report descriptor over Bluetooth (HID++ collection, no short reports)
date: 2026-11-25
source_task: portability seams (ADR 0010)
tags: [fact, device, mx-master-4, transport, hid]
---

Read from `/sys/class/hidraw/hidraw8/device/report_descriptor` (owner's MX Master 4,
`046D:B042`, Bluetooth, Linux; read-only, no device I/O). Three top-level collections:

1. Generic Desktop / Mouse, report **0x02** (7 buttons, 12-bit X/Y, wheel, AC Pan).
2. `06 43 FF 0A 02 02 A1 01 85 11 95 13 75 08 … 81 00 … 91 00`: vendor usage page
   **0xFF43**, usage **0x0202**, report **0x11**, 19 data bytes in and out. This is the
   only HID++ collection: **no report 0x10** (short HID++ reports don't exist over BT).
3. Haptics, usage page **0x0E**: report **0x3C** (feature: waveform list / durations) and
   **0x3D** (output: manual trigger, waveform, intensity, repeat, retrigger). A standard
   HID haptics collection, besides HID++ feature 0x19B0. Not used by Detent.

Consequences: off Linux (Windows / macOS hidapi), open the 0xFF43/0x0202 collection and
send long reports only (docs/specs/transport.md). Settles the open items in
docs/research/mx-master-4.md ("does B042 expose 0x10", "usage page 0x0E haptics").
