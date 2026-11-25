---
title: "HID++ 2.0 layouts: framing, Root, FeatureSet, kernel coexistence"
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# HID++ 2.0 layouts: framing, Root, FeatureSet, kernel coexistence

Part of the [HID++ 2.0 feature layouts](hidpp-layouts.md) appendix (MX Master 4, BT,
046D:B042, devidx 0xFF). Source tags (S-b, K, L, R, ...) and the licensing note are in
[the index](hidpp-layouts.md#sources-and-versions). Feature layouts are in
[device-level features](hidpp-layouts-device.md) (sections 3-6, 8) and
[input features](hidpp-layouts-input.md) (sections 7, 9-13).

## 0. Frame format and conventions (applies to every feature)

- Report IDs: 0x10 short (7 bytes total), 0x11 long (20 bytes), 0x12 very long (up to 64).
  K L44-50.
- FAP layout: `[report_id, device_index, feature_index, (function<<4)|swid, params...]`.
  Long reports carry 16 parameter bytes. K L414, R L130-138.
- Device index for a directly connected BT device is 0xFF. **Quirk:** Solaar accepts replies whose
  device index is `devnumber ^ 0xFF` (that is, 0x00) for BT devices (S-b L729, L830). Detent
  should accept both 0xFF and 0x00 on its hidraw node. **Verdict: adopt.** It costs nothing and
  prevents dropped replies.
- **Short reports may be unsupported over BT.** The kernel checks the HID report descriptor for
  0x10, 0x11 and 0x12, and promotes short to long when 0x10 is missing (K L436-439, L4616-4651).
  libratbag does the same (R L140-142). The kernel always sends FAP (2.0) requests as long reports
  (K L409-412). **Verdict: adopt.** Always send 0x11 for 2.0 requests, and accept both 0x10 and 0x11
  in replies. VERIFIED 2026-11-25: B042 over BT exposes no 0x10, only 0x11 (usage page 0xFF43,
  usage 0x0202; docs/specs/transport.md).
- Notifications (events) have `swid == 0`. Replies echo the request's swid. Solaar treats a report
  as a 2.0 notification when the low nibble of byte 3 is 0 (S-b L601-633). It ignores every
  reply carrying another client's swid. **Verdict: adopt.**
- Error replies:
  - HID++ 2.0: `[0x11, dev, 0xFF, feat_idx, fn|swid, err, ...]`. Match on feat_idx and fn|swid
    (K L478-485, S-b L747-767, R L179-198).
  - HID++ 1.0: `[0x10, dev, 0x8F, sub_id, address, err, 0]` (K L327-332, S-b L730-746).
- **Reply-to-every-reader:** hid-core hands each report to the driver's `raw_event` first. It
  aborts only on a *negative* return, then always calls `hid_report_raw_event`, which feeds hidraw
  (KC L2174-2180, L2103). So even reports the kernel "consumes" (return 1) still reach our hidraw
  fd. Detent sees the kernel's own replies (swid 1) and every event. **Verdict: adopt.** It is the
  basis of the coexistence strategy in section 14.

### Software IDs in use (pick one that doesn't collide)

| swid | Who | Source |
|---|---|---|
| 0x0 | device notifications | all |
| 0x1 | Linux kernel hid-logitech-hidpp | K L42 |
| 0x2 / 0x3 | logiops (0x3 for no-ack sends) | L `backend/hidpp/defs.h` L43-45 |
| 0x7 | OpenRGB | S-b L897-910 (comment) |
| 0x8 | libratbag | R L132 |
| 0xA | LGSTrayEx | S-b comment |
| 0xB | Solaar (fixed) | S-b L910 |
| 0xD | G HUB | S-b comment |
| 0xF | Logitech firmware self-enumeration | S-b comment |

**Verdict: adopt a fixed swid of 0xE for Detent.** It is free in every list above and has the
high bit set, like Solaar's reasoning. A fixed value lets cooperating tools filter us out, and
Solaar dropped rotation for that reason. Record this in the transport spec.

### Error codes

HID++ 2.0 (byte 5 of a 0xFF error frame). K L236-246, S-c L281-290, R hidpp-generic.c L56-67:

| code | name | note |
|---|---|---|
| 0x00 | NoError | |
| 0x01 | Unknown | |
| 0x02 | InvalidArgument | |
| 0x03 | OutOfRange | |
| 0x04 | HardwareError | |
| 0x05 | LogitechInternal (kernel: NotAllowed) | names differ, same code |
| 0x06 | InvalidFeatureIndex | |
| 0x07 | InvalidFunctionId | |
| 0x08 | Busy | kernel retries (see below) |
| 0x09 | Unsupported | |

HID++ 1.0 (byte 5 of a 0x8F error frame). K L221-234, R hidpp-generic.c L40-52:
0x01 InvalidSubId, 0x02 InvalidAddress, 0x03 InvalidValue, 0x04 ConnectFail,
0x05 TooManyDevices, 0x06 AlreadyExists, 0x07 Busy, 0x08 UnknownDevice,
0x09 ResourceError, 0x0A RequestUnavailable, 0x0B InvalidParamValue, 0x0C WrongPinCode.
**Verdict: adopt both tables as enums.**

### Timeouts and retries (prior art)

| Tool | Per-request timeout | Retries | Source |
|---|---|---|---|
| kernel | 5 s (`5*HZ`) | up to 3 attempts, only on BUSY (1.0 0x07 / 2.0 0x08) | K L319-320, L356-381 |
| Solaar | 4 s to a device, 0.9 s to a receiver (2x for long register reads); ping 4 s | none; restarts the clock when an unrelated report arrives | S-b L126-133, L698-701, L784-785 |
| logiops | 500 ms default (`io_timeout`, configurable) | write() retried up to 8 times; device-add backoff of 2^n x base, 5 tries | L `Configuration.h` L31, `RawDevice.cpp` L43/L215, `DeviceMonitor.h` L36 |
| libratbag | blocking read; one 10 ms re-read on ETIMEDOUT | none | R L161-168 |

**Verdict: adapt.**
- Use a 1 s per-request timeout on the control thread. 4-5 s is too long for an interactive
  daemon, and 500 ms is tight for BLE on a busy link.
- Retry up to 3 times on BUSY with a short backoff (about 20 ms, then 40 ms, then 80 ms).
- Don't retry other errors.
- Treat a timeout as "device maybe asleep or disconnected" and fall back to waiting for an event.
- Never block the input path on any of this.

## 1. Root 0x0000 (always at index 0)

| fn | name | request params | reply |
|---|---|---|---|
| 0 | getFeature | `[id_hi, id_lo]` (feature ID **big-endian**) | `[index, type_flags, version]`. Index 0 means not present. |
| 1 | getProtocolVersion / ping | `[0, 0, ping_byte]` | `[major, minor, ping_byte]` (ping byte echoed in param 2) |

- Byte order is BE: K L954, S-h20 L379 (`!H`), R L273, L `Feature.cpp` L47-49.
- **Source disagreement:** logiops' `Root::getFeature` helper builds the ID little-endian
  (L `Root.cpp` L26-29). The path logiops actually uses (`Feature.cpp` L47-49) is BE, so the
  helper is a dormant bug. **Trust BE.**
- Reply byte 1 (type) and byte 2 (version): S-h20 L383-386, R L279-281.
- Ping semantics:
  - The kernel sends ping byte 0x5A and rejects the reply if `params[2]` doesn't echo it
    (K L971-1007).
  - Solaar puts a random byte in param 2 and checks `reply[4]` (byte 2 of the params) against it
    (S-b L818-839).
  - logiops' "stability test" pings five times with the bytes of "hello" and requires every
    echo to match (L `backend/hidpp/Device.cpp` L285-300).
  - A HID++ 1.0 device answers with a 1.0 error InvalidSubId (0x01), meaning protocol 1.0
    (K L984-988, S-b L847-849).
  - ResourceError (0x09) or UnknownDevice (0x08) mean "not connected right now" (K L991-993).
- **Verdict: adopt.**
  - Ping with a rotating mark byte, and treat a matching echo as "online, protocol major.minor".
  - Re-ping after every reconnect before touching features.
  - The 5-ping "stability" check from logiops is optional. Adopt one ping plus feature-index
    revalidation instead.

## 2. FeatureSet 0x0001

| fn | name | params | reply |
|---|---|---|---|
| 0 | getCount | none | `[count]`. Count **excludes** Root, so indices run 0..=count. |
| 1 | getFeatureId | `[index]` | `[id_hi, id_lo, type_flags, version]` |

- Sources: S-h20 L161-176 (`count[0] + 1`), L316-328 (`!H` ID, `[2]` flags, `[3]` version).
  R L320-365. L `FeatureSet.cpp`.
- Type-flag bits: 0x80 obsolete, 0x40 hidden (software-hidden), 0x20 internal/engineering
  (S-c L243-248, L `Root.h`). Prior-art tools define no other bits. Logitech docs reportedly
  also define 0x10 and 0x08 (manufacturing/compliance deactivatable). Unverified.
- **Verdict: adopt.**
  - Enumerate the whole table once per connection and build a `feature_id -> index` map.
  - Don't hard-code indices. They differ per firmware.
  - Keep `version` per feature, because some layouts are version-gated (e.g. 0x1B04's second
    flags byte).

## 14. Kernel coexistence (hid-logitech-hidpp on B042)

- **The kernel binds B042.** The device table entry is "MX Master 4 mouse over Bluetooth",
  `HID_BLUETOOTH_DEVICE(USB_VENDOR_ID_LOGITECH, 0xb042)` with no quirks (K L4958-4959).
- The kernel uses swid 0x01 (K L42), one request at a time under a mutex, a 5 s timeout and
  3 attempts on BUSY.
- What the kernel does on probe and on every connect event (K L4461-4583):
  1. Ping / getProtocolVersion (short report, promoted to long if needed). Ping byte 0x5A.
  2. reprog-controls connect. It is a no-op for B042, because only the M650 has that quirk
     (K L3742-3766, L4945).
  3. Looks up 0x1D4B and remembers its index.
  4. Reads the name via 0x0005 if not yet named.
  5. Initialises battery. It tries 0x1000, then 0x1004, then voltage and ADC (K L4315-4340), and
     queries status. It also listens to battery events from hidraw (K L4169-4186).
  6. On non-USB (BT) devices, it detects 0x2121 (else 0x2120) and calls
     `setWheelMode(target=HID, hires=1, invert=0)`. It reads the multiplier from
     getWheelCapability (K L3904-3974, L4537-4538, L4560-4561).
- What triggers a connect event: any report on the 0x1D4B feature index, or HID++ 1.0 sub_id
  0x41 (K L487-494).
- The kernel does **not** touch 0x1B04 diversion (for B042), 0x2110/0x2111, 0x2150, 0x2201,
  0x1814/0x1815 or 0x19B0. So diverting buttons and the thumbwheel doesn't conflict with it.
  Diverted controls simply stop producing evdev events.
- The kernel's reply matcher compares feature index and the full `fn|swid` byte (K L471-485), so
  it won't swallow our replies. Our hidraw reader sees its replies and all events anyway (KC
  L2174-2180).
- **Conflicts to design around:**
  1. Hi-res wheel mode reset on reconnect ([section 10](hidpp-layouts-input.md#10-hireswheel-0x2121)).
     Detect the kernel's swid-1 setWheelMode reply and re-apply.
  2. Never query the 0x1D4B index ([section 8](hidpp-layouts-device.md#8-wirelessdevicestatus-0x1d4b)).
  3. Volatile state (temporary diverts, 0x2150 divert, 0x2121 mode) is lost on power cycle or
     reconnect. Re-apply on 0x1D4B event 0 with request == 1. Use a short delay (logiops 100 ms,
     Solaar 200 ms), or better, after the kernel's swid-1 wheel write.
  4. Solaar running at the same time will apply *its* persisted settings on the same triggers.
     Document "don't run Solaar's settings writer alongside Detent", or detect swid 0xB traffic
     and warn.
- How Solaar handles foreign swids: it ignores them. A report becomes a notification only with
  swid 0, and a reply must match Solaar's request bytes exactly (S-b L601-633, L729-781). It
  restarts its timeout when an unmatched report arrives, so foreign traffic delays but doesn't
  break it. **Verdict: adopt the same filtering.**

