use super::gauge::crosses_ten;
use super::*;

mod fan;

fn radial() -> Radial {
    let json = r#"{"menus": {"main": {"items": [
            {"label": "Up", "action": "kde.overview"},
            {"label": "Right", "keys": "Ctrl+C"},
            {"label": "Down", "items": [
                {"label": "F-Up", "keys": "A"},
                {"label": "F-Down", "keys": "B"}
            ]},
            {"label": "Left", "keys": "Ctrl+V"}
        ], "apps": {"firefox": {"items": [{"label": "Tab", "keys": "Ctrl+W"}]}}}}}"#;
    Radial::new(Settings::parse(json).unwrap_or_else(|e| panic!("{e}")))
}

fn keys(k: &str) -> Effect {
    Effect::Run(vec![Step::Invoke(
        "keys".into(),
        format!(r#"{{"keys":"{k}"}}"#),
    )])
}

fn at() -> At {
    At {
        x: 300.,
        y: 200.,
        output: "DP-1".into(),
        size: None,
    }
}

fn thumb(press: bool) -> Input {
    Input::Button {
        control: "thumb".into(),
        press,
    }
}

fn click(target: Target) -> Input {
    Input::Click { left: true, target }
}

/// Thumb press → lease + locate → cursor reply → shown at the cursor.
fn open(r: &mut Radial) {
    let fx = r.handle(Input::Open {
        menu: "main".into(),
        control: Some("thumb".into()),
    });
    assert_eq!(
        fx,
        vec![
            Effect::RequestLease {
                control: "thumb".into()
            },
            Effect::LocateCursor { seq: r.seq }
        ]
    );
    r.handle(Input::LeaseGranted);
    let fx = r.handle(Input::Cursor(at()));
    assert!(
        matches!(&fx[0], Effect::Show { ring: Ring { back: false, items }, at: Some(a) } if items.len() == 4 && *a == at())
    );
}

#[test]
fn press_release_then_click_a_bubble() {
    let mut r = radial();
    open(&mut r);
    // quick release over nothing: the ring stays, the lease goes
    assert_eq!(r.handle(thumb(false)), vec![Effect::ReleaseLease]);
    assert!(r.is_open());
    // hover → highlight + tick; same bubble again → nothing
    assert_eq!(
        r.handle(Input::Hover(Some(1))),
        vec![Effect::Highlight(Some(1)), Effect::Haptic(4)]
    );
    assert!(r.handle(Input::Hover(Some(1))).is_empty());
    let fx = r.handle(click(Target::Item(1)));
    assert_eq!(fx, vec![Effect::Haptic(1), Effect::Hide, keys("Ctrl+C")]);
    assert!(!r.is_open());
}

#[test]
fn hold_and_release_on_a_bubble() {
    let mut r = radial();
    open(&mut r);
    r.handle(Input::Hover(Some(3)));
    let fx = r.handle(thumb(false));
    assert_eq!(
        fx,
        vec![
            Effect::Haptic(1),
            Effect::Hide,
            Effect::ReleaseLease,
            keys("Ctrl+V")
        ]
    );
}

#[test]
fn leaving_a_bubble_clears_the_highlight() {
    let mut r = radial();
    open(&mut r);
    r.handle(Input::Hover(Some(0)));
    assert_eq!(r.handle(Input::Hover(None)), vec![Effect::Highlight(None)]);
    // released between bubbles: stays open
    assert_eq!(r.handle(thumb(false)), vec![Effect::ReleaseLease]);
    assert!(r.is_open());
}

#[test]
fn closing() {
    for input in [
        Input::Key(Key::Escape),
        click(Target::Center),
        click(Target::Outside),
        Input::Click {
            left: false,
            target: Target::Item(0),
        },
    ] {
        let mut r = radial();
        open(&mut r);
        r.handle(thumb(false));
        assert_eq!(r.handle(input.clone()), vec![Effect::Hide], "{input:?}");
        assert!(!r.is_open());
    }
    // pressing the trigger again with nothing highlighted closes too
    let mut r = radial();
    open(&mut r);
    r.handle(thumb(false));
    let fx = r.handle(Input::Open {
        menu: "main".into(),
        control: Some("thumb".into()),
    });
    assert_eq!(fx, vec![Effect::Hide]);
}

#[test]
fn keyboard() {
    let mut r = radial();
    open(&mut r);
    r.handle(thumb(false));
    assert_eq!(
        r.handle(Input::Key(Key::Tab)),
        vec![Effect::Highlight(Some(0)), Effect::Haptic(4)]
    );
    r.handle(Input::Key(Key::Tab));
    assert_eq!(
        r.handle(Input::Key(Key::BackTab))[0],
        Effect::Highlight(Some(0))
    );
    assert_eq!(
        r.handle(Input::Key(Key::BackTab))[0],
        Effect::Highlight(Some(3))
    );
    let fx = r.handle(Input::Key(Key::Select));
    assert_eq!(fx.last(), Some(&keys("Ctrl+V")));
}

#[test]
fn folders_and_escape_back() {
    let mut r = radial();
    open(&mut r);
    r.handle(Input::Hover(Some(2)));
    let fx = r.handle(click(Target::Item(2)));
    assert_eq!(fx[0], Effect::Haptic(1));
    assert!(matches!(&fx[1], Effect::Navigate(Ring { back: true, items }) if items.len() == 2));
    // entering a folder drops the hold lease: selection inside is by click
    assert_eq!(fx[2], Effect::ReleaseLease);
    // Esc goes back to the main ring, a second Esc closes
    let fx = r.handle(Input::Key(Key::Escape));
    assert!(matches!(&fx[0], Effect::Navigate(Ring { back: false, items }) if items.len() == 4));
    assert_eq!(r.handle(Input::Key(Key::Escape)), vec![Effect::Hide]);
    // into the folder again; the centre (←) goes back too; pick an item
    open(&mut r);
    r.handle(click(Target::Item(2)));
    assert!(matches!(
        &r.handle(click(Target::Center))[0],
        Effect::Navigate(_)
    ));
    r.handle(click(Target::Item(2)));
    let fx = r.handle(click(Target::Item(1)));
    assert_eq!(fx.last(), Some(&keys("B")));
    assert!(!r.is_open());
}

#[test]
fn hold_into_a_folder_and_release_on_a_sub_bubble() {
    let mut r = radial();
    open(&mut r);
    r.handle(Input::Hover(Some(2)));
    r.handle(thumb(false)); // released on the folder: opens it
    assert!(r.is_open());
    r.handle(Input::Hover(Some(0)));
    assert_eq!(r.handle(click(Target::Item(0))).last(), Some(&keys("A")));
}

#[test]
fn no_cursor_reply_still_opens_without_guessing() {
    let mut r = radial();
    r.handle(Input::Open {
        menu: "main".into(),
        control: None,
    });
    let seq = r.seq;
    // a stale timeout from an older request is ignored
    assert!(
        r.handle(Input::LocateTimeout(seq.wrapping_sub(1)))
            .is_empty()
    );
    let fx = r.handle(Input::LocateTimeout(seq));
    assert!(matches!(&fx[0], Effect::Show { at: None, .. }));
    // a late reply after showing changes nothing
    assert!(r.handle(Input::Cursor(at())).is_empty());
}

#[test]
fn release_before_the_ring_appears() {
    let mut r = radial();
    r.handle(Input::Open {
        menu: "main".into(),
        control: Some("thumb".into()),
    });
    assert_eq!(r.handle(thumb(false)), vec![Effect::ReleaseLease]);
    r.handle(Input::Cursor(at()));
    // open, not holding: hovering + releasing nothing selects only on click
    r.handle(Input::Hover(Some(0)));
    assert!(r.handle(thumb(false)).is_empty());
    assert!(r.is_open());
}

#[test]
fn lease_expiry_keeps_the_ring_escape_chord_closes_it() {
    let mut r = radial();
    open(&mut r);
    assert!(r.handle(Input::Revoked { close: false }).is_empty());
    assert!(r.is_open());
    assert_eq!(r.handle(Input::Revoked { close: true }), vec![Effect::Hide]);
    assert!(!r.is_open());
}

#[test]
fn per_app_ring() {
    let mut r = radial();
    assert!(r.handle(Input::App("firefox".into())).is_empty());
    r.handle(Input::Open {
        menu: "main".into(),
        control: None,
    });
    let fx = r.handle(Input::Cursor(at()));
    assert!(
        matches!(&fx[0], Effect::Show { ring: Ring { items, .. }, .. } if items[0].label == "Tab")
    );
}

// ── adjustable item (volume) ────────────────────────────────────────────────

fn volume_radial() -> Radial {
    let json = r#"{"menus": {"main": {"items": [
            {"label": "Volume", "adjust": "kde.volume"},
            {"label": "Copy", "keys": "Ctrl+C"},
            {"label": "Term", "action": "kde.terminal"}
        ]}}}"#;
    Radial::new(Settings::parse(json).unwrap_or_else(|e| panic!("{e}")))
}

