//! # Multi Text Typewriter
//!
//! Cycles through several strings: each one is typed character by character, held, then
//! backspaced away before the next one starts. When the last string has been deleted the cycle
//! restarts from the first, so the component plays as an endless loop. A blinking `|` cursor
//! rides the end of the line and keeps blinking on the unclamped clock — through the deletions
//! and through the empty gaps between strings.
//!
//! Like [`crate::components::type_writer`], the animation is driven by a **simulated event
//! timeline**: `simulateCycle` replays one complete type-hold-delete cycle and records the string
//! that is on screen at every frame. The cycle is replayed per frame — it is O(characters), pure
//! and cheap — so a frame only has to look up the last event that has already happened. There is
//! no randomness here, so the timeline is trivially frame-deterministic.
//!
//! Beat by beat, with the default options (`"Design."`, `"Develop."`, `"Deploy."`, `"Repeat."`),
//! the first two strings of the cycle:
//!
//! ```text
//! frame    0   the empty screen, cursor invisible (it fades in)
//! frame    3   "D"
//! ...
//! frame   21   "Design." complete
//! frame   51   the 30 frame hold ends, the backspaces start (the first lands at 53)
//! ...
//! frame   65   "Design." is gone, the 15 frame gap starts
//! frame   83   "D" of "Develop."
//! ...
//! frame  325   end of the component: back to empty, the 15 frame gap has been played out
//! ```
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `texts` | `Vec<String>` | `["Design.", "Develop.", "Deploy.", "Repeat."]` | Strings cycled through: type, hold, backspace, next. |
//! | `type_speed` | `usize` | `3` | Frames per typed character. |
//! | `delete_speed` | `usize` | `2` | Frames per backspaced character. |
//! | `pause_after_type` | `usize` | `30` | Frames the completed line holds before deleting starts. |
//! | `pause_after_delete` | `usize` | `15` | Frames the empty line holds before the next string starts. |
//! | `cursor` | `bool` | `true` | Draw the blinking `\|` cursor. |
//! | `blink_speed` | `usize` | `30` | Full blink cycle in frames (half on, half off). |
//! | `delay` | `usize` | `0` | Frames to wait before typing starts. |
//!
//! The styling is fixed, because the source component only exposes a free-form CSS `style` prop:
//! font size `56`, weight `700`, monospace family ([`RenderCtx::mono`]), line height `1.2`, fill
//! `#e6edf3`, background `#0d1117`. A `\n` inside a string still starts a new line. The block's
//! left edge is pinned to the width the longest string will need, so characters grow to the right
//! instead of the line re-centring on every frame.
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components multi-text-typewriter
//! ```
//!
//! ## Credit
//!
//! Ported from `MultiTextTypewriter.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_left};
use crate::motion::EASE_OUT;

/// Font size of the typed text, in px.
const FONT_SIZE: f32 = 56.0;
/// Font weight of the typed text (`700`, as the source component sets it).
const FONT_WEIGHT: u16 = 700;
/// Line height as a multiple of [`FONT_SIZE`].
const LINE_HEIGHT: f32 = 1.2;
/// Fill of the typed text and of the cursor (`#e6edf3`, the source library's text colour).
const COLOR: &str = "#e6edf3";
/// Background painted behind the component (`#0d1117`, the source library's canvas colour).
const BACKGROUND: &str = "#0d1117";
/// The cursor glyph drawn after the last line.
const CURSOR: &str = "|";
/// Frames the cursor takes to fade in and out of each blink half-cycle.
const BLINK_FADE_FRAMES: f32 = 2.0;

/// Configuration for [`MultiTextTypewriter`].
#[derive(Debug, Clone, PartialEq)]
pub struct MultiTextTypewriterConfig {
    /// Strings cycled through: type, hold, backspace, next.
    pub texts: Vec<String>,
    /// Frames per typed character.
    pub type_speed: usize,
    /// Frames per backspaced character.
    pub delete_speed: usize,
    /// Frames the completed line holds before deleting starts.
    pub pause_after_type: usize,
    /// Frames the empty line holds before the next string starts.
    pub pause_after_delete: usize,
    /// Draw the blinking `|` cursor.
    pub cursor: bool,
    /// Full blink cycle in frames (half on, half off).
    pub blink_speed: usize,
    /// Frames to wait before typing starts.
    pub delay: usize,
}

impl Default for MultiTextTypewriterConfig {
    fn default() -> Self {
        Self {
            texts: vec![
                "Design.".to_owned(),
                "Develop.".to_owned(),
                "Deploy.".to_owned(),
                "Repeat.".to_owned(),
            ],
            type_speed: 3,
            delete_speed: 2,
            pause_after_type: 30,
            pause_after_delete: 15,
            cursor: true,
            blink_speed: 30,
            delay: 0,
        }
    }
}

