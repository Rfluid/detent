---
title: "MX Master 4 on Linux (research appendix)"
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# MX Master 4 (046D:B042) on Linux: prior-art research

Researched 2026-11-23 for Detent (MIT). Only facts are extracted here. No GPL code was copied.

Legend:
- **CONFIRMED**: seen in a public dump or capture from real hardware (solaar show, hidpp-list-features, logid debug, a byte capture).
- **REPORTED**: stated by one source without a dump, or inferred from code.
- **GUESS**: our inference.
- Verdicts: **adopt** (use as is), **adapt** (use with changes), **reject** (don't use).

Licenses of the sources:
- Solaar: GPL-2.0.
- logiops: GPL-3.0. A third-party README calls it MIT; that is wrong. Its file headers say GPLv3.
- Linux kernel: GPL-2.0.
- logitune: GPL-3.0.
- OpenLogi: repo Apache-2.0; `openlogi-hidpp` crate 0BSD.
- lus/logy `hidpp` crate: 0BSD.
- familiar: MIT.
- WhiteKr/mx4: MIT.
- UsiDiamond/mx-master-4-linux-desktop: MIT.
- gnacho/mx-master4-linux: AGPL-3.0.

Only the code of OpenLogi, logy, familiar and WhiteKr/mx4 could legally be reused (with attribution). We still extract only facts.

---

## 1. Device identity and feature table

**CONFIRMED**, from three independent dumps:
- Solaar #2964, two `solaar show` dumps over Bolt: https://github.com/pwr-Solaar/Solaar/issues/2964
- `hidpp-list-features` over Bluetooth hidraw (046d:b042), logiops #520, comment by kris7t: https://github.com/PixlOne/logiops/issues/520#issuecomment-3456346930
- familiar's protocol notes, one unit over Bolt: https://github.com/Gingerbreadfork/familiar/blob/main/docs/protocol/mx-master-4.md

Identity:
- HID++ 4.5.
- Model ID `B04200000000`, transport `btleid B042`.
- Firmware RBM 27.00.B0015, bootloader LD 04.00, hardware 72.
- The Business variant is B048, firmware RBM 27.02.B0017, with the same features (Solaar #3264: https://github.com/pwr-Solaar/Solaar/issues/3264).

The table has 46 features. The feature indexes are **identical over Bluetooth and Bolt**: the BT dump and the Bolt dumps agree.

| Idx | ID | Name | Ver | Notes |
|---|---|---|---|---|
| 0x00 | 0000 | Root | 0 | |
| 0x01 | 0001 | Feature Set | 0 (familiar reads v2) | |
| 0x02 | 0003 | Device FW Version | 8 | |
| 0x03 | 0005 | Device Name | 5 | "MX Master 4", kind mouse |
| 0x04 | 1D4B | Wireless Device Status | 0 | reconnect/reconfigure notifications |
| 0x05 | 0020 | Config Change (cookie) | 1 | |
| 0x06 | 0021 | Crypto ID | 1 | |
| 0x07 | 0007 | Device Friendly Name | 0 | |
| 0x08 | 0011 | Property Access | 0 | |
| 0x09 | 1004 | Unified Battery | 5 | |
| 0x0A | 1701 | unknown | 0 | fn0 returns `01 00 0a` |
| 0x0B | **19B0** | **Haptic** | 0 | |
| 0x0C | **19C0** | **Force Sensing Button** | 0 | the thumb panel's press force |
| 0x0D | 1B04 | Reprog Controls v4 | 6 | |
| 0x0E | 1814 | Change Host | 2 | |
| 0x0F | 1815 | Hosts Info | 2 | |
| 0x10 | 2250 | XY Stats | 1 | |
| 0x11 | 2111 | SmartShift Enhanced | 0 | |
| 0x12 | 2121 | HiRes Wheel | 1 | |
| 0x13 | 2150 | Thumb Wheel | 0 | |
| 0x14 | 2201 | Adjustable DPI | 2 | |
| 0x15 | 2251 | Wheel Stats | 0 | |
| 0x16 | 00D1 | DFU | 0 | |
| 0x17 | 1802 | Device Reset | 0 | hidden, internal |
| 0x18 | 1803 | ? | 1 | hidden, internal |
| 0x19 | 1807 | ? | 4 | hidden, internal |
| 0x1A | 1816 | ? | 0 | hidden, internal |
| 0x1B | 1805 | OOB State | 0 | hidden, internal |
| 0x1C | 1830 | ? | 0 | hidden, internal |
| 0x1D | 1891 | ? | 9 | hidden, internal |
| 0x1E | 18A1 | ? | 0 | hidden, internal |
| 0x1F | 1E00 | Enable hidden features | 0 | hidden |
| 0x20 | 1E02 | ? | 0 | hidden, internal |
| 0x21 | 1E22 | ? | 1 | hidden, internal |
| 0x22 | 1E30 | ? | 0 | hidden, internal |
| 0x23 | 1602 | ? (BLE password auth?) | 0 | |
| 0x24 | 1EB0 | ? | 0 | hidden, internal |
| 0x25 | 1861 | ? | 1 | hidden, internal |
| 0x26 | 9205 | ? | 1 | hidden, internal |
| 0x27 | 9201 | ? | 0 | hidden, internal |
| 0x28 | 9300 | ? | 1 | hidden, internal |
| 0x29 | 9401 | ? | 0 | hidden, internal |
| 0x2A | 9402 | ? | 0 | hidden, internal |
| 0x2B | 9001 | ? | 1 | hidden, internal |
| 0x2C | 18B1 | ? | 0 | hidden, internal |
| 0x2D | 18C0 | ? | 0 | hidden, internal |

- **Adopt**: resolve every feature through Root.getFeature (0x0000 fn0) at connect. Never hardcode indexes. kris7t in logiops #520 makes the same point.
- **Adapt**: the table is a test fixture and a sanity check for our device. It is not a source of truth.
- **Reject**: anything in 18xx/1Exx/9xxx (internal/hidden). Never write to these: unknown and risky.

## 2. Reprogrammable controls (0x1B04 v6)

**CONFIRMED**:
- familiar's capture gives CIDs, task IDs and flags: https://github.com/Gingerbreadfork/familiar/blob/main/docs/protocol/mx-master-4.md (lines 84-97).
- The logid debug table matches the CIDs: https://github.com/PixlOne/logiops/issues/521
- The Solaar #2964 flags agree, including group 2 and gmask g1,g2 for the divertable keys.

| Idx | CID | Name | Default task | Flags | Group / gmask | Plain HID when not diverted |
|---|---|---|---|---|---|---|
| 0 | 0x0050 | Left | 0x0038 | mse, analytics | 1 / – | BTN_LEFT |
| 1 | 0x0051 | Right | 0x0039 | mse, analytics | 1 / – | BTN_RIGHT |
| 2 | 0x0052 | Middle | 0x003A | mse, reprog, divertable, rawXY | 2 / g1,g2 | BTN_MIDDLE |
| 3 | 0x0053 | Back | 0x003C | mse, reprog, divertable, rawXY | 2 / g1,g2 | BTN_SIDE |
| 4 | 0x0056 | Forward | 0x003E | mse, reprog, divertable, rawXY | 2 / g1,g2 | BTN_EXTRA |
| 5 | 0x00C3 | Gesture button (front thumb button) | 0x009C | mse, reprog, divertable, rawXY | 2 / g1,g2 | – |
| 6 | 0x00C4 | Smart Shift (behind wheel) | 0x009D | mse, reprog, divertable, rawXY | 2 / g1,g2 | handled in firmware |
| 7 | **0x01A0** | **Haptic Sense Panel ("Haptic", the thumb panel)** | **0x0109** (unnamed in Solaar/logid) | mse, reprog, divertable, rawXY | 2 / g1,g2 | **BTN_BACK** (button 7), per familiar over Bolt |
| 8 | 0x00D7 | Virtual Gesture Button | 0x00B4 | divertable, virtual, rawXY, force-rawXY | 3 / – | – |

- **None of the controls is persistently divertable.** Diversion is temporary and lost on sleep or reconnect, so the daemon must re-apply it (familiar lines 96-97). **Adopt.**
- In Solaar's CID table, 0x01A0 is named "Haptic": https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/special_keys.py line 318. 0x00C3 is Mouse_Gesture_Button (line 218), 0x00C4 is Smart_Shift (line 219), 0x00D7 is Virtual_Gesture_Button (line 238).
- The logiops CID map is in #520 and #529 (https://github.com/PixlOne/logiops/pull/529): 0x1a0 is the "big thumb pad", 0xc3 the gesture button, 0xc4 the top button.
- Diverted press capture from logitune PR #127, over Bolt with device index 2:
  - Press: `11 02 0d 00 01 a0 ...` (feature 0x0D = 1B04, event 0 divertedButtonsEvent, CID list).
  - Release: `11 02 0d 00 00 00 ...`.
  - Source: https://github.com/mmaher88/logitune/pull/127
- Raw XY (familiar lines 123-128): event 1 carries dx and dy as BE i16 while the control is held, about 78 events/s. **The first event 1 after each press is garbage** (seen `-6125,-7389`) and must be dropped. **Adopt** as a quirk to verify.
- **How the Actions Ring works** (REPORTED/GUESS, from OpenLogi `crates/openlogi-device/src/session/gesture.rs` lines 163-177):
  - The panel is a gesture source like 0x00C3. It is diverted with raw XY.
  - The "panel gestures" (up/down/left/right/click) are **pointer movement while the panel is pressed**. They are not touch swipes on the panel itself.
  - familiar's open question: does Options+ use 0x00D7 with force-raw-XY for the ring? Force-raw-XY freezes the pointer.
  - Verdict: **adapt**. Detent's radial menu should divert 0x01A0 with raw XY and accumulate deltas.
- 0x0109 also shows up as a Solaar *control* name ("Multiplatform_App_Switch__Launchpad", special_keys.py line 288). The task namespace differs, so this is irrelevant.
- logiops #556 (https://github.com/PixlOne/logiops/issues/556): after a Fedora 44 upgrade, the 0x1a0 remap started acting as KEY_BACK. This is consistent with the undiverted panel emitting BTN_BACK and with diversion being lost. GUESS: the diversion was lost or the kernel driver now binds. Verify on device.

## 3. Haptics: feature 0x19B0 ("Haptic" / "HapticFeedback")

Not in Logitech's public HID++ docs. Everything here was reverse-engineered by the community, but several sources cross-check.

### Function layouts

| Fn | Name (community) | Request | Reply | Sources |
|---|---|---|---|---|
| 0 | getCapabilities | – | bytes 0..3 unknown; **bytes 4..7 = BE u32 bitmask, bit N = waveform ID N supported** | Solaar `settings_templates.py` lines 4425-4431; OpenLogi `haptic_feedback.rs` lines 163-179; familiar line 69 (raw `00 01 00 3c 00 00 7f ff`, waveforms 0-14 on that unit) |
| 1 | getConfig / getLevel | – | byte0 flags: bit0 ENABLED, bit1 POWER_SAVE (battery saving), other bits keep; byte1 intensity 0..100; byte2 levels (Solaar: bit0 = "only 4 discrete levels"; OpenLogi: hi-nibble step, lo-nibble count) | Solaar lines 4382-4406; OpenLogi lines 37-113; logiops PR #524 (`kVibration=0x01`, `kBatterySaving=0x02`) |
| 2 | setConfig / setLevel | `[flags, intensity, 0]` | same layout as fn1 (device's applied state) | Solaar lines 4389-4395 (off = `00 32`: disabled, keep 50%); OpenLogi lines 199-210; logiops PR #524 SetStrength=2 |
| 3 | getHapticInfo / status | – | byte0 status: 0 = ready. Options+ refuses to play while non-zero (per OpenLogi's reverse engineering of Options+ 2.8) | OpenLogi lines 124-153, 212-222 (REPORTED; not in Solaar) |
| 4 | playWaveform | `[waveform_id, 0, 0]` | ack | Solaar `rw_options write_fnid 0x40` lines 4412-4432; logiops PR #524 PlayEffect=4; familiar line 72; OpenLogi line 226 |

Raw frames that worked on hardware (CONFIRMED by users), from Solaar #3073 (https://github.com/pwr-Solaar/Solaar/issues/3073), maintainer pfps:
- Bluetooth: `11 FF 0B 4C 03 00…` (long report, devidx 0xFF, feat idx 0x0B, fn 4 | swid 0xC, waveform 3). Played over BT with `hidconsole -i B042`.
- Bolt slot 2: `11 02 0B 45 00 00…` plays SHARP_STATE_CHANGE.
- WhiteKr/mx4 tests build `11 FF 0B 41 <wf>` (fn 4, swid 1). MIT; https://github.com/WhiteKr/mx4 `crates/mx4-core/src/haptics.rs` line 150.

### Waveform IDs

Source: Solaar `hidpp20_constants.py` lines 368-385 (https://github.com/pwr-Solaar/Solaar/blob/master/lib/logitech_receiver/hidpp20_constants.py), added in PR #3024 (https://github.com/pwr-Solaar/Solaar/pull/3024, closed 2025-11).

| ID | Name |
|---|---|
| 0x00 | SHARP_STATE_CHANGE |
| 0x01 | DAMP_STATE_CHANGE |
| 0x02 | SHARP_COLLISION |
| 0x03 | DAMP_COLLISION |
| 0x04 | SUBTLE_COLLISION |
| 0x05 | HAPPY_ALERT |
| 0x06 | ANGRY_ALERT |
| 0x07 | COMPLETED |
| 0x08 | SQUARE |
| 0x09 | WAVE |
| 0x0A | FIREWORK |
| 0x0B | MAD |
| 0x0C | KNOCK |
| 0x0D | JINGLE |
| 0x0E | RINGING |
| 0x1B | WHISPER_COLLISION |

Options+ usage, from usbmon captures (kris7t, logiops #520):
- 0 = turning haptics on, and the desktop-switch gesture ("very prominent, two clicks").
- 4 = Actions Ring hover ("single subtle click").
- 8 = intensity preview.

OpenLogi:
- 1 (DAMP_STATE_CHANGE) = ring action activated.
- 4 (SUBTLE_COLLISION) = ring hover.
- The bitmask is decoded the same way as Options+ 2.8.

The supported set varies by unit or firmware:
- familiar's unit: mask 0x7FFF (IDs 0-14, no 0x1B).
- UsiDiamond's README says its unit supports only SHARP/DAMP/SUBTLE_COLLISION and HAPPY_ALERT (https://github.com/UsiDiamond/mx-master-4-linux-desktop README lines 111-113).
- **Adopt**: always read the fn0 mask and refuse IDs the device doesn't advertise.

Intensity:
- 0-100 percent. Options+ presets per kris7t: subtle 15, low 45, medium 60, high 100.
- Solaar's choice set when the device reports 4 levels: 0/25/50/75/100.
- familiar's unit reported "continuous", intensity 100.
- Off: Solaar writes `00 32` (flags=0, keep 50). familiar reports that writing 0 sends `00 <previous>`.

Timing and safety:
- familiar (lines 151-156): fn4 accepts a new waveform every 12-25 ms with no busy error, even back to back. **Adopt**: rate-limit anyway; OpenLogi coalesces ring hovers so "one stationary pointer cannot flood the HID++ haptic queue".
- Play is non-persistent and harmless. Solaar marks haptic-play `persist=False`, so it never writes it at startup.
- OpenLogi quirk (`openlogi-device/src/write/haptic.rs` lines 255-327): "some power transitions clear the enable flag, after which play calls are accepted but produce no physical feedback". OpenLogi re-arms ENABLED (keeping intensity, power-save and unknown bits) once per ring session, and restores it afterwards. **Adapt**: on reconnect, re-assert the user's configured flags and intensity; don't silently force enable.
- Options+ battery saving turns haptics off at 10% battery (Logitech hub guide: https://hub.sync.logitech.com/mx-master-4/post/haptic-feedback-guide-fgAEJmLZXL9iQXK). This maps to byte0 bit1.
- Solaar PR #3341 (https://github.com/pwr-Solaar/Solaar/pull/3341, merged): fn1 can return no reply while the mouse is asleep or the BT link hiccups. Handle None.
- Solaar PR #3346 (open): `int.from_bytes` without byteorder breaks on Python 3.10. A Python-only issue; the mask is big-endian.
- Latency: Solaar's CLI takes about 1-3 s per haptic, so people write directly to hidraw in under 1 ms (Solaar #3073; WhiteKr/mx4 README lines 150-151). **Adopt**: Detent plays haptics from the device thread over the already-open hidraw.

**Reject**:
- The lukasfri/mx4notifications payload "0x0B4E … 0x01 0x00 0x00 – 0x0E" hardcodes index and swid. kris7t calls it "likely bogus". https://github.com/lukasfri/mx4notifications
- The unverified "send 100 after the waveform byte" variant (a search summary mentions OpenLogi or a fork). All verified senders pad with zeros.

## 4. Force Sensing Button (0x19C0), the thumb panel's click force

Sources: Solaar `hidpp20.py` lines 2321-2400, familiar line 73-75, OpenLogi `openlogi-core/src/hid/force_sensing.rs` lines 49-55.

Functions:
- fn0: count (=1).
- fn1(n): `flags:u16 (bit0 changeable), default:u16, max:u16, min:u16`.
- fn2(n): current u16.
- fn3: `[n, value:u16]` sets the force.

Ranges differ by unit, so they must be read at runtime:
- familiar: default 4630, max 6158, min 3704.
- OpenLogi fixture: default 5089, min 4071, max 6768.

Verdict: **adapt**. Expose it as a setting, clamped to the min/max read from the device. Writing is a user-visible, persistent device change, so confirm with the owner before the first write.

## 5. Other MX4 setting facts (familiar table, lines 56-77; CONFIRMED on one unit)

- **2201 DPI**:
  - fn1 list: 200-8000 in steps of 50, encoded as `0xE000|step`.
  - fn2 reads `sensor, cur, default` (1000/1000).
  - fn3 sets.
- **2111 SmartShift Enhanced**:
  - fn0 caps `01 0a 4b`: bit0 tunable torque, default threshold 10, default torque 75.
  - fn1 get / fn2 set `[mode(1 free, 2 ratchet), threshold(0xFF never), torque]`.
  - **The mode byte must be sent for torque and threshold to apply.** Solaar `settings_templates.py` line 762: "for MX Master 4, the ratchet setting needs to be written for changes to take effect".
  - **Mode is volatile across sleep**: gnacho/mx-master4-linux README reports it reset after sleep, fixed by re-writing.
  - **Adopt**: re-apply on every reconnect.
- **2121 HiRes Wheel**:
  - fn0: multiplier 15, flags 0x08 invert-capable, 0x04 ratchet switch.
  - fn1/2 mode bits: bit0 diverted, bit1 hi-res, bit2 invert.
  - Event 0 is `flags, delta:i16`.
- **2150 Thumb Wheel**:
  - Native res 20, diverted res 120 per turn. Capabilities: touch + timestamp, **no tap** and no proximity (also in the logid #521 log).
  - Event layout: `delta:i16, elapsed_ms:u16, phase(1 start, 2 active, 3 stop, 0 inactive), flags(0x02 touch)`.
- **1815**: fn4 `setHostFriendlyName` writes 14-byte chunks.

## 6. Kernel `hid-logitech-hidpp` and B042

- **CONFIRMED**: B042 is in the driver's BT ID table. drivers/hid/hid-logitech-hidpp.c lines 4958-4959 (torvalds/linux master), commit `70031e70ca15` "HID: logitech-hidpp: Enable MX Master 4 over bluetooth" (2026-03-12), merged in the hid-for-linus 2026-03-17 pull. B048 is not listed.
- With the driver bound, it probes and **enables hi-res scrolling through 0x2121** (`hi_res_scroll_enable`, lines 3904-3924: set_wheel_mode(invert=false, hires=true)). It talks on the same HID++ channel with **SW ID 0x01** (line 42 `LINUX_KERNEL_SW_ID`).
  - **Adopt**: Detent must use a different SW ID (not 0x01) and match replies by feature, function and SW ID.
  - **Adopt**: don't toggle 0x2121 hi-res in a way that fights the kernel's evdev scaling.
- logiops #544 (https://github.com/PixlOne/logiops/issues/544): "scroll sluggish with new kernel" on MX4 BT. It broke between 6.19.11 and 6.19.12, 6.12.76 and 6.12.82, and on 7.0.x. GUESS: the backport of the commit above changed scroll handling, colliding with logid's hiresscroll. logid also failed to open the device at boot on those kernels.
- logiops #565 (https://github.com/PixlOne/logiops/issues/565): on kernel 7.0, BT needs `hires: true` and Bolt needs `hires: false`. That's because the kernel driver binds B042 over BT, but the Bolt path differs. familiar notes `hid-generic` bound to its Bolt receiver.
- OpenLogi notes (gesture.rs lines 84-90) that Linux 7.3 `hid-logitech-hidpp` re-reports diverted Back/Forward as evdev buttons on a BT Signature M650 (`HIDPP_QUIRK_HIDPP_REPROG_CONTROLS_BTNS`, M650 only). **Verify on the MX4** that diverted CIDs do not also arrive on evdev.
- The owner's machine runs Linux 7.3.4, so the driver binds B042 there. Expect kernel-initiated hi-res mode and kernel traffic on hidraw9.

## 7. Sleep, reconnect and lost diversion

- familiar: nothing is persistently divertable, so re-divert on every wake or reconnect. Settings come back about 50 ms after reconnect (README line 108).
- Solaar #3377 (https://github.com/pwr-Solaar/Solaar/issues/3377, open), MX4 over Bolt:
  - If the mouse is asleep at first probe, 1B04 lookups get no reply and Solaar caches "no REPROG_CONTROLS", never retrying.
  - Re-running full detection during a reconnect burst "discovered" features the MX4 doesn't have.
  - **Adopt**: treat a timeout as "unknown, retry", never as "unsupported". Re-probe on 0x1D4B notifications. Don't trust the burst of replies right after reconnect.
- Solaar PR #3329 (https://github.com/pwr-Solaar/Solaar/pull/3329, open), MX4 over BT:
  - On reconnect the device sends the online notification, then about 1 s later a `WIRELESS_DEVICE_STATUS` (0x1D4B) event with `data[1]==1` ("software reconfiguration requested").
  - Solaar's config-cookie dedup skipped the second push, so DPI wasn't applied.
  - **Adopt**: always re-apply the full config (diversion, DPI, SmartShift, haptic flags) on a 1D4B reconfigure request, even if a push just happened.
- Solaar #3046: diverted buttons on the MX4 seemed to stay diverted "longer than older mice". The owner fixed it by undiverting and rebooting. **Adopt**: on clean shutdown, restore diversion to default so a crashed daemon doesn't leave dead buttons. Leases already expire.
- logiops #564: logid's 500 ms io_timeout is too short at boot over Bolt; 3000 ms works. logiops #129 covers idle drop-outs. **Adapt**: use generous first-contact timeouts with backoff.

## 8. Logi Options+ behaviour (public)

Sources:
- Logitech haptics page: https://www.logitech.com/en-us/software/logi-options-plus/haptics.html
- Logitech hub guide: https://hub.sync.logitech.com/mx-master-4/post/haptic-feedback-guide-fgAEJmLZXL9iQXK

What they say:
- Haptics are on by default.
- The Haptic Sense Panel opens the **Actions Ring**, which needs Options+ 1.83+. A haptic plays when moving between bubbles and when an action is selected (OpenLogi maps these to waveforms 4 and 1).
- Haptics on gestures (desktop switch), Smart Actions steps, and Marketplace plugins (Zoom, Photoshop, Lightroom).
- Battery saving turns haptics off at 10% battery.
- **Native Windows haptics** (Windows 11 from end of March 2026) need a firmware update from Options+. Intensity is set in Windows settings.
  - VERIFIED 2026-11-25 (owner's unit, BT): the report descriptor has a standard HID haptics collection (usage page 0x0E, feature report 0x3C, output report 0x3D) next to HID++ 0x19B0. Firmware version not recorded with it. Fact: `.agent/memory/facts/2026-11-25-mx4-bt-report-descriptor.md`.

## 9. Existing Rust HID++ crates and projects

| Crate / repo | License | Maturity | MX4 support | Verdict |
|---|---|---|---|---|
| `hidpp` 0.2.0 (lus/logy, https://github.com/lus/logy) | 0BSD (repo); crates.io says "non-standard" | experimental, 22★, last push 2025-12 | no 19B0 | reject as a dependency (async, experimental); fine as a reference |
| `openlogi-hidpp` 0.9.0 + `-derive`, `openlogi-hid`, `openlogi-device` … (AprilNEA/OpenLogi, https://github.com/AprilNEA/OpenLogi) | crate 0BSD, repo Apache-2.0 | very active, 26k★, v0.9.0 2026-11-03; hard fork of `hidpp` | **yes**: 19B0, 19C0, CID 0x1A0, Actions Ring, haptic restore | **adapt**: best reference for layouts and quirks. Too heavy to depend on (async/tokio, `async-hid`) for an allocation-free hot path. Its 0BSD license would allow borrowing small pieces |
| `hidpp-transport` 0.1.0 (ahmetbarut/logiops-rs) | GPL-2.0 | tiny, 2026-01 | ? | reject (GPL) |
| familiar (Gingerbreadfork/familiar, https://github.com/Gingerbreadfork/familiar) | MIT | young, 0★, active 2026-11; daemon + D-Bus + Tauri GUI; own `familiar-hidpp`, `familiar-hidraw` crates | **yes**, MX4-specific; Bolt-tested, **BT untested** | **adapt**: its `docs/protocol/mx-master-4.md` is the best single fact sheet |
| WhiteKr/mx4 (https://github.com/WhiteKr/mx4) | MIT | tiny, active 2026-11 | haptics only (BT 0xFF or Bolt), reconnect after sleep | reference only |
| `mxswitch`, `rigbat` | – | single-purpose | – | ignore |

Non-Rust MX4 projects, for reference only:
- UsiDiamond/mx-master-4-linux-desktop (MIT, Python): Actions Ring radial menu and haptics on **KDE Plasma 6 Wayland**, with a KWin script for focus haptics. Closest prior art to Detent's radial plugin.
- Physics-of-Data/mx4-haptic-linux: no license, so don't read the code.
- vivajarvis777-dev/logitech-mx-master-4-linux: MIT, Python.
- logiops PR #524 (GPL; facts only): D-Bus `PlayEffect(byte)`, config `haptic_feedback {enabled, strength, battery_saving}`, clamps effects to ≤14 and strength to 1..100, default 60.

---

## Open items: verify on the owner's device (BT, /dev/hidraw9)

1. Run Root.getFeature for 0x19B0, 0x19C0 and 0x1B04 and confirm indexes 0x0B, 0x0C and 0x0D over BT. Record the firmware version from 0x0003; the Windows-haptics firmware may differ from RBM 27.00.B0015.
2. Read 0x19B0 fn0: which waveform bits are set (0x7FFF? 0x0800_7FFF?) and what bytes 0-3 mean. Read fn1 for flags, intensity and the levels byte (to settle Solaar's vs OpenLogi's reading). Check whether fn3 exists.
3. Play one waveform (fn4) at the current intensity. Confirm a BT long report `11 FF 0B 4x <id> 00…` works. Check whether short reports (0x10) work over BT at all, using the report descriptor.
4. Read 0x1B04 getCidInfo for all 9 controls. Confirm task 0x0109 for 0x01A0 and that no control has the persistently-divertable flag.
5. Divert 0x01A0 with raw XY:
   - Confirm press and release events, and the junk first rawXY event.
   - Check whether the kernel (7.3) still emits evdev BTN_BACK or any other button for a diverted CID.
   - Confirm undiverted 0x01A0 = BTN_BACK over BT.
6. Sleep and wake over BT:
   - Confirm diversion, SmartShift mode and haptic ENABLED are lost.
   - Capture the 0x1D4B events (online, then `data[1]==1` reconfigure) and their timing.
7. Kernel interaction:
   - Confirm `hid_logitech_hidpp` binds 0005:046D:B042 and sets 0x2121 hi-res mode.
   - Confirm its SW ID is 0x01 on hidraw traffic.
   - Pick a different SW ID for Detent and check that we don't steal kernel replies.
8. Read 0x19C0 fn1 for the owner's unit's min, max and default force. Writing it is a persistent change, so ask the owner first.
9. ~~Report descriptor~~ VERIFIED 2026-11-25: yes, a usage page 0x0E haptics collection exists; HID++ over BT is on usage page **0xFF43** usage 0x0202, report 0x11 only (no 0x10). docs/specs/transport.md.
10. Ask the owner (not standard): which waveform IDs to use for radial hover and activate. The recommendation is Options+'s 4 (hover) and 1 (activate), falling back to whatever the mask allows.
