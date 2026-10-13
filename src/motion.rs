//! Shared motion tokens, ported from the source library's `motion.ts`.
//!
//! A single easing / duration / spring vocabulary every component draws from, so the same
//! "word arrives" gesture lands with the same weight in every component.
//!
//! Source: <https://github.com/remocn> (see the crate README for credit).

/// Default reference frame rate of the source design tables.
pub const FPS: f32 = 30.0;

/// `min(canvas_width, canvas_height) / 100` of the canvas the remocn components were authored
/// on: 1280x720, so `7.2`.
///
/// A component multiplies its own design px by `ctx.vmin() / REFERENCE_VMIN` so it keeps the
/// source's proportions on any canvas. On a 720-tall canvas the factor is `1.0` — the source's
/// own px values — and on 1080p, the default here, it is `1.5`.
pub const REFERENCE_VMIN: f32 = 7.2;

/// Convert milliseconds to whole frames (at least 1).
pub fn ms(milliseconds: f32) -> usize {
    ((milliseconds / 1000.0) * FPS).round().max(1.0) as usize
}

/// Press / tick feedback (100–160ms).
pub const DUR_SNAP: usize = 4;
/// Tooltips and small popovers (125–200ms).
pub const DUR_QUICK: usize = 5;
/// Per-character reveals and small label settles.
pub const DUR_CHAR: usize = 6;
/// Dropdowns, selects, word-level reveals (150–250ms).
pub const DUR_POPOVER: usize = 6;
/// Entrances that carry spatial meaning — line-level reveals.
pub const DUR_ENTER: usize = 8;
/// Modals, drawers, image cuts (200–500ms).
pub const DUR_OVERLAY: usize = 10;

/// A cubic Bézier easing curve, solved the way CSS solves `cubic-bezier(x1, y1, x2, y2)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl CubicBezier {
    /// Build a curve from its two control points.
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Sample the curve at `t` (clamped to `0..=1`).
    ///
    /// The x component is inverted with a bisection search, which is plenty accurate for a
    /// 30 fps render and keeps the whole library dependency-free.
    pub fn ease(self, t: f32) -> f32 {
        let t = clamp01(t);
        if t <= 0.0 {
            return 0.0;
        }
        if t >= 1.0 {
            return 1.0;
        }
        // Solve x(s) = t for s.
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        let mut s = t;
        for _ in 0..24 {
            let x = bezier_axis(self.x1, self.x2, s);
            if (x - t).abs() < 1e-5 {
                break;
            }
            if x < t {
                lo = s;
            } else {
                hi = s;
            }
            s = 0.5 * (lo + hi);
        }
        bezier_axis(self.y1, self.y2, s)
    }
}

fn bezier_axis(c1: f32, c2: f32, s: f32) -> f32 {
    let inv = 1.0 - s;
    3.0 * inv * inv * s * c1 + 3.0 * inv * s * s * c2 + s * s * s
}

/// Strong ease-out for UI: `cubic-bezier(0.23, 1, 0.32, 1)`.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.23, 1.0, 0.32, 1.0);
/// Strong ease-in-out for on-screen movement: `cubic-bezier(0.77, 0, 0.175, 1)`.
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.77, 0.0, 0.175, 1.0);
/// Heavily eased-in settle: `cubic-bezier(0.5, 0, 0.05, 1)` — the library's "slow clock".
pub const EASE_ZOOM_IN: CubicBezier = CubicBezier::new(0.5, 0.0, 0.05, 1.0);
/// Deliberate departure: `cubic-bezier(0.64, 0, 0.78, 0)`. Exits only.
pub const EASE_DEPART: CubicBezier = CubicBezier::new(0.64, 0.0, 0.78, 0.0);
/// Small mechanical settle: `cubic-bezier(0.2, 0.8, 0.2, 1)` — chips, captions.
pub const EASE_SNAPPY: CubicBezier = CubicBezier::new(0.2, 0.8, 0.2, 1.0);
/// Blur-bridge settle: `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_BLUR_SETTLE: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// Blur-bridge dissolve: `cubic-bezier(0.6, 0.04, 0.98, 0.34)`.
pub const EASE_BLUR_DISSOLVE: CubicBezier = CubicBezier::new(0.6, 0.04, 0.98, 0.34);

// Easing curves used by the remocn typography ports. Each is written inline in its source
// component as `Easing.bezier(...)`; they live here so the same gesture reads identically
// wherever it appears.
//
// Source: <https://github.com/Remocn/remocn> (MIT).

