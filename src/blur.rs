//! Shared softness stand-in: the low-opacity copies components draw in place of a real gaussian.
//!
//! A component is compiled in only when its own Cargo feature is on, so a softness stand-in two
//! components share cannot live in either of them. This module holds the one set the character- and
//! word-level reveals share; it is **not** the only set in the crate. The word-level components
//! [`blur_slide_text`](crate::components::blur_slide_text) and
//! [`text_build`](crate::components::text_build) each carry their own local constants, with
//! deliberately different numbers (a 10 px reference, a 0.35 cap, a 0.6 spread ratio and a single
//! trailing copy), and they are independent of these.
//!
//! Source: <https://github.com/Remocn/remocn> (MIT).

use crate::motion::clamp01;

/// Softness radius at which the secondary copies reach [`BLUR_GHOST_MAX_ALPHA`], in reference px.
pub const BLUR_GHOST_REFERENCE_PX: f32 = 12.0;

/// Total opacity the secondary copies reach at [`BLUR_GHOST_REFERENCE_PX`].
///
/// The source's gaussian is isotropic; this stand-in spreads the budget over
/// [`BLUR_GHOST_COPIES`] offset copies, so each one is dimmer than the single trailing copy of
/// the word-level components.
pub const BLUR_GHOST_MAX_ALPHA: f32 = 0.45;

/// How many offsets the softness stand-in draws per glyph.
pub const BLUR_GHOST_COPIES: usize = 2;

/// Displacement of the furthest secondary copy per px of softness radius.
pub const BLUR_GHOST_SPREAD_RATIO: f32 = 0.5;

/// Below this softness radius no secondary copy is drawn.
pub const BLUR_GHOST_MIN_PX: f32 = 0.05;

/// Alpha of each secondary copy for a softness radius in canvas px.
///
/// The reference radius is the configured starting softness brought onto the canvas, so the budget
/// is spent exactly when a glyph starts at full softness.
pub fn blur_ghost_alpha(blur_px: f32, unit: f32) -> f32 {
    let reference = BLUR_GHOST_REFERENCE_PX * unit;
    clamp01(blur_px / reference.max(f32::EPSILON)) * BLUR_GHOST_MAX_ALPHA
        / BLUR_GHOST_COPIES.max(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_alpha_shares_the_budget_and_is_capped() {
        let unit = 1.5;
        assert!(blur_ghost_alpha(0.0, unit).abs() < 1e-6);
        let at_reference = blur_ghost_alpha(BLUR_GHOST_REFERENCE_PX * unit, unit);
        assert!((at_reference - BLUR_GHOST_MAX_ALPHA / BLUR_GHOST_COPIES as f32).abs() < 1e-6);
        assert!((blur_ghost_alpha(1000.0, unit) - at_reference).abs() < 1e-6);
        // Full softness at a smaller unit is a smaller radius; the budget still saturates.
        assert!(
            (blur_ghost_alpha(BLUR_GHOST_REFERENCE_PX, 1.0) - at_reference).abs() < 1e-6,
            "the reference must follow the canvas scale"
        );
    }
}
