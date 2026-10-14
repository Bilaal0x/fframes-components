//! A hand-drawn blue circle scribbled around one word of a headline.
//!
//! Ported from Remotion Elements' `CircleMarker`, which sets "How much circular financing is in
//! AI?" in 80 px Cormorant Garamond Bold inside an 800 px wide box and draws
//! `@remotion/rough-notation`'s `Circle` around "circular". The ring's `progress` runs 0 to 1 over
//! frames 0..43 on `Easing.bezier(0.42, 0, 0.58, 1)`, posterized to 4 frames, and its `seed` runs
//! 1 to 90 over frames 0..89, posterized the same way, so the wobble re-rolls every four frames.
//!
//! Deliberate differences from the source:
//!
//! - **Font.** Cormorant Garamond is not embedded in this crate, so the default is the embedded
//!   Inter Bold. Set [`CircleMarkerConfig::font_family`] to a family the renderer can resolve to
//!   get a serif back. Line breaks come from measuring the face that is actually used.
//! - **Rough ellipse.** [`rough_ring`] is a port of rough.js 4.6.6's `ellipse` (its Lehmer
//!   random generator, call for call, with rough-notation's options: roughness 1.8, curve fitting
//!   0.95, 9 curve steps, single stroke, two iterations on seeds `seed` and `seed + 1`), laid out
//!   as rough-notation's `box="inside"` circle. The Bezier segments it produces are flattened
//!   into a polyline here.
//! - **Progress.** The source cuts each path with `evolvePath`'s dash offset, giving every
//!   iteration an equal share of `progress`. Here the polyline is cut at the same length.
//! - **Word box.** The source measures the inline-block span in the browser; here it is the
//!   word's measured width by the full line height.
//! - **Background and scale.** The source draws nothing behind the text; here the page is a light
//!   neutral so the dark ink reads. The 800 px box is drawn at 1:1 on a 1080 px tall canvas and
//!   scaled with the canvas height, capped to 90% of the canvas width.
//! - **Duration.** The source leaves it to the host; this component runs 90 frames.

use std::f32::consts::{FRAC_PI_2, TAU};

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, lerp};
use crate::ui::text;

/// The page behind the text: a light neutral, chosen because the source has no background.
pub const BACKGROUND: &str = "#fafaf9";
/// The text box's width: `width: 800`.
pub const BOX_W: f32 = 800.0;
/// The font size: `fontSize: 80`.
pub const FONT_SIZE: f32 = 80.0;
/// The font weight: `fontWeight: 700`.
pub const FONT_WEIGHT: u16 = 700;
/// The line height, as a multiple of the size: `lineHeight: 1.1`.
pub const LINE_HEIGHT: f32 = 1.1;
/// The text colour: `color: '#171717'`.
pub const INK: &str = "#171717";
/// The canvas height the box is drawn 1:1 on.
pub const REFERENCE_HEIGHT: f32 = 1080.0;
/// The share of the canvas width the box may fill.
pub const MAX_WIDTH_SHARE: f32 = 0.9;

/// The ring's colour: `color="#2563eb"`.
pub const RING_COLOR: &str = "#2563eb";
/// The ring's stroke width: `strokeWidth={12}`.
pub const RING_WIDTH: f32 = 12.0;
/// The ring's roughness: `roughness={1.8}`.
pub const RING_ROUGHNESS: f32 = 1.8;
/// The padding between the word's box and the ring on every side: `padding` of 10.
pub const RING_PADDING: f32 = 10.0;
/// How many times the ring is traced, as rough-notation's default `iterations`.
pub const RING_PASSES: u32 = 2;
/// rough.js's `curveStepCount`.
const CURVE_STEP_COUNT: f32 = 9.0;
/// rough.js's `curveFitting`.
const CURVE_FITTING: f32 = 0.95;
/// How many samples each curve segment gets when the ring is flattened.
const SEGMENT_STEPS: usize = 14;