/// Character-rise opacity: `cubic-bezier(0.2, 0.8, 0.2, 1)`.
pub const EASE_CHAR: CubicBezier = CubicBezier::new(0.2, 0.8, 0.2, 1.0);
/// Character-rise travel: `cubic-bezier(0.2, 0.8, 0.6, 0.85)`.
pub const EASE_CHAR_TRAVEL: CubicBezier = CubicBezier::new(0.2, 0.8, 0.6, 0.85);
/// Letter landing from one direction: `cubic-bezier(0.18, 1, 0.32, 1)`.
pub const EASE_LETTER_SOFT: CubicBezier = CubicBezier::new(0.18, 1.0, 0.32, 1.0);
/// Material-style decelerating settle: `cubic-bezier(0.22, 1, 0.36, 1)`.
pub const EASE_MATERIAL_OUT: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);
/// Soft material exit: `cubic-bezier(0.32, 0.72, 0, 1)`.
pub const EASE_SOFT_OUT: CubicBezier = CubicBezier::new(0.32, 0.72, 0.0, 1.0);
/// Back-out overshoot — passes its target before settling: `cubic-bezier(0.34, 1.56, 0.64, 1)`.
pub const EASE_BACK_OUT: CubicBezier = CubicBezier::new(0.34, 1.56, 0.64, 1.0);
/// Long, soft deceleration for a phrase reveal: `cubic-bezier(0.5, 0, 0.7, 0.7)`.
///
/// This is the remocn primitives' `phraseEasing`: a camera travelling along a line of text that
/// should still be moving, gently, when the words run out.
pub const EASE_PHRASE: CubicBezier = CubicBezier::new(0.5, 0.0, 0.7, 0.7);

/// `easeInOutCubic`, as used by the scroller and the 3D carousel.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = clamp01(t);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// `Easing.out(Easing.cubic)`.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = clamp01(t);
    1.0 - (1.0 - t).powi(3)
}

/// `easeInQuad`, used by the text scroller's exit.
pub fn ease_in_quad(t: f32) -> f32 {
    let t = clamp01(t);
    t * t
}

/// `Easing.out(Easing.quart)` — a very strong deceleration, `1 - (1 - t)⁴`.
///
/// Used by the letter rises and the dot's drop in the remocn primitives.
pub fn ease_out_quart_value(t: f32) -> f32 {
    let t = clamp01(t);
    1.0 - (1.0 - t).powi(4)
}

/// A Remotion-style spring (`damping` / `mass` / `stiffness`) sampled at a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    /// Damping coefficient. Higher settles sooner; below 2*sqrt(stiffness*mass) overshoots.
    pub damping: f32,
    /// Mass of the moving value.
    pub mass: f32,
    /// Spring stiffness. Higher moves faster.
    pub stiffness: f32,
}

impl Spring {
    /// Analytic damped-oscillator solution, matching Remotion's `spring()`.
    ///
    /// Returns `0` at `frame = 0` and approaches `1` as the spring settles.
    pub fn value(self, frame: f32) -> f32 {
        let Spring {
            damping,
            mass,
            stiffness,
        } = self;
        if mass <= 0.0 || stiffness <= 0.0 {
            return if frame <= 0.0 { 0.0 } else { 1.0 };
        }
        // Remotion works in seconds, clamped at 0.
        let t = (frame / FPS).max(0.0);
        let omega0 = (stiffness / mass).sqrt();
        // Damping ratio.
        let zeta = damping / (2.0 * (stiffness * mass).sqrt());

        if zeta < 1.0 {
            // Underdamped.
            let omega_d = omega0 * (1.0 - zeta * zeta).sqrt();
            1.0 - (-zeta * omega0 * t).exp()
                * ((omega_d * t).cos() + (zeta * omega0 / omega_d) * (omega_d * t).sin())
        } else if (zeta - 1.0).abs() < 1e-6 {
            // Critically damped.
            1.0 - (1.0 + omega0 * t) * (-omega0 * t).exp()
        } else {
            // Overdamped.
            let s = omega0 * (zeta * zeta - 1.0).sqrt();
            let r1 = -zeta * omega0 + s;
            let r2 = -zeta * omega0 - s;
            1.0 - (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1)
        }
    }
}

/// A bubble / chip popping into a list. Mildly underdamped (≈4% past rest).
pub const SPRING_POP: Spring = Spring {
    damping: 15.0,
    mass: 0.72,
    stiffness: 170.0,
};

