---
title: "Native plugin: radial"
status: current
version: 0.7.0
last_updated: 2026-11-25
last_verified: 2026-11-25
source_refs: ["plugins/radial/", "plugins/kde/src/dbus.rs", "bins/detentd/src/bus.rs", "bins/detentd/src/bus/", "crates/engine/src/engine.rs", "crates/engine/src/engine/", "crates/config/src/native.rs", "config/detent.example.toml"]
owner: "@rfluid"
tags: [plugins, native, radial, gpui]
---

# Native plugin: `radial` (`detent-radial`)

Press the thumb button (Haptic Sense Panel) and a ring of actions appears at the cursor,
modelled on the Logitech **Actions Ring** (Logi Options+): same layout, interaction and
haptics, with Detent's own glyphs and fully user-defined contents. Built with **GPUI**
(upstream, with Wayland layer-shell; ADR 0005 rev. 2). Linux only for now: on macOS and
Windows the binary builds without GPUI and exits with a "not yet supported" message (ADR 0010).

## Reference: what the Actions Ring does (researched 2026-11-23/24)

From Logitech's support article ("Actions Ring", support.logi.com 17844647906967), the MX
Master 4 getting-started and haptics guides (hub.sync.logitech.com), the product page, reviews
and frames of Logitech's own intro video (`action-ring-intro-loop-video-no-logo.mp4`).
Quotes are Logitech's:

| Aspect | Actions Ring | Detent |
|---|---|---|
| Slots | 8 round "bubbles", evenly spaced on a full circle around the cursor, no backdrop | same (1–8 items) |
| Bubble | pale lavender disc, dark single-colour glyph | same in light; a dark palette for dark desktops; `theme = "system"` (default) follows Plasma; colours configurable |
| Centre | small grey disc with × | same; inside a folder it shows ← (back) |
| Highlight | bubble fills violet, glyph turns white | same (accent `#814efa`) |
| Labels | none on the ring in use (names only in the editor) | owner's choice: a tooltip after 500 ms of hover (`labels = "hover"`, `tooltip_ms`); `never` = Logitech's look |
| Icons | every bubble has an icon | same: an item without `icon` gets one chosen from its action (`defaults.rs`) |
| Open | bubbles grow outward from the centre | same (~160 ms ease-out, `animation_ms`); every other transition goes back into the centre (see "Motion") |
| Position | "The ring opens at your cursor." | same: centred on the cursor (position from KWin), on the cursor's output |
| Cursor | moves freely; "move your cursor over one of the round elements" | same: real pointer, bubble under it is highlighted |
| Select | both, no setting: "Push and release … Hover over a bubble → Click it" or "Push … without releasing → Hover over a bubble → Release it on the bubble" | same, both always |
| Close | "Press Esc to close the Actions Ring." | Esc; also the centre ×, right click, click outside, pressing the panel again |
| Keyboard | Tab / Shift+Tab rotate the highlight, Enter or Space select | same |
| Haptics | "when navigating between the action bubbles" and "when you click on an action bubble" | waveform 4 on each bubble entered, 1 on select |
| Folders | a bubble opens up to 9 more actions; "Esc will also return you to the main Actions Ring from within a folder" | `items = [...]`; Esc, the centre ← or right click go back |
| Per-app rings | the ring follows the active app | `menus.<m>.apps.<app>` (needs the `app` context from `detent-kde`) |
| Sizes | Small / Medium (default) / Large | `size = "small" \| "medium" \| "large"` or `radius` |
| Adjustable items | hover + wheel or drag shows a slider pill to the right | `adjust = "kde.volume"`: a 270° dial around the bubble; hovered, the value shows in the bubble and a pill names the output; wheel ±2%, thumbwheel ±1%; the pill switches the output; see "Adjustable items" |

Detent doesn't ship or copy Logitech artwork; glyphs come from the user's icon theme
(Breeze `-symbolic` icons on KDE) or the user's own files.

## Interaction

