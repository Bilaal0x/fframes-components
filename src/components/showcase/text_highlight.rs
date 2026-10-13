//! A chosen span receives one of several animated emphasis treatments.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, clamp01, interpolate, lerp, normalised_speed};
use crate::ui::color::{Rgba, parse_color, to_hex};
use crate::ui::{Mode, UiTheme, text, with_alpha};

/// The sentence's tracking, as the source sets it: `letterSpacing: "-0.02em"`.
pub const TRACKING_EM: f32 = -0.02;
/// The sentence's line height, as the source sets it: `lineHeight: 1.25`.
pub const LINE_HEIGHT: f32 = 1.25;
/// The wipe's curve: `Easing.bezier(0.2, 0.6, 0.35, 1)`.
pub const WIPE_EASE: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);
/// The rush's curve — the *travel*, not the size: `Easing.bezier(0.45, 0.2, 0.45, 1)`.
pub const RUSH_EASE: CubicBezier = CubicBezier::new(0.45, 0.2, 0.45, 1.0);
/// The spring the marker and underline sweeps run on, before [`SpringOverrides`]: `damping: 18`.
pub const SWEEP_DAMPING: f32 = 18.0;
/// The sweep spring's mass — Remotion's own default.
pub const SWEEP_MASS: f32 = 1.0;
/// The sweep spring's stiffness — Remotion's own default.
pub const SWEEP_STIFFNESS: f32 = 100.0;
/// Remotion's default `durationRestThreshold`, which is what decides the natural duration.
pub const REST_THRESHOLD: f32 = 0.005;
/// The gloss's own angle: `linear-gradient(110deg, …)`.
pub const SHINE_ANGLE_DEG: f32 = 110.0;
/// Where the gloss starts, peaks and ends along its gradient line: `transparent 30%, shine 50%,
/// transparent 70%`.
pub const SHINE_STOPS: [f32; 3] = [0.3, 0.5, 0.7];
/// The gloss's travel, as `background-position` percentages: `200% → -100%`.
pub const SHINE_POSITION: [f32; 2] = [200.0, -100.0];

/// The brand mark's own width, in its path's coordinate space — the source's `viewBox`.
const MARK_W: f32 = 464.0;
/// The brand mark's own height, in its path's coordinate space.
const MARK_H: f32 = 409.0;
/// The `scale(0.1, -0.1)` the source's mark group applies inside that viewBox.
const MARK_PATH_SCALE: f32 = 0.1;
/// The mark's lateral gap from the word at rest: `fontSize * 0.16`.
const MARK_GAP_EM: f32 = 0.16;
/// The lockup's left padding that reserves the mark's slot: `fontSize * 0.3`.
const MARK_SLOT_EM: f32 = 0.3;
/// How much of the mark's swing its ink does not follow — the source's `INK_LAG`.
const INK_LAG: f32 = 0.7;
/// The marker's vertical inset around the span: `inset: -0.02em …`.
const MARKER_INSET_Y_EM: f32 = 0.02;
/// The marker's horizontal overhang: `inset: … -0.12em`.
const MARKER_INSET_X_EM: f32 = 0.12;
/// How far below the span's box the underline sits: `bottom: -0.06em`.
const UNDERLINE_DROP_EM: f32 = 0.06;
/// How far the replacement rises as it cross-fades in: `translateY(8px → 0)`.
const STRIKE_LIFT_PX: f32 = 8.0;
/// The most frames [`spring_calculation`] will simulate before calling the spring settled.
///
/// The source's own loop runs to whatever frame it is handed; by twenty seconds the envelope is
/// `e^-180` and the state is exactly `1.0` in `f64`, so this only bounds the work.
const SPRING_FRAME_CAP: f32 = 600.0;
/// The brand mark the source draws when no `logo` prop is given, verbatim from `SnapMark`.
const MARK_PATH: &str = "M589 3805 c-167 -41 -305 -149 -380 -297 -60 -120 -59 -84 -59 -1239 0 -1004 1 -1059 19 -1112 48 -147 148 -226 331 -262 100 -20 127 -33 186 -87 53 -49 74 -109 74 -212 1 -121 42 -196 133 -244 l52 -27 980 -3 c883 -3 986 -1 1042 13 75 21 133 67 171 139 l27 51 3 315 c2 173 2 342 0 374 l-3 60 -31 -50 c-36 -59 -101 -115 -170 -146 l-49 -23 -810 -3 c-591 -2 -825 0 -865 8 -128 28 -258 121 -325 234 -65 110 -66 128 -63 826 l3 625 24 58 c44 111 147 210 270 259 l56 23 830 3 c809 2 831 2 895 -18 80 -25 149 -74 198 -142 l37 -52 3 334 c3 367 -3 418 -56 494 -30 44 -98 91 -152 106 -22 6 -457 10 -1175 9 -923 -1 -1151 -3 -1196 -14z M3800 2983 c-327 -196 -598 -361 -602 -367 -4 -6 -7 -254 -5 -550 l2 -540 145 -91 c80 -50 208 -130 285 -177 77 -47 283 -174 457 -282 174 -108 326 -196 337 -196 11 0 27 7 35 16 14 14 16 141 16 1254 0 781 -4 1248 -10 1264 -7 20 -16 26 -37 26 -18 -1 -232 -123 -623 -357z";

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
    /// A rectangle from its corner and size.
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The x of the rectangle's horizontal centre.
    #[must_use]
    pub fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
    }

    /// The y of the rectangle's vertical centre.
    #[must_use]
    pub fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }
}

/// Which emphasis the highlighted span gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextHighlightPreset {
    /// A mark wipes the word open, rests as a lockup, then rushes the camera and floods the frame.
    #[default]
    LogoWipe,
    /// A translucent bar sweeps left to right behind the words, and their colour shifts with it.
    Marker,
    /// The span's ink fades from the base colour to the emphasis colour. Nothing else moves.
    Color,
    /// A rule draws in from the left under the span, and the span's colour shifts with it.
    Underline,
    /// A rule strikes the span out, then the old word cross-fades into the replacement.
    Strikethrough,
    /// A gloss sweeps across the span's own letters.
    Shimmer,
}

impl TextHighlightPreset {
    /// Every preset, in the source's own order.
    pub const ALL: [Self; 6] = [
        Self::LogoWipe,
        Self::Marker,
        Self::Color,
        Self::Underline,
        Self::Strikethrough,
        Self::Shimmer,
    ];

    /// The preset's `config.ts` value, as the source spells it.
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::LogoWipe => "logo-wipe",
            Self::Marker => "marker",
            Self::Color => "color",
            Self::Underline => "underline",
            Self::Strikethrough => "strikethrough",
            Self::Shimmer => "shimmer",
        }
    }

    /// Whether this preset draws *inside* the sentence, leaving `before` and `after` alone.
    ///
    /// `logo-wipe` is the exception: the source returns early for it and paints the lockup on its
    /// own, so the two static halves never reach the screen.
    #[must_use]
    pub fn keeps_the_sentence(self) -> bool {
        self != Self::LogoWipe
    }
}

/// The subset of Remotion's `SpringConfig` a component can override here.
///
/// `overshootClamping` is not exposed: the source never sets it, so it is always `false`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringConfig {
    /// Damping coefficient. Higher settles sooner.
    pub damping: f32,
    /// Mass of the moving value.
    pub mass: f32,
    /// Stiffness. Higher moves faster.
    pub stiffness: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            damping: SWEEP_DAMPING,
            mass: SWEEP_MASS,
            stiffness: SWEEP_STIFFNESS,
        }
    }
}

/// The spring simulation's own state: the timestamp it last advanced to, and the position and
/// velocity it arrived with.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SpringState {
    last_timestamp: f64,
    current: f64,
    velocity: f64,
}

impl SpringConfig {
    /// One advance step of Remotion's simulation, up to `now` milliseconds.
    ///
    /// This is the source's `advance()`: a timestep clamped to 64 ms, the under-damped closed form
    /// when `zeta < 1` and the critically damped one otherwise. There is no over-damped branch —
    /// Remotion's `zeta < 1` test sends everything else through the critical form.
    fn advance(self, state: SpringState, now: f64) -> SpringState {
        let delta = (now - state.last_timestamp).min(64.0);
        let c = f64::from(self.damping);
        let m = f64::from(self.mass);
        let k = f64::from(self.stiffness);

        let v0 = -state.velocity;
        let x0 = 1.0 - state.current;
        let zeta = c / (2.0 * (k * m).sqrt());
        let omega0 = (k / m).sqrt();
        let t = delta / 1000.0;

        if zeta < 1.0 {
            let omega1 = omega0 * (1.0 - zeta * zeta).sqrt();
            let (sin1, cos1) = (omega1 * t).sin_cos();
            let envelope = (-zeta * omega0 * t).exp();
            let fragment = envelope * (sin1 * ((v0 + zeta * omega0 * x0) / omega1) + x0 * cos1);
            SpringState {
                last_timestamp: now,
                current: 1.0 - fragment,
                velocity: zeta * omega0 * fragment
                    - envelope * (cos1 * (v0 + zeta * omega0 * x0) - omega1 * x0 * sin1),
            }
        } else {
            let envelope = (-omega0 * t).exp();
            SpringState {
                last_timestamp: now,
                current: 1.0 - envelope * (x0 + (v0 + omega0 * x0) * t),
                velocity: envelope * (v0 * (t * omega0 - 1.0) + t * x0 * omega0 * omega0),
            }
        }
    }
}

