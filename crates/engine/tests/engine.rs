//! Engine behaviour tests: taps, swipes, chords with grab, layers, native fallback, holds.

#![allow(clippy::unwrap_used)]

use std::time::{Duration, Instant};

use detent_engine::action::{ActionId, CmdId, DeviceCmd, KeyChord, ParamsRef, PluginSlot};
use detent_engine::out::code::*;
use detent_engine::{
    Action, Compiled, ControlOut, Edge, Engine, GrabChange, LayerId, ModMask, Out, OutEvent,
    SlotInfo, Tables, slot,
};

const GESTURE_CID: u16 = 0x00C3;
const MIDDLE_CID: u16 = 0x0052;
const THUMB_CID: u16 = 0x01A0;
const KEY_LEFTCTRL: u16 = 29;
const KEY_TAB: u16 = 15;

/// gesture = modifier 0 with a layer (layer 1): swipes, `left` → exec 2, wheel → Ctrl+Tab.
/// Base: gesture.tap → exec 1, thumb.press → action 7. Middle diverted (used in layer).
fn cfg() -> Compiled {
    let mut c = Compiled::default();
    c.slots[slot::GESTURE.0 as usize] = SlotInfo {
        cid: GESTURE_CID,
        modifier: Some(0),
        ..Default::default()
    };
    c.slots[slot::MIDDLE.0 as usize] = SlotInfo {
        cid: MIDDLE_CID,
        native_button: BTN_MIDDLE,
        ..Default::default()
    };
    c.slots[slot::THUMB.0 as usize] = SlotInfo {
        cid: THUMB_CID,
        native_button: BTN_BACK,
        hold_ms: 0,
        ..Default::default()
    };
    c.slots[slot::LEFT.0 as usize] = SlotInfo {
        evdev_button: BTN_LEFT,
        ..Default::default()
    };
    c.slots[slot::RIGHT.0 as usize] = SlotInfo {
        evdev_button: BTN_RIGHT,
        ..Default::default()
    };
    c.modifier_slots[0] = Some(slot::GESTURE);
    c.grab_mods = ModMask(1);
    c.swipe_mods = ModMask(1);
    c.swipe_threshold_um = 6000;
    let mut t = Tables::new(2);
    let mut m = ModMask::default();
    m.press(0);
    t.layers.set(m, LayerId(1));
    let b = &mut t.bindings;
    b.set(LayerId(0), slot::GESTURE, Edge::Tap, Action::Exec(CmdId(1)));
    b.set(
        LayerId(0),
        slot::THUMB,
        Edge::Press,
        Action::Plugin {
            plugin: PluginSlot(0),
            action: ActionId(7),
            params: ParamsRef(0),
        },
    );
    b.set(LayerId(1), slot::LEFT, Edge::Press, Action::Exec(CmdId(2)));
    b.set(
        LayerId(1),
        slot::SWIPE,
        Edge::SwipeUp,
        Action::Exec(CmdId(3)),
    );
    b.set(
        LayerId(1),
        slot::SWIPE,
        Edge::SwipeLeft,
        Action::Exec(CmdId(4)),
    );
    let ctrl_tab = KeyChord::new(&[KEY_LEFTCTRL, KEY_TAB]).unwrap();
    b.set(
        LayerId(1),
        slot::WHEEL,
        Edge::RotatePos,
        Action::Keys(ctrl_tab),
    );
    b.set(
        LayerId(1),
        slot::MIDDLE,
        Edge::Press,
        Action::Exec(CmdId(5)),
    );
    c.profiles = vec![t];
    c
}

fn execs(out: &Out) -> Vec<u16> {
    out.controls()
        .filter_map(|c| match c {
            ControlOut::Exec(CmdId(n)) => Some(n),
            _ => None,
        })
        .collect()
}

struct H {
    c: Compiled,
    e: Engine,
    out: Out,
    now: Instant,
}