/// The frame at which the ring is complete: `interpolate(frame, [0, 43], [0, 1])`.
pub const PROGRESS_FRAMES: f32 = 43.0;
/// The ring's easing: `Easing.bezier(0.42, 0, 0.58, 1)`.
pub const PROGRESS_EASE: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);
/// The frame at which the seed reaches its end: `interpolate(frame, [0, 89], [1, 90])`.
pub const SEED_FRAMES: f32 = 89.0;
/// The seed's first and last value.
pub const SEED_RANGE: [f32; 2] = [1.0, 90.0];
/// The `posterize: 4` that holds both channels for four frames at a time.
pub const POSTERIZE: f32 = 4.0;

/// The ring's progress at `frame`: 0 to 1 over [`PROGRESS_FRAMES`], eased and posterized.
#[must_use]
pub fn ring_progress(frame: f32) -> f32 {
    let held = (frame / POSTERIZE).floor() * POSTERIZE;
    PROGRESS_EASE.ease((held / PROGRESS_FRAMES).clamp(0.0, 1.0))
}

/// The ring's seed at `frame`: 1 to 90 over [`SEED_FRAMES`], posterized.
#[must_use]
pub fn ring_seed(frame: f32) -> u32 {
    let held = (frame / POSTERIZE).floor() * POSTERIZE;
    lerp(
        SEED_RANGE[0],
        SEED_RANGE[1],
        (held / SEED_FRAMES).clamp(0.0, 1.0),
    )
    .round() as u32
}

/// rough.js's seeded generator: a Lehmer step, `seed = imul(48271, seed) & 0x7fffffff`.
#[derive(Debug, Clone, Copy)]
struct Rough {
    seed: i32,
}

impl Rough {
    fn new(seed: u32) -> Self {
        Self { seed: seed as i32 }
    }

    /// The next value in `0..1`.
    fn next(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(48271);
        (self.seed & 0x7fff_ffff) as f64 as f32 / 2_147_483_648.0
    }

    /// rough.js's `_offset(min, max)`: a value in `min..max`, scaled by the roughness.
    fn offset(&mut self, min: f32, max: f32) -> f32 {
        RING_ROUGHNESS * (self.next() * (max - min) + min)
    }

    /// rough.js's `_offsetOpt(x)`: a value in `-x..x`, scaled by the roughness.
    fn offset_opt(&mut self, x: f32) -> f32 {
        self.offset(-x, x)
    }
}

/// rough.js's `_computeEllipsePoints` for the single-stroke case: the points the curve runs through.
#[allow(clippy::too_many_arguments)]
fn ellipse_points(
    rng: &mut Rough,
    increment: f32,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    overlap: f32,
) -> Vec<(f32, f32)> {
    let offset = 1.0;
    let rad = rng.offset_opt(0.5) - FRAC_PI_2;
    let mut points = Vec::new();
    let point = |rng: &mut Rough, k: f32, angle: f32| {
        let x = rng.offset_opt(offset) + cx + k * rx * angle.cos();
        let y = rng.offset_opt(offset) + cy + k * ry * angle.sin();
        (x, y)
    };
    points.push(point(rng, 0.9, rad - increment));
    let end = TAU + rad - 0.01;
    let mut angle = rad;
    while angle < end {
        points.push(point(rng, 1.0, angle));
        angle += increment;
    }
    points.push(point(rng, 1.0, rad + TAU + overlap * 0.5));
    points.push(point(rng, 0.98, rad + overlap));
    points.push(point(rng, 0.9, rad + overlap * 0.5));
    points
}

