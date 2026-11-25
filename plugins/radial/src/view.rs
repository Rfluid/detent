//! The overlay: a full-output, transparent wlr-layer-shell surface on the output under the
//! cursor (overlay layer). It takes the keyboard while open (Esc / Tab / Enter, as on the
//! Actions Ring) and receives the real pointer: the cursor moves freely, the bubble under
//! it is highlighted, clicks select. Hover, clicks and keys go to the state machine
//! through `Emit`; what is drawn comes back as `Ring` / `highlighted`.
//!
//! Look (the Actions Ring): separate round bubbles on a circle around the cursor, no
//! backdrop; pale bubbles with single-colour glyphs; the highlighted bubble fills with the
//! accent and its glyph turns white; a small centre disc closes (×) or, inside a folder,
//! goes back (←). Item 0 is at 12 o'clock, clockwise (`geometry`).
//!
//! Motion (`motion`): everything comes out of the centre and goes back into it. On open
//! the bubbles grow outward; on close they collapse into the centre, then it closes; a
//! selected bubble travels into the centre, which takes it in; a folder change collapses
//! the old ring before the new one grows; the fan goes back into its bubble. While
//! closing, the surface takes no pointer input (empty input region) and ignores keys.
//!
//! Position: centred on the cursor. From `kde.cursor` when available (`At`), else from the
//! compositor's pointer `enter` / first motion. Until the position is known nothing is
//! drawn: the ring never appears anywhere but around the cursor.
//!
//! Adjustable item (volume): a 270° dial around its bubble shows the level; hovered, the
//! bubble shows the number and a pill outside the ring names the output, and opens the fan
//! of outputs (`dial`). The wheel / thumbwheel arrive here as `wl_pointer.axis` (Detent
//! never diverts wheels), are turned into whole notches and sent as `Input::Scroll`.
//!
//! Layout: `render` draws the ring, `dial` the adjustable item, `paint` holds the shared
//! drawing helpers, `motion` the transition timing, `window` opens the surface.
//!
//! Theme: `theme = "system"` follows the desktop's light/dark preference, which GPUI reads
//! from the XDG portal (`org.freedesktop.appearance color-scheme`) and keeps current.

mod dial;
mod motion;
mod paint;
mod render;
#[cfg(test)]
mod tests;
mod window;

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    Context, FocusHandle, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point,
    ScrollDelta, ScrollEvent,
};

use crate::geometry::{Hit, hit};
use crate::menu::{Colors, Labels, Layout};
use crate::state::{Gauge, Input, Key, Ring, Target};

pub use motion::{Close, close_total_ms};
pub use window::{open, warm_up};

/// Where the overlay sends hover / click / key inputs (to the state machine).
pub type Emit = Rc<dyn Fn(Input)>;

/// Extra px around each bubble / the centre that still counts as "on" it.
const HIT_SLOP: f32 = 3.;
/// GPUI on Linux reports one wheel notch as 3 lines (`SCROLL_LINES`; high-resolution
/// wheels send fractions of it, `value120 / 120 × 3`).
const LINES_PER_NOTCH: f32 = 3.;
/// Smooth scrolling (touchpads): pixels per notch.
const PX_PER_NOTCH: f32 = 50.;

