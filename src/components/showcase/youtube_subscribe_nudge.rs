//! A channel card whose Subscribe button is clicked by a cursor, then the bell is rung.
//!
//! Ported from Remotion Elements' `YouTubeSubscribeNudge`. The source lays the card out with flex
//! boxes; here every box is resolved to fixed scene coordinates in the source's own 760 x 240 px
//! box, which one group then fits to the canvas.
//!
//! Deliberate differences from the source:
//!
//! - **No audio.** The click and the bell chime are dropped: a components video carries no audio.
//! - **Name weight.** The channel name is drawn in Inter 700, not 800: only Inter 400-700 is
//!   embedded.
//! - **Avatar.** The default picture is the embedded `avatar1.png`, not the remote Remotion logo.
//! - **Duration.** The source reads `durationInFrames` from its host and has no fixed length, so
//!   the default here is 150 frames.
//! - **Shadows.** SVG filters are not used in this crate, so the card's `box-shadow` and the
//!   cursor's `drop-shadow` are stacked translucent copies.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::{CubicBezier, Spring, clamp01, ease_in_out_cubic, lerp};
use crate::ui::text;

/// The scene's width, in source px.
pub const SCENE_W: f32 = 760.0;
/// The scene's height, in source px.
pub const SCENE_H: f32 = 240.0;
/// The widest the scene may get, as a share of the canvas width.
pub const MAX_WIDTH_SHARE: f32 = 0.9;

/// The card's top edge, in scene y.
pub const CARD_TOP: f32 = 48.0;
/// The card's height.
pub const CARD_H: f32 = 144.0;
/// The card's corner radius.
pub const CARD_RADIUS: f32 = 26.0;
/// The card's fill.
pub const CARD_FILL: &str = "#15161a";
/// The card's horizontal padding.
pub const CARD_PAD: f32 = 24.0;
/// The scene y every row in the card is centred on.
pub const CENTER_Y: f32 = CARD_TOP + CARD_H / 2.0;
/// The page behind the card: the source has none, so a light neutral lets the dark card read.
pub const BACKGROUND: &str = "#f1f1f3";

/// The avatar's diameter.
pub const AVATAR_D: f32 = 88.0;
/// The gap between the avatar and the text column.
pub const AVATAR_GAP: f32 = 18.0;
/// Where the text column starts, in scene x.
pub const TEXT_X: f32 = CARD_PAD + AVATAR_D + AVATAR_GAP;
/// The gap between the text column and the button; a longer name is cut with an ellipsis.
pub const TEXT_GAP: f32 = 32.0;

/// The channel name's size.
pub const NAME_SIZE: f32 = 34.0;
/// The channel name's line height, as a multiple of its size.
pub const NAME_LINE: f32 = 1.1;
/// The channel name's weight (the source's 800, clamped to the heaviest embedded Inter).
pub const NAME_WEIGHT: u16 = 700;
/// The handle's size.
pub const HANDLE_SIZE: f32 = 21.0;
/// The handle's line height, as a multiple of its size.
pub const HANDLE_LINE: f32 = 1.15;
/// The handle's weight.
pub const HANDLE_WEIGHT: u16 = 600;
/// The handle's colour.
pub const HANDLE_FILL: &str = "#a9abb3";
/// The gap between the name and the handle.
pub const NAME_HANDLE_GAP: f32 = 6.0;

/// The bell's diameter.
pub const BELL_D: f32 = 64.0;
/// The bell's left edge: its right edge sits on the card's padding.
pub const BELL_X: f32 = SCENE_W - CARD_PAD - BELL_D;
/// The bell icon's size.
pub const BELL_ICON: f32 = 36.0;
/// The bell's fill before the click lands.
pub const BELL_FILL: &str = "#24252b";
/// The bell's fill once it is pressed.
pub const BELL_FILL_ON: &str = "#303137";
/// The bell icon's colour.
pub const BELL_INK: &str = "#f4f4f5";
/// The outline bell's body.
pub const BELL_BODY: &str =
    "M5.5 17h13c-1.7-1.9-2.2-3.9-2.2-7 0-2.8-1.7-5-4.3-5s-4.3 2.2-4.3 5c0 3.1-.5 5.1-2.2 7Z";
