//! An answer writes itself before a drag selects a statement and highlights one word.

use std::fmt::Write as _;
use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{ease_out_cubic, interpolate, normalised_speed};
use crate::ui::{Mode, UiTheme, blur, clamp01, mix_oklch, text, to_hex, with_alpha};

/// The reference canvas height the source's fractions are quoted against.
pub const REFERENCE_HEIGHT: f32 = 720.0;

/// The reference canvas width the source's measure fraction is quoted against.
pub const REFERENCE_WIDTH: f32 = 1280.0;

/// The question pill's height, as a fraction of the composition height (`PILL_H`).
pub const PILL_H: f32 = 0.0553;

/// The pill's corner radius, as a fraction of its own height (`PILL_R`).
///
/// Deliberately not `0.5`: the source's own comment says a stadium is not what the reference shows.
pub const PILL_R: f32 = 0.28;

/// The pill's horizontal padding either side of the label, as a fraction of its own height
/// (`PILL_PAD`).
pub const PILL_PAD: f32 = 0.42;

/// The question's type size, as a fraction of the composition height (`QUESTION_FONT`).
pub const QUESTION_FONT: f32 = 0.0223;

/// The answer's type size, as a fraction of the composition height (`ANSWER_FONT`).
pub const ANSWER_FONT: f32 = 0.0385;

/// The answer's line height, as a multiple of its own type size (`ANSWER_LEADING`).
///
/// Looser than a paragraph wants, and for a reason the source spells out: the band fills the line
/// box, so anything tighter makes the bands of two selected lines merge into one block of colour.
pub const ANSWER_LEADING: f32 = 1.64;

/// The answer's measure, as a fraction of the composition width (`MEASURE`).
pub const MEASURE: f32 = 0.6;

/// The gap between the pill's bottom and the answer's first line box, as a fraction of the
/// composition height (`PILL_GAP`).
pub const PILL_GAP: f32 = 0.075;

/// The band's padding above and below the type, as a fraction of the answer type size
/// (`BAND_PAD_Y`).
pub const BAND_PAD_Y: f32 = 0.18;

/// The band's corner radius, as a fraction of the answer type size (`BAND_R`).
pub const BAND_R: f32 = 0.1;

/// The caret's width, as a fraction of the answer type size (`CARET_W`).
pub const CARET_W: f32 = 0.06;

/// The question's entrance — scale, opacity and blur — in seconds (`INTRO_S`).
pub const INTRO_S: f32 = 0.375;

/// Seconds the question's entrance blur takes to clear (`INTRO_BLUR_S`).
pub const INTRO_BLUR_S: f32 = 0.2;

/// The scale the question enters at (`INTRO_SCALE`).
pub const INTRO_SCALE: f32 = 1.31;

/// The question's blur at `t = 0`, as a fraction of the composition height (`INTRO_BLUR`).
pub const INTRO_BLUR: f32 = 0.0141;

/// One word of the answer's arrival, in seconds (`WORD_IN_S`).
///
/// Longer than [`WORD_STEP_S`] on purpose: four or five words are always mid-arrival, which is what
/// stops the answer reading as a ticker.
pub const WORD_IN_S: f32 = 0.22;

/// The gap between two arrivals, in seconds (`WORD_STEP_S`).
pub const WORD_STEP_S: f32 = 0.055;

/// How far a word rises as it arrives, as a fraction of its own type size (`WORD_RISE`).
pub const WORD_RISE: f32 = 0.34;

/// Seconds the drag spends on each word of the statement (`BAND_STEP_S`).
pub const BAND_STEP_S: f32 = 0.11;

/// Seconds the picked word's own band takes to strengthen (`WORD_BAND_S`).
pub const WORD_BAND_S: f32 = 0.2;

/// The word gap as an em of the answer type (`WORD_GAP`).
///
/// It is `padding-right` on the word's own span, not a margin and not a space character: that is
/// what makes the band continuous across a selected run instead of four chips with holes between
/// them.
pub const WORD_GAP: f32 = 0.28;

/// The selection band's strength over the page: `withAlpha(accent, 0.16)`.
pub const BAND_ALPHA: f32 = 0.16;

/// How much stronger the picked word's band ends up: `0.16 + 0.14 * wordIn`.
pub const PICKED_ALPHA_GAIN: f32 = 0.14;

/// The answer's font weight — the source hard-codes `400` on the answer block.
pub const ANSWER_WEIGHT: u16 = 400;

/// The question's font weight — the source hard-codes `500` on the pill.
pub const QUESTION_WEIGHT: u16 = 500;

/// The question's line height, as a multiple of its own type size — the source sets `lineHeight: 1`.
pub const QUESTION_LEADING: f32 = 1.0;

/// The luma a pixel must move by to count as motion in the source frame measurement
/// (`lib/motion-check/measure.ts`, `noise` default).
pub const MOTION_NOISE_LUMA: f32 = 2.0;

/// A rectangle in canvas px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// A rectangle from its top-left corner and its size.
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The vertical centre, which is where a horizontal gradient's axis sits.
    #[must_use]
    pub fn center_y(self) -> f32 {
        self.y + self.h / 2.0
    }

    /// The right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.w
    }
}

/// Configuration for [`AnswerHighlight`].
///
/// The field list is `config.ts`'s control list, field for field, plus the plumbing every component
/// in this crate carries.
#[derive(Debug, Clone, PartialEq)]
pub struct AnswerHighlightConfig {
    /// The prompt, in the pill at the top.
    pub question: String,
    /// The answer. It writes itself a word at a time; the line breaks are this crate's, set by the
    /// measure.
    pub answer: String,
    /// The run inside `answer` that gets selected — word for word, as written. A run that is not
    /// found simply never highlights.
    pub statement: String,
    /// The one word inside `statement` that takes the second, stronger band.
    pub word: String,
    /// Seconds the question holds alone before the answer starts.
    pub question_hold: f32,
    /// Seconds between two words of the answer's arrival.
    pub word_step: f32,
    /// Seconds between the answer finishing and the drag starting.
    pub before_drag: f32,
    /// Seconds the drag spends on each word of the statement.
    pub drag_step: f32,
    /// Seconds the selected statement holds before the word is picked out.
    pub before_word: f32,
    /// Page behind the scene. `None` takes the theme's `background`.
    pub paper_color: Option<String>,
    /// Pill fill. `None` takes the theme's `card`.
    pub pill_color: Option<String>,
    /// Question ink. `None` takes the theme's `muted_foreground`.
    pub question_color: Option<String>,
    /// Answer ink. `None` takes the theme's `foreground`.
    pub answer_color: Option<String>,
    /// The selection, the caret and the picked word. `None` takes the theme's `primary`.
    pub accent_color: Option<String>,
    /// The face to paint in. `None` keeps the theme's face, or the crate's embedded Inter.
    pub font_family: Option<String>,
    /// Where the lockup's centre sits, as a fraction of the canvas height.
    pub center_y: f32,
    /// Timeline multiplier. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// Composition length in frames.
    pub duration: usize,
    /// Whether to draw the blur stand-in for the question's entrance.
    pub simulate_blur: bool,
}

