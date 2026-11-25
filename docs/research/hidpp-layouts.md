---
title: "HID++ 2.0 feature layouts (research appendix)"
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# HID++ 2.0 feature layouts for Detent (MX Master 4, BT, 046D:B042, devidx 0xFF)

Researched 2026-11-23. Everything below is restated protocol fact (byte offsets, IDs,
semantics) in our own words. No code was copied. Solaar and logiops are GPL; the kernel
is GPL-2.0; libratbag is MIT. Only facts are taken from any of them.

This page is the index: sources, the summary of verdicts (section 15) and the open items
(section 16). The per-feature layouts (sections 0-14) live in the three [parts](#parts).

## Sources and versions

| Tag | Source | Version / commit | License |
|---|---|---|---|
| S-h20 | Solaar `lib/logitech_receiver/hidpp20.py` https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/hidpp20.py | master fetched 2026-11-23 (clone HEAD e7304c4) | GPL-2+ |
| S-c | Solaar `hidpp20_constants.py` (same dir) | same | GPL-2+ |
| S-st | Solaar `settings_templates.py` | same | GPL-2+ |
| S-b | Solaar `base.py` | same | GPL-2+ |
| S-n | Solaar `notifications.py` | same | GPL-2+ |
| S-d | Solaar `diversion.py`, `settings.py`, `common.py`, `special_keys.py` | same | GPL-2+ |
| K | Linux `drivers/hid/hid-logitech-hidpp.c` https://github.com/torvalds/linux/blob/master/drivers/hid/hid-logitech-hidpp.c | master fetched 2026-11-23 | GPL-2 |
| KC | Linux `drivers/hid/hid-core.c` | same | GPL-2 |
| L | logiops `src/logid/backend/hidpp20/features/*` https://github.com/PixlOne/logiops/tree/main/src/logid/backend/hidpp20/features | e157995 | GPL-3 |
| R | libratbag `src/hidpp20.c`, `src/hidpp-generic.c` https://github.com/libratbag/libratbag/blob/master/src/hidpp20.c | master fetched 2026-11-23 | MIT |

Line numbers refer to those snapshots. Master moves, so re-anchor before you link them in a spec.

## Parts

| Doc | Sections |
|---|---|
| [hidpp-layouts-protocol.md](hidpp-layouts-protocol.md) | 0 frame format, software IDs, error codes, timeouts and retries; 1 Root 0x0000; 2 FeatureSet 0x0001; 14 kernel coexistence |
| [hidpp-layouts-device.md](hidpp-layouts-device.md) | 3 DeviceFwVersion 0x0003; 4 DeviceName 0x0005 / DeviceFriendlyName 0x0007; 5 UnifiedBattery 0x1004; 6 ChangeHost 0x1814 / HostsInfo 0x1815; 8 WirelessDeviceStatus 0x1D4B |
| [hidpp-layouts-input.md](hidpp-layouts-input.md) | 7 ReprogControlsV4 0x1B04; 9 SmartShift 0x2110 / 0x2111; 10 HiResWheel 0x2121; 11 Thumbwheel 0x2150; 12 AdjustableDPI 0x2201 / 0x2202; 13 HAPTIC 0x19B0 |

## 15. Request handling recommendations for Detent (summary of verdicts)

| Topic | Decision | Why |
|---|---|---|
| Report type | Always 0x11 for 2.0 requests. Accept 0x10 and 0x11 replies. | The kernel does this, and BLE mice may lack 0x10. |
| Device index | Send 0xFF. Accept 0xFF or 0x00 in replies. | Solaar's BT quirk. |
| swid | Fixed 0xE | Free in every known tool's list, and filterable. |
| Matching | Reply = same feat_idx and same `fn\|swid`. Error = `0xFF, feat_idx, fn\|swid, code` (or 0x8F for 1.0). | Kernel, libratbag and Solaar all match this way. |
| Concurrency | One outstanding request per device (control thread). Events are parsed on the input path regardless. | All prior art serialises requests. |
| Timeout | 1 s | Middle ground between logiops (0.5 s) and Solaar/kernel (4-5 s). |
| Retry | BUSY only, 3 attempts, short backoff | Kernel. |
| After reconnect | ping, check the feature-index map is still valid (refresh if firmware changed), re-apply volatile config after the kernel's wheel write | Kernel, Solaar and logiops sequencing. |
| Writes to unknown features | Forbidden without owner OK | AGENTS.md |

## 16. Open items to verify on the real B042 (record in `.agent/memory/facts/`)

1. Full FeatureSet dump: indices, IDs, versions and flags. In particular, confirm 0x2111 vs
   0x2110, 0x2201 vs 0x2202, 0x19B0, 0x1004, 0x1D4B, 0x1814 and 0x1815.
2. ~~Whether the hidraw report descriptor exposes 0x10 short reports.~~ No (verified
   2026-11-25: `.agent/memory/facts/2026-11-25-mx4-bt-report-descriptor.md`).
3. Whether replies come back with device index 0xFF or 0x00.
4. getCidInfo for every control: CIDs, the divertable and rawXY flags, and what 0x01A0 is.
5. 0x1B04 setCidReporting with a 6th byte of 0x00 is accepted. Divert state is lost after a
   power cycle, and 0x1D4B request == 1 arrives.
6. 0x1004 chargingStatus values while charging (kernel vs Solaar table), and externalPower.
7. 0x2111: the capabilities byte layout, torque range, and the "mode byte required" torque quirk.
8. 0x2121: the multiplier value, and that the kernel's swid-1 setWheelMode reply is visible on
   hidraw after reconnect.
9. 0x2150: capabilities (touch/proximity/tap) and divertedRes.
10. 0x19B0: raw fn0 and fn1 replies. Confirm the waveform bitmap and level semantics before any
    set-config write.
11. 0x0003 getFwInfo byte 8 and the extras layout.
