---
title: "ADR 0009: The tray icon is an IPC client in bins/, not a plugin"
status: accepted
version: 1.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["bins/detent-tray/", "packaging/systemd/detent-tray.service", "crates/update/"]
owner: "@rfluid"
tags: [adr, tray, packaging]
---

# ADR 0009: The tray icon is an IPC client in bins/, not a plugin

**Context.** Detent needs a panel icon that shows whether it is holding the mouse, and lets
the owner pause remapping, reload the config, start or restart the daemon, open the config
and logs, and learn about updates (Nexus page "Detent: system tray icon proposal", approved
2026-11-24). ADR 0004 says first-party extensions live in `plugins/` and use the bus.

**Decision.** The tray is `bins/detent-tray`, a separate process on its own systemd user
unit (`detent-tray.service`, `WantedBy=graphical-session.target`, deliberately *not*
`PartOf`/`BindsTo` `detent.service`). It talks to the daemon only through the public client
IPC socket (`ctl.sock`, `detent_ipc::Client`), exactly like `detentctl`, and starts or
restarts the daemon with `systemctl --user` on `detent.service`.

Why not a plugin under `plugins/`:
- The daemon starts, supervises and stops bus plugins. A plugin tray would disappear
  exactly when it matters: it could never show "stopped" or offer **Start daemon**.
- The bus carries input events and actions; pause, reload and status are client-IPC
  methods. A plugin would need bus protocol additions that duplicate `ctl.sock`.

ADR 0004's rule still holds in spirit: the tray uses only a public interface (the IPC
spec, docs/specs/ipc.md) and gets no private shortcut into the daemon.

**Crates.** `ksni` 0.3 (0.3.7 in Cargo.lock; StatusNotifierItem + dbusmenu on zbus 5, features `blocking` +
`async-io`) so the tree keeps a single zbus with the async-io executor already used by
`detent-kde`; not `tray-icon`, which needs GTK and libappindicator on Linux.
Notifications use `notify-rust` (already in the tree via GPUI, same zbus). The update check
lives in `crates/update` (`detent-update`, ureq + rustls, semver) and is shared with
`detentctl update check`.

**Follows Rfluid/aura's proven tray setup** (same owner, same panel; adapted, not copied):
- ksni pinned as aura does (`0.3`, `blocking` + `async-io`), spawned with
  `assume_sni_available(true)`; `watcher_offline` logs and returns `true`,
  `watcher_online` logs, so the icon survives the login race where the tray starts before
  Plasma claims `org.kde.StatusNotifierWatcher` (and plasmashell restarts).
- Single instance: `flock(LOCK_EX|LOCK_NB)` on `$XDG_RUNTIME_DIR/detent-tray.lock`
  (fallback `$TMPDIR/detent-tray-<uid>.lock`), fd leaked on purpose; a second copy exits 0.
- Unit modelled on `aura.service`: `Type=simple`, `After=`/`PartOf=`/`WantedBy=`
  `graphical-session.target`, `Restart=on-failure`, `RestartSec=5s` (so "Quit tray", a
  clean exit, is respected), `PassEnvironment=` for the session variables.
- State pushes are diffed against the last applied state (aura's `apply_pending_status`),
  so identical polls and events cost no D-Bus traffic.
- Opening things runs `xdg-open` on a detached thread; a file that won't open falls back to
  `org.freedesktop.FileManager1.ShowItems`, then to opening its directory.
- Update check after aura's update-button plan: background, against `CARGO_PKG_VERSION`,
  notify once per version (a newer one re-notifies), `[update] check = false` turns it off,
  and the menu shows the installed version (`Check for updates · vX.Y.Z`).
- Sponsor nudge ported from aura's sponsor card (`detent_update::sponsor`): one
  notification a week after the first run, never again once answered or dismissed;
  `first_run` / `sponsor_nudge_done` are state (`$XDG_STATE_HOME/detent/state.json`), the
  opt-out `[sponsor] nudge = false` is config. A notification instead of aura's in-window
  card because the tray has no window; it runs on its own thread, waits ten minutes after
  start and asks at most once per process, so it can neither nag nor block the menu.
- Named symbolic icons (hosts prefer `IconName` over `IconPixmap`) plus `IconThemePath`
  pointing at the installed icon dir, so they resolve before any icon cache refreshes. No
  `IconPixmap` fallback: it would need an SVG rasteriser for a case not seen on Plasma.
- The installer prints aura's hint for the `^` overflow (Configure System Tray → Entries
  → Detent → Always shown).

**Consequences.** One more binary and unit to install (scripts/install.sh handles both,
plus the six icons in the hicolor theme). The tray keeps working when the daemon crashes,
and "Quit tray" only removes the icon. The tray can't do anything a script couldn't do with
`detentctl`, which keeps the IPC surface honest.
