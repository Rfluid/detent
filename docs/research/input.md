---
title: "Input side: gestures, uinput, evdev grab, udev, systemd (research appendix)"
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [research, prior-art]
---

# Detent prior-art research: input side (gestures, diversion resync, uinput, evdev grab, tap-hold, udev, systemd, key names)

Date: 2026-11-23. All sources were cloned shallow into `/tmp/detent-research/src/` and read directly. The links below are pinned to the commit I read, so line numbers stay stable.

| Repo | License | Commit read |
|---|---|---|
| PixlOne/logiops | GPL-3.0 | `e15799553f97c1b8bab5d9b22b58453513b56217` |
| pwr-Solaar/Solaar | GPL-2.0 | `e7304c4c451cc9bb4f206a914844525e67856a28` |
| rvaiya/keyd | MIT | `f564288ac2b19d2305a5b39023c474805ff8fce5` |
| jtroo/kanata | LGPL-3.0 | `f0e668088a9f9dca8b73977ba3f6fd5d3c9999dd` |
| emberian/evdev (crate `evdev` 0.13.2) | MIT OR Apache-2.0 | `f90916dbecaf46efd54df27ac19d839d2c499dff` |
| sezanzeb/input-remapper | GPL-3.0 | `e38b3a3113a66fd792b81dd72bcb93d937c0994d` |
| wez/evremap | MIT | `cc618e8b973f5c6f66682d1477b3b868a768c545` |

We took facts and techniques only. No GPL code was copied. Each finding is marked **ADOPT**, **ADAPT** or **REJECT**, with a one-line reason.

Abbreviations: `LO` = `https://github.com/PixlOne/logiops/blob/e15799553f97c1b8bab5d9b22b58453513b56217`, `SO` = `https://github.com/pwr-Solaar/Solaar/blob/e7304c4c451cc9bb4f206a914844525e67856a28`, `KD` = `https://github.com/rvaiya/keyd/blob/f564288ac2b19d2305a5b39023c474805ff8fce5`, `KA` = `https://github.com/jtroo/kanata/blob/f0e668088a9f9dca8b73977ba3f6fd5d3c9999dd`, `EV` = `https://github.com/emberian/evdev/blob/f90916dbecaf46efd54df27ac19d839d2c499dff`, `IR` = `https://github.com/sezanzeb/input-remapper/blob/e38b3a3113a66fd792b81dd72bcb93d937c0994d`.

---

## 0. Findings on the owner's machine (read-only checks, 2026-11-23)

These are not prior art, but they change decisions. Record them in `.agent/memory/facts/` once confirmed.

- **Other tools are already driving the mouse.** `solaar --window=hide` (pid 5075), `juhradiald` + `juhradial-overlay.py` (JuhRadial MX) and `ydotoold` are all running. Solaar re-applies its own `DivertKeys` settings on every reconnect and on every 0x1D4B reconfiguration event (see §1.2). JuhRadial almost certainly diverts the gesture button too. **Two HID++ "owners" will fight over CID reporting.** Detent must detect this (`pgrep solaar|juhradiald|logid`) and warn in `detentctl doctor`. The owner should disable the conflicting tools, or at least set Solaar's divert settings for the MX Master 4 to "Regular". → ASK OWNER (it changes their setup).
- **hidraw number is not stable.** The MX Master 4 is now `/dev/hidraw10` (`HID_ID=0005:0000046D:0000B042`), not `/dev/hidraw9` as AGENTS.md says. Always discover by `HID_ID` / sysfs (udev enumerate) and never hardcode it. AGENTS.md "Machine facts" should say "e.g. /dev/hidraw9; number varies".
- The mouse has a **single evdev node**: `/dev/input/event9` "Logitech MX Master 4". Its properties are `ID_INPUT_MOUSE=1` and `ID_BUS=bluetooth`, its key capability is `7f0000 0 0 0 0` (BTN_LEFT..BTN_TASK only, no keyboard keys), and its phys is the BT MAC. Grabbing it therefore only affects pointer buttons, motion and wheel.
- Today `event9` and `/dev/uinput` already carry a user ACL. The ACL comes from **third-party rules**: `/etc/udev/rules.d/70-juhradialmx-uaccess.rules`, Solaar's `/usr/lib/udev/rules.d/42-logitech-unify-permissions.rules` (package `solaar-udev-1.1.20-1.fc44`), plus kdeconnect, ydotool and steam rules for uinput. **Detent must ship its own rules** and must not rely on these.
- The JuhRadial rules file contains a useful comment: a `TAG+="uaccess"` set in a `99-*` file runs **after** `73-seat-late.rules` and is therefore ineffective. This confirms the ordering pitfall described in §5.

---

## 1. logiops (GPL-3)

### 1.1 Gestures with raw XY

