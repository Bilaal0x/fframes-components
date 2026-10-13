//! # Slide In Text
//!
//! Words slide in from one edge of the frame and ease into their resting places, one after
//! another. Every word travels the same vector and shares the library's single `EASE_OUT`
//! entrance curve, so each word moves as a unit while the stagger cascades them; opacity rides
//! the very same eased progress, so a word is fully faded in exactly when it stops moving.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text.text` | `String` | `"Slides in from an edge"` | The line to reveal. |
//! | `text.font_size` | `f32` | `64.0` | Font size in px. |
//! | `text.font_weight` | `u16` | `700` | Font weight. |
//! | `text.color` | `String` | `"#e6edf3"` | Text fill. |
//! | `text.max_width` | `f32` | `900.0` | Wrap width in px. |
//! | `text.line_height` | `f32` | `1.25` | Line height as a multiple of `font_size`. |
//! | `text.tracking_em` | `f32` | `-0.02` | Letter spacing in `em`. |
//! | `direction` | [`SlideDirection`] | `Bottom` | Edge the words enter from; `top` slides down, `bottom` slides up. |
//! | `duration` | `usize` | `30` | Frames each word takes to arrive. |
//! | `stagger` | `usize` | `1` | Frames between consecutive word starts. |
//! | `delay` | `usize` | `0` | Frames to wait before the first word starts. |
//! | `distance` | `f32` | `40.0` | Pixels each word travels in from its resting place. |
//! | `tail` | `usize` | `45` | Frames held after the last word lands. |
//! | `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components slide-in-text -o out.mp4
//! ```
//!
//! ## Fidelity
//!
//! The port reproduces the source exactly: this animation uses no channel SVG cannot express.
//! The offset vector is used linearly, the travel is a plain 2D translation, and the
//! `EASE_OUT` clock is the library's own. Two differences are worth knowing, and both are
//! cosmetic: words are split on any run of whitespace (the shared `ctx::words`) where the source
//! splits on the literal space character, and the words are laid out by the shared wrapper rather
//! than left in the inline flow of a `<span>`.
//!
//! ## Credit
//!
//! Ported from `SlideInText.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{LayoutCache, RenderCtx, text_node_centered};
use crate::motion::{EASE_OUT, clamp01, motion_scale};
use crate::text_spec::TextSpec;

/// Frames held after the last word lands by [`SlideInTextConfig::default`].
pub const DEFAULT_TAIL: usize = 45;

/// The edge the words enter from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlideDirection {
    /// Words enter from the left and travel right.
    Left,
    /// Words enter from the right and travel left.
    Right,
    /// Words enter from above and travel down.
    Top,
    /// Words enter from below and travel up. The source's default.
    #[default]
    Bottom,
}

impl SlideDirection {
    /// The unit offset a word starts at, as `(x, y)`: `left { x: -1, y: 0 }`,
    /// `right { x: 1, y: 0 }`, `top { x: 0, y: -1 }`, `bottom { x: 0, y: 1 }`.
    pub const fn offset(self) -> (f32, f32) {
        match self {
            Self::Left => (-1.0, 0.0),
            Self::Right => (1.0, 0.0),
            Self::Top => (0.0, -1.0),
            Self::Bottom => (0.0, 1.0),
        }
    }
}

/// Configuration for [`SlideInText`].
#[derive(Debug, Clone, PartialEq)]
pub struct SlideInTextConfig {
    /// Text styling and layout.
    pub text: TextSpec,
    /// Edge the words enter from.
    pub direction: SlideDirection,
    /// Frames each word takes to arrive.
    pub duration: usize,
    /// Frames between consecutive word starts.
    pub stagger: usize,
    /// Frames to wait before the first word starts.
    pub delay: usize,
    /// Pixels each word travels in from its resting place.
    pub distance: f32,
    /// Frames held after the last word lands.
    pub tail: usize,
    /// Collapse travel; keep the fades.
    pub reduced_motion: bool,
}

