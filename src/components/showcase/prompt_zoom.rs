//! A prompt types itself before the view cuts into the caret.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::normalised_speed;
use crate::ui::{Mode, ReferenceFrame, UiTheme, mix_oklch, text, to_hex};

/// The reference stage's width, in reference px — the source's `REF_W`.
pub const REF_W: f32 = 810.0;
/// The reference stage's height, in reference px — the source's `REF_H`.
pub const REF_H: f32 = 450.0;

/// The field's box, as `[x, y, width, height]` in reference-stage px — the source's `BOX`.
pub const BOX: [f32; 4] = [173.0, 197.0, 461.0, 86.0];
/// The field's corner radius, in reference-stage px.
pub const BOX_RADIUS: f32 = 11.0;
/// The field's border width, in reference-stage px.
pub const BORDER: f32 = 1.0;
/// The field's inner padding, in reference-stage px — the source's `PAD`.
pub const PAD: f32 = 20.0;

/// The greeting row's top edge: the source's `HEAD_Y - 6`, where `HEAD_Y` is the measured ink top.
pub const HEAD_TOP: f32 = 139.0;
/// The greeting's type size, in reference-stage px.
pub const HEAD_SIZE: f32 = 30.0;
/// The greeting's line height, as a multiple of its type size.
pub const HEAD_LINE_HEIGHT: f32 = 1.1;
/// The gap between the mark and the greeting.
pub const HEAD_GAP: f32 = 12.0;
/// The starburst mark's size, in reference-stage px.
pub const MARK: f32 = 19.0;
/// How many rays the starburst has. A ray is a bar centred on the origin, so six bars draw twelve.
pub const MARK_RAYS: usize = 6;
/// The degrees between the starburst's rays.
pub const MARK_RAY_STEP: f32 = 30.0;

/// The chips row's top edge, in reference-stage px.
pub const CHIP_Y: f32 = 301.0;
/// A chip's height, in reference-stage px.
pub const CHIP_H: f32 = 26.0;
/// A chip's horizontal padding, in reference-stage px.
pub const CHIP_PAD: f32 = 11.0;
/// The gap between two chips, in reference-stage px.
pub const CHIP_GAP: f32 = 9.0;
/// The gap between a chip's glyph and its label, in reference-stage px.
pub const CHIP_ICON_GAP: f32 = 5.0;
/// A chip's type size, in reference-stage px.
pub const CHIP_SIZE: f32 = 11.0;
/// A chip's glyph size, in reference-stage px.
pub const CHIP_GLYPH: f32 = 12.0;
/// A chip's line height, as a multiple of its type size (`lineHeight: 1`).
pub const CHIP_LINE_HEIGHT: f32 = 1.0;

/// The prompt line's top edge inside the field, in reference-stage px — the source's `PAD - 4`.
pub const PROMPT_TOP: f32 = PAD - 4.0;
/// The prompt line's type size, in reference-stage px.
pub const PROMPT_SIZE: f32 = 12.0;
/// The prompt line's line height, as a multiple of its type size.
pub const PROMPT_LEADING: f32 = 1.4;
/// The caret's width, in reference-stage px.
pub const CARET_W: f32 = 1.4;
/// The caret's height, in reference-stage px.
pub const CARET_H: f32 = 15.0;
/// The caret's gap from the run it follows, in reference-stage px.
pub const CARET_GAP: f32 = 1.0;

/// The footer's type size, in reference-stage px.
pub const FOOTER_SIZE: f32 = 10.0;
/// The footer's distance from the field's padding-box bottom, in reference-stage px.
pub const FOOTER_BOTTOM: f32 = 12.0;
/// The gap between the model label and the effort beside it, in reference-stage px.
pub const FOOTER_GAP: f32 = 6.0;
/// The add glyph's size in the footer, in reference-stage px.
pub const PLUS: f32 = 13.0;
/// The stroke width every 24x24 icon is drawn with.
pub const GLYPH_STROKE: f32 = 1.8;

/// The composition's frame rate: the source is authored at 30 fps.
pub const FPS: f32 = 30.0;
/// The caret's half blink cycle, in seconds — a full cycle is 1.06 s.
pub const CARET_HALF_SECONDS: f32 = 0.53;

/// The prompt line's weight — the source sets none, so the face's own regular.
const PROMPT_WEIGHT: u16 = 400;
/// The greeting's weight.
const HEAD_WEIGHT: u16 = 400;
/// The model label's weight.
const MODEL_WEIGHT: u16 = 500;
/// The effort label's weight.
const EFFORT_WEIGHT: u16 = 400;
/// A chip label's weight.
const CHIP_WEIGHT: u16 = 500;

/// The starburst's six rays, as `rotate(<angle> 12 12)` inside its 24x24 box.
const MARK_ANGLES: [f32; MARK_RAYS] = [0.0, 30.0, 60.0, 90.0, 120.0, 150.0];

/// The five chip glyphs, in order, from the source's `CHIP_ICONS`.
pub const CHIP_ICONS: [&str; 5] = [
    "M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z",
    "M22 10 12 5 2 10l10 5 10-5ZM6 12v5c3 3 9 3 12 0v-5",
    "m8 6-6 6 6 6M16 6l6 6-6 6",
    "M17 8h1a4 4 0 1 1 0 8h-1M3 8h14v9a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4Z",
    "M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4",
];
/// The add affordance's glyph in the field's footer.
pub const PLUS_ICON: &str = "M12 5v14M5 12h14";

