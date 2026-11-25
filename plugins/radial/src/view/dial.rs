//! The adjustable item, drawn as a dial (design: Nexus page `pag_zuxlNpXnwxa4`, "P-A"):
//! a 270° arc around its bubble, open at the bottom like a knob, ticks every 10 % while it
//! is open, the value as a number inside the bubble (render.rs), and a pill outside the ring
//! naming what is adjusted. With two or more targets the pill has a chevron and opens the
//! fan: one small bubble per target on an arc outside the ring, centred on the item.

use std::f32::consts::PI;
use std::time::Instant;

use gpui::{
    AnyElement, Div, Font, Hsla, SharedString, TextRun, Window, canvas, div, prelude::*, px, rgba,
    svg,
};

use super::MenuView;
use super::paint::{
    arc, content, ease_out_cubic, edged, on_circle, soft_shadow, ticks, with_alpha,
};
use crate::menu::{Colors, Item, ItemAction};
use crate::state::{Choice, Gauge, LevelState};

/// The dial: 0 at 7:30, the maximum at 4:30, clockwise (radians from 12 o'clock).
pub(super) const DIAL_START: f32 = -0.75 * PI;
pub(super) const DIAL_SWEEP: f32 = 1.5 * PI;
/// Bubble edge → dial centre line.
const DIAL_GAP: f32 = 6.;
/// Bubble edge → pill (clears the dial and its ticks).
pub(super) const PILL_GAP: f32 = 14.;
pub(super) const PILL_H: f32 = 24.;
const PILL_FONT: f32 = 12.;
const PILL_PAD: f32 = 10.;
const PILL_ICON: f32 = 12.;
/// Ring bubbles → fan bubbles (clears the dial's ticks).
pub(super) const FAN_GAP: f32 = 18.;
const FAN_ANIM_MS: f32 = 160.;
/// The fan's label holds full device names: wider than the dial's pill.
const FAN_PILL_MAX_W: f32 = 640.;

const CHEVRON_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6 9.5l6 6 6-6" stroke="#000" stroke-width="2.8" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>"##;
const PIN_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M9 3.5h6l-1 6.5 3.5 3.5h-11L10 10zM12 13.5V21" stroke="#000" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>"##;

/// Fan bubble diameter for a ring bubble of `bubble`.
pub(super) fn fan_diameter(bubble: f32) -> f32 {
    (bubble * 0.78).round()
}

/// Centres of `n` fan bubbles of diameter `fd`, on an arc around the ring centre `c` just
/// outside the ring, side by side and centred on direction `a` (the adjustable item's).
pub(super) fn fan_centers(
    n: usize,
    a: f32,
    ring: f32,
    bubble: f32,
    fd: f32,
    c: (f32, f32),
) -> Vec<(f32, f32)> {
    let r = ring + bubble / 2. + FAN_GAP + fd / 2.;
    let step = 2. * ((fd + 10.) / (2. * r)).min(1.).asin();
    (0..n)
        .map(|j| on_circle(c, r, a + (j as f32 - (n as f32 - 1.) / 2.) * step))
        .collect()
}

/// The pill's rectangle `(x, y, w, h)`: outward from a bubble of diameter `d` at `(bx, by)`
/// in direction `(dx, dy)` from the ring centre (above a bubble at the top, beside one at
/// the sides, below one at the bottom), kept on screen.
pub(super) fn pill_rect(
    (bx, by): (f32, f32),
    (dx, dy): (f32, f32),
    d: f32,
    w: f32,
    (vw, vh): (f32, f32),
) -> (f32, f32, f32, f32) {
    let off = d / 2. + PILL_GAP;
    let (x, y) = if dx > 0.35 {
        (bx + off, by - PILL_H / 2.)
    } else if dx < -0.35 {
        (bx - off - w, by - PILL_H / 2.)
    } else if dy < 0. {
        (bx - w / 2., by - off - PILL_H)
    } else {
        (bx - w / 2., by + off)
    };
    let clamp = |v: f32, len: f32, max: f32| v.clamp(4., (max - len - 4.).max(4.));
    (clamp(x, w, vw), clamp(y, PILL_H, vh), w, PILL_H)
}

/// Tick angles: every 10 of `max` (so 15 ticks up to 150).
pub(super) fn tick_angles(max: u32) -> Vec<f32> {
    let max = max.max(10);
    (0..=max / 10)
        .map(|k| DIAL_START + DIAL_SWEEP * (k * 10) as f32 / max as f32)
        .collect()
}

/// What the pill says: (pin glyph, text, chevron).
fn pill_text(item: &Item, g: &Gauge) -> (bool, String, bool) {
    let chevron = g.targets.len() >= 2;
    match &g.level {
        LevelState::Ready(l) => {
            let name = if l.label.is_empty() {
                &item.label
            } else {
                &l.label
            };
            let text = if l.muted {
                format!("Muted · {name}")
            } else {
                name.clone()
            };
            (g.pinned, text, chevron)
        }
        LevelState::Loading => (false, format!("{} …", item.label), false),
        LevelState::Unavailable(why) => {
            (false, format!("{} unavailable · {why}", item.label), false)
        }
    }
}