```
 thumb.press ─► daemon ─ActionInvoked(open, {menu}, trigger "thumb.press")─► detent-radial
                  │◄─ LeaseRequest(Control("thumb"), 10 s) ──────────────────┤ (hold detection)
                  │◄─ Invoke(kde.cursor, {reply: "radial.cursor"}) ──────────┤
 detent-kde ◄─────┤   KWin script: workspace.cursorPos + screenAt            │
 detent-kde ─────►│── ActionInvoked(cursor, {x, y, output, w, h}) ──────────►│ open overlay there
 pointer ───────► KWin ── wl_pointer motion / buttons, keyboard ────────────►│ hover / click / keys
                  │◄─ Device(HapticPulse 4) ───────────────────────────────── │ bubble entered
 thumb release ─► │── Event::Button{thumb, release} ────────────────────────►│ on a bubble: select;
                  │◄─ LeaseRelease ────────────────────────────────────────── │ elsewhere: stay open
                  │◄─ Invoke(step…) ──────────────────────────────────────── │ hide overlay
```

What the user does (both styles work at once; there is no mode setting):

| Input | Effect |
|---|---|
| press + release the thumb panel | ring opens around the cursor and stays open |
| move the mouse | the cursor moves freely; the bubble under it is highlighted (haptic tick); gaps and the area outside highlight nothing |
| left click a bubble | select it (haptic); an action runs and the ring closes, a folder opens |
| hold the thumb panel, move, release on a bubble | select it; released anywhere else, the ring stays open for a click |
| press the thumb panel again | select the highlighted bubble, or close if none |
| Esc, right click, click the centre (× / ←) | inside a folder: back to the main ring; at the top: close |
| click outside the ring | close |
| Tab / Shift+Tab | highlight the next bubble clockwise / anticlockwise |
| Enter / Space | select the highlighted bubble |
| on an adjustable bubble (volume) | hover 120 ms: the dial opens; wheel / thumbwheel / ↑ ↓: change it; middle click: mute; click (or hold-release): pin; click its pill: choose the output, see "Adjustable items" |

Logitech's pages don't say what clicking outside, right clicking or pressing the panel again
do; Detent closes (outside) and treats right click like Esc, the usual popup behaviour.

Selecting a **folder** replaces the ring with the folder's items; the centre then shows ←. A
folder always continues click-to-select (the thumb lease is dropped when it opens).

The trigger is leased only while it is held, and only the trigger (`Control("thumb")`): the
pointer and the other buttons are never grabbed. The lease ends on release, so an open ring
holds no input and has no time limit. Lease expiry ends a hold but keeps the ring; the escape
chord closes it.

## Rendering

- **Window:** wlr-layer-shell surface, `Overlay` layer, anchored to all edges with the
  output's logical size from `kde.cursor` (0×0 without it; both = the whole output, but see
  "Failure behaviour" for fractional scales), exclusive zone −1, transparent, created per opening and removed on
  close. Folder changes update the same window. `KeyboardInteractivity::Exclusive`: KWin
  activates the overlay while it is open (so Esc/Tab/Enter reach it) and gives focus back to
  the previous window when it closes (verified in a nested KWin 6.8). `keys` steps wait 60 ms
  after closing so they land in that window. `detent-kde` ignores the overlay's activation,
  so profiles don't switch.
- **Position — always around the cursor:** on open the plugin asks `kde.cursor` (a KWin
  script reads `workspace.cursorPos` and the output under it) and waits up to 200 ms. The
  overlay opens on that output (GPUI display matched by output name) centred on that point.
  Without a reply (no `detent-kde`, not KWin) it opens without a position and takes the
  compositor's pointer `enter` / first motion instead. Until a position is known **nothing is
  drawn**, so the ring never appears anywhere else. Near an edge the ring is shifted just
  enough to stay fully visible.
- **Hit test** (`geometry::hit`): a bubble is hovered when the cursor is within its radius
  (+3 px); the centre disc likewise; everything else is "outside". Drawing and hit testing use
  the same `bubble_center`, so the highlighted bubble is the one under the cursor.