/// A heavier surface arriving (sheets, panels). Critically damped — no bounce.
pub const SPRING_SETTLE: Spring = Spring {
    damping: 18.0,
    mass: 1.0,
    stiffness: 150.0,
};

/// Collapse every movement channel while leaving opacity intact.
///
/// Multiply *movement* values only (travel, scale, rotation, blur, stagger) — never opacity,
/// so a reduced-motion variant still cross-fades.
pub fn motion_scale(reduced: bool) -> f32 {
    if reduced { 0.0 } else { 1.0 }
}

// Timeline and stagger helpers shared by every component whose entry is a staircase of characters
// or words. They live here rather than in any one component: a component module is compiled in only
// when its own feature is on, so a helper two components share cannot live in either of them.
//
// Source: <https://github.com/Remocn/remocn> (MIT).

/// A usable timeline scale: non-finite or non-positive speeds fall back to `1`.
///
/// The source passes `speed` straight into `frame * speed`; a zero or negative value there would
/// freeze or reverse the animation silently, so this library normalises it instead.
pub fn normalised_speed(speed: f32) -> f32 {
    if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    }
}

/// Frames until the last character (or word) of a staggered entry lands, before the tail.
///
/// `duration` is one item's own clock and `stagger` the gap between consecutive starts, so the
/// whole entrance spans `duration + (count - 1) * stagger` source frames, divided by the timeline
/// [`normalised_speed`].
pub fn entry_frames(count: usize, duration: usize, stagger: usize, speed: f32) -> usize {
    if count == 0 {
        return 0;
    }
    let span = duration + (count - 1) * stagger;
    ((span as f32) / normalised_speed(speed)).ceil().max(0.0) as usize
}

/// Opacity and vertical offset of one letter on a staggered entry clock.
///
/// Takes the timing values rather than a component's config so
/// [`bottom_up_letters`](crate::components::bottom_up_letters) and
/// [`top_down_letters`](crate::components::top_down_letters) — the same component with the travel
/// sign flipped — can share it. `direction` is `1` to rise from below, `-1` to drop from above.
#[allow(clippy::too_many_arguments)]
pub fn letter_channels(
    frame: f32,
    index: usize,
    stagger: usize,
    char_duration_frames: usize,
    char_travel_frames: usize,
    distance: f32,
    reduced_motion: bool,
    speed: f32,
    unit: f32,
    direction: f32,
) -> (f32, f32) {
    let local = frame * normalised_speed(speed) - index as f32 * stagger as f32;
    let travel = distance * unit * motion_scale(reduced_motion) * direction;
    let opacity = EASE_LETTER_SOFT.ease(clamp01(local / char_duration_frames.max(1) as f32));
    // The source hands the same curve to both `interpolate` calls, so the travel is eased over
    // its own (shorter) window rather than running linearly.
    let travel_p = EASE_LETTER_SOFT.ease(clamp01(local / char_travel_frames.max(1) as f32));
    let y = interpolate(travel_p, [0.0, 1.0], [travel, 0.0]);
    (opacity, y)
}

/// Normalised speed `|curve'(t)|`, divided by the peak of `|curve'|` measured once.
///
/// Drive blur from SPEED, never from progress: a curved path is sharp at the turnaround
/// (derivative 0) and smeared only at peak velocity.
pub fn speed_of(curve: CubicBezier, t: f32) -> f32 {
    const EPS: f32 = 1e-3;
    let raw = |t: f32| (curve.ease(t + EPS) - curve.ease(t - EPS)).abs() / (2.0 * EPS);
    let mut peak: f32 = 0.0;
    for i in 0..=1000 {
        peak = peak.max(raw(i as f32 / 1000.0));
    }
    if peak > 0.0 { raw(t) / peak } else { 0.0 }
}

/// Speed of a plain `fn(f32) -> f32` curve, normalised to its peak.
pub fn speed_of_fn(curve: impl Fn(f32) -> f32, t: f32) -> f32 {
    const EPS: f32 = 1e-3;
    let raw = |t: f32| (curve(clamp01(t + EPS)) - curve(clamp01(t - EPS))).abs() / (2.0 * EPS);
    let mut peak: f32 = 0.0;
    for i in 0..=1000 {
        peak = peak.max(raw(i as f32 / 1000.0));
    }
    if peak > 0.0 {
        raw(clamp01(t)) / peak
    } else {
        0.0
    }
}

