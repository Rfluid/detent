---
title: Product specification
status: draft
version: 0.1.2
last_updated: 2026-11-25
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [product, spec, requirements]
---

# Detent: product specification

## 1. Vision

The MX Master 4 is one of the most capable mice available: gesture button, thumbwheel, SmartShift wheel, haptic feedback, multi-host. On Linux most of that power is unreachable or needs Python tools that are hard to extend. **Detent makes every capability of the mouse available on Linux: fast, scriptable and extensible.** Native plugins make it useful the moment it's installed.

## 2. Users

| Persona | Wants | Success looks like |
|---|---|---|
| **Power user on KDE** (primary; the author) | Gestures for desktops and Overview, a radial menu on the thumb, per-app bindings | Uses Detent all day without thinking about it, edits one TOML file |
| **Tinkerer** | To script their own behaviour | Writes a 50-line plugin in Rust or Python in an afternoon |
| **Distro / packager** | A daemon that installs cleanly | udev rules + systemd user unit, no root daemon, no surprises |
| **Contributor** | To add a device or HID++ feature | Follows a skill doc, adds a `Feature` module plus tests from a capture |

## 3. Core use cases

1. **Remap** any divertable control to keys, mouse buttons, scroll, commands or plugin actions.
2. **Chord**: `gesture + left` runs a command without also clicking.
3. **Layers**: while the gesture button is held, the wheel switches tabs and the thumbwheel changes volume.
4. **Gestures**: hold the gesture button and swipe up for KDE Overview; left or right to change desktop.
5. **Radial menu**: press the thumb button and a ring of actions appears around the cursor, behaving like the Logitech Actions Ring: the cursor moves freely, hovering a bubble highlights it (haptic tick), clicking selects; or hold the button and release it on a bubble. Esc closes. Contents, icons/photos and actions come from the config file.
6. **Per-app profiles**: in Firefox, gesture swipes go back and forward; elsewhere they switch desktops.
7. **See and edit everything**: `detentctl get` shows every feature the mouse advertises and every setting (desired vs actual). `detentctl set dpi 1600 --persist` changes it live and saves it.
8. **Survive real life**: settings and diversions come back by themselves after sleep, reconnect and Easy-Switch.
9. **Extend**: a third-party plugin subscribes to events, adds actions, leases input and sends haptics.

## 4. Functional requirements

| ID | Requirement | Phase |
|---|---|---|
| F1 | Discover Logitech HID++ devices (Bluetooth direct and Bolt receiver) and hotplug them | 0–1 |
| F2 | Enumerate and display every advertised HID++ feature, known or not | 0 |
| F3 | Read and display: identity, firmware, battery, DPI, SmartShift, wheel, thumbwheel, host slots, haptics | 1 |
| F4 | Divert controls and turn their events into actions through a virtual input device | 2 |
| F5 | Chords, layers, tap/hold/double-tap, swipe gestures | 2, 6 |
| F6 | Chords on non-divertable buttons (left/right) without leaking the click | 2 |
| F7 | Desired/reported state with a reconciler; re-apply on reconnect | 3 |
| F8 | Config in TOML with hot reload; an invalid config never replaces a working one | 3 |
| F9 | Client IPC + `detentctl` for every user-facing capability (get/set/watch/check/plugins) | 3 |
| F10 | Plugin bus: actions, subscriptions, leases, device commands, context, state edits | 4 |
| F11 | Native `kde` plugin (see docs/plugins/kde.md) | 4 |
| F12 | Native `radial` plugin (see docs/plugins/radial.md) | 5 |
| F13 | Per-app profiles driven by plugin-published context | 4 |
| F14 | Warn when Solaar or logiops is running (policy configurable: warn/refuse/ignore) | 1 |

## 5. Non-functional requirements

| ID | Requirement | Target |
|---|---|---|
| N1 | Latency added by the daemon, event → uinput write, built-in action | p99 < 50 µs |
| N2 | Latency event → plugin has the message | p99 < 100 µs |
| N3 | Radial menu visible after the thumb press | the next compositor frame after the plugin receives the action |
| N4 | Idle CPU | ~0 (blocked in poll); no timers on the data plane |
| N5 | Memory (daemon, no plugins) | < 15 MB RSS |
| N6 | Robustness | A plugin crash or hang never affects pointer input; leases always expire |
| N7 | Privileges | Runs as the user; access through udev `uaccess` |
| N8 | Allocation | Zero heap allocations per event on the data plane (verified by a test allocator) |

The N1/N2 targets are measured by benchmarks (`just bench`) and an end-to-end timing harness. See docs/architecture/performance.md.

## 6. Non-goals (for now)

- A kernel driver, or replacing `hid-logitech-hidpp`.
- The daemon on macOS and Windows, in the phases above. Since v0.1.0, `detentctl` reads and
  changes device settings there (a preview, ADR 0010); remapping, plugins and the tray on
  those OSes are open issues for contributors with the hardware (roadmap, "Platforms").
- Firmware updates (DFU), pairing UI, onboard profiles of gaming mice (libratbag covers these).
- Keyboards (the protocol layer won't rule them out, but no product work).
- A full settings GUI before Phase 6. The CLI comes first, and a GUI is just another IPC client.

## 7. Open questions

- Haptics feature ID and functions on the MX Master 4 (Phase 0 discovery).
- Which MX Master 4 controls are divertable (Phase 0 dump decides which can be modifiers).
- GPUI layer-shell: upstream status vs our patch (ADR 0005).
