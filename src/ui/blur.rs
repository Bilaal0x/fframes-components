//! Approximating CSS `filter: blur()` for SVG.
//!
//! SVG filters are not dependable in this pipeline, so a blurred draw becomes a small ring of
//! offset copies at reduced opacity. That is an approximation, not a Gaussian: it widens and
//! softens the shape, which is what the source uses blur for here (a defocused arrival and a
//! soft pulse while a line reflows), but it will not reproduce a true Gaussian falloff.
//!
//! Every component that blurs something should use this helper so the approximation stays
//! consistent, and should let the caller disable it with a `simulate_blur` flag.

/// How many offset copies stand in for one blurred draw.
pub const COPIES: usize = 4;

/// The ring's radius as a fraction of the blur radius.
pub const SPREAD_RATIO: f32 = 0.6;

/// The total extra opacity the ring contributes at [`REFERENCE_PX`].
pub const MAX_ALPHA: f32 = 0.55;

/// The blur radius, in reference-canvas px, at which the ring reaches [`MAX_ALPHA`].
pub const REFERENCE_PX: f32 = 6.0;

/// Below this radius a blur is invisible, so no copies are drawn.
pub const MIN_PX: f32 = 0.05;

/// The opacity of each individual ring copy for a blur of `blur_px` reference px.
///
/// Scales with the radius up to [`REFERENCE_PX`] and is then capped, so a large blur reads as
/// softness rather than as a second, brighter glyph.
#[must_use]
pub fn ghost_alpha(blur_px: f32, unit: f32) -> f32 {
    let reference = REFERENCE_PX * unit;
    if reference <= 0.0 {
        return 0.0;
    }
    (blur_px / reference).clamp(0.0, 1.0) * MAX_ALPHA / COPIES as f32
}

/// The ring offsets, in canvas px, for a blur of `blur_px` reference px.
///
/// The copies sit evenly around a circle, which is what makes an isotropic blur read as defocus
/// rather than as a drop shadow.
#[must_use]
pub fn ghost_offsets(blur_px: f32, unit: f32) -> [(f32, f32); COPIES] {
    let radius = blur_px * unit * SPREAD_RATIO;
    std::array::from_fn(|index| {
        let angle = std::f32::consts::TAU * index as f32 / COPIES as f32;
        (radius * angle.cos(), radius * angle.sin())
    })
}

/// Whether a blur of `blur_px` reference px is worth drawing copies for.
#[must_use]
pub fn is_visible(blur_px: f32) -> bool {
    blur_px > MIN_PX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_grows_with_the_radius_and_caps() {
        assert!((ghost_alpha(0.0, 1.0)).abs() < 1e-6);
        assert!(ghost_alpha(3.0, 1.0) < ghost_alpha(6.0, 1.0));
        let at_reference = ghost_alpha(REFERENCE_PX, 1.0);
        assert!((at_reference - MAX_ALPHA / COPIES as f32).abs() < 1e-6);
        assert!((ghost_alpha(1000.0, 1.0) - at_reference).abs() < 1e-6);
    }

    #[test]
    fn alpha_follows_the_canvas_scale() {
        // A blur expressed in reference px is physically larger at 1080p, so it needs less
        // relative opacity to read the same.
        assert!(ghost_alpha(6.0, 1.5) < ghost_alpha(6.0, 1.0));
    }

    #[test]
    fn offsets_sit_on_a_ring_of_the_right_radius() {
        let offsets = ghost_offsets(6.0, 1.0);
        let expected = 6.0 * SPREAD_RATIO;
        for (x, y) in offsets {
            let radius = x.hypot(y);
            assert!(
                (radius - expected).abs() < 1e-4,
                "radius {radius} should be {expected}"
            );
        }
        // The first copy points along +x, the third along -x.
        assert!((offsets[0].0 - expected).abs() < 1e-4);
        assert!((offsets[2].0 + expected).abs() < 1e-4);
    }

    #[test]
    fn an_invisible_blur_draws_nothing() {
        assert!(!is_visible(0.0));
        assert!(!is_visible(MIN_PX));
        assert!(is_visible(MIN_PX + 0.01));
    }
}
