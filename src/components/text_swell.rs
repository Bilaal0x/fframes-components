//! # Text Swell
//!
//! A title reveal built around one idea: the lead word is pushed aside by the words that follow
//! it.
//!
//! 1. The lead word rises from below (0.7 em) and settles, centred, at natural size.
//! 2. The whole line floats forward toward the viewer (scale 1 -> 2.1, capped so the widest
//!    moment stays inside `FIT` of the frame width). The lead is still alone, so it stays dead
//!    centre as it grows.
//! 3. The second word cuts in from the right while its letters swell up off the baseline one
//!    after another; each arriving word shoves the line left by exactly its share of the width it
//!    added. Later words push in without a letter bounce.
//! 4. Once everything has landed the line falls back to its final size and settles.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Big Ideas Move Fast"` | One line; the first word is the lead word. |
//! | `rise_distance_em` | `f32` | `0.7` | Beat 1: how far below rest the lead starts, in `em`. |
//! | `rise_duration` | `usize` | `10` | Beat 1: frames the lead's rise takes. |
//! | `front_scale` | `f32` | `2.1` | Beat 2: peak scale the line floats forward to. |
//! | `approach_delay` | `usize` | `14` | Beat 2: frames before the float forward starts. |
//! | `approach_duration` | `usize` | `20` | Beat 2: frames the float forward takes. |
//! | `word_delay` | `usize` | `27` | Beat 3: frames before the first trailing word pushes in. |
//! | `word_stagger` | `usize` | `14` | Beat 3: frames between consecutive words pushing in. |
//! | `word_push_em` | `f32` | `0.15` | Beat 3: how far right a word starts, in `em`. |
//! | `word_push_duration` | `usize` | `12` | Beat 3: frames one word's push takes. |
//! | `bounce_words` | `usize` | `1` | How many leading trailing words bounce letter-by-letter. |
//! | `letter_swell` | `f32` | `0.23` | Peak extra scale of a bouncing letter. |
//! | `letter_stagger` | `usize` | `2` | Frames between consecutive letters bouncing. |
//! | `letter_rise` | `usize` | `3` | Frames a letter's bounce up takes. |
//! | `letter_hold` | `usize` | `0` | Frames a letter holds its peak scale. |
//! | `letter_fall` | `usize` | `6` | Frames a letter's fall back takes. |
//! | `hold_duration` | `usize` | `6` | Beat 4: frames held after the last word lands. |
//! | `recede_duration` | `usize` | `18` | Beat 4: frames the fall back to final size takes. |
//! | `delay` | `usize` | `0` | Frames to wait before the reveal starts. |
//! | `font_size` | `f32` | `72.0` | Font size in px. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `color` | `String` | `"#e6edf3"` | Text fill (the source's `lineStyle`). |
//! | `tracking_em` | `f32` | `-0.03` | Letter spacing in `em` (the source's `lineStyle`). |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components text-swell -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `TextSwell.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered, words};
use crate::motion::{CubicBezier, EASE_ZOOM_IN, clamp01};

/// Beat 1: the lead word's rise off the baseline.
pub const RISE_EASE: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);
/// Beat 2: the line floating forward toward the viewer.
pub const APPROACH_EASE: CubicBezier = CubicBezier::new(0.4, 0.0, 0.15, 1.0);
/// Beat 3: one letter's bounce up off the baseline.
pub const LETTER_EASE: CubicBezier = CubicBezier::new(0.4, 0.0, 0.2, 1.0);
/// Beat 3: one word pushing in.
pub const PUSH_EASE: CubicBezier = CubicBezier::new(0.22, 0.8, 0.36, 1.0);
/// The widest moment stays within this fraction of the frame width.
pub const FIT: f32 = 0.97;

