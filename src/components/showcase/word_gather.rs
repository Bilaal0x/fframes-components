//! A sentence arrives out of order before its words gather into place.

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::normalised_speed;
use crate::ui::{Mode, UiTheme, text};

/* ─────────────────────────────────────────────────────────────────────────────────────────────
The card, in the reference's own 710 x 388 pixels
───────────────────────────────────────────────────────────────────────────────────────────── */

/// The card's width, in card px.
pub const REF_W: f32 = 710.0;
/// The card's height, in card px.
pub const REF_H: f32 = 388.0;
/// The card's horizontal middle.
pub const CENTRE_X: f32 = REF_W / 2.0;
/// Where the lines are centred, which is not quite the middle of the card.
///
/// A line is centred on its advance box and read back as its ink, and the two differ by half the
/// difference of the first character's left bearing and the last one's right — a face-dependent
/// three and a half pixels here.
pub const LINE_CX: f32 = REF_W / 2.0 - 3.53;
/// The card's vertical middle.
pub const CENTRE_Y: f32 = REF_H / 2.0;
/// The card's corner radius, in card px, measured off the coverage of its own antialiasing.
pub const RADIUS: f32 = 5.5;
/// The frame the recording runs out on, which is also the composition's own length.
pub const END: f32 = 49.0;

/// The last beat the recording reaches.
pub const BEATS: f32 = 39.0;

/// The recording's own clock, as `(frame, beat)` pairs.
///
/// It was captured at about 24 frames a second with a screen recorder's jitter and this plays at
/// 30, so the two only line up every fifth frame. Every other table here is in *beats* rather than
/// frames for that reason: it is the only way a number measured off frame 23 of the recording can
/// be the number this scene is on when frame 23 of the recording is what it is being compared to.
pub const BEAT: &[(f32, f32)] = &[
    (0.0, 0.0),
    (1.0, 1.0),
    (2.0, 2.0),
    (3.0, 3.0),
    (4.0, 4.0),
    (6.0, 5.0),
    (7.0, 6.0),
    (8.0, 7.0),
    (9.0, 8.0),
    (10.0, 9.0),
    (12.0, 10.0),
    (13.0, 11.0),
    (14.0, 12.0),
    (15.0, 13.0),
    (17.0, 14.0),
    (18.0, 15.0),
    (19.0, 16.0),
    (20.0, 17.0),
    (22.0, 18.0),
    (23.0, 19.0),
    (24.0, 20.0),
    (26.0, 21.0),
    (27.0, 22.0),
    (28.0, 23.0),
    (29.0, 24.0),
    (31.0, 25.0),
    (32.0, 26.0),
    (33.0, 27.0),
    (34.0, 28.0),
    (35.0, 29.0),
    (37.0, 30.0),
    (38.0, 31.0),
    (39.0, 32.0),
    (41.0, 33.0),
    (42.0, 34.0),
    (43.0, 35.0),
    (44.0, 36.0),
    (45.0, 37.0),
    (47.0, 38.0),
    (48.0, 39.0),
];

/// Linear read of a `(key, value)` table, clamped at both ends.
///
/// This is the source's own `track`, and it is the only interpolator in the component.
#[must_use]
pub fn track(table: &[(f32, f32)], at: f32) -> f32 {
    let (Some(&first), Some(&last)) = (table.first(), table.last()) else {
        return 0.0;
    };
    if at <= first.0 {
        return first.1;
    }
    if at >= last.0 {
        return last.1;
    }
    for pair in table.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if at <= b.0 {
            let t = (at - a.0) / (b.0 - a.0);
            return a.1 + (b.1 - a.1) * t;
        }
    }
    last.1
}

/* ─────────────────────────────────────────────────────────────────────────────────────────────
Type
───────────────────────────────────────────────────────────────────────────────────────────── */

/// Cap height of the settled block, in card px, at scale 1.
pub const CAP: f32 = 35.82;
/// Line spacing of the settled block, in card px, at scale 1.
pub const LINE_H: f32 = 48.18;
/// How far the sentence block sits below where the lead word sat alone, in card px.
///
/// Measured, and not a rounding error: the lead word's cap box is centred on the card to within a
/// tenth of a pixel, and the two-line block that replaces it sits nearly six pixels lower than the
/// same rule would put it. Left out, every baseline in the scene is six pixels high.
pub const DROP: f32 = 5.77;
/// The face's cap height, which is what [`CAP`] is turned into a font size with.
pub const CAP_RATIO: f32 = 0.74909;
/// The face's own hhea metrics: at `lineHeight: LEADING` the baseline is this far down the box.
///
/// Recorded for provenance. The source needs it to turn a baseline into a CSS `top`; SVG positions
/// text by its baseline, so the draw path never uses it.
pub const ASC: f32 = 0.9527;
/// The source's CSS `line-height`, which is the box [`ASC`] is measured against.
///
/// Recorded for provenance, for the same reason as [`ASC`].
pub const LEADING: f32 = 1.2103;
/// Letter spacing, in em. Negative, and it lands after every character including the last.
pub const TRACKING: f32 = -0.055;
/// An extra trim on the space, in em, on top of the tracking.
///
/// The reference sets its words 0.15 of a cap height apart. The face's own space is more than twice
/// that, and tracking tight enough to close it would take the letters with it — so the two are
/// separate numbers, which is what a typesetter would call word-spacing and letter-spacing and is
/// exactly the distinction.
pub const SPACE_TRIM: f32 = -0.0549;

