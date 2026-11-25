//! The one-time "Enjoying Detent?" notification. The gating (a week after the first run,
//! answered is final, `[sponsor] nudge = false` opts out) lives in
//! `detent_update::sponsor`; this module records the first run, asks at most once per tray
//! process, and turns the answer into a link or a retired nudge.
//!
//! Everything runs on the `sponsor` thread: showing the notification and waiting for the
//! answer are blocking D-Bus calls and never touch the refresh loop or the menu.

use std::time::{Duration, SystemTime};

use detent_update::SponsorSettings;
use detent_update::sponsor::{self, PIX_URL, SPONSOR_URL};
use detent_update::state::{self, State};
use notify_rust::{ActionResponse, CloseReason, Notification, Timeout};

use crate::notify::{APP, ICON};
use crate::platform;
use crate::sni::Handle;

pub const TITLE: &str = "Enjoying Detent?";
pub const BODY: &str = "Detent is free and built by one person. \
                        If it earns its place on your desk, consider sponsoring it.";

/// Action ids and their button labels, in the order the server shows them.
const GITHUB: (&str, &str) = ("sponsor-github", "Sponsor on GitHub");
const PIX: (&str, &str) = ("sponsor-pix", "Pix");
const DONE: (&str, &str) = ("sponsor-done", "Don't ask again");

/// First look after the tray starts: well after login, so the nudge never lands among
/// the session's start-up notifications.
const FIRST_CHECK: Duration = Duration::from_secs(10 * 60);
/// Later looks, for a tray that runs for days.
const RECHECK: Duration = Duration::from_secs(6 * 60 * 60);

/// Stamp the first run in state.json if it isn't there yet. Called once at start.
pub fn record_first_run() {
    let mut s = State::load();
    if sponsor::record_first_run(&mut s, SystemTime::now())
        && let Err(e) = s.save()
    {
        // Without a saved first run the nudge stays off: the clock never starts.
        eprintln!("detent-tray: can't record {}: {e}", state::path().display());
    }
}

/// `[sponsor] nudge`. An unreadable tray.toml holds the nudge back: it may be the file
/// that opts out.
fn enabled() -> bool {
    SponsorSettings::load().map_or_else(
        |e| {
            eprintln!("detent-tray: {e}; not showing the sponsor notification");
            false
        },
        |s| s.nudge,
    )
}

/// What an answer to the notification does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// Open a link; the nudge stays pending (a later tray process may ask again).
    Open(&'static str),
    /// "Don't ask again" or dismissed by the user: never ask again.
    Retire,
    /// Expired, closed by the server, or anything else: nothing.
    Ignore,
}

fn answer(r: &ActionResponse) -> Answer {
    match r {
        ActionResponse::Custom(a) if *a == GITHUB.0 => Answer::Open(SPONSOR_URL),
        ActionResponse::Custom(a) if *a == PIX.0 => Answer::Open(PIX_URL),
        ActionResponse::Custom(a) if *a == DONE.0 => Answer::Retire,
        ActionResponse::Closed(CloseReason::Dismissed) => Answer::Retire,
        _ => Answer::Ignore,
    }
}

/// Set `sponsor_nudge_done`, re-reading the file first so nothing else in it is lost.
fn retire() {
    let mut s = State::load();
    s.sponsor_nudge_done = true;
    if let Err(e) = s.save() {
        eprintln!("detent-tray: can't record {}: {e}", state::path().display());
    }
}

/// Show the notification and wait (on this thread) for its answer. `false` if the
/// notification server refused it, so a later look may try again.
fn ask() -> bool {
    let mut n = Notification::new();
    n.appname(APP)
        .summary(TITLE)
        .body(BODY)
        .icon(ICON)
        .timeout(Timeout::Default);
    for (id, label) in [GITHUB, PIX, DONE] {
        n.action(id, label);
    }
    let handle = match n.show() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("detent-tray: sponsor notification failed: {e}");
            return false;
        }
    };
    let mut got = Answer::Ignore;
    zbus::block_on(handle.wait_for_action_async(|r| got = answer(r)));
    match got {
        Answer::Open(url) => platform::open_url(url.to_string()),
        Answer::Retire => retire(),
        Answer::Ignore => {}
    }
    true
}

/// The sponsor thread: look after [`FIRST_CHECK`], then every [`RECHECK`]; ask at most
/// once, then end. Also ends once the nudge is answered or the tray is gone.
pub fn run(handle: Handle) {
    std::thread::sleep(FIRST_CHECK);
    loop {
        if handle.is_closed() {
            return;
        }
        let s = State::load();
        if s.sponsor_nudge_done {
            return;
        }
        if sponsor::nudge_due(&s, enabled(), SystemTime::now()) && ask() {
            return; // once per process, whatever the answer
        }
        std::thread::sleep(RECHECK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers() {
        let custom = |a: &str| answer(&ActionResponse::Custom(a));
        assert_eq!(custom("sponsor-github"), Answer::Open(SPONSOR_URL));
        assert_eq!(custom("sponsor-pix"), Answer::Open(PIX_URL));
        assert_eq!(custom("sponsor-done"), Answer::Retire);
        assert_eq!(custom("default"), Answer::Ignore);
        let closed = |r: u32| answer(&ActionResponse::Closed(CloseReason::from(r)));
        assert_eq!(closed(2), Answer::Retire, "dismissed by the user");
        assert_eq!(closed(1), Answer::Ignore, "expired");
        assert_eq!(
            closed(3),
            Answer::Ignore,
            "closed by a CloseNotification call"
        );
        assert_eq!(closed(4), Answer::Ignore, "reserved");
    }

    #[test]
    fn strings() {
        assert_eq!(TITLE, "Enjoying Detent?");
        assert_eq!(
            [GITHUB.1, PIX.1, DONE.1],
            ["Sponsor on GitHub", "Pix", "Don't ask again"]
        );
        assert!(!BODY.contains("  "), "{BODY}");
    }
}
