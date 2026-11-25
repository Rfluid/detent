---
title: Stack map
status: current
version: 0.2.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["Cargo.toml", "crates/", "bins/", "plugins/", "bins/detent-tray/", "crates/update/"]
owner: "@rfluid"
tags: [context]
---

# Stack map

**One sentence:** `detentd` reads HID++ frames from hidraw on a per-device allocation-free thread, maps them through a compiled binding table to uinput output or plugin messages, and keeps a reconciled model of device state that `detentctl` and plugins can read and edit.

## Crate → responsibility → key types

| Crate | Key types / files | Hot path |
|---|---|---|
| `detent-hidpp-proto` | `Frame`, `ReportId`, `SoftwareId::{KERNEL, DETENT}`, `classify() → Kind`, `FeatureId` | yes |
| `detent-hidpp-transport` | `Transport` trait, `OsTransport` (`HidrawTransport` on Linux, `HidapiTransport` on macOS/Windows), `MockTransport`, `discover::logitech() → DeviceInfo` | yes (I/O) |
| `detent-hidpp-features` | `Feature` trait, `Requester` trait (`request`, no-reply `send`), `FeatureTable`, `root::get_feature`, `feature_set::enumerate`, per-feature `State`, `hosts` (Easy-Switch 0x1814/0x1815: read, `switch`, `rename`) | no |
| `detent-device` | `DeviceState`, `Section<T>` (desired/reported/generation), `reconcile::plan`, `receiver` (Bolt pairings by slot, HID++ 1.0 register reads), `requester::send_retrying` | no |
| `detent-engine` | `Action` (Copy enum), `BindingTable::lookup`, `ModMask`/`LayerMap`, `LeaseTable`; `Engine` in `engine.rs` + `engine/{controls,evdev,swipe}.rs`. No runtime dependencies (ADR 0008) | yes |
| `detent-config` | `schema::Config`, `Trigger::parse`, `NativePlugin`, `parse()`, `keys` (Detent-owned key-name table, no `evdev`) | compile time |
| `detent-bus` | `PluginToDaemon`, `DaemonToPlugin`, `Manifest`, `Capability`, `LeaseTarget` | message types |
| `detent-ipc` | JSON-RPC `Client`, `method::*`, `code::*`, `types::Status/RemapState/...`, `settings::SETTINGS` (settable-path catalogue), `platform` (`DAEMON_SUPPORTED`, the "not yet supported on <OS>" message) | no |
| `detent-update` | `check`, `fetch_latest`, `compare`, `Settings` (`[update]` in tray.toml), `find_token` (gh, then file; only on the anonymous rate limit), `assets` (target → release asset name); GitHub Releases over ureq/rustls. Also `sponsor` (`SPONSOR_URL`, `PIX_URL`, `nudge_due`/`record_first_run`), `state::State` (`$XDG_STATE_HOME/detent/state.json`: `first_run`, `sponsor_nudge_done`; tolerant load, atomic save) and `SponsorSettings` (`[sponsor] nudge`) | no |
| `detent-plugin` | `Plugin` trait, `Context`, `ActionCall`, `run`, `connect` | no |
| `detentd` | `app.rs` (startup, hotplug, shutdown), `receivers.rs` (attach receiver slots, watch for paired devices waking up), `dataplane.rs`, `worker.rs` + `worker/{ring,settings,hosts}.rs`, `dispatch.rs`, `bus.rs` + `bus/{handle,supervisor,sink}.rs`, `ipc.rs`, `config_watch.rs`, `persist.rs`, `uinput.rs`, `device.rs` | data plane |
| `detentctl` | `cli.rs` (clap tree), `cmd/<group>.rs` (logic), `output/` (human printers), `exit.rs` (sysexits), `direct.rs` (no-daemon device access); reference docs/user/cli.md | |
| `detent-tray` | `model.rs` (Snapshot → state, icon, tooltip), `menu.rs` (pure menu model), `sni.rs` (ksni), `poll.rs` (IPC + events), `actions.rs`, `platform.rs`, `sponsor.rs` (one-time sponsor notification on its own thread), `icons.rs` (generates packaging/icons); an IPC client, not a plugin (ADR 0009) | |
| `detent-plugin-kde` | `actions.rs` (verified shortcut names), `dbus.rs` (zbus thread, KWin script) | |
| `detent-plugin-radial` | `state.rs` (pure state machine) + `state/{gauge,open}.rs`, `view.rs` (GPUI layer-shell) + `view/{render,card,paint,window}.rs`, `menu.rs` + `menu/{parse,theme}.rs`, `geometry.rs` | |

