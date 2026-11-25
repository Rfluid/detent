---
title: Add a native plugin
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["plugins/", "crates/config/src/native.rs", "crates/plugin-sdk/", "Cargo.toml"]
owner: "@rfluid"
tags: [skill, plugins]
---

# Add a native plugin

Native plugins are first-party plugins in `plugins/` (contract: `plugins/README.md`, ADR 0004).

1. **Spec first:** `docs/plugins/<name>.md` with frontmatter: user stories, actions (`<name>.*`), settings, capabilities, failure behaviour.
2. **Crate:** `plugins/<name>/` with package `detent-plugin-<name>`, `[[bin]] name = "detent-<name>"`, depending on `detent-plugin.workspace = true`. Add it to `members` in the root `Cargo.toml`. It must build on macOS and Windows too (ADR 0010): put its deps under `[target.'cfg(target_os = "linux")'.dependencies]` (plus `detent-ipc` for other OSes), its code in `app.rs`, and in `main.rs` only `#[cfg(target_os = "linux")]` module declarations and two `main`s (copy `plugins/kde/src/main.rs`).
3. **Register:** add a variant to `NativePlugin` in `crates/config/src/native.rs` (`name`, `binary`, `default_caps`).
4. **Implement** `detent_plugin::Plugin`: `manifest()` declares actions and caps; `on_config` validates settings (log and ignore bad keys, don't crash); `on_action`/`on_event` return fast and push slow work to a thread.
5. **Only public bus messages.** Need something new? Follow `.agent/workflows/change-protocol.md` first.
6. **Example config:** add `[plugins.<name>]` and a binding to `config/detent.example.toml` (the test keeps it valid).
7. **Tests:** pure logic unit tests; SDK mock-daemon tests for the message flow.
8. **Docs:** `plugins/README.md` table row, `README.md` native plugin list.
9. `just verify` (and `just check` if it pulls GPUI).
