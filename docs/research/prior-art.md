---
title: Prior art (Phase R)
status: current
version: 1.0.0
last_updated: 2026-11-24
last_verified: 2026-11-23
source_refs: ["docs/research/"]
owner: "@rfluid"
tags: [research, prior-art]
---

# Prior art (Phase R)

What existing tools already figured out, so Detent reuses it. Each finding has a verdict:
**adopt** (use as is), **adapt** (use with changes) or **reject**. Details, byte layouts and
line-level citations live in four appendices:

| Appendix | Covers |
|---|---|
| [hidpp-layouts.md](hidpp-layouts.md) | HID++ 2.0 framing, swids, errors, every feature layout Detent uses, kernel coexistence |
| [mx-master-4.md](mx-master-4.md) | MX Master 4 feature table, CIDs, haptics (0x19B0), force button (0x19C0), sleep behaviour, Rust crates |
| [input.md](input.md) | gestures, resync triggers, uinput, evdev grab pitfalls, tap/hold, udev rules, systemd unit, key names |
| [kde-gpui.md](kde-gpui.md) | kglobalaccel names, volume/brightness OSD, KWin script context, GPUI layer-shell |

**License hygiene.** Solaar (GPL-2), logiops (GPL-3), the kernel (GPL-2), input-remapper (GPL-3)
and kanata (LGPL-3) were read for *facts* only; no code was copied. MIT/0BSD/Apache sources
(libratbag, keyd, xremap, familiar, OpenLogi's `openlogi-hidpp`, Zed/GPUI) may be reused with
attribution; so far none is.

Sources studied: Solaar master @e7304c4, logiops @e157995, Linux `drivers/hid/hid-logitech-hidpp.c`
(master), libratbag master, keyd, kanata, input-remapper, evremap, the `evdev` crate 0.13.2, xremap,
kdotool, KWin/plasma-pa/powerdevil sources, Zed PR #35610, gpui-pre 0.3.14, familiar, OpenLogi,
Solaar issues #2964 #3329, logiops issues #520 #564.

## 1. Transport, discovery, coexistence with the kernel

| Finding | Source | Verdict |
|---|---|---|
| Always send **long** (0x11) requests; some Bluetooth devices reject short ones | Solaar `base.py`, kernel `hidpp_send_message_sync` ([appendix §0](hidpp-layouts-protocol.md#0-frame-format-and-conventions-applies-to-every-feature)) | **adopt** (`DirectRequester`) |
| Replies from direct devices may carry device index 0x00 instead of 0xFF | Solaar `base.py` | **adopt** |
| Software IDs in use: kernel 1, logiops 2/3, OpenRGB 7, libratbag 8, LGSTrayEx A, Solaar B, G HUB D, firmware F → **Detent uses 0xE** (was 0xD, which collides with G HUB) | [appendix §0](hidpp-layouts-protocol.md#software-ids-in-use-pick-one-that-doesnt-collide) | **adopt** |
| Kernel: 5 s timeout, retries only on BUSY (0x08). Solaar/logiops: ~1–2 s. MX4 init failed with 500 ms in logiops #564 | appendices | **adapt**: 1 s per attempt, 2 retries on timeout, retry on BUSY with backoff |
| Kernel binds `B042` ("MX Master 4 mouse over Bluetooth", no quirks) and runs HID++ with swid 1 | kernel device table | **adopt**: classify swid≠ours as `Foreign` |
| **Never send a request on the 0x1D4B feature index**: the kernel treats any report there as "device connected" and reruns its setup | kernel `hidpp_connect_event` path ([§8](hidpp-layouts-device.md#8-wirelessdevicestatus-0x1d4b)) | **adopt** (hard rule) |
| Kernel re-sets HiResWheel mode (hi-res on, not diverted, not inverted) on every connect, and tracks every setWheelMode reply | kernel ([§14](hidpp-layouts-protocol.md#14-kernel-coexistence-hid-logitech-hidpp-on-b042)) | **adapt**: Detent never diverts the wheel; layers read wheel events from evdev during a grab instead. `wheel.invert` is re-applied after the kernel's reconnect write |

## 2. Feature layouts Detent uses

All verified read-only on the owner's device (see [specs/devices/mx-master-4.md](../specs/devices/mx-master-4.md)).
Summary table in [specs/hidpp.md](../specs/hidpp.md); byte layouts in [hidpp-layouts.md](hidpp-layouts.md).

| Feature | Verdict | Notes |
|---|---|---|
| Root 0x0000, FeatureSet 0x0001 | adopt | getFeature is big-endian (logiops has a dormant LE bug: **reject** that) |
| DeviceFwInfo 0x0003, DeviceName 0x0005 | adopt | |
| UnifiedBattery 0x1004 | adapt | charging byte uses the **kernel's** table (0 discharging, 1 charging, 2 slow, 3 full, 4 error); Solaar's older table rejected |
| ChangeHost 0x1814 | adopt (read-only) | switching hosts disconnects the mouse: never written |
| ReprogControlsV4 0x1B04 | adopt | "valid" bit per setting; remap 0 = keep; button event is the *set* of ≤ 4 held CIDs (diff consecutive sets); **temporary** divert only (persistent divert would leave buttons dead without Detent) |
| WirelessDeviceStatus 0x1D4B | adopt | event 0 `[status, request, reason]`; `request == 1` → device lost diversion, re-apply (logiops waits ~100 ms) |
| SmartShift Enhanced 0x2111 | adapt | fn 0/1/2; 0 = unchanged; MX4 quirk: torque only applies when the mode byte is written in the same request |
| HiResWheel 0x2121 | adopt (read + invert only) | see §1 |
| Thumbwheel 0x2150 | adopt | not diverted by default: native REL_HWHEEL is read from evdev during grabs |
| AdjustableDPI 0x2201 | adopt | `0xE000|step` range marker (Solaar's reading) |
| Haptics 0x19B0 | adopt (from 3 agreeing reverse-engineered sources) | see §3 |

## 3. MX Master 4 specifics

| Finding | Source | Verdict |
|---|---|---|
| 46 features; indexes identical over BT and Bolt; HID++ 4.5 | logiops #520 dump, Solaar #2964, familiar; **matches our dump** | adopt |
| CIDs: 0x50 L, 0x51 R, 0x52 middle, 0x53 back, 0x56 forward, 0xC3 gesture, 0xC4 SmartShift button, **0x1A0 Haptic Sense Panel**, 0xD7 virtual gesture | familiar capture, logid log; **matches our dump** | adopt |
| Nothing is persistently divertable → diversion lost on sleep/reconnect; must re-apply | dump flags, Solaar #3329 | adopt |
| First raw-XY report after a press can be garbage (MX3S) | Solaar | adopt: gesture FSM drops the first raw-XY frame after the modifier press |
| Haptics 0x19B0: fn0 caps (bytes 4–7 = waveform bitmap), fn1 config `[flags, intensity, levels]`, fn2 set config, fn4 play `[waveform,0,0]`; play is non-persistent, safe every ≥ 12 ms | Solaar, OpenLogi, familiar, logiops PR | adopt; read-only verified on device |
| Options+ uses waveform 4 (subtle collision) for Actions Ring hover, 1 for select | OpenLogi notes | adopt for radial ticks |
| Force Sensing Button 0x19C0 writes are **persistent** | OpenLogi | **reject writing**: read-only in Detent |
| Some power transitions clear haptics "enabled"; play is then silently ignored | OpenLogi | adopt: haptics config is re-applied on resync |
| Rust crates: `openlogi-hidpp` (0BSD, async, heavy), familiar (MIT), `hidpp` (0BSD, experimental), `hidpp-transport` (GPL) | crates.io | **reject as dependency** (hot-path rules, async); use as reference |

## 4. Reconnect, sleep, host switch

| Finding | Verdict |
|---|---|
| Resync triggers: 0x1D4B event with `request=1` (MX4 sends it ~1 s after reconnect), hidraw node re-added (BT link down/up: node number changes), logind resume | **adopt** all three; plus a cheap periodic verify |
| Always re-apply on connect (diversion is volatile); retry with backoff capped at 10 s; never exit when the mouse is absent | **adopt** |
| Solaar sleeps 200 ms after the kernel's reconnect writes before its own | **adapt**: wait 300 ms after link-up before resync, so the kernel's setup finishes first |

## 5. Diversion, raw XY gestures, tap/hold

| Finding | Verdict |
|---|---|
| logiops gestures: dominant axis, modes on_release / on_threshold / on_interval / axis, default threshold 50 counts | **adapt**: threshold in **mm** (counts × 25.4 / DPI), so it doesn't depend on DPI. Owner chose **fire at ~6 mm** (threshold mode); 1.3× dominance ratio before committing a direction |
| Tap vs swipe vs chord: chord decided immediately when another control is pressed; swipe when threshold crossed (later chords ignored); tap on release if neither | **adopt** (kanata "permissive hold"-like, logiops-like) |
| Diversion makes the firmware stop sending the native event; unbound diverted controls must be re-emitted (e.g. middle → BTN_MIDDLE) | **adopt** |

## 6. evdev grab + uinput

| Finding | Verdict |
|---|---|
| `evdev` crate 0.13 (MIT/Apache) for uinput setup and grab | **adopt** |
| Declare every capability at creation (all keys, BTN_LEFT..BTN_TASK, REL_X/Y/WHEEL/HWHEEL + `_HI_RES`); never recreate to add keys | **adopt** |
| Own `phys = "detent/..."` and vendor/product so Detent ignores its own device | **adopt** |
| Stuck-button pitfall: grabbing while a button is down splits press/release across devices | **adopt**: grab only when no button is held (EVIOCGKEY); stay grabbed until every swallowed button is released; drain queued events after grab; release all on exit; handle `SYN_DROPPED` |
| Wheel events from uinput: send legacy REL_WHEEL and REL_WHEEL_HI_RES together (120 per detent) | **adopt** |

## 7. KDE integration

All names verified on the owner's Plasma 6.8.3 with `busctl --user` (no `qdbus` on Fedora; `qdbus-qt6` is the binary name).

| Finding | Verdict |
|---|---|
| `invokeShortcut(name)` on `org.kde.kglobalaccel /component/<c>` (`org.kde.kglobalaccel.Component`) | **adopt** |
| `kwin`: `Overview`, `Grid View`, `Expose`, `ExposeAll`, `Switch to Next/Previous Desktop`, `Switch One Desktop to the Left/Right`, `Window Maximize/Minimize/Close`, `Window Quick Tile Left/Right`, `view_zoom_in/out`, `view_actual_size` | **adopt** |
| Volume: component `kmix` (plasma-pa kept the name): `increase_volume`, `decrease_volume`, `mute` → shows OSD, respects step setting; `wpctl` rejected (no OSD) | **adopt** |
| Brightness: `org_kde_powerdevil` `Increase/Decrease Screen Brightness` | **adopt** |
| Active app: load a KWin script via `org.kde.KWin /Scripting loadScript(path, name)` → `/Scripting/Script<id> run()`; script listens to `workspace.windowActivated` and `callDBus`es our service (xremap's approach, MIT) | **adopt** |

## 8. GPUI layer-shell

| Finding | Verdict |
|---|---|
| Upstream Zed merged layer-shell in PR #35610 (commit `b92664c5`, 2025-10-29): `WindowKind::LayerShell(LayerShellOptions{..})` | **adopt** |
| crates.io `gpui` 0.2.2 predates it; `gpui-pre` 0.3.x (Apache-2.0 snapshot of Zed) includes it | **adapt**: re-vendor a layer-shell-capable GPUI instead of writing our own patch (ADR 0005 updated in Phase 5) |
| KWin focuses keyboard-interactive overlay surfaces immediately → use `KeyboardInteractivity::None`; selection driven by the daemon's leased motion, not pointer focus | **adopt** |
| Fallback renderer: SCTK layer shell + tiny-skia (~500 lines) | keep as fallback |

## 9. Decisions this research settles

Recorded in the specs named:

1. swid **0xE**, long reports, 1 s timeout × 3 attempts, BUSY retry → `docs/specs/hidpp.md`.
2. Never write 0x1D4B, 0x1814 (host switch), 0x19C0, persistent divert, or unknown features → `docs/specs/hidpp.md`.
3. Wheel and thumbwheel are *not* diverted; layer bindings on them use evdev events during the grab → `docs/architecture/bindings.md`.
4. Resync triggers and delays → `docs/architecture/device-state.md`.
5. Gesture threshold in mm, owner choice 6 mm threshold mode → `docs/specs/config.md`.
6. udev rule (uaccess, file number ≤ 72) and systemd unit bound to `graphical-session.target` → `packaging/`.
7. KDE shortcut names → `docs/plugins/kde.md`, `plugins/kde/src/actions.rs`.
8. GPUI: re-vendor upstream layer-shell → `docs/plugins/radial.md`, ADR 0005 (Phase 5).
9. Radial haptic ticks: waveform 4 on hover, 1 on select → `docs/plugins/radial.md`.
