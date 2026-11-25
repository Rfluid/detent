//! Sponsor links and the one-time "Enjoying Detent?" nudge (ported from Rfluid/aura).
//!
//! The tray asks once, a week after it first ran, and never again once the user answered
//! with "Don't ask again" or dismissed the notification. Both facts live in
//! [`State`] (`state.json`), not `tray.toml`: they are facts Detent records about itself,
//! not settings. The only user-facing knob is the opt-out, `[sponsor] nudge` in tray.toml
//! ([`crate::SponsorSettings`]).
//!
//! Everything here takes `now` as an argument so the gating is testable without a clock.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::state::State;

/// GitHub Sponsors: the nudge's primary button, the tray's "Sponsor Detent…" row and
/// `detentctl sponsor --open`.
pub const SPONSOR_URL: &str = "https://github.com/sponsors/Rfluid";

/// Pix: a one-off tip in BRL for people without a GitHub Sponsors-friendly card.
pub const PIX_URL: &str = "https://livepix.gg/rfluid";

/// SPONSOR.md: every way to chip in, including Bitcoin and Lightning.
pub const SPONSOR_DOC_URL: &str = "https://github.com/Rfluid/detent/blob/main/SPONSOR.md";

/// How long after the first run the nudge waits before appearing.
pub const NUDGE_DELAY: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// `now` as Unix seconds (0 for a clock before 1970).
pub fn unix_secs(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Stamp `state.first_run` with `now` if it has never been set. Returns whether the state
/// changed, so the caller only writes the file when it has to.
///
/// A state file from before the field (or a lost one) has no `first_run`, so that run
/// counts as the first: the user gets the full week rather than a nudge at once.
pub fn record_first_run(state: &mut State, now: SystemTime) -> bool {
    if state.first_run.is_some() {
        return false;
    }
    state.first_run = Some(unix_secs(now));
    true
}

/// Whether the nudge should show right now.
///
/// False when the user opted out (`enabled` is `[sponsor] nudge`), when it was already
/// answered, and when there is no recorded first run: a missing timestamp means the clock
/// hasn't started, not that it ran out. A first run in the future (the system clock moved
/// backwards) also reads as "not yet".
pub fn nudge_due(state: &State, enabled: bool, now: SystemTime) -> bool {
    if !enabled || state.sponsor_nudge_done {
        return false;
    }
    let Some(first_run) = state.first_run else {
        return false;
    };
    let first_run = UNIX_EPOCH + Duration::from_secs(first_run);
    now.duration_since(first_run)
        .is_ok_and(|elapsed| elapsed >= NUDGE_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// 2026-09-01T12:00:00Z.
    fn t0() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_788_264_000)
    }

    fn started_at(first_run: SystemTime) -> State {
        State {
            first_run: Some(unix_secs(first_run)),
            ..State::default()
        }
    }

    #[test]
    fn hidden_before_seven_days() {
        let state = started_at(t0());
        assert!(!nudge_due(&state, true, t0()));
        assert!(!nudge_due(&state, true, t0() + 6 * DAY));
        let just_short = t0() + NUDGE_DELAY - Duration::from_secs(1);
        assert!(!nudge_due(&state, true, just_short));
    }

    #[test]
    fn shown_at_and_after_seven_days() {
        let state = started_at(t0());
        assert!(nudge_due(&state, true, t0() + NUDGE_DELAY));
        assert!(nudge_due(&state, true, t0() + 90 * DAY));
    }

    #[test]
    fn never_shown_once_done() {
        let state = State {
            sponsor_nudge_done: true,
            ..started_at(t0())
        };
        assert!(!nudge_due(&state, true, t0() + 30 * DAY));
    }

    #[test]
    fn opt_out_is_respected() {
        let state = started_at(t0());
        assert!(!nudge_due(&state, false, t0() + 30 * DAY));
    }

    #[test]
    fn clock_moving_backwards_reads_as_not_yet() {
        let state = started_at(t0());
        assert!(!nudge_due(&state, true, t0() - 30 * DAY));
    }

    #[test]
    fn missing_first_run_initializes_rather_than_shows() {
        // An upgraded install: state.json exists but predates the field.
        let mut state = State::default();
        let now = t0() + 365 * DAY;
        assert!(!nudge_due(&state, true, now));

        assert!(record_first_run(&mut state, now));
        assert_eq!(state.first_run, Some(unix_secs(now)));
        // Still not due: the week starts now.
        assert!(!nudge_due(&state, true, now));
        assert!(nudge_due(&state, true, now + NUDGE_DELAY));
    }

    #[test]
    fn record_first_run_never_overwrites() {
        let mut state = started_at(t0());
        assert!(!record_first_run(&mut state, t0() + 3 * DAY));
        assert_eq!(state.first_run, Some(unix_secs(t0())));
    }

    #[test]
    fn links() {
        assert_eq!(SPONSOR_URL, "https://github.com/sponsors/Rfluid");
        assert_eq!(PIX_URL, "https://livepix.gg/rfluid");
        assert!(SPONSOR_DOC_URL.starts_with(crate::REPO_URL));
        assert_eq!(NUDGE_DELAY.as_secs(), 604_800);
    }
}
