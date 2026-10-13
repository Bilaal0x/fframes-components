//! # Echo Stack
//!
//! One word, blurred into focus, inside a vertical stack of outlined echoes that unfold away from
//! it and then drift through the frame forever.
//!
//! Three clocks run at once:
//!
//! | clock | window | what it does |
//! | --- | --- | --- |
//! | main | frames 0→14, `EASE_OUT_CUBIC` | the filled word blurs from 12 reference px to 0, fades in, scales `1.04 → 1` |
//! | unfold | frames 8→26, `EASE_OUT_CUBIC` | the echo spacing opens from `0.6` pitch to `1` pitch |
//! | fade in | frames 8→24, `EASE_OUT_CUBIC` | the echoes reach their full `echo_opacity` |
//!
//! After the unfold the stack keeps moving: the phase advances by one pitch every `cycle` frames
//! (upwards by default, downwards with `direction: Down`), and echoes recycle rows so the drift
//! never ends. Each echo's opacity also falls off with its distance from the centre — nothing
//! closer than `0.55` pitches is drawn at all, because it would be hidden behind the word.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Bold."` | The word to stack. |
//! | `font_size` | `f32` | `170.0` | Reference-canvas font size in px. |
//! | `font_weight` | `u16` | `800` | Font weight. |
//! | `color` | `String` | `"#e6edf3"` | The word's fill. |
//! | `echo_color` | `String` | `"#e6edf3"` | Echo outline colour. Empty means `color`. |
//! | `echoes` | `usize` | `3` | How many echo rows reach out from the centre. |
//! | `echo_opacity` | `f32` | `0.35` | Peak opacity of an echo row. |
//! | `stroke_width` | `f32` | `1.5` | Echo outline thickness in reference-canvas px. |
//! | `direction` | `EchoDirection` | `Up` | Which way the stack drifts. |
//! | `cycle` | `f32` | `60.0` | Frames per pitch of drift. |
//! | `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
//! | `tail` | `usize` | `45` | Frames held before the stack's own loop is cut off. |
//! | `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for the main word's gaussian blur. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components echo-stack -o out.mp4
//! ```
//!
//! ## Duration
//!
//! ```text
//! duration = ceil(max(unfold_end, fade_in_end, main_end) / speed) + tail
//! ```
//!
//! which is `26` frames of entrance plus the tail, so 71 frames, 2.37 s at 30 fps.
//!
//! ## Fidelity notes
//!
//! - **The outline.** The source builds the echo outline with an SVG filter — `feMorphology`
//!   eroding `SourceAlpha`, `feComposite` subtracting it, and `feFlood` recolouring the ring — so
//!   that the variable font's overlapping contours cannot show through a plain text stroke. This
//!   renderer does not execute SVG filters, so the port strokes the echo rows directly at
//!   `stroke_width`. A stroked glyph outline is the same ring where the contours do not overlap,
//!   which is the common case; where the source font has overlapping contours the port's ring may
//!   show a seam the source hides.
//! - **Blur.** The main word's `filter: blur(12px → 0)` is **approximated** with offset low-opacity
//!   copies.
//! - **Scaling.** The source is authored on a 1280x720 canvas, so every design px is multiplied by
//!   `ctx.vmin() / 7.2` (1.5 on the default 1920x1080).
//!
//! ## Credit
//!
//! Ported from `echo-stack/index.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::blur::{BLUR_GHOST_COPIES, BLUR_GHOST_SPREAD_RATIO, blur_ghost_alpha};
use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered};
use crate::motion::{
    REFERENCE_VMIN, clamp01, ease_out_cubic, interpolate, normalised_speed, smoothstep,
};

/// Distance between echo rows, in `em`.
pub const ECHO_STACK_PITCH: f32 = 1.05;

/// Echoes closer to the centre than this, in pitches, hide behind the word.
pub const HIDDEN: f32 = 0.55;

/// Frame window the main word resolves over.
pub const MAIN: [f32; 2] = [0.0, 14.0];