/// The fan's pill: the highlighted target's full (raw) name and its level, else the current
/// one's. The dial's own pill keeps the short name.
fn fan_text(g: &Gauge) -> String {
    let level = |t: &Choice| match (t.muted, t.percent) {
        (true, _) => " · muted".to_string(),
        (false, Some(p)) => format!(" · {p}"),
        (false, None) => String::new(),
    };
    match (
        g.fan_hl.and_then(|j| g.targets.get(j)),
        g.targets.iter().find(|t| t.current),
    ) {
        (Some(t), _) => format!("{}{}", t.description, level(t)),
        (None, Some(t)) => format!("{} · current", t.description),
        (None, None) => "Choose an output".to_string(),
    }
}

fn text_width(window: &Window, text: &str, font: Font) -> f32 {
    let run = TextRun {
        len: text.len(),
        font,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .layout_line(text, px(PILL_FONT), &[run], None)
            .width,
    )
}

/// Width of a pill holding `text` (+ the optional glyphs), at most `max_w`.
fn pill_width(window: &Window, text: &str, glyphs: usize, max_w: f32) -> f32 {
    // The measured line runs a few % short of what the div renders (seen with long raw
    // device names, 2026-11-24): leave room so the text never gets an ellipsis it doesn't
    // need. +2 is the 1 px border on each side.
    let w = text_width(window, text, window.text_style().font()) * 1.06
        + 2. * PILL_PAD
        + glyphs as f32 * (PILL_ICON + 4.)
        + 2.
        + 6.;
    w.min(max_w)
}

fn pill(
    rect: (f32, f32, f32, f32),
    text: String,
    pin: bool,
    chevron: bool,
    hot: bool,
    c: &Colors,
) -> Div {
    let (x, y, w, h) = rect;
    let glyph = |data: &'static [u8], color: u32| {
        svg()
            .data(data)
            .flex_none()
            .size(px(PILL_ICON))
            .text_color(rgba(color))
    };
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(w))
        .h(px(h))
        .px(px(PILL_PAD))
        .flex()
        .items_center()
        .gap(px(4.))
        .rounded_full()
        .bg(rgba(c.label_bg))
        .shadow(soft_shadow(c, 1.0))
        .border_1()
        .border_color(if hot { rgba(c.accent) } else { rgba(c.edge) })
        .text_size(px(PILL_FONT))
        .text_color(rgba(c.label_text))
        .when(pin, |p| p.child(glyph(PIN_SVG, c.accent)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(SharedString::from(text)),
        )
        .when(chevron, |p| {
            p.child(glyph(
                CHEVRON_SVG,
                if hot { c.accent } else { c.label_text },
            ))
        })
}

/// The fan growing out of its bubble (live: hit tests, pill) or going back into it.
pub(super) enum FanMode {
    Live,
    Leaving(f32),
}

/// Where the adjustable item is: its centre and direction from the ring centre.
pub(super) struct At {
    pub(super) b: (f32, f32),
    pub(super) dir: (f32, f32),
    /// Its angle (radians, clockwise from 12 o'clock).
    pub(super) a: f32,
}

impl MenuView {
    /// The dial around the adjustable item's bubble: track, value, ticks while open.
    /// Before the first reply, or when unavailable: just the track.
    pub(super) fn dial(
        &self,
        g: &Gauge,
        at: &At,
        d: f32,
        alpha: f32,
        engaged: bool,
        accent: u32,
    ) -> AnyElement {
        let value = match &g.level {
            LevelState::Ready(l) => Some((
                (l.percent as f32 / l.max.max(1) as f32).clamp(0., 1.),
                l.muted,
                l.max,
            )),
            _ => None,
        };
        self.dial_canvas(at.b, d, value, alpha, accent, engaged, g.open)
    }

