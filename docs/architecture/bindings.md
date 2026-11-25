---
title: Bindings, chords, layers and gestures
status: current
version: 0.2.0
last_updated: 2026-11-24
last_verified: 2026-11-24
source_refs: ["crates/engine/src/bindings.rs", "crates/engine/src/modifiers.rs", "crates/engine/src/engine.rs", "crates/engine/src/engine/", "crates/engine/src/compiled.rs", "crates/config/src/trigger.rs", "crates/config/src/compile.rs"]
owner: "@rfluid"
tags: [architecture, bindings, hot-path]
---

# Bindings, chords, layers and gestures

## Compiled table

Config is compiled ahead of time (`crates/config/src/compile.rs`) into flat arrays:

```
action = profile.bindings[layer][control_slot][edge]   // BindingTable::lookup, O(1)
layer  = profile.layers[held_modifier_mask]             // LayerMap, 256-entry array
```

- `control_slot`: fixed slots (`left right middle back forward gesture smartshift thumb wheel
  thumbwheel swipe`) plus extra slots for unnamed CIDs.
- `edge`: Press, Release, Tap, Hold, RotatePos/Neg, SwipeUp/Down/Left/Right.
- `Action`: closed `Copy` enum ≤ 32 bytes (`crates/engine/src/action.rs`).
- Profiles: `profiles[0]` is the base; each `[profile.x]` is base + its overrides, compiled
  up front. Switching profile is an index change, no HID++ traffic (diversion is the union).

## Where events come from

| Control | Source | Needs |
|---|---|---|
| gesture, thumb (Haptic Sense Panel), middle, back, forward, smartshift | HID++ diversion (`0x1B04` event 0) | divertable CID |
| swipe | HID++ raw XY (`0x1B04` event 1) while a modifier with swipe bindings is held | raw-XY capable |
| left, right, wheel, thumbwheel | the mouse's **evdev** node, only while grabbed | a held modifier |

The wheel and thumbwheel are **not** diverted: the kernel resets the wheel mode on every
reconnect, and a grab gives the same effect with no HID++ writes. Hence: left/right/wheel/
thumbwheel bindings must be chords (`gesture + wheel.up`); the compiler rejects them alone.

## Lookup order

1. current layer, 2. base layer, 3. native behaviour: a diverted control without a binding
re-emits its own button (middle → BTN_MIDDLE, back → BTN_SIDE, forward → BTN_EXTRA; the
SmartShift button toggles ratchet), and an evdev event during a grab passes through.

## Modifiers and layers

Any divertable control used before `+` (or as a layer's `hold`) is a modifier (max 8). The
held set is a bitmask; each mapped mask selects a layer. **The modifier must be pressed
first.**

## Chords on non-divertable buttons: grab while armed

1. Modifier pressed (diverted) → if its layer needs evdev events and **no physical button is
   down**, `EVIOCGRAB` the mouse's evdev node.
2. While grabbed, bound presses fire their action and are swallowed (their release too);
   unbound events are forwarded to Detent's virtual pointer.
3. Modifier released **and** no button down → ungrab.

## Tap vs chord vs swipe vs hold

A press starts "undecided":
- another control pressed (or the wheel turned) → **chord**; tap/swipe/hold cancelled
- raw XY movement past the threshold → **swipe** (dominant axis) or **diagonal** (nothing),
  decided once per hold
- held for `hold(N ms)` → **hold** (timer = the data plane's poll timeout)
- released while still undecided → **tap**

`keys`/`button` bound to a press are held down while the control is held; everything else
fires once.

## Gesture state machine

- The first raw-XY frame after a press is dropped (some firmware sends garbage).
- Movement is accumulated in device counts and converted to physical distance with the
  current DPI: `threshold_mm` (default **6 mm**, owner's choice) is the same feel at any DPI.
- Direction is decided once, when the movement passes the threshold: the dominant axis if
  it exceeds the other by **1.3×**, else a **diagonal**, which fires nothing and also
  cancels the tap (owner's choice, 2026-11-24: diagonals are "no action", not "the nearest
  axis" and not "a tap"). Screen convention: +y is down.
- Haptic confirmation (`[gesture] haptic`, owner's choice 2026-11-24): when a swipe
  modifier's hold resolves as a gesture (a swipe in any direction, bound or not; a
  diagonal; or a tap, i.e. no direction) the engine pushes one `HapticPulse` to the
  control ring alongside the action. Chords and holds don't pulse. The pulse goes through
  the worker like any device command, so it never blocks the event path.

## Config surface

See [../specs/config.md](../specs/config.md): `[[bind]] when = "gesture + left"` or
`[layer.<name>] hold = "gesture"`. Both compile to the same table.