/// The geometry the source measures from a hidden, untransformed copy of the line.
#[derive(Debug, Clone, PartialEq)]
pub struct SwellMetrics {
    /// Full width of the line, spaces included.
    pub line_width: f32,
    /// Centre of the lead word, measured from the line's left edge.
    pub lead_center: f32,
    /// Top of the line box -> text baseline. Always `0.0` here: the SVG port draws every word in
    /// a coordinate system whose origin is already the baseline, which is exactly what the
    /// source's zero-height ruler measures.
    pub baseline: f32,
    /// Width of each word, lead first.
    pub word_widths: Vec<f32>,
    /// Width of one space between two words.
    pub space_width: f32,
}

/// Build the metrics from measured widths, the way the source builds them from the DOM.
///
/// `word_widths` must be indexed the way the line reads, lead word first.
pub fn swell_metrics(word_widths: Vec<f32>, space_width: f32) -> SwellMetrics {
    let count = word_widths.len();
    let line_width = word_widths.iter().sum::<f32>() + space_width * count.saturating_sub(1) as f32;
    let lead_center = word_widths.first().copied().unwrap_or(0.0) / 2.0;
    SwellMetrics {
        line_width,
        lead_center,
        baseline: 0.0,
        word_widths,
        space_width,
    }
}

/// First frame of the push of trailing word `j` (`j = 0` is the word after the lead).
pub fn word_start(j: usize, word_delay: usize, word_stagger: usize) -> f32 {
    word_delay as f32 + j as f32 * word_stagger as f32
}

/// `clamp01(numerator / duration)`; a zero-frame beat completes instantly instead of `0/0`.
fn progress(numerator: f32, duration: usize) -> f32 {
    if duration == 0 {
        return if numerator > 0.0 { 1.0 } else { 0.0 };
    }
    clamp01(numerator / duration as f32)
}

/// `share[j] = added[j] / totalAdded`, where `added[j]` is word `j + 1` plus one space.
///
/// Returns an empty vector when there is no trailing word; a line whose added width is 0 gets
/// zero shares rather than `0/0`.
pub fn swell_shares(word_widths: &[f32], space_width: f32, trailing_count: usize) -> Vec<f32> {
    if trailing_count == 0 {
        return Vec::new();
    }
    let added: Vec<f32> = (0..trailing_count)
        .map(|j| word_widths.get(j + 1).copied().unwrap_or(0.0) + space_width)
        .collect();
    let total: f32 = added.iter().sum();
    if total <= 0.0 {
        return vec![0.0; trailing_count];
    }
    added.iter().map(|w| w / total).collect()
}

/// How much of the shove has been applied by `frame`: the sum of each landed word's share.
pub fn swell_shoved(
    frame: f32,
    shares: &[f32],
    word_delay: usize,
    word_stagger: usize,
    word_push_duration: usize,
) -> f32 {
    let mut shoved = 0.0;
    for (j, share) in shares.iter().enumerate() {
        let p = PUSH_EASE.ease(progress(
            frame - word_start(j, word_delay, word_stagger),
            word_push_duration,
        ));
        shoved += share * p;
    }
    shoved
}

/// The line's horizontal offset under scale, pivoted at the line centre + baseline.
///
/// `tx = scale * (line_width / 2 - lead_center) * (1 - shoved)`: the lead sits dead centre while
/// it is alone, and the line ends exactly at its flexbox rest position.
pub fn swell_tx(scale: f32, line_width: f32, lead_center: f32, shoved: f32) -> f32 {
    scale * (line_width / 2.0 - lead_center) * (1.0 - shoved)
}

/// The fit cap on the front scale: `FIT * frame_width / line_width`.
///
/// The source falls back to `front_scale` when the line has not been measured; a zero-width line
/// gets the same treatment here.
pub fn swell_fit_cap(front_scale: f32, line_width: f32, frame_width: f32) -> f32 {
    if line_width > 0.0 {
        FIT * frame_width / line_width
    } else {
        front_scale
    }
}

/// Beat 2: the line's scale at `frame` (1 before the approach starts).
pub fn swell_line_scale(
    frame: f32,
    approach_delay: usize,
    approach_duration: usize,
    front_scale: f32,
    line_width: f32,
    frame_width: f32,
) -> f32 {
    let approach_p = APPROACH_EASE.ease(progress(frame - approach_delay as f32, approach_duration));
    let front = front_scale.min(swell_fit_cap(front_scale, line_width, frame_width));
    if frame < approach_delay as f32 {
        1.0
    } else {
        1.0 + (front - 1.0) * approach_p
    }
}

