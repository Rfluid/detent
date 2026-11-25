---
title: Add or extend an HID++ feature
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["crates/hidpp-features/", "crates/hidpp-proto/src/feature_id.rs", "crates/device/src/state.rs", "docs/specs/hidpp.md"]
owner: "@rfluid"
tags: [skill, hidpp]
---

# Add or extend an HID++ feature

## 1. Verify before coding
- Confirm the feature ID and version on the real device: `just ctl dump-features --json`.
- Find the function semantics (Solaar's source and public HID++ notes are good references). Write down request/response layouts in `docs/specs/hidpp.md`.
- Capture real request/response bytes (`detentctl raw` / capture) for tests. **Never guess bytes.**

## 2. Code
1. `crates/hidpp-proto/src/feature_id.rs`: add the `FeatureId` const and its `name()`.
2. `crates/hidpp-features/src/<feature>.rs`: a `State` struct (serde, kebab-case enums) and `impl Feature`:
   - `read`: resolve the index via `Requester::feature_index`, send reads, decode. Return `RequestError::Malformed` on short responses (no panics, no indexing without checks).
   - `apply`: only send what differs between `desired` and `current`.
   - `on_notification`: if the feature emits events.
   - `schema()`: every user-editable field with its range or choices.
3. `crates/hidpp-features/src/lib.rs`: `pub mod`.
4. `crates/device/src/state.rs`: add a `Section<feature::State>`; include it in `mark_disconnected` if it's volatile, and in `reconcile::plan`.
5. `crates/config/src/schema.rs` + `docs/specs/config.md` + `config/detent.example.toml`: if users can set it.

## 3. Test
- Unit tests with `MockTransport` using the captured frames (request matches, response decodes).
- Malformed/short response tests.

## 4. Finish
- `docs/specs/hidpp.md`: remove ⚠ for what you verified.
- Memory: a `facts/` entry for anything surprising about the device.
- `just verify`.
