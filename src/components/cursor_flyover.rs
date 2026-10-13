//! # Cursor Flyover
//!
//! A "camera" flies over a UI surface: it opens on a centred overview of the whole surface, dives
//! in on each of four corners in turn (each dive ends with that corner filling the screen centre),
//! then pulls back out to a slightly smaller overview — while a cursor arrow leads the way. The
//! cursor fades in at each corner, and pulses on the two left-hand ones.
//!
//! Beat by beat, with the defaults (`step_duration = 45`, four corners):
//!
//! ```text
//! steps     0 overview | 1 top-left | 2 top-right | 3 bottom-left | 4 bottom-right | 5 zoom-out
//! frame     0   step 0 holds the overview pose: the whole surface centred at 65% scale,
//!               the cursor at the canvas centre and invisible
//! frame    45   step 1 begins: the camera starts travelling from the overview to the top-left
//!               pose over 45 frames (EASE_IN_OUT), and the cursor starts travelling the same
//!               path on its own, shorter 35-frame clock
//! frame    45   the cursor starts pulsing — top-left pulses, so the arrow swells to 2x
//! frame  67.5   the pulse is at its peak (2x)
//! frame    80   the cursor has arrived at the corner; the camera is still moving
//! frame    90   the top-left corner of the surface is exactly on the canvas centre (82% scale);
//!               step 2 starts travelling to the top-right from here, and so on
//! frame   225   the zoom-out step begins
//! frame   270   end of the component
//! ```
//!
//! Because `transition_duration` equals `step_duration` by default, each step is one continuous
//! move and every pose is reached exactly as the next step begins. A shorter
//! `transition_duration` leaves the pose resting for the rest of the step instead.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `corners` | `Vec<Corner>` | `[TopLeft, TopRight, BottomLeft, BottomRight]` | Corner sequence to dive to, in order. |
//! | `step_duration` | `usize` | `45` | Frames each step owns; the last step holds until the end. |
//! | `transition_duration` | `usize` | `45` | Frames the camera travels at the start of a step. |
//! | `cursor_transition_duration` | `usize` | `35` | The cursor's own, shorter pose clock. |
//! | `perspective` | `f32` | `1200.0` | CSS `perspective` distance in px. |
//! | `label` | `String` | `"App"` | Text of the placeholder surface card. |
//!
//! The background is `#0a0a0a`.
//!
//! ## How the projection is ported
//!
//! The source composes a three.js `Matrix4` from a pose that is **always a pure translation**
//! (both quaternions are the identity and both scales are 1), inverts it and emits it as CSS
//! `matrix3d` inside a `perspective: d` container. A point `(x, y, 0)` therefore lands at
//!
//! ```text
//! s = d / (d + pose.z)
//! screen = (center + (x - pose.x) * s, center + (y - pose.y) * s)
//! ```
//!
//! which is exactly [`projection_scale`] plus a uniform scale about the canvas centre, applied to
//! the surface card and to the cursor alike. **Every pose, every step boundary and every easing
//! curve is the source's**; the only thing left behind is three.js itself. There is no 3D rotation
//! to lose: the source never used one.
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components cursor-flyover -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `CursorFlyover.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered};
use crate::motion::{EASE_IN_OUT, clamp01, lerp};

/// Background painted behind the component.
pub const BACKGROUND: &str = "#0a0a0a";

/// Fill of the placeholder surface card.
pub const CARD_COLOR: &str = "#1c1c1c";

/// Fill of the placeholder card's label.
pub const LABEL_COLOR: &str = "#e6edf3";

/// The cursor arrow's path, in a `24 x 24` view box, verbatim from the source.
pub const CURSOR_ARROW: &str = "M5 3 L19 12.2 L12 13.4 L8.6 20.4 Z";

/// The cursor box's side in `vmin` (the source's `width={vmin * 5}`).
pub const CURSOR_SIZE_VMIN: f32 = 5.0;

/// The cursor arrow's view box side.
pub const CURSOR_VIEWBOX: f32 = 24.0;

/// The surface card's width in `vmin` (the source's `imgWidth = vmin * 120`).
pub const IMG_WIDTH_VMIN: f32 = 120.0;

/// The surface card's height, as a fraction of its width (the source's `imgHeight`).
pub const IMG_HEIGHT_RATIO: f32 = 0.4;

/// The zoom pose's `z`, as a fraction of the card width (the source's `zoom`).
pub const ZOOM_RATIO: f32 = 0.2;

/// The label's size, in `vmin`, before the projection scale is applied.
pub const LABEL_SIZE_VMIN: f32 = 4.0;

/// One of the surface's four corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
}

impl Corner {
    /// The corner's centre offset from the surface centre, in units of half the surface size.
    pub fn point(self) -> (f32, f32) {
        match self {
            Self::TopLeft => (-0.5, -0.5),
            Self::TopRight => (0.5, -0.5),
            Self::BottomLeft => (-0.5, 0.5),
            Self::BottomRight => (0.5, 0.5),
        }
    }

    /// The source's step id for this corner.
    pub fn id(self) -> &'static str {
        match self {
            Self::TopLeft => "topLeft",
            Self::TopRight => "topRight",
            Self::BottomLeft => "bottomLeft",
            Self::BottomRight => "bottomRight",
        }
    }

    /// Whether the cursor pulses while this corner's step is active.
    ///
    /// The source pulses on the two left-hand corners only.
    pub fn pulses(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }
}

/// A camera pose: a pure translation, in px.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CameraPose {
    /// Horizontal offset from the world origin, in px.
    pub x: f32,
    /// Vertical offset from the world origin, in px.
    pub y: f32,
    /// Depth toward the viewer, in px. Larger is closer, so the projection *shrinks* as it grows.
    pub z: f32,
}

/// A cursor pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPose {
    /// Horizontal position in world px.
    pub x: f32,
    /// Vertical position in world px.
    pub y: f32,
    /// Opacity, `0.0`..`1.0`.
    pub opacity: f32,
    /// Scale multiplier. [`cursor_pose`] already folds the step's pulse into this.
    pub scale: f32,
}

impl Default for CursorPose {
    /// An invisible cursor at the world origin, at scale 1.
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            opacity: 0.0,
            scale: 1.0,
        }
    }
}

/// One step of the flyover timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct FlyoverStep {
    /// The source's step id: `"overview"`, a corner id, or `"zoom-out"`.
    pub id: String,
    /// Where the camera sits during this step.
    pub pose: CameraPose,
    /// Where the cursor sits at the end of this step.
    pub cursor: CursorPose,
    /// Whether the cursor pulses while this step is active.
    pub pulse: bool,
}

/// Configuration for [`CursorFlyover`].
#[derive(Debug, Clone, PartialEq)]
pub struct CursorFlyoverConfig {
    /// Corner sequence to dive to, in order.
    pub corners: Vec<Corner>,
    /// Frames each step owns; the last step holds until the component ends.
    pub step_duration: usize,
    /// Frames the camera travels at the start of a step.
    pub transition_duration: usize,
    /// The cursor's own, shorter pose clock.
    pub cursor_transition_duration: usize,
    /// CSS `perspective` distance in px.
    pub perspective: f32,
    /// Text of the placeholder surface card.
    pub label: String,
}

impl Default for CursorFlyoverConfig {
    fn default() -> Self {
        Self {
            corners: vec![
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomLeft,
                Corner::BottomRight,
            ],
            step_duration: 45,
            transition_duration: 45,
            cursor_transition_duration: 35,
            perspective: 1200.0,
            label: "App".to_owned(),
        }
    }
}

/// The source's step list: an overview, one step per corner, then a zoom-out.
pub fn flyover_steps(
    corners: &[Corner],
    img_width: f32,
    img_height: f32,
    zoom: f32,
) -> Vec<FlyoverStep> {
    let mut steps = Vec::with_capacity(corners.len() + 2);
    steps.push(FlyoverStep {
        id: "overview".to_owned(),
        pose: CameraPose {
            x: 0.0,
            y: 0.0,
            z: img_width * 0.5,
        },
        cursor: CursorPose {
            x: 0.0,
            y: 0.0,
            opacity: 0.0,
            scale: 1.0,
        },
        pulse: false,
    });
    for corner in corners {
        let (px, py) = corner.point();
        steps.push(FlyoverStep {
            id: corner.id().to_owned(),
            pose: CameraPose {
                x: px * img_width,
                y: py * img_height,
                z: zoom,
            },
            cursor: CursorPose {
                x: px * img_width,
                y: py * img_height,
                opacity: 1.0,
                scale: 1.0,
            },
            pulse: corner.pulses(),
        });
    }
    steps.push(FlyoverStep {
        id: "zoom-out".to_owned(),
        pose: CameraPose {
            x: 0.0,
            y: 0.0,
            z: img_width * 0.6,
        },
        cursor: CursorPose {
            x: 0.0,
            y: 0.0,
            opacity: 0.0,
            scale: 0.5,
        },
        pulse: false,
    });
    steps
}

/// Total frames of the flyover: `steps * step_duration`, with `steps = corners + 2`.
///
/// The source has no exported duration helper — its composition picks `durationInFrames` by hand
/// (300 frames in the demo, which is longer than the 270 the default timeline needs). This is the
/// timeline's own length: the last step holds until its slot ends.
pub fn get_cursor_flyover_duration(corner_count: usize, step_duration: usize) -> usize {
    (corner_count + 2) * step_duration
}

/// The index of the active step: the last `k` whose `enter_frame` is `<= frame`, starting at 0.
///
/// `enter_frame(i) = i * step_duration`, and step 0 always owns every frame before step 1 starts.
pub fn active_step(frame: f32, step_count: usize, step_duration: usize) -> usize {
    let mut active = 0;
    for index in 0..step_count {
        if frame >= (index * step_duration) as f32 {
            active = index;
        }
    }
    active
}

/// The uniform projection scale of a point at depth `z`: `perspective / (perspective + z)`.
///
/// This is CSS `perspective: d` with the camera at `translateZ(-z)`. A pose that would land on the
/// camera plane itself (a zero denominator) returns `0.0` instead of an infinity.
pub fn projection_scale(perspective: f32, z: f32) -> f32 {
    let denominator = perspective + z;
    if denominator.abs() < f32::EPSILON {
        0.0
    } else {
        perspective / denominator
    }
}

/// Project a world point into canvas coordinates.
///
/// The camera is a pure translation, so the point is offset by the pose before the perspective
/// divide: with `pose.x = pose.y = 0` this is `(center_x + x * s, center_y + y * s)`.
pub fn project_point(
    x: f32,
    y: f32,
    camera: CameraPose,
    perspective: f32,
    center_x: f32,
    center_y: f32,
) -> (f32, f32) {
    let scale = projection_scale(perspective, camera.z);
    (
        center_x + (x - camera.x) * scale,
        center_y + (y - camera.y) * scale,
    )
}

/// The camera pose at `frame`.
///
/// Step 0 is the overview pose itself (there is nothing to travel from); every later step
/// interpolates its position component-wise from the previous step with
/// [`EASE_IN_OUT`]`(clamp01((frame - enter_frame) / transition_duration))`, then holds.
/// A `transition_duration` of `0` is treated as one frame, so a pose can never become `NaN`.
pub fn camera_pose(
    steps: &[FlyoverStep],
    frame: f32,
    step_duration: usize,
    transition_duration: usize,
) -> CameraPose {
    let Some(first) = steps.first() else {
        return CameraPose::default();
    };
    let active = active_step(frame, steps.len(), step_duration);
    if active == 0 {
        return first.pose;
    }
    let Some(target) = steps.get(active) else {
        return first.pose;
    };
    let progress =
        clamp01((frame - (active * step_duration) as f32) / transition_duration.max(1) as f32);
    let eased = EASE_IN_OUT.ease(progress);
    let from = steps.get(active - 1).map_or(first.pose, |step| step.pose);
    CameraPose {
        x: lerp(from.x, target.pose.x, eased),
        y: lerp(from.y, target.pose.y, eased),
        z: lerp(from.z, target.pose.z, eased),
    }
}

/// The cursor state at `frame`.
///
/// Same structure as [`camera_pose`], on the shorter [`CursorFlyoverConfig::cursor_transition_duration`]
/// clock, with `scale` already multiplied by the step's pulse:
/// `1 + sin(PI * clamp01((frame - enter_frame) / step_duration))` while the step pulses (raw sine,
/// not eased), otherwise `1`.
pub fn cursor_pose(
    steps: &[FlyoverStep],
    frame: f32,
    step_duration: usize,
    cursor_transition_duration: usize,
) -> CursorPose {
    let Some(first) = steps.first() else {
        return CursorPose::default();
    };
    let active = active_step(frame, steps.len(), step_duration);
    let target = steps.get(active).map_or(first.cursor, |step| step.cursor);
    let from = if active == 0 {
        first.cursor
    } else {
        steps
            .get(active - 1)
            .map_or(first.cursor, |step| step.cursor)
    };

    let progress = clamp01(
        (frame - (active * step_duration) as f32) / cursor_transition_duration.max(1) as f32,
    );
    let eased = EASE_IN_OUT.ease(progress);
    let base_scale = lerp(from.scale, target.scale, eased);

    let within = clamp01((frame - (active * step_duration) as f32) / step_duration.max(1) as f32);
    let pulse = if steps.get(active).is_some_and(|step| step.pulse) {
        1.0 + (std::f32::consts::PI * within).sin()
    } else {
        1.0
    };

    CursorPose {
        x: lerp(from.x, target.x, eased),
        y: lerp(from.y, target.y, eased),
        opacity: lerp(from.opacity, target.opacity, eased),
        scale: base_scale * pulse,
    }
}

/// The Cursor Flyover component.
#[derive(Debug, Clone, Default)]
pub struct CursorFlyover {
    /// Component options.
    pub config: CursorFlyoverConfig,
}

impl CursorFlyover {
    /// Build with the given options.
    pub fn new(config: CursorFlyoverConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "cursor-flyover",
            name: "Cursor Flyover",
            summary: "A cursor leads a camera that dives to each corner of a UI surface.",
            feature: "cursor-flyover",
        }
    }
}

impl Component for CursorFlyover {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_cursor_flyover_duration(self.config.corners.len(), self.config.step_duration).max(1)
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKGROUND)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let vmin = ctx.vmin();
        let img_width = vmin * IMG_WIDTH_VMIN;
        let img_height = img_width * IMG_HEIGHT_RATIO;
        let zoom = img_width * ZOOM_RATIO;
        let frame = ctx.frame();

        let steps = flyover_steps(&self.config.corners, img_width, img_height, zoom);
        let camera = camera_pose(
            &steps,
            frame,
            self.config.step_duration,
            self.config.transition_duration,
        );
        let cursor = cursor_pose(
            &steps,
            frame,
            self.config.step_duration,
            self.config.cursor_transition_duration,
        );

        // A perspective at or below the pose depth puts the surface at or behind the camera plane;
        // there is nothing meaningful to draw then, so the frame stays empty instead of mirroring.
        let scale = projection_scale(self.config.perspective, camera.z);
        if !scale.is_finite() || scale <= 0.0 {
            return Svgr::empty();
        }

        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let family = ctx.sans();

        // The surface card, centred on the projected world origin.
        let (card_x, card_y) = project_point(
            0.0,
            0.0,
            camera,
            self.config.perspective,
            center_x,
            center_y,
        );
        let card_width = img_width * scale;
        let card_height = img_height * scale;
        let label_size = vmin * LABEL_SIZE_VMIN * scale;

        let card = fframes::svgr!(
            <rect x={card_x - card_width / 2.0} y={card_y - card_height / 2.0}
                  width={card_width} height={card_height} rx={vmin * 1.5 * scale}
                  fill={CARD_COLOR} />
        );
        let label = text_node_centered(
            &self.config.label,
            card_x,
            card_y + ctx.baseline_offset(&family, label_size, label_size),
            family,
            label_size,
            ctx.fonts.weight_medium,
            0.0,
            LABEL_COLOR.to_owned(),
            1.0,
        );

        // The cursor: the source's arrow, in a `vmin * 5` box scaled by the same projection and by
        // the cursor's own scale, centred on its projected world position.
        let (cursor_x, cursor_y) = project_point(
            cursor.x,
            cursor.y,
            camera,
            self.config.perspective,
            center_x,
            center_y,
        );
        let arrow_box = vmin * CURSOR_SIZE_VMIN * scale * cursor.scale;
        let arrow_scale = if arrow_box.abs() < 1e-4 {
            0.0
        } else {
            arrow_box / CURSOR_VIEWBOX
        };
        let transform = format!(
            "translate({cursor_x:.4} {cursor_y:.4}) scale({arrow_scale:.6}) \
             translate({:.4} {:.4})",
            -CURSOR_VIEWBOX / 2.0,
            -CURSOR_VIEWBOX / 2.0
        );
        let cursor_node = fframes::svgr!(
            <g transform={transform} opacity={cursor.opacity}>
                <path d={CURSOR_ARROW} fill="#ffffff" stroke="#000000" stroke-width="2"
                      stroke-linejoin="round" />
            </g>
        );

        let nodes = Svgr::from(vec![card, label, cursor_node]);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps() -> Vec<FlyoverStep> {
        let vmin = 10.8;
        let img_width = vmin * IMG_WIDTH_VMIN;
        flyover_steps(
            &CursorFlyoverConfig::default().corners,
            img_width,
            img_width * IMG_HEIGHT_RATIO,
            img_width * ZOOM_RATIO,
        )
    }

    #[test]
    fn steps_are_the_source_sequence() {
        let steps = steps();
        assert_eq!(steps.len(), 6);
        let ids: Vec<&str> = steps.iter().map(|step| step.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "overview",
                "topLeft",
                "topRight",
                "bottomLeft",
                "bottomRight",
                "zoom-out"
            ]
        );
        // imgWidth = 120 * 10.8 = 1296, imgHeight = 518.4, zoom = 259.2.
        assert!((steps[0].pose.z - 648.0).abs() < 1e-3);
        assert!((steps[1].pose.x + 648.0).abs() < 1e-3);
        assert!((steps[1].pose.y + 259.2).abs() < 1e-3);
        assert!((steps[1].pose.z - 259.2).abs() < 1e-3);
        assert!((steps[5].pose.z - 777.6).abs() < 1e-3);
        // Cursor follows the camera to each corner and rests at the centre at the ends.
        assert!((steps[1].cursor.x + 648.0).abs() < 1e-3);
        assert!((steps[5].cursor.opacity).abs() < 1e-6);
        assert!((steps[5].cursor.scale - 0.5).abs() < 1e-6);
        // Only the two left-hand corners pulse.
        let pulses: Vec<bool> = steps.iter().map(|step| step.pulse).collect();
        assert_eq!(pulses, vec![false, true, false, true, false, false]);
    }

    #[test]
    fn duration_is_steps_times_step_duration() {
        assert_eq!(get_cursor_flyover_duration(4, 45), 270);
        assert_eq!(get_cursor_flyover_duration(0, 45), 90);
        assert_eq!(get_cursor_flyover_duration(4, 0), 0);
        // A corner list of any length changes the count, never the formula.
        let two = CursorFlyover::new(CursorFlyoverConfig {
            corners: vec![Corner::TopLeft, Corner::BottomRight],
            ..CursorFlyoverConfig::default()
        });
        assert_eq!(two.duration(), 180);
        // A non-positive step duration still yields a positive slot.
        let degenerate = CursorFlyover::new(CursorFlyoverConfig {
            step_duration: 0,
            ..CursorFlyoverConfig::default()
        });
        assert_eq!(degenerate.duration(), 1);
        assert_eq!(CursorFlyover::default().duration(), 270);
        assert_eq!(CursorFlyover::default().meta().id, "cursor-flyover");
        assert_eq!(CursorFlyover::default().background(), Some("#0a0a0a"));
    }

    #[test]
    fn active_step_holds_the_last_one() {
        let steps = steps();
        assert_eq!(active_step(0.0, steps.len(), 45), 0);
        assert_eq!(active_step(44.9, steps.len(), 45), 0);
        assert_eq!(active_step(45.0, steps.len(), 45), 1);
        assert_eq!(active_step(224.0, steps.len(), 45), 4);
        assert_eq!(active_step(225.0, steps.len(), 45), 5);
        assert_eq!(active_step(10_000.0, steps.len(), 45), 5);
        // A zero step duration puts every step on frame 0, so the last one wins.
        assert_eq!(active_step(0.0, steps.len(), 0), 5);
    }

    #[test]
    fn projection_scale_is_the_css_perspective_divide() {
        assert!((projection_scale(1200.0, 0.0) - 1.0).abs() < 1e-6);
        // The overview pose: 1200 / (1200 + 648).
        assert!((projection_scale(1200.0, 648.0) - 1200.0 / 1848.0).abs() < 1e-6);
        // Depth shrinks, and a pose behind the origin enlarges.
        assert!(projection_scale(1200.0, 648.0) < projection_scale(1200.0, 259.2));
        assert!(projection_scale(1200.0, -600.0) > 1.0);
        // The camera plane itself cannot be projected; it reports zero rather than an infinity.
        assert!(projection_scale(1200.0, -1200.0).abs() < 1e-6);
        assert!((projection_scale(0.0, 500.0)).abs() < 1e-6);
    }

    #[test]
    fn project_point_scales_about_the_centre() {
        let camera = CameraPose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // At scale 1 a world point is just offset from the centre.
        assert_eq!(
            project_point(10.0, -20.0, camera, 1200.0, 960.0, 540.0),
            (970.0, 520.0)
        );
        // The world origin lands on the projected camera position, not on the centre.
        let camera = CameraPose {
            x: 648.0,
            y: 259.2,
            z: 259.2,
        };
        let scale = projection_scale(1200.0, 259.2);
        let (x, y) = project_point(0.0, 0.0, camera, 1200.0, 960.0, 540.0);
        assert!((x - (960.0 - 648.0 * scale)).abs() < 1e-3);
        assert!((y - (540.0 - 259.2 * scale)).abs() < 1e-3);
        // The point the camera is looking at lands on the canvas centre.
        let (x, y) = project_point(648.0, 259.2, camera, 1200.0, 960.0, 540.0);
        assert!((x - 960.0).abs() < 1e-3);
        assert!((y - 540.0).abs() < 1e-3);
    }

    #[test]
    fn camera_holds_the_overview_then_eases_between_poses() {
        let steps = steps();
        // Step 0 is the pose itself, at every frame of the step.
        for frame in [0.0, 1.0, 22.0, 44.9] {
            assert_eq!(camera_pose(&steps, frame, 45, 45), steps[0].pose);
        }
        // Half way through the first *transition* — the middle of step 1, which starts at frame
        // 45 — the pose is the eased interpolation of the two poses. `EASE_IN_OUT` is
        // `cubic-bezier(0.77, 0, 0.175, 1)`, which is **not** symmetric, so that is visibly not
        // the linear midpoint.
        let middle = camera_pose(&steps, 67.5, 45, 45);
        let eased = crate::motion::lerp(steps[0].pose.z, steps[1].pose.z, EASE_IN_OUT.ease(0.5));
        assert!((middle.z - eased).abs() < 1e-3);
        let linear = (steps[0].pose.z + steps[1].pose.z) / 2.0;
        assert!(
            (middle.z - linear).abs() > 1.0,
            "the curve must not be linear: {} vs {}",
            middle.z,
            linear
        );
        // A transition shorter than its step holds the target for the rest of the step.
        assert!(close(camera_pose(&steps, 80.0, 45, 30), steps[1].pose));
        // The next step starts exactly from the previous pose (progress 0).
        assert_eq!(camera_pose(&steps, 90.0, 45, 45), steps[1].pose);
        // A zero transition duration is one frame, never a NaN.
        let snapped = camera_pose(&steps, 45.0, 45, 0);
        assert!(snapped.x.is_finite() && snapped.z.is_finite());
        // An empty step list has no pose to report.
        assert_eq!(camera_pose(&[], 10.0, 45, 45), CameraPose::default());
    }

    /// Component-wise comparison with a tolerance, since `lerp(a, b, 1.0)` can round by an ulp.
    fn close(a: CameraPose, b: CameraPose) -> bool {
        (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3 && (a.z - b.z).abs() < 1e-3
    }

    #[test]
    fn cursor_pose_leads_on_its_own_shorter_clock_and_pulses() {
        let steps = steps();
        // The cursor is invisible at the overview, and step 0 has nothing to travel from.
        assert!(cursor_pose(&steps, 0.0, 45, 35).opacity.abs() < 1e-6);
        assert!(close_cursor(
            cursor_pose(&steps, 44.9, 45, 35),
            steps[0].cursor
        ));
        // It arrives at the first corner 35 frames into the step, ten frames before the camera.
        let arrived = cursor_pose(&steps, 80.0, 45, 35);
        assert!((arrived.x - steps[1].cursor.x).abs() < 1e-3);
        assert!((arrived.opacity - 1.0).abs() < 1e-6);
        assert!(
            (camera_pose(&steps, 80.0, 45, 45).x - steps[1].pose.x).abs() > 1.0,
            "the camera should still be moving when the cursor has landed"
        );
        // The pulse is the raw sine, peaking at +1 in the middle of a pulsing step...
        let peak = cursor_pose(&steps, 67.5, 45, 35);
        assert!(
            (peak.scale - 2.0).abs() < 1e-4,
            "expected 2.0, got {}",
            peak.scale
        );
        // ...starting and ending at rest...
        assert!((cursor_pose(&steps, 45.0, 45, 35).scale - 1.0).abs() < 1e-6);
        assert!(cursor_pose(&steps, 89.9, 45, 35).scale < 1.01);
        // ...and absent on a step that does not pulse (top-right is step 2, frames 90..135).
        let right = cursor_pose(&steps, 112.5, 45, 35);
        assert!((right.scale - 1.0).abs() < 1e-6, "got {}", right.scale);
        // The zoom-out shrinks the cursor to 0.5 and fades it out.
        let zoom_out = cursor_pose(&steps, 270.0, 45, 35);
        assert!((zoom_out.scale - 0.5).abs() < 1e-6);
        assert!(zoom_out.opacity.abs() < 1e-6);
        // Every cursor value stays in range across the whole timeline.
        for frame in 0..=270 {
            let pose = cursor_pose(&steps, frame as f32, 45, 35);
            assert!(
                pose.scale > 0.0 && pose.scale <= 2.0001,
                "scale {}",
                pose.scale
            );
            assert!(
                (0.0..=1.0).contains(&pose.opacity),
                "opacity {}",
                pose.opacity
            );
        }
        // A zero cursor transition duration and an empty step list never produce a NaN.
        assert!(cursor_pose(&steps, 45.0, 45, 0).x.is_finite());
        assert_eq!(cursor_pose(&[], 0.0, 45, 35), CursorPose::default());
    }

    /// Component-wise comparison of two cursor poses with a tolerance.
    fn close_cursor(a: CursorPose, b: CursorPose) -> bool {
        (a.x - b.x).abs() < 1e-3
            && (a.y - b.y).abs() < 1e-3
            && (a.opacity - b.opacity).abs() < 1e-3
            && (a.scale - b.scale).abs() < 1e-3
    }

    #[test]
    fn every_frame_renders_without_panicking() {
        let component = CursorFlyover::default();
        let meters = crate::embedded_meters();
        let duration = component.duration();
        assert_eq!(duration, 270);
        for frame in 0..duration {
            let mut raw = fframes::Frame::new(frame, frame, 30);
            let info = crate::ctx::ComponentFrame {
                local: frame as f32,
                global: frame,
                fps: 30.0,
                duration,
                width: 1920.0,
                height: 1080.0,
            };
            let mut ctx = RenderCtx::new(&mut raw, crate::core::Fonts::default(), &meters, info);
            let svg = component.show(&mut ctx);
            assert!(
                !format!("{svg}").is_empty(),
                "frame {frame} produced nothing"
            );
        }

        // A perspective at or below the pose depth has no projection to draw, but must not panic.
        let flat = CursorFlyover::new(CursorFlyoverConfig {
            perspective: 0.0,
            ..CursorFlyoverConfig::default()
        });
        let mut raw = fframes::Frame::new(0, 0, 30);
        let info = crate::ctx::ComponentFrame {
            local: 0.0,
            global: 0,
            fps: 30.0,
            duration: flat.duration(),
            width: 1920.0,
            height: 1080.0,
        };
        let mut ctx = RenderCtx::new(&mut raw, crate::core::Fonts::default(), &meters, info);
        let _ = flat.show(&mut ctx);
    }
}
