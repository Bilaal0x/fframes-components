//! # Text Reveal
//!
//! A cinematic four-beat lead-word reveal. The first word of the line is the **lead**: it fades
//! up huge (`initial_scale` 2.3) centred on the canvas, drifts closer still (`×push_scale`) and
//! hangs, then recedes back to its final size on a short, heavily eased-in clock. From the same
//! frame the whole line slides left on a much longer clock while the trailing words push in one
//! by one from the right — they stay hidden until `word_delay` frames into the recede, so nothing
//! appears while the lead word is still travelling.
//!
//! The scale pivots on the lead word's centre at the text baseline, so the zoom reads as centred
//! even while the line is sliding. The source component measures that pivot from the DOM
//! (`lineWidth`, `leadRatio = leadCenter / lineWidth`, `baseline`); this port measures the line
//! with [`RenderCtx::text_width`] instead, which gives the same geometry without a DOM.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Make Something People Love"` | One line of text; the FIRST word is the lead word. |
//! | `intro_duration` | `usize` | `6` | Beat 1: frames to fade the lead word in at `initial_scale`. |
//! | `hold_duration` | `usize` | `12` | Beat 2: frames drifting from `initial_scale` to `initial_scale * push_scale`. |
//! | `recede_duration` | `usize` | `14` | Beat 3: frames receding from full scale back to 1. |
//! | `assemble_duration` | `usize` | `30` | Beat 4: frames for the whole line to slide left into rest. |
//! | `word_delay` | `usize` | `7` | Trailing words stay hidden this many frames into the recede. |
//! | `word_duration` | `usize` | `14` | Frames each trailing word takes to push into its slot. |
//! | `word_stagger` | `usize` | `4` | Frames between trailing word starts. |
//! | `word_fade` | `usize` | `2` | Frames each trailing word's fade takes — deliberately tiny: they are PUSHED, not faded. |
//! | `word_push_em` | `f32` | `0.5` | How far right of its slot each trailing word starts, in `em`. |
//! | `initial_scale` | `f32` | `2.3` | Scale the lead word fades in at. |
//! | `push_scale` | `f32` | `1.06` | Extra scale the lead word drifts to before receding. |
//! | `delay` | `usize` | `0` | Frames to wait before the reveal starts. |
//! | `font_size` | `f32` | `72.0` | Font size in px. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components text-reveal
//! ```
//!
//! ## Credit
//!
//! Ported from `TextReveal.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{LayoutCache, RenderCtx, text_node_centered, words};
use crate::motion::{CubicBezier, EASE_ZOOM_IN, clamp01, interpolate};

/// Beat 2's drift curve — the source's `PUSH_EASE`, `cubic-bezier(0.25, 1, 0.5, 1)`.
const PUSH_EASE: CubicBezier = CubicBezier::new(0.25, 1.0, 0.5, 1.0);
/// Beat 4's push curve — the source's `WORD_EASE`, `cubic-bezier(0.22, 0.8, 0.36, 1)`.
const WORD_EASE: CubicBezier = CubicBezier::new(0.22, 0.8, 0.36, 1.0);

/// The backdrop the source demo paints behind this component.
const BACKDROP: &str = "#0d1117";

/// CSS `line-height: normal` for a family, i.e. the height of the line box SVG's baseline is
/// measured from. The source centres a `white-space: nowrap` line whose box height comes from the
/// face's own metrics, so this port reads the same metrics instead of assuming a ratio.
fn natural_line_height(ctx: &RenderCtx<'_>, family: &str, size: f32) -> f32 {
    ctx.meter(family)
        .map_or(size * 1.2, |meter| meter.line_height(size))
}

