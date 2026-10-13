//! # Word Flip
//!
//! A headline types itself out character by character behind a blinking caret, then the
//! designated word cycles through a replacement list on a flip: an accelerating throw out and a
//! decelerating settle in, overlapping so the two words hand over at identity transforms.
//!
//! Zero reflow: every candidate word is measured once, and the flip slot reserves
//! `max(wordWidths)` from the first keystroke, so the line never moves.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `prefix` | `String` | `"Looking For A"` | Text typed before the flipping word. |
//! | `words` | `Vec<String>` | `["Modern", "Stunning", "Minimal"]` | Replacement words. The first is typed; each flip advances one entry, stopping at the last. |
//! | `suffix` | `String` | `"Portfolio"` | Text typed after the flipping word. |
//! | `cps` | `f32` | `9.0` | Typing speed in characters per second. |
//! | `type_start` | `f32` | `4.0` | Frames before the first keystroke lands. |
//! | `char_fade` | `f32` | `6.0` | Fade-in duration per character, in frames. |
//! | `jitter` | `f32` | `0.18` | Keystroke jitter as a fraction of a character slot. |
//! | `pause` | `f32` | `6.0` | Frames between the last character and the first flip. |
//! | `cycle` | `f32` | `35.0` | Frames per flip cycle (one exit plus one enter). |
//! | `exit_duration` | `f32` | `9.0` | Exit throw duration, in frames. |
//! | `enter_duration` | `f32` | `9.0` | Settle-in duration, in frames. |
//! | `overlap` | `f32` | `3.0` | Frames the settle-in overlaps the exit throw. |
//! | `caret` | `bool` | `true` | Show the blinking caret; it keeps blinking after typing completes. |
//! | `font_size` | `f32` | `72.0` | Font size in px. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `tracking_em` | `f32` | `-0.03` | Headline tracking in `em`. Negative tightens. |
//! | `color` | `String` | `"#f4f4f8"` | Base text colour. |
//! | `accent_color` | `String` | `"#7c5cff"` | Colour of the flipping word and the caret bar. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components word-flip
//! ```
//!
//! ## Credit
//!
//! Ported from `WordFlip.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_left};
use crate::motion::{EASE_DEPART, EASE_OUT, FPS, clamp01, interpolate, speed_of};

/// Exit throw, in `em`. The outgoing word is flung upward.
const EXIT_Y_EM: f32 = -1.29;
/// Enter settle, in `em`. The incoming word drops in from slightly below.
const ENTER_Y_EM: f32 = 0.135;
/// The flip's rotation, in degrees. The source does a real `rotateX`; see [`squash`].
const EXIT_ROTATE_DEG: f32 = 90.0;
/// Scale the outgoing word shrinks to.
const EXIT_SCALE: f32 = 0.98;
/// Peak motion blur at maximum exit speed, in `em`.
const BLUR_EM: f32 = 0.085;
/// Caret bar width as a fraction of the font size.
const CARET_WIDTH_EM: f32 = 0.055;
/// Caret bar height in `em`.
const CARET_HEIGHT_EM: f32 = 0.78;
/// How far the caret bar's foot sits above the baseline, in `em`.
const CARET_BOTTOM_EM: f32 = 0.04;
/// The source's default hold after the final flip settles.
const DEFAULT_TAIL: f32 = 12.0;

/// The backdrop the source demo paints behind this component.
const BACKDROP: &str = "#0b0b10";

/// Deterministic keystroke jitter from two incommensurable sines — never a random source.
///
/// `jitter * (sin(i * 12.9898) * 0.6 + sin(i * 4.1414) * 0.4)`, ported verbatim.
pub fn keystroke_offset(i: usize, jitter: f32) -> f32 {
    let i = i as f32;
    jitter * ((i * 12.9898).sin() * 0.6 + (i * 4.1414).sin() * 0.4)
}

/// `fps / cps`, guarded so a degenerate speed cannot produce an infinite timeline.
pub fn frames_per_char(cps: f32, fps: f32) -> f32 {
    if cps.is_finite() && cps > 0.0 && fps.is_finite() && fps > 0.0 {
        fps / cps
    } else {
        1.0
    }
}

/// The typed line: `` `${prefix} ${firstWord} ${suffix}` ``.
pub fn typed_line(prefix: &str, first_word: &str, suffix: &str) -> String {
    format!("{prefix} {first_word} {suffix}")
}

