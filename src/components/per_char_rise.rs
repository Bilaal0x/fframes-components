//! # Per Character Rise
//!
//! Every character of a single line rises into place from below, crisp and without blur, on a
//! one-frame stagger. Two clocks drive each glyph: opacity runs over
//! `char_duration_frames` (21) under `EASE_CHAR`, the vertical travel runs over
//! `char_travel_frames` (10) under `EASE_CHAR_TRAVEL`, and both are clamped at 0 before the
//! glyph's turn comes.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"One more thing."` | The line to reveal. |
//! | `distance` | `f32` | `32.0` | Reference-canvas px each character rises from. |
//! | `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `stagger` | `usize` | `1` | Frames between consecutive character starts. |
//! | `char_duration_frames` | `usize` | `21` | Frames each character takes to fade in. |
//! | `char_travel_frames` | `usize` | `10` | Frames each character takes to finish rising. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `0` | Frames held after the last character lands. |
//! | `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
//! | `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components per-character-rise -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil((char_duration_frames + (chars - 1) * stagger) / speed) + tail
//! ```
//!
//! With the defaults (15 characters) that is 35 frames, 1.17 s at 30 fps.
//!
//! ## Credit
//!
//! Ported from `per-character-rise/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, char_centers, text_node_centered};
use crate::motion::{
    EASE_CHAR, EASE_CHAR_TRAVEL, REFERENCE_VMIN, clamp01, motion_scale, normalised_speed,
};

/// Configuration for [`PerCharacterRise`].
#[derive(Debug, Clone, PartialEq)]
pub struct PerCharacterRiseConfig {
    /// The line to reveal.
    pub text: String,
    /// Reference-canvas px each character rises from.
    pub distance: f32,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Text fill.
    pub color: String,
    /// Font weight.
    pub font_weight: u16,
    /// Frames between consecutive character starts.
    pub stagger: usize,
    /// Frames each character takes to fade in.
    pub char_duration_frames: usize,
    /// Frames each character takes to finish rising.
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

impl Default for PerCharacterRiseConfig {
    fn default() -> Self {
        Self {
            text: "One more thing.".to_owned(),
            distance: 32.0,
            font_size: 72.0,
            color: "#e6edf3".to_owned(),
            font_weight: 600,
            stagger: 1,
            char_duration_frames: 21,
            char_travel_frames: 10,
            speed: 1.0,
            tail: 0,
            tracking_em: -0.05,
            reduced_motion: false,
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

/// The two animated channels of one character at the component's frame clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiseChannels {
    /// Opacity, eased by `EASE_CHAR`.
    pub opacity: f32,
    /// Vertical offset in canvas px; positive means the character is below its resting place.
    pub y: f32,
}

/// Sample one character's channels.
///
/// `local = frame * speed - index * stagger`, then each channel is an `interpolate` of that
/// local clock, clamped at both ends — verbatim from the source.
pub fn character_channels(
    frame: f32,
    index: usize,
    config: &PerCharacterRiseConfig,
    unit: f32,
) -> RiseChannels {
    let local = frame * normalised_speed(config.speed) - index as f32 * config.stagger as f32;
    let travel = config.distance * unit * motion_scale(config.reduced_motion);
    RiseChannels {
        opacity: EASE_CHAR.ease(clamp01(local / config.char_duration_frames.max(1) as f32)),
        // Travel is an `interpolate` whose easing is applied to the progress first: the source
        // hands `travelEasing` to `interpolate`, which eases the normalised position, not the
        // output value.
        y: travel
            * (1.0
                - EASE_CHAR_TRAVEL.ease(clamp01(local / config.char_travel_frames.max(1) as f32))),
    }
}

/// The Per Character Rise component.
#[derive(Debug, Clone, Default)]
pub struct PerCharacterRise {
    /// Component options.
    pub config: PerCharacterRiseConfig,
}

impl PerCharacterRise {
    /// Build with the given options.
    pub fn new(config: PerCharacterRiseConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "per-character-rise",
            name: "Per Character Rise",
            summary: "Letters rise into place one by one, crisp and unblurred.",
            feature: "per-character-rise",
        }
    }
}

impl Component for PerCharacterRise {
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
        // The source is authored on a 1280x720 canvas, so its px values are multiplied by the
        // canvas' own scale factor: 1.0 on a 720-tall canvas, 1.5 on 1920x1080.
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let size = config.font_size * unit;

        let (centres, _) = char_centers(ctx, &family, &config.text, size);
        let center_x = ctx.width() / 2.0;
        let baseline = ctx.height() / 2.0 + ctx.baseline_offset(&family, size, size * 1.2);
        let frame = ctx.frame();

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, (character, dx)) in config.text.chars().zip(centres).enumerate() {
            let channels = character_channels(frame, index, config, unit);
            nodes.push(text_node_centered(
                &character.to_string(),
                center_x + dx,
                baseline + channels.y,
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
mod tests {
    use super::*;

    fn config() -> PerCharacterRiseConfig {
        PerCharacterRiseConfig::default()
    }

    #[test]
    fn entry_frames_staggers_every_character() {
        assert_eq!(entry_frames(0, 21, 1, 1.0), 0);
        assert_eq!(entry_frames(1, 21, 1, 1.0), 21);
        assert_eq!(entry_frames(15, 21, 1, 1.0), 35);
        // Speed 2 halves the timeline.
        assert_eq!(entry_frames(15, 21, 1, 2.0), 18);
    }

    #[test]
    fn channels_start_travelled_and_end_at_rest() {
        let config = config();
        let first = character_channels(0.0, 0, &config, 1.0);
        assert!(first.opacity.abs() < 1e-6);
        assert!((first.y - config.distance).abs() < 1e-6);

        let settled = character_channels(60.0, 0, &config, 1.0);
        assert!((settled.opacity - 1.0).abs() < 1e-6);
        assert!(settled.y.abs() < 1e-6);
    }

    #[test]
    fn later_characters_are_still_hidden_when_the_first_starts() {
        let config = config();
        let second = character_channels(0.0, 1, &config, 1.0);
        assert!(second.opacity.abs() < 1e-6);
        // Travel is clamped before the character's turn, so it waits at the full distance.
        assert!((second.y - config.distance).abs() < 1e-6);
    }

    #[test]
    fn unit_scales_travel() {
        let config = config();
        let tall = character_channels(0.0, 0, &config, 1.5);
        assert!((tall.y - config.distance * 1.5).abs() < 1e-5);
    }

    #[test]
    fn reduced_motion_keeps_the_fade() {
        let config = PerCharacterRiseConfig {
            reduced_motion: true,
            ..config()
        };
        let channels = character_channels(10.0, 0, &config, 1.5);
        assert!(channels.y.abs() < 1e-6);
        assert!(channels.opacity > 0.0 && channels.opacity <= 1.0);
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = PerCharacterRise::default();
        assert_eq!(component.config.text.chars().count(), 15);
        assert_eq!(component.duration(), 35);
        assert_eq!(component.meta().id, "per-character-rise");
        assert_eq!(component.meta().feature, "per-character-rise");
    }

    #[test]
    fn empty_text_has_no_frames() {
        let component = PerCharacterRise::new(PerCharacterRiseConfig {
            text: String::new(),
            ..config()
        });
        assert_eq!(component.duration(), 0);
    }
}
