//! Shared frame-based timeline helpers.
//!
//! These functions expose reveal counts, stepped states, and transitions as deterministic
//! frame-in/value-out calculations.

/// Clamp to `[0, 1]`, treating NaN as 0 the way the source's `Math.max(0, Math.min(1, t))` does.
#[must_use]
pub fn clamp01(t: f32) -> f32 {
    if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) }
}

/// How many characters a typewriter has revealed at `local_frame`.
///
/// `cps` is characters per second; the whole string takes `len / cps` seconds, so `over` is that
/// span in frames. A non-positive span reveals everything at once, and a frame at or before zero
/// reveals nothing.
#[must_use]
pub fn reveal_count(local_frame: f32, fps: f32, len: usize, cps: f32) -> usize {
    let over = (len as f32 / cps) * fps;
    if over <= 0.0 {
        return len;
    }
    let progress = (local_frame / over) * len as f32;
    if progress.is_nan() || progress <= 0.0 {
        return 0;
    }
    let count = progress.floor() as i64;
    count.clamp(0, len as i64) as usize
}

/// The first `count` characters of `full`.
///
/// The source slices by UTF-16 code unit. This counts characters instead, which differs only for
/// text outside the basic multilingual plane; all shipped default strings use compatible text.
#[must_use]
pub fn revealed_text(full: &str, count: usize) -> String {
    full.chars().take(count).collect()
}

/// One step of a stepped animation: a state that begins at a frame, optionally with its own
/// transition duration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step<'a> {
    /// The frame this state begins at, in unscaled frames.
    pub at: f32,
    /// The state's name.
    pub state: &'a str,
    /// Frames the transition *into* this state takes; `None` uses the caller's default.
    pub duration: Option<f32>,
}

impl<'a> Step<'a> {
    /// A step with the caller's default transition duration.
    #[must_use]
    pub fn new(at: f32, state: &'a str) -> Self {
        Self {
            at,
            state,
            duration: None,
        }
    }

    /// A step with an explicit transition duration.
    #[must_use]
    pub fn with_duration(at: f32, state: &'a str, duration: f32) -> Self {
        Self {
            at,
            state,
            duration: Some(duration),
        }
    }
}

/// The state in effect at `frame` — `useCurrentState` in the source.
///
/// Ties are resolved in favour of the **later** entry in the list, because the source's scan
/// keeps a candidate when `step.at >= best_at`.
#[must_use]
pub fn current_state<'a>(steps: &[Step<'a>], default: &'a str, frame: f32, speed: f32) -> &'a str {
    let effective = frame * speed;
    let mut current = default;
    let mut best_at = f32::NEG_INFINITY;
    for step in steps {
        if step.at <= effective && step.at >= best_at {
            best_at = step.at;
            current = step.state;
        }
    }
    current
}

/// A state transition in flight: the state being left, the one being entered, and how far along
/// it is — `useStateTransition` in the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateTransition<'a> {
    /// The state being left.
    pub from: &'a str,
    /// The state being entered.
    pub to: &'a str,
    /// Progress through the transition, `0..=1`.
    pub progress: f32,
}

