//! # Carousel 3D
//!
//! A ring of cards spinning in 3D space, viewed through a perspective camera. Card `i` sits at
//! angle `i * 360/N + spin` with `x = sin * r`, `z = cos * r`, so the ring turns once every
//! `revolution_frames` frames. The camera transform is the inverse of the configured
//! [`CarouselPose`], which is what makes the world slide toward the viewer when the pose moves.
//!
//! fframes renders SVG, which has no 3D: there is no `perspective`, no `preserve-3d` and no
//! `rotateY`. The ring maths, the timing, the painter ordering and the card size are ported
//! exactly and projected with a real perspective divide; each card is then drawn as a **2.5D
//! approximation** — a rounded rect plus its label, scaled by the projection and squashed
//! horizontally by `|cos theta|` the way a flat card seen at an angle foreshortens. See the
//! README's fidelity notes for exactly what is exact and what is approximated.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `cards` | `Vec<String>` | `["1" .. "8"]` | The card labels, one per ring slot. |
//! | `radius` | `f32` | `60.0` | Ring radius in `vmin`. |
//! | `card_width` | `f32` | `40.0` | Card width in `vmin`. |
//! | `card_height` | `Option<f32>` | `None` | Card height in `vmin`; `None` matches the width. |
//! | `revolution_frames` | `usize` | `300` | Frames per full revolution of the ring. |
//! | `spin_offset` | `f32` | `0.0` | Starting angle of the ring, in degrees. |
//! | `pose` | `CarouselPose` | all zeros | Camera step pose; the emitted camera is its inverse. |
//! | `perspective` | `f32` | `1000.0` | Perspective distance in px. |
//! | `card_facing` | `CardFacing` | `Outward` | Which way the card fronts point. |
//! | `card_fill` | `String` | `"#22303f"` | Card fill. |
//! | `card_text` | `String` | `"#e6edf3"` | Card label fill. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components carousel-3d -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `Carousel3D.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered};

/// Lower clamp on the perspective scale, so a card at the camera plane cannot explode.
pub const MIN_PERSPECTIVE_SCALE: f32 = 0.05;
/// Upper clamp on the perspective scale.
pub const MAX_PERSPECTIVE_SCALE: f32 = 20.0;
/// Card label font size in `vmin` (`fontSize: vmin * 4`).
pub const CARD_FONT_SIZE_VMIN: f32 = 4.0;
/// Card label font weight.
pub const CARD_FONT_WEIGHT: u16 = 700;
/// `|cos theta|` below this counts as edge-on, where a card degenerates to a sliver.
pub const EDGE_ON_EPSILON: f32 = 1e-4;

/// Which way a card's front points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardFacing {
    /// Card fronts face away from the ring's centre, so the near half of the ring shows its face.
    #[default]
    Outward,
    /// Card fronts face the ring's centre, so the far half of the ring shows its face.
    Inward,
}

/// The camera step pose, in `vmin` and degrees.
///
/// The emitted camera transform is this pose's **inverse**, so a negative `z` pulls the camera
/// back away from the ring.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CarouselPose {
    /// Translation along x, in `vmin`.
    pub x: f32,
    /// Translation along y, in `vmin`.
    pub y: f32,
    /// Translation along z, in `vmin`. Negative pulls the camera back.
    pub z: f32,
    /// Rotation about x in degrees, applied first.
    pub rotate_x: f32,
    /// Rotation about y in degrees, applied second.
    pub rotate_y: f32,
    /// Rotation about z in degrees, applied third.
    pub rotate_z: f32,
}

/// A camera transform: `p_camera = rotation * p_world + translation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraTransform {
    /// Row-major rotation (`R^T` of the pose's own rotation).
    rotation: [[f32; 3]; 3],
    /// Translation, already `-R^T * t`.
    translation: [f32; 3],
}

impl CameraTransform {
    /// Transform a world-space point into camera space.
    pub fn apply(&self, point: [f32; 3]) -> [f32; 3] {
        let mut out = [0.0_f32; 3];
        for (row, value) in out.iter_mut().enumerate() {
            *value = self.rotation[row][0] * point[0]
                + self.rotation[row][1] * point[1]
                + self.rotation[row][2] * point[2]
                + self.translation[row];
        }
        out
    }