/// Beat 4: the first frame of the fall back to final size.
///
/// The last word lands either when the last trailing word's push ends, or when the approach ends
/// when the line is a single word; `hold_duration` is waited out on top of that.
#[allow(clippy::too_many_arguments)]
pub fn swell_recede_start(
    trailing_count: usize,
    approach_delay: usize,
    approach_duration: usize,
    word_delay: usize,
    word_stagger: usize,
    word_push_duration: usize,
    hold_duration: usize,
) -> f32 {
    let last_end = if trailing_count == 0 {
        (approach_delay + approach_duration) as f32
    } else {
        word_start(trailing_count - 1, word_delay, word_stagger) + word_push_duration as f32
    };
    last_end.max((approach_delay + approach_duration) as f32) + hold_duration as f32
}

/// One bouncing letter's scale: `1 + letter_swell * (rise_pulse - fall_pulse)`.
pub fn letter_swell_scale(rise_pulse: f32, fall_pulse: f32, letter_swell: f32) -> f32 {
    1.0 + letter_swell * (rise_pulse - fall_pulse)
}

/// The subset of [`TextSwellConfig`] that [`get_text_swell_duration`] reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSwellTiming {
    /// Frames before the float forward starts.
    pub approach_delay: usize,
    /// Frames the float forward takes.
    pub approach_duration: usize,
    /// Frames before the first trailing word pushes in.
    pub word_delay: usize,
    /// Frames between consecutive words pushing in.
    pub word_stagger: usize,
    /// Frames one word's push takes.
    pub word_push_duration: usize,
    /// Frames held after the last word lands.
    pub hold_duration: usize,
    /// Frames the fall back to final size takes.
    pub recede_duration: usize,
}

impl Default for TextSwellTiming {
    fn default() -> Self {
        Self {
            approach_delay: 14,
            approach_duration: 20,
            word_delay: 27,
            word_stagger: 14,
            word_push_duration: 12,
            hold_duration: 6,
            recede_duration: 18,
        }
    }
}

/// `getTextSwellDuration`: total frames until the line settles at final size.
///
/// The source defaults every option to the component's own default, so
/// `get_text_swell_duration(text, &TextSwellTiming::default())` is the source's no-options call.
/// A single-word line ends when the approach ends.
pub fn get_text_swell_duration(text: &str, timing: &TextSwellTiming) -> usize {
    let trailing = words(text).len().saturating_sub(1);
    let last_end = if trailing == 0 {
        timing.approach_delay + timing.approach_duration
    } else {
        timing.word_delay + (trailing - 1) * timing.word_stagger + timing.word_push_duration
    };
    last_end.max(timing.approach_delay + timing.approach_duration)
        + timing.hold_duration
        + timing.recede_duration
}

/// Configuration for [`TextSwell`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextSwellConfig {
    /// One line; the first word is the lead word that gets shoved aside.
    pub text: String,
    /// Beat 1: how far below rest the lead starts, in `em`.
    pub rise_distance_em: f32,
    /// Beat 1: frames the lead's rise takes.
    pub rise_duration: usize,
    /// Beat 2: peak scale the line floats forward to.
    pub front_scale: f32,
    /// Beat 2: frames before the float forward starts.
    pub approach_delay: usize,
    /// Beat 2: frames the float forward takes.
    pub approach_duration: usize,
    /// Beat 3: frames before the first trailing word pushes in.
    pub word_delay: usize,
    /// Beat 3: frames between consecutive words pushing in.
    pub word_stagger: usize,
    /// Beat 3: how far right a word starts, in `em`.
    pub word_push_em: f32,
    /// Beat 3: frames one word's push takes.
    pub word_push_duration: usize,
    /// How many leading trailing words bounce letter-by-letter.
    pub bounce_words: usize,
    /// Peak extra scale of a bouncing letter.
    pub letter_swell: f32,
    /// Frames between consecutive letters bouncing.
    pub letter_stagger: usize,
    /// Frames a letter's bounce up takes.
    pub letter_rise: usize,
    /// Frames a letter holds its peak scale.
    pub letter_hold: usize,
    /// Frames a letter's fall back takes.
    pub letter_fall: usize,
    /// Beat 4: frames held after the last word lands.
    pub hold_duration: usize,
    /// Beat 4: frames the fall back to final size takes.
    pub recede_duration: usize,
    /// Frames to wait before the reveal starts.
    pub delay: usize,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Text fill.
    pub color: String,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
}

