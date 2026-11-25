---
title: Detent design
status: current
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: [".design/assets/logo-mark.svg", "packaging/icons/"]
owner: "@rfluid"
tags: [design, brand]
---

# `.design/`

Everything visual about Detent: the logo, how to use it, and which design system we build on. Read this before you draw an icon, pick a colour, add a font, or write UI (the GPUI radial menu, docs pages, a tray icon, a README banner).

| File | What it holds |
|---|---|
| [`logo.md`](logo.md) | The logo: idea, geometry, colours, clear space, minimum sizes, do and don't |
| [`design-system.md`](design-system.md) | We use the Nexus brand system (`@nexus/brand`): tokens, type, tiles, and what to map where |
| [`assets/`](assets/) | The SVG files. These are the source of truth. Do not redraw them |

## The decision in one paragraph

The logo is the **ratchet**: an eight-tooth sawtooth wheel with a hub hole. Seven teeth are ink, and the eighth is ice blue, the tooth that is currently engaged. A detent is the notch a scroll wheel clicks into, and the ratchet is that mechanism. The wordmark is lowercase `detent` in DM Sans.

## Quick rules for agents

1. **Use the files in `assets/`.** Pick by surface: `logo-mark-dark.svg` on dark, `logo-mark-light.svg` on light, `logo-mark.svg` when ink should follow `currentColor`, `logo-mark-symbolic.svg` for a tray or panel icon, `app-icon.svg` for the application icon.
2. **Use Nexus tokens, not new colours.** Map every colour to a `--nx-*` token (see `design-system.md`). A raw hex needs a reason written next to it.
3. **The only accent is ice blue** (`--nx-hue-ice`, `#68CCF5`; on light surfaces `--nx-status-info`, `#0A84FF`). Never recolour the engaged tooth to another hue.
4. **Do not change the geometry.** Teeth, hub and proportions are fixed in `logo.md`. Scaling is fine, and nothing else is.
5. **Fonts:** DM Sans for text, Geist Mono for labels and code. No other families.
6. **If you change anything here,** update the doc that describes it and bump `last_verified`, per `.agent/context/conventions.md`.

## Where the exploration lives

The options, the rejected shaded variants (faceted, curved and lifted) and the in-use mockups are on a Nexus page, `pag_4DIihP3x76xy` ("Detent: logo options", version 4). It is a record of how we got here. The files in this folder are what ships.

## Not decided yet

- The tray icons (`packaging/icons/hicolor/scalable/status/detent-tray-*-symbolic.svg`) are generated from `logo-mark-symbolic.svg` by `bins/detent-tray/src/icons.rs` (teeth flattened, Plasma's `ColorScheme-Text` class, lower-right cut-out badges). Checked in a Plasma 6.8 panel at 22 px on Breeze light and dark (recolouring and mask work); 16 px not checked yet. See docs/user/tray.md.
- The wordmark is live text. Outline it before any use where DM Sans may be missing (README banner, release assets).
- No PNG exports yet. `app-icon.svg` is 512 px and scales down. Add a `just` target if raster files are needed.