/// Characters revealed by effective frame `frame`, clamped to `total`.
///
/// The source's `typedCount`. Note the boundary: at exactly `start_frame` nothing has landed, and a
/// non-positive rate reveals the whole string at once.
#[must_use]
pub fn typed_count(frame: f32, start_frame: f32, per_frame: f32, total: usize) -> usize {
    if frame <= start_frame {
        return 0;
    }
    if per_frame <= 0.0 {
        return total;
    }
    let revealed = ((frame - start_frame) * per_frame).floor();
    if revealed <= 0.0 {
        0
    } else {
        (revealed as usize).min(total)
    }
}

/// The first `count` characters of `text`, as `text.slice(0, count)` takes them.
#[must_use]
pub fn typed_prefix(text: &str, count: usize) -> String {
    text.chars().take(count).collect()
}

/// Whether the cut has fired. A hard boolean: no ramp, no easing, deliberately.
#[must_use]
pub fn is_zoomed(frame: f32, cut_frame: f32) -> bool {
    frame >= cut_frame
}

/// The caret's half blink cycle, in frames.
#[must_use]
pub fn caret_half_cycle(fps: f32) -> f32 {
    CARET_HALF_SECONDS * fps
}

/// Caret visibility at `frame`: solid whenever a character is actively landing, blinking otherwise.
#[must_use]
pub fn caret_on(frame: f32, fps: f32, typing: bool) -> bool {
    if typing {
        return true;
    }
    let half = caret_half_cycle(fps);
    if half <= 0.0 || !half.is_finite() {
        return true;
    }
    ((frame / half).floor() as i64).rem_euclid(2) == 0
}

/// The frame the typing finishes on, as a fractional frame: `start + total / per_frame`.
///
/// The first whole frame that shows the complete string is `ceil` of this; the last frame that
/// still differs from the one before it because of typing is one fewer.
#[must_use]
pub fn typing_end_frame(start_frame: f32, per_frame: f32, total: usize) -> f32 {
    if per_frame <= 0.0 || !per_frame.is_finite() || total == 0 {
        return start_frame;
    }
    start_frame + total as f32 / per_frame
}

/// The last frame of a `duration`-frame composition on which anything changes.
///
/// `registry/__measured__.json` reads per-frame differences off the source render and records the
/// last one as `settle.base`. For this component the typing is over long before the composition
/// ends, so what is left is the caret's blink: the last change is the last blink edge that lands
/// inside `duration`. For the shipped default that is frame **80**, which is the recorded value.
#[must_use]
pub fn settle_frame(fps: f32, duration: usize, typing_end: f32) -> usize {
    let last = duration.saturating_sub(1) as f32;
    let mut settle = typing_end.max(0.0).ceil();
    let half = caret_half_cycle(fps);
    if half > 0.0 && half.is_finite() {
        let edges = (last / half).floor();
        if edges >= 1.0 {
            let edge = (edges * half).ceil();
            if edge <= last {
                settle = settle.max(edge);
            }
        }
    }
    settle.max(0.0) as usize
}

/// The cut's focal point, in reference-stage px: `(focus_x * REF_W, focus_y * REF_H)`.
#[must_use]
pub fn zoom_pivot(focus_x: f32, focus_y: f32) -> (f32, f32) {
    (focus_x * REF_W, focus_y * REF_H)
}

/// The group transform for the cut: a `scale` about the stage's focal point.
#[must_use]
pub fn zoom_transform(focus_x: f32, focus_y: f32, scale: f32) -> String {
    let (ox, oy) = zoom_pivot(focus_x, focus_y);
    format!(
        "translate({ox} {oy}) scale({scale}) translate({} {})",
        -ox, -oy
    )
}

/// Where a point in the reference stage lands **on the canvas**, through the stage fit and the cut.
///
/// This is the mapping [`PromptZoom::show`] emits, in the same order — the fit the
/// [`ReferenceFrame`] carries, then the cut about the focal point. A value can be right in stage
/// coordinates and wrong here, which is exactly what a zoom makes easy to miss, so every geometric
/// test reads through this call rather than through the raw constants.
#[must_use]
pub fn canvas_point(
    view: &ReferenceFrame,
    focus_x: f32,
    focus_y: f32,
    scale: f32,
    x: f32,
    y: f32,
) -> (f32, f32) {
    let (ox, oy) = zoom_pivot(focus_x, focus_y);
    view.point(ox + (x - ox) * scale, oy + (y - oy) * scale)
}

/// The greeting row's mark and text left edges, for a row of `mark + gap + text_width` centred on
/// the stage.
#[must_use]
pub fn greeting_origin(mark: f32, gap: f32, text_width: f32) -> (f32, f32) {
    let left = REF_W / 2.0 - (mark + gap + text_width) / 2.0;
    (left, left + mark + gap)
}

/// A chip's width for a label of `label_width`: the borders, the padding, its glyph and the gap.
#[must_use]
pub fn chip_width(label_width: f32) -> f32 {
    2.0 * BORDER + 2.0 * CHIP_PAD + CHIP_GLYPH + CHIP_ICON_GAP + label_width
}

/// The left edge of every chip, with the row centred in `row_width` from `row_left`.
#[must_use]
pub fn chip_row_lefts(widths: &[f32], gap: f32, row_left: f32, row_width: f32) -> Vec<f32> {
    let total: f32 = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
    let mut cursor = row_left + (row_width - total) / 2.0;
    widths
        .iter()
        .map(|width| {
            let left = cursor;
            cursor += width + gap;
            left
        })
        .collect()
}

