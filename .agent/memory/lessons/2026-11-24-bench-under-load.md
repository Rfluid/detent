---
title: "Hot-path benches: compare A/B interleaved, and name the bench targets"
date: 2026-11-24
source_task: crate layout + file splits (ADR 0008)
tags: [lesson, performance, benchmarks]
---

- `cargo bench -p detent-engine -- --save-baseline X` fails ("Unrecognized option") because
  the lib's libtest harness gets the criterion flags too. Name the targets:
  `cargo bench -p detent-engine -p detent-hidpp-proto --bench dispatch --bench parse -- ...`.
- With other agents compiling (load ~20 on 16 cores), a saved baseline compared minutes
  later showed +30–60 % on every bench, including `parse+classify`, whose code hadn't
  changed. Sequential before/after numbers are noise under load. Check the untouched bench
  first; if it moved, measure A/B interleaved: a `git worktree` of the base commit with its
  own `CARGO_TARGET_DIR`, then alternate before/after runs several times and compare medians.