impl Default for TextSwellConfig {
    fn default() -> Self {
        Self {
            text: "Big Ideas Move Fast".to_owned(),
            rise_distance_em: 0.7,
            rise_duration: 10,
            front_scale: 2.1,
            approach_delay: 14,
            approach_duration: 20,
            word_delay: 27,
            word_stagger: 14,
            word_push_em: 0.15,
            word_push_duration: 12,
            bounce_words: 1,
            letter_swell: 0.23,
            letter_stagger: 2,
            letter_rise: 3,
            letter_hold: 0,
            letter_fall: 6,
            hold_duration: 6,
            recede_duration: 18,
            delay: 0,
            font_size: 72.0,
            font_weight: 600,
            color: "#e6edf3".to_owned(),
            tracking_em: -0.03,
        }
    }
}

impl TextSwellConfig {
    /// The duration-relevant subset of this configuration.
    pub fn timing(&self) -> TextSwellTiming {
        TextSwellTiming {
            approach_delay: self.approach_delay,
            approach_duration: self.approach_duration,
            word_delay: self.word_delay,
            word_stagger: self.word_stagger,
            word_push_duration: self.word_push_duration,
            hold_duration: self.hold_duration,
            recede_duration: self.recede_duration,
        }
    }
}

/// The Text Swell component.
#[derive(Debug, Clone, Default)]
pub struct TextSwell {
    /// Component options.
    pub config: TextSwellConfig,
}

impl TextSwell {
    /// Build with the given options.
    pub fn new(config: TextSwellConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-swell",
            name: "Text Swell",
            summary: "The lead word is shoved aside by the words that follow it as the line swells.",
            feature: "text-swell",
        }
    }
}

/// Distance from the vertical centre of a font's em box down to its baseline.
fn em_center_to_baseline(ctx: &RenderCtx<'_>, family: &str, size: f32) -> f32 {
    match ctx.meter(family) {
        Some(meter) => (meter.ascent(size) - meter.descent(size)) / 2.0,
        None => size * 0.25,
    }
}

impl Component for TextSwell {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_text_swell_duration(&self.config.text, &self.config.timing())
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = ctx.sans();
        let size = config.font_size;
        let frame = (ctx.frame() - config.delay as f32).max(0.0);
        let tracking_px = config.tracking_em * size;

        let source_words = words(&config.text);
        let trailing_count = source_words.len().saturating_sub(1);
        let space_width = ctx.space_width(&family, size) + tracking_px;
        let word_widths: Vec<f32> = source_words
            .iter()
            .map(|word| {
                let count = word.chars().count();
                ctx.text_width(&family, size, word) + tracking_px * count.saturating_sub(1) as f32
            })
            .collect();
        let metrics = swell_metrics(word_widths, space_width);

        // Beat 2: float forward, fit-capped.
        let scale = swell_line_scale(
            frame,
            config.approach_delay,
            config.approach_duration,
            config.front_scale,
            metrics.line_width,
            ctx.width(),
        );

        // Beat 3: per-word shove.
        let shares = swell_shares(&metrics.word_widths, metrics.space_width, trailing_count);
        let shoved = swell_shoved(
            frame,
            &shares,
            config.word_delay,
            config.word_stagger,
            config.word_push_duration,
        );
        let tx = swell_tx(scale, metrics.line_width, metrics.lead_center, shoved);

