//! Ring geometry: where the bubbles are and what the cursor is over. Pure.
//!
//! Item 0 is at 12 o'clock; items go clockwise. Screen coordinates: +x right, +y down.
//! The view draws with `bubble_center` and hit-tests with `hit`, so what is highlighted is
//! exactly the bubble under the cursor (the Actions Ring model: hover a bubble, click it).

/// What a point on the overlay is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Item(usize),
    /// The centre disc (close, or back inside a folder).
    Center,
    Outside,
    /// The adjustable item's pill (the view's own hit test; `hit` never returns it).
    Pill,
    /// A fan bubble (the view's own hit test).
    Choice(usize),
}

/// Centre of bubble `i` of `n` on a ring of radius `ring` around `(cx, cy)`.
pub fn bubble_center(i: usize, n: usize, ring: f32, (cx, cy): (f32, f32)) -> (f32, f32) {
    let a = (i as f32) * std::f32::consts::TAU / n.max(1) as f32;
    (cx + a.sin() * ring, cy - a.cos() * ring)
}

/// Hit-test `(x, y)` against `n` bubbles of diameter `bubble` on a ring of radius `ring`
/// around `c`, and the centre disc of diameter `center`. `slop` widens every target a bit
/// (the gaps between bubbles stay dead space, as on the Actions Ring).
pub fn hit(
    (x, y): (f32, f32),
    c: (f32, f32),
    n: usize,
    ring: f32,
    bubble: f32,
    center: f32,
    slop: f32,
) -> Hit {
    let d2 = |(ax, ay): (f32, f32)| (x - ax).powi(2) + (y - ay).powi(2);
    if d2(c) <= (center / 2. + slop).powi(2) {
        return Hit::Center;
    }
    let r2 = (bubble / 2. + slop).powi(2);
    (0..n)
        .map(|i| (i, d2(bubble_center(i, n, ring, c))))
        .filter(|(_, d)| *d <= r2)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map_or(Hit::Outside, |(i, _)| Hit::Item(i))
}

#[cfg(test)]
mod tests {
    use super::*;

    const C: (f32, f32) = (500., 400.);

    fn at(x: f32, y: f32) -> Hit {
        hit((x, y), C, 4, 84., 48., 28., 4.)
    }

    #[test]
    fn bubbles_clockwise_from_top() {
        let close =
            |a: (f32, f32), b: (f32, f32)| (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01;
        assert!(close(bubble_center(0, 4, 84., C), (500., 316.)));
        assert!(close(bubble_center(1, 4, 84., C), (584., 400.)));
        assert!(close(bubble_center(2, 4, 84., C), (500., 484.)));
        assert!(close(bubble_center(3, 4, 84., C), (416., 400.)));
    }

    #[test]
    fn hover_targets() {
        assert_eq!(at(500., 316.), Hit::Item(0));
        assert_eq!(at(584. + 20., 400. + 10.), Hit::Item(1)); // inside the bubble edge
        assert_eq!(at(500., 400.), Hit::Center);
        assert_eq!(at(510., 395.), Hit::Center);
        // between the centre and a bubble, and between two bubbles: nothing
        assert_eq!(at(500., 360.), Hit::Outside);
        assert_eq!(at(560., 340.), Hit::Outside);
        // far away
        assert_eq!(at(900., 50.), Hit::Outside);
        assert_eq!(hit((1., 1.), C, 0, 84., 48., 28., 4.), Hit::Outside);
    }
}