/// Frame window the stack unfolds over.
pub const UNFOLD: [f32; 2] = [8.0, 26.0];

/// Frame window the echoes fade in over.
pub const FADE_IN: [f32; 2] = [8.0, 24.0];

/// Pitch the stack starts at, as a share of the settled pitch.
pub const START_SPREAD: f32 = 0.6;

/// Softness of the main word at frame 0, in reference px.
pub const MAX_BLUR: f32 = 12.0;

/// Which way the echo stack drifts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EchoDirection {
    /// The stack drifts upwards.
    #[default]
    Up,
    /// The stack drifts downwards.
    Down,
}

impl EchoDirection {
    /// `-1` for up, `+1` for down — the sign the phase is multiplied by.
    pub fn sign(self) -> f32 {
        match self {
            Self::Up => -1.0,
            Self::Down => 1.0,
        }
    }
}

/// Configuration for [`EchoStack`].
#[derive(Debug, Clone, PartialEq)]
pub struct EchoStackConfig {
    /// The word to stack.
    pub text: String,
    /// Reference-canvas font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// The word's fill.
    pub color: String,
    /// Echo outline colour. Empty means `color`.
    pub echo_color: String,
    /// How many echo rows reach out from the centre.
    pub echoes: usize,
    /// Peak opacity of an echo row.
    pub echo_opacity: f32,
    /// Echo outline thickness in reference-canvas px.
    pub stroke_width: f32,
    /// Which way the stack drifts.
    pub direction: EchoDirection,
    /// Frames per pitch of drift.
    pub cycle: f32,
    /// Scales the timeline; `> 1` is faster.
    pub speed: f32,
    /// Frames held after the entrance, before the stack's own loop is cut off.
    pub tail: usize,
    /// Draw the secondary copies that stand in for the main word's gaussian blur.
    pub simulate_blur: bool,
}

impl Default for EchoStackConfig {
    fn default() -> Self {
        Self {
            text: "Bold.".to_owned(),
            font_size: 170.0,
            font_weight: 800,
            color: "#e6edf3".to_owned(),
            echo_color: String::new(),
            echoes: 3,
            echo_opacity: 0.35,
            stroke_width: 1.5,
            direction: EchoDirection::Up,
            cycle: 60.0,
            speed: 1.0,
            tail: 45,
            simulate_blur: true,
        }
    }
}

/// A usable cycle: non-finite or non-positive values fall back to `60`.
pub fn normalised_cycle(cycle: f32) -> f32 {
    if cycle.is_finite() && cycle > 0.0 {
        cycle
    } else {
        60.0
    }
}

/// The main word's channels at the component's frame clock.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MainChannels {
    /// Softness radius in canvas px.
    pub blur: f32,
    /// Opacity.
    pub opacity: f32,
    /// Scale, `1.04` at frame 0.
    pub scale: f32,
}

/// Sample the main word.
pub fn main_channels(frame: f32, config: &EchoStackConfig, unit: f32) -> MainChannels {
    let local = frame * normalised_speed(config.speed);
    let k = ease_out_cubic(clamp01(interpolate(local, MAIN, [0.0, 1.0])));
    MainChannels {
        blur: MAX_BLUR * (1.0 - k) * unit,
        opacity: k,
        scale: 1.04 - 0.04 * k,
    }
}

/// Echo visibility at `distance` pitches from the centre, before `echo_opacity`.
///
/// Nothing within [`HIDDEN`] pitches is visible; the falloff then ramps up to one pitch and decays
/// out to `echoes + 0.5` pitches.
pub fn echo_fade(distance: f32, echoes: usize) -> f32 {
    let d = distance.abs();
    if d < HIDDEN {
        return 0.0;
    }
    if d < 1.0 {
        return smoothstep((d - HIDDEN) / (1.0 - HIDDEN));
    }
    let reach = (echoes as f32 + 0.5).max(1.0);
    (1.0 - (d - 1.0) / (reach - 1.0)).max(0.0)
}

