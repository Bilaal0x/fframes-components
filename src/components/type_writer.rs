//! # Type Writer
//!
//! Types its text one character at a time at a fixed per-character cadence, optionally fumbling a
//! wrong character now and then (and backspacing it before typing the right one), holding the
//! finished string, then either stopping or replaying the whole session. A blinking `|` cursor
//! rides the end of the last line.
//!
//! The animation is driven by a **simulated event timeline** rather than by a per-frame formula:
//! [`simulate_type_writer`] replays the entire session and records the string that is on screen at
//! every frame, exactly like the source component's `simulateTypeWriter`. The session is replayed
//! per frame — it is O(characters), pure and cheap — so a frame only has to look up the last event
//! that has already happened: typing, pausing, correcting a typo and clearing the screen all fall
//! out of one table. The timeline is frame-deterministic: typo decisions come from
//! [`crate::motion::random`], keyed on the seed, the string index and the character index, so every
//! render reproduces the same typos.
//!
//! Beat by beat, with the default options (one 56-character string, 3 frames per character):
//!
//! ```text
//! frame    0   the empty screen, cursor invisible (it fades in)
//! frame    3   "H"
//! frame    6   "He"
//! ...
//! frame  168   the whole string is on screen (56 characters at 3 frames each)
//! frame  198   end of the component: the 30 hold frames after the last character
//! ```
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `texts` | `Vec<String>` | `["Hello, World!\nThis text is typed character by character."]` | Strings typed one after another, each `\n` starting a new line. |
//! | `type_speed` | `usize` | `3` | Frames per typed character. |
//! | `delete_speed` | `usize` | `2` | Frames per deleted character. |
//! | `pause_after_type` | `usize` | `30` | Frames held after a string finishes typing. |
//! | `pause_after_delete` | `usize` | `15` | Frames held after a string is fully deleted. |
//! | `error_rate` | `f32` | `0.0` | `0..=1` per-character probability of a typo that is corrected. |
//! | `error_correct_delay` | `usize` | `5` | Frames a wrong character sits before being backspaced. |
//! | `seed` | `String` | `"typewriter"` | Seed for deterministic typos and wrong characters. |
//! | `cursor` | `bool` | `true` | Draw the blinking `\|` cursor. |
//! | `blink_speed` | `usize` | `30` | Full blink cycle in frames (half on, half off). |
//! | `show_cursor_after_complete` | `bool` | `true` | Keep the cursor blinking once the session has finished. |
//! | `delay` | `usize` | `0` | Frames to wait before typing starts. |
//! | `looping` | `bool` | `false` | Replay the whole timeline indefinitely (the source `loop` prop — `loop` is a Rust keyword). |
//! | `delete_before_next` | `bool` | `true` | Delete the previous string before typing the next one. When `false` the strings append to each other. |
//! | `tail` | `usize` | `30` | Frames held after the session ends. **Not in the source component** — see the README. |
//!
//! The styling is fixed, because the source component only exposes a free-form CSS `style` prop:
//! font size `42`, weight `400`, monospace family ([`RenderCtx::mono`]), line height `1.5`,
//! fill `#e6edf3`, background `#0d1117`. The block's left edge is pinned to the width the fully
//! typed text will need, so characters grow to the right instead of the line re-centring on
//! every frame.
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components type-writer
//! ```
//!
//! ## Credit
//!
//! Ported from `TypeWriter.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_left};
use crate::motion::{EASE_OUT, random};

/// Font size of the typed text, in px.
const FONT_SIZE: f32 = 42.0;
/// Font weight of the typed text (the source inherits the CSS default).
const FONT_WEIGHT: u16 = 400;
/// Line height as a multiple of [`FONT_SIZE`].
const LINE_HEIGHT: f32 = 1.5;
/// Fill of the typed text and of the cursor (`#e6edf3`, the source library's text colour).
const COLOR: &str = "#e6edf3";
/// Background painted behind the component (`#0d1117`, the source library's canvas colour).
const BACKGROUND: &str = "#0d1117";
/// The cursor glyph drawn after the last line.
const CURSOR: &str = "|";

/// The characters a typo may produce, in the source library's order (`KEYBOARD_CHARS`).
pub const KEYBOARD_CHARS: [char; 67] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4',
    '5', '6', '7', '8', '9', '.', ',', '!', '?', ' ',
];

