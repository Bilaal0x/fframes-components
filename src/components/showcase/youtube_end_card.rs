//! A YouTube end screen: a Subscribe call to action, social handles, and two thumbnail frames.
//!
//! Ported from Remotion Elements' `YouTubeEndCard`. The source lays the card out with flex boxes
//! on a 1920 x 1080 canvas; here every box is resolved to fixed scene coordinates in that same
//! box, which one group then fits to the canvas.
//!
//! Deliberate differences from the source:
//!
//! - **Avatar.** The default picture is the embedded `avatar1.png`, not the remote Remotion logo.
//! - **Handles.** The defaults are this crate's own channel handles, not `remotion`.
//! - **Website row.** The source always shows `remotion.dev`; here the row is omitted unless
//!   [`YouTubeEndCardConfig::website`] is set.
//! - **Springs.** `Easing.spring({ damping: 200 })` and `spring({ durationInFrames })` are both
//!   modelled as the damping-200 spring stretched so that it settles at the segment's end.
//! - **Duration.** The source reads `durationInFrames` from its host and has no fixed length, so
//!   the default here is 150 frames.
//! - **Shadows.** SVG filters are not used in this crate, so the avatar's `box-shadow` is stacked
//!   translucent circles.
//!
//! The glyphs are Font Awesome Free brand icons, as in the source.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{Spring, clamp01, lerp};
use crate::ui::text;

/// The scene's width, in source px.
pub const SCENE_W: f32 = 1920.0;
/// The scene's height, in source px.
pub const SCENE_H: f32 = 1080.0;
/// The page behind the scene.
pub const BACKGROUND: &str = "#FAFAFA";
/// The colour of text, borders and the button.
pub const INK: &str = "#000000";
/// The colour of the button's label.
pub const BUTTON_INK: &str = "#ffffff";

/// The left column's left edge.
pub const COLUMN_X: f32 = 80.0;
/// The avatar's diameter.
pub const AVATAR_D: f32 = 140.0;
/// The gap between the avatar and the button.
pub const CTA_GAP: f32 = 30.0;
/// The button's width.
pub const BUTTON_W: f32 = 400.0;
/// The button's height.
pub const BUTTON_H: f32 = 140.0;
/// The button's corner radius.
pub const BUTTON_RADIUS: f32 = 70.0;
/// The font size of every text run.
pub const TEXT_SIZE: f32 = 50.0;
/// The font weight of every text run.
pub const TEXT_WEIGHT: u16 = 500;

/// The gap between the call to action and the first social row.
pub const LIST_MARGIN_TOP: f32 = 80.0;
/// A social row's vertical padding, top and bottom.
pub const ROW_PAD: f32 = 20.0;
/// The icon box's width.
pub const ICON_BOX_W: f32 = 140.0;
/// The icon box's height.
pub const ICON_BOX_H: f32 = 60.0;
/// How far the icon box is shifted down.
pub const ICON_BOX_DROP: f32 = 4.0;
/// The gap between the icon box and the text.
pub const ICON_TEXT_GAP: f32 = 30.0;
/// The text's left margin.
pub const TEXT_MARGIN: f32 = 20.0;
/// The gap above the optional website row.
pub const WEBSITE_GAP: f32 = 80.0;

/// The Instagram icon's rendered height.
pub const INSTAGRAM_H: f32 = 70.0;
/// The LinkedIn icon's rendered height. The source's extra `-4px` cancels the box's drop.
pub const LINKEDIN_H: f32 = 60.0;
/// The X icon's rendered height.
pub const X_H: f32 = 60.0;
/// The link icon's rendered height.
pub const LINK_H: f32 = 60.0;
/// The Instagram icon's view box (width, height).
pub const INSTAGRAM_VIEW: (f32, f32) = (448.0, 512.0);
/// The LinkedIn icon's view box (width, height).
pub const LINKEDIN_VIEW: (f32, f32) = (448.0, 512.0);
/// The X icon's view box (width, height).
pub const X_VIEW: (f32, f32) = (512.0, 512.0);
/// The link icon's view box (width, height).
pub const LINK_VIEW: (f32, f32) = (640.0, 512.0);

