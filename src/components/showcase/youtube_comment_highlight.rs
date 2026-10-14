//! A YouTube comment card that swings in with a 3D tilt, holds, then drops away.
//!
//! Ported from Remotion Elements' `YouTubeCommentHighlight`. The source lays the card out with a
//! CSS grid inside a 1120 x 360 px box; here every box is resolved to fixed scene coordinates in
//! that same box, which one group then fits to the canvas.
//!
//! Deliberate differences from the source:
//!
//! - **No real 3D.** SVG has no perspective, so the source's `rotate: y <angle>` under
//!   `perspective: 1500px` is approximated by squeezing the card horizontally by `cos(angle)`
//!   about its centre. The far edge does not shrink relative to the near edge (no trapezoid).
//! - **Shadow.** SVG filters are not used in this crate, so `box-shadow: 0 18px 40px` is stacked
//!   translucent rounded rectangles.
//! - **Comment, handle and avatar.** The defaults are this crate's own: a comment about how easy
//!   the library is to use, the handle `@Bilaal0x` and the embedded `avatar1.png`.
//! - **Background.** The source draws nothing behind the card; here the page is a dark neutral
//!   so the dark card reads against it.
//! - **Slide distance.** The source slides 760 px, enough to clear a 16:9 canvas. On a portrait
//!   canvas that would leave the card visible, so the distance grows to clear the canvas bottom.
//! - **Scene fit.** The scene is fitted to 90% of the canvas, in the same way for both
//!   orientations.
//! - **Springs.** `Easing.spring` is modelled as the spring stretched so that its natural length
//!   (until it is within 0.0005 of rest) fills the 48 frame segment, blended to rest at its end.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{Spring, clamp01, lerp};
use crate::ui::text;

/// The scene's width, in source px.
pub const SCENE_W: f32 = 1120.0;
/// The scene's height, in source px.
pub const SCENE_H: f32 = 360.0;
/// The share of the canvas the scene may fill.
pub const MAX_SHARE: f32 = 0.9;
/// The page behind the scene: a dark neutral, chosen because the source has no background.
pub const BACKGROUND: &str = "#0b0b0d";

/// The card's top edge within the scene.
pub const CARD_TOP: f32 = 30.0;
/// The card's width.
pub const CARD_W: f32 = 1120.0;
/// The card's height.
pub const CARD_H: f32 = 300.0;
/// The card's corner radius.
pub const CARD_RADIUS: f32 = 26.0;
/// The card's fill.
pub const CARD_FILL: &str = "#15161a";
/// The card's border width.
pub const CARD_BORDER: f32 = 1.0;
/// The card's border opacity (white).
pub const CARD_BORDER_OPACITY: f32 = 0.09;
/// The card's horizontal padding.
pub const PAD_X: f32 = 40.0;
/// The card's vertical padding.
pub const PAD_Y: f32 = 24.0;
/// The gap between the avatar and the text column.
pub const COLUMN_GAP: f32 = 18.0;

/// The shadow's vertical offset.
pub const SHADOW_DY: f32 = 18.0;
/// How many translucent layers stand in for the shadow's blur.
pub const SHADOW_LAYERS: usize = 10;
/// How far each shadow layer grows past the previous one.
pub const SHADOW_STEP: f32 = 4.0;
/// The shadow's peak opacity at the card's edge.
pub const SHADOW_OPACITY: f32 = 0.26;

/// The avatar's diameter.
pub const AVATAR_D: f32 = 88.0;
/// The avatar's backing fill.
pub const AVATAR_FILL: &str = "#2a2b31";
/// The avatar's border opacity (white).
pub const AVATAR_BORDER_OPACITY: f32 = 0.12;

/// The handle's font size.
pub const HANDLE_SIZE: f32 = 26.0;
/// The handle's font weight.
pub const HANDLE_WEIGHT: u16 = 700;
/// The handle's line height, as a multiple of its size.
pub const HANDLE_LINE: f32 = 1.1;
/// The handle's colour.
pub const HANDLE_INK: &str = "#ffffff";