/// The field's inner geometry, in reference-stage px.
///
/// CSS positions an absolutely positioned child against its container's **padding box** — inside
/// the border — so every inset the source writes (`left: PAD`, `top: PAD - 4`, `bottom: 12`) is
/// measured from here rather than from the box's outer edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Field {
    /// Padding-box left edge.
    pub left: f32,
    /// Padding-box top edge.
    pub top: f32,
    /// Padding-box right edge.
    pub right: f32,
    /// Padding-box bottom edge.
    pub bottom: f32,
}

impl Field {
    /// The field the source's `BOX` describes.
    #[must_use]
    pub fn source() -> Self {
        Self {
            left: BOX[0] + BORDER,
            top: BOX[1] + BORDER,
            right: BOX[0] + BOX[2] - BORDER,
            bottom: BOX[1] + BOX[3] - BORDER,
        }
    }

    /// The border box, as `[x, y, width, height]`.
    #[must_use]
    pub fn border_box(&self) -> [f32; 4] {
        [
            self.left - BORDER,
            self.top - BORDER,
            (self.right - self.left) + 2.0 * BORDER,
            (self.bottom - self.top) + 2.0 * BORDER,
        ]
    }

    /// The prompt line's left edge.
    #[must_use]
    pub fn prompt_left(&self) -> f32 {
        self.left + PAD
    }

    /// The prompt line's top edge.
    #[must_use]
    pub fn prompt_top(&self) -> f32 {
        self.top + PROMPT_TOP
    }

    /// The prompt line's line-box height (`font_size * line_height`).
    #[must_use]
    pub fn prompt_line_height(&self) -> f32 {
        PROMPT_SIZE * PROMPT_LEADING
    }

    /// The caret's top edge: centred in the prompt line's line box.
    #[must_use]
    pub fn caret_top(&self) -> f32 {
        self.prompt_top() + (self.prompt_line_height() - CARET_H) / 2.0
    }

    /// The footer's bottom edge.
    #[must_use]
    pub fn footer_bottom(&self) -> f32 {
        self.bottom - FOOTER_BOTTOM
    }

    /// The footer's top edge, for a footer `height` tall.
    #[must_use]
    pub fn footer_top(&self, height: f32) -> f32 {
        self.footer_bottom() - height
    }

    /// The right edge the footer's model readout is aligned to.
    #[must_use]
    pub fn footer_right(&self) -> f32 {
        self.right - PAD
    }
}

/// The top edge of a chip's line box: its type centred in the chip's content box.
#[must_use]
pub fn chip_text_top() -> f32 {
    CHIP_Y + BORDER + (CHIP_H - 2.0 * BORDER - CHIP_SIZE) / 2.0
}

/// The top edge of a chip's glyph: centred in the chip's content box.
#[must_use]
pub fn chip_glyph_top() -> f32 {
    CHIP_Y + BORDER + (CHIP_H - 2.0 * BORDER - CHIP_GLYPH) / 2.0
}

/// Configuration for [`PromptZoom`].
#[derive(Debug, Clone, PartialEq)]
pub struct PromptZoomConfig {
    /// The greeting above the field.
    pub greeting: String,
    /// The prompt that types itself once the cut has landed.
    pub text: String,
    /// Field placeholder, shown until the first character is typed.
    pub placeholder: String,
    /// Model label in the field's footer.
    pub model: String,
    /// The effort label beside the model.
    pub effort: String,
    /// Suggestion pills under the field. Empty draws none, which is the shipped default.
    pub chips: Vec<String>,
    /// Seconds at which typing begins.
    pub type_start: f32,
    /// Seconds at which the view cuts. Not a transition — one frame.
    pub cut_at: f32,
    /// Typing speed, in characters per second.
    pub chars_per_second: f32,
    /// How far the cut pushes in.
    pub zoom: f32,
    /// Focal point of the cut, as a fraction of the reference stage's width.
    pub focus_x: f32,
    /// Focal point of the cut, as a fraction of the reference stage's height.
    pub focus_y: f32,
    /// The starburst mark's colour. `None` takes the theme's `primary`.
    pub accent_color: Option<String>,
    /// Timeline multiplier. Non-positive and non-finite values are treated as `1`.
    pub speed: f32,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// The face to paint in. `None` keeps the theme's face, then the crate's embedded face.
    pub font_family: Option<String>,
    /// Composition length in frames.
    pub duration: usize,
}

impl Default for PromptZoomConfig {
    fn default() -> Self {
        // `config.ts`'s demo defaults, field for field, including the brand accent the props
        // alone would have left as `theme.primary`.
        Self {
            greeting: "Up late?".to_owned(),
            text: "Get me a plan for tomorrow".to_owned(),
            placeholder: "How can I help you today?".to_owned(),
            model: "Auto".to_owned(),
            effort: "Medium".to_owned(),
            chips: Vec::new(),
            type_start: 0.35,
            cut_at: 1.0,
            chars_per_second: 18.0,
            zoom: 2.547,
            focus_x: 0.27,
            focus_y: 0.516,
            accent_color: Some("#266DF0".to_owned()),
            speed: 1.0,
            mode: Mode::Light,
            theme: None,
            font_family: None,
            duration: 90,
        }
    }
}

/// The **Prompt Zoom** component.
#[derive(Debug, Clone, Default)]
pub struct PromptZoom {
    /// Component options.
    pub config: PromptZoomConfig,
}

