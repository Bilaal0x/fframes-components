//! # Short Slide Right
//!
//! A compact entrance: the whole line slides in from the left over 8 frames while a slight
//! softness (1.2 px) clears over 16, and the words fade in one after another on a 3-frame stagger
//! under `EASE_CHAR`. The travel and the blur belong to the *line*, the opacity to each word —
//! which is what makes the words appear to catch up with the line rather than ride it.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text.text` | `String` | `"Slide in from the left"` | The line to reveal. |
//! | `text.font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `text.font_weight` | `u16` | `600` | Font weight. |
//! | `text.color` | `String` | `"#e6edf3"` | Text fill. |
//! | `text.max_width` | `f32` | `960.0` | Wrap width in reference-canvas px. |
//! | `text.line_height` | `f32` | `1.2` | Line height as a multiple of `font_size`. |
//! | `text.tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
//! | `distance` | `f32` | `24.0` | Reference-canvas px the line travels in. |
//! | `stagger` | `usize` | `3` | Frames between consecutive word starts. |
//! | `enter_frames` | `usize` | `16` | Frames the line's softness takes to clear. |
//! | `travel_frames` | `usize` | `8` | Frames the line takes to reach its place. |
//! | `word_frames` | `usize` | `6` | Frames each word takes to fade in. |
//! | `blur_from` | `f32` | `1.2` | Starting softness radius in reference-canvas px. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `45` | Frames held after the last word lands. |
//! | `reduced_motion` | `bool` | `false` | Collapse travel and softness; keep the fades. |
//! | `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for a real gaussian blur. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components short-slide-right -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil(max(enter_frames, travel_frames, word_frames + (words - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (5 words) that is 63 frames, 2.10 s at 30 fps.
//!
//! ## Fidelity
//!
//! `filter: blur(1.2px)` is **approximated** with offset low-opacity copies; every design px is
//! multiplied by `ctx.vmin() / 7.2`, because the source is authored on 1280x720 and this crate
//! renders 1080p. A 1.2 px radius is small enough that the stand-in barely reads — set
//! `simulate_blur: false` for a purely positional slide.
//!
//! ## Credit
//!
//! Ported from `short-slide-right/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::blur::{BLUR_GHOST_COPIES, BLUR_GHOST_SPREAD_RATIO, blur_ghost_alpha};
use crate::core::{Component, ComponentMeta};
use crate::ctx::{LayoutCache, RenderCtx, text_node_centered};
use crate::motion::{
    EASE_CHAR, REFERENCE_VMIN, clamp01, interpolate, motion_scale, normalised_speed,
};
use crate::text_spec::TextSpec;

/// Configuration for [`ShortSlideRight`].
#[derive(Debug, Clone, PartialEq)]
pub struct ShortSlideRightConfig {
    /// Text styling and layout.
    pub text: TextSpec,
    /// Reference-canvas px the line travels in.
    pub distance: f32,
    /// Frames between consecutive word starts.
    pub stagger: usize,
    /// Frames the line's softness takes to clear.
    pub enter_frames: usize,
    /// Frames the line takes to reach its place.
    pub travel_frames: usize,
    /// Frames each word takes to fade in.
    pub word_frames: usize,
    /// Starting softness radius in reference-canvas px.
    pub blur_from: f32,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the last word lands.
    pub tail: usize,
    /// Collapse travel and softness; keep the fades.
    pub reduced_motion: bool,
    /// Draw the secondary copies that stand in for a real gaussian blur.
    pub simulate_blur: bool,
}

impl Default for ShortSlideRightConfig {
    fn default() -> Self {
        Self {
            text: TextSpec {
                text: "Slide in from the left".to_owned(),
                font_size: 72.0,
                font_weight: 600,
                max_width: 960.0,
                line_height: 1.2,
                tracking_em: -0.03,
                ..TextSpec::default()
            },
            distance: 24.0,
            stagger: 3,
            enter_frames: 16,
            travel_frames: 8,
            word_frames: 6,
            blur_from: 1.2,
            speed: 1.0,
            tail: 45,
            reduced_motion: false,
            simulate_blur: true,
        }
    }
}

/// Frames until the last word lands, before the tail.
pub fn entry_frames(config: &ShortSlideRightConfig, word_count: usize) -> usize {
    let words = if word_count == 0 {
        0
    } else {
        config.word_frames + (word_count - 1) * config.stagger
    };
    config.enter_frames.max(config.travel_frames).max(words)
}

