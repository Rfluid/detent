---
title: Conventions
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["Cargo.toml", "clippy.toml", "justfile", "scripts/check-file-size.sh", "scripts/file-size-allowlist.txt"]
owner: "@rfluid"
tags: [context]
---

# Conventions

## Code
- Rust stable, edition 2024, workspace lints (see root `Cargo.toml`). `rustfmt` default config.
- **No `unwrap()`/`expect()` in non-test code**; use `?` or explicit handling. `#[allow(...)]` needs a comment saying why.
- No `todo!()`/`unimplemented!()` in code paths a user can reach. Return a typed error and leave `// TODO(phase-N): ...`.
- Library crates use `thiserror`; binaries may use `anyhow`.
- Hot-path crates (`hidpp-proto`, `engine`, the device thread) follow `.agent/skills/hot-path-change.md`.
- Public protocol types (`crates/bus`, `crates/ipc`) change only with their spec doc in the same commit.
- New dependencies: add them to `[workspace.dependencies]` and reference them with `.workspace = true`. Prefer small crates; justify heavy ones in the PR/commit body.

## File size
Hard caps: **600 lines per `.rs` file, 400 per `.md` doc, 100 lines per function** (`vendor/`, `target/`, `Cargo.lock` excluded). Enforced by `scripts/check-file-size.sh` in `just lint` and CI.
- Approaching the cap? Split by responsibility (a submodule per concern, tests in `tests.rs` or a `tests/` dir, a doc per topic) before adding more.
- `scripts/file-size-allowlist.txt` is a ratchet for files that were already over the cap: each entry's number is that file's cap and it may only go down. Never add a file to it or raise a number; split the file. When you shrink an allowlisted file, lower its number (`scripts/check-file-size.sh --update`) or delete the entry once it's under the default cap (the check fails on stale entries). It is empty since 2026-11-24; keep it that way.
- **Functions: ≤ 100 lines** (`clippy::too_many_lines`, threshold in `clippy.toml`; a hard error in `just lint`). Split long functions into named steps or per-variant handlers. The few functions that predate the rule carry `#[expect(clippy::too_many_lines, reason = "...")]`; `expect` (not `allow`) fails the build once the function is short enough, so remove it then. Never add a new one.
- **Module layout:** `foo.rs` + `foo/bar.rs` (no `mod.rs`). The parent file owns the shared types/state and starts with a doc comment whose "Layout" line says what each submodule holds; submodules add `impl` blocks and use `pub(super)` for what siblings need. Large test suites go in `foo/tests.rs`.
- **Crates and dependencies** (ADR 0008): `crates/<dir>` is package `detent-<dir>` (exception: `crates/plugin-sdk` → `detent-plugin`); `plugins/<dir>` is `detent-plugin-<dir>` with binary `detent-<dir>`. `detent-hidpp-proto` and `detent-engine` have no runtime dependencies; anything they need for tests goes in `[dev-dependencies]`. `[workspace.dependencies]` lists only crates some member uses (cargo warns otherwise); new members inherit `version`, `edition`, `rust-version`, `license` and `authors` from the workspace.

## CLI surface
Every user-facing feature is reachable from `detentctl`. Read commands support `--json`. Design the CLI and IPC method together (docs/specs/ipc.md).

## Docs
Every Markdown doc has YAML frontmatter: `title, status (draft|current|deprecated), version, last_updated, last_verified, source_refs, owner, tags`. If you change code listed in a doc's `source_refs`, update the doc and bump `last_verified`. Hardware/desktop facts marked ⚠ must be verified before use.

## Memory
After each non-trivial task, add one file per learning under `.agent/memory/{facts,lessons,patterns}/` named `YYYY-MM-DD-slug.md` with frontmatter (`title, date, source_task, tags`), and add a line to `.agent/memory/INDEX.md`. Facts = verified truths about hardware/libraries; lessons = mistakes to avoid; patterns = approaches that worked.

## Commits
`type(scope): summary`. Types: `feat, fix, refactor, perf, docs, test, chore`. Scopes: crate or plugin name (`proto`, `engine`, `config`, `bus`, `kde`, `radial`, `ctl`, `daemon`, `vendor`; repo tooling: `ci`, `scripts`, `packaging`, `release`). One logical change per commit; `just verify` green before committing.

## Definition of done
1. `just verify` passes (and `just check` if GPUI-related).
2. Tests cover the change (bug fixes include a failing-before test).
3. Docs and `source_refs` docs updated; roadmap checkbox/phase status updated if relevant.
4. Memory updated if something was learned.
5. For device-facing work: verified on the real MX Master 4 (`.agent/skills/verify-on-device.md`).
