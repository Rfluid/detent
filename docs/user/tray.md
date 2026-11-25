---
title: Tray icon
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["bins/detent-tray/", "crates/update/", "packaging/systemd/detent-tray.service", "packaging/icons/", "scripts/install.sh", "scripts/uninstall.sh"]
owner: "@rfluid"
tags: [user, tray]
---

# Tray icon

Linux only for now (StatusNotifierItem over D-Bus); on macOS and Windows `detent-tray`
exits at once with a "not yet supported" message (ADR 0010).

`detent-tray` puts the Detent mark in the Plasma panel. It shows at a glance whether Detent
is holding the mouse, and its menu pauses remapping, reloads the config, starts the daemon,
checks for updates and links to sponsoring. It is a separate process on its own user unit, so it keeps working
(and offers **Start daemon**) when `detentd` is down. Why it isn't a plugin: ADR 0009.

`just install` (or a tarball's `./install.sh`) installs it, enables `detent-tray.service`
and starts it when you are in a graphical session. If the icon lands in the **^** overflow
group: right-click the tray → Configure System Tray → Entries → Detent → Visibility
**Always shown**.

## What the icon says

| Icon | State | Tooltip |
|---|---|---|
| Mark with the engaged tooth | Daemon running, mouse connected, remapping on | `MX Master 4, connected, 85%. Remapping on.` |
| Dimmed mark | Daemon running, no mouse | `No mouse connected.` |
| Pause bars, lower right | Remapping paused (the mouse acts like a stock mouse) | `Remapping is off. Middle-click to resume.` |
| Dimmed, cross | Daemon not running | `Open the menu and choose Start daemon.` |
| Exclamation (and Plasma's attention state) | Config rejected, or `detent.service` failed | the first line of the error |
| Down arrow | A newer release exists | `... Update vX.Y.Z available.` |

When several apply, the first in this order wins: error, stopped, paused, no mouse,
update. The icons are symbolic, so Plasma paints them in the panel's text colour on light
and dark themes.

**Left click** opens the menu. **Middle click** pauses or resumes remapping. Pausing isn't
remembered: a daemon restart (or a reboot) always starts with remapping on.

## Menu

| Row | Does |
|---|---|
| Device rows (disabled) | Name, then `Connected, Bluetooth, 85%` per connected device |
| Pause remapping / Resume remapping | `remap.set` over the IPC socket (same as `detentctl disable` / `enable`) |
| Reload config | `config.reload`; a rejected file keeps the previous config and shows a notification |
| Restart daemon / Start daemon | `systemctl --user restart\|start detent.service` |
| Open config… | Opens the config in its default app; falls back to showing it in the file manager |
| View logs… | Konsole (else `$TERMINAL -e`, else `xdg-terminal-exec`) running `journalctl --user -u detent.service -f` |
| Update available: vX.Y.Z… | Only when a newer release exists; opens its release page |
| Check for updates · vX.Y.Z | Checks now and answers with a notification (`vX.Y.Z` is the installed version) |
| About Detent | Notification with version and commit, and a button to open the repository |
| Sponsor Detent… | Opens GitHub Sponsors (`https://github.com/sponsors/Rfluid`) in the browser. Always there |
| Quit tray | Removes the icon only. The daemon and the mouse keep working. It comes back at the next login, or with `systemctl --user start detent-tray` |

## Updates

The tray asks GitHub Releases for the newest Detent release when it starts and then once a
day, and compares it with its own version. A newer one shows the arrow badge, a menu row and
one notification per version. It never downloads or installs anything; update by running
the installer again (`curl -fsSL https://raw.githubusercontent.com/Rfluid/detent/main/install.sh | bash`),
`just update`, or a new tarball's `./install.sh`. `detentctl update check` runs the same
check from a shell.

The check needs no account or token, and a failed check is only logged (a manual one
answers with a notification). GitHub allows 60 anonymous requests an hour per address;
only if that limit is hit does the check retry with a read-only token: `gh auth token`
when the GitHub CLI is logged in, otherwise a file you point to.

The last version you were told about is kept in `~/.local/state/detent/tray-update-notified`.

## Sponsoring

Detent is free and built by one person. The tray asks about sponsoring **once**: a week
after it first ran, it shows one notification, **Enjoying Detent?**, with three buttons:

| Button | Does |
|---|---|
| Sponsor on GitHub | Opens `https://github.com/sponsors/Rfluid` |
| Pix | Opens `https://livepix.gg/rfluid` (a one-off tip in BRL) |
| Don't ask again | Never shows it again |

Closing the notification yourself (its ×) counts as "Don't ask again". Clicking a link
doesn't: if the notification only expires or you follow a link, a later tray start may ask
once more, and answering or closing it ends that for good. It never shows more than once
per tray run, never in the first ten minutes after the tray starts (so not among the login
notifications), and never if `tray.toml` can't be read. **Sponsor Detent…** in the menu and
`detentctl sponsor` are always there; Bitcoin, Lightning and other options are in
[SPONSOR.md](../../SPONSOR.md).

When the tray first ran, and whether you answered, are kept in
`~/.local/state/detent/state.json` (`$XDG_STATE_HOME/detent/state.json`), not in the
config: `{"first_run": <unix seconds>, "sponsor_nudge_done": true|false}`. A missing or
damaged file reads as empty, which starts the week again.

## Settings

`~/.config/detent/tray.toml` (optional; a missing table or key means its default):

```toml
[update]
check = true                                   # false: no background checks or update prompts
token_file = "~/.config/detent/github-token"   # rate-limit fallback when `gh auth token` has nothing

[sponsor]
nudge = true                                   # false: never show "Enjoying Detent?"
```

A misspelt key is an error: an invalid `[update]` falls back to the update defaults, an
invalid `[sponsor]` keeps the notification off. The two tables are read separately, so a
mistake in one doesn't affect the other.

## Troubleshooting

| Symptom | Fix |
|---|---|
| No icon at all | `systemctl --user status detent-tray`; `journalctl --user -u detent-tray`. A second copy exits silently (one tray per user) |
| Icon is a blank square | The icons are missing from `~/.local/share/icons/hicolor/scalable/status/`; run the installer again |
| It says stopped but the daemon runs | The tray reads `$DETENT_SOCKET` / `$XDG_RUNTIME_DIR/detent/ctl.sock`, like detentctl; check `detentctl status` |
| Pause does nothing | The daemon predates `remap.set`; restart it on the new build (`detentctl daemon restart`) |

For development: `cargo run -p detent-tray` with `DETENT_TRAY_ICON_PATH=packaging/icons`
uses the icons from the checkout. `detent-tray --preview <state>` shows a fixed state
(`running`, `nomouse`, `paused`, `stopped`, `error`, `update`) without a daemon. Icon
sources: `packaging/icons/` is generated by `detent-tray --write-icons` from the logo
geometry (bins/detent-tray/src/icons.rs); a test fails if the files are stale.
