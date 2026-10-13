//! # Bottom Up Letters
//!
//! Every character of a single line slides up into place from below on a wide character stagger,
//! which is what gives the source its staircase read: `stagger_delay` (3) is deliberately larger
//! than the one-frame stagger of `per-character-rise`, and the travel is longer (46 against 32).
//!
//! Opacity runs over `char_duration_frames` (12) and travel over `char_travel_frames` (7), both
//! under `EASE_LETTER_SOFT` and both clamped before the character's turn comes.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Bottom up letters."` | The line to reveal. |
//! | `stagger` | `usize` | `3` | Frames between consecutive character starts. |
//! | `distance` | `f32` | `46.0` | Reference-canvas px each character rises from. |
//! | `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `char_duration_frames` | `usize` | `12` | Frames each character takes to fade in. |
//! | `char_travel_frames` | `usize` | `7` | Frames each character takes to finish rising. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `0` | Frames held after the last character lands. |
//! | `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
//! | `reduced_motion` | bool | `false` | Collapse travel; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components bottom-up-letters -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil((char_duration_frames + (chars - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (18 characters) that is 63 frames, 2.10 s at 30 fps.
//!
//! ## Credit
//!
//! Ported from `bottom-up-letters/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, char_centers, text_node_centered};
use crate::motion::{REFERENCE_VMIN, entry_frames, letter_channels};

/// Configuration for [`BottomUpLetters`].
#[derive(Debug, Clone, PartialEq)]
pub struct BottomUpLettersConfig {
    /// The line to reveal.
    pub text: String,
    /// Frames between consecutive character starts.
    pub stagger: usize,
    /// Reference-canvas px each character rises from. Positive rises from below.
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

impl Default for BottomUpLettersConfig {
    fn default() -> Self {
        Self {
            text: "Bottom up letters.".to_owned(),
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

/// [`letter_channels`] for a [`BottomUpLettersConfig`].
pub fn config_channels(
    frame: f32,
    index: usize,
    config: &BottomUpLettersConfig,
    unit: f32,
    direction: f32,
) -> (f32, f32) {
    letter_channels(
        frame,
        index,
        config.stagger,
        config.char_duration_frames,
        config.char_travel_frames,
        config.distance,
        config.reduced_motion,
        config.speed,
        unit,
        direction,
    )
}

/// The Bottom Up Letters component.
#[derive(Debug, Clone, Default)]
pub struct BottomUpLetters {
    /// Component options.
    pub config: BottomUpLettersConfig,
}

impl BottomUpLetters {
    /// Build with the given options.
    pub fn new(config: BottomUpLettersConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "bottom-up-letters",
            name: "Bottom Up Letters",
            summary: "Letters slide up into place on a wide staircase stagger.",
            feature: "bottom-up-letters",
        }
    }
}

impl Component for BottomUpLetters {
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
            let (opacity, y) = config_channels(frame, index, config, unit, 1.0);
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
    fn letters_wait_above_or_below_until_their_turn() {
        let config = BottomUpLettersConfig::default();
        let (opacity, y) = config_channels(0.0, 0, &config, 1.0, 1.0);
        assert!(opacity.abs() < 1e-6);
        assert!((y - config.distance).abs() < 1e-6);

        let (opacity, y) = config_channels(0.0, 0, &config, 1.0, -1.0);
        assert!((y + config.distance).abs() < 1e-6);
        assert!(opacity.abs() < 1e-6);

        let (opacity, y) = config_channels(200.0, 0, &config, 1.0, 1.0);
        assert!((opacity - 1.0).abs() < 1e-6);
        assert!(y.abs() < 1e-6);
    }

    #[test]
    fn stagger_holds_later_letters_back() {
        let config = BottomUpLettersConfig::default();
        let (opacity, _) = config_channels(2.0, 1, &config, 1.0, 1.0);
        assert!(opacity.abs() < 1e-6);
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = BottomUpLetters::default();
        assert_eq!(component.config.text.chars().count(), 18);
        assert_eq!(component.duration(), 63);
    }

    #[test]
    fn reduced_motion_drops_the_travel() {
        let config = BottomUpLettersConfig {
            reduced_motion: true,
            ..BottomUpLettersConfig::default()
        };
        let (_, y) = config_channels(5.0, 0, &config, 1.5, 1.0);
        assert!(y.abs() < 1e-6);
    }
}