    #[allow(clippy::too_many_arguments)] // one drawing call, its inputs
    fn dial_canvas(
        &self,
        (bx, by): (f32, f32),
        d: f32,
        value: Option<(f32, bool, u32)>,
        alpha: f32,
        accent: u32,
        engaged: bool,
        open: bool,
    ) -> AnyElement {
        let c = self.colors;
        let r = d / 2. + DIAL_GAP;
        let w = (d * 0.07).max(3.);
        let value_color = match value {
            Some((_, true, _)) => with_alpha(c.icon, 0.3 * alpha),
            _ if engaged => with_alpha(accent, alpha),
            _ => with_alpha(c.icon, 0.45 * alpha),
        };
        let tick_color = with_alpha(c.icon, 0.35 * alpha);
        let mark_color = with_alpha(c.icon, 0.9 * alpha);
        let track = with_alpha(c.track, alpha);
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
                let at = (bx + o.0, by + o.1);
                if let Some(t) = arc(at, r, DIAL_START, DIAL_SWEEP, w) {
                    window.paint_path(t, track);
                }
                let Some((frac, _, max)) = value else { return };
                if let Some(v) = arc(at, r, DIAL_START, DIAL_SWEEP * frac, w) {
                    window.paint_path(v, value_color);
                }
                if open {
                    let (r0, r1) = (r + w / 2. + 3., r + w / 2. + 6.);
                    if let Some(t) = ticks(at, r0, r1, &tick_angles(max), 1.5) {
                        window.paint_path(t, tick_color);
                    }
                    // Above 100 %: a bright mark at 100.
                    if max > 100 {
                        let a = DIAL_START + DIAL_SWEEP * 100. / max as f32;
                        if let Some(m) = ticks(at, r0 - 1., r1 + 2., &[a], 2.) {
                            window.paint_path(m, mark_color);
                        }
                    }
                }
            },
        )
        .absolute()
        .left_0()
        .top_0()
        .size_full()
        .into_any_element()
    }

    /// The pill next to the open dial. Records its rectangle for hit tests.
    pub(super) fn gauge_pill(
        &mut self,
        window: &Window,
        item: &Item,
        g: &Gauge,
        at: &At,
        bubble: f32,
        vp: (f32, f32),
    ) -> Div {
        let (pin, text, chevron) = pill_text(item, g);
        let glyphs = usize::from(pin) + usize::from(chevron);
        let w = pill_width(window, &text, glyphs, (bubble * 7.).min(vp.0 - 8.));
        let rect = pill_rect(at.b, at.dir, bubble, w, vp);
        self.pill_rect = Some(rect);
        pill(
            rect,
            text,
            pin,
            chevron,
            chevron && self.pill_hot,
            &self.colors,
        )
    }

    /// The fan: one bubble per target, growing out of the adjustable item, and its pill.
    /// Records the bubbles' centres for hit tests. `Leaving(q)`: going back into the
    /// bubble (`q` 1 → 0), drawn only.
    pub(super) fn fan(
        &mut self,
        window: &mut Window,
        root: Div,
        g: &Gauge,
        at: &At,
        (center, vp): ((f32, f32), (f32, f32)),
        mode: FanMode,
    ) -> Div {
        let (q, live) = match mode {
            FanMode::Live => {
                let opened = *self.fan_opened_at.get_or_insert_with(Instant::now);
                let q = ease_out_cubic(opened.elapsed().as_secs_f32() * 1000. / FAN_ANIM_MS);
                (q, true)
            }
            FanMode::Leaving(q) => (q, false),
        };
        if q < 1.0 {
            window.request_animation_frame();
        }
        let (ring, bubble) = (self.layout.ring, self.layout.bubble);
        let fd = fan_diameter(bubble);
        let centers = fan_centers(g.targets.len(), at.a, ring, bubble, fd, center);
        let c = self.colors;
        let mut root = root;
        for (j, (t, &(x, y))) in g.targets.iter().zip(&centers).enumerate() {
            let hl = g.fan_hl == Some(j);
            // Grow out of the adjustable item's bubble.
            let (x, y) = (at.b.0 + (x - at.b.0) * q, at.b.1 + (y - at.b.1) * q);
            let dd = fd * (0.6 + 0.4 * q);
            let shown = Item {
                label: t.name.clone(),
                icon: t.icon.clone(),
                action: ItemAction::Exec(String::new()),
                color: None,
                accent: None,
            };
            let glyph = if hl { c.accent_icon } else { c.icon };
            let mut b = edged(
                div()
                    .absolute()
                    .left(px(x - dd / 2.))
                    .top(px(y - dd / 2.))
                    .size(px(dd))
                    .rounded_full()
                    .bg(rgba(if hl { c.accent } else { c.bubble }))
                    .shadow(soft_shadow(&c, q))
                    .opacity(q)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(content(&shown, dd * 0.9, glyph)),
                &c,
            );
            if t.current && !hl {
                b = b.border_2().border_color(rgba(c.accent));
            }
            root = root.child(b);
        }
        if !live {
            return root;
        }
        self.fan_centers = centers;
        self.fan_d = fd;
        if q >= 1.0 {
            let text = fan_text(g);
            let r = ring + bubble / 2. + FAN_GAP + fd / 2.;
            let mid = on_circle(center, r, at.a);
            // Raw device names run long ("Core Ultra … HDMI / DisplayPort 1 Output (Stereo)").
            let w = pill_width(window, &text, 0, FAN_PILL_MAX_W.min(vp.0 - 8.));
            root = root.child(pill(
                pill_rect(mid, at.dir, fd, w, vp),
                text,
                false,
                false,
                false,
                &c,
            ));
        }
        root
    }
}

/// The bubble's content while the dial is open: the value as a number.
pub(super) fn number(percent: u32, d: f32, color: u32) -> AnyElement {
    div()
        .text_size(px(d * 0.36))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgba(color))
        .child(percent.to_string())
        .into_any_element()
}
