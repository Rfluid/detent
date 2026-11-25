---
title: evdev 0.13 VirtualDeviceBuilder::with_phys fails with EINVAL
date: 2026-11-23
source_task: phase-2
tags: [fact, uinput, evdev]
---

On the owner's kernel 7.3 (Fedora 44), `VirtualDevice::builder()?.with_phys(c"detent/kbd")` makes
`build()` fail with `EINVAL`; the same builder without `with_phys` works. Detent's virtual devices
are therefore identified by name ("Detent virtual keyboard/pointer") and ID `BUS_VIRTUAL
1209:DE71/DE72`. The research appendix's udev line matching `ATTRS{phys}=="detent/*"` was not adopted.
