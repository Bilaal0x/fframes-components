//! A line writes itself, selects its opening, and rewrites the ending.

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, clamp01, ease_out_cubic, normalised_speed};
use crate::ui::{Mode, ReferenceFrame, UiTheme, mix_oklch, text, to_hex, with_alpha};

// --- The stage -------------------------------------------------------------
//
// The reference recording is 810×458 and every number below was read off its frames, so there is
// no second conversion to get wrong.

/// The reference recording's width, in its own pixels — the source's `REF_W`.
pub const REF_W: f32 = 810.0;
/// The reference recording's height, in its own pixels — the source's `REF_H`.
pub const REF_H: f32 = 458.0;

// --- Measured proportions --------------------------------------------------

/// The line is set tight — a hair of negative tracking, tighter word gaps.
pub const TRACKING: f32 = -0.012;
/// Word-spacing, in em of the type it separates — the source's `WORD_GAP`.
pub const WORD_GAP: f32 = -0.05;

/// The selection box's height, in em of the type it sits behind.
pub const SEL_HEIGHT: f32 = 0.954;
/// How far the selection's bottom sits below the baseline, in em.
pub const SEL_BELOW: f32 = 0.083;
/// How far the selection reaches past the type on an unclipped end, in em.
pub const SEL_PAD: f32 = 0.11;

/// The mark's clearance from the last visible letter, in em.
pub const MARK_GAP: f32 = 0.4;
/// The mark's box, in em. The reference's mark inks 36×40 px of a 0.83em square; this one fills
/// about 70% of its own box, so 1.15em of box puts the same amount of ink on the line.
pub const MARK_SIZE: f32 = 1.15;
/// How far the mark's centre sits above the baseline, in em.
pub const MARK_RISE: f32 = 0.4;

/// The sheen, as `[position, × the base alpha]`.
///
/// Measured on eight frames spanning three different box widths and two phases — 402 px selected,
/// 272 px after the rewrite, 193 px mid-erase. It lands in the same place every time: **deepest at
/// 0.36, palest at 0.575, a 3.2× swing**. That is a gradient painted on the box, not on the page,
/// which is why it appears to travel as the box is dragged and then eaten.
pub const SHEEN: [(f32, f32); 12] = [
    (0.0, 0.93),
    (0.1, 1.19),
    (0.19, 1.24),
    (0.3, 1.49),
    (0.37, 1.56),
    (0.44, 1.03),
    (0.51, 0.57),
    (0.59, 0.49),
    (0.69, 0.59),
    (0.78, 0.75),
    (0.89, 1.03),
    (1.0, 1.16),
];

/// The curve the erase and the rewrite both run on.
///
/// Fitted on the recording's own timestamps — it is a 60 fps capture with eight dropped frames, so
/// a frame index is not a clock. The erase's clip edge over 29 frames gives
/// `cubic-bezier(0.419, −0.050, 0.184, 1.044)` at rms 0.0086 of the travel; the rewrite, fitted
/// independently on the line's *centre*, gives `cubic-bezier(0.427, 0.021, 0.193, 0.999)` at rms
/// 0.0059. Two different signals, two different halves of the gesture, the same curve. It is an
/// ease-in-out, and that is the whole character of the move: the edge creeps for four frames,
/// crosses the sentence in twelve, and settles for twelve more.
pub const SWEEP_EASE: CubicBezier = CubicBezier::new(0.42, 0.0, 0.19, 1.0);

/// The forward move's curve, used only when `zoom_duration` gives the cut a ramp: a short moderate
/// decelerate.
pub const ZOOM_EASE: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);

/// The exponent of the selection drag: `Easing.out(Easing.poly(3.5))`.
///
/// Measured without touching the gradient at all: take nineteen fixed columns across the line and
/// find the frame the warmth arrives in each. The edge is 28% across by 17 ms, 54% by 33 ms, 71% by
/// 50 ms, 91% by 100 ms and 97% by 150 ms. A `quint` is far too fast off the mark and misses the
/// first two points by twenty points of travel.
pub const DRAG_EXPONENT: f32 = 3.5;

/// The mark: the same outline `public/logo.svg` draws, in a `0 0 100 100` box.
pub const MARK_PATH: &str = "M15.757 15.459c-3.324 0.816 -6.07 2.966 -7.563 5.911 -1.194 2.388 -1.174 1.672 -1.174 24.66 0 19.982 0.02 21.077 0.378 22.132 0.955 2.926 2.946 4.498 6.588 5.215 1.99 0.398 2.528 0.657 3.702 1.732 1.055 0.975 1.473 2.169 1.473 4.219 0.02 2.408 0.836 3.901 2.647 4.856l1.035 0.537 19.505 0.06c17.574 0.06 19.624 0.02 20.739 -0.259 1.493 -0.418 2.647 -1.333 3.403 -2.766l0.537 -1.015 0.06 -6.269c0.04 -3.443 0.04 -6.807 0 -7.444l-0.06 -1.194 -0.617 0.995c-0.717 1.174 -2.01 2.289 -3.383 2.906l-0.975 0.458 -16.121 0.06c-11.763 0.04 -16.42 -0 -17.216 -0.159 -2.548 -0.557 -5.135 -2.408 -6.468 -4.657 -1.294 -2.189 -1.314 -2.548 -1.254 -16.44l0.06 -12.439 0.478 -1.154c0.876 -2.209 2.926 -4.18 5.374 -5.155l1.115 -0.458 16.519 -0.06c16.101 -0.04 16.539 -0.04 17.813 0.358 1.592 0.498 2.966 1.473 3.941 2.826l0.736 1.035 0.06 -6.648c0.06 -7.304 -0.06 -8.319 -1.115 -9.832 -0.597 -0.876 -1.95 -1.811 -3.025 -2.11 -0.438 -0.119 -9.096 -0.199 -23.386 -0.179 -18.37 0.02 -22.908 0.06 -23.804 0.279zM79.665 31.819c-6.508 3.901 -11.902 7.185 -11.981 7.304 -0.08 0.119 -0.139 5.055 -0.1 10.947l0.04 10.748 2.886 1.811c1.592 0.995 4.14 2.587 5.672 3.523 1.533 0.935 5.632 3.463 9.096 5.613 3.463 2.15 6.488 3.901 6.707 3.901 0.219 -0 0.537 -0.139 0.697 -0.318 0.279 -0.279 0.318 -2.806 0.318 -24.958 0 -15.544 -0.08 -24.839 -0.199 -25.157 -0.139 -0.398 -0.318 -0.517 -0.736 -0.517 -0.358 0.02 -4.617 2.448 -12.399 7.105z";

/// The frame rate the source's seconds are quoted against.
pub const FPS: f32 = 30.0;

/// The weight the line is drawn at by default — Inter Medium, which the source's demo ships.
pub const DEFAULT_FONT_WEIGHT: u16 = 500;

/// The crop of a ramp that has no width: `t >= from`.
fn ramp(t: f32, from: f32, span: f32) -> f32 {
    if span <= 0.0 {
        if t >= from { 1.0 } else { 0.0 }
    } else {
        clamp01((t - from) / span)
    }
}

// --- The timeline, as pure functions ---------------------------------------

/// How many words have landed, and how far the newest one has opened the line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Landing {
    /// How many words are on the line.
    pub count: usize,
    /// How far the newest word has opened the line, `0..=1`.
    pub grow: f32,
}

/// How many words have landed at `t` seconds, and how far the newest one has opened the line.
///
/// The reference does **not** reflow when a word lands. Measured across all four arrivals, both
/// ends of the line move outward by the same amount over the same five frames — +18, +8, +4, +2,
/// +1 on the right and the mirror of that on the left — while the centre never leaves 404 of 810.
/// So the line's *width* is tweened around a fixed centre and the new word is revealed by it.
///
/// A non-positive `stagger` (which the source's own control cannot produce) lands every word at
/// once rather than dividing by zero.
#[must_use]
pub fn landed(t: f32, n: usize, at: f32, stagger: f32, dur: f32) -> Landing {
    if t < at {
        return Landing {
            count: 0,
            grow: 0.0,
        };
    }
    // +1e-6: a cue lands on an exact multiple of the stagger, and in doubles
    // 0.217 + 3 × 0.189 divides back to 2.9999999, which drops a word.
    let count = if stagger > 0.0 {
        let raw = ((t - at) / stagger + 1e-6).floor() + 1.0;
        if raw >= n as f32 {
            n
        } else {
            (raw as usize).min(n)
        }
    } else {
        n
    };
    let cue = at + (count as f32 - 1.0) * stagger;
    let grow = if dur <= 0.0 {
        1.0
    } else {
        ease_out_cubic(ramp(t, cue, dur))
    };
    Landing { count, grow }
}

/// A word's ink: its opacity, and how far it has cooled out of the accent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordInk {
    /// The word's opacity, `0..=1`.
    pub opacity: f32,
    /// How far the accent has become the foreground, `0..=1`.
    pub cool: f32,
}

