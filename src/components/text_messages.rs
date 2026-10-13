//! # Text Messages
//!
//! A phone chat thread plays out message by message. The status bar, contact header and input bar
//! fade in over the first 14 frames while the "Today 6:42 PM" divider fades in over frames 8–22;
//! every incoming message is preceded by a "typing…" bubble whose three dots hop in sequence;
//! each bubble springs into the thread (32px rise, 0.82 → 1.0 scale, 6-frame fade); the last
//! message the user sent gets a "Delivered" receipt that fades in 32 frames after its bubble; and
//! the finished thread holds for 75 frames.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `messages` | `Vec<ChatMessage>` | the 8-message demo conversation | Conversation, in chronological order. |
//! | `contact_name` | `String` | `"Maya"` | Contact display name shown in the header. |
//! | `avatar_initials` | `Option<String>` | `None` | Initials in the avatar circle; `None` derives them from `contact_name`. |
//! | `typing_frames` | `usize` | `55` | Frames a "typing…" indicator shows before each incoming message. |
//! | `gap_frames` | `usize` | `20` | Frames between consecutive bubbles. |
//! | `start_delay` | `usize` | `30` | Frames before the conversation starts. |
//! | `anchor` | `Anchor` | `Anchor::Top` | Where the thread grows from. |
//! | `reduced_motion` | `bool` | `false` | Collapse the rise and the scale-up; keep the fades. |
//! | `fit_to_thread` | `bool` | `true` | Scale the whole conversation down (never up) so it fits between the header and the input bar. |
//! | `scale` | `f32` | `1.0` | Multiplies every source px metric. |
//!
//! ## Palette
//!
//! The source hard-codes the shadcn light theme: background `#faf9f6`, card `#ffffff`,
//! foreground `#141414`, primary `#3072db`, primary foreground `#ffffff`, muted `#f2f0eb`, muted
//! foreground `#6e6a63`, border `#d9d9d9`, online `#34c759`, avatar gradient `#3072db` → `#7c5cff`
//! at 135°, and typing dots at `#6e6a63` with 58% opacity.
//!
//! ## Rendering
//!
//! ```text
//! # the native aspect: the source composes at 1080x1920
//! cargo run --release -- --portrait --components text-messages render -o story.mp4
//!
//! # landscape also works; the component scales itself down to fit the shorter thread viewport
//! cargo run --release -- --components text-messages render -o out.mp4
//! ```
//!
//! ## Canvas
//!
//! This component was authored for 1080x1920, so `--portrait` is its native aspect: `vmin` is
//! `1080/100 = 10.8`, every source px metric comes out at exactly `1.0`, and no fit-down is
//! needed. On the landscape canvas the thread viewport is 737px instead of 1577px, so
//! `fit_to_thread` scales the layout by roughly `0.79` to keep the conversation on screen.
//!
//! ## Fidelity
//!
//! Every source px metric is scaled by `ctx.vmin() / 10.8`, which is exactly `1.0` whenever the
//! canvas' short edge is 1080 (both 1920x1080 and 1080x1920), and by `scale`. With
//! `fit_to_thread` on (the default) the layout is additionally scaled down by
//! `min(1, height / (stack + chrome))` so the conversation always fits; the fit never scales up,
//! so it is inert on the portrait canvas. SVG `<rect>` has one `rx`, so the bubbles' per-corner
//! "tail" radii are a uniform 26px, and the source's soft `box-shadow` is not drawn (this library
//! does not use SVG filters). See `text_messages.README.md` for the full list.
//!
//! ## Credit
//!
//! Ported from `TextMessages.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, layout_paragraph, text_node_centered, text_node_left};
use crate::motion::{SPRING_POP, ease_out_cubic, interpolate_multi, lerp, motion_scale};

/// Frames a bubble takes to spring into the thread.
pub const ENTRANCE_FRAMES: usize = 24;
/// Frames after a bubble's entrance before its "Delivered" receipt starts fading in.
pub const DELIVERED_LAG_FRAMES: usize = 8;
/// Frames the "Delivered" receipt takes to fade in.
pub const DELIVERED_FADE_FRAMES: usize = 12;
/// Frames the finished thread is held after the last bubble lands.
pub const END_HOLD_FRAMES: usize = 75;
/// `vmin` of the source's 1080x1920 canvas — the 1:1 reference for every source px value.
///
/// The source authors its metrics against a 1080-wide canvas (`min(1080, 1920) / 100 = 10.8`),
/// so multiplying every source px value by `ctx.vmin() / REFERENCE_VMIN` reproduces them
/// exactly on any canvas.
pub const REFERENCE_VMIN: f32 = 10.8;

/// The "Today" divider label above the first bubble.
const DIVIDER_LABEL: &str = "Today 6:42 PM";

// ─── Source metrics, in the source's own px (1080-wide canvas) ──────────────────────────────

const STATUS_HEIGHT: f32 = 64.0;
const STATUS_PAD_X: f32 = 48.0;
const STATUS_TIME_SIZE: f32 = 28.0;
const STATUS_TIME_WEIGHT: u16 = 600;
const STATUS_TIME_TRACKING: f32 = 0.28; // 0.01em at 28px
const STATUS_ICON_GAP: f32 = 14.0;

const HEADER_HEIGHT: f32 = 136.0;
const HEADER_PAD_X: f32 = 40.0;
const HEADER_GAP: f32 = 26.0;
const HEADER_BACK_W: f32 = 15.0;
const HEADER_BACK_H: f32 = 28.0;
const HEADER_AVATAR: f32 = 86.0;
const HEADER_AVATAR_INITIALS_SIZE: f32 = 34.0;
const HEADER_AVATAR_INITIALS_WEIGHT: u16 = 600;
const HEADER_ONLINE_DOT: f32 = 22.0;
const HEADER_ONLINE_BORDER: f32 = 4.0;
const HEADER_NAME_SIZE: f32 = 34.0;
const HEADER_NAME_WEIGHT: u16 = 600;
const HEADER_NAME_LINE_HEIGHT: f32 = 1.2;
const HEADER_STATUS_SIZE: f32 = 25.0;
const HEADER_STATUS_OFFSET_Y: f32 = 4.0;
const HEADER_MENU_DOT: f32 = 7.0;
const HEADER_MENU_GAP: f32 = 5.0;
const HEADER_MENU_PAD_RIGHT: f32 = 6.0;

const THREAD_PAD_X: f32 = 40.0;
const THREAD_PAD_TOP: f32 = 26.0;
const THREAD_PAD_BOTTOM: f32 = 20.0;

const DIVIDER_SIZE: f32 = 24.0;
const DIVIDER_WEIGHT: u16 = 500;
const DIVIDER_PAD_X: f32 = 22.0;
const DIVIDER_PAD_Y: f32 = 8.0;
const DIVIDER_MARGIN_BOTTOM: f32 = 26.0;

const BUBBLE_MAX_WIDTH: f32 = 0.76;
const BUBBLE_PAD_X: f32 = 26.0;
const BUBBLE_PAD_Y: f32 = 16.0;
const BUBBLE_RADIUS: f32 = 26.0;
const BUBBLE_MARGIN_BOTTOM: f32 = 22.0;
const BUBBLE_BORDER: f32 = 1.0;
const MESSAGE_SIZE: f32 = 31.0;
const MESSAGE_LINE_HEIGHT: f32 = 1.42;

const DELIVERED_SIZE: f32 = 22.0;
const DELIVERED_MARGIN_TOP: f32 = 8.0;
const DELIVERED_MARGIN_RIGHT: f32 = 6.0;

const TYPING_PAD_X: f32 = 28.0;
const TYPING_PAD_TOP: f32 = 24.0;
const TYPING_PAD_BOTTOM: f32 = 18.0;
const TYPING_DOT: f32 = 15.0;
const TYPING_DOT_GAP: f32 = 12.0;
const TYPING_RADIUS: f32 = 26.0;

const INPUT_PAD_TOP: f32 = 22.0;
const INPUT_PAD_X: f32 = 36.0;
const INPUT_CIRCLE: f32 = 78.0;
const INPUT_GAP: f32 = 22.0;
const INPUT_FIELD_HEIGHT: f32 = 80.0;
const INPUT_FIELD_PAD_LEFT: f32 = 36.0;
const INPUT_PLACEHOLDER_SIZE: f32 = 31.0;
const INPUT_PLUS_SIZE: f32 = 46.0;
const INPUT_PLUS_OFFSET_Y: f32 = -4.0;
const INPUT_SEND_ICON: f32 = 38.0;
const HOME_BAR_W: f32 = 340.0;
const HOME_BAR_H: f32 = 9.0;
const HOME_BAR_MARGIN_TOP: f32 = 20.0;
const HOME_BAR_MARGIN_BOTTOM: f32 = 12.0;

