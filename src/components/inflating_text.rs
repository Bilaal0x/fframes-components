//! # Inflating Text
//!
//! Letters arrive one after another and inflate past their rest size. Each letter's own clock
//! opens with a 4 frame anticipation: a small squash (wider, shorter) that releases exactly as
//! the swell starts. The swell is a damped sine kick normalised to its own peak, so a letter
//! rises to `inflate_scale`, wobbles once *below* rest and lands exactly on 1 after 14 frames.
//! While a letter is away from 1 it rotates up to `wobble` degrees (a deterministic per-letter
//! direction), lifts `bounce * font_size` px, and tints from `color` toward `accent_color` by how
//! far it is from rest. After the run holds, letters deflate out one after another: each shrinks
//! 15%, fades to 0 and holds the accent tint as it goes.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Make It Pop"` | The line to animate, split unicode-safe per character. |
//! | `font_size` | `f32` | `110.0` | Font size in px. |
//! | `font_weight` | `u16` | `800` | Font weight. |
//! | `inflate_scale` | `f32` | `1.22` | Peak scale a letter overshoots to mid-inflate. |
//! | `stagger` | `usize` | `2` | Frames between consecutive letters starting their swell. |
//! | `delay` | `usize` | `0` | Frames before the first letter starts. |
//! | `color` | `String` | `"#f5f5f7"` | Base fill of the letters. |
//! | `accent_color` | `String` | `"#ffd166"` | Letters tint toward this colour at peak inflation. |
//! | `tracking_em` | `f32` | `-0.02` | Letter spacing in `em` (`letterSpacing: "-0.02em"`). |
//! | `hold` | `usize` | `20` | Frames the whole line holds, inflated, before the deflate tail. |
//! | `deflate_stagger` | `usize` | `2` | Frames between consecutive letters starting to deflate. |
//! | `reduced_motion` | `bool` | `false` | Collapse every size/position channel; keep the tint and fades. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components inflating-text -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `InflatingText.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, chars};
use crate::motion::{clamp01, mix_hex, motion_scale, random};

/// Frames a letter spends in the anticipation squash before the swell starts.
pub const ANTICIPATE_FRAMES: usize = 4;
/// Frames a letter's swell takes, kick included.
pub const INFLATE_FRAMES: usize = 14;
/// Frames the whole line holds, inflated, before the deflate tail starts.
pub const HOLD_FRAMES: usize = 20;
/// Frames a letter's deflate-out takes.
pub const DEFLATE_FRAMES: usize = 8;
/// Anticipation squash depth (`scale_x` gains, `scale_y` loses).
pub const SQUASH: f32 = 0.08;
/// Max rotation wobble in degrees, at mid-inflate.
pub const WOBBLE_DEG: f32 = 4.0;
/// Upward bounce at peak inflation, as a fraction of `font_size`.
pub const BOUNCE_EM: f32 = 0.05;
/// Gooey blur cap as a fraction of `font_size` — kept as documentation only, see below.
pub const GOOEY_BLUR_CAP: f32 = 0.04;
/// `|scale velocity|` sum of the fastest neighbour pair that maps to full goo — documentation
/// only, see below.
pub const SPEED_NORM: f32 = 0.3;
/// The `feColorMatrix` of the source's gooey filter. Reproduced verbatim for the record; the
/// filter is **not emitted** (see the README: SVG filters are not executable in this pipeline).
pub const GOOEY_MATRIX: &str = "1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 19 -9";

/// The source's `INFLATE_KICK`: a damped sine, `exp(-2u) * sin(2*pi*u)`, on `0..=1`.
///
/// It rises to a single positive lobe, crosses zero at `u = 0.5`, dips into a negative lobe and
/// returns to 0 at `u = 1`, which is what makes a letter overshoot, fall slightly short and land
/// exactly on its rest size.
pub fn inflate_kick(u: f32) -> f32 {
    (-2.0 * u).exp() * (2.0 * std::f32::consts::PI * u).sin()
}

