//! Model B geometry: a scene drawn inside a measured reference recording's own pixel space.
//!
//! Most components paint straight onto a 1280x720 layout canvas, so their numbers scale by
//! `ctx.vmin() / REFERENCE_VMIN` (see [`crate::ui::text::unit`]). Others instead wrap
//! the whole scene in an SVG whose `viewBox` is a **recording the component was fitted to**, and
//! scale that box to fit the frame:
//!
//! ```js
//! const u = Math.min(width / REF_W, height / REF_H);
//! const left = (width - REF_W * u) / 2;
//! const top  = (height - REF_H * u) / 2;
//! ```
//!
//! Every interior number in such a component is in *reference-recording* pixels, and multiplying it
//! by `unit` is wrong by a large factor — for check-cycle's 718x394 box the two differ by
//! `2.6744 / 1.5`, i.e. the scene renders at 56% of its proper size.
//!
//! So a Model B port keeps its interior coordinates untouched and wraps them in one group carrying
//! [`ReferenceFrame::transform`].

use crate::ctx::RenderCtx;

/// A measured reference recording, fitted to the canvas and centred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReferenceFrame {
    /// Width of the reference recording, in its own pixels.
    pub width: f32,
    /// Height of the reference recording, in its own pixels.
    pub height: f32,
    /// The factor the box is scaled by: `min(canvas_w / width, canvas_h / height)`.
    pub scale: f32,
    /// Left edge of the fitted box on the canvas.
    pub left: f32,
    /// Top edge of the fitted box on the canvas.
    pub top: f32,
}

impl ReferenceFrame {
    /// Fit a `width` x `height` reference box into a canvas of the given size, centred.
    ///
    /// The canvas size is a parameter rather than read from a [`RenderCtx`] so the arithmetic can
    /// be tested without a frame.
    #[must_use]
    pub fn fit_in(canvas_width: f32, canvas_height: f32, width: f32, height: f32) -> Self {
        if width <= 0.0 || height <= 0.0 {
            return Self {
                width,
                height,
                scale: 1.0,
                left: 0.0,
                top: 0.0,
            };
        }
        let scale = (canvas_width / width).min(canvas_height / height);
        Self {
            width,
            height,
            scale,
            left: (canvas_width - width * scale) / 2.0,
            top: (canvas_height - height * scale) / 2.0,
        }
    }

    /// Fit a reference box to the canvas this frame is being drawn on.
    #[must_use]
    pub fn fit(ctx: &RenderCtx<'_>, width: f32, height: f32) -> Self {
        Self::fit_in(ctx.width(), ctx.height(), width, height)
    }

    /// The `transform` for a group whose children are in reference-recording pixels.
    #[must_use]
    pub fn transform(&self) -> String {
        format!(
            "translate({} {}) scale({})",
            self.left, self.top, self.scale
        )
    }

    /// A reference-space point, in canvas px.
    #[must_use]
    pub fn point(&self, x: f32, y: f32) -> (f32, f32) {
        (self.left + x * self.scale, self.top + y * self.scale)
    }

    /// A length in reference pixels, in canvas px.
    #[must_use]
    pub fn length(&self, value: f32) -> f32 {
        value * self.scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// check-cycle's box, which is the component this helper was written for.
    const REF_W: f32 = 718.0;
    const REF_H: f32 = 394.0;

    #[test]
    fn a_wider_canvas_is_limited_by_width() {
        // 1280/718 = 1.783, 720/394 = 1.827 -> the width is the tighter constraint.
        let frame = ReferenceFrame::fit_in(1280.0, 720.0, REF_W, REF_H);
        assert!((frame.scale - 1.782_729_8).abs() < 1e-5, "{}", frame.scale);
        // The box is narrower than the canvas is tall, so it is centred vertically.
        assert!(frame.left.abs() < 1e-4);
        assert!((frame.top - 8.76).abs() < 0.05, "{}", frame.top);
    }

    #[test]
    fn at_1080p_the_scale_is_not_the_design_unit() {
        // The bug this module exists to prevent: unit (1.5) is not the right factor here.
        let frame = ReferenceFrame::fit_in(1920.0, 1080.0, REF_W, REF_H);
        assert!((frame.scale - 2.674_4).abs() < 0.001, "{}", frame.scale);
        let unit: f32 = 1080.0 / 100.0 / 7.2;
        assert!((unit - 1.5).abs() < 1e-6, "the design unit is 1.5 at 1080p");
        assert!(
            (frame.scale / unit - 1.782_9).abs() < 0.001,
            "using the design unit would render the scene at {:.3}x",
            unit / frame.scale
        );
    }

    #[test]
    fn a_tall_canvas_is_still_width_limited() {
        // 1080/718 = 1.504, 1920/394 = 4.873 -> the *width* is still the tighter constraint, so
        // the box fills the width and is centred vertically.
        let frame = ReferenceFrame::fit_in(1080.0, 1920.0, REF_W, REF_H);
        assert!((frame.scale - 1.504_178_3).abs() < 1e-5, "{}", frame.scale);
        assert!(frame.left.abs() < 1e-4, "left {}", frame.left);
        assert!(
            (frame.top - (1920.0 - REF_H * frame.scale) / 2.0).abs() < 1e-3,
            "top {}",
            frame.top
        );
        assert!((frame.top - 663.68).abs() < 0.05, "top {}", frame.top);
    }

    #[test]
    fn a_box_wider_than_the_canvas_is_height_limited() {
        // A 400x400 box on a 1920x1080 canvas: 4.8 vs 2.7, so the height binds and the box is
        // centred horizontally instead.
        let frame = ReferenceFrame::fit_in(1920.0, 1080.0, 400.0, 400.0);
        assert!((frame.scale - 2.7).abs() < 1e-5, "{}", frame.scale);
        assert!(frame.top.abs() < 1e-4, "top {}", frame.top);
        assert!((frame.left - (1920.0 - 400.0 * 2.7) / 2.0).abs() < 1e-3);
    }

    #[test]
    fn the_transform_places_and_scales_the_interior() {
        let frame = ReferenceFrame::fit_in(1920.0, 1080.0, REF_W, REF_H);
        assert_eq!(
            frame.transform(),
            format!(
                "translate({} {}) scale({})",
                frame.left, frame.top, frame.scale
            )
        );
        // A reference-space point maps to the middle of the canvas at the box's centre.
        let (cx, cy) = frame.point(REF_W / 2.0, REF_H / 2.0);
        assert!((cx - 960.0).abs() < 0.05, "{cx}");
        assert!((cy - 540.0).abs() < 0.05, "{cy}");
    }

    #[test]
    fn a_degenerate_box_does_not_divide_by_zero() {
        let frame = ReferenceFrame::fit_in(1920.0, 1080.0, 0.0, 0.0);
        assert!((frame.scale - 1.0).abs() < 1e-6);
        assert!(frame.transform().contains("scale(1)"));
    }
}
