---
title: detentctl command reference
status: current
version: 0.3.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["bins/detentctl/src/", "crates/ipc/src/", "crates/update/src/", "justfile"]
owner: "@rfluid"
tags: [user, cli, reference]
---

# detentctl

`detentctl` is the command-line interface to Detent. Commands use a noun-verb form:
`detentctl <group> <verb> [args]`. A group with no verb runs its read command, so `devices`
is the same as `devices list`. The shape follows `systemctl`, `tailscale` and `podman`; the
device commands follow `ratbagctl` and Solaar's CLI.

Most commands talk to the running daemon (`detentd`) over the client IPC socket
(docs/specs/ipc.md). These also work when the daemon isn't running, by talking to the
mouse or reading the file directly: `devices`, `settings list|get|set` (without
`--persist`), `bindings`, `config path|show|edit|check`, `reset`, `debug`.

**macOS and Windows.** There is no daemon there yet (ADR 0010), so `detentctl` always talks
to the mouse directly, over hidapi: `devices`, `devices info|battery`, `get`, `set`
(without `--persist`), `settings list`, `reset`, `dump-features`, `dump-controls` work.
Commands that need the daemon (`status`, `daemon`, `logs`, `enable`/`disable`/`toggle`,
`reload`, `events`, `plugins`, `actions`, `haptic`, `settings unset`, `set --persist`) exit
69 with "not yet supported on <OS>" and a link to the open `platform:<os>` issues.
Device-side behaviour there is ⚠ unverified on real hardware: docs/specs/transport.md.

## Global options

These work before or after the subcommand.

| Option | Meaning |
|---|---|
| `--json` | Machine-readable output on stdout for every read command (schemas below). `events` prints NDJSON. |
| `-v`, `--verbose` | More detail on stderr. `-vv` also traces every IPC request and reply. |
| `-q`, `--quiet` | Print only errors and the requested data (no "done" lines). Can't be combined with `-v`. |
| `--socket <PATH>` | Daemon socket. Env: `DETENT_SOCKET`. Default: `$XDG_RUNTIME_DIR/detent/ctl.sock`. |
| `--color auto\|always\|never` | `auto` (default) colors only on a terminal and honours `NO_COLOR` and `TERM=dumb`. |
| `-d`, `--device <DEVICE>` | Pick a device by hidraw node (`hidraw10`, `/dev/hidraw10`), daemon device id, or part of its name. Default: the first one. |
| `-V`, `--version` | Version. `--version` also shows the commit, target and build profile. |

## Commands

```
detentctl status                     daemon health: version, pid, uptime, config, remapping, devices, plugins
detentctl enable | disable | toggle  resume / pause remapping in the running daemon
detentctl daemon start|stop|restart|status   the systemd user service (detent.service)
detentctl logs [-f] [-n N] [--since T] [--plugin NAME]   journalctl --user -u detent.service
detentctl config path|show|edit|check|reload
detentctl devices [list|info|battery]
detentctl settings [list|get|set|unset]      alias: setting
detentctl bindings [--profile P] [--file F]
detentctl actions [list|invoke]
detentctl plugins [list|status|restart]
detentctl events [TOPIC...] [-n N]           aliases: monitor, watch
detentctl haptic [WAVEFORM] [--list]
detentctl reset                              give the buttons back (no daemon needed)
detentctl debug features|controls            raw HID++ inspection (no daemon needed)
detentctl completions <bash|zsh|fish|elvish|powershell>
detentctl version
detentctl update check
detentctl sponsor [--open]                   GitHub Sponsors and Pix links (any OS, no daemon)
```

### Daemon

- `status`: daemon health. Exits 69 when the daemon isn't running; with `--json` it still
  prints `{"running": false, "socket": ..., "service": "inactive"}`.
- `enable` / `disable` / `toggle`: pause or resume **remapping** without stopping the daemon.
  While paused, Detent undiverts every button, releases anything it holds and drops the evdev
  grab, so the mouse acts as if Detent weren't running. Device settings (DPI, SmartShift,
  wheels, haptics) and plugins keep running. The paused state is not saved: a daemon restart
  starts with remapping on. The tray icon uses the same calls (`remap.get` / `remap.set`)
  and can follow the `remap` event.
- `daemon start|restart` run `systemctl --user <verb> detent.service` and then wait (up to
  10 s) until the daemon answers. Pass `--no-wait` to return as soon as systemctl does.
  `daemon stop` stops the service; its `ExecStopPost` gives the buttons back.
  `daemon status` shows `systemctl --user status`. With `--json` it prints `{unit,
  active_state, sub_state, unit_file_state, main_pid}`. To start Detent at login, use
  `systemctl --user enable detent` (`just install` explains this).