/* ─────────────────────────────────────────────────────────────────────────────────────────────
The clock
───────────────────────────────────────────────────────────────────────────────────────────── */

/// The beat the second word drops in on.
pub const FIRST_AT: f32 = 20.0;
/// The gap, in beats, between one word dropping in and the next.
pub const STAGGER: f32 = 1.0;
/// How many beats a word stays in the accent colour after it drops in.
pub const SETTLE: f32 = 4.0;
/// The lead word starts this far above its own resting place, in card px.
pub const LEAD_RISE: f32 = 20.6;

/// How far a word has travelled from where it dropped in to its slot, against beats since it
/// dropped.
///
/// It is an exponential settle — about 0.7 of the remaining distance left every beat — and the five
/// words that do it are within 0.07 of each other on the first beat and 0.02 by the fourth, which
/// is the capture's sub-frame jitter and not five different curves.
pub const TRAVEL: &[(f32, f32)] = &[
    (0.0, 0.0),
    (1.0, 0.391),
    (2.0, 0.563),
    (3.0, 0.686),
    (4.0, 0.77),
    (5.0, 0.836),
    (6.0, 0.887),
    (7.0, 0.928),
    (8.0, 0.96),
    (9.0, 0.982),
    (10.0, 0.991),
    (12.0, 0.996),
    (14.0, 1.0),
];

/// The lead word sliding across the line, against beats.
///
/// Two tables because the reference has two moves. [`LEAD_X`] is the line reflowing — an ease *in*
/// and out, twenty-one beats long, which is what a line that has just been told it is about to get
/// five more words does. [`LEAD_Y`] is the block going from one line to two, and it is over in six.
pub const LEAD_X: &[(f32, f32)] = &[
    (10.0, 0.0),
    (11.0, 0.004),
    (12.0, 0.007),
    (13.0, 0.02),
    (14.0, 0.036),
    (15.0, 0.063),
    (16.0, 0.107),
    (17.0, 0.18),
    (18.0, 0.317),
    (19.0, 0.547),
    (20.0, 0.701),
    (21.0, 0.793),
    (22.0, 0.852),
    (23.0, 0.892),
    (24.0, 0.921),
    (25.0, 0.946),
    (26.0, 0.959),
    (27.0, 0.97),
    (28.0, 0.98),
    (29.0, 0.99),
    (30.0, 0.992),
    (31.0, 1.0),
];

/// The lead word dropping onto its line, against beats.
pub const LEAD_Y: &[(f32, f32)] = &[
    (0.0, 0.0),
    (1.0, 0.131),
    (2.0, 0.211),
    (3.0, 0.27),
    (4.0, 0.329),
    (5.0, 0.35),
    (6.0, 0.391),
    (7.0, 0.411),
    (8.0, 0.4341),
    (9.0, 0.47),
    (10.0, 0.514),
    (11.0, 0.612),
    (12.0, 0.817),
    (13.0, 0.905),
    (14.0, 0.949),
    (15.0, 0.985),
    (16.0, 1.0),
];

/// The block's own scale, against beats.
///
/// It holds at 1.289 while the words are still coming and then falls to 1 over ten beats, with the
/// whole middle of that fall inside two of them. The first eighteen are flat because they are
/// measured off one word's ink width and wobble a third of a percent; the fall is every word at
/// once and does not.
pub const SCALE: &[(f32, f32)] = &[
    (0.0, 1.289),
    (18.0, 1.289),
    (19.0, 1.2798),
    (20.0, 1.2782),
    (21.0, 1.2727),
    (22.0, 1.264),
    (23.0, 1.2546),
    (24.0, 1.2414),
    (25.0, 1.2184),
    (26.0, 1.159),
    (27.0, 1.0689),
    (28.0, 1.0417),
    (29.0, 1.0275),
    (30.0, 1.0193),
    (31.0, 1.0132),
    (32.0, 1.0081),
    (33.0, 1.0064),
    (34.0, 1.0),
    (39.0, 1.0),
];

/// Where each word drops in, relative to its own slot, in card px.
///
/// Measured off the frame each word first appears on, and indexed by the word's place in the
/// sentence rather than by the order it drops in. There is no pattern in them and there is not
/// meant to be — three of the five come in flat from the side and two fall past their line — so
/// they are a table and not a formula, and a sentence longer than the reference's simply reuses
/// them.
pub const SCATTER: &[(f32, f32)] = &[
    (44.4, -59.2),
    (73.8, -1.0),
    (-31.7, -1.7),
    (12.4, 41.0),
    (96.2, -1.7),
];

/// The order the words drop in, as offsets into the sentence.
///
/// The last word first, then the third, the first, the fourth, the second — so the sentence is
/// never briefly readable and never briefly wrong, it is just scattered until it is not.
pub const ORDER: &[usize] = &[5, 3, 1, 4, 2];

/* ─────────────────────────────────────────────────────────────────────────────────────────────
The drop order
───────────────────────────────────────────────────────────────────────────────────────────── */

