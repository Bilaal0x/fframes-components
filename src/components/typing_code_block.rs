//! # Typing Code Block
//!
//! A syntax-highlighted editor window types a code sample out one character at a time, behind a
//! right-aligned line-number gutter, with a `|` caret blinking at the end of the last line. The
//! caret keeps blinking after the code is complete, because its clock is the *unclamped* one.
//!
//! Beat by beat, with the defaults (the 7-line `greet` sample, 0.3 frames per character):
//!
//! ```text
//! frame   0   the empty editor window; the caret is there but dark
//! frame   1   the caret ramps up over BLINK_FADE_FRAMES (2) frames
//! frame   1   "// c" — 3 characters per frame at this speed
//! ...
//! frame  45   the whole sample is on screen (148 characters x 0.3 = 44.4 frames, so 45)
//! frame  45+  the caret blinks on a 30-frame cycle, forever (the clock is unclamped)
//! frame 135   end of the component: the 90 hold frames after the last character
//! ```
//!
//! The highlight is re-derived from the visible prefix on every frame, which is exactly what the
//! source does (`Highlight` re-tokenizes the sliced string). The prefix is not re-tokenized here
//! for speed: the whole sample is tokenized once per component instance and the visible prefix is
//! taken from that, which is provably the same token list, because a comment or a string can never
//! change retroactively.
//!
//! ## Syntax highlighting
//!
//! The source uses `prism-react-renderer` with the `vsDark` theme, a full grammar per language.
//! This port ships a **small, dependency-free approximation** of it instead: `//` and `/* */`
//! comments, `"`/`'`/backtick string literals, numbers, a merged C-like + Python-ish keyword set,
//! and "an identifier immediately followed by `(`" as a function name. The colours approximate the
//! `vsDark` palette (keyword `#569cd6`, string `#ce9178`, comment `#6a9955`, number `#b5cea8`,
//! function `#dcdcaa`, everything else `#d4d4d4`). It is **not** Prism: the `language` field is kept
//! for API compatibility and is ignored. See the README for the full list of differences.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `code` | `String` | the source's `DEMO_CODE` | Full code string; typing reveals it left to right. |
//! | `language` | `String` | `"tsx"` | Prism language id, kept for API compatibility; ignored by the tokenizer. |
//! | `typing_speed` | `f32` | `0.3` | Frames per character; below `1.0` types several characters per frame. |
//! | `delay` | `usize` | `0` | Frames to wait before typing starts. |
//! | `show_line_numbers` | `bool` | `true` | Show the right-aligned `2.5em` line-number gutter. |
//! | `background` | `String` | `"#1e1e1e"` | Editor fill colour. |
//! | `font_size` | `Option<f32>` | `None` | Code font size in px; `None` resolves to `ctx.width() * 0.02`. |
//!
//! The code is drawn in the monospaced family ([`RenderCtx::mono`]) at weight `400` and a line
//! height of `1.5`, with an `12px`-rounded editor card padded by one font size.
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components typing-code-block -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `TypingCodeBlock.tsx` in <https://github.com/Remocn/remocn> (MIT).

use std::sync::OnceLock;

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_left};
use crate::motion::EASE_OUT;

/// The sample code the source's composition types out, verbatim.
pub const DEMO_CODE: &str = r#"// components-generator
export const greet = (name: string) => {
  const message = `Hello, ${name}!`;
  console.log(message);
};

greet("Remotion");"#;

/// Frames held after the last character lands (the source's `TYPING_CODE_TAIL_FRAMES`).
pub const HOLD_TAIL_FRAMES: usize = 90;

/// One cursor blink cycle, in frames.
pub const BLINK_CYCLE_FRAMES: f32 = 30.0;

/// Frames the caret takes to ramp in and out of each blink half-cycle.
pub const BLINK_FADE_FRAMES: f32 = 2.0;

/// Gutter width as a multiple of the font size (the source's `width: 2.5em`).
pub const GUTTER_EM: f32 = 2.5;

/// Space between the line number and the code (the source's `padding-right: 1em`).
pub const GUTTER_PADDING_EM: f32 = 1.0;

/// Line height as a multiple of the font size.
pub const LINE_HEIGHT: f32 = 1.5;

/// Corner radius of the editor card, in px (the source's `border-radius: 12`).
pub const CARD_RADIUS: f32 = 12.0;

/// Code font weight (the source inherits the CSS default).
pub const FONT_WEIGHT: u16 = 400;

/// Fill of the line numbers.
pub const LINE_NUMBER_COLOR: &str = "#666";

/// Opacity of the line numbers.
pub const LINE_NUMBER_OPACITY: f32 = 0.5;

