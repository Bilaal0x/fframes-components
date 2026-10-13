//! # Confetti
//!
//! A deterministic particle burst: `particle_count` rounded rectangles fire from a point, each on
//! its own angle and speed from a seeded PRNG, and fall under gravity while fluttering around their
//! own axis.
//!
//! Every particle's state is a closed-form function of the burst time, so there is no simulation
//! and no per-frame state: the same frame always renders the same pixels, and the whole burst can
//! be scrubbed.
//!
//! | channel | formula |
//! | --- | --- |
//! | launch velocity | `(cos θ * speed * power + drift, sin θ * speed * power)`, in reference px/frame |
//! | position | `origin + v * t`, plus `0.5 * gravity * t²` on `y` |
//! | rotation | `rot0 + spin * t` radians |
//! | flutter | `cos(rot0 + spin * t * 1.6)`, used as the horizontal scale |
//! | opacity | ramp in over 4 frames, hold, then ramp out over the last 30% of the lifetime |
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `particle_count` | `usize` | `140` | How many pieces the burst fires. |
//! | `colors` | `Vec<String>` | 6 colours | Palette the pieces draw from. |
//! | `origin_x` | `f32` | `0.5` | Burst origin across the canvas, `0..1`. |
//! | `origin_y` | `f32` | `0.5` | Burst origin down the canvas, `0..1`. |
//! | `start_frame` | `usize` | `0` | Frame the burst fires on. |
//! | `lifetime` | `usize` | `90` | Frames the burst lives for. |
//! | `power` | `f32` | `17.0` | Launch speed, reference px per frame. |
//! | `gravity` | `f32` | `0.45` | Downward acceleration, reference px per frame². |
//! | `size` | `f32` | `13.0` | Base piece size in reference px. |
//! | `seed` | `u32` | `1` | PRNG seed; the same seed always renders the same burst. |
//! | `aspect` | `f32` | `0.62` | Piece height as a share of its width. |
//! | `corner_radius` | `f32` | `2.0` | Piece corner radius in reference px. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components confetti -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = start_frame + lifetime
//! ```
//!
//! With the defaults that is 90 frames, 3.00 s at 30 fps.
//!
//! ## Fidelity notes
//!
//! - **The palette default.** The source's six brand colours are reproduced exactly, on the dark
//!   house backdrop rather than the source's own demo canvas.
//! - **Scaling.** The source is authored on a 1280x720 canvas and scales its particle sizes and
//!   physics by `height / 720`. The port uses `ctx.vmin() / 7.2` (`REFERENCE_VMIN`), which is the
//!   same factor on the reference canvas and canvas-independent everywhere else. Sizes are
//!   reference px: `size: 13` draws at 19.5 px on 1080p.
//!
//! ## Credit
//!
//! Ported from `confetti/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{Mulberry32, REFERENCE_VMIN, interpolate};

/// The palette [`ConfettiConfig::default`] draws from.
pub const DEFAULT_COLORS: [&str; 6] = [
    "#1d9bf0", "#ff5da2", "#ffd23f", "#22c55e", "#a855f7", "#ff7a45",
];

/// One piece's immutable launch parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    /// Launch angle in radians.
    pub angle: f32,
    /// Speed multiplier in `0.35..1.0`.
    pub speed: f32,
    /// The piece's colour.
    pub color: String,
    /// Size multiplier in `0.7..1.3`.
    pub size: f32,
    /// Initial rotation in radians.
    pub rot0: f32,
    /// Spin per frame in radians.
    pub spin: f32,
    /// Constant horizontal drift added to the launch velocity.
    pub drift: f32,
}

/// Build the deterministic particle set for a seed.
///
/// The draw order is exactly the source's, so the same seed gives the same scatter.
pub fn make_particles(count: usize, seed: u32, colors: &[String]) -> Vec<Particle> {
    let palette: Vec<String> = if colors.is_empty() {
        DEFAULT_COLORS.iter().map(|c| (*c).to_owned()).collect()
    } else {
        colors.to_vec()
    };
    let mut rand = Mulberry32::new(seed);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let angle = rand.next_f32() * std::f32::consts::TAU;
        let speed = 0.35 + 0.65 * rand.next_f32();
        // The colour is the next draw, even when the palette has one entry, so the sequence stays
        // aligned with the source's.
        let color = palette[rand.index(palette.len())].clone();
        out.push(Particle {
            angle,
            speed,
            color,
            size: 0.7 + 0.6 * rand.next_f32(),
            rot0: rand.next_f32() * std::f32::consts::TAU,
            spin: (rand.next_f32() - 0.5) * 0.6,
            drift: (rand.next_f32() - 0.5) * 2.0,
        });
    }
    out
}

