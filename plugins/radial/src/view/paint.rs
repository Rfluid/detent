//! Shared drawing helpers: colours, arcs, easing, bubble content, on-screen clamping.

use gpui::{
    AnyElement, BoxShadow, Hsla, ObjectFit, PathBuilder, SharedString, StyledImage, div, img,
    point, prelude::*, px, rgba, svg,
};

use crate::icon::Icon;
use crate::menu::{Colors, Item};

pub(super) fn with_alpha(c: u32, alpha: f32) -> Hsla {
    let mut h: Hsla = rgba(c).into();
    h.a *= alpha;
    h
}

/// The point at angle `a` (radians, clockwise from 12 o'clock) on a circle.
pub(super) fn on_circle((cx, cy): (f32, f32), r: f32, a: f32) -> (f32, f32) {
    (cx + r * a.sin(), cy - r * a.cos())
}

/// A stroked arc around `c` from angle `start` clockwise over `sweep` (radians, 0 = 12
/// o'clock), as a polyline (smooth at these sizes; a full turn needs no special case).
pub(super) fn arc(c: (f32, f32), r: f32, start: f32, sweep: f32, width: f32) -> Option<gpui::Path> {
    let sweep = sweep.clamp(0., std::f32::consts::TAU);
    if sweep <= 0. {
        return None;
    }
    let n = ((sweep / std::f32::consts::TAU * 96.).ceil() as usize).max(2);
    let mut b = PathBuilder::stroke(px(width));
    for k in 0..=n {
        let (x, y) = on_circle(c, r, start + sweep * k as f32 / n as f32);
        if k == 0 {
            b.move_to(point(px(x), px(y)));
        } else {
            b.line_to(point(px(x), px(y)));
        }
    }
    b.build().ok()
}

/// Radial strokes from radius `r0` to `r1` at each angle (the dial's ticks), one path.
pub(super) fn ticks(
    c: (f32, f32),
    r0: f32,
    r1: f32,
    angles: &[f32],
    width: f32,
) -> Option<gpui::Path> {
    if angles.is_empty() {
        return None;
    }
    let mut b = PathBuilder::stroke(px(width));
    for &a in angles {
        let (x0, y0) = on_circle(c, r0, a);
        let (x1, y1) = on_circle(c, r1, a);
        b.move_to(point(px(x0), px(y0)));
        b.line_to(point(px(x1), px(y1)));
    }
    b.build().ok()
}

/// `c` with its alpha scaled by `a` (stays a `0xRRGGBBAA`).
pub(super) fn fade(c: u32, a: f32) -> u32 {
    let alpha = ((c & 0xff) as f32 * a.clamp(0., 1.)).round() as u32;
    (c & !0xff) | alpha
}

pub(super) fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Up to two initials for an item without an icon.
pub(super) fn initials(label: &str) -> String {
    label
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .flat_map(char::to_uppercase)
        .collect()
}

/// The bubble's content, `d` = bubble diameter.
pub(super) fn content(item: &Item, d: f32, glyph_color: u32) -> AnyElement {
    match &item.icon {
        Icon::Glyph(p) => svg()
            .external_path(SharedString::from(p.to_string_lossy().into_owned()))
            .size(px(d * 0.52))
            .text_color(rgba(glyph_color))
            .into_any_element(),
        Icon::Image(p) => img(p.clone())
            .size(px(d * 0.6))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        Icon::Photo(p) => img(p.clone())
            .size(px(d))
            .rounded_full()
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        // Plain text only: pictographs/emoji don't render in this GPUI on Linux (verified
        // 2026-11-23, Noto Color Emoji and Noto Emoji both installed); use `icon`/`image`.
        Icon::Text(t) => div()
            .text_size(px(d * 0.4))
            .text_color(rgba(glyph_color))
            .child(t.clone())
            .into_any_element(),
        Icon::None => div()
            .text_size(px(d * 0.3))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(rgba(glyph_color))
            .child(initials(&item.label))
            .into_any_element(),
    }
}

/// Clamp the ring centre so a ring of radius `extent` stays inside the viewport.
pub(super) fn keep_on_screen((x, y): (f32, f32), extent: f32, (vw, vh): (f32, f32)) -> (f32, f32) {
    let clamp = |v: f32, max: f32| {
        if max < 2. * extent {
            max / 2.
        } else {
            v.clamp(extent, max - extent)
        }
    };
    (clamp(x, vw), clamp(y, vh))
}

pub(super) fn soft_shadow(c: &Colors, alpha: f32) -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.), px(2.), with_alpha(c.shadow, alpha)).blur_radius(px(10.))]
}

/// A hairline in the theme's edge colour (none when it is transparent).
pub(super) fn edged<E: Styled>(e: E, c: &Colors) -> E {
    if c.edge & 0xff == 0 {
        e
    } else {
        e.border_1().border_color(rgba(c.edge))
    }
}
