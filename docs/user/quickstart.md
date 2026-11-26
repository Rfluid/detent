---
title: Quickstart
status: current
version: 0.3.1
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["justfile", "packaging/", "scripts/install.sh", "scripts/uninstall.sh", "config/detent.example.toml", "bins/detentctl/src/", "bins/detent-tray/"]
owner: "@rfluid"
tags: [user, quickstart]
---

# Quickstart

Detent drives a Logitech MX Master 4 on Linux (KDE Plasma 6, Wayland): gestures, chords, a
radial menu, per-app profiles, and every device setting from one TOML file.
On macOS and Windows only `detentctl`'s device commands work so far (read and set DPI,
SmartShift, wheel and haptics settings): see docs/user/cli.md and ADR 0010.

Needs: a Plasma 6 session (and, to build from source, Rust stable and `just`). Tested on
Fedora 44 + Plasma 6.8.3 with the mouse on Bluetooth. This page is the full-support path.
Other Linux desktops get settings and remapping but not the KDE plugin or the radial menu;
macOS and Windows are a `detentctl`-only preview. See the README's
[platform table](../../README.md#platform-support).

## 1. Install

Either the one-line installer (downloads the latest release):

```sh
curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/install.sh | bash
```

or from source:

```sh
git clone https://github.com/Rfluid/detent.git && cd detent
just install                         # = scripts/install.sh: build, install, enable the service
```

`just install` builds the daemon (`detentd`), the CLI (`detentctl`), the two native plugins
(`detent-kde`, `detent-radial`; the radial menu's GPU toolkit makes the first build take a few
minutes) and the tray icon (`detent-tray`), copies them to `~/.local/bin`, installs the
systemd user units and enables them (`systemctl --user enable --now detent detent-tray`).
The tray icon shows whether Detent is running and lets you pause it: [tray.md](tray.md). The udev rule
`/etc/udev/rules.d/70-detent.rules` is the only step that needs root: the script prints the
exact `sudo` commands and asks before running them (skipped if the rule is already identical).

The one-line installer and a release tarball (extract it, run `./install.sh` inside) do the
same steps without building. Run `just install` again to update; it restarts the running service.

**Stop Solaar, logiops or JuhRadial first** and disable their autostart: they divert the same
buttons and fight Detent. `detentctl status` and the log warn if one is running.

## 2. Check it sees your mouse

```sh
detentctl devices        # the mouse's hidraw node (the number changes between connections)
detentctl status         # daemon, config, profile, device, plugins
detentctl devices info   # battery, DPI, SmartShift, wheels, haptics, Easy-Switch host
```

## 3. Configure

```sh
mkdir -p ~/.config/detent
cp config/detent.example.toml ~/.config/detent/config.toml     # from a source checkout
# no checkout (one-line installer): fetch it instead
curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/config/detent.example.toml \
  -o ~/.config/detent/config.toml
```

Edit and save: changes apply immediately, no restart. A broken file is rejected and the
previous config keeps working: `detentctl status` shows why, and `detentctl config check` explains
any file before you save it.

Change device settings live, optionally saving them into the file:

```sh
detentctl settings set dpi 1600 --persist
detentctl settings set smartshift.mode free-spin
detentctl settings set haptics.intensity 70 --persist
```

## 4. Try it (example config)

| Do | Result |
|---|---|
| Press the **thumb panel** (Haptic Sense Panel) | ring of actions around the cursor, like Logitech's Actions Ring: move the cursor onto a bubble (you feel a tick) and **left click** it, or hold the panel and release it on a bubble. **Esc**, right click or the centre × close it; Tab / Enter work too |
| Tap the **gesture button** (under the thumb, front) | like a 4-finger touchpad swipe up: Overview (tap again → Grid) |
| Hold gesture + move **down** | like a 4-finger touchpad swipe down: Grid (from Grid → Overview, from Overview → closed) |
| Hold gesture + move **up** | maximize / restore the window |
| Hold gesture + move **left / right** | the next / previous desktop |
| Hold gesture + move **diagonally** | nothing (bind `swipe.up_left`, ... to use them; see `docs/specs/config.md`) |
| Hold gesture + **left click** | region screenshot (Spectacle) |
| Hold gesture + **right click** | play / pause |
| Hold gesture + **wheel** | next / previous tab |
| Hold gesture + **thumbwheel** | volume with the Plasma OSD |
| In Firefox: hold gesture + **left click** | close tab (the `firefox` profile) |

Find the `app` name for your own profiles with `detentctl status` (it shows the focused
app, e.g. `google-chrome`, `org.kde.konsole`).

## 5. Troubleshooting

| Symptom | Fix |
|---|---|
| `permission denied` on `/dev/hidraw*`, `/dev/uinput` or `/dev/input/event*` | `just install` and accept the udev step (or run the `sudo` commands it prints), then log out and in (or unplug/reconnect the mouse) |
| Gesture/thumb buttons do nothing after Detent crashed or was killed | `detentctl reset` gives the buttons back (the service also does this automatically when it stops) |
| A chord fires but the click still reaches the app | press the modifier **first**, then the other button |
| Bindings stopped after sleep / reconnect | they come back within ~1 s; if not: `journalctl --user -u detent -n 30` |
| Volume/Overview actions do nothing | `detentctl plugins` should show `kde connected`; `detentctl actions invoke kde.overview` to test |
| Radial menu doesn't appear | `detentctl plugins` should show `radial connected`; it needs Wayland with layer-shell (KWin has it) |
| Ring won't go away | press **Esc** or click outside it; hold **left + right** for 2 s (escape chord) also closes it |
| Ring not centred on the cursor | `detent-kde` must be running (`detentctl plugins`): the ring asks it (KWin) for the cursor position |
| Natural scrolling differs while gesture is held | Detent's virtual pointer uses default libinput settings; set "Detent virtual pointer" in System Settings → Mouse |
| See what Detent does | `detentctl events` (actions, profile, context, config, link, remap events). Pause remapping with `detentctl disable`, resume with `detentctl enable`. Full reference: [cli.md](cli.md) |

## Uninstall

```sh
just uninstall      # = scripts/uninstall.sh: stops the service, removes binaries and unit,
                    # asks before removing the udev rule with sudo; keeps ~/.config/detent
```
