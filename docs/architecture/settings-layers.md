---
title: Layered device settings (click force, haptics, and every other knob)
status: draft
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["bins/detentd/src/worker.rs", "bins/detentd/src/worker/settings.rs", "crates/ipc/src/settings.rs", "bins/detentd/src/bus.rs", "bins/detentd/src/bus/", "crates/device/src/state.rs", "crates/hidpp-features/src/haptics.rs"]
owner: "@rfluid"
tags: [architecture, state, settings, plugins, proposal]
---

# Layered device settings

**Status: proposal.** Nothing here is implemented yet. Open questions are at the end.

## Problem

1. The thumb panel's click force (HID++ 0x19C0) can't be changed. Writes to it are
   **stored on the mouse**: they survive sleep, other hosts and Detent being stopped.
2. Every setting has one owner today. The config value is the default.
   `detentctl set` and plugin `SetState` both write into one flat `worker.overrides` map:
   the last writer wins, nothing records who set what, and plugin values never expire.
   One plugin can overwrite another plugin's value, and a plugin that crashes leaves its
   value behind.
3. Profiles (per-app) can change bindings but not device settings.
4. `cycle-dpi` and `toggle-smartshift` write to the mouse without updating `desired`, so the
   next resync puts the config value back.

## Goal

The config gives the defaults. Profiles, held layers, bindings and plugins can **declare**
values on top of them. Detent always knows which source is in effect and why, and it puts
the mouse back to a known value when a source goes away.

## Model

### Settings and paths

Every setting has a typed path (a `SettingPath` enum in code, a dotted string at the edges):

| Path | Type | Durability |
|---|---|---|
| `click_force.thumb` | force (see "Click force scale") | **stored** |
| `haptics.enabled`, `haptics.intensity`, `haptics.battery_saving` | bool, 0..=100, bool | volatile |
| `dpi` | int | volatile |
| `smartshift.mode`, `.auto_threshold`, `.torque` | choice, int, int | volatile |
| `wheel.invert`, `thumbwheel.invert` | bool | volatile |

- **Volatile**: the mouse forgets it on sleep, reconnect or host switch. Detent already
  re-applies these on resync.
- **Stored**: the mouse keeps it after Detent stops. Stored settings get extra safeguards
  (see "Stored-setting safeguards").

Each field is resolved separately. A plugin can own `haptics.intensity` while
`haptics.enabled` still comes from the config.

### Scopes

Each path holds a stack of **claims**. A claim is `{ scope, owner, value, since }`. The
effective value is the claim in the highest scope. Lowest to highest:

| # | Scope | Source | Lifetime |
|---|---|---|---|
| 0 | `device` | what the mouse reports; Detent has no opinion | always there |
| 1 | `base` | `[device."<name>"]` in the config | until the config changes |
| 2 | `profile` | `[profile.<p>.device]` while the profile's `match` holds | while the profile is active |
| 3 | `plugin` | bus `Claim` | until released, the TTL ends, or the plugin dies |
| 4 | `hold` | `settings` in a `[layer.<l>]`, while its `hold` button is held | while held |
| 5 | `session` | `detentctl set` without `--persist`, and binding actions `set`, `toggle`, `cycle` | until `detentctl unset` or a config reload |

Ties inside the `plugin` scope go to the plugin with the higher `priority` from
`[plugins.<name>]` (default 0), then to the newer claim.

Why this order: the more deliberate and recent the user's intent, the higher the scope. A
held layer is the user's choice in the moment, so it beats a plugin's standing claim. An
explicit edit (CLI or a binding that sets a value) beats everything until it's undone. The
config stays the floor.

`device` is never written. When no scope has a value, Detent leaves the setting alone (no
config key means hands off, as today).

### Resolution and the reconciler

```
 claims (per path) ──resolve──► Section.desired ──► reconciler (unchanged) ──► mouse
      ▲                                                                         │
 config · profile switch · bus Claim/Release · layer enter/exit · ctl set/unset │
                                                   reported ◄────────────────────┘
```

- A new pure module, `crates/device/src/settings.rs`, holds `SettingStack`:
  `claim`, `release`, `release_owner`, `resolve(path) -> Option<(Value, Provenance)>`.
  It has no I/O, so its unit tests need no device.