/// The line-box multiplier the source's unspecified `lineHeight` resolves to.
const NORMAL_LINE_HEIGHT: f32 = 1.2;

/// Smallest fraction of the nominal unit the automatic thread fit will scale down to.
const MIN_FIT: f32 = 0.2;

/// The shadcn light-theme tokens the source hard-codes.
#[derive(Debug, Clone, Copy)]
struct Palette {
    background: &'static str,
    card: &'static str,
    foreground: &'static str,
    primary: &'static str,
    primary_foreground: &'static str,
    muted: &'static str,
    muted_foreground: &'static str,
    border: &'static str,
    online: &'static str,
}

/// Every literal colour the source uses.
const COLORS: Palette = Palette {
    background: "#faf9f6",
    card: "#ffffff",
    foreground: "#141414",
    primary: "#3072db",
    primary_foreground: "#ffffff",
    muted: "#f2f0eb",
    muted_foreground: "#6e6a63",
    border: "#d9d9d9",
    online: "#34c759",
};

/// First stop of the avatar's 135° gradient.
const AVATAR_GRADIENT_FROM: &str = "#3072db";
/// Second stop of the avatar's 135° gradient.
const AVATAR_GRADIENT_TO: &str = "#7c5cff";
/// Alpha the source bakes into the typing-dot colour (`#6e6a6394`).
const TYPING_DOT_ALPHA: f32 = 0.58;

/// Who sent a message — decides the bubble side and its colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatSender {
    /// The contact: left-aligned, white card bubble with a hairline border.
    Them,
    /// The user: right-aligned, primary-blue bubble with no border.
    Me,
}

/// One message in the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    /// Who sent it.
    pub from: ChatSender,
    /// Message body. `\n` starts a new line inside the bubble.
    pub text: String,
}

/// Where the thread grows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Stack downward from the header (the source default).
    Top,
    /// Measure the stack and pin it just above the input bar.
    Bottom,
}

/// The three timing knobs the schedule formulas read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatTiming {
    /// Frames before the conversation starts.
    pub start_delay: usize,
    /// Frames between consecutive bubbles.
    pub gap_frames: usize,
    /// Frames a "typing…" indicator shows before each incoming message.
    pub typing_frames: usize,
}

impl Default for ChatTiming {
    fn default() -> Self {
        Self {
            start_delay: 30,
            gap_frames: 20,
            typing_frames: 55,
        }
    }
}

/// One scheduled message and the frames it mounts at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatScheduleEntry {
    /// The message this entry mounts.
    pub message: ChatMessage,
    /// First frame the bubble is mounted.
    pub entry: usize,
    /// First frame its typing indicator is mounted, or `None` when none shows.
    pub typing_from: Option<usize>,
}

/// The pose of a bubble `local_frame` frames after it mounted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BubbleEntrance {
    /// Bubble opacity, `0.0..=1.0`.
    pub opacity: f32,
    /// Downward offset in source px (multiply by the component's unit).
    pub translate_y: f32,
    /// Uniform scale factor about the bubble's centre.
    pub scale: f32,
}

/// The fade and rise of one chrome row (status bar, header, input bar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeFade {
    /// Row opacity, `0.0..=1.0`.
    pub opacity: f32,
    /// Vertical offset in source px (multiply by the component's unit).
    pub translate_y: f32,
}

/// Configuration for [`TextMessages`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextMessagesConfig {
    /// Conversation, in chronological order.
    pub messages: Vec<ChatMessage>,
    /// Contact display name shown in the header.
    pub contact_name: String,
    /// Initials in the avatar circle. `None` derives them from [`Self::contact_name`].
    pub avatar_initials: Option<String>,
    /// Frames a "typing…" indicator shows before each incoming message.
    pub typing_frames: usize,
    /// Frames between consecutive bubbles.
    pub gap_frames: usize,
    /// Frames before the conversation starts.
    pub start_delay: usize,
    /// Where the thread grows from.
    pub anchor: Anchor,
    /// Collapse the rise and the scale-up; keep the fades.
    pub reduced_motion: bool,
    /// Scale the whole conversation down (never up) so it fits between the header and the input
    /// bar.
    ///
    /// The source is composed for a 1080x1920 canvas, where its thread viewport is 1577px tall and
    /// the demo conversation (about 985px) fits easily. This library renders 1920x1080, where the
    /// viewport is only 737px, so at 1:1 the newest bubbles would run off the bottom of the
    /// canvas. With this on (the default) every metric is multiplied by an additional factor
    /// `min(1, available / needed)` computed from *every* configured message, so the thread always
    /// fits and never breathes as bubbles arrive. Turn it off for exact source metrics.
    pub fit_to_thread: bool,
    /// Multiplies every source px metric.
    ///
    /// The metrics are authored for a 1080-wide canvas and are scaled by `ctx.vmin() / 10.8`
    /// before this factor is applied, so `1.0` reproduces the source exactly at 1080p (subject to
    /// [`Self::fit_to_thread`]).
    pub scale: f32,
}

impl Default for TextMessagesConfig {
    fn default() -> Self {
        Self {
            messages: demo_conversation(),
            contact_name: "Maya".to_owned(),
            avatar_initials: None,
            typing_frames: 55,
            gap_frames: 20,
            start_delay: 30,
            anchor: Anchor::Top,
            reduced_motion: false,
            fit_to_thread: true,
            scale: 1.0,
        }
    }
}

impl TextMessagesConfig {
    /// The schedule timing this configuration asks for.
    pub fn timing(&self) -> ChatTiming {
        ChatTiming {
            start_delay: self.start_delay,
            gap_frames: self.gap_frames,
            typing_frames: self.typing_frames,
        }
    }

    /// The initials to draw: the explicit override, or the ones derived from the contact name.
    pub fn initials(&self) -> String {
        match &self.avatar_initials {
            Some(explicit) => explicit.clone(),
            None => derive_initials(&self.contact_name),
        }
    }
}

/// The eight-message demo conversation the source composition ships.
pub fn demo_conversation() -> Vec<ChatMessage> {
    let them = |text: &str| ChatMessage {
        from: ChatSender::Them,
        text: text.to_owned(),
    };
    let me = |text: &str| ChatMessage {
        from: ChatSender::Me,
        text: text.to_owned(),
    };
    vec![
        them("Hey! Still on for tonight? 🎉"),
        me("Wouldn't miss it. What time again?"),
        them("7pm — but come early, Sam's bringing the dog 🐶"),
        me("Best news I've had all week"),
        them("Also… we got you a cake.\nDon't ask."),
        me("I'm asking.\nWhat's on it??"),
        them("You'll see 😏"),
        me("Leaving now. See you in 20!"),
    ]
}

/// Pure frame schedule: one entry per message with its mount moments.
///
/// `cursor` starts at `max(0, startDelay)`; every message after the first adds `gap_frames`; an
/// incoming message then reserves `typing_frames` for its indicator before its bubble mounts.
pub fn compute_chat_schedule(
    messages: &[ChatMessage],
    timing: ChatTiming,
) -> Vec<ChatScheduleEntry> {
    let mut schedule = Vec::with_capacity(messages.len());
    let mut cursor = timing.start_delay;
    for (index, message) in messages.iter().enumerate() {
        if index > 0 {
            cursor += timing.gap_frames;
        }
        let typing = if message.from == ChatSender::Them && timing.typing_frames > 0 {
            Some(cursor)
        } else {
            None
        };
        if typing.is_some() {
            cursor += timing.typing_frames;
        }
        schedule.push(ChatScheduleEntry {
            message: message.clone(),
            entry: cursor,
            typing_from: typing,
        });
    }
    schedule
}

/// Total frames a conversation occupies, including the end hold.
pub fn get_text_messages_duration(messages: &[ChatMessage], timing: ChatTiming) -> usize {
    if messages.is_empty() {
        return timing.start_delay + END_HOLD_FRAMES;
    }
    let schedule = compute_chat_schedule(messages, timing);
    let last_entry = schedule.last().map_or(0, |entry| entry.entry);
    let has_sent_message = messages.iter().any(|m| m.from == ChatSender::Me);
    let delivered_tail = if has_sent_message {
        DELIVERED_LAG_FRAMES + DELIVERED_FADE_FRAMES
    } else {
        0
    };
    last_entry + ENTRANCE_FRAMES + delivered_tail + END_HOLD_FRAMES
}