/// The line's horizontal offset and softness at the component's frame clock.
pub fn line_channels(frame: f32, config: &ShortSlideRightConfig, unit: f32) -> (f32, f32) {
    let local = frame * normalised_speed(config.speed);
    let travel = motion_scale(config.reduced_motion);
    let travel_p = EASE_CHAR.ease(clamp01(local / config.travel_frames.max(1) as f32));
    let enter_p = EASE_CHAR.ease(clamp01(local / config.enter_frames.max(1) as f32));
    (
        interpolate(travel_p, [0.0, 1.0], [-config.distance * unit, 0.0]) * travel,
        (1.0 - enter_p) * config.blur_from * unit * travel,
    )
}

/// Opacity of one word at the component's frame clock.
pub fn word_opacity(frame: f32, index: usize, config: &ShortSlideRightConfig) -> f32 {
    let local = frame * normalised_speed(config.speed) - index as f32 * config.stagger as f32;
    EASE_CHAR.ease(clamp01(local / config.word_frames.max(1) as f32))
}

/// The Short Slide Right component.
#[derive(Debug, Clone, Default)]
pub struct ShortSlideRight {
    /// Component options.
    pub config: ShortSlideRightConfig,
    layout: LayoutCache,
}

impl ShortSlideRight {
    /// Build with the given options.
    pub fn new(config: ShortSlideRightConfig) -> Self {
        Self {
            config,
            layout: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "short-slide-right",
            name: "Short Slide Right",
            summary: "A line slides in from the left while its words catch up.",
            feature: "short-slide-right",
        }
    }
}

impl Component for ShortSlideRight {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        let frames = entry_frames(&self.config, self.config.text.word_count()) as f32;
        (frames / normalised_speed(self.config.speed))
            .ceil()
            .max(0.0) as usize
            + self.config.tail
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

        let (x, blur) = line_channels(ctx.frame(), config, unit);
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;

        // The blur belongs to the line, so every word carries the same softness stand-in for the
        // frames where the line is still clearing.
        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        let spread = blur * BLUR_GHOST_SPREAD_RATIO;
        let alpha = blur_ghost_alpha(blur, unit);
        let blurred = config.simulate_blur && blur > 0.05;

        for (index, word, baseline) in layout.all_words(center_y) {
            let opacity = word_opacity(ctx.frame(), index, config);
            let word_x = center_x + word.x + x;
            if blurred {
                for copy in 1..=BLUR_GHOST_COPIES {
                    let sign = if copy % 2 == 0 { -1.0 } else { 1.0 };
                    nodes.push(text_node_centered(
                        &word.text,
                        word_x + spread * (copy as f32 / BLUR_GHOST_COPIES as f32) * sign,
                        baseline,
                        family.clone(),
                        size,
                        spec.font_weight,
                        spec.tracking_em,
                        spec.color.clone(),
                        opacity * alpha,
                    ));
                }
            }
            nodes.push(text_node_centered(
                &word.text,
                word_x,
                baseline,
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
    fn the_longest_channel_sets_the_entry() {
        let config = ShortSlideRightConfig::default();
        // 5 words: the word stagger (6 + 4*3 = 18) outlasts enter (16) and travel (8).
        assert_eq!(entry_frames(&config, 5), 18);
        // A single word is shorter than the line's own channels.
        assert_eq!(entry_frames(&config, 1), 16);
        assert_eq!(entry_frames(&config, 0), 16);
    }

    #[test]
    fn the_line_starts_left_and_soft() {
        let config = ShortSlideRightConfig::default();
        let (x, blur) = line_channels(0.0, &config, 1.0);
        assert!((x + config.distance).abs() < 1e-6, "got {x}");
        assert!((blur - config.blur_from).abs() < 1e-6);

        let (x, blur) = line_channels(30.0, &config, 1.0);
        assert!(x.abs() < 1e-6, "the line should have arrived, got {x}");
        assert!(blur.abs() < 1e-6);
    }

    #[test]
    fn words_fade_in_behind_the_line() {
        let config = ShortSlideRightConfig::default();
        assert!(word_opacity(0.0, 0, &config).abs() < 1e-6);
        assert!(word_opacity(0.0, 1, &config).abs() < 1e-6);
        let first = word_opacity(3.0, 0, &config);
        let second = word_opacity(3.0, 1, &config);
        assert!(
            first > second,
            "word 0 must lead word 1: {first} vs {second}"
        );
        assert!((word_opacity(60.0, 4, &config) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_keeps_the_word_fades() {
        let config = ShortSlideRightConfig {
            reduced_motion: true,
            ..ShortSlideRightConfig::default()
        };
        let (x, blur) = line_channels(4.0, &config, 1.5);
        assert!(x.abs() < 1e-6);
        assert!(blur.abs() < 1e-6);
        assert!(word_opacity(4.0, 0, &config) > 0.0);
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = ShortSlideRight::default();
        assert_eq!(component.config.text.word_count(), 5);
        assert_eq!(component.duration(), 63);
        assert_eq!(component.meta().id, "short-slide-right");
    }
}