/// The frame counts that shape [`get_text_reveal_duration`].
///
/// Mirrors the source's `opts` object: every field falls back to the same default the `.tsx`
/// uses, so `TextRevealDurations::default()` reproduces the source timeline exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRevealDurations {
    /// Beat 1: frames to fade the lead word in at `initial_scale`.
    pub intro_duration: usize,
    /// Beat 2: frames drifting from `initial_scale` to `initial_scale * push_scale`.
    pub hold_duration: usize,
    /// Beat 3: frames receding from full scale back to 1.
    pub recede_duration: usize,
    /// Beat 4: frames for the whole line to slide left into rest.
    pub assemble_duration: usize,
    /// Trailing words stay hidden this many frames into the recede.
    pub word_delay: usize,
    /// Frames each trailing word takes to push into its slot.
    pub word_duration: usize,
    /// Frames between trailing word starts.
    pub word_stagger: usize,
}

impl Default for TextRevealDurations {
    fn default() -> Self {
        Self {
            intro_duration: 6,
            hold_duration: 12,
            recede_duration: 14,
            assemble_duration: 30,
            word_delay: 7,
            word_duration: 14,
            word_stagger: 4,
        }
    }
}

impl TextRevealDurations {
    /// The frame the recede (beat 3) and the slide (beat 4) both start on: `intro + hold`.
    pub fn recede_start(&self) -> usize {
        self.intro_duration.saturating_add(self.hold_duration)
    }
}

/// The frame the trailing word with the given 0-based trailing index starts pushing in.
///
/// The source places word `j` at `recedeStart + wordDelay + j * wordStagger`.
pub fn word_start(durations: &TextRevealDurations, trailing_index: usize) -> usize {
    durations
        .recede_start()
        .saturating_add(durations.word_delay)
        .saturating_add(trailing_index.saturating_mul(durations.word_stagger))
}

/// Total frames until the last trailing word settles.
///
/// Ported verbatim from `getTextRevealDuration`: `recedeStart + max(assemble, wordsEnd, recede)`,
/// where `wordsEnd` is the last trailing word's settle frame.
pub fn get_text_reveal_duration(text: &str, durations: &TextRevealDurations) -> usize {
    let trailing = words(text).len().saturating_sub(1);
    let recede_start = durations.recede_start();
    let words_end = if trailing == 0 {
        0
    } else {
        durations
            .word_delay
            .saturating_add((trailing - 1).saturating_mul(durations.word_stagger))
            .saturating_add(durations.word_duration)
    };
    recede_start.saturating_add(
        durations
            .assemble_duration
            .max(words_end)
            .max(durations.recede_duration),
    )
}

/// The lead word's centre as a fraction of the line width.
///
/// The source reads this out of the DOM as `lead.offsetLeft + lead.offsetWidth / 2`, divided by
/// `el.offsetWidth`. Measured, the lead word's centre sits `lead_width / 2` to the right of the
/// line's left edge, so the ratio is `(lead_width / 2) / line_width`.
pub fn lead_ratio(lead_width: f32, line_width: f32) -> f32 {
    if line_width > 0.0 {
        (lead_width / 2.0) / line_width
    } else {
        0.5
    }
}

/// How far the line starts slid so that the lead word's centre lands on the canvas centre.
///
/// The source computes `lineWidth * (0.5 - leadRatio)`; it is exactly the negative of the lead
/// word's centre offset from the line centre. The line slides back to `0` over beat 4.
pub fn slide_distance(line_width: f32, lead_ratio: f32) -> f32 {
    line_width * (0.5 - lead_ratio)
}

/// The line's scale at `frame`, across beats 1–3.
///
/// - `frame < intro_duration` — pinned at `initial_scale`.
/// - `frame < recede_start` — `PUSH_EASE` from `initial_scale` to `initial_scale * push_scale`.
/// - otherwise — `EASE_ZOOM_IN` from that peak back to `1`.
pub fn reveal_scale(
    frame: f32,
    durations: &TextRevealDurations,
    initial_scale: f32,
    push_scale: f32,
) -> f32 {
    let intro = durations.intro_duration.max(1) as f32;
    let recede_start = durations.recede_start() as f32;
    let full_scale = initial_scale * push_scale;
    if frame < intro {
        initial_scale
    } else if frame < recede_start {
        initial_scale
            + (full_scale - initial_scale)
                * PUSH_EASE.ease(clamp01(
                    (frame - intro) / durations.hold_duration.max(1) as f32,
                ))
    } else {
        full_scale
            + (1.0 - full_scale)
                * EASE_ZOOM_IN.ease(clamp01(
                    (frame - recede_start) / durations.recede_duration.max(1) as f32,
                ))
    }
}

