//! A work-chat transcript fills itself in and scrolls as messages arrive.

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::ui::{Mode, UiTheme, mix_oklch, text, to_hex, with_alpha};

/// The recording's own canvas width, in reference px.
pub const REF_W: f32 = 638.0;
/// The recording's own canvas height, in reference px.
pub const REF_H: f32 = 354.0;

/// Baseline of a group's first message, below the name.
pub const NAME_GAP: f32 = 37.59;
/// Baseline to baseline, within a group.
pub const MSG_GAP: f32 = 46.01;
/// Last message of a group to the next group's name.
pub const GROUP_GAP: f32 = 69.0;
/// Where the oldest message sits before anything has arrived.
pub const START_Y: f32 = 158.9;
/// The line the newest message rests on.
///
/// A row moves the stack by however far its baseline falls *past* this line — zero until the
/// thread is tall enough to reach it. One constant replaces a per-message "does this one scroll"
/// flag, and it is what makes the arithmetic close: `158.90 + 46.01` is above the anchor and moves
/// nothing, and the next two rows put the newest baseline at `311.50` and the scroll settles at
/// exactly `92.02 + 46.01`, which is what the recording measures.
pub const ANCHOR: f32 = 219.48;

/// The avatar is square, and its top sits this far above the name's baseline.
pub const AVATAR: f32 = 70.0;
/// Left of the avatar tile's box.
pub const AVATAR_X: f32 = 119.5;
/// How far the avatar's box top sits above the name's line box top.
pub const AVATAR_RISE: f32 = 28.0;
/// The avatar tile's corner radius.
pub const AVATAR_RADIUS: f32 = 8.0;
/// The avatar's initial, as a share of the tile.
pub const AVATAR_LETTER: f32 = 0.45;
/// Weight of the avatar's initial.
pub const AVATAR_WEIGHT: u16 = 600;
/// Left of the text column's box. Ink starts at 201.1 (name) and 202.4 (body).
pub const TEXT_X: f32 = 202.0;

/// Body type size. Cap height measured at 22.40 px.
pub const SIZE: f32 = 31.2;
/// The timestamp's size, measured on its digits: cap 18.09.
pub const TIME_SIZE: f32 = 24.9;
/// The distance from the top of a text element's line box down to its baseline, at
/// `lineHeight: 1`.
///
/// The source positions every run by its box top — `top: y - ASC` — so its baselines live at the
/// box top plus this. A face metric, measured on the recording, not guessed. This port works in
/// baselines throughout (see [`baseline_px`]), so `ASC` is used only to read the source's own box
/// tops; it never enters the renderer's `y`.
pub const ASC: f32 = 28.95;
/// Weight of the name row and of the message lines.
pub const BODY_WEIGHT: u16 = 400;
/// Weight of the name in the group header.
pub const NAME_WEIGHT: u16 = 700;
/// Weight of the timestamp.
pub const TIME_WEIGHT: u16 = 500;
/// The space between the name's ink and the timestamp, which ride one baseline.
pub const TIME_GAP: f32 = 13.4;

/// Typing dots: on the line the message will take, a little above its baseline.
pub const DOT: f32 = 5.0;
/// Distance between the dots' centres.
pub const DOT_GAP: f32 = 8.4;
/// How far the dots' line sits above the message baseline.
pub const DOT_RISE: f32 = 5.7;
/// The dots' left inset from the text column.
pub const DOT_INSET: f32 = 3.3;
/// A dot every this many seconds, then the ellipsis starts over.
pub const DOT_BEAT: f32 = 0.1;
/// How much of the foreground the dots are mixed up toward.
pub const DOT_MIX: f32 = 0.3;
/// The share of the tile's tint behind the monogram.
pub const AVATAR_TINT: f32 = 0.16;

/// Frames between a group's row opening and the group appearing in it.
///
/// The room is made first and the person arrives into it: the recording starts scrolling on frame
/// 37 and does not paint the avatar, the name or the first dot until frame 40. It is also what the
/// dots are timed from — anchor their cycle to the scroll instead and the ellipsis is a step out of
/// phase for its whole life.
pub const LEAD: f32 = 3.0;
/// Frame the fifth message lands on, and the step every later message takes.
pub const NEXT_MSG: f32 = 24.0;
/// Frame the fifth row opens on, and the step every later row takes.
pub const NEXT_OPEN: f32 = 24.0;

/// One arrival's scroll, as `(seconds since it started, share of the travel)`.
///
/// Recovered by correlating each frame's vertical ink profile against the last — which reads the
/// move off every visible row at once rather than tracking one word — and checked against the
/// absolute baseline of the bold name row, which agrees to a third of a pixel. It accelerates for
/// four frames, peaks at 5.1 px, and then spends three quarters of a second arriving.
pub const SCROLL: &[(f32, f32)] = &[
    (0.0, 0.0),
    (0.0167, 0.004),
    (0.05, 0.016),
    (0.0833, 0.044),
    (0.1167, 0.093),
    (0.15, 0.178),
    (0.1833, 0.289),
    (0.2167, 0.398),
    (0.25, 0.486),
    (0.2833, 0.557),
    (0.3167, 0.616),
    (0.35, 0.663),
    (0.3833, 0.702),
    (0.4167, 0.737),
    (0.45, 0.767),
    (0.4833, 0.791),
    (0.5167, 0.814),
    (0.55, 0.834),
    (0.5833, 0.852),
    (0.6167, 0.867),
    (0.65, 0.881),
    (0.6917, 0.893),
    (0.7167, 0.903),
    (0.75, 0.912),
    (0.7833, 0.919),
    (1.1, 1.0),
];

/// How far a line is faded, by its height in the frame.
///
/// Read as an envelope over the whole clip: the brightest any pixel of a line ever got at each
/// height, taken as a maximum across every frame, which is what makes it a property of the
/// *position* rather than of the line sitting there. It stops at a tenth rather than at nothing:
/// the oldest line is cut off by the frame, not erased by the fade.
pub const FADE: &[(f32, f32)] = &[
    (10.0, 0.098),
    (25.0, 0.125),
    (40.0, 0.149),
    (55.0, 0.216),
    (70.0, 0.357),
    (85.0, 0.518),
    (100.0, 0.651),
    (115.0, 0.843),
    (140.0, 0.98),
    (175.0, 1.0),
];

/// The frame each message lands on, for the recording's own four arrivals. After them: every 24.
const BEATS: &[f32] = &[0.0, 12.0, 60.0, 84.0];
/// The frame each row opens on, before its words land. After them: every 24.
const OPENS: &[f32] = &[0.0, 12.0, 37.0, 72.0];

/// The picture the demo gives rhea: `media/avatar1.png`, embedded by [`crate::ComponentsMedia`].
///
/// A [`ThreadMessage::avatar`] is a *name* in the host's media provider, and this is the one the
/// crate itself ships for the first speaker. The source's own demo uses the photograph
/// `/avatars/07.jpg` here; naming that path would resolve to nothing, because the file is not in
/// `media/`.
pub const AVATAR_ONE: &str = "avatar1.png";
/// The picture the demo gives sam: `media/avatar2.png`. See [`AVATAR_ONE`].
pub const AVATAR_TWO: &str = "avatar2.png";

/// The recording's demo conversation, exactly as the source's `DEFAULT_MESSAGES`.
///
/// The tuples are the source's own `[author, time, face, text]`: the source's `config.ts` carries
/// the same four messages in the string form ([`parse_messages`] reads it), and its two photographs
/// are `/avatars/07.jpg` and `/avatars/13.jpg`. This crate points at the two pictures embedded in
/// `media/` instead — [`AVATAR_ONE`] and [`AVATAR_TWO`] — so a bare render of this component shows
/// faces rather than monograms.
pub const DEFAULT_MESSAGES: &[(&str, &str, &str, &str)] = &[
    ("rhea", "9:41 AM", AVATAR_ONE, "Launch video by Thursday?"),
    ("rhea", "9:41 AM", AVATAR_ONE, "We have nothing shot."),
    ("sam", "9:42 AM", AVATAR_TWO, "Already done."),
    ("sam", "9:42 AM", AVATAR_TWO, "Built it with fframes."),
];

/// One message in the conversation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ThreadMessage {
    /// What was said.
    pub text: String,
    /// Who said it. Also what the monogram shows when the message heads a group and names no
    /// picture.
    pub author: String,
    /// The quiet timestamp beside the name.
    pub time: String,
    /// The speaker's picture, as a name in the host's media provider (`avatar1.png` for the file
    /// embedded in `media/`). Empty draws the source's own monogram tile instead.
    pub avatar: String,
    /// Frame the words land on. `None` takes the measured schedule
    /// (`0, 12, 60, 84`, then every 24).
    pub at: Option<u32>,
    /// Frame the row opens on, before the words land — typing dots, if it heads a group.
    /// `None` takes the measured schedule (`0, 12, 37, 72`, then every 24).
    pub opens: Option<u32>,
}