fn level(percent: u32, muted: bool) -> Level {
    Level {
        percent,
        max: 100,
        muted,
        label: "Speakers".into(),
    }
}

fn invoked(fx: &[Effect]) -> Vec<(String, serde_json::Value)> {
    fx.iter()
        .filter_map(|e| match e {
            Effect::Run(s) => match s.first() {
                Some(Step::Invoke(a, p)) => Some((
                    a.clone(),
                    serde_json::from_str(p).unwrap_or_else(|e| panic!("{e}")),
                )),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn gauge_of(fx: &[Effect]) -> Option<Gauge> {
    fx.iter().rev().find_map(|e| match e {
        Effect::Gauge(g) => g.clone(),
        _ => None,
    })
}

/// Open (press + release), answer the level request with `percent`.
fn open_volume(r: &mut Radial, percent: u32) {
    r.handle(Input::Open {
        menu: "main".into(),
        control: Some("thumb".into()),
    });
    r.handle(Input::LeaseGranted);
    let fx = r.handle(Input::Cursor(at()));
    let calls = invoked(&fx);
    assert_eq!(calls[0].0, "kde.volume.get");
    assert_eq!(calls[0].1["reply"], "radial.level");
    let seq = calls[0].1["seq"].as_u64().unwrap_or_else(|| panic!("seq"));
    assert!(fx.contains(&Effect::After {
        ms: LEVEL_TIMEOUT_MS,
        input: Box::new(Input::Due(Timer::LevelTimeout, seq)),
    }));
    assert_eq!(
        gauge_of(&fx).map(|g| (g.item, g.open, g.level)),
        Some((0, false, LevelState::Loading))
    );
    r.handle(thumb(false));
    let fx = r.handle(Input::Level {
        source: "kde.volume".into(),
        seq,
        level: Ok(level(percent, false)),
    });
    assert_eq!(
        gauge_of(&fx).map(|g| g.level),
        Some(LevelState::Ready(level(percent, false)))
    );
}

/// The `Due` input of the `After` effect in `fx`.
fn due(fx: &[Effect]) -> Input {
    fx.iter()
        .find_map(|e| match e {
            Effect::After { input, .. } => Some((**input).clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no timer in {fx:?}"))
}

#[test]
fn hovering_the_volume_opens_its_gauge_after_a_moment() {
    let mut r = volume_radial();
    open_volume(&mut r, 42);
    let fx = r.handle(Input::Hover(Some(0)));
    assert!(fx.contains(&Effect::Highlight(Some(0))));
    let open_timer = due(&fx);
    assert!(matches!(open_timer, Input::Due(Timer::GaugeHover, _)));
    assert!(gauge_of(&r.handle(open_timer)).is_some_and(|g| g.open));
    // leaving closes it after GAUGE_LEAVE_MS, unless the cursor comes back in time
    let leave = due(&r.handle(Input::Hover(None)));
    r.handle(Input::Hover(Some(0)));
    assert!(r.handle(leave).is_empty());
    let leave = due(&r.handle(Input::Hover(Some(1))));
    assert!(gauge_of(&r.handle(leave)).is_some_and(|g| !g.open));
}

#[test]
fn the_wheel_sets_the_volume_with_haptic_marks() {
    let mut r = volume_radial();
    open_volume(&mut r, 46);
    // not on the bubble: the wheel does nothing
    assert!(r.handle(Input::Scroll { coarse: 1, fine: 0 }).is_empty());
    r.handle(Input::Hover(Some(0)));
    // two notches up: 46 → 50 (+2% each), a tick at the 50% mark; opens the gauge
    let fx = r.handle(Input::Scroll { coarse: 2, fine: 0 });
    assert!(fx.contains(&Effect::Haptic(4)));
    let calls = invoked(&fx);
    assert_eq!(calls[0].0, "kde.volume.set");
    assert_eq!(calls[0].1["percent"], 50);
    assert_eq!(calls[0].1["osd"], false);
    let g = gauge_of(&fx).unwrap_or_else(|| panic!("gauge"));
    assert!(g.open);
    assert!(matches!(
        g.level,
        LevelState::Ready(Level { percent: 50, .. })
    ));
    // thumbwheel: 1% per step, no mark crossed, no haptic
    let fx = r.handle(Input::Scroll {
        coarse: 0,
        fine: -3,
    });
    assert!(!fx.iter().any(|e| matches!(e, Effect::Haptic(_))));
    assert_eq!(invoked(&fx)[0].1["percent"], 47);
    // a stale reply (older request) doesn't move the gauge back
    let seq = invoked(&fx)[0].1["seq"].as_u64().unwrap_or(0);
    assert!(
        r.handle(Input::Level {
            source: "kde.volume".into(),
            seq: seq - 1,
            level: Ok(level(50, false)),
        })
        .is_empty()
    );
    // the latest one is taken (the provider's rounding wins)
    let fx = r.handle(Input::Level {
        source: "kde.volume".into(),
        seq,
        level: Ok(level(48, false)),
    });
    assert!(matches!(
        gauge_of(&fx).map(|g| g.level),
        Some(LevelState::Ready(Level { percent: 48, .. }))
    ));
    // down to 0: a firm stop; further down: nothing at all
    let fx = r.handle(Input::Scroll {
        coarse: -30,
        fine: 0,
    });
    assert!(fx.contains(&Effect::Haptic(1)));
    assert_eq!(invoked(&fx)[0].1["percent"], 0);
    assert!(
        r.handle(Input::Scroll {
            coarse: -1,
            fine: 0
        })
        .is_empty()
    );
}

#[test]
fn clicking_the_volume_pins_it_and_the_ring_stays() {
    let mut r = volume_radial();
    open_volume(&mut r, 30);
    r.handle(Input::Hover(Some(0)));
    let fx = r.handle(click(Target::Item(0)));
    assert!(!fx.contains(&Effect::Hide));
    assert!(gauge_of(&fx).is_some_and(|g| g.pinned && g.open));
    assert!(r.is_open());
    // pinned: hovering elsewhere keeps the highlight, the wheel still works
    assert!(r.handle(Input::Hover(Some(1))).is_empty());
    let fx = r.handle(Input::Scroll { coarse: 1, fine: 0 });
    assert_eq!(invoked(&fx)[0].1["percent"], 32);
    // middle click anywhere mutes; scrolling up unmutes
    let fx = r.handle(Input::Middle(Target::Outside));
    assert_eq!(invoked(&fx)[0].0, "kde.volume.mute");
    assert!(matches!(
        gauge_of(&fx).map(|g| g.level),
        Some(LevelState::Ready(Level { muted: true, .. }))
    ));
    let fx = r.handle(Input::Scroll { coarse: 1, fine: 0 });
    assert_eq!(invoked(&fx)[0].1["unmute"], true);
    // the thumb panel closes (the volume has nothing to run)
    let fx = r.handle(thumb(true));
    assert_eq!(fx, vec![Effect::Hide]);
    assert!(!r.is_open());
}

#[test]
fn hold_and_release_on_the_volume_pins_it() {
    let mut r = volume_radial();
    r.handle(Input::Open {
        menu: "main".into(),
        control: Some("thumb".into()),
    });
    r.handle(Input::Cursor(at()));
    r.handle(Input::Hover(Some(0)));
    let fx = r.handle(thumb(false));
    assert!(fx.contains(&Effect::ReleaseLease));
    assert!(gauge_of(&fx).is_some_and(|g| g.pinned));
    assert!(r.is_open());
}

#[test]
fn no_answer_means_unavailable() {
    let mut r = volume_radial();
    r.handle(Input::Open {
        menu: "main".into(),
        control: None,
    });
    let fx = r.handle(Input::Cursor(at()));
    let timeout = fx
        .iter()
        .find_map(|e| match e {
            Effect::After { input, .. } => Some((**input).clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no timeout"));
    let fx = r.handle(timeout);
    assert!(matches!(
        gauge_of(&fx).map(|g| g.level),
        Some(LevelState::Unavailable(_))
    ));
    // the wheel does nothing without a value; other items still work
    r.handle(Input::Hover(Some(0)));
    let fx = r.handle(Input::Scroll { coarse: 1, fine: 0 });
    assert!(invoked(&fx).is_empty());
    let fx = r.handle(click(Target::Item(2)));
    assert_eq!(invoked(&fx)[0].0, "kde.terminal");
}

#[test]
fn ten_percent_marks() {
    assert!(crosses_ten(48, 50));
    assert!(crosses_ten(51, 49));
    assert!(crosses_ten(52, 50));
    assert!(!crosses_ten(50, 52));
    assert!(!crosses_ten(41, 47));
    assert!(crosses_ten(9, 11));
}

#[test]
fn trigger_parsing() {
    assert_eq!(trigger_control("thumb.press").as_deref(), Some("thumb"));
    assert_eq!(trigger_control("gesture + thumb").as_deref(), Some("thumb"));
    assert_eq!(trigger_control("plugin:kde"), None);
    assert_eq!(trigger_control("detentctl"), None);
}
