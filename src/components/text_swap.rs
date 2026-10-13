//! # Text Swap
//!
//! Replaces one line of text with another using exit-then-enter scheduling: the old line's
//! segments travel out (staggered for the word-unit presets), and the new line starts entering at
//! `exit_duration + (segments - 1) * exit_stagger - overlap + micro_delay`, so the two cross over
//! rather than cutting.
//!
//! Five presets, ported verbatim from the source's `PRESETS` table:
//!
//! | preset | unit | reads as |
//! | --- | --- | --- |
//! | `fly-through` | block | perspective rush out (with an 18-sample shutter) and a small drop in |
//! | `fade-through` | block | a short, moderate fade out and a slightly longer drop-in |
//! | `crossfade` | word | every word swaps on its own stagger |
//! | `shared-axis-y` | block | the old line rises out and the new one drops in on the same axis |
//! | `shared-axis-z` | block | the old line scales up and dissolves as the new one scales up into place |
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `from_text` | `String` | `"The Old Way of Thinking"` | The line that leaves. |
//! | `to_text` | `String` | `"The New Way of Building"` | The line that arrives. |
//! | `transition` | `SwapPreset` | `SwapPreset::FlyThrough` | Which of the five presets drives the swap. |
//! | `exit_stagger` | `usize` | `1` | Frames between exit segments (word-unit transitions). |
//! | `enter_stagger` | `usize` | `2` | Frames between enter segments (word-unit transitions). |
//! | `overlap` | `usize` | `1` | Enter starts this many frames before the exit fully ends. |
//! | `micro_delay` | `usize` | `2` | Extra frames after the computed enter start. |
//! | `speed` | `f32` | `1.0` | Scales all durations; `> 1` is faster. |
//! | `delay` | `usize` | `0` | Frames to wait before the swap starts. |
//! | `font_size` | `f32` | `72.0` | Font size in px. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components text-swap
//! ```
//!
//! ## Credit
//!
//! Ported from `TextSwap.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{
    LayoutCache, LineLayout, PlacedLine, PlacedWord, RenderCtx, text_node_centered, words,
};
use crate::motion::CubicBezier;

/// The exit's curve — the source's `EXIT_RUSH`, `cubic-bezier(1, 0.65, 0.85, 1)`.
const EXIT_RUSH: CubicBezier = CubicBezier::new(1.0, 0.65, 0.85, 1.0);
/// The enter's curve — the source's `ENTER_SOFT`, `cubic-bezier(0.2, 0.6, 0.35, 1)`.
const ENTER_SOFT: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);
/// The source's `MODERATE` — the same curve as [`ENTER_SOFT`], kept as its own token so the
/// preset table reads like the original.
const MODERATE: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);

/// How many samples the `fly-through` shutter draws instead of a motion blur.
pub const SHUTTER_SAMPLES: usize = 18;

/// The backdrop the source demo paints behind this component.
const BACKDROP: &str = "#0d1117";

/// Which layout unit a preset animates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapUnit {
    /// The whole line travels as one block.
    Block,
    /// Each word travels on its own, with the preset's stagger.
    Word,
}

/// The five transition presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SwapPreset {
    /// `fly-through` — a perspective rush out under an 18-sample shutter, then a small drop in.
    #[default]
    FlyThrough,
    /// `fade-through` — a quick moderate fade out, a slightly longer drop in.
    FadeThrough,
    /// `crossfade` — the per-word variant.
    Crossfade,
    /// `shared-axis-y` — the two lines trade vertical space.
    SharedAxisY,
    /// `shared-axis-z` — the two lines trade depth.
    SharedAxisZ,
}

/// One row of the source's `PRESETS` table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSwapPreset {
    /// Whether the preset animates the whole line or each word.
    pub unit: SwapUnit,
    /// Frames the exit takes at `speed = 1`.
    pub exit_duration: f32,
    /// Frames the enter takes at `speed = 1`.
    pub enter_duration: f32,
    /// Easing of the exit.
    pub exit_ease: CubicBezier,
    /// Easing of the enter.
    pub enter_ease: CubicBezier,
    /// Vertical exit travel in px, before easing.
    pub exit_y: f32,
    /// Vertical enter travel in px, before easing.
    pub enter_y: f32,
    /// `fly-through`: the perspective `maxScale` of the rush. The others: flat exit scale.
    pub exit_scale: f32,
    /// The scale the enter starts at and settles from.
    pub enter_scale: f32,
    /// Peak exit blur in px. Carried from the source, but not rendered — SVG has no blur here.
    pub exit_blur: f32,
    /// Peak enter blur in px. Carried from the source, but not rendered — SVG has no blur here.
    pub enter_blur: f32,
    /// Fraction of the exit after which the opacity starts falling.
    pub exit_fade_start: f32,
    /// Whether the exit draws the additive 18-sample shutter.
    pub shutter: bool,
}