/// Whether a value is a usable positive duration: finite, non-zero and not negative.
///
/// Written as a predicate so a caller can say "not usable" without a negated float comparison,
/// which is both hard to read and easy to get wrong on `NaN`.
#[must_use]
pub fn is_positive(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

/// Remotion's `springCalculation`: the spring's raw position at `frame`, advanced one frame at a
/// time from nought.
///
/// The simulation is deliberately *not* the analytic solution [`crate::motion::Spring`] evaluates:
/// Remotion integrates it frame by frame, and the two disagree in the third decimal. Matching the
/// source means matching its arithmetic.
#[must_use]
pub fn spring_calculation(frame: f32, fps: f32, config: SpringConfig) -> f32 {
    if !is_positive(fps) {
        return 1.0;
    }
    let frame = frame.clamp(0.0, SPRING_FRAME_CAP);
    let last = frame.floor();
    let uneven_rest = frame - last;
    let mut state = SpringState {
        last_timestamp: 0.0,
        current: 0.0,
        velocity: 0.0,
    };
    let steps = last as i64;
    for step in 0..=steps {
        let at = if step == steps {
            last + uneven_rest
        } else {
            step as f32
        };
        state = config.advance(state, f64::from(at / fps) * 1000.0);
    }
    state.current as f32
}

/// Remotion's `measureSpring`: how many frames the spring takes to settle, and to *stay* settled.
///
/// A spring is inside the threshold many times on its way in, so Remotion keeps animating until
/// twenty consecutive frames hold — which is what makes the answer usable as a duration.
#[must_use]
pub fn measure_spring(fps: f32, config: SpringConfig) -> f32 {
    let difference = |frame: f32| (spring_calculation(frame, fps, config) - 1.0).abs();
    let mut frame = 0.0_f32;
    while difference(frame) >= REST_THRESHOLD && frame < 10_000.0 {
        frame += 1.0;
    }
    let mut finished = frame;
    let mut held = 0;
    while held < 20 {
        frame += 1.0;
        if difference(frame) >= REST_THRESHOLD {
            held = 0;
            finished = frame + 1.0;
        } else {
            held += 1;
        }
    }
    finished
}

/// Remotion's `spring({frame, fps, config, durationInFrames})`.
///
/// `durationInFrames` rescales the clock by the spring's own natural duration, so the spring
/// reaches its rest exactly when the window closes, however stiff or slack it is. `None` runs it on
/// its natural clock.
///
/// Unlike [`crate::motion::Spring::value`], this takes the frame in *composition* frames — the
/// source passes `frame - startAt`, which can be negative, and Remotion clamps that to nought.
#[must_use]
pub fn spring_at(
    frame: f32,
    fps: f32,
    config: SpringConfig,
    duration_in_frames: Option<f32>,
) -> f32 {
    let Some(duration) = duration_in_frames else {
        return spring_calculation(frame, fps, config);
    };
    if !is_positive(duration) {
        return spring_calculation(frame, fps, config);
    }
    if frame > duration {
        return 1.0;
    }
    let natural = measure_spring(fps, config);
    if !is_positive(natural) {
        return 1.0;
    }
    spring_calculation(frame / (duration / natural), fps, config)
}

/// The spring overrides the source takes as `springConfig` — every field optional, because the
/// source spreads it over its own `{damping: 18}`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpringOverrides {
    /// Damping override; `None` keeps [`SWEEP_DAMPING`].
    pub damping: Option<f32>,
    /// Mass override; `None` keeps [`SWEEP_MASS`].
    pub mass: Option<f32>,
    /// Stiffness override; `None` keeps [`SWEEP_STIFFNESS`].
    pub stiffness: Option<f32>,
}

impl SpringOverrides {
    /// The spring these overrides resolve to, as `{damping: 18, ...springConfig}` does.
    #[must_use]
    pub fn resolve(self) -> SpringConfig {
        SpringConfig {
            damping: self.damping.unwrap_or(SWEEP_DAMPING),
            mass: self.mass.unwrap_or(SWEEP_MASS),
            stiffness: self.stiffness.unwrap_or(SWEEP_STIFFNESS),
        }
    }
}

/// The timing every emphasis preset is scheduled on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmphasisTiming {
    /// The spring the marker and underline sweeps run on.
    pub spring: SpringConfig,
    /// The frame the emphasis starts on.
    pub start_at: f32,
    /// Frames the draw or sweep takes.
    pub draw_duration: f32,
}

impl EmphasisTiming {
    /// The timing a configuration asks for.
    #[must_use]
    pub fn from_config(config: &TextHighlightConfig) -> Self {
        Self {
            spring: config.spring_config.resolve(),
            start_at: config.start_at,
            draw_duration: config.draw_duration,
        }
    }
}

/// The marker and underline sweep at `frame`: a high-damping spring over `draw_duration` frames,
/// starting at `start_at`.
#[must_use]
pub fn sweep_at(frame: f32, fps: f32, timing: EmphasisTiming) -> f32 {
    spring_at(
        frame - timing.start_at,
        fps,
        timing.spring,
        Some(timing.draw_duration),
    )
}

/// How far the sweep's colour shift has run: `clamp01((sweep - 0.5) / 0.35)`.
///
/// The ink waits until the bar is more than half drawn and has arrived by 85%, so the colour change
/// reads as *following* the mark rather than racing it.
#[must_use]
pub fn sweep_color_t(sweep: f32) -> f32 {
    clamp01((sweep - 0.5) / 0.35)
}

/// How far the `color` preset's fade has run: `clamp01((frame - start_at) / draw_duration)`, with
/// no easing at all.
#[must_use]
pub fn color_mix_at(frame: f32, timing: EmphasisTiming) -> f32 {
    if !is_positive(timing.draw_duration) {
        return 1.0;
    }
    clamp01((frame - timing.start_at) / timing.draw_duration)
}

/// The source's `defaultThickness`: `max(2, round(fontSize * 0.08))`, in reference-canvas px.
#[must_use]
pub fn default_thickness(font_size: f32) -> f32 {
    (font_size * 0.08).round().max(2.0)
}

/// The marker's translucent bar, in canvas px.
///
/// `inset: -0.02em -0.12em` extends the bar past the span's own line box on all four sides; it is
/// drawn behind the words and scaled about its own left edge by the sweep.
#[must_use]
pub fn marker_bar(highlight: Rect, size: f32, sweep: f32) -> Rect {
    Rect::new(
        highlight.x - MARKER_INSET_X_EM * size,
        highlight.y - MARKER_INSET_Y_EM * size,
        (highlight.w + 2.0 * MARKER_INSET_X_EM * size) * sweep.max(0.0),
        highlight.h + 2.0 * MARKER_INSET_Y_EM * size,
    )
}

/// The underline's rule, in canvas px: flush with the span, sitting `-0.06em` below its box.
#[must_use]
pub fn underline_rule(highlight: Rect, size: f32, thickness: f32) -> Rect {
    Rect::new(
        highlight.x,
        highlight.y + highlight.h + UNDERLINE_DROP_EM * size - thickness,
        highlight.w,
        thickness,
    )
}

/// The `strikethrough` preset's four beat frames.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrikePhases {
    /// The rule starts drawing.
    pub draw_start: f32,
    /// The rule is full width; the cross-fade begins.
    pub draw_end: f32,
    /// The old word starts fading out.
    pub fade_start: f32,
    /// The replacement is fully in.
    pub fade_end: f32,
}

/// The source's `strikethroughPhases`: the rule draws over `draw_duration` frames, then the old
/// text cross-fades into the replacement over half that long.
#[must_use]
pub fn strike_phases(start_at: f32, draw_duration: f32) -> StrikePhases {
    let draw_end = start_at + draw_duration;
    StrikePhases {
        draw_start: start_at,
        draw_end,
        fade_start: draw_end,
        fade_end: draw_end + draw_duration * 0.5,
    }
}

/// The rule's width at `frame`, as a percentage of the struck run.
#[must_use]
pub fn strike_percent(frame: f32, phases: StrikePhases) -> f32 {
    interpolate(frame, [phases.draw_start, phases.draw_end], [0.0, 100.0])
}

/// The cross-fade at `frame`: the old run's opacity, the replacement's opacity, and how far the
/// replacement has risen, in reference-canvas px.
#[must_use]
pub fn strike_crossfade(frame: f32, phases: StrikePhases) -> (f32, f32, f32) {
    let range = [phases.fade_start, phases.fade_end];
    (
        interpolate(frame, range, [1.0, 0.0]),
        interpolate(frame, range, [0.0, 1.0]),
        interpolate(frame, range, [STRIKE_LIFT_PX, 0.0]),
    )
}

/// The gloss's `background-position` at `frame`: `200% → -100%` across the draw window.
#[must_use]
pub fn shimmer_position(frame: f32, timing: EmphasisTiming) -> f32 {
    interpolate(
        frame,
        [timing.start_at, timing.start_at + timing.draw_duration],
        SHINE_POSITION,
    )
}

/// The id of the `shimmer`'s gloss gradient at `frame`.
///
/// It moves with the frame because `usvgr::Cache` keys resolved paint servers by element id and is
/// deliberately kept across frames to speed up cross-SVG work: a constant id hands every frame
/// whichever gradient was resolved first, and this one sweeps, so the gloss would freeze there.
#[must_use]
pub fn shimmer_id(frame: f32) -> String {
    format!("text-highlight-shine-{}", frame.round() as i32)
}

/// The gloss's gradient line in canvas px, as `[x1, y1, x2, y2]`.
///
/// The source paints the gloss as a `background-image` with `background-size: 200% 100%` and a
/// `background-position` that runs `200% → -100%`, so the image is twice the run's width and its
/// own origin moves by `(width - image_width) * position / 100`. The default
/// `background-repeat: repeat` then tiles it every `2 * width`; the gloss is fully transparent
/// outside its `30%..70%` stops, and its band is narrower than the run itself, so at most one tile
/// can ever be visible inside the run — this returns that one. (That has a consequence worth
/// knowing: the pattern is periodic in `2 * width`, so the `300%`-wide position sweep carries the
/// gloss past the run one and a half times, and the run is gloss-free in the gap between passes.)
/// The gradient is `110deg`, measured across the *image's* box, which is why its length uses
/// `2 * width`.
#[must_use]
pub fn shimmer_line(highlight: Rect, position_percent: f32) -> [f32; 4] {
    let (sin_a, cos_a) = SHINE_ANGLE_DEG.to_radians().sin_cos();
    // CSS angles run clockwise from "to top"; SVG's y grows downward, so "up" is -y.
    let (ux, uy) = (sin_a, -cos_a);

    let image_w = 2.0 * highlight.w;
    // The tile whose gloss is nearest to the run, in the run's own coordinates. Distance is to the
    // run as an *interval*, not to its centre: with the band narrower than the run that picks the
    // tile which actually covers it.
    let centre_rel = if image_w > 0.0 {
        let origin = (highlight.w - image_w) * position_percent / 100.0;
        let base = (origin + image_w / 2.0).rem_euclid(image_w);
        let gap = |value: f32| {
            if value < 0.0 {
                -value
            } else if value > highlight.w {
                value - highlight.w
            } else {
                0.0
            }
        };
        let mut best = base;
        let mut best_gap = gap(base);
        for candidate in [base - image_w, base + image_w] {
            let candidate_gap = gap(candidate);
            if candidate_gap < best_gap {
                best = candidate;
                best_gap = candidate_gap;
            }
        }
        best
    } else {
        0.0
    };

    let length = (image_w * ux).abs() + (highlight.h * uy).abs();
    let center_y = highlight.center_y();
    [
        highlight.x + centre_rel - ux * length / 2.0,
        center_y - uy * length / 2.0,
        highlight.x + centre_rel + ux * length / 2.0,
        center_y + uy * length / 2.0,
    ]
}