/// One echo row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EchoRow {
    /// Signed distance from the centre, in pitches, before the unfold.
    pub offset: f32,
    /// Row position in pitches after the unfold.
    pub position: f32,
    /// Row opacity, already multiplied by `echo_opacity`.
    pub opacity: f32,
}

/// Every echo row that the stack is drawing at the component's frame clock.
pub fn echo_rows(frame: f32, config: &EchoStackConfig) -> Vec<EchoRow> {
    let local = frame * normalised_speed(config.speed);
    let echoes = config.echoes;
    let cycle = normalised_cycle(config.cycle);
    let phase = config.direction.sign() * ((local / cycle) % 1.0);
    let spread = interpolate(
        ease_out_cubic(clamp01(interpolate(local, UNFOLD, [0.0, 1.0]))),
        [0.0, 1.0],
        [START_SPREAD, 1.0],
    );
    let fade_in = ease_out_cubic(clamp01(interpolate(local, FADE_IN, [0.0, 1.0])));

    let mut rows = Vec::new();
    let reach = echoes as i64 + 2;
    for k in -reach..=reach {
        let offset = k as f32 + phase;
        let opacity = config.echo_opacity * fade_in * echo_fade(offset, echoes);
        if opacity <= 0.0 {
            continue;
        }
        rows.push(EchoRow {
            offset,
            position: offset * spread,
            opacity,
        });
    }
    rows
}

/// The Echo Stack component.
#[derive(Debug, Clone, Default)]
pub struct EchoStack {
    /// Component options.
    pub config: EchoStackConfig,
}

impl EchoStack {
    /// Build with the given options.
    pub fn new(config: EchoStackConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "echo-stack",
            name: "Echo Stack",
            summary: "A word resolves inside a drifting stack of outlined echoes.",
            feature: "echo-stack",
        }
    }
}

impl Component for EchoStack {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        let end = UNFOLD[1].max(FADE_IN[1]).max(MAIN[1]) / normalised_speed(self.config.speed);
        end.ceil().max(0.0) as usize + self.config.tail
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
        let main = main_channels(ctx.frame(), config, unit);
        let rows = echo_rows(ctx.frame(), config);
        let pitch = ECHO_STACK_PITCH * size;
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let baseline = center_y + ctx.baseline_offset(&family, size, size);

        let echo_fill = if config.echo_color.is_empty() {
            config.color.clone()
        } else {
            config.echo_color.clone()
        };
        // A text node with no fill and a stroke is the outline; the source's filter produced the
        // same ring from the filled silhouette.
        let stroke = config.stroke_width * unit;

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for row in rows {
            nodes.push(fframes::svgr!(
                <text x={center_x} y={baseline + row.position * pitch} font-family={family.clone()}
                      font-size={size} font-weight={config.font_weight} fill="none"
                      stroke={echo_fill.clone()} stroke-width={stroke} opacity={row.opacity}
                      text-anchor="middle">{config.text.clone()}</text>
            ));
        }

        // The main word, at its own scale and softness.
        let main_size = size * main.scale;
        let main_baseline = center_y + ctx.baseline_offset(&family, main_size, main_size);
        if config.simulate_blur && main.blur > 0.05 {
            let alpha = blur_ghost_alpha(main.blur, unit);
            let spread = main.blur * BLUR_GHOST_SPREAD_RATIO;
            for copy in 1..=BLUR_GHOST_COPIES {
                let sign = if copy % 2 == 0 { -1.0 } else { 1.0 };
                nodes.push(text_node_centered(
                    &config.text,
                    center_x,
                    main_baseline + spread * (copy as f32 / BLUR_GHOST_COPIES as f32) * sign,
                    family.clone(),
                    main_size,
                    config.font_weight,
                    0.0,
                    config.color.clone(),
                    main.opacity * alpha,
                ));
            }
        }
        nodes.push(text_node_centered(
            &config.text,
            center_x,
            main_baseline,
            family,
            main_size,
            config.font_weight,
            0.0,
            config.color.clone(),
            main.opacity,
        ));

