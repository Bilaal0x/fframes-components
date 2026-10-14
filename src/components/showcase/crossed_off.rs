//! A word in a sentence that is crossed out by a hand-drawn red X.
//!
//! Ported from Remotion Elements' `CrossedOffText`. The source sets "Please remove this" at 80 px
//! and wraps "remove" in `@remotion/rough-notation`'s `CrossedOff`, whose progress is
//! `interpolate(frame, [18, 39], [0, 1])` clamped. The annotation is 20 rough.js `line()` strokes
//! at seed 1: 10 iterations of the diagonal from the box's top-left to its bottom-right (each
//! odd one reversed), then 10 of the other diagonal. Every stroke is one wobbling cubic Bezier
//! (`disableMultiStroke` is on for single strokes), all of them drawing from one shared
//! pseudo-random sequence, as rough.js does. `renderAnnotation` gives path `i` of `n` the
//! progress `progress * n - i` clamped to `0..1`, so the strokes are drawn one after another. The
//! rough.js generator and `_line` are reproduced here, so the strokes have the same wobble as the
//! source for the same box.
//!
//! Deliberate differences from the source:
//!
//! - **Font.** The source uses Cormorant Garamond Bold, which this crate does not embed. The
//!   default is the embedded Inter Bold; set [`CrossedOffConfig::font_family`] to use another face.
//! - **Reveal.** `evolvePath` animates `stroke-dasharray`. Here each curve is instead cut where
//!   the drawn arc length equals the progress of the total (a de Casteljau split), which draws the
//!   same stroke with a butt end and needs no dashing.
//! - **Layout.** The source centres the sentence with a flex box; here the three runs are placed
//!   by measured widths, centred on the canvas. The annotation box is the struck word's line box,
//!   `line-height` 1.1 tall, which is the size of the source's inline-block tracker span.
//! - **Canvas.** The source's size is not given; the scene is authored for 1080 px of the canvas's
//!   shorter edge and scaled to the actual canvas.
//! - **Background and length.** The source draws no background and sits in a sequence of unknown
//!   length; here the page is white and the clip lasts 75 frames.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{clamp01, interpolate};
use crate::ui::text;

/// The canvas edge, in px, that the source's sizes are written for.
pub const REFERENCE_EDGE: f32 = 1080.0;
/// The page behind the text.
pub const BACKGROUND: &str = "#ffffff";
/// The font size, in source px.
pub const FONT_SIZE: f32 = 80.0;
/// The font weight.
pub const FONT_WEIGHT: u16 = 700;
/// The line height, as a multiple of the font size.
pub const LINE_HEIGHT: f32 = 1.1;
/// The text colour.
pub const TEXT_INK: &str = "#171717";
/// The strike colour.
pub const STRIKE_INK: &str = "#eb2525";
/// The strike stroke width, in source px.
pub const STRIKE_WIDTH: f32 = 6.0;
/// How many strokes each diagonal gets.
pub const ITERATIONS: usize = 10;
/// The frame at which the strike starts to draw.
pub const DRAW_START: f32 = 18.0;
/// The frame at which the strike is fully drawn.
pub const DRAW_END: f32 = 39.0;

/// rough.js' default `maxRandomnessOffset`, as rough-notation sets it.
pub const MAX_RANDOMNESS_OFFSET: f32 = 5.0;
/// The `roughness` the source passes.
pub const ROUGHNESS: f32 = 2.0;
/// rough.js' default `bowing`.
pub const BOWING: f32 = 1.0;
/// The seed rough-notation uses when none is given.
pub const SEED: i32 = 1;

/// The strike's progress at `frame`: `interpolate(frame, [18, 39], [0, 1])`, clamped.
#[must_use]
pub fn progress(frame: f32) -> f32 {
    clamp01(interpolate(frame, [DRAW_START, DRAW_END], [0.0, 1.0]))
}

/// rough.js' `Random`: a Lehmer generator on the signed 32-bit seed.
#[derive(Debug, Clone, Copy)]
pub struct RoughRandom {
    seed: i32,
}

