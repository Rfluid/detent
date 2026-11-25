//! The output fan (docs/plugins/radial.md, "Switching the output").

use super::*;
use crate::icon::Icon;

fn choice(k: usize, current: bool) -> Choice {
    Choice {
        id: k.to_string(),
        name: format!("Out {k}"),
        description: format!("Output device {k}"),
        icon: Icon::None,
        percent: Some(10 * (k as u32 + 1)),
        muted: false,
        current,
    }
}

/// The volume ring, open at 50 %, its provider listing `n` outputs (`Out 1` current).
/// `control`: opened by holding the thumb (else by an action, nothing held).
fn ring_with_outputs(n: usize, control: Option<&str>) -> Radial {
    let mut r = volume_radial();
    r.handle(Input::Open {
        menu: "main".into(),
        control: control.map(String::from),
    });
    r.handle(Input::LeaseGranted);
    let fx = r.handle(Input::Cursor(at()));
    let calls = invoked(&fx);
    assert_eq!(calls[1].0, "kde.volume.targets");
    assert_eq!(calls[1].1["reply"], "radial.targets");
    let seq = |k: usize| calls[k].1["seq"].as_u64().unwrap_or_else(|| panic!("seq"));
    r.handle(Input::Level {
        source: "kde.volume".into(),
        seq: seq(0),
        level: Ok(level(50, false)),
    });
    // a stale or foreign answer is ignored
    let stale = Input::Targets {
        source: "kde.volume".into(),
        seq: seq(1) + 7,
        targets: Ok(vec![choice(0, true), choice(1, false)]),
    };
    assert!(r.handle(stale).is_empty());
    let fx = r.handle(Input::Targets {
        source: "kde.volume".into(),
        seq: seq(1),
        targets: Ok((0..n).map(|k| choice(k, k == 1)).collect()),
    });
    assert_eq!(gauge_of(&fx).map(|g| g.targets.len()), Some(n));
    r
}

/// Hover the volume and click its pill: the fan opens.
fn open_fan(r: &mut Radial) {
    r.handle(Input::Hover(Some(0)));
    let fx = r.handle(click(Target::Pill));
    assert!(fx.contains(&Effect::Haptic(4)));
    let g = gauge_of(&fx).unwrap_or_else(|| panic!("gauge"));
    assert!(g.fan && g.open && g.fan_hl.is_none());
}

fn fan_of(fx: &[Effect]) -> Option<(bool, Option<usize>)> {
    gauge_of(fx).map(|g| (g.fan, g.fan_hl))
}

#[test]
fn the_pill_opens_the_fan_and_a_choice_switches_the_output() {
    let mut r = ring_with_outputs(3, None);
    open_fan(&mut r);
    // the ring under the fan doesn't react
    assert!(r.handle(Input::Hover(Some(2))).is_empty());
    assert!(r.handle(Input::Middle(Target::Item(0))).is_empty());
    let fx = r.handle(Input::HoverChoice(Some(0)));
    assert!(fx.contains(&Effect::Haptic(4)));
    assert_eq!(fan_of(&fx), Some((true, Some(0))));
    let fx = r.handle(click(Target::Choice(0)));
    let calls = invoked(&fx);
    assert_eq!(calls[0].0, "kde.volume.target");
    assert_eq!(calls[0].1["id"], "0");
    assert_eq!(calls[0].1["reply"], "radial.level");
    assert!(fx.contains(&Effect::Haptic(1)));
    let g = gauge_of(&fx).unwrap_or_else(|| panic!("gauge"));
    assert!(!g.fan && g.targets[0].current && !g.targets[1].current);
    // the new output's level shows at once; its reply is the one taken next
    assert!(matches!(&g.level, LevelState::Ready(l) if l.percent == 10 && l.label == "Out 0"));
    let seq = calls[0].1["seq"].as_u64().unwrap_or(0);
    let fx = r.handle(Input::Level {
        source: "kde.volume".into(),
        seq,
        level: Ok(level(12, false)),
    });
    assert!(matches!(
        gauge_of(&fx).map(|g| g.level),
        Some(LevelState::Ready(Level { percent: 12, .. }))
    ));
    assert!(r.is_open());
}