/// The frame the call to action starts sliding up.
pub const CTA_START: f32 = 35.0;
/// How many frames the slide takes.
pub const CTA_FRAMES: f32 = 30.0;
/// How far below its rest position the call to action starts.
pub const CTA_DROP: f32 = 281.0;
/// The frame the first row starts fading in.
pub const ROW_START: f32 = 35.0;
/// How many frames a row's fade takes.
pub const ROW_FRAMES: f32 = 15.0;
/// The spring every fade and slide uses.
pub const EASE_SPRING: Spring = Spring {
    damping: 200.0,
    mass: 1.0,
    stiffness: 100.0,
};

/// A thumbnail frame's content width.
pub const THUMB_W: f32 = 631.0;
/// A thumbnail frame's content height.
pub const THUMB_H: f32 = 361.0;
/// A thumbnail frame's border width. The source's boxes are content-box, so it adds to the size.
pub const THUMB_BORDER: f32 = 6.0;
/// The gap between the top thumbnail's top edge and the canvas top.
pub const THUMB_TOP: f32 = 135.0;
/// The gap between the bottom thumbnail's bottom edge and the canvas bottom.
pub const THUMB_BOTTOM: f32 = 155.0;
/// The gap between the thumbnails and the canvas right edge.
pub const THUMB_RIGHT: f32 = 100.0;

/// The Font Awesome `linkedin-in` glyph (448 x 512 view box).
pub const LINKEDIN_PATH: &str =
    "M100.28 448H7.4V148.9h92.88zM53.79 108.1C24.09 108.1 0 83.5 0 53.8a53.79 53.79 0 0 1 107.58 0c0 29.7-24.1 54.3-53.79 54.3zM447.9 448h-92.68V302.4c0-34.7-.7-79.2-48.29-79.2-48.29 0-55.69 37.7-55.69 76.7V448h-92.78V148.9h89.08v40.8h1.3c12.4-23.5 42.69-48.3 87.88-48.3 94 0 111.28 61.9 111.28 142.3V448z";

/// The Font Awesome `x-twitter` glyph (512 x 512 view box).
pub const X_PATH: &str =
    "M389.2 48h70.6L305.6 224.2 487 464H345L233.7 318.6 106.5 464H35.8L200.7 275.5 26.8 48H172.4L272.9 180.9 389.2 48zM364.4 421.8h39.1L151.1 88h-42L364.4 421.8z";

/// The Font Awesome `link` glyph (640 x 512 view box).
pub const LINK_PATH: &str =
    "M580.2 267.3c56.2-56.2 56.2-147.4 0-203.6S432.8 7.4 376.6 63.7L365.3 75l45.3 45.3 11.3-11.3c31.2-31.2 81.9-31.2 113.1 0s31.2 81.9 0 113.1L421.8 335.2c-31.2 31.2-81.9 31.2-113.1 0c-25.6-25.6-30.3-64.3-13.8-94.6c1.8-3.4 3.9-6.7 6.3-9.8l-51.2-38.4c-4.3 5.7-8.1 11.6-11.4 17.8c-29.5 54.6-21.3 124.2 24.9 170.3c56.2 56.2 147.4 56.2 203.6 0L580.2 267.3zM59.8 244.7c-56.2 56.2-56.2 147.4 0 203.6s147.4 56.2 203.6 0L274.7 437l-45.3-45.3-11.3 11.3c-31.2 31.2-81.9 31.2-113.1 0s-31.2-81.9 0-113.1L218.2 176.8c31.2-31.2 81.9-31.2 113.1 0c25.6 25.6 30.3 64.3 13.8 94.6c-1.8 3.4-3.9 6.7-6.3 9.8l51.2 38.4c4.3-5.7 8.1-11.6 11.4-17.8c29.5-54.6 21.3-124.2-24.9-170.3c-56.2-56.2-147.4-56.2-203.6 0L59.8 244.7z";

