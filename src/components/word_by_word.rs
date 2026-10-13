//! # Word by Word
//!
//! Words appear one after another with staggered timing, each rising a short distance while
//! fading in. One shared ease-out curve drives both channels per word, so each word moves as a
//! unit, and it is the same curve every other reveal in this library uses.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text.text` | `String` | `"Words appear one after another in sequence"` | The line to reveal. |
//! | `text.font_size` | `f32` | `64.0` | Font size in px. |
//! | `text.font_weight` | `u16` | `700` | Font weight. |
//! | `text.color` | `String` | `"#e6edf3"` | Text fill. |
//! | `text.max_width` | `f32` | `900.0` | Wrap width in px. |
//! | `text.line_height` | `f32` | `1.25` | Line height as a multiple of `font_size`. |
//! | `text.tracking_em` | `f32` | `-0.02` | Letter spacing in `em`. |
//! | `duration` | `usize` | `30` | Frames each word takes to enter. |
//! | `stagger` | `usize` | `3` | Frames between consecutive word starts. |
//! | `delay` | `usize` | `0` | Frames to wait before the first word starts. |
//! | `distance` | `f32` | `20.0` | Pixels each word rises while fading in. |
//! | `tail` | `usize` | `72` | Frames held after the last word lands. |
//! | `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components word-by-word
//! ```
//!
//! ## Credit
//!
//! Ported from `WordByWord.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{LayoutCache, RenderCtx, text_node_centered};
use crate::motion::{EASE_OUT, clamp01, motion_scale};
use crate::text_spec::TextSpec;

/// Configuration for [`WordByWord`].
#[derive(Debug, Clone, PartialEq)]
pub struct WordByWordConfig {
    /// Text styling and layout.
    pub text: TextSpec,
    /// Frames each word takes to enter.
    pub duration: usize,
    /// Frames between consecutive word starts.
    pub stagger: usize,
    /// Frames to wait before the first word starts.
    pub delay: usize,
    /// Pixels each word rises while fading in.
    pub distance: f32,
    /// Frames held after the last word lands.
    pub tail: usize,
    /// Collapse travel; keep the fades.
    pub reduced_motion: bool,
}

impl Default for WordByWordConfig {
    fn default() -> Self {
        Self {
            text: TextSpec::default(),
            duration: 30,
            stagger: 3,
            delay: 0,
            distance: 20.0,
            tail: 72,
            reduced_motion: false,
        }
    }
}

/// Frames until the last word lands (entry only, no hold tail).
pub fn entry_frames(word_count: usize, duration: usize, stagger: usize) -> usize {
    if word_count == 0 {
        return 0;
    }
    duration + (word_count - 1) * stagger
}

/// Per-word progress: stagger shifts each word's clock back, so word `i` starts `i * stagger`
/// frames later than word 0.
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

/// The Word by Word component.
#[derive(Debug, Clone, Default)]
pub struct WordByWord {
    /// Component options.
    pub config: WordByWordConfig,
    layout: LayoutCache,
}

impl WordByWord {
    /// Build with the given options.
    pub fn new(config: WordByWordConfig) -> Self {
        Self {
            config,
            layout: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "word-by-word",
            name: "Word by Word",
            summary: "Words rise and fade in one after another, staggered.",
            feature: "word-by-word",
        }
    }
}

impl Component for WordByWord {
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

        let travel = self.config.distance * motion_scale(self.config.reduced_motion);
        let frame = ctx.frame();
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, word, baseline) in layout.all_words(center_y) {
            let progress = EASE_OUT.ease(word_progress(
                frame,
                index,
                self.config.duration,
                self.config.stagger,
                self.config.delay,
            ));
            let y = baseline + (1.0 - progress) * travel;
            nodes.push(text_node_centered(
                &word.text,
                center_x + word.x,
                y,
                family.clone(),
                spec.font_size,
                spec.font_weight,
                spec.tracking_em,
                spec.color.clone(),
                progress,
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
    fn entry_frames_counts_words() {
        assert_eq!(entry_frames(3, 30, 3), 36);
        assert_eq!(entry_frames(0, 30, 3), 0);
        assert_eq!(entry_frames(1, 10, 5), 10);
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
    fn duration_includes_the_hold_tail() {
        let component = WordByWord::default();
        // The default text is 7 words: 30 + 6*3 = 48 entry frames, plus the 72 frame tail.
        assert_eq!(component.duration(), entry_frames(7, 30, 3) + 72);
        assert_eq!(component.duration(), 120);
        assert_eq!(component.meta().id, "word-by-word");
    }

    #[test]
    fn zero_word_text_does_not_underflow() {
        let component = WordByWord::new(WordByWordConfig {
            text: TextSpec {
                text: "   ".to_owned(),
                ..TextSpec::default()
            },
            ..WordByWordConfig::default()
        });
        assert_eq!(component.duration(), 72);
    }
}