/// The per-word options the source passes to `wordInk` as one object literal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WordInkTiming {
    /// Seconds at which the first word lands.
    pub at: f32,
    /// Seconds between words.
    pub stagger: f32,
    /// Seconds a word takes to fade up.
    pub dur: f32,
    /// Seconds a word holds the accent before it starts cooling.
    pub cool_delay: f32,
    /// Seconds the accent takes to become the foreground.
    pub cool_dur: f32,
    /// Seconds at which the selection starts being dragged, for the last word's own cooling.
    pub drag_at: Option<f32>,
    /// Seconds the drag takes.
    pub drag_dur: f32,
    /// Whether this is the line's last word.
    pub last: bool,
}

impl WordInkTiming {
    /// The timing for word `index` of a line rendered with `config`.
    #[must_use]
    pub fn for_word(config: &TextRewriteConfig, last: bool) -> Self {
        Self {
            at: config.word_at,
            stagger: config.word_stagger,
            dur: config.word_duration,
            cool_delay: config.cool_delay,
            cool_dur: config.cool_duration,
            drag_at: if config.drag_duration > 0.0 {
                Some(config.drag_at)
            } else {
                None
            },
            drag_dur: config.drag_duration,
            last,
        }
    }
}

/// A word's ink at `t` seconds: its opacity, and how far it has cooled out of the accent.
///
/// Every word arrives in the accent, holds it for one beat — the same 189 ms as the stagger — and
/// cools over the next, so exactly one word is ever coloured and it walks the line ahead of the
/// reader. The last word has no next word to hand off to: on the reference it cools *while the
/// selection is dragged over it*, in half the time the drag takes, and is dark by the time the
/// line is selected.
#[must_use]
pub fn word_ink(t: f32, i: usize, o: WordInkTiming) -> WordInk {
    let cue = o.at + i as f32 * o.stagger;
    if t < cue {
        return WordInk {
            opacity: 0.0,
            cool: 0.0,
        };
    }
    let opacity = ease_out_cubic(ramp(t, cue, o.dur));
    let use_drag = o.last && o.drag_at.is_some();
    let from = if use_drag {
        o.drag_at.unwrap_or(cue)
    } else {
        cue + o.cool_delay
    };
    let span = if use_drag {
        o.drag_dur / 2.0
    } else {
        o.cool_dur
    };
    WordInk {
        opacity,
        cool: clamp01((t - from) / span.max(1e-4)),
    }
}

/// `Easing.out(Easing.poly(3.5))`: `1 - (1 - t)^3.5`.
#[must_use]
pub fn drag_ease(t: f32) -> f32 {
    1.0 - (1.0 - clamp01(t)).powf(DRAG_EXPONENT)
}

/// The selection being dragged across, as a fraction of the line.
///
/// 28% across at 17 ms, 54% at 33 ms, 71% at 50 ms — see [`DRAG_EXPONENT`] for how that was
/// measured without the gradient getting in the way. It starts on the same frame the line comes
/// forward.
#[must_use]
pub fn drag_in(t: f32, at: f32, dur: f32) -> f32 {
    if dur <= 0.0 {
        if t >= at { 1.0 } else { 0.0 }
    } else {
        drag_ease(clamp01((t - at) / dur))
    }
}

/// A sweep on the measured ease-in-out. Both halves of the rewrite use it.
#[must_use]
pub fn sweep(t: f32, at: f32, dur: f32) -> f32 {
    if dur <= 0.0 {
        if t >= at { 1.0 } else { 0.0 }
    } else {
        SWEEP_EASE.ease(clamp01((t - at) / dur))
    }
}

/// The deselect: the highlight's left edge running out to meet its right one.
///
/// Nine frames, and **linear to within 6% of the travel** — 0.109, 0.233, 0.400, 0.567, 0.691,
/// 0.778, 0.851 against a straight line's 0.22, 0.33, 0.44, 0.55, 0.67, 0.78, 0.89. No named curve
/// does meaningfully better over nine frames, and nothing about it reads as an ease.
#[must_use]
pub fn deselect(t: f32, at: f32, dur: f32) -> f32 {
    if dur <= 0.0 {
        if t >= at { 1.0 } else { 0.0 }
    } else {
        clamp01((t - at) / dur)
    }
}

/// The forward move, as a scale.
///
/// On the reference this is a **cut**: 188 px of ink on one frame and 453 px on the next, with
/// nothing in between anywhere in the recording, which is `zoomDuration = 0` exactly. It is not the
/// default for a *ramp*: at 30 fps a step in scale does not read as coming forward, it reads as an
/// edit — so a duration turns it into a short moderate decelerate.
#[must_use]
pub fn zoom_scale(t: f32, at: f32, dur: f32, to: f32) -> f32 {
    if dur <= 0.0 {
        if t >= at { to } else { 1.0 }
    } else {
        1.0 + (to - 1.0) * ZOOM_EASE.ease(clamp01((t - at) / dur))
    }
}

// --- Measurement -----------------------------------------------------------

/// Every width the geometry is built from, in stage px.
///
/// The source reads these out of a hidden copy of the real line with `offsetWidth`, because the
/// gesture needs geometry a DOM flow cannot give it: where the surviving words sat in the *old*
/// layout, how far a clip has to travel to reach them, and where the mark goes when the last
/// visible letter is halfway through being eaten.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurements {
    /// The headline's words, split on whitespace.
    pub words: Vec<String>,
    /// Each word's advance width, including its own tracking.
    pub widths: Vec<f32>,
    /// One inter-word gap: the space glyph's advance plus `word_spacing`.
    ///
    /// `word-spacing` is a separate concept from letter-spacing and it is *here*, in the layout
    /// maths, not only on the text node: every offset, run width, clip edge and mark position is
    /// built from this one number.
    pub space: f32,
    /// The appended text's advance width.
    pub append: f32,
}

/// Left edge of word `i` inside the line, in stage px.
#[must_use]
pub fn offset(i: usize, sizes: &Measurements) -> f32 {
    sizes
        .widths
        .iter()
        .take(i.min(sizes.widths.len()))
        .map(|width| width + sizes.space)
        .sum()
}

/// The line's width when its first `n` words are on it, in stage px.
#[must_use]
pub fn run_width(n: usize, sizes: &Measurements) -> f32 {
    if n == 0 {
        0.0
    } else {
        offset(n, sizes) - sizes.space
    }
}

/// Every position the line, its window, its selection and its mark occupy at one frame, in stage
/// px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineGeometry {
    /// How many words have landed.
    pub count: usize,
    /// How far the newest word has opened the line, `0..=1`.
    pub grow: f32,
    /// The line's left edge — the text's own origin, before the window clips it.
    pub left: f32,
    /// The clip window's left edge: the erase.
    pub k0: f32,
    /// The clip window's right edge: the rewrite, and the end of the visible line.
    pub k1: f32,
    /// The selection's left edge after the drag and the deselect.
    pub s0: f32,
    /// The selection's right edge.
    pub s1: f32,
    /// Whether the selection is painted at all.
    pub sel_shown: bool,
    /// Where the mark's own box starts — `0.4em` clear of the last *visible* letter.
    pub mark_x: f32,
    /// The forward move's scale.
    pub scale: f32,
    /// The erase's progress along its own travel, `0..=1`.
    pub erase: f32,
    /// The rewrite's progress, `0..=1`.
    pub write: f32,
    /// Whether the rewrite half has begun: the source's `write > 0`.
    pub rewriting: bool,
    /// How many words off the end of the headline survive.
    pub kept: usize,
    /// Where the survivors start in the old layout.
    pub cut: usize,
}

