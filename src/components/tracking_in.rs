//! # Tracking In
//!
//! A single line settles its letter spacing. One spring drives everything spatial: `t` runs 0 → 1
//! as the spring settles and the tracking is an `interpolate` of it from `start_tracking` (0.5 em,
//! wide open) to `-0.03` em (tightened), with the softness falling from `start_blur` to 0 on the
//! same clock. Opacity is a separate, shorter linear ramp over 15 frames, so the line is already
//! visible while the letters are still closing up.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"TRACKING IN"` | The line to settle. |
//! | `start_tracking` | `f32` | `0.5` | Starting letter spacing in `em`. |
//! | `start_blur` | `f32` | `12.0` | Starting softness radius in reference-canvas px. |
//! | `end_tracking` | `f32` | `-0.03` | Resting letter spacing in `em`. |
//! | `font_size` | `f32` | `96.0` | Reference-canvas font size in px. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `font_weight` | `u16` | `700` | Font weight. |
//! | `damping` | `f32` | `18.0` | Spring damping. |
//! | `stiffness` | `f32` | `90.0` | Spring stiffness. |
//! | `opacity_frames` | `usize` | `15` | Frames the opacity ramp takes. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `45` | Frames held after the line settles. |
//! | `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for a real gaussian blur. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components tracking-in -o out.mp4
//! ```
//!
//! ## Duration
//!
//! `duration = ceil(spring settle frames / speed) + tail`. The spring is solved numerically, so
//! the settle frame is the first frame whose value is within `1e-4` of 1; for the default spring
//! that is frame 28, giving 73 frames, 2.43 s at 30 fps.
//!
//! ## Fidelity
//!
//! `filter: blur(12px) → 0` is **approximated** with offset low-opacity copies, as in
//! [`soft_blur_in`](crate::components::soft_blur_in); SVG has no gaussian blur in this pipeline.
//! Set `simulate_blur: false` to drop the stand-in.
//!
//! ## Credit
//!
//! Ported from `tracking-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::blur::{BLUR_GHOST_COPIES, BLUR_GHOST_SPREAD_RATIO, blur_ghost_alpha};
use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered};
use crate::motion::{FPS, REFERENCE_VMIN, Spring, clamp01, map_range, normalised_speed};

/// The frame at which a spring is considered settled, within `tolerance` of `1.0`.
pub fn spring_settle_frames(spring: Spring, tolerance: f32) -> usize {
    // A few seconds of frames is a generous ceiling; the default spring settles long before it.
    for frame in 0..(FPS as usize * 10) {
        if (spring.value(frame as f32) - 1.0).abs() <= tolerance {
            return frame;
        }
    }
    (FPS as usize * 10).saturating_sub(1)
}

/// Configuration for [`TrackingIn`].
#[derive(Debug, Clone, PartialEq)]
pub struct TrackingInConfig {
    /// The line to settle.
    pub text: String,
    /// Starting letter spacing in `em`.
    pub start_tracking: f32,
    /// Starting softness radius in reference-canvas px.
    pub start_blur: f32,
    /// Resting letter spacing in `em`.
    pub end_tracking: f32,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Text fill.
    pub color: String,
    /// Font weight.
    pub font_weight: u16,
    /// Spring damping.
    pub damping: f32,
    /// Spring stiffness.
    pub stiffness: f32,
    /// Frames the opacity ramp takes.
    pub opacity_frames: usize,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the line settles.
    pub tail: usize,
    /// Draw the secondary copies that stand in for a real gaussian blur.
    pub simulate_blur: bool,
}

impl Default for TrackingInConfig {
    fn default() -> Self {
        Self {
            text: "TRACKING IN".to_owned(),
            start_tracking: 0.5,
            start_blur: 12.0,
            end_tracking: -0.03,
            font_size: 96.0,
            color: "#e6edf3".to_owned(),
            font_weight: 700,
            damping: 18.0,
            stiffness: 90.0,
            opacity_frames: 15,
            speed: 1.0,
            tail: 45,
            simulate_blur: true,
        }
    }
}

impl TrackingInConfig {
    /// The spring driving the tracking and the softness.
    ///
    /// The source uses Remotion's default mass of `1`.
    pub fn spring(&self) -> Spring {
        Spring {
            damping: self.damping,
            mass: 1.0,
            stiffness: self.stiffness,
        }
    }
}

