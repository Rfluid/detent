---
title: Author's MX Master 4 connection facts
date: 2026-11-23
source_task: scaffold
tags: [fact, device, mx-master-4]
---

- Connected over **Bluetooth**: `HID_ID=0005:0000046D:0000B042`, `HID_NAME=Logitech MX Master 4`.
- hidraw node **is not stable**: `/dev/hidraw9` in one session, `/dev/hidraw10` later the same day. Identify devices by vendor/product/serial, never by path.
- Direct connection → HID++ device index `0xFF`.
- Kernel module `hid_logitech_hidpp` is loaded (shares the HID++ channel, swid 1).
- hidraw node and `/dev/uinput` already had user ACLs on the author's Fedora/KDE machine.