    /// The camera rotation, row-major.
    pub fn rotation(&self) -> [[f32; 3]; 3] {
        self.rotation
    }

    /// The camera translation.
    pub fn translation(&self) -> [f32; 3] {
        self.translation
    }
}

/// The inverse of a pose matrix, exactly as the source builds it.
///
/// The source composes the pose with three.js as `Matrix4.compose(position, quaternion, scale)`
/// from `Euler(x, y, z, "XYZ")`, then inverts it. For a rotation plus a translation the inverse is
/// exactly `[R^T | -R^T t]`, which is what this returns; the source's `zeroNoise` step only
/// cleaned up the printed CSS `matrix3d` digits and does not affect the maths.
pub fn pose_camera_inverse(pose: &CarouselPose) -> CameraTransform {
    let (sin_x, cos_x) = pose.rotate_x.to_radians().sin_cos();
    let (sin_y, cos_y) = pose.rotate_y.to_radians().sin_cos();
    let (sin_z, cos_z) = pose.rotate_z.to_radians().sin_cos();

    // three.js `Euler(x, y, z, "XYZ")` -> `R = Rx * Ry * Rz`.
    let forward = [
        [cos_y * cos_z, -cos_y * sin_z, sin_y],
        [
            cos_x * sin_z + sin_x * sin_y * cos_z,
            cos_x * cos_z - sin_x * sin_y * sin_z,
            -sin_x * cos_y,
        ],
        [
            sin_x * sin_z - cos_x * sin_y * cos_z,
            sin_x * cos_z + cos_x * sin_y * sin_z,
            cos_x * cos_y,
        ],
    ];
    let rotation = [
        [forward[0][0], forward[1][0], forward[2][0]],
        [forward[0][1], forward[1][1], forward[2][1]],
        [forward[0][2], forward[1][2], forward[2][2]],
    ];
    let position = [pose.x, pose.y, pose.z];
    let mut translation = [0.0_f32; 3];
    for (row, value) in translation.iter_mut().enumerate() {
        *value = -(rotation[row][0] * position[0]
            + rotation[row][1] * position[1]
            + rotation[row][2] * position[2]);
    }

    CameraTransform {
        rotation,
        translation,
    }
}

/// The ring's spin at `frame`, in degrees (`spinOffset + frame * 360 / revolutionFrames`).
pub fn spin_deg(frame: f32, spin_offset: f32, revolution_frames: usize) -> f32 {
    spin_offset + frame * (360.0 / revolution_frames.max(1) as f32)
}

/// Card `index`'s angle on the ring, in degrees.
pub fn card_theta_deg(index: usize, count: usize, spin: f32) -> f32 {
    if count == 0 {
        return spin;
    }
    index as f32 * (360.0 / count as f32) + spin
}

/// Card `index`'s place on the ring, as `(x, z)` in px (`x = sin * r`, `z = cos * r`).
pub fn card_offset(theta_deg: f32, vmin: f32, radius: f32) -> (f32, f32) {
    let theta = theta_deg.to_radians();
    (theta.sin() * vmin * radius, theta.cos() * vmin * radius)
}

/// The perspective divide: `perspective / (perspective - depth)`, clamped.
///
/// Returns `None` when the point is at or behind the camera plane, where a perspective divide is
/// meaningless (and where the source's CSS would project it with a negative scale).
pub fn perspective_scale(depth: f32, perspective: f32) -> Option<f32> {
    let denominator = perspective - depth;
    if denominator <= 0.0 {
        return None;
    }
    Some((perspective / denominator).clamp(MIN_PERSPECTIVE_SCALE, MAX_PERSPECTIVE_SCALE))
}

/// The `rotateY` flip the card facing adds, in degrees.
pub fn card_facing_flip(facing: CardFacing) -> f32 {
    match facing {
        CardFacing::Outward => 0.0,
        CardFacing::Inward => 180.0,
    }
}