impl PromptZoom {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: PromptZoomConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "prompt-zoom",
            name: "Prompt Zoom",
            summary: "A prompt types itself into an assistant's field, then the view cuts into the caret",
            feature: "prompt-zoom",
        }
    }

    /// The starburst mark, as a 24x24 icon box scaled to `size` with its top-left at `(x, y)`.
    fn mark(&self, color: &str, size: f32, x: f32, y: f32) -> Svgr<'static> {
        let scale = size / 24.0;
        let transform = format!("translate({x} {y}) scale({scale})");
        let mut rays: Vec<Svgr<'static>> = MARK_ANGLES
            .iter()
            .map(|angle| {
                let rotate = format!("rotate({angle} 12 12)");
                let fill = color.to_owned();
                fframes::svgr!(
                    <rect x={10.9} y={1.6} width={2.2} height={20.8} rx={1.1}
                          fill={fill} transform={rotate} />
                )
            })
            .collect();
        rays.push(fframes::svgr!(
            <circle cx={12.0} cy={12.0} r={3.6} fill={color.to_owned()} />
        ));
        let rays = Svgr::from(rays);
        fframes::svgr!(<g transform={transform}>{rays}</g>)
    }
}

/// A 24x24 stroke glyph at `size` px, with its top-left at `(x, y)` — the source's `Glyph`.
fn glyph(d: &str, size: f32, color: &str, x: f32, y: f32) -> Svgr<'static> {
    let scale = size / 24.0;
    let transform = format!("translate({x} {y}) scale({scale})");
    fframes::svgr!(
        <g transform={transform}>
            <path d={d.to_owned()} fill="none" stroke={color.to_owned()}
                  stroke-width={GLYPH_STROKE} stroke-linecap="round" stroke-linejoin="round" />
        </g>
    )
}

