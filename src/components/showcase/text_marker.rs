//! A sentence in which one phrase is picked out by a rough, hand-drawn highlighter stroke.
//!
//! Ported from Remotion Elements' `TextMarker`. The source is an 800 px wide block of 80 px bold
//! text with `line-height: 1.1`, whose word "remarkable" is wrapped in `@remotion/rough-notation`'s
//! `Highlight` with `color: rgba(255, 236, 79, 0.62)`, `roughness: 2.3`, `maxRandomnessOffset: 10`,
//! `bowing: 0` and 20 px of horizontal padding. The stroke is drawn on over frames 0..25 through a
//! spring easing (`damping 200`, `mass 1`, `stiffness 100`, rest threshold 0.02, tail allowed).
//!
//! Deliberate differences from the source:
//!
//! - **Font.** The source loads Cormorant Garamond 700 from Google Fonts. That face is not
//!   embedded in this crate, so the embedded Inter Bold is the default; set
//!   [`TextMarkerConfig::font_family`] to use another registered family. Inter is wider, so the
//!   sentence may wrap differently from the source.
//! - **Rough stroke.** The two highlight strokes are rough.js 4.6.6's `line` exactly (seeds 1 and
//!   2, the second drawn right to left, one non-overlay `_line` each, same random draw order),
//!   each as thick as the span's content box. rough-notation's `evolvePath` dash reveal is the
//!   first `progress * 2 - i` of each stroke by arc length, cut as a shorter cubic.
//! - **Spring.** `Easing.spring` with `allowTail` is modelled as the spring played over its natural
//!   length (until within 0.02 of rest) across the 25 frame segment, with the leftover blended out
//!   so the stroke ends exactly complete.
//! - **Page.** The source draws no background; here the page is a light neutral so the dark
//!   text reads, and the block is centred on the canvas and scaled with the canvas height
//!   (1:1 at 1080 px).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{Spring, clamp01, interpolate};
use crate::ui::text;

/// The text block's width, in source px.
pub const BLOCK_W: f32 = 800.0;
/// The font size.
pub const FONT_SIZE: f32 = 80.0;
/// The font weight.
pub const FONT_WEIGHT: u16 = 700;
/// The line height, as a multiple of the font size.
pub const LINE_HEIGHT: f32 = 1.1;
/// The text colour.
pub const INK: &str = "#171717";
/// The page behind the block: a light neutral, chosen because the source has no background.
pub const BACKGROUND: &str = "#f6f5f1";
/// The canvas height at which the block is drawn at source size.
pub const REFERENCE_HEIGHT: f32 = 1080.0;

/// The highlight's colour (`rgba(255, 236, 79, 0.62)`), without alpha.
pub const HIGHLIGHT_COLOR: &str = "#ffec4f";
/// The highlight's alpha.
pub const HIGHLIGHT_ALPHA: f32 = 0.62;
/// The highlight's roughness.
pub const ROUGHNESS: f32 = 2.3;
/// The highlight's `maxRandomnessOffset`.
pub const MAX_RANDOMNESS_OFFSET: f32 = 10.0;
/// The highlight's horizontal padding on each side.
pub const PADDING_X: f32 = 20.0;
/// The highlight's `bowing`.
pub const BOWING: f32 = 0.0;
/// The number of strokes rough-notation draws for a highlight (`iterations`).
pub const PASSES: usize = 2;
/// The base seed; stroke `i` uses `SEED + i`.
pub const SEED: i32 = 1;

/// The frame at which the draw-on ends.
pub const DRAW_FRAMES: f32 = 25.0;
/// The draw-on's easing spring.
pub const DRAW_SPRING: Spring = Spring {
    damping: 200.0,
    mass: 1.0,
    stiffness: 100.0,
};
/// How near rest the spring counts as finished (`durationRestThreshold`).
pub const REST_THRESHOLD: f32 = 0.02;

/// The spring's value at `t` of the draw-on segment.
///
/// The segment is stretched to the spring's natural length, the last frame at which it is still
/// [`REST_THRESHOLD`] or more from rest; the small leftover is blended out so `t = 1` is exactly 1.
#[must_use]
pub fn draw_progress(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let mut natural = 1.0;
    for frame in 0..1000 {
        if (DRAW_SPRING.value(frame as f32) - 1.0).abs() >= REST_THRESHOLD {
            natural = frame as f32 + 1.0;
        }
    }
    let leftover = 1.0 - DRAW_SPRING.value(natural);
    let blend = t * t * (3.0 - 2.0 * t);
    DRAW_SPRING.value(t * natural) + leftover * blend
}

/// The highlight's progress at `frame`: `interpolate(frame, [0, 25], [0, 1])` through the spring.
#[must_use]
pub fn progress_at(frame: f32) -> f32 {
    draw_progress(interpolate(frame, [0.0, DRAW_FRAMES], [0.0, 1.0]))
}

/// One word of the sentence and whether the highlighter covers it.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    /// The word as drawn.
    pub text: String,
    /// Whether the word is part of the highlighted phrase.
    pub marked: bool,
}

