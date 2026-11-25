---
title: "ADR 0010: Portability seams: device commands everywhere, the daemon on Linux first"
status: accepted
version: 1.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/hidpp-transport/", "crates/config/src/keys.rs", "crates/config/src/keys/", "crates/ipc/src/platform.rs", "crates/ipc/src/client.rs", "crates/ipc/src/client/", "crates/plugin-sdk/", "bins/detentd/src/main.rs", "bins/detent-tray/src/main.rs", "plugins/kde/src/main.rs", "plugins/radial/src/main.rs", "bins/detentctl/src/", ".github/workflows/ci.yml"]
owner: "@rfluid"
tags: [adr, portability, transport, ipc]
---

# ADR 0010: Portability seams: device commands everywhere, the daemon on Linux first

## Context

v0.1.0 ships release builds for Linux, macOS and Windows. Linux was built in at every
layer: hidraw + sysfs discovery in the transport, evdev grab / uinput / udev in the daemon,
Unix sockets for both protocols, ksni + zbus in the tray and the KDE plugin, a Wayland
layer-shell overlay in the radial plugin, and `evdev` for key names in the config crate.

HID++ settings (DPI, SmartShift, wheel, haptics) live **on the device**, so the feature code
in `detent-hidpp-features` / `detent-device` works anywhere frames can be sent. Remapping
does not: it needs an input grab and a virtual input device, which are different per OS
and a design of their own.

## Decision

**What runs where (v0.1.0)**

| Program | Linux | macOS / Windows |
|---|---|---|
| `detentctl` device commands (`devices`, `dump-features`, `dump-controls`, `get`, `set`, `settings list`, `reset`) | hidraw | hidapi |
| `detentctl` daemon commands (`status`, `daemon`, `logs`, `enable`, `reload`, `events`, `plugins`, …) | IPC | exit 69, "not yet supported on <OS>; see <issues>" |
| `detentd`, `detent-tray`, `detent-kde`, `detent-radial` | as before | build, then exit 69 with the same message |
| `detent-plugin` (SDK) | as before | empty crate |

The message and the issue link (`label:platform:<macos|windows>`) come from one place,
`detent_ipc::platform`.

**Gating style: build everywhere, exit early.** Cargo cannot leave a workspace member out
per target, so every member must compile on every target. Each Linux-only binary keeps its
code in `app.rs` and its modules behind `#[cfg(target_os = "linux")]`; `main.rs` holds only
the module list and two `main`s. Their OS-specific dependencies move to
`[target.'cfg(target_os = "linux")'.dependencies]`, so a macOS or Windows build never
compiles GPUI, zbus, ksni, evdev or udev. This is simpler than a feature flag (nothing to
remember on the command line) and keeps `cargo check --workspace` meaningful on every OS.

**Transport: hidapi off Linux, behind the existing `Transport` trait.**
`detent_hidpp_transport::OsTransport` is `HidrawTransport` on Linux and `HidapiTransport`
(`hid.rs`) elsewhere; `discover::logitech()` lists devices for it (`DeviceInfo`, renamed from
`HidrawInfo`). Callers stay generic (`DirectRequester<OsTransport>`): static dispatch, no
`dyn`, no allocation per frame. The Linux data plane is unchanged and still names
`HidrawTransport` (it polls the fd).

- Crate: `hidapi` 2.6 (MIT, the de facto standard; also what Solaar uses on macOS).
  `default-features = false`; `windows-native` on Windows (pure Rust, no C toolchain),
  `macos-shared-device` on macOS (opens without seizing the device). Linux does not depend
  on it at all. Rejected: `async-hid` (pure Rust on macOS too, used by openlogi-hid) because
  everything here is blocking and a runtime for one CLI read is not worth it.
- Which handle: Windows exposes one device per top-level collection and macOS one entry
  per collection. We open the **HID++ long-report collection** (0xFF00/0x0002; 0xFF43/0x0202
  on Bluetooth LE; 0xFF43/0x0602 on wired G keyboards) and send every request as a long
  report, which the requester already does. Short-report-only notifications on the
  0xFF00/0x0001 collection are not read; no portable command needs them. Prior art and
  what is still unverified: docs/specs/transport.md.

**Key names: a Detent-owned table.** `detent-config` no longer depends on `evdev`. Its key
vocabulary is the Linux input-event-codes numbers (`KEY_*`, `BTN_*`), kept as a table in
`crates/config/src/keys/names.rs` generated from the `evdev` 0.13 list. These numbers are
what the engine already stores and what the Linux daemon hands to uinput, so nothing changes
on Linux; a future macOS / Windows output backend translates them at its edge. A Linux-only
test compares the table with `evdev::KeyCode::from_str` name by name, so every config parses
exactly as before. A newtype over the code was considered and rejected for now: it would
touch the allocation-free engine (`Action` holds raw codes) for no behaviour change.

**IPC: no named pipes yet.** `detent-ipc` stays a Unix-socket client (Linux and macOS). On
Windows its stream type is an uninhabited stand-in (`client/no_socket.rs`): `connect_to`
returns `ClientError::Unsupported`, and the rest compiles unchanged. `detentctl` needs the
crate on Windows for the settings catalogue and types, not for a connection. Named pipes
(`interprocess`) wait until there is a Windows daemon to talk to; ADR 0006 is amended.
The plugin bus (`SOCK_SEQPACKET`) is Linux-only, and so is the SDK.

**CI.** `ci.yml` gains macOS (`macos-15`) and Windows (`windows-latest`) jobs that run
`cargo check`, clippy `-D warnings` and `cargo test` for the whole workspace (the Linux-only
members build as stubs). `release.yml` builds the preview on `macos-15` (Apple silicon),
`macos-15-intel` and `windows-latest`.

## Consequences

- macOS and Windows users get a working `detentctl` for device settings in v0.1.0; the rest
  says clearly that it is not there yet and where to follow it.
- Linux behaviour is unchanged; the only Linux-visible renames are in
  `detent-hidpp-transport` (`DeviceInfo`, `discover::logitech`).
- New code in the daemon, tray or plugins compiles only on Linux, so a mistake there shows
  up in the Linux jobs, not the macOS / Windows ones.
- Cross-checking from Linux works with `cargo check --target …` plus build-script overrides
  for the C parts (`ring`; hidapi's macOS backend): docs/dev/ci-and-release.md.
- Follow-ups (issues labelled `platform:macos` / `platform:windows`): verify the transport on
  real hardware, a remapping backend per OS (input grab + virtual device), a daemon IPC
  transport on Windows, per-OS config and runtime directories, packaging.
