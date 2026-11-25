---
title: Pause remapping with a second ArcSwap, not by swapping the real config
date: 2026-11-24
source_task: detentctl CLI rework (enable/disable for the tray)
tags: [daemon, config, data-plane, pattern]
---

`detentctl disable` / `remap.set {enabled:false}` must leave plugins, device settings and
profiles running. Storing an empty config in `Shared::config` would have stopped every
plugin, because the bus supervisor's `reconcile` starts and stops plugins from that config.

What worked: `Shared::active` is a separate `ArcSwap<CompiledConfig>` that only the data
planes (and `Worker::apply_diversion`) read. While paused it holds the same config with
`engine: Compiled::default()`. `Shared::install_config` keeps both in step on reload, so a
reload while paused stays paused. Workers get `Cmd::Rebind` (ReleaseAll + undivert), not
`Cmd::Reload`, so live `settings set` edits survive a pause. The engine's `grab_change`
releases the evdev grab by itself once `grab_mods` is empty. The data plane is unchanged:
still one `ArcSwap::load` per iteration.
