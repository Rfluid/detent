---
title: "Native plugin: kde"
status: current
version: 0.5.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["plugins/kde/"]
owner: "@rfluid"
tags: [plugins, native, kde]
---

# Native plugin: `kde` (`detent-kde`)

Interact with KDE Plasma through mouse motions: gestures and wheels drive KWin and Plasma, and
the plugin tells Detent which app is focused so profiles switch automatically.
Verified on Plasma/KWin 6.8.3 (Wayland, Fedora). Linux only: on macOS and Windows the binary
builds but exits at once with a "not yet supported" message (ADR 0010).

## User stories

- The gesture button behaves like the touchpad's 4-finger swipes (the default config): press →
  4 fingers up (Overview, again → Grid); hold + down → 4 fingers down; hold + up → maximize;
  hold + left → the next desktop, hold + right → the previous desktop; diagonals do nothing.
- Gesture + thumbwheel → volume, with the Plasma OSD showing the level.
- A profile (e.g. `app = "firefox"`) can turn the swipes into back/forward.

## Actions (`kde.*`)

All discrete actions call `invokeShortcut(name)` on `org.kde.kglobalaccel /component/<c>`
(`org.kde.kglobalaccel.Component`). Using kglobalaccel instead of synthesized keystrokes keeps
working when the user rebinds shortcuts and doesn't depend on the keyboard layout.

| Action | Component / shortcut |
|---|---|
| `overview` | `kwin` / `Overview` |
| `overview.up`, `overview.down` | `kwin` / `Cycle Overview`, `Cycle Overview Opposite`: the same states as KWin's 4-finger touchpad swipe up (desktop → Overview → Grid) and down (desktop → Grid, Grid → Overview, Overview → closed). Source: KWin `Plasma/6.8` `src/plugins/overview/overvieweffect.cpp` (`cycle()` / `reverseCycle()`, touchpad gestures Up 4 / Down 4) |
| `grid` | `kwin` / `Grid View` |
| `windows`, `windows.all` | `kwin` / `Expose`, `ExposeAll` |
| `desktop.next`, `desktop.prev` | `kwin` / `Switch to Next Desktop`, `Switch to Previous Desktop` |
| `desktop.left`, `desktop.right` | `kwin` / `Switch One Desktop to the Left/Right` |
| `window.maximize`, `window.minimize`, `window.close` | `kwin` / `Window Maximize`, `Window Minimize`, `Window Close` |
| `tile.left`, `tile.right` | `kwin` / `Window Quick Tile Left/Right` |
| `volume.up`, `volume.down`, `volume.mute` | `kmix` / `increase_volume`, `decrease_volume`, `mute` (plasma-pa kept the old component name). `volume.mute` with `{ reply }` toggles through PipeWire instead and replies (see "Volume") |
| `volume.get`, `volume.set` | PipeWire through `wpctl`, with replies: see "Volume" |
| `volume.targets`, `volume.target` | list the output devices / switch to one and move what is playing: see "Outputs" |
| `terminal` | starts the default terminal (see "Terminal") |
| `volume` (continuous) | `kmix` up/down per detent: OSD + Plasma's volume step (default 5%) |
| `brightness` (continuous) | `org_kde_powerdevil` / `Increase/Decrease Screen Brightness` |
| `zoom` (continuous) | `kwin` / `view_zoom_in`, `view_zoom_out` |
| `shortcut` | params `{ component, name }`: any global shortcut |
| `cursor` | params `{ reply = "plugin.action" }`: asks KWin for the pointer position and invokes `reply` with `{ x, y, output, width, height }` (logical px relative to that output; `width`/`height` = the output's logical size). Used by `radial` to open around the cursor. |

Continuous actions accept `invert = true` (e.g. `thumbwheel = { action = "kde.volume", invert = true }`).
Bound to a discrete trigger (e.g. `thumb.press`), a continuous action steps once "up".

List the names on your machine (Fedora has no `qdbus`; use `busctl`):
```sh
busctl --user call org.kde.kglobalaccel /component/kwin org.kde.kglobalaccel.Component shortcutNames
```

## Context publishing

- On start the plugin owns `dev.detent.Kde` on the session bus with object `/Context`
  (methods `Activated(ss)`, `Desktop(s)`; zbus exports them in PascalCase).
- It writes a KWin script to `$XDG_RUNTIME_DIR/detent/kde-context.js` and loads it:
  `org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript(path, "detent-context")` → id →
  `/Scripting/Script<id>` `org.kde.kwin.Script.run()`. The script listens to
  `workspace.windowActivated` and `workspace.currentDesktopChanged` and calls back with `callDBus`.
- The plugin forwards `SetContext app=<resourceClass, lowercased>`, `title`, `desktop`.
  Activations of the radial menu's own overlay (`detent-radial`, which takes the keyboard
  while open) are skipped, so opening the ring never switches profiles.