impl ThreadMessage {
    /// A message from `author` at `time` with no picture, and the measured schedule.
    ///
    /// No picture means the source's monogram tile; [`ThreadMessage::with_avatar`] names one.
    #[must_use]
    pub fn new(author: &str, time: &str, text: &str) -> Self {
        Self {
            text: text.to_owned(),
            author: author.to_owned(),
            time: time.to_owned(),
            avatar: String::new(),
            at: None,
            opens: None,
        }
    }

    /// A message from `author` at `time` whose picture is `avatar`, and the measured schedule.
    ///
    /// `avatar` is a name the host's media provider resolves — the crate ships `avatar1.png` and
    /// `avatar2.png` in `media/`. See the module's fidelity notes for what an unresolved name does.
    #[must_use]
    pub fn with_avatar(author: &str, time: &str, avatar: &str, text: &str) -> Self {
        Self {
            avatar: avatar.to_owned(),
            ..Self::new(author, time, text)
        }
    }
}

/// One group of messages under a single name — what the channel reads as one arrival.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadGroup {
    /// Who is speaking.
    pub person: String,
    /// Their timestamp.
    pub time: String,
    /// The picture that heads the group, as a name in the host's media provider. Empty means the
    /// source's monogram tile. Part of the group key, as it is in the source.
    pub avatar: String,
    /// The messages in the group, in order.
    pub lines: Vec<String>,
}

/// One laid-out line of the transcript, in the recording's own unscaled pixels.
///
/// Positions are absolute and never re-flow: the scroll is a single translate over the top of
/// them. That is what lets every arrival be a number rather than a layout pass, and it is why the
/// stack can be perfectly still between them.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Baseline in transcript space, before any scroll.
    pub y: f32,
    /// Baseline of this group's name row, when this line heads a group.
    pub name_y: f32,
    /// Index of the group this line belongs to.
    pub group: usize,
    /// First line of a group, i.e. the one the name and avatar belong to.
    pub head: bool,
    /// The words.
    pub text: String,
    /// Frame the words land on. The row itself opens at `opens`.
    pub at: f32,
    /// Frame the row opens on, before the words land. `-inf` for the very first row, which is
    /// already open on frame 0.
    pub opens: f32,
}

/// Linear read of a rising table, clamped at both ends.
///
/// Values outside the table take its endpoints, which is what the source's own `read` does. A
/// table that is empty reads as `0` rather than panicking.
#[must_use]
pub fn read(table: &[(f32, f32)], x: f32) -> f32 {
    let (Some(first), Some(last)) = (table.first(), table.last()) else {
        return 0.0;
    };
    if x <= first.0 {
        return first.1;
    }
    if x >= last.0 {
        return last.1;
    }
    for pair in table.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        if x <= hi.0 {
            let span = hi.0 - lo.0;
            if span == 0.0 {
                return hi.1;
            }
            return lo.1 + (hi.1 - lo.1) * (x - lo.0) / span;
        }
    }
    last.1
}

/// The string form of the conversation: messages split by `|`, each `avatar > name time > text`.
///
/// The name runs to the first space and the rest of that part is the timestamp. A message that is
/// only text continues the one above it, keeping that speaker's name, time and picture. Blank parts
/// are dropped, exactly as the source's `toMessages` does — and so is text that trims to nothing.
///
/// ```text
/// "avatar1.png > rhea 9:41 AM > Launch video by Thursday? | We have nothing shot."
/// ```
///
/// The avatar is carried through verbatim as [`ThreadMessage::avatar`], so the source's own
/// `/avatars/07.jpg` comes out as `/avatars/07.jpg`; whether that resolves is the host's business.
#[must_use]
pub fn parse_messages(input: &str) -> Vec<ThreadMessage> {
    let mut head = (String::new(), String::new(), String::new());
    let mut out = Vec::new();

    for part in input.split('|') {
        if part.trim().is_empty() {
            continue;
        }
        let mut fields: Vec<String> = part
            .split('>')
            .map(|field| field.trim().to_owned())
            .collect();
        let text = fields.pop().unwrap_or_default();
        if let Some(who) = fields.pop() {
            let cut = who.find(' ');
            head = match cut {
                Some(index) => (
                    who[..index].to_owned(),
                    who[index + 1..].to_owned(),
                    fields.pop().unwrap_or_default(),
                ),
                None => (who, String::new(), fields.pop().unwrap_or_default()),
            };
        }
        if text.trim().is_empty() {
            continue;
        }
        out.push(ThreadMessage {
            text,
            author: head.0.clone(),
            time: head.1.clone(),
            avatar: head.2.clone(),
            at: None,
            opens: None,
        });
    }

    out
}

/// Group consecutive messages by author, time and avatar.
///
/// A message with the same author, time and picture as the one above it joins that group, under one
/// name — which is why a second message from the same person never repeats it. The picture is part
/// of the key because the source's own rule is `person && time && avatar`: the same person at the
/// same time with a different face is a new arrival, and heads its own group.
#[must_use]
pub fn group_messages(messages: &[ThreadMessage]) -> Vec<ThreadGroup> {
    let mut groups: Vec<ThreadGroup> = Vec::new();
    for message in messages {
        if message.text.trim().is_empty() {
            continue;
        }
        match groups.last_mut() {
            Some(group)
                if group.person == message.author
                    && group.time == message.time
                    && group.avatar == message.avatar =>
            {
                group.lines.push(message.text.trim().to_owned());
            }
            _ => groups.push(ThreadGroup {
                person: message.author.clone(),
                time: message.time.clone(),
                avatar: message.avatar.clone(),
                lines: vec![message.text.trim().to_owned()],
            }),
        }
    }
    groups
}

/// Lay the transcript out once, in transcript space.
#[must_use]
pub fn layout(groups: &[ThreadGroup]) -> Vec<Line> {
    let mut out = Vec::new();
    let mut last = START_Y - NAME_GAP;
    let mut first = true;

    for (index, group) in groups.iter().enumerate() {
        for (line_index, text) in group.lines.iter().enumerate() {
            let head = line_index == 0;
            let name_y = if head {
                if first { last } else { last + GROUP_GAP }
            } else {
                0.0
            };
            let y = if head {
                name_y + NAME_GAP
            } else {
                last + MSG_GAP
            };
            out.push(Line {
                y,
                name_y,
                group: index,
                head,
                text: text.clone(),
                at: 0.0,
                opens: 0.0,
            });
            last = y;
            first = false;
        }
    }

    out
}

/// The frame the message at `index` lands on: the measured four, then every [`NEXT_MSG`].
#[must_use]
pub fn land_frame(index: usize, message: Option<&ThreadMessage>) -> f32 {
    if let Some(frame) = message.and_then(|message| message.at) {
        return frame as f32;
    }
    match BEATS.get(index) {
        Some(frame) => *frame,
        None => 84.0 + NEXT_MSG * (index as f32 - 3.0),
    }
}

/// The frame the row at `index` opens on: the measured four, then every [`NEXT_OPEN`].
///
/// The very first row is already open on frame `0`, which the source spells as
/// `Number.NEGATIVE_INFINITY` — there is no frame at which it has not opened.
#[must_use]
pub fn open_frame(index: usize, message: Option<&ThreadMessage>) -> f32 {
    if index == 0 {
        return f32::NEG_INFINITY;
    }
    if let Some(frame) = message.and_then(|message| message.opens) {
        return frame as f32;
    }
    match OPENS.get(index) {
        Some(frame) => *frame,
        None => 72.0 + NEXT_OPEN * (index as f32 - 3.0),
    }
}

/// Give every laid-out line its schedule: the frame its words land on and the frame its row opens.
///
/// The schedule is the source's own measured one, and it is what the whole scene is timed by —
/// every arrival, every scroll and every dot beat comes from these two numbers per line.
#[must_use]
pub fn schedule(lines: &[Line], messages: &[ThreadMessage]) -> Vec<Line> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| Line {
            at: land_frame(index, messages.get(index)),
            opens: open_frame(index, messages.get(index)),
            ..line.clone()
        })
        .collect()
}

/// The whole transcript at one frame: laid out, scheduled and scrolled.
///
/// This is the single entry point `show` draws from, so a test against it is a test against what
/// is rendered.
#[must_use]
pub fn scene(messages: &[ThreadMessage], frame: f32, fps: f32) -> ThreadFrame {
    let lines = schedule(&layout(&group_messages(messages)), messages);
    ThreadFrame::resolve(lines, frame, fps)
}

/// The scroll still to travel for one opened row: `max(0, y - ANCHOR)`.
///
/// A row that does not reach the anchor moves nothing — that is the stillness in the reference,
/// rather than an exception carved out for it.
#[must_use]
pub fn scroll_want(line: &Line) -> f32 {
    (line.y - ANCHOR).max(0.0)
}

/// How much faster a longer move is read: `(step / MSG_GAP)^0.68`.
///
/// Measured, twice: the recording's two-row scroll runs 1.60x faster in normalised time than its
/// one-row scroll, which is what a browser's native smooth scroll does with distance and what the
/// recording therefore does. One curve, read faster the further it has to go.
#[must_use]
pub fn scroll_rate(step: f32) -> f32 {
    (step / MSG_GAP).powf(0.68)
}