impl Default for SlideInTextConfig {
    fn default() -> Self {
        Self {
            text: TextSpec {
                text: "Slides in from an edge".to_owned(),
                ..TextSpec::default()
            },
            direction: SlideDirection::Bottom,
            duration: 30,
            stagger: 1,
            delay: 0,
            distance: 40.0,
            tail: DEFAULT_TAIL,
            reduced_motion: false,
        }
    }
}

/// The animated channels of one word, sampled at one frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlideChannels {
    /// Opacity, from the eased progress.
    pub opacity: f32,
    /// Horizontal travel still to go, in px, in the direction the word came from.
    pub dx: f32,
    /// Vertical travel still to go, in px, in the direction the word came from.
    pub dy: f32,
}

/// The unit offset a direction starts a word at.
pub const fn direction_offset(direction: SlideDirection) -> (f32, f32) {
    direction.offset()
}

/// Frames until the last word lands (entry only, no hold tail).
///
/// Zero words take no frames, so the caller never has to guard the subtraction.
pub fn entry_frames(word_count: usize, duration: usize, stagger: usize) -> usize {
    if word_count == 0 {
        return 0;
    }
    duration + (word_count - 1) * stagger
}

/// Per-word progress: stagger shifts each word's clock back, so word `i` starts `i * stagger`
/// frames later than word 0.
///
/// `relative = max(0, frame - delay) - index * stagger`, then `clamp01(relative / duration)` —
/// verbatim from the source.
pub fn word_progress(
    frame: f32,
    index: usize,
    duration: usize,
    stagger: usize,
    delay: usize,
) -> f32 {
    let relative = (frame - delay as f32).max(0.0) - index as f32 * stagger as f32;
    clamp01(relative / duration.max(1) as f32)
}

/// The opacity and remaining translation of one word at raw progress `t`.
///
/// `progress = EASE_OUT.ease(t)`; `opacity = progress`; each axis is
/// `(1 - progress) * offset * distance * motionScale(reduced)`.
pub fn word_channels(
    t: f32,
    direction: SlideDirection,
    distance: f32,
    reduced_motion: bool,
) -> SlideChannels {
    let progress = EASE_OUT.ease(clamp01(t));
    let (ox, oy) = direction.offset();
    let travel = (1.0 - progress) * distance * motion_scale(reduced_motion);
    SlideChannels {
        opacity: progress,
        dx: ox * travel,
        dy: oy * travel,
    }
}

/// The Slide In Text component.
#[derive(Debug, Clone, Default)]
pub struct SlideInText {
    /// Component options.
    pub config: SlideInTextConfig,
    layout: LayoutCache,
}

impl SlideInText {
    /// Build with the given options.
    pub fn new(config: SlideInTextConfig) -> Self {
        Self {
            config,
            layout: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "slide-in-text",
            name: "Slide In Text",
            summary: "Words slide in from one edge and ease into place, staggered.",
            feature: "slide-in-text",
        }
    }
}

impl Component for SlideInText {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        entry_frames(
            self.config.text.word_count(),
            self.config.duration,
            self.config.stagger,
        ) + self.config.tail
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let spec = &self.config.text;
        let family = ctx.fonts.sans.clone();
        let layout = self.layout.resolve(
            ctx,
            &family,
            &spec.text,
            spec.font_size,
            spec.max_width,
            spec.line_height,
        );