/// The opacity envelope over a piece's lifetime: in over 4 frames, out over the last 30%.
pub fn particle_opacity(tt: f32, lifetime: f32) -> f32 {
    if tt < 0.0 || tt > lifetime {
        return 0.0;
    }
    let fade_in = interpolate(tt, [0.0, 4.0], [0.0, 1.0]);
    let fade_out = interpolate(tt, [lifetime * 0.7, lifetime], [1.0, 0.0]);
    fade_in.min(fade_out)
}

/// A piece's position, rotation and flutter at burst time `tt`.
///
/// `origin` is in canvas px and the returned position is too; `scale` is the canvas unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticlePose {
    /// Centre x in canvas px.
    pub x: f32,
    /// Centre y in canvas px.
    pub y: f32,
    /// Rotation in degrees.
    pub rotate_deg: f32,
    /// Horizontal scale, the flutter.
    pub flutter: f32,
}

/// Sample one piece at burst time `tt`.
pub fn particle_pose(
    particle: &Particle,
    tt: f32,
    origin: (f32, f32),
    power: f32,
    gravity: f32,
    unit: f32,
) -> ParticlePose {
    let (ox, oy) = origin;
    let vx = particle.angle.cos() * particle.speed * power + particle.drift;
    let vy = particle.angle.sin() * particle.speed * power;
    ParticlePose {
        x: ox + vx * tt * unit,
        y: oy + (vy * tt + 0.5 * gravity * tt * tt) * unit,
        rotate_deg: (particle.rot0 + particle.spin * tt).to_degrees(),
        flutter: (particle.rot0 + particle.spin * tt * 1.6).cos(),
    }
}

/// Configuration for [`Confetti`].
#[derive(Debug, Clone, PartialEq)]
pub struct ConfettiConfig {
    /// How many pieces the burst fires.
    pub particle_count: usize,
    /// Palette the pieces draw from. Empty means [`DEFAULT_COLORS`].
    pub colors: Vec<String>,
    /// Burst origin across the canvas, `0..1`.
    pub origin_x: f32,
    /// Burst origin down the canvas, `0..1`.
    pub origin_y: f32,
    /// Frame the burst fires on.
    pub start_frame: usize,
    /// Frames the burst lives for.
    pub lifetime: usize,
    /// Launch speed, reference px per frame.
    pub power: f32,
    /// Downward acceleration, reference px per frame².
    pub gravity: f32,
    /// Base piece size in reference px.
    pub size: f32,
    /// PRNG seed.
    pub seed: u32,
    /// Piece height as a share of its width.
    pub aspect: f32,
    /// Piece corner radius in reference px.
    pub corner_radius: f32,
}

impl Default for ConfettiConfig {
    fn default() -> Self {
        Self {
            particle_count: 140,
            colors: Vec::new(),
            origin_x: 0.5,
            origin_y: 0.5,
            start_frame: 0,
            lifetime: 90,
            power: 17.0,
            gravity: 0.45,
            size: 13.0,
            seed: 1,
            aspect: 0.62,
            corner_radius: 2.0,
        }
    }
}

/// The Confetti component.
#[derive(Debug, Clone)]
pub struct Confetti {
    /// Component options.
    pub config: ConfettiConfig,
    particles: std::sync::OnceLock<Vec<Particle>>,
}

impl Default for Confetti {
    fn default() -> Self {
        Self::new(ConfettiConfig::default())
    }
}

impl Confetti {
    /// Build with the given options.
    pub fn new(config: ConfettiConfig) -> Self {
        Self {
            config,
            particles: std::sync::OnceLock::new(),
        }
    }

    /// The particle set, built once per component instance.
    ///
    /// `OnceLock` keeps `show` free of per-frame allocation and stays `Sync` for the concurrent
    /// renderer.
    pub fn particles(&self) -> &[Particle] {
        self.particles.get_or_init(|| {
            make_particles(
                self.config.particle_count,
                self.config.seed,
                &self.config.colors,
            )
        })
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "confetti",
            name: "Confetti",
            summary: "A deterministic burst of drifting confetti pieces.",
            feature: "confetti",
        }
    }
}