/// The outline bell's clapper.
pub const BELL_CLAPPER: &str = "M9.8 20c.5 1 1.2 1.5 2.2 1.5s1.7-.5 2.2-1.5";
/// The filled, ringing bell.
pub const BELL_RINGING: &str = "M19.395 1.196a1 1 0 0 0-.199 1.4A9 9 0 0 1 21 8a1 1 0 0 0 2 0 11 11 0 0 0-2.205-6.605 1 1 0 0 0-1.4-.199Zm-16.192.2A11 11 0 0 0 1 8a1 1 0 0 0 2 0 9 9 0 0 1 1.803-5.404 1 1 0 0 0-1.6-1.2ZM12 1a7 7 0 0 0-7 7v4.446a1 1 0 0 1-.144.515L3.05 15.972C2.25 17.305 3.21 19 4.766 19H8a4 4 0 1 0 8 0h3.233c1.555 0 2.515-1.695 1.715-3.029l-1.805-3.01a1 1 0 0 1-.143-.515V8a7 7 0 0 0-7-7Zm0 2a5 5 0 0 1 5 5v4.445a3 3 0 0 0 .428 1.545L19.233 17H4.766l1.806-3.01c.28-.466.428-1 .428-1.544V8a5 5 0 0 1 5-5Zm-2 16h4a2 2 0 0 1-4 0Z";
/// The frame the bell turns from outline to filled.
pub const BELL_SWAP: f32 = 84.0;
/// The frame the bell's fill brightens.
pub const BELL_LIT: f32 = 81.0;
/// Where the bell icon pivots, as a share of its height from the top.
pub const BELL_PIVOT_Y: f32 = 0.2;
/// The bell's press: keys.
pub const BELL_PRESS_KEYS: [f32; 3] = [78.0, 81.0, 93.0];
/// The bell's press scales at [`BELL_PRESS_KEYS`].
pub const BELL_PRESS_SCALE: [f32; 3] = [1.0, 0.9, 1.0];
/// The spring the bell settles on.
pub const BELL_SPRING: Spring = Spring {
    damping: 7.0,
    mass: 0.55,
    stiffness: 160.0,
};
/// The bell's wobble: keys.
pub const BELL_ROCK_KEYS: [f32; 6] = [81.0, 85.0, 89.0, 93.0, 97.0, 101.0];
/// The bell's wobble angles at [`BELL_ROCK_KEYS`], in degrees.
pub const BELL_ROCK_DEG: [f32; 6] = [0.0, -18.0, 16.0, -11.0, 7.0, 0.0];

/// The button's width.
pub const BUTTON_W: f32 = 200.0;
/// The button's height.
pub const BUTTON_H: f32 = 62.0;
/// The button's corner radius.
pub const BUTTON_RADIUS: f32 = 14.0;
/// The gap between the button and the bell.
pub const BUTTON_GAP: f32 = 16.0;
/// The button's left edge, in scene x.
pub const BUTTON_X: f32 = BELL_X - BUTTON_GAP - BUTTON_W;
/// The button label's size.
pub const BUTTON_SIZE: f32 = 26.0;
/// The button label's weight.
pub const BUTTON_WEIGHT: u16 = 700;
/// The button label's tracking, in px.
pub const BUTTON_TRACKING: f32 = -0.25;
/// The Subscribe button's red.
pub const SUBSCRIBE_FILL: &str = "#ff1744";
/// The Subscribed button's grey.
pub const SUBSCRIBED_FILL: &str = "#2a2b31";
/// The check's size.
pub const CHECK_SIZE: f32 = 26.0;
/// The gap between the check and the label.
pub const CHECK_GAP: f32 = 10.0;
/// The check's path, in a 24 px box.
pub const CHECK_PATH: &str = "m4 12.5 5 4.8L20 7";
/// The frame the buttons cross-fade from; the fade takes one frame.
pub const SWAP_AT: f32 = 63.0;
/// The button's press: keys.
pub const PRESS_KEYS: [f32; 3] = [58.0, 61.0, 68.0];
/// The button's press scales at [`PRESS_KEYS`].
pub const PRESS_SCALE: [f32; 3] = [1.0, 0.94, 1.0];
/// The spring the button settles on.
pub const PRESS_SPRING: Spring = Spring {
    damping: 9.0,
    mass: 0.45,
    stiffness: 180.0,
};

/// The cursor's path, in a 48 x 54 box.
pub const CURSOR_PATH: &str = "M5 3 39 35l-16 2 9 13-8 5-9-14L5 50Z";
/// The cursor's flight: keys.
pub const CURSOR_KEYS: [f32; 4] = [40.0, 60.0, 66.0, 78.0];
/// The cursor box's left edge at [`CURSOR_KEYS`].
pub const CURSOR_LEFT: [f32; 4] = [690.0, 620.0, 620.0, 699.0];
/// The cursor box's top edge at [`CURSOR_KEYS`].
pub const CURSOR_TOP: [f32; 4] = [202.0, 124.0, 124.0, 117.0];
/// The cursor's fade: keys.
pub const CURSOR_FADE_KEYS: [f32; 4] = [38.0, 43.0, 84.0, 91.0];

