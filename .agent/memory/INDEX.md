---
title: Memory index
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: []
owner: "@rfluid"
tags: [memory, index]
---

# Memory index

One line per entry. Agents append after each task that produced new learning.

## facts/

- [2026-11-23-mx-master-4-link](facts/2026-11-23-mx-master-4-link.md): author's MX Master 4 is Bluetooth `046D:B042`; hidraw node number changes between connections (seen 9 → 10); device index `0xFF`
- [2026-11-23-gpui-no-layer-shell](facts/2026-11-23-gpui-no-layer-shell.md): crates.io GPUI 0.2.2 (latest as of 2026-11-23) has no Wayland layer-shell support
- [2026-11-24-github-private-repo-limits](facts/2026-11-24-github-private-repo-limits.md): **superseded 2026-11-25** (repo public from v0.1.0; settings to turn on are in docs/dev/ci-and-release.md). Was: a private repo on a free plan has no branch protection, rulesets, secret scanning or CodeQL. Still true: `gh repo view` hangs without a TTY
- [2026-11-23-gpui-libc-pin](facts/2026-11-23-gpui-libc-pin.md): GPUI 0.2.2 → `xattr` 0.2.3 fails to build with `libc` 0.2.191 (`ENOATTR`); lockfile seeded from aura's to keep a working resolution
- [2026-11-23-mx4-features-controls](facts/2026-11-23-mx4-features-controls.md): verified MX4 feature indexes (haptics 0x19B0@11), CIDs (0x1A0 = Haptic Sense Panel = `thumb`), defaults; nothing persistently divertable
- [2026-11-23-conflicting-tools](facts/2026-11-23-conflicting-tools.md): Solaar + JuhRadial were diverting the mouse; disabled with owner approval; `detentctl reset` clears leftover diversion
- [2026-11-23-kernel-coexistence](facts/2026-11-23-kernel-coexistence.md): never request on 0x1D4B; kernel resets 0x2121 on connect; swid map → Detent 0xE
- [2026-11-23-kde-shortcut-names](facts/2026-11-23-kde-shortcut-names.md): verified kglobalaccel names (kwin, kmix volume, powerdevil); use `busctl --user`
- [2026-11-23-gpui-layer-shell-upstream](facts/2026-11-23-gpui-layer-shell-upstream.md): upstream GPUI has layer-shell (PR #35610); re-vendor instead of patching; KeyboardInteractivity::None on KWin
- [2026-11-23-gpui-overlay-on-kwin](facts/2026-11-23-gpui-overlay-on-kwin.md): measured: first GPUI window ~0.7 s, later ~150 ms; pointer `enter` position is best-effort; KWin has no cursor D-Bus API
- [2026-11-23-uinput-phys-einval](facts/2026-11-23-uinput-phys-einval.md): evdev 0.13 `with_phys` → EINVAL on kernel 7.3; identify Detent's virtual devices by name/ID
- [2026-11-23-gpui-layer-shell-size](facts/2026-11-23-gpui-layer-shell-size.md): GPUI layer surfaces default to 1536×1095 even when anchored to all edges; pass 0×0 bounds to fill the output
- [2026-11-24-kwin-cursor-and-overlay-focus](facts/2026-11-24-kwin-cursor-and-overlay-focus.md): KWin script `workspace.cursorPos`/`screenAt` gives the pointer; exclusive-keyboard overlay is activated and focus returns on close; GPUI display UUID = v5(output name)
- [2026-11-24-mx4-force-button-range](facts/2026-11-24-mx4-force-button-range.md): owner's thumb panel force (0x19C0): one sensor, changeable, min 4384 / default 5480 / max 7288, owner's value 5342; fn3 writes survive the power switch (stored setting)
- [2026-11-24-volume-terminal-theme](facts/2026-11-24-volume-terminal-theme.md): `wpctl` output format, Plasma OSD `volumeChanged(ii)`, no terminal key in kdeglobals → konsole, portal color-scheme 1 under Breeze Dark, GPUI scroll signs; thumbwheel direction unverified
- [2026-11-24-pipewire-outputs](facts/2026-11-24-pipewire-outputs.md): owner's sinks (HDMI 2/3 unplugged = route `available: "no"`), `node.nick` short names, cubic volumes, `target.object -1` moves pinned streams to the default
- [2026-11-25-gpui-dead-wayland-connection](facts/2026-11-25-gpui-dead-wayland-connection.md): 0×0 layer surface on a fractional-scale output → GPUI `wp_viewport.set_destination(0,0)` → KWin drops the client; GPUI keeps running and every window fails ("Adapter … not compatible with the display surface"); open at the output's logical size from `kde.cursor`
- [2026-11-25-gpui-layer-input-region](facts/2026-11-25-gpui-layer-input-region.md): GPUI 0.3.14 `Window::set_input_region(Some(&[]))` makes a mapped layer surface click-through; keyboard interactivity is fixed at creation, so `keys` actions must remove the overlay at once
- [2026-11-24-plasma-tray-icons](facts/2026-11-24-plasma-tray-icons.md): SNI symbolic icons recolour via `ColorScheme-Text`; `IconThemePath` works uninstalled; QtSvg 6.11 renders `<mask>`; no `--` in XML comments; dbusmenu `_` mnemonics; owner's panels auto-hide
- [2026-11-25-mx4-bt-report-descriptor](facts/2026-11-25-mx4-bt-report-descriptor.md): MX4 over BT: HID++ only on usage page 0xFF43 / usage 0x0202, report 0x11 (no 0x10 short reports); also a standard HID haptics collection (page 0x0E, reports 0x3C/0x3D)
- [2026-11-25-cross-check-c-build-scripts](facts/2026-11-25-cross-check-c-build-scripts.md): `cargo check --target {aarch64-apple-darwin,x86_64-pc-windows-msvc}` from Linux works if the C build scripts (`ring`, hidapi's macOS backend) are overridden with `--config target.<t>.<links>...`; never override hidapi on Windows
- [2026-11-25-notify-rust-close-reason](facts/2026-11-25-notify-rust-close-reason.md): notify-rust 4.17's `wait_for_action` reports every close as `"__closed"`; for the reason (2 = dismissed by the user) use `zbus::block_on(handle.wait_for_action_async(..))`
- [2026-11-25-easy-switch-prior-art](facts/2026-11-25-easy-switch-prior-art.md): 0x1814 setCurrentHost has no reply, never persist it, wrap `prev`; 0x1815 layout (Solaar's) verified on the MX4: names ≤ 24 bytes, any slot renamable, shorter name replaces fully; a real switch to another slot not yet run

- [2026-11-25-bolt-receiver](facts/2026-11-25-bolt-receiver.md): Bolt C548 on Linux 7.3 = hid-generic (no dj), HID++ on interface 2, MX4 at slot 2 (key B042:A204BE6C), pairing/name registers, 0x1D4B on power-on, transient EPIPE + spurious first error (retry)

## lessons/

- [2026-11-23-hidpp-error-layout](lessons/2026-11-23-hidpp-error-layout.md): in HID++ error reports the original feature index is byte 3 (header position), not the first param
- [2026-11-23-research-before-swid](lessons/2026-11-23-research-before-swid.md): the scaffold's swid 0xD collided with G HUB; check the prior-art swid map before picking protocol constants
- [2026-11-23-gpui-headless-fallback](lessons/2026-11-23-gpui-headless-fallback.md): without gpui_platform's `wayland` feature GPUI runs headless silently (one render, nothing shown); use QuitMode::Explicit
- [2026-11-23-zbus-pascalcase](lessons/2026-11-23-zbus-pascalcase.md): zbus `#[interface]` exports methods in PascalCase; KWin `callDBus` must use `Activated`, not `activated`
- [2026-11-23-pkill-self](lessons/2026-11-23-pkill-self.md): `pkill -f <pattern>` inside a `sh -c` test script matches the script itself; use `pkill -x <comm>`
- [2026-11-24-verify-does-not-rebuild-bins](lessons/2026-11-24-verify-does-not-rebuild-bins.md): `just verify` builds test binaries only; `cargo build` before visual checks
- [2026-11-24-bench-under-load](lessons/2026-11-24-bench-under-load.md): name `--bench` targets for criterion flags; under machine load compare before/after interleaved, not against an old baseline
- [2026-11-24-zsh-no-word-split](lessons/2026-11-24-zsh-no-word-split.md): the shared terminal is zsh (unquoted `$1` doesn't split); preview screenshots: `--at` + `--backdrop`, check crops for private content
- [2026-11-24-nested-session-leftovers](lessons/2026-11-24-nested-session-leftovers.md): children of a nested kwin session script survive it; kill the recorded pids, never pkill plasmashell/kded6

## patterns/

- [2026-11-24-remap-pause-engine-view](patterns/2026-11-24-remap-pause-engine-view.md): pause remapping via `Shared::active` (data-plane view without bindings); swapping `Shared::config` would stop plugins
- [2026-11-23-nested-kwin-screenshots](patterns/2026-11-23-nested-kwin-screenshots.md): screenshot overlays in `dbus-run-session kwin_wayland --virtual` + spectacle, isolated from the owner's session
