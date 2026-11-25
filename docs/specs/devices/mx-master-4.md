---
title: "Device: MX Master 4 (046D:B042, Bluetooth)"
status: current
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["crates/hidpp-features/src/cid.rs", "crates/hidpp-features/src/haptics.rs", "docs/specs/devices/captures/"]
owner: "@rfluid"
tags: [spec, device, mx-master-4]
---

# MX Master 4 (046D:B042, Bluetooth)

Captured on the owner's mouse with `detentctl dump-features` / `dump-controls`
(2026-11-23, firmware RBM 27.03 B0019, HID++ 4.5). Full JSON:
[captures/mx-master-4-features.json](captures/mx-master-4-features.json),
[captures/mx-master-4-controls.json](captures/mx-master-4-controls.json).
Matches the public dumps in [research/mx-master-4.md](../../research/mx-master-4.md).

## Features

| idx | ID | ver | Name | Detent |
|---|---|---|---|---|
| 0 | 0x0000 | 0 | Root | discovery |
| 1 | 0x0001 | 2 | FeatureSet | discovery |
| 2 | 0x0003 | 8 | DeviceFwInfo | read |
| 3 | 0x0005 | 5 | DeviceNameType | read |
| 4 | 0x1D4B | 0 | WirelessDeviceStatus | notifications only; **never send requests** |
| 5 | 0x0020 | 1 | ConfigChange | — |
| 6 | 0x0021 | 1 | CryptoId | — |
| 7 | 0x0007 | 0 | DeviceFriendlyName | — |
| 8 | 0x0011 | 0 | (unknown) | — |
| 9 | 0x1004 | 5 | UnifiedBattery | read + events |
| 10 | 0x1701 | 0 | (unknown) | — |
| 11 | 0x19B0 | 0 | Haptics | read, config, play |
| 12 | 0x19C0 | 0 | ForceSensingButton | not touched (writes are persistent) |
| 13 | 0x1B04 | 6 | ReprogControlsV4 | read, temporary divert |
| 14 | 0x1814 | 2 | ChangeHost | read only |
| 15 | 0x1815 | 2 | HostsInfo | — |
| 16 | 0x2250 | 1 | XyStats | — |
| 17 | 0x2111 | 0 | SmartShiftEnhanced | read/write |
| 18 | 0x2121 | 1 | HiResWheel | read; invert only |
| 19 | 0x2150 | 0 | Thumbwheel | read; invert only |
| 20 | 0x2201 | 2 | AdjustableDpi | read/write |
| 21 | 0x2251 | 0 | WheelStats | — |
| 22 | 0x00D1 | 0 | (unknown) | — |
| 23–45 | 0x1802 … 0x18C0, 0x1E00 …, 0x9xxx | | hidden/engineering | never touched |

## Controls (0x1B04 v6)

| CID | Task | Control | Config name | Divertable | Raw XY | Notes |
|---|---|---|---|---|---|---|
| 0x0050 | 0x0038 | Left | `left` | no | no | chords via evdev grab |
| 0x0051 | 0x0039 | Right | `right` | no | no | chords via evdev grab |
| 0x0052 | 0x003A | Middle | `middle` | yes | yes | |
| 0x0053 | 0x003C | Back | `back` | yes | yes | |
| 0x0056 | 0x003E | Forward | `forward` | yes | yes | |
| 0x00C3 | 0x009C | Gesture button | `gesture` | yes | yes | default modifier |
| 0x00C4 | 0x009D | SmartShift button | `smartshift` | yes | yes | |
| 0x01A0 | 0x0109 | Haptic Sense Panel | `thumb` | yes | yes | Actions Ring button in Options+ |
| 0x00D7 | 0x00B4 | Virtual gesture | — | yes (virtual) | yes | not used |

No control is persistently divertable: diversion is lost on sleep/reconnect and Detent
re-applies it.

## Settings read on the owner's device (2026-11-23)

```
MX Master 4  [/dev/hidraw10]  link: connected
  firmware      RBM 27.03 B0019
  battery       85% (full, discharging)
  dpi           1000   (range 200..8000 step 50, default 1000)
  smartshift    ratchet   auto-shift 10   torque 75
  wheel         hires on  invert off  diverted off  ratchet on
  thumbwheel    invert off  diverted off
  haptics       on  intensity 50  (16 waveforms)
  easy-switch   host 1 of 3
```

## Haptics (0x19B0)

Supported waveforms on this unit: all 16 published IDs (0x00–0x0E, 0x1B), see
`crates/hidpp-features/src/haptics.rs`. Detent uses `subtle-collision` (4) for radial
hover ticks and `damp-state-change` (1) for selection (Options+ convention).

## Force Sensing Button (0x19C0)

Verified on 2026-11-24:

| Function | Reply | Meaning |
|---|---|---|
| fn0 | `01` | one sensor (index 0 = Haptic Sense Panel, `thumb`) |
| fn1(0) | `00 01 15 68 1C 78 11 20` | flags 0x0001 (changeable), default **5480**, max **7288**, min **4384** |
| fn2(0) | `14 DE` | current **5342**, the owner's value (differs from the default) |
| fn3 `[0, hi, lo]` | `00 hi lo` | sets the value; fn2 reflects it at once |

**Writes are stored in the mouse's memory**: a value written with fn3 was still there after
the power switch was turned off and on. Detent's daemon doesn't write this feature yet;
the plan is in [settings-layers.md](../../architecture/settings-layers.md).