/// How long the card's entrance takes, in frames.
pub const ENTER_FRAMES: f32 = 24.0;
/// How long its exit takes, in frames.
pub const EXIT_FRAMES: f32 = 16.0;
/// The scale the card enters from.
pub const ENTER_SCALE: f32 = 0.98;
/// The scale the card leaves to.
pub const EXIT_SCALE: f32 = 0.97;
/// How far below its place the card enters from, in scene px.
pub const ENTER_DROP: f32 = 14.0;
/// How far below its place the card leaves to, in scene px.
pub const EXIT_DROP: f32 = 20.0;

/// The ease the card enters on.
pub const ENTER_EASE: CubicBezier = CubicBezier::new(0.65, 0.0, 0.35, 1.0);
/// The ease the card leaves on.
pub const EXIT_EASE: CubicBezier = CubicBezier::new(0.7, 0.0, 0.84, 0.0);
/// The ease a press sinks on.
pub const PRESS_IN: CubicBezier = CubicBezier::new(0.4, 0.0, 1.0, 1.0);
/// The ease the cursor lands on.
pub const CURSOR_LAND: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// The ease the cursor glides to the bell on.
pub const CURSOR_GLIDE: CubicBezier = CubicBezier::new(0.65, 0.0, 0.35, 1.0);

/// How the value moves across one segment of a [`segmented`] track.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
    /// Straight line.
    Linear,
    /// A CSS cubic Bézier.
    Bezier(CubicBezier),
    /// `Easing.inOut(Easing.cubic)`.
    InOutCubic,
    /// `Easing.spring`: the spring sampled over the segment's own length in frames.
    Spring(Spring),
}

impl Ease {
    /// Progress through a segment `frames` long, at `t` in `0..=1`.
    fn at(self, t: f32, frames: f32) -> f32 {
        match self {
            Self::Linear => t,
            Self::Bezier(curve) => curve.ease(t),
            Self::InOutCubic => ease_in_out_cubic(t),
            // As in Remotion, the spring is pinned to rest at the segment's end.
            Self::Spring(spring) => {
                if t >= 1.0 {
                    1.0
                } else {
                    spring.value(t * frames)
                }
            }
        }
    }
}

/// Remotion's `interpolate` with a per-segment ease, clamped on both sides.
///
/// [`crate::motion::interpolate_multi`] is linear only; the source eases every segment on its own.
/// When `eases` is shorter than the segment count, its last entry is reused.
#[must_use]
pub fn segmented(frame: f32, keys: &[f32], values: &[f32], eases: &[Ease]) -> f32 {
    let last = keys.len() - 1;
    if frame <= keys[0] {
        return values[0];
    }
    if frame >= keys[last] {
        return values[last];
    }
    let mut index = 0;
    while frame >= keys[index + 1] {
        index += 1;
    }
    let frames = keys[index + 1] - keys[index];
    let t = (frame - keys[index]) / frames;
    let ease = eases[index.min(eases.len() - 1)];
    lerp(values[index], values[index + 1], ease.at(t, frames))
}

/// [`segmented`] with Remotion's `output: 'perceptual-scale'`: the values blend in log space.
#[must_use]
pub fn segmented_scale(frame: f32, keys: &[f32], values: &[f32], eases: &[Ease]) -> f32 {
    let logs: Vec<f32> = values.iter().map(|value| value.ln()).collect();
    segmented(frame, keys, &logs, eases).exp()
}

/// The entrance/exit keys for a scene `duration` frames long.
fn edge_keys(duration: usize) -> [f32; 4] {
    let d = duration as f32;
    [0.0, ENTER_FRAMES, d - EXIT_FRAMES, d - 1.0]
}

/// The entrance/exit eases.
fn edge_eases() -> [Ease; 3] {
    [
        Ease::Bezier(ENTER_EASE),
        Ease::Linear,
        Ease::Bezier(EXIT_EASE),
    ]
}

/// The card's opacity at `frame`: in over the first 24 frames, out over the last 16.
#[must_use]
pub fn edge_opacity(frame: f32, duration: usize) -> f32 {
    segmented(
        frame,
        &edge_keys(duration),
        &[0.0, 1.0, 1.0, 0.0],
        &edge_eases(),
    )
}

/// The card's scale at `frame`, about the scene's centre.
#[must_use]
pub fn edge_scale(frame: f32, duration: usize) -> f32 {
    segmented_scale(
        frame,
        &edge_keys(duration),
        &[ENTER_SCALE, 1.0, 1.0, EXIT_SCALE],
        &edge_eases(),
    )
}