impl Default for AnswerHighlightConfig {
    fn default() -> Self {
        // `config.ts`'s demo controls, field for field. Its five colour defaults are the light
        // theme's own token values, spelled out so the customizer has concrete swatches; they are
        // kept as `Option`s here so `None` still means "whatever the theme says".
        Self {
            question: "How should we structure the demo video?".to_owned(),
            answer: "Build one composition per scene and keep the timeline declarative, \
                     so the same props always render the same frames. Anything that reads \
                     the clock at render time is a bug you will find in the export."
                .to_owned(),
            statement: "keep the timeline declarative".to_owned(),
            word: "declarative".to_owned(),
            question_hold: 0.6,
            word_step: WORD_STEP_S,
            before_drag: 0.45,
            drag_step: BAND_STEP_S,
            before_word: 0.6,
            paper_color: Some("#faf9f6".to_owned()),
            pill_color: Some("#ffffff".to_owned()),
            question_color: Some("#6e6a63".to_owned()),
            answer_color: Some("#141414".to_owned()),
            accent_color: Some("#3072db".to_owned()),
            font_family: None,
            center_y: 0.5,
            speed: 1.0,
            mode: Mode::Light,
            theme: None,
            duration: 190,
            simulate_blur: true,
        }
    }
}

/// The answer, as the tokens the scene animates.
///
/// Split on whitespace with the punctuation left **attached to its word**: the band is painted per
/// token, so a comma that became its own token would get its own band and its own arrival, and the
/// selection would come apart at every clause.
#[must_use]
pub fn split_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

/// A token with its outer punctuation taken off, for matching only.
///
/// So that `statement: "keep the timeline declarative"` finds the answer's `declarative,`:
/// `\p{L}` and `\p{N}` are exactly Rust's `is_alphabetic` and `is_numeric`.
#[must_use]
pub fn bare(token: &str) -> String {
    let start = token
        .char_indices()
        .find(|(_, c)| c.is_alphanumeric())
        .map_or(token.len(), |(i, _)| i);
    let end = token
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_alphanumeric())
        .map_or(start, |(i, c)| i + c.len_utf8());
    token[start..end].to_owned()
}

/// Where `phrase` sits in `words`, as a half-open `[start, end)` word range.
///
/// Matched on the joined text rather than word by word, so a phrase written with its own internal
/// spacing still lands. Returns `None` when it is not there, which is what makes a drifted pair of
/// strings visible as a missing band instead of a band over a near miss.
#[must_use]
pub fn find_run(words: &[String], phrase: &str) -> Option<(usize, usize)> {
    let target: Vec<String> = split_words(phrase).iter().map(|w| bare(w)).collect();
    if target.is_empty() {
        return None;
    }
    if target.len() > words.len() {
        return None;
    }
    let wanted = target.join(" ");
    let haystack: Vec<String> = words.iter().map(|word| bare(word)).collect();
    for start in 0..=(words.len() - target.len()) {
        if haystack[start..start + target.len()].join(" ") == wanted {
            return Some((start, start + target.len()));
        }
    }
    None
}

/// Progress of a value that starts at `at` and takes `dur` frames — the source's `ramp`.
///
/// A non-positive duration is a step, and `NaN` reads as nothing has happened.
#[must_use]
pub fn ramp(frame: f32, at: f32, dur: f32) -> f32 {
    if dur <= 0.0 {
        return if frame >= at { 1.0 } else { 0.0 };
    }
    clamp01((frame - at) / dur)
}

/// The frame the drag's leading edge is on, as a fractional word index — the source's `dragEdge`.
///
/// Fractional on purpose: the caret is drawn on `floor(edge)` and the band on everything behind it,
/// so the edge can sit between two words instead of jumping a whole word a frame.
#[must_use]
pub fn drag_edge(frame: f32, at: f32, step: f32, count: usize) -> f32 {
    if step <= 0.0 {
        return if frame >= at { count as f32 } else { 0.0 };
    }
    let limited = count as f32;
    if (frame - at).is_nan() {
        return 0.0;
    }
    ((frame - at) / step).clamp(0.0, limited)
}

/// How far the drag has crossed word `index` of a run that starts at `run_start`.
#[must_use]
pub fn word_across(index: usize, run_start: usize, edge: f32) -> f32 {
    clamp01(edge - (index - run_start) as f32)
}

/// The strength of a word's band: the selection's own tint, deepened for the picked word.
#[must_use]
pub fn band_alpha(picked: bool, word_mix: f32) -> f32 {
    if picked {
        BAND_ALPHA + PICKED_ALPHA_GAIN * clamp01(word_mix)
    } else {
        BAND_ALPHA
    }
}

/// The width of a word's inline box: its advance plus the gap that follows it.
#[must_use]
pub fn word_box(advance: f32, font_size: f32) -> f32 {
    advance + WORD_GAP * font_size
}

/// The answer's line pitch, in reference-canvas px.
///
/// Not `ANSWER_LEADING * font_size`: each word is an `inline-block` carrying `BAND_PAD_Y` of
/// vertical padding, so the line box grows to the inline box's outer height. The two identities
/// collapse to `(1.64 + 2 * 0.18) * font_size`, i.e. exactly `2 * font_size`.
#[must_use]
pub fn line_pitch(font_size: f32) -> f32 {
    ANSWER_LEADING * font_size + 2.0 * BAND_PAD_Y * font_size
}

/// Greedy wrapping of the answer's word boxes into lines of at most `measure`.
///
/// Returns one half-open `[start, end)` word range per line. There is no whitespace between the
/// source's spans — the gap *is* the padding — so the boxes are laid end to end with nothing
/// between them, and a line breaks where the next box would not fit.
#[must_use]
pub fn wrap(boxes: &[f32], measure: f32) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let Some(&first) = boxes.first() else {
        return lines;
    };
    let mut start = 0;
    let mut width = first;
    for (index, box_width) in boxes.iter().enumerate().skip(1) {
        if width + box_width > measure {
            lines.push((start, index));
            start = index;
            width = *box_width;
        } else {
            width += box_width;
        }
    }
    lines.push((start, boxes.len()));
    lines
}

/// A rounded rectangle as an SVG path, with independent corner radii.
///
/// SVG's `<rect rx>` rounds all four corners, and a band needs one side rounded and the other
/// square: the source rounds the run's *first* word on the left and its *last* word on the right,
/// so a band that wraps to a second line is square where it starts there. `radii` is
/// `[top-left, top-right, bottom-right, bottom-left]`.
#[must_use]
pub fn rounded_rect(rect: Rect, radii: [f32; 4]) -> String {
    let Rect { x, y, w, h } = rect;
    let [tl, tr, br, bl] = radii;
    let mut d = format!("M {} {}", x + tl, y);
    corner(&mut d, tr, x + w - tr, y, x + w, y + tr);
    corner(&mut d, br, x + w, y + h - br, x + w - br, y + h);
    corner(&mut d, bl, x + bl, y + h, x, y + h - bl);
    corner(&mut d, tl, x, y + tl, x + tl, y);
    d.push_str(" Z");
    d
}

/// Walk to a corner's entry point and, when it has a radius, arc round to its exit point.
///
/// A zero radius emits the single straight edge segment the two points collapse to, so a square
/// band is a plain four-sided path rather than a stack of degenerate arcs.
fn corner(d: &mut String, radius: f32, entry_x: f32, entry_y: f32, exit_x: f32, exit_y: f32) {
    let _ = write!(d, " L {entry_x} {entry_y}");
    if radius > 1e-4 {
        let _ = write!(d, " A {radius} {radius} 0 0 1 {exit_x} {exit_y}");
    }
}

