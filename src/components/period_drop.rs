//! # Period Drop
//!
//! A word rises letter by letter out of its own clipping box, and then a coloured dot falls from
//! high above the line, lands, and bounces twice with a squash-and-stretch deformation.
//!
//! The physics is the source's own, not an eased imitation. Gravity is set by the fall time
//! (`g = 2 / fall²`, so the landing speed is `2 / fall`), each bounce leaves the floor at
//! `restitution` times the previous speed, and the deformation is the impact squash decaying as
//! `e^(-τ / recover)` while the in-flight stretch tracks the instantaneous speed. Every lift,
//! squash and scale is derived from those two constants, so the dot's rhythm is exact.
//!
//! | beat | frames (30 fps) | what happens |
//! | --- | --- | --- |
//! | 0 → 22 | rise | each letter climbs from `115%` below its line box, 2 frames apart, over 12 frames under `EASE_OUT_QUART` |
//! | 20 | release | the dot becomes visible at `-0.62 * height` |
//! | 32 | landing | `drop_start + fall_frames` — first impact at full speed, squashing on contact |
//! | 36.2 | first apex | top of the `0.35`-restitution hop |
//! | 40.4 | second impact | the first bounce lands |
//! | 41.87 | second apex | top of the next hop |
//! | 43.34 | third impact | the last bounce lands; the deformation decays from here |
//! | 44 | rest | `motion_frames`: the deformation has decayed below the `0.005` snap threshold |
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"remocn"` | The word the dot follows. |
//! | `font_size` | `f32` | `160.0` | Reference-canvas font size in px. |
//! | `font_weight` | `u16` | `800` | Font weight. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `dot_color` | `String` | `"#d97757"` | The dot's fill. |
//! | `dot_size` | `f32` | `0.2` | The dot's diameter in `em`. |
//! | `rise_frames` | `usize` | `12` | Frames each letter takes to climb. |
//! | `rise_stagger` | `usize` | `2` | Frames between consecutive letter starts. |
//! | `drop_start` | `f32` | `20.0` | Frame the dot is released on. |
//! | `fall_frames` | `f32` | `12.0` | Frames the dot takes to fall. |
//! | `restitution` | `f32` | `0.35` | Speed kept at each bounce. |
//! | `bounces` | `usize` | `2` | How many bounces the dot takes. |
//! | `drop_height` | `f32` | `0.62` | Drop height as a share of the canvas height. |
//! | `stretch` | `f32` | `0.3` | `scale_y` gained at full speed. |
//! | `squash` | `f32` | `0.22` | `scale_y` lost at a full-speed impact. |
//! | `recover` | `f32` | `1.1` | Frames for an impact squash to decay by 1/e. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `45` | Frames held after the dot comes to rest. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components period-drop -o out.mp4
//! ```
//!
//! ## Duration
//!
//! `duration = ceil(max(final impact, letter rise) / speed) + tail`, where the impacts are
//! `drop_start + fall_frames`, then each bounce adds `2 * restitution^n * fall_frames`. With the
//! defaults the last impact is frame 43.34, so 44 frames of motion plus the tail: 89 frames,
//! 2.97 s at 30 fps.
//!
//! ## Fidelity notes
//!
//! - **The letter clip.** The source wraps every letter in an `overflow: hidden` box with a
//!   `padding-bottom` / negative `margin-bottom` pair so descenders survive. SVG clips to a
//!   rectangle, so the port clips each letter to its own line box extended by the descender
//!   allowance — the same crop, computed from the embedded face's metrics instead of the browser's
//!   line box.
//! - **Scaling.** The source is authored on a 1280x720 canvas, so every design px is multiplied by
//!   `ctx.vmin() / 7.2` (1.5 on the default 1920x1080). The drop height is a share of the canvas
//!   height on both sides.
//!
//! ## Credit
//!
//! Ported from `period-drop/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, char_centers};
use crate::motion::{REFERENCE_VMIN, ease_out_quart_value, normalised_speed};