/// Add `index` to the order if it is a word after the lead and has not been taken yet.
fn take(n: usize, out: &mut Vec<usize>, seen: &mut [bool], index: usize) {
    if index >= 1 && index <= n && !seen[index] {
        seen[index] = true;
        out.push(index);
    }
}

/// The order `n` words after the lead drop in, as offsets into the sentence.
///
/// The reference's own order where it fits, folded into range where it does not, and then whatever
/// is left in reading order. It has to be a permutation — one repeated index is a word that never
/// drops and a hole in the sentence — which is why it is built as a list and not as arithmetic.
#[must_use]
pub fn drop_order(n: usize) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen = vec![false; n + 1];
    for &seed in ORDER {
        let folded = if seed <= n { seed } else { (seed - 1) % n + 1 };
        take(n, &mut out, &mut seen, folded);
    }
    for index in 1..=n {
        take(n, &mut out, &mut seen, index);
    }
    out
}

/// Which word drops in `k`th, for `n` words after the lead.
#[must_use]
pub fn order_at(k: usize, n: usize) -> usize {
    drop_order(n).get(k).copied().unwrap_or(k + 1)
}

/// Which drop-in slot word `index` of the sentence is on, for `n` words after the lead.
///
/// The lead word is never in the order — it is already on the card — so an index the order does not
/// contain falls back to its reading position, which is what the source does.
#[must_use]
pub fn drop_slot(index: usize, n: usize) -> usize {
    match drop_order(n).iter().position(|&i| i == index) {
        Some(slot) => slot,
        None => index.saturating_sub(1),
    }
}

/// One word's arrival: when it drops in and when it stops wearing the accent colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arrival {
    /// The beat the word first appears on.
    pub drop: f32,
    /// The beat it stops wearing the accent colour on. It is still travelling here.
    pub accent_until: f32,
}

/// The arrival window of word `index` of a `word_count`-word sentence.
///
/// `None` for the lead word, which is already on the card, and for anything past the end of the
/// sentence.
#[must_use]
pub fn arrival(index: usize, word_count: usize) -> Option<Arrival> {
    if index == 0 || index >= word_count {
        return None;
    }
    let drop = FIRST_AT + STAGGER * drop_slot(index, word_count - 1) as f32;
    Some(Arrival {
        drop,
        accent_until: drop + SETTLE,
    })
}

/// The beat the last word of a `word_count`-word sentence drops in on.
///
/// `None` when there is nothing to drop: the lead word alone is the whole sentence.
#[must_use]
pub fn final_drop_beat(word_count: usize) -> Option<f32> {
    if word_count < 2 {
        return None;
    }
    Some(FIRST_AT + STAGGER * (word_count as f32 - 2.0))
}

/// How far home a word is `since_drop` beats after it dropped in.
#[must_use]
pub fn travel(since_drop: f32) -> f32 {
    track(TRAVEL, since_drop)
}

/// The offset word `index` drops in from, in card px, out of [`SCATTER`].
#[must_use]
pub fn scatter(index: usize) -> (f32, f32) {
    let at = index.saturating_sub(1) % SCATTER.len();
    SCATTER[at]
}

/// The block's own scale at `beat`.
#[must_use]
pub fn block_scale(beat: f32) -> f32 {
    track(SCALE, beat)
}

/* ─────────────────────────────────────────────────────────────────────────────────────────────
Layout
───────────────────────────────────────────────────────────────────────────────────────────── */

/// One word's slot: where its advance origin sits and which baseline it sits on, in card px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slot {
    /// The word's left edge — its advance origin — in card px.
    pub left: f32,
    /// The word's baseline, in card px.
    pub baseline: f32,
}

/// The sentence, wrapped greedily and centred line by line.
///
/// `max_width` is in card px at scale 1, so it is the same number whatever the block's scale is
/// doing — which is the only reason the line break holds still while the block shrinks by a third.
#[must_use]
pub fn solve(widths: &[f32], space: f32, max_width: f32) -> Vec<Slot> {
    let mut lines: Vec<Vec<usize>> = vec![Vec::new()];
    let mut run = 0.0_f32;
    for (index, width) in widths.iter().copied().enumerate() {
        let last = lines.len() - 1;
        let add = if lines[last].is_empty() { 0.0 } else { space } + width;
        if !lines[last].is_empty() && run + add > max_width {
            lines.push(vec![index]);
            run = width;
        } else {
            lines[last].push(index);
            run += add;
        }
    }

    let top = CENTRE_Y + CAP / 2.0 + DROP - ((lines.len() as f32 - 1.0) * LINE_H) / 2.0;
    let mut out = vec![
        Slot {
            left: 0.0,
            baseline: top,
        };
        widths.len()
    ];
    for (line_index, line) in lines.iter().enumerate() {
        let mut total = 0.0_f32;
        for &index in line {
            total += widths[index];
        }
        total += space * (line.len() as f32 - 1.0);
        let mut x = LINE_CX - total / 2.0;
        for &index in line {
            out[index] = Slot {
                left: x,
                baseline: top + line_index as f32 * LINE_H,
            };
            x += widths[index] + space;
        }
    }
    out
}

/// One word's place on the card at a beat, at the block's scale of 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordPlacement {
    /// The word's left edge — its advance origin — in card px.
    pub left: f32,
    /// The word's baseline, in card px.
    pub baseline: f32,
    /// Whether the word is still wearing the accent colour.
    pub travelling: bool,
}