impl H {
    fn new() -> Self {
        let mut e = Engine::default();
        e.set_dpi(1000);
        Self {
            c: cfg(),
            e,
            out: Out::default(),
            now: Instant::now(),
        }
    }
    fn buttons(&mut self, cids: [u16; 4]) -> &Out {
        self.out.clear();
        self.e
            .on_diverted_buttons(&self.c, cids, self.now, &mut self.out);
        &self.out
    }
    fn xy(&mut self, dx: i16, dy: i16) -> &Out {
        self.out.clear();
        self.e.on_raw_xy(&self.c, dx, dy, &mut self.out);
        &self.out
    }
    fn ev(&mut self, ty: u16, code: u16, value: i32) -> &Out {
        self.out.clear();
        self.e
            .on_evdev(&self.c, ty, code, value, self.now, &mut self.out);
        &self.out
    }
    fn apply_grab(&mut self) -> Option<GrabChange> {
        let g = self.e.grab_change(&self.c);
        match g {
            Some(GrabChange::Grab) => self.e.set_grabbed(true),
            Some(GrabChange::Ungrab) => self.e.set_grabbed(false),
            None => {}
        }
        g
    }
}

#[test]
fn gesture_tap_fires_tap_only() {
    let mut h = H::new();
    assert!(execs(h.buttons([GESTURE_CID, 0, 0, 0])).is_empty());
    assert_eq!(h.e.current_layer(&h.c), LayerId(1));
    assert_eq!(execs(h.buttons([0; 4])), vec![1]);
    assert_eq!(h.e.current_layer(&h.c), LayerId::BASE);
}

#[test]
fn swipe_fires_once_and_cancels_tap() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    // first frame is ignored (garbage after press)
    assert!(execs(h.xy(0, -500)).is_empty());
    // 6 mm at 1000 dpi = 236 counts
    assert!(execs(h.xy(5, -100)).is_empty());
    assert_eq!(execs(h.xy(5, -150)), vec![3]);
    assert!(execs(h.xy(0, -400)).is_empty(), "once per hold");
    assert!(execs(h.buttons([0; 4])).is_empty(), "no tap after swipe");
}

#[test]
fn diagonal_below_threshold_waits_for_dominant_axis() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.xy(0, 0);
    // 2.8 mm diagonal: still undecided
    assert!(execs(h.xy(-80, -80)).is_empty());
    assert_eq!(execs(h.xy(-300, 0)), vec![4]);
}

#[test]
fn diagonal_past_threshold_does_nothing_and_cancels_tap() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.xy(0, 0);
    assert!(execs(h.xy(-200, -200)).is_empty(), "diagonal fires nothing");
    assert!(execs(h.xy(-400, 0)).is_empty(), "decided: once per hold");
    assert!(
        execs(h.buttons([0; 4])).is_empty(),
        "no tap after a diagonal"
    );
}

fn haptics(out: &Out) -> usize {
    out.controls()
        .filter(|c| {
            matches!(
                c,
                ControlOut::Device(DeviceCmd::HapticPulse { waveform: 1, .. })
            )
        })
        .count()
}

#[test]
fn gesture_haptic_on_swipe_diagonal_and_tap_only() {
    let mut h = H::new();
    h.c.swipe_haptic = Some(1);
    // swipe (bound direction): one pulse when decided, none on release
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.xy(0, 0);
    assert_eq!(haptics(h.xy(0, -300)), 1);
    assert_eq!(haptics(h.xy(0, -300)), 0, "once per hold");
    assert_eq!(haptics(h.buttons([0; 4])), 0);
    // swipe in an unbound direction still pulses
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.xy(0, 0);
    assert_eq!(haptics(h.xy(300, 0)), 1);
    h.buttons([0; 4]);
    // diagonal
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.xy(0, 0);
    assert_eq!(haptics(h.xy(-200, -200)), 1);
    h.buttons([0; 4]);
    // tap (no direction)
    h.buttons([GESTURE_CID, 0, 0, 0]);
    assert_eq!(haptics(h.buttons([0; 4])), 1);
    // chord (gesture + left) is not a gesture: no pulse
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.apply_grab();
    assert_eq!(haptics(h.ev(EV_KEY, BTN_LEFT, 1)), 0);
    h.ev(EV_KEY, BTN_LEFT, 0);
    assert_eq!(haptics(h.buttons([0; 4])), 0);
}

#[test]
fn gesture_haptic_off_by_default() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.xy(0, 0);
    assert_eq!(haptics(h.xy(0, -300)), 0);
    h.buttons([0; 4]);
    h.buttons([GESTURE_CID, 0, 0, 0]);
    assert_eq!(haptics(h.buttons([0; 4])), 0);
}

