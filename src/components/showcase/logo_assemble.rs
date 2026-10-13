//! A ring of image cards collapses into a logo and brand name.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, normalised_speed};
use crate::ui::{Mode, UiTheme, blur, mix_oklch, text, to_hex};

// ─── Timeline (frames @ 30fps; the source's own comments) ────────────────────────────────────

/// Frame the ring starts draining inward.
pub const CONTRACT_START: f32 = 50.0;
/// Frame the ring has fully drained to the centre.
pub const CONTRACT_END: f32 = 66.0;
/// Frame the middle line has finished fading out.
pub const MIDDLE_OUT: f32 = 52.0;
/// Frame the mark starts to appear at the centre.
pub const LOGO_IN: f32 = 58.0;
/// Frame the mark starts sliding from the centre toward its left resting spot.
pub const LOGO_SHIFT_START: f32 = 74.0;
/// Frame the mark has reached that spot — and the last frame on which anything moves.
pub const LOGO_SHIFT_END: f32 = 90.0;
/// Frame the brand name starts to reveal. Equal to [`LOGO_SHIFT_START`]: the mark moving and the
/// name arriving happen simultaneously, not one after the other.
pub const NAME_IN: f32 = LOGO_SHIFT_START;

/// Steady angular speed of the ring, in radians per frame.
pub const ORBIT_SPEED: f32 = 0.1;
/// Extra spin added as the ring drains inward — the water-down-the-plughole.
pub const DRAIN_SPIN: f32 = std::f32::consts::PI * 1.1;
/// Card height as a multiple of its width: portrait cards.
pub const CARD_RATIO: f32 = 1.22;

/// The mark's slide and the name's reveal: `cubic-bezier(0.2, 0.6, 0.35, 1)`.
pub const EASE: CubicBezier = CubicBezier::new(0.2, 0.6, 0.35, 1.0);
/// The collapse: `cubic-bezier(0.5, 0, 0.75, 0.4)` — accelerate into the centre.
pub const EASE_IN: CubicBezier = CubicBezier::new(0.5, 0.0, 0.75, 0.4);

// ─── Scene constants (the source's own numbers) ──────────────────────────────────────────────

/// Orbit radius as a fraction of the canvas's short side.
const ORBIT_RADIUS_FRACTION: f32 = 0.36;
/// Card width as a fraction of the short side.
const CARD_W_FRACTION: f32 = 0.15;
/// Card corner radius, in reference-canvas px (`borderRadius: 4`).
const CARD_RADIUS: f32 = 4.0;
/// The mark's box as a fraction of the short side.
const LOGO_FRACTION: f32 = 0.125;
/// The gap between the settled mark and the brand name, as a fraction of the short side.
const GAP_FRACTION: f32 = 0.035;
/// How far left of centre the mark settles, as a fraction of the short side.
const REST_FRACTION: f32 = 0.13;
/// Brand-name font size as a fraction of the short side.
const NAME_FRACTION: f32 = 0.055;
/// Middle-line font size as a fraction of the short side.
const MIDDLE_FRACTION: f32 = 0.04;
/// The middle line's `line-height`.
const MIDDLE_LINE_HEIGHT: f32 = 1.2;
/// Letter spacing of both text runs, in em (`-0.02em`).
const TRACKING_EM: f32 = -0.02;
/// The middle line's weight.
const MIDDLE_WEIGHT: u16 = 600;
/// The brand name's weight.
const NAME_WEIGHT: u16 = 600;
/// The monogram's weight.
const LOGO_WEIGHT: u16 = 700;
/// The monogram's font size as a multiple of the mark's box.
///
/// `1.0` sets the letter at the box's own size, which lands its cap height (≈`0.727 em`) on the
/// ink height of the source's own mark — measured at 49 demo px against the reference's 50 — so
/// the reveal reads at the weight the source intended even though the glyph is a stand-in.
const LOGO_LETTER_RATIO: f32 = 1.0;

/// The card shadow's downward offset, in reference-canvas px (`0 14px 34px`).
const SHADOW_DY: f32 = 14.0;
/// The card shadow's blur radius, in reference-canvas px.
const SHADOW_BLUR: f32 = 34.0;
/// The card shadow's own alpha (`withAlpha(..., 0.4)`).
const SHADOW_ALPHA: f32 = 0.4;

/// The original abstract placeholder images embedded in `media/`, in ring order.
///
/// Local media names keep the default render deterministic, offline, and free of third-party
/// image licensing requirements.
pub const DEFAULT_IMAGES: [&str; 8] = [
    "placeholder-01.png",
    "placeholder-02.png",
    "placeholder-03.png",
    "placeholder-04.png",
    "placeholder-05.png",
    "placeholder-06.png",
    "placeholder-07.png",
    "placeholder-08.png",
];

