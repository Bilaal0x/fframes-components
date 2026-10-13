//! # Squeeze In
//!
//! A line is wiped in from the left, one character at a time, and then closes up: after the last
//! character is revealed the whole run squeezes together by `squeeze * font_size` per gap over 16
//! frames under `EASE_OUT_CUBIC`, while the run recentres itself so the line stays centred as it
//! tightens.
//!
//! The wipe is a clip, not a per-character fade: the source sets `clip-path: inset(...)` on the
//! run and animates its right inset from `100%` to `0`, so characters appear at their final
//! opacity already in place.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"SQUEEZE IN"` | The line to reveal. |
//! | `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `font_weight` | `u16` | `400` | Font weight. |
//! | `stagger` | `usize` | `3` | Frames between consecutive character reveals. |
//! | `squeeze` | `f32` | `0.03` | Gap every character closes by, in `em`, once revealed. |
//! | `squeeze_frames` | `usize` | `16` | Frames the squeeze takes. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `45` | Frames held after the squeeze finishes. |
//! | `tracking_em` | `f32` | `-0.02` | Letter spacing in `em`. Negative tightens. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components squeeze-in -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! reveal_end = chars * stagger
//! duration   = ceil((reveal_end + squeeze_frames) / speed) + tail
//! ```
//!
//! With the defaults (10 characters) that is 91 frames, 3.03 s at 30 fps.
//!
//! ## Fidelity notes
//!
//! - **The wipe.** The source clips the run with `clip-path: inset(-0.2em R% -0.25em -0.1em)`,
//!   where `R` runs `100 → 0`. SVG's `clip-path` takes shapes rather than insets, so this port
//!   masks the run with a `<clipPath>` rectangle and animates its width. The visible effect is the
//!   same left-to-right wipe; the source's overflow allowances above and below the line box do not
//!   exist here, so glyphs are never clipped vertically.
//! - **Scaling.** The source is authored on a 1280x720 canvas, so every design px is multiplied by
//!   `ctx.vmin() / 7.2` (1.5 on the default 1920x1080).
//!
//! ## Credit
//!
//! Ported from `squeeze-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered};
use crate::motion::{REFERENCE_VMIN, clamp01, ease_out_cubic, interpolate, normalised_speed};

/// Configuration for [`SqueezeIn`].
#[derive(Debug, Clone, PartialEq)]
pub struct SqueezeInConfig {
    /// The line to reveal.
    pub text: String,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Text fill.
    pub color: String,
    /// Font weight.
    pub font_weight: u16,
    /// Frames between consecutive character reveals.
    pub stagger: usize,
    /// Gap every character closes by, in `em`, once revealed.
    pub squeeze: f32,
    /// Frames the squeeze takes.
    pub squeeze_frames: usize,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the squeeze finishes.
    pub tail: usize,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
}

impl Default for SqueezeInConfig {
    fn default() -> Self {
        Self {
            text: "SQUEEZE IN".to_owned(),
            font_size: 72.0,
            color: "#e6edf3".to_owned(),
            font_weight: 400,
            stagger: 3,
            squeeze: 0.03,
            squeeze_frames: 16,
            speed: 1.0,
            tail: 45,
            tracking_em: -0.02,
        }
    }
}

/// Frame the last character is revealed on: `chars * stagger`.
pub fn reveal_end(char_count: usize, stagger: usize) -> usize {
    char_count * stagger
}

/// The run's own channels at the component's frame clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SqueezeChannels {
    /// Gap between adjacent characters in canvas px.
    pub gap: f32,
    /// The source's own horizontal shift for the whole run, in canvas px: negative, and equal to
    /// `-(gap * (chars - 1)) / 2`, which is exactly the recentring the live gaps require. Consumers
    /// are free to use either — [`SqueezeIn`] derives the shift from `gap` so the two can never
    /// disagree.
    pub anchor_x: f32,
    /// Share of the run still hidden by the wipe, `100` at frame 0 and `0` once revealed.
    pub wipe_percent: f32,
}

/// Sample the run's channels.
pub fn squeeze_channels(
    frame: f32,
    config: &SqueezeInConfig,
    unit: f32,
    char_count: usize,
) -> SqueezeChannels {
    let local = frame * normalised_speed(config.speed);
    let last_reveal = reveal_end(char_count, config.stagger) as f32;
    let squeeze_p = ease_out_cubic(clamp01(
        (local - last_reveal) / config.squeeze_frames.max(1) as f32,
    ));
    let gap = config.squeeze * config.font_size * unit * (1.0 - squeeze_p);
    SqueezeChannels {
        gap,
        anchor_x: -(gap * char_count.saturating_sub(1) as f32) / 2.0,
        wipe_percent: interpolate(local, [0.0, last_reveal.max(1.0)], [100.0, 0.0]),
    }
}

/// The Squeeze In component.
#[derive(Debug, Clone, Default)]
pub struct SqueezeIn {
    /// Component options.
    pub config: SqueezeInConfig,
}