        let nodes = Svgr::from(nodes);
        // The stack drifts past the frame edges by design; the source crops it with
        // `overflow: hidden`, so the port clips to the canvas it renders in.
        let clip_id = "echo-stack-clip".to_owned();
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id={clip_id.clone()}>
                        <rect x="0" y="0" width={ctx.width()} height={ctx.height()} />
                    </clipPath>
                </defs>
                <g clip-path={format!("url(#{clip_id})")}>{nodes}</g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_word_resolves_over_its_window() {
        let config = EchoStackConfig::default();
        let start = main_channels(0.0, &config, 1.0);
        assert!(start.opacity.abs() < 1e-6);
        assert!((start.blur - MAX_BLUR).abs() < 1e-6);
        assert!((start.scale - 1.04).abs() < 1e-6);

        let end = main_channels(14.0, &config, 1.0);
        assert!((end.opacity - 1.0).abs() < 1e-6);
        assert!(end.blur.abs() < 1e-6);
        assert!((end.scale - 1.0).abs() < 1e-6);
    }

    #[test]
    fn nothing_is_drawn_inside_the_hidden_band() {
        for distance in [-0.4_f32, -0.1, 0.0, 0.3, 0.54] {
            assert!(echo_fade(distance, 3).abs() < 1e-6, "drew at {distance}");
        }
        assert!(echo_fade(1.0, 3) > 0.0);
    }

    #[test]
    fn echoes_fade_out_beyond_their_reach() {
        let near = echo_fade(1.0, 3);
        let far = echo_fade(2.0, 3);
        assert!(near > far, "{near} should exceed {far}");
        assert!(echo_fade(4.0, 3).abs() < 1e-6);
    }

    #[test]
    fn the_stack_starts_folded_and_opens() {
        let config = EchoStackConfig::default();
        // Frame 0: the fade-in has not started, so no echo is visible and no row is returned.
        assert!(
            echo_rows(0.0, &config).is_empty(),
            "no echo may be visible before the fade-in"
        );

        // Mid-unfold the rows are still compressed; by frame 30 they sit a full pitch apart.
        let mid = echo_rows(17.0, &config);
        assert!(!mid.is_empty());
        for row in &mid {
            assert!(
                row.position.abs() < row.offset.abs(),
                "row {row:?} should not have fully unfolded yet"
            );
        }

        let open = echo_rows(30.0, &config);
        assert!(!open.is_empty());
        for row in &open {
            assert!(
                (row.position - row.offset).abs() < 1e-5,
                "row {row:?} did not unfold"
            );
            assert!(row.opacity > 0.0);
        }
    }

    #[test]
    fn the_phase_drifts_and_the_direction_flips_it() {
        let up = EchoStackConfig::default();
        let down = EchoStackConfig {
            direction: EchoDirection::Down,
            ..EchoStackConfig::default()
        };
        // A quarter of the cycle in, the stack has moved a quarter of a pitch.
        let up_rows = echo_rows(15.0, &up);
        let down_rows = echo_rows(15.0, &down);
        assert!(!up_rows.is_empty() && !down_rows.is_empty());
        let up_phase = up_rows[0].offset - up_rows[0].offset.round();
        let down_phase = down_rows[0].offset - down_rows[0].offset.round();
        assert!(
            (up_phase + down_phase).abs() < 1e-5,
            "{up_phase} vs {down_phase}"
        );
    }

    #[test]
    fn a_useless_cycle_falls_back() {
        assert!((normalised_cycle(0.0) - 60.0).abs() < 1e-6);
        assert!((normalised_cycle(f32::NAN) - 60.0).abs() < 1e-6);
        assert!((normalised_cycle(12.0) - 12.0).abs() < 1e-6);
    }

    #[test]
    fn duration_matches_the_documented_formula() {
        let component = EchoStack::default();
        assert_eq!(component.duration(), 71);
        assert_eq!(component.meta().id, "echo-stack");
    }
}