/// Frame the last typed character finishes fading in.
///
/// `typeStart + (last + keystrokeOffset(last, jitter)) * framesPerChar + charFade`, ported
/// verbatim.
pub fn typing_end_frame(
    total_chars: usize,
    cps: f32,
    type_start: f32,
    char_fade: f32,
    jitter: f32,
    fps: f32,
) -> f32 {
    let per_char = frames_per_char(cps, fps);
    let last = total_chars.saturating_sub(1);
    type_start + (last as f32 + keystroke_offset(last, jitter)) * per_char + char_fade
}

/// The flip slot's scale for a candidate word.
///
/// The slot is `max(wordWidths)` wide, so a word can only ever be scaled down, and a word that
/// already fits is left alone.
pub fn fit_scale(word_width: f32, slot_width: f32) -> f32 {
    if slot_width > 0.0 && word_width > slot_width {
        slot_width / word_width
    } else {
        1.0
    }
}

/// Emulates a CSS `rotateX(angle)` in SVG.
///
/// SVG has no 3D transforms. A rotation about the X axis through the pivot reads as a vertical
/// squash — the glyph's height shrinks by `cos(angle)` while its width is untouched — and the
/// source's `perspective: 6.5em` only adds a little more of the same compression as the top edge
/// swings away from the camera. So the port uses the orthographic compression and drops the
/// perspective term.
pub fn squash(angle_deg: f32) -> f32 {
    angle_deg.to_radians().cos()
}

/// The exit's motion blur in px at `progress` through the throw.
///
/// Keyed to the *speed* of the departure curve, never to progress: sharp at the moment the word
/// lets go (derivative zero) and smeared at peak velocity. Computed for fidelity and covered by
/// tests; **not rendered**, because SVG has no blur filter path here (see the module README).
pub fn exit_blur_px(progress: f32, font_size: f32) -> f32 {
    speed_of(EASE_DEPART, progress) * BLUR_EM * font_size
}

/// The source's `WordFlipDurationOptions`, with the source's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct WordFlipDurationOptions {
    /// Text typed before the flipping word.
    pub prefix: String,
    /// Replacement words; the first is typed and each flip advances one entry.
    pub words: Vec<String>,
    /// Text typed after the flipping word.
    pub suffix: String,
    /// Typing speed in characters per second.
    pub cps: f32,
    /// Frames before the first keystroke lands.
    pub type_start: f32,
    /// Fade-in duration per character, in frames.
    pub char_fade: f32,
    /// Keystroke jitter as a fraction of a character slot.
    pub jitter: f32,
    /// Frames between the last character and the first flip.
    pub pause: f32,
    /// Frames per flip cycle.
    pub cycle: f32,
    /// Frame rate the typing speed is expressed against.
    pub fps: f32,
    /// Extra hold after the final flip settles.
    pub tail: f32,
}

impl Default for WordFlipDurationOptions {
    fn default() -> Self {
        Self {
            prefix: "Looking For A".to_owned(),
            words: vec![
                "Modern".to_owned(),
                "Stunning".to_owned(),
                "Minimal".to_owned(),
            ],
            suffix: "Portfolio".to_owned(),
            cps: 9.0,
            type_start: 4.0,
            char_fade: 6.0,
            jitter: 0.18,
            pause: 6.0,
            cycle: 35.0,
            fps: FPS,
            tail: DEFAULT_TAIL,
        }
    }
}

/// Total timeline: typing, pause, one cycle per replacement word, tail.
///
/// Ported verbatim from `getWordFlipDuration`.
pub fn get_word_flip_duration(options: &WordFlipDurationOptions) -> usize {
    let first_word = options.words.first().map_or("", String::as_str);
    let total_chars = typed_line(&options.prefix, first_word, &options.suffix)
        .chars()
        .count();
    let typing_end = typing_end_frame(
        total_chars,
        options.cps,
        options.type_start,
        options.char_fade,
        options.jitter,
        options.fps,
    );
    let total =
        typing_end + options.pause + options.words.len() as f32 * options.cycle + options.tail;
    total.ceil().max(1.0) as usize
}