- **Diversion flags.** A Gestures action sets `TemporaryDiverted | RawXYDiverted` on the CID, both on HID++ 0x1B04 setCidReporting (`LO/src/logid/actions/GestureAction.cpp#L220-L223`). Button events and raw XY then arrive over HID++. The cursor does not move while the button is held because RawXY diversion stops the pointer reports. **ADOPT**: this is the mechanism Detent needs for swipes.
- **Default threshold = 50 raw units** (`LO/src/logid/Configuration.h#L33`, `gesture_threshold = 50`). The unit is raw device counts at the current DPI, with no DPI normalization. That is 1.27 mm at 1000 DPI but only 0.32 mm at 4000 DPI, and the MX4 example config in issue #520 uses `dpi: 4000`. **REJECT the raw-count unit**: Detent's thresholds must be DPI-normalized (see §4 and the recommendation).
- **Direction resolution.** 4-way plus "None", using the dominant axis on the accumulated (x, y). Ties go to Right or Down for positive values and Left or Up for negative ones (`GestureAction.cpp#L64-L73`). **ADOPT** the dominant-axis rule. Detent should add an ambiguity ratio, e.g. require |major| ≥ 1.3×|minor| before committing, otherwise wait.
- **Accumulation.** `move()` keeps separate per-direction axes and handles crossing the origin by un-accumulating, e.g. going Right then back Left (`GestureAction.cpp#L151-L218`). **ADAPT**: Detent only needs a running (Σx, Σy). Per-direction bookkeeping is only needed for interval modes.
- **Release resolution** (`GestureAction.cpp#L117-L149`):
  1. The primary direction comes from the total vector.
  2. If the primary gesture did not meet its threshold, any other direction that did is used instead.
  3. The "None" gesture (a tap) fires only when **no** direction met its threshold. The threshold for None is forced to 0 (`#L95-L100`).

  **ADOPT**: "tap = released and no direction crossed the threshold" is exactly Detent's rule.
- **Modes** (config names in `LO/src/logid/config/schema.h#L150-L204`):
  - `OnRelease` (`ReleaseGesture.cpp#L49-L56`): fires once on release if the primary direction met the threshold.
  - `OnThreshold` (`ThresholdGesture.cpp#L57-L67`): fires once, the moment the threshold is crossed while still held. Its D-Bus `interface_name` is wrongly `"OnRelease"` (`ThresholdGesture.cpp#L24`). This is a copy-paste bug in logiops.
  - `OnInterval` / `OnFewPixels` (`IntervalGesture.cpp#L62-L81`): after the threshold, fires press+release each time `(axis - threshold) / interval` increments, for things like volume or zoom repeat.
  - `Axis` (`AxisGesture.cpp`): maps movement to a REL axis (scroll) with `axis_multiplier`.
  - `NoPress`: does nothing.

  **ADOPT** all four semantics as Detent gesture modes: `on_release`, `on_threshold`, `on_interval{interval}`, `axis{axis, multiplier}`. Keep names close so logiops users can migrate.