        // Beat 4: recede back to final size.
        let recede_start = swell_recede_start(
            trailing_count,
            config.approach_delay,
            config.approach_duration,
            config.word_delay,
            config.word_stagger,
            config.word_push_duration,
            config.hold_duration,
        );
        let recede_p = EASE_ZOOM_IN.ease(progress(frame - recede_start, config.recede_duration));
        let final_scale = scale - (scale - 1.0) * recede_p;

        // Beat 1: the lead rises from below.
        let rise_p = RISE_EASE.ease(progress(frame, config.rise_duration));
        let lead_y = (1.0 - rise_p) * config.rise_distance_em * size;

        let center_x = ctx.width() / 2.0;
        let baseline = ctx.height() / 2.0 + em_center_to_baseline(ctx, &family, size);

        let mut nodes: Vec<Svgr<'_>> = Vec::with_capacity(source_words.len());
        let mut cursor = 0.0_f32;
        for (index, word) in source_words.iter().enumerate() {
            let width = metrics.word_widths.get(index).copied().unwrap_or(0.0);
            let left = cursor;
            cursor += width + metrics.space_width;
            let word_center = left + width / 2.0;
            let visible = index == 0
                || frame >= word_start(index - 1, config.word_delay, config.word_stagger);
            let visibility = if visible { "visible" } else { "hidden" }.to_owned();

            if index == 0 {
                let content = text_node_centered(
                    word,
                    word_center,
                    0.0,
                    family.clone(),
                    size,
                    config.font_weight,
                    config.tracking_em,
                    config.color.clone(),
                    1.0,
                );
                let transform = format!("translate(0 {lead_y:.4})");
                nodes.push(fframes::svgr!(
                    <g transform={transform} visibility={visibility}>{content}</g>
                ));
                continue;
            }

            let j = index - 1;
            let push_p = PUSH_EASE.ease(progress(
                frame - word_start(j, config.word_delay, config.word_stagger),
                config.word_push_duration,
            ));
            let push_x = (1.0 - push_p) * config.word_push_em * size;
            let transform = format!("translate({push_x:.4} 0)");

            if j >= config.bounce_words {
                let content = text_node_centered(
                    word,
                    word_center,
                    0.0,
                    family.clone(),
                    size,
                    config.font_weight,
                    config.tracking_em,
                    config.color.clone(),
                    1.0,
                );
                nodes.push(fframes::svgr!(
                    <g transform={transform} visibility={visibility}>{content}</g>
                ));
                continue;
            }

            // Bouncing word: its letters swell up off the baseline one after another.
            let start = word_start(j, config.word_delay, config.word_stagger);
            let mut letters: Vec<Svgr<'_>> = Vec::new();
            let mut letter_left = left;
            for (c, ch) in word.chars().enumerate() {
                let char_width = ctx.text_width(&family, size, &ch.to_string());
                let char_center = letter_left + char_width / 2.0;
                letter_left += char_width + tracking_px;

                let rise_start = start + c as f32 * config.letter_stagger as f32;
                let fall_start = rise_start + config.letter_rise as f32 + config.letter_hold as f32;
                let rise_pulse = LETTER_EASE.ease(progress(frame - rise_start, config.letter_rise));
                let fall_pulse = LETTER_EASE.ease(progress(frame - fall_start, config.letter_fall));
                let letter_scale = letter_swell_scale(rise_pulse, fall_pulse, config.letter_swell);
                // The pivot is the letter's own centre on the baseline.
                let transform = format!("translate({char_center:.4} 0) scale({letter_scale:.6})");
                let glyph = fframes::svgr!(
                    <text x={0.0_f32} y={0.0_f32} font-family={family.clone()} font-size={size}
                          font-weight={config.font_weight} letter-spacing={config.tracking_em}
                          fill={config.color.clone()} text-anchor="middle">{ch.to_string()}</text>
                );
                letters.push(fframes::svgr!(<g transform={transform}>{glyph}</g>));
            }
            let letters = Svgr::from(letters);
            nodes.push(fframes::svgr!(
                <g transform={transform} visibility={visibility}>{letters}</g>
            ));
        }

