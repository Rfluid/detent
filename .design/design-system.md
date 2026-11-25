---
title: Detent design system
status: current
version: 0.1.0
last_updated: 2026-11-25
last_verified: 2026-11-24
source_refs: [".design/logo.md"]
owner: "@rfluid"
tags: [design, tokens]
---

# Design system

Detent does not have a design system of its own. It uses the Nexus brand system, `@nexus/brand` (id `ds_nexus_brand`, source `brand/v5.0`), so that Detent looks like it belongs next to Nexus tools. Tokens are CSS custom properties prefixed `--nx-`.

## Getting the tokens

In a Nexus session, call the `getDesignSystem` tool with id `ds_nexus_brand`. The reply is large (about 500 variables) and includes ready-to-use CSS with `:root`, `[data-theme="dark"]` and `[data-theme="light"]` blocks. Save it to a file and search it. Do not read it whole.

`checkPage` (Nexus MCP) lists colours, font sizes and spacing that are off the tokens, and text below WCAG AA contrast, for an html or jsx page.

## The tokens we use

Dark is the default. Use these names, not the hex values.

| Job | Token | Dark | Light |
|---|---|---|---|
| Page background | `--nx-page` | `#000000` | `#F5F5F7` |
| Panel | `--nx-panel` | `#0A0A0B` | `#FFFFFF` |
| Raised surface | `--nx-raised` | `#1C1C1E` | `#FFFFFF` |
| Hairline | `--nx-line-2` | `#262628` | `#E8E8EA` |
| Body text | `--nx-text` | `#F5F5F7` | `#111113` |
| Strong text | `--nx-strong` | `#FFFFFF` | `#000000` |
| Secondary text | `--nx-muted` | `#A1A1A6` | `#5C5C60` |
| Logo ink | `--nx-mark-ink` | `#F2F4F6` | `#111312` |
| Accent (engaged state) | `--nx-hue-ice` | `#68CCF5` | use `--nx-status-info` `#0A84FF` |
| Logo tile | `--nx-tile-dark-top` → `--nx-tile-dark-bottom`, `--nx-tile-light-top` → `--nx-tile-light-bottom` | `#26292D` → `#07080A` | `#FFFFFF` → `#E9ECEA` |

Spacing is the `--nx-space-*` scale (4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 88, 136). Radii: `--nx-radius-sm` 10, `-control` 12, `-md` 14, `-card` 16, `-lg` 20, `-pill` 999. Status colours (`--nx-status-running`, `-waiting`, `-failed`, `-info`) are for state, not decoration.

## Type

| Role | Family | Notes |
|---|---|---|
| Text, headings, wordmark | DM Sans | Display and headings weight 300, titles 500, body 400 |
| Labels, code, device IDs | Geist Mono | Labels 12 px, weight 500, tracking 0.08 em, uppercase |

Sizes: body 16/1.55, small 13.5, title 20, h3 30, h2 52, display 62. Use the `--nx-*-size` and `--nx-*-weight` tokens.

## Accent means "engaged"

Ice blue marks the one thing that is currently active. In the logo it is the engaged tooth. In the UI it should mean the same: the selected radial segment, the active binding, the connected device. Don't use it for decoration, and don't use more than one accent colour on a surface.

## Mapping to Detent's UI

- **Radial menu (GPUI, `plugins/radial`):** it follows the system light or dark theme. Take colours from the dark and light columns above. The highlighted segment uses the accent. Keep shapes flat, with a hairline edge in `--nx-line-2`, and avoid heavy shadows.
- **Docs and README:** use `logo-lockup-dark.svg` or `-light.svg`, picked by `prefers-color-scheme` where the host allows it.
- **Tray / panel icon:** `assets/logo-mark-symbolic.svg`. KDE recolours symbolic icons itself.
- **Terminal output (`detentctl`):** no colour required. If you add some, use ice blue only for the active or connected item and keep `NO_COLOR` working.

## Where raw values are allowed

Only where a token cannot reach: SVG files that must stand alone (everything in `assets/`) and drop-shadow flood colours. Each such value should be a copy of a token, with a comment naming it, as the assets do.

## Making a design artifact

Design pages (logo options, UI mockups) are published as Nexus pages through the built-in `nexus` MCP server (`createPage`, `writePageVersion`), not from the CLI. Follow the rules in this file, check the page with `checkPage`, and link the page id from the doc that depends on it. The logo exploration is `pag_4DIihP3x76xy`. The radial menu's motion proposal (every transition moves toward the centre: close, select, folder, fan) is `pag_OkXg1xaVPSlQ`.