/// One intermediate stop list, interpolated exactly as Remotion's `interpolate` does.
///
/// The detail that matters is that Remotion applies `easing` **per segment**, not once across the
/// whole range: `logoPose.scale` runs `0.6 → 1.05 → 1`, and the same curve is restarted at the
/// `1.05` keyframe, which is what makes the pop read as a pop. Its input is clamped to the
/// segment at both ends, and a segment whose two outputs are equal returns the first without
/// consulting the curve.
fn interpolate_stops(
    frame: f32,
    inputs: &[f32],
    outputs: &[f32],
    ease: Option<CubicBezier>,
) -> f32 {
    if inputs.len() != outputs.len() || inputs.len() < 2 {
        return outputs.first().copied().unwrap_or(0.0);
    }
    // Remotion's `findRange`: the segment is the first whose upper stop is >= the input, and the
    // search never runs past the second-to-last pair.
    let mut segment = inputs.len() - 2;
    let inner_stops = inputs.len().saturating_sub(1);
    for (index, stop) in inputs.iter().enumerate().take(inner_stops).skip(1) {
        if *stop >= frame {
            segment = index - 1;
            break;
        }
    }
    let (low, high) = (inputs[segment], inputs[segment + 1]);
    let (from, to) = (outputs[segment], outputs[segment + 1]);
    if (to - from).abs() < f32::EPSILON {
        return from;
    }
    let clamped = frame.clamp(low.min(high), low.max(high));
    let span = high - low;
    let progress = if span.abs() < f32::EPSILON {
        0.0
    } else {
        (clamped - low) / span
    };
    let eased = ease.map_or(progress, |curve| curve.ease(progress));
    from + (to - from) * eased
}

/// `0` while orbiting, `1` when fully drained to the centre.
#[must_use]
pub fn contract_progress(frame: f32) -> f32 {
    interpolate_stops(
        frame,
        &[CONTRACT_START, CONTRACT_END],
        &[0.0, 1.0],
        Some(EASE_IN),
    )
}

/// The ring's rotation in radians: steady while orbiting, then a burst of extra spin as it drains.
#[must_use]
pub fn orbit_angle(frame: f32) -> f32 {
    frame * ORBIT_SPEED + contract_progress(frame) * DRAIN_SPIN
}

/// Radius multiplier: `1` while orbiting, `0` once the ring has drained in.
#[must_use]
pub fn ring_scale(frame: f32) -> f32 {
    1.0 - contract_progress(frame)
}

/// Card scale: full size on the ring, shrinking as it pulls to the centre.
#[must_use]
pub fn card_scale(frame: f32) -> f32 {
    1.0 - 0.66 * contract_progress(frame)
}

/// Card opacity: solid on the ring, vanishing over the last eight frames of the drain.
#[must_use]
pub fn card_opacity(frame: f32) -> f32 {
    interpolate_stops(
        frame,
        &[CONTRACT_END - 8.0, CONTRACT_END],
        &[1.0, 0.0],
        None,
    )
}

/// Middle line opacity: fades in early, holds through the orbit, out as the ring drains.
#[must_use]
pub fn middle_text_opacity(frame: f32) -> f32 {
    interpolate_stops(
        frame,
        &[6.0, 16.0, MIDDLE_OUT - 6.0, MIDDLE_OUT],
        &[0.0, 1.0, 1.0, 0.0],
        None,
    )
}

/// The mark's opacity and scale at a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogoPose {
    /// Opacity, `0..=1`.
    pub opacity: f32,
    /// Scale about the mark's own centre.
    pub scale: f32,
}

/// The mark is born at the centre with a small overshoot as the ring vanishes.
#[must_use]
pub fn logo_pose(frame: f32) -> LogoPose {
    LogoPose {
        opacity: interpolate_stops(frame, &[LOGO_IN, LOGO_IN + 8.0], &[0.0, 1.0], None),
        scale: interpolate_stops(
            frame,
            &[LOGO_IN, LOGO_IN + 10.0, LOGO_IN + 18.0],
            &[0.6, 1.05, 1.0],
            Some(EASE),
        ),
    }
}

/// How far the mark has slid from the centre toward its left resting spot (`0 → 1`).
#[must_use]
pub fn logo_shift(frame: f32) -> f32 {
    interpolate_stops(
        frame,
        &[LOGO_SHIFT_START, LOGO_SHIFT_END],
        &[0.0, 1.0],
        Some(EASE),
    )
}

/// The brand name's opacity and horizontal offset at a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamePose {
    /// Opacity, `0..=1`.
    pub opacity: f32,
    /// Horizontal offset in reference-canvas px, sliding to `0`.
    pub dx: f32,
}

/// The brand name: fades and slides in from the right, in step with the mark's slide.
#[must_use]
pub fn name_pose(frame: f32) -> NamePose {
    NamePose {
        opacity: interpolate_stops(frame, &[NAME_IN, NAME_IN + 10.0], &[0.0, 1.0], None),
        dx: interpolate_stops(frame, &[NAME_IN, NAME_IN + 12.0], &[18.0, 0.0], Some(EASE)),
    }
}

/// The last frame on which anything in the scene moves.
///
/// The slide is the final channel to come to rest, so this is [`LOGO_SHIFT_END`], which is what
/// `registry/__measured__.json` records as this component's `settle.base`.
#[must_use]
pub fn settle_frame() -> f32 {
    LOGO_SHIFT_END
}

/// The scene's fixed geometry, in the reference canvas's own pixels (its short side is 720).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogoGeometry {
    /// Orbit radius before the contract.
    pub radius: f32,
    /// Card width.
    pub card_w: f32,
    /// Card height.
    pub card_h: f32,
    /// The mark's box, square.
    pub logo_size: f32,
    /// Gap between the settled mark and the brand name.
    pub gap: f32,
    /// How far left of centre the mark settles.
    pub rest_x: f32,
    /// Brand-name font size.
    pub name_size: f32,
    /// Middle-line font size.
    pub middle_size: f32,
}

