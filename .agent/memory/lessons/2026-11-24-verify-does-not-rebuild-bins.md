---
title: "`just verify` doesn't rebuild the binaries you run"
date: 2026-11-24
source_task: radial default icons + tooltips
tags: [lesson, build, testing]
---

`just verify` (`cargo test` + clippy) builds test binaries only. `target/debug/detent-radial`
(used by `--preview` screenshots) stayed stale, and a screenshot showed none of the change.
Run `cargo build -p <crate>` before any manual/visual check of a binary.
