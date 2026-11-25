//! Transition timing (design: Nexus page `pag_OkXg1xaVPSlQ`, option B "collapse into the
//! centre"): everything comes out of the centre and goes back into it. Opening grows out
//! (ease-out, `animation_ms`); closing, selecting and leaving a ring collapse in (ease-in,
//! shorter, because the person has already decided). Pure functions, so they are tested.

use crate::menu::Step;
use crate::state::Effect;

/// A bubble's pose relative to the settled ring: radius factor, size factor, opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Pose {
    pub r: f32,
    pub d: f32,
    pub o: f32,
}

pub(super) fn ease_in_cubic(t: f32) -> f32 {
    t.clamp(0., 1.).powi(3)
}

/// Opening, `p` = eased progress 0 → 1: out of the centre.
pub(super) fn opening(p: f32) -> Pose {
    Pose {
        r: 0.45 + 0.55 * p,
        d: 0.6 + 0.4 * p,
        o: p,
    }
}

/// Leaving, `q` = eased progress 0 → 1: into the centre, shrinking and fading.
pub(super) fn collapsing(q: f32) -> Pose {
    Pose {
        r: 1. - q,
        d: 1. - 0.7 * q,
        o: 1. - q,
    }
}

/// The selected bubble while it travels into the centre: stays solid until absorbed.
pub(super) fn chosen(q: f32) -> Pose {
    Pose {
        r: 1. - q,
        d: 1. - 0.3 * q,
        o: 1.,
    }
}

/// The ring collapsing on close (`animation_ms` 160 → 120).
pub(super) fn close_ms(animation_ms: u32) -> u32 {
    animation_ms * 3 / 4
}

/// The ring collapsing before a folder's (or the parent's) ring grows (160 → 100); also
/// the fan going back into its bubble.
pub(super) fn out_ms(animation_ms: u32) -> u32 {
    animation_ms * 5 / 8
}

/// How long the overlay stays up after a close: the bubbles' collapse, then the centre's
/// (it starts at ¾ of the collapse and ends ⅓ after it; the selection's hub flash ends
/// at the same time). 0 = close at once.
pub fn close_total_ms(animation_ms: u32) -> u32 {
    close_ms(animation_ms) * 4 / 3
}

/// The centre's scale while closing, `t` = time / `close_ms` (from 0 to 4/3). A plain close
/// shrinks it after the bubbles reached it; a selection first flashes it larger as it takes
/// the chosen bubble in (`flash`), then shrinks it.
pub(super) fn hub_closing(t: f32, selected: bool) -> (f32, bool) {
    if selected {
        if t < 1. {
            (1., false)
        } else {
            (1.18 * (1. - ease_in_cubic((t - 1.) * 3.)), true)
        }
    } else {
        (1. - ease_in_cubic((t - 0.75) * 2.), false)
    }
}

/// How the overlay goes away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Close {
    /// Remove the surface at once: a `keys` step needs the keyboard back, and a layer
    /// surface can't give up keyboard focus while mapped (`KeyboardInteractivity` is fixed
    /// at creation in GPUI).
    Now,
    /// Collapse into the centre (Esc, right click, outside, ×, trigger again).
    Collapse,
    /// The highlighted bubble travels into the centre, which takes it in.
    Select,
}

impl Close {
    /// For the effects of one input (the `Hide` and the selected item's `Run` come
    /// together).
    pub fn for_effects(fx: &[Effect]) -> Close {
        let run = fx.iter().find_map(|e| match e {
            Effect::Run(steps) => Some(steps),
            _ => None,
        });
        match run {
            Some(steps) if steps.iter().any(needs_focus) => Close::Now,
            Some(_) => Close::Select,
            None => Close::Collapse,
        }
    }
}

fn needs_focus(s: &Step) -> bool {
    matches!(s, Step::Invoke(a, _) if a == "keys")
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Pose {
        const SETTLED: Pose = Pose {
            r: 1.,
            d: 1.,
            o: 1.,
        };
    }

    fn run(action: &str) -> Effect {
        Effect::Run(vec![
            Step::DelayMs(10),
            Step::Invoke(action.into(), "{}".into()),
        ])
    }

    #[test]
    fn keys_close_at_once_other_selections_travel_in() {
        assert_eq!(Close::for_effects(&[Effect::Hide]), Close::Collapse);
        assert_eq!(
            Close::for_effects(&[Effect::Haptic(1), Effect::Hide, run("keys")]),
            Close::Now
        );
        assert_eq!(
            Close::for_effects(&[Effect::Hide, run("exec")]),
            Close::Select
        );
    }

    #[test]
    fn poses_start_and_end_where_the_design_says() {
        assert_eq!(opening(1.), Pose::SETTLED);
        assert_eq!(collapsing(0.), Pose::SETTLED);
        assert_eq!(chosen(0.), Pose::SETTLED);
        // Into the centre, small and gone.
        let end = collapsing(1.);
        assert_eq!((end.r, end.o), (0., 0.));
        assert!((end.d - 0.3).abs() < 1e-6);
        // The chosen bubble reaches the centre solid.
        assert_eq!((chosen(1.).r, chosen(1.).o), (0., 1.));
        assert_eq!(ease_in_cubic(0.5), 0.125);
        assert_eq!(ease_in_cubic(2.), 1.);
    }

    #[test]
    fn timings_follow_animation_ms() {
        assert_eq!(
            (close_ms(160), out_ms(160), close_total_ms(160)),
            (120, 100, 160)
        );
        assert_eq!(close_total_ms(0), 0);
    }

    #[test]
    fn the_centre_closes_last() {
        // Plain close: untouched until ¾, gone at 1¼.
        assert_eq!(hub_closing(0.5, false), (1., false));
        assert!(hub_closing(1.25, false).0.abs() < 1e-6);
        // Selection: the centre flashes as the bubble arrives, then shrinks away.
        assert_eq!(hub_closing(0.9, true), (1., false));
        assert_eq!(hub_closing(1., true), (1.18, true));
        assert!(hub_closing(4. / 3., true).0.abs() < 1e-5);
    }
}