impl Component for Confetti {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.start_frame + self.config.lifetime.max(1)
    }

    /// The library's dark house backdrop, in place of the source's own demo canvas.
    fn background(&self) -> Option<&'static str> {
        Some("#0d1117")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let unit = ctx.vmin() / REFERENCE_VMIN;
        let tt = ctx.frame() - config.start_frame as f32;
        let opacity = particle_opacity(tt, config.lifetime as f32);
        if opacity <= 0.0 {
            return Svgr::empty();
        }

        let origin = (
            ctx.width() * config.origin_x,
            ctx.height() * config.origin_y,
        );
        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for particle in self.particles() {
            let pose = particle_pose(particle, tt, origin, config.power, config.gravity, unit);
            let width = config.size * particle.size * unit;
            let height = width * config.aspect;
            let rx = config.corner_radius * unit;
            // The flutter squashes the piece horizontally; the rotation is about the piece's own
            // centre, which SVG needs as an explicit transform.
            let transform = format!(
                "rotate({:.3} {:.3} {:.3}) translate({:.3} {:.3}) scale({:.4} 1)",
                pose.rotate_deg,
                pose.x,
                pose.y,
                -width / 2.0,
                -height / 2.0,
                pose.flutter,
            );
            nodes.push(fframes::svgr!(
                <rect x="0" y="0" width={width} height={height} rx={rx}
                      fill={particle.color.clone()} opacity={opacity}
                      transform={transform} />
            ));
        }

        let nodes = Svgr::from(nodes);
        // Pieces are born inside the frame and leave it; the source has no clip, so neither does
        // the port, and the renderer simply drops what is off-canvas.
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors() -> Vec<String> {
        DEFAULT_COLORS.iter().map(|c| (*c).to_owned()).collect()
    }

    #[test]
    fn the_same_seed_builds_the_same_particles() {
        let a = make_particles(20, 7, &colors());
        let b = make_particles(20, 7, &colors());
        assert_eq!(a, b);
        let c = make_particles(20, 8, &colors());
        assert_ne!(a, c);
    }

    #[test]
    fn particles_stay_inside_their_documented_ranges() {
        for particle in make_particles(500, 3, &colors()) {
            assert!((0.35..=1.0).contains(&particle.speed), "{}", particle.speed);
            assert!((0.7..=1.3).contains(&particle.size), "{}", particle.size);
            assert!((-0.3..=0.3).contains(&particle.spin), "{}", particle.spin);
            assert!((-1.0..=1.0).contains(&particle.drift), "{}", particle.drift);
            assert!(colors().contains(&particle.color));
        }
    }

    #[test]
    fn an_empty_palette_falls_back_to_the_defaults() {
        let particles = make_particles(5, 1, &[]);
        assert!(
            particles
                .iter()
                .all(|p| DEFAULT_COLORS.contains(&p.color.as_str()))
        );
    }

    #[test]
    fn the_opacity_envelope_rises_and_falls() {
        assert!(particle_opacity(-1.0, 90.0).abs() < 1e-6);
        assert!(particle_opacity(91.0, 90.0).abs() < 1e-6);
        assert!(particle_opacity(0.0, 90.0).abs() < 1e-6);
        assert!((particle_opacity(4.0, 90.0) - 1.0).abs() < 1e-6);
        assert!((particle_opacity(45.0, 90.0) - 1.0).abs() < 1e-6);
        assert!(particle_opacity(90.0, 90.0).abs() < 1e-6);
        // The fade-out starts at 70% of the lifetime.
        assert!(particle_opacity(72.0, 90.0) < 1.0);
    }

    #[test]
    fn the_flight_follows_the_closed_form() {
        let config = ConfettiConfig::default();
        let particle = Particle {
            angle: 0.0,
            speed: 1.0,
            color: "#fff".to_owned(),
            size: 1.0,
            rot0: 0.0,
            spin: 0.1,
            drift: 0.0,
        };
        // Straight along +x: no gravity effect on x, and y only falls once gravity has acted.
        let pose = particle_pose(
            &particle,
            0.0,
            (100.0, 200.0),
            config.power,
            config.gravity,
            1.0,
        );
        assert!((pose.x - 100.0).abs() < 1e-6);
        assert!((pose.y - 200.0).abs() < 1e-6);

        let pose = particle_pose(
            &particle,
            2.0,
            (100.0, 200.0),
            config.power,
            config.gravity,
            1.0,
        );
        assert!((pose.x - (100.0 + 2.0 * config.power)).abs() < 1e-4);
        assert!((pose.y - (200.0 + 0.5 * config.gravity * 4.0)).abs() < 1e-4);
        // spin 0.1 rad/frame over 2 frames = 0.2 rad.
        assert!((pose.rotate_deg - 0.2_f32.to_degrees()).abs() < 1e-3);
    }

    #[test]
    fn duration_covers_the_burst() {
        let component = Confetti::default();
        assert_eq!(component.duration(), 90);
        assert_eq!(component.particles().len(), 140);
        assert_eq!(component.meta().id, "confetti");
    }
}