/// Fill of the caret.
pub const CURSOR_COLOR: &str = "#d4d4d4";

/// The caret glyph.
pub const CURSOR: &str = "|";

/// The keywords the tokenizer highlights: a merged C-like and Python-ish set.
///
/// This is a small hand-written list, not Prism's grammar; see the README.
pub const KEYWORDS: &[&str] = &[
    "abstract",
    "and",
    "any",
    "as",
    "async",
    "await",
    "bool",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "def",
    "default",
    "del",
    "delete",
    "do",
    "elif",
    "else",
    "enum",
    "except",
    "export",
    "extends",
    "false",
    "finally",
    "float",
    "for",
    "from",
    "function",
    "global",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "int",
    "interface",
    "is",
    "lambda",
    "let",
    "new",
    "None",
    "not",
    "null",
    "number",
    "or",
    "package",
    "pass",
    "private",
    "protected",
    "public",
    "raise",
    "return",
    "self",
    "static",
    "str",
    "string",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "type",
    "typeof",
    "undefined",
    "var",
    "void",
    "while",
    "with",
    "yield",
];

/// What a token is, which decides its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Plain code: identifiers, punctuation, whitespace.
    Default,
    /// A reserved word from [`KEYWORDS`].
    Keyword,
    /// A string literal: `"..."`, `'...'` or a template literal.
    String,
    /// A comment: `//` to the end of the line, or a `/* ... */` block.
    Comment,
    /// A numeric literal.
    Number,
    /// An identifier immediately followed by `(`.
    Function,
}

impl TokenKind {
    /// The `vsDark` colour this kind is painted with.
    pub fn color(self) -> &'static str {
        match self {
            Self::Default => "#d4d4d4",
            Self::Keyword => "#569cd6",
            Self::String => "#ce9178",
            Self::Comment => "#6a9955",
            Self::Number => "#b5cea8",
            Self::Function => "#dcdcaa",
        }
    }
}

/// One lexical token of one source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The token's text, exactly as it appears in the source. Never contains a newline.
    pub text: String,
    /// What the token is, which decides its colour.
    pub kind: TokenKind,
}

/// Configuration for [`TypingCodeBlock`].
#[derive(Debug, Clone, PartialEq)]
pub struct TypingCodeBlockConfig {
    /// Full code string; typing reveals it left to right.
    pub code: String,
    /// Prism language id, kept for API compatibility. The tokenizer is language-agnostic and
    /// ignores this field, which is why the port can never fail on an unknown language.
    pub language: String,
    /// Frames per character; below `1.0` types several characters per frame.
    pub typing_speed: f32,
    /// Frames to wait before typing starts.
    pub delay: usize,
    /// Show the right-aligned `2.5em` line-number gutter.
    pub show_line_numbers: bool,
    /// Editor fill colour.
    pub background: String,
    /// Code font size in px, or `None` for `ctx.width() * 0.02`.
    pub font_size: Option<f32>,
}

impl Default for TypingCodeBlockConfig {
    fn default() -> Self {
        Self {
            code: DEMO_CODE.to_owned(),
            language: "tsx".to_owned(),
            typing_speed: 0.3,
            delay: 0,
            show_line_numbers: true,
            background: "#1e1e1e".to_owned(),
            font_size: None,
        }
    }
}

/// Frames until the last character lands, before any hold tail.
///
/// The source's `getTypingCodeBlockDuration`: `ceil(delay + code.length * typingSpeed)`.
/// `code_len` is a **character** count (see [`typed_char_count`] for why).
pub fn get_typing_code_block_duration(code_len: usize, typing_speed: f32, delay: usize) -> usize {
    let frames = delay as f32 + code_len as f32 * typing_speed;
    // `ceil` of a non-finite or negative value would make the cast meaningless, so clamp first.
    frames.ceil().max(0.0) as usize
}

/// Characters revealed at `frame`, the source's `typedCount`.
///
/// `min(char_count, max(0, floor((frame - delay) / typing_speed)))`. `typing_speed` is not
/// clamped: `0.0` types everything at once, exactly like the source's division by zero, and a
/// negative speed types nothing. Neither can panic.
pub fn typed_char_count(frame: f32, delay: usize, typing_speed: f32, char_count: usize) -> usize {
    if char_count == 0 {
        return 0;
    }
    let typed = ((frame - delay as f32) / typing_speed).floor().max(0.0);
    (typed as usize).min(char_count)
}

