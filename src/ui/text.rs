//! Measured text drawing for the cinematic component collection.
//!
//! This is deliberately separate from [`crate::ctx::text_node_centered`]. Tracking is emitted in
//! pixels, the anchor is `start`, and a run is centred from measured glyph advances so layout and
//! SVG rendering use the same units.

use fframes::Svgr;

use crate::ctx::RenderCtx;
use crate::motion::REFERENCE_VMIN;

/// The width of `text` as CSS lays it out with `tracking_px` between glyphs.
///
/// Tracking applies after every glyph including the last, so the ink spans `n - 1` gaps.
#[must_use]
pub fn tracked_width(
    ctx: &RenderCtx<'_>,
    family: &str,
    size: f32,
    text: &str,
    tracking_px: f32,
) -> f32 {
    let glyphs = text.chars().count();
    let advances = ctx.text_width(family, size, text);
    if glyphs > 1 {
        advances + (glyphs - 1) as f32 * tracking_px
    } else {
        advances
    }
}

/// The tracking a source `em` value means at this size and canvas scale.
///
/// `em` tracking is relative to the font size, which is already scaled, so this needs the scale
/// factor only once.
#[must_use]
pub fn tracking_px_from_em(tracking_em: f32, size: f32) -> f32 {
    tracking_em * size
}

/// Advance width of `text` as the face a renderer would draw at `weight` measures it.
///
/// A renderer resolves `font-family` plus `font-weight` to the nearest available face, and advance
/// widths differ between weights — so a run *drawn* at one weight must be *measured* at that
/// weight, or it gets centred and spaced against the wrong width. Falls back to the family's
/// default face when no weighted face is registered.
fn advances_at(ctx: &RenderCtx<'_>, family: &str, weight: u16, size: f32, text: &str) -> f32 {
    match ctx.meter_at(family, weight) {
        Some(meter) => meter.text_width(text, size),
        None => ctx.text_width(family, size, text),
    }
}

/// [`tracked_width`], measured at a specific weight.
///
/// Components that lay a run out themselves — to place words, or to centre something by hand —
/// should use this one whenever they also draw with a `font-weight`.
#[must_use]
pub fn tracked_width_at(
    ctx: &RenderCtx<'_>,
    family: &str,
    weight: u16,
    size: f32,
    text: &str,
    tracking_px: f32,
) -> f32 {
    let glyphs = text.chars().count();
    let advances = advances_at(ctx, family, weight, size, text);
    if glyphs > 1 {
        advances + (glyphs - 1) as f32 * tracking_px
    } else {
        advances
    }
}

/// A `<text>` run centred on `center_x`, with tracking in px, drawn from its left edge.
///
/// Emitting `text-anchor="start"` at a width this crate measures keeps the visual centre correct
/// whatever the renderer assumes about trailing letter-spacing.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn text_node(
    ctx: &RenderCtx<'_>,
    text: &str,
    center_x: f32,
    baseline_y: f32,
    family: String,
    size: f32,
    weight: u16,
    tracking_px: f32,
    fill: String,
    opacity: f32,
) -> Svgr<'static> {
    let width = tracked_width_at(ctx, family.as_str(), weight, size, text, tracking_px);
    let x = center_x - width / 2.0;
    // The weight has to reach the renderer as text; see `ctx::font_weight_attr`.
    let weight = crate::ctx::font_weight_attr(weight);
    fframes::svgr!(
        <text x={x} y={baseline_y} font-family={family} font-size={size} font-weight={weight}
              letter-spacing={tracking_px} fill={fill} opacity={opacity}>{text.to_owned()}</text>
    )
}

/// A `<text>` run whose **left edge** sits at `left_x`, with tracking in px.
///
/// [`text_node`] centres a run, which is what a source that positions text by its middle wants.
/// A source that gives a slot's *left* coordinate instead needs this function. Passing a left edge
/// to [`text_node`] places the run half its width too far left.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn text_node_left(
    text: &str,
    left_x: f32,
    baseline_y: f32,
    family: String,
    size: f32,
    weight: u16,
    tracking_px: f32,
    fill: String,
    opacity: f32,
) -> Svgr<'static> {
    // The weight has to reach the renderer as text; see `ctx::font_weight_attr`.
    let weight = crate::ctx::font_weight_attr(weight);
    fframes::svgr!(
        <text x={left_x} y={baseline_y} font-family={family} font-size={size} font-weight={weight}
              letter-spacing={tracking_px} fill={fill} opacity={opacity}>{text.to_owned()}</text>
    )
}

/// The baseline that centres a line box on `center_y`.
///
/// CSS centres an inline box, not a baseline, so a component that positions text by its centre
/// needs the box's own height — the font's `line-height: normal` — and the distance from the box
/// top down to the baseline.
#[must_use]
pub fn baseline_for_centred_box(
    ctx: &RenderCtx<'_>,
    family: &str,
    size: f32,
    center_y: f32,
) -> f32 {
    let box_height = ctx
        .meter(family)
        .map_or(size * 1.2, |meter| meter.line_height(size));
    center_y - box_height / 2.0 + ctx.baseline_offset(family, size, box_height)
}

/// The canvas scale factor for collection geometry: `1.5` on 1920x1080, `1.0` on 720p.
#[must_use]
pub fn unit(ctx: &RenderCtx<'_>) -> f32 {
    ctx.vmin() / REFERENCE_VMIN
}

/// The face used when neither a component's config nor its theme names one.
///
/// The source `"Default"` option preserves the face each component was designed around. That is
/// usually Inter or a system UI stack that cannot be resolved outside a browser, so Inter is the
/// deterministic fallback for both.
///
/// Falls back to the crate's own sans face when Inter is not measurable, so this is safe to call
/// whether or not the embedded faces are wired up, and every component that uses it upgrades
/// itself the moment they are.
#[must_use]
pub fn default_face(ctx: &RenderCtx<'_>) -> String {
    /// The face this family prefers, when the crate can measure it.
    const PREFERRED: &str = "Inter";
    if ctx.meter(PREFERRED).is_some() {
        PREFERRED.to_owned()
    } else {
        ctx.sans()
    }
}

/// Resolve a component's face: its own option, then its theme's, then [`default_face`].
///
/// An explicit family is honoured as given even when this crate cannot measure it — a caller who
/// names a face is making a decision, and the READMEs say which faces are embedded.
#[must_use]
pub fn resolve_face(
    config_family: Option<&String>,
    theme_family: Option<&String>,
    ctx: &RenderCtx<'_>,
) -> String {
    config_family
        .or(theme_family)
        .cloned()
        .unwrap_or_else(|| default_face(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn em_tracking_scales_with_the_font_size() {
        assert!((tracking_px_from_em(-0.03, 108.0) + 3.24).abs() < 1e-6);
        assert!((tracking_px_from_em(0.0, 108.0)).abs() < 1e-6);
    }
}