- **Pitfall, logiops #98:** on old MX Master PIDs (4041), "Cannot divert raw XY movements for CID 0xc3". The gesture-support bit in 0x1B04 getCidInfo must be checked before diverting rawXY. **ADOPT**: read the `raw XY` capability flag and fail gracefully.
- **MX Master 4 CIDs** (community-reported in logiops #520, *to verify*): `0x0c3` gesture button, `0x0c4` top button (SmartShift toggle), `0x052` middle, `0x053` back, `0x056` forward, `0x1a0` thumb/haptic "Actions ring" button. The same thread lists the MX4 as HID++ 4.5 with feature 0x1D4B at index 0x04 (https://github.com/PixlOne/logiops/issues/520). **ADAPT**: treat these as "to verify" and confirm with `detentctl` getCidInfo on the real device.

### 1.2 Re-applying diversion after reconnect / sleep

- **0x1D4B Wireless Device Status.** The event is function 0 (`StatusBroadcast`), with params `[0]=reconnection`, `[1]=reconfNeeded`, `[2]=powerSwitch` (`LO/src/logid/backend/hidpp20/features/WirelessDeviceStatus.cpp#L28-L35`). When `reconfNeeded` is set, logiops schedules `Device::wakeup()` **after 100 ms** (`LO/src/logid/features/DeviceStatus.cpp#L44-L64`). `wakeup()` calls `reconfigure()`, which calls `reset()` (0x0020 Reset/ConfigChange) and then `configure()` on every feature, re-sending diversion, DPI and the rest (`LO/src/logid/Device.cpp#L185-L203`). This listener only exists for **direct (BT/USB) devices**. On receivers, the receiver's link-established notification drives `wakeup()`/`sleep()` instead (`LO/src/logid/Receiver.cpp#L90-L98`). **ADOPT**: on 0x1D4B with `reconfNeeded=1`, wait about 100 ms, then re-push all state (diversion and settings), idempotently.
- **Device removal / re-add.** On Bluetooth, a reconnect often destroys and recreates the hidraw node. logiops handles this with udev add/remove and retries "Failed to add device … backing off 500 ms, 1000, 2000, 4000, 8000" (issue #468 log). **ADOPT**: udev monitor plus exponential backoff, but **do not give up after 5 tries**. Keep a capped retry (max about 10 s) forever while the node exists.
- **What still breaks in logiops (issue tracker):**
  - #129, #148, #156: "stops working after idle/suspend; service restart fixes it". Users add `/lib/systemd/system-sleep/logid.sh` to restart the service, or a udev `RUN+=` on `hci0:*` add. The root causes are missed reconnects and a device init that fails on the first try and never retries. An MX Master 4 user still reports this on Fedora 44 with logiops 0.3.5 in Nov 2025 (comment on #129, "fell asleep … woke up" then config lost).
  - #564: **MX Master 4 on Bolt fails to init at boot with the default 500 ms `io_timeout`**; 3000 ms fixes it.
  - #468: BlueZ 5.77 broke re-adding after reconnect; fixed in 5.78.
  - #302: a "fell asleep / woke up" loop, with stutter.

  **ADOPT** three defenses:
  1. Per-request HID++ timeout of about 2 s during init (500 ms is too short for the MX4), with retries.
  2. Resync triggers: (i) 0x1D4B reconfNeeded, (ii) hidraw add, (iii) logind `PrepareForSleep(false)` (resume), and (iv) a cheap **verification on the first diverted-button event after more than N minutes idle**. On any HID++ error, re-run init.
  3. Never require a service restart. Expose `detentctl resync`.
- **Pitfall, #376:** logid aborted when it could not reach D-Bus ("Cannot autolaunch D-Bus without X11 $DISPLAY"). **ADOPT**: the core must not depend on the session bus. Plugins use the bus; the device thread does not.

### 1.3 uinput virtual device

- **Setup** (`LO/src/logid/InputDevice.cpp#L43-L72`):
  - Uses libevdev. Name is `"LogiOps Virtual Input"` (`LO/src/logid/logid.cpp#L31`).
  - Enables EV_KEY codes 0–127 at start, plus EV_REL with no codes.
  - Each new key or axis used later triggers **destroy + recreate of the uinput device** (`_enableEvent`, `InputDevice.cpp#L162-L177`).
  - Each event is written with its own SYN_REPORT (`#L179-L183`).

  **REJECT the recreate-on-new-key approach**: it makes the device flap (the compositor re-detects it, and KDE may re-apply per-device settings) and it races with in-flight events. Detent declares every capability up front.
- **Pitfall, #132:** the logiops virtual device was "detected as an Xbox 360 controller" by Steam. It advertised a large, odd capability mix under a generic ID. PixlOne's fix was to use the Logitech vendor ID. **ADAPT**: keep capabilities to keyboard keys plus pointer buttons and wheels, use a distinctive non-Logitech ID, and never set ABS or BTN_GAMEPAD bits. Split keyboard and pointer devices (see §3).
- **systemd** (`LO/src/logid/logid.service.in`): system service, `User=root`, `WantedBy=graphical.target`. **REJECT**: Detent runs as a user without root.

---

## 2. Solaar (GPL-2) diversion rules

- **Synthesizing input.** Solaar uses python-evdev `UInput` named **"solaar-keyboard"**. Its capabilities are all `KEY_*` codes, mouse buttons BTN_LEFT..BTN_EXTRA, and `REL_WHEEL` + `REL_HWHEEL`; there are **no hi-res wheel axes** (`SO/lib/logitech_receiver/diversion.py#L226-L269`). Every event is written and then `syn()` is called (`#L319-L331`). On a write failure Solaar drops the device and recreates it lazily. xdotool is no longer used. **ADOPT** a single lazily recovered device for keys. **ADAPT**: add the hi-res axes.
- **Wayland** (`diversion.py#L60-L111`):
  - Rules cannot read the modifier state on Wayland: all modifiers are assumed up, so KeyPress presses them all.
  - Focused-process conditions only work on GNOME, with the Solaar GNOME extension over D-Bus `org.gnome.Shell` `/io/github/pwr_solaar/solaar` (`#L190-L206`).
  - KeyPress maps X keysyms to keycodes through GDK. This is fragile.

  **ADAPT**: Detent emits evdev keycodes directly, with no keysym layer, and gets focus/app context only through the KDE plugin.
- **Diverted-key processing** (`SO/lib/logitech_receiver/settings.py#L797-L856`, `RawXYProcessing`):
  - A 0x1B04 notification with address 0x00 carries up to 4 currently pressed CIDs. The diff against the previous set gives press and release.
  - Address 0x10 carries raw dx, dy.
  - While an initiating key is held, **any other diverted key press is recorded as a chord element** (`key_action`, `#L825-L827`).

  **ADOPT**: diff the 4-CID list to get edges (the same 0x1B04 event semantics Detent needs), and treat "another control pressed while the modifier is held" as a chord.
- **Mouse gestures** (`SO/lib/logitech_receiver/settings_templates.py#L900-L964`, `MouseGesturesXY`):
  - **DPI-normalized movement**: `dx/dpi*15`, about 6 units/cm (`#L939-L942`).
  - A **pause of 200 ms or more** closes a segment, so multi-stroke gestures are possible (`#L936-L937`).
  - On release, Solaar emits a list of segments and keys, matched by the `MouseGesture` condition (`SO/lib/logitech_receiver/diversion.py#L969-L1018`).
  - 8-way direction via `round(x/|v|), round(y/|v|)` (`diversion.py#L292-L316`): cardinals get about 60° wedges and diagonals about 30°.
  - **"hack to ignore strange first movement report from MX Master 3S"**: when the REPROG_CONTROLS_V4 version is ≥ 5, the first raw XY report after press is dropped (`#L933-L935`).

  **ADOPT** the DPI normalization and the first-report skip, behind a per-device quirk flag, *to verify on the MX4*. **REJECT** multi-segment gestures for v1, since they add latency and rarely help.
- **Re-applying settings** (`SO/lib/logitech_receiver/notifications.py#L321-L333`; `SO/lib/logitech_receiver/device.py#L594-L620`, `#L467-L503`):
  - On 0x1D4B with `data[1]==1` ("device is asking for software reconfiguration"), Solaar calls `changed(active=True)`. This applies all settings on the inactive→active transition, and `apply_settings_if_needed()` handles follow-up reconfiguration events.
  - Solaar uses the **0x0020 CONFIG_CHANGE cookie** to dedupe. After applying, it writes a fresh session cookie, and later skips re-applying if the live cookie matches. It explicitly does **not** trust the cookie across power cycles: the first activation always applies.

  **ADOPT**: always apply on (re)connect. Use the 0x0020 cookie only to skip redundant re-applies within a session.
- **Suspend/resume**: Solaar watches logind `org.freedesktop.login1.Manager.PrepareForSleep` on the system bus (`SO/lib/solaar/dbus.py#L51-L71`). On resume it pings all devices, or restarts all listeners with `--restart-on-wake-up` (`SO/lib/solaar/gtk.py#L196-L199`). **ADOPT**: resume triggers a resync (ping, then re-apply if needed).
- **hidraw discovery**: pyudev monitor with `filter_by(subsystem="hidraw")` (`SO/lib/hidapi/udev_impl.py#L246-L256`). **ADOPT** an equivalent (the `udev` crate, or a netlink monitor) for hot-plug.
- **Autostart**: an XDG autostart `.desktop` with `Exec=solaar --window=hide` (`SO/share/autostart/solaar.desktop`; the Fedora `solaar` package installs it at `/etc/xdg/autostart/solaar.desktop`). Solaar does not use systemd. **REJECT** for Detent: a systemd user unit gives restart, logging and ordering.

---

## 3. evdev grab + uinput patterns

### 3.1 Virtual device setup

- **keyd (MIT)** (`KD/src/vkbd/uinput.c#L34-L158`):
  - Creates **two** devices: "keyd virtual keyboard" (EV_KEY for named keycodes below 256, EV_REP, EV_LED) and **"keyd virtual pointer"** (EV_REL X/Y/WHEEL/HWHEEL/Z, EV_ABS X/Y, BTN_LEFT..BTN_TASK).
  - Opens with `O_WRONLY|O_NONBLOCK|O_CLOEXEC`.
  - Uses the legacy `write(uinput_user_dev)` instead of `UI_DEV_SETUP` "to support older kernels" (`#L93-L99`).
  - Uses a fixed fake vendor `0x0FAC`, with products `0x0ADE` and `0x1ADE` (`#L87-L89`, `#L141-L143`).
  - Mouse buttons go through the pointer device "to prevent X from identifying the virtual keyboard as a mouse" (`#L190-L196`).
  - Before a button event keyd sleeps 1 ms, so that key events already sent on the other device are processed first and do not arrive after the click (`#L198-L208`).

  **ADOPT** the split keyboard/pointer devices: libinput classifies a device by its capabilities, and mixed devices get odd treatment (DWT, tablet/joystick heuristics). **ADOPT** a distinctive fixed vendor/product so we can filter our own devices. **ADAPT** the cross-device ordering problem: for a chord like "Ctrl+click", write the modifier on the keyboard device, then the click on the pointer device. With non-blocking writes the kernel delivers per device, and the compositor reads them in poll order. Prefer emitting such combos as **one frame on one device**, i.e. a pointer device that also declares the modifier keys. Avoid sleeping on the hot path.
- **kanata (LGPL)** (`KA/src/oskbd/linux.rs#L419-L484`):
  - One device declaring keys 0..560: "For some reason 0..0x300 doesn't work, the closest that I've got to work is 560" (`#L428-L429`).
  - Rel axes WHEEL, HWHEEL, X, Y, Z, RX, RY, RZ, DIAL, MISC, **WHEEL_HI_RES, HWHEEL_HI_RES** (`#L430-L443`).
  - Name "kanata". `InputId(bus_type,1,1,1)` with a configurable bus, because "libinput's disable-while-typing doesn't work when bus_type is BUS_USB, but works with BUS_I8042" (`#L447-L449`).
  - Hi-res scroll emits `REL_WHEEL_HI_RES` and accumulates a remainder so that the legacy `REL_WHEEL` fires every 120 units (`#L605-L650`).
  - Uses evdev `VirtualDevice::emit` per frame, buffering raw events until SYN (`#L494-L512`).

  **ADOPT**: hi-res and legacy wheel events in the same frame, with remainder accumulation. **REJECT** the 0..560 key range: declare only the keys Detent can emit.
- **Kernel contract** (https://docs.kernel.org/input/event-codes.html): `REL_WHEEL_HI_RES` "accumulated value 120 represents movement by one detent" and is emitted "in addition to REL_WHEEL". After `SYN_DROPPED`, a client must ignore events up to the next `SYN_REPORT` and re-query state with `EVIOCG*`. **ADOPT** both: Detent must also handle SYN_DROPPED when it reads the grabbed mouse.
- **Rust `evdev` crate 0.13.2** (MIT OR Apache-2.0; crates.io max stable 0.13.2; git HEAD still 0.13.2 as of 2026-05-28):
  - `VirtualDevice::builder()?` opens `/dev/uinput` read-write. Then chain `.name(&str)`, `.input_id(InputId::new(BusType, vendor, product, version))`, `.with_phys(&CStr)`, `.with_keys(&AttributeSet<KeyCode>)?` (issues `UI_SET_EVBIT EV_KEY` plus `UI_SET_KEYBIT` per key), `.with_relative_axes(&AttributeSet<RelativeAxisCode>)?` (`UI_SET_RELBIT` per axis), `.with_properties`, `.with_msc`. Finish with `.build()?`, which calls `UI_DEV_SETUP` (`uinput_setup`) and then `UI_DEV_CREATE` (`EV/src/uinput.rs#L34-L239`, `#L264-L272`).
  - The default ID is USB 0x1234:0x5678 (`#L242-L247`). Always set your own.
  - **`emit(&[InputEvent])` writes the batch and then appends SYN_REPORT as a second `write()` syscall** (`#L318-L322`; `write_events` in `EV/src/lib.rs#L547-L550` is a `write_all` of the raw bytes). So each frame costs two syscalls, but there is **no heap allocation**.
  - `enumerate_dev_nodes_blocking()` / `get_syspath()` let you find the created `/dev/input/eventN`, which is useful for excluding it.
  - `KeyCode`, `RelativeAxisCode` and the rest implement `FromStr` from kernel names such as `"KEY_LEFTCTRL"` (`EV/src/attribute_set.rs#L274-L277`).

  **ADOPT** `evdev` 0.13 for both reading or grabbing (`Device::grab()`/`ungrab()`, `get_key_state()` = `EVIOCGKEY`) and uinput. **ADAPT**: if the double syscall shows up in benchmarks, write `[events…, SYN_REPORT]` ourselves with one `write(2)` on `as_raw_fd()`. An extra empty SYN is not forwarded by the input core, but it still costs a syscall.
- Alternatives:
  - `input-linux` 0.7.1 (MIT): lower-level typed ioctls. A reasonable fallback, but the API is less ergonomic.
  - `uinput` 0.1.3 (WTFPL, last release 2018): **REJECT**, unmaintained.
  - `evdev-rs` 0.6.3 (wraps C libevdev, MIT): **REJECT**, it adds a C dependency for no gain.

### 3.2 Avoiding feedback loops (ignoring our own devices)

- keyd: `dev->is_virtual = info.vendor == 0x0FAC` (`KD/src/device.c#L218`), and `manage_device` returns early for virtual devices (`KD/src/daemon.c#L265`).
- kanata: `if device.name() == Some("kanata") { return false; }` (`KA/src/oskbd/linux.rs#L306`).
- input-remapper: names devices `"input-remapper …"` and sets **phys to `input-remapper/<orig phys>`**. It says "the original phys path must not be reused" (`IR/inputremapper/injection/injector.py#L52-L107`). Its udev autoload rule skips `ATTRS{phys}=="input-remapper*"` (`IR/data/99-input-remapper.rules`).

**ADOPT**: set a fixed vendor:product (e.g. `0x0DE7:0x0001` for the keyboard and `0x0DE7:0x0002` for the pointer; pick IDs and record them in the spec) **and** `phys = "detent/…"`. Filter on both. Detent only opens the one Logitech evdev node anyway (match `ID_BUS`/vendor `046d`/product), so a loop is unlikely, but the filter costs nothing.

### 3.3 Grabbing while keys or buttons are held: the stuck-key pitfall

- **The general problem.** `EVIOCGRAB` redirects all subsequent events to the grabbing fd only. If a button was pressed **before** the grab and released **during** it, the compositor (libinput) never sees the release, and the button stays logically down: a stuck drag or stuck click. The reverse case (pressed during the grab, released after ungrab) gives the compositor an unmatched release, which libinput ignores, but the grabber must then **keep swallowing** that release.
- **kanata** (`KA/src/oskbd/linux.rs#L123-L131`, `#L838-L880`):
  - `wait_for_all_keys_unpressed()` polls `EVIOCGKEY` (`ioctl 'E', 0x18`) every 4 ms, up to 1250 tries (about 5 s), and only then grabs.
  - It also does grab → ungrab → grab, which "magically fixes" a Lenovo trackpad.
- **keyd** (`KD/src/device.c#L370-L390`):
  - Same EVIOCGKEY wait loop.
  - If keys were pending, `usleep(100)` "allow the key up events to propagate before grabbing".
  - After the grab it **drains** all pending events (non-blocking read until EAGAIN).
  - On reload it releases every key the virtual keyboard holds (`clear_vkbd`, `KD/src/daemon.c#L38-L47`, called from `reload`, `#L305`).
- **input-remapper**: retries `grab()` 10 times with a sleep, because "it might take a little time until the device is free if it was previously grabbed" (`IR/inputremapper/injection/injector.py#L317-L339`). It ungrabs at shutdown.
- **evremap**: grabs immediately after creating the output device (`https://github.com/wez/evremap/blob/cc618e8b973f5c6f66682d1477b3b868a768c545/src/remapper.rs#L105`), with no wait. This is the known "Enter repeats forever if you start it from a terminal" class of bug. **REJECT**.

**Detent rule set (ADAPT).** The grab is short-lived and taken only while the chord modifier is held:

1. Keep the mouse evdev fd **open non-grabbed** at all times, read in the device thread (or via epoll). We track button state from the stream. It is cheaper than EVIOCGKEY, and EVIOCGKEY serves as a re-sync after SYN_DROPPED.
2. When the modifier goes down (HID++ diverted CID press), grab **only if no pointer button is currently down**. If one is down (e.g. the user is dragging), **defer** the grab until those buttons are released (the event arrives on our non-grabbed fd), then grab. Never block, never sleep.
3. After the grab, discard the events already queued (drain until EAGAIN). Their information was already seen non-grabbed.
4. While grabbed, every event we do **not** consume (motion, wheel, other buttons) is forwarded through the Detent virtual pointer **in the same frame structure** (split at SYN_REPORT). With RawXY diversion active on the modifier, the mouse sends no REL_X/Y over evdev during a hold (*to verify on device*), so forwarded traffic is mostly wheel.
5. When the modifier is released: if any button we swallowed is still down, **stay grabbed until it is released** (swallow the release too), then ungrab. Otherwise ungrab immediately.
6. **Watchdog**: on daemon exit, panic, device removal, or a lease timeout, ungrab, then emit a release for every key or button our virtual devices hold (`clear_vkbd` pattern). The uinput fd closing destroys the device, which also releases its keys in the compositor, but explicit releases are cleaner.

If the forwarded pointer motion "feels different", that is the **input-remapper ID_BUS pitfall**. Virtual devices get no `ID_BUS`, so `70-mouse.rules` skips the hwdb `MOUSE_DPI` lookup and the pointer feel changes. input-remapper's fix is `ATTRS{phys}=="input-remapper/*", ENV{ID_BUS}=="", ENV{ID_BUS}="usb"` (`IR/data/69-input-remapper-forwarded.rules`). KDE also stores pointer acceleration per device (by name, vendor and product), so a virtual pointer gets the defaults. Mitigations:
- (a) Keep the grab window tiny, which this design already does.
- (b) If motion must be forwarded, set the virtual pointer's `input_id` bus to BLUETOOTH and its name to something stable, and document that KDE pointer settings apply to "Detent pointer" separately.
- (c) **ADOPT** an analogue of input-remapper's udev rule for `phys=="detent/*"`.

**Latency.** keyd and kanata do one `read()` and one or two `write()` calls per event, plus a 1 ms sleep only for cross-device ordering (keyd). For Detent's forwarding path: epoll wake → read batch → write batch + SYN. That is well under 100 µs and allocation-free if the frame buffer is a fixed array (e.g. `[InputEvent; 16]`). **ADOPT**: no sleeps, a fixed buffer, `O_NONBLOCK` on the uinput fd, and on `EAGAIN` drop and count the event (never block the input thread).

---

## 4. Tap / hold / chord resolution

- **QMK** (https://docs.qmk.fm/tap_hold):
  - `TAPPING_TERM` defaults to **200 ms**.
  - Default mode: hold only if held longer than the term.
  - `PERMISSIVE_HOLD`: "immediately selects the hold action when another key is tapped".
  - `HOLD_ON_OTHER_KEY_PRESS`: "immediately selects the hold action when another key is pressed while the dual-role key is held".
  - `RETRO_TAPPING`: "releasing the key without pressing another will send the original keycode even if it is outside the tapping term".
- **kanata** (`KA/docs/config.adoc#L2040-L2140`): `tap-hold-press` activates hold early when another key is pressed; `tap-hold-release` does so when another key is pressed **and released**; there are `-timeout` and `-keys` variants, `tap-hold-except-keys`, and `tap-hold-require-prior-idle`.
- **keyd** (`KD/docs/keyd.scdoc#L705-L752`): `overload(layer, action)` gives the layer while held and the action on a tap, **with no timeout by default**. `overload_tap_timeout` (`#L592-L594`) ignores the tap if the key was held longer than N ms. There are also `overloadt` and `overloadt2` (hold on an intervening tap) and `overloadi` (idle-based).
- **logiops and Solaar**: there are no timeouts at all. Tap means "released with no direction over threshold" (logiops `GestureAction::release`). A chord means "another diverted key pressed while held" (Solaar `key_action`).

**Best fit for Detent.** The modifier is pressed first. A tap is a release alone with no motion, a swipe is motion over the threshold, and a chord is another control pressed during the hold. This matches **keyd `overload` + QMK `HOLD_ON_OTHER_KEY_PRESS` + `RETRO_TAPPING` + logiops tap = "None" gesture**:

1. **Press.** State = `Pending`. Start accumulating DPI-normalized (Σx, Σy). Nothing is emitted. There is no hold timer needed for correctness, because a mouse button has no "typing roll" problem.
2. **Another control pressed** (a diverted CID, or an evdev BTN_* while grabbed). Resolve to **Chord(other)** immediately, i.e. hold-on-other-key-press. Swallow that control's press and release. Motion after a chord is ignored, which disables swipes for this hold.
3. **|Σ| crosses the commit threshold** while Pending. Resolve to **Swipe(direction)** via the dominant axis with a 1.3× ambiguity ratio. `on_threshold` gestures fire now, and `on_release` gestures fire at release. Once a swipe is committed, a later chord is either ignored or allowed. Recommend **ignored**, to avoid accidental fires.
4. **Release while still Pending.** Resolve to **Tap**, retro-tapping style, regardless of duration. **ADAPT** keyd's `overload_tap_timeout` as an *optional* `tap_timeout_ms` (default off; suggest 0 = disabled, or about 500 ms if the owner wants "long hold and release = nothing").
5. Optional `hold` action after `hold_ms` with no motion and no chord (e.g. open the radial menu). This is QMK's default mode. The owner should decide the default (it is UX feel).

**Thresholds (ADAPT, verify on device):**
- Normalize by DPI like Solaar (`counts / dpi * 25.4` = mm).
- **Jitter dead-zone about 1.5 mm**: below it the hold is still a candidate Tap.
- **Swipe commit about 6 mm.** logiops's 50 counts is 1.3 mm at 1000 DPI, which is too twitchy at 4000 DPI.
- **`on_interval` step about 6 mm.**
- Skip the first raw XY report after press if the MX4 exhibits the MX Master 3S quirk.
- These are starting values. Gesture feel is an owner decision per AGENTS.md: **ask, recommending 6 mm and 1.5 mm.**

---

## 5. hidraw (and evdev, uinput) access without root

- **Solaar's rule** (`SO/rules.d/42-logitech-unify-permissions.rules`; identical to Fedora `solaar-udev-1.1.20-1.fc44`, installed at `/usr/lib/udev/rules.d/42-logitech-unify-permissions.rules`):
  ```
  KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"
  ACTION == "remove", GOTO="solaar_end"
  SUBSYSTEM != "hidraw", GOTO="solaar_end"
  ATTRS{idVendor}=="046d", GOTO="solaar_apply"                 # USB receivers/devices
  ATTRS{idVendor}=="17ef", ATTRS{idProduct}=="6042", GOTO="solaar_apply"
  KERNELS == "0005:046D:*", GOTO="solaar_apply"                # Bluetooth
  GOTO="solaar_end"
  LABEL="solaar_apply"
  TAG+="uaccess"
  LABEL="solaar_end"
  ```
  So yes: **`TAG+="uaccess"` on hidraw for vendor 046d (matching `KERNELS=="0005:046D:*"` for BT, and `0003:046D:*` for USB) is the standard approach**. Fedora ships it in the `solaar-udev` subpackage. The `static_node=uinput` plus uaccess form for `/dev/uinput` is also the de-facto standard: kdeconnect `40-kdeconnect-uinput.rules` and the Steam rules ship the identical line on this machine.
- **Rule order.** uaccess tags must be set **before `73-seat-late.rules`** (which runs the `uaccess` builtin), so the file number must be ≤ 72. A `99-*` file does not work; the JuhRadial rules on this machine carry the same comment. **ADOPT** `70-detent.rules`.
- **evdev node.** systemd's `70-uaccess.rules` does **not** grant uaccess to mice or keyboards. Only joysticks, 3D mice and similar device classes get it. To read and grab the MX Master's `/dev/input/eventN`, Detent needs its own rule. Scope it narrowly: Logitech HID parent and `ID_INPUT_MOUSE`, never keyboards, to avoid creating a keylogger surface.
- Alternatives:
  - Kanata's documented setup (`KA/docs/setup-linux.md#L1-L80`) uses an `input` and `uinput` group plus `MODE="0660", GROUP="uinput"`. **REJECT**: it needs a re-login and group management, and grants access to *all* input devices.
  - Granting access to the `input` group as a whole. **REJECT** for the same reason: every keyboard becomes readable.
- **Write risk.** Solaar's comment warns that user write access to hidraw allows firmware updates. That is accepted industry practice, but Detent should only send known feature requests. This is already a non-negotiable.

**Recommended `packaging/udev/70-detent.rules`:**
```
# Detent: seat-user access to Logitech HID++ hidraw, the Logitech mouse evdev node, and uinput.
# Must sort before 73-seat-late.rules for uaccess to take effect.
KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess", OPTIONS+="static_node=uinput"

ACTION=="remove", GOTO="detent_end"
SUBSYSTEM=="hidraw", KERNELS=="0005:046D:*", TAG+="uaccess"
SUBSYSTEM=="hidraw", KERNELS=="0003:046D:*", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_INPUT_MOUSE}=="1", KERNELS=="0005:046D:*", TAG+="uaccess"
SUBSYSTEM=="input", KERNEL=="event*", ENV{ID_INPUT_MOUSE}=="1", KERNELS=="0003:046D:*", TAG+="uaccess"
# Give Detent's virtual pointer an ID_BUS so 70-mouse.rules/hwdb treat it like a real mouse (input-remapper technique).
SUBSYSTEM=="input", KERNEL=="event*", ATTRS{phys}=="detent/*", ENV{ID_BUS}=="", ENV{ID_BUS}="usb"
LABEL="detent_end"
```
Two parts need care:
- `ENV{ID_INPUT_MOUSE}` is set by `60-input-id.rules`, which runs before 70, so the match works.
- For USB receivers (Bolt/Unifying) the event node's parent HID is `0003:046D:C548…`, and the mouse's evdev node hangs off the receiver's child hidpp device. Verify the `KERNELS` match against the Bolt case before relying on it.

**Install:** `sudo install -m644 packaging/udev/70-detent.rules /etc/udev/rules.d/`, then `sudo modprobe uinput` (or `/etc/modules-load.d/uinput.conf` containing `uinput`), then `sudo udevadm control --reload && sudo udevadm trigger`. This needs sudo, so it is the owner's one-time step.

---

## 6. systemd user service

- **Prior art:**
  - logiops: system service, root, `WantedBy=graphical.target`.
  - keyd: system service, `WantedBy=multi-user.target`, root (`KD/keyd.service.in`).
  - input-remapper: system `Type=dbus` service plus a per-user XDG autostart `.desktop` that runs `input-remapper-control --command autoload` (`IR/data/input-remapper.service`, `IR/data/input-remapper-autoload.desktop`).
  - Solaar: XDG autostart only.
  - kanata's documented user unit: `Type=simple`, `Restart=on-failure`, `RestartSec=3`, `WantedBy=default.target`. It notes "The `--no-wait` flag is required for `Restart=on-failure` to work" (`KA/docs/setup-linux.md#L81-L109`).
- **systemd.special(7)** (https://www.freedesktop.org/software/systemd/man/latest/systemd.special.html): `graphical-session.target` "is active whenever any graphical session is running … used to stop user services which only apply to a graphical session when the session is terminated. Such services should have `PartOf=graphical-session.target`." Plasma 6 on Fedora uses systemd startup, so `plasma-workspace.target` pulls in `graphical-session.target`.
- **Why `graphical-session.target` and not `default.target` for Detent.** uaccess ACLs are granted only to the user of the *active seat session*. A user manager started by lingering, or before login, has no ACL on hidraw or uinput, so the daemon would fail with EACCES. The KDE plugin also needs `WAYLAND_DISPLAY` and the session bus. Tying to the graphical session solves both. On seat switch or lock, ACLs can be revoked mid-run (EACCES or EPERM on read), so treat that like a device removal: back off and retry. **ADOPT.**

**Recommended `packaging/systemd/detentd.service` (user unit):**
```
[Unit]
Description=Detent HID++ mouse daemon
Documentation=https://github.com/<owner>/detent
PartOf=graphical-session.target
After=graphical-session.target
Requisite=graphical-session.target

[Service]
Type=notify            # if we call sd_notify(READY=1) after the uinput device is created; else Type=exec
ExecStart=%h/.local/bin/detentd      # or /usr/bin/detentd when packaged
Restart=on-failure
RestartSec=1s
# Never exit on missing device: wait for udev instead (avoids restart loops / StartLimit hits).
Slice=session.slice    # essential session service per systemd DESKTOP_ENVIRONMENTS guidance
Nice=-5                # allowed unprivileged only via RLIMIT_NICE; otherwise drop. Prefer no CPUSchedulingPolicy (needs privileges).
NoNewPrivileges=yes
LockPersonality=yes
RestrictRealtime=no
MemoryDenyWriteExecute=yes

[Install]
WantedBy=graphical-session.target
```
- `Restart=on-failure` plus a short `RestartSec`, and the daemon must **never** exit just because the mouse is absent. It must wait on udev. That avoids hitting `StartLimitBurst` (default 5 starts in 10 s).
- Do not use the `--no-wait`-style interactive prompt (kanata pitfall).
- Do not ship XDG autostart; the systemd unit replaces it.
- On exit or `SIGTERM`, ungrab and release all keys (§3.3, step 6).
- Fedora KDE note: XDG autostart units are generated with `After=graphical-session.target` too (systemd.io/DESKTOP_ENVIRONMENTS), so the ordering is consistent.

---

## 7. Key names ("Ctrl+Shift+Tab") to evdev keycodes

- **logiops** uses raw kernel names: `keys: ["KEY_LEFTMETA", "KEY_PAGEDOWN"]`, resolved via `libevdev_event_code_from_name` (`LO/src/logid/InputDevice.cpp#L153-L159`).
- **Solaar** uses X keysym names through GDK (`diversion.py#L70-L84`). That is layout-dependent and X-centric. **REJECT**.
- **kanata** (LGPL; `KA/parser/src/keys/mod.rs#L175-L280`): many aliases, e.g. `lctl`/`lctrl`/`ctl`/`ControlLeft` → `KEY_LEFTCTRL`, `lsft`, `lmet`, `tab`, and W3C `KeyboardEvent.code` names like `ShiftLeft`.
- **keyd** (MIT; `KD/src/keys.c`): lowercase names (`leftcontrol`, `esc`/`escape`) plus modifier letters `C S A M G` (`KD/src/keys.h`) for chords like `C-S-tab`.
- **Rust:** `evdev::KeyCode` implements `FromStr` for exact kernel names (`"KEY_TAB"`), from `EV/src/attribute_set.rs#L274-L277`. `keyboard-types` 0.8.3 (MIT/Apache) provides W3C `Code`/`Key` enums but no evdev mapping. No MIT crate does "Ctrl+Shift+Tab" → evdev directly.

**ADAPT: write a tiny parser in `crates/config`.**
- Split on `+`. The last token is the key; the rest are modifiers.
- Modifier aliases (case-insensitive):
  - `ctrl`/`control`/`c` → `KEY_LEFTCTRL`
  - `shift`/`s` → `KEY_LEFTSHIFT`
  - `alt`/`a` → `KEY_LEFTALT`
  - `altgr` → `KEY_RIGHTALT`
  - `super`/`meta`/`win`/`logo`/`m` → `KEY_LEFTMETA`
- Key token, resolved in this order:
  1. An exact kernel name via `KeyCode::from_str` (accept the name with or without the `KEY_` prefix, uppercased).
  2. A small alias table: `esc`, `enter`/`return`, `pgup`/`pageup`, `pgdn`, `del`, `ins`, `space`, `tab`, `backspace`, `left`/`right`/`up`/`down`, `volumeup`/`volumedown`/`mute`, `playpause`, `f1`–`f24`.
- Use **lowercase human names with `+` separators**, the KDE/GNOME shortcut convention. That matches what the owner sees in System Settings shortcut dialogs.
- Keys map to physical evdev codes: "Ctrl+Shift+Tab" is layout-independent, but `KEY_Z` on AZERTY is the physical key, i.e. it types W. Document this, as kanata, keyd and logiops do.
- Emission order: press the modifiers in order, then the key, then release in reverse. Emit them as **separate SYN frames** (press frame, release frame). Some apps miss modifier+key delivered in a single frame, and keyd's and kanata's per-event SYN is the proven pattern.

---

## Sources (web)

- logiops issues: #98 https://github.com/PixlOne/logiops/issues/98, #129 https://github.com/PixlOne/logiops/issues/129, #132 https://github.com/PixlOne/logiops/issues/132, #148 https://github.com/PixlOne/logiops/issues/148, #302 https://github.com/PixlOne/logiops/issues/302, #376 https://github.com/PixlOne/logiops/issues/376, #468 https://github.com/PixlOne/logiops/issues/468, #520 https://github.com/PixlOne/logiops/issues/520, #564 https://github.com/PixlOne/logiops/issues/564
- Kernel event codes: https://docs.kernel.org/input/event-codes.html
- QMK tap-hold: https://docs.qmk.fm/tap_hold
- systemd.special(7): https://www.freedesktop.org/software/systemd/man/latest/systemd.special.html
- systemd desktop environment integration: https://systemd.io/DESKTOP_ENVIRONMENTS/
- crates.io API, versions and licenses: evdev 0.13.2 (MIT OR Apache-2.0), input-linux 0.7.1 (MIT), uinput 0.1.3 (WTFPL, 2018), evdev-rs 0.6.3, keyboard-types 0.8.3
