//! A vertical text wheel that spins one full turn and settles on its first item.
//!
//! Ported from Remotion Elements' `SpinningTextWheel`. The source stacks one absolutely
//! positioned row per item inside a 400 x 200 px box, places each row on a circle with
//! `translateZ` / `translateY` / `rotateX`, fades the box's top and bottom with a mask, and
//! drives a full turn with a slow overdamped spring. Here the box is resolved to scene
//! coordinates in that same 400 x 200 box, which one group fits to the canvas.
//!
//! Deliberate differences from the source:
//!
//! - **Projection.** The source's `perspective: 10000px` on the box is applied as the matching
//!   scale about the box centre (`10000 / (10000 - z)`), and each row's own `perspective: 1000px`
//!   is ignored. The counter-rotation inside each row is kept as the vertical squeeze it
//!   produces, `cos(angle)^2`, since the row's rotation and the text's counter-rotation both
//!   foreshorten it. Rows facing away (`cos(angle) <= 0`) are hidden, as `backface-visibility`
//!   does.
//! - **Mask.** The CSS `mask-image` gradient is an SVG luminance mask with the same stops.
//! - **Font.** The source loads Mona Sans Bold, which this crate does not embed; the embedded
//!   Inter Bold stands in.
//! - **Spring.** `durationInFrames: duration * 0.75` with `durationRestThreshold: 0.0001` makes
//!   Remotion stretch the spring in time so it comes to rest in that many frames; the same time
//!   stretch is applied here.
//! - **Background and size.** The source draws nothing behind the box and its composition size
//!   is not published; the page is a light neutral and the box is fitted to 40% of the canvas.
//! - **Duration.** The source takes the composition's length; the default here is 150 frames.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{Spring, clamp01, lerp};
use crate::ui::text;

/// The wheel box's width, in source px.
pub const BOX_W: f32 = 400.0;
/// The wheel box's height, in source px.
pub const BOX_H: f32 = 200.0;
/// The share of the canvas the box may fill.
pub const MAX_SHARE: f32 = 0.4;
/// The page behind the box: a light neutral, chosen because the source has no background.
pub const BACKGROUND: &str = "#f4f5f8";

/// The radius of the wheel the rows sit on, in px (`* 100` in the source's transform).
pub const WHEEL_RADIUS: f32 = 100.0;
/// The box's `perspective`, in px.
pub const PERSPECTIVE: f32 = 10000.0;
/// The opacity of every row but the selected one.
pub const DIM_OPACITY: f32 = 0.28;
/// The progress at which the selected row starts to brighten.
pub const BRIGHTEN_FROM: f32 = 0.88;
/// The mask stops, as fractions of the box height: transparent, opaque, opaque, transparent.
pub const MASK_STOPS: [f32; 4] = [0.0, 0.3, 0.7, 1.0];

/// The text's colour.
pub const INK: &str = "#182033";
/// The text's font size.
pub const FONT_SIZE: f32 = 65.0;
/// The text's font weight.
pub const FONT_WEIGHT: u16 = 700;

/// The share of the duration the spin takes (`durationInFrames * 0.75`).
pub const SPIN_SHARE: f32 = 0.75;
/// The distance from rest at which the spring counts as settled.
pub const REST_THRESHOLD: f32 = 0.0001;
/// The spin's spring.
pub const SPIN_SPRING: Spring = Spring {
    damping: 200.0,
    mass: 10.0,
    stiffness: 200.0,
};

/// The frame count at which [`SPIN_SPRING`] is first within [`REST_THRESHOLD`] of rest.
#[must_use]
pub fn natural_frames() -> f32 {
    for frame in 0..100_000 {
        if (SPIN_SPRING.value(frame as f32) - 1.0).abs() < REST_THRESHOLD {
            return frame as f32;
        }
    }
    100_000.0
}

/// The spin's progress, 0 to 1, at `frame` of a `duration` frame video.
///
/// The spring is stretched in time so that it is at rest after `duration * 0.75` frames.
#[must_use]
pub fn progress(frame: f32, duration: usize) -> f32 {
    let span = (duration as f32 * SPIN_SHARE).max(1.0);
    if frame >= span {
        return 1.0;
    }
    SPIN_SPRING.value(frame.max(0.0) * natural_frames() / span)
}

/// The remaining turns at `progress`: one full turn at the start, none at rest.
#[must_use]
pub fn rotation(progress: f32) -> f32 {
    lerp(1.0, 0.0, progress)
}