/// Frames, at the library's own reference rate, for a spring sampled at `fps`.
///
/// Remotion's `spring()` reads the composition's frame rate; this library's [`SPRING_POP`] is
/// sampled in the 30 fps frame the whole motion module is authored in. Converting the frame index
/// keeps the spring's real-time shape identical at any frame rate.
fn spring_frame(local_frame: f32, fps: f32) -> f32 {
    if fps > 0.0 {
        local_frame * crate::motion::FPS / fps
    } else {
        local_frame
    }
}

/// The opacity of the "Delivered" receipt `local_frame` frames after its bubble mounted.
pub fn delivered_opacity(local_frame: f32) -> f32 {
    let start = (ENTRANCE_FRAMES + DELIVERED_LAG_FRAMES) as f32;
    interpolate_multi(
        local_frame,
        &[start, start + DELIVERED_FADE_FRAMES as f32],
        &[0.0, 1.0],
    )
}

/// Spring pose of a bubble `local_frame` frames after it mounts. Settles around frame 20.
///
/// `opacity` fades over the first six frames with `ease_out_cubic`; the rise and the scale-up are
/// collapsed by `motion_scale(reduced)`, but the fade never is.
pub fn bubble_entrance(local_frame: f32, fps: f32, reduced: bool) -> BubbleEntrance {
    let progress = SPRING_POP.value(spring_frame(local_frame, fps));
    let travel = motion_scale(reduced);
    let fraction = interpolate_multi(local_frame, &[0.0, 6.0], &[0.0, 1.0]);
    BubbleEntrance {
        opacity: ease_out_cubic(fraction),
        translate_y: (1.0 - progress) * 32.0 * travel,
        scale: if reduced { 1.0 } else { 0.82 + 0.18 * progress },
    }
}

/// Upward hop of one typing dot in source px; staggered by index, resting between hops.
pub fn typing_dot_lift(frame: f32, fps: f32, dot_index: usize) -> f32 {
    let period = fps * 0.95;
    if period <= 0.0 {
        return 0.0;
    }
    let phase = ((frame % period) / period) * std::f32::consts::PI * 2.0 - dot_index as f32 * 1.05;
    -phase.sin().max(0.0) * 6.0
}

/// Fade and rise of one chrome row over its first 14 frames.
pub fn chrome_fade(frame: f32, from_y: f32) -> ChromeFade {
    let fraction = interpolate_multi(frame, &[0.0, 14.0], &[0.0, 1.0]);
    ChromeFade {
        opacity: fraction,
        translate_y: lerp(from_y, 0.0, ease_out_cubic(fraction)),
    }
}

/// The avatar initials the source derives from a contact name.
///
/// The first character of every whitespace-separated word, uppercased and truncated to two.
pub fn derive_initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|word| word.chars().next())
        .flat_map(char::to_uppercase)
        .take(2)
        .collect()
}

/// Whether any scheduled typing indicator is on screen at `frame` (drives the header status).
pub fn typing_active(schedule: &[ChatScheduleEntry], frame: f32) -> bool {
    schedule.iter().any(|entry| {
        entry
            .typing_from
            .is_some_and(|from| frame >= from as f32 && frame < entry.entry as f32)
    })
}

/// The index of the last message the user sent, if any (the one that gets "Delivered").
pub fn last_mine_index(messages: &[ChatMessage]) -> Option<usize> {
    messages
        .iter()
        .rposition(|message| message.from == ChatSender::Me)
}

/// Measured geometry of one bubble, in the pixel space of the canvas being rendered.
#[derive(Debug, Clone, PartialEq)]
struct BubbleMetrics {
    /// The message body, already wrapped, one entry per drawn line.
    lines: Vec<String>,
    /// Width of the widest line in px.
    text_width: f32,
    /// Whether the bubble is right-aligned (`Me`).
    mine: bool,
}

/// Everything the thread needs measured before it can be laid out.
#[derive(Debug, Clone, PartialEq)]
struct ChatMetrics {
    /// Source px → canvas px multiplier the metrics were measured at.
    ///
    /// Equal to the nominal unit until the automatic thread fit scales it down.
    unit: f32,
    /// Width of the divider label in px.
    divider_width: f32,
    /// One entry per configured message, in order.
    bubbles: Vec<BubbleMetrics>,
}

/// One item that is on screen this frame.
#[derive(Debug, Clone, Copy)]
enum VisibleItem {
    /// A typing bubble and the frames since it mounted.
    Typing { local_frame: f32 },
    /// A message bubble and the frames since it mounted.
    Bubble { index: usize, local_frame: f32 },
}

/// Owned parameters for the thread body, so its argument list stays short.
#[derive(Debug, Clone, Copy)]
struct ThreadGeometry {
    /// Source px → canvas px multiplier.
    unit: f32,
    /// Local frame index.
    frame: f32,
    /// Frame rate.
    fps: f32,
    /// Top of the thread viewport.
    top: f32,
    /// Bottom of the thread viewport (the input bar's top edge).
    bottom: f32,
    /// Left edge of the thread's content box.
    content_left: f32,
    /// Right edge of the thread's content box.
    content_right: f32,
    /// Anchor.
    anchor: Anchor,
    /// Reduced-motion flag.
    reduced: bool,
}

impl ThreadGeometry {
    /// Content-box width.
    fn content_width(&self) -> f32 {
        (self.content_right - self.content_left).max(1.0)
    }
}

/// The Text Messages component.
#[derive(Debug, Clone, Default)]
pub struct TextMessages {
    /// Component options.
    pub config: TextMessagesConfig,
    measured: OnceLock<ChatMetrics>,
}

impl TextMessages {
    /// Build with the given options.
    pub fn new(config: TextMessagesConfig) -> Self {
        Self {
            config,
            measured: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-messages",
            name: "Text Messages",
            summary: "A chat thread plays out message by message with typing indicators",
            feature: "text-messages",
        }
    }

    /// Measure (once) the wrapped bubbles and the divider for the canvas being rendered.
    ///
    /// The metrics depend on the canvas size and the configuration but never on the frame, so a
    /// video measures its conversation exactly once, on the first frame rendered.
    fn metrics<'m>(&'m self, ctx: &RenderCtx<'_>, family: &str, unit: f32) -> &'m ChatMetrics {
        self.measured.get_or_init(|| {
            let mut current = measure(ctx, family, &self.config, unit);
            // Shrinking the layout can re-wrap a line, which changes the height the fit was
            // computed from; a couple of passes converge on the largest unit that still fits.
            for _ in 0..3 {
                let fitted = fitted_unit(ctx.height(), &current, &self.config);
                if fitted >= current.unit - 0.001 {
                    break;
                }
                current = measure(ctx, family, &self.config, fitted);
            }
            current
        })
    }

    /// The message thread: divider pill, typing bubbles and message bubbles.
    fn thread(
        ctx: &RenderCtx<'_>,
        family: &str,
        geometry: &ThreadGeometry,
        metrics: &ChatMetrics,
        schedule: &[ChatScheduleEntry],
        last_mine: Option<usize>,
    ) -> Svgr<'static> {
        let unit = geometry.unit;
        let frame = geometry.frame;
        let family_owned = family.to_owned();
        let line_step = MESSAGE_SIZE * MESSAGE_LINE_HEIGHT * unit;
        let max_bubble = geometry.content_width() * BUBBLE_MAX_WIDTH;

        // Which items are on screen, and at which local frame.
        let mut visible: Vec<VisibleItem> = Vec::new();
        for (index, entry) in schedule.iter().enumerate() {
            if let Some(from) = entry.typing_from
                && frame >= from as f32
                && frame < entry.entry as f32
            {
                visible.push(VisibleItem::Typing {
                    local_frame: frame - from as f32,
                });
                continue;
            }
            if frame >= entry.entry as f32 {
                visible.push(VisibleItem::Bubble {
                    index,
                    local_frame: frame - entry.entry as f32,
                });
            }
        }

        let divider_step = divider_height(unit) + DIVIDER_MARGIN_BOTTOM * unit;
        let total_height: f32 = divider_step
            + visible
                .iter()
                .map(|item| match item {
                    VisibleItem::Typing { .. } => typing_height(unit),
                    VisibleItem::Bubble { index, .. } => {
                        let bubble = metrics
                            .bubbles
                            .get(*index)
                            .map_or(0.0, |bubble| bubble_height(bubble, unit));
                        let delivered = if Some(*index) == last_mine {
                            delivered_height(unit)
                        } else {
                            0.0
                        };
                        bubble + delivered + BUBBLE_MARGIN_BOTTOM * unit
                    }
                })
                .sum::<f32>();

        let (pad_top, pad_bottom) = match geometry.anchor {
            Anchor::Top => (THREAD_PAD_TOP * unit, THREAD_PAD_BOTTOM * unit),
            Anchor::Bottom => (THREAD_PAD_BOTTOM * unit, THREAD_PAD_TOP * unit),
        };