/// Configuration for [`TypeWriter`].
#[derive(Debug, Clone, PartialEq)]
pub struct TypeWriterConfig {
    /// Strings typed one after another, each `\n` starting a new line.
    pub texts: Vec<String>,
    /// Frames per typed character.
    pub type_speed: usize,
    /// Frames per deleted character.
    pub delete_speed: usize,
    /// Frames held after a string finishes typing.
    pub pause_after_type: usize,
    /// Frames held after a string is fully deleted.
    pub pause_after_delete: usize,
    /// `0..=1` per-character probability of a typo that is corrected.
    pub error_rate: f32,
    /// Frames a wrong character sits before being backspaced.
    pub error_correct_delay: usize,
    /// Seed for deterministic typos and wrong characters.
    pub seed: String,
    /// Draw the blinking `|` cursor.
    pub cursor: bool,
    /// Full blink cycle in frames (half on, half off).
    pub blink_speed: usize,
    /// Keep the cursor blinking once the session has finished.
    pub show_cursor_after_complete: bool,
    /// Frames to wait before typing starts.
    pub delay: usize,
    /// Replay the whole timeline indefinitely.
    pub looping: bool,
    /// Delete the previous string before typing the next one.
    pub delete_before_next: bool,
    /// Frames held after the session ends, so the finished text (and the
    /// `show_cursor_after_complete` state) is actually visible.
    pub tail: usize,
}

impl Default for TypeWriterConfig {
    fn default() -> Self {
        Self {
            texts: vec!["Hello, World!\nThis text is typed character by character.".to_owned()],
            type_speed: 3,
            delete_speed: 2,
            pause_after_type: 30,
            pause_after_delete: 15,
            error_rate: 0.0,
            error_correct_delay: 5,
            seed: "typewriter".to_owned(),
            cursor: true,
            blink_speed: 30,
            show_cursor_after_complete: true,
            delay: 0,
            looping: false,
            delete_before_next: true,
            tail: 30,
        }
    }
}

/// One point on the typing timeline: the string on screen from `frame` onwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEvent {
    /// Frame at which this text becomes visible.
    pub frame: usize,
    /// The text displayed from this frame until the next event.
    pub text: String,
}

/// A simulated typing session: a sorted event timeline plus its total length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeWriterTimeline {
    /// Every event, in non-decreasing frame order.
    pub events: Vec<TimelineEvent>,
    /// Total frames of one session (the loop period when `looping` is on).
    pub total_duration: usize,
}

impl TypeWriterTimeline {
    /// The text on screen at `relative_frame`: the last event whose frame has already passed.
    ///
    /// Mirrors the source's lookup, including its behaviour when several events share a frame —
    /// the last one wins.
    pub fn text_at(&self, relative_frame: f32) -> String {
        let limit = relative_frame.max(0.0) as usize;
        let mut displayed = String::new();
        for event in &self.events {
            if event.frame > limit {
                break;
            }
            displayed.clone_from(&event.text);
        }
        displayed
    }
}

/// Push the current text onto the timeline at the current time.
fn push(events: &mut Vec<TimelineEvent>, time: usize, current: &[char]) {
    events.push(TimelineEvent {
        frame: time,
        text: current.iter().collect(),
    });
}

/// Backspace everything that is on screen, then wait out `pause_after_delete`.
fn clear_all(
    events: &mut Vec<TimelineEvent>,
    time: &mut usize,
    current: &mut Vec<char>,
    delete_speed: usize,
    pause_after_delete: usize,
) {
    while !current.is_empty() {
        let _ = current.pop();
        *time += delete_speed;
        push(events, *time, current);
    }
    *time += pause_after_delete;
}