- `worker.overrides` and `desired_from_config` are replaced by the stack. `apply_settings`
  stays as it is: it diffs `desired` against `reported` and applies what differs.
- Device actions (`cycle-dpi`, `toggle-smartshift`, the new `set`/`toggle`/`cycle`) write a
  `session` claim and let the reconciler apply it. This fixes problem 4.

### Provenance

`device.get` reports, for each path, the effective value and its source:

```
$ detentctl get click_force.thumb --explain
click_force.thumb = 30%  (raw 5255)      from plugin `game-mode` (claim 3, since 14:02:11)
  session   —
  hold      —
  plugin    game-mode 30%  · obs 45% (priority 0, older)
  profile   —
  base      40%   [device."MX Master 4"]
  device    raw 5342 (device default 5480, range 4384..7288)
```

`--json` returns the same data. `StateDiff` gains an optional `source` field, so plugins see
when another source overrides their claim.

## Configuration

```toml
[device."MX Master 4"]
click_force = { thumb = 40 }                   # % of this unit's range, "default", or { raw = 5480 }
haptics = { enabled = true, intensity = 60 }

[profile.game]
match = { app = "steam_app_*" }
[profile.game.device]                          # same keys as [device."..."]
click_force = { thumb = 15 }
haptics = { intensity = 100 }

[layer.gesture]
hold = "gesture"
settings = { "haptics.intensity" = 100 }       # volatile settings only (see safeguards)
"swipe.up" = { action = "kde.overview" }

[[bind]]
when = "gesture + thumb"
do = { device = "cycle", path = "click_force.thumb", values = [20, 50, 80] }
feedback = "knock"                             # haptic pulse when this binding fires

[[bind]]
when = "smartshift.hold(600ms)"
do = { device = "toggle", path = "haptics.enabled" }

[plugins.game-mode]
exec = "~/.local/bin/detent-game-mode"
caps = ["SettingsClaim"]
claims = ["click_force.*", "haptics.intensity"] # paths this plugin may claim
priority = 10
```

### Per-binding feedback

`feedback = "<waveform>"` or `feedback = { waveform = "knock", intensity = 80 }` on any
binding or layer entry plays a pulse after the action fires. The compiled binding carries
`Option<Pulse>` (two `u8`s, `Copy`), and the data plane does a `try_send` of a
`DeviceCmd::HapticPulse` it already knows. No allocation and no blocking.

The mouse's play function has no intensity argument, so a pulse with its own `intensity`
works like this in the worker: `setConfig(intensity)` → `play` → put the resolved
intensity back after 300 ms of no pulses (bursts such as radial ticks cost one extra write,
not two per tick). The first pulse is about 20 ms later than a plain one.

## Plugin bus additions (additive; protocol stays v1)

| Plugin → daemon | Cap | Effect |
|---|---|---|
| `Claim { id, path, value_json, ttl_ms: Option<u32> }` | `SettingsClaim` + `path` matches the `claims` allowlist | push or replace claim `id` in the `plugin` scope |
| `ClaimRelease { id }` | | drop it |

| Daemon → plugin | When |
|---|---|
| `ClaimGranted { id, effective: bool }` | accepted; `effective = false` if a higher scope shadows it |
| `ClaimDenied { id, reason }` | cap or allowlist missing, value out of range, unknown path, rate limited |
| `ClaimRevoked { id, reason }` | TTL ended, plugin unhealthy, path removed from the allowlist on reload, device gone |

- Claims are released when the plugin disconnects, misses pings or gets `Shutdown`. They
  only outlive the process through the safeguards below. Unlike leases, they **survive a
  config reload** if the allowlist still permits them, because a reload shouldn't undo a
  plugin's state.
- `SetState { path, value }` becomes "claim id 0 of this plugin": same message, but now
  owned, shadowable and released on disconnect. This changes behaviour, so the plugin-bus
  spec notes it.
- SDK: `ctx.claim(path, value) -> ClaimGuard`. Dropping the guard sends `ClaimRelease`.

Leases govern **input**, so they must always expire. Claims govern **settings**: a plugin
may hold one for as long as it's alive (optionally with a TTL), and the mouse never stops
working because of one.

## Stored-setting safeguards (`click_force.*`)

1. **Range and capability.** Read min, max, default and the `changeable` flag (fn1) at
   resync. Refuse writes when not changeable. Clamp `%` values. A raw value out of range
   is an error, not a silent clamp.