- **Geometry:** `radius` = cursor → bubble centre. Bubble = 0.57 × radius, centre disc =
  0.34 × radius (the Actions Ring's proportions). small/medium/large = 68/84/104 px.
- **Theme:** `theme = "system"` (default) picks the light or dark palette from the desktop's
  preference, read when the ring opens. GPUI gets it from the XDG desktop portal
  (`org.freedesktop.appearance color-scheme`, kept current by its change signal), which Plasma
  sets from the colour scheme (Breeze Dark → `1` = prefer dark, checked 2026-11-24). Light =
  the Actions Ring look; dark = deep violet-grey bubbles (`#2b2833`), light glyphs, a lighter
  violet highlight (`#8f5cff`) for contrast, a faint light hairline (`edge`) so bubbles don't
  sink into dark wallpapers, deeper shadows. `colors = { … }` overrides apply to both.
- **Icons** (`icon.rs`): a theme name is looked up in the current icon theme (KDE's
  `kdeglobals [Icons] Theme`, its `Inherits`, `hicolor`, `pixmaps`); `<name>-symbolic` is
  preferred and drawn as a single-colour glyph (dark at rest, white when highlighted).
  Non-symbolic theme icons and PNG/JPEG files are drawn in colour; `image` photos fill the
  bubble, cropped to a circle (a highlighted photo gets an accent ring). Lookups run on the bus
  thread while parsing settings, never on the UI thread.
- **Latency (measured):** GPU init (~0.7 s) is paid once at plugin start with a 1×1 warm-up
  surface; each opening then costs ~130–150 ms (GPUI creates a GPU surface per window), plus
  the `kde.cursor` round trip. Phase 6: a persistent surface would bring this to one frame.

### Preview

`detent-radial --preview [CONFIG] [--menu NAME] [--app APP] [--at X,Y[,OUTPUT[,W,H]]] [--folder N]
[--highlight N [--pin [--fan]]] [--seconds S] [--backdrop "#rrggbb"] [--theme system|light|dark]
[--warm-up]`
runs the real ring (same state machine, window and input handling) from a config file
(default `~/.config/detent/config.toml`) without the daemon: no lease, no haptics, and a
selection prints what it would run. `--at` stands in for the `kde.cursor` reply (needed in a
nested `kwin_wayland --virtual`, which has no pointer); `W,H` is the output's logical size. Adjustable items get a stand-in
provider with three outputs (Speaker 42%, Headphones 25%, HDMI 1 80%); `--pin` pins the
highlighted item's dial and `--fan` then opens its output fan. `--warm-up` starts like the
plugin (1×1 warm-up surface first, the ring 2.5 s later) to reproduce problems on its window
path; with `WAYLAND_DEBUG=1` it traces the protocol.

## Settings (`[plugins.radial]`)

| Key | Default | Meaning |
|---|---|---|
| `size` | `"medium"` | `small`, `medium`, `large` |
| `radius` | (from `size`) | cursor → bubble centre in logical px (40–300); overrides `size` |
| `theme` | `"system"` | `system` (follow the desktop's light/dark), `light` (Actions Ring look) or `dark` |
| `colors` | | override colours in both palettes: `bubble, icon, accent, accent_icon, center, center_icon, label_bg, label_text, track` (gauge track), `edge` (hairline; `#00000000` = none), `shadow` as `#rgb`, `#rrggbb`, `#rrggbbaa` |
| `labels` | `"hover"` | `hover`: tooltip next to the highlighted bubble once it has been highlighted (hover or Tab) for `tooltip_ms`; `never`; `always` (all names, no delay) |
| `tooltip_ms` | `500` | hover time before the tooltip shows (0–5000) |
| `animation_ms` | `160` | opening; closing and leaving a ring are shorter (¾ and ⅝ of it) (0 = all off) |
| `haptic_ticks` | `true` | haptic pulse on item change (waveform 4 *subtle-collision*) and selection (1 *damp-state-change*), as Options+ does |
| `menus.<name>.items` | | 1–8 items, clockwise from 12 o'clock |
| `menus.<name>.apps.<app>.items` | | ring used instead when the focused app matches `<app>` (exact, or the last part of a reverse-DNS id: `konsole` ↔ `org.kde.konsole`) |

### Items

```toml
{ label = "Copy", icon = "edit-copy", keys = "Ctrl+C", color = "#c43e1c" }
```

Appearance (optional, one of). **Without one, the item gets a default icon** chosen from its
action (`defaults.rs`): keys combos (`Ctrl+C` → `edit-copy`, `Ctrl+V` → `edit-paste`,
`Ctrl+Z` → `edit-undo`, `PlayPause` → `media-playback-start`, `Alt+Left` → `go-previous`, …,
other combos → `input-keyboard`), `kde.*` actions (`kde.overview` → `view-grid`,
`kde.desktop.next` → `go-next`, volume, windows, tiles, …), `exec` / `app` → the program's own
icon from its desktop file (else `system-run`), `open` → `globe` (URL) / `folder` /
`document-open`, macros → their first step's icon, folders → `folder`. A named icon that
can't be found falls back to the default too; the label's initials appear only if the icon
theme has neither.

| Key | Meaning |
|---|---|
| `icon` | icon-theme name (`edit-copy`) or file path (`~/icons/x.svg`, `.png`); SVG files are tinted like glyphs unless `tint = false` |
| `image` | photo/picture file filling the bubble (cropped to a circle) |
| `text` | short plain text (`"M"`, `"42"`); emoji/pictographs don't render in GPUI on Linux yet (verified 2026-11-23), use `icon` or `image` |
| `color` / `accent` | this bubble's fill at rest / when highlighted |

Action (exactly one):

| Key | Runs |
|---|---|
| `action = "kde.overview"` (+ any params) | any action in the shared namespace (other plugins too) |
| `exec = "spectacle -r"` | shell command (built-in `exec`) |
| `keys = "Ctrl+C"` | key combo (built-in `keys`) |
| `app = "org.kde.konsole"` | launch a desktop app (`gtk-launch`); `label`/`icon` default to the desktop file's `Name`/`Icon` |
| `open = "https://…"` / `"~/Documents"` | URL, file or folder with the default handler (`xdg-open`) |
| `steps = [{ keys = "Ctrl+A" }, { delay_ms = 30 }, { keys = "Ctrl+C" }]` | macro: steps in order (any non-folder action, plus `delay_ms` ≤ 10 s) |
| `items = [...]` | folder of 1–9 items (folders don't nest) |
| `adjust = "kde.volume"` (+ `step`, `fine_step`, `max`, `invert_fine`) | an adjustable value, see below (one per ring) |

## Adjustable items

```toml
{ label = "Volume", adjust = "kde.volume", step = 2, fine_step = 1 }
```

| Key | Default | Meaning |
|---|---|---|
| `adjust` | | the provider, `plugin.name` (`kde.volume`) |
| `step` | `2` | percent per main-wheel notch (1–25) |
| `fine_step` | `1` | percent per thumbwheel step (1–25) |
| `max` | `100` | upper limit (up to 150, like Plasma's "raise maximum volume") |
| `invert_fine` | `false` | flip the thumbwheel direction |

What the user sees and does:

The look is design P-A, "the bubble is the dial" (Nexus pages `pag_CYTi6XEBnNvp` and
`pag_zuxlNpXnwxa4`, chosen 2026-11-24): one scale, no card.

- When the ring opens, the plugin asks the provider for the value and its targets. A
  **dial** hugs the bubble: a 270° arc open at the bottom like a knob, 0 at 7:30 and the
  maximum at 4:30. At rest it is grey; it takes the accent while the bubble is highlighted,
  pinned or its fan is open. Muted: faint, and the bubble shows the muted speaker glyph.
- **Hover the bubble for 120 ms** (or scroll on it, or click it): the dial opens. The
  bubble shows the **value as a number** instead of its glyph, **ticks** appear every 10 of
  the maximum (where the mouse gives a haptic tick; above 100 a bright mark sits at 100), and
  a **pill** outside the ring (above a bubble at 12 o'clock, beside one at 3 or 9) names
  what is adjusted: the output's short name, `Muted · ` first when muted, a pin glyph when
  pinned, a chevron when there are outputs to switch to. Hovering the pill counts as
  hovering the bubble. The other bubbles stay as they are.
- **Wheel** (vertical axis) changes by `step` per notch, up = louder; the **thumbwheel**
  (horizontal axis) by `fine_step`; **↑ / ↓** keys like the wheel. High-resolution wheels
  report fractions of a notch; they add up to whole notches (one step per notch, not faster).
- **Haptics:** a tick (waveform 4) each time the value reaches or crosses a 10% mark, a firm
  pulse (waveform 1) on reaching 0% or the maximum; nothing when pushing past it.
- **Middle click** on the bubble or pill mutes / unmutes; scrolling up while muted unmutes
  (as in Plasma's applet).
- **Selecting** it (click, Enter, hold-release) runs nothing and doesn't close the ring: it
  **pins** the dial. Pinned, the highlight stays on it and the wheel adjusts wherever the
  cursor is; select it again to unpin. Tab unpins too, unless there are outputs to switch
  to: then Tab opens the fan.
- Leaving an unpinned bubble closes the dial 300 ms later (coming back in time keeps it).
- Pressing the thumb panel again with the volume highlighted closes the ring (there is
  nothing to run). Esc, right click, the centre and a click outside close as usual.
- No answer within 1 s, or an error: the dial shows only its track and the pill says
  "Volume unavailable · <reason>"; the wheel does nothing, everything else works.

### Switching the output

With two or more outputs the pill has a chevron. **Click it** (or Tab while pinned) and the
**fan** opens: one small bubble per output (theme icon: speakers, headphones, display,
sound card), on an arc just outside the ring, centred on the volume bubble and growing out
of it (160 ms). The current output has an accent ring; the centre turns into ← and the
ring's other glyphs go faint (the ring doesn't react while the fan is open). The fan's own
pill uses the raw device names (the provider's `description`, e.g. "Core Ultra 200H/200V
Series Processors HD Audio Speaker", up to 640 px wide): `<raw name> · current`, or the
hovered output's raw name and level. The dial's pill keeps the short name (owner's choice,
2026-11-24: on hover, the raw name tells outputs apart better than an alias).

| Input while the fan is open | Effect |
|---|---|
| hover a fan bubble / wheel / Tab, Shift+Tab | highlight it (haptic tick); the wheel stops at the ends, Tab wraps; the first step starts at the current output |
| click it, Enter, release the held trigger on it, press the trigger again | switch to it (select haptic): the fan folds, the dial shows the new output's level at once and its reply corrects it; the ring stays open |
| the current output | just folds the fan |
| Esc, right click, the centre ←, a ring bubble, the pill | fold the fan, nothing changes |
| click outside everything | close the ring |

Switching moves the audio that is already playing to the new output, including streams an
app or mixer had pinned to one output (docs/plugins/kde.md, "Outputs"). One output only: no
chevron, and the pill acts like its bubble (it pins). At most 8 outputs are shown.

The wheels reach the overlay as ordinary `wl_pointer.axis` events: Detent never diverts the
wheel or thumbwheel (`.agent/memory/facts/2026-11-23-kernel-coexistence.md`), so no lease
and no daemon change are needed.

**Provider contract** (any plugin can provide one): the radial plugin invokes
`<adjust>.get { reply: "radial.level", seq, max }`,
`<adjust>.set { percent, max, unmute, osd: false, reply, seq }` and
`<adjust>.mute { reply, seq, max }`; the provider invokes `radial.level` with
`{ source: "<adjust>", seq, percent, muted, max, label }` or `{ source, seq, error }`. Only the
reply to the latest request is taken, so answers to superseded requests can't make the gauge
jump back while scrolling. `detent-kde` provides `kde.volume` (docs/plugins/kde.md, "Volume").
The value shown changes as soon as you scroll; the reply then sets the real one.

Targets (optional; a provider without them simply never answers): the plugin invokes
`<adjust>.targets { reply: "radial.targets", seq }` when the ring opens; the provider
invokes `radial.targets` with `{ source, seq, targets: [{ id, name, description, icon,
percent, muted, current }] }` (`id` a string, `name` short, `description` the full name,
optional, `icon` a theme icon name) or `{ source, seq, error }`.
Choosing one invokes `<adjust>.target { id, max, reply: "radial.level", seq }`, answered
like `.get` for the new target. The reply's icons are resolved on the bus thread.

## Capabilities

`lease.buttons` (for the trigger control only), `device.haptics`, `actions.invoke` (items,
and `kde.cursor`), `input.emit` (for `keys`), `events.subscribe` (the `app` context for
per-app rings; without it the top-level rings are used everywhere). No `lease.pointer`: the
cursor is never grabbed.

## Leases (daemon side)

- Control lease (`thumb`), while the trigger is held: the diverted control's press/release →
  `Event::Button{thumb}` instead of its binding. Released when the thumb is released (or a
  selection / folder / close happens first).
- One lease at a time; `max_ms` ≤ `daemon.max_lease_ms` (10 s).

## Failure behaviour

- Plugin crash or disconnect → the daemon revokes the lease → the thumb's binding returns.
  (Nothing else is held.)
- Lease expiry while still holding → the hold ends, the ring stays open for a click. The
  **escape chord** (left + right held 2 s), reload or disconnect → the ring closes.
- No `detent-kde` / not KWin → no cursor reply; after 200 ms the overlay opens and appears at
  the pointer once the compositor reports it (it draws nothing before that).
- No layer-shell (`LayerShellNotSupportedError`) → logged; the lease is released immediately.
- Any other failure to open the overlay → logged, the lease released, and the plugin exits
  (status 1) so the daemon restarts it (2 s backoff). The usual cause is KWin having
  dropped the plugin's Wayland connection for a protocol error (`error in client
  communication (pid N)` in `plasma-kwin_wayland.service`): GPUI keeps running without it,
  and every later window fails with `Adapter … is not compatible with the display surface`
  (Mesa can't query surface formats on a dead connection). `detentctl logs --plugin
  radial` shows both sides.
- **Fractional-scale outputs (fixed 2026-11-25):** opened at 0×0, the overlay on an output
  at e.g. 160% made GPUI apply the scale event that comes before the first `configure` with
  `wp_viewport.set_destination(0, 0)`. That is a protocol error ("invalid destination
  size"), so KWin disconnected the plugin and the ring stopped working on every monitor.
  The overlay now opens at the output's logical size from `kde.cursor` (`width`,
  `height`). Without them (no `detent-kde`) it still opens at 0×0, which is fine on
  integer-scale outputs only.
- Bad settings → logged, the previous settings stay. Missing icon/image files → logged, the
  bubble falls back to the label's initials.

## Motion

Design: Nexus page `pag_OkXg1xaVPSlQ` (option B, "collapse into the centre", approved
2026-11-25); code: `plugins/radial/src/view/motion.rs`. Everything comes out of the centre
and goes back into it. Arriving eases out, leaving eases in and is shorter.

| Transition | What moves | Time (default) |
|---|---|---|
| Open | bubbles grow out of the centre | 160 ms ease-out |
| Close (Esc, right click, outside, ×, trigger again) | bubbles slide in and shrink into the centre; the centre closes last | 120 ms, overlay gone at 160 ms |
| Select | the chosen bubble travels into the centre, which flashes the accent and closes | 120 + 40 ms |
| Folder / back | the old ring collapses (100 ms), the centre dips and swaps × ↔ ←, the new ring grows (160 ms) | 260 ms |
| Fan dismissed | the output bubbles go back into the volume bubble | 100 ms |

While closing, the overlay takes no pointer input (empty input region; clicks reach the
window below) and ignores keys. A layer surface can't give up keyboard focus while mapped
(GPUI fixes `KeyboardInteractivity` at creation), so an item whose steps include `keys`
closes at once, as before: its shortcut must reach the app, not the overlay. Opening the
ring again while one is closing replaces it immediately.

## Not done

- Adjustable items: the thumbwheel's direction (which way is "louder") is not yet checked on
  the device (`invert_fine` flips it); dragging the dial isn't supported; the level and the
  output list aren't updated live when they change elsewhere while the ring is open (both
  are read on each opening). The design's smaller animations (tick flash on a detent, the
  dial sweeping to a new output's level, crossfading glyph ↔ number) aren't done; more than
  8 outputs aren't reachable (the design's "More" bubble isn't done).
- Motion: the dial's arc retracting into its bubble when the dial closes (in the motion
  design) isn't done; the dial only shrinks with the ring on close.
- A theme change while the ring is open applies from the next opening.
- The per-app ring only knows the focused app after the first focus change since the plugin
  started (context is published on change, not replayed).