/// The measured geometry of the headline line, in absolute canvas px.
///
/// The line is laid out as `prefix | space | slot | space | suffix`, where the slot always
/// reserves `slot_width` — the widest candidate word — so nothing reflows when the word flips.
#[derive(Debug, Clone, PartialEq)]
pub struct WordFlipLine<'a> {
    /// Left edge of the whole headline.
    pub left: f32,
    /// Left edge of the flip slot.
    pub slot_x: f32,
    /// Right edge of the flip slot.
    pub slot_end: f32,
    /// Left edge of the suffix.
    pub suffix_x: f32,
    /// Total line width.
    pub width: f32,
    /// Baseline of the line.
    pub baseline: f32,
    /// Characters in the prefix.
    pub prefix_len: usize,
    /// Characters in the first (typed) word.
    pub word_len: usize,
    /// Advance width of every character of the typed line, tracking included.
    pub advances: &'a [f32],
}

impl WordFlipLine<'_> {
    /// Index of the first suffix character.
    pub fn suffix_start(&self) -> usize {
        self.prefix_len + self.word_len + 2
    }

    /// Width of everything before character `index`.
    pub fn advance_before(&self, index: usize) -> f32 {
        self.advances.iter().take(index).sum()
    }

    /// The pen position of character `index` of the typed line.
    ///
    /// The prefix and the space that follows it flow from the line's left edge; the typed word's
    /// characters are left-aligned inside the fixed-width slot; the space before the suffix and
    /// everything after it flow from the slot's right edge. The caret uses this too: the source
    /// emits the caret inside whichever of those buckets holds the character it precedes, so its
    /// position is the same expression.
    pub fn char_x(&self, index: usize) -> f32 {
        if index <= self.prefix_len {
            self.left + self.advance_before(index)
        } else if index < self.suffix_start() {
            if index + 1 == self.suffix_start() {
                self.slot_end
            } else {
                self.slot_x
                    + (self.advance_before(index) - self.advance_before(self.prefix_len + 1))
            }
        } else {
            self.suffix_x + (self.advance_before(index) - self.advance_before(self.suffix_start()))
        }
    }

    /// The right edge of the headline, where the caret parks once the flips start.
    pub fn right(&self) -> f32 {
        self.left + self.width
    }
}

/// Everything [`WordFlip`] measures once per instance.
#[derive(Debug, Clone, PartialEq)]
struct WordFlipMetrics {
    /// Advance width of every character of the typed line, tracking included.
    advances: Vec<f32>,
    /// Tracking-included width of each candidate word, in list order.
    word_widths: Vec<f32>,
    /// The widest candidate word; the flip slot reserves this width from the first keystroke.
    slot_width: f32,
    /// Line step in px (the source's `line-height: normal` box).
    line_step: f32,
    /// Baseline offset from the top of the line box.
    baseline_offset: f32,
}

impl WordFlipMetrics {
    /// Width of candidate word `index`, or zero when there is no such word.
    fn word_width(&self, index: usize) -> f32 {
        self.word_widths.get(index).copied().unwrap_or(0.0)
    }
}

/// Configuration for [`WordFlip`].
#[derive(Debug, Clone, PartialEq)]
pub struct WordFlipConfig {
    /// Text typed before the flipping word.
    pub prefix: String,
    /// Replacement words; the first is typed and each flip advances one entry.
    pub words: Vec<String>,
    /// Text typed after the flipping word.
    pub suffix: String,
    /// Typing speed in characters per second.
    pub cps: f32,
    /// Frames before the first keystroke lands.
    pub type_start: f32,
    /// Fade-in duration per character, in frames.
    pub char_fade: f32,
    /// Keystroke jitter as a fraction of a character slot.
    pub jitter: f32,
    /// Frames between the last character and the first flip.
    pub pause: f32,
    /// Frames per flip cycle (one exit plus one enter).
    pub cycle: f32,
    /// Exit throw duration, in frames.
    pub exit_duration: f32,
    /// Settle-in duration, in frames.
    pub enter_duration: f32,
    /// Frames the settle-in overlaps the exit throw.
    pub overlap: f32,
    /// Show the blinking caret; it keeps blinking after typing completes.
    pub caret: bool,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Headline tracking in `em`. Negative tightens.
    pub tracking_em: f32,
    /// Base text colour.
    pub color: String,
    /// Colour of the flipping word and the caret bar.
    pub accent_color: String,
}