/// rough.js's `_curve`: a Catmull-Rom spline through `points[1..]`, sampled into a polyline.
fn flatten_curve(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = points.len();
    if n < 4 {
        return Vec::new();
    }
    let mut out = vec![points[1]];
    let mut i = 1;
    while i + 2 < n {
        let (p0, p1, p2, p3) = (points[i - 1], points[i], points[i + 1], points[i + 2]);
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 + (p1.0 - p3.0) / 6.0, p2.1 + (p1.1 - p3.1) / 6.0);
        for s in 1..=SEGMENT_STEPS {
            let t = s as f32 / SEGMENT_STEPS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            out.push((
                a * p1.0 + b * c1.0 + c * c2.0 + d * p2.0,
                a * p1.1 + b * c1.1 + c * c2.1 + d * p2.1,
            ));
        }
        i += 1;
    }
    out
}

/// rough.js's `ellipse(cx, cy, width, height)` with this ring's options, as a polyline.
fn ellipse(seed: u32, cx: f32, cy: f32, width: f32, height: f32) -> Vec<(f32, f32)> {
    let mut rng = Rough::new(seed);
    let psq = (TAU * (((width / 2.0).powi(2) + (height / 2.0).powi(2)) / 2.0).sqrt()).sqrt();
    let steps = CURVE_STEP_COUNT
        .max(CURVE_STEP_COUNT / 200.0_f32.sqrt() * psq)
        .ceil();
    let increment = TAU / steps;
    let fit = 1.0 - CURVE_FITTING;
    let mut rx = (width / 2.0).abs();
    let mut ry = (height / 2.0).abs();
    rx += rng.offset_opt(rx * fit);
    ry += rng.offset_opt(ry * fit);
    let upper = rng.offset(0.4, 1.0);
    let overlap = increment * rng.offset(0.1, upper);
    flatten_curve(&ellipse_points(
        &mut rng, increment, cx, cy, rx, ry, overlap,
    ))
}

/// The hand-drawn ring around the box (`x`, `y`, `w`, `h`), as polylines, one per iteration.
///
/// This is rough-notation's `circle` with `box="inside"`: an ellipse as wide and tall as the box
/// grown by `padding` on every side, centred on the box. Iteration `i` uses seed `seed + i`.
#[must_use]
pub fn rough_ring(seed: u32, x: f32, y: f32, w: f32, h: f32, padding: f32) -> Vec<Vec<(f32, f32)>> {
    let (width, height) = (w + 2.0 * padding, h + 2.0 * padding);
    let (cx, cy) = (x + width / 2.0 - padding, y + height / 2.0 - padding);
    (0..RING_PASSES)
        .map(|i| ellipse(seed + i, cx, cy, width, height))
        .collect()
}

/// The length of a polyline.
fn length(line: &[(f32, f32)]) -> f32 {
    line.windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .sum()
}

/// The first `progress` of the passes as SVG path data, one `M..L..` sub-path per pass.
///
/// Like rough-notation, pass `i` of `n` gets `progress * n - i` of its own length, clamped to
/// 0..1 except for the last pass, which is never capped.
#[must_use]
pub fn trim_path(passes: &[Vec<(f32, f32)>], progress: f32) -> String {
    let n = passes.len() as f32;
    let mut d = String::new();
    for (i, pass) in passes.iter().enumerate() {
        let last = i + 1 == passes.len();
        let own = (progress * n - i as f32).max(0.0);
        let own = if last { own } else { own.min(1.0) };
        let mut budget = length(pass) * own;
        if budget <= 0.0 || pass.is_empty() {
            continue;
        }
        d.push_str(&format!("M{:.2} {:.2}", pass[0].0, pass[0].1));
        for w in pass.windows(2) {
            let seg = length(w);
            if seg <= budget {
                d.push_str(&format!("L{:.2} {:.2}", w[1].0, w[1].1));
                budget -= seg;
            } else {
                let t = if seg > 0.0 { budget / seg } else { 0.0 };
                d.push_str(&format!(
                    "L{:.2} {:.2}",
                    lerp(w[0].0, w[1].0, t),
                    lerp(w[0].1, w[1].1, t)
                ));
                break;
            }
        }
    }
    d
}