        // The line: translate (centre + shove) -> scale about the line centre + baseline ->
        // line-local coordinates whose origin is the line's left edge on the baseline.
        let line_transform = format!(
            "translate({:.4} {:.4}) scale({final_scale:.6}) translate({:.4} 0)",
            center_x + tx,
            baseline,
            -metrics.line_width / 2.0,
        );
        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g transform={line_transform}>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn named_curves_are_the_source_beziers() {
        assert_eq!(RISE_EASE, CubicBezier::new(0.2, 0.6, 0.35, 1.0));
        assert_eq!(APPROACH_EASE, CubicBezier::new(0.4, 0.0, 0.15, 1.0));
        assert_eq!(LETTER_EASE, CubicBezier::new(0.4, 0.0, 0.2, 1.0));
        assert_eq!(PUSH_EASE, CubicBezier::new(0.22, 0.8, 0.36, 1.0));
        assert!(close(FIT, 0.97, 0.0));
        // Endpoints, the property the source relies on to land exactly.
        for curve in [RISE_EASE, APPROACH_EASE, LETTER_EASE, PUSH_EASE] {
            assert!(close(curve.ease(0.0), 0.0, 1e-4));
            assert!(close(curve.ease(1.0), 1.0, 1e-4));
        }
    }

    #[test]
    fn metrics_add_spaces_and_centre_the_lead() {
        let metrics = swell_metrics(vec![100.0, 50.0, 25.0], 10.0);
        assert!(close(
            metrics.line_width,
            100.0 + 50.0 + 25.0 + 2.0 * 10.0,
            1e-5
        ));
        assert!(close(metrics.lead_center, 50.0, 1e-5));
        assert!(close(metrics.baseline, 0.0, 0.0));

        // A single word has no space between anything.
        let single = swell_metrics(vec![80.0], 10.0);
        assert!(close(single.line_width, 80.0, 1e-5));
    }

    #[test]
    fn shares_are_a_partition_of_the_added_width() {
        let shares = swell_shares(&[100.0, 50.0, 30.0], 10.0, 2);
        // added = [60, 40], total = 100.
        assert_eq!(shares.len(), 2);
        assert!(close(shares[0], 0.6, 1e-5));
        assert!(close(shares[1], 0.4, 1e-5));
        assert!(close(shares.iter().sum::<f32>(), 1.0, 1e-5));
        // No trailing word, no shove.
        assert!(swell_shares(&[100.0], 10.0, 0).is_empty());
        // A degenerate all-zero line gets zero shares, never NaN.
        let zeros = swell_shares(&[0.0, 0.0], 0.0, 1);
        assert!(close(zeros[0], 0.0, 0.0));
    }

    #[test]
    fn shoved_accumulates_each_landed_word() {
        let shares = [0.6, 0.4];
        // Before anything starts.
        assert!(close(swell_shoved(0.0, &shares, 27, 14, 12), 0.0, 1e-5));
        // The first word has fully landed by frame 27 + 12.
        assert!(close(swell_shoved(39.0, &shares, 27, 14, 12), 0.6, 1e-4));
        // Both landed.
        assert!(close(swell_shoved(53.0, &shares, 27, 14, 12), 1.0, 1e-4));
        // Halfway through the second push, PUSH_EASE(0.5) ~ 0.75.
        let mid = swell_shoved(47.0, &shares, 27, 14, 12);
        assert!(mid > 0.6 && mid < 1.0, "mid was {mid}");
    }

    #[test]
    fn tx_keeps_the_lead_centred_while_it_is_alone() {
        // Lead alone, no shove: tx = scale * (line_width / 2 - lead_center).
        let tx = swell_tx(2.0, 300.0, 50.0, 0.0);
        assert!(close(tx, 2.0 * (150.0 - 50.0), 1e-5));
        // Fully shoved: the line sits at its rest position.
        assert!(close(swell_tx(2.0, 300.0, 50.0, 1.0), 0.0, 1e-5));
        // A single-word line has lead_center == line_width / 2, so it never moves.
        assert!(close(swell_tx(2.0, 100.0, 50.0, 0.0), 0.0, 1e-5));
    }