/// Clamp into `0.0..=1.0`.
pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// The classic `smoothstep`: a clamped cubic Hermite ramp, `3t² - 2t³`.
///
/// The remocn primitives use it as their own easing where a plain ease would read too
/// mechanically; it has zero derivative at both ends.
pub fn smoothstep(t: f32) -> f32 {
    let x = clamp01(t);
    x * x * (3.0 - 2.0 * x)
}

/// Linear interpolation.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Remotion's `interpolate(v, [in_min, in_max], [out_min, out_max])`, clamped at both ends.
pub fn interpolate(v: f32, in_range: [f32; 2], out_range: [f32; 2]) -> f32 {
    let [in_min, in_max] = in_range;
    let [out_min, out_max] = out_range;
    if (in_max - in_min).abs() < f32::EPSILON {
        return out_min;
    }
    let t = clamp01((v - in_min) / (in_max - in_min));
    lerp(out_min, out_max, t)
}

/// A range map that does **not** clamp its input.
///
/// Remotion's `interpolate` clamps, which is what you want for a value that must stay inside its
/// range — an opacity, say — and wrong for one that is *meant* to overshoot. The back-out curves
/// in this library (`EASE_BACK_OUT`) pass their target before settling, so feeding their output
/// through [`interpolate`] would flatten the overshoot away and the gesture would read as a plain
/// ease-out. Use this where the curve's own excursion is the effect.
pub fn map_range(v: f32, in_range: [f32; 2], out_range: [f32; 2]) -> f32 {
    let [in_min, in_max] = in_range;
    let [out_min, out_max] = out_range;
    if (in_max - in_min).abs() < f32::EPSILON {
        return out_min;
    }
    lerp(out_min, out_max, (v - in_min) / (in_max - in_min))
}

/// Remotion's `interpolate` over an arbitrary number of stops, clamped at both ends.
pub fn interpolate_multi(v: f32, inputs: &[f32], outputs: &[f32]) -> f32 {
    if outputs.is_empty() {
        return 0.0;
    }
    if inputs.len() != outputs.len() || outputs.len() == 1 {
        return outputs[0];
    }
    if v <= inputs[0] {
        return outputs[0];
    }
    let last = inputs.len() - 1;
    if v >= inputs[last] {
        return outputs[last];
    }
    for i in 0..last {
        if v <= inputs[i + 1] {
            let span = inputs[i + 1] - inputs[i];
            let t = if span.abs() < f32::EPSILON {
                0.0
            } else {
                (v - inputs[i]) / span
            };
            return lerp(outputs[i], outputs[i + 1], t);
        }
    }
    outputs[last]
}

/// Sample a keyframe list with even input spacing, linearly interpolated and clamped.
pub fn sample_keyframes(values: &[f32], t: f32) -> f32 {
    let clamped = clamp01(t);
    match values.len() {
        0 => 0.0,
        1 => values[0],
        n => {
            let scaled = clamped * (n - 1) as f32;
            let i = ((n - 2) as f32).min(scaled.floor()) as usize;
            let fraction = scaled - i as f32;
            lerp(values[i], values[i + 1], fraction)
        }
    }
}

/// Deterministic pseudo-random number in `0.0..1.0`, ported from Remotion's `random(seed)`.
///
/// Remotion uses a 32-bit xorshift over the seed's UTF-8 bytes, so this reproduces the same
/// values for the same seed string. Frames are kept deterministic, which matters for renders.
pub fn random(seed: &str) -> f32 {
    let mut state: u32 = 0;
    for byte in seed.as_bytes() {
        state = state
            .wrapping_shl(5)
            .wrapping_sub(state)
            .wrapping_add(u32::from(*byte));
    }
    // xorshift32
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    (state as f64 / (u32::MAX as f64 + 1.0)) as f32
}

/// Deterministic pseudo-random sign/choice helper: `random(seed) < 0.5`.
pub fn random_bool(seed: &str, threshold: f32) -> bool {
    random(seed) < threshold
}

/// A seedable `mulberry32` generator, the PRNG Remotion's component library uses for particles.
///
/// It is bit-for-bit the same algorithm as the source (a wrapping 32-bit counter through an
/// avalanche mix), so a ported particle system reproduces the source's scatter for the same seed.
/// Frames are rendered on several threads, so a component must construct its own generator; one
/// `Mulberry32` is not shared.
#[derive(Debug, Clone)]
pub struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// Seed the generator.
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// The next value in `0.0..1.0`.
    pub fn next_f32(&mut self) -> f32 {
        self.state = self.state.wrapping_add(0x6d2b_79f5);
        // The source's avalanche mix, in wrapping u32 arithmetic:
        //   t = imul(t ^ (t >>> 15), t | 1)
        //   t ^= t + imul(t ^ (t >>> 7), t | 61)
        //   return ((t ^ (t >>> 14)) >>> 0) / 4294967296
        let mut t = self.state;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64 / 4_294_967_296.0) as f32
    }

    /// The next value in `min..max`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (max - min) * self.next_f32()
    }

    /// The next index in `0..len` (`0` for an empty slice).
    pub fn index(&mut self, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            ((self.next_f32() * len as f32) as usize).min(len - 1)
        }
    }
}