/// How far below its place the card sits at `frame`, in scene px.
#[must_use]
pub fn edge_drop(frame: f32, duration: usize) -> f32 {
    segmented(
        frame,
        &edge_keys(duration),
        &[ENTER_DROP, 0.0, 0.0, EXIT_DROP],
        &edge_eases(),
    )
}

/// How far the Subscribed button has replaced the Subscribe one: `0` through frame 63, `1` from 64.
#[must_use]
pub fn swap_mix(frame: f32) -> f32 {
    clamp01(frame - SWAP_AT)
}

/// The scale of the button at `frame`.
#[must_use]
pub fn button_scale(frame: f32) -> f32 {
    segmented_scale(
        frame,
        &PRESS_KEYS,
        &PRESS_SCALE,
        &[Ease::Bezier(PRESS_IN), Ease::Spring(PRESS_SPRING)],
    )
}

/// The scale of the bell at `frame`.
#[must_use]
pub fn bell_scale(frame: f32) -> f32 {
    segmented_scale(
        frame,
        &BELL_PRESS_KEYS,
        &BELL_PRESS_SCALE,
        &[Ease::Bezier(PRESS_IN), Ease::Spring(BELL_SPRING)],
    )
}

/// The bell icon's rotation at `frame`, in degrees.
#[must_use]
pub fn bell_rotation(frame: f32) -> f32 {
    segmented(frame, &BELL_ROCK_KEYS, &BELL_ROCK_DEG, &[Ease::InOutCubic])
}

/// Whether the bell has swapped to the filled, ringing icon at `frame`.
#[must_use]
pub fn bell_is_ringing(frame: f32) -> bool {
    frame >= BELL_SWAP
}

/// The cursor box's top-left corner at `frame`, in scene px.
#[must_use]
pub fn cursor_at(frame: f32) -> (f32, f32) {
    let eases = [
        Ease::Bezier(CURSOR_LAND),
        Ease::Linear,
        Ease::Bezier(CURSOR_GLIDE),
    ];
    (
        segmented(frame, &CURSOR_KEYS, &CURSOR_LEFT, &eases),
        segmented(frame, &CURSOR_KEYS, &CURSOR_TOP, &eases),
    )
}

/// The cursor's opacity at `frame`.
#[must_use]
pub fn cursor_opacity(frame: f32) -> f32 {
    segmented(
        frame,
        &CURSOR_FADE_KEYS,
        &[0.0, 1.0, 1.0, 0.0],
        &[Ease::Linear],
    )
}

/// The factor that fits the scene to a `canvas_width` px wide canvas at collection `unit`.
///
/// `unit` is `1.5` on 1080p; the width cap keeps the scene inside a portrait canvas.
#[must_use]
pub fn scene_scale(unit: f32, canvas_width: f32) -> f32 {
    unit.min(MAX_WIDTH_SHARE * canvas_width / SCENE_W)
}

/// `value` cut with an ellipsis until `measure` says it fits `max_width`.
///
/// `measure` is a run's width in px; the result is empty when not even `…` fits.
#[must_use]
pub fn truncate_to(value: &str, max_width: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(value) <= max_width {
        return value.to_owned();
    }
    let mut chars: Vec<char> = value.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let kept: String = chars.iter().collect();
        let candidate = format!("{}…", kept.trim_end());
        if measure(&candidate) <= max_width {
            return candidate;
        }
    }
    String::new()
}

/// Configuration for [`YouTubeSubscribeNudge`].
#[derive(Debug, Clone, PartialEq)]
pub struct YouTubeSubscribeNudgeConfig {
    /// The channel's name.
    pub channel_name: String,
    /// The channel's handle.
    pub handle: String,
    /// The red button's label.
    pub subscribe_label: String,
    /// The grey button's label, after the click.
    pub subscribed_label: String,
    /// The avatar, as a name in the host's media provider. Empty draws a monogram circle.
    pub avatar: String,
    /// The face to paint in. `None` is Inter.
    pub font_family: Option<String>,
    /// Composition length in frames.
    pub duration: usize,
}

impl Default for YouTubeSubscribeNudgeConfig {
    fn default() -> Self {
        Self {
            channel_name: "Bilaal0x".to_owned(),
            handle: "@bilaal0x".to_owned(),
            subscribe_label: "Subscribe".to_owned(),
            subscribed_label: "Subscribed".to_owned(),
            avatar: "avatar1.png".to_owned(),
            font_family: None,
            duration: 150,
        }
    }
}

/// The **YouTube Subscribe Nudge** component.
#[derive(Debug, Clone, Default)]
pub struct YouTubeSubscribeNudge {
    /// Component options.
    pub config: YouTubeSubscribeNudgeConfig,
}