/// Apparent size of something travelling toward the camera: `1 / (1 - p * travel)`, where `p` is
/// the fraction of the way to the eye that `max_scale` represents.
///
/// It creeps for most of the trip and then blows up right at the end, which is the whole character
/// of a thing coming at your face and is not something an easing curve can fake.
#[must_use]
pub fn perspective_scale(travel: f32, max_scale: f32) -> f32 {
    if max_scale <= 1.0 {
        return 1.0;
    }
    let p = 1.0 - 1.0 / max_scale;
    1.0 / (1.0 - p * clamp01(travel).min(0.9999))
}

/// The mark's lateral position along the swing, in **world** units — before the projection
/// multiplies it by the apparent scale.
///
/// `drift * travel` is the crossing; `bow * sin(pi * travel)` is the arc. Subtract it and the mark
/// swings wide on the way out, comes round, and the crossing carries it away on the far side: one
/// curve, with no corner in it.
#[must_use]
pub fn swing_world_x(travel: f32, drift: f32, bow: f32) -> f32 {
    drift * travel - bow * (std::f32::consts::PI * travel).sin()
}

/// How much the projection magnifies the bow at its widest.
///
/// The swing is authored in world space, and what you *see* is that path multiplied by the apparent
/// scale — so the arc is at its widest on screen where `sin(pi * t) * perspectiveScale(t)` peaks,
/// out around `t = 0.8`, by which point the mark is already several times its resting size.
/// Dividing the bow by this is what lets `swing_out` be quoted as a fraction of the frame.
#[must_use]
pub fn swing_gain(max_scale: f32) -> f32 {
    let mut peak = 1.0_f32;
    for step in 1..128 {
        let t = step as f32 / 128.0;
        let value = (std::f32::consts::PI * t).sin() * perspective_scale(t, max_scale);
        if value > peak {
            peak = value;
        }
    }
    peak
}

/// How far the ink has to reach to close the frame, from wherever the mark can be.
///
/// Nothing here measures the word, so nothing knows where the mark actually *is* — only that the
/// lockup is centred and the mark sits at its left end, which puts its resting centre somewhere in
/// `[0, width / 2]`. This is the distance from the worst of those positions, swung either way, to
/// the furthest corner of the frame.
#[must_use]
pub fn ink_reach(width: f32, height: f32, drift_px: f32) -> f32 {
    let nearest = drift_px;
    let furthest = width / 2.0 + drift_px;
    let horizontal = (nearest.abs())
        .max(furthest.abs())
        .max((width - nearest).abs())
        .max((width - furthest).abs());
    horizontal.hypot(height / 2.0) * 1.05
}

/// The `logo-wipe` preset's resolved numbers, in canvas px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WipeGeometry {
    /// The mark's resting size.
    pub mark_size: f32,
    /// Turns the mark makes while it wipes.
    pub spin_turns: f32,
    /// The frame the wipe starts on.
    pub start_at: f32,
    /// Frames the wipe takes.
    pub draw_duration: f32,
    /// Frames the finished lockup rests.
    pub hold_duration: f32,
    /// Frames the rush takes.
    pub rush_duration: f32,
    /// How many times its resting size the mark reaches.
    pub rush_scale: f32,
    /// Where the swing ends up, as a fraction of the canvas width.
    pub rush_drift: f32,
    /// About how far the mark bows left, as a fraction of the canvas width.
    pub swing_out: f32,
}

/// Everything the `logo-wipe` preset needs at one frame, in canvas px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WipeMotion {
    /// `0 → 1` across the wipe, on [`WIPE_EASE`].
    pub wipe: f32,
    /// `0 → 1` across the rush, on [`RUSH_EASE`].
    pub travel: f32,
    /// The clip edge, as a percentage of the word's width: `(1 - wipe) * 100`.
    pub edge: f32,
    /// The mark's apparent size, as a multiple of its resting size.
    pub rushed: f32,
    /// The mark's resting size.
    pub mark_size: f32,
    /// The mark's size right now: `mark_size * rushed`.
    pub mark_box: f32,
    /// How far the mark has swung sideways, in canvas px — already projected.
    pub swing_px: f32,
    /// How much of that swing the ink does *not* follow.
    pub ink_px: f32,
    /// `0 → 1` as the mark grows past the frame: the ink's gate.
    pub flood: f32,
    /// The ink disc's radius; `0` until the gate opens.
    pub flood_radius: f32,
    /// The mark's rotation at this frame, in degrees.
    pub rotate_deg: f32,
    /// The mark's own scale about its centre — the wipe's growth times the rush's.
    pub scale: f32,
}

/// The `logo-wipe` preset at `frame`, in canvas px.
///
/// `width` and `height` are the real canvas, because the rush is authored as fractions of the frame
/// — the swing, the bow and the ink's reach are all measured against it.
#[must_use]
pub fn wipe_motion(frame: f32, width: f32, height: f32, geometry: &WipeGeometry) -> WipeMotion {
    let wipe = WIPE_EASE.ease(clamp01(
        (frame - geometry.start_at) / geometry.draw_duration,
    ));
    let rush_start = geometry.start_at + geometry.draw_duration + geometry.hold_duration;
    let travel = RUSH_EASE.ease(clamp01((frame - rush_start) / geometry.rush_duration));

    let mark_size = geometry.mark_size;
    let rushed = perspective_scale(travel, geometry.rush_scale);
    let mark_box = mark_size * rushed;
    let diagonal = width.hypot(height);

    let world_drift = geometry.rush_drift * width / geometry.rush_scale;
    let world_bow = geometry.swing_out * width / swing_gain(geometry.rush_scale);
    let swing_px = rushed * swing_world_x(travel, world_drift, world_bow);
    let ink_px = swing_px * (1.0 - INK_LAG);

    let flood = interpolate(mark_box, [diagonal * 0.55, diagonal * 1.45], [0.0, 1.0]);
    let flood_radius = if flood <= 0.0 {
        0.0
    } else {
        lerp(
            mark_box * 0.03,
            ink_reach(width, height, geometry.rush_drift * width * (1.0 - INK_LAG)),
            flood,
        )
    };

    WipeMotion {
        wipe,
        travel,
        edge: (1.0 - wipe) * 100.0,
        rushed,
        mark_size,
        mark_box,
        swing_px,
        ink_px,
        flood,
        flood_radius,
        rotate_deg: geometry.spin_turns * 360.0 * wipe + 30.0 * travel,
        scale: interpolate(wipe, [0.0, 1.0], [0.55, 1.0]) * rushed,
    }
}

/// The region `clipPath: inset(0 0 0 edge%)` leaves visible: everything right of the edge.
#[must_use]
pub fn clip_rect(word: Rect, edge: f32) -> Rect {
    let x = word.x + edge / 100.0 * word.w;
    Rect::new(x, word.y, (word.x + word.w - x).max(0.0), word.h)
}

/// Where the mark's own box sits, in canvas px.
///
/// `left` is a percentage of the word's box, so the mark rides the wipe edge exactly; then
/// `translate: calc(-100% - 0.16em + swing)` parks it just clear of the letters — which is where it
/// comes to rest as the logo — and adds the rush's sideways swing. `top: 50%` with `-50%` centres
/// it on the line box.
#[must_use]
pub fn mark_box_rect(word: Rect, mark_size: f32, em_px: f32, edge: f32, swing_px: f32) -> Rect {
    Rect::new(
        word.x + edge / 100.0 * word.w - mark_size - MARK_GAP_EM * em_px + swing_px,
        word.center_y() - mark_size / 2.0,
        mark_size,
        mark_size,
    )
}

/// The ink disc's centre: carried by the mark, but lagging behind it by `INK_LAG` of the swing.
#[must_use]
pub fn disc_center(mark: Rect, swing_px: f32, ink_px: f32) -> (f32, f32) {
    (mark.center_x() - (swing_px - ink_px), mark.center_y())
}

/// The brand mark's path transform inside a `mark_size` square.
///
/// The source draws its own `<svg width={size} height={size} viewBox="0 0 464 409">`, which
/// letterboxes the 464x409 artwork inside the square (`preserveAspectRatio` defaults to
/// `xMidYMid meet`) and then applies `translate(0,409) scale(0.1,-0.1)` to the path group. Both are
/// folded into one `<g transform>` here, because a nested `<svg>` is not something to lean on.
#[must_use]
pub fn mark_inner_transform(mark_size: f32) -> String {
    let scale = MARK_PATH_SCALE * mark_size / MARK_W;
    let offset = mark_size * (MARK_H / MARK_W + (1.0 - MARK_H / MARK_W) / 2.0);
    format!("translate(0 {offset}) scale({scale} {})", -scale)
}

/// The transform that places the mark and spins and grows it about its own centre.
///
/// The source nests two elements so the sideways swing is *not* multiplied by the scale; the swing
/// therefore lives in [`mark_box_rect`] and only the rotation and scale are here. The mark's
/// artwork is emitted in the box's own `[0, mark_size]²` local space, so the transform has to carry
/// the box to its canvas position *and* pivot about the box's own centre — hence
/// `translate(centre) … translate(-size / 2)`, not a translate of the centre back again.
#[must_use]
pub fn mark_outer_transform(mark: Rect, rotate_deg: f32, scale: f32) -> String {
    let half = mark.w / 2.0;
    format!(
        "translate({} {}) rotate({rotate_deg}) scale({scale}) translate({} {})",
        mark.center_x(),
        mark.center_y(),
        -half,
        -half
    )
}

/// One measured run of the sentence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    /// The run's advance width with its own tracking, in canvas px.
    pub width: f32,
    /// Whether there is anything to draw at all — an empty run takes no tracking gap.
    pub present: bool,
}

/// Where each part of the sentence starts, in canvas px, relative to the line's centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSpans {
    /// The whole sentence's advance width.
    pub total: f32,
    /// The lead-in's left edge.
    pub before: f32,
    /// The highlighted span's left edge.
    pub highlight: f32,
    /// The tail's left edge.
    pub after: f32,
}

/// Resolve the three parts of the sentence into one centred run.
///
/// CSS puts `letter-spacing` after every glyph including the last, so the gap *between* two runs is
/// one tracking step and each run's own width already carries its internal ones. An empty run has
/// no glyph to space away from, so it leaves no gap — which is what keeps `before: ""` from
/// shifting the sentence by a tracking step.
#[must_use]
pub fn line_spans(before: Run, highlight: Run, after: Run, tracking_px: f32) -> LineSpans {
    let gap = |left: Run, right: Run| {
        if left.present && right.present {
            tracking_px
        } else {
            0.0
        }
    };
    let total = before.width
        + gap(before, highlight)
        + highlight.width
        + gap(highlight, after)
        + after.width;

    let mut cursor = -total / 2.0;
    let before_x = cursor;
    cursor += before.width + gap(before, highlight);
    let highlight_x = cursor;
    cursor += highlight.width + gap(highlight, after);
    LineSpans {
        total,
        before: before_x,
        highlight: highlight_x,
        after: cursor,
    }
}

