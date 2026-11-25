---
title: "HID++ 2.0 layouts: buttons, wheels, DPI, haptics"
status: current
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# HID++ 2.0 layouts: buttons, wheels, DPI, haptics

Part of the [HID++ 2.0 feature layouts](hidpp-layouts.md) appendix (MX Master 4, BT,
046D:B042, devidx 0xFF). Source tags (S-h20, S-st, K, L, R, ...) and the licensing note are in
[the index](hidpp-layouts.md#sources-and-versions). Framing, Root, FeatureSet and kernel
coexistence (sections 0-2, 14) are in [hidpp-layouts-protocol.md](hidpp-layouts-protocol.md);
device-level features (sections 3-6, 8) in [hidpp-layouts-device.md](hidpp-layouts-device.md).

## 7. ReprogControlsV4 0x1B04 (button diversion)

| fn | name | params | reply |
|---|---|---|---|
| 0 | getCount | none | `[count]` |
| 1 | getCidInfo | `[index]` | `[cid_hi, cid_lo, tid_hi, tid_lo, flags1, pos, group, gmask, flags2]` |
| 2 | getCidReporting | `[cid_hi, cid_lo]` | `[cid_hi, cid_lo, rflags1, remap_hi, remap_lo, rflags2]` |
| 3 | setCidReporting | `[cid_hi, cid_lo, rflags1, remap_hi, remap_lo, (rflags2)]` | echo of the request |

Sources: S-h20 L91-130 (flag tables), L529-612 (get/set), L761-777 (getCidInfo unpack `!HHBBBBB`).
K L3620-3740. L `ReprogControls.h` L31-73 and `.cpp` L80-184. R L1280-1466.

### getCidInfo capability flags

flags1 is byte 4 and flags2 is byte 8. Solaar treats them as one 16-bit field, `flags1 | flags2 << 8`.

| bit (byte 4) | meaning |
|---|---|
| 0x01 | mouse button |
| 0x02 | F-key |
| 0x04 | hotkey (Solaar: "nonstandard") |
| 0x08 | fnToggle (fn-sensitive) |
| 0x10 | reprogrammable (remappable hint) |
| 0x20 | divertable (temporary divert) |
| 0x40 | persistently divertable |
| 0x80 | virtual (no physical control, e.g. virtual gesture button 0x00D7) |

| bit (byte 8, "additional flags") | meaning |
|---|---|
| 0x01 | rawXY (can divert raw XY while held) |
| 0x02 | forceRawXY |
| 0x04 | analyticsKeyEvents |
| 0x08 | rawWheel |

- pos (byte 5): F-key position (1-16), otherwise 0.
- group (byte 6): the remap group this CID belongs to (0 = none).
- gmask (byte 7): the groups this CID may be remapped *to*.
- Agreement: libratbag reads only bit0 of byte 8 as rawXY (R L1327). logiops names the same
  bits in byte 4 and rawXY in byte 8 (L .h L52-64). The kernel tests divert as byte-4 bit5 and
  mouse as bit0 (K L3626-3627, L3694-3695).

### Reporting flags (getCidReporting and setCidReporting)

Each setting bit is paired with a "valid" (change) bit one position to its left. The device
applies a setting bit only when its valid bit is set, so you can change one flag without
reading the others.

| byte 2 bit | meaning |
|---|---|
| 0x01 | divert (temporary). **Lost on reset or reconnect.** |
| 0x02 | divert valid |
| 0x04 | persistent divert |
| 0x08 | persist valid |
| 0x10 | rawXY divert |
| 0x20 | rawXY valid |
| 0x40 | forceRawXY divert |
| 0x80 | forceRawXY valid |

| byte 5 bit (v4+, optional) | meaning |
|---|---|
| 0x01 | analyticsKeyEvents reporting |
| 0x02 | its valid bit |
| 0x04 | rawWheel |
| 0x08 | its valid bit |

- Sources: S-h20 L116-130 and L583-592 (`value << 1` = valid bit), L `.h` L66-73, R L1455-1463,
  K L3629-3630.
- remap (bytes 3-4): the target CID. 0 means "no change" on set and "not remapped" on get
  (S-h20 L543, L599-607).
  - The kernel sets remap = its own CID (K L3732-3734). libratbag writes the stored remap value
    back (R L1447).
  - **Adopt 0 = keep** when we don't intend to remap.
- byte 5 on set:
  - Solaar deliberately does **not** send it, "older devices might behave oddly" (S-h20 L607-609).
  - In a long report the byte is present anyway and will be 0x00, which with all valid bits
    clear means "no change". That should be safe. **To verify** on the MX4 that a 6th byte of
    0 is harmless.
- logiops reads only cid and flags1 from getCidReporting (L `.cpp` L163-174). Solaar also reads
  remap and flags2.
- **Verdict: adopt.**
  - Use temporary divert (0x03) for buttons and rawXY (0x30) for gesture-button motion. Never
    use persistent divert. It survives Detent dying and would leave the user's buttons dead
    without the daemon, so reject it as a default.
  - Undivert on clean shutdown (0x02 = valid without divert).

### Events (swid 0)

| event (fn nibble) | name | payload |
|---|---|---|
| 0 | divertedButtonsEvent | up to 4 x CID u16 BE, zero-padded. This is the **set of diverted controls currently held**, not a press/release edge. All zero = everything released. |
| 1 | divertedRawMouseXYEvent | `dx i16 BE, dy i16 BE` |
| 2 | analyticsKeyEvents | layout undocumented in prior art. Solaar only logs it (S-n L373-375). |

- Sources: S-n L364-377 (`!HHHH`, `!hh`), K L3768-3807 (four BE u16, presence = pressed),
  L `.cpp` L138-161 (stops at the first zero CID).
- **Verdict: adopt.** Derive press and release edges by diffing consecutive held-sets. This is
  allocation-free with a `[u16; 4]`.
- Is there an event 3 for rawWheel? Not seen in any prior art. **To verify** on the device if we
  ever set rawWheel.

### MX Master family CIDs (from S-d special_keys.py L116-122, L218-238, L318-326)

| CID | name |
|---|---|
| 0x0050 | left |
| 0x0051 | right |
| 0x0052 | middle |
| 0x0053 | back |
| 0x0056 | forward |
| 0x00C3 | gesture/thumb button ("Mouse_Gesture_Button") |
| 0x00C4 | SmartShift (top) button |
| 0x00D7 | virtual gesture button |
| 0x01A0 | "Haptic" (likely the MX4 haptic sense panel / Actions Ring trigger) |

**To verify:** dump getCidInfo for every index on the B042 and record which CIDs exist and which
are divertable and rawXY-capable. Don't trust this table for the MX4.

## 9. SmartShift 0x2110 and SmartShift Enhanced 0x2111

0x2110 (v1):

| fn | name | params | reply |
|---|---|---|---|
| 0 | getRatchetControlMode | none | `[mode, autoDisengage, autoDisengageDefault]` |
| 1 | setRatchetControlMode | `[mode, autoDisengage, autoDisengageDefault]` | echo |

0x2111 (enhanced, expected on the MX4):

| fn | name | params | reply |
|---|---|---|---|
| 0 | getCapabilities | none | `[capFlags, defaultAutoDisengage, defaultTorque, maxForce]`. capFlags bit0 = tunable torque. |
| 1 | getRatchetControlMode | none | `[mode, autoDisengage, torque]` |
| 2 | setRatchetControlMode | `[mode, autoDisengage, torque]` | echo |

- mode: 1 = freespin (permanently), 2 = ratchet. With mode 2, SmartShift auto-disengages above
  the threshold speed. **0 = leave unchanged.**
- autoDisengage: the speed threshold. 0 = unchanged, 0xFF = never disengage (always ratchet).
  Solaar's UI range is 1..=50, and it writes 255 for "max" (S-st L698-737).
- torque: 1..=100 (S-st L750-779). 0 = unchanged.
- Sources:
  - L `SmartShift.h/.cpp`: fn numbers for both versions. Mode is "active = mode-1". Set uses 0
    for "don't change" per field. V2 capabilities are bytes 0-3.
  - S-st L688-747: ScrollRatchet read fn0/write fn1 for 0x2110 and read fn1/write fn2 (0x10/0x20)
    for 0x2111. SmartShift writes `[0, threshold]` to keep the mode.
  - S-st L759-779: torque is written as `[mode, 0, torque]`.
- **MX Master 4 quirk (Solaar, S-st L759-766):** a torque change takes effect only if the mode
  byte is written in the same request. 0 (no change) isn't enough. Solaar reads the current
  ratchet setting and puts it in byte 0.
- The SmartShift button (CID 0x00C4) toggles mode on-device unless diverted. The device then
  emits 0x2121 event 1 (ratchet switch), and Solaar uses that to update its UI (S-n L386-392).
- **Verdict: adopt.**
  - Detect 0x2111 first and fall back to 0x2110.
  - Always send an explicit mode byte when changing torque.
  - Verify the "0 = unchanged" semantics for autoDisengage on the MX4. Prior art relies on it,
    but no MX4 log confirms it.

## 10. HiResWheel 0x2121

| fn | name | params | reply |
|---|---|---|---|
| 0 | getWheelCapability | none | `[multiplier, flags]`. flags bit2 = has ratchet switch, bit3 = invert supported. |
| 1 | getWheelMode | none | `[mode]` |
| 2 | setWheelMode | `[mode]` | echo `[mode]` |
| 3 | getRatchetSwitchState | none | `[state]`. bit0: 1 = ratchet, 0 = freewheel. |

mode bits: bit0 **target** (0 = native HID wheel reports, 1 = divert to HID++ events), bit1
**resolution** (1 = hi-res, scaled by multiplier), bit2 **invert**.

Events (swid 0):

| event | payload |
|---|---|
| 0 wheelMovement | byte 0: bit4 = hi-res flag, bits 0-3 = periods. Bytes 1-2: deltaV i16 BE. |
| 1 ratchetSwitch | byte 0: 0 = freewheel, 1 = ratchet |

- Sources: K L2040-2094 (fn0 multiplier = byte 0; set fn 0x20 with invert/hires/target bits
  2/1/0). S-h20 L2030-2054 (cap flags 0x08 invert and 0x04 ratchet; mode bits; fn3 bit0).
  L `HiresScroll.h/.cpp` (same, plus event layouts). S-n L379-392. S-d `diversion.py` L431-438
  (deltaV is `d[1:3]` signed).
- **Kernel behaviour (critical):** on every connect the kernel on a BT device calls
  `setWheelMode(invert=0, hires=1, target=0)` with swid 1 (K L3904-3936, L4536-4561, called at
  L4561). This **overwrites any mode Detent set**, including invert and divert.
- The kernel also snoops every setWheelMode *reply* on the 0x2121 index, from any swid. It
  updates its own multiplier from bit1 (K L3976-3999). Our own writes therefore keep the kernel's
  scaling consistent.
- Solaar sleeps 200 ms before applying settings "to get out of race condition with Linux HID++
  driver" (S-d `settings.py` L877-885).
- **Verdict: adapt.**
  - Keep target = 0 (native) by default and let the kernel and evdev scroll.
  - Divert (target = 1) only if Detent must own the wheel.
  - Whenever Detent wants a non-default mode, watch hidraw for a setWheelMode reply with
    **swid 1** on the 0x2121 index. That is the kernel's connect worker. Re-apply our mode right
    after it. This beats a fixed 200 ms sleep, and keep the sleep only as a fallback.

## 11. Thumbwheel 0x2150

| fn | name | params | reply |
|---|---|---|---|
| 0 | getInfo | none | `[nativeRes u16 BE, divertedRes u16 BE, defaultDirection, capabilities, timeElapsed u16 BE]` |
| 1 | getStatus | none | `[divert, flags]` |
| 2 | setReporting | `[divert, invert]` | `[divert, flags]` |

- getInfo bytes 0-7: defaultDirection non-zero means positive = rightward. capabilities: bit0
  timestamp, bit1 touch, bit2 proximity, bit3 single-tap.
- getStatus flags: bit0 invert, bit1 touch, bit2 proxy.

Event 0 (swid 0), payload bytes:
- 0-1: rotation i16 BE.
- 2-3: timestamp u16 BE.
- 4: rotationStatus. 0 inactive, 1 start, 2 active, 3 stop.
- 5: flags. bit1 touch, bit2 proximity, bit3 single-tap (same bit positions as the capabilities).

- Sources: L `ThumbWheel.h/.cpp` (complete layouts). L `features/ThumbWheel.cpp` L155-223 (flag
  use; uses divertedRes as the hi-res multiplier). S-st L515-532 (ThumbMode = byte 0 bit0,
  ThumbInvert = byte 1 bit0, read fn1/write fn2). S-d `diversion.py` L375-398, L1486-1489
  (rotation = `d[0:2]` signed; resets the accumulator when byte 4 <= 1).
- The kernel does **not** touch 0x2150. Undiverted, the thumbwheel arrives as a native HID
  horizontal wheel (AC Pan).
- **Verdict: adopt.** Divert only when bound to a non-scroll action, and accumulate by
  divertedRes. On MX-family mice, touch and proximity flags may be unsupported (check the
  capabilities byte). **To verify** on the device.

## 12. AdjustableDPI 0x2201 and ExtendedAdjustableDPI 0x2202

0x2201:

| fn | name | params | reply |
|---|---|---|---|
| 0 | getSensorCount | none | `[count]` |
| 1 | getSensorDpiList | `[sensor]` | `[sensor, dpi u16 BE, dpi u16 BE, ..., 0x0000]` |
| 2 | getSensorDpi | `[sensor]` | `[sensor, current u16 BE, default u16 BE]`. current 0 means use default (Solaar). |
| 3 | setSensorDpi | `[sensor, dpi u16 BE]` | echo |

**Range encoding in the list:** a word with its top 3 bits set (`>= 0xE000`) is a step marker,
and `word & 0x1FFF` is the step. The list `[min, 0xE000|step, max, 0]` means
`min, min+step, ..., max`. Discrete lists are just words, terminated by 0.

Sources and disagreements:
- S-st L1035-1077: Solaar treats the marker as "step, then next word = last value", and expands
  from the previous entry.
- L `AdjustableDPI.cpp`: `>= 0xE000`, step = word - 0xE000. It records isRange and step but
  doesn't use the word after.
- R L1532-1577: `> 0xE000` (excludes step 0). It takes min and max from the non-marker words.
- All three agree for real devices. Solaar's interpretation is the most complete. **Adopt
  Solaar's.**
- getSensorDpi: R L1580-1605, L `.cpp`, S-st L1079-1085.
- **Verdict: adopt.** Validate requested DPI against the expanded list or range before setting.
  Out-of-range values produce OutOfRange (0x03).

0x2202 (if the MX4 exposes it instead; prior art has only Solaar):
- fn1 (0x10) getSensorCapabilities `[sensor]`: byte 2 bit0 = separate Y DPI, bit1 = LOD
  supported.
- fn2 (0x20) getSensorDpiRanges `[sensor, direction(0 = X, 1 = Y), page]`: 3 header bytes, then
  the same word list and 0xE000 step encoding. Multi-page until a 0x0000 terminator.
- fn5 (0x50) getSensorDpiParameters `[sensor]`: `[sensor, dpiX, defaultX, dpiY, defaultY, lod]`
  (u16 BE for each DPI).
- fn6 (0x60) setSensorDpiParameters `[sensor, dpiX, dpiY, lod]`.
- Source: S-st L1088-1166.
- **Verdict: adapt.** Implement only if the MX4 enumerates 0x2202. **To verify** which DPI
  feature the B042 exposes (likely 0x2201).

## 13. HAPTIC 0x19B0 (MX Master 4)

The only prior art is Solaar (S-c L71, L368-385; S-st L4369-4432; docs/capabilities.md L217-220).
The community `mx4-haptic-linux` project just shells out to `solaar config ... haptic-play`
(https://github.com/Physics-of-Data/mx4-haptic-linux, no license, no protocol code). We found no
public Logitech spec.

| fn | Solaar use | params | reply |
|---|---|---|---|
| 0 (0x00) | capabilities | none | bytes 4..=7 = u32 BE **bitmap of supported waveform IDs** (bit n = waveform n). Bytes 0-3 unknown. |
| 1 (0x10) | get config | none | byte 0 bit0 = enabled, byte 1 = level (0-100), byte 2 bit0 = "only 4 discrete levels" |
| 2 (0x20) | set config | `[enabled(bit0), level]` | ack |
| 3 (0x30) | unknown | n/a | n/a |
| 4 (0x40) | play waveform | `[waveformId]` | ack |

- Solaar settings:
  - `haptic-level` (0 = off; otherwise 0-100, or Off/Low 25/Medium 50/High 75/Max 100 when
    byte 2 bit0 is set). To disable, Solaar writes `[0x00, 0x32]`, keeping 50 % stored.
  - `haptic-play` is hidden from the UI, CLI only, and not persisted.
- Waveform IDs (S-c L368-385):

| ID | waveform |
|---|---|
| 0x00 | sharp state change |
| 0x01 | damp state change |
| 0x02 | sharp collision |
| 0x03 | damp collision |
| 0x04 | subtle collision |
| 0x05 | happy alert |
| 0x06 | angry alert |
| 0x07 | completed |
| 0x08 | square |
| 0x09 | wave |
| 0x0A | firework |
| 0x0B | mad |
| 0x0C | knock |
| 0x0D | jingle |
| 0x0E | ringing |
| 0x1B | whisper collision |

  That is 16 total, matching the "16 waveforms" reported by users.
- Unknown: bytes 0-3 of fn0, what fn3 does, whether any haptic event exists, and the exact
  ack layout.
- **Verdict: adapt.**
  - Implement level get/set and play(waveformId), gated on the fn0 bitmap.
  - Treat everything else as **unknown HID++ feature, so don't write it**. Per AGENTS.md, writing
    unknown functions needs owner sign-off.
  - Verify fn0/fn1 byte layouts by reading them on the device before enabling writes. Read-only
    probes are safe. Playing a waveform is a harmless write, but confirm with the owner before
    exercising set-config.