/// The comment's font size.
pub const COMMENT_SIZE: f32 = 32.0;
/// The comment's font weight.
pub const COMMENT_WEIGHT: u16 = 500;
/// The comment's line height, as a multiple of its size.
pub const COMMENT_LINE: f32 = 1.3;
/// The comment's letter spacing, in px.
pub const COMMENT_TRACKING: f32 = -0.35;
/// The comment's colour.
pub const COMMENT_INK: &str = "#f4f4f5";
/// The gap between the handle and the comment.
pub const COMMENT_MARGIN_TOP: f32 = 8.0;

/// The gap between the comment and the action row.
pub const ACTIONS_MARGIN_TOP: f32 = 14.0;
/// The action row's colour.
pub const ACTIONS_INK: &str = "#a9abb3";
/// The count's font size.
pub const ACTIONS_SIZE: f32 = 22.0;
/// The count's font weight.
pub const ACTIONS_WEIGHT: u16 = 600;
/// The gap between the like group and the dislike icon.
pub const ACTIONS_GAP: f32 = 22.0;
/// The gap between the like icon and its count.
pub const LIKE_GAP: f32 = 8.0;
/// The thumb icons' rendered size.
pub const ICON_SIZE: f32 = 28.0;
/// The thumb icons' view box edge.
pub const ICON_VIEW: f32 = 24.0;
/// The thumb icons' stroke width, in view box units.
pub const ICON_STROKE: f32 = 1.8;

/// The slide's travel, in scene px, on a landscape canvas.
pub const SLIDE_DROP: f32 = 760.0;
/// How many frames the slide in and the slide out each take.
pub const SLIDE_FRAMES: f32 = 48.0;
/// The spring both slides use.
pub const SLIDE_SPRING: Spring = Spring {
    damping: 14.0,
    mass: 0.8,
    stiffness: 110.0,
};

/// The frames at which the rotation keyframes sit.
pub const TILT_FRAMES: [f32; 4] = [0.0, 90.0, 180.0, 270.0];
/// The rotation about the vertical axis at each keyframe, in degrees.
pub const TILT_DEGREES: [f32; 4] = [-10.0, 5.0, -5.0, 5.0];

/// The thumbs-up glyph (24 x 24 view box), copied from the source by script.
pub const THUMB_PATH: &str = "M7 10v11H4V10Zm2 11V10l4-7c1.4.2 2.2 1.5 1.8 2.8L14 9h4.5c1.3 0 2.3 1.2 2 2.5l-1.7 7A3.3 3.3 0 0 1 15.6 21Z";

/// The rotation about the vertical axis at `frame`, in degrees.
///
/// Piecewise between [`TILT_FRAMES`] with `Easing.inOut(Easing.sin)`, clamped at both ends.
#[must_use]
pub fn tilt_degrees(frame: f32) -> f32 {
    if frame <= TILT_FRAMES[0] {
        return TILT_DEGREES[0];
    }
    for i in 1..TILT_FRAMES.len() {
        if frame <= TILT_FRAMES[i] {
            let t = (frame - TILT_FRAMES[i - 1]) / (TILT_FRAMES[i] - TILT_FRAMES[i - 1]);
            let eased = (1.0 - (std::f32::consts::PI * t).cos()) / 2.0;
            return lerp(TILT_DEGREES[i - 1], TILT_DEGREES[i], eased);
        }
    }
    TILT_DEGREES[TILT_DEGREES.len() - 1]
}

/// The horizontal squeeze a rotation of `degrees` about the vertical axis becomes.
#[must_use]
pub fn tilt_scale_x(degrees: f32) -> f32 {
    degrees.to_radians().cos()
}

/// The spring's value at `t` of a segment, stretched so its natural length fills the segment.
///
/// The natural length is the last frame at which [`SLIDE_SPRING`] is still 0.0005 or more from
/// rest, plus one. The small leftover at the end of that length is blended out with a smoothstep,
/// so the value reaches exactly 1 at `t = 1` without a visible snap.
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
        if (SLIDE_SPRING.value(frame as f32) - 1.0).abs() >= 0.0005 {
            natural = frame as f32 + 1.0;
        }
    }
    let leftover = 1.0 - SLIDE_SPRING.value(natural);
    let blend = t * t * (3.0 - 2.0 * t);
    SLIDE_SPRING.value(t * natural) + leftover * blend
}