impl LogoGeometry {
    /// The reference canvas's short side is 720.
    pub const SHORT: f32 = 720.0;

    /// Every length the source derives from `Math.min(width, height)`, at the reference size.
    #[must_use]
    pub fn reference() -> Self {
        Self::for_short(Self::SHORT)
    }

    /// The same geometry at a `short` of some other size.
    ///
    /// The source rounds `cardW`, `logoSize`, `gap`, `nameSize` and the middle size at its own
    /// canvas size, so these are rounded here before they are scaled to the render canvas — at
    /// 1080p the scale is exactly `1.5`, and rounding after scaling would move a value by half a
    /// reference pixel.
    #[must_use]
    pub fn for_short(short: f32) -> Self {
        let card_w = (short * CARD_W_FRACTION).round();
        Self {
            radius: short * ORBIT_RADIUS_FRACTION,
            card_w,
            card_h: card_w * CARD_RATIO,
            logo_size: (short * LOGO_FRACTION).round(),
            gap: (short * GAP_FRACTION).round(),
            rest_x: short * REST_FRACTION,
            name_size: (short * NAME_FRACTION).round(),
            middle_size: (short * MIDDLE_FRACTION).round(),
        }
    }
}

/// How many cards ride the ring: `Math.max(1, Math.floor(count))`.
///
/// A non-finite count falls back to one card rather than to a ring of `NaN` positions.
#[must_use]
pub fn card_count(count: f32) -> usize {
    if !count.is_finite() {
        return 1;
    }
    let floored = count.floor();
    if floored < 1.0 { 1 } else { floored as usize }
}

/// One card's offset from the canvas centre, before the canvas scale, for a ring of `count` cards.
///
/// `radius` is the orbit radius **already multiplied by [`ring_scale`]**, which is how the source
/// drains the ring: the card keeps its place on the circle while the circle shrinks.
#[must_use]
pub fn orbit_offset(index: usize, count: usize, angle: f32, radius: f32) -> (f32, f32) {
    if count == 0 {
        return (0.0, 0.0);
    }
    let theta = index as f32 / count as f32 * std::f32::consts::TAU + angle;
    (theta.cos() * radius, theta.sin() * radius)
}

/// The middle line's own lines: split on newlines, each trimmed, blanks dropped.
///
/// The source's middle box is `white-space: pre-line`, which breaks on newlines and collapses the
/// surrounding white space.
#[must_use]
pub fn middle_lines(text: &str) -> Vec<String> {
    text.split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The monogram the port draws in place of the source's `logoSrc`: the brand name's initial.
#[must_use]
pub fn monogram(name: &str) -> String {
    name.chars()
        .next()
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default()
}

/// A card's backing fill, visible while its photograph is unresolved.
///
/// The ring cycles `images`, so the fallback tint follows the image's own index.
pub fn card_tint(theme: &UiTheme, image_index: usize, image_count: usize) -> String {
    let t = if image_count == 0 {
        0.0
    } else {
        (image_index as f32 + 1.0) / (image_count as f32 + 1.0)
    };
    to_hex(mix_oklch(&theme.secondary, &theme.primary, t))
}

/// The card shadow's `box-shadow: 0 14px 34px` as a ring of offset copies.
///
/// Returns the offsets — relative to the card's own centre, in canvas px, and already carrying the
/// source's downward offset and the card's own scale, because a CSS transform scales the painted
/// shadow with the box — and the opacity of each copy. The copies sit on the same ring the text
/// blur uses, so the two stand-ins read alike. It is not a Gaussian falloff: the total ink is the
/// source's own `0.4`, but it is spread over [`blur::COPIES`] hard copies.
#[must_use]
pub fn shadow_copies(unit: f32, scale: f32) -> ([(f32, f32); blur::COPIES], f32) {
    let ring = blur::ghost_offsets(SHADOW_BLUR, unit);
    let dy = SHADOW_DY * unit * scale;
    let offsets = std::array::from_fn(|index| {
        let (x, y) = ring[index];
        (x * scale, dy + y * scale)
    });
    (offsets, SHADOW_ALPHA / blur::COPIES as f32)
}

/// Configuration for [`LogoAssemble`].
#[derive(Debug, Clone, PartialEq)]
pub struct LogoAssembleConfig {
    /// The brand name revealed to the right of the settled mark.
    pub brand_name: String,
    /// The line held in the middle of the ring while the cards revolve. `\n` splits lines.
    pub middle_text: String,
    /// The mark's asset. Recorded for parity with the source; an empty value draws no mark.
    pub logo_src: String,
    /// The ring's media names, cycled around the circle and resolved by the host media provider.
    pub images: Vec<String>,
    /// How many cards ride the ring.
    pub count: f32,
    /// The stage colour. `None` uses the theme's `background`.
    pub background: Option<String>,
    /// Timeline multiplier. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// The face to paint in. `None` keeps the theme's face, or the crate's embedded face.
    pub font_family: Option<String>,
    /// Composition length in frames.
    pub duration: usize,
    /// Whether to draw the card-shadow stand-in.
    pub simulate_shadow: bool,
}

impl Default for LogoAssembleConfig {
    fn default() -> Self {
        Self {
            brand_name: "fframes".to_owned(),
            middle_text: "Cinematic components for React".to_owned(),
            logo_src: "/logo/fframes-white.png".to_owned(),
            images: DEFAULT_IMAGES.iter().map(|url| (*url).to_owned()).collect(),
            count: 10.0,
            background: Some("#050505".to_owned()),
            speed: 1.0,
            // The source's own prop default: the shipped mark is white, so the stage is dark.
            mode: Mode::Dark,
            theme: None,
            font_family: None,
            duration: 108,
            simulate_shadow: true,
        }
    }
}

/// The **Logo Assemble** component.
#[derive(Debug, Clone, Default)]
pub struct LogoAssemble {
    /// Component options.
    pub config: LogoAssembleConfig,
}

impl LogoAssemble {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: LogoAssembleConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "logo-assemble",
            name: "Logo Assemble",
            summary: "A ring of cards drains into the centre and gives birth to a logo and brand name",
            feature: "logo-assemble",
        }
    }
}