/// Configuration for [`PeriodDrop`].
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodDropConfig {
    /// The word the dot follows.
    pub text: String,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Text fill.
    pub color: String,
    /// The dot's fill.
    pub dot_color: String,
    /// The dot's diameter in `em`.
    pub dot_size: f32,
    /// Frames each letter takes to climb.
    pub rise_frames: usize,
    /// Frames between consecutive letter starts.
    pub rise_stagger: usize,
    /// Frame the dot is released on.
    pub drop_start: f32,
    /// Frames the dot takes to fall.
    pub fall_frames: f32,
    /// Speed kept at each bounce.
    pub restitution: f32,
    /// How many bounces the dot takes.
    pub bounces: usize,
    /// Drop height as a share of the canvas height.
    pub drop_height: f32,
    /// `scale_y` gained at full speed.
    pub stretch: f32,
    /// `scale_y` lost at a full-speed impact.
    pub squash: f32,
    /// Frames for an impact squash to decay by 1/e.
    pub recover: f32,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the dot comes to rest.
    pub tail: usize,
}

impl Default for PeriodDropConfig {
    fn default() -> Self {
        Self {
            text: "remocn".to_owned(),
            font_size: 160.0,
            font_weight: 800,
            color: "#e6edf3".to_owned(),
            dot_color: "#d97757".to_owned(),
            dot_size: 0.2,
            rise_frames: 12,
            rise_stagger: 2,
            drop_start: 20.0,
            fall_frames: 12.0,
            restitution: 0.35,
            bounces: 2,
            drop_height: 0.62,
            stretch: 0.3,
            squash: 0.22,
            recover: 1.1,
            speed: 1.0,
            tail: 45,
        }
    }
}

/// How far letter `index` still sits below its line, `1 → 0`.
///
/// `start = rise_stagger * index`, then an `EASE_OUT_QUART` ramp over `rise_frames`.
pub fn rise_offset(frame: f32, index: usize, config: &PeriodDropConfig) -> f32 {
    let local = frame * normalised_speed(config.speed);
    let start = (config.rise_stagger * index) as f32;
    let span = config.rise_frames.max(1) as f32;
    let p = ease_out_quart_value(((local - start) / span).clamp(0.0, 1.0));
    1.0 - p
}

/// Frames at which the dot touches the line: the landing, then each bounce.
pub fn impacts(config: &PeriodDropConfig) -> Vec<f32> {
    let fall = config.fall_frames;
    let mut out = vec![config.drop_start + fall];
    for n in 1..=config.bounces {
        let previous = out[n - 1];
        out.push(previous + 2.0 * config.restitution.powi(n as i32) * fall);
    }
    out
}

/// The dot's state at the component's frame clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotState {
    /// Whether the dot has been released yet.
    pub visible: bool,
    /// Height above the rest point as a share of the drop: `0` at rest, `-1` at release.
    pub y: f32,
    /// Horizontal squash, the reciprocal of `scale_y`.
    pub scale_x: f32,
    /// Vertical stretch.
    pub scale_y: f32,
}

/// The deformation the dot carries at `deform` (positive stretches, negative squashes).
pub fn dot_scale(deform: f32, config: &PeriodDropConfig) -> (f32, f32) {
    let scale_y = if deform >= 0.0 {
        1.0 + config.stretch * deform
    } else {
        1.0 + config.squash * deform.max(-1.0)
    };
    (1.0 / scale_y, scale_y)
}