        let mut cursor = match geometry.anchor {
            Anchor::Top => geometry.top + pad_top,
            Anchor::Bottom => geometry.bottom - pad_bottom - total_height,
        };

        let mut nodes: Vec<Svgr<'static>> = Vec::new();

        // ── Divider pill ──
        let divider_opacity = interpolate_multi(frame, &[8.0, 22.0], &[0.0, 1.0]);
        if divider_opacity > 0.001 {
            let h = divider_height(unit);
            let w = metrics.divider_width + 2.0 * DIVIDER_PAD_X * unit;
            let x = (geometry.content_left + geometry.content_right) / 2.0 - w / 2.0;
            nodes.push(rounded_rect(
                x,
                cursor,
                w,
                h,
                h / 2.0,
                COLORS.muted,
                divider_opacity,
            ));
            let label_size = DIVIDER_SIZE * unit;
            let label_step = label_size * NORMAL_LINE_HEIGHT;
            nodes.push(text_node_centered(
                DIVIDER_LABEL,
                x + w / 2.0,
                cursor + DIVIDER_PAD_Y * unit + ctx.baseline_offset(family, label_size, label_step),
                family_owned.clone(),
                label_size,
                DIVIDER_WEIGHT,
                0.0,
                COLORS.muted_foreground.to_owned(),
                divider_opacity,
            ));
        }
        cursor += divider_step;