impl YouTubeSubscribeNudge {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: YouTubeSubscribeNudgeConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "youtube-subscribe-nudge",
            name: "YouTube Subscribe Nudge",
            summary: "A cursor clicks a channel card's Subscribe button, then rings its bell",
            feature: "youtube-subscribe-nudge",
        }
    }
}

/// Wrap a node in a scale about a point.
fn scaled_about(node: Svgr<'static>, center_x: f32, center_y: f32, scale: f32) -> Svgr<'static> {
    let transform = format!(
        "translate({center_x} {center_y}) scale({scale}) translate({} {})",
        -center_x, -center_y
    );
    fframes::svgr!(<g transform={transform}>{node}</g>)
}

/// The card's drop shadow: a few translucent rounded rects, widest and faintest first.
fn card_shadow() -> Svgr<'static> {
    let mut layers: Vec<Svgr<'static>> = Vec::new();
    for step in 0..10 {
        let grow = -8.0 + step as f32 * 3.0;
        layers.push(fframes::svgr!(
            <rect x={-grow} y={CARD_TOP + 18.0 - grow} width={SCENE_W + 2.0 * grow}
                  height={CARD_H + 2.0 * grow} rx={CARD_RADIUS + grow.max(0.0)}
                  fill="#000000" opacity={0.028} />
        ));
    }
    Svgr::from(layers)
}

/// The avatar: a white disc with a hairline border, holding the picture or a monogram.
fn avatar(
    ctx: &RenderCtx<'_>,
    config: &YouTubeSubscribeNudgeConfig,
    family: &str,
) -> Svgr<'static> {
    let cx = CARD_PAD + AVATAR_D / 2.0;
    let r = AVATAR_D / 2.0;
    let picture = if config.avatar.is_empty() {
        let initial: String = config.channel_name.chars().take(1).collect();
        let size = 44.0;
        fframes::svgr!(
            <g>
                <circle cx={cx} cy={CENTER_Y} r={r} fill={SUBSCRIBE_FILL} />
                {text::text_node(
                    ctx,
                    &initial.to_uppercase(),
                    cx,
                    text::baseline_for_centred_box(ctx, family, size, CENTER_Y),
                    family.to_owned(),
                    size,
                    NAME_WEIGHT,
                    0.0,
                    "#ffffff".to_owned(),
                    1.0,
                )}
            </g>
        )
    } else {
        // `object-fit: cover` is `slice`; the circle is a clip because it rounds a picture.
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id="youtube-subscribe-nudge-avatar">
                        <circle cx={cx} cy={CENTER_Y} r={r - 1.0} />
                    </clipPath>
                </defs>
                <circle cx={cx} cy={CENTER_Y} r={r} fill="#ffffff" />
                <image href={config.avatar.clone()} x={cx - r} y={CENTER_Y - r}
                       width={AVATAR_D} height={AVATAR_D}
                       preserveAspectRatio="xMidYMid slice"
                       clip-path="url(#youtube-subscribe-nudge-avatar)" />
            </g>
        )
    };
    fframes::svgr!(
        <g>
            {picture}
            <circle cx={cx} cy={CENTER_Y} r={r - 0.5} fill="none" stroke="#ffffff"
                    stroke-opacity={0.12} stroke-width={1.0} />
        </g>
    )
}

/// The channel name and handle, stacked and centred on the card's middle.
fn text_column(
    ctx: &RenderCtx<'_>,
    config: &YouTubeSubscribeNudgeConfig,
    family: &str,
) -> Svgr<'static> {
    let room = BUTTON_X - TEXT_GAP - TEXT_X;
    let name_box = NAME_SIZE * NAME_LINE;
    let handle_box = HANDLE_SIZE * HANDLE_LINE;
    let top = CENTER_Y - (name_box + NAME_HANDLE_GAP + handle_box) / 2.0;

    let name = truncate_to(&config.channel_name, room, |run| {
        text::tracked_width_at(ctx, family, NAME_WEIGHT, NAME_SIZE, run, 0.0)
    });
    let handle = truncate_to(&config.handle, room, |run| {
        text::tracked_width_at(ctx, family, HANDLE_WEIGHT, HANDLE_SIZE, run, 0.0)
    });

    fframes::svgr!(
        <g>
            {text::text_node_left(
                &name,
                TEXT_X,
                text::baseline_for_centred_box(ctx, family, NAME_SIZE, top + name_box / 2.0),
                family.to_owned(),
                NAME_SIZE,
                NAME_WEIGHT,
                0.0,
                "#ffffff".to_owned(),
                1.0,
            )}
            {text::text_node_left(
                &handle,
                TEXT_X,
                text::baseline_for_centred_box(
                    ctx,
                    family,
                    HANDLE_SIZE,
                    top + name_box + NAME_HANDLE_GAP + handle_box / 2.0,
                ),
                family.to_owned(),
                HANDLE_SIZE,
                HANDLE_WEIGHT,
                0.0,
                HANDLE_FILL.to_owned(),
                1.0,
            )}
        </g>
    )
}