        let frame = ctx.frame();
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, word, baseline) in layout.all_words(center_y) {
            let channels = word_channels(
                word_progress(
                    frame,
                    index,
                    self.config.duration,
                    self.config.stagger,
                    self.config.delay,
                ),
                self.config.direction,
                self.config.distance,
                self.config.reduced_motion,
            );
            nodes.push(text_node_centered(
                &word.text,
                center_x + word.x + channels.dx,
                baseline + channels.dy,
                family.clone(),
                spec.font_size,
                spec.font_weight,
                spec.tracking_em,
                spec.color.clone(),
                channels.opacity,
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_offsets_match_the_source() {
        assert_eq!(direction_offset(SlideDirection::Left), (-1.0, 0.0));
        assert_eq!(direction_offset(SlideDirection::Right), (1.0, 0.0));
        assert_eq!(direction_offset(SlideDirection::Top), (0.0, -1.0));
        assert_eq!(direction_offset(SlideDirection::Bottom), (0.0, 1.0));
        assert_eq!(SlideDirection::default(), SlideDirection::Bottom);
    }

    #[test]
    fn progress_staggers_and_clamps() {
        assert!((word_progress(0.0, 0, 10, 2, 0) - 0.0).abs() < 1e-6);
        assert!((word_progress(2.0, 1, 10, 2, 0) - 0.0).abs() < 1e-6);
        assert!((word_progress(4.0, 1, 10, 2, 0) - 0.2).abs() < 1e-6);
        assert!((word_progress(100.0, 0, 10, 2, 0) - 1.0).abs() < 1e-6);
        assert!((word_progress(1.0, 0, 10, 2, 5) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn channels_slide_along_the_direction_vector() {
        let start = word_channels(0.0, SlideDirection::Bottom, 40.0, false);
        assert!((start.opacity - 0.0).abs() < 1e-6);
        assert!(start.dx.abs() < 1e-6);
        assert!((start.dy - 40.0).abs() < 1e-6);

        let left = word_channels(0.0, SlideDirection::Left, 40.0, false);
        assert!((left.dx + 40.0).abs() < 1e-6);
        assert!(left.dy.abs() < 1e-6);

        let top = word_channels(0.0, SlideDirection::Top, 40.0, false);
        assert!(top.dx.abs() < 1e-6);
        assert!((top.dy + 40.0).abs() < 1e-6);

        let right = word_channels(0.0, SlideDirection::Right, 40.0, false);
        assert!((right.dx - 40.0).abs() < 1e-6);

        let end = word_channels(1.0, SlideDirection::Bottom, 40.0, false);
        assert!((end.opacity - 1.0).abs() < 1e-6);
        assert!(end.dx.abs() < 1e-4);
        assert!(end.dy.abs() < 1e-4);
    }

    #[test]
    fn reduced_motion_keeps_the_fade_and_drops_the_travel() {
        let mid = word_channels(0.5, SlideDirection::Bottom, 40.0, true);
        assert!(mid.dx.abs() < 1e-6);
        assert!(mid.dy.abs() < 1e-6);
        // EASE_OUT is a strong ease-out, so half way through the clock the word is mostly in.
        assert!(mid.opacity > 0.5);
        let normal = word_channels(0.5, SlideDirection::Bottom, 40.0, false);
        assert!((mid.opacity - normal.opacity).abs() < 1e-6);
    }

    #[test]
    fn distance_scales_the_travel_linearly() {
        let far = word_channels(0.3, SlideDirection::Right, 80.0, false);
        let near = word_channels(0.3, SlideDirection::Right, 40.0, false);
        assert!((far.dx - near.dx * 2.0).abs() < 1e-4);
    }

    #[test]
    fn duration_includes_the_hold_tail() {
        let component = SlideInText::default();
        // The default text is 5 words: 30 + 4*1 = 34 entry frames, plus the 45 frame tail.
        assert_eq!(component.config.text.word_count(), 5);
        assert_eq!(component.duration(), 79);
        assert_eq!(component.duration(), entry_frames(5, 30, 1) + DEFAULT_TAIL);
        assert_eq!(component.meta().id, "slide-in-text");
        assert_eq!(component.meta().feature, "slide-in-text");
    }

    #[test]
    fn zero_word_text_does_not_underflow() {
        let component = SlideInText::new(SlideInTextConfig {
            text: TextSpec {
                text: "   ".to_owned(),
                ..TextSpec::default()
            },
            ..SlideInTextConfig::default()
        });
        assert_eq!(component.duration(), DEFAULT_TAIL);
    }
}