- `detentctl status` shows the current context: use it to find the `app` value for a profile
  (e.g. Konsole is `org.kde.konsole`, Firefox is `firefox`).
- The script is unloaded on shutdown.

## Cursor position (`kde.cursor`)

KWin has no D-Bus method for the pointer position, but its scripts do. Per request the plugin
writes a one-shot script to `$XDG_RUNTIME_DIR/detent/kde-cursor.js`, loads it as
`detent-cursor` (replacing the previous one) and runs it:

```js
var p = workspace.cursorPos, o = workspace.screenAt(p), g = o.geometry;
callDBus("dev.detent.Kde", "/Context", "dev.detent.Kde", "Cursor",
         String(p.x - g.x), String(p.y - g.y), String(o.name),
         String(g.width), String(g.height), "<reply>");
```

`Cursor(ssssss)` then invokes `<reply>` on the bus (`width`/`height` are left out when not
positive). The radial overlay opens at that size: 0×0 kills its Wayland connection on a
fractional-scale output (docs/plugins/radial.md, "Failure behaviour"). Values travel as strings because `callDBus`
may marshal JS numbers as int or double. `reply` must look like `plugin.action` (lowercase,
digits, `._-`) since it is embedded in the script. Verified on KWin 6.8 in a nested
`kwin_wayland --virtual` (2026-11-24): `Cursor("450", "320", "Virtual-0", "radial.cursor")`.

## Volume (`kde.volume.get` / `.set` / `.mute` with a reply)