impl Default for WordFlipConfig {
    fn default() -> Self {
        let options = WordFlipDurationOptions::default();
        Self {
            prefix: options.prefix,
            words: options.words,
            suffix: options.suffix,
            cps: options.cps,
            type_start: options.type_start,
            char_fade: options.char_fade,
            jitter: options.jitter,
            pause: options.pause,
            cycle: options.cycle,
            exit_duration: 9.0,
            enter_duration: 9.0,
            overlap: 3.0,
            caret: true,
            font_size: 72.0,
            font_weight: 600,
            tracking_em: -0.03,
            color: "#f4f4f8".to_owned(),
            accent_color: "#7c5cff".to_owned(),
        }
    }
}

impl WordFlipConfig {
    /// The first (typed) word.
    pub fn first_word(&self) -> &str {
        self.words.first().map_or("", String::as_str)
    }

    /// The duration knobs as [`get_word_flip_duration`] takes them.
    pub fn duration_options(&self) -> WordFlipDurationOptions {
        WordFlipDurationOptions {
            prefix: self.prefix.clone(),
            words: self.words.clone(),
            suffix: self.suffix.clone(),
            cps: self.cps,
            type_start: self.type_start,
            char_fade: self.char_fade,
            jitter: self.jitter,
            pause: self.pause,
            cycle: self.cycle,
            fps: FPS,
            tail: DEFAULT_TAIL,
        }
    }
}

/// The Word Flip component.
#[derive(Debug, Clone, Default)]
pub struct WordFlip {
    /// Component options.
    pub config: WordFlipConfig,
    metrics: OnceLock<WordFlipMetrics>,
}

impl WordFlip {
    /// Build with the given options.
    pub fn new(config: WordFlipConfig) -> Self {
        Self {
            config,
            metrics: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "word-flip",
            name: "Word Flip",
            summary: "A headline types itself out, then its key word flips through alternatives.",
            feature: "word-flip",
        }
    }

    /// Measure once per instance: per-character advances, word widths and the baseline.
    fn measure(&self, ctx: &RenderCtx<'_>, family: &str) -> WordFlipMetrics {
        let config = &self.config;
        let size = config.font_size;
        let track = config.tracking_em * size;
        let line = typed_line(&config.prefix, config.first_word(), &config.suffix);
        let advances: Vec<f32> = line
            .chars()
            .map(|ch| ctx.text_width(family, size, &ch.to_string()) + track)
            .collect();
        let word_widths: Vec<f32> = config
            .words
            .iter()
            .map(|word| ctx.text_width(family, size, word) + track * word.chars().count() as f32)
            .collect();
        let slot_width = word_widths.iter().copied().fold(0.0_f32, f32::max);
        let line_step = ctx
            .meter(family)
            .map_or(size * 1.2, |meter| meter.line_height(size));
        WordFlipMetrics {
            advances,
            word_widths,
            slot_width,
            line_step,
            baseline_offset: ctx.baseline_offset(family, size, line_step),
        }
    }

    /// One character of the typed line, faded in on its own clock.
    fn typed_char(
        &self,
        ch: char,
        x: f32,
        baseline: f32,
        family: &str,
        opacity: f32,
    ) -> Svgr<'static> {
        text_node_left(
            ch.to_string(),
            x,
            baseline,
            family.to_owned(),
            self.config.font_size,
            self.config.font_weight,
            self.config.tracking_em,
            self.config.color.clone(),
            opacity,
        )
    }

    /// A word drawn at the slot's left edge, on the accent colour.
    fn slot_word(&self, text: &str, x: f32, baseline: f32, family: &str) -> Svgr<'static> {
        text_node_left(
            text.to_owned(),
            x,
            baseline,
            family.to_owned(),
            self.config.font_size,
            self.config.font_weight,
            self.config.tracking_em,
            self.config.accent_color.clone(),
            1.0,
        )
    }
}

/// `translateY(y) scale(sx, sy)` taken about the pivot `(px, py)`.
fn flip_transform(px: f32, py: f32, y: f32, sx: f32, sy: f32) -> String {
    format!(
        "translate({:.4} {:.4}) scale({:.6} {:.6}) translate({:.4} {:.4})",
        px,
        py + y,
        sx,
        sy,
        -px,
        -py
    )
}