/// Where word `index` is at `beat`, or `None` while it is not on the card.
///
/// The lead word is always on the card and moves on two clocks at once: it drops onto its line over
/// [`LEAD_Y`] and slides left across it over [`LEAD_X`]. Every other word is absent until its own
/// [`Arrival::drop`], and from there it closes the [`scatter`] offset over [`TRAVEL`].
#[must_use]
pub fn word_placement(
    beat: f32,
    index: usize,
    word_count: usize,
    advances: &[f32],
    slots: &[Slot],
) -> Option<WordPlacement> {
    let slot = *slots.get(index)?;

    if index == 0 {
        let alone_left = LINE_CX - advances.first().copied().unwrap_or(0.0) / 2.0;
        let alone_baseline = CENTRE_Y + CAP / 2.0;
        let lx = track(LEAD_X, beat);
        let ly = track(LEAD_Y, beat);
        return Some(WordPlacement {
            left: slot.left + (alone_left - slot.left) * (1.0 - lx),
            baseline: slot.baseline + (alone_baseline - slot.baseline + LEAD_RISE) * (1.0 - ly),
            travelling: false,
        });
    }

    let arrival = arrival(index, word_count)?;
    if beat < arrival.drop {
        return None;
    }
    let p = travel(beat - arrival.drop);
    let (dx, dy) = scatter(index);
    Some(WordPlacement {
        left: slot.left + dx * (1.0 - p),
        baseline: slot.baseline + dy * (1.0 - p),
        travelling: beat < arrival.accent_until,
    })
}

/// The scale that makes the 710x388 card cover the canvas — `cover`, so a 16:9 composition crops
/// the card rather than letterboxing it.
#[must_use]
pub fn card_scale(width: f32, height: f32) -> f32 {
    (width / REF_W).max(height / REF_H)
}

/// Where the card's top-left corner lands once it is `cover`-scaled and centred.
#[must_use]
pub fn card_origin(width: f32, height: f32) -> (f32, f32) {
    let k = card_scale(width, height);
    ((width - REF_W * k) / 2.0, (height - REF_H * k) / 2.0)
}

/* ─────────────────────────────────────────────────────────────────────────────────────────────
The component
───────────────────────────────────────────────────────────────────────────────────────────── */

/// The sentence's measured advances and its word gap, cached across frames.
///
/// Card px, so they do not depend on the canvas: the card is a fixed 710x388 surface that is scaled
/// onto whatever canvas it lands on. They are measured once because neither the font size
/// (`CAP / CAP_RATIO`) nor the tracking depends on the canvas either.
#[derive(Debug, Clone, PartialEq)]
struct Metrics {
    /// Each word's advance width including the tracking that lands after its last character.
    advances: Vec<f32>,
    /// The advance of the gap between two words, including tracking and the word-space trim.
    space: f32,
}

/// Configuration for [`WordGather`].
#[derive(Debug, Clone, PartialEq)]
pub struct WordGatherConfig {
    /// The sentence. The first word is the one that holds the frame alone.
    pub text: String,
    /// Where the line breaks, in card px at scale 1. Wider is fewer lines; the reference breaks a
    /// six-word sentence into two.
    pub max_width: f32,
    /// The colour a word wears while it is still travelling. `None` takes the theme's `primary`.
    pub accent: Option<String>,
    /// Font weight.
    pub font_weight: u16,
    /// The face to paint in. `None` keeps the theme's face, then the crate's embedded sans face.
    pub font_family: Option<String>,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// Timeline multiplier. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Composition length in frames.
    pub duration: usize,
}

impl Default for WordGatherConfig {
    fn default() -> Self {
        Self {
            text: "Every scene is yours to own.".to_owned(),
            max_width: 330.0,
            accent: None,
            font_weight: 500,
            font_family: None,
            theme: None,
            mode: Mode::Light,
            speed: 1.0,
            duration: 49,
        }
    }
}

/// The **Word Gather** component.
#[derive(Debug, Clone, Default)]
pub struct WordGather {
    /// Component options.
    pub config: WordGatherConfig,
    metrics: OnceLock<Metrics>,
}