impl RoughRandom {
    /// A generator from `seed`.
    #[must_use]
    pub fn new(seed: i32) -> Self {
        Self { seed }
    }

    /// The next value in `[0, 1)`.
    pub fn next_value(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(48271);
        ((self.seed & 0x7fff_ffff) as f32) / 2_147_483_648.0
    }
}

/// A cubic Bezier: start, two controls and end.
pub type Cubic = [(f32, f32); 4];

/// rough.js' `_line` for a single, non-overlay stroke with a leading move.
///
/// Returns the cubic Bezier from the (offset) start point. `rng` is advanced in the order rough.js
/// draws its random numbers, and is shared by every stroke of the annotation.
#[must_use]
pub fn rough_line(x1: f32, y1: f32, x2: f32, y2: f32, rng: &mut RoughRandom) -> Cubic {
    let length_sq = (x1 - x2).powi(2) + (y1 - y2).powi(2);
    let length = length_sq.sqrt();
    let gain = if length < 200.0 {
        1.0
    } else if length > 500.0 {
        0.4
    } else {
        -0.001_666_8 * length + 1.233_334
    };
    let mut offset = MAX_RANDOMNESS_OFFSET;
    if offset * offset * 100.0 > length_sq {
        offset = length / 10.0;
    }
    // `_offsetOpt(x)` is `roughness * gain * (random * 2x - x)`.
    let off = |x: f32, rng: &mut RoughRandom| ROUGHNESS * gain * (rng.next_value() * 2.0 * x - x);

    let diverge = 0.2 + rng.next_value() * 0.2;
    let mid_x = off(BOWING * MAX_RANDOMNESS_OFFSET * (y2 - y1) / 200.0, rng);
    let mid_y = off(BOWING * MAX_RANDOMNESS_OFFSET * (x1 - x2) / 200.0, rng);

    let sx = x1 + off(offset, rng);
    let sy = y1 + off(offset, rng);
    let c1x = mid_x + x1 + (x2 - x1) * diverge + off(offset, rng);
    let c1y = mid_y + y1 + (y2 - y1) * diverge + off(offset, rng);
    let c2x = mid_x + x1 + 2.0 * (x2 - x1) * diverge + off(offset, rng);
    let c2y = mid_y + y1 + 2.0 * (y2 - y1) * diverge + off(offset, rng);
    let ex = x2 + off(offset, rng);
    let ey = y2 + off(offset, rng);
    [(sx, sy), (c1x, c1y), (c2x, c2y), (ex, ey)]
}

/// rough-notation's `crossed-off` strokes for the box at (`x`, `y`) of size `w` x `h`.
///
/// Ten strokes of the top-left to bottom-right diagonal, odd ones reversed, then ten of the other
/// diagonal, odd ones reversed, all from one generator seeded with [`SEED`].
#[must_use]
pub fn crossed_off_strokes(x: f32, y: f32, w: f32, h: f32) -> Vec<Cubic> {
    let mut rng = RoughRandom::new(SEED);
    let (x2, y2) = (x + w, y + h);
    let mut strokes = Vec::with_capacity(2 * ITERATIONS);
    for i in 0..ITERATIONS {
        strokes.push(if i % 2 == 1 {
            rough_line(x2, y2, x, y, &mut rng)
        } else {
            rough_line(x, y, x2, y2, &mut rng)
        });
    }
    for i in 0..ITERATIONS {
        strokes.push(if i % 2 == 1 {
            rough_line(x, y2, x2, y, &mut rng)
        } else {
            rough_line(x2, y, x, y2, &mut rng)
        });
    }
    strokes
}

/// The progress of stroke `index` of `count` at overall `progress`.
///
/// `renderAnnotation`'s `progress * count - index`, clamped to `0..1`.
#[must_use]
pub fn stroke_progress(progress: f32, index: usize, count: usize) -> f32 {
    clamp01(progress * count as f32 - index as f32)
}

/// The point of `curve` at parameter `t`.
fn point(curve: &Cubic, t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    let mut p = (0.0, 0.0);
    for (weight, c) in w.iter().zip(curve) {
        p.0 += weight * c.0;
        p.1 += weight * c.1;
    }
    p
}

