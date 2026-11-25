//! Drawing the overlay: centre disc, bubbles, labels, and the adjustable item's dial, pill
//! and fan (dial.rs). Each frame's poses come from `motion` (open, close, folder change).

use std::time::Instant;

use gpui::{
    Context, Div, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, ScrollEvent, Window,
    div, prelude::*, px, rgba, svg,
};

use super::MenuView;
use super::dial::{At, FAN_GAP, FanMode, PILL_GAP, PILL_H, fan_diameter, number};
use super::motion::{self, Pose};
use super::paint::{content, ease_out_cubic, edged, fade, keep_on_screen, soft_shadow};
use crate::geometry::bubble_center;
use crate::icon::Icon;
use crate::menu::{Colors, Item, ItemAction, Labels, Layout};
use crate::state::{LevelState, Ring};

/// Glyphs of the ring's other bubbles while the fan is open (the ring is inactive).
const FAINT: f32 = 0.35;

/// Centre glyphs (drawn for Detent, single-colour masks).
const CLOSE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6.5 6.5l11 11M17.5 6.5l-11 11" stroke="#000" stroke-width="2.6" stroke-linecap="round" fill="none"/></svg>"##;
const BACK_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M19 12H5.5M11.5 5.5L5 12l6.5 6.5" stroke="#000" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>"##;

/// One frame's geometry: the final ring centre, the ring's pose (radius / bubble size /
/// opacity in px), the selected bubble's own pose while it travels in, the grow progress
/// (labels, pill and fan wait for 1), the centre disc's scale, the viewport.
#[derive(Clone, Copy)]
struct Geo {
    c: (f32, f32),
    r: f32,
    d: f32,
    o: f32,
    p: f32,
    sel: Option<(usize, Pose)>,
    hub: f32,
    flash: bool,
    n: usize,
    vp: (f32, f32),
}

/// The time-dependent part of a frame (`MenuView::frame`).
struct Frame {
    pose: Pose,
    p: f32,
    sel: Option<(usize, Pose)>,
    hub: f32,
    flash: bool,
    /// The previous ring collapsing: its eased progress 0 → 1.
    leaving: Option<f32>,
}

impl Render for MenuView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let mut root = self.root(cx);
        if let Some(b) = self.backdrop {
            root = root.bg(rgba(b));
        }
        // Without `kde.cursor`, the pointer position comes from the compositor's `enter`.
        let mouse = window.mouse_position();
        if self.center.is_none() && (mouse.x > px(0.) || mouse.y > px(0.)) {
            self.center = Some(mouse);
        }
        let debug = std::env::var_os("DETENT_RADIAL_DEBUG").is_some();
        // The first frame can come before the compositor's configure (0×0): wait for it.
        let (Some(center), true) = (self.center, viewport.width > px(1.)) else {
            if debug {
                eprintln!("detent-radial: cursor position not known yet; not drawing");
            }
            window.request_animation_frame();
            return root;
        };
        let f = self.frame(window);
        if debug {
            eprintln!(
                "detent-radial: frame center {center:?} viewport {viewport:?} p {:.2}",
                f.p
            );
        }
        let Layout { ring, bubble, .. } = self.layout;
        let vp = (f32::from(viewport.width), f32::from(viewport.height));
        // Near an edge, shift the ring so all of it is visible: its labels, and the
        // adjustable item's fan and pill.
        let label_room = if self.labels == Labels::Never {
            0.
        } else {
            34.
        };
        let fan_room = if self.gauge.is_some() {
            FAN_GAP + fan_diameter(bubble) + PILL_GAP + PILL_H + 4.
        } else {
            0.
        };
        let c = keep_on_screen(
            (f32::from(center.x), f32::from(center.y)),
            ring + bubble / 2. + 8. + f32::max(label_room, fan_room),
            vp,
        );
        // Hit tests use the final geometry (not the animated one).
        self.drawn = Some(c);
        let geo = Geo {
            c,
            r: ring * f.pose.r,
            d: bubble * f.pose.d,
            o: f.pose.o,
            p: f.p,
            sel: f.sel,
            hub: f.hub,
            flash: f.flash,
            n: self.ring.items.len().max(1),
            vp,
        };
        if let (Some(q), Some((old, hl, _))) = (f.leaving, self.leaving.clone()) {
            root = self.ghost(root, &old, hl, q, &geo);
        }
        root = root.child(self.centre(&geo));
        root = self.bubbles(root, &geo);
        root = self.gauge_layer(root, window, &geo);
        root = self.fan_leaving_layer(root, window, &geo);
        // Hovering, tooltip not shown yet: draw again until it is due.
        if self.labels == Labels::Hover && self.highlighted.is_some() && !self.tooltip_due() {
            window.request_animation_frame();
        }
        root
    }
}

