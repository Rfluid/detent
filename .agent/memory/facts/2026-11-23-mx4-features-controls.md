---
title: MX Master 4 feature table and controls (verified on device)
date: 2026-11-23
source_task: phase-0
tags: [fact, device, mx-master-4, hidpp]
---

- Firmware RBM 27.03 B0019, HID++ 4.5, 46 features. Same indexes as public dumps (logiops #520, Solaar #2964).
- Key indexes: 0x1D4B@4, 0x1004@9, **0x19B0 haptics@11**, 0x19C0 force button@12, 0x1B04@13 (v6), 0x1814@14, 0x2111@17 (no 0x2110), 0x2121@18, 0x2150@19, 0x2201@20 (no 0x2202).
- CIDs: 0x50 L, 0x51 R (not divertable), 0x52 middle, 0x53 back, 0x56 forward, 0xC3 gesture, 0xC4 SmartShift button, **0x1A0 Haptic Sense Panel** (config name `thumb`), 0xD7 virtual gesture. All divertable ones are raw-XY capable; **none persistently divertable**.
- Defaults read: DPI 1000 (200..8000 step 50), SmartShift ratchet / auto 10 / torque 75, haptics on / 50, all 16 waveforms supported.
- Round trip over BT ≈ 20 ms; full feature dump ≈ 1.3 s.
- Full detail: docs/specs/devices/mx-master-4.md.