/// Beat 4: how far the line is still slid from its resting place at `frame`.
///
/// Runs on the same start frame as the recede but on the much longer `assemble_duration` clock,
/// so the slide is still travelling after the lead word has landed.
pub fn slide_offset(frame: f32, durations: &TextRevealDurations, slide_distance: f32) -> f32 {
    let recede_start = durations.recede_start() as f32;
    let progress = EASE_ZOOM_IN.ease(clamp01(
        (frame - recede_start) / durations.assemble_duration.max(1) as f32,
    ));
    slide_distance * (1.0 - progress)
}

/// Beat 1: the lead word's fade, on the same eased clock as its scale change.
///
/// The source deliberately runs the fade on `EASE_ZOOM_IN` rather than a linear ramp, so opacity
/// and scale read as one gesture.
pub fn lead_opacity(frame: f32, durations: &TextRevealDurations) -> f32 {
    EASE_ZOOM_IN.ease(clamp01(frame / durations.intro_duration.max(1) as f32))
}

/// Beat 4: the push progress of the trailing word with the given 0-based trailing index.
///
/// Word `j` starts at [`word_start`].
pub fn word_push(frame: f32, durations: &TextRevealDurations, trailing_index: usize) -> f32 {
    let start = word_start(durations, trailing_index);
    WORD_EASE.ease(clamp01(
        (frame - start as f32) / durations.word_duration.max(1) as f32,
    ))
}

/// How far right of its slot a trailing word still is, in px.
pub fn word_push_offset(
    frame: f32,
    durations: &TextRevealDurations,
    trailing_index: usize,
    word_push_em: f32,
    font_size: f32,
) -> f32 {
    (1.0 - word_push(frame, durations, trailing_index)) * word_push_em * font_size
}

/// Beat 4: a trailing word's opacity — a deliberately tiny `word_fade`-frame ramp.
///
/// `word_fade` is a prop rather than a duration knob because the source's
/// `getTextRevealDuration` does not take it.
pub fn word_opacity(
    frame: f32,
    durations: &TextRevealDurations,
    trailing_index: usize,
    word_fade: usize,
) -> f32 {
    let start = word_start(durations, trailing_index) as f32;
    interpolate(frame, [start, start + word_fade.max(1) as f32], [0.0, 1.0])
}

/// Configuration for [`TextReveal`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextRevealConfig {
    /// One line of text; the FIRST word is the lead word.
    pub text: String,
    /// Beat 1: frames to fade the lead word in at `initial_scale`.
    pub intro_duration: usize,
    /// Beat 2: frames drifting from `initial_scale` to `initial_scale * push_scale`.
    pub hold_duration: usize,
    /// Beat 3: frames receding from full scale back to 1.
    pub recede_duration: usize,
    /// Beat 4: frames for the whole line to slide left into rest.
    pub assemble_duration: usize,
    /// Trailing words stay hidden this many frames into the recede.
    pub word_delay: usize,
    /// Frames each trailing word takes to push into its slot.
    pub word_duration: usize,
    /// Frames between trailing word starts.
    pub word_stagger: usize,
    /// Frames each trailing word's fade takes — deliberately tiny: they are PUSHED, not faded.
    pub word_fade: usize,
    /// How far right of its slot each trailing word starts, in `em`.
    pub word_push_em: f32,
    /// Scale the lead word fades in at.
    pub initial_scale: f32,
    /// Extra scale the lead word drifts to before receding.
    pub push_scale: f32,
    /// Frames to wait before the reveal starts.
    pub delay: usize,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
    /// Text fill.
    pub color: String,
}