/// The peak of [`inflate_kick`] on `0..=1`, sampled once at `i / 1000` for `i` in `1..=1000`.
///
/// The source computes this at module load so the kick can be normalised to exactly 1. The
/// sampled maximum is deterministic, which is what keeps frames reproducible.
pub fn inflate_kick_peak() -> f32 {
    static PEAK: OnceLock<f32> = OnceLock::new();
    *PEAK.get_or_init(|| {
        let mut peak = 0.0_f32;
        for i in 1..=1000 {
            peak = peak.max(inflate_kick(i as f32 / 1000.0));
        }
        peak
    })
}

/// A letter's three scale channels during the swell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LetterScale {
    /// The uniform scale the kick drives.
    pub scale: f32,
    /// `scale` plus the anticipation squash on x.
    pub scale_x: f32,
    /// `scale` minus the anticipation squash on y.
    pub scale_y: f32,
}

/// Scale multipliers of a letter at absolute `frame`, whose swell starts at `start`.
///
/// `start` is in frames and fractional frames are fine, so the same function serves both the
/// current frame and the previous one.
pub fn letter_scale(frame: f32, start: f32, inflate_scale: f32) -> LetterScale {
    let anticipate = ANTICIPATE_FRAMES as f32;
    let inflate = INFLATE_FRAMES as f32;
    let t_a = clamp01((frame - start) / anticipate);
    // 0 -> 1 -> 0 inside the anticipation window.
    let squash = (std::f32::consts::PI * t_a).sin();
    let t_i = clamp01((frame - start - anticipate) / inflate);
    let peak = inflate_kick_peak();
    let kick = if peak > 0.0 {
        inflate_kick(t_i) / peak
    } else {
        0.0
    };
    let scale = 1.0 + (inflate_scale - 1.0) * kick;
    LetterScale {
        scale,
        scale_x: scale * (1.0 + SQUASH * squash),
        scale_y: scale * (1.0 - SQUASH * squash),
    }
}

/// One letter's animated state at a frame, after the phase maths.
#[derive(Debug, Clone, PartialEq)]
pub struct LetterState {
    /// The character this state belongs to.
    pub ch: char,
    /// Scale along x: the swell, the anticipation squash and the deflate multiplier.
    pub scale_x: f32,
    /// Scale along y: the swell, the anticipation squash and the deflate multiplier.
    pub scale_y: f32,
    /// Rotation in degrees for the deterministic wobble.
    pub rotate_deg: f32,
    /// Vertical offset in px; negative lifts the letter.
    pub translate_y: f32,
    /// Fill at this frame, tinted from `color` toward `accent_color`.
    pub fill: String,
    /// Opacity, driven by the deflate tail.
    pub opacity: f32,
}

/// The animated state of letter `index` of `count` at `frame`.
///
/// Returns `None` for a space, exactly like the source's per-letter map: spaces advance the line
/// without getting a transform of their own.
pub fn letter_state(
    frame: f32,
    index: usize,
    count: usize,
    ch: char,
    config: &InflatingTextConfig,
) -> Option<LetterState> {
    if ch == ' ' {
        return None;
    }
    let move_scale = motion_scale(config.reduced_motion);
    let start = config.delay as f32 + index as f32 * config.stagger as f32;
    let pose = letter_scale(frame, start, config.inflate_scale);

    // Deflate-out tail, staggered after the global hold.
    let deflate_start = config.delay as f32
        + count.saturating_sub(1) as f32 * config.stagger as f32
        + ANTICIPATE_FRAMES as f32
        + INFLATE_FRAMES as f32
        + config.hold as f32;
    let t_d = clamp01(
        (frame - deflate_start - index as f32 * config.deflate_stagger as f32)
            / DEFLATE_FRAMES as f32,
    );
    let opacity = 1.0 - t_d;
    let deflate_mul = 1.0 - 0.15 * t_d;

    let t_i = clamp01((frame - start - ANTICIPATE_FRAMES as f32) / INFLATE_FRAMES as f32);
    // Deterministic wobble direction per letter, peaking mid-inflate.
    let dir = if random(&format!("inflate-{index}")) < 0.5 {
        -1.0
    } else {
        1.0
    };
    let rotate = dir * WOBBLE_DEG * (std::f32::consts::PI * t_i).sin() * move_scale;

    // The span the kick covers, guarded so a degenerate `inflate_scale` cannot divide by zero.
    let span = (config.inflate_scale - 1.0).max(1e-6);
    // Gentle lift synced to the inflation overshoot.
    let bounce = (pose.scale - 1.0) / span * BOUNCE_EM * config.font_size * move_scale;

    // Tint toward the accent colour with the same intensity.
    let intensity = clamp01((pose.scale - 1.0).abs() / span);
    let fill = mix_hex(&config.color, &config.accent_color, intensity);

    Some(LetterState {
        ch,
        // Reduced motion keeps the accent tint as the state cue and drops every
        // size/position channel.
        scale_x: 1.0 + (pose.scale_x * deflate_mul - 1.0) * move_scale,
        scale_y: 1.0 + (pose.scale_y * deflate_mul - 1.0) * move_scale,
        rotate_deg: rotate,
        translate_y: -bounce,
        fill,
        opacity,
    })
}