/// Sample the dot's fall and bounces.
///
/// Heights are shares of the drop height; time is in 30 fps frames.
pub fn dot_state(frame: f32, config: &PeriodDropConfig) -> DotState {
    let t = frame * normalised_speed(config.speed);
    let fall = config.fall_frames.max(f32::EPSILON);
    if t < config.drop_start {
        return DotState {
            visible: false,
            y: -1.0,
            scale_x: 1.0,
            scale_y: 1.0,
        };
    }

    // `g = 2 / fall²` makes the landing speed exactly `2 / fall`.
    let g = 2.0 / (fall * fall);
    let landing = g * fall;
    let impacts = impacts(config);

    // The dot's own gradient: the fall accelerates, a bounce decelerates, and both are solved
    // from the same `g` so the impacts line up with `impacts()` exactly.
    let mut y = 0.0_f32;
    let mut speed = 0.0_f32;
    let mut deform;

    if t < impacts[0] {
        let u = t - config.drop_start;
        y = -1.0 + 0.5 * g * u * u;
        speed = g * u;
        deform = speed / landing;
    } else {
        let mut n = impacts.len() - 1;
        while n > 0 && t < impacts[n] {
            n -= 1;
        }
        let tau = t - impacts[n];
        let bounce = n + 1;
        let launch = if bounce < impacts.len() {
            config.restitution.powi(bounce as i32) * landing
        } else {
            0.0
        };
        let flight = if launch > 0.0 { 2.0 * launch / g } else { 0.0 };
        if tau < flight {
            y = -(launch * tau - 0.5 * g * tau * tau);
            speed = (launch - g * tau).abs();
        }
        let impact = config.restitution.powi(n as i32);
        let decay = (-tau / config.recover.max(f32::EPSILON)).exp();
        deform = -impact * decay + (1.0 - decay) * (speed / landing);
    }

    // Snap the last imperceptible wobble so the dot rests exactly.
    if deform.abs() < 0.005 {
        deform = 0.0;
    }
    let (scale_x, scale_y) = dot_scale(deform, config);
    DotState {
        visible: true,
        y: y.min(0.0),
        scale_x,
        scale_y,
    }
}

/// Frames the whole beat takes, before the tail.
pub fn motion_frames(config: &PeriodDropConfig) -> usize {
    let last_impact = impacts(config).last().copied().unwrap_or(0.0);
    let rise = (config.rise_stagger * config.text.chars().count().saturating_sub(1)) as f32
        + config.rise_frames as f32;
    let span = last_impact.max(rise) / normalised_speed(config.speed);
    span.ceil().max(0.0) as usize
}

/// The Period Drop component.
#[derive(Debug, Clone, Default)]
pub struct PeriodDrop {
    /// Component options.
    pub config: PeriodDropConfig,
}

impl PeriodDrop {
    /// Build with the given options.
    pub fn new(config: PeriodDropConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "period-drop",
            name: "Period Drop",
            summary: "A word rises letter by letter, then a dot falls and bounces.",
            feature: "period-drop",
        }
    }
}

