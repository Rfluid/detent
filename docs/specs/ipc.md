---
title: Client IPC
status: current
version: 0.4.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/ipc/src/", "bins/detentd/src/ipc.rs", "bins/detentd/src/ipc/handlers.rs", "bins/detentctl/src/"]
owner: "@rfluid"
tags: [spec, ipc, cli]
---

# Client IPC

JSON-RPC 2.0, newline-delimited, over `$XDG_RUNTIME_DIR/detent/ctl.sock`. Setting
`$DETENT_SOCKET` overrides the path for both the daemon and clients, which is useful for
tests or a second daemon. Clients are `detentctl`, the tray icon (`detent-tray`: `status`,
`device.get` for the battery, `remap.set`, `config.reload`, `events.subscribe` on `remap`,
`link`, `config`, `plugin`), a future GUI and scripts. It isn't latency-critical, so it's designed to be easy to debug:

```sh
echo '{"jsonrpc":"2.0","id":1,"method":"status","params":{}}' | socat - UNIX:$XDG_RUNTIME_DIR/detent/ctl.sock
```

Reusable client: `detent_ipc::Client` (`connect`, `connect_to`, `call`, `call_as`,
`status`, `remap`, `set_remap`, `watch`). Result types are in `detent_ipc::types`, and the
settings catalogue is in `detent_ipc::settings`. A GUI should link the crate rather than
re-implement the protocol.

**Platforms (ADR 0010).** The daemon, and so this socket, exists on Linux only for now.
`detent_ipc::platform::DAEMON_SUPPORTED` says so at compile time; elsewhere `detentctl`
never connects: device commands go straight to the device, daemon commands fail with
`ClientError::Unsupported` (exit 69, "not yet supported on <OS>; see <issues>"). The
client builds on Windows, where `Client::connect_to` always returns
`ClientError::Unsupported` (no Unix sockets; named pipes wait for a Windows daemon).

Every request runs on a per-client IPC thread. Device requests go to that device's worker
over a channel and wait at most 10 s. Nothing here touches the data plane except
`remap.set`, which swaps one `ArcSwap` that the data plane already loads every iteration.

## Methods

| Method | Params | Result | CLI |
|---|---|---|---|
| `status` | `{}` | `types::Status`: version, pid, uptime_s, config path + error, bindings, profiles, active profile, context, `remap_enabled`, devices (with `frames`/`dropped`), plugins | `detentctl status` |
| `remap.get` | `{}` | `{enabled, changed: false}` | `detentctl status` |
| `remap.set` | `{ enabled: bool }` | `{enabled, changed}` | `detentctl enable \| disable \| toggle` |
| `devices.list` | `{}` | `[DeviceSummary]` (attached devices) | `detentctl devices` (merged with the sysfs scan) |
| `device.get` | `{ device? }` | `DeviceState` | `detentctl devices info`, `settings get` |
| `device.schema` | `{ device? }` | `[SettingInfo]`: settable paths this device supports, with type, range, value, desired, pending | `detentctl settings list` |
| `device.set` | `{ device?, path, value, persist? }` | the section's new reported value | `detentctl settings set <path> <value> [--persist]` |
| `device.unset` | `{ device?, path }` | the section's reported value | `detentctl settings unset <path>` |
| `device.features` | `{ device? }` | feature table | (`detentctl debug features` reads the device directly) |
| `device.raw` | `{ device?, feature, function, params }` | raw response bytes | not exposed in the CLI (expert, unverified writes) |
| `haptic.play` | `{ device?, waveform }` (name or id) | ok | `detentctl haptic [waveform]` |
| `events.subscribe` | `{ topics: [...] }` | stream of notifications | `detentctl events [topics]` |
| `config.reload` | `{}` | ok / errors | `detentctl config reload` |
| `config.validate` | `{ text }` | ok / errors | `detentctl config check` (validates locally) |
| `bindings.list` | `{}` | `[BindingRow]` of the loaded config | `detentctl bindings` |
| `plugins.list` | `{}` | `[PluginInfo]` | `detentctl plugins`, `plugins status <name>` |
| `plugins.restart` | `{ name }` | ok | `detentctl plugins restart <name>` |
| `actions.list` | `{}` | `[ActionInfo]` | `detentctl actions` |
| `actions.invoke` | `{ action, params }` | ok | `detentctl actions invoke <action> [json]` |

The CLI's command tree and `--json` schemas are documented in docs/user/cli.md.

Notes:
- `device` selects by daemon device id, node (or a node suffix such as `hidraw10`) or
  part of the name. The default is the first device.
- `device.set` changes are live until the next config reload. `persist: true` also
  writes the value into `[device."<name>"]` with `toml_edit` (comments and formatting are
  kept), and refuses any edit that would make the file invalid. Paths and values are
  validated against `detent_ipc::settings::SETTINGS`, which has only hardware-verified
  features. `device.unset` drops the live edit, so the config value applies again (or the
  device is left alone).
- `remap.set {enabled: false}` pauses remapping without stopping the daemon. The data
  planes switch to the loaded config with an empty binding table
  (`Shared::active`). Workers undivert every control and release held keys, and the
  engine drops its evdev grab. Device settings, plugins, profiles and the dispatch thread
  keep using the real config. A config reload while paused stays paused. The state isn't
  persisted: the daemon always starts enabled.
- `device.raw` refuses the 0x1D4B feature (the kernel treats any request there as a reconnect).
- `events.subscribe` acknowledges, then streams `{"method":"event","params":{topic,...}}`
  lines. Topics: `action`, `exec`, `link`, `config`, `profile`, `context`, `plugin`,
  `remap` (`{enabled}`), `setting` (`{device, path, value, persist}` or
  `{device, path, value: null, unset: true}`). Slow subscribers lose events and never
  block the daemon.

## Errors

| Code | Meaning | detentctl exit |
|---|---|---|
| -32700 | parse error | 76 |
| -32601 | unknown method (daemon older than the client) | 76 |
| -32602 | invalid params / path / value out of schema range | 65 |
| 1001 | device not found / disconnected / didn't answer | 66 |
| 1002 | HID++ device error (message includes code) | 74 |
| 1003 | config invalid or couldn't be written | 78 |

When nothing listens on the socket, clients get `ClientError::NotRunning`, and detentctl
exits with 69.
