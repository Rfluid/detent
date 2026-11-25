---
title: Plugin model
status: current
version: 0.2.1
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/bus/", "crates/plugin-sdk/", "crates/config/src/native.rs", "plugins/"]
owner: "@rfluid"
tags: [architecture, plugins]
---

# Plugin model

## Definition

> A **plugin** is a long-running program, supervised by `detentd` and connected over the plugin bus. It **adds named actions** users bind in config, **subscribes** to device events and state, can **lease** controls for exclusive use, and can **command** the device, the daemon and other plugins within the capabilities its manifest declares.

## Why out-of-process (ADR 0003)

The model was derived from the two reference plugins:

| | `kde` | `radial` |
|---|---|---|
| Needs | session D-Bus, KWin scripts, Plasma OSD | Wayland overlay (GPUI) with the real pointer and keyboard, the cursor position, haptics |
| Latency floor | D-Bus round trip | next compositor frame (~7–16 ms) |
| Talks back | publishes `app` context → profile switch; answers `kde.cursor` | leases the trigger while held, pulses haptics, invokes other actions (`kde.cursor`, items) |

Both need I/O, desktop access and UI toolkits, and both have latency floors far above the ~10–30 µs a process boundary costs. Separate processes give crash isolation and any toolkit or language. Native `.so` plugins were rejected: Rust has no stable ABI, and a crash would take the input stack down with it. In-process WASM is kept as an optional, later extension for pure per-frame logic only.

## Kinds of extension

| Kind | Runs where | Cost | Use for |
|---|---|---|---|
| Built-in actions | data plane | ~0 | keys, buttons, scroll, device commands, exec, profiles |
| **Plugins** (native or third-party) | own process, plugin bus | ~10–30 µs per event + plugin work | anything with I/O, desktop integration or UI |
| Feature support | compiled into the daemon | 0 | new HID++ features (`Feature` trait) |
| WASM filters (optional, later) | data plane, epoch deadline | low µs | per-frame pure logic (custom gesture recognizers, curves) |

## Native plugins (ADR 0004)

First-party plugins in `plugins/`, built with the workspace, installed next to `detentd`, registered in `crates/config/src/native.rs`. Users enable them with `[plugins.kde]`, with no `exec`. **They use only the public bus and SDK.** When a native plugin needs a new capability, the protocol grows for everyone. Contract: [`plugins/README.md`](../../plugins/README.md).

## Shared action namespace

Actions are named `<plugin>.<action>` (`kde.overview`, `radial.open`); built-ins have no prefix. Any action can be bound in config, used as a radial menu item, or called by another plugin through `Invoke`. Plugins compose without knowing each other.

## Flows

**Binding to a plugin action**
```
thumb press → data plane: table → Action::Plugin{action, params}
            → try_send to the dispatch thread → non-blocking send ActionInvoked to detent-radial
              (the dispatch thread keeps plugin actions from waiting behind HID++ requests)
```

**Radial menu open (plugins composing over the bus)**
```
radial → LeaseRequest{Control(thumb); max 10 s}     route the thumb to the plugin while held
radial → Invoke("kde.cursor", {reply: "radial.cursor"})
kde    → KWin script (workspace.cursorPos) → Invoke("radial.cursor", {x, y, output, width, height})
radial → overlay on that output, centred there; the pointer stays free (compositor input)
radial → Device(HapticPulse) per bubble entered; on select Invoke(item); LeaseRelease
```
(Pointer/Buttons leases, which grab the mouse and freeze the cursor, remain available to
plugins; the radial menu no longer uses them.)

**Context → profile**
```
KWin script → D-Bus → detent-kde → SetContext{app: "firefox"} → daemon matches [profile.firefox]
            → swaps the active binding table (ArcSwap)
```

## Safety

- Capabilities are granted per plugin (manifest ∩ config `caps` ∩ native defaults).
- Leases have deadlines (`daemon.max_lease_ms`), are revoked on disconnect or missed pings, and the escape chord (left + right held 2 s) revokes all of them.
- The supervisor restarts crashed plugins with exponential backoff and gives up after N failures in M minutes (reported via IPC).
- A full plugin socket drops messages (counted, exposed in `plugins.list`); it never blocks the data plane.