/// The two stacked buttons, cross-fading at [`SWAP_AT`], pressed together.
fn button(
    ctx: &RenderCtx<'_>,
    config: &YouTubeSubscribeNudgeConfig,
    family: &str,
    frame: f32,
) -> Svgr<'static> {
    let mix = swap_mix(frame);
    let center_x = BUTTON_X + BUTTON_W / 2.0;
    let top = CENTER_Y - BUTTON_H / 2.0;
    let baseline = text::baseline_for_centred_box(ctx, family, BUTTON_SIZE, CENTER_Y);

    let mut layers: Vec<Svgr<'static>> = Vec::new();
    if mix < 1.0 {
        layers.push(fframes::svgr!(
            <g opacity={1.0 - mix}>
                <rect x={BUTTON_X} y={top} width={BUTTON_W} height={BUTTON_H}
                      rx={BUTTON_RADIUS} fill={SUBSCRIBE_FILL} />
                {text::text_node(
                    ctx,
                    &config.subscribe_label,
                    center_x,
                    baseline,
                    family.to_owned(),
                    BUTTON_SIZE,
                    BUTTON_WEIGHT,
                    BUTTON_TRACKING,
                    "#ffffff".to_owned(),
                    1.0,
                )}
            </g>
        ));
    }
    if mix > 0.0 {
        // The check and the label are one centred group, with the check dropped 1 px.
        let label = text::tracked_width_at(
            ctx,
            family,
            BUTTON_WEIGHT,
            BUTTON_SIZE,
            &config.subscribed_label,
            BUTTON_TRACKING,
        );
        let left = center_x - (CHECK_SIZE + CHECK_GAP + label) / 2.0;
        let check = format!(
            "translate({left} {}) scale({})",
            CENTER_Y - CHECK_SIZE / 2.0 + 1.0,
            CHECK_SIZE / 24.0
        );
        layers.push(fframes::svgr!(
            <g opacity={mix}>
                <rect x={BUTTON_X + 0.5} y={top + 0.5} width={BUTTON_W - 1.0}
                      height={BUTTON_H - 1.0} rx={BUTTON_RADIUS - 0.5} fill={SUBSCRIBED_FILL}
                      stroke="#ffffff" stroke-opacity={0.1} stroke-width={1.0} />
                <path transform={check} d={CHECK_PATH.to_owned()} fill="none" stroke="#ffffff"
                      stroke-width={3.0} stroke-linecap="round" stroke-linejoin="round" />
                {text::text_node_left(
                    &config.subscribed_label,
                    left + CHECK_SIZE + CHECK_GAP,
                    baseline,
                    family.to_owned(),
                    BUTTON_SIZE,
                    BUTTON_WEIGHT,
                    BUTTON_TRACKING,
                    "#ffffff".to_owned(),
                    1.0,
                )}
            </g>
        ));
    }
    scaled_about(Svgr::from(layers), center_x, CENTER_Y, button_scale(frame))
}

/// The notification bell: a disc, pressed at frame 78, with its icon swapping and rocking.
fn bell(frame: f32) -> Svgr<'static> {
    let cx = BELL_X + BELL_D / 2.0;
    let fill = if frame >= BELL_LIT {
        BELL_FILL_ON
    } else {
        BELL_FILL
    };
    let icon = if bell_is_ringing(frame) {
        fframes::svgr!(<path d={BELL_RINGING.to_owned()} fill={BELL_INK} />)
    } else {
        fframes::svgr!(
            <g fill="none" stroke={BELL_INK} stroke-width={1.8} stroke-linecap="round"
               stroke-linejoin="round">
                <path d={BELL_BODY.to_owned()} />
                <path d={BELL_CLAPPER.to_owned()} />
            </g>
        )
    };
    let place = format!(
        "translate({} {}) scale({})",
        cx - BELL_ICON / 2.0,
        CENTER_Y - BELL_ICON / 2.0,
        BELL_ICON / 24.0
    );
    // The rotation pivots at (50%, 20%) of the icon, in the icon's own 24 px units.
    let rock = format!(
        "rotate({} 12 {})",
        bell_rotation(frame),
        24.0 * BELL_PIVOT_Y
    );
    let node = fframes::svgr!(
        <g>
            <circle cx={cx} cy={CENTER_Y} r={BELL_D / 2.0} fill={fill} />
            <circle cx={cx} cy={CENTER_Y} r={BELL_D / 2.0 - 0.5} fill="none" stroke="#ffffff"
                    stroke-opacity={0.12} stroke-width={1.0} />
            <g transform={place}><g transform={rock}>{icon}</g></g>
        </g>
    );
    scaled_about(node, cx, CENTER_Y, bell_scale(frame))
}

