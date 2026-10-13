//! # Top Down Letters
//!
//! The mirror of [`bottom_up_letters`](crate::components::bottom_up_letters): every character of a
//! single line drops into place from above on the same wide 3-frame stagger, with the same
//! timings (`char_duration_frames` 12 for opacity, `char_travel_frames` 7 for travel) and the same
//! `EASE_LETTER_SOFT` curve. Only the sign of the travel differs, which is what flips the
//! staircase.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Top down letters."` | The line to reveal. |
//! | `stagger` | `usize` | `3` | Frames between consecutive character starts. |
//! | `distance` | `f32` | `46.0` | Reference-canvas px each character drops from. |
//! | `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `char_duration_frames` | `usize` | `12` | Frames each character takes to fade in. |
//! | `char_travel_frames` | `usize` | `7` | Frames each character takes to finish dropping. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `0` | Frames held after the last character lands. |
//! | `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
//! | `reduced_motion` | bool | `false` | Collapse travel; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components top-down-letters -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil((char_duration_frames + (chars - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (17 characters) that is 60 frames, 2.00 s at 30 fps.
//!
//! ## Credit
//!
//! Ported from `top-down-letters/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, char_centers, text_node_centered};
use crate::motion::{REFERENCE_VMIN, entry_frames, letter_channels};

/// Configuration for [`TopDownLetters`].
///
/// Structurally identical to
/// [`BottomUpLettersConfig`](crate::components::bottom_up_letters::BottomUpLettersConfig) — the two
/// source files are the same component with the travel sign flipped — but it is its own type with
/// its own default demo line, because [`Default`] is what the catalog builds components from.
#[derive(Debug, Clone, PartialEq)]
pub struct TopDownLettersConfig {
    /// The line to reveal.
    pub text: String,
    /// Frames between consecutive character starts.
    pub stagger: usize,
    /// Reference-canvas px each character drops from. Positive means above the resting place.
    pub distance: f32,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Text fill.
    pub color: String,
    /// Font weight.
    pub font_weight: u16,
    /// Frames each character takes to fade in.
    pub char_duration_frames: usize,
    /// Frames each character takes to finish its travel.
    pub char_travel_frames: usize,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the last character lands.
    pub tail: usize,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
    /// Collapse travel; keep the fades.
    pub reduced_motion: bool,
}

impl Default for TopDownLettersConfig {
    fn default() -> Self {
        Self {
            text: "Top down letters.".to_owned(),
            stagger: 3,
            distance: 46.0,
            font_size: 72.0,
            color: "#e6edf3".to_owned(),
            font_weight: 600,
            char_duration_frames: 12,
            char_travel_frames: 7,
            speed: 1.0,
            tail: 0,
            tracking_em: -0.05,
            reduced_motion: false,
        }
    }
}

/// The Top Down Letters component.
#[derive(Debug, Clone, Default)]
pub struct TopDownLetters {
    /// Component options.
    pub config: TopDownLettersConfig,
}

impl TopDownLetters {
    /// Build with the given options.
    pub fn new(config: TopDownLettersConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "top-down-letters",
            name: "Top Down Letters",
            summary: "Letters drop into place from above on a wide staircase stagger.",
            feature: "top-down-letters",
        }
    }
}

impl Component for TopDownLetters {
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
            // `-1` is the only difference from the bottom-up component.
            let (opacity, y) = letter_channels(
                frame,
                index,
                config.stagger,
                config.char_duration_frames,
                config.char_travel_frames,
                config.distance,
                config.reduced_motion,
                config.speed,
                unit,
                -1.0,
            );
            nodes.push(text_node_centered(
                &character.to_string(),
                center_x + dx,
                baseline + y,
                family.clone(),
                size,
                config.font_weight,
                config.tracking_em,
                config.color.clone(),
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
    fn drops_from_above() {
        let component = TopDownLetters::default();
        let config = &component.config;
        let (opacity, y) = letter_channels(
            0.0,
            0,
            config.stagger,
            config.char_duration_frames,
            config.char_travel_frames,
            config.distance,
            config.reduced_motion,
            config.speed,
            1.0,
            -1.0,
        );
        assert!(opacity.abs() < 1e-6);
        assert!(
            y < 0.0,
            "expected the character above its resting place, got {y}"
        );
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = TopDownLetters::default();
        assert_eq!(component.config.text.chars().count(), 17);
        assert_eq!(component.duration(), 60);
        assert_eq!(component.meta().id, "top-down-letters");
    }
}