impl Component for PeriodDrop {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        motion_frames(&self.config) + self.config.tail
    }

    /// The library's dark house backdrop, in place of the source's own demo canvas.
    fn background(&self) -> Option<&'static str> {
        Some("#0d1117")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = ctx.fonts.sans.clone();
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let size = config.font_size * unit;
        let frame = ctx.frame();

        let (_, total) = char_centers(ctx, &family, &config.text, size);
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let ascent = ctx.ascent(&family, size);
        let descent = size - ascent;
        let baseline = center_y + ctx.baseline_offset(&family, size, size);

        // Every letter climbs out of its own clip box, which the source builds with
        // `overflow: hidden` plus a descender allowance.
        let clip_top = baseline - ascent - size * 0.2;
        let clip_bottom = baseline + descent + size * 0.2;

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        let mut pen = -total / 2.0;
        for (index, character) in config.text.chars().enumerate() {
            let glyph = character.to_string();
            let width = ctx.text_width(&family, size, &glyph);
            let rise = rise_offset(frame, index, config);
            let x = center_x + pen;
            let dy = rise * size * 1.15;
            let clip_id = format!("period-drop-{index}");
            nodes.push(fframes::svgr!(
                <g>
                    <defs>
                        <clipPath id={clip_id.clone()}>
                            <rect x={x - 1.0} y={clip_top} width={width + 2.0}
                                  height={clip_bottom - clip_top} />
                        </clipPath>
                    </defs>
                    <g clip-path={format!("url(#{clip_id})")}>
                        <text x={x} y={baseline + dy} font-family={family.clone()}
                              font-size={size} font-weight={config.font_weight}
                              fill={config.color.clone()}
                              letter-spacing="0">{glyph}</text>
                    </g>
                </g>
            ));
            pen += width;
        }

        // The dot, anchored at its bottom so the squash pivots on the line it lands on.
        let dot = dot_state(frame, config);
        if dot.visible {
            let diameter = config.dot_size * size;
            let drop = config.drop_height * ctx.height();
            let rest_bottom = baseline;
            let scaled_height = diameter * dot.scale_y;
            let scaled_width = diameter * dot.scale_x;
            // The source offsets the dot by `marginLeft: 0.04em`, which is a share of the *font
            // size*, not of the dot's own diameter.
            let center_dot_x = center_x + total / 2.0 + size * 0.04 + scaled_width / 2.0;
            let bottom = rest_bottom + dot.y * drop;
            nodes.push(fframes::svgr!(
                <ellipse cx={center_dot_x} cy={bottom - scaled_height / 2.0}
                         rx={scaled_width / 2.0} ry={scaled_height / 2.0}
                         fill={config.dot_color.clone()} />
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_climb_one_stagger_apart() {
        let config = PeriodDropConfig::default();
        assert!((rise_offset(0.0, 0, &config) - 1.0).abs() < 1e-6);
        assert!((rise_offset(0.0, 1, &config) - 1.0).abs() < 1e-6);
        // Letter 1 starts 2 frames later, so at frame 2 letter 0 has moved and letter 1 has not.
        assert!(rise_offset(2.0, 0, &config) < 1.0);
        assert!((rise_offset(2.0, 1, &config) - 1.0).abs() < 1e-6);
        // Everything has landed once the slowest letter is done.
        assert!(rise_offset(60.0, 5, &config).abs() < 1e-6);
    }

    #[test]
    fn impact_frames_follow_the_restitution() {
        let config = PeriodDropConfig::default();
        let impacts = impacts(&config);
        // landing, then +2*0.35*12 and +2*0.1225*12
        assert!((impacts[0] - 32.0).abs() < 1e-6);
        assert!((impacts[1] - 40.4).abs() < 1e-6);
        assert!((impacts[2] - 43.34).abs() < 1e-4);
    }

    #[test]
    fn the_dot_is_hidden_until_release() {
        let config = PeriodDropConfig::default();
        let before = dot_state(19.0, &config);
        assert!(!before.visible);
        assert!((before.y + 1.0).abs() < 1e-6);

        let after = dot_state(20.0, &config);
        assert!(after.visible);
        assert!((after.y + 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_dot_lands_at_the_rest_point_at_full_speed() {
        let config = PeriodDropConfig::default();
        // A hair before contact the dot is falling at full speed, so it is stretched.
        let falling = dot_state(config.drop_start + config.fall_frames - 0.25, &config);
        assert!(
            falling.y.abs() < 0.06,
            "expected the dot just above the rest point, got {}",
            falling.y
        );
        assert!(
            falling.scale_y > 1.2,
            "full-speed fall should stretch the dot, got {}",
            falling.scale_y
        );
        assert!(falling.scale_x < 1.0);

        // At the contact frame the impact squash takes over immediately.
        let contact = dot_state(config.drop_start + config.fall_frames, &config);
        assert!(contact.y.abs() < 1e-6);
        assert!(
            contact.scale_y < 1.0,
            "the impact should squash the dot, got {}",
            contact.scale_y
        );
    }

    #[test]
    fn the_dot_squashes_on_impact_and_settles() {
        let config = PeriodDropConfig::default();
        // Just after the first landing the impact squash dominates.
        let squash = dot_state(config.drop_start + config.fall_frames + 0.3, &config);
        assert!(
            squash.scale_y < 1.0,
            "expected a squash, got {}",
            squash.scale_y
        );
        assert!(squash.scale_x > 1.0, "the squash must preserve area");
        assert!(
            (squash.scale_x * squash.scale_y - 1.0).abs() < 1e-5,
            "squash must preserve area"
        );

        // Long after the last bounce the dot is at rest and undeformed.
        let rest = dot_state(200.0, &config);
        assert!(rest.y.abs() < 1e-6);
        assert!((rest.scale_y - 1.0).abs() < 1e-6);
        assert!((rest.scale_x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_dot_never_rises_above_its_release_point() {
        let config = PeriodDropConfig::default();
        for step in 0..600 {
            let state = dot_state(step as f32 * 0.1, &config);
            assert!(
                state.y <= 1e-6,
                "the dot went above the line at {}: {}",
                step as f32 * 0.1,
                state.y
            );
        }
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = PeriodDrop::default();
        // The last impact is frame 43.34; the letter rise ends at 12 + 5*2 = 22.
        assert_eq!(motion_frames(&component.config), 44);
        assert_eq!(component.duration(), 89);
        assert_eq!(component.meta().id, "period-drop");
    }
}