/// The Font Awesome `instagram` glyph (448 x 512 view box).
pub const INSTAGRAM_PATH: &str =
    "M224.1 141c-63.6 0-114.9 51.3-114.9 114.9s51.3 114.9 114.9 114.9S339 319.5 339 255.9 287.7 141 224.1 141zm0 189.6c-41.1 0-74.7-33.5-74.7-74.7s33.5-74.7 74.7-74.7 74.7 33.5 74.7 74.7-33.6 74.7-74.7 74.7zm146.4-194.3c0 14.9-12 26.8-26.8 26.8-14.9 0-26.8-12-26.8-26.8s12-26.8 26.8-26.8 26.8 12 26.8 26.8zm76.1 27.2c-1.7-35.9-9.9-67.7-36.2-93.9-26.2-26.2-58-34.4-93.9-36.2-37-2.1-147.9-2.1-184.9 0-35.8 1.7-67.6 9.9-93.9 36.1s-34.4 58-36.2 93.9c-2.1 37-2.1 147.9 0 184.9 1.7 35.9 9.9 67.7 36.2 93.9s58 34.4 93.9 36.2c37 2.1 147.9 2.1 184.9 0 35.9-1.7 67.7-9.9 93.9-36.2 26.2-26.2 34.4-58 36.2-93.9 2.1-37 2.1-147.8 0-184.8zM398.8 388c-7.8 19.6-22.9 34.7-42.6 42.6-29.5 11.7-99.5 9-132.1 9s-102.7 2.6-132.1-9c-19.6-7.8-34.7-22.9-42.6-42.6-11.7-29.5-9-99.5-9-132.1s-2.6-102.7 9-132.1c7.8-19.6 22.9-34.7 42.6-42.6 29.5-11.7 99.5-9 132.1-9s102.7 2.6 132.1 9c19.6 7.8 34.7 22.9 42.6 42.6 11.7 29.5 9 99.5 9 132.1s2.7 102.7-9 132.1z";

/// The spring's value at `t` of a segment, stretched so that it settles at `t = 1`.
///
/// Remotion's `durationInFrames` rescales the natural spring to the requested length. The natural
/// length is the last frame at which the spring is still 0.005 or more from rest, plus one.
#[must_use]
pub fn stretched_spring(t: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    let mut natural = 1.0;
    for frame in 0..1000 {
        if (EASE_SPRING.value(frame as f32) - 1.0).abs() >= 0.005 {
            natural = frame as f32 + 1.0;
        }
    }
    EASE_SPRING.value(t * natural)
}

/// A fade from `start` over `frames`, as a stretched spring clamped to 0..1.
#[must_use]
pub fn spring_fade(frame: f32, start: f32, frames: f32) -> f32 {
    stretched_spring(clamp01((frame - start) / frames))
}

/// The start frame of the social row at `index_from_last` (Instagram 4, LinkedIn 3, X 2).
#[must_use]
pub fn row_start(index_from_last: f32) -> f32 {
    ROW_START + ((index_from_last - 1.0) / 4.0) * (30.0 - 15.0)
}

/// A social row's opacity at `frame`.
#[must_use]
pub fn row_opacity(frame: f32, index_from_last: f32) -> f32 {
    spring_fade(frame, row_start(index_from_last), ROW_FRAMES)
}

/// The website row's opacity at `frame`.
#[must_use]
pub fn website_opacity(frame: f32) -> f32 {
    spring_fade(frame, ROW_START, ROW_FRAMES)
}

/// How far below its rest position the call to action sits at `frame`.
#[must_use]
pub fn cta_offset(frame: f32) -> f32 {
    lerp(CTA_DROP, 0.0, spring_fade(frame, CTA_START, CTA_FRAMES))
}

/// The scale that fits the scene to the canvas.
#[must_use]
pub fn scene_scale(canvas_width: f32, canvas_height: f32) -> f32 {
    (canvas_width / SCENE_W).min(canvas_height / SCENE_H)
}

/// What a [`YouTubeEndCard`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct YouTubeEndCardConfig {
    /// The Instagram handle, shown as given.
    pub instagram: String,
    /// The LinkedIn name, shown as given.
    pub linkedin: String,
    /// The X handle, shown as given.
    pub x: String,
    /// The button's label.
    pub subscribe_label: String,
    /// The website shown in a fourth row, or `None` to omit the row.
    pub website: Option<String>,
    /// The avatar's picture name. Empty draws an initial instead.
    pub avatar: String,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for YouTubeEndCardConfig {
    fn default() -> Self {
        Self {
            instagram: "@Bilaal0x".to_owned(),
            linkedin: "Bilaal0x".to_owned(),
            x: "@Bilaal0x".to_owned(),
            subscribe_label: "Subscribe".to_owned(),
            website: None,
            avatar: "avatar1.png".to_owned(),
            font_family: None,
            duration: 150,
        }
    }
}

