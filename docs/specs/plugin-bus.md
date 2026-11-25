---
title: Plugin bus protocol
status: current
version: 0.2.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/bus/src/lib.rs", "crates/plugin-sdk/src/lib.rs", "crates/engine/src/leases.rs"]
owner: "@rfluid"
tags: [spec, plugins, protocol]
---

# Plugin bus protocol (v1)

Normative. Message types are defined in `crates/bus/src/lib.rs`, and that file and this doc change together. Any incompatible change bumps `PROTOCOL_VERSION`.

## Transport

- Unix `SOCK_SEQPACKET` at `$XDG_RUNTIME_DIR/detent/bus.sock` (dir mode 0700). Linux only,
  like the daemon; the SDK is an empty crate on other OSes (ADR 0010, ADR 0006 amendment).
- One message per packet; no framing. Max packet 64 KiB (`MAX_PACKET`).
- **Codec:** postcard (default) or JSON. The daemon sniffs the first packet (`Hello`): JSON if it
  starts with `{`, otherwise postcard; every later packet in both directions uses that codec.
  JSON makes Python/shell plugins trivial.
- Supervised plugins also get the socket path in `DETENT_BUS` and their name in `DETENT_PLUGIN`.
- Implementation: `bins/detentd/src/bus.rs` (server, handshake) with `bus/handle.rs` (messages,
  capabilities, leases), `bus/supervisor.rs` (processes, pings) and `bus/sink.rs` (daemon side);
  `crates/plugin-sdk` (client).

## Handshake

```
plugin → Hello(Manifest { name, version, protocol, codec, actions[], caps[] })
daemon → Welcome { protocol, granted: caps[] }      (or closes the socket with a reason logged)
daemon → Config { settings_json }                   (the plugin's [plugins.<name>] section)
plugin → Subscribe([topics])                        (optional)
```

Granted caps = requested ∩ (config `caps` or native defaults).

## Messages

### Plugin → daemon

| Message | Cap required | Effect |
|---|---|---|
| `Hello(Manifest)` | | register; actions become `<name>.<action>` |
| `Subscribe([Topic])` | `EventsSubscribe` | receive `Event`s for those topics |
| `LeaseRequest { id, targets, max_ms }` | `LeasePointer` / `LeaseButtons` | see Leases |
| `LeaseRelease { id }` | | end a lease |
| `Device(DeviceCommand)` | `DeviceHaptics` / `DeviceControl` | queued on the device request ring |
| `Invoke { action, params_json }` | `ActionsInvoke` | run any action (built-in or another plugin's) |
| `SetContext { key, value }` | `ContextPublish` | context for profile matching |
| `SetState { path, value_json }` | `StateWrite` | edit device state via the reconciler |
| `Pong { nonce }` | | liveness |

### Daemon → plugin

| Message | When |
|---|---|
| `Welcome`, `Config` | handshake, config reload |
| `ActionInvoked { action, params_json, trigger, delta, ts_us }` | a binding fired this plugin's action (`action` is the local name; `delta` = wheel detents for continuous actions, else 0; `ts_us` = wall clock µs) |
| `Event(Event)` | subscribed topics; also leased input |
| `StateDiff { path, value_json, generation }` | subscribed `StateDiff` |
| `LeaseGranted` / `LeaseDenied` / `LeaseRevoked { reason }` | lease lifecycle |
| `Ping { nonce }` | every 2 s; 3 missed pongs → leases revoked, plugin restarted |
| `Shutdown` | daemon stopping or plugin disabled |

### Built-in actions

Callable with `Invoke` (cap `ActionsInvoke`), no plugin prefix:

| Action | Params | Extra cap |
|---|---|---|
| `exec` | `{ "cmd": "spectacle -r" }` | |
| `keys` | `{ "keys": "Ctrl+C" }` (same names as config `keys`) | `InputEmit` |

## Supervision

- Enabled `[plugins.<name>]` entries are started by the daemon (native ones resolved next to
  `detentd`, then `$PATH`); stdout/stderr go to the daemon's journal.
- Exit → restart with backoff 2, 4, 8, 16, 30 s; a plugin that ran ≥ 60 s starts again at 2 s.
- `Ping` every 2 s; more than 3 unanswered → the process is killed and restarted.
- Config reload → every connected plugin gets `Config` again; disabled plugins get `Shutdown`
  and are stopped (SIGTERM after 0.5 s, then SIGKILL).
- Capabilities a plugin asks for but isn't allowed are logged as a warning, not fatal.

## Delivery guarantees

- Daemon → plugin sends are **non-blocking**. If the socket buffer is full the message is dropped and a counter increments (visible in `detentctl plugins`). Continuous events (motion, rotation) may be coalesced. Input is never delayed for a plugin.
- `ActionInvoked` for discrete actions is prioritized over motion events when coalescing.
- Order is preserved per plugin.

## Leases

A lease routes controls exclusively to one plugin for a bounded time.

| Target | Mechanism | Desktop sees |
|---|---|---|
| `Pointer` | evdev grab of the mouse; `REL_X/Y` summed per input frame → `Event::Motion` (raw-XY diversion only works while a diverted button is held, so it can't back a menu that stays open) | cursor frozen |
| `Buttons` | evdev grab; `BTN_*` → `Event::Button{left,right,middle,back,forward}` | nothing |
| `Control(name)` | a control that is already diverted (bound in config); its press/release → `Event::Button{name}` instead of its binding | nothing |
| `Wheel`, `Thumbwheel` | not implemented (denied) | |

Rules:
- `max_ms` ≤ `daemon.max_lease_ms` (default 10 s); expired leases are revoked (`Expired`).
- One owner per control; conflicting requests get `LeaseDenied`.
- Revoked on plugin disconnect, missed pings, config reload (`ConfigReload`), device disconnect.
- **Escape chord**: left + right held 2 s revokes all leases (`EscapeChord`). It cannot be leased or rebound.

## Versioning

`Hello.protocol` must equal the daemon's major version. Additive changes (new optional message variants handled as "unknown, ignore") don't bump the version; anything else does.