pub struct MenuView {
    ring: Ring,
    highlighted: Option<usize>,
    /// When the current highlight began (tooltip delay).
    highlighted_at: Instant,
    layout: Layout,
    colors: Colors,
    labels: Labels,
    tooltip_ms: u32,
    animation_ms: u32,
    /// First frame of the current ring (open / folder change), for the grow animation. In
    /// the future while the previous ring is still collapsing (`leaving`).
    shown_at: Option<Instant>,
    /// The previous ring collapsing into the centre after a folder change: (ring, its
    /// highlighted bubble, when it started).
    leaving: Option<(Ring, Option<usize>, Instant)>,
    /// The current ring replaced another (folder change): the centre dips and recovers
    /// instead of growing from small.
    swapped: bool,
    /// The overlay is closing: (when, the highlighted bubble travels into the centre).
    closing: Option<(Instant, bool)>,
    /// The cursor position the ring is centred on (`None` = not known yet: draw nothing).
    center: Option<Point<Pixels>>,
    /// The centre actually drawn (after keeping the ring on screen), for hit tests.
    drawn: Option<(f32, f32)>,
    /// Last hover sent, to emit only changes.
    hover: Option<usize>,
    /// The adjustable item's gauge (from the state machine).
    gauge: Option<Gauge>,
    /// The adjustable item's pill as drawn (x, y, w, h): hovering it counts as hovering its
    /// bubble; clicking it opens the fan.
    pill_rect: Option<(f32, f32, f32, f32)>,
    /// The cursor is over the pill (accent edge when it opens a fan).
    pill_hot: bool,
    /// When the fan opened (grow animation).
    fan_opened_at: Option<Instant>,
    /// The fan going back into its bubble: (the gauge it belonged to, when it started).
    fan_leaving: Option<(Gauge, Instant)>,
    /// The fan bubbles as drawn (centres, diameter), for hit tests.
    fan_centers: Vec<(f32, f32)>,
    fan_d: f32,
    /// Last fan hover sent.
    hover_choice: Option<usize>,
    /// Scroll not yet amounting to a whole notch: (horizontal, vertical).
    scroll_acc: (f32, f32),
    focus: FocusHandle,
    emit: Emit,
    /// Preview only: paint the whole output with this colour behind the ring.
    pub backdrop: Option<u32>,
}

impl MenuView {
    /// Enter / leave a folder: new items, nothing highlighted. The old ring collapses into
    /// the centre, then the new one grows out of it. Hit tests use the new ring at once.
    pub fn navigate(&mut self, ring: Ring) {
        let out = motion::out_ms(self.animation_ms);
        let old = std::mem::replace(&mut self.ring, ring);
        let now = Instant::now();
        if out > 0 && self.shown_at.is_some() {
            self.leaving = Some((old, self.highlighted, now));
            self.shown_at = Some(now + Duration::from_millis(u64::from(out)));
            self.swapped = true;
        } else {
            self.shown_at = None;
        }
        self.set_highlight(None);
        self.hover = None;
        self.pill_rect = None;
        self.fan_centers.clear();
        self.fan_leaving = None;
    }

    /// Start closing: collapse into the centre (`selected`: the highlighted bubble travels
    /// into it). The caller removes the surface after `close_total_ms`.
    pub fn start_close(&mut self, selected: bool) {
        self.closing = Some((Instant::now(), selected && self.highlighted.is_some()));
        self.pill_rect = None;
        self.fan_centers.clear();
    }

    pub fn set_gauge(&mut self, g: Option<Gauge>) {
        let was_fan = self.gauge.as_ref().is_some_and(|g| g.fan);
        let fan = g.as_ref().is_some_and(|g| g.fan);
        let open = g.as_ref().is_some_and(|g| g.open);
        if fan && !was_fan {
            self.fan_opened_at = None;
            self.fan_leaving = None;
        }
        if was_fan && !fan && self.animation_ms > 0 {
            self.fan_leaving = self.gauge.clone().map(|old| (old, Instant::now()));
        }
        if !fan {
            self.fan_centers.clear();
            self.hover_choice = None;
        }
        if !open || fan {
            self.pill_rect = None;
            self.pill_hot = false;
        }
        self.gauge = g;
    }

    /// From the state machine (hover or Tab). Restarts the tooltip delay on change.
    pub fn set_highlight(&mut self, h: Option<usize>) {
        if h != self.highlighted {
            self.highlighted = h;
            self.highlighted_at = Instant::now();
        }
    }

    /// Is the tooltip of the highlighted bubble due? (`labels = "hover"`)
    fn tooltip_due(&self) -> bool {
        self.highlighted.is_some()
            && self.highlighted_at.elapsed().as_millis() >= u128::from(self.tooltip_ms)
    }

    fn hit(&self, p: Point<Pixels>) -> Hit {
        let Some(c) = self.drawn else {
            return Hit::Outside;
        };
        let (px_, py_) = (f32::from(p.x), f32::from(p.y));
        let r2 = (self.fan_d / 2. + HIT_SLOP).powi(2);
        if let Some(j) = self
            .fan_centers
            .iter()
            .position(|(x, y)| (px_ - x).powi(2) + (py_ - y).powi(2) <= r2)
        {
            return Hit::Choice(j);
        }
        if let Some((x, y, w, h)) = self.pill_rect
            && px_ >= x
            && px_ <= x + w
            && py_ >= y
            && py_ <= y + h
        {
            return Hit::Pill;
        }
        let l = self.layout;
        hit(
            (px_, py_),
            c,
            self.ring.items.len(),
            l.ring,
            l.bubble,
            l.center,
            HIT_SLOP,
        )
    }