/// The lighter of two colours by Oklch lightness — a specular highlight follows the light, not the
/// mode, so the gloss takes whichever of the page's two extremes is actually lighter.
#[must_use]
pub fn lighter_of(a: &str, b: &str) -> String {
    if oklch_lightness(a) >= oklch_lightness(b) {
        a.to_owned()
    } else {
        b.to_owned()
    }
}

/// A colour's Oklch lightness, through the shared colour maths.
fn oklch_lightness(color: &str) -> f64 {
    crate::ui::rgb_to_oklch(parse_color(color)).l
}

/// Remotion's `interpolateColors` for two opaque colours: a **plain sRGB channel ramp**.
///
/// The source reaches for `mixOklch` where it wants perceptual mixing and calls `interpolateColors`
/// where it does not — the marker, underline and colour presets all do. The two are visibly
/// different, so this is a lerp of the sRGB channels rounded to 8 bits, which is what
/// `interpolateColors` computes, and deliberately not [`crate::ui::mix_oklch`].
#[must_use]
pub fn lerp_srgb(base: &str, emphasis: &str, t: f32) -> String {
    let t = f64::from(clamp01(t));
    let (a, b) = (parse_color(base), parse_color(emphasis));
    let mix = |x: f64, y: f64| x + (y - x) * t;
    to_hex(Rgba {
        r: mix(a.r, b.r),
        g: mix(a.g, b.g),
        b: mix(a.b, b.b),
        alpha: None,
    })
}

/// The source's own closed form for how long the scene runs:
/// `startAt + drawDuration + holdDuration + rushDuration`.
///
/// For the shipped defaults this is 56, which is `config.ts`'s `durationInFrames` and the
/// `settle.base` the registry's frame measurements record.
#[must_use]
pub fn settle_frame(
    start_at: f32,
    draw_duration: f32,
    hold_duration: f32,
    rush_duration: f32,
) -> f32 {
    start_at + draw_duration + hold_duration + rush_duration
}

/// Configuration for [`TextHighlight`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextHighlightConfig {
    /// The static lead-in. Not drawn by `logo-wipe`, which paints the lockup alone.
    pub before: String,
    /// The emphasised span.
    pub highlight: String,
    /// The static tail. Not drawn by `logo-wipe`.
    pub after: String,
    /// Which emphasis to draw.
    pub preset: TextHighlightPreset,
    /// The sentence's ink. `None` takes the theme's `foreground`.
    pub base_color: Option<String>,
    /// The accent — bar, rule, mark and strike. `None` takes the theme's `primary`.
    pub accent_color: Option<String>,
    /// The span's final ink. `None` takes [`TextHighlightConfig::accent_color`] for `color` and
    /// `strikethrough`, and [`TextHighlightConfig::base_color`] for every other preset.
    pub highlighted_text_color: Option<String>,
    /// `strikethrough` only: the word the struck run cross-fades into.
    pub replace_with: Option<String>,
    /// Frame the emphasis starts on.
    pub start_at: f32,
    /// Frames the draw or sweep takes.
    pub draw_duration: f32,
    /// Overrides for the sweep spring.
    pub spring_config: SpringOverrides,
    /// Rule height in reference-canvas px. `None` derives [`default_thickness`].
    pub thickness: Option<f32>,
    /// `shimmer` only: the gloss. `None` takes the lighter of the theme's `card` and `foreground`.
    pub shine_color: Option<String>,
    /// `logo-wipe` only: mark size as a multiple of `font_size`.
    pub logo_scale: f32,
    /// `logo-wipe` only: turns the mark makes while it wipes.
    pub spin_turns: f32,
    /// `logo-wipe` only: frames the finished lockup rests.
    pub hold_duration: f32,
    /// `logo-wipe` only: frames the rush takes.
    pub rush_duration: f32,
    /// `logo-wipe` only: how many times its resting size the mark reaches.
    pub rush_scale: f32,
    /// `logo-wipe` only: where the swing ends up, as a fraction of the canvas width.
    pub rush_drift: f32,
    /// `logo-wipe` only: about how far the mark bows left, as a fraction of the canvas width.
    pub swing_out: f32,
    /// `logo-wipe` only: the ink the rush floods in. `None` takes the accent.
    pub cover_color: Option<String>,
    /// Font size in reference-canvas px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
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
}

impl Default for TextHighlightConfig {
    fn default() -> Self {
        // `config.ts`'s demo controls, field for field — including the concrete `baseColor` and
        // `accentColor` the demo passes, which are not the theme's tokens.
        Self {
            before: "Powered by ".to_owned(),
            highlight: "Northwind".to_owned(),
            after: String::new(),
            preset: TextHighlightPreset::LogoWipe,
            base_color: Some("#101828".to_owned()),
            accent_color: Some("#266DF0".to_owned()),
            highlighted_text_color: Some("#101828".to_owned()),
            replace_with: Some("Acme".to_owned()),
            start_at: 6.0,
            draw_duration: 14.0,
            spring_config: SpringOverrides::default(),
            thickness: Some(4.0),
            shine_color: Some("#FAFAFA".to_owned()),
            logo_scale: 1.05,
            spin_turns: 1.0,
            hold_duration: 12.0,
            rush_duration: 24.0,
            rush_scale: 42.0,
            rush_drift: 0.55,
            swing_out: 0.25,
            cover_color: Some("#266DF0".to_owned()),
            font_size: 56.0,
            font_weight: 600,
            speed: 1.0,
            mode: Mode::Light,
            theme: None,
            font_family: None,
            duration: 56,
        }
    }
}

/// The **Text Highlight** component.
#[derive(Debug, Clone, Default)]
pub struct TextHighlight {
    /// Component options.
    pub config: TextHighlightConfig,
}

impl TextHighlight {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: TextHighlightConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-highlight",
            name: "Text Highlight",
            summary: "One span in a static sentence is emphasised — marker, colour, rule, strike, gloss, or a logo wipe",
            feature: "text-highlight",
        }
    }
}

/// The per-frame state every text node shares: face, size, tracking and the line's baseline.
struct Painter<'a> {
    family: &'a str,
    size: f32,
    weight: u16,
    tracking_px: f32,
    baseline: f32,
}

impl Painter<'_> {
    /// The advance width of `text` at this painter's face and tracking.
    fn width(&self, ctx: &RenderCtx<'_>, text: &str) -> f32 {
        text::tracked_width_at(
            ctx,
            self.family,
            self.weight,
            self.size,
            text,
            self.tracking_px,
        )
    }

    /// A run drawn from its left edge.
    fn node(
        &self,
        ctx: &RenderCtx<'_>,
        text: &str,
        left_x: f32,
        fill: String,
        opacity: f32,
    ) -> Svgr<'static> {
        let width = self.width(ctx, text);
        text::text_node(
            ctx,
            text,
            left_x + width / 2.0,
            self.baseline,
            self.family.to_owned(),
            self.size,
            self.weight,
            self.tracking_px,
            fill,
            opacity,
        )
    }
}

