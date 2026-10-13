//! Cinematic text transitions with multiple motion presets.

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, normalised_speed};
use crate::ui::{Mode, UiTheme, blur, text};

/// The source's `letterSpacing` on the line, in em. Not a prop: the line style sets it for both.
const TRACKING_EM: f32 = -0.02;

/// The source's `marginRight` on a word segment, in em.
const WORD_GAP_EM: f32 = 0.25;

/// The source's `SHUTTER`: how long the shutter stays open, in frames. One whole frame is a 360°
/// shutter, and anything shorter leaves gaps between the samples at the speeds this reaches.
const SHUTTER: f32 = 1.0;

/// Which transition preset replaces one line with another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextSwapTransition {
    /// The line rushes the camera and goes past as one block — the source's `"fly-through"`, and
    /// `config.ts`'s default.
    #[default]
    FlyThrough,
    /// The line fades out where it stands while the replacement fades in, both drifting a few px.
    FadeThrough,
    /// The two lines cross-fade, word by word.
    Crossfade,
    /// The lines exchange places along the y axis.
    SharedAxisY,
    /// The lines exchange places along the z axis: the outgoing grows past the camera, the
    /// incoming resolves from small and out of focus.
    SharedAxisZ,
    /// A hard cut on the step function — no drift, no scale, no blur.
    Cut,
}

/// Whether a line animates as a whole block or word by word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextSwapUnit {
    /// The line is one segment, and moves as one object.
    #[default]
    Block,
    /// The line is one segment per word, each with the source's `0.25em` right margin.
    Word,
}

/// One easing curve, as the source's `Easing` namespace provides them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
    /// `Easing.bezier(x1, y1, x2, y2)` — the same curve CSS solves.
    Curve(CubicBezier),
    /// `Easing.step1`: `0` until the input reaches `1`, then `1`. The source's hard cut.
    Step1,
}

impl Ease {
    /// Sample the curve at `t`.
    #[must_use]
    pub fn ease(self, t: f32) -> f32 {
        match self {
            Self::Curve(curve) => curve.ease(t),
            Self::Step1 => {
                if t >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

/// A preset's intrinsic unit and timings — `TRANSITION_DEFAULTS` in the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionDefaults {
    /// The unit the preset animates in when the caller does not pick one.
    pub unit: TextSwapUnit,
    /// Frames each outgoing segment takes to exit.
    pub exit_duration: f32,
    /// Frames each incoming segment takes to enter.
    pub enter_duration: f32,
}

/// A preset's motion targets — `TRANSITION_MOTION` in the source.
///
/// The exit drifts away from rest, the enter settles into rest, so every exit value is the state
/// at the *end* of the exit window and every enter value the state at its *start*.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionMotion {
    /// Y offset the outgoing line drifts to, in reference-canvas px.
    pub exit_y: f32,
    /// Scale the outgoing line ends at.
    pub exit_scale: f32,
    /// Blur the outgoing line ends at, in reference-canvas px.
    pub exit_blur: f32,
    /// Y offset the incoming line starts from, in reference-canvas px.
    pub enter_y: f32,
    /// Scale the incoming line starts at.
    pub enter_scale: f32,
    /// Blur the incoming line starts with, in reference-canvas px.
    pub enter_blur: f32,
    /// Whether the exit's scale is a **perspective rush** rather than a plain ramp — see
    /// [`perspective_scale`].
    pub exit_perspective: bool,
    /// Fraction of the exit that passes before the outgoing line starts to fade. `0` fades across
    /// the whole exit; the rush holds it back until the very end.
    pub exit_fade_start: f32,
    /// Motion-blur samples the shutter takes across the exit (at least one).
    pub exit_trail: usize,
}

impl TransitionMotion {
    /// Motion with no drift, no scale, no blur, no rush and a single shutter sample: the shape
    /// every preset but `fly-through` shares, which the fields that do move override.
    const fn still() -> Self {
        Self {
            exit_y: 0.0,
            exit_scale: 1.0,
            exit_blur: 0.0,
            enter_y: 0.0,
            enter_scale: 1.0,
            enter_blur: 0.0,
            exit_perspective: false,
            exit_fade_start: 0.0,
            exit_trail: 1,
        }
    }
}

/// A preset's two curves — `TRANSITION_EASINGS` in the source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionEasing {
    /// Shapes the exit's travel.
    pub exit: Ease,
    /// Shapes the enter's arrival.
    pub enter: Ease,
}

impl TextSwapTransition {
    /// The preset's intrinsic unit and timings.
    #[must_use]
    pub fn defaults(self) -> TransitionDefaults {
        match self {
            Self::FlyThrough => TransitionDefaults {
                // Always a block: per word it would be several objects each blowing up about its
                // own centre, which is not a camera move, it is a collision.
                unit: TextSwapUnit::Block,
                exit_duration: 20.0,
                enter_duration: 16.0,
            },
            Self::FadeThrough => TransitionDefaults {
                unit: TextSwapUnit::Block,
                exit_duration: 8.0,
                enter_duration: 13.0,
            },
            Self::Crossfade => TransitionDefaults {
                unit: TextSwapUnit::Word,
                exit_duration: 15.0,
                enter_duration: 21.0,
            },
            Self::SharedAxisY => TransitionDefaults {
                unit: TextSwapUnit::Block,
                exit_duration: 10.0,
                enter_duration: 14.0,
            },
            Self::SharedAxisZ => TransitionDefaults {
                unit: TextSwapUnit::Block,
                exit_duration: 11.0,
                enter_duration: 16.0,
            },
            Self::Cut => TransitionDefaults {
                unit: TextSwapUnit::Word,
                exit_duration: 8.0,
                enter_duration: 8.0,
            },
        }
    }

    /// The preset's motion targets.
    #[must_use]
    pub fn motion(self) -> TransitionMotion {
        match self {
            // The line does not fade out and it does not slide away: it comes at you and goes past.
            // Measured off the reference — apparent size reached ~12x before it was gone, and it
            // stayed solid almost the whole way there.
            Self::FlyThrough => TransitionMotion {
                exit_y: 0.0,
                exit_scale: 12.0,
                // The shutter does the smearing; this only closes the gaps between its discrete
                // samples at the very end of the rush. It is applied in the line's own space and
                // the scale multiplies it, so it is nothing at the start — when it must not soften
                // the type — and enough by the end.
                exit_blur: 0.5,
                exit_perspective: true,
                exit_fade_start: 0.72,
                exit_trail: 18,
                // The replacement is revealed *behind* the line that just flew past, so it arrives
                // from depth: small, out of focus, resolving into place.
                enter_y: 0.0,
                enter_scale: 0.82,
                enter_blur: 9.0,
            },
            Self::FadeThrough => TransitionMotion {
                exit_y: -4.0,
                exit_scale: 1.0,
                exit_blur: 0.0,
                enter_y: 6.0,
                enter_scale: 0.99,
                enter_blur: 2.0,
                ..TransitionMotion::still()
            },
            Self::Crossfade => TransitionMotion {
                exit_y: -6.0,
                exit_scale: 1.0,
                exit_blur: 0.0,
                enter_y: 8.0,
                enter_scale: 1.0,
                enter_blur: 0.0,
                ..TransitionMotion::still()
            },
            Self::SharedAxisY => TransitionMotion {
                exit_y: -24.0,
                exit_scale: 1.0,
                exit_blur: 0.0,
                enter_y: 24.0,
                enter_scale: 1.0,
                enter_blur: 0.0,
                ..TransitionMotion::still()
            },
            Self::SharedAxisZ => TransitionMotion {
                exit_y: 0.0,
                exit_scale: 1.06,
                exit_blur: 1.0,
                enter_y: 0.0,
                enter_scale: 0.9,
                enter_blur: 2.0,
                ..TransitionMotion::still()
            },
            // A cut is free of any drift, scale or blur.
            Self::Cut => TransitionMotion::still(),
        }
    }

