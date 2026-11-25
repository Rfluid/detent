---
title: Architecture overview
status: current
version: 0.3.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/", "bins/", "plugins/"]
owner: "@rfluid"
tags: [architecture]
---

# Architecture overview

## What Detent is

A **userspace daemon**. The kernel's `hid-logitech-hidpp` keeps driving basic input. Detent:

1. talks **HID++ 2.0** to the mouse firmware over `/dev/hidrawN`
2. **diverts** controls so the firmware reports them to Detent instead of as normal clicks
3. turns events into actions, emitted through a virtual **uinput** device or sent to plugins
4. keeps a typed **model of device state** that clients and plugins can read and edit

## Platforms

Everything below is the Linux design, and only Linux runs it today. On macOS and Windows
the workspace builds, `detentctl` reads and edits device-side settings directly over
hidapi (`devices`, `dump-features`, `dump-controls`, `get`, `set`, `reset`), and the daemon,
tray and plugins exit with a "not yet supported" message. The seams (transport backend per
OS, Detent-owned key codes, IPC client stub on Windows, `cfg`-gated binaries) are in
[ADR 0010](../adr/0010-portability-seams.md) and [specs/transport.md](../specs/transport.md).

## Three parts

```
  config.toml ─watch─►┌───────────────── CONTROL PLANE (tokio) ─────────────────┐
                      │ config compiler → CompiledConfig    state store ◄ reconciler │
  detentctl / GUI ◄──►│ client IPC (JSON-RPC)   plugin supervisor (spawn, caps)  │
                      └────────────────────────┬────────────────────────────────┘
                                     ArcSwap   │   ▲ SPSC rings ▼
  ┌──────────── DATA PLANE (1 thread per device, no async, no alloc) ─────────────┐
  │ hidraw ─► decode ─► classify ─┬─ response ─► pending slot ─► control plane    │
  │ evdev* ─┘                     └─ event ─► modifiers/layer ─► lease ─► binding │
  │                                   built-in action ─► uinput write (same thread)│
  │ ◄─ request ring (device cmds)     plugin action / leased input ─► bus send ──┐ │
  └──────────────────────────────────────────────────────────────────────────────┼─┘
                                                                                 ▼
  ┌──────────────── PLUGIN BUS (Unix SOCK_SEQPACKET, postcard) ──────────────────┐
  │   detent-kde (native)      detent-radial (native)      third-party ...        │
  └──────────────────────────────────────────────────────────────────────────────┘
   * evdev is opened only while a chord on a non-divertable button is armed
```

| Part | Job | Rules |
|---|---|---|
| **Data plane** | Every device byte in and out; event → action | No allocation, no locks, no async, O(1) per event. See [data-plane.md](data-plane.md) |
| **Control plane** | Config, reconciler, IPC, plugin supervision, exec | Async, may allocate, never on the event path |
| **Plugin bus** | Two-way channel to plugin processes | Non-blocking sends from the data plane; slow plugins lose messages, not input |

## Crates

| Crate | Role | Hot path? |
|---|---|---|
| `detent-hidpp-proto` | Frames, report IDs, swid, classify, feature IDs. `no_std` | **yes** |
| `detent-hidpp-transport` | hidraw I/O + sysfs discovery (Linux), hidapi (macOS, Windows), mock/replay transport | **yes** (I/O) |
| `detent-hidpp-features` | `Feature` trait + per-feature modules (read/apply/notify/schema) | no |
| `detent-device` | `DeviceState`, sections (desired/reported), reconciler, requester | no |
| `detent-engine` | Binding table, modifiers/layers, leases, gesture FSM, uinput | **yes** |
| `detent-config` | TOML schema, trigger grammar, key-name table, native plugin registry, compiler | no (compile time) |
| `detent-bus` | Plugin bus messages (public API) | message types |
| `detent-ipc` | Client JSON-RPC types, client, `platform` (what runs on which OS) | no |
| `detent-plugin` | Plugin SDK (used by native and third-party plugins); Linux only | no |
| `detentd` | Wires everything together; Linux only | |
| `detentctl` | CLI; device commands on every OS | |
| `detent-tray`, `plugins/kde`, `plugins/radial` | Tray icon, native plugins; Linux only | |

## Threads

| Thread | Count | Blocks on |
|---|---|---|
| Device data plane | 1 per device | `poll(hidraw, evdev?, eventfd)` |
| Control plane | std threads (ADR 0007): worker per device, dispatch, IPC, bus, supervisor, hotplug, config watcher | channels, sockets, inotify, udev |
| Plugins | separate processes | their own loops |

## Key flows

- **Button → action:** [data-plane.md](data-plane.md)
- **Chord / layer / gesture:** [bindings.md](bindings.md)
- **Edit a setting / reconnect:** [device-state.md](device-state.md)
- **Plugin action, lease, haptic reply:** [plugins.md](plugins.md)

## Decisions

See [../adr/](../adr/): userspace daemon (0001), split planes (0002), out-of-process plugins (0003), native plugins in repo (0004), GPUI vendored (0005), Unix socket protocols (0006), std threads (0007), crate layout (0008), tray as a client (0009), portability seams (0010).
