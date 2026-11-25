---
title: GPUI 0.2.2 needs an older libc in the lockfile
date: 2026-11-23
source_task: scaffold
tags: [fact, gpui, build]
---

Fresh resolution picked `libc` 0.2.191, and `xattr` 0.2.3 (via `zed-async-tar` ← `gpui_http_client` ← `gpui`) fails: `cannot find value ENOATTR in crate libc`. `cargo update -p libc --precise 0.2.189` is refused by other constraints. Fix used: seed `Cargo.lock` from `~/development/Rust/aura/Cargo.lock` (known-good GPUI 0.2.2 resolution, libc 0.2.189). Don't run a blanket `cargo update`.