impl Component for PromptZoom {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source's `AbsoluteFill` paints `theme.background`; `config.ts` declares no
        // `previewBackdrop`, and the light palette's background is a constant.
        Some("#faf9f6")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let face = text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);
        // The source sets the greeting in its own serif (Source Serif 4), which this crate does
        // not embed, so it draws in the face the rest of the scene uses.
        let greeting_face = text::default_face(ctx);
        let accent = config
            .accent_color
            .clone()
            .unwrap_or_else(|| theme.primary.clone());
        // `inputStyleContext(theme).idleBorder` walked 18% toward the foreground, exactly as
        // `input.rs` derives its own hover border.
        let border_color = to_hex(mix_oklch(&theme.input, &theme.foreground, 0.18));

        let frame = ctx.frame() * normalised_speed(config.speed);
        let total = config.text.chars().count();
        let typed_n = typed_count(
            frame,
            config.type_start * FPS,
            config.chars_per_second / FPS,
            total,
        );
        let typing = typed_n > 0 && typed_n < total;
        let caret_visible = caret_on(frame, FPS, typing);
        let scale = if is_zoomed(frame, config.cut_at * FPS) {
            config.zoom
        } else {
            1.0
        };

        let field = Field::source();
        let mut nodes: Vec<Svgr<'static>> = Vec::new();

        // ---- the greeting row, centred on the stage as a whole ----
        let head_line = HEAD_SIZE * HEAD_LINE_HEIGHT;
        let head_baseline = HEAD_TOP + ctx.baseline_offset(&greeting_face, HEAD_SIZE, head_line);
        let head_width = text::tracked_width_at(
            ctx,
            &greeting_face,
            HEAD_WEIGHT,
            HEAD_SIZE,
            &config.greeting,
            0.0,
        );
        let (mark_left, text_left) = greeting_origin(MARK, HEAD_GAP, head_width);
        let row_height = head_line.max(MARK);
        nodes.push(self.mark(
            &accent,
            MARK,
            mark_left,
            HEAD_TOP + (row_height - MARK) / 2.0,
        ));
        nodes.push(text::text_node_left(
            &config.greeting,
            text_left,
            head_baseline,
            greeting_face,
            HEAD_SIZE,
            HEAD_WEIGHT,
            0.0,
            theme.foreground.clone(),
            1.0,
        ));

        // ---- the field ----
        let rect = field.border_box();
        let half = BORDER / 2.0;
        nodes.push(fframes::svgr!(
            <rect x={rect[0] + half} y={rect[1] + half}
                  width={rect[2] - BORDER} height={rect[3] - BORDER}
                  rx={(BOX_RADIUS - half).max(0.0)} fill={theme.card.clone()}
                  stroke={border_color.clone()} stroke-width={BORDER} />
        ));

        // ---- the prompt line, with the caret riding its end ----
        let prompt_baseline = field.prompt_top()
            + ctx.baseline_offset(&face, PROMPT_SIZE, field.prompt_line_height());
        let (run, run_color) = if typed_n > 0 {
            (
                typed_prefix(&config.text, typed_n),
                theme.foreground.clone(),
            )
        } else {
            (config.placeholder.clone(), theme.muted_foreground.clone())
        };
        let run_width = text::tracked_width_at(ctx, &face, PROMPT_WEIGHT, PROMPT_SIZE, &run, 0.0);
        nodes.push(text::text_node_left(
            &run,
            field.prompt_left(),
            prompt_baseline,
            face.clone(),
            PROMPT_SIZE,
            PROMPT_WEIGHT,
            0.0,
            run_color,
            1.0,
        ));
        if caret_visible {
            let caret_x = field.prompt_left() + run_width + CARET_GAP;
            let caret_y = field.caret_top();
            nodes.push(fframes::svgr!(
                <rect x={caret_x} y={caret_y} width={CARET_W} height={CARET_H}
                      fill={theme.foreground.clone()} opacity={1.0} />
            ));
        }

        // ---- the footer: the add affordance, and the model readout ----
        let footer_line = ctx
            .meter(&face)
            .map_or(FOOTER_SIZE * 1.2, |meter| meter.line_height(FOOTER_SIZE));
        let footer_height = PLUS.max(footer_line);
        let footer_centre = field.footer_top(footer_height) + footer_height / 2.0;
        let footer_baseline =
            text::baseline_for_centred_box(ctx, &face, FOOTER_SIZE, footer_centre);
        nodes.push(glyph(
            PLUS_ICON,
            PLUS,
            &theme.muted_foreground,
            field.left + PAD,
            footer_centre - PLUS / 2.0,
        ));
        let model_width =
            text::tracked_width_at(ctx, &face, MODEL_WEIGHT, FOOTER_SIZE, &config.model, 0.0);
        let effort_width =
            text::tracked_width_at(ctx, &face, EFFORT_WEIGHT, FOOTER_SIZE, &config.effort, 0.0);
        let group_left = field.footer_right() - (model_width + FOOTER_GAP + effort_width);
        nodes.push(text::text_node_left(
            &config.model,
            group_left,
            footer_baseline,
            face.clone(),
            FOOTER_SIZE,
            MODEL_WEIGHT,
            0.0,
            theme.foreground.clone(),
            1.0,
        ));
        nodes.push(text::text_node_left(
            &config.effort,
            group_left + model_width + FOOTER_GAP,
            footer_baseline,
            face.clone(),
            FOOTER_SIZE,
            EFFORT_WEIGHT,
            0.0,
            theme.muted_foreground.clone(),
            1.0,
        ));

        // ---- the chips, static, centred in the field's own width ----
        if !config.chips.is_empty() {
            let widths: Vec<f32> = config
                .chips
                .iter()
                .map(|label| {
                    chip_width(text::tracked_width_at(
                        ctx,
                        &face,
                        CHIP_WEIGHT,
                        CHIP_SIZE,
                        label,
                        0.0,
                    ))
                })
                .collect();
            let lefts = chip_row_lefts(&widths, CHIP_GAP, BOX[0], BOX[2]);
            let glyph_top = chip_glyph_top();
            let chip_baseline = chip_text_top()
                + ctx.baseline_offset(&face, CHIP_SIZE, CHIP_SIZE * CHIP_LINE_HEIGHT);
            for (index, label) in config.chips.iter().enumerate() {
                let x = lefts[index];
                let width = widths[index];
                let chip_half = BORDER / 2.0;
                nodes.push(fframes::svgr!(
                    <rect x={x + chip_half} y={CHIP_Y + chip_half}
                          width={width - BORDER} height={CHIP_H - BORDER}
                          rx={(CHIP_H / 2.0 - chip_half).max(0.0)} fill={theme.card.clone()}
                          stroke={border_color.clone()} stroke-width={BORDER} />
                ));
                nodes.push(glyph(
                    CHIP_ICONS[index % CHIP_ICONS.len()],
                    CHIP_GLYPH,
                    &theme.muted_foreground,
                    x + BORDER + CHIP_PAD,
                    glyph_top,
                ));
                nodes.push(text::text_node_left(
                    label,
                    x + BORDER + CHIP_PAD + CHIP_GLYPH + CHIP_ICON_GAP,
                    chip_baseline,
                    face.clone(),
                    CHIP_SIZE,
                    CHIP_WEIGHT,
                    0.0,
                    theme.foreground.clone(),
                    1.0,
                ));
            }
        }

        // ---- the stage, then the cut, in that order and in the stage's own pixels ----
        let view = ReferenceFrame::fit_in(ctx.width(), ctx.height(), REF_W, REF_H);
        let nodes = Svgr::from(nodes);
        let scene = if (scale - 1.0).abs() < 1e-6 {
            fframes::svgr!(<g>{nodes}</g>)
        } else {
            let transform = zoom_transform(config.focus_x, config.focus_y, scale);
            fframes::svgr!(<g transform={transform}>{nodes}</g>)
        };
        let transform = view.transform();
        // The cut pushes the greeting off the frame, which is the source's own look — its stage
        // has no `overflow: hidden` either, and the composition's frame is what clips it. The
        // canvas clip says so explicitly: this subtree is cut on purpose, so the frame diagnostics
        // do not report the greeting as an accident.
        let (clip_w, clip_h) = (ctx.width(), ctx.height());
        fframes::svgr!(
            <g>
                <defs>
                    <clipPath id="prompt-zoom-canvas">
                        <rect x={0.0} y={0.0} width={clip_w} height={clip_h} />
                    </clipPath>
                </defs>
                <g clip-path="url(#prompt-zoom-canvas)">
                    <g transform={transform}>{scene}</g>
                </g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stage fit at 1080p, which every geometric expectation below is read through.
    fn view() -> ReferenceFrame {
        ReferenceFrame::fit_in(1920.0, 1080.0, REF_W, REF_H)
    }

    /// The shipped default's focal point, run through the same mapping `show` emits.
    fn default_point(x: f32, y: f32, scale: f32) -> (f32, f32) {
        canvas_point(&view(), 0.27, 0.516, scale, x, y)
    }

    #[test]
    fn the_defaults_are_the_sources_config_field_by_field() {
        let config = PromptZoomConfig::default();
        assert_eq!(config.greeting, "Up late?");
        assert_eq!(config.text, "Get me a plan for tomorrow");
        assert_eq!(config.placeholder, "How can I help you today?");
        assert_eq!(config.model, "Auto");
        assert_eq!(config.effort, "Medium");
        // `chips` is an array, so `config.ts` gives it no control and the demo shows none.
        assert!(config.chips.is_empty());
        assert!((config.type_start - 0.35).abs() < 1e-6);
        assert!((config.cut_at - 1.0).abs() < 1e-6);
        assert!((config.chars_per_second - 18.0).abs() < 1e-6);
        assert!((config.zoom - 2.547).abs() < 1e-6);
        assert!((config.focus_x - 0.27).abs() < 1e-6);
        assert!((config.focus_y - 0.516).abs() < 1e-6);
        assert_eq!(config.accent_color.as_deref(), Some("#266DF0"));
        assert!((config.speed - 1.0).abs() < 1e-6);
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.theme, None);
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 90);
    }

    #[test]
    fn the_identity_duration_and_backdrop_are_what_the_catalog_expects() {
        let component = PromptZoom::default();
        assert_eq!(component.meta().id, "prompt-zoom");
        assert_eq!(PromptZoom::meta_static().feature, "prompt-zoom");
        assert_eq!(component.duration(), 90);
        assert_eq!(component.background(), Some("#faf9f6"));
    }

    #[test]
    fn typed_count_is_the_sources_own_schedule() {
        let per_frame = 18.0 / 30.0;
        // Nothing before the cue, and nothing at the cue either.
        assert_eq!(typed_count(0.0, 10.5, per_frame, 26), 0);
        assert_eq!(typed_count(10.5, 10.5, per_frame, 26), 0);
        // The first character lands on frame 13: (13 - 10.5) * 0.6 = 1.5.
        assert_eq!(typed_count(12.0, 10.5, per_frame, 26), 0);
        assert_eq!(typed_count(13.0, 10.5, per_frame, 26), 1);
        // The whole string is up on frame 54 and stays there.
        assert_eq!(typed_count(53.0, 10.5, per_frame, 26), 25);
        assert_eq!(typed_count(54.0, 10.5, per_frame, 26), 26);
        assert_eq!(typed_count(400.0, 10.5, per_frame, 26), 26);
        // A zero rate reveals everything at once, as the source's guard does.
        assert_eq!(typed_count(11.0, 10.5, 0.0, 26), 26);
        // The cut lands mid-sentence: typing is running on both sides of it.
        let cut = 1.0 * 30.0;
        let at_cut = typed_count(cut, 10.5, per_frame, 26);
        assert!(at_cut > 0 && at_cut < 26);
        assert_eq!(at_cut, 11, "frame 30 shows the first 11 characters");
        assert!(typed_count(cut + 10.0, 10.5, per_frame, 26) > at_cut);
    }

    #[test]
    fn typed_prefix_slices_like_the_source() {
        assert_eq!(typed_prefix("Get me a pl", 0), "");
        assert_eq!(typed_prefix("Get me a pl", 4), "Get ");
        assert_eq!(typed_prefix("Get me a plan", 99), "Get me a plan");
        // A multi-byte character is one slice unit, as `Array.from` would treat it.
        assert_eq!(typed_prefix("aé☂", 2), "aé");
    }

    #[test]
    fn the_cut_is_a_step_never_a_ramp() {
        let cut_frame = 1.0 * FPS;
        assert!((cut_frame - 30.0).abs() < 1e-6);
        assert!(!is_zoomed(29.0, cut_frame));
        assert!(is_zoomed(30.0, cut_frame));
        // Monotone: once it has fired it never goes back.
        let mut seen = false;
        for frame in 0..=90 {
            let zoomed = is_zoomed(frame as f32, cut_frame);
            if seen {
                assert!(zoomed);
            }
            seen |= zoomed;
        }
    }

    #[test]
    fn the_caret_is_solid_while_typing_and_blinks_when_idle() {
        // Solid whenever a character is landing, whatever the phase.
        for frame in 0..60 {
            assert!(caret_on(frame as f32, FPS, true));
        }
        let half = caret_half_cycle(FPS);
        assert!((half - 15.9).abs() < 1e-4);
        assert!(caret_on(0.0, FPS, false));
        assert!(caret_on(half - 0.1, FPS, false));
        assert!(!caret_on(half + 0.1, FPS, false));
        assert!(!caret_on(2.0 * half - 0.1, FPS, false));
        assert!(caret_on(2.0 * half + 0.1, FPS, false));
        // Frame by frame across the hold, the flips are on 64 and 80.
        let flips: Vec<usize> = (1..90)
            .filter(|frame| {
                caret_on(*frame as f32, FPS, false) != caret_on(*frame as f32 - 1.0, FPS, false)
            })
            .collect();
        assert_eq!(flips, vec![16, 32, 48, 64, 80]);
    }

    #[test]
    fn the_closed_form_reproduces_the_measured_settle() {
        // `registry/__measured__.json` records `settle.base = 80` and `durationInFrames = 90` for
        // the shipped default. The source's own schedule must reproduce both.
        let config = PromptZoomConfig::default();
        let total = config.text.chars().count();
        assert_eq!(total, 26);
        let per_frame = config.chars_per_second / FPS;
        let typed_start = config.type_start * FPS;
        let typing_end = typing_end_frame(typed_start, per_frame, total);
        assert!((typing_end - 53.833_336).abs() < 1e-4, "{typing_end}");
        // Frame 53 still shows 25 characters; frame 54 shows all 26.
        assert_eq!(typed_count(53.0, typed_start, per_frame, total), 25);
        assert_eq!(typed_count(54.0, typed_start, per_frame, total), total);
        // After that the caret is the only thing left to move: the last blink edge inside the
        // composition is frame 80, and that is the recorded settle.
        assert_eq!(settle_frame(FPS, config.duration, typing_end), 80);
        assert_eq!(config.duration, 90);
    }

    #[test]
    fn a_longer_composition_settles_on_its_last_blink_edge() {
        // The settle is a caret edge, not a constant: 0.53 s of 30 fps is 15.9 frames, so the
        // edges fall on 16, 32, 48, 64, 80, 96 …
        assert_eq!(settle_frame(FPS, 91, 53.833), 80);
        assert_eq!(settle_frame(FPS, 97, 53.833), 96);
        // The caret outlives the typing: a 40-frame composition whose typing ends on frame 20
        // still changes on frame 32, the second blink edge.
        assert_eq!(settle_frame(FPS, 40, 20.0), 32);
        // Only a composition that ends before the first edge settles when its typing does.
        assert_eq!(settle_frame(FPS, 15, 10.0), 10);
    }

    #[test]
    fn the_pivot_is_the_caret_the_source_measured() {
        let (ox, oy) = zoom_pivot(0.27, 0.516);
        assert!((ox - 218.7).abs() < 1e-4, "{ox}");
        assert!((oy - 232.2).abs() < 1e-4, "{oy}");
        // "10% across and 41% down the input field", as the source's own comment states.
        let across = (ox - BOX[0]) / BOX[2];
        let down = (oy - BOX[1]) / BOX[3];
        assert!((across - 0.0991).abs() < 0.002, "{across}");
        assert!((down - 0.4093).abs() < 0.002, "{down}");
        // And it is inside the field, which is what makes the cut read as a push into the caret.
        assert!(ox > BOX[0] && ox < BOX[0] + BOX[2]);
        assert!(oy > BOX[1] && oy < BOX[1] + BOX[3]);
    }

    #[test]
    fn the_cut_is_a_transform_about_the_pivot() {
        let (ox, oy) = zoom_pivot(0.27, 0.516);
        let transform = zoom_transform(0.27, 0.516, 2.547);
        assert_eq!(
            transform,
            format!(
                "translate({ox} {oy}) scale(2.547) translate({} {})",
                -ox, -oy
            )
        );
        // The pivot is the one point the cut does not move — read through the canvas mapping that
        // `show` emits, not through the constants.
        let (px, py) = default_point(ox, oy, 2.547);
        let (ex, ey) = view().point(ox, oy);
        assert!((px - ex).abs() < 1e-3 && (py - ey).abs() < 1e-3);
        // At rest nothing moves at all.
        let (rx, ry) = default_point(BOX[0], BOX[1], 1.0);
        let (bx, by) = view().point(BOX[0], BOX[1]);
        assert!((rx - bx).abs() < 1e-3 && (ry - by).abs() < 1e-3);
    }

    #[test]
    fn the_drawn_geometry_matches_the_reference_demo() {
        // Measured off `reference/free/prompt-zoom.mp4` (960x540; canvas px = demo px x 2)
        // with a loose threshold on the field's border:
        //
        //   wide view (frame 0)   border box left 409, top 473, bottom 677
        //   after the cut (f60)   border box left 242, top 344, bottom 863
        //
        // The port's own chain — stage fit, then the cut about `(218.7, 232.2)` — has to land on
        // those numbers, because a value that is right in stage px can still be wrong once the
        // enclosing transforms are applied.
        let (left, top) = default_point(BOX[0], BOX[1], 1.0);
        let (_, bottom) = default_point(BOX[0], BOX[1] + BOX[3], 1.0);
        assert!((left - 410.1).abs() < 1.0, "{left}");
        assert!((top - 473.6).abs() < 1.0, "{top}");
        assert!((bottom - 677.4).abs() < 1.0, "{bottom}");
        assert!((left - 409.0).abs() < 3.0, "wide view left {left}");
        assert!((top - 473.0).abs() < 3.0, "wide view top {top}");
        assert!((bottom - 677.0).abs() < 3.0, "wide view bottom {bottom}");

        let (zleft, ztop) = default_point(BOX[0], BOX[1], 2.547);
        let (_, zbottom) = default_point(BOX[0], BOX[1] + BOX[3], 2.547);
        assert!((zleft - 242.5).abs() < 1.0, "{zleft}");
        assert!((ztop - 344.5).abs() < 1.0, "{ztop}");
        assert!((zbottom - 863.7).abs() < 1.0, "{zbottom}");
        assert!((zleft - 242.0).abs() < 3.0, "cut left {zleft}");
        assert!((ztop - 344.0).abs() < 3.0, "cut top {ztop}");
        assert!((zbottom - 863.0).abs() < 3.0, "cut bottom {zbottom}");
        // The cut pushes the left edge 168 canvas px left and the bottom 186 px down.
        assert!(zleft < left && zbottom > bottom);
    }

    #[test]
    fn the_stage_is_model_b_not_the_design_unit() {
        let view = view();
        assert!((view.scale - 2.370_370_3).abs() < 1e-5, "{}", view.scale);
        // 810/450 is 1.8 against the canvas's 1.7778, so the stage fills the width and is
        // letterboxed above and below.
        assert!(view.left.abs() < 1e-4);
        assert!((view.top - 6.666_667).abs() < 1e-4, "{}", view.top);
        assert!((REF_W * view.scale - 1920.0).abs() < 0.01);
        assert!(REF_H * view.scale < 1080.0);
        // The design unit of this family is 1.5 here; using it would render the scene at 63%.
        let design_unit = 1080.0_f32 / 100.0 / crate::motion::REFERENCE_VMIN;
        assert!((design_unit - 1.5).abs() < 1e-6);
        assert!((design_unit / view.scale - 0.632_8).abs() < 0.001);
        // The stage is centred: equal letterbox top and bottom.
        assert!((view.top - (1080.0 - REF_H * view.scale) / 2.0).abs() < 1e-4);
    }

    #[test]
    fn the_field_is_measured_from_its_padding_box() {
        let field = Field::source();
        assert!((field.left - 174.0).abs() < 1e-6);
        assert!((field.top - 198.0).abs() < 1e-6);
        assert!((field.right - 633.0).abs() < 1e-6);
        assert!((field.bottom - 282.0).abs() < 1e-6);
        // The drawn rect is the border box, with the stroke inset by half so its outer edge lands
        // on the box the source measured.
        let rect = field.border_box();
        assert_eq!(rect, BOX);
        assert!((rect[0] + BORDER / 2.0 - 173.5).abs() < 1e-6);

        // The prompt line: `left: PAD`, `top: PAD - 4` from the padding box.
        assert!((field.prompt_left() - 194.0).abs() < 1e-6);
        assert!((field.prompt_top() - 214.0).abs() < 1e-6);
        assert!((field.prompt_line_height() - 16.8).abs() < 1e-6);
        // The caret is 15 tall in a 16.8 line box, so it is 0.9 below the line's top.
        assert!((field.caret_top() - 214.9).abs() < 1e-6);
        // The footer: `bottom: 12` from the padding box.
        assert!((field.footer_bottom() - 270.0).abs() < 1e-6);
        assert!((field.footer_top(13.0) - 257.0).abs() < 1e-6);
        assert!((field.footer_right() - 613.0).abs() < 1e-6);
    }

    #[test]
    fn the_greeting_row_centres_the_mark_and_the_text() {
        // The reference demo's row: the mark's ink is centred on x 396..415 and the greeting's on
        // 432..562, in a 960-wide demo. The row as a whole is centred on the stage.
        let (mark, text) = greeting_origin(MARK, HEAD_GAP, 113.4);
        assert!((mark - 332.8).abs() < 0.1, "{mark}");
        assert!((text - 363.8).abs() < 0.1, "{text}");
        // Mark left, gap, text: the row spans mark..mark + MARK + HEAD_GAP + width, centred.
        let total = MARK + HEAD_GAP + 113.4;
        assert!((mark - (REF_W / 2.0 - total / 2.0)).abs() < 1e-4);
        assert!((text - (mark + MARK + HEAD_GAP)).abs() < 1e-4);
        // A wider face pushes the mark left, which is what the greeting's serif stand-in does.
        let (wider_mark, _) = greeting_origin(MARK, HEAD_GAP, 121.4);
        assert!(wider_mark < mark);
    }

    #[test]
    fn chip_geometry_is_the_sources_own_box() {
        // borders 2 + padding 22 + glyph 12 + gap 5 = 41 before the label.
        assert!((chip_width(0.0) - 41.0).abs() < 1e-6);
        assert!((chip_width(30.0) - 71.0).abs() < 1e-6);
        assert!((chip_glyph_top() - 308.0).abs() < 1e-6);
        assert!((chip_text_top() - 308.5).abs() < 1e-6);

        // The row is centred in the field's width: two 71-wide chips with a 9 gap span 151, so
        // they start at BOX[0] + (461 - 151) / 2 = 328.
        let lefts = chip_row_lefts(&[71.0, 71.0], CHIP_GAP, BOX[0], BOX[2]);
        assert_eq!(lefts.len(), 2);
        assert!((lefts[0] - 328.0).abs() < 1e-4, "{lefts:?}");
        assert!((lefts[1] - 408.0).abs() < 1e-4, "{lefts:?}");
        // A single chip is centred on the field.
        let one = chip_row_lefts(&[71.0], CHIP_GAP, BOX[0], BOX[2]);
        assert!((one[0] - (BOX[0] + (BOX[2] - 71.0) / 2.0)).abs() < 1e-4);
        // No chips, no positions.
        assert!(chip_row_lefts(&[], CHIP_GAP, BOX[0], BOX[2]).is_empty());
    }

    #[test]
    fn the_border_colour_is_the_inputs_own_token_walk() {
        let theme = UiTheme::light();
        // `inputStyleContext(theme).idleBorder` is `theme.input`, walked 18% toward the foreground
        // exactly as `input.rs` derives its hover border.
        let border = to_hex(mix_oklch(&theme.input, &theme.foreground, 0.18));
        assert_ne!(border, theme.input);
        assert_ne!(border, theme.foreground);
        assert!(border.starts_with('#'));
    }

    #[test]
    fn the_accent_falls_back_to_the_theme() {
        let config = PromptZoomConfig {
            accent_color: None,
            ..PromptZoomConfig::default()
        };
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let accent = config
            .accent_color
            .clone()
            .unwrap_or_else(|| theme.primary.clone());
        assert_eq!(accent, "#3072db");
        // The shipped default keeps `config.ts`'s brand accent instead.
        assert_eq!(
            PromptZoomConfig::default().accent_color.as_deref(),
            Some("#266DF0")
        );
    }

    #[test]
    fn the_icon_set_is_the_sources() {
        assert_eq!(CHIP_ICONS.len(), 5);
        assert_eq!(PLUS_ICON, "M12 5v14M5 12h14");
        assert!(
            CHIP_ICONS
                .iter()
                .all(|d| d.starts_with(|c: char| c.is_ascii_alphabetic()))
        );
        // Six bars, thirty degrees apart: a bar is centred on the origin, so it draws a ray at
        // both ends — twelve rays, which is the starburst.
        assert_eq!(MARK_ANGLES.len(), MARK_RAYS);
        assert!((MARK_ANGLES[5] - 150.0).abs() < 1e-6);
    }
}