/// The slide distance in scene px that clears the canvas bottom at `scale`.
///
/// It is [`SLIDE_DROP`] on a landscape canvas and grows when the canvas is taller.
#[must_use]
pub fn slide_drop(scale: f32, canvas_height: f32) -> f32 {
    SLIDE_DROP.max(canvas_height / (2.0 * scale) + (SCENE_H / 2.0 - CARD_TOP))
}

/// How far below its rest position the card sits at `frame` of `duration`, for a `drop` px slide.
///
/// It springs in over frames 0..48, holds, then springs out over the last 48 frames.
#[must_use]
pub fn slide_offset(frame: f32, duration: usize, drop: f32) -> f32 {
    let last = duration as f32 - 1.0;
    let out_start = (last - SLIDE_FRAMES).max(SLIDE_FRAMES);
    if frame <= 0.0 {
        drop
    } else if frame < SLIDE_FRAMES {
        let t = frame / SLIDE_FRAMES;
        lerp(drop, 0.0, stretched_spring(t))
    } else if frame <= out_start {
        0.0
    } else if frame < last {
        // `Easing.out(spring)`: the spring played backwards, so it leaves with an overshoot.
        let t = clamp01((frame - out_start) / (last - out_start));
        lerp(0.0, drop, 1.0 - stretched_spring(1.0 - t))
    } else {
        drop
    }
}

/// The scale that fits the scene to the canvas.
#[must_use]
pub fn scene_scale(canvas_width: f32, canvas_height: f32) -> f32 {
    (MAX_SHARE * canvas_width / SCENE_W).min(MAX_SHARE * canvas_height / SCENE_H)
}

/// The width of the text column.
#[must_use]
pub fn column_width() -> f32 {
    CARD_W - 2.0 * CARD_BORDER - 2.0 * PAD_X - AVATAR_D - COLUMN_GAP
}

/// `text` broken greedily into lines no wider than `max_width`, by the `measure` of a run.
///
/// A word wider than the line sits alone on its own line.
#[must_use]
pub fn wrap_words(text: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if line.is_empty() || measure(&candidate) <= max_width {
            line = candidate;
        } else {
            lines.push(std::mem::take(&mut line));
            line = word.to_owned();
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// What a [`YouTubeCommentHighlight`] shows.
#[derive(Debug, Clone, PartialEq)]
pub struct YouTubeCommentHighlightConfig {
    /// The author's handle, shown as given.
    pub handle: String,
    /// The comment, wrapped within the text column.
    pub comment: String,
    /// The like count, shown as given.
    pub likes: String,
    /// The avatar's picture name. Empty draws a plain disc.
    pub avatar: String,
    /// The font family, or `None` for the embedded Inter.
    pub font_family: Option<String>,
    /// The length in frames.
    pub duration: usize,
}

impl Default for YouTubeCommentHighlightConfig {
    fn default() -> Self {
        Self {
            handle: "@Bilaal0x".to_owned(),
            comment: "Added this to my video in a few lines of Rust. Easiest component \
                      library I've used - pick a component, set a few options, render."
                .to_owned(),
            likes: "2.4K".to_owned(),
            avatar: "avatar1.png".to_owned(),
            font_family: None,
            duration: 300,
        }
    }
}

/// The YouTube comment highlight component.
#[derive(Debug, Clone, Default)]
pub struct YouTubeCommentHighlight {
    /// What the card shows.
    pub config: YouTubeCommentHighlightConfig,
}

impl YouTubeCommentHighlight {
    /// A card with `config`.
    #[must_use]
    pub fn new(config: YouTubeCommentHighlightConfig) -> Self {
        Self { config }
    }

    /// The component's catalog entry, without an instance.
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "youtube-comment-highlight",
            name: "YouTube Comment Highlight",
            summary: "A YouTube comment card swings in with a 3D tilt, holds, then drops away",
            feature: "youtube-comment-highlight",
        }
    }
}

/// The card's shadow: stacked translucent rounded rects standing in for a 40 px blur.
fn card_shadow() -> Svgr<'static> {
    let per_layer = SHADOW_OPACITY / SHADOW_LAYERS as f32;
    let mut layers: Vec<Svgr<'static>> = Vec::new();
    for step in 0..SHADOW_LAYERS {
        let grow = step as f32 * SHADOW_STEP;
        layers.push(fframes::svgr!(
            <rect x={-grow} y={CARD_TOP + SHADOW_DY - grow} width={CARD_W + 2.0 * grow}
                  height={CARD_H + 2.0 * grow} rx={CARD_RADIUS + grow} fill="#000000"
                  opacity={per_layer} />
        ));
    }
    Svgr::from(layers)
}

