---
title: Development setup
status: draft
version: 0.2.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["justfile", "packaging/", "rust-toolchain.toml", "scripts/install.sh", "scripts/ci-apt-deps.sh"]
owner: "@rfluid"
tags: [dev, setup]
---

# Development setup

## Toolchain
- Rust stable (`rust-toolchain.toml`), edition 2024. `just` for tasks.
- Core build deps (Fedora): `sudo dnf install systemd-devel` (libudev, for hotplug).
- GPUI build deps (Fedora): `sudo dnf install wayland-devel libxkbcommon-devel libxkbcommon-x11-devel fontconfig-devel vulkan-loader-devel alsa-lib-devel openssl-devel` (same set `aura` needs).
  Debian/Ubuntu equivalents: `scripts/ci-apt-deps.sh` (what CI installs).
- Optional, used by `just pre-pr` (skipped with a hint when missing): `actionlint`, `shellcheck`, `gitleaks`, `cargo-audit`.

## Device access without root

```sh
just install                # build + ~/.local/bin + systemd user unit + udev rule (asks before sudo)
just install --no-build     # same, reusing target/release
```

`packaging/udev/70-detent.rules` grants the logged-in user (`uaccess`) access to:
- Logitech hidraw nodes (USB `046d` and Bluetooth `0005:046D:*`)
- `/dev/uinput`
- the mouse's evdev nodes (needed for grab-while-armed chords)

Check: `just ctl devices` lists the mouse, and `ls -l /dev/hidraw*` shows a `+` (ACL) on its node.

## Conflicting tools
Stop Solaar / logiops while developing (`systemctl --user stop solaar`, `sudo systemctl stop logid`). `detentd` warns if they run.

## Running
```sh
just run-daemon          # foreground, RUST_LOG=detent=debug
just ctl devices
```

## Lockfile note
`Cargo.lock` was seeded from `aura`'s lockfile so GPUI 0.2.2 resolves to a known-good set (newer `libc` breaks `xattr` 0.2.3). Avoid blanket `cargo update`; update specific crates and run `just check`.