    #[test]
    fn line_scale_floats_forward_and_is_fit_capped() {
        // 1 before the approach starts.
        assert!(close(
            swell_line_scale(0.0, 14, 20, 2.1, 400.0, 1920.0),
            1.0,
            1e-6
        ));
        // At the end of the approach it has reached the front scale (roomy frame).
        assert!(close(
            swell_line_scale(34.0, 14, 20, 2.1, 400.0, 1920.0),
            2.1,
            1e-4
        ));
        // A very wide line is capped: 0.97 * 1000 / 800 = 1.2125.
        assert!(close(
            swell_line_scale(34.0, 14, 20, 2.1, 800.0, 1000.0),
            FIT * 1000.0 / 800.0,
            1e-4
        ));
        // A zero-width line falls back to the front scale.
        assert!(close(swell_fit_cap(2.1, 0.0, 1920.0), 2.1, 0.0));
    }

    #[test]
    fn recede_starts_after_the_last_landing_plus_the_hold() {
        // Three trailing words: 27 + 2 * 14 + 12 = 67, plus the 6 frame hold.
        assert!(close(
            swell_recede_start(3, 14, 20, 27, 14, 12, 6),
            67.0 + 6.0,
            1e-5
        ));
        // One word: the approach end wins (34) over the last landing.
        assert!(close(
            swell_recede_start(0, 14, 20, 27, 14, 12, 6),
            34.0 + 6.0,
            1e-5
        ));
        // A trailing word that lands before the approach does not pull the recede earlier.
        assert!(close(
            swell_recede_start(1, 14, 20, 2, 1, 2, 6),
            34.0 + 6.0,
            1e-5
        ));
    }

    #[test]
    fn letter_bounce_rises_then_falls_back_to_rest() {
        assert!(close(letter_swell_scale(0.0, 0.0, 0.23), 1.0, 1e-6));
        // Fully risen: 1 + 0.23.
        assert!(close(letter_swell_scale(1.0, 0.0, 0.23), 1.23, 1e-6));
        // Fully fallen: back to rest.
        assert!(close(letter_swell_scale(1.0, 1.0, 0.23), 1.0, 1e-6));
    }

    #[test]
    fn duration_matches_the_source_formula() {
        let timing = TextSwellTiming::default();
        // "Big Ideas Move Fast" has 3 trailing words:
        // 27 + 2 * 14 + 12 = 67, max(67, 34) + 6 + 18 = 91.
        assert_eq!(get_text_swell_duration("Big Ideas Move Fast", &timing), 91);
        // One word ends when the approach ends: 34 + 6 + 18.
        assert_eq!(get_text_swell_duration("Alone", &timing), 58);
        // Two words: 27 + 0 + 12 = 39 > 34.
        assert_eq!(get_text_swell_duration("Two Words", &timing), 39 + 6 + 18);
        // Whitespace-only text has no trailing word.
        assert_eq!(get_text_swell_duration("   ", &timing), 58);
    }

    #[test]
    fn component_duration_uses_its_own_timing() {
        let component = TextSwell::default();
        assert_eq!(component.duration(), 91);
        assert_eq!(component.config.timing(), TextSwellTiming::default());
        assert_eq!(component.meta().id, "text-swell");
    }

    #[test]
    fn a_zero_frame_beat_completes_instead_of_dividing_by_zero() {
        // 0/0 would be NaN in the source; the port treats a zero-length beat as done.
        assert!(close(progress(0.0, 0), 0.0, 0.0));
        assert!(close(progress(1.0, 0), 1.0, 0.0));
        assert!(close(progress(-1.0, 0), 0.0, 0.0));
        assert!(close(progress(5.0, 10), 0.5, 1e-6));
    }

    #[test]
    fn word_start_steps_by_the_stagger() {
        assert!(close(word_start(0, 27, 14), 27.0, 1e-6));
        assert!(close(word_start(3, 27, 14), 69.0, 1e-6));
    }
}