/// The first `typed_count` characters of `code`.
///
/// The source slices with `code.slice(0, typedCount)`, which counts UTF-16 code units and can
/// therefore split a surrogate pair. This uses `char_indices`, so it never splits a character —
/// and never panics on a multibyte boundary.
pub fn typed_slice(code: &str, typed_count: usize) -> &str {
    match code.char_indices().nth(typed_count) {
        Some((index, _)) => &code[..index],
        None => code,
    }
}

/// Caret opacity at `clock`, the source's `cursorVisible`.
///
/// A hard on/off edge reads as a dropped frame at 30 fps, so the caret ramps in over
/// [`BLINK_FADE_FRAMES`] frames at the start of each cycle and out over the same span before the
/// half-way point; the second half of the cycle is fully dark. `clock` is the unclamped
/// `max(0, frame - delay)`, so the caret keeps blinking after the code is complete.
pub fn cursor_visibility(clock: f32) -> f32 {
    let within = clock.max(0.0) % BLINK_CYCLE_FRAMES;
    let half = BLINK_CYCLE_FRAMES / 2.0;
    let fade_in = EASE_OUT.ease((within / BLINK_FADE_FRAMES).min(1.0));
    let fade_out = EASE_OUT.ease(((half - within) / BLINK_FADE_FRAMES).min(1.0));
    fade_in.min(fade_out).clamp(0.0, 1.0)
}

/// Whether a character can be part of an identifier.
fn is_word_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_' || character == '$'
}

/// Whether an unhighlighted run has to stop *before* this character.
///
/// A `/` stops the run so an immediately following `//` or `/*` is still recognised as a comment.
fn starts_token(character: char) -> bool {
    is_word_char(character) || matches!(character, '"' | '\'' | '`' | '/' | '\n')
}

/// Whether the next non-whitespace character after `index` is `(`, i.e. this is a call.
fn is_call(chars: &[char], mut index: usize) -> bool {
    while chars.get(index).is_some_and(|c| c.is_whitespace()) {
        index += 1;
    }
    chars.get(index) == Some(&'(')
}

/// Append a token to the line being built, dropping empty tokens.
fn push_token(lines: &mut [Vec<Token>], token: Token) {
    if token.text.is_empty() {
        return;
    }
    if let Some(line) = lines.last_mut() {
        line.push(token);
    }
}

