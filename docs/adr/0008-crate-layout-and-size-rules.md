---
title: "ADR 0008: crate layout, dependency direction and size rules"
status: current
version: 1.0.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["Cargo.toml", "clippy.toml", "crates/", "bins/", "plugins/", "scripts/check-file-size.sh", "scripts/file-size-allowlist.txt"]
owner: "@rfluid"
tags: [adr, architecture, conventions]
---

# ADR 0008: crate layout, dependency direction and size rules

## Context

The owner asked for better crate organisation and for size rules strong enough to force it.
We had a file-size ratchet (`.rs` ≤ 600, `.md` ≤ 400 lines) with six allowlisted offenders
(`detentd` `bus.rs` 890, `engine.rs` 734, radial `state.rs` 1349 / `view.rs` 962 /
`menu.rs` 774, `docs/research/hidpp-layouts.md` 694) and three cargo manifest warnings on
Rust 1.100 (unused workspace deps `input-linux` and `tokio`, unused `workspace.package.authors`).

## Audit and decisions

### Crates: keep the current set

| Crate | Runtime deps (internal / external) | Verdict |
|---|---|---|
| `detent-hidpp-proto` | none | keep; `no_std`-style, data plane |
| `detent-hidpp-transport` | proto / rustix | keep; the only crate that does hidraw I/O |
| `detent-hidpp-features` | proto / serde, thiserror | keep; typed feature modules, no I/O |
| `detent-device` | proto, transport, features / serde | keep; small but a real boundary (requester + reconciler) |
| `detent-engine` | **none** (was proto: only its zero-alloc test used it, now a dev-dependency) | keep; data plane |
| `detent-config` | engine, features / evdev, serde, toml | keep; compile time only, compiles to engine tables |
| `detent-bus`, `detent-ipc` | serde (+ postcard) | keep separate: different peers (plugins vs clients), transports and versioning |
| `detent-plugin` (`crates/plugin-sdk`) | bus / rustix, thiserror | keep |

No crate is mis-scoped enough to justify a move: each has one reason to change and the
graph is acyclic and points from policy (config, daemon) to mechanism (engine, proto).
Rule from now on: **`detent-hidpp-proto` and `detent-engine` have no runtime
dependencies.** A need for one is a design discussion, not a manifest edit.

### `detentd` stays a binary crate (for now)

`detentd` is ~3.8k lines in about 20 modules. Moving it to a library crate would make it
"testable", but module unit tests already run under `cargo test` in a binary crate, and
nothing outside the daemon may use its internals (`Shared`, workers, bus): the tray, the
CLI and plugins talk to it over `detent-ipc` / `detent-bus` only. A library would turn
those internals into an accidental API and buy nothing yet.

Revisit when the first daemon-level integration test lands (MockTransport → data plane →
uinput sink) or a second binary genuinely needs daemon code. Then add `src/lib.rs` to the
same package with a deliberately narrow surface (`run(Options)` plus test hooks), not a new
crate.

### Naming

`crates/<dir>` → `detent-<dir>`; `plugins/<dir>` → package `detent-plugin-<dir>`, binary
`detent-<dir>`; `bins/<name>` → `<name>`. The one exception is `crates/plugin-sdk` →
`detent-plugin`. We keep it: `detent_plugin` is the name third-party plugins depend on, and
the directory name says "SDK" next to `plugins/` (where the plugins themselves live).
Renaming either side is churn for no reader.

### Modules: split by responsibility, `foo.rs` + `foo/`

Every allowlisted file was split along what it does, not into halves:

- `crates/engine/src/engine.rs` → state, lookup, diverted buttons, leases, timers, grab
  policy; `engine/controls.rs` (press/release, action output), `engine/evdev.rs` (physical
  mouse), `engine/swipe.rs` (raw XY classification). Same code, same `#[inline]` hints,
  same public API (`engine::{SwipeDir, swipe_direction}` re-exported).
- `bins/detentd/src/bus.rs` → state, accept loop, handshake; `bus/handle.rs` (plugin
  messages, capabilities, leases), `bus/supervisor.rs` (processes, pings, shutdown),
  `bus/sink.rs` (`ActionSink`, the daemon-facing side).
- `plugins/radial/src/{state,view,menu}.rs` → `state/{gauge,open,tests}.rs`,
  `view/{render,card,paint,window,tests}.rs`, `menu/{parse,theme,tests}.rs`.
- `docs/research/hidpp-layouts.md` → an index plus `-protocol`, `-device`, `-input` parts.

The allowlist is now empty; the ratchet mechanism stays for any future emergency, but
the policy is: split, don't list.

### Size rules

- Files: unchanged caps (600 / 400), now with no exceptions.
- **Functions: ≤ 100 lines** via `clippy::too_many_lines` (`clippy.toml`
  `too-many-lines-threshold = 100`, `[workspace.lints.clippy] too_many_lines = "warn"`,
  which `just lint`'s `-D warnings` makes an error). 100 is clippy's default and the common
  choice. Nine pre-existing functions (101–207 lines: radial `render`/`card`/`handle`/`apply`,
  detentd IPC `handle`, bus `handle`, data-plane `run`, KDE `dbus::run`, detentctl
  `summary`) carry `#[expect(clippy::too_many_lines, reason = ...)]`. `expect` rather than
  `allow` makes this a ratchet too: once a function is split under the limit the
  expectation goes unfulfilled and the build fails until the attribute is removed.

### Manifest hygiene

Removed the unused workspace deps `input-linux` (uinput goes through `evdev`) and `tokio`
(ADR 0007). Members inherit `authors.workspace = true` like the other package fields.

## Deliberately left alone

- **A per-directory file-count cap.** No standard tool enforces it, our largest source
  directory (`bins/detentd/src`) has 14 files, each one concern, and a count says little
  about cohesion. Future work if a
  directory starts collecting unrelated files.
- **Cognitive-complexity lints** (`clippy::cognitive_complexity`): noisy, and clippy itself
  keeps it in `nursery`. The function-length cap catches the same smell more predictably.
- **Splitting the nine long functions now:** this change is a pure refactor and must not
  alter behaviour; several of them (`dataplane::run`, the radial `render`) are hot or
  visual paths that deserve their own reviewed change.

## Consequences

- New code is held to 600 / 400 / 100 lines from day one, including new binaries
  (e.g. a tray) once they join the workspace.
- `cargo clippy` on Rust 1.100 prints no manifest warnings.
- The engine and proto crates' empty `[dependencies]` are part of the hot-path contract
  (`.agent/skills/hot-path-change.md`).

## Update (2026-11-25, ADR 0010)

Per-target dependencies: `detent-hidpp-transport` depends on rustix on Linux and on hidapi
on macOS / Windows; `detent-config` no longer depends on `evdev` (its key names are a
Detent-owned table, checked against `evdev` by a Linux-only test). Linux-only binaries keep
their code in `app.rs` and only module declarations plus two `main`s in `main.rs`.