/// The source's `PRESETS` table, verbatim and in source order.
pub const PRESETS: [TextSwapPreset; 5] = [
    // fly-through
    TextSwapPreset {
        unit: SwapUnit::Block,
        exit_duration: 20.0,
        enter_duration: 16.0,
        exit_ease: EXIT_RUSH,
        enter_ease: ENTER_SOFT,
        exit_y: 0.0,
        enter_y: 0.0,
        exit_scale: 12.0,
        enter_scale: 0.82,
        exit_blur: 0.5,
        enter_blur: 9.0,
        exit_fade_start: 0.72,
        shutter: true,
    },
    // fade-through
    TextSwapPreset {
        unit: SwapUnit::Block,
        exit_duration: 8.0,
        enter_duration: 13.0,
        exit_ease: MODERATE,
        enter_ease: MODERATE,
        exit_y: -4.0,
        enter_y: 6.0,
        exit_scale: 1.0,
        enter_scale: 0.99,
        exit_blur: 0.0,
        enter_blur: 2.0,
        exit_fade_start: 0.0,
        shutter: false,
    },
    // crossfade
    TextSwapPreset {
        unit: SwapUnit::Word,
        exit_duration: 15.0,
        enter_duration: 21.0,
        exit_ease: MODERATE,
        enter_ease: MODERATE,
        exit_y: -6.0,
        enter_y: 8.0,
        exit_scale: 1.0,
        enter_scale: 1.0,
        exit_blur: 0.0,
        enter_blur: 0.0,
        exit_fade_start: 0.0,
        shutter: false,
    },
    // shared-axis-y
    TextSwapPreset {
        unit: SwapUnit::Block,
        exit_duration: 10.0,
        enter_duration: 14.0,
        exit_ease: MODERATE,
        enter_ease: MODERATE,
        exit_y: -24.0,
        enter_y: 24.0,
        exit_scale: 1.0,
        enter_scale: 1.0,
        exit_blur: 0.0,
        enter_blur: 0.0,
        exit_fade_start: 0.0,
        shutter: false,
    },
    // shared-axis-z
    TextSwapPreset {
        unit: SwapUnit::Block,
        exit_duration: 11.0,
        enter_duration: 16.0,
        exit_ease: MODERATE,
        enter_ease: MODERATE,
        exit_y: 0.0,
        enter_y: 0.0,
        exit_scale: 1.06,
        enter_scale: 0.9,
        exit_blur: 1.0,
        enter_blur: 2.0,
        exit_fade_start: 0.0,
        shutter: false,
    },
];

impl SwapPreset {
    /// The row of [`PRESETS`] this preset selects.
    pub fn def(self) -> TextSwapPreset {
        match self {
            Self::FlyThrough => PRESETS[0],
            Self::FadeThrough => PRESETS[1],
            Self::Crossfade => PRESETS[2],
            Self::SharedAxisY => PRESETS[3],
            Self::SharedAxisZ => PRESETS[4],
        }
    }
}

/// The knobs [`get_text_swap_duration`] takes, mirroring the source's `opts` object.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextSwapDurationOptions {
    /// Frames between exit segments (word-unit transitions).
    pub exit_stagger: usize,
    /// Frames between enter segments (word-unit transitions).
    pub enter_stagger: usize,
    /// Enter starts this many frames before the exit fully ends.
    pub overlap: usize,
    /// Extra frames after the computed enter start.
    pub micro_delay: usize,
    /// Scales all durations; `> 1` is faster.
    pub speed: f32,
}

impl Default for TextSwapDurationOptions {
    fn default() -> Self {
        Self {
            exit_stagger: 1,
            enter_stagger: 2,
            overlap: 1,
            micro_delay: 2,
            speed: 1.0,
        }
    }
}