**Platforms (ADR 0010):** every member builds on Linux, macOS and Windows. Off Linux,
`detentctl` device commands use hidapi and everything else (`detentd`, `detent-tray`,
`detent-kde`, `detent-radial`) builds as a stub `main` that exits 69 with a pointer to the
`platform:<os>` issues; their code lives in `app.rs` behind `#[cfg(target_os = "linux")]`
modules, with Linux-only deps under `[target.'cfg(target_os = "linux")'.dependencies]`.

Package names: `crates/<dir>` → `detent-<dir>` (one exception: `crates/plugin-sdk` → `detent-plugin`),
`plugins/<dir>` → package `detent-plugin-<dir>`, binary `detent-<dir>`; `bins/<name>` → `<name>`.
Dependency direction and the reasons for this layout: ADR 0008.

## Threads and channels

```
data plane (per device) ──try_send──► worker (per device)    HID++ responses, device cmds, notifications
     │   ArcSwap<CompiledConfig>  ◄── rtrb ring + eventfd ── worker (requests, dpi, profile, lease)
     └──try_send──► dispatch thread ──► exec / plugin bus (non-blocking SOCK_SEQPACKET) / leases
IPC threads, config watcher, hotplug, supervisor: std threads (ADR 0007)
```

## Where things are decided

- Event → action: `crates/engine` (tables built by `crates/config` compiler)
- Device truth: `crates/device` (reconciler) via `crates/hidpp-features`
- Plugin contract: `crates/bus` + `docs/specs/plugin-bus.md`
- User contract: `crates/config` + `docs/specs/config.md` + `config/detent.example.toml`

## Current state (2026-11-23, Phases R–5 implemented)

All of it runs on the owner's machine as a systemd user service:
- `detentd`: per-device data plane (`dataplane.rs`: poll on hidraw + evdev + eventfd, engine, uinput),
  worker (`worker.rs`: resync/settings/diversion over the request ring), dispatch thread
  (exec, plugin actions, leases), hotplug (udev), config watcher (hot reload), IPC (`ipc.rs`),
  plugin bus + supervisor (`bus.rs`), persistence (`persist.rs`). std threads only (ADR 0007).
- `detent-engine`: allocation-free state machine (chords, layers, swipes in mm, holds, grab
  policy, leases + escape chord); zero-alloc test and criterion benches.
- `detent-config`: compiler to engine tables, key names, profiles.
- `detentctl`: noun-verb tree (status, enable/disable, daemon, logs, config, devices,
  settings, hosts (Easy-Switch list/switch/rename), bindings, actions, plugins, events,
  haptic, reset, debug, completions, version,
  update check, sponsor);
  `--json` everywhere; works without the daemon where possible. Remap pause: `Shared::active`
  (data-plane view, bindings stripped) vs `Shared::config` (everything else).
- `detent-kde` (zbus): kglobalaccel actions, kmix volume OSD, KWin-script app context;
  `kde.volume.get/set/mute` with replies (wpctl, `kde-audio` thread), `kde.terminal`.
- `detent-radial` (gpui-pre, layer-shell): Actions-Ring-style menu; free cursor + hover,
  click or hold-release, Esc/Tab; trigger-only lease; position from `kde.cursor`;
  adjustable items (`adjust = "kde.volume"`: gauge card, wheel/thumbwheel from the overlay's
  own `wl_pointer.axis`); light/dark palettes, `theme = "system"` via the XDG portal.

Not done: double_tap, wheel leases (not needed by the radial gauge: wheels are never
diverted, the overlay receives them), a persistent radial surface (opening costs ~140 ms).
Bolt receiver devices (slots 1–6, issue #5): discovery, `detentctl` and the daemon's attach
path are in; `detentctl` is verified on the owner's receiver, the daemon's end-to-end
acceptance run is pending. Unifying receivers are not enabled.