/// The whole scene's geometry at `now` seconds.
///
/// The window is the erase *and* the rewrite: its left edge is `k0`, its right edge `k1`, and
/// nothing else touches it. The text inside never moves relative to the line, so a letter is cut in
/// half by the edge instead of being deleted and re-laid-out. Before the rewrite the line is
/// centred on its grown width plus its tail; after it, on the new content's, and the survivors
/// travel between the two on the same sweep.
#[must_use]
pub fn line_geometry(now: f32, sizes: &Measurements, config: &TextRewriteConfig) -> LineGeometry {
    let n = sizes.words.len();
    let kept = (config.keep.round() as i64).clamp(0, n as i64) as usize;
    let cut = n - kept;

    let size = config.font_size;
    let gap = size * MARK_GAP;
    let mark_w = size * config.mark_size;
    let has_mark = config.mark_path.as_deref().is_some_and(|p| !p.is_empty());
    let tail = if has_mark { gap + mark_w } else { 0.0 };
    let pad = size * config.selection_pad;

    let landing = landed(
        now,
        n,
        config.word_at,
        config.word_stagger,
        config.word_duration,
    );
    let count = landing.count;
    let grow = landing.grow;
    let drag = drag_in(now, config.drag_at, config.drag_duration);
    let erase = sweep(now, config.erase_at, config.erase_duration);
    let write = sweep(now, config.rewrite_at, config.rewrite_duration);
    let drop = deselect(now, config.deselect_at, config.deselect_duration);
    let scale = zoom_scale(now, config.cut_at, config.zoom_duration, config.zoom);

    // Phase 1–2 layout. The line's *centring* width tweens around a fixed centre as each word
    // opens it. The word itself is drawn at full width from its first frame and fades: it lands
    // whole and pale, overlapping the mark, and the mark slides clear over the next four frames.
    let opening = if count > 0 {
        (if count > 1 { sizes.space } else { 0.0 })
            + sizes.widths.get(count - 1).copied().unwrap_or(0.0)
    } else {
        0.0
    };
    let grown = run_width(count.saturating_sub(1), sizes) + opening * grow;
    let drawn = run_width(count, sizes);
    let full = run_width(n, sizes);

    // Layout A: the opening line, centred. Layout B: what it is rewritten into.
    let a_left = REF_W / 2.0 - (full + tail) / 2.0;
    let kept_left = a_left + offset(cut, sizes);
    let kept_width = full - offset(cut, sizes);
    let written = if config.append.is_empty() {
        0.0
    } else {
        sizes.space + sizes.append
    };
    let new_width = kept_width + written;
    let b_left = REF_W / 2.0 - (new_width + tail) / 2.0;

    // Before the rewrite the line sits where it was written; after it, where its new content
    // belongs. Fitted separately, the slide and the reveal land on one shared progress.
    let rewriting = write > 0.0;
    let left = if rewriting {
        kept_left + (b_left - kept_left) * write
    } else {
        REF_W / 2.0 - (grown + tail) / 2.0
    };

    let k0 = if rewriting {
        left
    } else {
        left + offset(cut, sizes) * erase
    };
    let k1 = if rewriting {
        left + kept_width + written * write
    } else {
        left + drawn
    };

    // The selection is its own rectangle, not a lining inside the window: the drag sweeps it across
    // a line that is entirely visible the whole time, and the deselect runs its left edge out to
    // meet its right one over a line the clip has finished with.
    let sel_from = if erase > 0.0 || rewriting {
        k0
    } else {
        left - pad
    };
    let sel_to = if rewriting { k1 } else { left + drawn + pad };
    let s1 = sel_from + (sel_to - sel_from) * drag;
    let s0 = sel_from + (s1 - sel_from) * drop;

    LineGeometry {
        count,
        grow,
        left,
        k0,
        k1,
        s0,
        s1,
        sel_shown: drag > 0.0 && drop < 1.0,
        mark_x: if rewriting { k1 } else { left + grown },
        scale,
        erase,
        write,
        rewriting,
        kept,
        cut,
    }
}

/// The frame at which the scene has settled: the last frame on which anything moves.
///
/// `registry/__measured__.json` records `settle.base = 90` for the shipped default, and the
/// deselect — the last thing to move — closes on `deselect_at + deselect_duration` seconds.
#[must_use]
pub fn settle_frame(config: &TextRewriteConfig, fps: f32) -> f32 {
    (config.deselect_at + config.deselect_duration) * fps / normalised_speed(config.speed)
}

/// The SVG gradient line of a CSS `linear-gradient(<angle>deg, …)` over a `w × h` box, as
/// `[x1, y1, x2, y2]` in the box's own coordinates.
///
/// CSS runs the line through the box's centre at `angle` (0° points up, 90° right) and sizes it so
/// that its endpoints' projections cover the box: `|w·sin θ| + |h·cos θ|`.
#[must_use]
pub fn gradient_line(x: f32, y: f32, w: f32, h: f32, angle_deg: f32) -> [f32; 4] {
    let theta = angle_deg.to_radians();
    let (sin, cos) = (theta.sin(), theta.cos());
    let length = (w * sin).abs() + (h * cos).abs();
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let (dx, dy) = (sin * length / 2.0, -cos * length / 2.0);
    [cx - dx, cy - dy, cx + dx, cy + dy]
}

/// The alpha one sheen stop is painted at: `min(1, base × factor)`.
#[must_use]
pub fn sheen_stop_alpha(base: f32, factor: f32) -> f32 {
    (base * factor).min(1.0)
}

/// The source's `u = min(width / REF_W, height / REF_H)`, centred.
///
/// This is **not** the `vmin / REFERENCE_VMIN` design unit used by the other cinematic components:
/// the reference here is a measured screen recording, not a 1280x720 design canvas, so the two differ
/// by a factor of `2.358 / 1.5` and using the design unit renders the scene at 64% of its size.
#[must_use]
pub fn scene_view(width: f32, height: f32) -> ReferenceFrame {
    ReferenceFrame::fit_in(width, height, REF_W, REF_H)
}

/// Configuration for [`TextRewrite`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextRewriteConfig {
    /// The line that writes itself, one word at a time.
    pub headline: String,
    /// How many words off the end of `headline` survive the erase. The rest are swept away from the
    /// left, and `append` is typed onto what is left.
    pub keep: f32,
    /// What gets written onto the surviving words. Empty leaves them alone.
    pub append: String,
    /// Seconds at which the mark appears, ahead of the first word.
    pub mark_at: f32,
    /// Seconds at which the first word lands.
    pub word_at: f32,
    /// Seconds between words — also how long a word holds the accent.
    pub word_stagger: f32,
    /// How long a word takes to fade up and open the line.
    pub word_duration: f32,
    /// How long a word holds the accent before it starts cooling.
    pub cool_delay: f32,
    /// How long the accent takes to become the foreground.
    pub cool_duration: f32,
    /// Seconds at which the line comes forward.
    pub cut_at: f32,
    /// How long the forward move takes. `0` is the reference's single-frame cut.
    pub zoom_duration: f32,
    /// How far forward it goes.
    pub zoom: f32,
    /// Seconds at which the selection starts being dragged across.
    pub drag_at: f32,
    /// How long the drag takes.
    pub drag_duration: f32,
    /// Seconds at which the erase starts eating the line from the left.
    pub erase_at: f32,
    /// How long the erase takes.
    pub erase_duration: f32,
    /// Seconds at which `append` starts being written on.
    pub rewrite_at: f32,
    /// How long the rewrite takes.
    pub rewrite_duration: f32,
    /// Seconds at which the selection is dropped.
    pub deselect_at: f32,
    /// How long the deselect takes.
    pub deselect_duration: f32,
    /// Type size before the forward move, in stage px.
    pub font_size: f32,
    /// Weight of the line. Inter ships 400/500/600 here.
    pub font_weight: u16,
    /// Where the line sits down the frame, as a fraction.
    pub center_y: f32,
    /// Letter-spacing, in em.
    pub tracking: f32,
    /// Word-spacing, in em.
    pub word_gap: f32,
    /// Base strength of the selection, before the sheen multiplies it.
    pub selection_opacity: f32,
    /// Angle of the sheen. 90 is the reference's — straight across.
    pub selection_angle: f32,
    /// Override the sheen shape: `(position, × base alpha)` pairs.
    pub selection_sheen: Vec<(f32, f32)>,
    /// How far the selection reaches past the type on an unclipped end, in em.
    pub selection_pad: f32,
    /// The mark that trails the line, as a path in a `0 0 100 100` box. `None` (or an empty
    /// string, as in the source) leaves the line bare.
    pub mark_path: Option<String>,
    /// The mark's colour, if it should differ from the accent.
    pub mark_color: Option<String>,
    /// The mark's box, in em of the type size. 0 leaves the line bare.
    pub mark_size: f32,
    /// The soft wash behind the line.
    pub glow: bool,
    /// Colour of the wash, if it should differ from the accent.
    pub glow_color: Option<String>,
    /// Strength of the wash.
    pub glow_opacity: f32,
    /// The colour a word arrives in, and the colour the sheen is painted from.
    pub accent_color: Option<String>,
    /// Token overrides.
    pub theme: Option<UiTheme>,
    /// Which token set to resolve against.
    pub mode: Mode,
    /// The face this scene paints its words in. `None` keeps the face it was designed around,
    /// which is Inter.
    pub font_family: Option<String>,
    /// Multiplies the frame clock. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Composition length in frames.
    pub duration: usize,
}