impl MenuView {
    /// Where this frame is in the open / folder-change / close transitions; asks for the
    /// next frame while one is running.
    fn frame(&mut self, window: &mut Window) -> Frame {
        let ms = |since: Instant| since.elapsed().as_secs_f32() * 1000.;
        if let Some((at, selected)) = self.closing {
            let t = ms(at) / motion::close_ms(self.animation_ms).max(1) as f32;
            if t < 4. / 3. {
                window.request_animation_frame();
            }
            let q = motion::ease_in_cubic(t);
            let sel = self.highlighted.filter(|_| selected).map(|i| {
                (
                    i,
                    if t < 1. {
                        motion::chosen(q)
                    } else {
                        Pose {
                            o: 0.,
                            ..motion::chosen(1.)
                        }
                    },
                )
            });
            let (hub, flash) = motion::hub_closing(t, selected);
            return Frame {
                pose: motion::collapsing(q),
                p: 1. - q,
                sel,
                hub,
                flash,
                leaving: None,
            };
        }
        let started = *self.shown_at.get_or_insert_with(Instant::now);
        let t = if self.animation_ms == 0 {
            1.0
        } else {
            ms(started) / self.animation_ms as f32
        };
        let p = ease_out_cubic(t);
        // The previous ring collapsing (folder change); the centre dips meanwhile.
        let leaving = self.leaving.as_ref().and_then(|(_, _, at)| {
            let tl = ms(*at) / motion::out_ms(self.animation_ms).max(1) as f32;
            (tl < 1.).then(|| motion::ease_in_cubic(tl))
        });
        if leaving.is_none() {
            self.leaving = None;
        }
        if t < 1.0 || leaving.is_some() {
            window.request_animation_frame();
        }
        let hub = match leaving {
            Some(q) => 1. - 0.18 * q,
            None if self.swapped => 0.82 + 0.18 * p,
            None => 0.6 + 0.4 * p,
        };
        Frame {
            pose: motion::opening(p),
            p,
            sel: None,
            hub,
            flash: false,
            leaving,
        }
    }