/// Letter spacing, softness and opacity of the line at the component's frame clock.
///
/// `spring` is the raw spring value, which overshoots `1` and therefore lets the tracking pass
/// its resting value before coming back.
pub fn tracking_channels(frame: f32, config: &TrackingInConfig, unit: f32) -> (f32, f32, f32) {
    let local = frame * normalised_speed(config.speed);
    let spring = config.spring().value(local);
    // The spring overshoots, so the range maps must not clamp: `map_range` lets the tracking
    // close past its resting value and the softness pass through it, which is the source's read.
    let tracking = map_range(
        spring,
        [0.0, 1.0],
        [config.start_tracking, config.end_tracking],
    );
    let blur = map_range(spring, [0.0, 1.0], [config.start_blur * unit, 0.0]);
    let opacity = clamp01(local / config.opacity_frames.max(1) as f32);
    (tracking, blur, opacity)
}

/// The Tracking In component.
#[derive(Debug, Clone, Default)]
pub struct TrackingIn {
    /// Component options.
    pub config: TrackingInConfig,
}

impl TrackingIn {
    /// Build with the given options.
    pub fn new(config: TrackingInConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "tracking-in",
            name: "Tracking In",
            summary: "A line closes up its letter spacing as it sharpens into place.",
            feature: "tracking-in",
        }
    }
}

impl Component for TrackingIn {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        let settle = spring_settle_frames(self.config.spring(), 1e-4) as f32;
        (settle / normalised_speed(self.config.speed))
            .ceil()
            .max(0.0) as usize
            + self.config.tail
    }

    /// The library's dark house backdrop, in place of the source's `#ffffff` demo canvas.
    fn background(&self) -> Option<&'static str> {
        Some("#0d1117")
    }
    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = ctx.fonts.sans.clone();
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let size = config.font_size * unit;
        let (tracking, blur, opacity) = tracking_channels(ctx.frame(), config, unit);

        let center_x = ctx.width() / 2.0;
        let baseline = ctx.height() / 2.0 + ctx.baseline_offset(&family, size, size * 1.2);

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        if config.simulate_blur && blur > 0.05 {
            let alpha = blur_ghost_alpha(blur, unit);
            let spread = blur * BLUR_GHOST_SPREAD_RATIO;
            for copy in 1..=BLUR_GHOST_COPIES {
                let sign = if copy % 2 == 0 { -1.0 } else { 1.0 };
                nodes.push(text_node_centered(
                    &config.text,
                    center_x,
                    baseline + spread * (copy as f32 / BLUR_GHOST_COPIES as f32) * sign,
                    family.clone(),
                    size,
                    config.font_weight,
                    tracking,
                    config.color.clone(),
                    opacity * alpha,
                ));
            }
        }

        nodes.push(text_node_centered(
            &config.text,
            center_x,
            baseline,
            family,
            size,
            config.font_weight,
            tracking,
            config.color.clone(),
            opacity,
        ));

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_start_open_and_soft_and_end_tight_and_sharp() {
        let config = TrackingInConfig::default();
        let (tracking, blur, opacity) = tracking_channels(0.0, &config, 1.0);
        assert!((tracking - config.start_tracking).abs() < 1e-6);
        assert!((blur - config.start_blur).abs() < 1e-6);
        assert!(opacity.abs() < 1e-6);

        let (tracking, blur, opacity) = tracking_channels(120.0, &config, 1.0);
        assert!((tracking - config.end_tracking).abs() < 1e-3);
        assert!(blur.abs() < 1e-2);
        assert!((opacity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn opacity_ramps_over_its_own_shorter_clock() {
        let config = TrackingInConfig::default();
        let (tracking, _, opacity) = tracking_channels(15.0, &config, 1.0);
        assert!(
            (opacity - 1.0).abs() < 1e-6,
            "opacity should be done at 15 frames"
        );
        // The spring is nowhere near settled by then, so the tracking is still wide.
        assert!(
            tracking > config.end_tracking,
            "tracking should still be closing at frame 15: {tracking}"
        );
    }

    #[test]
    fn softness_follows_the_canvas_scale() {
        let config = TrackingInConfig::default();
        let (_, blur, _) = tracking_channels(0.0, &config, 1.5);
        assert!((blur - config.start_blur * 1.5).abs() < 1e-5);
    }

    #[test]
    fn spring_settles_within_the_ceiling() {
        let config = TrackingInConfig::default();
        let settle = spring_settle_frames(config.spring(), 1e-4);
        assert!(
            settle > 10 && settle < 300,
            "unexpected settle frame {settle}"
        );
        assert!((config.spring().value(settle as f32) - 1.0).abs() <= 1e-4);
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = TrackingIn::default();
        let settle = spring_settle_frames(component.config.spring(), 1e-4);
        // Damping 18 / stiffness 90 settles on frame 28; the module docs and README state it.
        assert_eq!(settle, 28);
        assert_eq!(component.duration(), 73);
        assert_eq!(component.meta().id, "tracking-in");
    }
}