    /// The preset's two curves.
    #[must_use]
    pub fn easing(self) -> TransitionEasing {
        match self {
            Self::FlyThrough => TransitionEasing {
                // This is the *travel*, not the scale — how far the line has moved toward the eye.
                // Fitted to the reference: near-constant speed with a little acceleration off the
                // mark. All the drama is the perspective, not the curve.
                exit: Ease::Curve(CubicBezier::new(1.0, 0.65, 0.85, 1.0)),
                // A moderate decelerate. Not an expo/quint-out: over a 16-frame settle those spend
                // most of their frames moving less than a pixel, which renders as identical frames
                // — the text visibly stops dead and waits.
                enter: Ease::Curve(CubicBezier::new(0.2, 0.6, 0.35, 1.0)),
            },
            Self::FadeThrough | Self::SharedAxisY | Self::SharedAxisZ => TransitionEasing {
                exit: Ease::Curve(CubicBezier::new(0.4, 0.0, 1.0, 1.0)),
                enter: Ease::Curve(CubicBezier::new(0.2, 0.0, 0.0, 1.0)),
            },
            Self::Crossfade => TransitionEasing {
                exit: Ease::Curve(CubicBezier::new(0.7, 0.0, 0.84, 0.0)),
                enter: Ease::Curve(CubicBezier::new(0.16, 1.0, 0.3, 1.0)),
            },
            Self::Cut => TransitionEasing {
                exit: Ease::Step1,
                enter: Ease::Step1,
            },
        }
    }
}

/// One segment's animated channels at a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentChannels {
    /// Y offset in reference-canvas px.
    pub y: f32,
    /// Scale about the segment's own box centre.
    pub scale: f32,
    /// Opacity, `0..=1`.
    pub opacity: f32,
    /// Blur radius in reference-canvas px.
    pub blur: f32,
}

/// Configuration for [`TextSwap`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextSwapConfig {
    /// The line that leaves.
    pub from_text: String,
    /// The line that arrives.
    pub to_text: String,
    /// Which preset drives the swap.
    pub transition: TextSwapTransition,
    /// Whether a line animates as one block or word by word.
    ///
    /// `config.ts` always sends its `unit` control, whose default is `Block`; the source's own
    /// fallback is the preset's unit, which [`TextSwapConfig::for_transition`] applies.
    pub unit: TextSwapUnit,
    /// Frames each outgoing segment takes to exit.
    pub exit_duration: f32,
    /// Frames each incoming segment takes to enter.
    pub enter_duration: f32,
    /// Frames between one outgoing segment starting and the next. Only bites in the word unit.
    pub exit_stagger: f32,
    /// Frames between one incoming segment starting and the next. Only bites in the word unit.
    pub enter_stagger: f32,
    /// Frames the incoming line's start is pulled back *into* the exit.
    pub overlap: f32,
    /// The beat inserted between the two moments, so the swap reads as two and not as one.
    pub micro_delay: f32,
    /// Font size in reference-canvas px.
    pub font_size: f32,
    /// Ink. `None` takes the theme's `foreground`.
    pub color: Option<String>,
    /// Font weight.
    pub font_weight: u16,
    /// Letter spacing in em.
    pub tracking_em: f32,
    /// Timeline multiplier. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// The face to paint in. `None` keeps the theme's face, or the crate's embedded face.
    pub font_family: Option<String>,
    /// Composition length in frames.
    pub duration: usize,
    /// Whether to draw the blur stand-in.
    pub simulate_blur: bool,
}

impl Default for TextSwapConfig {
    fn default() -> Self {
        Self {
            // `config.ts`'s demo copy and its own control defaults, field for field.
            from_text: "Manual invoicing".to_owned(),
            to_text: "Automated with Acme".to_owned(),
            transition: TextSwapTransition::FlyThrough,
            unit: TextSwapUnit::Block,
            exit_duration: 20.0,
            enter_duration: 16.0,
            exit_stagger: 1.0,
            enter_stagger: 2.0,
            overlap: 1.0,
            micro_delay: 2.0,
            font_size: 72.0,
            color: Some("#101828".to_owned()),
            font_weight: 600,
            tracking_em: TRACKING_EM,
            speed: 1.0,
            mode: Mode::Light,
            theme: None,
            font_family: None,
            duration: 90,
            simulate_blur: true,
        }
    }
}

impl TextSwapConfig {
    /// The shipped configuration with `transition`'s **own** unit and timings.
    ///
    /// This is the source component's resolution — `unit ?? defaults.unit`, and the same `??` for
    /// the two durations — for a caller who wants a preset as it was designed rather than as
    /// `config.ts` pins it. (The customizer sends every control, so its shipped default is
    /// [`TextSwapConfig::default`], whose values happen to be `fly-through`'s.)
    #[must_use]
    pub fn for_transition(transition: TextSwapTransition) -> Self {
        let defaults = transition.defaults();
        Self {
            transition,
            unit: defaults.unit,
            exit_duration: defaults.exit_duration,
            enter_duration: defaults.enter_duration,
            ..Self::default()
        }
    }

    /// The frame the last outgoing segment finishes exiting.
    #[must_use]
    pub fn exit_total(&self) -> f32 {
        exit_total(
            self.exit_duration,
            split_segments(&self.from_text, self.unit).len(),
            self.exit_stagger,
        )
    }

    /// The frame the incoming line starts moving.
    #[must_use]
    pub fn enter_start(&self) -> f32 {
        enter_start(
            self.exit_duration,
            split_segments(&self.from_text, self.unit).len(),
            self.exit_stagger,
            self.overlap,
            self.micro_delay,
        )
    }