    /// The full-output root that takes the pointer, wheel and keys.
    fn root(&self, cx: &mut Context<Self>) -> Div {
        let mut root = div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, _, _| this.on_key(e)))
            .on_mouse_move(cx.listener(|this, e: &MouseMoveEvent, _, cx| this.on_move(e, cx)));
        for b in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            root = root.on_mouse_down(
                b,
                cx.listener(|this, e: &MouseDownEvent, _, _| this.on_down(e)),
            );
        }
        root.on_scroll(cx.listener(|this, e: &ScrollEvent, _, _| this.on_scroll(e)))
    }

    fn fan_shown(&self) -> bool {
        self.gauge.as_ref().is_some_and(|g| g.fan)
    }

    /// The centre disc: close, or back (inside a folder, or from the fan). It switches
    /// glyph when the previous ring has gone into it; on a selection it takes the accent
    /// as the chosen bubble arrives.
    fn centre(&self, geo: &Geo) -> Div {
        let c = self.colors;
        let cd = self.layout.center * geo.hub.max(0.);
        let back = match &self.leaving {
            Some((old, _, _)) => old.back,
            None => self.ring.back || self.fan_shown(),
        };
        let (fill, glyph) = if geo.flash {
            (c.accent, c.accent_icon)
        } else {
            (c.center, c.center_icon)
        };
        let alpha = if self.closing.is_some() || self.swapped {
            1.
        } else {
            geo.p.max(0.05)
        };
        // Centred on the ring centre: the disc is `cd` square at `c - cd / 2`, and its
        // glyph is centred inside it by flex.
        edged(
            div()
                .absolute()
                .left(px(geo.c.0 - cd / 2.))
                .top(px(geo.c.1 - cd / 2.))
                .size(px(cd))
                .rounded_full()
                .bg(rgba(fill))
                .opacity(alpha)
                .shadow(soft_shadow(&c, geo.o))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .data(if back { BACK_SVG } else { CLOSE_SVG })
                        .size(px(cd * 0.42))
                        .text_color(rgba(glyph)),
                ),
            &c,
        )
    }

    /// This bubble's centre, diameter and opacity in this frame (the selected one has its
    /// own pose while it travels into the centre).
    fn place(&self, i: usize, geo: &Geo) -> ((f32, f32), f32, f32) {
        let Layout { ring, bubble, .. } = self.layout;
        match geo.sel {
            Some((s, pose)) if s == i => (
                bubble_center(i, geo.n, ring * pose.r, geo.c),
                bubble * pose.d,
                pose.o,
            ),
            _ => (bubble_center(i, geo.n, geo.r, geo.c), geo.d, geo.o),
        }
    }

    fn bubbles(&self, mut root: Div, geo: &Geo) -> Div {
        let c = self.colors;
        let gauge = self
            .gauge
            .as_ref()
            .filter(|g| g.item < self.ring.items.len());
        let fan = gauge.is_some_and(|g| g.fan);
        let dial_open = gauge.is_some_and(|g| g.open);
        for (i, item) in self.ring.items.iter().enumerate() {
            let ((bx, by), d, o) = self.place(i, geo);
            let a = i as f32 * std::f32::consts::TAU / geo.n as f32;
            let dir = (a.sin(), -a.cos());
            let hl = self.highlighted == Some(i);
            let is_gauge = gauge.is_some_and(|g| g.item == i);
            let accent = item.accent.unwrap_or(c.accent);
            let fill = if hl {
                accent
            } else {
                item.color.unwrap_or(c.bubble)
            };
            let mut glyph = if hl { c.accent_icon } else { c.icon };
            if fan && !is_gauge {
                glyph = fade(glyph, FAINT);
            }
            let level = gauge.filter(|_| is_gauge).and_then(|g| match &g.level {
                LevelState::Ready(l) => Some(l),
                _ => None,
            });
            let inner = match level {
                // Muted: the provider's muted glyph in place of the item's.
                Some(l) if l.muted => match &item.action {
                    ItemAction::Adjust(a) if a.muted_icon.is_some() => content(
                        &Item {
                            icon: a.muted_icon.clone().unwrap_or(Icon::None),
                            ..item.clone()
                        },
                        d,
                        glyph,
                    ),
                    _ => content(item, d, glyph),
                },
                // The open dial: the value as a number.
                Some(l) if dial_open => number(l.percent, d, glyph),
                _ => content(item, d, glyph),
            };
            let mut b = disc((bx, by), d, fill, o, &c).child(inner);
            if matches!(item.icon, Icon::Photo(_)) && hl {
                // A photo covers the fill: mark the highlight with an accent ring instead.
                b = b.border_3().border_color(rgba(accent));
            }
            root = root.child(b);
            // The open dial's pill names what it adjusts; the fan has its own.
            let own_pill = fan || (is_gauge && dial_open);
            let show_label = match self.labels {
                Labels::Never => false,
                Labels::Hover => hl && self.tooltip_due() && !own_pill,
                Labels::Always => !own_pill,
            };
            if show_label && geo.p >= 1.0 {
                root = root.child(label(item, (bx, by), dir, d, geo.vp, c, hl));
            }
        }
        root
    }

    /// The previous ring going into the centre after a folder change (`q` 0 → 1): plain
    /// bubbles, its highlight kept, no dial or labels.
    fn ghost(&self, mut root: Div, old: &Ring, hl: Option<usize>, q: f32, geo: &Geo) -> Div {
        let c = self.colors;
        let pose = motion::collapsing(q);
        let Layout { ring, bubble, .. } = self.layout;
        let (r, d) = (ring * pose.r, bubble * pose.d);
        let n = old.items.len().max(1);
        for (i, item) in old.items.iter().enumerate() {
            let on = hl == Some(i);
            let fill = if on {
                item.accent.unwrap_or(c.accent)
            } else {
                item.color.unwrap_or(c.bubble)
            };
            let glyph = if on { c.accent_icon } else { c.icon };
            let b = disc(bubble_center(i, n, r, geo.c), d, fill, pose.o, &c);
            root = root.child(b.child(content(item, d, glyph)));
        }
        root
    }

    /// Where the adjustable item `i` is drawn this frame.
    fn item_at(&self, i: usize, geo: &Geo) -> At {
        let a = i as f32 * std::f32::consts::TAU / geo.n as f32;
        At {
            b: self.place(i, geo).0,
            dir: (a.sin(), -a.cos()),
            a,
        }
    }

    /// The adjustable item's dial, and its pill or fan once the ring has grown.
    fn gauge_layer(&mut self, mut root: Div, window: &mut Window, geo: &Geo) -> Div {
        let Some(g) = self
            .gauge
            .clone()
            .filter(|g| g.item < self.ring.items.len())
        else {
            self.pill_rect = None;
            self.fan_centers.clear();
            return root;
        };
        let at = self.item_at(g.item, geo);
        let item = self.ring.items[g.item].clone();
        let engaged = self.highlighted == Some(g.item) || g.pinned || g.fan;
        let accent = item.accent.unwrap_or(self.colors.accent);
        let d = self.place(g.item, geo).1;
        root = root.child(self.dial(&g, &at, d, geo.o, engaged, accent));
        if geo.p < 1.0 || !(g.open || g.fan) {
            self.pill_rect = None;
            self.fan_centers.clear();
            return root;
        }
        if g.fan {
            self.pill_rect = None;
            self.fan(window, root, &g, &at, (geo.c, geo.vp), FanMode::Live)
        } else {
            self.fan_centers.clear();
            let pill = self.gauge_pill(window, &item, &g, &at, self.layout.bubble, geo.vp);
            root.child(pill)
        }
    }

    /// The fan going back into its bubble after it closed (an output picked, the dial left).
    fn fan_leaving_layer(&mut self, root: Div, window: &mut Window, geo: &Geo) -> Div {
        let Some((g, at0)) = self.fan_leaving.clone() else {
            return root;
        };
        let out = motion::out_ms(self.animation_ms).max(1) as f32;
        let t = at0.elapsed().as_secs_f32() * 1000. / out;
        if t >= 1. || g.item >= self.ring.items.len() || self.closing.is_some() {
            self.fan_leaving = None;
            return root;
        }
        let at = self.item_at(g.item, geo);
        let q = 1. - motion::ease_in_cubic(t);
        self.fan(window, root, &g, &at, (geo.c, geo.vp), FanMode::Leaving(q))
    }
}

