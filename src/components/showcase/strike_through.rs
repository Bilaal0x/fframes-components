//! A hand-drawn red strike-through that draws across one word of a short sentence.
//!
//! Ported from Remotion Elements' `StrikeThroughText`, which sets "The forbidden fruit" at 80 px
//! and draws `@remotion/rough-notation`'s `StrikeThrough` over "forbidden". The annotation is
//! rough.js's `line()` at seed 1 (one wobbling cubic Bezier, no double stroke), revealed through
//! `evolvePath` progress, which is `interpolate(frame, [10, 25], [0, 1])` clamped. The rough.js
//! pseudo-random generator and `_line` are reproduced, so the stroke has the same wobble as the
//! source for the same box.
//!
//! Deliberate differences from the source:
//!
//! - **Font.** The source uses Cormorant Garamond Bold, which this crate does not embed. The
//!   default is the embedded Inter Bold; set `font_family` to use another face.
//! - **Reveal.** `evolvePath` animates `stroke-dasharray`. Here the curve is instead cut where
//!   the drawn arc length equals `progress` of the total (a de Casteljau split), which draws the
//!   same stroke with a butt end and needs no dashing.
//! - **Layout.** The source centres the sentence with a flex box; here the three runs are placed
//!   by measured widths, centred on the canvas. The annotation box is the struck word's line box,
//!   `line-height` 1.1 tall, so the stroke sits at the middle of that box.
//! - **Canvas.** The source's size is not given; the scene is authored for 1080 px of the canvas's
//!   shorter edge and scaled to the actual canvas.
//! - **Background.** The source draws none; here the page is white so the `#171717` ink reads.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{clamp01, interpolate};
use crate::ui::text;

/// The page behind the sentence: white, chosen because the source has no background.
pub const BACKGROUND: &str = "#ffffff";
/// The reference canvas edge the scene is authored for, in px.
pub const REFERENCE_EDGE: f32 = 1080.0;
/// The font size.
pub const FONT_SIZE: f32 = 80.0;
/// The font weight.
pub const FONT_WEIGHT: u16 = 700;
/// The line height, as a multiple of the size.
pub const LINE_HEIGHT: f32 = 1.1;
/// The text colour.
pub const TEXT_INK: &str = "#171717";
/// The strike colour.
pub const STRIKE_INK: &str = "#f11515";
/// The strike's stroke width.
pub const STRIKE_WIDTH: f32 = 14.0;
/// The frame at which the strike starts to draw.
pub const DRAW_START: f32 = 10.0;
/// The frame at which the strike is fully drawn.
pub const DRAW_END: f32 = 25.0;

/// rough.js' default `maxRandomnessOffset`.
pub const MAX_RANDOMNESS_OFFSET: f32 = 5.0;
/// The `roughness` rough-notation uses for single strokes.
pub const ROUGHNESS: f32 = 1.5;
/// rough.js' default `bowing`.
pub const BOWING: f32 = 1.0;
/// The seed rough-notation uses when none is given.
pub const SEED: i32 = 1;

/// The strike's progress at `frame`: `interpolate(frame, [10, 25], [0, 1])`, clamped.
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
/// Returns the cubic Bezier from the (offset) start point. The order in which the random numbers
/// are drawn matches rough.js, so the same seed gives the same wobble.
#[must_use]
pub fn rough_line(x1: f32, y1: f32, x2: f32, y2: f32, seed: i32) -> Cubic {
    let mut rng = RoughRandom::new(seed);
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
    let mid_x = off(BOWING * MAX_RANDOMNESS_OFFSET * (y2 - y1) / 200.0, &mut rng);
    let mid_y = off(BOWING * MAX_RANDOMNESS_OFFSET * (x1 - x2) / 200.0, &mut rng);

    let sx = x1 + off(offset, &mut rng);
    let sy = y1 + off(offset, &mut rng);
    let c1x = mid_x + x1 + (x2 - x1) * diverge + off(offset, &mut rng);
    let c1y = mid_y + y1 + (y2 - y1) * diverge + off(offset, &mut rng);
    let c2x = mid_x + x1 + 2.0 * (x2 - x1) * diverge + off(offset, &mut rng);
    let c2y = mid_y + y1 + 2.0 * (y2 - y1) * diverge + off(offset, &mut rng);
    let ex = x2 + off(offset, &mut rng);
    let ey = y2 + off(offset, &mut rng);
    [(sx, sy), (c1x, c1y), (c2x, c2y), (ex, ey)]
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

/// What a [`StrikeThrough`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct StrikeThroughConfig {
    /// The text before the struck word (a space follows it).
    pub before: String,
    /// The struck word.
    pub struck: String,
    /// The text after the struck word (a space precedes it).
    pub after: String,
    /// The text colour.
    pub text_color: String,
    /// The strike colour.
    pub strike_color: String,
    /// The strike's stroke width, in source px.
    pub stroke_width: f32,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for StrikeThroughConfig {
    fn default() -> Self {
        Self {
            before: "The".to_owned(),
            struck: "forbidden".to_owned(),
            after: "fruit".to_owned(),
            text_color: TEXT_INK.to_owned(),
            strike_color: STRIKE_INK.to_owned(),
            stroke_width: STRIKE_WIDTH,
            font_family: None,
            duration: 90,
        }
    }
}

/// The strike-through component.
#[derive(Debug, Clone, Default)]
pub struct StrikeThrough {
    /// What the sentence shows.
    pub config: StrikeThroughConfig,
}

impl StrikeThrough {
    /// A sentence with `config`.
    #[must_use]
    pub fn new(config: StrikeThroughConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "strike-through",
            name: "Strike Through",
            summary: "A hand-drawn red strike-through draws across one word of a sentence",
            feature: "strike-through",
        }
    }
}