/// How many polyline steps measure a curve.
const STEPS: usize = 256;

/// The arc length of `curve`, by a fine polyline.
#[must_use]
pub fn curve_length(curve: &Cubic) -> f32 {
    let mut previous = point(curve, 0.0);
    let mut total = 0.0;
    for i in 1..=STEPS {
        let p = point(curve, i as f32 / STEPS as f32);
        total += ((p.0 - previous.0).powi(2) + (p.1 - previous.1).powi(2)).sqrt();
        previous = p;
    }
    total
}

/// The first `fraction` of `curve` by arc length, as its own cubic.
#[must_use]
pub fn partial_curve(curve: &Cubic, fraction: f32) -> Cubic {
    let fraction = clamp01(fraction);
    let target = curve_length(curve) * fraction;
    // Walk the polyline to the parameter at which the arc length reaches the target.
    let mut previous = point(curve, 0.0);
    let mut run = 0.0;
    let mut t = fraction;
    for i in 1..=STEPS {
        let p = point(curve, i as f32 / STEPS as f32);
        let seg = ((p.0 - previous.0).powi(2) + (p.1 - previous.1).powi(2)).sqrt();
        if run + seg >= target {
            let within = if seg > 0.0 { (target - run) / seg } else { 0.0 };
            t = ((i - 1) as f32 + within) / STEPS as f32;
            break;
        }
        run += seg;
        previous = p;
    }
    // de Casteljau: the left part of the split at `t`.
    let mix = |a: (f32, f32), b: (f32, f32)| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let ab = mix(curve[0], curve[1]);
    let bc = mix(curve[1], curve[2]);
    let cd = mix(curve[2], curve[3]);
    let abc = mix(ab, bc);
    let bcd = mix(bc, cd);
    let abcd = mix(abc, bcd);
    [curve[0], ab, abc, abcd]
}

/// The SVG path data of `curve`.
fn curve_d(c: &Cubic) -> String {
    format!(
        "M{} {} C{} {}, {} {}, {} {}",
        c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1, c[3].0, c[3].1
    )
}

/// The scale from the authored scene to the canvas.
#[must_use]
pub fn scene_scale(canvas_width: f32, canvas_height: f32) -> f32 {
    canvas_width.min(canvas_height) / REFERENCE_EDGE
}

/// What a [`CrossedOff`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct CrossedOffConfig {
    /// The words before the crossed one.
    pub before: String,
    /// The crossed-off word.
    pub struck: String,
    /// The words after the crossed one.
    pub after: String,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for CrossedOffConfig {
    fn default() -> Self {
        Self {
            before: "Please".to_owned(),
            struck: "remove".to_owned(),
            after: "this".to_owned(),
            font_family: None,
            duration: 75,
        }
    }
}

/// The crossed-off text component.
#[derive(Debug, Clone, Default)]
pub struct CrossedOff {
    /// What the text says.
    pub config: CrossedOffConfig,
}

impl CrossedOff {
    /// A component with `config`.
    #[must_use]
    pub fn new(config: CrossedOffConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "crossed-off",
            name: "Crossed Off",
            summary: "A word in a sentence is crossed out by a hand-drawn red X",
            feature: "crossed-off",
        }
    }
}