#[test]
fn chord_with_left_is_swallowed_and_grab_lifecycle() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    assert_eq!(h.apply_grab(), Some(GrabChange::Grab));
    assert_eq!(execs(h.ev(EV_KEY, BTN_LEFT, 1)), vec![2]);
    assert!(h.out.events().is_empty(), "press swallowed");
    h.ev(EV_SYN, SYN_REPORT, 0);
    assert!(h.out.events().is_empty());
    // gesture released while left still down: stay grabbed
    assert!(execs(h.buttons([0; 4])).is_empty(), "chord → no tap");
    assert_eq!(h.apply_grab(), None);
    h.ev(EV_KEY, BTN_LEFT, 0);
    assert!(h.out.events().is_empty(), "release swallowed too");
    assert_eq!(h.apply_grab(), Some(GrabChange::Ungrab));
}

#[test]
fn unbound_buttons_pass_through_while_grabbed() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.apply_grab();
    h.ev(EV_KEY, BTN_RIGHT, 1);
    assert_eq!(h.out.events(), &[OutEvent::key(BTN_RIGHT, true)]);
    h.ev(EV_REL, REL_X, 5);
    assert_eq!(h.out.events(), &[OutEvent::rel(REL_X, 5)]);
    h.ev(EV_SYN, SYN_REPORT, 0);
    assert_eq!(
        h.out.events(),
        &[] as &[OutEvent],
        "empty frame adds no SYN"
    );
    h.ev(EV_KEY, BTN_RIGHT, 0);
    assert_eq!(h.out.events(), &[OutEvent::key(BTN_RIGHT, false)]);
}

#[test]
fn never_grabs_while_a_button_is_down() {
    let mut h = H::new();
    h.ev(EV_KEY, BTN_LEFT, 1); // not grabbed: tracked only
    assert!(h.out.events().is_empty());
    h.buttons([GESTURE_CID, 0, 0, 0]);
    assert_eq!(h.apply_grab(), None);
    h.ev(EV_KEY, BTN_LEFT, 0);
    assert_eq!(h.apply_grab(), Some(GrabChange::Grab));
}

#[test]
fn wheel_in_layer_sends_keys_and_drops_hires() {
    let mut h = H::new();
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.apply_grab();
    h.ev(EV_REL, REL_WHEEL_HI_RES, 120);
    assert!(h.out.events().is_empty());
    h.ev(EV_REL, REL_WHEEL, 1);
    assert_eq!(
        h.out.events(),
        &[
            OutEvent::key(KEY_LEFTCTRL, true),
            OutEvent::key(KEY_TAB, true),
            OutEvent::SYN,
            OutEvent::key(KEY_TAB, false),
            OutEvent::key(KEY_LEFTCTRL, false),
            OutEvent::SYN,
        ]
    );
    // unbound direction passes through
    h.ev(EV_REL, REL_WHEEL, -1);
    assert_eq!(h.out.events(), &[OutEvent::rel(REL_WHEEL, -1)]);
}

#[test]
fn diverted_middle_keeps_native_click_on_base_layer() {
    let mut h = H::new();
    h.buttons([MIDDLE_CID, 0, 0, 0]);
    assert_eq!(
        h.out.events(),
        &[OutEvent::key(BTN_MIDDLE, true), OutEvent::SYN]
    );
    h.buttons([0; 4]);
    assert_eq!(
        h.out.events(),
        &[OutEvent::key(BTN_MIDDLE, false), OutEvent::SYN]
    );
    // in the gesture layer it runs the binding instead
    h.buttons([GESTURE_CID, 0, 0, 0]);
    h.buttons([GESTURE_CID, MIDDLE_CID, 0, 0]);
    assert_eq!(execs(&h.out), vec![5]);
    assert!(h.out.events().is_empty());
}

#[test]
fn plugin_action_on_press() {
    let mut h = H::new();
    h.buttons([THUMB_CID, 0, 0, 0]);
    let got: Vec<_> = h.out.controls().collect();
    assert_eq!(
        got,
        vec![ControlOut::Action {
            action: ActionId(7),
            params: ParamsRef(0),
            delta: 0
        }]
    );
    h.buttons([0; 4]);
    assert!(h.out.is_empty());
}

#[test]
fn hold_timer_fires_and_suppresses_tap() {
    let mut h = H::new();
    h.c.slots[slot::GESTURE.0 as usize].hold_ms = 300;
    h.c.profiles[0].bindings.set(
        LayerId(0),
        slot::GESTURE,
        Edge::Hold,
        Action::Exec(CmdId(9)),
    );
    h.buttons([GESTURE_CID, 0, 0, 0]);
    let d = h.e.next_deadline(&h.c).unwrap();
    assert_eq!(d, h.now + Duration::from_millis(300));
    h.out.clear();
    h.e.tick(&h.c, d, &mut h.out);
    assert_eq!(execs(&h.out), vec![9]);
    assert!(h.e.next_deadline(&h.c).is_none());
    assert!(execs(h.buttons([0; 4])).is_empty());
}