    /// The frame the last incoming segment lands on: the frame everything has settled.
    ///
    /// For the shipped default this is **37**, which is what `registry/__measured__.json` records
    /// for this component (`settle.base`).
    #[must_use]
    pub fn settle_frame(&self) -> f32 {
        settle_frame(
            self.enter_start(),
            split_segments(&self.to_text, self.unit).len(),
            self.enter_stagger,
            self.enter_duration,
        )
    }
}

/// Split a line into its animatable segments for a unit — the source's `splitSegments`.
///
/// A single space is the separator and only a single space, exactly as in the source, so a double
/// space yields an empty segment. That segment is still a slot in the schedule — the source's empty
/// inline-block carries the same `0.25em` margin and the same place in the stagger — it simply
/// draws no ink.
#[must_use]
pub fn split_segments(text: &str, unit: TextSwapUnit) -> Vec<String> {
    match unit {
        TextSwapUnit::Word => text.split(' ').map(str::to_owned).collect(),
        TextSwapUnit::Block => vec![text.to_owned()],
    }
}

/// The frame the last outgoing segment finishes exiting — the source's `exitTotal`.
#[must_use]
pub fn exit_total(exit_duration: f32, segment_count: usize, exit_stagger: f32) -> f32 {
    exit_duration + segment_count.saturating_sub(1) as f32 * exit_stagger
}

/// The frame the incoming line starts moving — the source's `getEnterStart`.
///
/// The incoming text starts once the outgoing text, including its stagger tail, is almost gone,
/// minus `overlap`, plus a `microDelay` beat.
#[must_use]
pub fn enter_start(
    exit_duration: f32,
    segment_count: usize,
    exit_stagger: f32,
    overlap: f32,
    micro_delay: f32,
) -> f32 {
    (exit_total(exit_duration, segment_count, exit_stagger) - overlap + micro_delay).max(0.0)
}

/// The frame the last incoming segment lands on.
#[must_use]
pub fn settle_frame(
    enter_start: f32,
    segment_count: usize,
    enter_stagger: f32,
    enter_duration: f32,
) -> f32 {
    enter_start + segment_count.saturating_sub(1) as f32 * enter_stagger + enter_duration
}

/// Apparent size of something travelling toward the camera.
///
/// `travel` is `0` at rest and `1` at the moment it reaches the eye; `max_scale` is how big it gets
/// before it is gone. Size goes as `1 / (1 - travel)`, so it creeps for most of the trip and then
/// blows up right at the end — the whole character of a thing rushing past you.
///
/// This is why the rush is not a plain `interpolate(…, [1, exit_scale])`. Fitted against the
/// reference, that shape is off by rmse 0.31 and *cannot* reach the blowup at all; this one lands
/// where the reference measured.
#[must_use]
pub fn perspective_scale(travel: f32, max_scale: f32) -> f32 {
    if max_scale <= 1.0 {
        return 1.0;
    }
    let p = 1.0 - 1.0 / max_scale;
    // Clamp short of 1: at `travel = 1 / p` the denominator is 0 and the line is already long gone.
    1.0 / (1.0 - p * travel.clamp(0.0, 0.9999))
}

/// A line laid out the way the browser lays it out: the ink width of every segment, the space
/// between them, and the trailing margin the source leaves on the last one.
#[derive(Debug, Clone, PartialEq)]
pub struct LineSlots {
    /// Ink width of every segment, in reference-canvas px.
    pub widths: Vec<f32>,
    /// The space between two segments, in reference-canvas px.
    pub gap: f32,
    /// The margin after the last segment, in reference-canvas px.
    pub trailing: f32,
    /// Every segment's centre, in reference-canvas px from the line's own centre.
    pub centers: Vec<f32>,
}

impl LineSlots {
    /// Lay `widths` out centred, with `gap` between the segments and `trailing` after the last.
    ///
    /// The source lays its segments out as inline-blocks inside a flex-centred span and gives
    /// **every** one of them `margin-right: 0.25em`, the last included — so the line's box is one
    /// gap wider than its ink and the ink sits half a gap left of the centre. `trailing` carries
    /// that; `0` is a line with no such margin (the block unit).
    #[must_use]
    pub fn new(widths: Vec<f32>, gap: f32, trailing: f32) -> Self {
        let count = widths.len();
        let total = widths.iter().sum::<f32>() + gap * count.saturating_sub(1) as f32 + trailing;
        let mut centers = Vec::with_capacity(count);
        let mut cursor = -total / 2.0;
        for width in &widths {
            centers.push(cursor + width / 2.0);
            cursor += width + gap;
        }
        Self {
            widths,
            gap,
            trailing,
            centers,
        }
    }
}

/// `interpolate(frame, [from, to], [0, 1])` with a curve and clamping, which is what the source's
/// Remotion `interpolate` does: the input is clamped into the window, then the curve is sampled.
///
/// A zero-length window has no answer in the source — it divides by it, and CSS drops the resulting
/// `NaN`, leaving the channel at whatever it already was. `empty` carries that start value.
fn eased_progress(frame: f32, from: f32, to: f32, empty: f32, ease: Ease) -> f32 {
    if (to - from).abs() < f32::EPSILON {
        return empty;
    }
    ease.ease(((frame - from) / (to - from)).clamp(0.0, 1.0))
}

/// The outgoing line's channels for one segment, `local` frames into that segment's own exit.
///
/// `local` is `frame - segment_index * exit_stagger`, as the source computes it.
#[must_use]
pub fn exit_channels(
    local: f32,
    duration: f32,
    motion: &TransitionMotion,
    ease: Ease,
) -> SegmentChannels {
    // 0 at rest, 1 at the eye. The easing shapes the *travel*; the perspective (or the plain ramp)
    // turns that into apparent size.
    let travel = eased_progress(local, 0.0, duration, 0.0, ease);
    // The fade window starts at `exit_fade_start` of the exit, and is shaped by the same curve over
    // that shorter span — so the rush holds solid until it is on top of you and then it is gone.
    let fade_from = motion.exit_fade_start * duration;
    let opacity = 1.0 - eased_progress(local, fade_from, duration, 0.0, ease);
    let scale = if motion.exit_perspective {
        perspective_scale(travel, motion.exit_scale)
    } else {
        1.0 + (motion.exit_scale - 1.0) * travel
    };
    SegmentChannels {
        y: motion.exit_y * travel,
        scale,
        opacity,
        blur: motion.exit_blur * travel,
    }
}

/// The incoming line's channels for one segment, `local` frames into that segment's own enter.
#[must_use]
pub fn enter_channels(
    local: f32,
    duration: f32,
    motion: &TransitionMotion,
    ease: Ease,
) -> SegmentChannels {
    let progress = eased_progress(local, 0.0, duration, 1.0, ease);
    SegmentChannels {
        y: motion.enter_y * (1.0 - progress),
        scale: motion.enter_scale + (1.0 - motion.enter_scale) * progress,
        opacity: progress,
        blur: motion.enter_blur * (1.0 - progress),
    }
}