/// A usable speed multiplier.
///
/// The source divides by `speed` unchecked; a zero, negative or non-finite value would produce an
/// infinite duration and an unwatchable timeline, so it falls back to `1` here.
fn effective_speed(speed: f32) -> f32 {
    if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    }
}

/// The frame the enter starts on: `exitDuration + (segments - 1) * exitStagger - overlap
/// + microDelay`, floored at zero.
pub fn enter_start(preset: SwapPreset, from_text: &str, options: &TextSwapDurationOptions) -> f32 {
    let def = preset.def();
    let exit_duration = def.exit_duration / effective_speed(options.speed);
    let exit_segments = segment_count(from_text, def.unit);
    (exit_duration + (exit_segments - 1) as f32 * options.exit_stagger as f32
        - options.overlap as f32
        + options.micro_delay as f32)
        .max(0.0)
}

/// `unit === "word" ? text.trim().split(/\s+/).length : 1`, never below one segment.
pub fn segment_count(text: &str, unit: SwapUnit) -> usize {
    match unit {
        SwapUnit::Word => words(text).len().max(1),
        SwapUnit::Block => 1,
    }
}

/// Total frames until the enter fully settles.
///
/// Ported verbatim from `getTextSwapDuration`:
/// `ceil(max(exitDuration + (exitSegments - 1) * exitStagger,
/// enterStart + enterDuration + (enterSegments - 1) * enterStagger))`.
pub fn get_text_swap_duration(
    from_text: &str,
    to_text: &str,
    preset: SwapPreset,
    options: &TextSwapDurationOptions,
) -> usize {
    let def = preset.def();
    let speed = effective_speed(options.speed);
    let exit_duration = def.exit_duration / speed;
    let enter_duration = def.enter_duration / speed;
    let exit_segments = segment_count(from_text, def.unit);
    let enter_segments = segment_count(to_text, def.unit);
    let start = enter_start(preset, from_text, options);

    let exit_end = exit_duration + (exit_segments - 1) as f32 * options.exit_stagger as f32;
    let enter_end =
        start + enter_duration + (enter_segments - 1) as f32 * options.enter_stagger as f32;

    exit_end.max(enter_end).ceil().max(0.0) as usize
}

/// Perspective rush: creeps then blows up.
///
/// `1 / (1 - (1 - 1/maxScale) * clamp(travel, 0, 0.9999))`, ported verbatim. A `maxScale` of zero
/// (which the source's table never has) would divide by zero, so it falls back to `1`.
pub fn perspective_scale(travel: f32, max_scale: f32) -> f32 {
    let max_scale = if max_scale.abs() < f32::EPSILON {
        1.0
    } else {
        max_scale
    };
    1.0 / (1.0 - (1.0 - 1.0 / max_scale) * travel.clamp(0.0, 0.9999))
}

/// The exit's opacity at `local` frames into its window.
///
/// Full until `exit_fade_start * exitDuration`, then a linear fall to zero over what remains.
pub fn exit_opacity(def: &TextSwapPreset, local: f32, exit_duration: f32) -> f32 {
    let fade_from = def.exit_fade_start * exit_duration;
    if local <= fade_from {
        1.0
    } else {
        1.0 - (local - fade_from) / (exit_duration - fade_from).max(1.0)
    }
}

/// One shutter sample's opacity: `1 / SHUTTER_SAMPLES` of the same fade, measured on the sample's
/// own travel value.
pub fn shutter_sample_opacity(def: &TextSwapPreset, travel: f32) -> f32 {
    let fade = if travel <= def.exit_fade_start {
        1.0
    } else {
        1.0 - (travel - def.exit_fade_start) / (1.0 - def.exit_fade_start).max(0.001)
    };
    fade / SHUTTER_SAMPLES as f32
}

/// CSS `line-height: normal` for a family, i.e. the height of the line box the layout centres.
fn natural_line_height(ctx: &RenderCtx<'_>, family: &str, size: f32) -> f32 {
    ctx.meter(family)
        .map_or(size * 1.2, |meter| meter.line_height(size))
}

/// `translateY(y) scale(scale)` taken about the pivot `(px, py)` — the SVG equivalent of a CSS
/// transform with `transform-origin` on that point.
fn pivot_transform(px: f32, py: f32, y: f32, scale: f32) -> String {
    format!(
        "translate({:.4} {:.4}) scale({:.6}) translate({:.4} {:.4})",
        px,
        py + y,
        scale,
        -px,
        -py
    )
}