impl Default for TextRewriteConfig {
    fn default() -> Self {
        // `config.ts`'s demo defaults, field for field, plus the `index.tsx` props that are not
        // controls (tracking, word-gap, the sheen, the mark's path and the wash's colour).
        Self {
            headline: "Or just add fframes".to_owned(),
            keep: 1.0,
            append: "registry".to_owned(),
            mark_at: 0.117,
            word_at: 0.217,
            word_stagger: 0.189,
            word_duration: 0.085,
            cool_delay: 0.189,
            cool_duration: 0.189,
            cut_at: 1.033,
            zoom_duration: 0.0,
            zoom: 2.41,
            drag_at: 1.033,
            drag_duration: 0.2,
            erase_at: 1.45,
            erase_duration: 0.517,
            rewrite_at: 2.0,
            rewrite_duration: 0.533,
            deselect_at: 2.85,
            deselect_duration: 0.15,
            font_size: 20.0,
            font_weight: DEFAULT_FONT_WEIGHT,
            center_y: 0.4902,
            tracking: TRACKING,
            word_gap: WORD_GAP,
            selection_opacity: 0.21,
            selection_angle: 90.0,
            selection_sheen: SHEEN.to_vec(),
            selection_pad: SEL_PAD,
            mark_path: Some(MARK_PATH.to_owned()),
            mark_color: None,
            mark_size: MARK_SIZE,
            glow: true,
            glow_color: None,
            glow_opacity: 0.075,
            accent_color: None,
            theme: None,
            mode: Mode::Light,
            font_family: None,
            speed: 1.0,
            duration: 102,
        }
    }
}

/// The **Text Rewrite** component.
#[derive(Debug, Clone, Default)]
pub struct TextRewrite {
    /// Component options.
    pub config: TextRewriteConfig,
    measured: OnceLock<Measurements>,
}

impl TextRewrite {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: TextRewriteConfig) -> Self {
        Self {
            config,
            measured: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-rewrite",
            name: "Text Rewrite",
            summary: "A line writes itself, is selected, has its opening clipped away and a new ending written on",
            feature: "text-rewrite",
        }
    }

    /// Measure the line once and cache it: the words, their widths, the word gap and the append.
    fn measure<'s>(&'s self, ctx: &RenderCtx<'_>, family: &str) -> &'s Measurements {
        self.measured.get_or_init(|| {
            let config = &self.config;
            let size = config.font_size;
            let weight = config.font_weight;
            let tracking_px = text::tracking_px_from_em(config.tracking, size);
            let words: Vec<String> = config
                .headline
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            Measurements {
                widths: words
                    .iter()
                    .map(|word| {
                        text::tracked_width_at(ctx, family, weight, size, word, tracking_px)
                    })
                    .collect(),
                words,
                // `word-spacing` is an em value on the source's style block, so it is the font size
                // in px — and it belongs to the *gap*, which is what the layout maths consumes.
                space: ctx.space_width_at(family, weight, size)
                    + text::tracking_px_from_em(config.word_gap, size),
                append: text::tracked_width_at(
                    ctx,
                    family,
                    weight,
                    size,
                    &config.append,
                    tracking_px,
                ),
            }
        })
    }

    /// One word's `<text>` node, its left edge at `x`.
    #[allow(clippy::too_many_arguments)]
    fn word_node(
        ctx: &RenderCtx<'_>,
        family: &str,
        size: f32,
        weight: u16,
        tracking_px: f32,
        text_value: &str,
        width: f32,
        x: f32,
        baseline: f32,
        fill: String,
        opacity: f32,
    ) -> Svgr<'static> {
        // `text_node` centres the run on the width it measures itself, which is the width this
        // layout already used, so the node's left edge lands exactly on `x`.
        text::text_node(
            ctx,
            text_value,
            x + width / 2.0,
            baseline,
            family.to_owned(),
            size,
            weight,
            tracking_px,
            fill,
            opacity,
        )
    }
}

