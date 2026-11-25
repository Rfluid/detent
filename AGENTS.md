---
title: Agent entrypoint
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["Cargo.toml", ".agent/", "docs/"]
owner: "@rfluid"
tags: [agents, entrypoint]
---

# AGENTS.md

You are working on **Detent**, a Rust userspace daemon that drives Logitech HID++ mice
(starting with the MX Master 4): it sends, receives and answers device events, exposes every
device setting for viewing and editing, and hosts plugins (native ones in this repo: KDE
integration and a radial menu).

This file is the entrypoint. Everything agent-specific lives in [`.agent/`](.agent/README.md).

## Read before you change anything

1. [`.agent/context/stack.md`](.agent/context/stack.md): crates, threads, data flow in one page
2. [`.agent/context/conventions.md`](.agent/context/conventions.md): code, docs, commits, memory
3. [`docs/architecture/overview.md`](docs/architecture/overview.md): the design and why
4. [`docs/product/roadmap.md`](docs/product/roadmap.md): which phase we're in, what "done" means
5. [`.agent/memory/INDEX.md`](.agent/memory/INDEX.md): facts learned the hard way (device quirks!)
6. [`.design/README.md`](.design/README.md): the logo and design system (read before drawing icons, picking colours or fonts, or writing UI)

Then pick the matching skill in [`.agent/skills/`](.agent/skills/) or workflow in
[`.agent/workflows/`](.agent/workflows/).

## Non-negotiables

- **Performance first.** The data plane (`crates/engine`, `crates/hidpp-proto`, the device
  thread) must stay allocation-free, lock-free and non-blocking per event. Read
  [`.agent/skills/hot-path-change.md`](.agent/skills/hot-path-change.md) before touching it.
  Benchmarks gate regressions.
- **Never block input on a plugin.** Plugin delivery is a non-blocking send; slow plugins lose
  messages, not the mouse. Leases always expire.
- **Native plugins use the public plugin bus only.** If one needs something new, extend the
  protocol (`crates/bus` + `docs/specs/plugin-bus.md`) for everyone.
- **Hardware facts must be verified.** HID++ feature IDs, CIDs and KDE D-Bus names marked
  "to verify" must be checked on the real device/desktop before code depends on them; record
  results in `.agent/memory/facts/` and the relevant spec.
- **Docs move with code.** Touching a file listed in a doc's `source_refs` means updating that
  doc and its `last_verified`.
- **No root.** Device access comes from udev rules (`packaging/udev/`).
- **Done means usable.** The acceptance criterion is the owner controlling their own mouse with Detent (docs/product/roadmap.md, "Daily-driver acceptance"). Passing tests isn't enough.
- **Research before inventing.** Check prior art (Solaar, logiops, the kernel driver, libratbag, input-remapper/keyd/kanata, existing Rust crates) before designing device-facing code. See `.agent/workflows/research-prior-art.md`. Don't copy GPL code; Detent is MIT.

## Decision policy

Decide yourself, and record the decision in the relevant spec or ADR, whenever the answer is an industry standard, a common practice in the prior-art tools, or follows from these docs. Examples: XDG paths, udev `uaccess` rules, systemd user units, uinput device setup, evdev grab semantics, retry/timeout/backoff values, crate choices among well-known options, error handling, CLI flag naming, test layout, log levels, file formats already chosen.

Ask the owner **only** when:
- the choice changes what the owner experiences and has no standard answer (default bindings, menu look, gesture feel);
- it's irreversible or risky for their device or system (writes to unknown HID++ features, anything needing sudo);
- the docs conflict with each other.

When you ask, batch the questions, give a recommendation for each, and keep working on everything that doesn't depend on the answer.

## Commands

```sh
just check        # cargo check, whole workspace (includes GPUI; slow first time)
just check-core   # everything except the GPUI plugin
just test
just lint         # fmt --check + clippy -D warnings
just verify       # lint + test: must pass before you say "done"
just ctl devices  # list Logitech hidraw nodes
```

## Machine facts (author's setup)

MX Master 4 over Bluetooth, `046D:B042`, hidraw node changes between connections (seen 9, 10; find it with `just ctl devices`), Fedora + KDE Plasma 6.8 (Wayland),
kernel `hid_logitech_hidpp` loaded. HID++ device index for Bluetooth = `0xFF`.

## Layout

```
crates/      hidpp-proto · hidpp-transport · hidpp-features · device · engine · config · bus · ipc · plugin-sdk
bins/        detentd (daemon) · detentctl (CLI)
plugins/     kde · radial          (native plugins, see plugins/README.md)
vendor/gpui  patched GPUI (see vendor/README.md)
config/      example config (parsed by a test)
packaging/   udev rules, systemd user unit
docs/        product, architecture, specs, plugins, ADRs, dev guides
.agent/      context, skills, workflows, prompts, memory
.design/     logo (SVG assets), design system usage (Nexus brand tokens)
```