impl Component for LogoAssemble {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // `config.ts`: `previewBackdrop: { type: "color", value: "#050505" }`.
        Some("#050505")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let unit = text::unit(ctx);
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let ink = theme.foreground.clone();
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);

        // The source's root element paints `background ?? theme.background`. `background()`
        // reports the shipped `previewBackdrop`, so an override is painted here instead.
        let stage = config
            .background
            .clone()
            .unwrap_or_else(|| theme.background.clone());

        let geometry = LogoGeometry::reference();
        let frame = ctx.frame() * normalised_speed(config.speed);
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;

        let angle = orbit_angle(frame);
        let ring = ring_scale(frame);
        let scale = card_scale(frame);
        let cards = card_opacity(frame);
        let middle = middle_text_opacity(frame);
        let logo = logo_pose(frame);
        let shift = logo_shift(frame);
        let name = name_pose(frame);

        let count = card_count(config.count);
        let mut defs: Vec<Svgr<'static>> = Vec::with_capacity(count);
        let mut nodes: Vec<Svgr<'static>> =
            Vec::with_capacity(blur::COPIES * count + 2 * count + 6);

        nodes.push(fframes::svgr!(
            <rect x="0" y="0" width={ctx.width()} height={ctx.height()} fill={stage.clone()} />
        ));

        if cards > 0.0 {
            let image_count = config.images.len();
            let shadow = to_hex(mix_oklch(&stage, "#000000", 0.75));
            let (offsets, copy_alpha) = shadow_copies(unit, scale);

            for index in 0..count {
                let (dx, dy) = orbit_offset(index, count, angle, geometry.radius * ring);
                let card_x = center_x + dx * unit;
                let card_y = center_y + dy * unit;
                let width = geometry.card_w * unit * scale;
                let height = geometry.card_h * unit * scale;
                let radius = CARD_RADIUS * unit * scale;

                // The shadow, behind the card and scaled with it, as the source's own box-shadow.
                if config.simulate_shadow {
                    for (offset_x, offset_y) in offsets {
                        nodes.push(fframes::svgr!(
                            <rect x={card_x - width / 2.0 + offset_x}
                                  y={card_y - height / 2.0 + offset_y}
                                  width={width} height={height} rx={radius}
                                  fill={shadow.clone()} opacity={copy_alpha * cards} />
                        ));
                    }
                }

                let tint = card_tint(&theme, index % image_count.max(1), image_count);
                let x = card_x - width / 2.0;
                let y = card_y - height / 2.0;
                nodes.push(fframes::svgr!(
                    <rect x={x} y={y} width={width} height={height} rx={radius}
                          fill={tint} opacity={cards} />
                ));
                if let Some(src) = config.images.get(index % image_count.max(1)) {
                    let clip_id = format!("logo-assemble-card-{index}");
                    defs.push(fframes::svgr!(
                        <clipPath id={clip_id.clone()}>
                            <rect x={x} y={y} width={width} height={height} rx={radius} />
                        </clipPath>
                    ));
                    nodes.push(fframes::svgr!(
                        <image href={src.clone()} x={x} y={y} width={width} height={height}
                               preserveAspectRatio="xMidYMid slice"
                               clip-path={format!("url(#{clip_id})")} opacity={cards} />
                    ));
                }
            }
        }

        // The middle line, held in the ring's clear centre.
        if middle > 0.0 && !config.middle_text.is_empty() {
            let size = geometry.middle_size * unit;
            let step = size * MIDDLE_LINE_HEIGHT;
            let tracking = text::tracking_px_from_em(TRACKING_EM, size);
            let lines = middle_lines(&config.middle_text);
            let line_count = lines.len() as f32;
            for (index, line) in lines.iter().enumerate() {
                // CSS centres the box of lines on the ring's centre, so each line's own box centre
                // is one `line-height` step from the last.
                let line_center = center_y + (index as f32 - (line_count - 1.0) / 2.0) * step;
                let baseline = text::baseline_for_centred_box(ctx, &family, size, line_center);
                nodes.push(text::text_node(
                    ctx,
                    line,
                    center_x,
                    baseline,
                    family.clone(),
                    size,
                    MIDDLE_WEIGHT,
                    tracking,
                    ink.clone(),
                    middle,
                ));
            }
        }

        // The mark: born at the centre, then sliding left.
        if logo.opacity > 0.0 && !config.logo_src.is_empty() {
            let letter = monogram(&config.brand_name);
            if !letter.is_empty() {
                let mark_x = center_x - geometry.rest_x * unit * shift;
                let size = geometry.logo_size * unit * logo.scale * LOGO_LETTER_RATIO;
                let baseline = text::baseline_for_centred_box(ctx, &family, size, center_y);
                nodes.push(text::text_node(
                    ctx,
                    &letter,
                    mark_x,
                    baseline,
                    family.clone(),
                    size,
                    LOGO_WEIGHT,
                    0.0,
                    ink.clone(),
                    logo.opacity,
                ));
            }
        }

        // The brand name: revealed to the right of the settled mark, positioned by its left edge.
        if name.opacity > 0.0 && !config.brand_name.is_empty() {
            let size = geometry.name_size * unit;
            let left = center_x
                + (-geometry.rest_x + geometry.logo_size / 2.0 + geometry.gap) * unit
                + name.dx * unit;
            let baseline = text::baseline_for_centred_box(ctx, &family, size, center_y);
            let tracking = text::tracking_px_from_em(TRACKING_EM, size);
            nodes.push(text::text_node_left(
                &config.brand_name,
                left,
                baseline,
                family.clone(),
                size,
                NAME_WEIGHT,
                tracking,
                ink,
                name.opacity,
            ));
        }

        let defs = Svgr::from(defs);
        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g><defs>{defs}</defs>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The middle of segment `index` of a stop list, as a frame.
    fn segment_midpoint(inputs: &[f32], index: usize) -> f32 {
        (inputs[index] + inputs[index + 1]) / 2.0
    }

    #[test]
    fn duration_defaults_to_the_source_composition_length() {
        let component = LogoAssemble::default();
        assert_eq!(component.duration(), 108);
        assert_eq!(component.meta(), LogoAssemble::meta_static());
        assert_eq!(component.meta().id, "logo-assemble");
        assert_eq!(component.meta().feature, "logo-assemble");
        assert_eq!(component.background(), Some("#050505"));
    }

    #[test]
    fn the_shipped_defaults_use_embedded_placeholder_images() {
        let config = LogoAssembleConfig::default();

        assert_eq!(config.brand_name, "fframes");
        assert_eq!(config.middle_text, "Cinematic components for React");
        assert_eq!(config.logo_src, "/logo/fframes-white.png");
        assert!((config.count - 10.0).abs() < 1e-6);
        assert_eq!(config.background.as_deref(), Some("#050505"));
        assert_eq!(config.font_family, None);
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.mode, Mode::Dark);
        assert_eq!(config.theme, None);
        assert_eq!(config.duration, 108);
        assert!(config.simulate_shadow);
        assert_eq!(
            config.images,
            DEFAULT_IMAGES
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>()
        );

        let media = crate::ComponentsMedia::new().expect("embedded media");
        let embedded = [
            media.placeholder_01_png,
            media.placeholder_02_png,
            media.placeholder_03_png,
            media.placeholder_04_png,
            media.placeholder_05_png,
            media.placeholder_06_png,
            media.placeholder_07_png,
            media.placeholder_08_png,
        ];
        for image in embedded {
            assert_eq!(image.metadata.width, 1200);
            assert_eq!(image.metadata.height, 1200);
        }
    }

    #[test]
    fn contract_progress_drains_across_the_contract_window() {
        // The source's own test: clamped at both ends of the window.
        assert!(contract_progress(CONTRACT_START).abs() < 1e-6);
        assert!((contract_progress(CONTRACT_END) - 1.0).abs() < 1e-6);
        assert!(contract_progress(0.0).abs() < 1e-6);
        assert!((contract_progress(90.0) - 1.0).abs() < 1e-6);

        // Eased *in*, so the collapse starts slow and is still short of the linear midpoint at
        // the halfway frame — it accelerates into the centre.
        let half = contract_progress(58.0);
        assert!(half < 0.5, "expected an accelerate-in curve, got {half}");
        assert!(half > 0.0);
        // And strictly increasing across the window.
        let mut previous = -1.0;
        for frame in 50..=66 {
            let value = contract_progress(frame as f32);
            assert!(value >= previous, "progress fell at frame {frame}");
            previous = value;
        }
    }

    #[test]
    fn orbit_angle_is_steady_then_spins_up_in_the_drain() {
        assert!(orbit_angle(0.0).abs() < 1e-6);

        // Before the drain, motion is purely the steady orbit.
        let a10 = orbit_angle(10.0);
        let a20 = orbit_angle(20.0);
        assert!((a20 - a10 - (a10 - orbit_angle(0.0))).abs() < 1e-4);

        // The drain adds extra spin on top of the steady rate.
        let added = orbit_angle(CONTRACT_END) - orbit_angle(CONTRACT_START);
        assert!(
            added > (CONTRACT_END - CONTRACT_START) * ORBIT_SPEED + DRAIN_SPIN * 0.99,
            "the drain should add {DRAIN_SPIN} rad, added {added}"
        );
    }

    #[test]
    fn the_ring_and_its_cards_shrink_then_vanish() {
        assert!((ring_scale(CONTRACT_START) - 1.0).abs() < 1e-6);
        assert!(ring_scale(CONTRACT_END).abs() < 1e-6);
        assert!(ring_scale(58.0) < 1.0 && ring_scale(58.0) > 0.0);

        assert!((card_scale(CONTRACT_START) - 1.0).abs() < 1e-6);
        assert!((card_scale(CONTRACT_END) - 0.34).abs() < 1e-6);

        // Solid through the orbit and most of the drain, gone at the end of it.
        assert!((card_opacity(0.0) - 1.0).abs() < 1e-6);
        assert!((card_opacity(CONTRACT_START) - 1.0).abs() < 1e-6);
        assert!((card_opacity(CONTRACT_END - 8.0) - 1.0).abs() < 1e-6);
        assert!((card_opacity(CONTRACT_END) - 0.0).abs() < 1e-6);
        assert!(card_opacity(62.0) > 0.0 && card_opacity(62.0) < 1.0);
    }

    #[test]
    fn the_middle_line_holds_through_the_orbit_then_clears() {
        assert!(middle_text_opacity(0.0).abs() < 1e-6);
        assert!(middle_text_opacity(6.0).abs() < 1e-6);
        assert!((middle_text_opacity(16.0) - 1.0).abs() < 1e-6);
        assert!((middle_text_opacity(20.0) - 1.0).abs() < 1e-6);
        assert!((middle_text_opacity(MIDDLE_OUT - 6.0) - 1.0).abs() < 1e-6);
        assert!(middle_text_opacity(MIDDLE_OUT).abs() < 1e-6);
        // The fade out runs over the six frames before `MIDDLE_OUT`, and is linear.
        let mid = (MIDDLE_OUT - 6.0 + MIDDLE_OUT) / 2.0;
        assert!((middle_text_opacity(mid) - 0.5).abs() < 1e-5);
        // It is fully gone before the mark arrives.
        assert!(middle_text_opacity(LOGO_IN).abs() < 1e-6);
    }

    #[test]
    fn the_mark_is_born_small_and_settles_with_an_overshoot() {
        assert!(logo_pose(LOGO_IN - 1.0).opacity.abs() < 1e-6);
        assert!(logo_pose(LOGO_IN).opacity.abs() < 1e-6);
        assert!((logo_pose(LOGO_IN + 8.0).opacity - 1.0).abs() < 1e-6);
        assert!((logo_pose(100.0).opacity - 1.0).abs() < 1e-6);

        // Born below 1, past 1 at the pop's keyframe, and at rest after the settle.
        assert!((logo_pose(LOGO_IN).scale - 0.6).abs() < 1e-6);
        assert!(logo_pose(LOGO_IN).scale < 1.0);
        assert!((logo_pose(LOGO_IN + 10.0).scale - 1.05).abs() < 1e-6);
        assert!((logo_pose(LOGO_IN + 18.0).scale - 1.0).abs() < 1e-6);
        assert!((logo_pose(LOGO_IN + 18.0).scale - logo_pose(108.0).scale).abs() < 1e-6);
        // The overshoot is above 1 somewhere between the keyframes.
        assert!(logo_pose(LOGO_IN + 14.0).scale > 1.0);
        assert!(logo_pose(LOGO_IN + 6.0).scale < 1.05 && logo_pose(LOGO_IN + 6.0).scale > 0.6);
    }

    #[test]
    fn the_mark_slides_from_the_centre_to_its_rest() {
        assert!(logo_shift(LOGO_IN).abs() < 1e-6);
        assert!(logo_shift(LOGO_SHIFT_START).abs() < 1e-6);
        assert!((logo_shift(LOGO_SHIFT_END) - 1.0).abs() < 1e-6);
        assert!((logo_shift(108.0) - 1.0).abs() < 1e-6);

        let mid = logo_shift(segment_midpoint(&[LOGO_SHIFT_START, LOGO_SHIFT_END], 0));
        assert!(mid > 0.0 && mid < 1.0, "mid slide {mid}");
        // Eased, so past the linear midpoint halfway through.
        assert!(mid > 0.5, "EASE decelerates into the rest, so {mid} > 0.5");

        let mut previous = -1.0;
        for frame in 74..=90 {
            let value = logo_shift(frame as f32);
            assert!(value >= previous, "the slide reversed at frame {frame}");
            previous = value;
        }
    }

    #[test]
    fn the_name_reveals_in_step_with_the_slide() {
        // The source's own assertion: the name starts exactly when the mark starts moving.
        assert!((NAME_IN - LOGO_SHIFT_START).abs() < f32::EPSILON);
        // ... and it is fully revealed while the mark is still travelling, so the two beats read
        // as one gesture rather than as two.
        assert!(name_pose(NAME_IN + 10.0).opacity >= 1.0 - 1e-6);
        assert!(logo_shift(NAME_IN + 10.0) < 1.0);

        assert!(name_pose(NAME_IN - 1.0).opacity.abs() < 1e-6);
        assert!(name_pose(NAME_IN).opacity.abs() < 1e-6);
        assert!((name_pose(NAME_IN + 10.0).opacity - 1.0).abs() < 1e-6);
        assert!((name_pose(108.0).opacity - 1.0).abs() < 1e-6);

        // It slides in from 18 px to the right of its slot.
        assert!((name_pose(NAME_IN).dx - 18.0).abs() < 1e-6);
        assert!(name_pose(NAME_IN + 6.0).dx > 0.0 && name_pose(NAME_IN + 6.0).dx < 18.0);
        assert!(name_pose(NAME_IN + 12.0).dx.abs() < 1e-6);
        assert!(name_pose(108.0).dx.abs() < 1e-6);
        assert!(name_pose(NAME_IN).opacity < name_pose(NAME_IN + 10.0).opacity);
    }

    #[test]
    fn the_default_scene_settles_on_the_measured_frame() {
        // `registry/__measured__.json` records settle.base = 90 for logo-assemble, and the
        // schedule's last channel to come to rest must reproduce it.
        assert!((settle_frame() - 90.0).abs() < 1e-6);

        let settled = settle_frame();
        let at_rest = |frame: f32| {
            (
                contract_progress(frame),
                ring_scale(frame),
                card_scale(frame),
                card_opacity(frame),
                middle_text_opacity(frame),
                logo_pose(frame),
                logo_shift(frame),
                name_pose(frame),
            )
        };
        assert_eq!(at_rest(settled), at_rest(108.0));
        // And something *is* still moving on frame 89, so 90 is the last frame that moves.
        assert!(logo_shift(settled - 1.0) < logo_shift(settled));
        // The measured fit's parameters (`baseWords: 1`, `perWord: 0`) are a linear model, not a
        // card or word count: this scene settles at the same frame whatever rides the ring, which
        // is why `settle_frame` takes no argument at all.
    }

    #[test]
    fn the_geometry_matches_the_sources_own_fractions() {
        let geometry = LogoGeometry::reference();
        assert!(
            (geometry.radius - 259.2).abs() < 1e-4,
            "{}",
            geometry.radius
        );
        assert!(
            (geometry.card_w - 108.0).abs() < 1e-6,
            "{}",
            geometry.card_w
        );
        assert!(
            (geometry.card_h - 131.76).abs() < 1e-4,
            "{}",
            geometry.card_h
        );
        assert!(
            (geometry.logo_size - 90.0).abs() < 1e-6,
            "{}",
            geometry.logo_size
        );
        assert!((geometry.gap - 25.0).abs() < 1e-6, "{}", geometry.gap);
        assert!((geometry.rest_x - 93.6).abs() < 1e-4, "{}", geometry.rest_x);
        assert!(
            (geometry.name_size - 40.0).abs() < 1e-6,
            "{}",
            geometry.name_size
        );
        assert!(
            (geometry.middle_size - 29.0).abs() < 1e-6,
            "{}",
            geometry.middle_size
        );

        // `cardH = cardW * CARD_RATIO`, portrait cards.
        assert!((geometry.card_h - geometry.card_w * CARD_RATIO).abs() < 1e-4);

        // And the whole scene scales with the canvas's short side, not with a config value: at
        // 1080p `unit` is 1.5, so the ring's radius is 388.8 canvas px.
        let unit = 1080.0 / 100.0 / crate::motion::REFERENCE_VMIN;
        assert!((unit - 1.5).abs() < 1e-6);
        assert!((geometry.radius * unit - 388.8).abs() < 1e-3);
    }

    #[test]
    fn orbit_offsets_place_the_ring() {
        let radius = 100.0;
        // Card 0 sits on the +x axis at angle 0.
        let (x, y) = orbit_offset(0, 4, 0.0, radius);
        assert!((x - radius).abs() < 1e-4 && y.abs() < 1e-4);

        // A quarter turn later card 1 takes its place, and the ring is evenly spaced.
        let (x, y) = orbit_offset(0, 4, std::f32::consts::FRAC_PI_2, radius);
        assert!(x.abs() < 1e-4 && (y - radius).abs() < 1e-4);
        let (x, y) = orbit_offset(1, 4, 0.0, radius);
        assert!(x.abs() < 1e-4 && (y - radius).abs() < 1e-4);

        // Every card is exactly `radius` from the centre, whatever the angle or the count.
        for frame in [0.0, 12.5, 50.0, 58.0, 66.0] {
            let angle = orbit_angle(frame);
            for index in 0..10 {
                let (x, y) = orbit_offset(index, 10, angle, radius * ring_scale(frame));
                let distance = x.hypot(y);
                let expected = radius * ring_scale(frame);
                assert!(
                    (distance - expected).abs() < 1e-2,
                    "frame {frame} card {index}: {distance} from the centre, expected {expected}"
                );
            }
        }

        // A ring of no cards has no offset to report rather than dividing by zero.
        assert_eq!(orbit_offset(3, 0, 1.0, radius), (0.0, 0.0));
    }

    #[test]
    fn the_card_count_floors_and_never_drops_below_one() {
        assert_eq!(card_count(10.0), 10);
        assert_eq!(card_count(10.9), 10);
        assert_eq!(card_count(5.0), 5);
        assert_eq!(card_count(16.0), 16);
        assert_eq!(card_count(0.5), 1);
        assert_eq!(card_count(0.0), 1);
        assert_eq!(card_count(-3.0), 1);
        assert_eq!(card_count(f32::NAN), 1);
        assert_eq!(card_count(f32::INFINITY), 1);
    }

    #[test]
    fn the_middle_text_breaks_on_newlines_and_drops_blanks() {
        assert_eq!(
            middle_lines("Cinematic components for React"),
            vec!["Cinematic components for React"]
        );
        assert_eq!(
            middle_lines("Cinematic components\nfor React"),
            vec!["Cinematic components", "for React"]
        );
        assert_eq!(middle_lines("  a  \n\n  b  "), vec!["a", "b"]);
        assert!(middle_lines("").is_empty());
        assert!(middle_lines("\n \n").is_empty());
    }

    #[test]
    fn the_monogram_is_the_brands_first_letter_uppercased() {
        assert_eq!(monogram("fframes"), "F");
        assert_eq!(monogram("élise"), "É");
        assert_eq!(monogram(""), "");
    }

    #[test]
    fn the_card_tint_cycles_with_the_image_list() {
        let theme = UiTheme::dark();
        // Two cards sharing an image share a tint, and the ramp is monotone across the list.
        assert_eq!(card_tint(&theme, 0, 8), card_tint(&theme, 0, 8));
        let tints: Vec<String> = (0..8).map(|index| card_tint(&theme, index, 8)).collect();
        let mut unique = tints.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            8,
            "every card in the ring should be its own tint"
        );
        assert_ne!(tints[0], tints[7]);
        assert_ne!(tints[0], tints[1]);

        // An empty image list still paints: the ramp falls back to the theme's own fill.
        assert_eq!(card_tint(&theme, 0, 0), theme.secondary);
        assert_eq!(card_tint(&theme, 4, 0), theme.secondary);
    }

    #[test]
    fn the_shadow_stand_in_keeps_the_sources_offset_and_total_alpha() {
        let unit = 1.5;
        let (offsets, alpha) = shadow_copies(unit, 1.0);
        assert_eq!(offsets.len(), blur::COPIES);
        assert!((alpha * blur::COPIES as f32 - SHADOW_ALPHA).abs() < 1e-6);

        // The copies are centred on the source's own `0 14px` offset.
        let mean_x = offsets.iter().map(|(x, _)| x).sum::<f32>() / blur::COPIES as f32;
        let mean_y = offsets.iter().map(|(_, y)| y).sum::<f32>() / blur::COPIES as f32;
        assert!(mean_x.abs() < 1e-4, "mean x {mean_x}");
        assert!((mean_y - SHADOW_DY * unit).abs() < 1e-4, "mean y {mean_y}");

        // And on a ring of `0.6 * blur`, the same spread the text blur uses.
        let expected_ring = SHADOW_BLUR * unit * blur::SPREAD_RATIO;
        for (x, y) in offsets {
            let distance = (x - mean_x).hypot(y - mean_y);
            assert!(
                (distance - expected_ring).abs() < 1e-3,
                "copy at {distance} from the shadow's centre, expected {expected_ring}"
            );
        }

        // A shrinking card scales its own painted shadow: CSS scales the box and its shadow at once.
        let (scaled, scaled_alpha) = shadow_copies(unit, 0.5);
        let scaled_mean_y = scaled.iter().map(|(_, y)| y).sum::<f32>() / blur::COPIES as f32;
        assert!((scaled_mean_y - SHADOW_DY * unit * 0.5).abs() < 1e-4);
        assert!((scaled_alpha - alpha).abs() < 1e-6);
    }

    #[test]
    fn easing_is_applied_per_segment_like_remotion() {
        // `logoPose.scale` runs 0.6 -> 1.05 -> 1: the curve restarts at the middle keyframe
        // rather than running once across the whole 58..76 range.
        let inputs = [LOGO_IN, LOGO_IN + 10.0, LOGO_IN + 18.0];
        let first = segment_midpoint(&inputs, 0);
        let second = segment_midpoint(&inputs, 1);

        let eased = EASE.ease(0.5);
        let expected_first = 0.6 + (1.05 - 0.6) * eased;
        let expected_second = 1.05 + (1.0 - 1.05) * eased;
        assert!((logo_pose(first).scale - expected_first).abs() < 1e-5);
        assert!((logo_pose(second).scale - expected_second).abs() < 1e-5);
        // A single curve across the whole range would put the midpoint in a different place.
        assert!((expected_first - expected_second).abs() > 0.01);

        // A segment with equal outputs ignores the curve entirely.
        assert!(
            (interpolate_stops(30.0, &[16.0, 46.0], &[1.0, 1.0], Some(EASE)) - 1.0).abs() < 1e-6
        );
        // Linear stops stay linear.
        assert!((interpolate_stops(11.0, &[6.0, 16.0], &[0.0, 1.0], None) - 0.5).abs() < 1e-6);
        // Clamped at both ends of the first and last segment.
        assert!(interpolate_stops(-5.0, &[6.0, 16.0], &[0.0, 1.0], None).abs() < 1e-6);
        assert!((interpolate_stops(99.0, &[6.0, 16.0], &[0.0, 1.0], None) - 1.0).abs() < 1e-6);
    }
}