/// A round bubble of diameter `d` centred on `(x, y)`, its content centred inside.
fn disc((x, y): (f32, f32), d: f32, fill: u32, o: f32, c: &Colors) -> Div {
    edged(
        div()
            .absolute()
            .left(px(x - d / 2.))
            .top(px(y - d / 2.))
            .size(px(d))
            .rounded_full()
            .bg(rgba(fill))
            .opacity(o.clamp(0., 1.))
            .shadow(soft_shadow(c, o))
            .flex()
            .items_center()
            .justify_center(),
        c,
    )
}

/// A label pill outside its bubble, on the side facing away from the centre.
fn label(
    item: &Item,
    (bx, by): (f32, f32),
    (dx, dy): (f32, f32),
    d: f32,
    (vw, _vh): (f32, f32),
    c: Colors,
    hl: bool,
) -> impl IntoElement {
    const W: f32 = 220.;
    const H: f32 = 26.;
    const GAP: f32 = 8.;
    let pill = div()
        .px_2()
        .py_0p5()
        .rounded_md()
        .bg(rgba(c.label_bg))
        .shadow(soft_shadow(&c, 1.0))
        .text_size(px(12.))
        .text_color(rgba(c.label_text))
        .when(hl, |p| p.font_weight(gpui::FontWeight::MEDIUM))
        .child(item.label.clone());
    let slot = div().absolute().w(px(W)).h(px(H)).flex().items_center();
    if dx > 0.35 {
        slot.left(px(bx + d / 2. + GAP))
            .top(px(by - H / 2.))
            .justify_start()
            .child(pill)
    } else if dx < -0.35 {
        slot.right(px(vw - (bx - d / 2. - GAP)))
            .top(px(by - H / 2.))
            .justify_end()
            .child(pill)
    } else {
        let top = if dy < 0. {
            by - d / 2. - GAP - H
        } else {
            by + d / 2. + GAP
        };
        slot.left(px(bx - W / 2.))
            .top(px(top))
            .justify_center()
            .child(pill)
    }
}