        // ── Bubbles and typing indicators ──
        for item in &visible {
            match item {
                VisibleItem::Typing { local_frame } => {
                    let h = typing_height(unit);
                    let w = TYPING_PAD_X * 2.0 * unit
                        + TYPING_DOT * 3.0 * unit
                        + TYPING_DOT_GAP * 2.0 * unit
                        + 2.0 * BUBBLE_BORDER * unit;
                    let x = geometry.content_left;
                    nodes.push(rounded_rect(
                        x,
                        cursor,
                        w,
                        h,
                        TYPING_RADIUS * unit,
                        COLORS.card,
                        1.0,
                    ));
                    nodes.push(stroked_rect(
                        x,
                        cursor,
                        w,
                        h,
                        TYPING_RADIUS * unit,
                        COLORS.border,
                        BUBBLE_BORDER * unit,
                        1.0,
                    ));
                    let r = TYPING_DOT * unit / 2.0;
                    for dot in 0..3 {
                        let cx = x
                            + BUBBLE_BORDER * unit
                            + TYPING_PAD_X * unit
                            + r
                            + dot as f32 * (TYPING_DOT + TYPING_DOT_GAP) * unit;
                        let cy = cursor
                            + BUBBLE_BORDER * unit
                            + TYPING_PAD_TOP * unit
                            + r
                            + typing_dot_lift(*local_frame, geometry.fps, dot) * unit;
                        nodes.push(circle(
                            cx,
                            cy,
                            r,
                            COLORS.muted_foreground,
                            TYPING_DOT_ALPHA,
                            None,
                        ));
                    }
                    cursor += h;
                }
                VisibleItem::Bubble { index, local_frame } => {
                    let Some(bubble) = metrics.bubbles.get(*index) else {
                        continue;
                    };
                    let pose = bubble_entrance(*local_frame, geometry.fps, geometry.reduced);
                    let border = if bubble.mine {
                        0.0
                    } else {
                        BUBBLE_BORDER * unit
                    };
                    let w = (bubble.text_width + 2.0 * BUBBLE_PAD_X * unit + 2.0 * border)
                        .min(max_bubble);
                    let h = bubble_height(bubble, unit);
                    let x = if bubble.mine {
                        geometry.content_right - w
                    } else {
                        geometry.content_left
                    };
                    let y = cursor;

                    let mut bubble_nodes: Vec<Svgr<'static>> = Vec::new();
                    bubble_nodes.push(rounded_rect(
                        x,
                        y,
                        w,
                        h,
                        BUBBLE_RADIUS * unit,
                        if bubble.mine {
                            COLORS.primary
                        } else {
                            COLORS.card
                        },
                        1.0,
                    ));
                    if !bubble.mine {
                        bubble_nodes.push(stroked_rect(
                            x,
                            y,
                            w,
                            h,
                            BUBBLE_RADIUS * unit,
                            COLORS.border,
                            border,
                            1.0,
                        ));
                    }
                    let fill = if bubble.mine {
                        COLORS.primary_foreground
                    } else {
                        COLORS.foreground
                    };
                    let body_size = MESSAGE_SIZE * unit;
                    let text_x = x + border + BUBBLE_PAD_X * unit;
                    let first_baseline =
                        y + BUBBLE_PAD_Y * unit + ctx.baseline_offset(family, body_size, line_step);
                    for (line_index, line) in bubble.lines.iter().enumerate() {
                        if line.is_empty() {
                            continue;
                        }
                        bubble_nodes.push(text_node_left(
                            line.clone(),
                            text_x,
                            first_baseline + line_index as f32 * line_step,
                            family_owned.clone(),
                            body_size,
                            400,
                            0.0,
                            fill.to_owned(),
                            1.0,
                        ));
                    }

                    let center_y = y + h / 2.0;
                    let transform = format!(
                        "translate(0 {:.4}) translate({:.4} {:.4}) scale({:.6}) translate({:.4} {:.4})",
                        0.0_f32 + pose.translate_y * unit,
                        x + w / 2.0,
                        center_y,
                        pose.scale,
                        -(x + w / 2.0),
                        -center_y,
                    );
                    let body = Svgr::from(bubble_nodes);
                    nodes.push(fframes::svgr!(
                        <g transform={transform} opacity={pose.opacity}>{body}</g>
                    ));

                    cursor += h;

                    if Some(*index) == last_mine {
                        let receipt = delivered_opacity(*local_frame);
                        let receipt_size = DELIVERED_SIZE * unit;
                        let receipt_step = receipt_size * NORMAL_LINE_HEIGHT;
                        nodes.push(text_node_right(
                            "Delivered".to_owned(),
                            x + w - DELIVERED_MARGIN_RIGHT * unit,
                            cursor
                                + DELIVERED_MARGIN_TOP * unit
                                + ctx.baseline_offset(family, receipt_size, receipt_step),
                            family_owned.clone(),
                            receipt_size,
                            400,
                            COLORS.muted_foreground.to_owned(),
                            receipt,
                        ));
                        cursor += delivered_height(unit);
                    }
                    cursor += BUBBLE_MARGIN_BOTTOM * unit;
                }
            }
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

/// Height of the divider pill in canvas px.
fn divider_height(unit: f32) -> f32 {
    2.0 * DIVIDER_PAD_Y * unit + DIVIDER_SIZE * NORMAL_LINE_HEIGHT * unit
}

/// Height of the typing indicator in canvas px, borders included.
fn typing_height(unit: f32) -> f32 {
    (TYPING_PAD_TOP + TYPING_DOT + TYPING_PAD_BOTTOM) * unit + 2.0 * BUBBLE_BORDER * unit
}

/// Height of one measured bubble in canvas px, padding and border included.
fn bubble_height(bubble: &BubbleMetrics, unit: f32) -> f32 {
    let border = if bubble.mine {
        0.0
    } else {
        BUBBLE_BORDER * unit
    };
    bubble.lines.len() as f32 * MESSAGE_SIZE * MESSAGE_LINE_HEIGHT * unit
        + 2.0 * BUBBLE_PAD_Y * unit
        + 2.0 * border
}

/// Height of the "Delivered" receipt block in canvas px.
fn delivered_height(unit: f32) -> f32 {
    (DELIVERED_MARGIN_TOP + DELIVERED_SIZE * NORMAL_LINE_HEIGHT) * unit
}

/// Height of the input bar in canvas px.
fn input_bar_height(unit: f32) -> f32 {
    (INPUT_PAD_TOP + INPUT_FIELD_HEIGHT + HOME_BAR_MARGIN_TOP + HOME_BAR_H + HOME_BAR_MARGIN_BOTTOM)
        * unit
}

/// Height of the full thread — divider, every bubble, the receipt and all the margins.
///
/// This is measured over *every* configured message, not just the ones that are on screen, so the
/// automatic fit is stable while the conversation plays.
fn stack_height(metrics: &ChatMetrics, last_mine: Option<usize>) -> f32 {
    let unit = metrics.unit;
    let mut total = divider_height(unit) + DIVIDER_MARGIN_BOTTOM * unit;
    for (index, bubble) in metrics.bubbles.iter().enumerate() {
        total += bubble_height(bubble, unit) + BUBBLE_MARGIN_BOTTOM * unit;
        if Some(index) == last_mine {
            total += delivered_height(unit);
        }
    }
    total
}

/// Everything that is not the thread body, in canvas px: status bar, header, input bar and both
/// thread paddings.
fn chrome_height(unit: f32) -> f32 {
    (STATUS_HEIGHT + HEADER_HEIGHT + THREAD_PAD_TOP + THREAD_PAD_BOTTOM) * unit
        + input_bar_height(unit)
}

/// The largest unit, at most `metrics.unit`, that keeps the whole conversation on the canvas.
///
/// Every height in the layout is linear in `unit`, so the largest unit that fits is
/// `height / (stack at unit 1 + chrome at unit 1)`.
fn fitted_unit(canvas_height: f32, metrics: &ChatMetrics, config: &TextMessagesConfig) -> f32 {
    let nominal = metrics.unit;
    if !config.fit_to_thread || nominal <= 0.0 || canvas_height <= 0.0 {
        return nominal;
    }
    let needed = stack_height(metrics, last_mine_index(&config.messages));
    if needed <= 0.0 {
        return nominal;
    }
    let per_unit = needed / nominal + chrome_height(1.0);
    (canvas_height / per_unit).clamp(nominal * MIN_FIT, nominal)
}

/// Measure the wrapped bubbles and the divider label for the canvas being rendered.
fn measure(
    ctx: &RenderCtx<'_>,
    family: &str,
    config: &TextMessagesConfig,
    unit: f32,
) -> ChatMetrics {
    let content_width = (ctx.width() - 2.0 * THREAD_PAD_X * unit).max(1.0);
    let max_bubble = content_width * BUBBLE_MAX_WIDTH;
    let font_size = MESSAGE_SIZE * unit;

    let mut bubbles = Vec::with_capacity(config.messages.len());
    for message in &config.messages {
        let mine = message.from == ChatSender::Me;
        let border = if mine { 0.0 } else { BUBBLE_BORDER * unit };
        let available = (max_bubble - 2.0 * BUBBLE_PAD_X * unit - 2.0 * border).max(1.0);
        let mut lines: Vec<String> = Vec::new();
        for hard_line in message.text.split('\n') {
            let layout = layout_paragraph(
                ctx,
                family,
                hard_line,
                font_size,
                available,
                MESSAGE_LINE_HEIGHT,
            );
            if layout.lines.is_empty() {
                lines.push(String::new());
                continue;
            }
            for line in &layout.lines {
                lines.push(
                    line.words
                        .iter()
                        .map(|word| word.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
        if lines.is_empty() {
            lines.push(String::new());
        }
        let text_width = lines
            .iter()
            .map(|line| ctx.text_width(family, font_size, line))
            .fold(0.0_f32, f32::max);
        bubbles.push(BubbleMetrics {
            lines,
            text_width,
            mine,
        });
    }

    ChatMetrics {
        unit,
        divider_width: ctx.text_width(family, DIVIDER_SIZE * unit, DIVIDER_LABEL),
        bubbles,
    }
}

/// A `<text>` node anchored at its right edge (the source's `text-align: right`).
#[allow(clippy::too_many_arguments)]
fn text_node_right(
    content: String,
    x: f32,
    y: f32,
    family: String,
    font_size: f32,
    weight: u16,
    fill: String,
    opacity: f32,
) -> Svgr<'static> {
    fframes::svgr!(
        <text x={x} y={y} font-family={family} font-size={font_size} font-weight={weight}
              fill={fill} opacity={opacity} text-anchor="end">{content}</text>
    )
}

/// A filled rounded rect.
fn rounded_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rx: f32,
    fill: &'static str,
    opacity: f32,
) -> Svgr<'static> {
    let fill = fill.to_owned();
    fframes::svgr!(
        <rect x={x} y={y} width={width} height={height} rx={rx} fill={fill} opacity={opacity} />
    )
}

/// A rounded rect drawn as an outline, used for the source's 1px CSS borders.
#[allow(clippy::too_many_arguments)]
fn stroked_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rx: f32,
    stroke: &'static str,
    stroke_width: f32,
    opacity: f32,
) -> Svgr<'static> {
    let stroke = stroke.to_owned();
    fframes::svgr!(
        <rect x={x} y={y} width={width} height={height} rx={rx} fill="none" stroke={stroke}
              stroke-width={stroke_width} opacity={opacity} />
    )
}

/// A circle with an optional outline.
fn circle(
    cx: f32,
    cy: f32,
    r: f32,
    fill: &'static str,
    opacity: f32,
    stroke: Option<(&'static str, f32)>,
) -> Svgr<'static> {
    match stroke {
        Some((color, width)) => {
            let fill = fill.to_owned();
            let color = color.to_owned();
            fframes::svgr!(
                <circle cx={cx} cy={cy} r={r} fill={fill} opacity={opacity} stroke={color}
                        stroke-width={width} />
            )
        }
        None => {
            let fill = fill.to_owned();
            fframes::svgr!(<circle cx={cx} cy={cy} r={r} fill={fill} opacity={opacity} />)
        }
    }
}

/// The transform that maps an icon's `viewBox` onto a `width` x `height` box, `xMidYMid meet`.
fn icon_transform(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    view_box_width: f32,
    view_box_height: f32,
) -> String {
    let scale = (width / view_box_width).min(height / view_box_height);
    let dx = x + (width - view_box_width * scale) / 2.0;
    let dy = y + (height - view_box_height * scale) / 2.0;
    format!("translate({dx:.4} {dy:.4}) scale({scale:.6})")
}

/// The iOS status bar: the clock on the left, signal / Wi-Fi / battery on the right.
fn status_bar(ctx: &RenderCtx<'_>, family: &str, unit: f32, width: f32) -> Svgr<'static> {
    let height = STATUS_HEIGHT * unit;
    let time_size = STATUS_TIME_SIZE * unit;
    let time_step = time_size * NORMAL_LINE_HEIGHT;
    let time_y = (height - time_step) / 2.0 + ctx.baseline_offset(family, time_size, time_step);

    let signal_w = 22.0 * unit;
    let signal_h = 15.0 * unit;
    let wifi_w = 23.0 * unit;
    let wifi_h = 17.0 * unit;
    let battery_w = 30.0 * unit;
    let battery_h = 15.0 * unit;
    let group_w = signal_w + STATUS_ICON_GAP * unit + wifi_w + STATUS_ICON_GAP * unit + battery_w;
    let group_x = width - STATUS_PAD_X * unit - group_w;
    let signal_x = group_x;
    let wifi_x = signal_x + signal_w + STATUS_ICON_GAP * unit;
    let battery_x = wifi_x + wifi_w + STATUS_ICON_GAP * unit;

    let foreground = COLORS.foreground.to_owned();
    let signal_transform = icon_transform(
        signal_x,
        (height - signal_h) / 2.0,
        signal_w,
        signal_h,
        20.0,
        14.0,
    );
    let wifi_y = (height - wifi_h) / 2.0;
    let wifi_transform = icon_transform(wifi_x, wifi_y, wifi_w, wifi_h, 24.0, 22.0);
    let battery_transform = icon_transform(
        battery_x,
        (height - battery_h) / 2.0,
        battery_w,
        battery_h,
        30.0,
        14.0,
    );

    fframes::svgr!(
        <g>
            {text_node_left(
                "9:41".to_owned(),
                STATUS_PAD_X * unit,
                time_y,
                family.to_owned(),
                time_size,
                STATUS_TIME_WEIGHT,
                STATUS_TIME_TRACKING * unit,
                foreground.clone(),
                1.0,
            )}
            <g transform={signal_transform} fill={foreground.clone()}>
                <rect x="0" y="9" width="3.5" height="5" rx="1.1" />
                <rect x="5.5" y="6" width="3.5" height="8" rx="1.1" />
                <rect x="11" y="3" width="3.5" height="11" rx="1.1" />
                <rect x="16.5" y="0" width="3.5" height="14" rx="1.1" />
            </g>
            <g transform={wifi_transform} fill="none" stroke={foreground.clone()}
               stroke-width="2.2" stroke-linecap="round">
                <path d="M2 11a10.5 10.5 0 0 1 20 0" />
                <path d="M5.5 14a6.5 6.5 0 0 1 13 0" />
                <path d="M9 17a3 3 0 0 1 6 0" />
                <circle cx="12" cy="20" r="1.9" fill={foreground.clone()} />
            </g>
            <g transform={battery_transform}>
                <rect x="1" y="1" width="24" height="12" rx="3.5" fill="none"
                      stroke={foreground.clone()} stroke-width="1.5" opacity="0.45" />
                <rect x="3" y="3" width="17" height="8" rx="2" fill={foreground.clone()} />
                <rect x="26.5" y="4.25" width="2.5" height="5.5" rx="1.25"
                      fill={foreground.clone()} opacity="0.45" />
            </g>
        </g>
    )
}

/// The contact header: back chevron, gradient avatar with an online dot, name, status, menu dots.
fn header_bar(
    ctx: &RenderCtx<'_>,
    family: &str,
    unit: f32,
    width: f32,
    contact_name: &str,
    initials: &str,
    typing: bool,
) -> Svgr<'static> {
    let height = HEADER_HEIGHT * unit;
    let card = COLORS.card.to_owned();
    let back_x = HEADER_PAD_X * unit;
    let back_transform = icon_transform(
        back_x,
        (height - HEADER_BACK_H * unit) / 2.0,
        HEADER_BACK_W * unit,
        HEADER_BACK_H * unit,
        12.0,
        22.0,
    );
    let avatar_x = back_x + HEADER_BACK_W * unit + HEADER_GAP * unit;
    let avatar_y = (height - HEADER_AVATAR * unit) / 2.0;
    let avatar_r = HEADER_AVATAR * unit / 2.0;
    let dot_r = HEADER_ONLINE_DOT * unit / 2.0;
    let dot_cx = avatar_x + HEADER_AVATAR * unit - 2.0 * unit - dot_r;
    let dot_cy = avatar_y + HEADER_AVATAR * unit - 2.0 * unit - dot_r;

    let text_x = avatar_x + HEADER_AVATAR * unit + HEADER_GAP * unit;
    let name_size = HEADER_NAME_SIZE * unit;
    let name_step = name_size * HEADER_NAME_LINE_HEIGHT;
    let status_size = HEADER_STATUS_SIZE * unit;
    let status_step = status_size * NORMAL_LINE_HEIGHT;
    let block_height = name_step + HEADER_STATUS_OFFSET_Y * unit + status_step;
    let block_top = (height - block_height) / 2.0;
    let name_y = block_top + ctx.baseline_offset(family, name_size, name_step);
    let status_y = block_top
        + name_step
        + HEADER_STATUS_OFFSET_Y * unit
        + ctx.baseline_offset(family, status_size, status_step);

    let dot_r_small = HEADER_MENU_DOT * unit / 2.0;
    let menu_right = width - HEADER_PAD_X * unit - HEADER_MENU_PAD_RIGHT * unit - dot_r_small;
    let menu_center_y = height / 2.0;
    let menu_gap = (HEADER_MENU_DOT + HEADER_MENU_GAP) * unit;
    let status_label = if typing { "typing…" } else { "online" }.to_owned();
    let initials = initials.to_owned();
    let contact_name = contact_name.to_owned();
    let family_owned = family.to_owned();
    let initials_size = HEADER_AVATAR_INITIALS_SIZE * unit;
    let initials_step = initials_size * HEADER_NAME_LINE_HEIGHT;
    let initials_y = avatar_y + avatar_r - initials_step / 2.0
        + ctx.baseline_offset(family, initials_size, initials_step);
    let primary = COLORS.primary.to_owned();
    let avatar_from = AVATAR_GRADIENT_FROM.to_owned();
    let avatar_to = AVATAR_GRADIENT_TO.to_owned();

    fframes::svgr!(
        <g>
            {rounded_rect(0.0, 0.0, width, height, 0.0, COLORS.card, 1.0)}
            {rounded_rect(0.0, height - unit, width, unit, 0.0, COLORS.border, 1.0)}
            <g transform={back_transform}>
                <path d="M10.5 1.5 2 11l8.5 9.5" fill="none" stroke={primary}
                      stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
            </g>
            <defs>
                <linearGradient id="text-messages-avatar" x1="0" y1="0" x2="1" y2="1">
                    <stop offset="0" stop-color={avatar_from} />
                    <stop offset="1" stop-color={avatar_to} />
                </linearGradient>
            </defs>
            <circle cx={avatar_x + avatar_r} cy={avatar_y + avatar_r} r={avatar_r}
                    fill="url(#text-messages-avatar)" />
            {text_node_centered(
                &initials,
                avatar_x + avatar_r,
                initials_y,
                family_owned.clone(),
                initials_size,
                HEADER_AVATAR_INITIALS_WEIGHT,
                0.0,
                card.clone(),
                1.0,
            )}
            {circle(dot_cx, dot_cy, dot_r - HEADER_ONLINE_BORDER * unit / 2.0,
                    COLORS.online, 1.0, Some((COLORS.card, HEADER_ONLINE_BORDER * unit)))}
            {text_node_left(
                contact_name,
                text_x,
                name_y,
                family_owned.clone(),
                name_size,
                HEADER_NAME_WEIGHT,
                0.0,
                COLORS.foreground.to_owned(),
                1.0,
            )}
            {text_node_left(
                status_label,
                text_x,
                status_y,
                family_owned.clone(),
                status_size,
                400,
                0.0,
                COLORS.muted_foreground.to_owned(),
                1.0,
            )}
            {circle(menu_right, menu_center_y - menu_gap, dot_r_small,
                    COLORS.muted_foreground, 1.0, None)}
            {circle(menu_right, menu_center_y, dot_r_small,
                    COLORS.muted_foreground, 1.0, None)}
            {circle(menu_right, menu_center_y + menu_gap, dot_r_small,
                    COLORS.muted_foreground, 1.0, None)}
        </g>
    )
}

/// The bottom bar: plus button, pill-shaped message field, send button and the home indicator.
fn input_bar(
    ctx: &RenderCtx<'_>,
    family: &str,
    unit: f32,
    width: f32,
    height: f32,
) -> Svgr<'static> {
    let bar_height = input_bar_height(unit);
    let top = height - bar_height;
    let row_y = top + INPUT_PAD_TOP * unit;
    let pad = INPUT_PAD_X * unit;
    let circle_d = INPUT_CIRCLE * unit;
    let plus_cx = pad + circle_d / 2.0;
    let plus_cy = row_y + circle_d / 2.0;
    let field_x = pad + circle_d + INPUT_GAP * unit;
    let send_x = width - pad - circle_d;
    let field_width = (send_x - INPUT_GAP * unit - field_x).max(1.0);
    let field_r = INPUT_FIELD_HEIGHT * unit / 2.0;

