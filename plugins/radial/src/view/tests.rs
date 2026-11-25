use std::f32::consts::PI;

use super::dial::{
    DIAL_START, DIAL_SWEEP, FAN_GAP, PILL_GAP, PILL_H, fan_centers, pill_rect, tick_angles,
};
use super::notches;
use super::paint::{arc, ease_out_cubic, fade, initials, keep_on_screen, on_circle, ticks};

#[test]
fn initials_and_easing() {
    assert_eq!(initials("next desktop"), "ND");
    assert_eq!(initials("Copy"), "C");
    assert_eq!(initials(""), "");
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(2.0), 1.0);
    assert!(ease_out_cubic(0.5) > 0.5);
}

#[test]
fn scroll_becomes_whole_notches() {
    let mut acc = (0., 0.);
    // a high-resolution wheel: 8 events of 1/8 notch each = one notch
    let mut total = 0;
    for _ in 0..8 {
        total += notches(&mut acc, (0., 0.125)).1;
    }
    assert_eq!(total, 1);
    assert_eq!(notches(&mut acc, (-1., -2.)), (-1, -2));
    assert_eq!(notches(&mut acc, (0.4, 0.)), (0, 0));
    assert_eq!(notches(&mut acc, (0.7, 0.)), (1, 0));
}

#[test]
fn the_pill_goes_outward_from_the_ring() {
    let vp = (1000., 800.);
    let (b, w) = (48., 160.);
    let off = b / 2. + PILL_GAP;
    // top bubble: above it, centred
    assert_eq!(
        pill_rect((500., 316.), (0., -1.), b, w, vp),
        (420., 316. - off - PILL_H, w, PILL_H)
    );
    // right / left / bottom
    assert_eq!(pill_rect((584., 400.), (1., 0.), b, w, vp).0, 584. + off);
    assert_eq!(
        pill_rect((416., 400.), (-1., 0.), b, w, vp).0,
        416. - off - w
    );
    assert_eq!(pill_rect((500., 484.), (0., 1.), b, w, vp).1, 484. + off);
    // near the left edge it stays on screen
    assert_eq!(pill_rect((60., 400.), (-1., 0.), b, w, vp).0, 4.);
}

#[test]
fn the_fan_sits_outside_the_ring_centred_on_the_item() {
    let c = (500., 400.);
    let (ring, bubble, fd) = (84., 48., 37.);
    let r = ring + bubble / 2. + FAN_GAP + fd / 2.;
    let close = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01;
    // one: straight out from the item (here at 12 o'clock)
    let one = fan_centers(1, 0., ring, bubble, fd, c);
    assert!(close(one[0], (500., 400. - r)));
    // three: symmetric around it, left to right, not overlapping
    let three = fan_centers(3, 0., ring, bubble, fd, c);
    assert!(close(three[1], (500., 400. - r)));
    assert!(three[0].0 < three[1].0 && three[1].0 < three[2].0);
    assert!((three[0].0 - 500.).abs() - (three[2].0 - 500.).abs() < 0.01);
    let gap = ((three[1].0 - three[0].0).powi(2) + (three[1].1 - three[0].1).powi(2)).sqrt();
    assert!(gap >= fd + 9.9, "bubbles overlap: {gap}");
    // an item at 3 o'clock: the fan is to its right
    let right = fan_centers(1, PI / 2., ring, bubble, fd, c);
    assert!(close(right[0], (500. + r, 400.)));
}

#[test]
fn dial_geometry() {
    // 0 at 7:30, the maximum at 4:30, clockwise through 12
    assert!(close_pt(
        on_circle((0., 0.), 10., DIAL_START),
        (-7.071, 7.071)
    ));
    assert!(close_pt(
        on_circle((0., 0.), 10., DIAL_START + DIAL_SWEEP),
        (7.071, 7.071)
    ));
    assert!(close_pt(
        on_circle((0., 0.), 10., DIAL_START + DIAL_SWEEP / 2.),
        (0., -10.)
    ));
    // a tick every 10 of the maximum, first and last at the ends
    let t = tick_angles(100);
    assert_eq!(t.len(), 11);
    assert!((t[0] - DIAL_START).abs() < 1e-6);
    assert!((t[10] - (DIAL_START + DIAL_SWEEP)).abs() < 1e-6);
    assert_eq!(tick_angles(150).len(), 16);
}

fn close_pt(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

#[test]
fn arcs_and_ticks() {
    assert!(arc((0., 0.), 10., 0., 0., 2.).is_none());
    assert!(arc((0., 0.), 10., DIAL_START, DIAL_SWEEP * 0.5, 2.).is_some());
    assert!(arc((0., 0.), 10., 0., 2. * PI, 2.).is_some());
    assert!(ticks((0., 0.), 10., 12., &[], 1.).is_none());
    assert!(ticks((0., 0.), 10., 12., &tick_angles(100), 1.).is_some());
}

#[test]
fn fading_keeps_the_colour() {
    assert_eq!(fade(0x1c1b1fff, 0.5), 0x1c1b1f80);
    assert_eq!(fade(0xffffff00, 0.5), 0xffffff00);
}

#[test]
fn ring_stays_on_screen() {
    let vp = (1000., 800.);
    assert_eq!(keep_on_screen((500., 400.), 100., vp), (500., 400.));
    assert_eq!(keep_on_screen((990., 790.), 100., vp), (900., 700.));
    assert_eq!(keep_on_screen((5., 5.), 100., vp), (100., 100.));
    // viewport smaller than the ring: centre it
    assert_eq!(keep_on_screen((5., 5.), 600., vp), (500., 400.));
}