/// Configuration for [`TextSwap`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextSwapConfig {
    /// The line that leaves.
    pub from_text: String,
    /// The line that arrives.
    pub to_text: String,
    /// Which of the five presets drives the swap.
    pub transition: SwapPreset,
    /// Frames between exit segments (word-unit transitions).
    pub exit_stagger: usize,
    /// Frames between enter segments (word-unit transitions).
    pub enter_stagger: usize,
    /// Enter starts this many frames before the exit fully ends.
    pub overlap: usize,
    /// Extra frames after the computed enter start.
    pub micro_delay: usize,
    /// Scales all durations; `> 1` is faster.
    pub speed: f32,
    /// Frames to wait before the swap starts.
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

impl Default for TextSwapConfig {
    fn default() -> Self {
        Self {
            from_text: "The Old Way of Thinking".to_owned(),
            to_text: "The New Way of Building".to_owned(),
            transition: SwapPreset::FlyThrough,
            exit_stagger: 1,
            enter_stagger: 2,
            overlap: 1,
            micro_delay: 2,
            speed: 1.0,
            delay: 0,
            font_size: 72.0,
            font_weight: 600,
            tracking_em: -0.03,
            color: "#e6edf3".to_owned(),
        }
    }
}

impl TextSwapConfig {
    /// The duration knobs as the pure helpers take them.
    pub fn options(&self) -> TextSwapDurationOptions {
        TextSwapDurationOptions {
            exit_stagger: self.exit_stagger,
            enter_stagger: self.enter_stagger,
            overlap: self.overlap,
            micro_delay: self.micro_delay,
            speed: self.speed,
        }
    }
}

/// The Text Swap component.
#[derive(Debug, Clone, Default)]
pub struct TextSwap {
    /// Component options.
    pub config: TextSwapConfig,
    layout_from: LayoutCache,
    layout_to: LayoutCache,
}

impl TextSwap {
    /// Build with the given options.
    pub fn new(config: TextSwapConfig) -> Self {
        Self {
            config,
            layout_from: LayoutCache::new(),
            layout_to: LayoutCache::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-swap",
            name: "Text Swap",
            summary: "One line rushes out and another takes its place, on five presets.",
            feature: "text-swap",
        }
    }

    /// One word's `<text>` node, centred on its measured slot.
    fn word_node(
        &self,
        word: &PlacedWord,
        center_x: f32,
        baseline: f32,
        family: &str,
    ) -> Svgr<'static> {
        text_node_centered(
            &word.text,
            center_x + word.x,
            baseline,
            family.to_owned(),
            self.config.font_size,
            self.config.font_weight,
            self.config.tracking_em,
            self.config.color.clone(),
            1.0,
        )
    }

    /// The body one segment draws, plus the x its transform pivots on.
    ///
    /// Block presets draw the whole line and pivot on the canvas centre; word presets draw a
    /// single word and pivot on that word's own box.
    fn segment_body(
        &self,
        layout: &LineLayout,
        unit: SwapUnit,
        segment: usize,
        center_x: f32,
        baseline: f32,
        family: &str,
    ) -> (f32, Svgr<'static>) {
        let Some(line) = layout.lines.first() else {
            return (center_x, Svgr::empty());
        };
        match unit {
            SwapUnit::Block => (center_x, self.line_body(line, center_x, baseline, family)),
            SwapUnit::Word => match line.words.get(segment) {
                Some(word) => (
                    center_x + word.x,
                    self.word_node(word, center_x, baseline, family),
                ),
                None => (center_x, Svgr::empty()),
            },
        }
    }

    /// A whole laid-out line as one body.
    fn line_body(
        &self,
        line: &PlacedLine,
        center_x: f32,
        baseline: f32,
        family: &str,
    ) -> Svgr<'static> {
        let nodes: Vec<Svgr<'static>> = line
            .words
            .iter()
            .map(|word| self.word_node(word, center_x, baseline, family))
            .collect();
        Svgr::from(nodes)
    }
}