/// Replay the whole typing session into a sorted event timeline.
///
/// This is the source component's `simulateTypeWriter`, ported verbatim: same increments, same
/// order of pushes, same `Math.round` (the clock only ever moves to whole frames, so it is the
/// identity here), same trailing-clear condition. It is pure and frame-deterministic, so the same
/// config always produces the same timeline — including the typos, which are sampled from
/// [`crate::motion::random`] with the source's `"{seed}-{str_index}-{i}"` keys.
///
/// The character buffer is a `Vec<char>` rather than a `&str`: JavaScript counts UTF-16 code
/// units and can split a surrogate pair when deleting, and a `Vec<char>` is the unicode-safe
/// equivalent of "one character".
pub fn simulate_type_writer(config: &TypeWriterConfig) -> TypeWriterTimeline {
    let mut events = vec![TimelineEvent {
        frame: 0,
        text: String::new(),
    }];
    let mut time: usize = 0;
    let mut current: Vec<char> = Vec::new();

    for (str_index, text) in config.texts.iter().enumerate() {
        if str_index > 0 && config.delete_before_next {
            clear_all(
                &mut events,
                &mut time,
                &mut current,
                config.delete_speed,
                config.pause_after_delete,
            );
        }
        for (i, ch) in text.chars().enumerate() {
            if random(&format!("{}-{}-{}", config.seed, str_index, i)) < config.error_rate {
                let wrong_seed = format!("{}-wrong-{}-{}", config.seed, str_index, i);
                // `Math.floor(random * len)`; the bound keeps a rounded-up 1.0 in range.
                let pick = (random(&wrong_seed) * KEYBOARD_CHARS.len() as f32).floor();
                let index = (pick.max(0.0) as usize).min(KEYBOARD_CHARS.len() - 1);
                current.push(KEYBOARD_CHARS[index]);
                time += config.type_speed;
                push(&mut events, time, &current);
                time += config.error_correct_delay;
                let _ = current.pop();
                push(&mut events, time, &current);
                time += config.delete_speed;
            }
            current.push(ch);
            time += config.type_speed;
            push(&mut events, time, &current);
        }
        time += config.pause_after_type;
    }

    // In append mode a looping run must still wipe the screen before restarting,
    // otherwise the same text appends onto itself forever.
    if config.looping && (config.delete_before_next || config.texts.len() > 1) {
        clear_all(
            &mut events,
            &mut time,
            &mut current,
            config.delete_speed,
            config.pause_after_delete,
        );
    }

    let total_duration = events.last().map_or(0, |event| event.frame);
    TypeWriterTimeline {
        events,
        total_duration,
    }
}

/// Total frames the typing session occupies (one cycle when looping).
///
/// The source's `getTypeWriterDuration`: [`TypeWriterConfig::default`] is the source's default
/// prop set, so calling this with a default config returns the default duration.
pub fn get_type_writer_duration(config: &TypeWriterConfig) -> usize {
    simulate_type_writer(config).total_duration
}

/// Cursor opacity at `clock`, on the unclamped clock.
///
/// A hard square on/off edge reads as a stutter at 30 fps, so the cursor ramps in and out of each
/// blink over [`BLINK_FADE_FRAMES`] frames with the library's shared ease-out curve.
fn blink_opacity(clock: f32, blink_speed: usize) -> f32 {
    // Guard blinkSpeed so a 0 or negative value cannot divide by zero.
    let safe_blink = blink_speed.max(2) as f32;
    let within_blink = clock % safe_blink;
    let half = safe_blink / 2.0;
    let fade = half.min(BLINK_FADE_FRAMES);
    if fade <= 0.0 {
        return 0.0;
    }
    let fade_in = EASE_OUT.ease((within_blink / fade).min(1.0));
    let fade_out = EASE_OUT.ease(((half - within_blink) / fade).min(1.0));
    fade_in.min(fade_out).clamp(0.0, 1.0)
}

/// Frames the cursor takes to fade in and out of each blink half-cycle.
const BLINK_FADE_FRAMES: f32 = 2.0;

/// Measurements that do not depend on the frame.
///
/// Measuring parses the embedded font face, so the frame-independent part — the width the fully
/// typed text needs, and the font's baseline offset — is taken once per component instance and
/// reused for every frame after that.
#[derive(Debug, Clone, Copy, Default)]
struct Metrics {
    /// x of the block's left edge: the fully typed text, centred.
    block_left: f32,
    /// Distance from the top of the line box to its baseline.
    baseline_offset: f32,
}

/// The Type Writer component.
#[derive(Debug, Clone, Default)]
pub struct TypeWriter {
    /// Component options.
    pub config: TypeWriterConfig,
    metrics: OnceLock<Metrics>,
}

impl TypeWriter {
    /// Build with the given options.
    pub fn new(config: TypeWriterConfig) -> Self {
        Self {
            config,
            metrics: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "type-writer",
            name: "Type Writer",
            summary: "Types text character by character, with typos, a cursor and an optional loop.",
            feature: "type-writer",
        }
    }

