---
title: Roadmap
status: current
version: 0.3.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: []
owner: "@rfluid"
tags: [product, roadmap]
---

# Roadmap

## The acceptance criterion

**The owner can install Detent and use it to control their own MX Master 4 every day.** Everything below serves this. A phase is done only when its result can be *used* on the real mouse, not merely when it compiles and its tests pass. The final gate is the [daily-driver acceptance](#daily-driver-acceptance) checklist, which the owner runs by following `docs/user/quickstart.md` alone.

Each phase also ends with `just verify` green, docs updated, and on-device checks done (`.agent/skills/verify-on-device.md`). Where a human has to press buttons, the agent gives exact steps and expected results.

Current phase: **daily-driver acceptance** (owner). Phases R–5 are implemented and verified by the agent on the owner's machine as far as possible without pressing buttons; the remaining hands-on checks are listed in the final report and in each phase below.

## Phase R: Research prior art (done 2026-11-23: docs/research/prior-art.md)
Study existing open-source tools before writing device code, so known solutions are reused instead of rediscovered. Output: `docs/research/prior-art.md` (see `.agent/workflows/research-prior-art.md`).
- **Accept:** the doc answers, with source links, every ⚠ item that public sources can settle: HID++ function layouts, MX Master 4 CIDs and haptics (if anyone has published them), reconnect handling, evdev grab and uinput patterns, KWin/kglobalaccel names, GPUI layer-shell status. Each finding is marked *adopt*, *adapt* or *reject*.

## Phase 0: Know the device (done 2026-11-23: docs/specs/devices/mx-master-4.md)
- Timed hidraw `recv` (poll), a blocking `Requester`
- `detentctl dump-features [--json]`, `detentctl dump-controls` (CIDs + flags)
- Haptics discovery (research findings first, then read-only probes)
- **Accept:** the owner runs `detentctl dump-features` and `dump-controls` and sees their mouse's real features and controls; the output is saved in `docs/specs/devices/mx-master-4.md`

## Phase 1: See everything (done 2026-11-23; `detentctl get` reads the device directly until the daemon's IPC lands in Phase 3)
- Feature modules: name/type, firmware, battery, DPI, SmartShift, hi-res wheel, thumbwheel, hosts
- `detentctl get [--json]`; conflict policy
- **Accept:** `detentctl get` shows the owner's real DPI, battery, SmartShift mode; values match after changing them physically or in another tool

## Phase 2: Control it (implemented 2026-11-23; agent-verified: attach, diversion, uinput, restore on stop; benches + zero-alloc test green. Owner: chords/swipes/reconnect by hand)
- Data plane, diversion, uinput, gesture FSM, chords + grab-while-armed, re-apply on reconnect, benches, zero-alloc test
- `just install` + udev rules + systemd user unit, so the daemon runs from login
- **Accept:** with Detent installed as a service and the example config, the owner remaps a button, uses `gesture + left`, and swipes, all on their mouse. Still works after sleep and Bluetooth reconnect. No click leaks from chords.

## Phase 3: Edit everything (implemented 2026-11-23; agent-verified: live set, --persist, broken config rejected, hot reload)
- Config compiler + hot reload, reconciler, client IPC, `detentctl get/set/watch/check/reload`
- **Accept:** the owner changes DPI and SmartShift with `detentctl set ... --persist` and by editing the config file; changes apply live and persist across restarts; a broken config is rejected with a clear message while the mouse keeps working

## Phase 4: Plugins + KDE (implemented 2026-11-23; agent-verified: kde actions, kmix volume, KWin app context, kill -9 restart)
- Plugin bus, supervisor, capabilities, leases, escape chord, SDK, native `detent-kde`, profiles
- **Accept:** on the owner's Plasma session: swipe up opens Overview, swipes change desktops, gesture + thumbwheel changes volume with the OSD, Firefox profile switches automatically; killing `detent-kde` never affects the pointer and it restarts

## Phase 5: Radial menu (implemented 2026-11-23; reworked 2026-11-24 to the Actions Ring look and behaviour; agent-verified in a nested KWin: ring at the KWin cursor position on the cursor's output, focus returns to the app. Owner: hover/click/hold by hand, haptic ticks. Known: ~140 ms to appear, see docs/plugins/radial.md)
- GPUI layer-shell, `detent-radial` modelled on the Logitech Actions Ring: free cursor + hover, click and hold-release both at once, Esc/Tab/Enter, folders, per-app rings; trigger-only lease; `kde.cursor` for the position; haptic ticks
- **Accept:** pressing the thumb panel shows the ring centred on the cursor; the owner selects by hover + click and by hold + release; Esc / right click / centre close; haptic ticks are felt; a plugin crash never leaves input grabbed

## Daily-driver acceptance

The owner, following only `docs/user/quickstart.md` on a fresh login:

1. Installs Detent (`just install`, plus one sudo step for udev rules) and enables the service.
2. Reboots or logs in again; `detentctl devices` and `detentctl get` show the mouse.
3. Copies the example config; every binding in it works on the mouse.
4. Edits a binding in `~/.config/detent/config.toml`; it applies on save without a restart.
5. Uses KDE gestures, the radial menu and a per-app profile for a normal work session.
6. Mouse sleeps / Bluetooth reconnects / Easy-Switch away and back: everything keeps working.
7. Stops the service: the mouse behaves exactly as before Detent (no stuck diversion or grab).
8. Uninstalls cleanly (`just uninstall`).

## Phase 6: Polish (after daily-driver acceptance)
Timing triggers and sequences, Plasma theming for radial, settings GUI (GPUI) as an IPC client, packaging (RPM/COPR), optional WASM filters if a real need appears.

## Platforms (from v0.1.0, open to contributors)

The phases above target the owner's setup: Linux, KDE Plasma 6 on Wayland, MX Master 4 over
Bluetooth. At v0.1.0 the rest stands as follows (the README's "Platform support" table is the
user-facing version and must stay in sync):

| Platform | At v0.1.0 | Open work |
|---|---|---|
| Linux, KDE Plasma 6 | full support | — |
| Linux, GNOME | settings, remapping, gestures; no desktop plugin, radial or verified tray | [`platform:linux-gnome`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3Alinux-gnome) |
| Linux, Hyprland / Sway | settings, remapping, gestures; no desktop plugin or radial | [`platform:linux-wlroots`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3Alinux-wlroots) |
| Linux, X11 sessions | settings, remapping, gestures; no cursor/app context, no radial | [`platform:linux-x11`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3Alinux-x11) |
| macOS | preview: `detentctl` reads and changes device settings over hidapi (live only, no `--persist`; not yet run on real hardware); daemon commands exit 69 | [`platform:macos`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3Amacos) |
| Windows | preview: `detentctl` reads and changes device settings over hidapi (live only, no `--persist`; not yet run on real hardware); daemon commands exit 69, no daemon IPC (Unix sockets only) | [`platform:windows`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aplatform%3Awindows) |
| Receivers and other devices | MX Master 4 over Bluetooth only | [`area:transport`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aarea%3Atransport) |
| Distribution packages, signing | release archives and installers | [`area:packaging`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3Aarea%3Apackaging) |

On macOS and Windows the first step is verifying the hidapi transport on real hardware (the
⚠ items in `docs/specs/transport.md`); the daemon ports build on that, on per-OS config
directories and, on Windows, on named-pipe IPC (ADR 0006 amendment, ADR 0010).

Everything open: [`help wanted`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22).
Each issue carries its own acceptance check, run on real hardware by whoever has it, and a
one-shot prompt for a coding agent (CONTRIBUTING.md). The same acceptance rule applies as
above: a port is done when someone controls their own mouse with it on that platform, not
when it compiles. Facts verified on a new platform go to `.agent/memory/facts/` and the
relevant spec, as they did for KDE.