    fn on_move(&mut self, e: &MouseMoveEvent, cx: &mut Context<Self>) {
        if self.closing.is_some() {
            return;
        }
        if self.center.is_none() {
            // No `kde.cursor` reply and no `enter`: the first motion tells us.
            self.center = Some(e.position);
            cx.notify();
            return;
        }
        let (h, choice, pill) = match self.hit(e.position) {
            Hit::Item(i) => (Some(i), None, false),
            // The pill belongs to its bubble: hovering it keeps the dial open.
            Hit::Pill => (self.gauge.as_ref().map(|g| g.item), None, true),
            Hit::Choice(j) => (None, Some(j), false),
            Hit::Center | Hit::Outside => (None, None, false),
        };
        if pill != self.pill_hot {
            self.pill_hot = pill;
            cx.notify();
        }
        if h != self.hover {
            self.hover = h;
            (self.emit)(Input::Hover(h));
        }
        if choice != self.hover_choice {
            self.hover_choice = choice;
            (self.emit)(Input::HoverChoice(choice));
        }
    }

    fn on_down(&mut self, e: &MouseDownEvent) {
        if self.closing.is_some() {
            return;
        }
        let target = match self.hit(e.position) {
            Hit::Item(i) => Target::Item(i),
            Hit::Center => Target::Center,
            Hit::Outside => Target::Outside,
            Hit::Pill => Target::Pill,
            Hit::Choice(j) => Target::Choice(j),
        };
        match e.button {
            MouseButton::Left => (self.emit)(Input::Click { left: true, target }),
            MouseButton::Right => (self.emit)(Input::Click {
                left: false,
                target,
            }),
            MouseButton::Middle => (self.emit)(Input::Middle(target)),
            _ => {}
        }
    }

    /// Wheel → whole notches (positive = up / louder): vertical = main wheel, horizontal =
    /// thumbwheel. Fractions (high-resolution wheels, touchpads) add up across events.
    fn on_scroll(&mut self, e: &ScrollEvent) {
        if self.closing.is_some() {
            return;
        }
        let (dx, dy) = match e.delta {
            ScrollDelta::Lines(p) => (p.x / LINES_PER_NOTCH, p.y / LINES_PER_NOTCH),
            ScrollDelta::Pixels(p) => {
                (f32::from(p.x) / PX_PER_NOTCH, f32::from(p.y) / PX_PER_NOTCH)
            }
        };
        let (fine, coarse) = notches(&mut self.scroll_acc, (dx, dy));
        if coarse != 0 || fine != 0 {
            (self.emit)(Input::Scroll { coarse, fine });
        }
    }

    fn on_key(&mut self, e: &KeyDownEvent) {
        if self.closing.is_some() {
            return;
        }
        let k = &e.keystroke;
        let input = match k.key.as_str() {
            "escape" => Input::Key(Key::Escape),
            "tab" if k.modifiers.shift => Input::Key(Key::BackTab),
            "tab" => Input::Key(Key::Tab),
            "enter" | "space" => Input::Key(Key::Select),
            // Arrows step the highlighted / pinned gauge like the wheel.
            "up" => Input::Scroll { coarse: 1, fine: 0 },
            "down" => Input::Scroll {
                coarse: -1,
                fine: 0,
            },
            _ => return,
        };
        (self.emit)(input);
    }
}

/// Add `(dx, dy)` to the accumulator and take the whole notches out of it.
fn notches(acc: &mut (f32, f32), (dx, dy): (f32, f32)) -> (i32, i32) {
    acc.0 += dx;
    acc.1 += dy;
    // A little tolerance so 8 × 0.125 counts as one notch despite float error.
    let take = |a: &mut f32| {
        let n = (*a + a.signum() * 1e-4).trunc();
        *a -= n;
        n as i32
    };
    (take(&mut acc.0), take(&mut acc.1))
}