/// A thumb icon whose 24 x 24 box has its top-left at (`x`, `y`), turned by `turn` degrees.
fn thumb(x: f32, y: f32, turn: f32) -> Svgr<'static> {
    let scale = ICON_SIZE / ICON_VIEW;
    let transform = format!(
        "translate({x} {y}) rotate({turn} {c} {c}) scale({scale})",
        c = ICON_SIZE / 2.0
    );
    fframes::svgr!(
        <path transform={transform} d={THUMB_PATH} fill="none" stroke={ACTIONS_INK}
              stroke-width={ICON_STROKE} stroke-linejoin="round" />
    )
}

/// The avatar disc centred on (`cx`, `cy`).
fn avatar(config: &YouTubeCommentHighlightConfig, cx: f32, cy: f32) -> Svgr<'static> {
    let r = AVATAR_D / 2.0;
    let picture = if config.avatar.is_empty() {
        fframes::svgr!(<g></g>)
    } else {
        // The circle is a clip because it rounds a picture.
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id="youtube-comment-highlight-avatar">
                        <circle cx={cx} cy={cy} r={r} />
                    </clipPath>
                </defs>
                <image href={config.avatar.clone()} x={cx - r} y={cy - r}
                       width={AVATAR_D} height={AVATAR_D}
                       preserveAspectRatio="xMidYMid slice"
                       clip-path="url(#youtube-comment-highlight-avatar)" />
            </g>
        )
    };
    fframes::svgr!(
        <g>
            <circle cx={cx} cy={cy} r={r} fill={AVATAR_FILL} />
            {picture}
            <circle cx={cx} cy={cy} r={r - 0.5} fill="none" stroke="#ffffff"
                    stroke-opacity={AVATAR_BORDER_OPACITY} stroke-width={1.0} />
        </g>
    )
}

impl Component for YouTubeCommentHighlight {
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

        // The slide, then the tilt about the card's centre.
        let drop = slide_drop(scale, ctx.height());
        let slide = format!(
            "translate(0 {})",
            slide_offset(frame, self.config.duration, drop)
        );
        let (cx, cy) = (CARD_W / 2.0, CARD_TOP + CARD_H / 2.0);
        let tilt = format!(
            "translate({cx} {cy}) scale({} 1) translate({} {})",
            tilt_scale_x(tilt_degrees(frame)),
            -cx,
            -cy
        );

        // Lay the column out as a grid row centred in the padded card.
        let lines = wrap_words(&config.comment, column_width(), |s| {
            text::tracked_width_at(
                ctx,
                &family,
                COMMENT_WEIGHT,
                COMMENT_SIZE,
                s,
                COMMENT_TRACKING,
            )
        });
        let handle_h = HANDLE_SIZE * HANDLE_LINE;
        let line_h = COMMENT_SIZE * COMMENT_LINE;
        let actions_h = ICON_SIZE;
        let content_h = handle_h
            + COMMENT_MARGIN_TOP
            + lines.len() as f32 * line_h
            + ACTIONS_MARGIN_TOP
            + actions_h;
        let inner_top = CARD_TOP + CARD_BORDER + PAD_Y;
        let inner_h = CARD_H - 2.0 * CARD_BORDER - 2.0 * PAD_Y;
        let top = inner_top + (inner_h - content_h).max(0.0) / 2.0;

        let avatar_x = CARD_BORDER + PAD_X;
        let text_x = avatar_x + AVATAR_D + COLUMN_GAP;

        let handle_base =
            top + ctx.baseline_offset_at(&family, HANDLE_WEIGHT, HANDLE_SIZE, handle_h);
        let handle = text::text_node_left(
            &config.handle,
            text_x,
            handle_base,
            family.clone(),
            HANDLE_SIZE,
            HANDLE_WEIGHT,
            0.0,
            HANDLE_INK.to_owned(),
            1.0,
        );