/// The frames the shutter sees, and the share of the exposure each one carries.
///
/// The source samples the exit `exit_trail` times across a one-frame shutter and blends the samples
/// with `plus-lighter`, each at `1 / n`. Samples that land on the **same** state are merged here
/// and their shares summed, which is exactly what additive blending gives them; without that merge
/// ordinary alpha compositing would leave a line at rest at `1 - (1 - 1 / n)^n` — 64% at the
/// source's 18 samples — instead of solid.
#[must_use]
pub fn shutter_samples(
    frame: f32,
    duration: f32,
    motion: &TransitionMotion,
    ease: Ease,
) -> Vec<(f32, f32)> {
    let trail = motion.exit_trail.max(1);
    let share = 1.0 / trail as f32;
    let mut groups: Vec<(f32, f32)> = Vec::with_capacity(trail);
    for sample in 0..trail {
        let sample_frame = frame - (sample as f32 / trail as f32) * SHUTTER;
        let channels = exit_channels(sample_frame, duration, motion, ease);
        match groups.last_mut() {
            Some((previous, weight))
                if same_channels(&exit_channels(*previous, duration, motion, ease), &channels) =>
            {
                *weight += share;
            }
            _ => groups.push((sample_frame, share)),
        }
    }
    groups
}

/// Whether two sets of channels are the same state, to within float noise.
fn same_channels(a: &SegmentChannels, b: &SegmentChannels) -> bool {
    const EPSILON: f32 = 1e-6;
    (a.y - b.y).abs() <= EPSILON
        && (a.scale - b.scale).abs() <= EPSILON
        && (a.opacity - b.opacity).abs() <= EPSILON
        && (a.blur - b.blur).abs() <= EPSILON
}

/// One line, measured once: its segments and where each one sits.
#[derive(Debug, Clone, PartialEq)]
struct Line {
    /// The segments, in the order the source lays them out.
    segments: Vec<String>,
    /// Where every segment sits, in reference-canvas px from the line's centre.
    slots: LineSlots,
}

/// Both lines of a swap, measured once per render.
#[derive(Debug, Clone, PartialEq)]
struct Measured {
    /// The outgoing line.
    from: Line,
    /// The incoming line.
    to: Line,
}

/// Everything a segment needs to be drawn that does not change from segment to segment.
struct SegmentStyle<'a> {
    /// The resolved face.
    family: &'a str,
    /// Font size in canvas px.
    size: f32,
    /// Font weight.
    weight: u16,
    /// Letter spacing in canvas px.
    tracking_px: f32,
    /// Ink.
    fill: &'a str,
    /// The baseline the line sits on, in canvas px.
    baseline: f32,
    /// The line box's vertical centre, in canvas px — the scale's y pivot.
    center_y: f32,
    /// The canvas scale factor for reference-canvas px.
    unit: f32,
    /// Whether to draw the blur stand-in.
    simulate_blur: bool,
}

/// Draw one segment of a line, with its own translate and scale about its own box centre.
///
/// The source puts `translate` and `scale` on the segment as CSS individual transform properties
/// with `transform-origin: 50% 50%`, so a point of the segment maps to
/// `origin + translate + scale * (point - origin)` — the translate is *outside* the scale and is
/// therefore never scaled. The emitted transform says exactly that.
fn draw_segment(
    nodes: &mut Vec<Svgr<'static>>,
    ctx: &RenderCtx<'_>,
    style: &SegmentStyle<'_>,
    body: &str,
    x: f32,
    channels: SegmentChannels,
) {
    // An empty segment is a slot with a margin and no ink, exactly as the source's empty span is.
    if body.is_empty() || channels.opacity <= 0.0 {
        return;
    }

    let mut parts: Vec<Svgr<'static>> = Vec::new();
    if style.simulate_blur && blur::is_visible(channels.blur) {
        // The blur is applied in the segment's own space and the scale then multiplies it, so the
        // copies live *inside* the transform. That is what CSS does with `filter` on a transformed
        // element, and it is what makes the rush's blur grow with the rush.
        let alpha = blur::ghost_alpha(channels.blur, style.unit) * channels.opacity;
        for (dx, dy) in blur::ghost_offsets(channels.blur, style.unit) {
            parts.push(text::text_node(
                ctx,
                body,
                x + dx,
                style.baseline + dy,
                style.family.to_owned(),
                style.size,
                style.weight,
                style.tracking_px,
                style.fill.to_owned(),
                alpha,
            ));
        }
    }
    parts.push(text::text_node(
        ctx,
        body,
        x,
        style.baseline,
        style.family.to_owned(),
        style.size,
        style.weight,
        style.tracking_px,
        style.fill.to_owned(),
        channels.opacity,
    ));

    let group = Svgr::from(parts);
    let shift = channels.y * style.unit;
    if (channels.scale - 1.0).abs() < 1e-4 && shift.abs() < 1e-4 {
        nodes.push(group);
        return;
    }
    let (nx, ny) = (-x, -style.center_y);
    let ty = style.center_y + shift;
    let scale = channels.scale;
    let transform = format!("translate({x} {ty}) scale({scale}) translate({nx} {ny})");
    nodes.push(fframes::svgr!(<g transform={transform}>{group}</g>));
}

/// The **Cinematic Text Swap** component.
#[derive(Debug, Clone, Default)]
pub struct TextSwap {
    /// Component options.
    pub config: TextSwapConfig,
    measured: OnceLock<Measured>,
}

impl TextSwap {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: TextSwapConfig) -> Self {
        Self {
            config,
            measured: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "cinematic-text-swap",
            name: "Cinematic Text Swap",
            summary: "One line exits and the next enters, on a shared schedule: rush, crossfade, share an axis, or cut",
            feature: "cinematic-text-swap",
        }
    }

    /// Measure both lines once, in reference-canvas px, and hold them for every later frame.
    ///
    /// Every segment is measured **at the weight it is drawn at**, because a run drawn at one
    /// weight and measured at another is centred and spaced against the wrong width.
    fn measured(&self, ctx: &RenderCtx<'_>, family: &str) -> &Measured {
        self.measured.get_or_init(|| {
            let config = &self.config;
            let size = config.font_size * text::unit(ctx);
            let tracking_px = text::tracking_px_from_em(config.tracking_em, size);
            // The gap between two words is the source's `0.25em` margin plus the tracking that CSS
            // leaves after the previous word's last glyph.
            let gap = WORD_GAP_EM * size + tracking_px;
            // Only a word segment carries the margin; the block unit's single span does not.
            let trailing = if config.unit == TextSwapUnit::Word {
                gap
            } else {
                0.0
            };
            let line = |source: &str| {
                let segments = split_segments(source, config.unit);
                let widths = segments
                    .iter()
                    .map(|segment| {
                        text::tracked_width_at(
                            ctx,
                            family,
                            config.font_weight,
                            size,
                            segment,
                            tracking_px,
                        )
                    })
                    .collect();
                Line {
                    segments,
                    slots: LineSlots::new(widths, gap, trailing),
                }
            };
            Measured {
                from: line(&config.from_text),
                to: line(&config.to_text),
            }
        })
    }
}

