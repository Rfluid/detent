## What and why

<!-- One paragraph: what changes for the user or the code, and why. Link issues (Fixes #123). -->

## How it was tested

<!-- Commands run, tests added. Device-facing changes: what you did on the real mouse. -->

## Checklist

- [ ] `just verify` passes (fmt, file-size ratchet, clippy -D warnings, tests); `just check` too if the GPUI plugin changed
- [ ] Tests cover the change (bug fixes include a test that failed before)
- [ ] Docs whose `source_refs` include touched files are updated, with `last_verified` bumped
- [ ] Hot path (`crates/engine`, `crates/hidpp-proto`, device thread): no new allocation, lock or blocking call per event; benches checked
- [ ] Device-facing change verified on real hardware (`.agent/skills/verify-on-device.md`), or N/A
- [ ] Public protocol change (`crates/bus`, `crates/ipc`) comes with its spec update, or N/A
- [ ] Commits follow `type(scope): summary`