impl Default for TextRevealConfig {
    fn default() -> Self {
        Self {
            text: "Make Something People Love".to_owned(),
            intro_duration: 6,
            hold_duration: 12,
            recede_duration: 14,
            assemble_duration: 30,
            word_delay: 7,
            word_duration: 14,
            word_stagger: 4,
            word_fade: 2,
            word_push_em: 0.5,
            initial_scale: 2.3,
            push_scale: 1.06,
            delay: 0,
            font_size: 72.0,
            font_weight: 600,
            tracking_em: -0.03,
            color: "#e6edf3".to_owned(),
        }
    }
}

impl TextRevealConfig {
    /// The duration knobs as the pure helpers take them.
    pub fn durations(&self) -> TextRevealDurations {
        TextRevealDurations {
            intro_duration: self.intro_duration,
            hold_duration: self.hold_duration,
            recede_duration: self.recede_duration,
            assemble_duration: self.assemble_duration,
            word_delay: self.word_delay,
            word_duration: self.word_duration,
            word_stagger: self.word_stagger,
        }
    }
}

/// The Text Reveal component.
#[derive(Debug, Clone, Default)]
pub struct TextReveal {
    /// Component options.
    pub config: TextRevealConfig,
    layout: LayoutCache,
}

impl TextReveal {
    /// Build with the given options.
    pub fn new(config: TextRevealConfig) -> Self {
        Self {
            config,
            layout: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-reveal",
            name: "Text Reveal",
            summary: "A lead word zooms in and recedes while the line slides into place.",
            feature: "text-reveal",
        }
    }
}

