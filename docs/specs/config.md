---
title: Configuration reference
status: current
version: 0.2.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["crates/config/", "config/detent.example.toml", "bins/detentd/src/config_watch.rs", "bins/detentd/src/persist.rs", "crates/update/src/settings.rs"]
owner: "@rfluid"
tags: [spec, config]
---

# Configuration reference

File: `~/.config/detent/config.toml` (`$XDG_CONFIG_HOME/detent/config.toml`). Full example:
[`config/detent.example.toml`](../../config/detent.example.toml), which a unit test keeps
valid. Check a file with `detentctl check [file]`. No file = no bindings, mouse unchanged.

Reload is automatic on save. **An invalid file is rejected and the previous config stays
active**; `detentctl status` and the log show the error.

## `[daemon]`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `conflict_policy` | `"warn" \| "refuse" \| "ignore"` | `"warn"` | What to do if Solaar, logiops or JuhRadial is running |
| `max_lease_ms` | int | `10000` | Longest a plugin may own input |

## `[gesture]`

| Key | Default | Meaning |
|---|---|---|
| `threshold_mm` | `6.0` | Physical movement while holding the modifier before a swipe fires (1–50) |
| `haptic` | `false` | Haptic pulse when a gesture resolves: a swipe in any direction, a diagonal, or a tap (no movement). `true` = `damp-state-change`, or a waveform name (`subtle-collision`, ...; list them with `detentctl haptic --list`; try one with `detentctl haptic <name>`). Chords don't pulse |

## `[device."<name or serial>"]`

Matched against the device name (`"MX Master 4"` matches "MX Master 4"), its serial/BT
address, or `"*"`. Every key is optional; omitted settings are left as they are.

| Key | Type | Example |
|---|---|---|
| `dpi` | int | `1600` (200..8000, step 50 on the MX Master 4) |
| `dpi_cycle` | list | `[800, 1600, 3200]` for `device = "cycle-dpi"` |
| `smartshift` | table | `{ mode = "ratchet" \| "free-spin", auto_threshold = 12, torque = 50 }` |
| `haptics` | table | `{ enabled = true, intensity = 60 }` |
| `thumbwheel` | table | `{ invert = true }` (default when omitted: inverted, natural direction) |
| `wheel` | table | `{ invert = false }` |

`detentctl set <path> <value> --persist` writes here, keeping your formatting and comments.

## Triggers

```text
trigger  := (modifier " + ")* target
target   := control ("." edge)?
edge     := press | release | tap | hold | hold(<N>ms) | up | down | left | right
```

| Example | Meaning |
|---|---|
| `thumb` / `thumb.press` | Haptic Sense Panel pressed |
| `gesture.tap` | gesture pressed and released with no movement or chord |
| `thumb.hold(400ms)` | held for 400 ms |
| `gesture + swipe.left` | hold gesture and move left ≥ `threshold_mm` (a diagonal move fires nothing and is no tap) |
| `gesture + wheel.up` | wheel up while gesture is held |
| `gesture + thumbwheel` | thumbwheel while gesture is held, continuous (needs a plugin `action`) |
| `gesture + left` | chord: gesture held, then left pressed |
| `gesture + thumb + wheel.down` | two modifiers |

**Controls** (MX Master 4): `left`, `right`, `middle`, `back`, `forward`, `gesture`,
`smartshift` (the button above the wheel), `thumb` (Haptic Sense Panel), `wheel`,
`thumbwheel`, `swipe`, and `cid_0x<hex>` for any other divertable control.
Rules (the compiler explains violations):
- modifiers must be divertable buttons (not left/right/wheels);
- `left`, `right`, `wheel`, `thumbwheel` and `swipe` only work while a modifier is held;
- `double_tap` is reserved (Phase 6).

## Actions (`do = { ... }`)

Exactly one of:

| Key | Example | Runs on |
|---|---|---|
| `keys` | `"Ctrl+Alt+Left"`, `"Super"`, `"VolumeUp"` (kernel key names, case-insensitive) | data plane (uinput) |
| `button` | `"back"`, `"forward"`, `"middle"`, `"left"`, `"right"` | data plane |
| `device` | `"cycle-dpi"`, `"toggle-smartshift"`, `"haptic"` (+ `waveform = "..."`), `"dpi"` (+ `value = 1600`), `"host"` (+ `value = 2`, `"next"` or `"prev"`: Easy-Switch to that computer; the mouse leaves this one) | worker → device |
| `exec` | `"spectacle -r"` (run with `sh -c`, detached) | control plane |
| `action` | `"kde.overview"`, `"radial.open"` | plugin |

Other keys are parameters passed to the action: `{ action = "radial.open", menu = "main" }`.

Key names: the aliases in `crates/config/src/keys.rs` (`ctrl`, `super`, `volup`, ...),
single letters and digits, and every Linux `KEY_*` / `BTN_*` name with or without the
`KEY_` prefix. They resolve through Detent's own table (`keys/names.rs`) to Linux
input-event-codes numbers, the OS-independent key codes the engine stores (ADR 0010); a
Linux-only test keeps the table identical to the `evdev` crate's names.

## `[[bind]]`

```toml
[[bind]]
when = "gesture + left"
do = { exec = "spectacle -r" }
```

## `[layer.<name>]`

Active while `hold` is held. Other keys are targets; **quote dotted keys**.

```toml
[layer.gesture]
hold = "gesture"
"swipe.up" = { action = "kde.window.maximize" }
thumbwheel = { action = "kde.volume" }
```

## `[plugins.<name>]`

| Key | Meaning |
|---|---|
| `enable` | default `true` |
| `exec` | path to the plugin binary. **Omit for native plugins** (`kde`, `radial`) |
| `caps` | capability list; native plugins have defaults |
| anything else | plugin-owned settings, sent to the plugin as JSON and validated by it |

## `[profile.<name>]`

```toml
[profile.firefox]
match = { app = "firefox" }          # context keys published by plugins (detent-kde: app, desktop)

[[profile.firefox.bind]]
when = "gesture + left"
do = { keys = "Ctrl+W" }

[profile.firefox.layer.gesture]
hold = "gesture"
"swipe.left" = { keys = "Alt+Left" }
```

A profile's bindings override the base ones for the same trigger while its `match` holds
(case-insensitive). The first matching profile in file order wins.

## Tray settings: `tray.toml`

The tray and `detentctl update check` read a second, optional file,
`~/.config/detent/tray.toml` (`$XDG_CONFIG_HOME/detent/tray.toml`; parsed by
`crates/update/src/settings.rs`, not by the daemon). Each table is read on its own; a
missing table or key is its default, a misspelt key is an error for that table only.

| Table | Key | Default | Meaning |
|---|---|---|---|
| `[update]` | `check` | `true` | Background update checks in the tray (manual checks always run) |
| `[update]` | `token_file` | none | Read-only GitHub token file, used only when the anonymous check hits the rate limit (`~/` expanded) |
| `[sponsor]` | `nudge` | `true` | `false`: never show the tray's one-time "Enjoying Detent?" notification (docs/user/tray.md). An unreadable file also keeps it off |

What Detent records about itself (when the tray first ran, whether the sponsor
notification was answered) is state, not config: `~/.local/state/detent/state.json`.