    /// Resolve the frame-independent measurements, measuring once per component instance.
    fn metrics(&self, ctx: &RenderCtx<'_>, family: &str) -> Metrics {
        *self.metrics.get_or_init(|| {
            // The block keeps the width of the longest fully typed line so the text grows to the
            // right instead of re-centring on every keystroke. The cursor's own advance is
            // reserved when it is drawn, so the finished line is centred with its cursor.
            let mut block_width = 0.0_f32;
            for text in &self.config.texts {
                for line in text.split('\n') {
                    block_width = block_width.max(ctx.text_width(family, FONT_SIZE, line));
                }
            }
            if self.config.cursor {
                block_width += ctx.text_width(family, FONT_SIZE, CURSOR);
            }
            Metrics {
                block_left: (ctx.width() - block_width) / 2.0,
                baseline_offset: ctx.baseline_offset(family, FONT_SIZE, FONT_SIZE * LINE_HEIGHT),
            }
        })
    }
}

impl Component for TypeWriter {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        (self.config.delay + get_type_writer_duration(&self.config) + self.config.tail).max(1)
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKGROUND)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let timeline = simulate_type_writer(&self.config);
        let family = ctx.mono();

        let clock = (ctx.frame() - self.config.delay as f32).max(0.0);
        let total_duration = timeline.total_duration as f32;
        let relative_frame = if total_duration > 0.0 {
            if self.config.looping {
                clock % total_duration
            } else {
                clock.min(total_duration)
            }
        } else {
            0.0
        };

        let displayed = timeline.text_at(relative_frame);
        let complete = !self.config.looping && relative_frame >= total_duration;
        let cursor_visible = !complete || self.config.show_cursor_after_complete;
        let cursor_opacity = if cursor_visible {
            blink_opacity(clock, self.config.blink_speed)
        } else {
            0.0
        };

        let metrics = self.metrics(ctx, &family);
        let line_step = FONT_SIZE * LINE_HEIGHT;
        let lines: Vec<&str> = displayed.split('\n').collect();
        let block_height = line_step * lines.len() as f32;
        let first_baseline = ctx.height() / 2.0 - block_height / 2.0 + metrics.baseline_offset;

        let mut nodes: Vec<Svgr<'_>> = Vec::with_capacity(lines.len() + 1);
        for (index, line) in lines.iter().enumerate() {
            nodes.push(text_node_left(
                (*line).to_owned(),
                metrics.block_left,
                first_baseline + index as f32 * line_step,
                family.clone(),
                FONT_SIZE,
                FONT_WEIGHT,
                0.0,
                COLOR.to_owned(),
                1.0,
            ));
        }

        if self.config.cursor {
            let last = lines.len().saturating_sub(1);
            let last_line = lines.get(last).copied().unwrap_or("");
            let x = metrics.block_left + ctx.text_width(&family, FONT_SIZE, last_line);
            let y = first_baseline + last as f32 * line_step;
            nodes.push(text_node_left(
                CURSOR.to_owned(),
                x,
                y,
                family,
                FONT_SIZE,
                FONT_WEIGHT,
                0.0,
                COLOR.to_owned(),
                cursor_opacity,
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(texts: &[&str]) -> TypeWriterConfig {
        TypeWriterConfig {
            texts: texts.iter().map(|text| (*text).to_owned()).collect(),
            ..TypeWriterConfig::default()
        }
    }

    fn frames(timeline: &TypeWriterTimeline) -> Vec<usize> {
        timeline.events.iter().map(|event| event.frame).collect()
    }

    fn texts(timeline: &TypeWriterTimeline) -> Vec<&str> {
        timeline
            .events
            .iter()
            .map(|event| event.text.as_str())
            .collect()
    }

    #[test]
    fn simulate_types_characters_at_type_speed() {
        let timeline = simulate_type_writer(&config(&["ab"]));
        assert_eq!(frames(&timeline), vec![0, 3, 6]);
        assert_eq!(texts(&timeline), vec!["", "a", "ab"]);
        assert_eq!(timeline.total_duration, 6);
    }

    #[test]
    fn non_looping_session_ends_on_the_last_typed_character() {
        // 56 characters at 3 frames each; the trailing pause is not part of the duration.
        let timeline = simulate_type_writer(&TypeWriterConfig::default());
        assert_eq!(timeline.events.len(), 57);
        assert_eq!(timeline.total_duration, 168);
        assert_eq!(get_type_writer_duration(&TypeWriterConfig::default()), 168);
        assert_eq!(
            timeline.text_at(168.0),
            TypeWriterConfig::default().texts[0]
        );
        assert_eq!(timeline.text_at(167.0).chars().count(), 55);
    }

    #[test]
    fn delete_before_next_clears_the_screen_between_strings() {
        let timeline = simulate_type_writer(&config(&["a", "b"]));
        assert_eq!(frames(&timeline), vec![0, 3, 35, 53]);
        assert_eq!(texts(&timeline), vec!["", "a", "", "b"]);
        assert_eq!(timeline.total_duration, 53);
    }

    #[test]
    fn append_mode_keeps_the_previous_string() {
        let timeline = simulate_type_writer(&TypeWriterConfig {
            delete_before_next: false,
            ..config(&["a", "b"])
        });
        assert_eq!(frames(&timeline), vec![0, 3, 36]);
        assert_eq!(texts(&timeline), vec!["", "a", "ab"]);
        assert_eq!(timeline.total_duration, 36);
    }

    #[test]
    fn looping_clears_the_screen_for_the_next_cycle() {
        let timeline = simulate_type_writer(&TypeWriterConfig {
            looping: true,
            ..config(&["ab"])
        });
        // Type "ab" (6), hold 30, then backspace both (38, 40).
        assert_eq!(frames(&timeline), vec![0, 3, 6, 38, 40]);
        assert_eq!(texts(&timeline), vec!["", "a", "ab", "a", ""]);
        assert_eq!(timeline.total_duration, 40);
    }

    #[test]
    fn looping_in_append_mode_with_one_string_does_not_clear() {
        let timeline = simulate_type_writer(&TypeWriterConfig {
            looping: true,
            delete_before_next: false,
            ..config(&["ab"])
        });
        assert_eq!(frames(&timeline), vec![0, 3, 6]);
        assert_eq!(timeline.total_duration, 6);
    }

    #[test]
    fn looping_in_append_mode_with_several_strings_clears() {
        let timeline = simulate_type_writer(&TypeWriterConfig {
            looping: true,
            delete_before_next: false,
            ..config(&["a", "b"])
        });
        // "a" (3), 30 hold, "ab" (36), 30 hold, then backspace both and pause.
        assert_eq!(frames(&timeline), vec![0, 3, 36, 68, 70]);
        assert_eq!(texts(&timeline), vec!["", "a", "ab", "a", ""]);
        assert_eq!(timeline.total_duration, 70);
    }

    #[test]
    fn error_path_is_deterministic_and_keeps_the_timeline_sorted() {
        let typo_config = TypeWriterConfig {
            error_rate: 0.5,
            seed: "typo-seed".to_owned(),
            ..config(&["hello world"])
        };
        let first = simulate_type_writer(&typo_config);
        let second = simulate_type_writer(&typo_config);
        assert_eq!(first, second);

        // A typo costs its own type frame, the correct-delay and the delete: 10 extra frames,
        // and two extra events.
        let typos = (0..11)
            .filter(|i| random(&format!("typo-seed-0-{i}")) < 0.5)
            .count();
        assert!(typos > 0, "this seed must produce at least one typo");
        assert_eq!(first.total_duration, 33 + typos * 10);
        assert_eq!(first.events.len(), 12 + typos * 2);
        assert!(
            first
                .events
                .windows(2)
                .all(|pair| pair[0].frame <= pair[1].frame),
            "events must stay in non-decreasing frame order"
        );
        // Every typo character comes out of the keyboard alphabet.
        for event in &first.events {
            for ch in event.text.chars() {
                let known = KEYBOARD_CHARS.contains(&ch) || "hello world".contains(ch);
                assert!(known, "unexpected character {ch:?}");
            }
        }
    }

    #[test]
    fn a_full_error_rate_terminates_and_always_corrects_itself() {
        let noisy = simulate_type_writer(&TypeWriterConfig {
            // Above 1.0 so the assertion cannot depend on `random` landing under the rate.
            error_rate: 2.0,
            ..config(&["abc"])
        });
        let clean = simulate_type_writer(&config(&["abc"]));
        assert_eq!(clean.total_duration, 9);
        // Each character costs type + correct-delay + delete + type = 3 + 5 + 2 + 3 = 13.
        assert_eq!(noisy.total_duration, 39);
        assert_eq!(noisy.events.len(), 1 + 3 * 3);
        assert_eq!(
            noisy.events.last().map(|event| event.text.as_str()),
            Some("abc")
        );
        assert_eq!(KEYBOARD_CHARS.len(), 67);
    }

    #[test]
    fn unicode_is_counted_in_characters_not_bytes() {
        let timeline = simulate_type_writer(&config(&["éü"]));
        assert_eq!(timeline.total_duration, 6);
        assert_eq!(texts(&timeline), vec!["", "é", "éü"]);
    }

    #[test]
    fn text_at_returns_the_last_event_that_has_passed() {
        let timeline = simulate_type_writer(&config(&["ab"]));
        assert_eq!(timeline.text_at(0.0), "");
        assert_eq!(timeline.text_at(2.9), "");
        assert_eq!(timeline.text_at(3.0), "a");
        assert_eq!(timeline.text_at(5.75), "a");
        assert_eq!(timeline.text_at(6.0), "ab");
        assert_eq!(timeline.text_at(10_000.0), "ab");
        assert_eq!(timeline.text_at(-5.0), "");
        assert_eq!(timeline.text_at(f32::NAN), "");
    }

    #[test]
    fn blink_ramps_and_stays_in_range() {
        assert!((blink_opacity(0.0, 30) - 0.0).abs() < 1e-6);
        assert!(blink_opacity(1.0, 30) > 0.0);
        assert!((blink_opacity(2.0, 30) - 1.0).abs() < 1e-6);
        assert!((blink_opacity(10.0, 30) - 1.0).abs() < 1e-6);
        // The second half fades out: fully dark at the halfway point and at the end of the cycle.
        assert!((blink_opacity(15.0, 30) - 0.0).abs() < 1e-6);
        assert!((blink_opacity(29.0, 30) - 0.0).abs() < 1e-6);
        // A zero or tiny blink speed is clamped to a 2-frame cycle, never a division by zero.
        assert!(blink_opacity(0.0, 0).is_finite());
        assert!((blink_opacity(0.0, 1) - 0.0).abs() < 1e-6);
        // A 2 frame cycle is all ramp: bright in the middle, dark at its two ends.
        assert!(blink_opacity(0.5, 2) > 0.0);
        assert!((blink_opacity(1.0, 2) - 0.0).abs() < 1e-6);
        for frame in 0..120 {
            let opacity = blink_opacity(frame as f32, 30);
            assert!(
                (0.0..=1.0).contains(&opacity),
                "opacity {opacity} out of range"
            );
        }
    }

    #[test]
    fn duration_covers_delay_session_and_tail() {
        let component = TypeWriter::default();
        // 56 characters at 3 frames each, plus the 30 frame tail.
        assert_eq!(component.duration(), 198);
        assert_eq!(component.meta().id, "type-writer");
        assert_eq!(component.background(), Some("#0d1117"));
        let shifted = TypeWriter::new(TypeWriterConfig {
            delay: 10,
            tail: 5,
            ..config(&["ab"])
        });
        assert_eq!(shifted.duration(), 10 + 6 + 5);
    }

    #[test]
    fn empty_texts_still_report_a_positive_duration() {
        let component = TypeWriter::new(TypeWriterConfig {
            texts: Vec::new(),
            tail: 0,
            ..TypeWriterConfig::default()
        });
        assert_eq!(get_type_writer_duration(&component.config), 0);
        assert_eq!(component.duration(), 1);
    }

    /// Renders real frames through `show`, which is the only way to catch a panic or a NaN in the
    /// layout. It does not assert on the SVG markup itself.
    #[test]
    fn show_renders_frames_without_panicking() {
        let component = TypeWriter::default();
        let meters = crate::embedded_meters();
        let fonts = crate::core::Fonts::default();
        let mut finished = String::new();
        for local in [0.0_f32, 1.0, 3.0, 84.0, 167.0, 168.0, 197.0] {
            let mut frame = fframes::Frame::new(local as usize, local as usize, 30);
            let info = crate::ctx::ComponentFrame {
                local,
                global: local as usize,
                fps: 30.0,
                duration: component.duration(),
                width: crate::core::CANVAS_WIDTH as f32,
                height: crate::core::CANVAS_HEIGHT as f32,
            };
            let mut ctx = RenderCtx::new(&mut frame, fonts.clone(), &meters, info);
            let svg = component.show(&mut ctx).to_string();
            assert!(!svg.is_empty(), "no SVG at frame {local}");
            if local == 197.0 {
                finished = svg;
            }
        }
        // The tail renders the finished string, newline split into two lines.
        assert!(finished.contains("Hello, World!"), "{finished}");
        assert!(
            finished.contains("This text is typed character by character."),
            "{finished}"
        );
    }
}
