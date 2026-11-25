---
title: Detent logo
status: current
version: 0.1.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: [".design/assets/logo-mark.svg", ".design/assets/logo-mark-dark.svg", ".design/assets/logo-mark-light.svg", ".design/assets/logo-mark-symbolic.svg", ".design/assets/logo-lockup-dark.svg", ".design/assets/logo-lockup-light.svg", ".design/assets/app-icon.svg"]
owner: "@rfluid"
tags: [design, brand, logo]
---

# Logo

## Idea

A detent is the notch a scroll wheel clicks into: a stop you can feel. Detent the daemon sits between the mouse and the desktop and decides what each click does. The mark is a ratchet wheel, the mechanism behind that click, with one tooth engaged.

## Anatomy

The mark is drawn on a 64 × 64 grid, centred on (32, 32).

| Part | Definition |
|---|---|
| Hub | Circle, radius 17, with a centred hole of radius 5 (even-odd fill) |
| Tooth | Triangle `32,15` → `49.36,11.32` → `42.93,18.98` |
| Tooth count | 8, each rotated 45° about (32, 32). The ramp rises clockwise and drops back radially |
| Engaged tooth | The tooth rotated 315°. It is the only accent-coloured part |
| Outer extent | About radius 27, so the mark fills roughly 5 to 59 of the 64 grid |

The tooth is defined once (`<polygon id="tooth">`) and reused with `rotate()`. Keep it that way.

## Colour

| Surface | Ink | Engaged tooth |
|---|---|---|
| Dark | `#F2F4F6` (`--nx-mark-ink`, dark theme) | `#68CCF5` (`--nx-hue-ice`) |
| Light | `#111312` (`--nx-mark-ink`, light theme) | `#0A84FF` (`--nx-status-info`) |
| Single colour | `currentColor` | same colour at 55% opacity |

Ice blue on white is too pale to carry, which is why the light version uses the brand's info blue.

## Files

| Asset | Use |
|---|---|
| `assets/logo-mark.svg` | Ink follows `currentColor`, accent fixed to ice blue. Default for the web and docs |
| `assets/logo-mark-dark.svg` | Fixed colours for dark surfaces |
| `assets/logo-mark-light.svg` | Fixed colours for light surfaces |
| `assets/logo-mark-symbolic.svg` | Single colour for tray and panel icons |
| `assets/logo-lockup-dark.svg`, `assets/logo-lockup-light.svg` | Mark plus wordmark, 280 × 64 |
| `assets/app-icon.svg` | 512 px application icon on the dark brand tile |

## Wordmark

Lowercase `detent`, DM Sans weight 400, letter spacing −0.01 em, in the ink colour. The wordmark height is the mark's height at a 56 : 64 ratio, with a 16-unit gap between them (lockup viewBox 280 × 64). The text in the lockup files is live. Convert it to outlines before using it where DM Sans is not installed.

## Clear space and size

- Clear space on every side: half the mark's width (`--nx-mark-clear-space: 0.5` in the Nexus tokens).
- Minimum mark size: 16 px. Below that, use the symbolic file and expect the engaged tooth to blur.
- Minimum lockup width: 96 px.
- App icon: the mark takes 62.5% of the tile. The tile corner radius is 22.5% of its side.

## Do

- Place the mark on the Nexus dark or light tiles (`--nx-tile-dark-*`, `--nx-tile-light-*`) or on plain page backgrounds.
- Scale it uniformly.
- Use the symbolic version when the platform recolours icons itself.

## Don't

- Don't add shadows, bevels, gradients or facets. We tried them (curved, faceted and lifted variants) and went back to the flat mark. It reads better small and fits the Nexus brand's flat marks.
- Don't recolour the engaged tooth, move it, or add a second accent.
- Don't change the tooth count or proportions.
- Don't rotate the whole mark. The engaged tooth sits at the upper left on purpose.
- Don't set the wordmark in another font, in capitals, or in a heavier weight.
- Don't put the mark in a circle or a badge. The app icon tile is the only container.

## Rejected options

Kept on the Nexus page for reference. Short reasons so nobody reopens them without cause:

| Option | Why not |
|---|---|
| Notch (ring with a ball in a gap) | Simplest and the most legible small, but it was not chosen over the ratchet. A good fallback if the ratchet fails in a tray |
| D-notch, Wheel, Ring, Mouse | Generic or literal. Ring would make the radial plugin the face of the whole project |
| Shaded ratchets (soft drop, machined metal, cast shadow, faceted, curved blades) | Rougher or busier, and the detail is lost at small sizes |

## Changing the logo

Change the SVG first, then update the anatomy and colour tables here, then check every file in `assets/` still matches, then bump `last_verified`. Do not fork per-surface geometry. Colour and container are the only things that differ between files.