/// Wrap a node in a horizontal scale about a point, collapsing to the node itself at rest.
fn scaled_about_x(node: Svgr<'static>, x: f32, y: f32, scale: f32) -> Svgr<'static> {
    if (scale - 1.0).abs() < 1e-6 {
        return node;
    }
    let transform = format!(
        "translate({x} {y}) scale({scale} 1) translate({} {})",
        -x, -y
    );
    fframes::svgr!(<g transform={transform}>{node}</g>)
}

/// Wrap a node in a vertical translate, collapsing to the node itself at rest.
fn lifted(node: Svgr<'static>, dy: f32) -> Svgr<'static> {
    if dy.abs() < 1e-6 {
        return node;
    }
    let transform = format!("translate(0 {dy})");
    fframes::svgr!(<g transform={transform}>{node}</g>)
}

/// Push one run of the sentence, unless it is empty.
fn push_text(
    ctx: &RenderCtx<'_>,
    painter: &Painter<'_>,
    text: &str,
    left_x: f32,
    fill: &str,
    opacity: f32,
    body: &mut Vec<Svgr<'static>>,
) {
    if text.is_empty() {
        return;
    }
    body.push(painter.node(ctx, text, left_x, fill.to_owned(), opacity));
}

/// The sentence's resolved runs at one frame.
struct SentenceSpans {
    before: f32,
    highlight: f32,
    after: f32,
    highlight_width: f32,
}

/// Resolve the sentence with the highlighted run measured at its own width.
fn emphasis_spans(
    ctx: &RenderCtx<'_>,
    painter: &Painter<'_>,
    config: &TextHighlightConfig,
) -> SentenceSpans {
    spans_for(ctx, painter, config, painter.width(ctx, &config.highlight))
}

/// Resolve the sentence with `strikethrough`'s own slot width — the wider of the struck run and its
/// replacement, because the source's `inline-grid` cell is as wide as its widest item.
fn strikethrough_spans(
    ctx: &RenderCtx<'_>,
    painter: &Painter<'_>,
    config: &TextHighlightConfig,
    replace: &str,
) -> SentenceSpans {
    let slot = painter
        .width(ctx, &config.highlight)
        .max(painter.width(ctx, replace));
    spans_for(ctx, painter, config, slot)
}

/// Resolve the sentence around a highlighted slot of `slot_w`.
fn spans_for(
    ctx: &RenderCtx<'_>,
    painter: &Painter<'_>,
    config: &TextHighlightConfig,
    slot_w: f32,
) -> SentenceSpans {
    let spans = line_spans(
        Run {
            width: painter.width(ctx, &config.before),
            present: !config.before.is_empty(),
        },
        Run {
            width: slot_w,
            present: !config.highlight.is_empty(),
        },
        Run {
            width: painter.width(ctx, &config.after),
            present: !config.after.is_empty(),
        },
        painter.tracking_px,
    );
    SentenceSpans {
        before: spans.before,
        highlight: spans.highlight,
        after: spans.after,
        highlight_width: slot_w,
    }
}

impl Component for TextHighlight {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source's own `previewBackdrop`.
        Some("#fafafa")
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

        let size = config.font_size * unit;
        let tracking_px = TRACKING_EM * size;
        let line_height = LINE_HEIGHT * size;
        let baseline_offset =
            ctx.baseline_offset_at(&family, config.font_weight, size, line_height);
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let baseline = center_y - line_height / 2.0 + baseline_offset;
        let box_top = baseline - baseline_offset;

        let base = config
            .base_color
            .clone()
            .unwrap_or_else(|| theme.foreground.clone());
        let accent = config
            .accent_color
            .clone()
            .unwrap_or_else(|| theme.primary.clone());
        let shine = config
            .shine_color
            .clone()
            .unwrap_or_else(|| lighter_of(&theme.card, &theme.foreground));
        let emphasis =
            config
                .highlighted_text_color
                .clone()
                .unwrap_or_else(|| match config.preset {
                    TextHighlightPreset::Color | TextHighlightPreset::Strikethrough => {
                        accent.clone()
                    }
                    _ => base.clone(),
                });
        let thickness = config
            .thickness
            .unwrap_or_else(|| default_thickness(config.font_size))
            * unit;

        let frame = ctx.frame() * normalised_speed(config.speed);
        let fps = ctx.info.fps;
        let timing = EmphasisTiming::from_config(config);
        let painter = Painter {
            family: &family,
            size,
            weight: config.font_weight,
            tracking_px,
            baseline,
        };

        let mut defs: Vec<Svgr<'static>> = Vec::new();
        let mut body: Vec<Svgr<'static>> = Vec::new();

        match config.preset {
            TextHighlightPreset::LogoWipe => {
                let mark_size = config.font_size * config.logo_scale * unit;
                let padding = mark_size + MARK_SLOT_EM * size;
                let word_w = painter.width(ctx, &config.highlight);
                // The lockup — the reserved mark slot plus the word — is what the flex box centres,
                // so the word itself sits `padding / 2` right of the frame's centre.
                let word_left = center_x - (padding + word_w) / 2.0 + padding;
                let word = Rect::new(word_left, box_top, word_w, line_height);

                let geometry = WipeGeometry {
                    mark_size,
                    spin_turns: config.spin_turns,
                    start_at: config.start_at,
                    draw_duration: config.draw_duration,
                    hold_duration: config.hold_duration,
                    rush_duration: config.rush_duration,
                    rush_scale: config.rush_scale,
                    rush_drift: config.rush_drift,
                    swing_out: config.swing_out,
                };
                let motion = wipe_motion(frame, ctx.width(), ctx.height(), &geometry);
                let mark = mark_box_rect(word, mark_size, size, motion.edge, motion.swing_px);
                let clip = clip_rect(word, motion.edge);

                // The word is painted through its glyphs, left to right, from the accent into the
                // emphasis colour — `background-clip: text` with a 90deg gradient.
                defs.push(fframes::svgr!(
                    <linearGradient id="text-highlight-wipe" gradientUnits="userSpaceOnUse"
                                    x1={word.x} y1={word.center_y()} x2={word.x + word.w}
                                    y2={word.center_y()}>
                        <stop offset={0.0} stop-color={accent.clone()} />
                        <stop offset={1.0} stop-color={emphasis.clone()} />
                    </linearGradient>
                ));

                // Before the wipe starts the clip leaves a zero-width strip, and a zero-width
                // `<rect>` is not a clip at all — it is a parse warning. Nothing would be visible
                // through it anyway, so the word is simply not drawn yet.
                if clip.w > 0.0 && !config.highlight.is_empty() {
                    defs.push(fframes::svgr!(
                        <clipPath id="text-highlight-wipe-clip">
                            <rect x={clip.x} y={clip.y} width={clip.w} height={clip.h} />
                        </clipPath>
                    ));

                    let word_node = painter.node(
                        ctx,
                        &config.highlight,
                        word.x,
                        "url(#text-highlight-wipe)".to_owned(),
                        1.0,
                    );
                    body.push(fframes::svgr!(
                        <g clip-path="url(#text-highlight-wipe-clip)">{word_node}</g>
                    ));
                }

                // The ink. A disc born inside the mark's own centre knot, carried along by it, that
                // outruns it and closes the frame. Both of its ends are keyed to the mark's size,
                // never to the clock.
                if motion.flood_radius > 0.0 {
                    let (cx, cy) = disc_center(mark, motion.swing_px, motion.ink_px);
                    let cover = config.cover_color.clone().unwrap_or_else(|| accent.clone());
                    body.push(fframes::svgr!(
                        <circle cx={cx} cy={cy} r={motion.flood_radius} fill={cover} />
                    ));
                }

                let outer = mark_outer_transform(mark, motion.rotate_deg, motion.scale);
                let inner = mark_inner_transform(mark_size);
                body.push(fframes::svgr!(
                    <g transform={outer}>
                        <g transform={inner}>
                            <path d={MARK_PATH} fill={accent.clone()} />
                        </g>
                    </g>
                ));
            }
            TextHighlightPreset::Marker => {
                let sweep = sweep_at(frame, fps, timing);
                let ink = lerp_srgb(&base, &emphasis, sweep_color_t(sweep));
                let spans = emphasis_spans(ctx, &painter, config);
                let highlight = Rect::new(
                    center_x + spans.highlight,
                    box_top,
                    spans.highlight_width,
                    line_height,
                );

                push_text(
                    ctx,
                    &painter,
                    &config.before,
                    center_x + spans.before,
                    &base,
                    1.0,
                    &mut body,
                );
                if sweep > 0.0 && highlight.w > 0.0 {
                    let bar = marker_bar(highlight, size, sweep);
                    let rect = fframes::svgr!(
                        <rect x={bar.x} y={bar.y} width={bar.w} height={bar.h}
                              rx={MARKER_INSET_X_EM * size}
                              fill={with_alpha(&accent, 0.16)} opacity={0.16} />
                    );
                    body.push(scaled_about_x(rect, bar.x, bar.center_y(), sweep));
                }
                push_text(
                    ctx,
                    &painter,
                    &config.highlight,
                    highlight.x,
                    &ink,
                    1.0,
                    &mut body,
                );
                push_text(
                    ctx,
                    &painter,
                    &config.after,
                    center_x + spans.after,
                    &base,
                    1.0,
                    &mut body,
                );
            }
            TextHighlightPreset::Color => {
                let ink = lerp_srgb(&base, &emphasis, color_mix_at(frame, timing));
                let spans = emphasis_spans(ctx, &painter, config);
                push_text(
                    ctx,
                    &painter,
                    &config.before,
                    center_x + spans.before,
                    &base,
                    1.0,
                    &mut body,
                );
                push_text(
                    ctx,
                    &painter,
                    &config.highlight,
                    center_x + spans.highlight,
                    &ink,
                    1.0,
                    &mut body,
                );
                push_text(
                    ctx,
                    &painter,
                    &config.after,
                    center_x + spans.after,
                    &base,
                    1.0,
                    &mut body,
                );
            }
            TextHighlightPreset::Underline => {
                let sweep = sweep_at(frame, fps, timing);
                let ink = lerp_srgb(&base, &emphasis, sweep_color_t(sweep));
                let spans = emphasis_spans(ctx, &painter, config);
                let highlight = Rect::new(
                    center_x + spans.highlight,
                    box_top,
                    spans.highlight_width,
                    line_height,
                );
                push_text(
                    ctx,
                    &painter,
                    &config.before,
                    center_x + spans.before,
                    &base,
                    1.0,
                    &mut body,
                );
                push_text(
                    ctx,
                    &painter,
                    &config.highlight,
                    highlight.x,
                    &ink,
                    1.0,
                    &mut body,
                );
                push_text(
                    ctx,
                    &painter,
                    &config.after,
                    center_x + spans.after,
                    &base,
                    1.0,
                    &mut body,
                );
                if sweep > 0.0 && highlight.w > 0.0 {
                    let rule = underline_rule(highlight, size, thickness);
                    let rect = fframes::svgr!(
                        <rect x={rule.x} y={rule.y} width={rule.w} height={rule.h}
                              rx={thickness / 2.0} fill={accent.clone()} />
                    );
                    body.push(scaled_about_x(rect, rule.x, rule.center_y(), sweep));
                }
            }
            TextHighlightPreset::Strikethrough => {
                let phases = strike_phases(config.start_at, config.draw_duration);
                let (old_opacity, new_opacity, lift) = strike_crossfade(frame, phases);
                let percent = strike_percent(frame, phases);
                let replace = config
                    .replace_with
                    .clone()
                    .unwrap_or_else(|| config.highlight.clone());
                let spans = strikethrough_spans(ctx, &painter, config, &replace);
                let slot = Rect::new(
                    center_x + spans.highlight,
                    box_top,
                    spans.highlight_width,
                    line_height,
                );
                // `justify-items: center` in the source's `inline-grid` centres each of the two
                // superimposed runs inside the shared cell.
                let old_w = painter.width(ctx, &config.highlight);
                let new_w = painter.width(ctx, &replace);
                let old_x = slot.x + (slot.w - old_w).max(0.0) / 2.0;
                let new_x = slot.x + (slot.w - new_w).max(0.0) / 2.0;

                push_text(
                    ctx,
                    &painter,
                    &config.before,
                    center_x + spans.before,
                    &base,
                    1.0,
                    &mut body,
                );
                if !config.highlight.is_empty() {
                    body.push(painter.node(
                        ctx,
                        &config.highlight,
                        old_x,
                        base.clone(),
                        old_opacity,
                    ));
                }
                if percent > 0.0 && old_w > 0.0 {
                    let bar = Rect::new(
                        old_x,
                        slot.center_y() - thickness / 2.0,
                        old_w * percent / 100.0,
                        thickness,
                    );
                    body.push(fframes::svgr!(
                        <rect x={bar.x} y={bar.y} width={bar.w} height={bar.h}
                              rx={thickness / 2.0} fill={accent.clone()} opacity={old_opacity} />
                    ));
                }
                if !replace.is_empty() {
                    body.push(lifted(
                        painter.node(ctx, &replace, new_x, emphasis.clone(), new_opacity),
                        lift * unit,
                    ));
                }
                push_text(
                    ctx,
                    &painter,
                    &config.after,
                    center_x + spans.after,
                    &base,
                    1.0,
                    &mut body,
                );
            }
            TextHighlightPreset::Shimmer => {
                let position = shimmer_position(frame, timing);
                let spans = emphasis_spans(ctx, &painter, config);
                let highlight = Rect::new(
                    center_x + spans.highlight,
                    box_top,
                    spans.highlight_width,
                    line_height,
                );
                let line = shimmer_line(highlight, position);
                let mut shine_fill = String::new();
                if !config.highlight.is_empty() {
                    // The id has to move with the frame. `usvgr::Cache` keys resolved paint
                    // servers by element id and is deliberately kept across frames to speed up
                    // cross-SVG work, so a constant id hands every frame whichever gradient was
                    // resolved first — and this one sweeps, so the gloss would freeze at that
                    // frame's position.
                    let id = shimmer_id(frame);
                    defs.push(fframes::svgr!(
                        <linearGradient id={id.clone()} gradientUnits="userSpaceOnUse"
                                        x1={line[0]} y1={line[1]} x2={line[2]} y2={line[3]}>
                            <stop offset={SHINE_STOPS[0]} stop-color={shine.clone()} stop-opacity="0" />
                            <stop offset={SHINE_STOPS[1]} stop-color={shine.clone()} stop-opacity="1" />
                            <stop offset={SHINE_STOPS[2]} stop-color={shine.clone()} stop-opacity="0" />
                        </linearGradient>
                    ));
                    shine_fill = format!("url(#{id})");
                }

                push_text(
                    ctx,
                    &painter,
                    &config.before,
                    center_x + spans.before,
                    &base,
                    1.0,
                    &mut body,
                );
                if !config.highlight.is_empty() {
                    body.push(painter.node(
                        ctx,
                        &config.highlight,
                        highlight.x,
                        emphasis.clone(),
                        1.0,
                    ));
                    body.push(painter.node(ctx, &config.highlight, highlight.x, shine_fill, 1.0));
                }
                push_text(
                    ctx,
                    &painter,
                    &config.after,
                    center_x + spans.after,
                    &base,
                    1.0,
                    &mut body,
                );
            }
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

    fn timing() -> EmphasisTiming {
        EmphasisTiming::from_config(&TextHighlightConfig::default())
    }

    fn wipe_geometry() -> WipeGeometry {
        WipeGeometry {
            mark_size: 58.8,
            spin_turns: 1.0,
            start_at: 6.0,
            draw_duration: 14.0,
            hold_duration: 12.0,
            rush_duration: 24.0,
            rush_scale: 42.0,
            rush_drift: 0.55,
            swing_out: 0.25,
        }
    }

    #[test]
    fn the_defaults_are_the_source_config_field_by_field() {
        let config = TextHighlightConfig::default();
        assert_eq!(config.before, "Powered by ");
        assert_eq!(config.highlight, "Northwind");
        assert_eq!(config.after, "");
        assert_eq!(config.preset, TextHighlightPreset::LogoWipe);
        assert_eq!(config.base_color.as_deref(), Some("#101828"));
        assert_eq!(config.accent_color.as_deref(), Some("#266DF0"));
        assert_eq!(config.highlighted_text_color.as_deref(), Some("#101828"));
        assert_eq!(config.replace_with.as_deref(), Some("Acme"));
        assert!((config.start_at - 6.0).abs() < 1e-6);
        assert!((config.draw_duration - 14.0).abs() < 1e-6);
        assert_eq!(config.spring_config, SpringOverrides::default());
        assert_eq!(config.thickness, Some(4.0));
        assert_eq!(config.shine_color.as_deref(), Some("#FAFAFA"));
        assert!((config.logo_scale - 1.05).abs() < 1e-6);
        assert!((config.spin_turns - 1.0).abs() < 1e-6);
        assert!((config.hold_duration - 12.0).abs() < 1e-6);
        assert!((config.rush_duration - 24.0).abs() < 1e-6);
        assert!((config.rush_scale - 42.0).abs() < 1e-6);
        assert!((config.rush_drift - 0.55).abs() < 1e-6);
        assert!((config.swing_out - 0.25).abs() < 1e-6);
        assert_eq!(config.cover_color.as_deref(), Some("#266DF0"));
        assert!((config.font_size - 56.0).abs() < 1e-6);
        assert_eq!(config.font_weight, 600);
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.theme, None);
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 56);
    }

    #[test]
    fn the_id_feature_and_backdrop_are_the_sources() {
        let component = TextHighlight::default();
        assert_eq!(component.meta().id, "text-highlight");
        assert_eq!(component.meta().feature, "text-highlight");
        assert_eq!(TextHighlight::meta_static().name, "Text Highlight");
        assert_eq!(component.background(), Some("#fafafa"));
    }

    #[test]
    fn the_spring_overrides_resolve_the_way_the_source_spreads_them() {
        // `{damping: 18, ...springConfig}` — the spread can only replace what it names.
        assert_eq!(
            SpringOverrides::default().resolve(),
            SpringConfig::default()
        );
        let overridden = SpringOverrides {
            damping: None,
            mass: Some(2.0),
            stiffness: Some(80.0),
        };
        let spring = overridden.resolve();
        assert!((spring.damping - 18.0).abs() < 1e-6);
        assert!((spring.mass - 2.0).abs() < 1e-6);
        assert!((spring.stiffness - 80.0).abs() < 1e-6);
    }

    #[test]
    fn the_duration_closed_form_reproduces_the_measured_settle() {
        // `registry/__measured__.json` records durationInFrames 56 and settle.base 56 for this
        // component, and `config.ts`'s own comment spells the same sum out: "6 start + 14 wipe +
        // 12 hold + 24 rush".
        let config = TextHighlightConfig::default();
        let formula = settle_frame(
            config.start_at,
            config.draw_duration,
            config.hold_duration,
            config.rush_duration,
        );
        assert!((formula - 56.0).abs() < 1e-6, "{formula}");
        assert_eq!(formula as usize, config.duration);
        assert_eq!(TextHighlight::default().duration(), 56);
        // The measurement's linear fit is flat — `baseWords: 1`, `perWord: 0` — because nothing in
        // the schedule depends on the copy. `baseWords` is a fit parameter, not a word count.
        assert!((settle_frame(0.0, 14.0, 12.0, 24.0) - 50.0).abs() < 1e-6);
    }

    #[test]
    fn the_sweep_spring_lands_on_the_frame_the_window_closes() {
        let fps = 30.0;
        let spring = SpringConfig::default();
        let natural = measure_spring(fps, spring);
        // The source's own spring settles inside the threshold on frame 17 and never leaves it
        // again, so `measureSpring` answers 17 — and `durationInFrames: 14` therefore runs it
        // slightly fast, reaching its rest 0.3% short of one.
        assert!((natural - 17.0).abs() < 1.0, "natural duration {natural}");
        // Remotion's `durationInFrames` rescales the clock so the spring rests when the window
        // closes: nought at the start frame, one at `startAt + drawDuration`.
        assert!(spring_at(-6.0, fps, spring, Some(14.0)).abs() < 1e-6);
        assert!(spring_at(0.0, fps, spring, Some(14.0)).abs() < 1e-6);
        assert!((spring_at(14.0, fps, spring, Some(14.0)) - 1.0).abs() < 0.01);
        // Beyond the window Remotion short-circuits to the target rather than letting the spring
        // ring past it.
        assert!((spring_at(15.0, fps, spring, Some(14.0)) - 1.0).abs() < 1e-6);
        // Halfway through the window it is already moving fast, and still short of one.
        let halfway = spring_at(7.0, fps, spring, Some(14.0));
        assert!(halfway > 0.5 && halfway < 1.0, "{halfway}");
    }

    #[test]
    fn the_sweep_is_monotone_and_zero_before_it_starts() {
        let timing = timing();
        assert!(sweep_at(0.0, 30.0, timing).abs() < 1e-6);
        assert!(sweep_at(6.0, 30.0, timing).abs() < 1e-6);
        let mut previous = 0.0_f32;
        for step in 0..=40 {
            let sweep = sweep_at(step as f32, 30.0, timing);
            assert!(
                sweep >= previous - 1e-4,
                "frame {step}: {sweep} < {previous}"
            );
            previous = sweep;
        }
        assert!((sweep_at(20.0, 30.0, timing) - 1.0).abs() < 0.01);
        assert!((sweep_at(40.0, 30.0, timing) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_marker_bar_sits_outside_the_span_and_sweeps_from_its_left_edge() {
        let span = Rect::new(100.0, 200.0, 300.0, 70.0);
        let full = marker_bar(span, 56.0, 1.0);
        assert!((full.x - (100.0 - 0.12 * 56.0)).abs() < 1e-4);
        assert!((full.y - (200.0 - 0.02 * 56.0)).abs() < 1e-4);
        assert!((full.w - (300.0 + 2.0 * 0.12 * 56.0)).abs() < 1e-4);
        assert!((full.h - (70.0 + 2.0 * 0.02 * 56.0)).abs() < 1e-4);

        // `transform-origin: left center; transform: scaleX(sweep)` — the bar grows from its own
        // left edge and its vertical extent never changes.
        let half = marker_bar(span, 56.0, 0.5);
        assert!((half.x - full.x).abs() < 1e-6);
        assert!((half.center_y() - full.center_y()).abs() < 1e-6);
        assert!((half.w - full.w / 2.0).abs() < 1e-4);
        assert!((half.h - full.h).abs() < 1e-6);
        assert!(marker_bar(span, 56.0, 0.0).w.abs() < 1e-6);
    }

    #[test]
    fn the_colour_shift_follows_the_sweep_but_lags_it() {
        // `interpolate(sweep, [0.5, 0.85], [0, 1])`: nothing until the bar is half drawn, done by
        // 85%.
        assert!(sweep_color_t(0.0).abs() < 1e-6);
        assert!(sweep_color_t(0.5).abs() < 1e-6);
        assert!((sweep_color_t(0.675) - 0.5).abs() < 1e-6);
        assert!((sweep_color_t(0.85) - 1.0).abs() < 1e-6);
        assert!((sweep_color_t(2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_colour_preset_fades_over_the_draw_window_with_no_easing() {
        let timing = timing();
        assert!(color_mix_at(0.0, timing).abs() < 1e-6);
        assert!(color_mix_at(6.0, timing).abs() < 1e-6);
        assert!((color_mix_at(13.0, timing) - 0.5).abs() < 1e-6);
        assert!((color_mix_at(20.0, timing) - 1.0).abs() < 1e-6);
        assert!((color_mix_at(200.0, timing) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_underline_rule_sits_below_the_span_and_draws_from_the_left() {
        let span = Rect::new(100.0, 200.0, 300.0, 70.0);
        let rule = underline_rule(span, 56.0, 4.0);
        assert!((rule.x - 100.0).abs() < 1e-6);
        assert!((rule.w - 300.0).abs() < 1e-6);
        assert!((rule.h - 4.0).abs() < 1e-6);
        // `bottom: -0.06em` puts the rule's own bottom edge below the span's box.
        assert!((rule.y + rule.h - (270.0 + 0.06 * 56.0)).abs() < 1e-4);
        // The sweep is a `scaleX` about the rule's left edge, so the drawn width is the rule's
        // width times the sweep — and the rule never moves vertically.
        let sweep = 0.5_f32;
        assert!((rule.w * sweep - 150.0).abs() < 1e-6);
        assert!((rule.y - underline_rule(span, 56.0, 4.0).y).abs() < 1e-6);
    }

    #[test]
    fn the_default_thickness_is_the_sources_rounding() {
        assert!((default_thickness(56.0) - 4.0).abs() < 1e-6);
        assert!((default_thickness(100.0) - 8.0).abs() < 1e-6);
        assert!((default_thickness(12.0) - 2.0).abs() < 1e-6);
        assert!((default_thickness(1.0) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn the_strikethrough_draws_over_the_draw_window_then_cross_fades() {
        let phases = strike_phases(15.0, 30.0);
        assert!((phases.draw_start - 15.0).abs() < 1e-6);
        assert!((phases.draw_end - 45.0).abs() < 1e-6);
        assert!((phases.fade_start - 45.0).abs() < 1e-6);
        assert!((phases.fade_end - 60.0).abs() < 1e-6);

        // The rule is a percentage of the struck run, and it is full at `drawEnd`.
        assert!(strike_percent(10.0, phases).abs() < 1e-6);
        assert!((strike_percent(30.0, phases) - 50.0).abs() < 1e-6);
        assert!((strike_percent(45.0, phases) - 100.0).abs() < 1e-6);
        assert!((strike_percent(90.0, phases) - 100.0).abs() < 1e-6);

        // No fade while the rule is still drawing.
        let drawing = strike_crossfade(30.0, phases);
        assert!((drawing.0 - 1.0).abs() < 1e-6);
        assert!(drawing.1.abs() < 1e-6);
        assert!((drawing.2 - STRIKE_LIFT_PX).abs() < 1e-6);

        // Halfway through the cross-fade both words are half there, and the replacement is halfway
        // up out of its 8 px drop.
        let mid = strike_crossfade(52.5, phases);
        assert!((mid.0 - 0.5).abs() < 1e-6);
        assert!((mid.1 - 0.5).abs() < 1e-6);
        assert!((mid.2 - 4.0).abs() < 1e-6);

        let done = strike_crossfade(60.0, phases);
        assert!(done.0.abs() < 1e-6);
        assert!((done.1 - 1.0).abs() < 1e-6);
        assert!(done.2.abs() < 1e-6);
    }

    #[test]
    fn the_strikethrough_keeps_the_ratio_the_sources_own_test_pins() {
        let phases = strike_phases(0.0, 40.0);
        let total = phases.fade_end / 0.6;
        assert!(((phases.draw_end - phases.draw_start) / total - 0.4).abs() < 1e-5);
        assert!(((phases.fade_end - phases.draw_end) / total - 0.2).abs() < 1e-5);
    }

    #[test]
    fn the_shimmer_gloss_runs_left_to_right_across_the_span() {
        // A 300 px span with a 70 px line box, as the geometry functions see it.
        let b = Rect::new(100.0, 200.0, 300.0, 70.0);
        let at = |p: f32| {
            let line = shimmer_line(b, p);
            (line[0] + line[2]) / 2.0
        };
        // `background-position: 100%` puts the twice-as-wide image's own centre on the span's left
        // edge, and `0%` puts it on the right edge — so the gloss crosses the span left to right
        // over the middle of the sweep.
        assert!((at(100.0) - b.x).abs() < 1e-3, "{}", at(100.0));
        assert!((at(0.0) - (b.x + b.w)).abs() < 1e-3, "{}", at(0.0));
        assert!(at(100.0) < at(50.0) && at(50.0) < at(0.0));
        // The sweep starts half a pass before that (`200%`, the tail of the previous pass leaving
        // on the right) and ends half a pass after it (`-100%`, the band gone off to the right).
        assert!(at(200.0) >= b.x && at(-100.0) <= b.x + b.w);
        // The band's own length: `|2w·sin(110°)| + |h·cos(110°)|`.
        let line = shimmer_line(b, 100.0);
        let length = (line[2] - line[0]).hypot(line[3] - line[1]);
        let expected = (600.0_f32 * SHINE_ANGLE_DEG.to_radians().sin()).abs()
            + (70.0_f32 * SHINE_ANGLE_DEG.to_radians().cos()).abs();
        assert!((length - expected).abs() < 1e-3, "{length} vs {expected}");
        // A degenerate run does not divide by zero.
        assert_eq!(
            shimmer_line(Rect::new(10.0, 20.0, 0.0, 0.0), 100.0)[0],
            10.0
        );
    }

    #[test]
    fn the_shimmer_position_runs_from_two_hundred_to_minus_a_hundred() {
        let timing = timing();
        assert!((shimmer_position(0.0, timing) - 200.0).abs() < 1e-6);
        assert!((shimmer_position(6.0, timing) - 200.0).abs() < 1e-6);
        assert!((shimmer_position(13.0, timing) - 50.0).abs() < 1e-6);
        assert!((shimmer_position(20.0, timing) + 100.0).abs() < 1e-6);
        assert!((shimmer_position(99.0, timing) + 100.0).abs() < 1e-6);
    }

    #[test]
    fn the_wipe_edge_reveals_the_word_from_the_right() {
        let word = Rect::new(500.0, 200.0, 300.0, 70.0);
        // Before the wipe the clip leaves nothing; once the wipe lands it leaves everything.
        let closed = clip_rect(word, 100.0);
        assert!(closed.w.abs() < 1e-4);
        assert!((closed.x - 800.0).abs() < 1e-4);

        let open = clip_rect(word, 0.0);
        assert!((open.x - 500.0).abs() < 1e-6);
        assert!((open.w - 300.0).abs() < 1e-6);

        let half = clip_rect(word, 40.0);
        assert!((half.x - 620.0).abs() < 1e-4);
        assert!((half.w - 180.0).abs() < 1e-4);
        assert!((half.y - word.y).abs() < 1e-6);
        assert!((half.h - word.h).abs() < 1e-6);
    }

    #[test]
    fn the_mark_rides_the_wipe_edge_and_rests_clear_of_the_word() {
        let word = Rect::new(500.0, 200.0, 300.0, 70.0);
        let size = 56.0;
        // At the end of the wipe the mark's box is one mark plus the 0.16em gap to the left of the
        // word, and centred on the line box.
        let rest = mark_box_rect(word, 58.8, size, 0.0, 0.0);
        assert!((rest.x - (500.0 - 58.8 - 0.16 * 56.0)).abs() < 1e-4);
        assert!((rest.center_y() - word.center_y()).abs() < 1e-6);
        assert!((rest.w - 58.8).abs() < 1e-6);

        // Before it, it waits at the word's right edge, and the swing carries it further.
        let start = mark_box_rect(word, 58.8, size, 100.0, 0.0);
        assert!((start.x - (800.0 - 58.8 - 0.16 * 56.0)).abs() < 1e-4);
        let swung = mark_box_rect(word, 58.8, size, 0.0, 25.0);
        assert!((swung.x - (rest.x + 25.0)).abs() < 1e-6);
        assert!((mark_box_rect(word, 58.8, size, 0.0, 0.0).center_y() - 235.0).abs() < 1e-6);
    }

    #[test]
    fn the_ink_lags_the_mark_by_the_sources_own_share() {
        let mark = Rect::new(100.0, 200.0, 58.8, 58.8);
        let swing = 40.0;
        let ink = swing * (1.0 - INK_LAG);
        let (cx, cy) = disc_center(mark, swing, ink);
        assert!((cx - (mark.center_x() - swing * INK_LAG)).abs() < 1e-4);
        assert!((cy - mark.center_y()).abs() < 1e-6);
    }

    #[test]
    fn the_mark_path_transform_folds_the_viewbox_fit_and_the_source_group() {
        // For a square mark box the artwork is `mark_size` wide and letterboxed vertically: the
        // 464x409 viewBox is fitted by `mark_size / 464` and centred, and the source's own
        // `translate(0,409) scale(0.1,-0.1)` is folded in.
        let transform = mark_inner_transform(58.8);
        let scale: f32 = 0.1 * 58.8 / 464.0;
        assert!(
            transform.contains(&format!("scale({scale} {})", -scale)),
            "{transform}"
        );
        let offset: f32 = 58.8 * (409.0 / 464.0 + (1.0 - 409.0 / 464.0) / 2.0);
        assert!(
            transform.contains(&format!("translate(0 {offset})")),
            "{transform}"
        );
        // The fit is a uniform scale, so the ink's aspect is the artwork's: measured on the
        // reference at 58.8 px the mark's ink is 56 px wide and 46.2 px tall.
        assert!((409.0_f32 / 464.0 + (1.0 - 409.0 / 464.0) / 2.0 - 0.940_73).abs() < 1e-4);
        assert!((409.0_f32 * 58.8 / 464.0 - 51.83).abs() < 0.01);
    }

    #[test]
    fn the_mark_spins_and_grows_about_its_own_centre() {
        let mark = Rect::new(100.0, 200.0, 58.8, 58.8);
        let transform = mark_outer_transform(mark, 90.0, 2.0);
        // The pivot is the box's own centre in its *local* space — so the mark is carried to its
        // canvas position and the swing's frame pixels are not multiplied by the scale, which is
        // exactly why the source nests the two elements.
        assert_eq!(
            transform,
            "translate(129.4 229.4) rotate(90) scale(2) translate(-29.4 -29.4)"
        );
        // A point at the artwork's local centre is the fixed point of the whole mapping.
        assert_eq!(mark.center_x(), 129.4);
        assert_eq!(mark.center_y(), 229.4);
    }

    #[test]
    fn the_perspective_creeps_and_then_blows_up() {
        assert!((perspective_scale(0.0, 42.0) - 1.0).abs() < 1e-6);
        assert!((perspective_scale(-1.0, 42.0) - 1.0).abs() < 1e-6);
        assert!((perspective_scale(0.5, 1.0) - 1.0).abs() < 1e-6);
        // Halfway in it has spent barely a fiftieth of its growth…
        assert!(perspective_scale(0.5, 42.0) < 2.0);
        // …and nearly all the rest arrives at the end.
        assert!(
            (perspective_scale(0.9999, 42.0) - 41.83).abs() < 0.01,
            "{}",
            perspective_scale(0.9999, 42.0)
        );
        assert!(perspective_scale(0.9, 42.0) < 10.0);
        let mut previous = 0.0_f32;
        for step in 0..=200 {
            let value = perspective_scale(step as f32 / 200.0, 42.0);
            assert!(value >= previous);
            previous = value;
        }
    }

    #[test]
    fn the_swing_bows_left_once_and_ends_on_the_far_side() {
        let (drift, bow, scale) = (16.0_f32, 12.0_f32, 42.0_f32);
        let projected = |t: f32| swing_world_x(t, drift, bow) * perspective_scale(t, scale);
        assert!(projected(0.0).abs() < 1e-6);
        assert!(projected(0.5) < 0.0, "the arc bows out to the left");
        assert!(projected(1.0) > 0.0, "and it crosses to the right");
        // One extreme only: two would mean it wandered back and forth.
        let mut turns = 0;
        let mut previous = projected(1.0 / 400.0) - projected(0.0);
        for step in 2..=400 {
            let delta = projected(step as f32 / 400.0) - projected((step - 1) as f32 / 400.0);
            if delta != 0.0 && previous != 0.0 && delta.signum() != previous.signum() {
                turns += 1;
            }
            if delta != 0.0 {
                previous = delta;
            }
        }
        assert_eq!(turns, 1);
        // With no bow it is a straight crossing.
        assert!((swing_world_x(0.5, drift, 0.0) - drift * 0.5).abs() < 1e-4);
    }

    #[test]
    fn the_swing_gain_divides_back_out() {
        let gain = swing_gain(42.0);
        assert!(
            (gain - 1.0).abs() > 1e-6,
            "the projection must magnify the bow"
        );
        let mut peak = 0.0_f32;
        for step in 1..500 {
            let t = step as f32 / 500.0;
            peak = peak.max((std::f32::consts::PI * t).sin() * perspective_scale(t, 42.0));
        }
        assert!(
            (gain - peak).abs() < 0.05,
            "gain {gain} should be the projected bow's peak {peak}"
        );
        // It never divides by less than one, however small the rush.
        assert!((swing_gain(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_ink_reaches_every_corner_from_anywhere_the_mark_can_rest() {
        let (width, height) = (1280.0_f32, 720.0_f32);
        for drift_fraction in [-0.8_f32, -0.45, 0.0, 0.45, 0.8] {
            let drift = drift_fraction * width;
            let reach = ink_reach(width, height, drift);
            for rest in [0.0_f32, width / 4.0, width / 2.0] {
                let (cx, cy) = (rest + drift, height / 2.0);
                for (x, y) in [(0.0, 0.0), (width, 0.0), (0.0, height), (width, height)] {
                    assert!(
                        (cx - x).hypot(cy - y) <= reach,
                        "corner ({x}, {y}) is outside {reach}"
                    );
                }
            }
        }
        // And further than the flat 0.85 x diagonal it replaced, once the mark swings out.
        let diagonal = width.hypot(height);
        assert!(ink_reach(width, height, 0.45 * width) > 0.85 * diagonal);
    }

    #[test]
    fn the_wipe_runs_before_the_rush_and_the_flood_follows_the_mark_size() {
        let geometry = wipe_geometry();
        let (width, height) = (1280.0_f32, 720.0_f32);

        // Before `startAt` nothing has moved, and the mark waits at the word's right edge.
        let before = wipe_motion(0.0, width, height, &geometry);
        assert!(before.wipe.abs() < 1e-6);
        assert!((before.edge - 100.0).abs() < 1e-6);
        assert!((before.scale - 0.55).abs() < 1e-6);
        assert!((before.rushed - 1.0).abs() < 1e-6);
        assert!(before.flood_radius.abs() < 1e-6);

        // The wipe lands on `startAt + drawDuration`, and the lockup holds until the rush.
        let wiped = wipe_motion(20.0, width, height, &geometry);
        assert!((wiped.wipe - 1.0).abs() < 1e-6);
        assert!(wiped.edge.abs() < 1e-6);
        assert!((wiped.scale - 1.0).abs() < 1e-6);
        assert!(
            (wiped.rotate_deg - 360.0).abs() < 1e-6,
            "{}",
            wiped.rotate_deg
        );
        let held = wipe_motion(31.0, width, height, &geometry);
        assert!((held.travel).abs() < 1e-6);
        assert!((held.scale - 1.0).abs() < 1e-6);

        // The rush is a perspective blow-up: 42x by the end of the window.
        let rushed = wipe_motion(56.0, width, height, &geometry);
        assert!((rushed.travel - 1.0).abs() < 1e-6);
        // `Math.min(Math.max(travel, 0), 0.9999)` is the source's own clamp, so the rush tops out
        // just under its nominal 42x rather than exactly on it. Remotion's own test for this reads
        // `toBeCloseTo(42, 0)`, which is the same tolerance.
        assert!((41.0..42.0).contains(&rushed.rushed), "{}", rushed.rushed);
        assert!(rushed.mark_box > 2000.0);
        // The ink only opens once the mark has outrun the frame: early in the rush the gate is
        // shut, and by the end the disc reaches the furthest corner.
        assert!(wipe_motion(50.0, width, height, &geometry).flood_radius < 1.0);
        let reach = ink_reach(width, height, 0.55 * width * (1.0 - INK_LAG));
        assert!((rushed.flood_radius - reach).abs() < 1e-3);
        // And the disc is keyed to the mark's size, not to the clock.
        assert!(
            wipe_motion(53.0, width, height, &geometry).flood_radius
                > wipe_motion(51.0, width, height, &geometry).flood_radius
        );
    }

    #[test]
    fn the_mark_swings_left_through_the_rush_and_ends_on_the_right() {
        let geometry = wipe_geometry();
        let (width, height) = (1280.0_f32, 720.0_f32);
        let mut left_most = 0.0_f32;
        for step in 0..=96 {
            let frame = 32.0 + step as f32 * 24.0 / 96.0;
            left_most = left_most.min(wipe_motion(frame, width, height, &geometry).swing_px);
        }
        assert!(left_most < -100.0, "the bow reached only {left_most}");
        let end = wipe_motion(56.0, width, height, &geometry).swing_px;
        assert!(
            (end - 0.55 * width).abs() < 0.05 * width,
            "the swing should end near 0.55 x width, got {end}"
        );
    }

    #[test]
    fn the_sentence_is_centred_and_the_highlight_does_not_move_the_rest_of_it() {
        let tracking = -1.0;
        let spans = line_spans(
            Run {
                width: 100.0,
                present: true,
            },
            Run {
                width: 200.0,
                present: true,
            },
            Run {
                width: 50.0,
                present: true,
            },
            tracking,
        );
        // 100 + 200 + 50 + two boundary gaps.
        assert!((spans.total - 348.0).abs() < 1e-4);
        assert!((spans.before + spans.total / 2.0).abs() < 1e-4);
        assert!((spans.highlight - (spans.before + 100.0 + tracking)).abs() < 1e-4);
        assert!((spans.after - (spans.highlight + 200.0 + tracking)).abs() < 1e-4);
        // The run spans the whole interval about zero: the lead-in's left edge and the tail's right
        // edge are equidistant from the centre.
        let (left, right) = (spans.before, spans.after + 50.0);
        assert!((left + right).abs() < 1e-4, "{left} vs {right}");

        // An empty lead-in takes no gap, which is what keeps `before: ""` from shifting anything.
        let bare = line_spans(
            Run {
                width: 0.0,
                present: false,
            },
            Run {
                width: 200.0,
                present: true,
            },
            Run {
                width: 0.0,
                present: false,
            },
            tracking,
        );
        assert!((bare.total - 200.0).abs() < 1e-6);
        assert!((bare.highlight + 100.0).abs() < 1e-6);

        // And the highlighted span's own left edge is exactly one lead-in plus one gap along.
        let long = line_spans(
            Run {
                width: 400.0,
                present: true,
            },
            Run {
                width: 200.0,
                present: true,
            },
            Run {
                width: 0.0,
                present: false,
            },
            tracking,
        );
        assert!((long.highlight - (long.before + 400.0 + tracking)).abs() < 1e-4);
        assert!((long.total - 599.0).abs() < 1e-4);
    }

    #[test]
    fn the_colour_ramp_is_srgb_not_oklch() {
        // Remotion's `interpolateColors` lerps the sRGB channels and rounds them.
        assert_eq!(lerp_srgb("#000000", "#ffffff", 0.0), "#000000");
        assert_eq!(lerp_srgb("#000000", "#ffffff", 1.0), "#ffffff");
        assert_eq!(lerp_srgb("#000000", "#ffffff", 0.5), "#808080");
        assert_eq!(lerp_srgb("#101828", "#266DF0", 0.0), "#101828");
        assert_eq!(lerp_srgb("#101828", "#266DF0", 1.0), "#266df0");
        // Halfway is the mean of the channels, which is *not* what an Oklch mix gives.
        let half = lerp_srgb("#000000", "#ff0000", 0.5);
        assert_eq!(half, "#800000");
        assert_ne!(
            half,
            to_hex(crate::ui::mix_oklch("#000000", "#ff0000", 0.5))
        );
        // Out-of-range values clamp, as `interpolate`'s own options do.
        assert_eq!(lerp_srgb("#000000", "#ffffff", -3.0), "#000000");
        assert_eq!(lerp_srgb("#000000", "#ffffff", 3.0), "#ffffff");
    }

    #[test]
    fn the_gloss_takes_the_lighter_of_the_two_extremes() {
        // White on the light palette, near-white on the dark one: a specular highlight follows the
        // light, not the mode.
        assert_eq!(lighter_of("#ffffff", "#141414"), "#ffffff");
        assert_eq!(lighter_of("#141414", "#ffffff"), "#ffffff");
        assert_eq!(lighter_of("#141417", "#fafafa"), "#fafafa");
    }

    #[test]
    fn every_preset_is_selectable_and_only_logo_wipe_drops_the_sentence() {
        assert_eq!(
            TextHighlightPreset::ALL.map(TextHighlightPreset::id),
            [
                "logo-wipe",
                "marker",
                "color",
                "underline",
                "strikethrough",
                "shimmer"
            ]
        );
        assert!(!TextHighlightPreset::LogoWipe.keeps_the_sentence());
        for preset in TextHighlightPreset::ALL {
            if preset != TextHighlightPreset::LogoWipe {
                assert!(preset.keeps_the_sentence(), "{preset:?}");
            }
            let config = TextHighlightConfig {
                preset,
                ..TextHighlightConfig::default()
            };
            assert_eq!(config.preset.id(), preset.id());
        }
    }

    #[test]
    fn the_shimmer_gradient_id_moves_with_the_frame() {
        // A constant id would be served from `usvgr::Cache`'s cross-frame paint map, so the gloss
        // would freeze at whichever frame resolved it first. See `shimmer_id`.
        assert_ne!(shimmer_id(6.0), shimmer_id(7.0));
        assert_eq!(shimmer_id(6.0), shimmer_id(6.4));
        assert_eq!(shimmer_id(6.0), "text-highlight-shine-6");
    }

    #[test]
    fn the_mark_path_is_the_sources_own() {
        assert!(MARK_PATH.starts_with("M589 3805"));
        assert!(MARK_PATH.ends_with('z'));
        assert!(MARK_PATH.len() > 800, "{}", MARK_PATH.len());
        assert!((MARK_W - 464.0).abs() < 1e-6);
        assert!((MARK_H - 409.0).abs() < 1e-6);
    }
}