impl Component for WordFlip {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_word_flip_duration(&self.config.duration_options())
    }

    fn background(&self) -> Option<&'static str> {
        Some(BACKDROP)
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = ctx.sans();
        let frame = ctx.frame();
        let fps = if ctx.info.fps > 0.0 {
            ctx.info.fps
        } else {
            1.0
        };
        let metrics = self.metrics.get_or_init(|| self.measure(ctx, &family));

        let units: Vec<char> = typed_line(&config.prefix, config.first_word(), &config.suffix)
            .chars()
            .collect();
        let per_char = frames_per_char(config.cps, fps);
        let starts: Vec<f32> = (0..units.len())
            .map(|i| config.type_start + (i as f32 + keystroke_offset(i, config.jitter)) * per_char)
            .collect();
        let typing_end = starts.last().copied().unwrap_or(config.type_start) + config.char_fade;
        let flip_base = typing_end + config.pause;
        let char_fade = config.char_fade.max(1.0);

        // Flip scheduling: flip `i` starts at `flipBase + i * cycle`, capped at the last word so
        // the headline comes to rest.
        let word_count = config.words.len();
        let flips_started = if frame < flip_base || word_count <= 1 {
            0
        } else {
            let cycles = if config.cycle > 0.0 {
                ((frame - flip_base) / config.cycle).floor()
            } else {
                0.0
            };
            ((cycles + 1.0).min((word_count - 1) as f32)).max(0.0) as usize
        };
        let last_local = if flips_started > 0 {
            frame - (flip_base + (flips_started - 1) as f32 * config.cycle)
        } else {
            -1.0
        };
        let exit_duration = config.exit_duration.max(1e-3);
        let enter_duration = config.enter_duration.max(1e-3);
        let enter_window_end = exit_duration - config.overlap + enter_duration;
        let transitioning = flips_started > 0 && last_local < enter_window_end;
        let current_word = config.words.get(flips_started).map_or("", String::as_str);
        let typing = flips_started == 0;

        // ---- geometry ----
        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        let baseline = center_y - metrics.line_step / 2.0 + metrics.baseline_offset;
        let prefix_len = config.prefix.chars().count();
        let word_len = config.first_word().chars().count();
        let suffix_start = prefix_len + word_len + 2;
        let prefix_width: f32 = metrics.advances.iter().take(prefix_len).sum();
        let suffix_width: f32 = metrics
            .advances
            .iter()
            .skip(suffix_start.min(metrics.advances.len()))
            .sum();
        let space_a = metrics.advances.get(prefix_len).copied().unwrap_or(0.0);
        let space_b = metrics
            .advances
            .get(prefix_len + word_len + 1)
            .copied()
            .unwrap_or(0.0);
        let width = prefix_width + space_a + metrics.slot_width + space_b + suffix_width;
        let left = center_x - width / 2.0;
        let slot_x = left + prefix_width + space_a;
        let slot_end = slot_x + metrics.slot_width;
        let line = WordFlipLine {
            left,
            slot_x,
            slot_end,
            suffix_x: slot_end + space_b,
            width,
            baseline,
            prefix_len,
            word_len,
            advances: &metrics.advances,
        };

        let mut nodes: Vec<Svgr<'_>> = Vec::new();

        // Prefix and suffix. While typing every character fades in on its own clock; once the
        // flips start the source renders them as plain text, so they are fully opaque.
        for (index, ch) in units.iter().enumerate() {
            let is_word_char = index > prefix_len && index < prefix_len + 1 + word_len;
            let is_slot_space = index == prefix_len + word_len + 1;
            if is_word_char || is_slot_space || *ch == ' ' {
                continue;
            }
            let opacity = if typing {
                interpolate(
                    frame,
                    [starts[index], starts[index] + char_fade],
                    [0.0, 1.0],
                )
            } else {
                1.0
            };
            nodes.push(self.typed_char(*ch, line.char_x(index), baseline, &family, opacity));
        }

        // ---- the flip slot ----
        if typing {
            for offset in 0..word_len {
                let index = prefix_len + 1 + offset;
                let Some(ch) = units.get(index) else {
                    continue;
                };
                if *ch == ' ' {
                    continue;
                }
                let opacity = interpolate(
                    frame,
                    [starts[index], starts[index] + char_fade],
                    [0.0, 1.0],
                );
                nodes.push(self.typed_char(*ch, line.char_x(index), baseline, &family, opacity));
            }
        } else if transitioning {
            if last_local < exit_duration {
                let u = clamp01(last_local / exit_duration);
                let p = EASE_DEPART.ease(u);
                let outgoing_index = flips_started.saturating_sub(1);
                let outgoing = config.words.get(outgoing_index).map_or("", String::as_str);
                let scale = (1.0 + p * (EXIT_SCALE - 1.0))
                    * fit_scale(metrics.word_width(outgoing_index), metrics.slot_width);
                let opacity = if u < 0.65 {
                    1.0
                } else {
                    interpolate(u, [0.65, 1.0], [1.0, 0.0])
                };
                let body = self.slot_word(outgoing, slot_x, baseline, &family);
                let transform = flip_transform(
                    slot_x,
                    baseline,
                    p * EXIT_Y_EM * config.font_size,
                    scale,
                    scale * squash(p * EXIT_ROTATE_DEG),
                );
                nodes.push(fframes::svgr!(<g transform={transform} opacity={opacity}>{body}</g>));
            }
            if last_local >= exit_duration - config.overlap {
                let e = EASE_OUT.ease(clamp01(
                    (last_local - (exit_duration - config.overlap)) / enter_duration,
                ));
                let scale = fit_scale(metrics.word_width(flips_started), metrics.slot_width);
                let opacity = interpolate(e, [0.0, 0.45], [0.0, 1.0]);
                let body = self.slot_word(current_word, slot_x, baseline, &family);
                let transform = flip_transform(
                    slot_x,
                    baseline,
                    (1.0 - e) * ENTER_Y_EM * config.font_size,
                    scale,
                    scale * squash((1.0 - e) * -EXIT_ROTATE_DEG),
                );
                nodes.push(fframes::svgr!(<g transform={transform} opacity={opacity}>{body}</g>));
            }
        } else {
            let scale = fit_scale(metrics.word_width(flips_started), metrics.slot_width);
            let body = self.slot_word(current_word, slot_x, baseline, &family);
            if (scale - 1.0).abs() < 1e-5 {
                nodes.push(body);
            } else {
                let transform = flip_transform(slot_x, baseline, 0.0, scale, scale);
                nodes.push(fframes::svgr!(<g transform={transform}>{body}</g>));
            }
        }

        // ---- caret ----
        if config.caret {
            let caret_at = starts.iter().filter(|start| frame >= **start).count();
            let x = if typing {
                line.char_x(caret_at.min(units.len()))
            } else {
                line.right()
            };
            let caret_width = (CARET_WIDTH_EM * config.font_size).max(2.0);
            let height = CARET_HEIGHT_EM * config.font_size;
            let bottom = baseline - CARET_BOTTOM_EM * config.font_size;
            let on = (((frame / fps) * 2.0).floor() as i64).rem_euclid(2) == 0;
            let fill = config.accent_color.clone();
            nodes.push(fframes::svgr!(
                <rect x={x - caret_width / 2.0} y={bottom - height} width={caret_width}
                      height={height} fill={fill} opacity={if on { 1.0 } else { 0.15 }} />
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keystroke_offset_is_deterministic_and_zero_at_the_first_key() {
        assert!((keystroke_offset(0, 0.18)).abs() < 1e-9);
        assert!((keystroke_offset(29, 0.18) - keystroke_offset(29, 0.18)).abs() < f32::EPSILON);
        // Bounded by the two sine weights: |0.6 sin| + |0.4 sin| <= 1.
        for i in 0..200 {
            assert!(keystroke_offset(i, 0.18).abs() <= 0.18 + 1e-6, "at {i}");
        }
        // A zero jitter removes the wobble entirely.
        assert!((keystroke_offset(17, 0.0)).abs() < 1e-9);
    }

    #[test]
    fn typing_end_frame_matches_the_source_defaults() {
        // 30 characters, cps 9 at 30 fps -> 10/3 frames per character.
        assert!((frames_per_char(9.0, 30.0) - 10.0 / 3.0).abs() < 1e-5);
        // The last character is index 29 with a 0.0169162 frame jitter.
        let end = typing_end_frame(30, 9.0, 4.0, 6.0, 0.18, 30.0);
        assert!((end - 106.723_05).abs() < 1e-3, "got {end}");
        // A single character ends at typeStart + charFade (its offset is exactly zero).
        assert!((typing_end_frame(1, 9.0, 4.0, 6.0, 0.18, 30.0) - 10.0).abs() < 1e-6);
        // Degenerate speeds fall back to one frame per character.
        assert!((frames_per_char(0.0, 30.0) - 1.0).abs() < 1e-6);
        assert!((frames_per_char(f32::NAN, 30.0) - 1.0).abs() < 1e-6);
        assert!(typing_end_frame(30, 0.0, 4.0, 6.0, 0.18, 30.0).is_finite());
    }

    #[test]
    fn duration_matches_the_source_defaults() {
        let options = WordFlipDurationOptions::default();
        // 30 chars -> typingEnd 106.723, + pause 6 + 3 * 35 + tail 12 -> ceil(229.723) = 230.
        assert_eq!(get_word_flip_duration(&options), 230);
        assert_eq!(WordFlip::default().duration(), 230);
    }

    #[test]
    fn duration_counts_the_words_as_cycles() {
        let single = get_word_flip_duration(&WordFlipDurationOptions {
            words: vec!["Modern".to_owned()],
            ..WordFlipDurationOptions::default()
        });
        let four = get_word_flip_duration(&WordFlipDurationOptions {
            words: vec![
                "Modern".to_owned(),
                "Stunning".to_owned(),
                "Minimal".to_owned(),
                "Flexible".to_owned(),
            ],
            ..WordFlipDurationOptions::default()
        });
        assert_eq!(four - single, 35 * 3);
    }

    #[test]
    fn fit_scale_only_ever_shrinks_a_word() {
        assert!((fit_scale(100.0, 200.0) - 1.0).abs() < 1e-6);
        assert!((fit_scale(200.0, 100.0) - 0.5).abs() < 1e-6);
        // The slot is `max(wordWidths)`, so in practice this is always 1 — but a zero-width slot
        // must not divide by zero.
        assert!((fit_scale(50.0, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn squash_is_the_orthographic_rotate_x() {
        assert!((squash(0.0) - 1.0).abs() < 1e-6);
        assert!(squash(90.0).abs() < 1e-6);
        assert!(squash(-90.0).abs() < 1e-6);
        // Monotonic compression across the quarter turn.
        let mut previous = 1.0_f32;
        for step in 1..=90 {
            let value = squash(step as f32);
            assert!(value < previous, "not compressing at {step}");
            previous = value;
        }
    }

    #[test]
    fn exit_blur_keys_to_speed_not_progress() {
        // The departure curve starts from rest, so the throw begins perfectly sharp.
        assert!(exit_blur_px(0.0, 72.0).abs() < 0.5);
        // ...and is smeared at its fastest.
        let peak = (0..=100)
            .map(|i| exit_blur_px(i as f32 / 100.0, 72.0))
            .fold(0.0_f32, f32::max);
        assert!((peak - BLUR_EM * 72.0).abs() < 0.5, "peak was {peak}");
    }

    #[test]
    fn line_geometry_places_every_bucket() {
        // Synthetic 10px advances: the arithmetic is what is under test, not the face.
        let advances = [10.0_f32; 8];
        let line = WordFlipLine {
            left: 100.0,
            slot_x: 130.0,
            slot_end: 180.0,
            suffix_x: 190.0,
            width: 200.0,
            baseline: 540.0,
            prefix_len: 2,
            word_len: 3,
            advances: &advances,
        };
        assert_eq!(line.suffix_start(), 7);
        // Prefix characters flow from the left edge.
        assert!((line.char_x(0) - 100.0).abs() < 1e-5);
        assert!((line.char_x(1) - 110.0).abs() < 1e-5);
        // The space after the prefix sits at the prefix's end.
        assert!((line.char_x(2) - 120.0).abs() < 1e-5);
        // Word characters are left-aligned inside the fixed-width slot, not on their advances.
        assert!((line.char_x(3) - 130.0).abs() < 1e-5);
        assert!((line.char_x(4) - 140.0).abs() < 1e-5);
        assert!((line.char_x(5) - 150.0).abs() < 1e-5);
        // The space before the suffix belongs to the trailing bucket, so it sits at the slot's
        // right edge even when the typed word is narrower than the slot.
        assert!((line.char_x(6) - 180.0).abs() < 1e-5);
        // Suffix characters flow from there.
        assert!((line.char_x(7) - 190.0).abs() < 1e-5);
        assert!((line.right() - 300.0).abs() < 1e-5);
    }

    #[test]
    fn caret_positions_follow_the_typed_line() {
        let advances = [10.0_f32; 8];
        let line = WordFlipLine {
            left: 0.0,
            slot_x: 30.0,
            slot_end: 80.0,
            suffix_x: 90.0,
            width: 100.0,
            baseline: 0.0,
            prefix_len: 2,
            word_len: 3,
            advances: &advances,
        };
        // Start of the line.
        assert!((line.char_x(0) - 0.0).abs() < 1e-5);
        // Inside the slot, one character at a time.
        assert!((line.char_x(3) - 30.0).abs() < 1e-5);
        // Completed word: the caret jumps to the slot's right edge.
        assert!((line.char_x(6) - 80.0).abs() < 1e-5);
        // Parked at the end of the line — exactly the line's right edge.
        assert!((line.char_x(8) - line.right()).abs() < 1e-5);
        assert!((line.right() - 100.0).abs() < 1e-5);
    }

    #[test]
    fn duration_options_defaults_match_the_source() {
        let options = WordFlipDurationOptions::default();
        assert_eq!(options.prefix, "Looking For A");
        assert_eq!(
            options.words,
            vec![
                "Modern".to_owned(),
                "Stunning".to_owned(),
                "Minimal".to_owned()
            ]
        );
        assert_eq!(options.suffix, "Portfolio");
        assert!((options.cps - 9.0).abs() < 1e-6);
        assert!((options.type_start - 4.0).abs() < 1e-6);
        assert!((options.char_fade - 6.0).abs() < 1e-6);
        assert!((options.jitter - 0.18).abs() < 1e-6);
        assert!((options.pause - 6.0).abs() < 1e-6);
        assert!((options.cycle - 35.0).abs() < 1e-6);
        assert!((options.fps - 30.0).abs() < 1e-6);
        assert!((options.tail - 12.0).abs() < 1e-6);
        assert_eq!(
            typed_line("Looking For A", "Modern", "Portfolio")
                .chars()
                .count(),
            30
        );
    }

    #[test]
    fn component_meta_and_background() {
        let component = WordFlip::default();
        assert_eq!(component.meta().id, "word-flip");
        assert_eq!(component.background(), Some("#0b0b10"));
        assert_eq!(component.config.first_word(), "Modern");
    }

    /// Wrap a component body in the same `<svg>` root [`crate::core::ComponentsVideo`] renders.
    fn document(body: Svgr<'_>) -> Svgr<'_> {
        fframes::svgr!(
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080" width="1920"
                 height="1080">
                <rect x="0" y="0" width="1920" height="1080" fill="#000000" />
                {body}
            </svg>
        )
    }

    /// Every frame must build a tree the renderer can convert — typing, every flip and the tail.
    ///
    /// This is the only test that exercises `show` itself: it catches NaN transforms, malformed
    /// attribute values and structural mistakes the pure-formula tests cannot see.
    #[test]
    fn every_default_frame_converts_to_a_renderable_tree() {
        let component = WordFlip::default();
        let meters = crate::embedded_meters();
        let duration = component.duration();
        let mut saw_transform = false;
        for local in 0..duration {
            let mut frame = fframes::Frame::new(local, local, 30);
            let info = crate::ctx::ComponentFrame {
                local: local as f32,
                global: local,
                fps: 30.0,
                duration,
                width: 1920.0,
                height: 1080.0,
            };
            let mut ctx = RenderCtx::new(&mut frame, crate::core::Fonts::default(), &meters, info);
            let svg = document(component.show(&mut ctx));
            let dump = format!("{svg}");
            saw_transform |= dump.contains("translate(");
            assert!(!dump.contains("NaN"), "frame {local} emitted a NaN");
            let mut cache = fframes::usvgr::Cache::default();
            let converted = svg.into_svg_tree(
                &fframes::usvgr::Options::default(),
                &mut cache,
                &fframes::usvgr::fontdb::Database::new(),
            );
            assert!(
                converted.is_ok(),
                "frame {local} did not convert: {:?}",
                converted.err()
            );
        }
        // The dump really does carry the interpolated attribute values, so the NaN check above
        // cannot pass vacuously.
        assert!(saw_transform, "no transform was emitted on any frame");
    }
}