impl WordGather {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: WordGatherConfig) -> Self {
        Self {
            config,
            metrics: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "word-gather",
            name: "Word Gather",
            summary: "A sentence arrives out of order, scatters into place and gathers",
            feature: "word-gather",
        }
    }

    /// The words of the sentence, split the way the source splits them.
    fn words(&self) -> Vec<&str> {
        self.config.text.split_whitespace().collect()
    }

    /// The measured advances and space, cached across frames.
    fn measure(&self, ctx: &RenderCtx<'_>, family: &str) -> &Metrics {
        self.metrics.get_or_init(|| {
            let size = CAP / CAP_RATIO;
            let tracking_px = TRACKING * size;
            Metrics {
                // `letter-spacing` lands after every character, the last one included, so a
                // word's advance grows by its whole length and the space by one.
                advances: self
                    .words()
                    .iter()
                    .map(|word| {
                        text::tracked_width(ctx, family, size, word, tracking_px) + tracking_px
                    })
                    .collect(),
                space: ctx.space_width(family, size) + (TRACKING + SPACE_TRIM) * size,
            }
        })
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

impl Component for WordGather {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source paints no backdrop of its own; its own full-bleed fill is the theme's
        // `background`, which is this colour in the default light palette.
        Some("#faf9f6")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let ink = theme.foreground.clone();
        // The customiser hands an unset colour control through as `""`, not `undefined`.
        let lit = config
            .accent
            .clone()
            .filter(|accent| !accent.is_empty())
            .unwrap_or_else(|| theme.primary.clone());
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);

        // Everything below is in the card's own pixels: the card is a fixed 710x388 surface that
        // `cover`-scales onto the canvas, so the layout never sees the canvas at all.
        let size = CAP / CAP_RATIO;
        let tracking_px = TRACKING * size;
        let metrics = self.measure(ctx, &family);
        let slots = solve(&metrics.advances, metrics.space, config.max_width);

        // And everything below that is on the recording's clock, not the composition's.
        let frame = ctx.frame() * normalised_speed(config.speed);
        let beat = track(BEAT, frame);
        let scale = block_scale(beat);

        let words = self.words();
        let mut nodes: Vec<Svgr<'static>> = Vec::new();
        for (index, word) in words.iter().enumerate() {
            let Some(placement) =
                word_placement(beat, index, words.len(), &metrics.advances, &slots)
            else {
                continue;
            };
            let Some(&advance) = metrics.advances.get(index) else {
                continue;
            };
            let fill = if placement.travelling {
                lit.clone()
            } else {
                ink.clone()
            };
            // `text_node` centres a run from a width it measures itself; its centre is the run's
            // left edge plus half of that, which puts the advance origin on the slot.
            let run = advance - tracking_px;
            nodes.push(text::text_node(
                ctx,
                word,
                placement.left + run / 2.0,
                placement.baseline,
                family.clone(),
                size,
                config.font_weight,
                tracking_px,
                fill,
                1.0,
            ));
        }

        // The block's own scale is one transform over the solved layout, pivoted on the card's
        // middle: every word's own scale pivots on its baseline, and those are all images of the
        // same point under the same uniform scale.
        let nodes = Svgr::from(nodes);
        let nodes = scaled_about(nodes, CENTRE_X, CENTRE_Y, scale);

        let (origin_x, origin_y) = card_origin(ctx.width(), ctx.height());
        let card = card_scale(ctx.width(), ctx.height());
        let transform = format!("translate({origin_x} {origin_y}) scale({card})");
        fframes::svgr!(
            <g transform={transform}>
                <rect x="0" y="0" width={REF_W} height={REF_H} rx={RADIUS}
                      fill={theme.background.clone()} opacity={1.0} />
                {nodes}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The widths the source's own test uses, for the layout arithmetic.
    const TEST_WIDTHS: [f32; 6] = [88.0, 89.0, 113.0, 85.0, 104.0, 73.0];

    /// A word's place, with a zeroed fallback so a test never panics on a missing one.
    fn placed(
        beat: f32,
        index: usize,
        words: usize,
        advances: &[f32],
        slots: &[Slot],
    ) -> WordPlacement {
        word_placement(beat, index, words, advances, slots).unwrap_or(WordPlacement {
            left: 0.0,
            baseline: 0.0,
            travelling: false,
        })
    }

    #[test]
    fn duration_defaults_to_the_source_composition_length() {
        assert_eq!(WordGather::default().duration(), 49);
        assert_eq!(WordGather::default().meta().id, "word-gather");
        assert_eq!(WordGather::meta_static().feature, "word-gather");
    }

    #[test]
    fn the_source_defaults_are_reproduced_field_by_field() {
        let config = WordGatherConfig::default();
        assert_eq!(config.text, "Every scene is yours to own.");
        assert!((config.max_width - 330.0).abs() < 1e-6);
        assert_eq!(config.accent, None);
        assert_eq!(config.font_weight, 500);
        assert_eq!(config.font_family, None);
        assert_eq!(config.theme, None);
        assert_eq!(config.mode, Mode::Light);
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.duration, 49);
    }

    #[test]
    fn the_composition_length_is_the_recording_s_own_end() {
        // `registry/__measured__.json` records settle.base = 49 for this component, and the source
        // pins that to its own `END`: the clock's last entry is the last frame of `END`.
        let config = WordGatherConfig::default();
        assert!((END - config.duration as f32).abs() < 1e-6);
        let last = BEAT.last().copied().unwrap_or((0.0, 0.0));
        assert!((last.0 - (END - 1.0)).abs() < 1e-6, "last frame {}", last.0);
        assert!((last.1 - BEATS).abs() < 1e-6);
    }

    #[test]
    fn the_clock_runs_forward_and_is_slower_than_the_composition() {
        let mut last = -1.0_f32;
        for &(frame, beat) in BEAT {
            assert!(beat > last, "beat {beat} after {last}");
            assert!(frame >= beat, "frame {frame} behind beat {beat}");
            last = beat;
        }

        let mut previous = -1.0_f32;
        for step in 0..=49 {
            let beat = track(BEAT, step as f32);
            assert!(
                beat >= previous - 1e-9,
                "the clock went backwards at {step}"
            );
            previous = beat;
        }
        assert!((track(BEAT, END + 20.0) - BEATS).abs() < 1e-6);
        assert!((track(BEAT, -5.0)).abs() < 1e-6);

        // 39 beats over 48 frames is 24 to the second against 30 — that is the ratio.
        let ratio = BEATS / (END - 1.0);
        assert!(ratio > 0.78 && ratio < 0.83, "ratio {ratio}");
    }

    #[test]
    fn drop_order_is_a_permutation_of_the_words_after_the_lead() {
        for n in [1_usize, 2, 3, 4, 5, 6, 7, 9, 12] {
            let order = drop_order(n);
            assert_eq!(order.len(), n, "n = {n}");
            let mut sorted = order.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted, (1..=n).collect::<Vec<_>>(), "n = {n}");
            for &index in &order {
                assert!(
                    index >= 1 && index <= n,
                    "index {index} out of range for {n}"
                );
            }
            // The lead word is never one of them; it is already on the card.
            assert!(!order.contains(&0));
        }
        assert!(drop_order(0).is_empty());
    }

    #[test]
    fn the_reference_s_own_order_survives_a_five_word_tail() {
        assert_eq!(drop_order(5), vec![5, 3, 1, 4, 2]);
        assert_eq!(drop_order(5), ORDER.to_vec());
        for (k, &expected) in ORDER.iter().enumerate() {
            assert_eq!(order_at(k, 5), expected);
        }
        // A pair the order cannot be read out in: it never opens with the second word.
        assert_ne!(ORDER[0], 1);
        // Past the end of the tail, `order_at` falls back to reading order.
        assert_eq!(order_at(9, 5), 10);
    }

    #[test]
    fn drop_slots_place_every_word_on_its_own_beat() {
        // `own.` is word 5 and drops first, `is` is word 2 and drops last.
        assert_eq!(drop_slot(5, 5), 0);
        assert_eq!(drop_slot(3, 5), 1);
        assert_eq!(drop_slot(1, 5), 2);
        assert_eq!(drop_slot(4, 5), 3);
        assert_eq!(drop_slot(2, 5), 4);
        // The lead word is never in the order, so it falls back to its reading position.
        assert_eq!(drop_slot(0, 5), 0);
    }

    #[test]
    fn arrival_windows_follow_the_source_schedule() {
        // Six words: five after the lead, one beat apart from beat 20.
        assert_eq!(arrival(0, 6), None);
        assert_eq!(arrival(6, 6), None);
        assert_eq!(
            arrival(5, 6),
            Some(Arrival {
                drop: 20.0,
                accent_until: 24.0
            })
        );
        assert_eq!(
            arrival(3, 6),
            Some(Arrival {
                drop: 21.0,
                accent_until: 25.0
            })
        );
        assert_eq!(
            arrival(1, 6),
            Some(Arrival {
                drop: 22.0,
                accent_until: 26.0
            })
        );
        assert_eq!(
            arrival(4, 6),
            Some(Arrival {
                drop: 23.0,
                accent_until: 27.0
            })
        );
        assert_eq!(
            arrival(2, 6),
            Some(Arrival {
                drop: 24.0,
                accent_until: 28.0
            })
        );

        // A one-word sentence has nothing to drop.
        assert_eq!(arrival(0, 1), None);
        assert_eq!(final_drop_beat(1), None);
        assert_eq!(final_drop_beat(2), Some(20.0));

        // Every word is down, and done wearing the accent, before the recording runs out.
        assert!((final_drop_beat(6).unwrap_or(0.0) + SETTLE) < BEATS);
    }

    #[test]
    fn the_travel_starts_where_it_dropped_and_reaches_home() {
        assert!((travel(0.0)).abs() < 1e-6);
        assert!((travel(-5.0)).abs() < 1e-6);
        assert!((travel(14.0) - 1.0).abs() < 1e-6);
        assert!((travel(500.0) - 1.0).abs() < 1e-6);

        // An exponential settle: never faster later than it was earlier, and more than a third of
        // the way home on its first beat.
        assert!(travel(1.0) > 0.35);
        let mut previous = f32::INFINITY;
        for beat in 0..14 {
            let step = travel(beat as f32 + 1.0) - travel(beat as f32);
            assert!(step >= -1e-9);
            assert!(step <= previous + 1e-9, "sped up at beat {beat}");
            previous = step;
        }
        // Still short of home one beat before the table's last entry, and home on it.
        assert!(travel(13.0) < 1.0);
        assert!((travel(14.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_lead_word_has_two_clocks_and_the_second_is_the_slow_one() {
        let y_done = LEAD_Y.last().copied().unwrap_or((0.0, 0.0)).0;
        let x_done = LEAD_X.last().copied().unwrap_or((0.0, 0.0)).0;
        // The drop onto the line is over in six beats; the slide along it takes twenty-one.
        assert!(y_done < x_done - 10.0, "y {y_done}, x {x_done}");
        assert!(track(LEAD_X, y_done) < 0.15);
        // And the slide eases *in* and out rather than out of the gate.
        assert!(track(LEAD_X, 12.0) - track(LEAD_X, 11.0) < 0.02);
        assert!(track(LEAD_X, 19.0) - track(LEAD_X, 18.0) > 0.2);
        assert!(track(LEAD_X, 29.0) - track(LEAD_X, 28.0) < 0.02);

        // Neither clock ever runs backwards, and both arrive.
        for table in [LEAD_X, LEAD_Y] {
            let mut last = -1.0_f32;
            for beat in 0..=39 {
                let value = track(table, beat as f32);
                assert!(value >= last - 1e-9, "went backwards at {beat}");
                last = value;
            }
            assert!((track(table, BEATS) - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn the_block_holds_big_then_only_shrinks() {
        assert!(block_scale(0.0) > 1.25);
        assert!((block_scale(18.0) - block_scale(0.0)).abs() < 1e-6);
        // It waits for the words: shrink while they are still arriving and they land in the wrong
        // place.
        assert!((block_scale(FIRST_AT - 2.0) - block_scale(0.0)).abs() < 1e-6);

        let mut last = f32::INFINITY;
        for beat in 0..=39 {
            let value = block_scale(beat as f32);
            assert!(value <= last + 1e-9, "grew at beat {beat}");
            last = value;
        }
        assert!((block_scale(BEATS) - 1.0).abs() < 1e-6);
        assert!((block_scale(1e6) - 1.0).abs() < 1e-6);
        assert!((block_scale(-1.0) - 1.289).abs() < 1e-6);
    }

    #[test]
    fn the_scatter_has_one_offset_for_every_word_the_reference_drops() {
        assert_eq!(SCATTER.len(), ORDER.len());
        for &(dx, dy) in SCATTER {
            // Far enough to read as scattered, and not off the card.
            assert!(dx.hypot(dy) > 25.0, "({dx}, {dy}) is not scattered");
            assert!(dx.abs() < 200.0 && dy.abs() < 120.0);
        }
        // Some come in sideways and some fall past their line.
        assert!(SCATTER.iter().filter(|(_, dy)| dy.abs() > 20.0).count() > 1);
        assert!(SCATTER.iter().filter(|(_, dy)| dy.abs() < 5.0).count() > 1);
        // A sentence longer than the reference's simply reuses them.
        assert_eq!(scatter(1), SCATTER[0]);
        assert_eq!(scatter(5), SCATTER[4]);
        assert_eq!(scatter(6), SCATTER[0]);
    }

    #[test]
    fn solve_wraps_greedily_and_centres_every_line() {
        let slots = solve(&TEST_WIDTHS, 10.0, 330.0);
        assert_eq!(slots.len(), 6);

        // Line one: 88 + 10 + 89 + 10 + 113 = 310 fits; adding 85 would be 405.
        let line_one: Vec<&Slot> = slots
            .iter()
            .filter(|slot| (slot.baseline - slots[0].baseline).abs() < 1e-4)
            .collect();
        assert_eq!(line_one.len(), 3);
        let left = slots[0].left;
        let right = slots[2].left + TEST_WIDTHS[2];
        assert!(((left + right) / 2.0 - LINE_CX).abs() < 1e-4);

        // Line two is 85 + 10 + 104 + 10 + 73 = 282, centred on the same axis.
        let left = slots[3].left;
        let right = slots[5].left + TEST_WIDTHS[5];
        assert!(((left + right) / 2.0 - LINE_CX).abs() < 1e-4);
    }

    #[test]
    fn solve_puts_the_lines_a_line_height_apart_and_the_block_a_measured_hair_low() {
        let slots = solve(&TEST_WIDTHS, 10.0, 330.0);
        assert!((slots[3].baseline - slots[0].baseline - LINE_H).abs() < 1e-4);
        for pair in slots.windows(2) {
            assert!(pair[1].baseline >= pair[0].baseline - 1e-6);
        }
        // The block is centred on the card a measured five and a bit pixels low.
        let mid = (slots[0].baseline + slots[3].baseline) / 2.0;
        assert!((mid - (CENTRE_Y + CAP / 2.0 + DROP)).abs() < 1e-4);

        // One line sits on the same rule with no line to make room for.
        let one = solve(&TEST_WIDTHS, 10.0, 10_000.0);
        for slot in &one {
            assert!((slot.baseline - (CENTRE_Y + CAP / 2.0 + DROP)).abs() < 1e-4);
        }
        // And an empty sentence still lays out.
        assert!(solve(&[], 10.0, 330.0).is_empty());
    }

    #[test]
    fn solve_gives_a_word_wider_than_the_measure_a_line_of_its_own() {
        let slots = solve(&[500.0, 40.0], 10.0, 100.0);
        assert_eq!(slots.len(), 2);
        assert!(slots[1].baseline > slots[0].baseline);
        // Both are alone on their line, so both are centred on it — the wide one simply reaches
        // further either side.
        assert!((slots[0].left - (LINE_CX - 500.0 / 2.0)).abs() < 1e-4);
        assert!((slots[1].left - (LINE_CX - 40.0 / 2.0)).abs() < 1e-4);
    }

    #[test]
    fn the_lead_word_starts_a_lead_rise_above_its_resting_place() {
        let advances = [88.0, 89.0, 113.0];
        let slots = solve(&advances, 10.0, 330.0);
        let start = placed(0.0, 0, 3, &advances, &slots);
        let alone_left = LINE_CX - advances[0] / 2.0;
        let alone_baseline = CENTRE_Y + CAP / 2.0;
        assert!((start.left - alone_left).abs() < 1e-4);
        assert!((start.baseline - (alone_baseline + LEAD_RISE)).abs() < 1e-4);
        assert!(!start.travelling);

        // By the time both clocks are done it is on its slot, wherever that is.
        let settled = placed(39.0, 0, 3, &advances, &slots);
        assert!((settled.left - slots[0].left).abs() < 1e-4);
        assert!((settled.baseline - slots[0].baseline).abs() < 1e-4);
    }

    #[test]
    fn a_later_word_waits_then_closes_its_scatter() {
        let advances = [88.0, 89.0, 113.0, 85.0, 104.0, 73.0];
        let slots = solve(&advances, 10.0, 330.0);

        // Word 5 of 6 drops on beat 20 and is not there a beat before it.
        assert!(word_placement(19.9, 5, 6, &advances, &slots).is_none());
        let dropped = placed(20.0, 5, 6, &advances, &slots);
        assert!((dropped.left - (slots[5].left + SCATTER[4].0)).abs() < 1e-4);
        assert!((dropped.baseline - (slots[5].baseline + SCATTER[4].1)).abs() < 1e-4);
        assert!(dropped.travelling, "it is still in the accent colour");

        // It is still the accent colour on the last beat of the settle, and ink from the next one.
        assert!(placed(23.9, 5, 6, &advances, &slots).travelling);
        assert!(!placed(24.0, 5, 6, &advances, &slots).travelling);
        // ...while still short of home: the accent lets go before the travel does.
        assert!(travel(SETTLE) > 0.6 && travel(SETTLE) < 1.0);

        // And it closes the offset without ever overshooting it, on both axes at once.
        let mut previous = f32::NEG_INFINITY;
        for step in 0..=80 {
            let beat = 20.0 + step as f32 * 0.5;
            let gone = travel(beat - 20.0);
            assert!(gone >= previous - 1e-9, "travel went backwards at {beat}");
            previous = gone;
            let at = placed(beat, 5, 6, &advances, &slots);
            let dx = at.left - slots[5].left;
            let dy = at.baseline - slots[5].baseline;
            assert!(
                dx.abs() <= SCATTER[4].0.abs() + 1e-4,
                "x overshot at {beat}"
            );
            assert!(
                dy.abs() <= SCATTER[4].1.abs() + 1e-4,
                "y overshot at {beat}"
            );
            if gone >= 1.0 {
                assert!((dx).abs() < 1e-4 && (dy).abs() < 1e-4, "not home at {beat}");
            }
        }

        // A word past the end of the sentence is never placed.
        assert!(word_placement(48.0, 6, 6, &advances, &slots).is_none());
    }

    #[test]
    fn the_card_covers_the_canvas() {
        // `cover`, so the card crops a 16:9 composition rather than letterboxing it.
        let k = card_scale(1920.0, 1080.0);
        assert!((k - 1080.0 / REF_H).abs() < 1e-4, "k = {k}");
        // The same number as the source's 1280x720 canvas times the crate's 1.5 design unit: at
        // 720 tall the card's `cover` scale is 720/388, and this canvas is 1.5 times that.
        assert!((k - 1.5 * (720.0 / REF_H)).abs() < 1e-4);
        let (ox, oy) = card_origin(1920.0, 1080.0);
        assert!(
            (oy).abs() < 1e-4,
            "the card's height lands exactly on the canvas"
        );
        assert!((ox - (1920.0 - REF_W * k) / 2.0).abs() < 1e-4);
        assert!(ox < 0.0 && REF_W * k > 1920.0);

        // A wider canvas flips which axis is the tight one, and the card's middle stays put.
        let k = card_scale(4000.0, 800.0);
        assert!((k - 4000.0 / REF_W).abs() < 1e-4);
        let (ox, oy) = card_origin(4000.0, 800.0);
        assert!(
            (ox).abs() < 1e-4,
            "the card's width lands exactly on the canvas"
        );
        assert!(oy < 0.0 && REF_H * k > 800.0);
        assert!((CENTRE_X * k + ox - 2000.0).abs() < 1e-4);
    }

    #[test]
    fn track_clamps_at_both_ends_and_interpolates_between() {
        let table = [(0.0, 10.0), (10.0, 20.0)];
        assert!((track(&table, -5.0) - 10.0).abs() < 1e-6);
        assert!((track(&table, 5.0) - 15.0).abs() < 1e-6);
        assert!((track(&table, 50.0) - 20.0).abs() < 1e-6);
        assert!((track(&[], 3.0)).abs() < 1e-6);
        assert!((track(&[(2.0, 7.0)], 9.0) - 7.0).abs() < 1e-6);
        // Exactly on a key reads that key.
        assert!((track(&table, 0.0) - 10.0).abs() < 1e-6);
        assert!((track(&table, 10.0) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn the_tracking_lands_after_the_last_character() {
        // The source's own arithmetic: a word's advance box grows by its whole length, the space by
        // one, so the two constants stay separate numbers.
        let size = CAP / CAP_RATIO;
        let tracking_px = TRACKING * size;
        assert!((tracking_px + 2.63).abs() < 0.01, "tracking {tracking_px}");
        let advance = |chars: usize| 100.0 + chars as f32 * tracking_px;
        assert!(advance(3) < advance(1));
        assert!((advance(3) - advance(1) - 2.0 * tracking_px).abs() < 1e-4);
        assert!((TRACKING + SPACE_TRIM + 0.1099).abs() < 1e-4);
    }
}
