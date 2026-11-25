---
title: "HID++ 2.0 layouts: device info, name, battery, hosts, wireless status"
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# HID++ 2.0 layouts: device info, name, battery, hosts, wireless status

Part of the [HID++ 2.0 feature layouts](hidpp-layouts.md) appendix (MX Master 4, BT,
046D:B042, devidx 0xFF). Source tags (S-h20, K, L, R, ...) and the licensing note are in
[the index](hidpp-layouts.md#sources-and-versions). Framing, Root, FeatureSet and kernel
coexistence (sections 0-2, 14) are in [hidpp-layouts-protocol.md](hidpp-layouts-protocol.md);
input features (sections 7, 9-13) in [hidpp-layouts-input.md](hidpp-layouts-input.md).

## 3. DeviceFwVersion 0x0003 (aka DeviceInformation)

| fn | name | params | reply |
|---|---|---|---|
| 0 | getDeviceInfo | none | `[entityCount, unitId(4), transport(2), modelId(6), extendedModelId?, ...]` |
| 1 | getFwInfo | `[entity]` | `[type, prefix(3 ASCII), number, revision, build_hi, build_lo, ?, extras...]` |

- getDeviceInfo:
  - Unit ID is bytes 1..=4. The kernel uses these as the serial (K L1031-1050), and Solaar does
    too (S-h20 L1821-1834).
  - Solaar reads the transport bitfield from byte 6, with bits 0x01 BT, 0x02 BLE, 0x04 eQuad
    (WPID), 0x08 USB. Model IDs are packed u16s from byte 7 on, one per set transport bit, in
    that bit order (S-h20 L1825-1833).
- getFwInfo:
  - Type is the low nibble of byte 0: 0 main firmware, 1 bootloader, 2 hardware, 3+ other
    (S-c FirmwareKind, S-h20 L1784-1795).
  - For types 0 and 1: prefix = bytes 1..=3 (ASCII), number = byte 4, revision = byte 5, and
    build = bytes 6..=7 (u16 BE). Solaar formats these as `NN.RR.Bbbbb`.
  - Solaar treats bytes 9 and up as "extras" and skips byte 8. Logitech's doc reportedly calls
    byte 8 an "active" flag and bytes 9-10 the transport PID. Unverified.
  - For type 2 (hardware), byte 1 is the hardware revision.
- **Verdict: adopt** for display ("Device info" panel). The meaning of byte 8 and the extras is
  **to verify** on the device.

## 4. DeviceName 0x0005 and DeviceFriendlyName 0x0007

0x0005:

| fn | name | params | reply |
|---|---|---|---|
| 0 | getCount | none | `[nameLength]` |
| 1 | getDeviceName | `[charIndex]` | up to 16 name bytes starting at charIndex (long report) |
| 2 | getDeviceType | none | `[type]`: 0 keyboard, 1 remote, 2 numpad, 3 mouse, 4 touchpad, 5 trackball, 6 presenter, 7 receiver |

- Chunking:
  - The kernel loops with charIndex += bytes received, and takes `16` (long) or `3` (short)
    bytes per reply (K L1101-1180).
  - Solaar loops with charIndex = len(name so far) (S-h20 L1851-1870).
  - logiops steps by 16 (L `DeviceName.cpp`).
  - Truncate to nameLength. The reply has no length byte.
- Type table: S-c L251-260, S-h20 L1836-1849.

0x0007 (friendly name):
- fn0 returns `[len, ...]`. Solaar reads only byte 0.
- fn1 takes `[index]` and replies `[index echo, chars...]`, so name bytes start at reply byte 1
  (S-h20 L1875-1894, which slices `fragment[1:...]`). That gives 15 chars per call.
- **Verdict:**
  - 0x0005: adopt.
  - 0x0007: adapt. Read only, if present. Its write function isn't documented in prior art, so
    don't write it.

## 5. UnifiedBattery 0x1004

| fn | name | params | reply |
|---|---|---|---|
| 0 | getCapabilities | none | `[supportedLevels, flags]` |
| 1 | getStatus | none | `[stateOfCharge%, level, chargingStatus, externalPower]` |
| event 0 | statusBroadcast | n/a | same 4 bytes as getStatus |

- supportedLevels bitmask: bit0 critical, bit1 low, bit2 good, bit3 full.
- flags: bit0 rechargeable, bit1 state-of-charge (percentage) supported. With bit1 set the kernel
  uses the percentage and ignores levels (K L1573-1621).
- level (getStatus byte 1): the same bit layout. Solaar maps 8 to full (90), 4 to good (50),
  2 to low (20) and 1 to critical (5) (S-h20 L2244-2265).
- chargingStatus (byte 2): **sources disagree.**
  - Kernel: 0 discharging, 1 charging, 2 charging slowly, 3 complete (full), 4 error, other =
    not charging (K L1626-1654).
  - Solaar reuses its generic BatteryStatus enum: 0 discharging, 1 recharging, 2 almost full,
    3 full, 4 slow recharge, 5 invalid battery, 6 thermal error (S-d `common.py` L640-647,
    S-h20 L2246).
  - Solaar's rule "charging" test treats 1..=3 as charging (S-d `diversion.py` L401-409).
  - **Adopt the kernel's table.** It is specific to 0x1004, while Solaar's enum was written for
    0x1000. Verify by plugging the mouse in and watching events.
- externalPower (byte 3): the kernel passes it in but ignores it (K L1699). Presumably
  0 = none, 1 = wired. **To verify.**
- Event 0 has the same layout. The kernel handles it only when function == 0 (K L1741-1772),
  and Solaar the same (S-n L273-277).
- **Kernel coexistence:** the kernel drives 0x1004 itself and exposes a power_supply
  (`/sys/class/power_supply/hidpp_battery_*`). It queries on every connect (K L4547-4551).
- **Verdict: adapt.**
  - Parse 0x1004 event 0 passively from hidraw. It costs nothing because the device broadcasts
    it.
  - Do one getStatus at connect.
  - Don't poll. Optionally read the sysfs power_supply as a cross-check.

## 6. ChangeHost 0x1814 and HostsInfo 0x1815

0x1814:

| fn | name | params | reply |
|---|---|---|---|
| 0 | getHostInfo | none | `[numHosts, currentHost (0-based), flags]`. flags bit0 = enhanced host switch. |
| 1 | setCurrentHost | `[host]` | **no reply.** The device drops the link at once. |
| 2 | getCookies | none | one cookie byte per host |
| 3 | setCookie | `[host, cookie]` | echo |

- Sources: L `ChangeHost.h/.cpp` (function numbers, no-response send, and it emulates
  InvalidArgument when host >= count). S-st L1295-1318 (`no_reply: True`, `persist = False`,
  host count/current read from fn0 bytes 0-1).
- **Verdict: adopt** (implemented 2026-11-25 in `crates/hidpp-features/src/hosts.rs`, on
  explicit user action only; see docs/specs/hidpp.md "Easy-Switch").
  - Send setCurrentHost fire-and-forget. Don't wait for a reply.
  - Validate host < numHosts locally.
  - Never persist or re-apply it on reconnect. Solaar marks persisting it as harmful.
  - `prev` must wrap: logiops' `current - 1` from host 0 becomes 255 and is rejected.

0x1815 (read side only):

| fn | name | params | reply |
|---|---|---|---|
| 0 | getFeatureInfo | none | `[capFlags, ?, numHosts, currentHost]`. capFlags bit0 = can read names, bit1 = can write names. |
| 1 | getHostInfo | `[host]` | `[host, status(paired?), ?, ?, nameLen, nameMaxLen]` |
| 3 | getHostFriendlyName | `[host, offset]` | `[host, offset, up to 14 name bytes]` |
| 4 | setHostFriendlyName | `[host, offset, up to 14 bytes]` | ack |

- Source: S-h20 L2064-2112.
- Solaar **automatically rewrites the current host's name on the device** to the local hostname
  (S-h20 L2083-2087). **Verdict: reject that behaviour.**
  - Don't write to the device implicitly. Writes happen only on `detentctl hosts rename`
    (owner decision 2026-11-25), in 14-byte chunks at offsets 0, 14, ... (Solaar
    `set_host_name`: fn 4 `[host, offset, bytes]`, no length byte, no terminator).
  - Reading names is fine.
  - Verified on the MX4 (2026-11-25): caps bit0 and bit1 set, `nameMaxLen` 24, fn 4
    accepts any slot, a shorter name replaces the stored one completely. Still unknown:
    fn 1 bytes 2-3.
- Bytes 1-3 of fn1 (bus type, page count?) are **unverified**.

## 8. WirelessDeviceStatus 0x1D4B

- No functions used by any prior art. Event 0 (statusBroadcast):
  - byte 0 `status`: 1 = reconnection.
  - byte 1 `request`: 1 = **software reconfiguration needed**. Diverts and other volatile state
    were lost.
  - byte 2 `reason`: 1 = power switch / powered on.
- Sources: L `WirelessDeviceStatus.h/.cpp` (reconnection, reconfNeeded, powerSwitch = params
  0..=2). S-n L321-333 (Solaar re-applies settings when byte 1 == 1 and treats byte 2 == 1 as
  "powered on").
- logiops waits 100 ms after this event before re-configuring (L `features/DeviceStatus.cpp`
  L56-60).
- **Kernel interaction (important):** the kernel looks up the 0x1D4B index at connect. It treats
  **any** HID++ report whose feature_index equals that index (regardless of function or swid) as
  a connect event and schedules its connect worker (K L487-494, L4153-4157, L4516-4521).
- **Verdict: adopt.**
  - On event 0 with request == 1, re-run Detent's "apply volatile config" sequence.
  - **Never send a request to the 0x1D4B index.** The reply would trigger the kernel's connect
    worker, which resets the hi-res wheel mode ([section 14](hidpp-layouts-protocol.md#14-kernel-coexistence-hid-logitech-hidpp-on-b042)).
  - Resolving the index via Root.getFeature is fine, because that reply carries index 0.