    let plus_size = INPUT_PLUS_SIZE * unit;
    let plus_step = plus_size * NORMAL_LINE_HEIGHT;
    let plus_baseline = plus_cy + INPUT_PLUS_OFFSET_Y * unit - plus_step / 2.0
        + ctx.baseline_offset(family, plus_size, plus_step);
    let placeholder_size = INPUT_PLACEHOLDER_SIZE * unit;
    let placeholder_step = placeholder_size * NORMAL_LINE_HEIGHT;
    let placeholder_baseline = row_y + INPUT_FIELD_HEIGHT * unit / 2.0 - placeholder_step / 2.0
        + ctx.baseline_offset(family, placeholder_size, placeholder_step);
    let send_transform = icon_transform(
        send_x + (circle_d - INPUT_SEND_ICON * unit) / 2.0,
        row_y + (circle_d - INPUT_SEND_ICON * unit) / 2.0,
        INPUT_SEND_ICON * unit,
        INPUT_SEND_ICON * unit,
        24.0,
        24.0,
    );
    let home_x = (width - HOME_BAR_W * unit) / 2.0;
    let home_y = row_y + INPUT_FIELD_HEIGHT * unit + HOME_BAR_MARGIN_TOP * unit;
    let family_owned = family.to_owned();
    let send_color = COLORS.primary_foreground.to_owned();

    fframes::svgr!(
        <g>
            {rounded_rect(0.0, top, width, bar_height, 0.0, COLORS.card, 1.0)}
            {rounded_rect(0.0, top, width, unit, 0.0, COLORS.border, 1.0)}
            {circle(plus_cx, plus_cy, circle_d / 2.0, COLORS.muted, 1.0, None)}
            {text_node_centered(
                "+",
                plus_cx,
                plus_baseline,
                family_owned.clone(),
                plus_size,
                400,
                0.0,
                COLORS.muted_foreground.to_owned(),
                1.0,
            )}
            {rounded_rect(field_x, row_y, field_width, INPUT_FIELD_HEIGHT * unit, field_r,
                          COLORS.muted, 1.0)}
            {text_node_left(
                "Message".to_owned(),
                field_x + INPUT_FIELD_PAD_LEFT * unit,
                placeholder_baseline,
                family_owned.clone(),
                placeholder_size,
                400,
                0.0,
                COLORS.muted_foreground.to_owned(),
                1.0,
            )}
            {circle(send_x + circle_d / 2.0, row_y + circle_d / 2.0, circle_d / 2.0,
                    COLORS.primary, 1.0, None)}
            <g transform={send_transform}>
                <path d="M12 19V6M6.2 11.2 12 5.4l5.8 5.8" fill="none" stroke={send_color}
                      stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
            </g>
            {rounded_rect(home_x, home_y, HOME_BAR_W * unit, HOME_BAR_H * unit,
                          HOME_BAR_H * unit / 2.0, COLORS.foreground, 0.85)}
        </g>
    )
}