        let comment_top = top + handle_h + COMMENT_MARGIN_TOP;
        let comment_offset = ctx.baseline_offset_at(&family, COMMENT_WEIGHT, COMMENT_SIZE, line_h);
        let comment: Vec<Svgr<'static>> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                text::text_node_left(
                    line,
                    text_x,
                    comment_top + i as f32 * line_h + comment_offset,
                    family.clone(),
                    COMMENT_SIZE,
                    COMMENT_WEIGHT,
                    COMMENT_TRACKING,
                    COMMENT_INK.to_owned(),
                    1.0,
                )
            })
            .collect();

        let actions_top = comment_top + lines.len() as f32 * line_h + ACTIONS_MARGIN_TOP;
        let actions_cy = actions_top + actions_h / 2.0;
        let count_x = text_x + ICON_SIZE + LIKE_GAP;
        let count_w = text::tracked_width_at(
            ctx,
            &family,
            ACTIONS_WEIGHT,
            ACTIONS_SIZE,
            &config.likes,
            0.0,
        );
        let count_base = actions_cy - ACTIONS_SIZE / 2.0
            + ctx.baseline_offset_at(&family, ACTIONS_WEIGHT, ACTIONS_SIZE, ACTIONS_SIZE);
        let count = text::text_node_left(
            &config.likes,
            count_x,
            count_base,
            family.clone(),
            ACTIONS_SIZE,
            ACTIONS_WEIGHT,
            0.0,
            ACTIONS_INK.to_owned(),
            1.0,
        );
        let dislike_x = count_x + count_w + ACTIONS_GAP;

        fframes::svgr!(
            <g transform={fit}>
                <g transform={slide}>
                    <g transform={tilt}>
                        {card_shadow()}
                        <rect x={CARD_BORDER / 2.0} y={CARD_TOP + CARD_BORDER / 2.0}
                              width={CARD_W - CARD_BORDER} height={CARD_H - CARD_BORDER}
                              rx={CARD_RADIUS - CARD_BORDER / 2.0} fill={CARD_FILL}
                              stroke="#ffffff" stroke-opacity={CARD_BORDER_OPACITY}
                              stroke-width={CARD_BORDER} />
                        {avatar(config, avatar_x + AVATAR_D / 2.0, top + AVATAR_D / 2.0)}
                        {handle}
                        {Svgr::from(comment)}
                        {thumb(text_x, actions_cy - ICON_SIZE / 2.0, 0.0)}
                        {count}
                        {thumb(dislike_x, actions_cy - ICON_SIZE / 2.0, 180.0)}
                    </g>
                </g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meter::TextMeter;

    /// The embedded Inter Medium face, as the comment is drawn.
    fn comment_meter() -> TextMeter {
        let media = crate::ComponentsMedia::new().expect("embedded media");
        TextMeter::new("Inter Medium", media.inter_medium_ttf.to_vec()).expect("Inter parses")
    }

    fn measure(meter: &TextMeter, s: &str) -> f32 {
        meter.text_width(s, COMMENT_SIZE) + (s.chars().count() as f32 - 1.0) * COMMENT_TRACKING
    }

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = YouTubeCommentHighlightConfig::default();
        assert_eq!(config.handle, "@Bilaal0x");
        assert!(config.comment.contains("Easiest component library"));
        assert_eq!(config.likes, "2.4K");
        assert_eq!(config.avatar, "avatar1.png");
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 300);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = YouTubeCommentHighlight::default();
        assert_eq!(component.meta().id, "youtube-comment-highlight");
        assert_eq!(
            YouTubeCommentHighlight::meta_static().feature,
            "youtube-comment-highlight"
        );
        assert_eq!(component.duration(), 300);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn the_card_slides_in_holds_and_slides_out() {
        let n = 300;
        assert_eq!(slide_offset(0.0, n, 760.0), 760.0);
        assert!(slide_offset(8.0, n, 760.0) > 0.0 && slide_offset(8.0, n, 760.0) < 760.0);
        assert!(slide_offset(48.0, n, 760.0).abs() < 1.0);
        assert_eq!(slide_offset(150.0, n, 760.0), 0.0);
        assert_eq!(slide_offset(251.0, n, 760.0), 0.0);
        assert!(slide_offset(295.0, n, 760.0) > 0.0);
        assert_eq!(slide_offset(299.0, n, 760.0), 760.0);
        assert_eq!(slide_offset(400.0, n, 760.0), 760.0);
    }

    #[test]
    fn the_slide_in_overshoots_like_the_underdamped_spring() {
        let peak = (0..48)
            .map(|f| slide_offset(f as f32, 300, 760.0))
            .fold(f32::MAX, f32::min);
        assert!(peak < 0.0, "the spring should overshoot rest, got {peak}");
    }

    #[test]
    fn the_tilt_hits_the_keyframes_and_clamps() {
        assert!((tilt_degrees(0.0) + 10.0).abs() < 1e-4);
        assert!((tilt_degrees(90.0) - 5.0).abs() < 1e-4);
        assert!((tilt_degrees(180.0) + 5.0).abs() < 1e-4);
        assert!((tilt_degrees(270.0) - 5.0).abs() < 1e-4);
        assert!((tilt_degrees(-20.0) + 10.0).abs() < 1e-4);
        assert!((tilt_degrees(500.0) - 5.0).abs() < 1e-4);
        // The sine ease puts the midpoint of a segment halfway.
        assert!((tilt_degrees(45.0) + 2.5).abs() < 1e-4);
    }

    #[test]
    fn a_tilt_squeezes_the_card_a_little() {
        assert!((tilt_scale_x(0.0) - 1.0).abs() < 1e-6);
        assert!(tilt_scale_x(10.0) < 1.0 && tilt_scale_x(10.0) > 0.98);
        assert_eq!(tilt_scale_x(10.0), tilt_scale_x(-10.0));
    }

    #[test]
    fn the_thumb_path_is_the_source_glyph() {
        assert!(THUMB_PATH.starts_with("M7 10v11H4V10Z"));
        assert!(THUMB_PATH.ends_with("15.6 21Z"));
    }

    #[test]
    fn wrapping_breaks_on_words_and_keeps_every_word() {
        let lines = wrap_words("aa bb cc dd", 5.0, |s| s.len() as f32);
        assert_eq!(lines, ["aa bb", "cc dd"]);
        let long = wrap_words("aaaaaaaa bb", 3.0, |s| s.len() as f32);
        assert_eq!(long, ["aaaaaaaa", "bb"]);
        assert!(wrap_words("", 10.0, |s| s.len() as f32).is_empty());
    }

    #[test]
    fn the_default_comment_fits_the_card() {
        let meter = comment_meter();
        let config = YouTubeCommentHighlightConfig::default();
        let lines = wrap_words(&config.comment, column_width(), |s| measure(&meter, s));
        assert!(lines.len() <= 3, "wrapped to {} lines", lines.len());
        for line in &lines {
            assert!(measure(&meter, line) <= column_width());
        }
        let content = HANDLE_SIZE * HANDLE_LINE
            + COMMENT_MARGIN_TOP
            + lines.len() as f32 * COMMENT_SIZE * COMMENT_LINE
            + ACTIONS_MARGIN_TOP
            + ICON_SIZE;
        assert!(content <= CARD_H - 2.0 * CARD_BORDER - 2.0 * PAD_Y);
    }

    #[test]
    fn the_scene_fits_landscape_and_portrait_canvases() {
        let landscape = scene_scale(1920.0, 1080.0);
        assert!(SCENE_W * landscape <= 1920.0 && SCENE_H * landscape <= 1080.0);
        let portrait = scene_scale(1080.0, 1920.0);
        assert!((portrait - 0.9 * 1080.0 / 1120.0).abs() < 1e-6);
        assert!(SCENE_W * portrait <= 1080.0);
    }

    #[test]
    fn the_slide_clears_the_canvas_bottom() {
        for (w, h) in [(1920.0, 1080.0), (1080.0, 1920.0)] {
            let scale = scene_scale(w, h);
            let drop = slide_drop(scale, h);
            assert!(drop >= SLIDE_DROP);
            let card_top = (h - SCENE_H * scale) / 2.0 + CARD_TOP * scale;
            assert!(card_top + drop * scale >= h - 1e-3, "{w}x{h}");
        }
        assert_eq!(slide_drop(scene_scale(1920.0, 1080.0), 1080.0), SLIDE_DROP);
    }
}