/// The paint-server id for one word's band gradient.
///
/// The frame is part of the id because `usvgr`'s paint-server cache is keyed by element id and the
/// entry written for one frame survives into the next: a constant id would hand every later frame
/// whichever gradient was resolved first, and the caret would freeze wherever it started.
#[must_use]
pub fn band_id(index: usize, frame: f32) -> String {
    format!("answer-highlight-band-{index}-{frame:.0}")
}

/// The band's gradient stops for a word the drag is part-way across.
///
/// Returns `(stop, caret_end)`, both as fractions of the box: fill up to `stop`, the caret from
/// `stop` to `caret_end`, and nothing after it. `caret_end` is clamped to the box, which is what
/// CSS's `calc(<stop> + <caret>)` does when the caret would run past the background.
#[must_use]
pub fn band_stops(across: f32, caret_width: f32, box_width: f32) -> (f32, f32) {
    let stop = clamp01(across);
    if box_width <= 0.0 {
        return (stop, stop);
    }
    (stop, (stop + caret_width / box_width).min(1.0))
}

/// The clock the whole scene is scheduled on, in local composition frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schedule {
    /// The frame the first answer word starts arriving on.
    pub answer_at: f32,
    /// Frames between two words' arrivals.
    pub word_step: f32,
    /// Frames one word's arrival takes.
    pub word_in: f32,
    /// The frame the last word lands on.
    pub answer_done: f32,
    /// The frame the drag's leading edge starts moving on.
    pub drag_at: f32,
    /// Frames the drag spends on each word of the statement.
    pub drag_step: f32,
    /// How many words the statement turned out to be.
    pub run_len: usize,
    /// The frame the drag's edge reaches the end of the statement.
    pub drag_done: f32,
    /// The frame the picked word's own ramp starts on.
    pub word_at: f32,
    /// Frames the picked word's ramp takes.
    pub word_ramp: f32,
    /// The frame the source's own schedule is finished on.
    pub word_end: f32,
}

impl Schedule {
    /// Resolve the source's clock for a copy of `word_count` words and a `run_len`-word statement.
    #[must_use]
    pub fn resolve(
        config: &AnswerHighlightConfig,
        fps: f32,
        word_count: usize,
        run_len: usize,
    ) -> Self {
        let answer_at = config.question_hold * fps;
        let word_step = config.word_step * fps;
        let word_in = WORD_IN_S * fps;
        let answer_done = answer_at + word_count.saturating_sub(1) as f32 * word_step + word_in;
        let drag_at = answer_done + config.before_drag * fps;
        let drag_step = config.drag_step * fps;
        let drag_done = drag_at + run_len as f32 * drag_step;
        let word_at = drag_done + config.before_word * fps;
        let word_ramp = WORD_BAND_S * fps;
        Self {
            answer_at,
            word_step,
            word_in,
            answer_done,
            drag_at,
            drag_step,
            run_len,
            drag_done,
            word_at,
            word_ramp,
            word_end: word_at + word_ramp,
        }
    }

    /// The first frame on which the finished render is indistinguishable from rest.
    ///
    /// This is the number the source frame measurement records as `settle.base` for the shipped
    /// default — **132**. The last thing to move is the picked word's ink, on a cubic ease-out, so
    /// the schedule's final frame (`word_end` = 133.05 here) moves it by less than one luma level
    /// and the metric's own 2-level floor stops seeing it a frame early. See
    /// [`visible_ramp_fraction`].
    #[must_use]
    pub fn settle_frame(&self, ink: &str, accent: &str) -> f32 {
        (self.word_at + self.word_ramp * visible_ramp_fraction(ink, accent)).ceil()
    }
}

/// Rec.601 luma, weighted exactly as the source frame metric does.
#[must_use]
pub fn luma(color: &str) -> f32 {
    let rgba = crate::ui::parse_color(color);
    (0.299 * rgba.r + 0.587 * rgba.g + 0.114 * rgba.b) as f32 * 255.0
}

/// How much of a cubic ease-out is *visible* when its ends are `ink` and `accent`.
///
/// The ramp is `1 - (1 - t)³`, so the distance still to travel at `t` is `(1 - t)³` of the ink's
/// total colour change. A frame-to-frame metric with a luma floor stops seeing that change once it
/// drops under [`MOTION_NOISE_LUMA`], which happens at `t = 1 - (noise / gap)^(1/3)` — 0.715 of the
/// way for the shipped default's `#141414` into `#3072db`, i.e. frame 131.35 of a six-frame ramp
/// that runs from 127.05. The first frame at rest is therefore 132.
///
/// A ramp whose ends are too close to move a pixel at all is fully visible at `t = 1`, because
/// nothing distinguishes its frames in the first place.
#[must_use]
pub fn visible_ramp_fraction(ink: &str, accent: &str) -> f32 {
    let gap = (luma(accent) - luma(ink)).abs();
    if gap <= MOTION_NOISE_LUMA {
        return 1.0;
    }
    1.0 - (MOTION_NOISE_LUMA / gap).cbrt()
}

/// One laid-out word of the answer, in canvas px.
#[derive(Debug, Clone, PartialEq)]
pub struct WordSlot {
    /// The word's index in the answer, punctuation included.
    pub index: usize,
    /// The word as drawn.
    pub text: String,
    /// Left edge of the word's inline box.
    pub box_left: f32,
    /// Top edge of the word's inline box — where its band starts.
    pub box_top: f32,
    /// The word's measured advance.
    pub advance: f32,
    /// The inline box's width: the advance plus the gap that follows it.
    pub box_width: f32,
    /// The line the word sits on, from zero.
    pub line: usize,
    /// The line's baseline, before the word's own rise.
    pub baseline: f32,
}

/// The whole scene laid out in canvas px — computed once per component instance.
#[derive(Debug, Clone, PartialEq)]
pub struct AnswerHighlightLayout {
    /// The canvas scale: `ctx.vmin() / REFERENCE_VMIN`.
    pub unit: f32,
    /// The answer's type size.
    pub font_size: f32,
    /// The question's type size.
    pub question_size: f32,
    /// The band's vertical padding, one side.
    pub band_pad: f32,
    /// The band's corner radius.
    pub band_radius: f32,
    /// The caret's width.
    pub caret_width: f32,
    /// How far a word rises as it arrives.
    pub word_rise: f32,
    /// The answer's line pitch — also the band's height.
    pub line_pitch: f32,
    /// The pill's box.
    pub pill: Rect,
    /// The pill's corner radius.
    pub pill_radius: f32,
    /// Left edge of the question's label.
    pub question_left: f32,
    /// The question's baseline.
    pub question_baseline: f32,
    /// The lockup's top edge.
    pub lockup_top: f32,
    /// The lockup's left edge — the answer's measure's own left edge.
    pub lockup_left: f32,
    /// Top edge of the answer block.
    pub answer_top: f32,
    /// Every word of the answer, in reading order.
    pub words: Vec<WordSlot>,
    /// How many lines the answer wrapped to.
    pub lines: usize,
    /// Where [`AnswerHighlightConfig::statement`] landed, as a word range.
    pub run: Option<(usize, usize)>,
    /// Where [`AnswerHighlightConfig::word`] landed, when it is inside the run.
    pub picked: Option<(usize, usize)>,
}