/// `getInflatingTextDuration`: total frames until every letter has swelled, held and deflated out.
///
/// The source takes `{ stagger = 2, delay = 0, hold = 20, deflateStagger = 2 }` as options and
/// returns `max(1, ceil(total))`; every term is a whole number of frames, so the ceiling is
/// applied by construction here. An empty line lasts 1 frame.
pub fn get_inflating_text_duration(
    text: &str,
    stagger: usize,
    delay: usize,
    hold: usize,
    deflate_stagger: usize,
) -> usize {
    let count = chars(text).len();
    if count == 0 {
        return 1;
    }
    let last_start = delay + (count - 1) * stagger;
    let total = last_start
        + ANTICIPATE_FRAMES
        + INFLATE_FRAMES
        + hold
        + (count - 1) * deflate_stagger
        + DEFLATE_FRAMES;
    total.max(1)
}

/// Configuration for [`InflatingText`].
#[derive(Debug, Clone, PartialEq)]
pub struct InflatingTextConfig {
    /// The line to animate, split unicode-safe per character.
    pub text: String,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Peak scale a letter overshoots to mid-inflate.
    pub inflate_scale: f32,
    /// Frames between consecutive letters starting their swell.
    pub stagger: usize,
    /// Frames before the first letter starts.
    pub delay: usize,
    /// Base fill of the letters.
    pub color: String,
    /// Letters tint toward this colour at peak inflation.
    pub accent_color: String,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
    /// Frames the whole line holds, inflated, before the deflate tail starts.
    pub hold: usize,
    /// Frames between consecutive letters starting to deflate.
    pub deflate_stagger: usize,
    /// Collapse every size/position channel; the tint and the fades stay.
    pub reduced_motion: bool,
}

impl Default for InflatingTextConfig {
    fn default() -> Self {
        Self {
            text: "Make It Pop".to_owned(),
            font_size: 110.0,
            font_weight: 800,
            inflate_scale: 1.22,
            stagger: 2,
            delay: 0,
            color: "#f5f5f7".to_owned(),
            accent_color: "#ffd166".to_owned(),
            tracking_em: -0.02,
            hold: HOLD_FRAMES,
            deflate_stagger: 2,
            reduced_motion: false,
        }
    }
}

/// The Inflating Text component.
#[derive(Debug, Clone, Default)]
pub struct InflatingText {
    /// Component options.
    pub config: InflatingTextConfig,
}

impl InflatingText {
    /// Build with the given options.
    pub fn new(config: InflatingTextConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "inflating-text",
            name: "Inflating Text",
            summary: "Letters squash, inflate past rest size, wobble and tint, then deflate out.",
            feature: "inflating-text",
        }
    }
}

/// Distance from the vertical centre of a font's em box down to its baseline.
fn em_center_to_baseline(ctx: &RenderCtx<'_>, family: &str, size: f32) -> f32 {
    match ctx.meter(family) {
        Some(meter) => (meter.ascent(size) - meter.descent(size)) / 2.0,
        // The same 0.75 / 0.25 ratios `RenderCtx` falls back to.
        None => size * 0.25,
    }
}

impl Component for InflatingText {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_inflating_text_duration(
            &self.config.text,
            self.config.stagger,
            self.config.delay,
            self.config.hold,
            self.config.deflate_stagger,
        )
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = ctx.sans();
        let size = config.font_size;
        let mid = em_center_to_baseline(ctx, &family, size);
        let baseline = ctx.height() / 2.0 + mid;
        let frame = ctx.frame();