/// How many of the three dots have arrived, `seconds` after the group appeared.
///
/// The ellipsis fills over three beats and starts over on the fourth — dot `i` is drawn while
/// `i < step` — which the source spells as `floor((since % (DOT_BEAT * 4)) / DOT_BEAT)`. This
/// counts the beats first and then takes their remainder, which is the same walk on whole and
/// fractional beat counts alike and does not lose the fourth beat to a float remainder.
#[must_use]
pub fn dot_step(since: f32) -> f32 {
    let beat = if DOT_BEAT > 0.0 { DOT_BEAT } else { 0.1 };
    let beats = (since / beat).floor();
    beats.rem_euclid(4.0)
}

/// How far the stack is scrolled at `frame`, in reference px.
///
/// Every row's own arrival adds its share of the travel, read along [`SCROLL`] from the moment its
/// row opened. A row whose window has passed contributes a constant, so the stack is still between
/// arrivals rather than easing into place.
#[must_use]
pub fn scroll_at(lines: &[Line], frame: f32, fps: f32) -> f32 {
    let now = frame / fps;
    let fps = if fps > 0.0 { fps } else { 30.0 };
    let mut scroll = 0.0;
    let mut target = 0.0;

    for line in lines {
        let want = scroll_want(line);
        let step = want - target;
        target = want;
        if step <= 0.0 {
            continue;
        }
        let rate = scroll_rate(step);
        scroll += step * read(SCROLL, (now - line.opens / fps) * rate);
    }

    scroll
}

/// The transcript's whole state at one frame, in reference px.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadFrame {
    /// The laid-out lines, unscrolled.
    pub lines: Vec<Line>,
    /// How far the stack is scrolled at this frame.
    pub scroll: f32,
    /// Whether each line is on screen at all, by the source's own `y` window.
    pub visible: Vec<bool>,
}

impl ThreadFrame {
    /// Resolve the transcript at `frame`.
    #[must_use]
    pub fn resolve(lines: Vec<Line>, frame: f32, fps: f32) -> Self {
        let scroll = scroll_at(&lines, frame, fps);
        let visible = lines
            .iter()
            .map(|line| {
                let y = line.y - scroll;
                (-MSG_GAP..=REF_H + MSG_GAP).contains(&y)
            })
            .collect();
        Self {
            lines,
            scroll,
            visible,
        }
    }

    /// A line's baseline on the canvas, before the canvas scale.
    #[must_use]
    pub fn baseline(&self, index: usize) -> Option<f32> {
        self.lines.get(index).map(|line| line.y - self.scroll)
    }

    /// The opacity of a line's *body*, read from the fade table at the scrolled baseline.
    #[must_use]
    pub fn body_alpha(&self, index: usize) -> f32 {
        self.lines
            .get(index)
            .map_or(0.0, |line| read(FADE, line.y - self.scroll))
    }

    /// The opacity of the avatar, read at the *centre* of the tile rather than at the name.
    #[must_use]
    pub fn avatar_alpha(&self, index: usize) -> f32 {
        self.lines.get(index).map_or(0.0, |line| {
            read(
                FADE,
                line.y - NAME_GAP - AVATAR_RISE + AVATAR / 2.0 - self.scroll,
            )
        })
    }
}

/// A transcript-space baseline, in canvas px.
///
/// The whole scene is laid out on baselines — the source's `y` values are CSS box tops only
/// because it also subtracts [`ASC`] from them, and what that arithmetic produces is a baseline —
/// and this renderer's `<text y>` **is** the baseline, so the mapping is one multiply:
///
/// ```text
/// canvas_baseline = reference_baseline * unit
/// ```
///
/// There is deliberately no box-top correction here. An earlier draft of this port subtracted the
/// face's box-top offset from every baseline, as if `<text y>` were the top of the line box. That
/// premise is false, and the error is measurable rather than arguable: it drew row 0's message
/// baseline at **559.2** at 1080p where the source's is `484.77` — 74 px too low, and every other
/// row with it. The pre-fix ink band was rows `488..579`; post-fix it is `413..504`. The README's
/// fidelity notes record the measurement.
#[must_use]
pub fn baseline_px(reference_baseline: f32, unit: f32) -> f32 {
    reference_baseline * unit
}

/// The scene's scale on a real canvas: the source's own `height / REF_H`.
///
/// This is **not** the collection's uniform `ctx.vmin() / REFERENCE_VMIN` scale that
/// [`crate::ui::text::unit`] returns. A 638 x 354 recording is 1.802 : 1 rather than 16:9,
/// so the two differ by `1 - (354 / 638) / (9 / 16)`, i.e. **1.4%** — 116 px at 1080p, far more
/// than the recording's own measurement noise.
///
/// The decider is **vertical**, because the horizontal is insensitive: at 960 x 540 the demo's own
/// frame 0 puts rhea's tile at x 175.5, where this scale predicts 175.7 and `vmin / 7.2` predicts
/// 179.3 — a 3.6 px gap that the recording's own bleed can swallow. Vertically there is no such
/// ambiguity: at the settle the demo's five baselines sit at 31.8, 102.2, 207.3, 264.6 and 334.8,
/// which is `20.87, 66.88, 135.88, 173.47, 219.48` times `height / REF_H` = 1.5254 to within half
/// a pixel, and `vmin / 7.2` = 0.75 would put them at half that. A 1280 x 720 render shown at
/// 960 x 540 has exactly one scale, and it is this one.
///
/// Two consequences, both the source's own and both kept:
///
/// * the box is `638 * unit` wide — 1946.44 px at 1080p, 26.44 px wider than the canvas — and the
///   source centres it, so the box's left edge is at -13.2. The type lives between x = 614 and
///   x = 1200 there, well inside the frame, so nothing visible is lost, and nothing is clipped:
///   this port draws straight onto the canvas.
/// * on a 9:16 portrait canvas `height / REF_H` is large and the transcript sits near the top of
///   the frame. That is what the source does; a portrait host that wants the thread centred should
///   composite it rather than expect the component to re-frame itself.
#[must_use]
pub fn scene_unit(canvas_height: f32) -> f32 {
    canvas_height / REF_H
}

/// Configuration for [`ChannelThread`].
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelThreadConfig {
    /// The conversation, in order. Use [`parse_messages`] for the source's string form.
    pub messages: Vec<ThreadMessage>,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// Message and name ink. `None` takes the theme's `foreground`.
    pub ink: Option<String>,
    /// The colour the timestamp is painted in. `None` takes the theme's `muted_foreground`.
    pub dim_color: Option<String>,
    /// The face to paint in. `None` keeps the theme's face, then the crate's embedded face.
    pub font_family: Option<String>,
    /// Composition length in frames.
    pub duration: usize,
}

impl Default for ChannelThreadConfig {
    fn default() -> Self {
        Self {
            messages: DEFAULT_MESSAGES
                .iter()
                .map(|(author, time, avatar, text)| {
                    ThreadMessage::with_avatar(author, time, avatar, text)
                })
                .collect(),
            // The only scene in the registry that ships dark, and deliberately: a transcript is
            // read against the room it is in, and the reference is white type on black.
            mode: Mode::Dark,
            theme: None,
            ink: None,
            dim_color: None,
            font_family: None,
            duration: 110,
        }
    }
}

/// The **Channel Thread** component.
#[derive(Debug, Clone, Default)]
pub struct ChannelThread {
    /// Component options.
    pub config: ChannelThreadConfig,
    groups: OnceLock<Vec<ThreadGroup>>,
}

impl ChannelThread {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: ChannelThreadConfig) -> Self {
        Self {
            config,
            groups: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "channel-thread",
            name: "Channel Thread",
            summary: "A work chat fills itself in and scrolls once per arrival",
            feature: "channel-thread",
        }
    }

    /// The conversation as groups, measured once per component instance.
    fn groups(&self) -> &[ThreadGroup] {
        self.groups
            .get_or_init(|| group_messages(&self.config.messages))
    }
}

/// The monogram's letter: the first character of `name`, uppercased.
fn monogram(name: &str) -> String {
    name.chars()
        .next()
        .map(|first| first.to_uppercase().collect())
        .unwrap_or_default()
}