/// How much a card is squashed horizontally, or `None` when it is past edge-on and hidden.
///
/// `cos(theta + flip) > 0` is exactly "the card's front still points at the camera": it is 1 when
/// the card faces the camera square on, falls to 0 at 90 degrees (a zero-width sliver) and is
/// hidden past that. `CardFacing::Inward` flips which half of the ring shows a face. Values
/// within [`EDGE_ON_EPSILON`] of zero count as edge-on, so float noise at exactly 90 or 270
/// degrees cannot leave a degenerate zero-width card in the tree.
pub fn card_squash(theta_deg: f32, facing: CardFacing) -> Option<f32> {
    let front = (theta_deg + card_facing_flip(facing)).to_radians().cos();
    if front > EDGE_ON_EPSILON {
        Some(front)
    } else {
        None
    }
}

/// Configuration for [`Carousel3D`].
#[derive(Debug, Clone, PartialEq)]
pub struct Carousel3DConfig {
    /// The card labels, one per ring slot.
    pub cards: Vec<String>,
    /// Ring radius in `vmin`.
    pub radius: f32,
    /// Card width in `vmin`.
    pub card_width: f32,
    /// Card height in `vmin`; `None` matches the width.
    pub card_height: Option<f32>,
    /// Frames per full revolution of the ring.
    pub revolution_frames: usize,
    /// Starting angle of the ring, in degrees.
    pub spin_offset: f32,
    /// Camera step pose; the emitted camera is its inverse.
    pub pose: CarouselPose,
    /// Perspective distance in px.
    pub perspective: f32,
    /// Which way the card fronts point.
    pub card_facing: CardFacing,
    /// Card fill.
    pub card_fill: String,
    /// Card label fill.
    pub card_text: String,
}

impl Default for Carousel3DConfig {
    fn default() -> Self {
        Self {
            cards: (1..=8).map(|n| n.to_string()).collect(),
            radius: 60.0,
            card_width: 40.0,
            card_height: None,
            revolution_frames: 300,
            spin_offset: 0.0,
            pose: CarouselPose::default(),
            perspective: 1000.0,
            card_facing: CardFacing::default(),
            card_fill: "#22303f".to_owned(),
            card_text: "#e6edf3".to_owned(),
        }
    }
}

/// The Carousel 3D component.
#[derive(Debug, Clone, Default)]
pub struct Carousel3D {
    /// Component options.
    pub config: Carousel3DConfig,
}

impl Carousel3D {
    /// Build with the given options.
    pub fn new(config: Carousel3DConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "carousel-3d",
            name: "Carousel 3D",
            summary: "A ring of cards turns through a perspective camera, projected to 2.5D.",
            feature: "carousel-3d",
        }
    }
}

/// Distance from the vertical centre of a font's em box down to its baseline.
fn em_center_to_baseline(ctx: &RenderCtx<'_>, family: &str, size: f32) -> f32 {
    match ctx.meter(family) {
        Some(meter) => (meter.ascent(size) - meter.descent(size)) / 2.0,
        None => size * 0.25,
    }
}