impl AnswerHighlightLayout {
    /// Lay the scene out against a live canvas.
    ///
    /// This is the single source of the drawn geometry: `show` and the tests both come through
    /// here, so a baseline or a left edge cannot drift between what is asserted and what is
    /// painted.
    #[must_use]
    pub fn resolve(ctx: &RenderCtx<'_>, config: &AnswerHighlightConfig, family: &str) -> Self {
        let unit = text::unit(ctx);
        let font_size = ANSWER_FONT * REFERENCE_HEIGHT * unit;
        let question_size = QUESTION_FONT * REFERENCE_HEIGHT * unit;
        let pill_height = PILL_H * REFERENCE_HEIGHT * unit;
        let pill_pad = PILL_PAD * pill_height;
        let gap = PILL_GAP * REFERENCE_HEIGHT * unit;
        let measure = MEASURE * REFERENCE_WIDTH * unit;
        let band_pad = BAND_PAD_Y * font_size;
        let line_height = ANSWER_LEADING * font_size;
        let pitch = line_pitch(font_size);

        let texts = split_words(&config.answer);
        let advances: Vec<f32> = texts
            .iter()
            .map(|word| text::tracked_width_at(ctx, family, ANSWER_WEIGHT, font_size, word, 0.0))
            .collect();
        let boxes: Vec<f32> = advances.iter().map(|a| word_box(*a, font_size)).collect();
        let ranges = wrap(&boxes, measure);
        let lockup_left = (ctx.width() - measure) / 2.0;
        let answer_height = ranges.len() as f32 * pitch;
        let lockup_height = pill_height + gap + answer_height;
        let lockup_top = config.center_y * ctx.height() - lockup_height / 2.0;
        let answer_top = lockup_top + pill_height + gap;
        // The inline box's own baseline: the strut's offset plus the band's top padding.
        let baseline_in_line =
            ctx.baseline_offset_at(family, ANSWER_WEIGHT, font_size, line_height) + band_pad;

        let mut words = Vec::with_capacity(texts.len());
        for (line, &(start, end)) in ranges.iter().enumerate() {
            let top = answer_top + line as f32 * pitch;
            let baseline = top + baseline_in_line;
            let mut box_left = lockup_left;
            for index in start..end {
                words.push(WordSlot {
                    index,
                    text: texts[index].clone(),
                    box_left,
                    box_top: top,
                    advance: advances[index],
                    box_width: boxes[index],
                    line,
                    baseline,
                });
                box_left += boxes[index];
            }
        }

        let question_advance = text::tracked_width_at(
            ctx,
            family,
            QUESTION_WEIGHT,
            question_size,
            &config.question,
            0.0,
        );
        let pill = Rect::new(
            lockup_left,
            lockup_top,
            question_advance + 2.0 * pill_pad,
            pill_height,
        );
        let question_line = QUESTION_LEADING * question_size;
        let question_box_top = pill.y + (pill_height - question_line) / 2.0;
        let question_baseline = question_box_top
            + ctx.baseline_offset_at(family, QUESTION_WEIGHT, question_size, question_line);

        let run = find_run(&texts, &config.statement);
        let word_run = run.and_then(|_| find_run(&texts, &config.word));
        let picked = match (word_run, run) {
            (Some(inner), Some(outer)) if inner.0 >= outer.0 && inner.1 <= outer.1 => Some(inner),
            _ => None,
        };

        Self {
            unit,
            font_size,
            question_size,
            band_pad,
            band_radius: BAND_R * font_size,
            caret_width: CARET_W * font_size,
            word_rise: WORD_RISE * font_size,
            line_pitch: pitch,
            pill,
            pill_radius: PILL_R * pill_height,
            question_left: pill.x + pill_pad,
            question_baseline,
            lockup_top,
            lockup_left,
            answer_top,
            words,
            lines: ranges.len(),
            run,
            picked,
        }
    }

    /// The band's box for a word, once the word's own rise is applied.
    #[must_use]
    pub fn band_box(&self, slot: &WordSlot, lift: f32) -> Rect {
        Rect::new(
            slot.box_left,
            slot.box_top + lift,
            slot.box_width,
            self.line_pitch,
        )
    }

    /// The corner radii of a word's band: the run's first word rounds left, its last rounds right.
    #[must_use]
    pub fn band_radii(&self, index: usize) -> (f32, f32) {
        match self.run {
            Some((start, end)) => (
                if index == start {
                    self.band_radius
                } else {
                    0.0
                },
                if index + 1 == end {
                    self.band_radius
                } else {
                    0.0
                },
            ),
            None => (0.0, 0.0),
        }
    }
}

/// Wrap a node in a scale about a point, collapsing to the node itself at rest.
fn scaled_about(node: Svgr<'static>, center_x: f32, center_y: f32, scale: f32) -> Svgr<'static> {
    if (scale - 1.0).abs() < 1e-4 {
        return node;
    }
    let transform = format!(
        "translate({center_x} {center_y}) scale({scale}) translate({} {})",
        -center_x, -center_y
    );
    fframes::svgr!(<g transform={transform}>{node}</g>)
}

/// The **Answer Highlight** component.
#[derive(Debug, Clone, Default)]
pub struct AnswerHighlight {
    /// Component options.
    pub config: AnswerHighlightConfig,
    layout: OnceLock<AnswerHighlightLayout>,
}

impl AnswerHighlight {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: AnswerHighlightConfig) -> Self {
        Self {
            config,
            layout: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "answer-highlight",
            name: "Answer Highlight",
            summary: "An answer writes itself, then a drag selects a statement and picks out one word",
            feature: "answer-highlight",
        }
    }

    /// The laid-out scene, measured once per component instance.
    fn layout(&self, ctx: &RenderCtx<'_>, family: &str) -> &AnswerHighlightLayout {
        self.layout
            .get_or_init(|| AnswerHighlightLayout::resolve(ctx, &self.config, family))
    }
}