For callers that show the level themselves (the radial menu's volume gauge). The default
output is read and set with WirePlumber's `wpctl` on `@DEFAULT_AUDIO_SINK@`, on the plugin's
`kde-audio` thread (each call spawns a short process, ~5 ms):

| Action | Params | Does |
|---|---|---|
| `volume.get` | `{ reply, seq?, max? }` | refreshes the device's short name (`wpctl inspect`: `node.nick`, else `node.description`), reads the level |
| `volume.set` | `{ percent, max? = 100 (≤ 150), unmute?, osd?, reply?, seq? }` | `wpctl set-volume -l <max> … <percent/100>`, then `set-mute 0` if `unmute`; Plasma's OSD (`org.kde.plasmashell /org/kde/osdService volumeChanged(percent, max)`) when `osd` (default: the `osd` setting) |
| `volume.mute` | `{ reply, seq?, max? }` | `wpctl set-mute … toggle` |

The reply invokes `reply` (a `plugin.action` name) with
`{ source: "kde.volume", seq, percent, muted, max, label }`, or `{ source, seq, error }` when
`wpctl` fails. `seq` is echoed so the caller can ignore answers to superseded requests.
Bursts are merged: of the jobs queued while one runs, every `get`/`mute` is kept but only the
last `set` (the latest target wins), so a spinning wheel never queues seconds of changes.

Verified on Fedora 44 / PipeWire (2026-11-24): `wpctl get-volume` prints `Volume: 0.80`
(` [MUTED]` appended when muted); `inspect` lists `* node.description = "…"` and
`* node.nick = "…"`. The nick is the short name ("HDMI 1", "Speaker", "AB13X USB Audio");
built-in outputs' descriptions repeat the card's long name, so they look alike once cut.

## Outputs (`kde.volume.targets` / `kde.volume.target`)

For the radial menu's output fan. Same thread and reply rules as "Volume".

| Action | Params | Does |
|---|---|---|
| `volume.targets` | `{ reply, seq? }` | `pw-dump` → invokes `reply` with `{ source: "kde.volume", seq, targets: [{ id, name, description, icon, percent, muted, current }] }`, or `{ source, seq, error }` |
| `volume.target` | `{ id, max?, reply?, seq? }` | `wpctl set-default <id>`, waits (≤ 300 ms) until `@DEFAULT_AUDIO_SINK@` is it, moves what is playing, replies like `volume.get` for the new output |

- **Which outputs:** every `Audio/Sink` node, by id, except those whose port is unplugged:
  the device's active route for the sink (`card.profile.device`) says `available: "no"`,
  or, without an active route, every route that could serve it does (what Plasma hides).
  `id` is the PipeWire global id as a string; `current` = the `default.audio.sink` metadata.
- **Name:** `node.nick`, else `node.description`, else `node.name`. **Description:** the
  raw `node.description` (else the name), shown by the radial menu when hovering an output.
- **Icon:** `device.form-factor` when set (headphone, headset, speaker, tv), else from the
  node name: `video-display` (HDMI / DisplayPort), `audio-headphones` (headphones, or
  Bluetooth), `audio-speakers` (speaker), else `audio-card`.
- **Level:** the cube root of the average `channelVolumes` (PipeWire stores them cubic:
  `wpctl` shows 0.71 for 0.357921), and `mute`.
- **Moving what is playing:** every playback stream (`Stream/Output/Audio`, not the inner
  half of a loopback / filter chain, `node.link-group`, not passive or monitor streams) gets
  `pw-metadata -- <id> target.object -1` and `target.node -1`: "follow the default". Streams
  that already followed it move by themselves; this also moves streams pinned to an output,
  like Plasma's "play all audio via this device".

Verified on the owner's laptop (Fedora 44, PipeWire 1.6, WirePlumber 0.5, 2026-11-24):
HDMI 2 / 3 without a monitor are sinks whose routes say `available: "no"`; Brave's stream
followed `wpctl set-default`; a `pw-cat --target` stream stayed until its `target.object`
was set to -1, then moved (deleting the key sent it back). Needs `pw-dump` and
`pw-metadata` (package `pipewire-utils`). Not checked yet: Bluetooth outputs (A2DP vs
headset profiles), and whether Plasma shows its own OSD when the default output changes.

## Terminal (`kde.terminal`)

Starts the terminal chosen in System Settings → Default Applications: `kdeglobals`
`[General] TerminalService` (a desktop-file id, launched with `gtk-launch`), else
`TerminalApplication` (a command, `/bin/sh -c`), else `konsole`. Plasma writes neither key
while the default is kept (checked on the owner's Plasma 6.8, 2026-11-24). It starts in `$HOME`
with stdio closed and is reaped on a thread.

## Continuous input

Rotation deltas are summed and flushed at most every 16 ms (≈ 60 Hz), at most 6 steps per
flush, on the plugin's D-Bus thread, so D-Bus never sees per-frame traffic.

## Settings (`[plugins.kde]`)

| Key | Default | Meaning |
|---|---|---|
| `context` | `true` | load the KWin script and publish `app`/`desktop` |
| `osd` | `true` | show Plasma's volume OSD for `kde.volume.set` (a call's `osd` overrides it) |

(For the kmix-shortcut actions, the OSD and the volume step come from Plasma's own settings.)
Volume with replies needs `wpctl` (package `wireplumber`, installed with Plasma on Fedora).

## Capabilities

`events.subscribe`, `context.publish`, `device.haptics`, `actions.invoke` (defaults in
`crates/config/src/native.rs`).

## Failure behaviour

- No session bus → logs once, actions do nothing, the plugin keeps running (no crash loop).
- KWin not running → logged; window/desktop actions and context unavailable.
- `wpctl` missing or failing → logged; volume replies carry `error` (the radial gauge shows
  "unavailable"); the kmix-shortcut volume actions still work.
- `pw-dump` missing or failing → `volume.targets` replies with `error` (the radial menu
  shows no chevron); switching fails with an error in the level reply.
- Killed or crashed → the supervisor restarts it (backoff 2 s, 4 s, … 30 s); the pointer is
  never affected (verified with `kill -9`).