impl Component for TextRewrite {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source's `AbsoluteFill` paints the theme's `background` token; the light palette's
        // value is a constant, so the static signature can name it.
        Some("#faf9f6")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);
        let accent = config
            .accent_color
            .clone()
            .unwrap_or_else(|| theme.primary.clone());
        let wash = config.glow_color.clone().unwrap_or_else(|| accent.clone());
        let mark_color = config.mark_color.clone().unwrap_or_else(|| accent.clone());

        let size = config.font_size;
        let tracking_px = text::tracking_px_from_em(config.tracking, size);
        let line_height = (size * 1.35).round();
        let now = ctx.frame() * normalised_speed(config.speed) / ctx.info.fps;

        let sizes = self.measure(ctx, &family);
        let geometry = line_geometry(now, sizes, config);
        let view = scene_view(ctx.width(), ctx.height());

        // The line's own line box is centred on `centerY * REF_H`, and CSS puts the baseline
        // `(ascent - descent) / 2` below that centre — the same number the source reads off its
        // zero-height probe span. It does not depend on the line height.
        let center_y = config.center_y * REF_H;
        let baseline = text::baseline_for_centred_box(ctx, &family, size, center_y);
        let box_top = center_y - line_height / 2.0;

        // A cut has no in-between frame to judder on, so it pivots on the line's optical centre,
        // which is what the reference does: its small line and its full-size line share a centre
        // at y 224 of 458, not a baseline. A *ramp* has in-between frames, and there the pivot has
        // to be the measured baseline, or the type climbs the pixel grid in whole-pixel jumps.
        let pivot_y = if config.zoom_duration > 0.0 {
            baseline
        } else {
            center_y
        };
        let line_transform = if (geometry.scale - 1.0).abs() < 1e-6 {
            "none".to_owned()
        } else {
            let (px, s) = (REF_W / 2.0, geometry.scale);
            format!(
                "translate({px} {pivot_y}) scale({s}) translate({} {})",
                -px, -pivot_y
            )
        };

        // --- the words on the line, left to right ------------------------------------------
        let mut nodes: Vec<Svgr<'static>> = Vec::new();
        if geometry.rewriting {
            // The survivors never change colour or opacity once the rewrite starts; only the
            // window's right edge moves over them.
            for i in geometry.cut..sizes.words.len() {
                let x = geometry.left + offset(i, sizes) - offset(geometry.cut, sizes);
                nodes.push(Self::word_node(
                    ctx,
                    &family,
                    size,
                    config.font_weight,
                    tracking_px,
                    &sizes.words[i],
                    sizes.widths[i],
                    x,
                    baseline,
                    theme.foreground.clone(),
                    1.0,
                ));
            }
            if !config.append.is_empty() {
                let x = geometry.left
                    + (run_width(sizes.words.len(), sizes) - offset(geometry.cut, sizes))
                    + sizes.space;
                nodes.push(Self::word_node(
                    ctx,
                    &family,
                    size,
                    config.font_weight,
                    tracking_px,
                    &config.append,
                    sizes.append,
                    x,
                    baseline,
                    theme.foreground.clone(),
                    1.0,
                ));
            }
        } else {
            for i in 0..geometry.count.min(sizes.words.len()) {
                let ink = word_ink(
                    now,
                    i,
                    WordInkTiming::for_word(config, i + 1 == sizes.words.len()),
                );
                let x = geometry.left + offset(i, sizes);
                let fill = to_hex(mix_oklch(&accent, &theme.foreground, ink.cool));
                nodes.push(Self::word_node(
                    ctx,
                    &family,
                    size,
                    config.font_weight,
                    tracking_px,
                    &sizes.words[i],
                    sizes.widths[i],
                    x,
                    baseline,
                    fill,
                    ink.opacity,
                ));
            }
        }
        let nodes = Svgr::from(nodes);

        // --- the selection ------------------------------------------------------------------
        let sel_height = size * SEL_HEIGHT;
        let sel_y = baseline + size * SEL_BELOW - sel_height;
        let sel_width = (geometry.s1 - geometry.s0).max(0.0);
        let sheen_line = gradient_line(
            geometry.s0,
            sel_y,
            sel_width,
            sel_height,
            config.selection_angle,
        );
        let sheen_hex = with_alpha(&accent, 1.0);
        // Nothing is emitted on a frame the source does not paint the highlight on — which is most
        // of the composition — so neither a zero-width rect nor an empty gradient ever reaches the
        // renderer.
        let (sheen, selection) = if geometry.sel_shown
            && sel_width > 0.0
            && !config.selection_sheen.is_empty()
        {
            let stops = Svgr::from(
                config
                    .selection_sheen
                    .iter()
                    .map(|(position, factor)| {
                        fframes::svgr!(<stop offset={*position}
                                              stop-color={sheen_hex.clone()}
                                              stop-opacity={sheen_stop_alpha(config.selection_opacity, *factor)} />)
                    })
                    .collect::<Vec<_>>(),
            );
            (
                fframes::svgr!(
                    <linearGradient id="text-rewrite-sheen"
                                    gradientUnits="userSpaceOnUse"
                                    x1={sheen_line[0]} y1={sheen_line[1]}
                                    x2={sheen_line[2]} y2={sheen_line[3]}>
                        {stops}
                    </linearGradient>
                ),
                fframes::svgr!(
                    <rect x={geometry.s0} y={sel_y} width={sel_width} height={sel_height}
                          fill="url(#text-rewrite-sheen)" />
                ),
            )
        } else {
            (Svgr::empty(), Svgr::empty())
        };

        // --- the window: the erase and the rewrite both -------------------------------------
        //
        // A window with no width shows nothing — the source's `width: Math.max(0, k1 - k0)` on an
        // `overflow: hidden` box — and a zero-width `<rect>` is not a valid clip, so a closed
        // window emits neither the clip nor the words it would have hidden.
        let window_width = (geometry.k1 - geometry.k0).max(0.0);
        let (window_clip, windowed) = if window_width > 0.0 {
            (
                fframes::svgr!(
                    <clipPath id="text-rewrite-window">
                        <rect x={geometry.k0} y={box_top} width={window_width}
                              height={line_height} />
                    </clipPath>
                ),
                fframes::svgr!(
                    <g clip-path="url(#text-rewrite-window)">{nodes}</g>
                ),
            )
        } else {
            (Svgr::empty(), Svgr::empty())
        };

        // --- the mark, riding the window's right edge ---------------------------------------
        let mark = match (config.mark_path.as_deref(), now >= config.mark_at) {
            (Some(path), true) if !path.is_empty() && config.mark_size > 0.0 => {
                let mark_w = size * config.mark_size;
                let transform = format!(
                    "translate({} {}) scale({})",
                    geometry.mark_x + size * MARK_GAP,
                    baseline - size * MARK_RISE - mark_w / 2.0,
                    mark_w / 100.0
                );
                fframes::svgr!(<g transform={transform}><path d={path.to_owned()} fill={mark_color} /></g>)
            }
            _ => Svgr::empty(),
        };

        // --- the wash ------------------------------------------------------------------------
        let glow = if config.glow {
            fframes::svgr!(
                <rect x="0" y="0" width={REF_W} height={REF_H}
                      fill="url(#text-rewrite-glow)" />
            )
        } else {
            Svgr::empty()
        };
        let wash_hex = with_alpha(&wash, 1.0);
        let glow_center_y = config.center_y * REF_H;
        let glow_transform = format!(
            "translate({} {}) scale({} {})",
            REF_W / 2.0,
            glow_center_y,
            REF_W * 0.46,
            REF_H * 0.34
        );

        let view_transform = view.transform();
        fframes::svgr!(
            <g transform={view_transform}>
                <defs>
                    <clipPath id="text-rewrite-stage">
                        <rect x="0" y="0" width={REF_W} height={REF_H} />
                    </clipPath>
                    <radialGradient id="text-rewrite-glow" gradientUnits="userSpaceOnUse"
                                    cx="0" cy="0" r="1" gradientTransform={glow_transform}>
                        <stop offset="0" stop-color={wash_hex.clone()}
                              stop-opacity={config.glow_opacity} />
                        <stop offset="1" stop-color={wash_hex} stop-opacity="0" />
                    </radialGradient>
                </defs>
                <g clip-path="url(#text-rewrite-stage)">
                    {glow}
                    <g transform={line_transform}>
                        <defs>
                            {window_clip}
                            {sheen}
                        </defs>
                        {selection}
                        {windowed}
                        {mark}
                    </g>
                </g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A line with four words whose widths are known, so the geometry can be checked by hand.
    fn measured() -> Measurements {
        Measurements {
            words: ["Or", "just", "add", "fframes"]
                .iter()
                .map(|word| (*word).to_owned())
                .collect(),
            widths: vec![20.0, 30.0, 30.0, 60.0],
            space: 4.5,
            append: 60.0,
        }
    }

    fn config() -> TextRewriteConfig {
        TextRewriteConfig::default()
    }

    #[test]
    fn the_source_config_defaults_are_reproduced_field_by_field() {
        let config = TextRewriteConfig::default();
        assert_eq!(config.headline, "Or just add fframes");
        assert!((config.keep - 1.0).abs() < 1e-6);
        assert_eq!(config.append, "registry");
        assert!((config.mark_at - 0.117).abs() < 1e-6);
        assert!((config.word_at - 0.217).abs() < 1e-6);
        assert!((config.word_stagger - 0.189).abs() < 1e-6);
        assert!((config.word_duration - 0.085).abs() < 1e-6);
        assert!((config.cool_delay - 0.189).abs() < 1e-6);
        assert!((config.cool_duration - 0.189).abs() < 1e-6);
        assert!((config.cut_at - 1.033).abs() < 1e-6);
        assert!((config.zoom_duration).abs() < 1e-6);
        assert!((config.zoom - 2.41).abs() < 1e-6);
        assert!((config.drag_at - 1.033).abs() < 1e-6);
        assert!((config.drag_duration - 0.2).abs() < 1e-6);
        assert!((config.erase_at - 1.45).abs() < 1e-6);
        assert!((config.erase_duration - 0.517).abs() < 1e-6);
        assert!((config.rewrite_at - 2.0).abs() < 1e-6);
        assert!((config.rewrite_duration - 0.533).abs() < 1e-6);
        assert!((config.deselect_at - 2.85).abs() < 1e-6);
        assert!((config.deselect_duration - 0.15).abs() < 1e-6);
        assert!((config.font_size - 20.0).abs() < 1e-6);
        assert_eq!(config.font_weight, 500);
        assert!((config.center_y - 0.4902).abs() < 1e-6);
        assert!((config.tracking - (-0.012)).abs() < 1e-6);
        assert!((config.word_gap - (-0.05)).abs() < 1e-6);
        assert!((config.selection_opacity - 0.21).abs() < 1e-6);
        assert!((config.selection_angle - 90.0).abs() < 1e-6);
        assert_eq!(config.selection_sheen, SHEEN.to_vec());
        assert!((config.selection_pad - 0.11).abs() < 1e-6);
        assert_eq!(config.mark_path.as_deref(), Some(MARK_PATH));
        assert_eq!(config.mark_color, None);
        assert!((config.mark_size - 1.15).abs() < 1e-6);
        assert!(config.glow);
        assert_eq!(config.glow_color, None);
        assert!((config.glow_opacity - 0.075).abs() < 1e-6);
        assert_eq!(config.accent_color, None);
        assert_eq!(config.theme, None);
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.font_family, None);
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.duration, 102);
    }

    #[test]
    fn the_duration_is_the_source_composition_length() {
        assert_eq!(TextRewrite::default().duration(), 102);
        assert_eq!(TextRewrite::default().meta().id, "text-rewrite");
        assert_eq!(TextRewrite::meta_static().feature, "text-rewrite");
        assert_eq!(TextRewrite::default().background(), Some("#faf9f6"));
    }

    #[test]
    fn the_settle_formula_reproduces_the_measured_frame() {
        // `registry/__measured__.json` records settle.base = 90 and durationInFrames = 102 for the
        // shipped default. The deselect is the last thing to move and closes on
        // `2.85 + 0.15 = 3.0 s`, which is frame 90 — the same closed form the measurement saw.
        let config = config();
        assert!((settle_frame(&config, FPS) - 90.0).abs() < 1e-6);
        assert!((config.deselect_at + config.deselect_duration - 3.0).abs() < 1e-9);
        // Nothing moves after it, and the composition holds the new claim for the rest of its
        // 102 frames (3.4 s).
        assert!((config.duration as f32) > settle_frame(&config, FPS));
        assert_eq!(config.duration, 102);
        // The measurement's fit has `perWord = 0`, so no term of it is the word count.
        assert!((settle_frame(&config, FPS) - 3.0 * FPS).abs() < 1e-6);
    }

    #[test]
    fn the_settle_scales_with_the_timeline_speed() {
        let mut fast = config();
        fast.speed = 2.0;
        assert!((settle_frame(&fast, FPS) - 45.0).abs() < 1e-6);
        // Zero and negatives are normalised to `1` rather than freezing or reversing.
        fast.speed = 0.0;
        assert!((settle_frame(&fast, FPS) - 90.0).abs() < 1e-6);
        fast.speed = -3.0;
        assert!((settle_frame(&fast, FPS) - 90.0).abs() < 1e-6);
    }

    #[test]
    fn the_write_schedule_holds_every_word_until_its_cue() {
        assert_eq!(landed(0.0, 4, 0.217, 0.189, 0.085).count, 0);
        assert_eq!(landed(0.216, 4, 0.217, 0.189, 0.085).count, 0);
        assert_eq!(landed(0.217, 4, 0.217, 0.189, 0.085).count, 1);
    }

    #[test]
    fn the_write_schedule_adds_a_word_every_measured_189ms() {
        for i in 0..4 {
            let at = 0.217 + i as f32 * 0.189 + 0.001;
            assert_eq!(landed(at, 4, 0.217, 0.189, 0.085).count, i + 1);
        }
        assert_eq!(landed(0.217 + 9.0 * 0.189, 4, 0.217, 0.189, 0.085).count, 4);
        // A zero stagger lands everything at once rather than dividing by zero.
        assert_eq!(landed(0.217, 4, 0.217, 0.0, 0.085).count, 4);
    }

    #[test]
    fn the_write_schedule_opens_the_line_over_85ms_decelerating() {
        let cue = 0.217 + 3.0 * 0.189;
        assert!((landed(cue, 4, 0.217, 0.189, 0.085).grow).abs() < 1e-6);
        assert!((landed(cue + 0.085, 4, 0.217, 0.189, 0.085).grow - 1.0).abs() < 1e-6);
        // Measured on the fourth arrival: the ends move +18, +8, +4, +2, +1 px of a 34 px opening
        // over five frames at 60 fps — better than half way in one.
        let at = |seconds: f32| landed(cue + seconds, 4, 0.217, 0.189, 0.085).grow;
        assert!(at(1.0 / 60.0) > 0.45);
        assert!(at(2.0 / 60.0) > 0.72);
        assert!(at(3.0 / 60.0) > 0.85);
        // And a zero-length opening is complete at the cue.
        assert!((landed(cue, 4, 0.217, 0.189, 0.0).grow - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_accent_walks_the_line_one_word_at_a_time() {
        let o = WordInkTiming::for_word(&config(), false);
        assert!((word_ink(o.at - 0.001, 0, o).opacity).abs() < 1e-6);
        assert!((word_ink(o.at + o.dur, 0, o).opacity - 1.0).abs() < 1e-6);

        // A word holds the accent for exactly one beat, then cools over another.
        let cue = o.at + o.stagger;
        assert!((word_ink(cue + 0.05, 1, o).cool).abs() < 1e-6);
        assert!((word_ink(cue + o.stagger, 1, o).cool).abs() < 1e-6);
        assert!((word_ink(cue + o.stagger + o.stagger, 1, o).cool - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_last_word_cools_on_the_drag_since_no_word_follows_it() {
        let o = WordInkTiming::for_word(&config(), true);
        // Still fully accent on the frame the line comes forward, dark half a drag later.
        assert!((word_ink(o.drag_at.unwrap_or(0.0), 3, o).cool).abs() < 1e-6);
        assert!(
            (word_ink(o.drag_at.unwrap_or(0.0) + o.drag_dur / 2.0, 3, o).cool - 1.0).abs() < 1e-6
        );
        // Only the line's last word takes that route. The word before it is already dark by the
        // frame the line comes forward — it cooled on its own hand-off, one beat behind its cue.
        let not_last = WordInkTiming::for_word(&config(), false);
        assert!((word_ink(1.033, 2, not_last).cool - 1.0).abs() < 1e-6);
        assert!(word_ink(1.033, 3, not_last).cool < 0.5);
        // So exactly one word is ever still in the accent.
        assert!((word_ink(0.8, 3, not_last).cool).abs() < 1e-6);
    }

    #[test]
    fn the_selection_is_dragged_on_the_measured_curve() {
        let (at, dur) = (1.033, 0.2);
        assert!((drag_in(at - 0.001, at, dur)).abs() < 1e-6);
        assert!((drag_in(at + dur, at, dur) - 1.0).abs() < 1e-6);
        // 28% by 17 ms, 54% by 33 ms, 71% by 50 ms, 91% by 100 ms, 97% by 150 ms — measured by
        // finding the frame the warmth arrives in nineteen fixed columns.
        assert!((0.24..0.34).contains(&drag_in(at + 0.017, at, dur)));
        assert!(drag_in(at + 0.033, at, dur) > 0.44);
        assert!(drag_in(at + 0.05, at, dur) > 0.6);
        assert!(drag_in(at + 0.1, at, dur) > 0.87);
        assert!(drag_in(at + 0.15, at, dur) > 0.97);
        // The source's own exponents: `Easing.out(Easing.poly(3.5))`.
        assert!((drag_ease(0.5) - (1.0 - 0.5_f32.powf(3.5))).abs() < 1e-6);
        // Monotonic over the whole composition.
        let mut previous = -1.0_f32;
        for frame in 0..=102 {
            let value = drag_in(frame as f32 / FPS, at, dur);
            assert!(value >= previous, "frame {frame} went backwards");
            previous = value;
        }
    }

    #[test]
    fn the_sweep_is_an_ease_in_out_on_the_measured_curve() {
        let (at, dur) = (1.45, 0.517);
        assert!(sweep(at + dur * 0.15, at, dur) < 0.12);
        assert!(sweep(at + dur * 0.5, at, dur) > 0.5);
        assert!(sweep(at + dur * 0.85, at, dur) > 0.96);
        // The clip edge as a fraction of its travel, at eighths of the sweep, read off the
        // nearest frame to each — which is where the slack below comes from.
        let want = [0.033, 0.236, 0.512, 0.772, 0.902, 0.967, 0.992];
        for (i, expected) in want.iter().enumerate() {
            let got = sweep(at + dur * (i + 1) as f32 / 8.0, at, dur);
            assert!(
                (got - expected).abs() < 0.06,
                "eighth {i}: {got} vs {expected}"
            );
        }
        // Both halves of the gesture share the curve.
        let (write_at, write_dur) = (2.0, 0.533);
        for fraction in [0.15, 0.3, 0.5, 0.7, 0.9] {
            assert!(
                (sweep(write_at + write_dur * fraction, write_at, write_dur)
                    - sweep(at + dur * fraction, at, dur))
                .abs()
                    < 1e-6
            );
        }
        // A zero duration is a threshold.
        assert!((sweep(at - 0.001, at, 0.0)).abs() < 1e-6);
        assert!((sweep(at, at, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_deselect_runs_straight_to_its_end() {
        let (at, dur) = (2.85, 0.15);
        for fraction in [0.25, 0.5, 0.75] {
            assert!((deselect(at + dur * fraction, at, dur) - fraction).abs() < 1e-6);
        }
        assert!((deselect(at - 0.001, at, dur)).abs() < 1e-6);
        assert!((deselect(at + dur, at, dur) - 1.0).abs() < 1e-6);
        assert!((deselect(99.0, at, dur) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_forward_move_is_the_references_cut() {
        let (at, dur, to) = (1.033, 0.0, 2.41);
        assert!((zoom_scale(at - 0.001, at, dur, to) - 1.0).abs() < 1e-6);
        assert!((zoom_scale(at, at, dur, to) - to).abs() < 1e-6);
        // There is no frame on which it is halfway: the whole scale takes two values.
        let mut seen: Vec<f32> = (0..=102)
            .map(|frame| zoom_scale(frame as f32 / FPS, at, dur, to))
            .collect();
        seen.sort_by(f32::total_cmp);
        seen.dedup();
        assert_eq!(seen, vec![1.0, to]);

        // And a duration turns it into a ramp that still lands on the target.
        let ramp = 0.2;
        let mid = zoom_scale(at + 0.1, at, ramp, to);
        assert!(mid > 1.0 && mid < to);
        assert!((zoom_scale(at + ramp, at, ramp, to) - to).abs() < 1e-6);
        assert!((zoom_scale(at - 0.001, at, ramp, to) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_clip_edge_cuts_letters_in_half_as_it_passes() {
        let sizes = measured();
        let config = config();
        let full = run_width(sizes.words.len(), &sizes);
        let tail = config.font_size * (MARK_GAP + config.mark_size);
        let a_left = REF_W / 2.0 - (full + tail) / 2.0;
        let cut = sizes.words.len() - 1;

        // The window never moves the text; it is only an edge crossing it.
        let settled = line_geometry(config.erase_at, &sizes, &config);
        assert!((settled.left - a_left).abs() < 1e-4, "{}", settled.left);
        // At the cue the edge is at the line's left edge and nothing is hidden yet.
        assert!((settled.k0 - a_left).abs() < 1e-4);
        assert!((settled.k1 - (a_left + full)).abs() < 1e-4);

        // Half way through the sweep the edge is strictly inside a word's own box, so the glyph
        // under it is drawn cut, not deleted.
        let mid = line_geometry(
            config.erase_at + config.erase_duration / 2.0,
            &sizes,
            &config,
        );
        assert!(mid.erase > 0.5 && mid.erase < 1.0, "{}", mid.erase);
        let straddled = (0..cut).find(|i| {
            let left = mid.left + offset(*i, &sizes);
            mid.k0 > left && mid.k0 < left + sizes.widths[*i]
        });
        assert!(
            straddled.is_some(),
            "the edge at {} is on a word boundary, not inside a glyph",
            mid.k0
        );

        // The edge only ever travels right through its own window, and stops exactly on the first
        // survivor's left edge. (Once the rewrite starts the window's *left* edge travels with the
        // sliding line instead — that is the other half of the gesture.)
        let mut previous = f32::NEG_INFINITY;
        for frame in 40..=60 {
            let g = line_geometry(frame as f32 / FPS, &sizes, &config);
            assert!(g.k0 >= previous, "frame {frame}: {} went backwards", g.k0);
            previous = g.k0;
        }
        let done = line_geometry(config.erase_at + config.erase_duration, &sizes, &config);
        assert!(
            (done.k0 - (a_left + offset(cut, &sizes))).abs() < 1e-3,
            "{}",
            done.k0
        );
        // And the survivors' own left edge in the old layout is where the sweep ends.
        assert!((done.k0 - (done.left + offset(cut, &sizes))).abs() < 1e-3);
    }

    #[test]
    fn the_rewrite_writes_the_new_ending_on_with_its_right_edge() {
        let sizes = measured();
        let config = config();
        let kept = sizes.words.len() - 1;
        let full = run_width(sizes.words.len(), &sizes);
        let kept_width = full - offset(kept, &sizes);
        let new_width = kept_width + sizes.space + sizes.append;
        let tail = config.font_size * (MARK_GAP + config.mark_size);
        let b_left = REF_W / 2.0 - (new_width + tail) / 2.0;

        let a_left = REF_W / 2.0 - (full + tail) / 2.0;
        let kept_left = a_left + offset(kept, &sizes);

        // The rewrite starts with the window already covering the survivors — they never leave the
        // screen — and none of the new ending written on yet. Its left edge is exactly where the
        // erase left it, so the survivors do not move on the frame the rewrite begins.
        let start = line_geometry(config.rewrite_at + 1e-4, &sizes, &config);
        assert!(start.rewriting);
        assert!(
            (start.k1 - start.k0 - kept_width).abs() < 0.05,
            "{}..{}",
            start.k0,
            start.k1
        );
        assert!(
            (start.k0 - kept_left).abs() < 0.05,
            "{} vs {kept_left}",
            start.k0
        );
        // …and ends with the whole new line revealed, in the new layout.
        let end = line_geometry(config.rewrite_at + config.rewrite_duration, &sizes, &config);
        assert!((end.left - b_left).abs() < 1e-3, "{} vs {b_left}", end.left);
        assert!((end.k1 - (end.left + new_width)).abs() < 1e-3);
        // The edge only ever travels right *over the text*, and the line only ever slides one way.
        // In absolute stage px the window's right edge drifts a little left while the ease-in is
        // still creeping — the line is sliding left faster than the new ending is being written —
        // so what is asserted here is the share of the new ending revealed, not the edge's
        // absolute position.
        let mut previous_write = f32::NEG_INFINITY;
        let mut previous_revealed = f32::NEG_INFINITY;
        let mut previous_left = f32::INFINITY;
        for frame in 61..=76 {
            let g = line_geometry(frame as f32 / FPS, &sizes, &config);
            assert!(g.write >= previous_write, "frame {frame}");
            assert!(
                g.k1 - (g.left + kept_width) >= previous_revealed - 1e-4,
                "frame {frame}"
            );
            assert!(g.left <= previous_left + 1e-6, "frame {frame}");
            previous_write = g.write;
            previous_revealed = g.k1 - (g.left + kept_width);
            previous_left = g.left;
        }
    }

    #[test]
    fn the_line_recentres_on_its_new_content_and_slides_between_the_two() {
        let sizes = measured();
        let config = config();
        let full = run_width(sizes.words.len(), &sizes);
        let tail = config.font_size * (MARK_GAP + config.mark_size);
        let kept = sizes.words.len() - 1;

        // Before the rewrite the line *plus its mark tail* is centred, and the survivors sit where
        // the old layout put them.
        let opening = line_geometry(config.cut_at, &sizes, &config);
        let a_left = REF_W / 2.0 - (full + tail) / 2.0;
        assert!((opening.left - a_left).abs() < 1e-4);
        assert!((opening.left + (full + tail) / 2.0 - REF_W / 2.0).abs() < 1e-4);

        // After it, the new line plus the same tail is centred again — the slide is a real move,
        // not a nudge.
        let done = line_geometry(config.rewrite_at + config.rewrite_duration, &sizes, &config);
        let new_width = (full - offset(kept, &sizes)) + sizes.space + sizes.append;
        assert!((done.left + (new_width + tail) / 2.0 - REF_W / 2.0).abs() < 1e-4);
        assert!(
            (done.left - opening.left).abs() > 1.0,
            "the line should visibly re-centre"
        );

        // The survivors' ink is continuous across the change of meaning: before, they start at
        // `left + offset(cut)`; after, at `left`. Half way, they are between the two layouts.
        let kept_left = a_left + offset(kept, &sizes);
        let half = line_geometry(
            config.rewrite_at + config.rewrite_duration / 2.0,
            &sizes,
            &config,
        );
        assert!(
            (half.left - kept_left).abs() > 1e-3 && (half.left - done.left).abs() > 1e-3,
            "{} is at neither end",
            half.left
        );
        assert!((done.left + offset(kept, &sizes) - offset(kept, &sizes) - done.left).abs() < 1e-6);
        assert!((kept_left - (opening.left + offset(kept, &sizes))).abs() < 1e-6);
    }

    #[test]
    fn the_mark_rides_the_end_of_what_is_visible() {
        let sizes = measured();
        let config = config();
        let full = run_width(sizes.words.len(), &sizes);

        // While the line is being written the mark sits at the end of the *grown* line…
        let early = line_geometry(config.word_at, &sizes, &config);
        let grown = run_width(early.count.saturating_sub(1), &sizes)
            + (if early.count > 1 { sizes.space } else { 0.0 } + sizes.widths[early.count - 1])
                * early.grow;
        assert!((early.mark_x - (early.left + grown)).abs() < 1e-4);
        // …which by the time the selection is dragged is the end of the whole line.
        let selected = line_geometry(config.drag_at, &sizes, &config);
        assert!((selected.mark_x - (selected.left + full)).abs() < 1e-4);
        // The erase eats from the left, so the mark does not move with it: the last visible letter
        // is still the last letter.
        let erasing = line_geometry(
            config.erase_at + config.erase_duration / 2.0,
            &sizes,
            &config,
        );
        assert!((erasing.mark_x - (erasing.left + full)).abs() < 1e-4);
        assert!((erasing.mark_x - erasing.k1).abs() < 1e-4);
        // The rewrite sweeps the window's right edge, and the mark goes with it, to the millimetre.
        for frame in 61..=76 {
            let g = line_geometry(frame as f32 / FPS, &sizes, &config);
            assert!(g.rewriting);
            assert!((g.mark_x - g.k1).abs() < 1e-4, "frame {frame}");
        }
        // A line with no mark has no tail, so it centres on its own ink.
        let bare = TextRewriteConfig {
            mark_path: None,
            ..config
        };
        let bare_line = line_geometry(bare.cut_at, &sizes, &bare);
        assert!((bare_line.left + full / 2.0 - REF_W / 2.0).abs() < 1e-4);
    }

    #[test]
    fn the_selection_is_its_own_box_and_is_dropped_at_the_end() {
        let sizes = measured();
        let config = config();
        let pad = config.font_size * config.selection_pad;

        // Once the drag has run its course and before the erase starts, the selection is a
        // whole-line box, wider than the ink by one pad a side.
        let idle = line_geometry(config.cut_at, &sizes, &config);
        assert!(!idle.sel_shown);
        assert!((idle.s0 - (idle.left - pad)).abs() < 1e-4);
        assert!((idle.s1 - (idle.left - pad)).abs() < 1e-4);
        let dragged = line_geometry(config.drag_at + config.drag_duration, &sizes, &config);
        assert!(dragged.sel_shown);
        assert!((dragged.s0 - (dragged.left - pad)).abs() < 1e-4);
        assert!(
            (dragged.s1 - (dragged.left + run_width(sizes.words.len(), &sizes) + pad)).abs() < 1e-4
        );

        // The drag pulls its right edge across. While the window is not yet touching the line the
        // selection is its own rectangle, reaching a pad past the type on both sides — which is why
        // its left edge sits *before* the window's.
        let half = line_geometry(config.drag_at + config.drag_duration / 2.0, &sizes, &config);
        assert!(half.sel_shown);
        assert!(half.s0 < half.s1);
        assert!((half.s0 - (half.left - pad)).abs() < 1e-4);
        assert!(
            half.s1 > half.s0 && half.s1 < half.left + run_width(sizes.words.len(), &sizes) + pad
        );
        // Once the erase has bitten, the selection's left edge is the window's.
        let biting = line_geometry(
            config.erase_at + config.erase_duration / 2.0,
            &sizes,
            &config,
        );
        assert!((biting.s0 - biting.k0).abs() < 1e-4);

        // Once the rewrite is finished the selection is the window, exactly.
        let selected = line_geometry(config.rewrite_at + config.rewrite_duration, &sizes, &config);
        assert!((selected.s0 - selected.k0).abs() < 1e-3);
        assert!((selected.s1 - selected.k1).abs() < 1e-3);

        // The deselect runs the left edge out to meet the right one, and then it is gone.
        let dropping = line_geometry(
            config.deselect_at + config.deselect_duration / 2.0,
            &sizes,
            &config,
        );
        assert!(dropping.sel_shown);
        assert!(dropping.s0 > selected.s0 && dropping.s0 < dropping.s1);
        let gone = line_geometry(
            config.deselect_at + config.deselect_duration,
            &sizes,
            &config,
        );
        assert!(!gone.sel_shown);
        assert!((gone.s0 - gone.s1).abs() < 1e-4);
    }

    #[test]
    fn word_spacing_reaches_the_layout_not_only_the_text_node() {
        let config = config();
        let sizes = measured();
        // `em × font size`, in px: the source sets `wordSpacing: -0.05em`.
        let expected = config.word_gap * config.font_size;
        assert!((expected - (-1.0)).abs() < 1e-6);
        assert!(
            (text::tracking_px_from_em(config.word_gap, config.font_size) - expected).abs() < 1e-6
        );

        // The gap is in every offset and in the run width, so a wider gap moves the line's ends.
        let wider = Measurements {
            space: sizes.space + 6.0,
            ..sizes.clone()
        };
        assert!((offset(3, &wider) - offset(3, &sizes) - 18.0).abs() < 1e-4);
        assert!(
            (run_width(4, &wider) - run_width(4, &sizes) - 18.0).abs() < 1e-4,
            "three gaps of six px"
        );
        // And it moves the clip edge and the centring with it.
        let a = line_geometry(config.erase_at, &sizes, &config);
        let b = line_geometry(config.erase_at, &wider, &config);
        assert!((a.k0 - b.k0).abs() > 1.0);
        // A single word has no gap at all.
        assert!((run_width(1, &sizes) - sizes.widths[0]).abs() < 1e-6);
        assert!((run_width(0, &sizes)).abs() < 1e-6);
    }

    #[test]
    fn the_sheen_keeps_the_measured_swing() {
        let alphas: Vec<f32> = SHEEN
            .iter()
            .map(|(_, factor)| sheen_stop_alpha(0.2, *factor))
            .collect();
        let max = alphas.iter().copied().fold(f32::MIN, f32::max);
        let min = alphas.iter().copied().fold(f32::MAX, f32::min);
        assert!((max / min - 3.2).abs() < 0.05, "swing {}", max / min);
        // Deepest early, palest just past the middle.
        let deepest = SHEEN[alphas
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i)]
        .0;
        let palest = SHEEN[alphas
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.total_cmp(b.1))
            .map_or(0, |(i, _)| i)]
        .0;
        assert!((deepest - 0.37).abs() < 1e-6);
        assert!((palest - 0.59).abs() < 1e-6);
        // Clamped rather than emitting an alpha above 1.
        for factor in [
            0.93_f32, 1.19, 1.24, 1.49, 1.56, 1.03, 0.57, 0.49, 0.59, 0.75, 0.89, 1.16,
        ] {
            assert!(sheen_stop_alpha(0.9, factor) <= 1.0);
        }
    }

    #[test]
    fn the_sheen_gradient_runs_across_the_box_at_ninety_degrees() {
        // CSS `linear-gradient(90deg, …)` runs left to right through the box's centre.
        let line = gradient_line(100.0, 50.0, 40.0, 20.0, 90.0);
        assert!((line[0] - 100.0).abs() < 1e-4);
        assert!((line[2] - 140.0).abs() < 1e-4);
        assert!((line[1] - 60.0).abs() < 1e-4);
        assert!((line[3] - 60.0).abs() < 1e-4);
        // 0deg points up, and the gradient line spans the box's height.
        let up = gradient_line(0.0, 0.0, 40.0, 20.0, 0.0);
        assert!((up[1] - 20.0).abs() < 1e-4 && (up[3]).abs() < 1e-4);
        // At 45deg the line is as long as the box's projections allow.
        let diagonal = gradient_line(0.0, 0.0, 40.0, 20.0, 45.0);
        let dx = diagonal[2] - diagonal[0];
        let dy = diagonal[3] - diagonal[1];
        assert!((dx.hypot(dy) - (40.0 + 20.0) / 2.0_f32.sqrt()).abs() < 1e-3);
    }

    #[test]
    fn the_reference_scene_is_the_sources_own_box() {
        assert!((REF_W - 810.0).abs() < 1e-6);
        assert!((REF_H - 458.0).abs() < 1e-6);
        // The source's own `u = min(width / REF_W, height / REF_H)`, not the design unit.
        let full = scene_view(1920.0, 1080.0);
        assert!((full.scale - (1920.0_f32 / REF_W).min(1080.0 / REF_H)).abs() < 1e-6);
        assert!((full.scale - 2.357_9).abs() < 0.001, "{}", full.scale);
        let design_unit = 1080.0_f32 / 100.0 / crate::motion::REFERENCE_VMIN;
        assert!((design_unit - 1.5).abs() < 1e-6);
        assert!((full.scale / design_unit - 1.571_9).abs() < 0.001);
        // The box is centred: it fills one axis and is letterboxed equally on the other.
        let (drawn_w, drawn_h) = (REF_W * full.scale, REF_H * full.scale);
        assert!((drawn_h - 1080.0).abs() < 0.01);
        assert!((drawn_w - 1909.9).abs() < 0.5, "{drawn_w}");
        assert!((full.left - (1920.0 - drawn_w) / 2.0).abs() < 1e-3);
        assert!(full.top.abs() < 1e-3);
        // On a 960x540 canvas — the reference demo's own size — the fit is 1.179.
        assert!((scene_view(960.0, 540.0).scale - 1.179).abs() < 0.001);
    }

    #[test]
    fn the_choreography_holds_together() {
        let config = config();
        assert!(config.drag_at >= config.cut_at);
        assert!(config.erase_at > config.drag_at + config.drag_duration);
        assert!(config.rewrite_at >= config.erase_at + config.erase_duration - 0.07);
        assert!(config.deselect_at > config.rewrite_at + config.rewrite_duration);
        assert!(config.deselect_at + config.deselect_duration < config.duration as f32 / FPS);
        // Every word has landed, and the line is open, before it comes forward.
        let all = landed(
            config.cut_at,
            4,
            config.word_at,
            config.word_stagger,
            config.word_duration,
        );
        assert_eq!(all.count, 4);
        assert!((all.grow - 1.0).abs() < 1e-6);
        // And the mark appears before the first word.
        assert!(config.mark_at < config.word_at);
    }

    #[test]
    fn a_headline_of_any_length_still_lays_out() {
        let config = config();
        for count in 0..=6 {
            let sizes = Measurements {
                words: (0..count).map(|i| format!("w{i}")).collect(),
                widths: vec![10.0; count],
                space: 4.0,
                append: 20.0,
            };
            for frame in 0..=102 {
                let geometry = line_geometry(frame as f32 / FPS, &sizes, &config);
                assert!(geometry.k0.is_finite(), "frame {frame}");
                assert!(geometry.k1.is_finite(), "frame {frame}");
                assert!(geometry.left.is_finite(), "frame {frame}");
                assert!(geometry.mark_x.is_finite(), "frame {frame}");
            }
            // `keep` clamps into the line, and the survivors are the line's tail.
            let clamped = line_geometry(config.erase_at, &sizes, &config);
            assert!(clamped.kept <= count);
            assert_eq!(clamped.kept + clamped.cut, count);
        }
    }

    #[test]
    fn keep_clamps_between_nothing_and_the_whole_line() {
        let mut config = config();
        let sizes = measured();
        for (keep, expected) in [
            (0.0, 0_usize),
            (1.0, 1),
            (2.4, 2),
            (4.0, 4),
            (9.0, 4),
            (-3.0, 0),
        ] {
            config.keep = keep;
            let geometry = line_geometry(config.erase_at, &sizes, &config);
            assert_eq!(geometry.kept, expected, "keep {keep}");
            assert_eq!(geometry.cut, sizes.words.len() - expected);
        }
    }

    #[test]
    fn an_empty_headline_draws_nothing_and_does_not_panic() {
        let sizes = Measurements {
            words: Vec::new(),
            widths: Vec::new(),
            space: 4.0,
            append: 20.0,
        };
        let config = TextRewriteConfig {
            headline: String::new(),
            ..config()
        };
        for frame in 0..=102 {
            let geometry = line_geometry(frame as f32 / FPS, &sizes, &config);
            assert_eq!(geometry.count, 0);
            assert_eq!(geometry.cut, 0);
            assert!(geometry.left.is_finite());
            // There is nothing to keep, so `append` is written onto an empty line — the source
            // does exactly this and the window only ever opens.
            assert!(geometry.k1 >= geometry.k0 - 1e-6);
        }
    }

    #[test]
    fn no_append_leaves_the_new_line_at_the_survivors_own_width() {
        let sizes = measured();
        let config = TextRewriteConfig {
            append: String::new(),
            ..config()
        };
        let kept = sizes.words.len() - 1;
        let full = run_width(sizes.words.len(), &sizes);
        let kept_width = full - offset(kept, &sizes);
        let tail = config.font_size * (MARK_GAP + config.mark_size);
        let done = line_geometry(config.rewrite_at + config.rewrite_duration, &sizes, &config);
        assert!((done.left - (REF_W / 2.0 - (kept_width + tail) / 2.0)).abs() < 1e-4);
        assert!((done.k1 - (done.left + kept_width)).abs() < 1e-4);
    }
}
