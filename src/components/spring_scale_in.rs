//! # Spring Scale In
//!
//! Words pop into place one after another: opacity and scale share the `EASE_BACK_OUT` curve
//! (`cubic-bezier(0.34, 1.56, 0.64, 1)`), which overshoots its target before settling, so each
//! word grows slightly past full size and comes back. `stagger_delay` (3) frames separate the
//! words and each takes 11 frames, which is what makes the line read as a quick ripple rather
//! than a single pop.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text.text` | `String` | `"Fast. Crisp. Fluid."` | The line to reveal. |
//! | `text.font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `text.font_weight` | `u16` | `600` | Font weight. |
//! | `text.color` | `String` | `"#e6edf3"` | Text fill. |
//! | `text.max_width` | `f32` | `960.0` | Wrap width in reference-canvas px. |
//! | `text.line_height` | `f32` | `1.2` | Line height as a multiple of `font_size`. |
//! | `text.tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
//! | `stagger` | `usize` | `3` | Frames between consecutive word starts. |
//! | `scale_from` | `f32` | `0.7` | Scale each word starts at. |
//! | `word_frames` | `usize` | `11` | Frames each word takes to land. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `40` | Frames held after the last word lands. |
//! | `reduced_motion` | `bool` | `false` | Collapse the scale; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components spring-scale-in -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil((word_frames + (words - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (3 words) that is 57 frames, 1.90 s at 30 fps.
//!
//! ## Credit
//!
//! Ported from `spring-scale-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{LayoutCache, RenderCtx, text_node_centered};
use crate::motion::{
    EASE_BACK_OUT, REFERENCE_VMIN, clamp01, interpolate, map_range, motion_scale, normalised_speed,
};
use crate::text_spec::TextSpec;

/// Configuration for [`SpringScaleIn`].
#[derive(Debug, Clone, PartialEq)]
pub struct SpringScaleInConfig {
    /// Text styling and layout.
    pub text: TextSpec,
    /// Frames between consecutive word starts.
    pub stagger: usize,
    /// Scale each word starts at.
    pub scale_from: f32,
    /// Frames each word takes to land.
    pub word_frames: usize,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the last word lands.
    pub tail: usize,
    /// Collapse the scale; keep the fades.
    pub reduced_motion: bool,
}

impl Default for SpringScaleInConfig {
    fn default() -> Self {
        Self {
            text: TextSpec {
                text: "Fast. Crisp. Fluid.".to_owned(),
                font_size: 72.0,
                font_weight: 600,
                max_width: 960.0,
                line_height: 1.2,
                tracking_em: -0.03,
                ..TextSpec::default()
            },
            stagger: 3,
            scale_from: 0.7,
            word_frames: 11,
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

/// Opacity and scale of one word at the component's frame clock.
///
/// Both channels use `EASE_BACK_OUT`, whose control point above 1 makes the value exceed its
/// target before settling — that overshoot is the whole gesture.
///
/// `scale` is the final font size as a factor of the configured one, so it is `1.0` at rest and
/// the canvas unit scale is folded in at draw time.
pub fn word_channels(
    frame: f32,
    index: usize,
    config: &SpringScaleInConfig,
    unit: f32,
) -> (f32, f32) {
    let local = frame * normalised_speed(config.speed) - index as f32 * config.stagger as f32;
    let span = config.word_frames.max(1) as f32;
    let eased = EASE_BACK_OUT.ease(clamp01(local / span));
    let opacity = eased;
    // Reduced motion collapses the scale channel to its resting value; the fade stays.
    let start = interpolate(
        motion_scale(config.reduced_motion),
        [0.0, 1.0],
        [1.0, config.scale_from],
    );
    let scale = map_range(eased, [0.0, 1.0], [start, 1.0]);
    // Return the canvas-scaled factor so the caller only has one multiplication to do.
    (opacity, scale * unit)
}

/// The Spring Scale In component.
#[derive(Debug, Clone, Default)]
pub struct SpringScaleIn {
    /// Component options.
    pub config: SpringScaleInConfig,
    layout: LayoutCache,
}

impl SpringScaleIn {
    /// Build with the given options.
    pub fn new(config: SpringScaleInConfig) -> Self {
        Self {
            config,
            layout: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "spring-scale-in",
            name: "Spring Scale In",
            summary: "Words pop in one by one with a slight overshoot.",
            feature: "spring-scale-in",
        }
    }
}

impl Component for SpringScaleIn {
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
        // Laid out at the resting size, then drawn at `size * scale`, which is what CSS's
        // `transform-origin: 50% 50%` does to an inline-block word.
        let layout = self.layout.resolve(
            ctx,
            &family,
            &spec.text,
            spec.font_size * unit,
            spec.max_width * unit,
            spec.line_height,
        );

        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let frame = ctx.frame();

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, word, baseline) in layout.all_words(center_y) {
            let (opacity, scale) = word_channels(frame, index, config, unit);
            nodes.push(text_node_centered(
                &word.text,
                center_x + word.x,
                baseline,
                family.clone(),
                spec.font_size * scale,
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
        assert_eq!(entry_frames(0, 11, 3, 1.0), 0);
        assert_eq!(entry_frames(1, 11, 3, 1.0), 11);
        assert_eq!(entry_frames(3, 11, 3, 1.0), 17);
    }

    #[test]
    fn words_start_small_and_overshoot_before_resting() {
        let config = SpringScaleInConfig::default();
        let (opacity, scale) = word_channels(0.0, 0, &config, 1.0);
        assert!(opacity.abs() < 1e-6);
        assert!((scale - config.scale_from).abs() < 1e-6);

        // Somewhere mid-flight the back-out curve passes 1, so the word is larger than rest.
        let overshoot = (1..=11)
            .map(|f| word_channels(f as f32, 0, &config, 1.0).1)
            .fold(0.0_f32, f32::max);
        assert!(
            overshoot > 1.0,
            "expected an overshoot, peak was {overshoot}"
        );

        let (opacity, scale) = word_channels(60.0, 0, &config, 1.0);
        assert!((opacity - 1.0).abs() < 1e-6);
        assert!((scale - 1.0).abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_keeps_the_words_at_rest_size() {
        let config = SpringScaleInConfig {
            reduced_motion: true,
            ..SpringScaleInConfig::default()
        };
        let (_, scale) = word_channels(5.0, 0, &config, 1.5);
        assert!((scale - 1.5).abs() < 1e-6, "got {scale}");
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = SpringScaleIn::default();
        assert_eq!(component.config.text.word_count(), 3);
        assert_eq!(component.duration(), 57);
        assert_eq!(component.meta().id, "spring-scale-in");
    }
}
