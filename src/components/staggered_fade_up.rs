//! # Staggered Fade Up
//!
//! Words fade in and rise one after another on a plain linear clock — no easing curve at all,
//! which is what separates it from the eased word reveals in this library. Each word is on screen
//! for 12 frames, `stagger_delay` (4) frames after the one before it.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text.text` | `String` | `"Staggered fade up, one word at a time"` | The line to reveal. |
//! | `text.font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `text.font_weight` | `u16` | `600` | Font weight. |
//! | `text.color` | `String` | `"#e6edf3"` | Text fill. |
//! | `text.max_width` | `f32` | `960.0` | Wrap width in reference-canvas px. |
//! | `text.line_height` | `f32` | `1.2` | Line height as a multiple of `font_size`. |
//! | `text.tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
//! | `stagger` | `usize` | `4` | Frames between consecutive word starts. |
//! | `distance` | `f32` | `20.0` | Reference-canvas px each word rises. |
//! | `word_frames` | `usize` | `12` | Frames each word takes to land. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `40` | Frames held after the last word lands. |
//! | `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components staggered-fade-up -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil((word_frames + (words - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (8 words) that is 80 frames, 2.67 s at 30 fps.
//!
//! ## Credit
//!
//! Ported from `staggered-fade-up/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{LayoutCache, RenderCtx, text_node_centered};
use crate::motion::{REFERENCE_VMIN, clamp01, motion_scale, normalised_speed};
use crate::text_spec::TextSpec;

/// Configuration for [`StaggeredFadeUp`].
#[derive(Debug, Clone, PartialEq)]
pub struct StaggeredFadeUpConfig {
    /// Text styling and layout.
    pub text: TextSpec,
    /// Frames between consecutive word starts.
    pub stagger: usize,
    /// Reference-canvas px each word rises.
    pub distance: f32,
    /// Frames each word takes to land.
    pub word_frames: usize,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the last word lands.
    pub tail: usize,
    /// Collapse travel; keep the fades.
    pub reduced_motion: bool,
}

impl Default for StaggeredFadeUpConfig {
    fn default() -> Self {
        Self {
            text: TextSpec {
                text: "Staggered fade up, one word at a time".to_owned(),
                font_size: 72.0,
                font_weight: 600,
                max_width: 960.0,
                line_height: 1.2,
                tracking_em: -0.03,
                ..TextSpec::default()
            },
            stagger: 4,
            distance: 20.0,
            word_frames: 12,
            speed: 1.0,
            tail: 40,
            reduced_motion: false,
        }
    }
}

/// Frames until the last word lands, before the tail.
pub fn entry_frames(word_count: usize, word_frames: usize, stagger: usize, speed: f32) -> usize {
    if word_count == 0 {
        return 0;
    }
    let span = word_frames + (word_count - 1) * stagger;
    ((span as f32) / normalised_speed(speed)).ceil().max(0.0) as usize
}

/// Opacity and vertical offset of one word at the component's frame clock.
///
/// Both channels interpolate linearly — the source passes no `easing` option here.
pub fn word_channels(
    frame: f32,
    index: usize,
    config: &StaggeredFadeUpConfig,
    unit: f32,
) -> (f32, f32) {
    let local = frame * normalised_speed(config.speed) - index as f32 * config.stagger as f32;
    let span = config.word_frames.max(1) as f32;
    let opacity = clamp01(local / span);
    let y = (1.0 - opacity) * config.distance * unit * motion_scale(config.reduced_motion);
    (opacity, y)
}

/// The Staggered Fade Up component.
#[derive(Debug, Clone, Default)]
pub struct StaggeredFadeUp {
    /// Component options.
    pub config: StaggeredFadeUpConfig,
    layout: LayoutCache,
}

impl StaggeredFadeUp {
    /// Build with the given options.
    pub fn new(config: StaggeredFadeUpConfig) -> Self {
        Self {
            config,
            layout: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "staggered-fade-up",
            name: "Staggered Fade Up",
            summary: "Words rise and fade in on an even, un-eased stagger.",
            feature: "staggered-fade-up",
        }
    }
}

impl Component for StaggeredFadeUp {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        entry_frames(
            self.config.text.word_count(),
            self.config.word_frames,
            self.config.stagger,
            self.config.speed,
        ) + self.config.tail
    }

    /// The library's dark house backdrop, in place of the source's `#ffffff` demo canvas.
    fn background(&self) -> Option<&'static str> {
        Some("#0d1117")
    }
    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let spec = &config.text;
        let family = ctx.fonts.sans.clone();
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let size = spec.font_size * unit;
        let layout = self.layout.resolve(
            ctx,
            &family,
            &spec.text,
            size,
            spec.max_width * unit,
            spec.line_height,
        );

        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let frame = ctx.frame();

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, word, baseline) in layout.all_words(center_y) {
            let (opacity, y) = word_channels(frame, index, config, unit);
            nodes.push(text_node_centered(
                &word.text,
                center_x + word.x,
                baseline + y,
                family.clone(),
                size,
                spec.font_weight,
                spec.tracking_em,
                spec.color.clone(),
                opacity,
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
    fn entry_frames_staggers_every_word() {
        assert_eq!(entry_frames(0, 12, 4, 1.0), 0);
        assert_eq!(entry_frames(1, 12, 4, 1.0), 12);
        assert_eq!(entry_frames(8, 12, 4, 1.0), 40);
    }

    #[test]
    fn channels_are_linear() {
        let config = StaggeredFadeUpConfig::default();
        let (opacity, y) = word_channels(6.0, 0, &config, 1.0);
        assert!(
            (opacity - 0.5).abs() < 1e-6,
            "expected a linear half, got {opacity}"
        );
        assert!((y - config.distance * 0.5).abs() < 1e-6);

        let (opacity, y) = word_channels(0.0, 0, &config, 1.0);
        assert!(opacity.abs() < 1e-6);
        assert!((y - config.distance).abs() < 1e-6);
    }

    #[test]
    fn stagger_holds_later_words_back() {
        let config = StaggeredFadeUpConfig::default();
        let (opacity, y) = word_channels(2.0, 1, &config, 1.0);
        assert!(opacity.abs() < 1e-6);
        assert!((y - config.distance).abs() < 1e-6);
    }

    #[test]
    fn words_carry_different_phases_on_a_single_line() {
        // A regression guard: the word index has to be the word's position in the paragraph, not
        // its position on its own line, or a layout that wraps every word onto its own line would
        // render the whole line at once.
        let config = StaggeredFadeUpConfig::default();
        // Each word starts 4 frames after the one before and takes 12 frames, so frame 22 is the
        // window where words 0 to 2 are all in flight at different phases.
        let frame = 14.0;
        let phases: Vec<f32> = (0..3)
            .map(|index| word_channels(frame, index, &config, 1.0).0)
            .collect();
        assert!(
            phases[0] > phases[1] && phases[1] > phases[2],
            "later words must lag: {phases:?}"
        );
        assert!(
            (phases[0] - 1.0).abs() < 1e-6,
            "the first word should have landed"
        );
        assert!(phases[2] > 0.0, "the third word should have started");
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = StaggeredFadeUp::default();
        assert_eq!(component.config.text.word_count(), 8);
        assert_eq!(component.duration(), 80);
        assert_eq!(component.meta().id, "staggered-fade-up");
    }
}