impl Component for TextMessages {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_text_messages_duration(&self.config.messages, self.config.timing())
    }

    fn background(&self) -> Option<&'static str> {
        Some(COLORS.background)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let nominal_unit = ctx.vmin() / REFERENCE_VMIN * self.config.scale;
        let family = ctx.sans();
        let frame = ctx.frame();
        let fps = ctx.info.fps;
        let width = ctx.width();
        let height = ctx.height();

        let metrics = self.metrics(ctx, &family, nominal_unit);
        // The fit may have scaled the nominal unit down; every metric below uses the measured one.
        let unit = metrics.unit;
        let schedule = compute_chat_schedule(&self.config.messages, self.config.timing());
        let typing = typing_active(&schedule, frame);
        let last_mine = last_mine_index(&self.config.messages);
        let initials = self.config.initials();

        let header_top = STATUS_HEIGHT * unit;
        let thread_top = header_top + HEADER_HEIGHT * unit;
        let thread_bottom = (height - input_bar_height(unit)).max(thread_top);
        let pad_x = THREAD_PAD_X * unit;

        let status = chrome_fade(frame, -14.0);
        let header = chrome_fade(frame, -10.0);
        let input = chrome_fade(frame, 12.0);

        let geometry = ThreadGeometry {
            unit,
            frame,
            fps,
            top: thread_top,
            bottom: thread_bottom,
            content_left: pad_x,
            content_right: width - pad_x,
            anchor: self.config.anchor,
            reduced: self.config.reduced_motion,
        };

        let status_layer = status_bar(ctx, &family, unit, width);
        let header_layer = header_bar(
            ctx,
            &family,
            unit,
            width,
            &self.config.contact_name,
            &initials,
            typing,
        );
        let thread_layer = Self::thread(ctx, &family, &geometry, metrics, &schedule, last_mine);
        let input_layer = input_bar(ctx, &family, unit, width, height);

        let status_transform = format!("translate(0 {:.4})", status.translate_y * unit);
        let header_transform = format!("translate(0 {:.4})", header.translate_y * unit);
        let input_transform = format!("translate(0 {:.4})", input.translate_y * unit);

        fframes::svgr!(
            <g>
                <g transform={status_transform} opacity={status.opacity}>{status_layer}</g>
                <g transform={header_transform} opacity={header.opacity}>{header_layer}</g>
                {thread_layer}
                <g transform={input_transform} opacity={input.opacity}>{input_layer}</g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn them(text: &str) -> ChatMessage {
        ChatMessage {
            from: ChatSender::Them,
            text: text.to_owned(),
        }
    }

    fn me(text: &str) -> ChatMessage {
        ChatMessage {
            from: ChatSender::Me,
            text: text.to_owned(),
        }
    }

    #[test]
    fn schedule_matches_the_ported_formula() {
        let messages = vec![them("a"), me("b"), them("c")];
        let schedule = compute_chat_schedule(&messages, ChatTiming::default());
        assert_eq!(schedule.len(), 3);
        // 30 (start) -> typing at 30, entry 30 + 55 = 85
        assert_eq!(schedule[0].typing_from, Some(30));
        assert_eq!(schedule[0].entry, 85);
        // 85 + 20 = 105, no typing for "me"
        assert_eq!(schedule[1].typing_from, None);
        assert_eq!(schedule[1].entry, 105);
        // 105 + 20 = 125, typing -> 180
        assert_eq!(schedule[2].typing_from, Some(125));
        assert_eq!(schedule[2].entry, 180);
    }

    #[test]
    fn schedule_skips_typing_when_the_budget_is_zero() {
        let timing = ChatTiming {
            typing_frames: 0,
            ..ChatTiming::default()
        };
        let schedule = compute_chat_schedule(&[them("a"), them("b")], timing);
        assert_eq!(schedule[0].typing_from, None);
        assert_eq!(schedule[0].entry, 30);
        assert_eq!(schedule[1].entry, 50);
    }

    #[test]
    fn schedule_clamps_a_negative_start_delay() {
        // `start_delay` is unsigned here, so the JS `Math.max(0, ...)` is structural: the first
        // entry can never be negative and no gap is charged before it.
        let schedule = compute_chat_schedule(&[them("a")], ChatTiming::default());
        assert_eq!(schedule[0].entry, 30 + 55);
    }

    #[test]
    fn duration_matches_the_source_demo() {
        let messages = demo_conversation();
        assert_eq!(messages.len(), 8);
        let duration = get_text_messages_duration(&messages, ChatTiming::default());
        // last entry 390 + 24 entrance + (8 + 12) delivered tail + 75 hold
        assert_eq!(duration, 390 + 24 + 20 + 75);
        assert_eq!(duration, 509);
    }

    #[test]
    fn duration_without_a_sent_message_drops_the_delivered_tail() {
        let messages = vec![them("hello"), them("there")];
        let schedule = compute_chat_schedule(&messages, ChatTiming::default());
        let last = schedule.last().map_or(0, |entry| entry.entry);
        assert_eq!(
            get_text_messages_duration(&messages, ChatTiming::default()),
            last + ENTRANCE_FRAMES + END_HOLD_FRAMES
        );
    }

    #[test]
    fn empty_conversation_is_start_delay_plus_hold() {
        assert_eq!(
            get_text_messages_duration(&[], ChatTiming::default()),
            30 + END_HOLD_FRAMES
        );
    }

    #[test]
    fn bubble_entrance_springs_from_a_shrunk_offset_pose() {
        let start = bubble_entrance(0.0, 30.0, false);
        assert!(start.opacity.abs() < 1e-6);
        assert!((start.translate_y - 32.0).abs() < 1e-3);
        assert!((start.scale - 0.82).abs() < 1e-6);

        let settled = bubble_entrance(120.0, 30.0, false);
        assert!((settled.opacity - 1.0).abs() < 1e-6);
        assert!(settled.translate_y.abs() < 0.01);
        assert!((settled.scale - 1.0).abs() < 0.01);
    }

    #[test]
    fn bubble_entrance_opacity_uses_the_cubic_ease() {
        // The raw fraction at frame 3 is 0.5, and ease_out_cubic(0.5) is 0.875.
        let pose = bubble_entrance(3.0, 30.0, false);
        assert!((pose.opacity - ease_out_cubic(0.5)).abs() < 1e-6);
        assert!((pose.opacity - 0.875).abs() < 1e-6);
    }

    #[test]
    fn reduced_motion_drops_travel_and_scale_but_keeps_the_fade() {
        let pose = bubble_entrance(0.0, 30.0, true);
        assert!(pose.opacity.abs() < 1e-6);
        assert!(pose.translate_y.abs() < 1e-6);
        assert!((pose.scale - 1.0).abs() < 1e-6);
        let later = bubble_entrance(4.0, 30.0, true);
        assert!(later.opacity > 0.5);
    }

    #[test]
    fn the_spring_is_frame_rate_aware() {
        // At 30 fps the conversion is the identity, so this is `SPRING_POP.value(frame)`.
        let at_30 = bubble_entrance(12.0, 30.0, false);
        assert!((at_30.scale - (0.82 + 0.18 * SPRING_POP.value(12.0))).abs() < 1e-6);
        // At 60 fps the same wall-clock moment is frame 24.
        let at_60 = bubble_entrance(24.0, 60.0, false);
        assert!((at_60.scale - at_30.scale).abs() < 1e-6);
    }

    #[test]
    fn typing_dots_hop_out_of_phase_and_rest_between_hops() {
        let fps = 30.0;
        let period = fps * 0.95;
        // At the start of a period the first dot is at the top of its hop.
        assert!(typing_dot_lift(period * 0.25, fps, 0) < -5.0);
        // Half a period later it is back at rest.
        assert!(typing_dot_lift(period * 0.75, fps, 0).abs() < 1e-3);
        // The second dot is a quarter-period behind the first.
        assert!(typing_dot_lift(period * 0.25, fps, 1) > -5.0);
        // The lift never exceeds the 6px source amplitude.
        for i in 0..200 {
            for dot in 0..3 {
                let lift = typing_dot_lift(i as f32 * 0.5, fps, dot);
                assert!((-6.001..=0.0).contains(&lift), "lift out of range: {lift}");
            }
        }
    }

    #[test]
    fn typing_dot_lift_survives_a_zero_frame_rate() {
        assert!(typing_dot_lift(10.0, 0.0, 1).abs() < 1e-6);
    }

    #[test]
    fn delivered_opacity_lands_on_the_source_window() {
        assert!(delivered_opacity(31.0).abs() < 1e-6);
        assert!(delivered_opacity(32.0).abs() < 1e-6);
        assert!((delivered_opacity(38.0) - 0.5).abs() < 1e-6);
        assert!((delivered_opacity(44.0) - 1.0).abs() < 1e-6);
        assert!((delivered_opacity(999.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn chrome_fade_rises_into_place() {
        let start = chrome_fade(0.0, -14.0);
        assert!(start.opacity.abs() < 1e-6);
        assert!((start.translate_y + 14.0).abs() < 1e-6);
        let end = chrome_fade(14.0, -14.0);
        assert!((end.opacity - 1.0).abs() < 1e-6);
        assert!(end.translate_y.abs() < 1e-6);
        // The transform is eased, the opacity is not.
        let mid = chrome_fade(7.0, -14.0);
        assert!((mid.opacity - 0.5).abs() < 1e-6);
        assert!(mid.translate_y > -7.0);
    }

    #[test]
    fn typing_active_tracks_the_indicator_window() {
        let schedule = compute_chat_schedule(&[them("hi")], ChatTiming::default());
        assert!(!typing_active(&schedule, 29.0));
        assert!(typing_active(&schedule, 30.0));
        assert!(typing_active(&schedule, 84.0));
        assert!(!typing_active(&schedule, 85.0));
    }

    #[test]
    fn initials_derive_from_the_contact_name() {
        assert_eq!(derive_initials("Maya"), "M");
        assert_eq!(derive_initials("Maya Kovács"), "MK");
        assert_eq!(derive_initials("  ana   maria  silva "), "AM");
        assert_eq!(derive_initials(""), "");
        assert_eq!(
            TextMessagesConfig {
                avatar_initials: Some("MK".to_owned()),
                ..TextMessagesConfig::default()
            }
            .initials(),
            "MK"
        );
        assert_eq!(TextMessagesConfig::default().initials(), "M");
    }

    #[test]
    fn last_mine_index_finds_the_delivered_bubble() {
        assert_eq!(
            last_mine_index(&[them("a"), me("b"), them("c"), me("d")]),
            Some(3)
        );
        assert_eq!(last_mine_index(&[them("a")]), None);
        assert_eq!(last_mine_index(&[]), None);
    }

    #[test]
    fn component_reports_the_demo_duration_and_metadata() {
        let component = TextMessages::default();
        assert_eq!(component.duration(), 509);
        assert_eq!(component.meta().id, "text-messages");
        assert_eq!(component.background(), Some(COLORS.background));
    }

    #[test]
    fn empty_conversation_still_has_a_positive_duration() {
        let component = TextMessages::new(TextMessagesConfig {
            messages: Vec::new(),
            ..TextMessagesConfig::default()
        });
        assert_eq!(component.duration(), 105);
    }

    #[test]
    fn layout_heights_scale_with_the_unit() {
        let metrics = ChatMetrics {
            unit: 1.0,
            divider_width: 100.0,
            bubbles: vec![BubbleMetrics {
                lines: vec!["a".to_owned(), "b".to_owned()],
                text_width: 50.0,
                mine: false,
            }],
        };
        let bubble = &metrics.bubbles[0];
        // Two lines at 31px and 1.42 line height, 16px padding top and bottom, 1px border.
        assert!((bubble_height(bubble, 1.0) - (2.0 * 31.0 * 1.42 + 32.0 + 2.0)).abs() < 1e-4);
        assert!((bubble_height(bubble, 2.0) - 2.0 * bubble_height(bubble, 1.0)).abs() < 1e-4);
        assert!((divider_height(2.0) - 2.0 * divider_height(1.0)).abs() < 1e-4);
        assert!((typing_height(2.0) - 2.0 * typing_height(1.0)).abs() < 1e-4);
        assert!((input_bar_height(2.0) - 2.0 * input_bar_height(1.0)).abs() < 1e-4);
        assert!((stack_height(&metrics, None) - stack_height(&metrics, Some(0))).abs() > 1.0);
    }

    #[test]
    fn the_thread_fits_the_canvas_at_the_source_canvas_height() {
        // Eight one-line bubbles: about 985px of thread on a 1080-tall canvas, which is the case
        // the automatic fit exists for.
        let metrics = ChatMetrics {
            unit: 1.0,
            divider_width: 100.0,
            bubbles: (0..8)
                .map(|_| BubbleMetrics {
                    lines: vec!["x".to_owned()],
                    text_width: 40.0,
                    mine: false,
                })
                .collect(),
        };
        let config = TextMessagesConfig::default();
        let fitted = fitted_unit(1080.0, &metrics, &config);
        assert!(fitted < 1.0, "the demo stack should not fit at 1:1");
        assert!(fitted > MIN_FIT, "the fit should not collapse");

        let scaled = ChatMetrics {
            unit: fitted,
            ..metrics.clone()
        };
        let total =
            stack_height(&scaled, last_mine_index(&config.messages)) + chrome_height(fitted);
        assert!(total <= 1080.0 + 1e-2, "stack {total} still overflows");
    }

    #[test]
    fn the_fit_never_scales_up_and_can_be_disabled() {
        let tiny = ChatMetrics {
            unit: 1.0,
            divider_width: 10.0,
            bubbles: vec![BubbleMetrics {
                lines: vec!["x".to_owned()],
                text_width: 10.0,
                mine: true,
            }],
        };
        // A short conversation on a tall canvas keeps the nominal unit.
        assert!((fitted_unit(1080.0, &tiny, &TextMessagesConfig::default()) - 1.0).abs() < 1e-6);
        // A tall conversation on a short canvas, with the fit off, keeps it too.
        let tall = ChatMetrics {
            unit: 1.0,
            divider_width: 10.0,
            bubbles: (0..40)
                .map(|_| BubbleMetrics {
                    lines: vec!["x".to_owned()],
                    text_width: 10.0,
                    mine: true,
                })
                .collect(),
        };
        let off = TextMessagesConfig {
            fit_to_thread: false,
            ..TextMessagesConfig::default()
        };
        assert!((fitted_unit(1080.0, &tall, &off) - 1.0).abs() < 1e-6);
        assert!(fitted_unit(1080.0, &tall, &TextMessagesConfig::default()) < 1.0);
    }

    /// Build the SVG tree for one frame of `component` on the library's 1920x1080 canvas.
    fn render(component: &dyn crate::Component, local: f32) -> Svgr<'_> {
        let global = local as usize;
        let mut frame = fframes::Frame::new(global, global, 30);
        let meters = crate::embedded_meters();
        let fonts = crate::Fonts::default();
        let info = crate::ctx::ComponentFrame {
            local,
            global,
            fps: 30.0,
            duration: component.duration(),
            width: 1920.0,
            height: 1080.0,
        };
        let mut ctx = RenderCtx::new(&mut frame, fonts, &meters, info);
        let body = component.show(&mut ctx);
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920"
                 height="1080">{body}</svg>
        )
    }

    #[test]
    fn every_frame_renders_without_panicking() {
        let component = TextMessages::default();
        for local in 0..component.duration() {
            let nodes = render(&component, local as f32).as_subtree();
            assert!(!nodes.is_empty(), "frame {local} produced nothing");
        }
    }

    #[test]
    fn the_rendered_svg_tree_parses() {
        let component = TextMessages::default();
        let duration = component.duration();
        // Start, the chrome fade, the first typing window, the first bubble, the middle, the end.
        for local in [0usize, 8, 14, 30, 85, 105, duration / 2, duration - 1] {
            let tree = render(&component, local as f32).into_svg_tree(
                &fframes::usvgr::Options::default(),
                &mut fframes::usvgr::Cache::default(),
                &fframes::usvgr::fontdb::Database::default(),
            );
            assert!(tree.is_ok(), "frame {local} did not parse: {tree:?}");
        }
    }

    #[test]
    fn a_bottom_anchored_thread_renders_too() {
        let component = TextMessages::new(TextMessagesConfig {
            anchor: Anchor::Bottom,
            ..TextMessagesConfig::default()
        });
        for local in [0usize, 105, 300, 508] {
            assert!(!render(&component, local as f32).as_subtree().is_empty());
        }
    }
}