/// Blend two `#rrggbb` colors (Remotion's `interpolateColors` for the two-stop case).
pub fn mix_hex(from: &str, to: &str, t: f32) -> String {
    let t = clamp01(t);
    let (Some(a), Some(b)) = (parse_hex(from), parse_hex(to)) else {
        return from.to_owned();
    };
    let c = [
        lerp(a.0 as f32, b.0 as f32, t),
        lerp(a.1 as f32, b.1 as f32, t),
        lerp(a.2 as f32, b.2 as f32, t),
    ];
    format!(
        "#{:02x}{:02x}{:02x}",
        c[0].round().clamp(0.0, 255.0) as u8,
        c[1].round().clamp(0.0, 255.0) as u8,
        c[2].round().clamp(0.0, 255.0) as u8
    )
}

fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.strip_prefix('#')?;
    if hex.len() < 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// CSS rotation of a 2D box, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Horizontal translation in px.
    pub x: f32,
    /// Vertical translation in px.
    pub y: f32,
    /// Uniform scale factor, 1.0 at rest.
    pub scale: f32,
    /// Rotation in degrees.
    pub rotate_deg: f32,
}

impl Default for Pose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
            rotate_deg: 0.0,
        }
    }
}

impl Pose {
    /// The SVG `transform` for this pose, as an SVG transform list string.
    ///
    /// Order matches the CSS transform order the source library uses:
    /// `translate() scale() rotate()`.
    pub fn to_svg_transform(self) -> String {
        let mut parts = Vec::new();
        if self.x.abs() > 1e-4 || self.y.abs() > 1e-4 {
            parts.push(format!("translate({:.4} {:.4})", self.x, self.y));
        }
        if (self.scale - 1.0).abs() > 1e-5 {
            parts.push(format!("scale({:.6})", self.scale));
        }
        if self.rotate_deg.abs() > 1e-4 {
            parts.push(format!("rotate({:.4})", self.rotate_deg));
        }
        if parts.is_empty() {
            "none".to_owned()
        } else {
            parts.join(" ")
        }
    }
}