/// Tokenize source code into one token list per `\n`-separated line.
///
/// Always returns at least one line, even for an empty string, because the source's
/// `Highlight` renders one (empty) line for empty code — which is where its caret sits.
///
/// This is a simplified, dependency-free stand-in for Prism. It recognises `//` line comments,
/// `/* ... */` block comments (which may span lines), `"`/`'`/backtick strings with backslash
/// escapes, decimal/hex/binary/octal numbers, the [`KEYWORDS`] set, and calls. Everything else
/// is [`TokenKind::Default`]. No token ever contains a newline.
pub fn tokenize(source: &str) -> Vec<Vec<Token>> {
    let chars: Vec<char> = source.chars().collect();
    let mut lines: Vec<Vec<Token>> = vec![Vec::new()];
    let mut index = 0;
    let mut in_block_comment = false;

    while index < chars.len() {
        let current = chars[index];

        if current == '\n' {
            lines.push(Vec::new());
            index += 1;
            continue;
        }

        // Inside `/* ... */`: consume up to the closing marker, or to the end of the line.
        if in_block_comment {
            let start = index;
            let mut closed = false;
            while index < chars.len() && chars[index] != '\n' {
                if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    index += 2;
                    closed = true;
                    break;
                }
                index += 1;
            }
            let text: String = chars[start..index].iter().collect();
            push_token(
                &mut lines,
                Token {
                    text,
                    kind: TokenKind::Comment,
                },
            );
            in_block_comment = !closed;
            continue;
        }

        // `//` to the end of the line.
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            let start = index;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            let text: String = chars[start..index].iter().collect();
            push_token(
                &mut lines,
                Token {
                    text,
                    kind: TokenKind::Comment,
                },
            );
            continue;
        }

        // `/*` opens a block comment; the branch above consumes it on the next pass.
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            in_block_comment = true;
            continue;
        }

        // A string literal, including a template literal. `${...}` inside a template literal is
        // not re-entered: the whole literal is one string token (see the README).
        if matches!(current, '"' | '\'' | '`') {
            let quote = current;
            let start = index;
            index += 1;
            while index < chars.len() && chars[index] != '\n' {
                if chars[index] == '\\' {
                    // Keep the escaped character inside the literal; a trailing backslash or a
                    // line continuation must not run past the end of the line.
                    if chars.get(index + 1).is_some_and(|next| *next != '\n') {
                        index += 2;
                    } else {
                        index += 1;
                    }
                    continue;
                }
                if chars[index] == quote {
                    index += 1;
                    break;
                }
                index += 1;
            }
            let text: String = chars[start..index].iter().collect();
            push_token(
                &mut lines,
                Token {
                    text,
                    kind: TokenKind::String,
                },
            );
            continue;
        }

        // A number: decimal with an optional fraction and exponent, or a 0x/0b/0o literal.
        if current.is_ascii_digit() {
            let start = index;
            let radix = current == '0'
                && chars
                    .get(index + 1)
                    .is_some_and(|c| matches!(*c, 'x' | 'X' | 'b' | 'B' | 'o' | 'O'));
            if radix {
                index += 2;
                while index < chars.len()
                    && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
                {
                    index += 1;
                }
            } else {
                while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '_')
                {
                    index += 1;
                }
                if chars.get(index) == Some(&'.')
                    && chars.get(index + 1).is_some_and(char::is_ascii_digit)
                {
                    index += 1;
                    while index < chars.len()
                        && (chars[index].is_ascii_digit() || chars[index] == '_')
                    {
                        index += 1;
                    }
                }
                if chars.get(index).is_some_and(|c| matches!(*c, 'e' | 'E')) {
                    let mut probe = index + 1;
                    if chars.get(probe).is_some_and(|c| matches!(*c, '+' | '-')) {
                        probe += 1;
                    }
                    if chars.get(probe).is_some_and(char::is_ascii_digit) {
                        index = probe;
                        while index < chars.len()
                            && (chars[index].is_ascii_digit() || chars[index] == '_')
                        {
                            index += 1;
                        }
                    }
                }
            }
            let text: String = chars[start..index].iter().collect();
            push_token(
                &mut lines,
                Token {
                    text,
                    kind: TokenKind::Number,
                },
            );
            continue;
        }

        // An identifier: a keyword, a call, or plain code.
        if current.is_alphabetic() || current == '_' || current == '$' {
            let start = index;
            while index < chars.len() && is_word_char(chars[index]) {
                index += 1;
            }
            let text: String = chars[start..index].iter().collect();
            let kind = if KEYWORDS.contains(&text.as_str()) {
                TokenKind::Keyword
            } else if is_call(&chars, index) {
                TokenKind::Function
            } else {
                TokenKind::Default
            };
            push_token(&mut lines, Token { text, kind });
            continue;
        }

        // Anything else: merge a run of plain characters into one token. The current character is
        // always consumed, so this branch can never fail to make progress.
        let mut end = index + 1;
        while end < chars.len() && !starts_token(chars[end]) {
            end += 1;
        }
        let text: String = chars[index..end].iter().collect();
        push_token(
            &mut lines,
            Token {
                text,
                kind: TokenKind::Default,
            },
        );
        index = end;
    }

    lines
}

/// The first `typed_count` characters of a tokenized source, as lines.
///
/// The last line is truncated mid-token when the typed prefix ends there, which is exactly what
/// re-tokenizing `code.slice(0, typedCount)` would produce. Always returns at least one line.
pub fn prefix_lines(lines: &[Vec<Token>], typed_count: usize) -> Vec<Vec<Token>> {
    let mut out: Vec<Vec<Token>> = Vec::new();
    let mut remaining = typed_count;

    for (index, line) in lines.iter().enumerate() {
        let length: usize = line.iter().map(|token| token.text.chars().count()).sum();
        if remaining >= length {
            out.push(line.clone());
            remaining -= length;
            if index + 1 < lines.len() {
                if remaining == 0 {
                    // The line's own newline has not been typed yet.
                    break;
                }
                // Consume the newline that separates this line from the next.
                remaining -= 1;
            }
            continue;
        }
        out.push(truncate_line(line, remaining));
        break;
    }

    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

/// The first `count` characters of one tokenized line.
fn truncate_line(line: &[Token], count: usize) -> Vec<Token> {
    let mut out = Vec::new();
    let mut remaining = count;
    for token in line {
        if remaining == 0 {
            break;
        }
        let length = token.text.chars().count();
        if length <= remaining {
            out.push(token.clone());
            remaining -= length;
        } else {
            out.push(Token {
                text: token.text.chars().take(remaining).collect(),
                kind: token.kind,
            });
            remaining = 0;
        }
    }
    out
}

/// The frame-independent geometry of one editor card.
///
/// Measuring walks the embedded font face, so it is done once per component instance and reused
/// for every frame after that — the card must not resize as the code types itself out.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Chrome {
    /// Resolved code font size in px.
    font_size: f32,
    /// Card left edge.
    x: f32,
    /// Card top edge.
    y: f32,
    /// Card width, sized to the longest line of the *full* code.
    width: f32,
    /// Card height, sized to the *full* code's line count.
    height: f32,
    /// Left edge of the code text.
    text_x: f32,
    /// Baseline of line `0`.
    first_baseline: f32,
    /// Distance between consecutive baselines.
    line_step: f32,
}