/// The cursor with its soft shadow, as translucent offset copies of its path.
fn cursor(frame: f32) -> Option<Svgr<'static>> {
    let opacity = cursor_opacity(frame);
    if opacity <= 0.0 {
        return None;
    }
    let (left, top) = cursor_at(frame);
    Some(fframes::svgr!(
        <g transform={format!("translate({left} {top})")} opacity={opacity}>
            <path d={CURSOR_PATH.to_owned()} transform="translate(0 4)" fill="#000000"
                  fill-opacity={0.1} stroke="#000000" stroke-opacity={0.1} stroke-width={8.0}
                  stroke-linejoin="round" />
            <path d={CURSOR_PATH.to_owned()} transform="translate(0 4)" fill="#000000"
                  fill-opacity={0.16} />
            <path d={CURSOR_PATH.to_owned()} fill="#ffffff" stroke="#111217" stroke-width={3.0}
                  stroke-linejoin="round" />
        </g>
    ))
}

impl Component for YouTubeSubscribeNudge {
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
        let scale = scene_scale(text::unit(ctx), ctx.width());
        let fit = format!(
            "translate({} {}) scale({scale})",
            (ctx.width() - SCENE_W * scale) / 2.0,
            (ctx.height() - SCENE_H * scale) / 2.0
        );
        // The entrance and exit: a drop, then a scale about the box's centre.
        let edge = format!(
            "translate(0 {}) translate({} {}) scale({}) translate({} {})",
            edge_drop(frame, config.duration),
            SCENE_W / 2.0,
            SCENE_H / 2.0,
            edge_scale(frame, config.duration),
            -SCENE_W / 2.0,
            -SCENE_H / 2.0
        );
        let opacity = edge_opacity(frame, config.duration);