/// Where one row sits on the wheel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowPose {
    /// How far below the box centre the row's centre projects.
    pub y: f32,
    /// The uniform scale perspective gives the row.
    pub scale: f32,
    /// The extra vertical squeeze from the row's tilt.
    pub squeeze: f32,
    /// Whether the row faces the viewer.
    pub visible: bool,
}

/// The pose of row `index` of `count` when `rotation` turns remain.
#[must_use]
pub fn row_pose(index: usize, count: usize, rotation: f32) -> RowPose {
    let wheel_index = index as f32 / count.max(1) as f32 + rotation;
    let angle = wheel_index * std::f32::consts::TAU;
    let facing = angle.cos();
    let scale = PERSPECTIVE / (PERSPECTIVE - facing * WHEEL_RADIUS);
    RowPose {
        y: angle.sin() * WHEEL_RADIUS * scale,
        scale,
        squeeze: facing * facing,
        visible: facing > 1e-4,
    }
}

/// The opacity of row `index` at `progress`.
#[must_use]
pub fn row_opacity(index: usize, progress: f32) -> f32 {
    if index == 0 {
        let t = clamp01((progress - BRIGHTEN_FROM) / (1.0 - BRIGHTEN_FROM));
        lerp(DIM_OPACITY, 1.0, t)
    } else {
        DIM_OPACITY
    }
}

/// The scale that fits the box to the canvas.
#[must_use]
pub fn scene_scale(canvas_width: f32, canvas_height: f32) -> f32 {
    (MAX_SHARE * canvas_width / BOX_W).min(MAX_SHARE * canvas_height / BOX_H)
}

/// The items of a newline separated list, trimmed, without blanks.
#[must_use]
pub fn parse_items(items: &str) -> Vec<String> {
    items
        .split('\n')
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect()
}

/// What a [`SpinningTextWheel`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct SpinningTextWheelConfig {
    /// The rows, one per line. The first is the one the wheel settles on.
    pub items: String,
    /// The text colour.
    pub color: String,
    /// The font size, in source px.
    pub font_size: f32,
    /// The font weight.
    pub font_weight: u16,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for SpinningTextWheelConfig {
    fn default() -> Self {
        Self {
            items: "Friday\nSaturday\nSunday\nMonday\nTuesday\nWednesday\nThursday".to_owned(),
            color: INK.to_owned(),
            font_size: FONT_SIZE,
            font_weight: FONT_WEIGHT,
            font_family: None,
            duration: 150,
        }
    }
}

/// The spinning text wheel component.
#[derive(Debug, Clone, Default)]
pub struct SpinningTextWheel {
    /// What the wheel shows.
    pub config: SpinningTextWheelConfig,
}

impl SpinningTextWheel {
    /// A wheel with `config`.
    #[must_use]
    pub fn new(config: SpinningTextWheelConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "spinning-text-wheel",
            name: "Spinning Text Wheel",
            summary: "A vertical text wheel spins a full turn and settles on its first item",
            feature: "spinning-text-wheel",
        }
    }
}

impl Component for SpinningTextWheel {
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
        let items = parse_items(&config.items);
        let frame = ctx.frame();

        let scale = scene_scale(ctx.width(), ctx.height());
        let fit = format!(
            "translate({} {}) scale({scale})",
            (ctx.width() - BOX_W * scale) / 2.0,
            (ctx.height() - BOX_H * scale) / 2.0
        );

        let p = progress(frame, config.duration);
        let turns = rotation(p);
        // A line box of one font size, centred on the row's origin.
        let baseline = ctx.baseline_offset_at(
            &family,
            config.font_weight,
            config.font_size,
            config.font_size,
        ) - config.font_size / 2.0;