/// One point on the cycle timeline: the string on screen from `frame` onwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEvent {
    /// Frame at which this text becomes visible.
    pub frame: usize,
    /// The text displayed from this frame until the next event.
    pub text: String,
}

/// One simulated type-hold-delete cycle: a sorted event timeline plus its total length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleTimeline {
    /// Every event, in non-decreasing frame order.
    pub events: Vec<TimelineEvent>,
    /// Total frames of one complete cycle, including the trailing pause.
    pub total_duration: usize,
}

impl CycleTimeline {
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

/// Replay one complete type-hold-delete cycle into a sorted event timeline.
///
/// This is the source component's `simulateCycle`, ported verbatim: same increments, same order of
/// pushes, same `Math.round` on the clock (the clock only ever moves to whole frames, so it is the
/// identity here). The trailing pause *is* part of the cycle, so the empty screen is held before
/// the loop restarts rather than snapping straight back into the first word.
///
/// The character buffer is a `Vec<char>` rather than a `&str`: JavaScript counts UTF-16 code units
/// and can split a surrogate pair when deleting, and a `Vec<char>` is the unicode-safe equivalent
/// of "one character".
pub fn simulate_cycle(config: &MultiTextTypewriterConfig) -> CycleTimeline {
    let mut events = vec![TimelineEvent {
        frame: 0,
        text: String::new(),
    }];
    let mut time: usize = 0;
    let mut current: Vec<char> = Vec::new();

    for text in &config.texts {
        for ch in text.chars() {
            current.push(ch);
            time += config.type_speed;
            push(&mut events, time, &current);
        }
        time += config.pause_after_type;
        while !current.is_empty() {
            let _ = current.pop();
            time += config.delete_speed;
            push(&mut events, time, &current);
        }
        time += config.pause_after_delete;
    }

    CycleTimeline {
        events,
        total_duration: time,
    }
}

/// Total frames of one complete type-and-delete cycle.
///
/// The source's `getMultiTextTypewriterDuration`: [`MultiTextTypewriterConfig::default`] is the
/// source's default prop set, so calling this with a default config returns the default duration.
pub fn get_multi_text_typewriter_duration(config: &MultiTextTypewriterConfig) -> usize {
    simulate_cycle(config).total_duration
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

/// Measurements that do not depend on the frame.
///
/// Measuring parses the embedded font face, so the frame-independent part — the width the longest
/// string needs, and the font's baseline offset — is taken once per component instance and reused
/// for every frame after that.
#[derive(Debug, Clone, Copy, Default)]
struct Metrics {
    /// x of the block's left edge: the longest string, centred.
    block_left: f32,
    /// Distance from the top of the line box to its baseline.
    baseline_offset: f32,
}

/// The Multi Text Typewriter component.
#[derive(Debug, Clone, Default)]
pub struct MultiTextTypewriter {
    /// Component options.
    pub config: MultiTextTypewriterConfig,
    metrics: OnceLock<Metrics>,
}

impl MultiTextTypewriter {
    /// Build with the given options.
    pub fn new(config: MultiTextTypewriterConfig) -> Self {
        Self {
            config,
            metrics: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "multi-text-typewriter",
            name: "Multi Text Typewriter",
            summary: "Types, holds and backspaces a list of strings in an endless cycle.",
            feature: "multi-text-typewriter",
        }
    }

    /// Resolve the frame-independent measurements, measuring once per component instance.
    fn metrics(&self, ctx: &RenderCtx<'_>, family: &str) -> Metrics {
        *self.metrics.get_or_init(|| {
            // The block keeps the width of the longest line of the longest string, so the text
            // grows to the right instead of re-centring on every keystroke. The cursor's own
            // advance is reserved when it is drawn, so the line is centred with its cursor.
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

impl Component for MultiTextTypewriter {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        (self.config.delay + get_multi_text_typewriter_duration(&self.config)).max(1)
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKGROUND)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let timeline = simulate_cycle(&self.config);
        let family = ctx.mono();

        // The cursor blinks on the unclamped clock, so it keeps blinking through the deletions
        // and the gaps between strings.
        let clock = (ctx.frame() - self.config.delay as f32).max(0.0);
        let total_duration = timeline.total_duration as f32;
        let relative_frame = if total_duration > 0.0 {
            clock % total_duration
        } else {
            0.0
        };

        let cursor_opacity = blink_opacity(clock, self.config.blink_speed);
        let displayed = timeline.text_at(relative_frame);

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

    fn config(texts: &[&str]) -> MultiTextTypewriterConfig {
        MultiTextTypewriterConfig {
            texts: texts.iter().map(|text| (*text).to_owned()).collect(),
            ..MultiTextTypewriterConfig::default()
        }
    }

    fn frames(timeline: &CycleTimeline) -> Vec<usize> {
        timeline.events.iter().map(|event| event.frame).collect()
    }

    fn texts(timeline: &CycleTimeline) -> Vec<&str> {
        timeline
            .events
            .iter()
            .map(|event| event.text.as_str())
            .collect()
    }

    #[test]
    fn cycle_types_holds_backspaces_and_holds_again() {
        let timeline = simulate_cycle(&config(&["ab"]));
        assert_eq!(frames(&timeline), vec![0, 3, 6, 38, 40]);
        assert_eq!(texts(&timeline), vec!["", "a", "ab", "a", ""]);
        // The last event is the empty screen at 40; the trailing pause takes the cycle to 55.
        assert_eq!(timeline.total_duration, 55);
    }

    #[test]
    fn default_cycle_duration_and_event_count() {
        let component = MultiTextTypewriter::default();
        let timeline = simulate_cycle(&component.config);
        // "Design." 80 + "Develop." 85 + "Deploy." 80 + "Repeat." 80.
        assert_eq!(timeline.total_duration, 325);
        // One opening event plus one push per typed and per backspaced character.
        assert_eq!(timeline.events.len(), 1 + 2 * 29);
        assert_eq!(get_multi_text_typewriter_duration(&component.config), 325);
        assert_eq!(component.duration(), 325);
        assert_eq!(component.meta().id, "multi-text-typewriter");
        assert_eq!(component.background(), Some("#0d1117"));
    }

    #[test]
    fn cycle_is_deterministic_and_sorted() {
        let first = simulate_cycle(&MultiTextTypewriterConfig::default());
        let second = simulate_cycle(&MultiTextTypewriterConfig::default());
        assert_eq!(first, second);
        assert!(
            first
                .events
                .windows(2)
                .all(|pair| pair[0].frame <= pair[1].frame),
            "events must stay in non-decreasing frame order"
        );
    }

    #[test]
    fn cycle_wraps_on_the_total_duration() {
        let timeline = simulate_cycle(&config(&["ab"]));
        // Text lookup inside one cycle.
        assert_eq!(timeline.text_at(0.0), "");
        assert_eq!(timeline.text_at(3.0), "a");
        assert_eq!(timeline.text_at(6.0), "ab");
        assert_eq!(timeline.text_at(37.0), "ab");
        assert_eq!(timeline.text_at(38.0), "a");
        assert_eq!(timeline.text_at(40.0), "");
        // The trailing pause stays empty, and the next cycle is the same table again.
        assert_eq!(timeline.text_at(54.0), "");
        // `show` takes the clock modulo the cycle, so clock 58 lands on frame 3 of the next one.
        let wrapped = 58.0_f32 % timeline.total_duration as f32;
        assert!((wrapped - 3.0).abs() < 1e-6);
        assert_eq!(timeline.text_at(wrapped), timeline.text_at(3.0));
    }

    #[test]
    fn unicode_is_counted_in_characters_not_bytes() {
        let timeline = simulate_cycle(&config(&["éü"]));
        assert_eq!(timeline.total_duration, 2 * 3 + 30 + 2 * 2 + 15);
        assert_eq!(texts(&timeline), vec!["", "é", "éü", "é", ""]);
    }

    #[test]
    fn empty_texts_still_report_a_positive_duration() {
        let component = MultiTextTypewriter::new(config(&[]));
        assert_eq!(get_multi_text_typewriter_duration(&component.config), 0);
        assert_eq!(component.duration(), 1);
    }

    /// Renders real frames through `show`, which is the only way to catch a panic or a NaN in the
    /// layout. It does not assert on the SVG markup itself.
    #[test]
    fn show_renders_frames_without_panicking() {
        let component = MultiTextTypewriter::default();
        let meters = crate::embedded_meters();
        let fonts = crate::core::Fonts::default();
        let mut typed = String::new();
        for local in [0.0_f32, 1.0, 3.0, 21.0, 40.0, 65.0, 83.0, 310.0, 324.0] {
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
            if local == 21.0 {
                typed = svg;
            }
        }
        assert!(typed.contains("Design."), "{typed}");
    }

    #[test]
    fn delay_is_added_on_top_of_one_cycle() {
        let component = MultiTextTypewriter::new(MultiTextTypewriterConfig {
            delay: 12,
            ..config(&["ab"])
        });
        assert_eq!(component.duration(), 12 + 55);
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
}