        let card = fframes::svgr!(
            <g>
                {card_shadow()}
                <rect x={0.5} y={CARD_TOP + 0.5} width={SCENE_W - 1.0} height={CARD_H - 1.0}
                      rx={CARD_RADIUS - 0.5} fill={CARD_FILL} stroke="#ffffff"
                      stroke-opacity={0.09} stroke-width={1.0} />
                {avatar(ctx, config, &family)}
                {text_column(ctx, config, &family)}
                {button(ctx, config, &family, frame)}
                {bell(frame)}
            </g>
        );
        let pointer = cursor(frame).unwrap_or_else(|| Svgr::from(Vec::<Svgr<'static>>::new()));

        fframes::svgr!(
            <g transform={fit}>
                <g transform={edge} opacity={opacity}>
                    {card}
                    {pointer}
                </g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_documented_config() {
        let config = YouTubeSubscribeNudgeConfig::default();
        assert_eq!(config.channel_name, "Bilaal0x");
        assert_eq!(config.handle, "@bilaal0x");
        assert_eq!(config.subscribe_label, "Subscribe");
        assert_eq!(config.subscribed_label, "Subscribed");
        assert_eq!(config.avatar, "avatar1.png");
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 150);
    }

    #[test]
    fn the_meta_names_the_component_and_its_feature() {
        let component = YouTubeSubscribeNudge::default();
        assert_eq!(component.meta().id, "youtube-subscribe-nudge");
        assert_eq!(
            YouTubeSubscribeNudge::meta_static().feature,
            "youtube-subscribe-nudge"
        );
        assert!(component.duration() > 0);
        assert_eq!(component.background(), Some(BACKGROUND));
    }

    #[test]
    fn the_layout_resolves_the_flex_row_right_to_left() {
        assert!((BELL_X - 672.0).abs() < 1e-6);
        assert!((BUTTON_X - 456.0).abs() < 1e-6);
        assert!((TEXT_X - 130.0).abs() < 1e-6);
        assert!((CENTER_Y - 120.0).abs() < 1e-6);
    }

    #[test]
    fn the_button_swaps_in_one_frame_at_63() {
        assert!(swap_mix(63.0).abs() < 1e-6);
        assert!((swap_mix(64.0) - 1.0).abs() < 1e-6);
        assert!((swap_mix(63.5) - 0.5).abs() < 1e-6);
        assert!(swap_mix(0.0).abs() < 1e-6);
    }

    #[test]
    fn the_button_presses_and_springs_back() {
        assert!((button_scale(58.0) - 1.0).abs() < 1e-6);
        assert!((button_scale(61.0) - 0.94).abs() < 1e-4);
        assert!((button_scale(68.0) - 1.0).abs() < 1e-6);
        assert!((button_scale(80.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_bell_swaps_icons_at_84_and_rocks_from_rest_to_rest() {
        assert!(!bell_is_ringing(83.0));
        assert!(bell_is_ringing(84.0));
        assert!(bell_rotation(81.0).abs() < 1e-6);
        assert!(bell_rotation(101.0).abs() < 1e-6);
        assert!((bell_rotation(85.0) + 18.0).abs() < 1e-4);
        assert!((bell_rotation(89.0) - 16.0).abs() < 1e-4);
        assert!(bell_rotation(60.0).abs() < 1e-6);
        assert!((bell_scale(81.0) - 0.9).abs() < 1e-4);
    }

    #[test]
    fn the_cursor_waits_over_the_button_then_leaves_for_the_bell() {
        for frame in 60..=66 {
            let (x, y) = cursor_at(frame as f32);
            assert!(
                (x - 620.0).abs() < 1e-3 && (y - 124.0).abs() < 1e-3,
                "{frame}"
            );
        }
        assert_eq!(cursor_at(40.0), (690.0, 202.0));
        assert_eq!(cursor_at(78.0), (699.0, 117.0));
        // The tip is inside the button.
        let (x, y) = (620.0 + 5.0, 124.0 + 3.0);
        assert!(x > BUTTON_X && x < BUTTON_X + BUTTON_W);
        assert!(y > CENTER_Y - BUTTON_H / 2.0 && y < CENTER_Y + BUTTON_H / 2.0);
    }

    #[test]
    fn the_cursor_is_invisible_before_38_and_after_91() {
        assert_eq!(cursor_opacity(0.0), 0.0);
        assert_eq!(cursor_opacity(38.0), 0.0);
        assert_eq!(cursor_opacity(50.0), 1.0);
        assert_eq!(cursor_opacity(84.0), 1.0);
        assert_eq!(cursor_opacity(91.0), 0.0);
        assert_eq!(cursor_opacity(120.0), 0.0);
        assert!(cursor(30.0).is_none() && cursor(100.0).is_none());
        assert!(cursor(60.0).is_some());
    }

    #[test]
    fn the_card_fades_in_and_out() {
        let d = YouTubeSubscribeNudgeConfig::default().duration;
        assert_eq!(edge_opacity(0.0, d), 0.0);
        assert_eq!(edge_opacity((d - 1) as f32, d), 0.0);
        assert_eq!(edge_opacity(75.0, d), 1.0);
        assert!((edge_scale(0.0, d) - ENTER_SCALE).abs() < 1e-5);
        assert!((edge_scale(75.0, d) - 1.0).abs() < 1e-6);
        assert!((edge_scale((d - 1) as f32, d) - EXIT_SCALE).abs() < 1e-5);
        assert!((edge_drop(0.0, d) - ENTER_DROP).abs() < 1e-5);
        assert!((edge_drop((d - 1) as f32, d) - EXIT_DROP).abs() < 1e-5);
    }

    #[test]
    fn the_scene_fits_landscape_and_portrait() {
        for (w, h) in [(1920.0_f32, 1080.0_f32), (1080.0, 1920.0)] {
            let unit = w.min(h) / 720.0;
            let scale = scene_scale(unit, w);
            assert!(SCENE_W * scale <= w, "{w}x{h}");
            assert!(SCENE_H * scale <= h, "{w}x{h}");
        }
        assert!((scene_scale(1.5, 1920.0) - 1.5).abs() < 1e-6);
    }

    #[test]
    fn truncation_never_exceeds_the_room() {
        let measure = |run: &str| run.chars().count() as f32 * 10.0;
        assert_eq!(truncate_to("Bilaal0x", 100.0, measure), "Bilaal0x");
        for room in [0.0, 5.0, 10.0, 35.0, 60.0, 79.0] {
            let cut = truncate_to("Bilaal0x", room, measure);
            assert!(measure(&cut) <= room, "{room}: {cut:?}");
        }
        assert_eq!(truncate_to("Bilaal0x", 60.0, measure), "Bilaa…");
    }

    #[test]
    fn a_spring_segment_rests_on_its_end_value() {
        let eases = [Ease::Spring(PRESS_SPRING)];
        let mid = segmented(3.0, &[0.0, 10.0], &[0.0, 1.0], &eases);
        assert!(mid > 0.0 && mid < 1.5);
        assert_eq!(segmented(10.0, &[0.0, 10.0], &[0.0, 1.0], &eases), 1.0);
    }
}
