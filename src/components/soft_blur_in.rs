//! # Soft Blur In
//!
//! Characters fade in one at a time, each arriving soft and resolving as it lands. It is the
//! gentlest of the character reveals: a 1-frame stagger, a 27-frame opacity clock, a 9-frame
//! travel of only 16 px, and a 12 px starting softness under the material deceleration curve.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Softly, then sharp."` | The line to reveal. |
//! | `blur_from` | `f32` | `12.0` | Starting softness radius in reference-canvas px; eases to 0. |
//! | `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `stagger` | `usize` | `1` | Frames between consecutive character starts. |
//! | `char_duration_frames` | `usize` | `27` | Frames each character takes to fade in and sharpen. |
//! | `char_travel_frames` | `usize` | `9` | Frames each character takes to finish rising. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `0` | Frames held after the last character lands. |
//! | `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
//! | `reduced_motion` | `bool` | `false` | Collapse travel and softness; keep the fades. |
//! | `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for a real gaussian blur. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components soft-blur-in -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil((char_duration_frames + (chars - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (19 characters) that is 45 frames, 1.50 s at 30 fps.
//!
//! ## Fidelity
//!
//! `filter: blur(12px) → 0` is **approximated**, the same way
//! [`blur_slide_text`](crate::components::blur_slide_text) does it: each glyph is drawn once
//! sharply and once as a low-opacity copy spread along the travel axis, the copy's alpha and
//! spread scaling with the radius. It reads as softness and resolves cleanly, but it is not an
//! isotropic gaussian. Set `simulate_blur: false` to drop the stand-in entirely.
//!
//! ## Credit
//!
//! Ported from `soft-blur-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::blur::{
    BLUR_GHOST_COPIES, BLUR_GHOST_MIN_PX, BLUR_GHOST_SPREAD_RATIO, blur_ghost_alpha,
};
use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, char_centers, text_node_centered};
use crate::motion::{
    EASE_MATERIAL_OUT, REFERENCE_VMIN, clamp01, interpolate, motion_scale, normalised_speed,
};

/// Configuration for [`SoftBlurIn`].
#[derive(Debug, Clone, PartialEq)]
pub struct SoftBlurInConfig {
    /// The line to reveal.
    pub text: String,
    /// Starting softness radius in reference-canvas px; eases to 0.
    pub blur_from: f32,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Text fill.
    pub color: String,
    /// Font weight.
    pub font_weight: u16,
    /// Frames between consecutive character starts.
    pub stagger: usize,
    /// Frames each character takes to fade in and sharpen.
    pub char_duration_frames: usize,
    /// Frames each character takes to finish rising.
    pub char_travel_frames: usize,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the last character lands.
    pub tail: usize,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
    /// Collapse travel and softness; keep the fades.
    pub reduced_motion: bool,
    /// Draw the secondary copies that stand in for a real gaussian blur.
    pub simulate_blur: bool,
}

impl Default for SoftBlurInConfig {
    fn default() -> Self {
        Self {
            text: "Softly, then sharp.".to_owned(),
            blur_from: 12.0,
            font_size: 72.0,
            color: "#e6edf3".to_owned(),
            font_weight: 600,
            stagger: 1,
            char_duration_frames: 27,
            char_travel_frames: 9,
            speed: 1.0,
            tail: 0,
            tracking_em: -0.05,
            reduced_motion: false,
            simulate_blur: true,
        }
    }
}

/// Frames until the last character lands, before the tail.
pub fn entry_frames(char_count: usize, duration: usize, stagger: usize, speed: f32) -> usize {
    if char_count == 0 {
        return 0;
    }
    let span = duration + (char_count - 1) * stagger;
    ((span as f32) / normalised_speed(speed)).ceil().max(0.0) as usize
}

/// The three animated channels of one character at the component's frame clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoftBlurChannels {
    /// Opacity, eased by `EASE_MATERIAL_OUT`.
    pub opacity: f32,
    /// Vertical offset in canvas px; positive means below the resting place.
    pub y: f32,
    /// Softness radius in canvas px, 0 when the glyph is sharp.
    pub blur: f32,
}

/// Sample one character's channels.
///
/// All three channels share one clock and one curve, as the source does: the opacity is the eased
/// progress, the travel interpolates from its offset to 0 on that same eased progress, and the
/// softness is what is left of the radius.
pub fn character_channels(
    frame: f32,
    index: usize,
    config: &SoftBlurInConfig,
    unit: f32,
) -> SoftBlurChannels {
    let local = frame * normalised_speed(config.speed) - index as f32 * config.stagger as f32;
    let span = config.char_duration_frames.max(1) as f32;
    let eased = EASE_MATERIAL_OUT.ease(clamp01(local / span));
    let travel = motion_scale(config.reduced_motion);
    SoftBlurChannels {
        opacity: eased,
        y: interpolate(eased, [0.0, 1.0], [16.0 * unit * travel, 0.0]),
        blur: (1.0 - eased) * config.blur_from * unit * travel,
    }
}