2. **No hold scope.** The config compiler rejects stored paths in `[layer.*] settings`
   ("`click_force` is stored in the mouse's memory; holding a layer would rewrite it on
   every press"). Every other scope may use them.
3. **Write limiter.** At most one write per stored path every 2 s. Newer values replace a
   pending one (only the last value is written). A per-device write counter shows in
   `detentctl status`. Plugin claims that change faster are coalesced, not denied.
4. **Baseline.** Before Detent writes a stored setting for the first time on a device, it saves the
   current device value to `$XDG_STATE_HOME/detent/baseline/<product>-<serial>.json`
   (owner's unit: 5342). It's never overwritten after that.
5. **Leave the mouse as the config says.** When `detentd` stops, crashes, or a device's
   worker ends, a stored setting whose effective value came from a scope above `base`
   is written back to the `base` value, or to the baseline if the config has none.
   `detentctl reset` (already in the systemd unit's `ExecStopPost`) does the same from the
   baseline file and config, so a crash can't leave a game profile's 15% behind.
6. `detentctl set click_force.thumb default` uses the device default (fn1).
   `detentctl reset --settings` restores the baseline.

## Click force scale

Raw values are opaque and differ between units (yours: 4384..7288, default 5480). The
config uses **percent of this unit's range**: `0` = lightest (min), `100` = firmest
(max), linear in raw units. Your default is about 38% and the current value about 33%.
`"default"` and `{ raw = n }` are the alternatives. `detentctl get` shows both % and raw.

## Hot-path impact

- Layer enter and exit already happen in the engine. It adds `ControlOut::Scope { layer:
  u8, active: bool }` (`Copy`, `try_send`) only for layers that declare `settings`. The
  worker turns it into `hold` claims.
- Per-binding `feedback`: one `Option<Pulse>` in the compiled binding, sent as
  `DeviceCmd::HapticPulse`.
- Everything else (the stack, resolution, limiter, baseline) is on the control plane.
  Benchmarks gate the engine change.

## Implementation plan

| Step | Change | Files |
|---|---|---|
| 1 | 0x19C0 feature module: `read` (count, info, current), `apply` (fn3), schema; MockTransport tests | `crates/hidpp-features/src/force_button.rs`, `feature_id.rs` |
| 2 | ✅ Done on 2026-11-24: writes survive the power switch, so the setting is stored on the mouse (see the device spec) | `.agent/memory/facts/`, device spec |
| 3 | `SettingStack` + `SettingPath` + provenance; replace `overrides`/`desired_from_config` | `crates/device/src/settings.rs`, `bins/detentd/src/worker.rs` |
| 4 | Config: `click_force`, `[profile.*.device]`, layer `settings`, `set`/`toggle`/`cycle`, `feedback`, plugin `claims`/`priority`; compiler errors for stored paths in hold scope | `crates/config/`, `docs/specs/config.md`, example config |
| 5 | Safeguards: limiter, baseline file, restore on stop and in `detentctl reset` | `bins/detentd/src/worker.rs`, `persist.rs`, `bins/detentctl/src/direct.rs` |
| 6 | Bus `Claim`/`ClaimRelease`/…, `SetState` as claim 0, SDK `ClaimGuard` | `crates/bus`, `crates/plugin-sdk`, `bins/detentd/src/bus.rs`, `docs/specs/plugin-bus.md` |
| 7 | IPC/CLI: `get --explain`, `unset`, provenance in `--json`, `StateDiff.source` | `crates/ipc`, `bins/detentctl`, `docs/specs/ipc.md` |
| 8 | Engine: `Scope` out-event, per-binding `feedback`; bench | `crates/engine`, `bins/detentd/src/dataplane.rs` |
| 9 | A new ADR recording the scope order and the stored/volatile split | `docs/adr/` |

Steps 1 and 3 don't depend on each other. Step 5 must ship before or together with
the first stored write the daemon makes on its own.

## Owner decisions (2026-11-24)

1. Click force scale: **% of the unit's range**, plus `"default"` and `{ raw = n }`.
2. When Detent stops: **restore the `base` value** (or the baseline if the config has none).
3. Scope order: **`session > hold > plugin > profile > base`**.
4. Step 2 verification writes: **approved** (5342 → 5480 → power cycle → read → 5342).
