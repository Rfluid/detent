---
title: HID++ 2.0 notes
status: current
version: 0.3.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/hidpp-proto/", "crates/hidpp-features/", "crates/device/src/requester.rs", "crates/device/src/receiver.rs", "crates/hidpp-transport/src/discover.rs", "bins/detentd/src/receivers.rs"]
owner: "@rfluid"
tags: [spec, hidpp, protocol]
---

# HID++ 2.0 notes

What Detent relies on. Facts come from prior art ([research/prior-art.md](../research/prior-art.md),
byte layouts in [research/hidpp-layouts.md](../research/hidpp-layouts.md)) and were checked
on the owner's MX Master 4 ([devices/mx-master-4.md](devices/mx-master-4.md)). Anything
still marked ⚠ is unverified.

## Frames

| Report | ID | Length | Params |
|---|---|---|---|
| Short | `0x10` | 7 | 3 |
| Long | `0x11` | 20 | 16 |
| Very long | `0x12` | 64 | 60 |

```
[report id][device index][feature index][fn<<4 | swid][params...]
```

- **Device index**: `0xFF` for direct connections (Bluetooth, USB cable); `1..=6` behind a
  Bolt/Unifying receiver. Direct devices sometimes answer with `0x00`; accept both.
- **swid** (4 bits): echoed in responses. `0` = notification. Taken: kernel 1, logiops 2/3,
  OpenRGB 7, libratbag 8, LGSTrayEx A, Solaar B, G HUB D, firmware F. **Detent uses `0xE`.**
- **Errors**: feature index `0xFF` (2.0) or `0x8F` (1.0): `[rid, dev, 0xFF, orig_feat_idx, orig_fn_swid, code]`.
  Codes: 1 unknown, 2 invalid argument, 3 out of range, 4 hardware, 5 internal, 6 invalid
  feature index, 7 invalid function, 8 busy, 9 unsupported.
- hidraw also delivers the mouse's normal input reports; anything that isn't an HID++
  report ID is skipped. Every opener of the hidraw node receives every report.

## Requests

- Always **long** reports (Bluetooth devices may not accept short ones).
- One request in flight per device. Match: same feature index, function and swid (or an
  error report naming them).
- Timeout **1 s per attempt, 3 attempts** on timeout; BUSY (0x08) is retried with a short
  backoff; other device errors fail immediately. (Kernel: 5 s, BUSY only; logiops saw MX4
  init fail at 500 ms.)
- Measured on the owner's device over Bluetooth: a full `dump-features` (≈ 60 requests)
  takes ≈ 1.3 s, i.e. ≈ 20 ms per round trip.

## Hard safety rules