impl Component for CrossedOff {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKGROUND)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = config
            .font_family
            .clone()
            .unwrap_or_else(|| crate::INTER_FAMILY.to_owned());
        let scale = scene_scale(ctx.width(), ctx.height());
        let (w, h) = (ctx.width() / scale, ctx.height() / scale);

        let width_of = |s: &str| ctx.text_width_at(&family, FONT_WEIGHT, FONT_SIZE, s);
        let space = ctx.space_width_at(&family, FONT_WEIGHT, FONT_SIZE);
        let before_w = width_of(&config.before) + space;
        let struck_w = width_of(&config.struck);
        let after_w = width_of(&config.after);
        let total = before_w + struck_w + space + after_w;

        let line_h = FONT_SIZE * LINE_HEIGHT;
        let x0 = (w - total) / 2.0;
        let top = (h - line_h) / 2.0;
        let baseline = top + ctx.baseline_offset_at(&family, FONT_WEIGHT, FONT_SIZE, line_h);
        let struck_x = x0 + before_w;

        let run = |s: &str, x: f32| {
            text::text_node_left(
                s,
                x,
                baseline,
                family.clone(),
                FONT_SIZE,
                FONT_WEIGHT,
                0.0,
                TEXT_INK.to_owned(),
                1.0,
            )
        };

        // The annotation box is the struck word's line box.
        let strokes = crossed_off_strokes(struck_x, top, struck_w, line_h);
        let p = progress(ctx.frame());
        let mut drawn: Vec<Svgr<'static>> = Vec::new();
        for (i, curve) in strokes.iter().enumerate() {
            let t = stroke_progress(p, i, strokes.len());
            if t <= 0.0 {
                continue;
            }
            drawn.push(fframes::svgr!(
                <path d={curve_d(&partial_curve(curve, t))} fill="none" stroke={STRIKE_INK}
                      stroke-width={STRIKE_WIDTH} />
            ));
        }

        fframes::svgr!(
            <g transform={format!("scale({scale})")}>
                {run(&config.before, x0)}
                {run(&config.struck, struck_x)}
                {run(&config.after, struck_x + struck_w + space)}
                {Svgr::from(drawn)}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = CrossedOffConfig::default();
        assert_eq!(config.before, "Please");
        assert_eq!(config.struck, "remove");
        assert_eq!(config.after, "this");
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 75);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = CrossedOff::default();
        assert_eq!(component.meta().id, "crossed-off");
        assert_eq!(CrossedOff::meta_static().feature, "crossed-off");
        assert_eq!(component.duration(), 75);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn progress_runs_linearly_over_frames_18_to_39() {
        assert_eq!(progress(0.0), 0.0);
        assert_eq!(progress(18.0), 0.0);
        assert!((progress(28.5) - 0.5).abs() < 1e-6);
        assert_eq!(progress(39.0), 1.0);
        assert_eq!(progress(70.0), 1.0);
    }

    #[test]
    fn the_rough_generator_matches_roughjs() {
        let mut rng = RoughRandom::new(1);
        assert!((rng.next_value() - 48271.0 / 2_147_483_648.0).abs() < 1e-9);
        assert!((0.0..1.0).contains(&rng.next_value()));
    }

    #[test]
    fn there_are_twenty_strokes_on_two_diagonals() {
        let s = crossed_off_strokes(0.0, 0.0, 300.0, 88.0);
        assert_eq!(s.len(), 2 * ITERATIONS);
        // The first stroke runs top-left to bottom-right, the eleventh top-right to bottom-left.
        assert!(s[0][0].0 < 20.0 && s[0][3].0 > 280.0 && s[0][3].1 > 70.0);
        assert!(s[10][0].0 > 280.0 && s[10][3].0 < 20.0 && s[10][3].1 > 70.0);
        // Odd strokes run the other way.
        assert!(s[1][0].0 > 280.0 && s[1][3].0 < 20.0);
        assert_eq!(s, crossed_off_strokes(0.0, 0.0, 300.0, 88.0));
    }

    #[test]
    fn the_strokes_are_drawn_one_after_another() {
        assert_eq!(stroke_progress(0.0, 0, 20), 0.0);
        assert!((stroke_progress(0.025, 0, 20) - 0.5).abs() < 1e-5);
        assert_eq!(stroke_progress(0.025, 1, 20), 0.0);
        assert_eq!(stroke_progress(0.5, 3, 20), 1.0);
        assert_eq!(stroke_progress(1.0, 19, 20), 1.0);
    }

    #[test]
    fn a_partial_curve_is_that_fraction_of_the_length() {
        let c = crossed_off_strokes(0.0, 0.0, 300.0, 88.0)[0];
        let full = curve_length(&c);
        let half = curve_length(&partial_curve(&c, 0.5));
        assert!((half - full / 2.0).abs() < 1.0, "{half} vs {full}");
        let whole = partial_curve(&c, 1.0);
        assert!((whole[3].0 - c[3].0).abs() < 0.5);
    }
}