/// The Soft Blur In component.
#[derive(Debug, Clone, Default)]
pub struct SoftBlurIn {
    /// Component options.
    pub config: SoftBlurInConfig,
}

impl SoftBlurIn {
    /// Build with the given options.
    pub fn new(config: SoftBlurInConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "soft-blur-in",
            name: "Soft Blur In",
            summary: "Characters fade up from soft focus one at a time.",
            feature: "soft-blur-in",
        }
    }
}

impl Component for SoftBlurIn {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        entry_frames(
            self.config.text.chars().count(),
            self.config.char_duration_frames,
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
        let family = ctx.fonts.sans.clone();
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let size = config.font_size * unit;

        let (centres, _) = char_centers(ctx, &family, &config.text, size);
        let center_x = ctx.width() / 2.0;
        let baseline = ctx.height() / 2.0 + ctx.baseline_offset(&family, size, size * 1.2);
        let frame = ctx.frame();

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, (character, dx)) in config.text.chars().zip(centres).enumerate() {
            let channels = character_channels(frame, index, config, unit);
            let glyph = character.to_string();
            let x = center_x + dx;
            let y = baseline + channels.y;

            if config.simulate_blur && channels.blur > BLUR_GHOST_MIN_PX {
                let spread = channels.blur * BLUR_GHOST_SPREAD_RATIO;
                let alpha = blur_ghost_alpha(channels.blur, unit);
                for copy in 1..=BLUR_GHOST_COPIES {
                    // Copies fan out along both sides of the travel axis.
                    let sign = if copy % 2 == 0 { -1.0 } else { 1.0 };
                    let distance = spread * (copy as f32 / BLUR_GHOST_COPIES as f32) * sign;
                    nodes.push(text_node_centered(
                        &glyph,
                        x,
                        y + distance,
                        family.clone(),
                        size,
                        config.font_weight,
                        config.tracking_em,
                        config.color.clone(),
                        channels.opacity * alpha,
                    ));
                }
            }

            nodes.push(text_node_centered(
                &glyph,
                x,
                y,
                family.clone(),
                size,
                config.font_weight,
                config.tracking_em,
                config.color.clone(),
                channels.opacity,
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::blur::{
        BLUR_GHOST_COPIES, BLUR_GHOST_MAX_ALPHA, BLUR_GHOST_MIN_PX, BLUR_GHOST_REFERENCE_PX,
        BLUR_GHOST_SPREAD_RATIO,
    };

    #[test]
    fn the_shared_ghost_tokens_are_the_character_level_set() {
        // These numbers are the product: a character-level reveal spends a 0.45 budget over two
        // copies at a 12 px reference. The word-level components keep their own 0.35 / 10 px / 0.6
        // set locally, and the two must not drift into one another.
        assert!((BLUR_GHOST_REFERENCE_PX - 12.0).abs() < f32::EPSILON);
        assert!((BLUR_GHOST_MAX_ALPHA - 0.45).abs() < f32::EPSILON);
        assert_eq!(BLUR_GHOST_COPIES, 2);
        assert!((BLUR_GHOST_SPREAD_RATIO - 0.5).abs() < f32::EPSILON);
        assert!((BLUR_GHOST_MIN_PX - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn entry_frames_staggers_every_character() {
        assert_eq!(entry_frames(0, 27, 1, 1.0), 0);
        assert_eq!(entry_frames(19, 27, 1, 1.0), 45);
    }

    #[test]
    fn channels_resolve_to_sharp_and_resting() {
        let config = SoftBlurInConfig::default();
        let start = character_channels(0.0, 0, &config, 1.0);
        assert!(start.opacity.abs() < 1e-6);
        assert!((start.blur - config.blur_from).abs() < 1e-6);
        assert!((start.y - 16.0).abs() < 1e-6);

        let end = character_channels(60.0, 0, &config, 1.0);
        assert!((end.opacity - 1.0).abs() < 1e-6);
        assert!(end.blur.abs() < 1e-6);
        assert!(end.y.abs() < 1e-6);
    }

    #[test]
    fn softness_and_travel_are_scaled_by_the_canvas() {
        let config = SoftBlurInConfig::default();
        let channels = character_channels(0.0, 0, &config, 1.5);
        assert!((channels.blur - config.blur_from * 1.5).abs() < 1e-5);
        assert!((channels.y - 24.0).abs() < 1e-5);
    }

    #[test]
    fn reduced_motion_leaves_only_the_fade() {
        let config = SoftBlurInConfig {
            reduced_motion: true,
            ..SoftBlurInConfig::default()
        };
        let channels = character_channels(6.0, 0, &config, 1.5);
        assert!(channels.blur.abs() < 1e-6);
        assert!(channels.y.abs() < 1e-6);
        assert!(channels.opacity > 0.0 && channels.opacity <= 1.0);
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = SoftBlurIn::default();
        assert_eq!(component.config.text.chars().count(), 19);
        assert_eq!(component.duration(), 45);
        assert_eq!(component.meta().id, "soft-blur-in");
    }
}