/// The Typing Code Block component.
#[derive(Debug, Clone, Default)]
pub struct TypingCodeBlock {
    /// Component options.
    pub config: TypingCodeBlockConfig,
    tokens: OnceLock<Vec<Vec<Token>>>,
    chrome: OnceLock<Chrome>,
}

impl TypingCodeBlock {
    /// Build with the given options.
    pub fn new(config: TypingCodeBlockConfig) -> Self {
        Self {
            config,
            tokens: OnceLock::new(),
            chrome: OnceLock::new(),
        }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "typing-code-block",
            name: "Typing Code Block",
            summary: "A highlighted editor window types a code sample out character by character.",
            feature: "typing-code-block",
        }
    }

    /// The whole sample, tokenized once per component instance.
    fn token_lines(&self) -> &Vec<Vec<Token>> {
        self.tokens.get_or_init(|| tokenize(&self.config.code))
    }

    /// The measured card, resolved once per component instance.
    fn chrome(&self, ctx: &RenderCtx<'_>, family: &str) -> Chrome {
        *self.chrome.get_or_init(|| {
            // A non-positive font size cannot be drawn; 1 px keeps the layout finite.
            let font_size = self
                .config
                .font_size
                .unwrap_or_else(|| ctx.width() * 0.02)
                .max(1.0);
            let lines = self.token_lines();
            let line_step = font_size * LINE_HEIGHT;
            let padding = font_size;
            let gutter = if self.config.show_line_numbers {
                GUTTER_EM * font_size
            } else {
                0.0
            };

            let mut code_width: f32 = 0.0;
            for line in lines {
                let width: f32 = line
                    .iter()
                    .map(|token| ctx.text_width(family, font_size, &token.text))
                    .sum();
                code_width = code_width.max(width);
            }

            let width = code_width + gutter + padding * 2.0;
            let height = line_step * lines.len().max(1) as f32 + padding * 2.0;
            let x = (ctx.width() - width) / 2.0;
            let y = (ctx.height() - height) / 2.0;
            Chrome {
                font_size,
                x,
                y,
                width,
                height,
                text_x: x + padding + gutter,
                first_baseline: y + padding + ctx.baseline_offset(family, font_size, line_step),
                line_step,
            }
        })
    }
}