impl Component for TextSwap {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_text_swap_duration(
            &self.config.from_text,
            &self.config.to_text,
            self.config.transition,
            &self.config.options(),
        )
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKDROP)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let def = config.transition.def();
        let speed = effective_speed(config.speed);
        let exit_duration = def.exit_duration / speed;
        let enter_duration = def.enter_duration / speed;
        let start = enter_start(config.transition, &config.from_text, &config.options());
        let frame = (ctx.frame() - config.delay as f32).max(0.0);

        let family = ctx.sans();
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let line_step = natural_line_height(ctx, &family, config.font_size);
        let from_layout = self.layout_from.resolve(
            ctx,
            &family,
            &config.from_text,
            config.font_size,
            f32::INFINITY,
            line_step,
        );
        let to_layout = self.layout_to.resolve(
            ctx,
            &family,
            &config.to_text,
            config.font_size,
            f32::INFINITY,
            line_step,
        );
        let from_baseline = from_layout.baseline_y(0, center_y);
        let to_baseline = to_layout.baseline_y(0, center_y);
        let exit_segments = segment_count(&config.from_text, def.unit);
        let enter_segments = segment_count(&config.to_text, def.unit);

        let mut nodes: Vec<Svgr<'_>> = Vec::new();

        // ---- exit ----
        for segment in 0..exit_segments {
            let segment_start = segment as f32 * config.exit_stagger as f32;
            let local = (frame - segment_start).clamp(0.0, exit_duration);
            let travel = def.exit_ease.ease(local / exit_duration);
            let opacity = exit_opacity(&def, local, exit_duration);
            let y = def.exit_y * travel;
            let (pivot_x, body) = self.segment_body(
                &from_layout,
                def.unit,
                segment,
                center_x,
                from_baseline,
                &family,
            );

            if def.shutter && travel > 0.0 && opacity > 0.0 {
                let travel_prev = def
                    .exit_ease
                    .ease((frame - 1.0 - segment_start).clamp(0.0, exit_duration) / exit_duration);
                for sample in 0..SHUTTER_SAMPLES {
                    let blend = sample as f32 / (SHUTTER_SAMPLES - 1) as f32;
                    let sample_travel = travel_prev + (travel - travel_prev) * blend;
                    let sample_opacity = shutter_sample_opacity(&def, sample_travel);
                    // The source draws every sample with `mix-blend-mode: plus-lighter`. SVG has
                    // no blend mode here, so each of the 18 copies is drawn at 1/18 opacity and
                    // ordinary alpha compositing stands in for the additive sum.
                    let transform = pivot_transform(
                        pivot_x,
                        center_y,
                        y,
                        perspective_scale(sample_travel, def.exit_scale),
                    );
                    let body = body.clone();
                    nodes.push(
                        fframes::svgr!(<g transform={transform} opacity={sample_opacity}>{body}</g>),
                    );
                }
                continue;
            }

            let scale = if def.shutter {
                perspective_scale(travel, def.exit_scale)
            } else {
                1.0 + (def.exit_scale - 1.0) * travel
            };
            let transform = pivot_transform(pivot_x, center_y, y, scale);
            nodes.push(fframes::svgr!(<g transform={transform} opacity={opacity}>{body}</g>));
        }

        // ---- enter ----
        for segment in 0..enter_segments {
            let segment_start = start + segment as f32 * config.enter_stagger as f32;
            let local = (frame - segment_start).clamp(0.0, enter_duration);
            let progress = def.enter_ease.ease(local / enter_duration);
            let y = def.enter_y * (1.0 - progress);
            let scale = def.enter_scale + (1.0 - def.enter_scale) * progress;
            let (pivot_x, body) = self.segment_body(
                &to_layout,
                def.unit,
                segment,
                center_x,
                to_baseline,
                &family,
            );
            let transform = pivot_transform(pivot_x, center_y, y, scale);
            nodes.push(fframes::svgr!(<g transform={transform} opacity={progress}>{body}</g>));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FROM: &str = "The Old Way of Thinking";
    const TO: &str = "The New Way of Building";

    #[test]
    fn presets_table_is_ported_verbatim() {
        assert_eq!(PRESETS.len(), 5);
        let fly = PRESETS[0];
        assert_eq!(fly.unit, SwapUnit::Block);
        assert_eq!(fly.exit_duration, 20.0);
        assert_eq!(fly.enter_duration, 16.0);
        assert_eq!(fly.enter_scale, 0.82);
        assert_eq!(fly.exit_scale, 12.0);
        assert_eq!(fly.exit_blur, 0.5);
        assert_eq!(fly.enter_blur, 9.0);
        assert_eq!(fly.exit_fade_start, 0.72);
        assert!(fly.shutter);

        let fade = PRESETS[1];
        assert_eq!(fade.exit_y, -4.0);
        assert_eq!(fade.enter_y, 6.0);
        assert_eq!(fade.enter_scale, 0.99);
        assert!(!fade.shutter);

        assert_eq!(PRESETS[2].unit, SwapUnit::Word);
        assert_eq!(PRESETS[2].exit_y, -6.0);
        assert_eq!(PRESETS[2].enter_y, 8.0);

        assert_eq!(PRESETS[3].exit_y, -24.0);
        assert_eq!(PRESETS[3].enter_y, 24.0);

        assert_eq!(PRESETS[4].exit_scale, 1.06);
        assert_eq!(PRESETS[4].enter_scale, 0.9);

        // The presets expose the table in the enum's order.
        assert_eq!(SwapPreset::default(), SwapPreset::FlyThrough);
        assert_eq!(SwapPreset::Crossfade.def(), PRESETS[2]);
    }

    #[test]
    fn duration_matches_the_ported_formula_for_every_preset() {
        let options = TextSwapDurationOptions::default();
        // fly-through: enterStart = 20 - 1 + 2 = 21; max(20, 21 + 16) = 37.
        assert_eq!(
            get_text_swap_duration(FROM, TO, SwapPreset::FlyThrough, &options),
            37
        );
        // fade-through: enterStart = 8 - 1 + 2 = 9; max(8, 9 + 13) = 22.
        assert_eq!(
            get_text_swap_duration(FROM, TO, SwapPreset::FadeThrough, &options),
            22
        );
        // crossfade (word unit, five words each): enterStart = 15 + 4 - 1 + 2 = 20;
        // max(15 + 4, 20 + 21 + 4 * 2) = 49.
        assert_eq!(
            get_text_swap_duration(FROM, TO, SwapPreset::Crossfade, &options),
            49
        );
        assert_eq!(
            get_text_swap_duration(FROM, TO, SwapPreset::SharedAxisY, &options),
            25
        );
        assert_eq!(
            get_text_swap_duration(FROM, TO, SwapPreset::SharedAxisZ, &options),
            28
        );
    }

    #[test]
    fn duration_scales_with_speed() {
        let fast = TextSwapDurationOptions {
            speed: 2.0,
            ..TextSwapDurationOptions::default()
        };
        // exit 10, enter 8, enterStart = 10 - 1 + 2 = 11 -> max(10, 19) = 19.
        assert_eq!(
            get_text_swap_duration(FROM, TO, SwapPreset::FlyThrough, &fast),
            19
        );
    }

    #[test]
    fn degenerate_speed_falls_back_to_one() {
        for speed in [0.0, -3.0, f32::NAN, f32::INFINITY] {
            let options = TextSwapDurationOptions {
                speed,
                ..TextSwapDurationOptions::default()
            };
            assert_eq!(effective_speed(speed), 1.0);
            assert_eq!(
                get_text_swap_duration(FROM, TO, SwapPreset::FlyThrough, &options),
                37
            );
        }
    }

    #[test]
    fn enter_start_is_floored_at_zero() {
        let options = TextSwapDurationOptions {
            exit_stagger: 1,
            enter_stagger: 2,
            overlap: 500,
            micro_delay: 2,
            speed: 1.0,
        };
        assert!(enter_start(SwapPreset::FlyThrough, FROM, &options).abs() < 1e-6);
    }

    #[test]
    fn segment_count_never_drops_below_one() {
        assert_eq!(segment_count("", SwapUnit::Word), 1);
        assert_eq!(segment_count("  ", SwapUnit::Word), 1);
        assert_eq!(segment_count("a b c", SwapUnit::Word), 3);
        assert_eq!(segment_count("a b c", SwapUnit::Block), 1);
        assert_eq!(segment_count("", SwapUnit::Block), 1);
    }

    #[test]
    fn perspective_scale_creeps_then_blows_up() {
        // At rest the rush is exactly 1:1.
        assert!((perspective_scale(0.0, 12.0) - 1.0).abs() < 1e-6);
        // Travel is clamped to 0.9999, so the scale tops out just under the 12x asymptote.
        let peak = perspective_scale(1.0, 12.0);
        assert!(peak > 11.0 && peak < 12.0, "got {peak}");
        // Negative travel clamps to the same 1:1 start.
        assert!((perspective_scale(-5.0, 12.0) - 1.0).abs() < 1e-6);
        // It is monotonic in travel.
        let mut previous = 0.0;
        for step in 0..=100 {
            let value = perspective_scale(step as f32 / 100.0, 12.0);
            assert!(value >= previous, "not monotonic at {step}");
            previous = value;
        }
    }

    #[test]
    fn exit_opacity_holds_then_falls() {
        let fly = PRESETS[0];
        // exitFadeStart 0.72 * 20 = 14.4.
        assert!((exit_opacity(&fly, 0.0, 20.0) - 1.0).abs() < 1e-6);
        assert!((exit_opacity(&fly, 14.4, 20.0) - 1.0).abs() < 1e-6);
        assert!(exit_opacity(&fly, 17.0, 20.0) < 1.0);
        assert!((exit_opacity(&fly, 20.0, 20.0) - 0.0).abs() < 1e-6);

        // crossfade has no fade window: it is opaque until the very end of the exit, and the
        // denominator is floored at 1 frame.
        let cross = PRESETS[2];
        assert!((exit_opacity(&cross, 0.0, 15.0) - 1.0).abs() < 1e-6);
        assert!((exit_opacity(&cross, 14.0, 15.0) - (1.0 - 14.0 / 15.0)).abs() < 1e-6);
        assert!(exit_opacity(&cross, 15.0, 15.0).abs() < 1e-6);
    }

    #[test]
    fn shutter_samples_split_the_fade_eighteen_ways() {
        let fly = PRESETS[0];
        assert!((shutter_sample_opacity(&fly, 0.0) - 1.0 / 18.0).abs() < 1e-6);
        assert!((shutter_sample_opacity(&fly, 0.72) - 1.0 / 18.0).abs() < 1e-6);
        assert!(shutter_sample_opacity(&fly, 1.0).abs() < 1e-6);
        // Eighteen full samples would sum back to the un-blurred opacity.
        let sum: f32 = (0..SHUTTER_SAMPLES)
            .map(|_| shutter_sample_opacity(&fly, 0.0))
            .sum();
        assert!((sum - 1.0).abs() < 1e-5, "got {sum}");
    }

    #[test]
    fn component_duration_and_meta_match_the_config() {
        let component = TextSwap::default();
        assert_eq!(component.duration(), 37);
        assert_eq!(component.meta().id, "text-swap");
        assert_eq!(component.background(), Some("#0d1117"));

        let cross = TextSwap::new(TextSwapConfig {
            transition: SwapPreset::Crossfade,
            ..TextSwapConfig::default()
        });
        assert_eq!(cross.duration(), 49);
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

    /// Every frame of every preset must build a tree the renderer can convert.
    ///
    /// This is the only test that exercises `show` itself: it catches NaN transforms, malformed
    /// attribute values and structural mistakes the pure-formula tests cannot see.
    #[test]
    fn every_preset_frame_converts_to_a_renderable_tree() {
        let meters = crate::embedded_meters();
        for preset in [
            SwapPreset::FlyThrough,
            SwapPreset::FadeThrough,
            SwapPreset::Crossfade,
            SwapPreset::SharedAxisY,
            SwapPreset::SharedAxisZ,
        ] {
            let component = TextSwap::new(TextSwapConfig {
                transition: preset,
                ..TextSwapConfig::default()
            });
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
                let mut ctx =
                    RenderCtx::new(&mut frame, crate::core::Fonts::default(), &meters, info);
                let svg = document(component.show(&mut ctx));
                let dump = format!("{svg}");
                saw_transform |= dump.contains("translate(");
                assert!(
                    !dump.contains("NaN"),
                    "{preset:?} frame {local} emitted a NaN"
                );
                let mut cache = fframes::usvgr::Cache::default();
                let converted = svg.into_svg_tree(
                    &fframes::usvgr::Options::default(),
                    &mut cache,
                    &fframes::usvgr::fontdb::Database::new(),
                );
                assert!(
                    converted.is_ok(),
                    "{preset:?} frame {local} did not convert: {:?}",
                    converted.err()
                );
            }
            // The dump really does carry the interpolated attribute values, so the NaN check above
            // cannot pass vacuously.
            assert!(
                saw_transform,
                "{preset:?} emitted no transform on any frame"
            );
        }
    }
}