impl SqueezeIn {
    /// Build with the given options.
    pub fn new(config: SqueezeInConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "squeeze-in",
            name: "Squeeze In",
            summary: "A line wipes in character by character, then closes up tight.",
            feature: "squeeze-in",
        }
    }
}

impl Component for SqueezeIn {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        let count = self.config.text.chars().count();
        let frames = reveal_end(count, self.config.stagger) + self.config.squeeze_frames;
        (frames as f32 / normalised_speed(self.config.speed))
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
        let family = ctx.fonts.sans.clone();
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let size = config.font_size * unit;
        let chars: Vec<char> = config.text.chars().collect();
        if chars.is_empty() {
            return Svgr::empty();
        }
        let channels = squeeze_channels(ctx.frame(), config, unit, chars.len());

        // Measured at the resting advances, then re-spaced by the shrinking gap so the run
        // recentres around its own middle exactly as the source's inline gaps do.
        let widths = advances(ctx, &family, size, &chars);
        let total: f32 = widths.iter().sum();
        let recentre = if chars.len() > 1 {
            channels.gap * (chars.len() - 1) as f32 / 2.0
        } else {
            0.0
        };
        let mut x = -total / 2.0 - recentre;
        let center_x = ctx.width() / 2.0;
        let baseline = ctx.height() / 2.0 + ctx.baseline_offset(&family, size, size * 1.1);

        let clip_id = "squeeze-in-clip".to_owned();
        // The clip is the *laid-out* run, which is wider than the resting advances by the live
        // gaps and shifted by the same recentring the characters are. Anchoring it to the resting
        // width instead would slice the leading glyph once the run had shifted left of it.
        let clip_left = center_x + x;
        // At frame 0 the wipe hides everything; a zero-width rect is invalid SVG, so the clip is
        // floored at a pixel, which is still exactly one frame of a 72 px emblem hidden.
        let clip_width = ((total + channels.gap * (chars.len() - 1) as f32)
            * (1.0 - channels.wipe_percent / 100.0))
            .max(1.0);

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, character) in chars.iter().enumerate() {
            nodes.push(text_node_centered(
                &character.to_string(),
                center_x + x + widths[index] / 2.0,
                baseline,
                family.clone(),
                size,
                config.font_weight,
                config.tracking_em,
                config.color.clone(),
                1.0,
            ));
            x += widths[index] + channels.gap;
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id={clip_id.clone()}>
                        <rect x={clip_left} y="0" width={clip_width}
                              height={ctx.height()} />
                    </clipPath>
                </defs>
                <g clip-path={format!("url(#{clip_id})")}>{nodes}</g>
            </g>
        )
    }
}

/// Measured advance of every character, which is the whole spacing budget of the run.
fn advances(ctx: &RenderCtx<'_>, family: &str, size: f32, chars: &[char]) -> Vec<f32> {
    chars
        .iter()
        .map(|character| {
            let mut buffer = [0_u8; 4];
            ctx.text_width(family, size, character.encode_utf8(&mut buffer))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wipe_reveals_one_character_per_stagger() {
        let config = SqueezeInConfig::default();
        assert_eq!(reveal_end(10, 3), 30);
        // The wipe is full at frame 0 and gone by the last reveal.
        assert!((squeeze_channels(0.0, &config, 1.0, 10).wipe_percent - 100.0).abs() < 1e-6);
        assert!(squeeze_channels(30.0, &config, 1.0, 10).wipe_percent.abs() < 1e-6);
    }

    #[test]
    fn the_squeeze_only_starts_after_the_last_reveal() {
        let config = SqueezeInConfig::default();
        let open = squeeze_channels(30.0, &config, 1.0, 10);
        assert!((open.gap - config.squeeze * config.font_size).abs() < 1e-6);
        // The run is recentred by exactly half of its own gaps, at every point of the squeeze.
        let expected = -(open.gap * 9.0) / 2.0;
        assert!(
            (open.anchor_x - expected).abs() < 1e-4,
            "got {}",
            open.anchor_x
        );

        let closed = squeeze_channels(46.0, &config, 1.0, 10);
        assert!(closed.gap.abs() < 1e-6, "gap was {}", closed.gap);
        assert!(closed.anchor_x.abs() < 1e-6, "a closed run needs no shift");
    }

    #[test]
    fn the_gap_scales_with_the_canvas() {
        let config = SqueezeInConfig::default();
        let wide = squeeze_channels(0.0, &config, 1.5, 10);
        let tall = squeeze_channels(0.0, &config, 1.0, 10);
        assert!((wide.gap / tall.gap - 1.5).abs() < 1e-5);
    }

    #[test]
    fn duration_counts_the_whole_run() {
        let component = SqueezeIn::default();
        assert_eq!(component.config.text.chars().count(), 10);
        assert_eq!(component.duration(), 91);
        assert_eq!(component.meta().id, "squeeze-in");
    }

    #[test]
    fn empty_text_is_just_the_squeeze_window() {
        let component = SqueezeIn::new(SqueezeInConfig {
            text: String::new(),
            ..SqueezeInConfig::default()
        });
        assert_eq!(component.duration(), 16 + 45);
    }
}