/// A single SVG `transform` for translation and uniform scale about the element's centre.
pub fn transform_xy_scale(x: f32, y: f32, scale: f32) -> String {
    let mut parts = Vec::new();
    if x.abs() > 1e-4 || y.abs() > 1e-4 {
        parts.push(format!("translate({x:.4} {y:.4})"));
    }
    if (scale - 1.0).abs() > 1e-5 {
        parts.push(format!("scale({scale:.6})"));
    }
    if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_endpoints_are_exact() {
        for curve in [
            EASE_OUT,
            EASE_IN_OUT,
            EASE_ZOOM_IN,
            EASE_DEPART,
            EASE_SNAPPY,
            EASE_BLUR_SETTLE,
            EASE_BLUR_DISSOLVE,
        ] {
            assert!((curve.ease(0.0) - 0.0).abs() < 1e-4);
            assert!((curve.ease(1.0) - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn easing_is_monotonic_enough_for_ease_out() {
        // EASE_OUT is a strong decelerating curve: most of the travel happens early.
        assert!(EASE_OUT.ease(0.25) > 0.5);
    }

    #[test]
    fn speed_is_normalised_and_never_exceeds_one() {
        // The normalisation scans 1001 samples, so the discrete peak lands marginally under 1.
        let peak = (0..=1000)
            .map(|i| speed_of(EASE_DEPART, i as f32 / 1000.0))
            .fold(0.0_f32, f32::max);
        assert!(peak <= 1.0 + 1e-6, "speed exceeded 1: {peak}");
        assert!(peak > 0.95, "speed was never near its peak: {peak}");
    }

    #[test]
    fn spring_starts_at_zero_and_settles_at_one() {
        assert!(SPRING_SETTLE.value(0.0).abs() < 1e-6);
        assert!((SPRING_SETTLE.value(120.0) - 1.0).abs() < 0.01);
        // SPRING_POP is underdamped, so it overshoots past 1 at some point.
        let peak = (0..200)
            .map(|i| SPRING_POP.value(i as f32))
            .fold(0.0_f32, f32::max);
        assert!(peak > 1.0, "expected overshoot, peak was {peak}");
    }

    #[test]
    fn smoothstep_ramps_between_its_ends() {
        assert!(smoothstep(0.0).abs() < 1e-6);
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-6);
        assert!((smoothstep(1.0) - 1.0).abs() < 1e-6);
        assert!(smoothstep(-5.0).abs() < 1e-6);
        assert!((smoothstep(5.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn interpolate_clamps() {
        assert!((interpolate(-5.0, [0.0, 10.0], [0.0, 1.0]) - 0.0).abs() < 1e-6);
        assert!((interpolate(15.0, [0.0, 10.0], [0.0, 1.0]) - 1.0).abs() < 1e-6);
        assert!((interpolate(5.0, [0.0, 10.0], [0.0, 1.0]) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn keyframes_sample_evenly() {
        let k = [1.0, 0.0, 0.0, 0.1, 0.0, 0.0, 1.0];
        assert!((sample_keyframes(&k, 0.0) - 1.0).abs() < 1e-6);
        assert!((sample_keyframes(&k, 1.0) - 1.0).abs() < 1e-6);
        // t = 1/6 lands exactly on keyframe index 1 -> 0.0
        assert!((sample_keyframes(&k, 1.0 / 6.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn random_is_deterministic_and_in_range() {
        let a = random("glitch-10-0-3");
        let b = random("glitch-10-0-3");
        assert!((a - b).abs() < f32::EPSILON);
        for i in 0..500 {
            let v = random(&format!("seed-{i}"));
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
        // Different seeds should not all collapse to the same value.
        assert!((random("a") - random("b")).abs() > f32::EPSILON);
    }

    #[test]
    fn mix_hex_interpolates() {
        assert_eq!(mix_hex("#000000", "#ffffff", 0.0), "#000000");
        assert_eq!(mix_hex("#000000", "#ffffff", 1.0), "#ffffff");
        assert_eq!(mix_hex("#000000", "#ffffff", 0.5), "#808080");
    }

    #[test]
    fn speed_falls_back_when_unusable() {
        assert!((normalised_speed(2.0) - 2.0).abs() < 1e-6);
        assert!((normalised_speed(0.0) - 1.0).abs() < 1e-6);
        assert!((normalised_speed(-3.0) - 1.0).abs() < 1e-6);
        assert!((normalised_speed(f32::NAN) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn entry_frames_staggers_the_whole_staircase() {
        assert_eq!(entry_frames(0, 12, 3, 1.0), 0);
        assert_eq!(entry_frames(1, 12, 3, 1.0), 12);
        assert_eq!(entry_frames(18, 12, 3, 1.0), 63);
        // Speed 2 halves the timeline.
        assert_eq!(entry_frames(15, 21, 1, 2.0), 18);
    }

    #[test]
    fn letter_channels_drops_from_above_when_the_direction_flips() {
        let (opacity, y) = letter_channels(0.0, 0, 3, 12, 7, 46.0, false, 1.0, 1.0, 1.0);
        assert!(opacity.abs() < 1e-6);
        assert!((y - 46.0).abs() < 1e-6);

        let (opacity, y) = letter_channels(0.0, 0, 3, 12, 7, 46.0, false, 1.0, 1.0, -1.0);
        assert!(opacity.abs() < 1e-6);
        assert!((y + 46.0).abs() < 1e-6);

        let (opacity, y) = letter_channels(200.0, 0, 3, 12, 7, 46.0, false, 1.0, 1.0, 1.0);
        assert!((opacity - 1.0).abs() < 1e-6);
        assert!(y.abs() < 1e-6);
    }

    #[test]
    fn letter_channels_holds_later_letters_back_and_honours_reduced_motion() {
        let (opacity, _) = letter_channels(2.0, 1, 3, 12, 7, 46.0, false, 1.0, 1.0, 1.0);
        assert!(opacity.abs() < 1e-6);

        let (_, y) = letter_channels(5.0, 0, 3, 12, 7, 46.0, true, 1.0, 1.5, 1.0);
        assert!(y.abs() < 1e-6);
    }

    #[test]
    fn ms_converts_and_floors_at_one() {
        assert_eq!(ms(120.0), 4);
        assert_eq!(ms(280.0), 8);
        assert_eq!(ms(0.0), 1);
    }
}
