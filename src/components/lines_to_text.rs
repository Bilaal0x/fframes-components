//! # Lines to Text
//!
//! A morph in three movements. Abstract white bars drift across the canvas like a data field,
//! then converge into striped blocks sized to each text line, then resolve into readable type
//! through a shared vertical clip edge: the wipe reveals glyphs on its left while consuming the
//! bar block on its right, so the lines literally become the text. The exit mirrors the whole
//! morph, landing back on the abstract line field, which makes the component loopable.
//!
//! Beat by beat, at the 30 fps defaults (`drift 12`, `converge 14`, `resolve 8`, `hold 48`,
//! `exit 18`):
//!
//! - frames 0–12: the line field drifts, bars wandering on fixed sines;
//! - frames 12–26: each bar converges, staggered 2 frames apart, into a stripe of its line;
//! - frames 22–30: the resolve wipe runs, one line every 2 frames;
//! - frames 30–78: the resolved text holds;
//! - frames 78–96: the exit runs, bars drifting back out while the text is consumed.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `lines` | `Vec<String>` | `["LINES", "TO", "TEXT"]` | Text lines, top to bottom. Rendered uppercase. |
//! | `drift_frames` | `usize` | `12` | Frames the line field drifts before converging. |
//! | `converge_duration` | `usize` | `14` | Frames the converge morph takes. |
//! | `resolve_duration` | `usize` | `8` | Frames one line's text resolve takes. |
//! | `hold_frames` | `usize` | `48` | Frames the resolved text holds before the exit. |
//! | `exit_duration` | `usize` | `18` | Frames the mirror exit back to lines takes. |
//! | `bars_per_line` | `usize` | `3` | Striped bars per text line. |
//! | `start_frame` | `usize` | `0` | Frame the morph starts at. |
//!
//! There are no styling options: the source derives everything from the canvas. Font size is
//! `round(height * 0.12)`, the line step `round(font_size * 1.16)`, the tracking `-0.02em`, the
//! bar height `max(2, round(font_size * 0.22))` and the bar gap `round(font_size * 0.05)`. Bar
//! and text fills are `#ffffff` on a `#e8e9ec` background.
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components lines-to-text -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `LinesToText.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, centred_text};
use crate::motion::{CubicBezier, clamp01, random};

/// The source's `CONVERGE_EASING`: `Easing.bezier(0.2, 0.6, 0.35, 1)`.
pub const CONVERGE_EASING: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);

/// Letter spacing in `em` applied to the resolved text, and the correction the source adds to its
/// measured line widths.
pub const LETTER_SPACING_EM: f32 = -0.02;

/// Fraction of the canvas height the resolved text is set at.
pub const FONT_SIZE_RATIO: f32 = 0.12;

/// Configuration for [`LinesToText`].
#[derive(Debug, Clone, PartialEq)]
pub struct LinesToTextConfig {
    /// Text lines, top to bottom. Rendered uppercase.
    pub lines: Vec<String>,
    /// Frames the line field drifts before converging.
    pub drift_frames: usize,
    /// Frames the converge morph takes.
    pub converge_duration: usize,
    /// Frames one line's text resolve takes.
    pub resolve_duration: usize,
    /// Frames the resolved text holds before the exit.
    pub hold_frames: usize,
    /// Frames the mirror exit back to lines takes.
    pub exit_duration: usize,
    /// Striped bars per text line.
    pub bars_per_line: usize,
    /// Frame the morph starts at.
    pub start_frame: usize,
}

impl Default for LinesToTextConfig {
    fn default() -> Self {
        Self {
            lines: vec!["LINES".to_owned(), "TO".to_owned(), "TEXT".to_owned()],
            drift_frames: 12,
            converge_duration: 14,
            resolve_duration: 8,
            hold_frames: 48,
            exit_duration: 18,
            bars_per_line: 3,
            start_frame: 0,
        }
    }
}

/// The source's `linesToTextDuration`: drift + converge + resolve + hold + exit.
pub fn lines_to_text_duration(
    drift_frames: usize,
    converge_duration: usize,
    resolve_duration: usize,
    hold_frames: usize,
    exit_duration: usize,
) -> usize {
    (drift_frames + converge_duration + resolve_duration + hold_frames + exit_duration).max(1)
}

/// Every size the morph is derived from, all off the canvas height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Font size in px (`round(height * 0.12)`).
    pub font_size: f32,
    /// Line step in px (`round(font_size * 1.16)`).
    pub line_step: f32,
    /// Letter spacing in `em`.
    pub letter_spacing: f32,
    /// Bar height in px (`max(2, round(font_size * 0.22))`).
    pub bar_height: f32,
    /// Gap between the stripes of one line, in px (`round(font_size * 0.05)`).
    pub bar_gap: f32,
}

impl Metrics {
    /// Derive the sizes from the canvas height.
    pub fn new(height: f32) -> Self {
        let font_size = (height * FONT_SIZE_RATIO).round().max(1.0);
        Self {
            font_size,
            line_step: (font_size * 1.16).round(),
            letter_spacing: LETTER_SPACING_EM,
            bar_height: (font_size * 0.22).round().max(2.0),
            bar_gap: (font_size * 0.05).round(),
        }
    }

    /// The measured width the source's `measureLineWidth` produced: the advance sum plus the
    /// per-character tracking correction `line.length * fontSize * letterSpacing`.
    pub fn line_width(&self, measured_advance: f32, chars: usize) -> f32 {
        measured_advance + chars as f32 * self.font_size * self.letter_spacing
    }
}

/// The frame the resolve starts at: `start + drift + converge - 4`.
///
/// It starts four frames before the bars finish landing, so each line transforms into glyphs on
/// arrival instead of resting as a block.
pub fn resolve_start(start_frame: usize, drift_frames: usize, converge_duration: usize) -> f32 {
    start_frame as f32 + drift_frames as f32 + converge_duration as f32 - 4.0
}

/// The frame the exit starts at: the resolve start plus the resolve and the hold.
pub fn exit_start(
    start_frame: usize,
    drift_frames: usize,
    converge_duration: usize,
    resolve_duration: usize,
    hold_frames: usize,
) -> f32 {
    resolve_start(start_frame, drift_frames, converge_duration)
        + resolve_duration as f32
        + hold_frames as f32
}

/// Progress of one channel that runs `[start, start + duration] -> [0, 1]` under
/// [`CONVERGE_EASING`], clamped at both ends.
fn eased_progress(frame: f32, start: f32, duration: usize) -> f32 {
    CONVERGE_EASING.ease(clamp01((frame - start) / duration.max(1) as f32))
}

/// The Lines to Text component.
#[derive(Debug, Default)]
pub struct LinesToText {
    /// Component options.
    pub config: LinesToTextConfig,
    /// Measured line widths, resolved once per instance.
    widths: OnceLock<Vec<f32>>,
}

impl LinesToText {
    /// Build with the given options.
    pub fn new(config: LinesToTextConfig) -> Self {
        Self {
            config,
            widths: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "lines-to-text",
            name: "Lines to Text",
            summary: "Stripe bars drift, converge onto each line, then resolve into type.",
            feature: "lines-to-text",
        }
    }

    /// The measured width of every uppercase line, at the size the canvas asks for.
    ///
    /// The source measures with canvas `measureText` and falls back to `chars * fontSize * 0.62`
    /// off-DOM. The embedded face is always available here, so the fallback is unnecessary and
    /// the same measurement is reused on every later frame.
    fn line_widths(
        &self,
        ctx: &RenderCtx<'_>,
        family: &str,
        metrics: Metrics,
        upper: &[String],
    ) -> Vec<f32> {
        if let Some(cached) = self.widths.get() {
            return cached.clone();
        }
        let measured: Vec<f32> = upper
            .iter()
            .map(|line| {
                let advance = ctx.text_width(family, metrics.font_size, line);
                metrics.line_width(advance, line.chars().count())
            })
            .collect();
        let _ = self.widths.set(measured.clone());
        measured
    }
}

impl Clone for LinesToText {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            widths: OnceLock::new(),
        }
    }
}

impl Component for LinesToText {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        lines_to_text_duration(
            self.config.drift_frames,
            self.config.converge_duration,
            self.config.resolve_duration,
            self.config.hold_frames,
            self.config.exit_duration,
        )
    }

    fn background(&self) -> Option<&'static str> {
        Some("#e8e9ec")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        if config.lines.is_empty() {
            return Svgr::empty();
        }

        let bars_per_line = config.bars_per_line.max(1);
        let width = ctx.width().max(1.0);
        let height = ctx.height();
        let metrics = Metrics::new(height);

        let family = ctx.sans();
        let upper: Vec<String> = config
            .lines
            .iter()
            .map(|line| line.to_uppercase())
            .collect();
        let widths = self.line_widths(ctx, &family, metrics, &upper);

        let line_count = upper.len();
        let total_bars = line_count * bars_per_line;
        let block_height = (line_count as f32 - 1.0) * metrics.line_step + metrics.font_size;
        let centre_y = height / 2.0;

        let resolve_at = resolve_start(
            config.start_frame,
            config.drift_frames,
            config.converge_duration,
        );
        let exit_at = exit_start(
            config.start_frame,
            config.drift_frames,
            config.converge_duration,
            config.resolve_duration,
            config.hold_frames,
        );

        let frame = ctx.frame();
        // Exit reversal: 0 before the exit, 1 at the end, mirroring the morph so the final frame
        // reproduces frame 0.
        let exit_t = eased_progress(frame, exit_at, config.exit_duration);
        let mirror = 1.0 - exit_t;

        let mut bars: Vec<Svgr<'_>> = Vec::with_capacity(total_bars);
        for gi in 0..total_bars {
            let line = gi / bars_per_line;
            let stripe = gi % bars_per_line;
            let line_width = widths.get(line).copied().unwrap_or(0.0);

            // Field state: varied left offset, length and vertical jitter.
            let field_left = (0.06 + random(&format!("ltt-x-{gi}")) * 0.17) * width;
            let field_scale = 0.22 + random(&format!("ltt-w-{gi}")) * 0.58;
            let field_y = centre_y
                + (if total_bars == 1 {
                    0.0
                } else {
                    (gi as f32 / (total_bars - 1) as f32 - 0.5) * block_height * 1.2
                })
                + (random(&format!("ltt-j-{gi}")) - 0.5) * metrics.font_size * 0.3;

            // Converged state: stacked stripes centred on the line's text box.
            let line_centre_y =
                centre_y + (line as f32 - (line_count as f32 - 1.0) / 2.0) * metrics.line_step;
            let bar_y = line_centre_y
                + (stripe as f32 - (bars_per_line as f32 - 1.0) / 2.0)
                    * (metrics.bar_height + metrics.bar_gap);
            let target_scale = line_width / width;

            // Per-bar stagger carries the converge; the exit factor mirrors it.
            let converge = eased_progress(
                frame,
                config.start_frame as f32 + config.drift_frames as f32 + gi as f32 * 2.0,
                config.converge_duration,
            ) * mirror;
            let drift = (gi as f32 * 2.13 + frame * 0.09).sin() * 9.0 * (1.0 - converge);
            let left =
                (field_left + drift) * (1.0 - converge) + ((width - line_width) / 2.0) * converge;
            let scale = field_scale * (1.0 - converge) + target_scale * converge;
            let y = field_y * (1.0 - converge) + bar_y * converge;

            // The resolve wipe consumes the block from its left edge.
            let p2 = eased_progress(
                frame,
                resolve_at + line as f32 * 2.0,
                config.resolve_duration,
            ) * mirror;
            let bar_width = width * scale;
            let visible = bar_width * (1.0 - p2);
            if visible <= 0.0 {
                continue;
            }
            let x = left + bar_width * p2;
            bars.push(fframes::svgr!(
                <rect x={x} y={y - metrics.bar_height / 2.0} width={visible}
                      height={metrics.bar_height} fill="#ffffff" />
            ));
        }

        // The text reveals through a clip whose right edge follows the same wipe.
        let mut defs: Vec<Svgr<'_>> = Vec::with_capacity(line_count);
        let mut texts: Vec<Svgr<'_>> = Vec::with_capacity(line_count);
        let baseline_offset = ctx.baseline_offset(&family, metrics.font_size, metrics.font_size);
        let weight: u16 = 800;
        for (index, line) in upper.iter().enumerate() {
            let p2 = eased_progress(
                frame,
                resolve_at + index as f32 * 2.0,
                config.resolve_duration,
            ) * mirror;
            let clip_width = width * p2;
            if clip_width <= 0.0 {
                continue;
            }
            let id = format!("lines-to-text-clip-{index}");
            defs.push(fframes::svgr!(
                <clipPath id={id.clone()}>
                    <rect x="0" y="0" width={clip_width} height={height} />
                </clipPath>
            ));

            // One line box of `line-height: 1`, centred on the canvas, tracked by -0.02em.
            let y = centre_y + (index as f32 - (line_count as f32 - 1.0) / 2.0) * metrics.line_step
                - metrics.font_size / 2.0
                + baseline_offset;
            let (content, x) =
                centred_text(line, width / 2.0, metrics.font_size, metrics.letter_spacing);
            let clip = format!("url(#{id})");
            texts.push(fframes::svgr!(
                <text x={x} y={y} font-family={family.clone()} font-size={metrics.font_size}
                      font-weight={weight} letter-spacing={metrics.letter_spacing}
                      fill="#ffffff" text-anchor="middle" clip-path={clip}>{content}</text>
            ));
        }

        let defs = Svgr::from(defs);
        let bars = Svgr::from(bars);
        let texts = Svgr::from(texts);
        fframes::svgr!(
            <g>
                <defs>{defs}</defs>
                {bars}
                {texts}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_is_the_source_sum() {
        assert_eq!(lines_to_text_duration(12, 14, 8, 48, 18), 100);
        assert_eq!(lines_to_text_duration(1, 1, 1, 1, 1), 5);
        // Never zero, so the assembled timeline always has a frame to draw.
        assert_eq!(lines_to_text_duration(0, 0, 0, 0, 0), 1);
    }

    #[test]
    fn default_component_duration_and_meta() {
        let component = LinesToText::default();
        assert_eq!(component.duration(), 100);
        assert_eq!(component.meta().id, "lines-to-text");
        assert_eq!(component.background(), Some("#e8e9ec"));
    }

    #[test]
    fn metrics_at_1080p_match_the_source() {
        let metrics = Metrics::new(1080.0);
        assert!((metrics.font_size - 130.0).abs() < 1e-6);
        assert!((metrics.line_step - 151.0).abs() < 1e-6);
        assert!((metrics.bar_height - 29.0).abs() < 1e-6);
        assert!((metrics.bar_gap - 7.0).abs() < 1e-6);
        assert!((metrics.letter_spacing + 0.02).abs() < 1e-6);
        // A tiny canvas never collapses the bar.
        let small = Metrics::new(1.0);
        assert!(small.bar_height >= 2.0);
        assert!(small.font_size >= 1.0);
    }

    #[test]
    fn line_width_applies_the_tracking_correction() {
        let metrics = Metrics::new(1080.0);
        // Five characters at -0.02em of 130px is -13px.
        assert!((metrics.line_width(500.0, 5) - 487.0).abs() < 1e-4);
        assert!((metrics.line_width(500.0, 0) - 500.0).abs() < 1e-4);
    }

    #[test]
    fn resolve_and_exit_starts_match_the_source() {
        assert!((resolve_start(0, 12, 14) - 22.0).abs() < 1e-6);
        assert!((resolve_start(30, 12, 14) - 52.0).abs() < 1e-6);
        assert!((exit_start(0, 12, 14, 8, 48) - 78.0).abs() < 1e-6);
        assert!((exit_start(30, 12, 14, 8, 48) - 108.0).abs() < 1e-6);
        // The resolve starts four frames before the converge ends, so the morph finishes four
        // frames before the source's duration helper says; those last four frames hold the
        // mirrored field state.
        let end = exit_start(0, 12, 14, 8, 48) + 18.0;
        let duration = lines_to_text_duration(12, 14, 8, 48, 18) as f32;
        assert!((end - (duration - 4.0)).abs() < 1e-6);
    }

    #[test]
    fn eased_progress_clamps_and_uses_the_converge_curve() {
        assert!((eased_progress(-100.0, 22.0, 8) - 0.0).abs() < 1e-6);
        assert!((eased_progress(22.0, 22.0, 8) - 0.0).abs() < 1e-6);
        assert!((eased_progress(30.0, 22.0, 8) - 1.0).abs() < 1e-6);
        assert!((eased_progress(1000.0, 22.0, 8) - 1.0).abs() < 1e-6);
        // The curve is front-loaded, the way a `0.2, 0.6, 0.35, 1` bezier is.
        let half = eased_progress(26.0, 22.0, 8);
        assert!(half > 0.5, "got {half}");
        // A zero duration still resolves rather than dividing by zero.
        assert!((eased_progress(22.0, 22.0, 0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn the_exit_mirrors_the_morph() {
        // `mirror` is what every converge/wipe progress is multiplied by, so at the end of the
        // exit everything is back at its field state and the text is clipped away.
        let exit_at = exit_start(0, 12, 14, 8, 48);
        let at_end = eased_progress(exit_at + 18.0, exit_at, 18);
        assert!((at_end - 1.0).abs() < 1e-6);
        assert!((1.0 - at_end).abs() < 1e-6);
    }

    #[test]
    fn empty_lines_do_not_panic() {
        let component = LinesToText::new(LinesToTextConfig {
            lines: Vec::new(),
            ..LinesToTextConfig::default()
        });
        assert_eq!(component.duration(), 100);
    }

    #[test]
    fn converged_stripes_centre_on_their_line() {
        let metrics = Metrics::new(1080.0);
        let centre_y = 1080.0 / 2.0;
        let (line_count, bars_per_line) = (3_usize, 3_usize);
        let spacing = metrics.bar_height + metrics.bar_gap;
        let line_centre_y = centre_y + (1.0 - (line_count as f32 - 1.0) / 2.0) * metrics.line_step;
        let middle = line_centre_y + (1.0 - (bars_per_line as f32 - 1.0) / 2.0) * spacing;
        let top = line_centre_y + (0.0 - (bars_per_line as f32 - 1.0) / 2.0) * spacing;
        // The middle line's middle stripe lands exactly on the canvas centre...
        assert!((line_centre_y - centre_y).abs() < 1e-4);
        assert!((middle - centre_y).abs() < 1e-4);
        // ...and the stripes are one `bar_height + bar_gap` apart.
        assert!((middle - top - spacing).abs() < 1e-4);
        // The converged bar is centred on the measured line width.
        let (width, line_width) = (1920.0_f32, 500.0_f32);
        let left = (width - line_width) / 2.0;
        assert!((left - 710.0).abs() < 1e-4);
        assert!((left + line_width - (width - left)).abs() < 1e-4);
    }
}