/// The YouTube end card component.
#[derive(Debug, Clone, Default)]
pub struct YouTubeEndCard {
    /// What the card shows.
    pub config: YouTubeEndCardConfig,
}

impl YouTubeEndCard {
    /// A card with `config`.
    #[must_use]
    pub fn new(config: YouTubeEndCardConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "youtube-end-card",
            name: "YouTube End Card",
            summary: "A Subscribe call to action and social handles beside two thumbnail frames",
            feature: "youtube-end-card",
        }
    }
}

/// An icon centred on (`cx`, `cy`) at `height`, filled black.
fn icon(path: &str, view: (f32, f32), height: f32, cx: f32, cy: f32) -> Svgr<'static> {
    let scale = height / view.1;
    let transform = format!(
        "translate({} {}) scale({scale})",
        cx - view.0 * scale / 2.0,
        cy - height / 2.0
    );
    fframes::svgr!(<path transform={transform} d={path.to_owned()} fill={INK} />)
}

/// The avatar's shadow: stacked translucent circles standing in for a 20 px blur.
fn avatar_shadow(cx: f32, cy: f32) -> Svgr<'static> {
    let mut layers: Vec<Svgr<'static>> = Vec::new();
    for step in 0..10 {
        let grow = step as f32 * 1.6;
        layers.push(fframes::svgr!(
            <circle cx={cx} cy={cy} r={AVATAR_D / 2.0 + grow} fill="#000000" opacity={0.014} />
        ));
    }
    Svgr::from(layers)
}

/// The avatar circle centred on (`cx`, `cy`).
fn avatar(
    ctx: &RenderCtx<'_>,
    config: &YouTubeEndCardConfig,
    family: &str,
    cx: f32,
    cy: f32,
) -> Svgr<'static> {
    let r = AVATAR_D / 2.0;
    let picture = if config.avatar.is_empty() {
        let initial: String = config
            .instagram
            .trim_start_matches('@')
            .chars()
            .take(1)
            .collect();
        fframes::svgr!(
            <g>
                <circle cx={cx} cy={cy} r={r} fill={INK} />
                {text::text_node(
                    ctx,
                    &initial.to_uppercase(),
                    cx,
                    text::baseline_for_centred_box(ctx, family, TEXT_SIZE, cy),
                    family.to_owned(),
                    TEXT_SIZE,
                    TEXT_WEIGHT,
                    0.0,
                    BUTTON_INK.to_owned(),
                    1.0,
                )}
            </g>
        )
    } else {
        // The circle is a clip because it rounds a picture.
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id="youtube-end-card-avatar">
                        <circle cx={cx} cy={cy} r={r} />
                    </clipPath>
                </defs>
                <image href={config.avatar.clone()} x={cx - r} y={cy - r}
                       width={AVATAR_D} height={AVATAR_D}
                       preserveAspectRatio="xMidYMid slice"
                       clip-path="url(#youtube-end-card-avatar)" />
            </g>
        )
    };
    fframes::svgr!(<g>{avatar_shadow(cx, cy)}{picture}</g>)
}

/// The call to action: avatar, gap and button, with the avatar's left edge at `COLUMN_X`.
fn call_to_action(
    ctx: &RenderCtx<'_>,
    config: &YouTubeEndCardConfig,
    family: &str,
    top: f32,
) -> Svgr<'static> {
    let cy = top + BUTTON_H / 2.0;
    let button_x = COLUMN_X + AVATAR_D + CTA_GAP;
    fframes::svgr!(
        <g>
            {avatar(ctx, config, family, COLUMN_X + AVATAR_D / 2.0, cy)}
            <rect x={button_x} y={top} width={BUTTON_W} height={BUTTON_H} rx={BUTTON_RADIUS}
                  fill={INK} />
            {text::text_node(
                ctx,
                &config.subscribe_label,
                button_x + BUTTON_W / 2.0,
                text::baseline_for_centred_box(ctx, family, TEXT_SIZE, cy),
                family.to_owned(),
                TEXT_SIZE,
                TEXT_WEIGHT,
                0.0,
                BUTTON_INK.to_owned(),
                1.0,
            )}
        </g>
    )
}