        let characters = chars(&config.text);
        let widths: Vec<f32> = characters
            .iter()
            .map(|ch| ctx.text_width(&family, size, &ch.to_string()))
            .collect();
        let tracking_px = config.tracking_em * size;
        let total =
            widths.iter().sum::<f32>() + tracking_px * characters.len().saturating_sub(1) as f32;
        let mut cursor = ctx.width() / 2.0 - total / 2.0;

        let mut nodes: Vec<Svgr<'_>> = Vec::with_capacity(characters.len());
        for (index, ch) in characters.iter().enumerate() {
            let width = widths.get(index).copied().unwrap_or(0.0);
            let center = cursor + width / 2.0;
            cursor += width + tracking_px;

            // A space advances the line and is not drawn.
            let Some(state) = letter_state(frame, index, characters.len(), *ch, config) else {
                continue;
            };
            let transform = format!(
                "translate({:.4} {:.4}) rotate({:.4}) scale({:.6} {:.6})",
                center,
                baseline + state.translate_y,
                state.rotate_deg,
                state.scale_x,
                state.scale_y,
            );
            let glyph = fframes::svgr!(
                <text x={0.0_f32} y={mid} font-family={family.clone()} font-size={size}
                      font-weight={config.font_weight} letter-spacing={config.tracking_em}
                      fill={state.fill} opacity={1.0} text-anchor="middle">
                    {state.ch.to_string()}
                </text>
            );
            nodes.push(fframes::svgr!(
                <g transform={transform} opacity={state.opacity}>{glyph}</g>
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn kick_is_a_damped_sine_on_zero_to_one() {
        assert!(close(inflate_kick(0.0), 0.0, 1e-6));
        assert!(close(inflate_kick(0.5), 0.0, 1e-5), "{}", inflate_kick(0.5));
        // The negative lobe below rest.
        assert!(inflate_kick(0.7) < 0.0);
        // And it returns to 0 at the end.
        assert!(close(inflate_kick(1.0), 0.0, 1e-3), "{}", inflate_kick(1.0));
    }

    #[test]
    fn kick_peak_matches_the_sampled_maximum() {
        // The analytic peak sits at u = atan(pi) / (2*pi) ~ 0.20095.
        let peak = inflate_kick_peak();
        assert!(close(peak, 0.6375, 1e-3), "peak was {peak}");
        // The scan starts at i = 1, so the peak is the largest sampled value.
        for i in 0..=1000 {
            assert!(inflate_kick(i as f32 / 1000.0) <= peak + 1e-6);
        }
        // Sampled once: the same value every call.
        assert!(close(inflate_kick_peak(), peak, 0.0));
    }

    #[test]
    fn letter_scale_starts_at_rest_squashes_then_overshoots() {
        // At the letter's own start frame the anticipation squash has not begun.
        let at_start = letter_scale(10.0, 10.0, 1.22);
        assert!(close(at_start.scale, 1.0, 1e-6));
        assert!(close(at_start.scale_x, 1.0, 1e-6));
        assert!(close(at_start.scale_y, 1.0, 1e-6));

        // Mid-anticipation: wider and shorter, still at rest scale.
        let mid_anticipate = letter_scale(12.0, 10.0, 1.22);
        assert!(close(mid_anticipate.scale, 1.0, 1e-6));
        assert!(mid_anticipate.scale_x > 1.0);
        assert!(mid_anticipate.scale_y < 1.0);
        assert!(close(mid_anticipate.scale_x, 1.0 + SQUASH, 1e-5));

        // Full anticipation, the kick has not started yet.
        let after_anticipate = letter_scale(14.0, 10.0, 1.22);
        assert!(close(after_anticipate.scale, 1.0, 1e-6));

        // u = 0.2 into the 14 frame swell is (almost) the normalised peak: 1 + (1.22 - 1) * 1.
        let peak = letter_scale(14.0 + 0.2 * INFLATE_FRAMES as f32, 10.0, 1.22);
        assert!(close(peak.scale, 1.22, 2e-3), "scale was {}", peak.scale);

        // The negative lobe takes it below rest.
        let under = letter_scale(14.0 + 0.7 * INFLATE_FRAMES as f32, 10.0, 1.22);
        assert!(under.scale < 1.0, "scale was {}", under.scale);

        // The end of the swell lands exactly on rest.
        let end = letter_scale(
            10.0 + (ANTICIPATE_FRAMES + INFLATE_FRAMES) as f32,
            10.0,
            1.22,
        );
        assert!(close(end.scale, 1.0, 1e-3), "scale was {}", end.scale);
    }

    #[test]
    fn letter_state_holds_the_accent_tint_and_fades_out() {
        let config = InflatingTextConfig::default();
        // 11 characters: the last deflate starts at 20 + 20 + 4 + 14 + 20 = 78.
        let count = chars(&config.text).len();
        assert_eq!(count, 11);

        let at_peak =
            letter_state(2.0 + 4.0 + 2.8, 0, count, 'M', &config).expect("a letter gets a state");
        assert!(at_peak.fill.starts_with('#'));
        assert!(
            at_peak.translate_y < 0.0,
            "the letter lifts at peak inflation"
        );

        // Spaces get no state at all.
        assert!(letter_state(10.0, 0, count, ' ', &config).is_none());

        // Well past the tail everything has faded out.
        let deflated =
            letter_state(200.0, count - 1, count, 'p', &config).expect("a letter gets a state");
        assert!(close(deflated.opacity, 0.0, 1e-6));
        // The deflate multiplier never scales below 0.85.
        assert!(close(deflated.scale_x, 1.0 - 0.15, 1e-5));
    }

    #[test]
    fn reduced_motion_drops_every_size_channel_but_keeps_the_tint() {
        let config = InflatingTextConfig {
            reduced_motion: true,
            ..InflatingTextConfig::default()
        };
        let state = letter_state(8.0, 0, 11, 'M', &config).expect("a letter gets a state");
        assert!(close(state.scale_x, 1.0, 1e-6));
        assert!(close(state.scale_y, 1.0, 1e-6));
        assert!(close(state.rotate_deg, 0.0, 1e-6));
        assert!(close(state.translate_y, 0.0, 1e-6));
        // The tint is not a movement channel, so it stays.
        assert_ne!(state.fill, config.color);
    }

    #[test]
    fn duration_matches_the_source_formula() {
        // "Make It Pop": 11 characters, 11 - 1 = 10 gaps of 2 frames.
        // 20 + 4 + 14 + 20 + 10 * 2 + 8 = 86.
        assert_eq!(get_inflating_text_duration("Make It Pop", 2, 0, 20, 2), 86);
        // One character: no stagger at all.
        assert_eq!(
            get_inflating_text_duration("A", 2, 0, 20, 2),
            4 + 14 + 20 + 8
        );
        // An empty line still occupies a frame.
        assert_eq!(get_inflating_text_duration("", 2, 0, 20, 2), 1);
        // The delay shifts every letter.
        assert_eq!(
            get_inflating_text_duration("A", 2, 5, 20, 2),
            5 + 4 + 14 + 20 + 8
        );
        // The deflate stagger is counted per gap, not for the last letter.
        assert_eq!(
            get_inflating_text_duration("ABC", 0, 0, 20, 3),
            4 + 14 + 20 + 2 * 3 + 8
        );
    }

    #[test]
    fn component_duration_matches_the_helper() {
        let component = InflatingText::default();
        assert_eq!(component.duration(), 86);
        assert_eq!(component.meta().id, "inflating-text");
        assert_eq!(component.meta().feature, "inflating-text");
    }

    #[test]
    fn every_published_constant_keeps_the_source_value() {
        assert_eq!(ANTICIPATE_FRAMES, 4);
        assert_eq!(INFLATE_FRAMES, 14);
        assert_eq!(HOLD_FRAMES, 20);
        assert_eq!(DEFLATE_FRAMES, 8);
        assert!(close(SQUASH, 0.08, 0.0));
        assert!(close(WOBBLE_DEG, 4.0, 0.0));
        assert!(close(BOUNCE_EM, 0.05, 0.0));
        assert!(close(GOOEY_BLUR_CAP, 0.04, 0.0));
        assert!(close(SPEED_NORM, 0.3, 0.0));
        assert_eq!(GOOEY_MATRIX, "1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 19 -9");
    }
}