/// The transition in flight at `frame` — `useStateTransition` in the source.
///
/// Steps are ordered by their start frame, ties broken by their original index so a later entry
/// wins; steps that share a start frame with a later step never display, and so can never serve
/// as the `from` state.
#[must_use]
pub fn state_transition<'a>(
    steps: &[Step<'a>],
    default: &'a str,
    frame: f32,
    speed: f32,
    default_duration: f32,
) -> StateTransition<'a> {
    let effective = frame * speed;

    let mut index: Vec<usize> = (0..steps.len()).collect();
    index.sort_by(|&a, &b| {
        steps[a]
            .at
            .partial_cmp(&steps[b].at)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });

    let started: Vec<&Step<'a>> = index
        .into_iter()
        .map(|i| &steps[i])
        .filter(|step| step.at <= effective)
        .collect();

    // Same-`at` ties: the later entry wins, so an entry whose successor starts on the same frame
    // never displays and can never serve as the `from` state. The source drops exactly those.
    let started: Vec<&Step<'a>> = started
        .iter()
        .enumerate()
        .filter(|&(i, step)| i + 1 == started.len() || started[i + 1].at != step.at)
        .map(|(_, step)| *step)
        .collect();

    let Some(&to) = started.last() else {
        return StateTransition {
            from: default,
            to: default,
            progress: 1.0,
        };
    };

    let from = if started.len() >= 2 {
        started[started.len() - 2].state
    } else {
        default
    };

    let duration = to.duration.unwrap_or(default_duration);
    let progress = if duration > 0.0 {
        clamp01((effective - to.at) / duration)
    } else {
        1.0
    };

    StateTransition {
        from,
        to: to.state,
        progress,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_count_walks_the_string_and_clamps() {
        // 10 characters at 20 cps is half a second, i.e. 15 frames at 30 fps.
        assert_eq!(reveal_count(0.0, 30.0, 10, 20.0), 0);
        assert_eq!(reveal_count(15.0, 30.0, 10, 20.0), 10);
        assert_eq!(reveal_count(30.0, 30.0, 10, 20.0), 10);
        assert_eq!(reveal_count(-5.0, 30.0, 10, 20.0), 0);
        // Halfway through the reveal (frame 7.5 of 15) exactly half the characters are up.
        assert_eq!(reveal_count(7.5, 30.0, 10, 20.0), 5);
        // Just under the halfway mark, the floor keeps it at four.
        assert_eq!(reveal_count(7.4, 30.0, 10, 20.0), 4);
    }

    #[test]
    fn a_zero_length_string_is_immediately_complete() {
        assert_eq!(reveal_count(0.0, 30.0, 0, 20.0), 0);
        assert_eq!(reveal_count(10.0, 30.0, 0, 20.0), 0);
    }

    #[test]
    fn revealed_text_takes_a_character_prefix() {
        assert_eq!(revealed_text("Ship. Test.", 5), "Ship.");
        assert_eq!(revealed_text("Ship. Test.", 0), "");
        assert_eq!(revealed_text("Ship. Test.", 99), "Ship. Test.");
    }

    #[test]
    fn current_state_picks_the_last_started_step() {
        let steps = [Step::new(0.0, "idle"), Step::new(10.0, "typing")];
        assert_eq!(current_state(&steps, "idle", 5.0, 1.0), "idle");
        assert_eq!(current_state(&steps, "idle", 10.0, 1.0), "typing");
        assert_eq!(current_state(&steps, "idle", 100.0, 1.0), "typing");
    }

    #[test]
    fn current_state_breaks_ties_towards_the_later_entry() {
        let steps = [Step::new(4.0, "first"), Step::new(4.0, "second")];
        assert_eq!(current_state(&steps, "idle", 4.0, 1.0), "second");
    }

    #[test]
    fn speed_scales_the_frame_before_steps_are_matched() {
        let steps = [Step::new(10.0, "typing")];
        // At double speed, frame 5 is effective frame 10.
        assert_eq!(current_state(&steps, "idle", 5.0, 2.0), "typing");
        assert_eq!(current_state(&steps, "idle", 4.0, 2.0), "idle");
    }

    #[test]
    fn state_transition_reports_from_to_and_progress() {
        let steps = [
            Step::new(0.0, "idle"),
            Step::with_duration(10.0, "typing", 8.0),
        ];
        let before = state_transition(&steps, "idle", 5.0, 1.0, 8.0);
        assert_eq!((before.from, before.to), ("idle", "idle"));
        // Progress is measured from the *current* step's own start, not from the whole timeline,
        // so five frames into a step that declares eight frames of transition is 0.625.
        assert!(
            (before.progress - 0.625).abs() < 1e-6,
            "progress {}",
            before.progress
        );

        let mid = state_transition(&steps, "idle", 14.0, 1.0, 8.0);
        assert_eq!((mid.from, mid.to), ("idle", "typing"));
        assert!(
            (mid.progress - 0.5).abs() < 1e-6,
            "progress {}",
            mid.progress
        );

        let done = state_transition(&steps, "idle", 30.0, 1.0, 8.0);
        assert_eq!((done.from, done.to), ("idle", "typing"));
        assert!((done.progress - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_step_hidden_by_a_tie_never_becomes_the_from_state() {
        // Both steps start at 4; the later one wins, so the earlier is never displayed and the
        // transition goes straight from the default.
        let steps = [
            Step::new(0.0, "idle"),
            Step::new(4.0, "skipped"),
            Step::new(4.0, "shown"),
        ];
        let transition = state_transition(&steps, "idle", 5.0, 1.0, 8.0);
        assert_eq!(transition.to, "shown");
        assert_eq!(transition.from, "idle");
    }

    #[test]
    fn a_zero_duration_step_is_immediately_complete() {
        let steps = [Step::with_duration(10.0, "cut", 0.0)];
        let transition = state_transition(&steps, "idle", 10.0, 1.0, 8.0);
        assert!((transition.progress - 1.0).abs() < 1e-6);
    }

    #[test]
    fn clamp01_treats_nan_as_zero() {
        assert!((clamp01(f32::NAN) - 0.0).abs() < 1e-6);
        assert!((clamp01(-2.0) - 0.0).abs() < 1e-6);
        assert!((clamp01(2.0) - 1.0).abs() < 1e-6);
    }
}