/// A placed run of words on one line.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The line's text.
    pub text: String,
    /// The `(start, end)` x extent of each highlighted run on the line, from the line's left edge.
    pub marks: Vec<(f32, f32)>,
}

/// `before`, `highlight` and `after` as one flat list of words.
#[must_use]
pub fn words_of(before: &str, highlight: &str, after: &str) -> Vec<Word> {
    let mut out = Vec::new();
    for (part, marked) in [(before, false), (highlight, true), (after, false)] {
        for word in part.split_whitespace() {
            out.push(Word {
                text: word.to_owned(),
                marked,
            });
        }
    }
    out
}

/// `words` broken greedily into lines no wider than `max_width`, by the `measure` of a run.
///
/// A word wider than the line sits alone on its own line. Each line records the x extents of its
/// highlighted runs (a run covers its words and the spaces between them, not the spaces around).
#[must_use]
pub fn layout(words: &[Word], max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<Line> {
    let mut rows: Vec<Vec<&Word>> = Vec::new();
    let mut current: Vec<&Word> = Vec::new();
    for word in words {
        let candidate = current
            .iter()
            .map(|w| w.text.as_str())
            .chain(std::iter::once(word.text.as_str()))
            .collect::<Vec<_>>()
            .join(" ");
        if current.is_empty() || measure(&candidate) <= max_width {
            current.push(word);
        } else {
            rows.push(std::mem::take(&mut current));
            current.push(word);
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    rows.into_iter()
        .map(|row| {
            let mut spans: Vec<(usize, usize)> = Vec::new();
            let mut run: Option<(usize, usize)> = None;
            for (i, word) in row.iter().enumerate() {
                match (word.marked, run) {
                    (true, None) => run = Some((i, i)),
                    (true, Some((a, _))) => run = Some((a, i)),
                    (false, Some(r)) => {
                        spans.push(r);
                        run = None;
                    }
                    (false, None) => {}
                }
            }
            if let Some(r) = run {
                spans.push(r);
            }
            let join = |a: usize, b: usize| {
                row[a..=b]
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let marks = spans
                .into_iter()
                .map(|(a, b)| {
                    let start = if a == 0 {
                        0.0
                    } else {
                        measure(&format!("{} ", join(0, a - 1)))
                    };
                    (start, measure(&join(0, b)))
                })
                .collect();
            Line {
                text: join(0, row.len() - 1),
                marks,
            }
        })
        .collect()
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
/// The random numbers are drawn in rough.js' order, so the same seed gives the same wobble.
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
    let mix = |a: (f32, f32), b: (f32, f32)| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
    let ab = mix(curve[0], curve[1]);
    let bc = mix(curve[1], curve[2]);
    let cd = mix(curve[2], curve[3]);
    let abc = mix(ab, bc);
    let bcd = mix(bc, cd);
    [curve[0], ab, abc, mix(abc, bcd)]
}

/// The SVG path data of `curve`.
fn curve_d(c: &Cubic) -> String {
    format!(
        "M{} {} C{} {}, {} {}, {} {}",
        c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1, c[3].0, c[3].1
    )
}

/// The `i`th highlight stroke across `x0..x1` at height `y`: left to right when `i` is even,
/// right to left when odd, seeded `SEED + i`.
#[must_use]
pub fn highlight_stroke(x0: f32, x1: f32, y: f32, i: usize) -> Cubic {
    let seed = SEED + i as i32;
    if i % 2 == 1 {
        rough_line(x1, y, x0, y, seed)
    } else {
        rough_line(x0, y, x1, y, seed)
    }
}

/// How much of the `i`th of [`PASSES`] strokes is drawn at overall `progress`.
#[must_use]
pub fn stroke_progress(progress: f32, i: usize) -> f32 {
    (progress * PASSES as f32 - i as f32).clamp(0.0, 1.0)
}

/// What a [`TextMarker`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMarkerConfig {
    /// The text before the highlighted phrase.
    pub before: String,
    /// The highlighted phrase.
    pub highlight: String,
    /// The text after the highlighted phrase.
    pub after: String,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for TextMarkerConfig {
    fn default() -> Self {
        Self {
            before: "A truly".to_owned(),
            highlight: "remarkable".to_owned(),
            after: "end to the World cup".to_owned(),
            font_family: None,
            duration: 90,
        }
    }
}

/// The text marker component.
#[derive(Debug, Clone, Default)]
pub struct TextMarker {
    /// What the block shows.
    pub config: TextMarkerConfig,
}

impl TextMarker {
    /// A block with `config`.
    #[must_use]
    pub fn new(config: TextMarkerConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-marker",
            name: "Text Marker",
            summary: "A rough, hand-drawn highlighter stroke is drawn across one phrase",
            feature: "text-marker",
        }
    }
}

impl Component for TextMarker {
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
        let progress = progress_at(ctx.frame());

        let words = words_of(&config.before, &config.highlight, &config.after);
        let lines = layout(&words, BLOCK_W, |s| {
            text::tracked_width_at(ctx, &family, FONT_WEIGHT, FONT_SIZE, s, 0.0)
        });
        let line_h = FONT_SIZE * LINE_HEIGHT;
        // The content area rough-notation measures: the font's ascent plus descent.
        let content_h = ctx
            .meter_at(&family, FONT_WEIGHT)
            .map_or(FONT_SIZE * 1.2, |m| {
                m.ascent(FONT_SIZE) + m.descent(FONT_SIZE)
            });
        let baseline = ctx.baseline_offset_at(&family, FONT_WEIGHT, FONT_SIZE, line_h);

        let block_h = lines.len() as f32 * line_h;
        let scale = ctx.height() / REFERENCE_HEIGHT;
        let fit = format!(
            "translate({} {}) scale({scale})",
            (ctx.width() - BLOCK_W * scale) / 2.0,
            (ctx.height() - block_h * scale) / 2.0
        );

        let mut strokes: Vec<Svgr<'static>> = Vec::new();
        let mut runs: Vec<Svgr<'static>> = Vec::new();
        for (row, line) in lines.iter().enumerate() {
            let top = row as f32 * line_h;
            runs.push(text::text_node_left(
                &line.text,
                0.0,
                top + baseline,
                family.clone(),
                FONT_SIZE,
                FONT_WEIGHT,
                0.0,
                INK.to_owned(),
                1.0,
            ));
            for &(start, end) in &line.marks {
                let x0 = start - PADDING_X;
                let x1 = end + PADDING_X;
                let cy = top + line_h / 2.0;
                for pass in 0..PASSES {
                    let p = stroke_progress(progress, pass);
                    if p <= 0.0 {
                        continue;
                    }
                    let d = curve_d(&partial_curve(&highlight_stroke(x0, x1, cy, pass), p));
                    strokes.push(fframes::svgr!(
                        <path d={d} fill="none" stroke={HIGHLIGHT_COLOR}
                              stroke-opacity={HIGHLIGHT_ALPHA} stroke-width={content_h} />
                    ));
                }
            }
        }

        fframes::svgr!(
            <g transform={fit}>
                {Svgr::from(strokes)}
                {Svgr::from(runs)}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> f32 {
        s.chars().count() as f32 * 10.0
    }

    #[test]
    fn the_defaults_are_the_demo() {
        let config = TextMarkerConfig::default();
        assert_eq!(config.before, "A truly");
        assert_eq!(config.highlight, "remarkable");
        assert_eq!(config.after, "end to the World cup");
        assert_eq!(config.font_family, None);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = TextMarker::default();
        assert_eq!(component.meta().id, "text-marker");
        assert_eq!(TextMarker::meta_static().feature, "text-marker");
        assert_eq!(component.background(), Some(BACKGROUND));
        assert_eq!(component.duration(), 90);
    }

    #[test]
    fn the_draw_on_runs_from_nothing_to_complete_over_25_frames() {
        assert_eq!(progress_at(0.0), 0.0);
        assert_eq!(progress_at(-5.0), 0.0);
        assert!((progress_at(25.0) - 1.0).abs() < 1e-6);
        assert_eq!(progress_at(80.0), 1.0);
        let mut last = 0.0;
        for f in 1..=25 {
            let p = progress_at(f as f32);
            assert!(p >= last - 1e-4, "frame {f}: {p} < {last}");
            last = p;
        }
        assert!(progress_at(5.0) > 0.0 && progress_at(5.0) < 1.0);
    }

    #[test]
    fn layout_marks_the_highlighted_word() {
        let words = words_of("A truly", "remarkable", "end");
        let lines = layout(&words, 1000.0, w);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "A truly remarkable end");
        // "A truly " is 8 characters, "A truly remarkable" 18.
        assert_eq!(lines[0].marks, vec![(80.0, 180.0)]);
    }

    #[test]
    fn layout_wraps_and_keeps_every_word() {
        let words = words_of("aa bb", "cc", "dd");
        let lines = layout(&words, 55.0, w);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text, "aa bb");
        assert_eq!(lines[1].text, "cc dd");
        assert!(lines[0].marks.is_empty());
        assert_eq!(lines[1].marks, vec![(0.0, 20.0)]);
    }

    #[test]
    fn the_strokes_are_deterministic_and_differ_by_seed() {
        let a = highlight_stroke(0.0, 100.0, 0.0, 0);
        assert_eq!(a, highlight_stroke(0.0, 100.0, 0.0, 0));
        assert_ne!(a, highlight_stroke(0.0, 100.0, 0.0, 1));
        // The second stroke runs right to left.
        assert!(highlight_stroke(0.0, 100.0, 0.0, 1)[0].0 > 50.0);
    }

    #[test]
    fn the_strokes_are_drawn_one_after_the_other() {
        assert_eq!(stroke_progress(0.25, 0), 0.5);
        assert_eq!(stroke_progress(0.25, 1), 0.0);
        assert_eq!(stroke_progress(0.75, 0), 1.0);
        assert_eq!(stroke_progress(0.75, 1), 0.5);
        assert_eq!(stroke_progress(1.0, 1), 1.0);
    }

    #[test]
    fn the_random_generator_matches_roughjs() {
        let mut r = RoughRandom::new(1);
        assert!((r.next_value() - 48271.0 / 2_147_483_648.0).abs() < 1e-9);
    }
}