#[test]
fn with_one_output_the_pill_acts_like_its_bubble() {
    let mut r = ring_with_outputs(1, None);
    r.handle(Input::Hover(Some(0)));
    let g = gauge_of(&r.handle(click(Target::Pill))).unwrap_or_else(|| panic!("gauge"));
    assert!(g.pinned && !g.fan);
}

#[test]
fn escape_right_click_and_the_centre_close_only_the_fan() {
    let mut r = ring_with_outputs(2, None);
    for back in [
        Input::Key(Key::Escape),
        Input::Click {
            left: false,
            target: Target::Outside,
        },
        click(Target::Center),
        click(Target::Item(1)),
    ] {
        open_fan(&mut r);
        let fx = r.handle(back);
        assert_eq!(fan_of(&fx), Some((false, None)));
        assert!(invoked(&fx).is_empty());
        assert!(r.is_open());
    }
    // a click outside everything closes the ring, as usual
    open_fan(&mut r);
    assert!(r.handle(click(Target::Outside)).contains(&Effect::Hide));
    assert!(!r.is_open());
}

#[test]
fn wheel_and_tab_move_through_the_fan_enter_chooses() {
    let mut r = ring_with_outputs(3, None);
    open_fan(&mut r);
    // the first step starts at the current output, then moves; the wheel stops at the end
    let down = Input::Scroll {
        coarse: -1,
        fine: 0,
    };
    assert_eq!(fan_of(&r.handle(down.clone())), Some((true, Some(1))));
    assert_eq!(fan_of(&r.handle(down.clone())), Some((true, Some(2))));
    assert!(r.handle(down).is_empty());
    // Tab wraps around
    assert_eq!(
        fan_of(&r.handle(Input::Key(Key::Tab))),
        Some((true, Some(0)))
    );
    assert_eq!(
        fan_of(&r.handle(Input::Key(Key::BackTab))),
        Some((true, Some(2)))
    );
    let fx = r.handle(Input::Key(Key::Select));
    assert_eq!(invoked(&fx)[0].1["id"], "2");
    assert_eq!(fan_of(&fx), Some((false, None)));
}

#[test]
fn choosing_the_current_output_just_folds_the_fan() {
    let mut r = ring_with_outputs(2, None);
    open_fan(&mut r);
    let fx = r.handle(click(Target::Choice(1)));
    assert!(invoked(&fx).is_empty());
    assert_eq!(fan_of(&fx), Some((false, None)));
}

#[test]
fn tab_on_the_pinned_volume_opens_the_fan() {
    let mut r = ring_with_outputs(2, None);
    r.handle(Input::Hover(Some(0)));
    assert!(gauge_of(&r.handle(click(Target::Item(0)))).is_some_and(|g| g.pinned));
    assert_eq!(fan_of(&r.handle(Input::Key(Key::Tab))), Some((true, None)));
}

#[test]
fn releasing_the_held_thumb_on_a_fan_bubble_chooses_it() {
    let mut r = ring_with_outputs(3, Some("thumb"));
    open_fan(&mut r);
    r.handle(Input::HoverChoice(Some(2)));
    let fx = r.handle(thumb(false));
    assert!(fx.contains(&Effect::ReleaseLease));
    assert_eq!(invoked(&fx)[0].1["id"], "2");
    assert!(r.is_open());
}

#[test]
fn the_fan_keeps_the_dial_open_when_the_cursor_leaves() {
    let mut r = ring_with_outputs(2, None);
    let open_timer = due(&r.handle(Input::Hover(Some(0))));
    assert!(gauge_of(&r.handle(open_timer)).is_some_and(|g| g.open));
    // a leave timer started before the fan opened must not close it
    let leave = due(&r.handle(Input::Hover(None)));
    r.handle(Input::Hover(Some(0)));
    open_fan(&mut r);
    assert!(r.handle(leave).is_empty());
}
