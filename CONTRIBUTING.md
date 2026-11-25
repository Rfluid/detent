# Contributing to Detent

Thanks for helping. This page covers picking work, building, checking and submitting changes.
By contributing you agree to license your work under the project's MIT license, and to follow
the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go through
[SECURITY.md](SECURITY.md), not public issues.

The design docs live in [`docs/`](docs/README.md). Agent-facing context (conventions,
skills, the memory of device quirks) lives in [`.agent/`](.agent/README.md) and applies to
humans too: start with [`.agent/context/conventions.md`](.agent/context/conventions.md).

## Picking something to work on

Detent is fully supported on one setup (Linux, KDE Plasma 6, MX Master 4 over Bluetooth),
because that's the setup its author owns. The rest is written up as issues:

- [`help wanted`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22):
  platform ports, devices, packaging. Filter by your platform with the `platform:*` labels
  (`platform:linux-gnome`, `platform:linux-wlroots`, `platform:linux-x11`, `platform:macos`,
  `platform:windows`) or by area (`area:daemon`, `area:plugin`, `area:transport`,
  `area:packaging`).
- [`good first issue`](https://github.com/Rfluid/detent/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22):
  small and well scoped.

Comment on an issue before you start, so two people don't port the same thing. Each port
issue says what it depends on; start with the ones whose dependencies are done.

## The one-shot prompt workflow

Every port issue ends with a **one-shot prompt**: a self-contained brief for an AI coding
agent (Claude Code, Codex, or similar). To use it:

1. Fork and clone the repo fresh, on the machine that has the OS and the mouse.
2. Start the agent at the repo root and paste the prompt. It names the issue number.
3. The agent reads [`AGENTS.md`](AGENTS.md) and what it lists, reads the issue and its
   comments, researches prior art (`.agent/workflows/research-prior-art.md`), does the work,
   runs `just verify`, and opens a PR whose body says `Closes #<issue>`.
4. **You** run the manual test from the issue on your real hardware and paste the result into
   the PR. The agent can't press the buttons; you can.

The prompts follow the rules below, so a PR made by hand is held to exactly the same bar.

## Hardware and OS facts must be verified

Detent never guesses about devices or desktops. Every HID++ feature ID, control ID (CID),
byte layout, OS API behaviour, D-Bus name, compositor IPC command or permission prompt that
code depends on must be **checked on a real system** before the code relies on it:

- Record what you verified, how, and on what (OS/desktop version, device, connection) in a
  new file under `.agent/memory/facts/` (`YYYY-MM-DD-slug.md`, frontmatter
  `title, date, source_task, tags`) and add a line to `.agent/memory/INDEX.md`.
- Put the fact in the relevant spec too (`docs/specs/hidpp.md`, `docs/specs/devices/*.md`,
  `docs/plugins/<plugin>.md`, ...). Items marked ⚠ / "to verify" stay marked until checked.
- Research prior art first (Solaar, logiops, the kernel driver, libratbag, input-remapper,
  keyd, kanata, and per-platform tools named in each issue). **Detent is MIT: learn facts
  from GPL code, never copy it.**
- If you can't verify something, say so in the PR instead of shipping a guess.

Device safety: read-only probes before writes, no writes to unknown HID++ features, and
stopping Detent must leave the mouse exactly as it was (`.agent/skills/verify-on-device.md`).

## Getting started

- Rust stable (pinned by `rust-toolchain.toml`) and [`just`](https://github.com/casey/just).
- Linux system packages: `systemd-devel` (libudev) for everything; the GPUI plugin also needs
  the Wayland/X11/Vulkan/font/audio headers. Fedora and Ubuntu lists:
  [`docs/dev/setup.md`](docs/dev/setup.md) and [`scripts/ci-apt-deps.sh`](scripts/ci-apt-deps.sh).
- Device access without root comes from the udev rule: `just install` installs it (asking
  before it runs `sudo`) together with the binaries and the user service.
- macOS and Windows: no system packages. `cargo check --workspace --all-targets` and
  `cargo test --workspace` (what CI's portable job runs) build `detentctl` for real over
  hidapi and every Linux-only member as a stub that exits early (ADR 0010). Some `just`
  recipes (`lint`, so `verify`) call `bash`; on Windows run them from Git Bash or use the cargo commands directly.

```sh
git clone https://github.com/Rfluid/detent.git && cd detent
just check-core      # fast: everything except the GPUI radial plugin
just check           # everything (first GPUI build takes a few minutes)
just run-daemon      # detentd in the foreground with debug logs
just ctl devices     # detentctl against it
```

Stop Solaar, logiops or other mouse tools while developing; they fight over the same
buttons.

## Testing without the author's setup

You don't need an MX Master 4 on KDE to work on most of Detent:

- **Protocol, features, engine, config:** pure unit tests. `MockTransport`
  (`crates/hidpp-transport/src/mock.rs`) scripts request → response pairs taken from real
  captures; the engine has a zero-allocation test and benches. See
  [`docs/dev/testing.md`](docs/dev/testing.md).
- **Real device captures:** `docs/specs/devices/captures/` holds the MX Master 4's feature and
  control dumps. Other devices: capture yours (`detentctl debug features --json` and
  `detentctl debug controls --json`; both only read) and scrub serials before committing.
- **Plugins:** the SDK's mock daemon feeds messages and checks what the plugin sends; pure
  logic (radial geometry, menu parsing, state machines) is unit-tested. Run a plugin against
  `just run-daemon` to try it live.
- **Radial menu visuals:** `detent-radial --preview` runs the real ring without the daemon;
  a nested `kwin_wayland --virtual` session keeps experiments off your desktop
  (`.agent/memory/patterns/2026-11-23-nested-kwin-screenshots.md`).
- **Another desktop or OS:** that is exactly what's missing, so your machine *is* the test
  bench. Verify there and record what you found (above).

What you can't do without the hardware is claim a device-facing change works: that needs the
manual check on a real mouse.

## Before you push

| Command | What it runs |
|---|---|
| `just verify` | `cargo fmt --check`, the file-size ratchet, clippy `-D warnings`, tests. Must pass. |
| `just pre-pr` | Everything CI runs: `cargo fmt` (auto-fix), ratchet, clippy, tests, actionlint, shellcheck, gitleaks, cargo-audit (missing tools are skipped with a hint). `--release` adds a release build. |
| `just bench` | Hot-path benchmarks; needed when you touch `crates/engine`, `crates/hidpp-proto` or the device thread. |

Rules that CI enforces or reviewers check:

- **Size**: 600 lines per `.rs`, 400 per `.md`, 100 per function (`clippy::too_many_lines`).
  Split by responsibility instead of growing (ADR 0008). `scripts/file-size-allowlist.txt`
  is empty; don't add to it.
- **Hot path**: no allocation, lock or blocking call per event (`.agent/skills/hot-path-change.md`).
- **Plugins use the public bus only**: if a native plugin needs something new, extend the
  protocol for everyone (`crates/bus` + `docs/specs/plugin-bus.md`,
  `.agent/workflows/change-protocol.md`).
- **No `unwrap()`/`expect()`** outside tests.
- **Docs move with code**: if you touch a file listed in a doc's `source_refs`, update the doc
  and bump its `last_verified`. A port also updates the README's platform table.
- **Hardware facts are verified** on a real device before code depends on them (above).

## Commits and pull requests

- [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`, types
  `feat, fix, refactor, perf, docs, test, chore`, scopes are crate/plugin names or `ci`,
  `scripts`, `packaging`, `release`. One logical change per commit.
- Branch names: `<type>/<slug>`, e.g. `feat/gnome-plugin`.
- The PR body links the issue (`Closes #123`), says how it was tested, and for device- or
  OS-facing work pastes the manual test you ran on real hardware. Fill in the PR template's
  checklist.
- PRs are squash- or rebase-merged (no merge commits) and the branch is deleted after merge.
- CI must be green: see [`docs/dev/ci-and-release.md`](docs/dev/ci-and-release.md).

## Reporting bugs and requesting features

Use the issue forms. For bugs, include `detentctl --version`, the device and connection,
OS/kernel, desktop/compositor and `journalctl --user -u detent` logs (on macOS and Windows,
the failing `detentctl -vv …` output instead). If your buttons stop
working, `detentctl reset` gives them back to the mouse. To propose a port that has no issue
yet, use the "Platform or desktop port" form.

## Releases

Maintainers: [`docs/dev/release.md`](docs/dev/release.md).