- `logs` replaces itself with `journalctl --user -u detent.service -n 50`. `-f` follows,
  `--since "10 min ago"` filters, `--json` switches to `-o json`. Plugin output is in the
  same unit. `--plugin radial` keeps only that plugin's lines (`detent-radial: …`), the
  supervisor's lines about it (exits, restarts, missed pings) and KWin's
  `error in client communication (pid N)` from `plasma-kwin_wayland.service`, which is how
  a plugin that lost its compositor connection shows up.

### Config

- `config path`: the daemon's config file, or the XDG default
  (`~/.config/detent/config.toml`) when no daemon is running.
- `config show`: the file as it is. `--json` prints it parsed (TOML converted to JSON).
- `config edit`: opens the file in `$VISUAL`, then `$EDITOR`, then `vi`. If the file
  doesn't exist yet, it's created from the example config first. The file is checked
  after the editor closes. The daemon picks up the change by itself.
- `config check [FILE]` (alias `validate`): parses and compiles a file without applying it.
  Exits 78 if the file is invalid.
- `config reload`: reload now (normally automatic on save). Exits 78 if the file was
  rejected; the previous config stays active.

### Devices and settings

- `devices` / `devices list`: Logitech hidraw nodes from sysfs, plus the daemon's device id
  for each node it has attached. On macOS / Windows: each device's HID++ collection as
  hidapi lists it (the `node` is the hidapi path).
- `devices info`: everything about one device: identity, firmware, battery, settings
  (desired vs reported), Easy-Switch host, diverted controls. `--json` prints the full
  `DeviceState`.
- `devices battery`: battery level and charging state.
- `settings list`: every setting this device supports, with its value, range and any
  pending change. The settable paths are `dpi`, `smartshift.mode`,
  `smartshift.auto_threshold`, `smartshift.torque`, `haptics.enabled`,
  `haptics.intensity`, `wheel.invert` and `thumbwheel.invert`. Only settings whose HID++
  features are verified on hardware appear here. Thumb-panel click force (0x19C0) is
  stored on the mouse, so it will be added with the layered settings work
  (docs/architecture/settings-layers.md).
- `settings get <path>`: one value. This is either a setting or any state path, such as
  `battery.percent` or `identity.name`.
- `settings set <path> <value> [--persist]`: a live change. It lasts until the next config
  reload. With `--persist` it's also written into `[device."<name>"]` in the config.
  Booleans accept `on/off`, `true/false`, `yes/no` and `1/0`. Without the daemon, the value
  is applied directly to the mouse, and `--persist` is refused.
- `settings unset <path>`: drops a live change so the config value applies again (needs the
  daemon).
- `haptic [WAVEFORM]` plays a waveform (default `damp-state-change`). `haptic --list`
  lists the waveform names.

### Bindings, actions, plugins, events

- `bindings`: every binding in the loaded config: profile, layer, trigger and action.
  `--profile firefox` filters by profile. `--file F` reads a file instead of asking the
  daemon.
- `actions` lists the built-in and plugin actions. `actions invoke kde.overview
  '{"param": 1}'` runs one.
- `plugins` lists plugins (state, pid, restarts, dropped messages, capabilities).
  `plugins status <name>` shows one plugin. `plugins restart <name>` restarts one.
- `events [TOPIC...]` streams live events until Ctrl+C, or until `-n N` events have
  arrived. Topics: `action`, `exec`, `link`, `config`, `profile`, `context`, `plugin`,
  `remap`, `setting`.

### Recovery and debugging

- `reset`: undoes temporary button diversion on the mouse. Use it if buttons stop working
  after a crash, or another tool left them diverted. The systemd unit runs `reset --quiet`
  after the daemon stops.
- `debug features` lists every HID++ feature the device advertises. `debug controls` lists
  every reprogrammable control (CID) and its reporting. Both talk to the device directly
  and only read. (The old spellings `dump-features` and `dump-controls` still work.)

### Shell integration and build info

- `completions <shell>` prints a completion script, for example
  `detentctl completions zsh > ~/.local/share/zsh/site-functions/_detentctl`.
  `just completions` writes bash, zsh and fish scripts to `target/completions/`.
- Man pages: `just man` writes `detentctl.1` plus one page per subcommand to
  `target/man/` (the hidden `detentctl man --out-dir DIR` command). View one with
  `man -l target/man/detentctl.1`.