impl Component for TypingCodeBlock {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_typing_code_block_duration(
            self.config.code.chars().count(),
            self.config.typing_speed,
            self.config.delay,
        ) + HOLD_TAIL_FRAMES
    }

    fn background(&self) -> Option<&'static str> {
        Some("#0d1117")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let family = ctx.mono();
        let chrome = self.chrome(ctx, &family);
        let frame = ctx.frame();

        let char_count = self.config.code.chars().count();
        let typed = typed_char_count(
            frame,
            self.config.delay,
            self.config.typing_speed,
            char_count,
        );
        let sliced = typed_slice(&self.config.code, typed);
        let lines = prefix_lines(self.token_lines(), typed);

        let clock = (frame - self.config.delay as f32).max(0.0);
        let cursor_opacity = cursor_visibility(clock);

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        nodes.push(fframes::svgr!(
            <rect x={chrome.x} y={chrome.y} width={chrome.width} height={chrome.height}
                  rx={CARD_RADIUS} fill={self.config.background.clone()} />
        ));

        // The line number's right edge sits one em left of the gutter's right edge
        // (`width: 2.5em; padding-right: 1em; text-align: right`).
        let number_x = chrome.text_x - GUTTER_PADDING_EM * chrome.font_size;

        for (index, line) in lines.iter().enumerate() {
            let y = chrome.first_baseline + index as f32 * chrome.line_step;
            if self.config.show_line_numbers {
                let number = (index + 1).to_string();
                nodes.push(fframes::svgr!(
                    <text x={number_x} y={y} font-family={family.clone()}
                          font-size={chrome.font_size} fill={LINE_NUMBER_COLOR}
                          opacity={LINE_NUMBER_OPACITY} text-anchor="end">{number}</text>
                ));
            }
            // Every token is placed absolutely, so a whitespace run only has to advance the pen;
            // it never has to be drawn (and therefore cannot be re-flowed by the renderer).
            let mut x = chrome.text_x;
            for token in line {
                if !token.text.chars().all(char::is_whitespace) {
                    nodes.push(text_node_left(
                        token.text.clone(),
                        x,
                        y,
                        family.clone(),
                        chrome.font_size,
                        FONT_WEIGHT,
                        0.0,
                        token.kind.color().to_owned(),
                        1.0,
                    ));
                }
                x += ctx.text_width(&family, chrome.font_size, &token.text);
            }
        }

        // The caret rides the end of the last visible line.
        let last_index = lines.len().saturating_sub(1);
        let last_line = sliced.rsplit('\n').next().unwrap_or_default();
        let cursor_x = chrome.text_x + ctx.text_width(&family, chrome.font_size, last_line);
        let cursor_y = chrome.first_baseline + last_index as f32 * chrome.line_step;
        nodes.push(text_node_left(
            CURSOR.to_owned(),
            cursor_x,
            cursor_y,
            family,
            chrome.font_size,
            FONT_WEIGHT,
            0.0,
            CURSOR_COLOR.to_owned(),
            cursor_opacity,
        ));

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &[Token]) -> Vec<(TokenKind, &str)> {
        line.iter()
            .map(|token| (token.kind, token.text.as_str()))
            .collect()
    }

    #[test]
    fn demo_code_is_the_source_sample() {
        assert_eq!(DEMO_CODE.lines().count(), 7);
        assert!(DEMO_CODE.starts_with("// components-generator\n"));
        assert!(DEMO_CODE.ends_with("greet(\"Remotion\");"));
        assert_eq!(DEMO_CODE.chars().count(), 148);
    }

    #[test]
    fn duration_matches_the_source_formula() {
        // ceil(delay + length * speed).
        assert_eq!(get_typing_code_block_duration(10, 0.5, 4), 9);
        assert_eq!(get_typing_code_block_duration(10, 0.0, 3), 3);
        assert_eq!(get_typing_code_block_duration(0, 0.3, 0), 0);
        assert_eq!(get_typing_code_block_duration(148, 0.3, 0), 45);
        assert_eq!(get_typing_code_block_duration(10, 0.5, 7), 12);
        // 30 * 0.3 is exactly 9.0 in binary floating point — in `f32` here and in JavaScript's
        // `f64` — so the ceiling is 9, not 10.
        assert_eq!(get_typing_code_block_duration(30, 0.3, 0), 9);
        // Degenerate inputs stay finite and never panic.
        assert_eq!(get_typing_code_block_duration(10, -5.0, 0), 0);
        assert_eq!(
            get_typing_code_block_duration(10, f32::INFINITY, 0),
            usize::MAX
        );
    }

    #[test]
    fn typed_char_count_matches_the_source_formula() {
        // Clean halves, so the assertions do not depend on binary rounding.
        assert_eq!(typed_char_count(0.0, 0, 0.5, 100), 0);
        assert_eq!(typed_char_count(4.9, 0, 0.5, 100), 9);
        assert_eq!(typed_char_count(5.0, 0, 0.5, 100), 10);
        assert_eq!(typed_char_count(1_000.0, 0, 0.5, 100), 100);
        assert_eq!(typed_char_count(5.0, 10, 0.5, 100), 0);
        assert_eq!(typed_char_count(15.0, 10, 0.5, 100), 10);
        // The default speed behaves exactly like the source's `frame / 0.3`: 3 / 0.3 rounds to
        // exactly 10.0 in `f32` here and in JavaScript's `f64`, so frame 3 shows 10 characters.
        assert_eq!(typed_char_count(0.0, 0, 0.3, 148), 0);
        assert_eq!(typed_char_count(1.0, 0, 0.3, 148), 3);
        assert_eq!(typed_char_count(3.0, 0, 0.3, 148), 10);
        assert_eq!(typed_char_count(45.0, 0, 0.3, 148), 148);
        // A speed of zero types everything at once (the source divides by zero too).
        assert_eq!(typed_char_count(0.0, 0, 0.0, 42), 0);
        assert_eq!(typed_char_count(1.0, 0, 0.0, 42), 42);
        // A negative speed types nothing.
        assert_eq!(typed_char_count(100.0, 0, -1.0, 42), 0);
        assert_eq!(typed_char_count(0.0, 0, 0.3, 0), 0);
    }

    #[test]
    fn typed_slice_never_splits_a_character() {
        let code = "aé😀b";
        assert_eq!(code.chars().count(), 4);
        assert_eq!(typed_slice(code, 0), "");
        assert_eq!(typed_slice(code, 1), "a");
        assert_eq!(typed_slice(code, 2), "aé");
        assert_eq!(typed_slice(code, 3), "aé😀");
        assert_eq!(typed_slice(code, 4), code);
        // Past the end is the whole string, not a panic.
        assert_eq!(typed_slice(code, 99), code);
    }

    #[test]
    fn cursor_blinks_on_a_thirty_frame_cycle() {
        // Hidden at the cycle boundary, ramping in over 2 frames.
        assert!((cursor_visibility(0.0) - 0.0).abs() < 1e-6);
        assert!(cursor_visibility(1.0) > 0.0);
        assert!((cursor_visibility(2.0) - 1.0).abs() < 1e-6);
        assert!((cursor_visibility(10.0) - 1.0).abs() < 1e-6);
        // Dark at the half-way point and for the whole second half.
        assert!((cursor_visibility(15.0) - 0.0).abs() < 1e-6);
        assert!((cursor_visibility(20.0) - 0.0).abs() < 1e-6);
        assert!((cursor_visibility(29.9) - 0.0).abs() < 1e-6);
        // The cycle repeats, and the clock is clamped at zero.
        assert!(cursor_visibility(31.0) > 0.0);
        assert!((cursor_visibility(30.0) - 0.0).abs() < 1e-6);
        assert!((cursor_visibility(-5.0) - 0.0).abs() < 1e-6);
        for frame in 0..120 {
            let value = cursor_visibility(frame as f32);
            assert!((0.0..=1.0).contains(&value), "opacity {value} out of range");
        }
    }

    #[test]
    fn tokenizer_recognises_the_demo_code() {
        let lines = tokenize(DEMO_CODE);
        assert_eq!(lines.len(), 7);
        assert_eq!(
            kinds(&lines[0]),
            vec![(TokenKind::Comment, "// components-generator")]
        );

        let export = &lines[1];
        assert_eq!(export[0].kind, TokenKind::Keyword);
        assert_eq!(export[0].text, "export");
        assert_eq!(export[1].text, " ");
        // `string` is a keyword. `greet` is *not* a function here: a declaration name is followed
        // by `=`, and this tokenizer only colours calls (see the README's fidelity notes).
        assert!(
            export
                .iter()
                .any(|token| token.kind == TokenKind::Keyword && token.text == "string")
        );
        assert!(
            export
                .iter()
                .any(|token| token.kind == TokenKind::Default && token.text == "greet")
        );

        // A template literal is one string token, `${name}` included.
        let template = lines[2]
            .iter()
            .find(|token| token.kind == TokenKind::String)
            .expect("the template literal");
        assert_eq!(template.text, "`Hello, ${name}!`");

        // `log` is a call because it is followed by `(`; `console` is not (a `.` follows it).
        assert!(
            lines[3]
                .iter()
                .any(|token| token.kind == TokenKind::Function && token.text == "log")
        );
        // The call site, `greet("Remotion");`, does colour as a function.
        assert!(
            lines[6]
                .iter()
                .any(|token| token.kind == TokenKind::Function && token.text == "greet")
        );
        assert!(
            lines[3]
                .iter()
                .any(|token| token.kind == TokenKind::Default && token.text == "console")
        );
        assert!(
            lines[3]
                .iter()
                .any(|token| token.kind == TokenKind::Default && token.text == "message")
        );

        // Every line's tokens rebuild the line exactly.
        for (line, tokens) in DEMO_CODE.split('\n').zip(&lines) {
            let rebuilt: String = tokens.iter().map(|token| token.text.as_str()).collect();
            assert_eq!(rebuilt, line);
        }
    }

    #[test]
    fn tokenizer_handles_numbers_strings_escapes_and_block_comments() {
        let lines = tokenize("let x = 0x1F + 1.5e-3 + 42; // tail");
        let texts: Vec<_> = lines[0].iter().map(|t| (t.kind, t.text.as_str())).collect();
        assert!(texts.contains(&(TokenKind::Number, "0x1F")));
        assert!(texts.contains(&(TokenKind::Number, "1.5e-3")));
        assert!(texts.contains(&(TokenKind::Number, "42")));
        assert!(texts.contains(&(TokenKind::Keyword, "let")));
        assert!(texts.contains(&(TokenKind::Comment, "// tail")));

        // Escapes keep the quote inside the literal.
        let escaped = tokenize(r#"a = "he said \"hi\"" + 'x';"#);
        assert_eq!(
            escaped[0]
                .iter()
                .filter(|t| t.kind == TokenKind::String)
                .map(|t| t.text.as_str())
                .collect::<Vec<_>>(),
            vec![r#""he said \"hi\"""#, "'x'"]
        );

        // An unterminated string or block comment runs to the end of the line, not past it.
        let unterminated = tokenize("a = 'oops\nb = 2");
        assert_eq!(unterminated.len(), 2);
        assert_eq!(unterminated[0][2].kind, TokenKind::String);
        assert_eq!(unterminated[1][2].kind, TokenKind::Number);

        // A block comment spans lines and colours every line it covers.
        let block = tokenize("/* one\ntwo */ let a = 1;");
        assert_eq!(block.len(), 2);
        assert_eq!(block[0][0].kind, TokenKind::Comment);
        assert_eq!(block[1][0].text, "two */");
        assert_eq!(block[1][0].kind, TokenKind::Comment);
        assert!(block[1].iter().any(|t| t.text == "let"));

        // An empty source still has one (empty) line, which is where the caret sits.
        assert_eq!(tokenize(""), vec![Vec::<Token>::new()]);
        assert_eq!(
            tokenize("\n"),
            vec![Vec::<Token>::new(), Vec::<Token>::new()]
        );
    }

    #[test]
    fn tokenizer_does_not_loop_forever_on_odd_input() {
        // A lone slash is plain code, not a comment; an empty source, trailing punctuation and
        // non-ASCII identifiers all terminate.
        assert_eq!(tokenize("/")[0][0].text, "/");
        assert_eq!(tokenize("a / b")[0].len(), 4);
        assert_eq!(tokenize("=>{}")[0].len(), 1);
        assert_eq!(tokenize("héllo_1")[0][0].text, "héllo_1");
        assert_eq!(tokenize("'").len(), 1);
        assert_eq!(tokenize("`a\\").len(), 1);
    }

    #[test]
    fn prefix_lines_matches_a_slice_of_the_source() {
        let lines = tokenize("ab\ncd");
        let render = |count: usize| -> Vec<String> {
            prefix_lines(&lines, count)
                .iter()
                .map(|line| line.iter().map(|t| t.text.as_str()).collect::<String>())
                .collect()
        };
        assert_eq!(render(0), vec![""]);
        assert_eq!(render(1), vec!["a"]);
        assert_eq!(render(2), vec!["ab"]);
        // Two characters plus the newline: the second line exists but is still empty.
        assert_eq!(render(3), vec!["ab", ""]);
        assert_eq!(render(4), vec!["ab", "c"]);
        assert_eq!(render(5), vec!["ab", "cd"]);
        assert_eq!(render(99), vec!["ab", "cd"]);
    }

    #[test]
    fn prefix_lines_truncates_mid_token_and_tracks_the_code() {
        let code = "const value = 1234;\n// note";
        let lines = tokenize(code);
        for count in 0..=code.chars().count() {
            let rebuilt: Vec<String> = prefix_lines(&lines, count)
                .iter()
                .map(|line| line.iter().map(|t| t.text.as_str()).collect::<String>())
                .collect();
            let expected: Vec<String> = typed_slice(code, count)
                .split('\n')
                .map(str::to_owned)
                .collect();
            assert_eq!(rebuilt, expected, "prefix of {count} characters");
        }
    }

    #[test]
    fn duration_covers_typing_and_the_hold_tail() {
        let component = TypingCodeBlock::default();
        // 148 characters at 0.3 frames each = 44.4 -> 45 frames, plus the 90 frame tail.
        assert_eq!(component.duration(), 45 + 90);
        assert_eq!(component.meta().id, "typing-code-block");
        assert_eq!(component.background(), Some("#0d1117"));

        let delayed = TypingCodeBlock::new(TypingCodeBlockConfig {
            code: "ab".to_owned(),
            delay: 10,
            typing_speed: 1.0,
            ..TypingCodeBlockConfig::default()
        });
        assert_eq!(delayed.duration(), 12 + 90);

        // Empty code is still a positive duration: the caret blinks over the hold tail.
        let empty = TypingCodeBlock::new(TypingCodeBlockConfig {
            code: String::new(),
            ..TypingCodeBlockConfig::default()
        });
        assert_eq!(empty.duration(), HOLD_TAIL_FRAMES);
    }

    #[test]
    fn every_frame_renders_without_panicking() {
        let component = TypingCodeBlock::default();
        let meters = crate::embedded_meters();
        let duration = component.duration();
        assert_eq!(duration, 135);
        for frame in 0..duration {
            let mut raw = fframes::Frame::new(frame, frame, 30);
            let info = crate::ctx::ComponentFrame {
                local: frame as f32,
                global: frame,
                fps: 30.0,
                duration,
                width: 1920.0,
                height: 1080.0,
            };
            let mut ctx = RenderCtx::new(&mut raw, crate::core::Fonts::default(), &meters, info);
            let svg = component.show(&mut ctx);
            assert!(
                !format!("{svg}").is_empty(),
                "frame {frame} produced nothing"
            );
        }
    }
}