/// The height of a social row's content: the taller of the icon box and the text line.
fn content_height(ctx: &RenderCtx<'_>, family: &str) -> f32 {
    let line = ctx
        .meter(family)
        .map_or(TEXT_SIZE * 1.2, |meter| meter.line_height(TEXT_SIZE));
    ICON_BOX_H.max(line)
}

/// One social row whose padding box starts at `top`.
#[allow(clippy::too_many_arguments)]
fn social_row(
    ctx: &RenderCtx<'_>,
    family: &str,
    top: f32,
    path: &str,
    view: (f32, f32),
    icon_height: f32,
    label: &str,
    opacity: f32,
) -> Svgr<'static> {
    let content = content_height(ctx, family);
    let cy = top + ROW_PAD + content / 2.0;
    let icon_cx = COLUMN_X + ICON_BOX_W / 2.0;
    let text_x = COLUMN_X + ICON_BOX_W + ICON_TEXT_GAP + TEXT_MARGIN;
    fframes::svgr!(
        <g opacity={opacity}>
            {icon(path, view, icon_height, icon_cx, cy + ICON_BOX_DROP)}
            {text::text_node_left(
                label,
                text_x,
                text::baseline_for_centred_box(ctx, family, TEXT_SIZE, cy),
                family.to_owned(),
                TEXT_SIZE,
                TEXT_WEIGHT,
                0.0,
                INK.to_owned(),
                1.0,
            )}
        </g>
    )
}

/// A bordered, empty thumbnail frame whose outer box has its top-left at (`x`, `y`).
fn thumbnail(x: f32, y: f32) -> Svgr<'static> {
    // The stroke is centred on the rect's edge, so the rect is inset by half the border.
    let half = THUMB_BORDER / 2.0;
    fframes::svgr!(
        <rect x={x + half} y={y + half} width={THUMB_W + THUMB_BORDER}
              height={THUMB_H + THUMB_BORDER} fill="none" stroke={INK}
              stroke-width={THUMB_BORDER} />
    )
}

impl Component for YouTubeEndCard {
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
        let frame = ctx.frame();

        // The source's box, fitted to the canvas centre.
        let scale = scene_scale(ctx.width(), ctx.height());
        let fit = format!(
            "translate({} {}) scale({scale})",
            (ctx.width() - SCENE_W * scale) / 2.0,
            (ctx.height() - SCENE_H * scale) / 2.0
        );

        let row_h = ROW_PAD * 2.0 + content_height(ctx, &family);
        let mut column_h = BUTTON_H + LIST_MARGIN_TOP + 3.0 * row_h;
        if config.website.is_some() {
            column_h += WEBSITE_GAP + row_h;
        }
        let top = (SCENE_H - column_h) / 2.0;
        let list_top = top + BUTTON_H + LIST_MARGIN_TOP;

        let cta = call_to_action(ctx, config, &family, top);
        let cta_transform = format!("translate(0 {})", cta_offset(frame));

