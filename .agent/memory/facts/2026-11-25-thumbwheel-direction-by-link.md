---
title: MX4 thumbwheel natural direction needs a different invert bit per link
date: 2026-11-25
source_task: thumbwheel reversed through the Bolt receiver
tags: [fact, device, mx-master-4, bolt, thumbwheel]
---

- Owner's definition of natural: **thumb down → scroll left, thumb up → scroll right**.
- With 0x2150 invert **on**, Bluetooth felt right (the default since v0.1.0) and the Bolt
  receiver felt reversed (owner report). So natural = invert on over Bluetooth, off through
  the receiver. Detent's default follows the link (`bins/detentd/src/worker/settings.rs`).
- Through Bolt (hid-generic, no dj) the pointer, wheel and thumbwheel arrive on the receiver's
  "Logitech USB Receiver Mouse" node (REL_HWHEEL + REL_HWHEEL_HI_RES), not a per-device node.
- Not yet measured as evdev values on both links. `/tmp`-style capture: read REL_HWHEEL (code 6)
  from the mouse's event node while the owner rolls down then up; the owner must be told
  before the capture starts (a mid-turn prompt went unseen once).