impl Component for Carousel3D {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    /// The source has no duration helper; the ring's own revolution is the natural length.
    fn duration(&self) -> usize {
        self.config.revolution_frames.max(1)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let count = config.cards.len();
        if count == 0 {
            return Svgr::empty();
        }

        let family = ctx.sans();
        let vmin = ctx.vmin();
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let spin = spin_deg(ctx.frame(), config.spin_offset, config.revolution_frames);
        let camera = pose_camera_inverse(&config.pose);

        let width = vmin * config.card_width;
        let height = vmin * config.card_height.unwrap_or(config.card_width);
        let label_size = vmin * CARD_FONT_SIZE_VMIN;
        let label_mid = em_center_to_baseline(ctx, &family, label_size);

        // Ring maths first, then the camera, then the projection.
        let mut projected: Vec<(f32, usize, f32, [f32; 3])> = Vec::with_capacity(count);
        for index in 0..count {
            let theta = card_theta_deg(index, count, spin);
            let (px, pz) = card_offset(theta, vmin, config.radius);
            let cam = camera.apply([px, 0.0, pz]);
            projected.push((pz, index, theta, cam));
        }
        // Painter's algorithm: far (small z) to near, so nearer cards overlap farther ones.
        projected.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut nodes: Vec<Svgr<'_>> = Vec::with_capacity(count);
        for (_, index, theta, cam) in projected {
            let Some(scale) = perspective_scale(cam[2], config.perspective) else {
                continue;
            };
            let Some(squash) = card_squash(theta, config.card_facing) else {
                continue;
            };
            let x = center_x + cam[0] * scale;
            let y = center_y + cam[1] * scale;
            // A non-uniform scale is exactly how a flat card foreshortens: the label squashes
            // with the card, the way it would on a real `rotateY`.
            let transform = format!(
                "translate({x:.4} {y:.4}) scale({:.6} {scale:.6})",
                scale * squash,
            );
            let rect = fframes::svgr!(
                <rect x={-width / 2.0} y={-height / 2.0} width={width} height={height}
                      rx={vmin} fill={config.card_fill.clone()} />
            );
            let label = text_node_centered(
                config.cards.get(index).map_or("", String::as_str),
                0.0,
                label_mid,
                family.clone(),
                label_size,
                CARD_FONT_WEIGHT,
                0.0,
                config.card_text.clone(),
                1.0,
            );
            nodes.push(fframes::svgr!(<g transform={transform}>{rect}{label}</g>));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        (a - b).abs() <= tolerance
    }

    #[test]
    fn spin_turns_a_full_revolution_per_period() {
        assert!(close(spin_deg(0.0, 0.0, 300), 0.0, 1e-6));
        assert!(close(spin_deg(75.0, 0.0, 300), 90.0, 1e-5));
        assert!(close(spin_deg(300.0, 0.0, 300), 360.0, 1e-4));
        // The offset shifts the whole ring.
        assert!(close(spin_deg(0.0, 45.0, 300), 45.0, 1e-6));
        // A zero period is clamped to one frame rather than dividing by zero.
        assert!(close(spin_deg(1.0, 0.0, 0), 360.0, 1e-4));
    }

    #[test]
    fn ring_places_cards_evenly_with_x_from_sin_and_z_from_cos() {
        let count = 8;
        // Index 0 at spin 0 is the front card: x = 0, z = +r.
        let theta = card_theta_deg(0, count, 0.0);
        assert!(close(theta, 0.0, 1e-6));
        let (x, z) = card_offset(theta, 10.0, 60.0);
        assert!(close(x, 0.0, 1e-4));
        assert!(close(z, 600.0, 1e-3));

        // Index 2 is a quarter turn around: x = +r, z = 0.
        let theta = card_theta_deg(2, count, 0.0);
        assert!(close(theta, 90.0, 1e-5));
        let (x, z) = card_offset(theta, 10.0, 60.0);
        assert!(close(x, 600.0, 1e-3), "x was {x}");
        assert!(close(z, 0.0, 1e-3));

        // Index 4 is the far side: z = -r.
        let theta = card_theta_deg(4, count, 0.0);
        let (_, z) = card_offset(theta, 10.0, 60.0);
        assert!(close(z, -600.0, 1e-3));
        // No cards, no division by zero.
        assert!(close(card_theta_deg(0, 0, 12.0), 12.0, 1e-6));
    }

    #[test]
    fn camera_inverse_is_the_identity_for_a_zero_pose() {
        let camera = pose_camera_inverse(&CarouselPose::default());
        assert_eq!(
            camera.rotation(),
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
        );
        let point = camera.apply([3.0, -4.0, 5.0]);
        assert!(close(point[0], 3.0, 1e-6));
        assert!(close(point[1], -4.0, 1e-6));
        assert!(close(point[2], 5.0, 1e-6));
    }

    #[test]
    fn camera_inverse_rolls_and_translates_the_world() {
        // rotateZ 180 maps +x to -x and leaves z alone.
        let camera = pose_camera_inverse(&CarouselPose {
            rotate_z: 180.0,
            ..CarouselPose::default()
        });
        let point = camera.apply([100.0, 0.0, 0.0]);
        assert!(close(point[0], -100.0, 1e-3), "x was {}", point[0]);
        assert!(close(point[2], 0.0, 1e-3));

        // rotateY 90 turns the world's +x round onto camera +z (toward the viewer).
        let camera = pose_camera_inverse(&CarouselPose {
            rotate_y: 90.0,
            ..CarouselPose::default()
        });
        let point = camera.apply([100.0, 0.0, 0.0]);
        assert!(close(point[2], 100.0, 1e-3), "z was {}", point[2]);

        // A pose translated to z = 100 puts the world origin 100 behind the camera.
        let camera = pose_camera_inverse(&CarouselPose {
            z: 100.0,
            ..CarouselPose::default()
        });
        let point = camera.apply([0.0, 0.0, 0.0]);
        assert!(close(point[2], -100.0, 1e-4), "z was {}", point[2]);
    }

    #[test]
    fn perspective_divide_scales_by_depth() {
        // At the screen plane nothing changes.
        assert!(close(
            perspective_scale(0.0, 1000.0).expect("in front"),
            1.0,
            1e-6
        ));
        // Halfway to the camera the card doubles.
        assert!(close(
            perspective_scale(500.0, 1000.0).expect("in front"),
            2.0,
            1e-6
        ));
        // Farther away it shrinks.
        assert!(close(
            perspective_scale(-1000.0, 1000.0).expect("in front"),
            0.5,
            1e-6
        ));
        // At or behind the camera plane there is no projection.
        assert!(perspective_scale(1000.0, 1000.0).is_none());
        assert!(perspective_scale(1500.0, 1000.0).is_none());
        // And the scale is clamped at both ends.
        assert!(close(
            perspective_scale(999.999, 1000.0).expect("in front"),
            MAX_PERSPECTIVE_SCALE,
            1e-4
        ));
        assert!(close(
            perspective_scale(-1.0e9, 1000.0).expect("in front"),
            MIN_PERSPECTIVE_SCALE,
            1e-6
        ));
    }

    #[test]
    fn facing_decides_which_half_of_the_ring_shows_a_face() {
        // Outward: the near half (theta within 90 degrees of the front) is visible.
        assert!(close(
            card_squash(0.0, CardFacing::Outward).expect("visible"),
            1.0,
            1e-6
        ));
        assert!(close(
            card_squash(60.0, CardFacing::Outward).expect("visible"),
            0.5,
            1e-5
        ));
        assert!(card_squash(90.0, CardFacing::Outward).is_none());
        assert!(card_squash(270.0, CardFacing::Outward).is_none());
        assert!(card_squash(180.0, CardFacing::Outward).is_none());
        assert!(card_squash(-120.0, CardFacing::Outward).is_none());

        // Inward flips it: the far half is the one facing the camera.
        assert!(card_squash(0.0, CardFacing::Inward).is_none());
        assert!(card_squash(180.0, CardFacing::Inward).is_some());
        assert!(close(
            card_squash(120.0, CardFacing::Inward).expect("visible"),
            0.5,
            1e-5
        ));
        assert!(close(card_facing_flip(CardFacing::Inward), 180.0, 0.0));
        assert!(close(card_facing_flip(CardFacing::Outward), 0.0, 0.0));
    }

    #[test]
    fn card_size_comes_from_vmin() {
        // 1920x1080 -> vmin = 10.8; the default 40 vmin card is 432 px each way.
        let vmin = 1080.0_f32.min(1920.0) / 100.0;
        assert!(close(vmin * 40.0, 432.0, 1e-4));
        assert!(close(vmin * 60.0, 648.0, 1e-4));
        // A 2:1 card is wider than it is tall.
        let config = Carousel3DConfig {
            card_height: Some(20.0),
            ..Carousel3DConfig::default()
        };
        assert!(close(
            vmin * config.card_height.unwrap_or(config.card_width),
            216.0,
            1e-4
        ));
    }

    #[test]
    fn component_duration_is_one_revolution() {
        let component = Carousel3D::default();
        assert_eq!(component.duration(), 300);
        assert_eq!(component.meta().id, "carousel-3d");
        assert_eq!(component.config.cards.len(), 8);
        assert_eq!(component.config.cards[0], "1");
        assert_eq!(component.config.cards[7], "8");
        let short = Carousel3D::new(Carousel3DConfig {
            revolution_frames: 0,
            ..Carousel3DConfig::default()
        });
        assert_eq!(short.duration(), 1);
    }
}
