---
title: GPUI 0.2.2 lacks layer-shell
date: 2026-11-23
source_task: scaffold
tags: [fact, gpui, wayland]
---

`cargo info gpui` on 2026-11-23 reports 0.2.2 as the latest crates.io version. Grepping its source (aura's vendored copy) for `layer_shell`/`zwlr_layer` finds nothing. The radial overlay therefore needs a patch in `vendor/gpui` (or a newer upstream Zed GPUI that has it; check before patching). See ADR 0005.