#[test]
fn release_all_lets_go_of_held_output() {
    let mut h = H::new();
    h.buttons([MIDDLE_CID, 0, 0, 0]);
    h.out.clear();
    h.e.release_all(&mut h.out);
    assert_eq!(
        h.out.events(),
        &[OutEvent::key(BTN_MIDDLE, false), OutEvent::SYN]
    );
}

#[test]
fn pointer_and_button_lease_route_to_plugin() {
    use detent_engine::engine::Lease;
    let mut h = H::new();
    h.e.set_lease(Lease {
        pointer: true,
        buttons: true,
        controls: 1 << slot::THUMB.0,
        until: Some(h.now + Duration::from_secs(10)),
    });
    assert_eq!(h.apply_grab(), Some(GrabChange::Grab));
    h.ev(EV_REL, REL_X, 5);
    h.ev(EV_REL, REL_Y, -3);
    assert!(h.out.events().is_empty(), "cursor frozen");
    h.ev(EV_SYN, SYN_REPORT, 0);
    assert_eq!(
        h.out.controls().collect::<Vec<_>>(),
        vec![ControlOut::LeaseMotion { dx: 5, dy: -3 }]
    );
    h.ev(EV_KEY, BTN_LEFT, 1);
    assert_eq!(
        h.out.controls().collect::<Vec<_>>(),
        vec![ControlOut::LeaseButton {
            code: BTN_LEFT,
            slot: 0,
            down: true
        }]
    );
    assert!(h.out.events().is_empty());
    h.ev(EV_KEY, BTN_LEFT, 0);
    // the leased thumb goes to the plugin instead of firing its binding
    h.buttons([THUMB_CID, 0, 0, 0]);
    assert_eq!(
        h.out.controls().collect::<Vec<_>>(),
        vec![ControlOut::LeaseButton {
            code: 0,
            slot: slot::THUMB.0,
            down: true
        }]
    );
    h.buttons([0; 4]);
    h.e.set_lease(Lease::default());
    assert_eq!(h.apply_grab(), Some(GrabChange::Ungrab));
}

#[test]
fn lease_expires_and_escape_chord_revokes() {
    use detent_engine::engine::{ESCAPE_CHORD, Lease};
    let mut h = H::new();
    h.e.set_lease(Lease {
        pointer: true,
        until: Some(h.now + Duration::from_secs(1)),
        ..Default::default()
    });
    assert_eq!(
        h.e.next_deadline(&h.c),
        Some(h.now + Duration::from_secs(1))
    );
    h.out.clear();
    h.e.tick(&h.c, h.now + Duration::from_secs(2), &mut h.out);
    assert_eq!(
        h.out.controls().collect::<Vec<_>>(),
        vec![ControlOut::LeaseEnded { escape: false }]
    );
    assert!(!h.e.lease().is_active());

    h.e.set_lease(Lease {
        pointer: true,
        buttons: true,
        until: Some(h.now + Duration::from_secs(60)),
        ..Default::default()
    });
    h.apply_grab();
    h.ev(EV_KEY, BTN_LEFT, 1);
    h.ev(EV_KEY, BTN_RIGHT, 1);
    assert_eq!(h.e.next_deadline(&h.c), Some(h.now + ESCAPE_CHORD));
    h.out.clear();
    h.e.tick(&h.c, h.now + ESCAPE_CHORD, &mut h.out);
    assert_eq!(
        h.out.controls().collect::<Vec<_>>(),
        vec![ControlOut::LeaseEnded { escape: true }]
    );
}

#[test]
fn swipe_threshold_scales_with_dpi() {
    use detent_engine::engine::{SwipeDir, swipe_direction};
    // 6 mm: 236 counts at 1000 dpi, 945 at 4000 dpi
    assert_eq!(
        swipe_direction(0, -240, 6000, 1000),
        SwipeDir::Edge(Edge::SwipeUp)
    );
    assert_eq!(swipe_direction(0, -240, 6000, 4000), SwipeDir::Pending);
    assert_eq!(
        swipe_direction(950, 0, 6000, 4000),
        SwipeDir::Edge(Edge::SwipeRight)
    );
    assert_eq!(swipe_direction(700, 700, 6000, 4000), SwipeDir::Diagonal);
}
