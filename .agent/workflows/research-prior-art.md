---
title: Research prior art
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: ["docs/research/"]
owner: "@rfluid"
tags: [workflow, research]
---

# Research prior art

Goal: learn how existing open-source tools solved what Detent needs, so we reuse known answers and settle ⚠ items without guessing or bothering the owner.

## Sources to study (minimum)

| Project | License | Look for |
|---|---|---|
| **Solaar** (pwr-Solaar/Solaar) | GPL-2.0 | the most complete HID++ 2.0 implementation: `lib/logitech_receiver/hidpp20.py` (function layouts), `hidpp20_constants.py` (feature IDs), `special_keys.py` (CIDs, flags), `settings_templates.py` (DPI, SmartShift, thumbwheel, haptics if present), diversion rules, reconnect handling |
| **logiops** (PixlOne/logiops) | GPL-3.0 | C++ daemon: config model for gestures/diversion, raw XY gestures, re-applying settings on reconnect, uinput virtual device |
| **Linux kernel** `drivers/hid/hid-logitech-hidpp.c` | GPL-2.0 | the kernel's software ID, which features it manages (hi-res scroll, battery), MX quirks, Bluetooth device handling |
| **libratbag** | MIT | HID++ 2.0 driver structure, device database approach |
| **input-remapper**, **keyd**, **kanata** | GPL / MIT / LGPL | evdev grab + uinput patterns, latency, layers, tap-hold resolution |
| Rust crates on crates.io (search `hidpp`, `logitech`) | various | existing Rust HID++ work to reuse or learn from |
| Zed (zed-industries/zed) issues/PRs, GPUI forks | Apache-2.0 / various | Wayland layer-shell support status for GPUI |
| KWin scripting docs, kglobalaccel, kdotool | various | active-window tracking on Wayland, shortcut names, `callDBus` patterns |
| MX Master 4 issues in Solaar/logiops trackers | | haptics feature ID and functions, CIDs, firmware quirks |

## Rules

- **License hygiene.** Detent is MIT. Learn protocol facts and techniques from GPL projects (facts aren't copyrightable), but **don't copy GPL code**; reimplement from your own understanding. MIT/Apache code can be reused with attribution.
- Cite every finding with a link (file + line or issue URL).
- Mark each finding **adopt**, **adapt** or **reject**, with one line on why.
- Where sources disagree or don't cover the MX Master 4, say so and plan a device verification instead of guessing.

## Output

`docs/research/prior-art.md` (with doc frontmatter), organized by topic:
1. HID++ transport, discovery, swid coexistence with the kernel
2. Feature function layouts used by Detent (table per feature)
3. MX Master 4 specifics (CIDs, divertability, haptics, thumb/haptic panel)
4. Reconnect / sleep / host-switch handling
5. Diversion, raw XY gestures, tap/hold resolution
6. evdev grab + uinput patterns and pitfalls
7. KDE integration (shortcut names, active-window context, OSD)
8. GPUI layer-shell
9. Decisions this research settles (feed back into specs and ADRs)

Then update `docs/specs/hidpp.md`, `docs/plugins/kde.md`, `docs/plugins/radial.md` and `.agent/memory/facts/` with what was settled.