impl Component for ChannelThread {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source's own `AbsoluteFill` paints `theme.background`, and `config.ts` ships `dark`.
        Some("#0a0a0b")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let ink = config
            .ink
            .clone()
            .unwrap_or_else(|| theme.foreground.clone());
        let dim = config
            .dim_color
            .clone()
            .unwrap_or_else(|| theme.muted_foreground.clone());
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);

        let unit = scene_unit(ctx.height());
        let fps = if ctx.info.fps > 0.0 {
            ctx.info.fps
        } else {
            30.0
        };
        let frame = ctx.frame();

        let lines = schedule(&layout(self.groups()), &config.messages);
        if lines.is_empty() {
            return Svgr::empty();
        }
        let state = ThreadFrame::resolve(lines, frame, fps);

        // Every number below is the recording's own px, scaled once at the draw boundary, and the
        // box is centred horizontally exactly as the source centres it.
        let box_left = (ctx.width() - REF_W * unit) / 2.0;
        let text_x = box_left + TEXT_X * unit;
        let size = SIZE * unit;
        let time_size = TIME_SIZE * unit;
        let dot_colour = to_hex(mix_oklch(&dim, &ink, DOT_MIX));
        let now = frame / fps;

        let mut nodes: Vec<Svgr<'static>> = Vec::new();

        for (index, line) in state.lines.iter().enumerate() {
            if !state.visible.get(index).copied().unwrap_or(false) {
                continue;
            }
            // The source returns before the row has opened; the first row never has not.
            if frame < line.opens {
                continue;
            }
            let y = line.y - state.scroll;
            let arrived = frame >= line.opens + LEAD;
            let body_alpha = state.body_alpha(index);
            let group = self.groups().get(line.group);

            if line.head
                && arrived
                && let Some(group) = group
            {
                // The avatar: the speaker's own picture when the message names one, and the
                // source's monogram tile when it does not. Both are the source's own box, radius
                // and fade.
                let tile_alpha = state.avatar_alpha(index);
                let avatar_left = box_left + AVATAR_X * unit;
                let avatar_top = (y - NAME_GAP - AVATAR_RISE) * unit;
                let avatar_side = AVATAR * unit;
                if tile_alpha > 0.0 {
                    if group.avatar.is_empty() {
                        let tile = fframes::svgr!(
                            <rect x={avatar_left} y={avatar_top} width={avatar_side}
                                  height={avatar_side} rx={AVATAR_RADIUS * unit}
                                  fill={with_alpha(&theme.primary, AVATAR_TINT)}
                                  opacity={tile_alpha} />
                        );
                        nodes.push(tile);

                        let letter = monogram(&group.person);
                        let letter_size = AVATAR * AVATAR_LETTER * unit;
                        // The tile centres its glyph with `align-items: center`, so the letter's own
                        // box is centred on the tile's centre and the baseline follows from it.
                        let letter_centre = avatar_top + avatar_side / 2.0;
                        let letter_baseline = text::baseline_for_centred_box(
                            ctx,
                            &family,
                            letter_size,
                            letter_centre,
                        );
                        nodes.push(text::text_node(
                            ctx,
                            &letter,
                            avatar_left + avatar_side / 2.0,
                            letter_baseline,
                            family.clone(),
                            letter_size,
                            AVATAR_WEIGHT,
                            0.0,
                            theme.primary.clone(),
                            tile_alpha,
                        ));
                    } else {
                        // `object-fit: cover` is `preserveAspectRatio="xMidYMid slice"`, and the
                        // source's `borderRadius: 8` is a clip rather than a rounded rect because
                        // what is being rounded is a picture. The id is the line's own index —
                        // exactly one head line per group, so it names this tile and nothing else in
                        // the frame.
                        let clip = format!("channel-thread-avatar-{index}");
                        nodes.push(fframes::svgr!(
                            <defs>
                                <clipPath id={clip.clone()}>
                                    <rect x={avatar_left} y={avatar_top} width={avatar_side}
                                          height={avatar_side} rx={AVATAR_RADIUS * unit} />
                                </clipPath>
                            </defs>
                        ));
                        // A `href` the host's media provider cannot resolve draws nothing at all,
                        // which is what the source does with an image that fails to load: see the
                        // fidelity notes. The clip keeps the picture inside the tile either way.
                        nodes.push(fframes::svgr!(
                            <image href={group.avatar.clone()} x={avatar_left} y={avatar_top}
                                   width={avatar_side} height={avatar_side}
                                   preserveAspectRatio="xMidYMid slice"
                                   clip-path={format!("url(#{clip})")} opacity={tile_alpha} />
                        ));
                    }
                }

                // The name, and the timestamp riding its baseline as one inline run.
                let name_alpha = read(FADE, y - NAME_GAP);
                let name_left = box_left + TEXT_X * unit;
                let name_width =
                    text::tracked_width_at(ctx, &family, NAME_WEIGHT, size, &group.person, 0.0);
                let baseline = baseline_px(y - NAME_GAP, unit);
                if name_alpha > 0.0 {
                    nodes.push(text::text_node(
                        ctx,
                        &group.person,
                        name_left + name_width / 2.0,
                        baseline,
                        family.clone(),
                        size,
                        NAME_WEIGHT,
                        0.0,
                        ink.clone(),
                        name_alpha,
                    ));
                }
                if !group.time.is_empty() {
                    let time_width = text::tracked_width_at(
                        ctx,
                        &family,
                        TIME_WEIGHT,
                        time_size,
                        &group.time,
                        0.0,
                    );
                    // The timestamp rides the name's baseline, one inline run in the source.
                    nodes.push(text::text_node(
                        ctx,
                        &group.time,
                        name_left + name_width + TIME_GAP * unit + time_width / 2.0,
                        baseline,
                        family.clone(),
                        time_size,
                        TIME_WEIGHT,
                        0.0,
                        dim.clone(),
                        name_alpha,
                    ));
                }
            }

            if frame >= line.at {
                let body_width =
                    text::tracked_width_at(ctx, &family, BODY_WEIGHT, size, &line.text, 0.0);
                nodes.push(text::text_node(
                    ctx,
                    &line.text,
                    text_x + body_width / 2.0,
                    baseline_px(y, unit),
                    family.clone(),
                    size,
                    BODY_WEIGHT,
                    0.0,
                    ink.clone(),
                    body_alpha,
                ));
            } else if line.head && arrived && group.is_some() {
                // Three dots that arrive one at a time, then start over.
                let since = now - (line.opens + LEAD) / fps;
                let step = dot_step(since);
                let dots_y = (y - DOT_RISE) * unit;
                for index in 0..3 {
                    if f32::from(u8::try_from(index).unwrap_or(0)) >= step {
                        continue;
                    }
                    let dot_left =
                        box_left + (TEXT_X + DOT_INSET) * unit + index as f32 * DOT_GAP * unit;
                    nodes.push(fframes::svgr!(
                        <circle cx={dot_left + DOT * unit / 2.0} cy={dots_y}
                                r={DOT * unit / 2.0} fill={dot_colour.clone()}
                                opacity={body_alpha} />
                    ));
                }
            }
        }

        let nodes = Svgr::from(nodes);
        // The source clips its whole scene to a fixed-size wrapper; the recording's own frame is
        // the wrapper's equivalent here, and it is what a browser does with the box's `overflow`
        // anyway. Rows scrolled past either edge are cut off by the frame — which is what the
        // fade's own floor is about — and nothing inside the frame is affected.
        fframes::svgr!(
            <defs>
                <clipPath id="channel-thread-stage">
                    <rect x={0.0} y={0.0} width={ctx.width()} height={ctx.height()} />
                </clipPath>
            </defs>
            <g clip-path="url(#channel-thread-stage)">{nodes}</g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The source's own test script: two speakers, two messages each, neither with a picture.
    fn script() -> Vec<ThreadGroup> {
        vec![
            ThreadGroup {
                person: "abby".to_owned(),
                time: "1:01 PM".to_owned(),
                avatar: String::new(),
                lines: vec!["one".to_owned(), "two".to_owned()],
            },
            ThreadGroup {
                person: "finn".to_owned(),
                time: "1:02 PM".to_owned(),
                avatar: String::new(),
                lines: vec!["three".to_owned(), "four".to_owned()],
            },
        ]
    }

    /// The default conversation, laid out **and scheduled** exactly as `show` builds it.
    ///
    /// This goes through [`scene`], which is the same call `show` makes, so a test against these
    /// lines is a test against what is drawn rather than against the constants alone.
    fn default_lines() -> Vec<Line> {
        scene(&ChannelThreadConfig::default().messages, 0.0, 30.0).lines
    }

    /// The default conversation at `frame`.
    fn default_scene(frame: f32) -> ThreadFrame {
        scene(&ChannelThreadConfig::default().messages, frame, 30.0)
    }

    #[test]
    fn duration_defaults_to_the_source_composition_length() {
        assert_eq!(ChannelThread::default().duration(), 110);
        assert_eq!(ChannelThread::default().meta().id, "channel-thread");
        assert_eq!(ChannelThread::default().meta().feature, "channel-thread");
    }

    #[test]
    fn the_source_defaults_are_reproduced_exactly() {
        // `config.ts` ships the demo copy, `mode: "dark"`, `durationInFrames: 110`.
        let config = ChannelThreadConfig::default();
        assert_eq!(config.messages.len(), 4);
        assert_eq!(
            config.messages[0],
            ThreadMessage::with_avatar("rhea", "9:41 AM", AVATAR_ONE, "Launch video by Thursday?")
        );
        assert_eq!(
            config.messages[1],
            ThreadMessage::with_avatar("rhea", "9:41 AM", AVATAR_ONE, "We have nothing shot.")
        );
        assert_eq!(
            config.messages[2],
            ThreadMessage::with_avatar("sam", "9:42 AM", AVATAR_TWO, "Already done.")
        );
        assert_eq!(
            config.messages[3],
            ThreadMessage::with_avatar("sam", "9:42 AM", AVATAR_TWO, "Built it with fframes.")
        );
        assert!(config.messages.iter().all(|message| message.at.is_none()));
        assert!(
            config
                .messages
                .iter()
                .all(|message| message.opens.is_none())
        );
        assert_eq!(config.mode, Mode::Dark);
        assert_eq!(config.theme, None);
        assert_eq!(config.ink, None);
        assert_eq!(config.dim_color, None);
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 110);
        assert_eq!(ChannelThread::default().background(), Some("#0a0a0b"));
    }

    #[test]
    fn the_demo_pictures_are_the_files_the_crate_embeds() {
        // The two names the default conversation carries have to be the two pictures in `media/`,
        // because `ComponentsMedia` is what resolves them by name — a typo here is a blank tile in
        // every render rather than a compile error. The size is checked too, so the file is a
        // picture and not a placeholder that happens to have the right name.
        let media = crate::ComponentsMedia::new().expect("embedded media");
        let embedded = [
            (AVATAR_ONE, media.avatar1_png, (199_u32, 199_u32)),
            (AVATAR_TWO, media.avatar2_png, (199, 200)),
        ];
        for (name, image, (width, height)) in embedded {
            assert_eq!(image.metadata.width, width, "{name} width");
            assert_eq!(image.metadata.height, height, "{name} height");
        }
        // The demo is the only place either name appears, and each speaker keeps their own.
        assert_eq!(
            DEFAULT_MESSAGES
                .iter()
                .map(|(.., avatar, _)| *avatar)
                .collect::<Vec<_>>(),
            vec![AVATAR_ONE, AVATAR_ONE, AVATAR_TWO, AVATAR_TWO]
        );
        let groups = group_messages(&ChannelThreadConfig::default().messages);
        assert_eq!(groups[0].avatar, AVATAR_ONE);
        assert_eq!(groups[1].avatar, AVATAR_TWO);
    }

    #[test]
    fn the_source_string_form_continues_the_speaker_above() {
        // The avatar of the head is carried into the continuation, exactly as the name and the
        // time are: `/a.jpg` stays on both of rhea's messages.
        assert_eq!(
            parse_messages("/a.jpg > rhea 9:41 AM > Hi | There | sam > Yo"),
            vec![
                ThreadMessage::with_avatar("rhea", "9:41 AM", "/a.jpg", "Hi"),
                ThreadMessage::with_avatar("rhea", "9:41 AM", "/a.jpg", "There"),
                ThreadMessage::new("sam", "", "Yo"),
            ]
        );
    }

    #[test]
    fn the_string_form_is_the_configs_own_demo_copy() {
        // The same four messages as the shipped default, in the string form: the crate's own
        // picture names, so the parsed conversation is the default one field for field.
        let parsed = parse_messages(
            "avatar1.png > rhea 9:41 AM > Launch video by Thursday? | We have nothing shot. \
             | avatar2.png > sam 9:42 AM > Already done. | Built it with fframes.",
        );
        assert_eq!(parsed, ChannelThreadConfig::default().messages);
        assert_eq!(group_messages(&parsed).len(), 2);

        // The source's own string — its photographs rather than this crate's pictures — parses to
        // the same schedule with those paths kept verbatim, so a host that provides them by those
        // names gets the source's own demo copy.
        let source = parse_messages(
            "/avatars/07.jpg > rhea 9:41 AM > Launch video by Thursday? | We have nothing shot. \
             | /avatars/13.jpg > sam 9:42 AM > Already done. | Built it with fframes.",
        );
        assert_eq!(source.len(), 4);
        assert_eq!(source[0].avatar, "/avatars/07.jpg");
        assert_eq!(source[2].avatar, "/avatars/13.jpg");
        assert_eq!(source[0].text, parsed[0].text);
        assert_eq!(source[3].text, parsed[3].text);
    }

    #[test]
    fn a_second_message_from_the_same_person_joins_the_group() {
        let groups = group_messages(&ChannelThreadConfig::default().messages);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].person, "rhea");
        assert_eq!(
            groups[0].lines,
            vec!["Launch video by Thursday?", "We have nothing shot."]
        );
        assert_eq!(groups[1].person, "sam");
        assert_eq!(
            groups[1].lines,
            vec!["Already done.", "Built it with fframes."]
        );
        // A different time is a different arrival, even from the same person.
        let shifted: Vec<ThreadMessage> = ChannelThreadConfig::default()
            .messages
            .iter()
            .map(|message| {
                let mut next = message.clone();
                next.time = "9:43 AM".to_owned();
                next
            })
            .collect();
        // Both of rhea's messages still share a time, so they still share a group.
        assert_eq!(group_messages(&shifted).len(), 2);
        let mut split = shifted;
        split[1].time = "9:44 AM".to_owned();
        assert_eq!(group_messages(&split).len(), 3);
        // A different picture is a different arrival too, which is the source's own key
        // (`person && time && avatar`): the same person, at the same minute, with a new face.
        let mut refaced = ChannelThreadConfig::default().messages;
        refaced[1].avatar = AVATAR_TWO.to_owned();
        assert_eq!(group_messages(&refaced).len(), 3);
        let refaced = group_messages(&refaced);
        assert_eq!(refaced[0].lines, vec!["Launch video by Thursday?"]);
        assert_eq!(refaced[1].avatar, AVATAR_TWO);
        assert_eq!(refaced[1].lines, vec!["We have nothing shot."]);
        // Blank text never makes a group of its own.
        let mut blank = ChannelThreadConfig::default().messages;
        blank.push(ThreadMessage::new("ghost", "9:50 AM", "   "));
        assert_eq!(group_messages(&blank).len(), 2);
    }

    #[test]
    fn a_message_with_no_picture_takes_the_monogram_path() {
        // `ThreadMessage::new` names no picture, so the group's avatar is empty and `show` draws
        // the source's own monogram tile; `with_avatar` names one, and the same message draws a
        // picture. The group carries whichever the message carried.
        let plain = ThreadMessage::new("rhea", "9:41 AM", "Hi");
        assert!(plain.avatar.is_empty());
        let named = ThreadMessage::with_avatar("rhea", "9:41 AM", AVATAR_TWO, "Hi");
        assert_eq!(named.avatar, AVATAR_TWO);
        assert_eq!(named.text, plain.text);
        assert_eq!(named.author, plain.author);
        // The default struct is the same as `new`, field for field.
        assert_eq!(ThreadMessage::default(), ThreadMessage::new("", "", ""));
        assert_eq!(group_messages(&[plain])[0].avatar, "");
        assert_eq!(group_messages(&[named])[0].avatar, AVATAR_TWO);
    }

    #[test]
    fn the_transcript_stacks_on_the_measured_grid() {
        let lines = layout(&script());
        assert_eq!(
            lines.iter().map(|line| line.y).collect::<Vec<_>>(),
            vec![
                START_Y,
                START_Y + MSG_GAP,
                START_Y + MSG_GAP + GROUP_GAP + NAME_GAP,
                START_Y + MSG_GAP + GROUP_GAP + NAME_GAP + MSG_GAP,
            ]
        );
        assert!((lines[2].y - 311.5).abs() < 0.01, "{}", lines[2].y);
        assert!((lines[3].y - 357.51).abs() < 0.01, "{}", lines[3].y);
    }

    #[test]
    fn a_group_is_headed_by_its_name_and_never_repeats_it() {
        let lines = layout(&script());
        assert_eq!(
            lines.iter().map(|line| line.head).collect::<Vec<_>>(),
            vec![true, false, true, false]
        );
        assert!((lines[0].name_y - (START_Y - NAME_GAP)).abs() < 1e-5);
        // Only a head carries a name baseline at all.
        assert!((lines[1].name_y).abs() < 1e-6);
        assert_eq!(
            lines.iter().map(|line| line.group).collect::<Vec<_>>(),
            vec![0, 0, 1, 1]
        );
    }

    #[test]
    fn only_the_rows_that_reach_the_anchor_scroll() {
        // The source's own test: the second message is the one the recording leaves motionless.
        let lines = layout(&script());
        let want: Vec<f32> = lines.iter().map(scroll_want).collect();
        assert!((want[0]).abs() < 1e-6);
        assert!((want[1]).abs() < 1e-6);
        assert!((want[2] - 92.02).abs() < 0.01, "{}", want[2]);
        assert!((want[3] - 138.03).abs() < 0.01, "{}", want[3]);
        // …and the oldest line ends up where the recording's last frame has it.
        assert!((START_Y - want[3] - 20.87).abs() < 0.01);
    }

    #[test]
    fn the_measured_recording_arithmetic_closes_to_the_pixel() {
        // The module doc's claim: four rows from `START_Y` land the newest baseline at 357.51 and
        // the oldest at 20.87 once the two arrivals have scrolled, against 21 in the recording.
        // The doc's other half — the newest *name* at 138.9 against a measured 139 — is the
        // source's own arithmetic and is not this scene's: its last name moves with `scroll`,
        // while the source's `nameY` is a fixed transcript-space number. Both are computed here
        // so the difference is pinned rather than papered over.
        let lines = default_lines();
        let scroll = scroll_at(&lines, 219.0, 30.0);
        assert!((scroll - 138.03).abs() < 0.01, "scroll {scroll}");
        let oldest = lines[0].y - scroll;
        assert!((oldest - 20.87).abs() < 0.01, "oldest {oldest}");
        // The newest baseline, before and after the scroll, is exactly what the layout says.
        assert!((lines[3].y - 357.51).abs() < 0.01, "{}", lines[3].y);
        assert!((lines[3].y - scroll - 219.48).abs() < 0.01);
        // The newest group's name baseline in transcript space is 273.91, and the source's own
        // paragraph is about that row *plus* the 2 x 46.01 of scroll under it — which is where its
        // "predicts 138.9" comes from. Both halves are pinned.
        assert!(
            (lines[2].name_y - 273.91).abs() < 0.01,
            "{}",
            lines[2].name_y
        );
        assert!((lines[2].name_y - scroll - 135.88).abs() < 0.02);
        // A line that does not head a group carries no name baseline at all.
        assert!((lines[3].name_y).abs() < 1e-6);
    }

    #[test]
    fn the_scroll_table_rolls_forwards_only_and_arrives() {
        assert!((read(SCROLL, 0.0)).abs() < 1e-6);
        assert!((read(SCROLL, 5.0) - 1.0).abs() < 1e-6);
        assert!((read(SCROLL, -1.0)).abs() < 1e-6);
        for pair in SCROLL.windows(2) {
            assert!(pair[1].0 > pair[0].0, "the seconds repeat at {}", pair[1].0);
            assert!(pair[1].1 > pair[0].1, "the share repeats at {}", pair[1].1);
        }
        assert!((SCROLL[0].1).abs() < 1e-6);
        assert!((SCROLL[SCROLL.len() - 1].1 - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_fade_rises_toward_the_top_of_the_frame_and_stops() {
        assert!(read(FADE, 0.0) < 0.11);
        assert!((read(FADE, 200.0) - 1.0).abs() < 1e-6);
        assert!((read(FADE, 1000.0) - 1.0).abs() < 1e-6);
        for pair in FADE.windows(2) {
            assert!(pair[1].1 > pair[0].1, "the fade repeats at {}", pair[1].0);
        }
        // A line a whole frame height below the top is at best dim, which is why the oldest line
        // reads as distance rather than as a rendering fault. The table holds full ink from 175
        // reference px down, so anything deeper in the frame is simply fully inked.
        assert!((read(FADE, REF_H) - 1.0).abs() < 1e-6);
        // And the fade never reaches zero inside the table: the oldest line is cut off by the
        // frame, not erased by the fade.
        assert!(FADE[0].1 > 0.09);
    }

    #[test]
    fn an_empty_table_reads_zero_rather_than_panicking() {
        assert!((read(&[], 0.5)).abs() < 1e-6);
    }

    #[test]
    fn the_message_schedule_is_the_measured_one() {
        // Default: the four measured arrivals, then every 24 frames.
        assert!((land_frame(0, None) - 0.0).abs() < 1e-6);
        assert!((land_frame(1, None) - 12.0).abs() < 1e-6);
        assert!((land_frame(2, None) - 60.0).abs() < 1e-6);
        assert!((land_frame(3, None) - 84.0).abs() < 1e-6);
        assert!((land_frame(4, None) - 108.0).abs() < 1e-6);
        assert!((land_frame(5, None) - 132.0).abs() < 1e-6);

        assert_eq!(open_frame(0, None), f32::NEG_INFINITY);
        assert!((open_frame(1, None) - 12.0).abs() < 1e-6);
        assert!((open_frame(2, None) - 37.0).abs() < 1e-6);
        assert!((open_frame(3, None) - 72.0).abs() < 1e-6);
        assert!((open_frame(4, None) - 96.0).abs() < 1e-6);

        // A message's own frames win over the schedule.
        let mut message = ThreadMessage::new("x", "1:00", "hi");
        message.at = Some(7);
        message.opens = Some(5);
        assert!((land_frame(9, Some(&message)) - 7.0).abs() < 1e-6);
        assert!((open_frame(9, Some(&message)) - 5.0).abs() < 1e-6);
        // …except for the first row, which is always already open.
        assert_eq!(open_frame(0, Some(&message)), f32::NEG_INFINITY);
    }

    #[test]
    fn the_dots_window_is_the_next_four_beats() {
        // `floor((since % (BEAT * 4)) / BEAT)`: 0, 1, 2, 3, then round again. The fourth beat is
        // the one where the ellipsis is momentarily empty again.
        assert!((dot_step(0.0)).abs() < 1e-6);
        assert!((dot_step(0.05)).abs() < 1e-6);
        assert!((dot_step(0.1) - 1.0).abs() < 1e-6);
        assert!((dot_step(0.2) - 2.0).abs() < 1e-6);
        assert!((dot_step(0.3) - 3.0).abs() < 1e-6);
        assert!((dot_step(0.4)).abs() < 1e-6);
        assert!((dot_step(0.5) - 1.0).abs() < 1e-6);
        // Never more than three, so the ellipsis can never be a fourth dot.
        for tenth in 0..40 {
            let step = dot_step(tenth as f32 * 0.02);
            assert!((0.0..=3.0).contains(&step), "step {step}");
        }
    }

    #[test]
    fn the_transcript_is_perfectly_still_between_arrivals() {
        // The whole product: byte-identical frames between arrivals. The second row opens and
        // lands on frame 12 and asks for no scroll at all, so frames 12..37 are frozen.
        let reference = default_scene(12.0).scroll;
        assert!(
            (reference).abs() < 1e-6,
            "the second message must move nothing"
        );
        for frame in 12..37_u32 {
            let here = default_scene(frame as f32).scroll;
            assert!(
                (here - reference).abs() < 1e-9,
                "frame {frame} moved the stack to {here}"
            );
        }
        // And again after the second scroll has arrived: from 105 on, nothing moves.
        let settled = default_scene(105.0).scroll;
        for frame in 105..110_u32 {
            let here = default_scene(frame as f32).scroll;
            assert!(
                (here - settled).abs() < 1e-9,
                "frame {frame} moved to {here}"
            );
        }
        assert!((settled - 138.03).abs() < 0.01, "settled {settled}");
    }

    #[test]
    fn the_scroll_starts_on_the_frame_a_row_opens_and_lands_once() {
        // Read through `scene`, so this is the schedule `show` actually draws with.
        let scroll = |frame: f32| default_scene(frame).scroll;
        let lines = default_lines();
        // The third row opens on 37 and is the first thing that moves at all.
        assert!((scroll(36.0)).abs() < 1e-6);
        // Frame 37 *is* the row's own opening frame, and the curve starts at zero there, so the
        // opening frame itself is motionless; the move begins on 38.
        assert!(
            (scroll(37.0)).abs() < 1e-6,
            "the opening frame itself must not have moved"
        );
        assert!(scroll(38.0) > 0.0);
        assert!(scroll(38.0) < scroll_want(&lines[2]));
        assert!((scroll(70.0) - 92.02).abs() < 0.01);
        // The move is measured as roughly 0.7 s, which at 30 fps is 21 frames: 37 + 21 = 58.
        let settled_by = (37..70)
            .find(|frame| (scroll(*frame as f32) - 92.02).abs() < 0.01)
            .expect("the one-row move arrives inside its own window");
        assert!(
            (55..=60).contains(&settled_by),
            "a 0.7 s move at 30 fps should land near frame 58, got {settled_by}"
        );
        // It is a single move: the fourth row opens on 72, at which point the third row's travel
        // is already complete and untouched, and the fourth row's own step is *added* to it.
        let at_fourth = scroll(72.0);
        assert!((at_fourth - 92.02).abs() < 0.01, "at fourth {at_fourth}");
        let settled = scroll(109.0);
        assert!((settled - 138.03).abs() < 0.01, "settled {settled}");
        assert!(settled > at_fourth);
        // And the whole travel is the largest target, not the sum of the targets: the second
        // arrival adds the 46.01 it falls past the first.
        assert!(
            (settled - scroll_want(&lines[3])).abs() < 0.02,
            "the total travel is the largest target: {settled} vs {}",
            scroll_want(&lines[3])
        );
    }

    #[test]
    fn a_longer_move_is_read_faster() {
        // `rate = (step / MSG_GAP)^0.68`: one row's move is rate 1, two rows' is 1.6.
        assert!((scroll_rate(MSG_GAP) - 1.0).abs() < 1e-6);
        assert!(
            (scroll_rate(2.0 * MSG_GAP) - 1.60).abs() < 0.01,
            "{}",
            scroll_rate(2.0 * MSG_GAP)
        );
        assert!(scroll_rate(3.0 * MSG_GAP) > scroll_rate(2.0 * MSG_GAP));
    }

    #[test]
    fn the_default_scroll_settles_the_recorded_last_frame() {
        // `registry/__measured__.json` records `settle.base = 110` for this component, and the
        // composition is 110 frames, so the last frame is the settled one by construction. What
        // matters is that the scroll has *arrived* by then rather than still easing.
        let last = default_scene(109.0).scroll;
        let after = default_scene(110.0).scroll;
        assert!((last - after).abs() < 1e-9);
        assert!((last - 138.03).abs() < 0.01, "settled {last}");
        // And it is the final target, not a fraction of it.
        let wants: Vec<f32> = default_scene(0.0).lines.iter().map(scroll_want).collect();
        assert!((last - wants[3]).abs() < 0.02);
    }

    #[test]
    fn the_visible_window_is_the_sources_own_band() {
        let state = default_scene(0.0);
        // Frame 0: the fourth row's baseline is 357.51, which is inside the source's own
        // `y <= REF_H + MSG_GAP` band, so the guard keeps it. Its words have not landed at frame
        // 0 anyway (`at = 84`), so nothing draws there.
        assert!(state.visible[3]);
        assert!(state.visible.iter().all(|visible| *visible));
        // A row scrolled far enough up leaves the band, and so does one far below it.
        let mut pushed = state.lines.clone();
        for line in &mut pushed {
            line.y -= 600.0;
        }
        let pushed = ThreadFrame::resolve(pushed, 0.0, 30.0);
        assert!(pushed.visible.iter().all(|visible| !*visible));
        // At the end every row is on screen, top to bottom.
        let state = default_scene(109.0);
        assert!(state.visible.iter().all(|visible| *visible));
        assert!(
            (state.body_alpha(0) - 0.2).abs() < 0.1,
            "{}",
            state.body_alpha(0)
        );
        assert!((state.body_alpha(3) - 1.0).abs() < 1e-6);
        // The oldest row is dimmer than the newest, which is the transcript reading top-down.
        assert!(state.body_alpha(0) < state.body_alpha(3));
        // The oldest line ends up where the recording's last frame has it.
        assert!((state.lines[0].y - state.scroll - 20.87).abs() < 0.01);
        // And the newest name row sits at the anchor, which is why the stack stopped moving.
        assert!((state.lines[3].y - state.scroll - ANCHOR).abs() < 0.01);
    }

    #[test]
    fn the_source_constants_are_the_measured_ones() {
        assert!((REF_W - 638.0).abs() < 1e-6);
        assert!((REF_H - 354.0).abs() < 1e-6);
        assert!((NAME_GAP - 37.59).abs() < 1e-6);
        assert!((MSG_GAP - 46.01).abs() < 1e-6);
        assert!((GROUP_GAP - 69.0).abs() < 1e-6);
        assert!((START_Y - 158.9).abs() < 1e-6);
        assert!((ANCHOR - 219.48).abs() < 1e-6);
        assert!((AVATAR - 70.0).abs() < 1e-6);
        assert!((AVATAR_X - 119.5).abs() < 1e-6);
        assert!((AVATAR_RISE - 28.0).abs() < 1e-6);
        assert!((TEXT_X - 202.0).abs() < 1e-6);
        assert!((SIZE - 31.2).abs() < 1e-6);
        assert!((TIME_SIZE - 24.9).abs() < 1e-6);
        assert!((ASC - 28.95).abs() < 1e-6);
        assert!((DOT - 5.0).abs() < 1e-6);
        assert!((DOT_GAP - 8.4).abs() < 1e-6);
        assert!((DOT_RISE - 5.7).abs() < 1e-6);
        assert!((DOT_BEAT - 0.1).abs() < 1e-6);
        assert!((LEAD - 3.0).abs() < 1e-6);
        assert_eq!(AVATAR_RADIUS, 8.0);
        assert_eq!(AVATAR_LETTER, 0.45);
        assert_eq!(AVATAR_WEIGHT, 600);
        assert_eq!(BODY_WEIGHT, 400);
        assert_eq!(NAME_WEIGHT, 700);
        assert_eq!(TIME_WEIGHT, 500);
        assert!((TIME_GAP - 13.4).abs() < 1e-6);
        assert!((DOT_INSET - 3.3).abs() < 1e-6);
    }

    #[test]
    fn the_scene_scales_by_the_recordings_own_ratio_not_the_family_unit() {
        // 1920x1080: `height / REF_H` is 3.0508, *not* the family's 1.5. The two are 2.03x apart,
        // so this is the single biggest number in the component and it is pinned here.
        let hd = scene_unit(1080.0);
        assert!((hd - 1080.0 / 354.0).abs() < 1e-5);
        assert!((hd - 3.050_847).abs() < 1e-5, "{hd}");
        assert!(
            hd > 3.0,
            "the family unit of 1.5 would draw this at half size"
        );
        // 1280x720 is the source layout canvas; 960x540 is the reference demo's frame.
        assert!((scene_unit(720.0) - 2.033_898).abs() < 1e-5);
        assert!((scene_unit(540.0) - 1.525_423_7).abs() < 1e-5);
        // The box is wider than a 16:9 canvas, and centred, so the box's left edge is negative.
        assert!((REF_W * hd - 1946.44).abs() < 0.05, "{}", REF_W * hd);
        assert!(((1920.0 - REF_W * hd) / 2.0 + 13.22).abs() < 0.05);
        // On a 720-tall canvas the box comes out 1280 px wide to a fraction of a pixel.
        assert!((REF_W * scene_unit(720.0) - 1297.62).abs() < 0.05);
        // And only at exactly REF_H is the recording reproduced pixel for pixel.
        assert!((scene_unit(REF_H) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_demo_frames_own_ink_positions_are_reproduced() {
        // The decider between the two geometry models, against the demo's own last frame (file
        // frame 219, i.e. local frame 109.5 at the source's 30 fps) at 960 x 540. That frame is the
        // settled one, so every baseline is at a known address — in the recording's own reference
        // px, after the 138.03 px of travel:
        //
        //     row 0  rhea's first message   START_Y - scroll                ->  20.87
        //     row 1  rhea's second          + MSG_GAP                       ->  66.88
        //     row 2  sam's name             + GROUP_GAP - NAME_GAP          -> 135.88
        //     row 3  sam's first message    + GROUP_GAP                     -> 173.47
        //     row 4  sam's second           + GROUP_GAP + MSG_GAP           -> 219.48
        //
        // The demo at 960 x 540 is that times `height / REF_H` = 1.5254, i.e. 31.8, 102.0, 207.3,
        // 264.6 and 334.8 — and the recording's own ink bands put those baselines at 31.8, 102.2,
        // 207.3, 264.6 and 334.8. Under the family unit (`vmin / 7.2` = 0.75 at this size) the
        // same rows land at 15.7, 50.2, 101.9, 130.1 and 164.6, which is wrong by a factor, not a
        // rounding: the recording is 638 wide in a 960-wide frame, so it can only have been made
        // at `height / REF_H`.
        let unit = scene_unit(540.0);
        let family = (960.0_f32).min(540.0) / 100.0 / crate::motion::REFERENCE_VMIN;
        assert!((family - 0.75).abs() < 1e-6);
        assert!((scene_unit(540.0) - 1.525_423_7).abs() < 1e-5);

        let rows: Vec<f32> = default_scene(219.0)
            .lines
            .iter()
            .map(|line| line.y - default_scene(219.0).scroll)
            .collect();
        assert!((rows[0] - 20.87).abs() < 0.01, "{}", rows[0]);
        assert!((rows[1] - 66.88).abs() < 0.01, "{}", rows[1]);
        let label = default_scene(219.0).lines[2].name_y - default_scene(219.0).scroll;
        // The label row is the second group's name, which rides `NAME_GAP` above its first
        // message and is therefore *between* the two rhea rows and sam's first message.
        let expected = [rows[0], rows[1], label, rows[2], rows[3]];
        let measured = [31.8_f32, 102.2, 207.3, 264.6, 334.8];
        for (index, (source, expected)) in expected.iter().zip(measured).enumerate() {
            let mine = source * unit;
            let family_offset = source * family;
            assert!(
                (mine - expected).abs() < 0.6,
                "row {index}: height/REF_H gives {mine}, the recording measures {expected}"
            );
            assert!(
                (mine - expected).abs() < (family_offset - expected).abs(),
                "row {index}: height/REF_H ({mine}) must beat vmin/7.2 ({family_offset}) \
                 against the recording's {expected}"
            );
        }
        // And the horizontal, where the models are only 5 px apart — *inside* the recording's own
        // bleed, so it does not decide anything and is pinned as the design value only. The
        // recording measures the tile's left edge at 175.5 and the name's ink at 303; the model
        // predicts 182.3 and 308.1, and the family unit predicts 179.3 and 303.0. Both are a few
        // px right of the recording, which is what a downscaled, re-encoded render does to a hard
        // edge. The vertical test above is the one that carries the argument.
        // The tile sits 119.5 reference px from the box's left edge, and the text column at 202.
        let box_left = (960.0 - REF_W * unit) / 2.0;
        assert!(
            (box_left - -6.61).abs() < 0.05,
            "the box is wider than the frame: {box_left}"
        );
        assert!(
            (box_left + AVATAR_X * unit - 175.68).abs() < 0.05,
            "tile left edge"
        );
        assert!(
            (box_left + TEXT_X * unit - 301.50).abs() < 0.05,
            "text box left edge"
        );
        // The recording measures the tile's left edge at 175.5 — half a pixel from the model, and
        // the name's ink at 303 against the model's 301.5 + the glyph's own left side bearing.
        assert!(((box_left + AVATAR_X * unit) - 175.5).abs() < 0.5);

        // At 1080p — the canvas this crate renders — the same design values land here. The
        // renderer's own SVG for frame 0 contains a tile whose left edge is 375.76 and whose side
        // is 189.15, which is this arithmetic on the canvas the component is handed.
        // At 1080p — the canvas this crate renders — the same design values land here, and the
        // renderer's own SVG for frame 0 contains a tile whose left edge is 375.76 and whose side
        // is 189.15. (That SVG measures the 1920x1080 canvas this crate was asked for; the `frame
        // --scale` flag lays the component out on a smaller canvas, so its numbers are half again
        // and are not the ones to check here.)
        let hd = scene_unit(1080.0);
        let hd_left = (1920.0 - REF_W * hd) / 2.0;
        assert!((hd - 3.050_847_5).abs() < 1e-5);
        assert!(
            (hd_left - -13.22).abs() < 0.01,
            "the box overhangs: {hd_left}"
        );
        assert!((hd_left + AVATAR_X * hd - 351.36).abs() < 0.01);
        assert!((AVATAR * hd - 213.56).abs() < 0.01);
        assert!((hd_left + TEXT_X * hd - 603.05).abs() < 0.01);
        // Scaled to the demo's own frame the design tile lands at 175.7 and the text box at 301.5;
        // the ink threshold finds the recording's tile at 175.5 and its name ink at 303, the
        // difference being the glyph's own left side bearing. The comparison is *not* asserted
        // here: the only way to render at the demo's size is the `frame --scale` flag, which lays
        // the component out on a smaller canvas and therefore in different canvas px. The vertical
        // argument above needs no such caveat, because both sides of it are in reference px.
        let demo_left = (960.0 - REF_W * unit) / 2.0;
        assert!((demo_left + AVATAR_X * unit - 175.7).abs() < 0.1);
        assert!((demo_left + TEXT_X * unit - 301.5).abs() < 0.1);
    }

    #[test]
    fn the_monogram_is_the_names_first_letter_uppercased() {
        assert_eq!(monogram("rhea"), "R");
        assert_eq!(monogram("sam"), "S");
        assert_eq!(monogram(""), "");
        assert_eq!(monogram("élise"), "É");
    }

    #[test]
    fn the_baseline_mapping_applies_no_box_top_correction() {
        // `<text y>` is the baseline in this renderer, so a reference baseline needs one multiply
        // and nothing else. The correction this test forbids is
        // `(line_height + size - 2 * ascent) / 2`, which an earlier check-cycle probe had
        // recorded as real; measured at 1080p it would put row 0's message ink band at 559.20
        // instead of the source's 484.77.
        let unit = scene_unit(1080.0);
        assert!((baseline_px(START_Y, unit) - 484.77).abs() < 0.05);
        assert!((baseline_px(0.0, unit)).abs() < 1e-6);
        assert!((baseline_px(START_Y, 1.0) - START_Y).abs() < 1e-6);
        // The forbidden correction, for scale: 22 px on a 720-tall canvas and 33 px at 1080p,
        // both of which are visible and neither of which is applied.
        let shift_at = |height: f32| {
            let unit = scene_unit(height);
            let size = SIZE * unit;
            (size * 1.2 + size - 2.0 * size * 0.75) / 2.0
        };
        let shift720 = shift_at(720.0);
        let shift1080 = shift_at(1080.0);
        assert!((shift720 - 22.21).abs() < 0.3, "720 {shift720}");
        assert!((shift1080 - 33.31).abs() < 0.3, "1080 {shift1080}");
        assert!(
            shift1080 > 30.0,
            "the forbidden shift is not negligible: {shift1080}"
        );
        // The error this port actually applied, as measured on the rendered PNG rather than
        // derived: pre-fix, row 0's message ink occupied rows 488..579 at 1080p, whose baseline is
        // 559 (the band top plus Inter's 0.73 em ascender at 95.2 px), where the source's is
        // 484.77. That is **74 px too low** — not a sub-pixel judgement call.
        let delta = 559.2_f32 - 484.77_f32;
        assert!(
            (delta - 74.43).abs() < 0.05,
            "the applied error was {delta} px"
        );
        assert!(delta > 20.0, "the error was visible, not a rounding");
    }

    #[test]
    fn the_drawn_baselines_are_the_measured_ones() {
        // The same check as `the_demo_frames_own_ink_positions_are_reproduced`, but in canvas px
        // through the function `show` actually calls, so a reintroduced box-top correction fails
        // here as well as in the PNG.
        let unit = scene_unit(1080.0);
        let state = default_scene(0.0);
        // Frame 0, no scroll: row 0's message baseline is START_Y and its name is NAME_GAP above.
        assert!((baseline_px(state.lines[0].y - state.scroll, unit) - 484.77).abs() < 0.05);
        assert!(
            (baseline_px(state.lines[0].y - NAME_GAP - state.scroll, unit) - 370.10).abs() < 0.05
        );
        // At the settle every row is on the transcribed grid, and the five canvas baselines divided
        // by the demo's own factor reproduce the recording's five measured demo px.
        let settled = default_scene(219.0);
        let demo = unit / 1.525_423_7;
        let drawn: Vec<f32> = settled
            .lines
            .iter()
            .map(|line| baseline_px(line.y - settled.scroll, unit) / demo)
            .collect();
        let label = baseline_px(settled.lines[2].name_y - settled.scroll, unit) / demo;
        assert!((drawn[0] - 31.84).abs() < 0.6, "{}", drawn[0]);
        assert!((drawn[1] - 102.04).abs() < 0.6, "{}", drawn[1]);
        assert!((label - 207.29).abs() < 0.6, "{label}");
        assert!((drawn[2] - 264.67).abs() < 0.6, "{}", drawn[2]);
        assert!((drawn[3] - 334.79).abs() < 0.6, "{}", drawn[3]);
    }

    #[test]
    fn the_drawn_output_is_byte_identical_between_arrivals() {
        // The stronger form of the stillness claim: not just the scroll, but every number that
        // reaches the renderer — the scroll, the baselines, the schedule and the fade. This is
        // what "the transcript never creeps" means as an artefact rather than as an intent.
        let config = ChannelThreadConfig::default();
        // Frames 12..37: the second message has landed and nothing else has happened yet.
        let reference = scene(&config.messages, 12.0, 30.0);
        for frame in 12..37_u32 {
            assert!(
                scene(&config.messages, frame as f32, 30.0) == reference,
                "frame {frame} differs from frame 12"
            );
        }
        // Frames 105..110: the second scroll has arrived.
        let reference = scene(&config.messages, 105.0, 30.0);
        for frame in 105..=110_u32 {
            assert!(
                scene(&config.messages, frame as f32, 30.0) == reference,
                "frame {frame} differs from frame 105"
            );
        }
        // Frame 37 is the third row's opening frame, and it is identical to frame 36: the curve
        // starts at zero, so the frame a row opens on does not itself move anything.
        assert!(scene(&config.messages, 36.0, 30.0) == scene(&config.messages, 37.0, 30.0));
    }

    #[test]
    fn the_last_frames_are_byte_identical() {
        // The tail the composition is mostly made of: the source asks for 110 frames but the
        // scene's own last movement is long over by then.
        let settled = default_scene(105.0).scroll;
        for frame in 105..=110_u32 {
            assert!((default_scene(frame as f32).scroll - settled).abs() < 1e-9);
        }
        // And the per-frame step decays: under 0.9 reference px at frame 89, under 0.4 at 97,
        // under 0.1 from 105 on. The tail is an exponential settle, not a second movement.
        let step = |frame: u32| {
            default_scene(frame as f32 + 1.0).scroll - default_scene(frame as f32).scroll
        };
        assert!(step(89) < 0.9, "frame 89 -> 90 moved {}", step(89));
        assert!(step(97) < 0.4, "frame 97 -> 98 moved {}", step(97));
        for frame in 89..110_u32 {
            assert!(step(frame) < 0.9, "frame {frame} moved {}", step(frame));
        }
        for frame in 105..110_u32 {
            assert!(step(frame) < 0.1, "frame {frame} moved {}", step(frame));
        }
        // Monotone and bounded: the stack only ever rises, and never past the target.
        let target = scroll_want(&default_scene(0.0).lines[3]);
        let mut previous = 0.0;
        for frame in 0..=110_u32 {
            let here = default_scene(frame as f32).scroll;
            assert!(here >= previous - 1e-6, "frame {frame} moved backwards");
            assert!(here <= target + 1e-3, "frame {frame} overshot the target");
            previous = here;
        }
    }

    #[test]
    fn an_empty_conversation_draws_nothing_and_does_not_divide_by_zero() {
        let groups = group_messages(&[]);
        assert!(groups.is_empty());
        let lines = layout(&groups);
        assert!(lines.is_empty());
        assert!((scroll_at(&[], 0.0, 0.0)).abs() < 1e-6);
        let state = ThreadFrame::resolve(Vec::new(), 0.0, 30.0);
        assert!((state.body_alpha(0)).abs() < 1e-6);
        assert!((state.avatar_alpha(0)).abs() < 1e-6);
        assert!(state.baseline(0).is_none());
    }
}