Never send:
- any request on the **0x1D4B** index (the kernel treats any report there as "device
  connected" and reruns its setup);
- **0x1814** setCurrentHost (moves the mouse to another computer) except on an explicit
  user action (`hosts switch`, a `device = "host"` binding, `device.host`, the tray). It
  is fire-and-forget (no reply; the link drops) and never re-applied on reconnect;
- **0x1815** setHostFriendlyName except on an explicit `hosts rename` (never the local
  hostname behind the user's back, as Solaar does);
- **0x19C0** writes (persistent force-button calibration);
- **persistent** diversion bits in setCidReporting (buttons would stay dead without Detent);
- anything to a feature not listed below, or to a hidden/engineering feature.

## Discovery

1. Root (`0x0000`, index 0) fn 1: protocol version + ping byte echo.
2. Root fn 0 `getFeature(id BE)` → `(index, flags, version)`. Index 0 = not supported.
3. FeatureSet (`0x0001`) fn 0 `getCount`, fn 1 `getFeatureId(i)` → full table. Flags:
   0x80 obsolete, 0x40 hidden, 0x20 engineering.

## Features used

| Feature | ID | Use | Status |
|---|---|---|---|
| Root / FeatureSet | `0x0000` / `0x0001` | discovery | verified |
| Device FW info | `0x0003` | firmware versions | verified |
| Device name/type | `0x0005` | name | verified |
| Unified battery | `0x1004` | battery | verified (charging byte: kernel table) |
| Change host | `0x1814` | Easy-Switch: slot count, current; switch on request | read verified; switch to another host ⚠ (not run yet) |
| Hosts info | `0x1815` | Easy-Switch: paired, names; rename on request | verified 2026-11-25 (read, rename any slot) |
| Reprog controls v4 | `0x1B04` (v6 on MX4) | divert buttons, raw XY | verified (CIDs below) |
| Wireless device status | `0x1D4B` | reconnect notification | verified through Bolt: power-on sends `11 02 04 00 01 01 01` (status 1, reconfigure 1) |
| SmartShift enhanced | `0x2111` | wheel mode, auto-shift, torque | verified (MX4 has no 0x2110) |
| Hi-res wheel | `0x2121` | resolution, invert | verified (read) |
| Thumbwheel | `0x2150` | invert | verified (read) |
| Adjustable DPI | `0x2201` | DPI | verified (MX4 has no 0x2202) |
| Haptics | `0x19B0` | enable, intensity, play waveform | verified (read) |
| Force sensing button | `0x19C0` | — | never written |

## Easy-Switch (`0x1814`, `0x1815`)

Layouts in [research/hidpp-layouts-device.md](../research/hidpp-layouts-device.md#6-changehost-0x1814-and-hostsinfo-0x1815);
code in `crates/hidpp-features/src/hosts.rs`.
- Read on attach: 0x1814 fn 0, then (if present) 0x1815 fn 0, fn 1 per slot and fn 3 in
  14-byte chunks per name. HostsInfo failures leave names empty; switching still works.
- Switch: re-read fn 0, resolve `next`/`prev` with wrap-around, check `slot < count`,
  skip if already there, then `Requester::send` (one write, no reply, no retry).
- Rename: needs caps bit1; name ≤ `nameMaxLen` bytes, no control characters; written in
  14-byte chunks at offsets 0, 14, ...; then read back, and the CLI prints what the device
  kept. Verified on the MX4 (2026-11-25, through the Bolt receiver): caps `0x03`, 24-byte
  names, a shorter name replaces a longer one completely, and slots other than the
  current one can be renamed.

## Reprog controls v4 (`0x1B04`)

- fn 0 `getCount`; fn 1 `getCidInfo(i)` → `cid u16, tid u16, flags u8, pos, group, gmask, flags2`.
  flags: 0x20 divertable, 0x40 persistently divertable, 0x80 virtual. flags2: 0x01 raw XY,
  0x02 force raw XY, 0x08 raw wheel.
- fn 2 `getCidReporting(cid)` → `cid u16, report u8, remap u16, report2 u8`; fn 3
  `setCidReporting(cid, report, remap, ...)`. `report`: bit0 divert / bit1 valid, bit2
  persist / bit3 valid, bit4 rawXY / bit5 valid, bit6 forceRawXY / bit7 valid. Only bits whose
  valid bit is set are applied; remap 0 = unchanged.
- Notifications: event 0 = the set of up to 4 diverted CIDs **currently held** (diff
  consecutive sets for press/release); event 1 = raw XY `dx i16, dy i16` (BE).
- **Diversion is volatile**: lost on sleep, reconnect and host switch. Re-applied on resync.
- Left/right click are **not divertable** on the MX Master 4 (verified).

## MX Master 4 controls

See [devices/mx-master-4.md](devices/mx-master-4.md#controls-0x1b04-v6).

## Haptics (`0x19B0`)

- fn 0 capabilities: params bytes 4..8 = u32 BE waveform bitmap.
- fn 1 config: `flags (bit0 enabled, bit1 battery saving), intensity 0..=100, levels`.
- fn 2 set config `[flags, intensity, 0]` (persistent on the device, like Options+).
- fn 4 play `[waveform, 0, 0]`: one-shot, not stored; safe every ≥ 12 ms.
- Waveforms: 0x00–0x0E and 0x1B (names in `crates/hidpp-features/src/haptics.rs`).

## Receivers (Logi Bolt)

Verified 2026-11-25 on the owner's Bolt receiver (`046D:C548`, Linux 7.3.4) with the MX
Master 4 paired at slot 2. Fact file: `.agent/memory/facts/2026-11-25-bolt-receiver.md`.

- **Kernel**: `hid-logitech-dj` has no alias for C548, so `hid-generic` binds all four
  interfaces. HID++ is on interface 2 only (descriptor: 0xFF00/0x0001 report 0x10 and
  0xFF00/0x0002 report 0x11). No per-device node exists; Detent opens the receiver node and
  addresses the device by slot. The kernel sends no HID++ here (no swid-1 traffic, no
  hi-res wheel reset). Pointer input comes from interface 1's "Logitech USB Receiver Mouse"
  evdev node, which the data plane uses for chords.
- **Discovery** (`discover::is_receiver`, `crates/device/src/receiver.rs`): only nodes with
  a receiver PID *and* the 0xFF00/0x0002 collection count; the receiver's other interfaces
  are not listed. If `logitech-djreceiver` owns the receiver, it is skipped (the kernel then
  gives each device its own node, reached at 0xFF) ⚠ untested.
- **Pairings**: HID++ 1.0 `GET_LONG_REGISTER` (`10 FF 83 B5 <0x50+n> 00 00`) →
  `11 FF 83 B5 <0x50+n> <kind> <wpid LE ×2> <unit id ×4> ...`; the unit id equals the
  device's own `0x0003` unitId, so the device key is `<wpid>:<unit id>` (`B042:A204BE6C`).
  Name: `B5 <0x60+n> 01` → `<0x60+n> 01 <len> <utf-8>`. Empty slot: `8F 83 B5 08`.
- **Slots**: HID++ 2.0 at index 1–6 through the same node. Ping at an empty slot:
  `10 0n 8F 00 1E 09 00` (1.0 error form, our swid, code 9). Register `0x02` reports the
  connected count; `0xF1` is not supported (`8F 81 F1 02`). Register `0x00` (notification
  flags) reads `00 00 00`; Detent leaves it alone.
- **Reconnect**: with the receiver's notifications off there is no disconnect event, but
  a device that powers on sends `0x1D4B` at its slot; the worker resyncs on it (as over
  Bluetooth). A device that is off when the daemon starts attaches as soon as it sends
  anything: hotplug keeps receivers with unattached pairings open (`receivers::Watch`).
- **Write stalls**: a write sometimes fails with EPIPE and succeeds within ~10 ms;
  `requester::send_retrying` retries up to three times (control plane only). The first
  register read after a quiet spell can also return a spurious error (`8F 83 B5 08` for
  a paired slot), so register reads retry a device error once.
- Unifying (C52B / C532) is not enabled: its pairing registers differ and nobody has
  checked it on hardware (issue #5).

## Coexistence with the kernel driver

`hid-logitech-hidpp` binds B042, uses swid 1, enables hi-res scrolling via 0x2121 on every
connect, and watches 0x1D4B. Detent ignores `Foreign` frames, never diverts the wheel or
thumbwheel (layer bindings read them from evdev during a grab), and re-applies its own
settings ~300 ms after a reconnect so the kernel's setup runs first.