- `version` prints the detentctl version, commit, target and profile, plus the running
  daemon's version and pid.
- `update check`: compares this build with the latest GitHub Release (the same check as
  the tray, docs/user/tray.md). Exits 0 whether or not a newer one exists and prints where
  to get it; never downloads. When a newer one exists, human output adds one line:
  `Detent is free and built by one person — sponsor: https://github.com/sponsors/Rfluid`
  (not with `-q` or `--json`). The request is anonymous and needs no account; only when
  GitHub's anonymous rate limit answers does it retry with a token (`gh auth token`, or
  `token_file` under `[update]` in `~/.config/detent/tray.toml`). No release published
  yet exits 69. A rejected token exits 77; a network error, or the rate limit with no
  token, exits 74.
- `sponsor`: prints the GitHub Sponsors (`https://github.com/sponsors/Rfluid`) and Pix
  (`https://livepix.gg/rfluid`) links and points to SPONSOR.md for Bitcoin, Lightning and
  other options. `--open` also opens GitHub Sponsors in the default browser (`xdg-open`
  on Linux, `open` on macOS, `start` on Windows); exits 1 if that fails. Works on every OS
  without the daemon. The tray's one-time sponsor notification is described in
  docs/user/tray.md.

Decision: completions and man pages are generated at runtime from the same clap tree
(`clap_complete`, `clap_mangen`), as ripgrep, uv and starship do. There's no `build.rs`
generation step, so they can't drift from the binary. Packaging can call the same commands.

## Legacy spellings

These are hidden but still work: `get [path]` (→ `settings get` / `devices info`),
`set` (→ `settings set`), `check` (→ `config check`), `reload` (→ `config reload`),
`dump-features` / `dump-controls` (→ `debug ...`), `invoke` (→ `actions invoke`), `watch`
(→ `events`).

## JSON output

`--json` output is a stable contract. Fields are only ever added, never renamed or
removed. The types are in `crates/ipc/src/types.rs` and `settings.rs`, and
`crates/config/src/listing.rs`.

| Command | Shape |
|---|---|
| `status` | `{running: true, version, pid, uptime_s, config_path, config_error, bindings, profiles[], active_profile, context{}, remap_enabled, devices[{id,name,node,frames,dropped}], plugins[...]}`, or `{running: false, socket, service}` |
| `enable`/`disable`/`toggle` | `{enabled, changed}` |
| `version` | `{detentctl: {version, commit, target, profile}, detentd: {version, pid} \| null}` |
| `devices list` | `[{node, bus, vendor, product, name, attached: id \| null}]` |
| `devices info` | the daemon's `DeviceState` |
| `devices battery` | `{device, percent, level, charging, raw}` |
| `settings list` | `[{path, type: int\|bool\|choice, min?, max?, step?, values?, value, desired, pending, doc}]` |
| `settings get` | the bare JSON value |
| `settings set` | `{path, value, persisted, via: daemon\|direct, reported}` |
| `bindings` | `[{profile, layer?, trigger, kind, action, params?}]` |
| `plugins list` | `[{name, state, pid, restarts, last_error, dropped, caps, version, uptime_s}]` |
| `actions list` | `[{name, doc, continuous, plugin}]` |
| `config path` | `{path, exists, source: daemon\|default}` |
| `config check` | `{path, ok, error?, bindings, profiles, plugins}` |
| `daemon status` | `{unit, active_state, sub_state, unit_file_state, main_pid}` |
| `update check` | `{current, latest, update_available, supported, url, message}` (`latest`, `update_available`, `url` are `null` when the check failed; `supported` is always `true` now that no token is required) |
| `sponsor` | `{github_sponsors, pix, more, opened}` (`more`: the SPONSOR.md URL; `opened`: whether `--open` launched the browser) |
| `events` | one event object per line: `{topic, ...}` |

## Exit codes

These are sysexits(3) values, so scripts can tell "not running" apart from "bad value".

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | generic failure (systemctl failed, an action was refused) |
| 2 | bad command line (clap) |
| 65 | invalid value (`settings set dpi fast`, bad JSON params) |
| 66 | not found (no such device, config file, path or plugin) |
| 69 | daemon not running or not supported on this OS yet, or the feature isn't available (`update check` with no release published) |
| 74 | I/O error talking to the daemon or the device |
| 76 | protocol mismatch (for example, the daemon is older than detentctl: restart it) |
| 77 | permission denied (install the udev rules) |
| 78 | invalid config file |

Errors go to stderr as `error: ...`, sometimes followed by a `hint: ...` line with the next
step (for example, how to start the daemon).