        let mut rows: Vec<Svgr<'static>> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let pose = row_pose(index, items.len(), turns);
            if !pose.visible {
                continue;
            }
            let transform = format!(
                "translate({} {}) scale({} {})",
                BOX_W / 2.0,
                BOX_H / 2.0 + pose.y,
                pose.scale,
                pose.scale * pose.squeeze
            );
            rows.push(fframes::svgr!(
                <g transform={transform}>
                    {text::text_node(
                        ctx,
                        item,
                        0.0,
                        baseline,
                        family.clone(),
                        config.font_size,
                        config.font_weight,
                        0.0,
                        config.color.clone(),
                        row_opacity(index, p),
                    )}
                </g>
            ));
        }

        fframes::svgr!(
            <g transform={fit}>
                <defs>
                    <linearGradient id="spinning-text-wheel-fade" gradientUnits="userSpaceOnUse"
                                    x1={0.0} y1={0.0} x2={0.0} y2={BOX_H}>
                        <stop offset={MASK_STOPS[0]} stop-color="#000000" />
                        <stop offset={MASK_STOPS[1]} stop-color="#ffffff" />
                        <stop offset={MASK_STOPS[2]} stop-color="#ffffff" />
                        <stop offset={MASK_STOPS[3]} stop-color="#000000" />
                    </linearGradient>
                    <mask id="spinning-text-wheel-mask" maskUnits="userSpaceOnUse"
                          x={0.0} y={0.0} width={BOX_W} height={BOX_H}>
                        <rect x={0.0} y={0.0} width={BOX_W} height={BOX_H}
                              fill="url(#spinning-text-wheel-fade)" />
                    </mask>
                </defs>
                <g mask="url(#spinning-text-wheel-mask)">
                    {Svgr::from(rows)}
                </g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = SpinningTextWheelConfig::default();
        let items = parse_items(&config.items);
        assert_eq!(items.len(), 7);
        assert_eq!(items[0], "Friday");
        assert_eq!(items[6], "Thursday");
        assert_eq!(config.color, "#182033");
        assert_eq!(config.font_size, 65.0);
        assert_eq!(config.font_weight, 700);
        assert_eq!(config.duration, 150);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = SpinningTextWheel::default();
        assert_eq!(component.meta().id, "spinning-text-wheel");
        assert_eq!(
            SpinningTextWheel::meta_static().feature,
            "spinning-text-wheel"
        );
        assert_eq!(component.duration(), 150);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn items_are_trimmed_and_blanks_dropped() {
        assert_eq!(parse_items(" a \n\n b\n  "), ["a", "b"]);
    }

    #[test]
    fn the_spin_starts_a_turn_out_and_comes_to_rest() {
        assert!(progress(0.0, 150).abs() < 1e-6);
        assert!((rotation(0.0) - 1.0).abs() < 1e-6);
        assert!((progress(112.4, 150) - 1.0).abs() < 1e-3);
        assert_eq!(progress(149.0, 150), 1.0);
        assert!(rotation(1.0).abs() < 1e-6);
        let mid = progress(30.0, 150);
        assert!(mid > 0.0 && mid < 1.0);
        // Monotonic: the spring is overdamped.
        assert!(progress(40.0, 150) >= mid);
    }

    #[test]
    fn a_full_turn_looks_like_rest() {
        for i in 0..7 {
            let start = row_pose(i, 7, 1.0);
            let rest = row_pose(i, 7, 0.0);
            assert!((start.y - rest.y).abs() < 1e-2);
            assert_eq!(start.visible, rest.visible);
        }
    }

    #[test]
    fn at_rest_the_first_row_is_centred_and_upright() {
        let pose = row_pose(0, 7, 0.0);
        assert!(pose.y.abs() < 1e-4);
        assert!((pose.squeeze - 1.0).abs() < 1e-6);
        assert!(pose.visible);
        // The row after it hangs below, at the bottom of the fade.
        let next = row_pose(1, 7, 0.0);
        assert!(next.y > 0.0 && next.y < WHEEL_RADIUS);
    }

    #[test]
    fn rows_facing_away_are_hidden() {
        // Rows 2 to 5 of 7 sit between 103 and 257 degrees.
        assert!(!row_pose(3, 7, 0.0).visible);
        assert!(!row_pose(4, 7, 0.0).visible);
    }

    #[test]
    fn only_the_first_row_brightens() {
        assert_eq!(row_opacity(0, 0.5), DIM_OPACITY);
        assert_eq!(row_opacity(0, 0.88), DIM_OPACITY);
        assert!((row_opacity(0, 1.0) - 1.0).abs() < 1e-6);
        assert_eq!(row_opacity(2, 1.0), DIM_OPACITY);
    }

    #[test]
    fn the_box_fits_landscape_and_portrait_canvases() {
        let landscape = scene_scale(1920.0, 1080.0);
        assert!(BOX_W * landscape <= 1920.0 && BOX_H * landscape <= 1080.0);
        let portrait = scene_scale(1080.0, 1920.0);
        assert!(BOX_W * portrait <= 1080.0);
    }
}
