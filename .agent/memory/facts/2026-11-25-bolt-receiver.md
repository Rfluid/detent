---
title: Logi Bolt receiver on the owner's machine
date: 2026-11-25
source_task: issue-5-receiver-support
tags: [fact, hidpp, receiver, bolt, kernel]
---

- Receiver `046D:C548`, Linux 7.3.4 (Fedora 44): `hid-logitech-dj` has **no C548 alias**, so
  `hid-generic` binds all 4 interfaces (hid-multitouch takes interface 3). No per-device
  hidraw node; HID++ is on interface 2 only (`06 00 FF 09 01 ... 85 10`, `06 00 FF 09 02 ... 85 11`).
- The kernel sends no HID++ on it: no swid-1 traffic, no hi-res wheel reset.
- MX Master 4 paired at **slot 2**; HID++ 4.5 there. Pairing register
  `B5 52` → `52 02 42 B0 A2 04 BE 6C 02 01 80 14 01`: kind 2 (mouse), wpid B042 (LE), unit id
  `A204BE6C` = the mouse's own 0x0003 unitId. Name `B5 62 01` → `"MX Master 4"`.
- Empty pairing slot: `8F 83 B5 08`. Ping at an empty slot: `10 0n 8F 00 1E 09 00`.
- Register 0x00 (notification flags) = `00 00 00`; 0x02 = 1 connected; 0xF1 unsupported.
- Power-on through Bolt: `11 02 04 00 01 01 01` (0x1D4B at index 4, reconfigure) then a
  battery event `11 02 09 00 55 08`. No disconnect event while the receiver's
  notifications are off.
- Writes sometimes fail with **EPIPE** and succeed within ~10 ms; the first register read
  after a quiet spell can return a spurious `8F 83 B5 08` for a paired slot. Retry both.
- Pointer evdev: interface 1, "Logitech USB Receiver Mouse" (`capabilities/rel` 1943).
- The installed detentd v0.1.0 pings every unattached Logitech node at 0xFF every 60 s;
  the receiver answers `10 FF 8F 00 1E 01 00` (that's the "stray" frame in probes).
