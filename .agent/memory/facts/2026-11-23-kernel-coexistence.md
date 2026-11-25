---
title: Kernel hid-logitech-hidpp coexistence rules
date: 2026-11-23
source_task: phase-r
tags: [fact, kernel, hidpp]
---

- Kernel binds B042 (driver `logitech-hidpp-device`), swid 1, re-sets HiResWheel (0x2121) mode on every connect and tracks every setWheelMode reply.
- **Never send a request on the 0x1D4B index**: the kernel treats any report there as "connected" and reruns setup.
- swid map: kernel 1, logiops 2/3, OpenRGB 7, libratbag 8, Solaar B, G HUB D, firmware F → Detent **0xE**.
- Decision: Detent never diverts wheel/thumbwheel; layer bindings on them read evdev events during the chord grab.
- Source: docs/research/hidpp-layouts-protocol.md §0, §14; docs/research/hidpp-layouts-device.md §8.