impl Component for StrikeThrough {
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
        let frame = ctx.frame();
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
                config.text_color.clone(),
                1.0,
            )
        };

        // The annotation box is the struck word's line box; the stroke runs through its middle.
        let y = top + line_h / 2.0;
        let full = rough_line(struck_x, y, struck_x + struck_w, y, SEED);
        let p = progress(frame);
        let strike = if p > 0.0 {
            let d = curve_d(&partial_curve(&full, p));
            fframes::svgr!(
                <path d={d} fill="none" stroke={config.strike_color.clone()}
                      stroke-width={config.stroke_width} />
            )
        } else {
            fframes::svgr!(<g></g>)
        };

        fframes::svgr!(
            <g transform={format!("scale({scale})")}>
                {run(&config.before, x0)}
                {run(&config.struck, struck_x)}
                {run(&config.after, struck_x + struck_w + space)}
                {strike}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = StrikeThroughConfig::default();
        assert_eq!(config.struck, "forbidden");
        assert_eq!(config.text_color, "#171717");
        assert_eq!(config.strike_color, "#f11515");
        assert_eq!(config.stroke_width, 14.0);
        assert_eq!(config.font_family, None);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = StrikeThrough::default();
        assert_eq!(component.meta().id, "strike-through");
        assert_eq!(StrikeThrough::meta_static().feature, "strike-through");
        assert_eq!(component.duration(), 90);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn the_strike_draws_between_frames_ten_and_twenty_five() {
        assert_eq!(progress(0.0), 0.0);
        assert_eq!(progress(10.0), 0.0);
        assert!((progress(17.5) - 0.5).abs() < 1e-6);
        assert_eq!(progress(25.0), 1.0);
        assert_eq!(progress(80.0), 1.0);
    }

    #[test]
    fn the_rough_generator_matches_roughjs() {
        let mut rng = RoughRandom::new(1);
        assert!((rng.next_value() - 48271.0 / 2_147_483_648.0).abs() < 1e-9);
        let second = rng.next_value();
        assert!((0.0..1.0).contains(&second));
    }

    #[test]
    fn the_rough_line_stays_near_its_straight_line() {
        let c = rough_line(0.0, 50.0, 300.0, 50.0, SEED);
        assert!((c[0].0).abs() < 8.0 && (c[0].1 - 50.0).abs() < 8.0);
        assert!((c[3].0 - 300.0).abs() < 8.0 && (c[3].1 - 50.0).abs() < 8.0);
        assert_eq!(c, rough_line(0.0, 50.0, 300.0, 50.0, SEED));
    }

    #[test]
    fn a_partial_curve_is_that_fraction_of_the_length() {
        let c = rough_line(0.0, 50.0, 300.0, 50.0, SEED);
        let full = curve_length(&c);
        let half = curve_length(&partial_curve(&c, 0.5));
        assert!((half - full / 2.0).abs() < 1.0, "{half} vs {full}");
        let whole = partial_curve(&c, 1.0);
        assert!((whole[3].0 - c[3].0).abs() < 0.5);
    }
}
