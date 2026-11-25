---
title: Cross-checking macOS / Windows from Linux needs build-script overrides
date: 2026-11-25
source_task: portability seams (ADR 0010)
tags: [fact, ci, portability, cross-compile]
---

`cargo check --target aarch64-apple-darwin` (or `x86_64-pc-windows-msvc`) on the owner's
Fedora machine fails in C build scripts, not in Rust:

- `ring` 0.17 (via `ureq` → rustls in `detent-update`): gcc rejects `-arch`; with
  `CC=clang` it pulls glibc headers. Needs a real SDK / MSVC.
- `hidapi` 2.6 on macOS compiles `etc/hidapi/mac/hid.c` → needs `IOKit/hid/IOHIDManager.h`.
  On Windows the `windows-native` feature is pure Rust: no C at all.

`cargo check` doesn't link, so overriding those build scripts (both declare `links`) is
enough to type-check every Rust line:

```sh
t=aarch64-apple-darwin
cargo clippy --workspace --all-targets --target $t \
  --config "target.$t.ring_core_0_17_14_.rustc-link-lib=[]" \
  --config "target.$t.hidapi.rustc-cfg=[\"hidapi\"]" -- -D warnings
t=x86_64-pc-windows-msvc
cargo clippy --workspace --all-targets --target $t \
  --config "target.$t.ring_core_0_17_14_.rustc-link-lib=[]" -- -D warnings
```

Never pass the hidapi override for Windows: it sets `cfg(hidapi)`, which selects the
C-library code path instead of `windows-native` and fails with missing trait items.
The `links` name of ring changes with its version (`ring_core_<version with _>`).
Building or testing still needs the real OS: that's the macOS / Windows CI jobs.