impl Component for AnswerHighlight {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // `config.ts` declares no `previewBackdrop`; the source's own `AbsoluteFill` paints
        // `paperColor ?? theme.background`, so the frame behind is the light palette's page.
        Some("#faf9f6")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let unit = text::unit(ctx);
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);
        let layout = self.layout(ctx, &family);

        let paper = config
            .paper_color
            .clone()
            .unwrap_or_else(|| theme.background.clone());
        let pill_fill = config
            .pill_color
            .clone()
            .unwrap_or_else(|| theme.card.clone());
        let ask_ink = config
            .question_color
            .clone()
            .unwrap_or_else(|| theme.muted_foreground.clone());
        let ink = config
            .answer_color
            .clone()
            .unwrap_or_else(|| theme.foreground.clone());
        let accent = config
            .accent_color
            .clone()
            .unwrap_or_else(|| theme.primary.clone());

        let frame = ctx.frame() * normalised_speed(config.speed);
        let fps = ctx.info.fps;
        let run_len = layout.run.map_or(0, |(start, end)| end - start);
        let schedule = Schedule::resolve(config, fps, layout.words.len(), run_len);
        let edge = drag_edge(frame, schedule.drag_at, schedule.drag_step, run_len);
        let word_mix = ease_out_cubic(ramp(frame, schedule.word_at, schedule.word_ramp));

        let mut defs: Vec<Svgr<'static>> = Vec::new();
        let mut body: Vec<Svgr<'static>> = Vec::new();

        // The page. The source's own `AbsoluteFill` paints it, so it is drawn here rather than
        // left to the frame behind.
        body.push(fframes::svgr!(
            <rect x="0" y="0" width={ctx.width()} height={ctx.height()} fill={paper} />
        ));

        for slot in &layout.words {
            let arrive = ease_out_cubic(ramp(
                frame,
                schedule.answer_at + slot.index as f32 * schedule.word_step,
                schedule.word_in,
            ));
            if arrive <= 0.0 {
                continue;
            }
            // `translateY((1 - arrive) * WORD_RISE * font)`: the word starts below its slot.
            let lift = (1.0 - arrive) * layout.word_rise;
            let is_picked = layout
                .picked
                .is_some_and(|(start, end)| slot.index >= start && slot.index < end);
            let across = match layout.run {
                Some((start, end)) if slot.index >= start && slot.index < end => {
                    word_across(slot.index, start, edge)
                }
                _ => 0.0,
            };
            let fill_alpha = band_alpha(is_picked, word_mix);
            let word_ink = if is_picked {
                to_hex(mix_oklch(&ink, &accent, word_mix))
            } else {
                ink.clone()
            };

            // The band goes behind the word and travels with it.
            let band = layout.band_box(slot, lift);
            if across > 0.0 && band.w > 0.0 {
                let (left, right) = layout.band_radii(slot.index);
                let path = rounded_rect(band, [left, right, right, left]);
                if across >= 1.0 {
                    body.push(fframes::svgr!(
                        <path d={path} fill={with_alpha(&accent, fill_alpha)}
                              fill-opacity={fill_alpha} />
                    ));
                } else {
                    let (stop, caret_end) = band_stops(across, layout.caret_width, band.w);
                    let id = band_id(slot.index, ctx.frame());
                    defs.push(fframes::svgr!(
                        <linearGradient id={id.clone()} gradientUnits="userSpaceOnUse"
                                        x1={band.x} y1={band.center_y()} x2={band.right()}
                                        y2={band.center_y()}>
                            <stop offset="0" stop-color={with_alpha(&accent, fill_alpha)}
                                  stop-opacity={fill_alpha} />
                            <stop offset={stop} stop-color={with_alpha(&accent, fill_alpha)}
                                  stop-opacity={fill_alpha} />
                            <stop offset={stop} stop-color={accent.clone()} stop-opacity="1" />
                            <stop offset={caret_end} stop-color={accent.clone()} stop-opacity="1" />
                            <stop offset={caret_end} stop-color={accent.clone()} stop-opacity="0" />
                        </linearGradient>
                    ));
                    body.push(fframes::svgr!(
                        <path d={path} fill={format!("url(#{id})")} />
                    ));
                }
            }

            // The word is positioned by its *left* edge: the source lays the spans out in a line,
            // it does not centre them.
            body.push(text::text_node_left(
                &slot.text,
                slot.box_left,
                slot.baseline + lift,
                family.clone(),
                layout.font_size,
                ANSWER_WEIGHT,
                0.0,
                word_ink,
                arrive,
            ));
        }

        // The question, in its pill, with an entrance scaled about the pill's own left edge.
        let intro = ease_out_cubic(ramp(frame, 0.0, INTRO_S * fps));
        let blur_clear = ease_out_cubic(ramp(frame, 0.0, INTRO_BLUR_S * fps));
        let intro_blur = INTRO_BLUR * REFERENCE_HEIGHT * (1.0 - blur_clear);
        let intro_scale = interpolate(intro, [0.0, 1.0], [INTRO_SCALE, 1.0]);
        let origin_x = layout.pill.x;
        let origin_y = layout.pill.center_y();
        let question = config.question.clone();
        let question_left = layout.question_left;
        let question_baseline = layout.question_baseline;
        let question_size = layout.question_size;
        let pill = layout.pill;
        let pill_radius = layout.pill_radius;
        let pill_node = |dx: f32, dy: f32, opacity: f32| -> Svgr<'static> {
            let label = text::text_node_left(
                &question,
                question_left,
                question_baseline,
                family.clone(),
                question_size,
                QUESTION_WEIGHT,
                0.0,
                ask_ink.clone(),
                1.0,
            );
            let node = fframes::svgr!(
                <g>
                    <rect x={pill.x} y={pill.y} width={pill.w} height={pill.h}
                          rx={pill_radius} fill={pill_fill.clone()} />
                    {label}
                </g>
            );
            let node = scaled_about(node, origin_x, origin_y, intro_scale);
            if dx == 0.0 && dy == 0.0 {
                fframes::svgr!(<g opacity={opacity}>{node}</g>)
            } else {
                let transform = format!("translate({dx} {dy})");
                fframes::svgr!(<g transform={transform} opacity={opacity}>{node}</g>)
            }
        };

        if intro > 0.0 {
            if config.simulate_blur && blur::is_visible(intro_blur) {
                let alpha = blur::ghost_alpha(intro_blur, unit) * intro;
                for (dx, dy) in blur::ghost_offsets(intro_blur, unit) {
                    body.push(pill_node(dx, dy, alpha));
                }
            }
            body.push(pill_node(0.0, 0.0, intro));
        }

        let defs = Svgr::from(defs);
        let body = Svgr::from(body);
        fframes::svgr!(
            <g>
                <defs>{defs}</defs>
                {body}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Fonts;
    use crate::ctx::{ComponentFrame, RenderCtx};

    fn fps() -> f32 {
        30.0
    }

    fn schedule(config: &AnswerHighlightConfig, words: usize, run: usize) -> Schedule {
        Schedule::resolve(config, fps(), words, run)
    }

    #[test]
    fn the_source_defaults_are_reproduced_exactly() {
        let config = AnswerHighlightConfig::default();
        assert_eq!(config.question, "How should we structure the demo video?");
        assert_eq!(
            config.answer,
            "Build one composition per scene and keep the timeline declarative, \
             so the same props always render the same frames. Anything that reads \
             the clock at render time is a bug you will find in the export."
        );
        assert_eq!(config.statement, "keep the timeline declarative");
        assert_eq!(config.word, "declarative");
        assert!((config.question_hold - 0.6).abs() < 1e-6);
        assert!((config.word_step - 0.055).abs() < 1e-6);
        assert!((config.before_drag - 0.45).abs() < 1e-6);
        assert!((config.drag_step - 0.11).abs() < 1e-6);
        assert!((config.before_word - 0.6).abs() < 1e-6);
        assert_eq!(config.paper_color.as_deref(), Some("#faf9f6"));
        assert_eq!(config.pill_color.as_deref(), Some("#ffffff"));
        assert_eq!(config.question_color.as_deref(), Some("#6e6a63"));
        assert_eq!(config.answer_color.as_deref(), Some("#141414"));
        assert_eq!(config.accent_color.as_deref(), Some("#3072db"));
        assert_eq!(config.font_family, None);
        assert!((config.center_y - 0.5).abs() < 1e-6);
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.theme, None);
        assert_eq!(config.duration, 190);
        assert!(config.simulate_blur);
    }

    #[test]
    fn the_proportions_are_the_sources_own_fractions() {
        // Every constant is the number in `index.tsx`, to the digit.
        assert!((PILL_H - 0.0553).abs() < 1e-9);
        assert!((PILL_R - 0.28).abs() < 1e-9);
        assert!((PILL_PAD - 0.42).abs() < 1e-9);
        assert!((QUESTION_FONT - 0.0223).abs() < 1e-9);
        assert!((ANSWER_FONT - 0.0385).abs() < 1e-9);
        assert!((ANSWER_LEADING - 1.64).abs() < 1e-9);
        assert!((MEASURE - 0.6).abs() < 1e-9);
        assert!((PILL_GAP - 0.075).abs() < 1e-9);
        assert!((BAND_PAD_Y - 0.18).abs() < 1e-9);
        assert!((BAND_R - 0.1).abs() < 1e-9);
        assert!((CARET_W - 0.06).abs() < 1e-9);
        assert!((INTRO_S - 0.375).abs() < 1e-9);
        assert!((INTRO_BLUR_S - 0.2).abs() < 1e-9);
        assert!((INTRO_SCALE - 1.31).abs() < 1e-9);
        assert!((INTRO_BLUR - 0.0141).abs() < 1e-9);
        assert!((WORD_IN_S - 0.22).abs() < 1e-9);
        assert!((WORD_STEP_S - 0.055).abs() < 1e-9);
        assert!((WORD_RISE - 0.34).abs() < 1e-9);
        assert!((BAND_STEP_S - 0.11).abs() < 1e-9);
        assert!((WORD_BAND_S - 0.2).abs() < 1e-9);
        assert!((WORD_GAP - 0.28).abs() < 1e-9);
        assert_eq!(ANSWER_WEIGHT, 400);
        assert_eq!(QUESTION_WEIGHT, 500);
    }

    #[test]
    fn duration_and_meta_come_from_the_registry() {
        let component = AnswerHighlight::default();
        assert_eq!(component.duration(), 190);
        let meta = AnswerHighlight::meta_static();
        assert_eq!(meta.id, "answer-highlight");
        assert_eq!(meta.feature, "answer-highlight");
        assert_eq!(component.meta().id, meta.id);
        assert_eq!(component.background(), Some("#faf9f6"));
    }

    #[test]
    fn split_words_keeps_punctuation_attached_to_its_word() {
        assert_eq!(
            split_words("keep it declarative, always"),
            ["keep", "it", "declarative,", "always"]
        );
    }

    #[test]
    fn split_words_collapses_runs_of_whitespace() {
        assert_eq!(split_words("  a\n\n b   c "), ["a", "b", "c"]);
        assert!(split_words("   ").is_empty());
    }

    #[test]
    fn bare_strips_outer_punctuation_only() {
        assert_eq!(bare("declarative,"), "declarative");
        assert_eq!(bare("\"quoted.\""), "quoted");
        assert_eq!(bare("state-of-the-art"), "state-of-the-art");
        assert_eq!(bare("30-second"), "30-second");
        assert_eq!(bare("..."), "");
    }

    #[test]
    fn find_run_matches_without_the_punctuation_written_in() {
        let words = split_words(
            "Build one composition per scene and keep the timeline declarative, so it renders.",
        );
        assert_eq!(
            find_run(&words, "keep the timeline declarative"),
            Some((6, 10))
        );
        assert_eq!(find_run(&words, "declarative"), Some((9, 10)));
    }

    #[test]
    fn find_run_returns_none_rather_than_a_near_miss() {
        let words =
            split_words("Build one composition per scene and keep the timeline declarative.");
        assert_eq!(find_run(&words, "keep the timeline honest"), None);
        assert_eq!(find_run(&words, ""), None);
        assert_eq!(find_run(&[], "declarative"), None);
    }

    #[test]
    fn find_run_takes_the_first_occurrence_when_a_phrase_repeats() {
        let words = split_words("a b a b");
        assert_eq!(find_run(&words, "a b"), Some((0, 2)));
    }

    #[test]
    fn ramp_clamps_at_both_ends_and_steps_at_zero_duration() {
        assert!((ramp(0.0, 10.0, 5.0) - 0.0).abs() < 1e-6);
        assert!((ramp(12.5, 10.0, 5.0) - 0.5).abs() < 1e-6);
        assert!((ramp(99.0, 10.0, 5.0) - 1.0).abs() < 1e-6);
        assert!((ramp(9.0, 10.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((ramp(10.0, 10.0, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn drag_edge_is_fractional_inside_a_word_so_the_edge_never_jumps() {
        assert!((drag_edge(0.0, 10.0, 3.0, 4) - 0.0).abs() < 1e-6);
        assert!((drag_edge(10.0, 10.0, 3.0, 4) - 0.0).abs() < 1e-6);
        assert!((drag_edge(999.0, 10.0, 3.0, 4) - 4.0).abs() < 1e-6);
        assert!((drag_edge(11.5, 10.0, 3.0, 4) - 0.5).abs() < 1e-6);
        assert!((drag_edge(16.0, 10.0, 3.0, 4) - 2.0).abs() < 1e-6);
        // A zero step is a cut, not a freeze.
        assert!((drag_edge(9.0, 10.0, 0.0, 4) - 0.0).abs() < 1e-6);
        assert!((drag_edge(10.0, 10.0, 0.0, 4) - 4.0).abs() < 1e-6);
    }

    #[test]
    fn word_across_covers_one_word_at_a_time() {
        // Edge at 1.5 of a four-word run: the first word is fully covered, the second half, the
        // rest untouched.
        assert!((word_across(0, 0, 1.5) - 1.0).abs() < 1e-6);
        assert!((word_across(1, 0, 1.5) - 0.5).abs() < 1e-6);
        assert!((word_across(2, 0, 1.5) - 0.0).abs() < 1e-6);
        // And the statement's own first word is index 6 of the answer, not 0.
        assert!((word_across(6, 6, 0.25) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn the_band_is_a_tint_and_the_picked_word_gains_fourteen_hundredths() {
        assert!((band_alpha(false, 1.0) - 0.16).abs() < 1e-6);
        assert!((band_alpha(true, 0.0) - 0.16).abs() < 1e-6);
        assert!((band_alpha(true, 1.0) - 0.30).abs() < 1e-6);
        assert!((band_alpha(true, 0.5) - 0.23).abs() < 1e-6);
        // Out-of-range progress cannot push the band past the accent's own strength.
        assert!((band_alpha(true, 4.0) - 0.30).abs() < 1e-6);
    }

    #[test]
    fn the_word_box_is_the_advance_plus_the_gap_the_band_covers() {
        let font = 27.72;
        let advance = 100.0;
        let padded = word_box(advance, font);
        assert!((padded - (advance + WORD_GAP * font)).abs() < 1e-4);
        // The gap is padding, so the band is genuinely wider than the ink.
        assert!(padded - advance > 7.7);
    }

    #[test]
    fn the_line_pitch_is_the_inline_boxes_outer_height() {
        let font = 27.72;
        let pitch = line_pitch(font);
        // The band's padding is inside the line box, so the pitch is exactly two ems...
        assert!((pitch - 2.0 * font).abs() < 1e-4, "pitch {pitch}");
        // ... and *not* the leading alone, which is the trap: 45.46 against 55.44.
        let leading = ANSWER_LEADING * font;
        assert!((leading - 45.4608).abs() < 1e-3);
        assert!((pitch - 55.44).abs() < 1e-3);
        assert!(pitch - leading > 9.9);
    }

    #[test]
    fn wrap_breaks_where_the_next_box_would_not_fit() {
        // Four boxes of 10 in a 25-wide measure: two per line.
        assert_eq!(wrap(&[10.0, 10.0, 10.0, 10.0], 25.0), [(0, 2), (2, 4)]);
        // Exactly fitting is a fit, not a break.
        assert_eq!(wrap(&[10.0, 10.0], 20.0), [(0, 2)]);
        // One box wider than the measure still gets its own line rather than being dropped.
        assert_eq!(wrap(&[30.0, 10.0], 25.0), [(0, 1), (1, 2)]);
        assert_eq!(wrap(&[], 25.0), []);
    }

    #[test]
    fn the_gap_padding_counts_towards_the_measure() {
        // Four 100 px advances with a 27.72 px font are 4 * 107.76 = 431 px of boxes: a 430 px
        // measure fits three of them, and the fourth wraps. Ignoring the padding would fit all
        // four, which is the bug the source's own comment warns about.
        let font = 27.72;
        let boxes: Vec<f32> = [100.0; 4].iter().map(|a| word_box(*a, font)).collect();
        assert!((boxes[0] - 107.7616).abs() < 1e-3);
        assert_eq!(wrap(&boxes, 430.0), [(0, 3), (3, 4)]);
        assert_eq!(wrap(&boxes, 440.0), [(0, 4)]);
    }

    #[test]
    fn rounded_rect_only_arcs_where_a_radius_is_set() {
        let box_20 = Rect::new(0.0, 10.0, 100.0, 20.0);
        let one_sided = rounded_rect(box_20, [4.0, 0.0, 0.0, 4.0]);
        assert!(one_sided.starts_with("M 4 10"));
        assert_eq!(one_sided.matches('A').count(), 2);
        assert!(one_sided.ends_with(" Z"));
        let square = rounded_rect(box_20, [0.0; 4]);
        assert_eq!(square.matches('A').count(), 0);
        assert_eq!(square, "M 0 10 L 100 10 L 100 30 L 0 30 L 0 10 Z");
        let stadium = rounded_rect(Rect::new(0.0, 0.0, 40.0, 20.0), [10.0; 4]);
        assert_eq!(stadium.matches('A').count(), 4);
    }

    #[test]
    fn band_stops_put_the_caret_just_past_the_stop_and_inside_the_box() {
        let (stop, end) = band_stops(0.5, 2.0, 100.0);
        assert!((stop - 0.5).abs() < 1e-6);
        assert!((end - 0.52).abs() < 1e-6);
        // Past the end of the box, CSS's `calc` clamps and so does this.
        let (_, end) = band_stops(0.99, 5.0, 100.0);
        assert!((end - 1.0).abs() < 1e-6);
        // With no width to divide by there is no caret at all.
        let (stop, end) = band_stops(0.4, 2.0, 0.0);
        assert!((stop - 0.4).abs() < 1e-6);
        assert!((end - 0.4).abs() < 1e-6);
    }

    #[test]
    fn the_band_id_carries_the_frame_so_the_gradient_cannot_freeze() {
        // `usvgr`'s paint-server cache is keyed by element id and survives into the next frame, so
        // a sweeping gradient with a constant id renders frozen.
        assert_ne!(band_id(3, 20.0), band_id(3, 21.0));
        assert_ne!(band_id(3, 20.0), band_id(4, 20.0));
        assert_eq!(band_id(3, 20.0), band_id(3, 20.4));
        assert_eq!(band_id(3, 20.0), "answer-highlight-band-3-20");
    }

    #[test]
    fn the_schedule_is_the_sources_own_clock() {
        let config = AnswerHighlightConfig::default();
        let words = split_words(&config.answer);
        assert_eq!(words.len(), 36);
        let run = find_run(&words, &config.statement).expect("the statement is in the answer");
        assert_eq!(run, (6, 10));
        let clock = schedule(&config, words.len(), run.1 - run.0);
        assert!((clock.answer_at - 18.0).abs() < 1e-4);
        assert!((clock.word_step - 1.65).abs() < 1e-4);
        assert!((clock.word_in - 6.6).abs() < 1e-4);
        assert!((clock.answer_done - 82.35).abs() < 1e-3);
        assert!((clock.drag_at - 95.85).abs() < 1e-3);
        assert!((clock.drag_step - 3.3).abs() < 1e-4);
        assert_eq!(clock.run_len, 4);
        assert!((clock.drag_done - 109.05).abs() < 1e-3);
        assert!((clock.word_at - 127.05).abs() < 1e-3);
        assert!((clock.word_end - 133.05).abs() < 1e-3);
    }

    #[test]
    fn the_default_copy_settles_on_the_measured_frame() {
        // `registry/__measured__.json` records `settle.base = 132` for this component's shipped
        // default: the last frame at which anything moves. The ramp's own end is 133.05, and the
        // frame before it moves the ink by less than the metric's two-level floor.
        let config = AnswerHighlightConfig::default();
        let words = split_words(&config.answer);
        let run = find_run(&words, &config.statement).expect("the statement is in the answer");
        let clock = schedule(&config, words.len(), run.1 - run.0);
        let ink = config.answer_color.clone().unwrap();
        let accent = config.accent_color.clone().unwrap();
        assert!((clock.settle_frame(&ink, &accent) - 132.0).abs() < 1e-6);
        // Which is a frame short of the schedule's own end — the two numbers are different facts.
        assert!((clock.settle_frame(&ink, &accent) - clock.word_end).abs() > 1.0);
        // The fraction of the ease-out that is visible is where the remaining travel drops under
        // the noise floor, and it sits strictly inside the ramp.
        let visible = visible_ramp_fraction(&ink, &accent);
        assert!((visible - 0.7148).abs() < 1e-3, "visible {visible}");
        assert!(visible < 1.0);
        // Two colours that cannot move a pixel between them are visible from the start.
        assert!((visible_ramp_fraction("#141414", "#141415") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_picked_word_is_only_ever_the_one_inside_the_statement() {
        // `declarative` appears inside the statement, so it is picked.
        let words = split_words(&AnswerHighlightConfig::default().answer);
        let run = find_run(&words, "keep the timeline declarative").unwrap();
        let inner = find_run(&words, "declarative").unwrap();
        assert_eq!(inner, (9, 10));
        assert!(inner.0 >= run.0 && inner.1 <= run.1);
        // An earlier occurrence outside the run is not the one meant.
        let words = split_words("composition per scene and composition inside the run");
        let run = find_run(&words, "composition inside the run").unwrap();
        let inner = find_run(&words, "composition").unwrap();
        assert_eq!(inner, (0, 1));
        assert!(inner.0 < run.0, "the early word is outside the statement");
    }

    /// The lockup, as the renderer draws it, on the default 1920x1080 canvas.
    ///
    /// Real measurement against the crate's own embedded faces, not a stub: this is the same
    /// [`AnswerHighlightLayout::resolve`] the render path calls.
    fn default_layout() -> (AnswerHighlightLayout, String) {
        let meters = crate::embedded_meters();
        let mut frame = fframes::Frame::default();
        let info = ComponentFrame {
            local: 0.0,
            global: 0,
            fps: 30.0,
            duration: 190,
            width: 1920.0,
            height: 1080.0,
        };
        let ctx = RenderCtx::new(&mut frame, Fonts::default(), &meters, info);
        let family = text::resolve_face(None, None, &ctx);
        let layout =
            AnswerHighlightLayout::resolve(&ctx, &AnswerHighlightConfig::default(), &family);
        (layout, family)
    }

    #[test]
    fn the_drawn_lockup_lands_where_the_reference_demo_puts_it() {
        let (layout, family) = default_layout();
        assert_eq!(family, "Inter");
        assert!((layout.unit - 1.5).abs() < 1e-6);

        // The reference demo is 960x540, so one demo pixel is two canvas px at 1080p. Measured off
        // `reference/free/answer-highlight.mp4` on source frame 132 (the settle):
        //
        // * the pill's top edge sits at demo y 151.7 -> canvas 303.4
        // * the answer's four baselines sit at demo y 249.5 / 290.5 / 332.5 / 373.5, i.e.
        //   canvas 499 / 581 / 665 / 747
        // * the answer's ink starts at demo x 192 -> canvas 384
        // * the line pitch is demo 41.6 -> canvas 83.2
        //
        // The tolerance is four canvas px, twice the reading error of the encoded demo.
        assert!(
            (layout.pill.y - 303.4).abs() < 2.0,
            "pill top {} should be near 303.4",
            layout.pill.y
        );
        assert!(
            (layout.lockup_left - 384.0).abs() < 1e-3,
            "left edge {} should be 384",
            layout.lockup_left
        );
        assert_eq!(layout.lines, 4);
        assert!(
            (layout.line_pitch - 83.2).abs() < 2.0,
            "pitch {} should be near 83.2",
            layout.line_pitch
        );
        let first = &layout.words[0];
        assert_eq!(first.text, "Build");

        let measured = [499.0, 581.0, 665.0, 747.0];
        for slot in &layout.words {
            let expected = measured[slot.line];
            assert!(
                (slot.baseline - expected).abs() < 4.0,
                "{} on line {} is drawn at baseline {} rather than near {expected}",
                slot.text,
                slot.line,
                slot.baseline
            );
        }
        // And in reading order the lines only ever step by the pitch, from the first to the last.
        for pair in layout.words.windows(2) {
            if pair[1].line != pair[0].line {
                assert!(
                    (pair[1].baseline - pair[0].baseline - layout.line_pitch).abs() < 1e-3,
                    "line step {} is not the pitch {}",
                    pair[1].baseline - pair[0].baseline,
                    layout.line_pitch
                );
            }
        }
    }

    #[test]
    fn the_band_starts_at_the_words_own_left_edge_not_half_a_word_early() {
        // `text_node` centres a run, `text_node_left` does not; a band drawn from a centred run
        // would sit half a word to the left of its glyphs. This pins the drawn origin.
        let (layout, _family) = default_layout();
        let run = layout.run.expect("the default statement is found");
        assert_eq!(run, (6, 10));
        let slot = layout.words.iter().find(|w| w.index == run.0).unwrap();
        let band = layout.band_box(slot, 0.0);
        assert!((band.x - slot.box_left).abs() < 1e-6);
        assert!((band.right() - (slot.box_left + slot.box_width)).abs() < 1e-6);
        // The run's first word rounds its left corners and its second does not.
        assert!((layout.band_radii(run.0).0 - layout.band_radius).abs() < 1e-6);
        assert!((layout.band_radii(run.0 + 1).0).abs() < 1e-6);
        // The statement's last word rounds its right corners, and it is on the next line.
        let last = layout.words.iter().find(|w| w.index == run.1 - 1).unwrap();
        assert_eq!(last.text, "declarative,");
        assert_eq!(last.line, 1);
        assert!(layout.band_radii(run.1 - 1).1 > 0.0);
        assert!((layout.band_radii(run.0).1).abs() < 1e-6);
        // A word that wraps keeps its left edge at the measure's, so the band breaks square there.
        assert!((last.box_left - layout.lockup_left).abs() < 1e-6);
    }

    #[test]
    fn the_band_covers_the_gap_after_its_last_word() {
        // Measured off the demo: the band over "keep the timeline" runs demo x 561..735, i.e.
        // canvas 1122..1470, and this port's band on "timeline" ends at 1470.56.
        let (layout, _family) = default_layout();
        let timeline = layout
            .words
            .iter()
            .find(|w| w.text == "timeline")
            .expect("timeline is in the answer");
        let band = layout.band_box(timeline, 0.0);
        assert!(
            (band.right() - 1470.0).abs() < 2.0,
            "band right {} should be near 1470",
            band.right()
        );
        // And the run's first word on that line is where the band starts: demo 561 -> canvas 1122.
        let keep = layout
            .words
            .iter()
            .find(|w| w.text == "keep")
            .expect("keep is in the answer");
        assert!(
            (keep.box_left - 1122.0).abs() < 2.0,
            "band left {} should be near 1122",
            keep.box_left
        );
        // Its ink ends 0.28 em before the band does: the gap is inside the band, not outside it.
        assert!(
            (band.right() - (band.x + timeline.advance) - WORD_GAP * layout.font_size).abs() < 1e-3
        );
        // The picked word's band on the next line runs demo 192..311, i.e. canvas 384..622.
        let picked = layout
            .words
            .iter()
            .find(|w| w.index == layout.picked.expect("the picked word").0)
            .expect("the picked word is in the answer");
        assert_eq!(picked.text, "declarative,");
        assert!(
            (picked.box_left + picked.box_width - 622.0).abs() < 4.0,
            "the picked band ends at {} rather than near 622",
            picked.box_left + picked.box_width
        );
    }

    #[test]
    fn the_pill_is_the_questions_advance_plus_its_own_padding() {
        let (layout, _family) = default_layout();
        // Measured off the demo: the pill's own fill runs demo x 192..455 on every row it spans,
        // i.e. canvas 384..910 — 528 wide. This port draws 535.7, 1.5% wider, because the
        // embedded Inter measures the label a shade wider than the browser's Inter did.
        assert!(
            (layout.pill.w - 528.0).abs() < 15.0,
            "pill width {} should be near 528",
            layout.pill.w
        );
        assert!(
            (layout.pill.h - 59.7).abs() < 0.2,
            "pill height {}",
            layout.pill.h
        );
        assert!((layout.pill.x - layout.lockup_left).abs() < 1e-6);
        // The label is inset by the padding, and its baseline sits inside the pill.
        assert!(layout.question_left > layout.pill.x);
        assert!(layout.question_baseline > layout.pill.y);
        assert!(layout.question_baseline < layout.pill.y + layout.pill.h);
        // The answer block starts one pill-height plus the gap below the pill's top.
        assert!(
            (layout.answer_top - (layout.pill.y + layout.pill.h + PILL_GAP * 720.0 * layout.unit))
                .abs()
                < 1e-3
        );
    }
}