impl Component for TextReveal {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_text_reveal_duration(&self.config.text, &self.config.durations())
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKDROP)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let durations = config.durations();
        let family = ctx.sans();
        // The source sets `white-space: nowrap`, so the line never wraps: one line, however long.
        let line_step = natural_line_height(ctx, &family, config.font_size);
        let layout = self.layout.resolve(
            ctx,
            &family,
            &config.text,
            config.font_size,
            f32::INFINITY,
            line_step,
        );

        let Some(line) = layout.lines.first() else {
            return Svgr::empty();
        };
        let Some(lead) = line.words.first() else {
            return Svgr::empty();
        };

        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let baseline = layout.baseline_y(0, center_y);
        let ratio = lead_ratio(lead.width, line.width);
        let distance = slide_distance(line.width, ratio);
        // `transform-origin: {leadRatio * 100}% {baseline}px` — the lead word's centre, on the
        // baseline. SVG transform lists are applied left to right about the origin point, so the
        // slide composes with a scale taken about that pivot.
        let pivot_x = line.left(center_x) + lead.width / 2.0;

        let frame = (ctx.frame() - config.delay as f32).max(0.0);
        let scale = reveal_scale(frame, &durations, config.initial_scale, config.push_scale);
        let slide_x = slide_offset(frame, &durations, distance);
        let opacity = lead_opacity(frame, &durations);

        let transform = format!(
            "translate({:.4} {:.4}) scale({:.6}) translate({:.4} {:.4})",
            pivot_x + slide_x,
            baseline,
            scale,
            -pivot_x,
            -baseline
        );

        let mut nodes: Vec<Svgr<'_>> = Vec::with_capacity(line.words.len());
        for (index, word) in line.words.iter().enumerate() {
            let alpha = if index == 0 {
                opacity
            } else {
                word_opacity(frame, &durations, index - 1, config.word_fade)
            };
            let node = text_node_centered(
                &word.text,
                center_x + word.x,
                baseline,
                family.clone(),
                config.font_size,
                config.font_weight,
                config.tracking_em,
                config.color.clone(),
                alpha,
            );
            if index == 0 {
                nodes.push(node);
            } else {
                let push = format!(
                    "translate({:.4} 0)",
                    word_push_offset(
                        frame,
                        &durations,
                        index - 1,
                        config.word_push_em,
                        config.font_size,
                    )
                );
                nodes.push(fframes::svgr!(<g transform={push}>{node}</g>));
            }
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g transform={transform}>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_matches_the_ported_formula() {
        let durations = TextRevealDurations::default();
        // "Make Something People Love" — 3 trailing words.
        // recede_start = 18; words_end = 7 + 2*4 + 14 = 29; max(30, 29, 14) = 30 -> 48.
        assert_eq!(
            get_text_reveal_duration("Make Something People Love", &durations),
            48
        );
        // No trailing words: wordsEnd is 0, so the assemble clock decides.
        assert_eq!(get_text_reveal_duration("Hello", &durations), 48);
        assert_eq!(get_text_reveal_duration("", &durations), 48);
    }

    #[test]
    fn duration_is_driven_by_the_trailing_words_when_they_outlast_the_slide() {
        let durations = TextRevealDurations::default();
        let long = "a b c d e f g h i j k l";
        // 11 trailing words: words_end = 7 + 10*4 + 14 = 61 > assemble 30 -> 18 + 61 = 79.
        assert_eq!(get_text_reveal_duration(long, &durations), 79);
    }

    #[test]
    fn lead_ratio_is_the_lead_centre_over_the_line_width() {
        assert!((lead_ratio(100.0, 400.0) - 0.125).abs() < 1e-6);
        // A degenerate zero-width line falls back to the source's 0.5.
        assert!((lead_ratio(100.0, 0.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn slide_distance_puts_the_lead_centre_on_the_canvas_centre() {
        // A 400px line whose lead word is 100px wide: left = -200, lead centre = -150.
        let ratio = lead_ratio(100.0, 400.0);
        let distance = slide_distance(400.0, ratio);
        assert!((distance - 150.0).abs() < 1e-5, "got {distance}");
        // The lead centre offset from the line centre is -150, so slideDistance == -offset.
        assert!((distance + (-150.0)).abs() < 1e-5);
    }

    #[test]
    fn reveal_scale_walks_the_three_beats() {
        let durations = TextRevealDurations::default();
        // Beat 1 holds at initial_scale.
        assert!((reveal_scale(0.0, &durations, 2.3, 1.06) - 2.3).abs() < 1e-5);
        assert!((reveal_scale(6.0, &durations, 2.3, 1.06) - 2.3).abs() < 1e-5);
        // Beat 2 drifts upward from initial_scale.
        let mid = reveal_scale(12.0, &durations, 2.3, 1.06);
        assert!(mid > 2.3, "expected drift upward, got {mid}");
        // Beat 3 lands exactly at 1.
        let landed = reveal_scale(18.0 + 14.0, &durations, 2.3, 1.06);
        assert!((landed - 1.0).abs() < 1e-5, "got {landed}");
        assert!((reveal_scale(1000.0, &durations, 2.3, 1.06) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn slide_runs_on_the_longer_clock_and_still_travels_after_the_recede() {
        let durations = TextRevealDurations::default();
        assert!((slide_offset(18.0, &durations, 150.0) - 150.0).abs() < 1e-5);
        // 18 + 14: the recede has landed but the slide is still moving.
        let partial = slide_offset(32.0, &durations, 150.0);
        assert!(partial > 0.0 && partial < 150.0, "got {partial}");
        assert!((slide_offset(48.0, &durations, 150.0) - 0.0).abs() < 1e-4);
    }

    #[test]
    fn lead_opacity_fades_on_the_intro_clock() {
        let durations = TextRevealDurations::default();
        assert!((lead_opacity(0.0, &durations) - 0.0).abs() < 1e-6);
        assert!((lead_opacity(6.0, &durations) - 1.0).abs() < 1e-4);
        assert!(lead_opacity(3.0, &durations) > 0.5);
    }

    #[test]
    fn trailing_words_stagger_and_push_from_the_right() {
        let durations = TextRevealDurations::default();
        // Word 0 starts at 18 + 7 = 25; word 1 four frames later.
        assert!((word_push(25.0, &durations, 0) - 0.0).abs() < 1e-6);
        assert!(word_push(25.0, &durations, 1) <= word_push(25.0, &durations, 0));
        assert!((word_push(1000.0, &durations, 1) - 1.0).abs() < 1e-5);
        // At the start the word is a full push to the right; it settles at 0.
        let start = word_push_offset(25.0, &durations, 0, 0.5, 72.0);
        assert!((start - 36.0).abs() < 1e-4, "got {start}");
        assert!((word_push_offset(1000.0, &durations, 0, 0.5, 72.0)).abs() < 1e-4);
    }

    #[test]
    fn trailing_word_opacity_is_a_tiny_clamped_ramp() {
        let durations = TextRevealDurations::default();
        assert!((word_opacity(25.0, &durations, 0, 2) - 0.0).abs() < 1e-6);
        assert!((word_opacity(26.0, &durations, 0, 2) - 0.5).abs() < 1e-5);
        assert!((word_opacity(100.0, &durations, 0, 2) - 1.0).abs() < 1e-5);
        // A zero-length fade still has to be a ramp, not a division by zero.
        assert!(word_opacity(26.0, &durations, 0, 0).is_finite());
        assert_eq!(word_start(&durations, 0), 25);
        assert_eq!(word_start(&durations, 1), 29);
    }

    #[test]
    fn degenerate_zero_length_beats_do_not_produce_nan() {
        let durations = TextRevealDurations {
            intro_duration: 0,
            hold_duration: 0,
            recede_duration: 0,
            assemble_duration: 0,
            word_duration: 0,
            ..TextRevealDurations::default()
        };
        for frame in [0.0, 1.0, 10.0, 100.0] {
            assert!(reveal_scale(frame, &durations, 2.3, 1.06).is_finite());
            assert!(slide_offset(frame, &durations, 150.0).is_finite());
            assert!(lead_opacity(frame, &durations).is_finite());
            assert!(word_opacity(frame, &durations, 0, 0).is_finite());
            assert!(word_push(frame, &durations, 0).is_finite());
        }
    }

    #[test]
    fn component_duration_and_meta_match_the_config() {
        let component = TextReveal::default();
        assert_eq!(component.duration(), 48);
        assert_eq!(component.meta().id, "text-reveal");
        assert_eq!(component.background(), Some("#0d1117"));

        let long = TextReveal::new(TextRevealConfig {
            text: "one two three four five six seven eight nine ten eleven twelve".to_owned(),
            ..TextRevealConfig::default()
        });
        assert_eq!(long.duration(), 79);
    }

    /// Wrap a component body in the same `<svg>` root [`crate::core::ComponentsVideo`] renders.
    fn document(body: Svgr<'_>) -> Svgr<'_> {
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920"
                 height="1080">
                <rect x="0" y="0" width="1920" height="1080" fill="#000000" />
                {body}
            </svg>
        )
    }

    /// Every frame of the default configuration must build a tree the renderer can convert.
    ///
    /// This is the only test that exercises `show` itself: it catches NaN transforms, malformed
    /// attribute values and structural mistakes the pure-formula tests cannot see.
    #[test]
    fn every_default_frame_converts_to_a_renderable_tree() {
        let component = TextReveal::default();
        let meters = crate::embedded_meters();
        let duration = component.duration();
        let mut saw_transform = false;
        for local in 0..duration {
            let mut frame = fframes::Frame::new(local, local, 30);
            let info = crate::ctx::ComponentFrame {
                local: local as f32,
                global: local,
                fps: 30.0,
                duration,
                width: 1920.0,
                height: 1080.0,
            };
            let mut ctx = RenderCtx::new(&mut frame, crate::core::Fonts::default(), &meters, info);
            let svg = document(component.show(&mut ctx));
            let dump = format!("{svg}");
            saw_transform |= dump.contains("translate(");
            assert!(!dump.contains("NaN"), "frame {local} emitted a NaN");
            let mut cache = fframes::usvgr::Cache::default();
            let converted = svg.into_svg_tree(
                &fframes::usvgr::Options::default(),
                &mut cache,
                &fframes::usvgr::fontdb::Database::new(),
            );
            assert!(
                converted.is_ok(),
                "frame {local} did not convert: {:?}",
                converted.err()
            );
        }
        // The dump really does carry the interpolated attribute values, so the NaN check above
        // cannot pass vacuously.
        assert!(saw_transform, "no transform was emitted on any frame");
    }
}