/// Word indices broken greedily into lines no wider than `max_width`, by the `measure` of a run.
///
/// A word wider than the line sits alone on its own line.
#[must_use]
pub fn wrap_words(
    words: &[&str],
    max_width: f32,
    measure: impl Fn(&str) -> f32,
) -> Vec<Vec<usize>> {
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut line: Vec<usize> = Vec::new();
    for i in 0..words.len() {
        let mut candidate: Vec<&str> = line.iter().map(|&k| words[k]).collect();
        candidate.push(words[i]);
        if line.is_empty() || measure(&candidate.join(" ")) <= max_width {
            line.push(i);
        } else {
            lines.push(std::mem::take(&mut line));
            line.push(i);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// What a [`CircleMarker`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct CircleMarkerConfig {
    /// The words before the circled one.
    pub before: String,
    /// The circled word.
    pub circled: String,
    /// The words after the circled one.
    pub after: String,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for CircleMarkerConfig {
    fn default() -> Self {
        Self {
            before: "How much".to_owned(),
            circled: "circular".to_owned(),
            after: "financing is in AI?".to_owned(),
            font_family: None,
            duration: 90,
        }
    }
}

/// The circle marker component.
#[derive(Debug, Clone, Default)]
pub struct CircleMarker {
    /// What the headline says.
    pub config: CircleMarkerConfig,
}

impl CircleMarker {
    /// A headline with `config`.
    #[must_use]
    pub fn new(config: CircleMarkerConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "circle-marker",
            name: "Circle Marker",
            summary: "A hand-drawn circle scribbles itself around one word of a headline",
            feature: "circle-marker",
        }
    }
}

/// The scale that draws the box at 1:1 on a 1080 px canvas and fits it to the width.
#[must_use]
pub fn scene_scale(canvas_width: f32, canvas_height: f32) -> f32 {
    (canvas_height / REFERENCE_HEIGHT).min(MAX_WIDTH_SHARE * canvas_width / BOX_W)
}

impl Component for CircleMarker {
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
        let measure = |s: &str| ctx.text_width_at(&family, FONT_WEIGHT, FONT_SIZE, s);

        let mut words: Vec<&str> = config.before.split_whitespace().collect();
        let circled_at = words.len();
        words.push(config.circled.as_str());
        words.extend(config.after.split_whitespace());
        let lines = wrap_words(&words, BOX_W, measure);

        let scale = scene_scale(ctx.width(), ctx.height());
        let line_h = FONT_SIZE * LINE_HEIGHT;
        let block_h = lines.len() as f32 * line_h;
        let fit = format!(
            "translate({} {}) scale({scale})",
            (ctx.width() - BOX_W * scale) / 2.0,
            (ctx.height() - block_h * scale) / 2.0
        );

        let offset = ctx.baseline_offset_at(&family, FONT_WEIGHT, FONT_SIZE, line_h);

        let mut runs: Vec<Svgr<'static>> = Vec::new();
        let mut ring_box = None;
        for (row, line) in lines.iter().enumerate() {
            let texts: Vec<&str> = line.iter().map(|&k| words[k]).collect();
            let baseline = row as f32 * line_h + offset;
            if let Some(pos) = line.iter().position(|&k| k == circled_at) {
                let prefix = if pos == 0 {
                    0.0
                } else {
                    let before = format!("{} ", texts[..pos].join(" "));
                    ctx.text_width_at(&family, FONT_WEIGHT, FONT_SIZE, &before)
                };
                let w = ctx.text_width_at(&family, FONT_WEIGHT, FONT_SIZE, texts[pos]);
                ring_box = Some((prefix, row as f32 * line_h, w, line_h));
            }
            runs.push(text::text_node_left(
                &texts.join(" "),
                0.0,
                baseline,
                family.clone(),
                FONT_SIZE,
                FONT_WEIGHT,
                0.0,
                INK.to_owned(),
                1.0,
            ));
        }

        let ring = match ring_box {
            Some((x, y, w, h)) => {
                let passes = rough_ring(ring_seed(frame), x, y, w, h, RING_PADDING);
                let d = trim_path(&passes, ring_progress(frame));
                if d.is_empty() {
                    fframes::svgr!(<g></g>)
                } else {
                    fframes::svgr!(
                        <path d={d} fill="none" stroke={RING_COLOR} stroke-width={RING_WIDTH}
                              />
                    )
                }
            }
            None => fframes::svgr!(<g></g>),
        };

        fframes::svgr!(
            <g transform={fit}>
                {Svgr::from(runs)}
                {ring}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = CircleMarkerConfig::default();
        assert_eq!(config.before, "How much");
        assert_eq!(config.circled, "circular");
        assert_eq!(config.after, "financing is in AI?");
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 90);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = CircleMarker::default();
        assert_eq!(component.meta().id, "circle-marker");
        assert_eq!(CircleMarker::meta_static().feature, "circle-marker");
        assert_eq!(component.duration(), 90);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn progress_is_eased_posterized_and_clamped() {
        assert_eq!(ring_progress(0.0), 0.0);
        assert_eq!(ring_progress(-5.0), 0.0);
        assert_eq!(ring_progress(44.0), 1.0);
        assert_eq!(ring_progress(200.0), 1.0);
        // Four-frame holds.
        assert_eq!(ring_progress(8.0), ring_progress(11.0));
        assert!(ring_progress(12.0) > ring_progress(11.0));
        assert!((PROGRESS_EASE.ease(0.5) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn the_seed_runs_from_one_to_ninety_in_steps_of_four() {
        assert_eq!(ring_seed(0.0), 1);
        assert_eq!(ring_seed(3.0), 1);
        assert_eq!(ring_seed(4.0), 5);
        assert_eq!(ring_seed(88.0), 89);
        assert_eq!(ring_seed(500.0), 90);
    }

    #[test]
    fn the_same_seed_draws_the_same_ring_and_another_seed_a_different_one() {
        let a = rough_ring(5, 0.0, 0.0, 300.0, 80.0, 10.0);
        let b = rough_ring(5, 0.0, 0.0, 300.0, 80.0, 10.0);
        let c = rough_ring(9, 0.0, 0.0, 300.0, 80.0, 10.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), RING_PASSES as usize);
    }

    #[test]
    fn the_ring_surrounds_the_padded_box() {
        let passes = rough_ring(3, 100.0, 50.0, 300.0, 80.0, 10.0);
        for pass in &passes {
            let min_x = pass.iter().map(|p| p.0).fold(f32::MAX, f32::min);
            let max_x = pass.iter().map(|p| p.0).fold(f32::MIN, f32::max);
            assert!(min_x < 120.0 && max_x > 380.0, "{min_x} {max_x}");
        }
    }

    #[test]
    fn trimming_draws_a_growing_prefix() {
        let passes = rough_ring(1, 0.0, 0.0, 200.0, 60.0, 10.0);
        assert!(trim_path(&passes, 0.0).is_empty());
        let half = trim_path(&passes, 0.5);
        let full = trim_path(&passes, 1.0);
        assert!(!half.is_empty() && half.len() < full.len());
        assert_eq!(full.matches('M').count(), RING_PASSES as usize);
    }

    #[test]
    fn wrapping_breaks_on_words_and_keeps_every_word() {
        let lines = wrap_words(&["aa", "bb", "cc", "dd"], 5.0, |s| s.len() as f32);
        assert_eq!(lines, vec![vec![0, 1], vec![2, 3]]);
        let long = wrap_words(&["aaaaaaaa", "bb"], 3.0, |s| s.len() as f32);
        assert_eq!(long, vec![vec![0], vec![1]]);
    }

    #[test]
    fn the_scene_fits_landscape_and_portrait_canvases() {
        assert!((scene_scale(1920.0, 1080.0) - 1.0).abs() < 1e-6);
        assert!(BOX_W * scene_scale(1080.0, 1920.0) <= 1080.0);
    }
}