        let mut rows: Vec<Svgr<'static>> = vec![
            social_row(
                ctx,
                &family,
                list_top,
                INSTAGRAM_PATH,
                INSTAGRAM_VIEW,
                INSTAGRAM_H,
                &config.instagram,
                row_opacity(frame, 4.0),
            ),
            social_row(
                ctx,
                &family,
                list_top + row_h,
                LINKEDIN_PATH,
                LINKEDIN_VIEW,
                LINKEDIN_H,
                &config.linkedin,
                row_opacity(frame, 3.0),
            ),
            social_row(
                ctx,
                &family,
                list_top + 2.0 * row_h,
                X_PATH,
                X_VIEW,
                X_H,
                &config.x,
                row_opacity(frame, 2.0),
            ),
        ];
        if let Some(website) = &config.website {
            rows.push(social_row(
                ctx,
                &family,
                list_top + 3.0 * row_h + WEBSITE_GAP,
                LINK_PATH,
                LINK_VIEW,
                LINK_H,
                website,
                website_opacity(frame),
            ));
        }

        let outer_w = THUMB_W + 2.0 * THUMB_BORDER;
        let outer_h = THUMB_H + 2.0 * THUMB_BORDER;
        let thumb_x = SCENE_W - THUMB_RIGHT - outer_w;
        let top_thumb = thumbnail(thumb_x, THUMB_TOP);
        let bottom_thumb = thumbnail(thumb_x, SCENE_H - THUMB_BOTTOM - outer_h);

        fframes::svgr!(
            <g transform={fit}>
                <rect x={0} y={0} width={SCENE_W} height={SCENE_H} fill={BACKGROUND} />
                <g transform={cta_transform}>{cta}</g>
                {Svgr::from(rows)}
                {top_thumb}
                {bottom_thumb}
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = YouTubeEndCardConfig::default();
        assert_eq!(config.instagram, "@Bilaal0x");
        assert_eq!(config.linkedin, "Bilaal0x");
        assert_eq!(config.x, "@Bilaal0x");
        assert_eq!(config.subscribe_label, "Subscribe");
        assert_eq!(config.website, None);
        assert_eq!(config.avatar, "avatar1.png");
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 150);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = YouTubeEndCard::default();
        assert_eq!(component.meta().id, "youtube-end-card");
        assert_eq!(YouTubeEndCard::meta_static().feature, "youtube-end-card");
        assert!(component.duration() > 0);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn the_rows_start_staggered_from_the_instagram_row_down() {
        assert!((row_start(4.0) - 46.25).abs() < 1e-4);
        assert!((row_start(3.0) - 42.5).abs() < 1e-4);
        assert!((row_start(2.0) - 38.75).abs() < 1e-4);
    }

    #[test]
    fn a_fade_is_clamped_and_settles() {
        assert_eq!(row_opacity(0.0, 4.0), 0.0);
        assert_eq!(row_opacity(46.25, 4.0), 0.0);
        assert_eq!(row_opacity(61.25, 4.0), 1.0);
        assert_eq!(row_opacity(149.0, 4.0), 1.0);
        let mid = row_opacity(53.0, 4.0);
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(website_opacity(34.0), 0.0);
        assert_eq!(website_opacity(50.0), 1.0);
    }

    #[test]
    fn the_stretched_spring_rises_monotonically() {
        let mut last = 0.0;
        for step in 0..=20 {
            let value = stretched_spring(step as f32 / 20.0);
            assert!(value >= last - 1e-6);
            last = value;
        }
        assert_eq!(stretched_spring(1.0), 1.0);
    }

    #[test]
    fn the_call_to_action_rises_from_281_to_rest() {
        assert!((cta_offset(0.0) - 281.0).abs() < 1e-4);
        assert!((cta_offset(35.0) - 281.0).abs() < 1e-4);
        assert!(cta_offset(50.0) < 281.0 && cta_offset(50.0) > 0.0);
        assert!(cta_offset(65.0).abs() < 1e-4);
        assert!(cta_offset(120.0).abs() < 1e-4);
    }

    #[test]
    fn the_thumbnails_sit_inside_the_canvas() {
        let outer_w = THUMB_W + 2.0 * THUMB_BORDER;
        assert!((SCENE_W - THUMB_RIGHT - outer_w - 1177.0).abs() < 1e-4);
        let bottom_y = SCENE_H - THUMB_BOTTOM - (THUMB_H + 2.0 * THUMB_BORDER);
        assert!((bottom_y - 552.0).abs() < 1e-4);
        assert!(THUMB_TOP + THUMB_H + 2.0 * THUMB_BORDER < bottom_y);
    }

    #[test]
    fn the_scene_fits_landscape_and_portrait_canvases() {
        assert!((scene_scale(1920.0, 1080.0) - 1.0).abs() < 1e-6);
        assert!((scene_scale(1080.0, 1920.0) - 0.5625).abs() < 1e-6);
    }
}