impl Component for TextSwap {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source paints no background of its own; its preview stage uses `#FFFFFF`.
        Some("#ffffff")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let unit = text::unit(ctx);
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let fill = config
            .color
            .clone()
            .unwrap_or_else(|| theme.foreground.clone());
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);

        let size = config.font_size * unit;
        let tracking_px = text::tracking_px_from_em(config.tracking_em, size);
        let frame = ctx.frame() * normalised_speed(config.speed);
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let baseline = text::baseline_for_centred_box(ctx, &family, size, center_y);

        let measured = self.measured(ctx, &family);
        let motion = config.transition.motion();
        let easing = config.transition.easing();
        let enter_start = config.enter_start();

        let style = SegmentStyle {
            family: &family,
            size,
            weight: config.font_weight,
            tracking_px,
            fill: &fill,
            baseline,
            center_y,
            unit,
            simulate_blur: config.simulate_blur,
        };

        let mut nodes: Vec<Svgr<'static>> = Vec::new();

        // The outgoing line, sampled across the shutter. Each sample is one whole copy of the line.
        for (sample_frame, share) in
            shutter_samples(frame, config.exit_duration, &motion, easing.exit)
        {
            for (index, segment) in measured.from.segments.iter().enumerate() {
                let local = sample_frame - index as f32 * config.exit_stagger;
                let channels = exit_channels(local, config.exit_duration, &motion, easing.exit);
                let channels = SegmentChannels {
                    opacity: (share * channels.opacity).min(1.0),
                    ..channels
                };
                let x = center_x + measured.from.slots.centers[index] * unit;
                draw_segment(&mut nodes, ctx, &style, segment, x, channels);
            }
        }

        // The incoming line.
        for (index, segment) in measured.to.segments.iter().enumerate() {
            let local = frame - enter_start - index as f32 * config.enter_stagger;
            let channels = enter_channels(local, config.enter_duration, &motion, easing.enter);
            let x = center_x + measured.to.slots.centers[index] * unit;
            draw_segment(&mut nodes, ctx, &style, segment, x, channels);
        }

        let nodes = Svgr::from(nodes);
        // A rendered composition clips its output to the frame — the source's `<Player>` and
        // Remotion's renderer both do — which is what lets the rush blow straight past the canvas
        // edge without becoming a layout defect. Emitting that clip is the honest statement of it.
        let width = ctx.width();
        let height = ctx.height();
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id="cinematic-text-swap-stage">
                        <rect x="0" y="0" width={width} height={height} />
                    </clipPath>
                </defs>
                <g clip-path="url(#cinematic-text-swap-stage)">{nodes}</g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every preset the source ships.
    const PRESETS: [TextSwapTransition; 6] = [
        TextSwapTransition::FlyThrough,
        TextSwapTransition::FadeThrough,
        TextSwapTransition::Crossfade,
        TextSwapTransition::SharedAxisY,
        TextSwapTransition::SharedAxisZ,
        TextSwapTransition::Cut,
    ];

    #[test]
    fn duration_defaults_to_the_source_composition_length() {
        let component = TextSwap::default();
        assert_eq!(component.duration(), 90);
        assert_eq!(component.meta().id, "cinematic-text-swap");
        assert_eq!(component.meta().feature, "cinematic-text-swap");
        assert_eq!(component.background(), Some("#ffffff"));
    }

    #[test]
    fn the_source_defaults_are_reproduced_exactly() {
        let config = TextSwapConfig::default();
        assert_eq!(config.from_text, "Manual invoicing");
        assert_eq!(config.to_text, "Automated with Acme");
        assert_eq!(config.transition, TextSwapTransition::FlyThrough);
        assert_eq!(config.unit, TextSwapUnit::Block);
        assert!((config.exit_duration - 20.0).abs() < 1e-6);
        assert!((config.enter_duration - 16.0).abs() < 1e-6);
        assert!((config.exit_stagger - 1.0).abs() < 1e-6);
        assert!((config.enter_stagger - 2.0).abs() < 1e-6);
        assert!((config.overlap - 1.0).abs() < 1e-6);
        assert!((config.micro_delay - 2.0).abs() < 1e-6);
        assert!((config.font_size - 72.0).abs() < 1e-6);
        assert_eq!(config.color.as_deref(), Some("#101828"));
        assert_eq!(config.font_weight, 600);
        assert!((config.tracking_em + 0.02).abs() < 1e-6);
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.theme, None);
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 90);
        assert!(config.simulate_blur);
    }

    #[test]
    fn the_default_copy_settles_on_the_measured_frame() {
        // `registry/__measured__.json` records settle.base = 37 for this component's shipped
        // configuration, and the source's own schedule has to reproduce it. The composition runs
        // on to 90, so the rest is a hold.
        let config = TextSwapConfig::default();
        assert_eq!(config.from_text.split(' ').count(), 2, "two words leave");
        assert_eq!(config.to_text.split(' ').count(), 3, "three arrive");
        assert!((config.exit_total() - 20.0).abs() < 1e-6);
        assert!((config.enter_start() - 21.0).abs() < 1e-6);
        assert!((config.settle_frame() - 37.0).abs() < 1e-6);
    }

    #[test]
    fn the_block_unit_schedule_does_not_depend_on_the_word_count() {
        // Which is why the registry's linear fit reports `perWord: 0` for this component: the
        // shipped default animates as one block, so an extra word in either line changes nothing.
        // (`baseWords` is that fit's own parameter and is not a claim about the demo copy.)
        let config = TextSwapConfig::default();
        let settle = config.settle_frame();
        for (from, to) in [
            ("one", "two"),
            ("a much longer line of words", "another line entirely"),
            ("Manual invoicing and then some", "Automated with Acme"),
        ] {
            let wider = TextSwapConfig {
                from_text: from.to_owned(),
                to_text: to.to_owned(),
                ..TextSwapConfig::default()
            };
            assert!(
                (wider.settle_frame() - settle).abs() < 1e-6,
                "{from:?} -> {to:?} settled on {} instead of {settle}",
                wider.settle_frame()
            );
        }
    }

    #[test]
    fn each_preset_carries_the_source_timing_and_unit() {
        let expected = [
            (TextSwapUnit::Block, 20.0, 16.0),
            (TextSwapUnit::Block, 8.0, 13.0),
            (TextSwapUnit::Word, 15.0, 21.0),
            (TextSwapUnit::Block, 10.0, 14.0),
            (TextSwapUnit::Block, 11.0, 16.0),
            (TextSwapUnit::Word, 8.0, 8.0),
        ];
        for (preset, (unit, exit, enter)) in PRESETS.iter().zip(expected) {
            let defaults = preset.defaults();
            assert_eq!(defaults.unit, unit, "{preset:?} unit");
            assert!(
                (defaults.exit_duration - exit).abs() < 1e-6,
                "{preset:?} exit"
            );
            assert!(
                (defaults.enter_duration - enter).abs() < 1e-6,
                "{preset:?} enter"
            );
            // The customizer's own resolution, which `for_transition` reproduces.
            let config = TextSwapConfig::for_transition(*preset);
            assert_eq!(config.unit, unit);
            assert!((config.exit_duration - exit).abs() < 1e-6);
            assert!((config.enter_duration - enter).abs() < 1e-6);
            // Everything the preset does not own stays on `config.ts`'s value.
            assert_eq!(config.from_text, TextSwapConfig::default().from_text);
            assert!((config.exit_stagger - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn each_preset_carries_the_source_motion_targets() {
        // exit_y, exit_scale, exit_blur, enter_y, enter_scale, enter_blur
        let expected = [
            (0.0, 12.0, 0.5, 0.0, 0.82, 9.0),
            (-4.0, 1.0, 0.0, 6.0, 0.99, 2.0),
            (-6.0, 1.0, 0.0, 8.0, 1.0, 0.0),
            (-24.0, 1.0, 0.0, 24.0, 1.0, 0.0),
            (0.0, 1.06, 1.0, 0.0, 0.9, 2.0),
            (0.0, 1.0, 0.0, 0.0, 1.0, 0.0),
        ];
        for (preset, (exit_y, exit_scale, exit_blur, enter_y, enter_scale, enter_blur)) in
            PRESETS.iter().zip(expected)
        {
            let motion = preset.motion();
            assert!((motion.exit_y - exit_y).abs() < 1e-6, "{preset:?} exit_y");
            assert!(
                (motion.exit_scale - exit_scale).abs() < 1e-6,
                "{preset:?} exit_scale"
            );
            assert!(
                (motion.exit_blur - exit_blur).abs() < 1e-6,
                "{preset:?} exit_blur"
            );
            assert!(
                (motion.enter_y - enter_y).abs() < 1e-6,
                "{preset:?} enter_y"
            );
            assert!(
                (motion.enter_scale - enter_scale).abs() < 1e-6,
                "{preset:?} enter_scale"
            );
            assert!(
                (motion.enter_blur - enter_blur).abs() < 1e-6,
                "{preset:?} enter_blur"
            );
        }
    }

    #[test]
    fn only_the_fly_through_rushes_and_only_it_uses_the_shutter() {
        for preset in PRESETS {
            let motion = preset.motion();
            let rushes = preset == TextSwapTransition::FlyThrough;
            assert_eq!(motion.exit_perspective, rushes, "{preset:?} perspective");
            assert_eq!(motion.exit_trail > 1, rushes, "{preset:?} trail");
            assert_eq!(
                motion.exit_fade_start > 0.0,
                rushes,
                "{preset:?} fade start"
            );
            if rushes {
                // It holds solid almost the whole way in, then goes.
                assert!(motion.exit_fade_start > 0.5);
                assert!(motion.exit_scale > 5.0);
                assert_eq!(motion.exit_trail, 18);
                assert!((motion.exit_fade_start - 0.72).abs() < 1e-6);
            } else {
                assert_eq!(motion.exit_trail, 1);
                assert!((motion.exit_fade_start).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn each_preset_carries_the_source_easings() {
        let shared = TransitionEasing {
            exit: Ease::Curve(CubicBezier::new(0.4, 0.0, 1.0, 1.0)),
            enter: Ease::Curve(CubicBezier::new(0.2, 0.0, 0.0, 1.0)),
        };
        let expected = [
            TransitionEasing {
                exit: Ease::Curve(CubicBezier::new(1.0, 0.65, 0.85, 1.0)),
                enter: Ease::Curve(CubicBezier::new(0.2, 0.6, 0.35, 1.0)),
            },
            shared,
            TransitionEasing {
                exit: Ease::Curve(CubicBezier::new(0.7, 0.0, 0.84, 0.0)),
                enter: Ease::Curve(CubicBezier::new(0.16, 1.0, 0.3, 1.0)),
            },
            shared,
            shared,
            TransitionEasing {
                exit: Ease::Step1,
                enter: Ease::Step1,
            },
        ];
        for (preset, easing) in PRESETS.iter().zip(expected) {
            assert_eq!(preset.easing(), easing, "{preset:?}");
        }
    }

    #[test]
    fn split_segments_follows_the_source_unit() {
        assert_eq!(
            split_segments("Manual invoicing today", TextSwapUnit::Word),
            vec!["Manual", "invoicing", "today"]
        );
        assert_eq!(
            split_segments("Manual invoicing today", TextSwapUnit::Block),
            vec!["Manual invoicing today"]
        );
        assert_eq!(split_segments("Acme", TextSwapUnit::Word), vec!["Acme"]);
        assert_eq!(split_segments("Acme", TextSwapUnit::Block), vec!["Acme"]);
        // A single space is the separator and only a single space, so a double space is an empty
        // segment — a slot with a margin and no ink, exactly as the source's empty span is.
        assert_eq!(
            split_segments("Manual  invoicing", TextSwapUnit::Word),
            vec!["Manual", "", "invoicing"]
        );
    }

    #[test]
    fn get_enter_start_follows_the_source_schedule() {
        // The four cases from the source's own `__tests__`.
        assert!(
            (enter_start(8.0, 1, 1.0, 1.0, 2.0) - 9.0).abs() < 1e-6,
            "a single block: exitTotal 8 - 1 + 2"
        );
        assert!(
            (enter_start(15.0, 3, 1.0, 5.0, 2.0) - 14.0).abs() < 1e-6,
            "exitTotal = 15 + 2 * 1 = 17, then 17 - 5 + 2"
        );
        assert!(
            (enter_start(2.0, 1, 0.0, 10.0, 0.0) - 0.0).abs() < 1e-6,
            "the enter is never scheduled before frame 0"
        );
        assert!(
            (enter_start(8.0, 0, 4.0, 0.0, 0.0) - 8.0).abs() < 1e-6,
            "a zero segment count adds no stagger tail and does not underflow"
        );
    }

    #[test]
    fn exit_total_extends_by_the_stagger_tail() {
        assert!((exit_total(8.0, 1, 1.0) - 8.0).abs() < 1e-6);
        assert!((exit_total(15.0, 3, 1.0) - 17.0).abs() < 1e-6);
        assert!((exit_total(15.0, 2, 2.0) - 17.0).abs() < 1e-6);
        assert!((settle_frame(21.0, 1, 2.0, 16.0) - 37.0).abs() < 1e-6);
        assert!((settle_frame(21.0, 3, 2.0, 16.0) - 41.0).abs() < 1e-6);
    }

    #[test]
    fn the_per_preset_schedules_are_the_sources() {
        // `for_transition` resolves each preset's own unit and timings against `config.ts`'s copy
        // and staggers, which is what the source component does when its props are omitted.
        let expected = [
            // exit_total, enter_start, settle
            (20.0, 21.0, 37.0),
            (8.0, 9.0, 22.0),
            // crossfade is the word unit: 2 words leave, 3 arrive, 2 frames between the arrivals.
            (16.0, 17.0, 42.0),
            (10.0, 11.0, 25.0),
            (11.0, 12.0, 28.0),
            (9.0, 10.0, 22.0),
        ];
        for (preset, (exit, start, settle)) in PRESETS.iter().zip(expected) {
            let config = TextSwapConfig::for_transition(*preset);
            assert!(
                (config.exit_total() - exit).abs() < 1e-6,
                "{preset:?} exit total {}",
                config.exit_total()
            );
            assert!(
                (config.enter_start() - start).abs() < 1e-6,
                "{preset:?} enter start {}",
                config.enter_start()
            );
            assert!(
                (config.settle_frame() - settle).abs() < 1e-6,
                "{preset:?} settle {}",
                config.settle_frame()
            );
        }
    }

    #[test]
    fn the_word_unit_extends_the_exit_by_its_stagger_tail() {
        // 15 frames each, two words, one frame apart: the last word is still exiting on frame 16.
        let config = TextSwapConfig::for_transition(TextSwapTransition::Crossfade);
        assert_eq!(config.unit, TextSwapUnit::Word);
        assert_eq!(split_segments(&config.from_text, config.unit).len(), 2);
        assert!((config.exit_duration - 15.0).abs() < 1e-6);
        assert!((config.exit_total() - 16.0).abs() < 1e-6);
    }

    #[test]
    fn perspective_scale_is_one_at_rest_and_max_scale_at_the_eye() {
        assert!((perspective_scale(0.0, 12.0) - 1.0).abs() < 1e-6);
        assert!((perspective_scale(1.0, 12.0) - 12.0).abs() < 0.05);
        // A no-op max scale degrades to 1, and nothing here divides by zero.
        assert!((perspective_scale(0.5, 1.0) - 1.0).abs() < 1e-6);
        assert!((perspective_scale(0.5, 0.0) - 1.0).abs() < 1e-6);
        for travel in [-1.0, 0.0, 0.5, 1.0, 2.0] {
            assert!(
                perspective_scale(travel, 12.0).is_finite(),
                "travel {travel}"
            );
        }
        // Out-of-range travel clamps rather than running away: the growth is capped at ~12x.
        assert!(perspective_scale(2.0, 12.0) <= 12.0);
        assert!(perspective_scale(2.0, 12.0) > 11.0);
    }

    #[test]
    fn perspective_scale_creeps_early_and_blows_up_late() {
        // A plain ramp would be at the halfway size by halfway. Perspective is not: it is still
        // small at half travel and then runs away at the end.
        let half = perspective_scale(0.5, 12.0);
        let linear_half = 1.0 + (12.0 - 1.0) * 0.5;
        assert!(half < linear_half / 2.0, "half travel gives {half}");
        assert!(perspective_scale(0.9, 12.0) > perspective_scale(0.8, 12.0) * 1.5);
    }

    #[test]
    fn perspective_scale_reproduces_the_references_blowup() {
        // Frame-by-frame off the reference recording: at 98.5% of the way through the exit the
        // line measured 10.28x its resting size. The model lands at 10.30; a linear ramp at the
        // same point would only be at 11.8, and it gets there by marching steadily, with none of
        // the late rush that reads as something passing your face.
        let scale = perspective_scale(0.985, 12.0);
        assert!(
            (10.0..10.6).contains(&scale),
            "the model lands on {scale}, the reference measured 10.28"
        );
        // And the curve's own travel at 95% of the exit window, which is what feeds it: the
        // remaining 5% of the window is worth the last fifth of the travel.
        let travel = TextSwapTransition::FlyThrough.easing().exit.ease(0.95);
        assert!(
            (0.97..0.99).contains(&travel),
            "the curve has spent {travel} of the travel at 95% of the window"
        );
    }

    #[test]
    fn each_preset_reaches_its_motion_targets_by_the_end_of_each_window() {
        for preset in PRESETS {
            let motion = preset.motion();
            let easing = preset.easing();
            let defaults = preset.defaults();
            let exit = defaults.exit_duration;
            let enter = defaults.enter_duration;

            // At rest the exit has not moved: the line is exactly where it stands.
            let at_rest = exit_channels(0.0, exit, &motion, easing.exit);
            assert!((at_rest.y).abs() < 1e-6, "{preset:?} y at rest");
            assert!(
                (at_rest.scale - 1.0).abs() < 1e-6,
                "{preset:?} scale at rest"
            );
            assert!(
                (at_rest.opacity - 1.0).abs() < 1e-6,
                "{preset:?} opacity at rest"
            );
            assert!((at_rest.blur).abs() < 1e-6, "{preset:?} blur at rest");

            // And by the end of the window it is exactly on its targets, and gone.
            let gone = exit_channels(exit, exit, &motion, easing.exit);
            assert!((gone.y - motion.exit_y).abs() < 1e-4, "{preset:?} exit y");
            assert!(
                (gone.scale - motion.exit_scale).abs() < 0.05,
                "{preset:?} exit scale {} vs {}",
                gone.scale,
                motion.exit_scale
            );
            assert!((gone.opacity).abs() < 1e-6, "{preset:?} exit opacity");
            assert!(
                (gone.blur - motion.exit_blur).abs() < 1e-4,
                "{preset:?} exit blur"
            );
            // The exit never moves backwards out of its window either.
            let past = exit_channels(exit + 50.0, exit, &motion, easing.exit);
            assert!((past.opacity).abs() < 1e-6, "{preset:?} stays gone");

            // The enter starts exactly on its targets, invisible.
            let arriving = enter_channels(0.0, enter, &motion, easing.enter);
            assert!(
                (arriving.y - motion.enter_y).abs() < 1e-6,
                "{preset:?} enter y"
            );
            assert!(
                (arriving.scale - motion.enter_scale).abs() < 1e-6,
                "{preset:?} enter scale"
            );
            assert!((arriving.opacity).abs() < 1e-6, "{preset:?} enter opacity");
            assert!(
                (arriving.blur - motion.enter_blur).abs() < 1e-6,
                "{preset:?} enter blur"
            );

            // And it lands on rest, fully opaque, sharp.
            let landed = enter_channels(enter, enter, &motion, easing.enter);
            assert!((landed.y).abs() < 1e-6, "{preset:?} landed y");
            assert!((landed.scale - 1.0).abs() < 1e-6, "{preset:?} landed scale");
            assert!(
                (landed.opacity - 1.0).abs() < 1e-6,
                "{preset:?} landed opacity"
            );
            assert!((landed.blur).abs() < 1e-6, "{preset:?} landed blur");
        }
    }

    #[test]
    fn the_rush_holds_solid_and_then_goes() {
        // The outgoing line does not dim on the way in: it stays solid until 72% of the exit and
        // then it is gone. Between those two beats it falls monotonically — no dip and no return.
        let motion = TextSwapTransition::FlyThrough.motion();
        let easing = TextSwapTransition::FlyThrough.easing();
        let duration = TextSwapTransition::FlyThrough.defaults().exit_duration;
        let fade_from = motion.exit_fade_start * duration;
        assert!((fade_from - 14.4).abs() < 1e-6);

        let mut previous = 1.0;
        for frame in 0..=200 {
            let frame = frame as f32 / 10.0;
            let channels = exit_channels(frame, duration, &motion, easing.exit);
            assert!(
                channels.opacity <= previous + 1e-6,
                "opacity rose at {frame}"
            );
            if frame <= fade_from {
                assert!(
                    (channels.opacity - 1.0).abs() < 1e-6,
                    "at {frame} the line is already at {}",
                    channels.opacity
                );
            }
            previous = channels.opacity;
        }
        assert!(
            (previous).abs() < 1e-6,
            "the line is still visible at the end"
        );
    }

    #[test]
    fn the_cut_pops_on_the_step_function() {
        // `Easing.step1` holds the outgoing line exactly where it is until the window closes, then
        // removes it in one frame; the incoming line appears in one frame too.
        let motion = TextSwapTransition::Cut.motion();
        let easing = TextSwapTransition::Cut.easing();
        let duration = TextSwapTransition::Cut.defaults().exit_duration;

        let before = exit_channels(duration - 0.001, duration, &motion, easing.exit);
        assert!((before.opacity - 1.0).abs() < 1e-6);
        assert!((before.scale - 1.0).abs() < 1e-6);
        let after = exit_channels(duration, duration, &motion, easing.exit);
        assert!((after.opacity).abs() < 1e-6);

        let waiting = enter_channels(duration - 0.001, duration, &motion, easing.enter);
        assert!((waiting.opacity).abs() < 1e-6);
        let shown = enter_channels(duration, duration, &motion, easing.enter);
        assert!((shown.opacity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_shutters_samples_are_merged_where_they_coincide() {
        let motion = TextSwapTransition::FlyThrough.motion();
        let easing = TextSwapTransition::FlyThrough.easing();
        let duration = TextSwapTransition::FlyThrough.defaults().exit_duration;

        // At rest every sample sees the same state, and the merge leaves one copy at full opacity —
        // which is what `plus-lighter` would give 18 coincident samples, and what plain alpha
        // compositing could not.
        let at_rest = shutter_samples(0.0, duration, &motion, easing.exit);
        assert_eq!(at_rest.len(), 1, "{at_rest:?}");
        assert!((at_rest[0].0 - 0.0).abs() < 1e-6);
        assert!((at_rest[0].1 - 1.0).abs() < 1e-6);

        // Once the line is moving the samples separate, and each carries 1/18 of the exposure.
        let rushing = shutter_samples(18.0, duration, &motion, easing.exit);
        assert_eq!(rushing.len(), 18, "{rushing:?}");
        let total: f32 = rushing.iter().map(|(_, share)| share).sum();
        assert!((total - 1.0).abs() < 1e-6, "shares sum to {total}");
        for (_, share) in &rushing {
            assert!((share - 1.0 / 18.0).abs() < 1e-6);
        }
        // The shutter is a whole frame wide: the samples span exactly SHUTTER frames.
        let newest = rushing[0].0;
        let oldest = rushing[rushing.len() - 1].0;
        assert!((newest - oldest - SHUTTER * 17.0 / 18.0).abs() < 1e-6);

        // Every preset but the rush takes a single sample, so it costs nothing.
        for preset in PRESETS {
            if preset == TextSwapTransition::FlyThrough {
                continue;
            }
            let motion = preset.motion();
            let samples = shutter_samples(
                3.0,
                preset.defaults().exit_duration,
                &motion,
                preset.easing().exit,
            );
            assert_eq!(samples.len(), 1, "{preset:?} {samples:?}");
            assert!((samples[0].1 - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn line_slots_centre_the_line_and_keep_the_trailing_margin() {
        // Three words of 40/60/20 with a 10 px gap and the source's trailing margin of one gap.
        let slots = LineSlots::new(vec![40.0, 60.0, 20.0], 10.0, 10.0);
        assert!((slots.gap - 10.0).abs() < 1e-6);
        assert_eq!(slots.widths.len(), 3);
        // Ink is 120 wide, the two inner gaps 20, the margin 10: a 150 box centred on zero.
        assert!(
            (slots.centers[0] + 55.0).abs() < 1e-6,
            "{:?}",
            slots.centers
        );
        assert!((slots.centers[1] - 5.0).abs() < 1e-6);
        assert!((slots.centers[2] - 55.0).abs() < 1e-6);
        // The box is centred; the *ink* therefore sits half a gap left of centre, because the
        // source leaves the margin on the last word too.
        let ink_left = slots.centers[0] - 20.0;
        let ink_right = slots.centers[2] + 10.0;
        assert!(
            (ink_left + ink_right - (-10.0)).abs() < 1e-6,
            "{ink_left}..{ink_right}"
        );
        assert!((ink_left + 75.0).abs() < 1e-6);
        assert!((ink_right - 65.0).abs() < 1e-6);
    }

    #[test]
    fn a_one_segment_line_is_centred_on_its_ink_when_it_has_no_margin() {
        // The block unit: one span, no `margin-right`, so the ink sits centred and only the gap the
        // caller passes can move it.
        let slots = LineSlots::new(vec![100.0], 18.0, 0.0);
        assert!((slots.centers[0]).abs() < 1e-6);
        assert!((slots.trailing).abs() < 1e-6);
    }

    #[test]
    fn a_zero_length_window_leaves_each_channel_where_it_started() {
        // `config.ts` floors both durations at 2, but a caller can still pass 0, and the source
        // divides by the window there and lets CSS drop the `NaN` — which leaves the channel alone.
        let motion = TextSwapTransition::FadeThrough.motion();
        let easing = TextSwapTransition::FadeThrough.easing();
        let exit = exit_channels(0.0, 0.0, &motion, easing.exit);
        assert!(
            (exit.opacity - 1.0).abs() < 1e-6,
            "the from-line stays visible"
        );
        assert!((exit.scale - 1.0).abs() < 1e-6, "and stays where it is");
        let enter = enter_channels(0.0, 0.0, &motion, easing.enter);
        assert!(
            (enter.opacity - 1.0).abs() < 1e-6,
            "the to-line is simply there"
        );
        assert!((enter.scale - 1.0).abs() < 1e-6);
        assert!((enter.blur).abs() < 1e-6);
    }
}
