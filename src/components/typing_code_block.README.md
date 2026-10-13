# Typing Code Block

A syntax-highlighted editor window types a code sample out one character at a time, behind a
right-aligned line-number gutter, with a `|` caret blinking at the end of the last line. The caret
keeps blinking after the code is complete, because its clock is the *unclamped* one.

Component id: `typing-code-block` · Cargo feature: `typing-code-block` ·
Module: `src/components/typing_code_block.rs`

## What it does

The source reveals `code.slice(0, typedCount)` and re-tokenizes it every frame; this port tokenizes
the whole sample once per component instance and takes the visible prefix of that token list. The
two are provably the same list — a comment or a string literal can never change retroactively — so
the highlight is identical frame for frame and the work is done once instead of 300 times.

The caret has two clocks on purpose. Its *position* follows the typed prefix (it always sits at the
end of the last visible line, measured with `ctx.text_width`), while its *opacity* follows
`clock = max(0, frame - delay)` with no clamping, so it never stops blinking — not during the delay
and not after the code is finished.

Beat by beat with the defaults (the 7-line `greet` sample, 148 characters, `typing_speed = 0.3`,
`delay = 0`):

| frame | what is on screen |
| --- | --- |
| `0` | the empty editor card; the caret is present but dark (`clock % 30 == 0`) |
| `1`–`2` | the caret ramps up to full opacity (`EASE_OUT` over 2 frames) |
| `1` | 3 characters (`floor(1 / 0.3)` = 3) |
| `3` | 10 characters — `3 / 0.3` rounds to exactly `10.0` in binary floating point, here and in JavaScript |
| … | 3 characters per frame, one line number per line |
| `45` | all 148 characters are on screen (`ceil(148 × 0.3)` = 45) |
| `45`–`59` | the caret is dark (the second half of its 30-frame cycle) |
| `61`–`74` | the caret is bright again; it keeps blinking for the rest of the component |
| `135` | end of the component: 90 hold frames after the last character |

With `delay = 10` the same typing starts at frame 10 and everything shifts by 10 frames; the caret
still blinks through the delay.

### The editor chrome

The card is one rounded `<rect>`; the code, the gutter and the caret are `<text>` nodes on top.

| property | value |
| --- | --- |
| family | `ctx.mono()` (the monospaced family; `Fonts::mono`, default `"DM Sans"`) |
| weight | `400` |
| font size | `font_size`, or `ctx.width() * 0.02` when `None` (38.4 px at 1080p) |
| line height | `1.5` × font size |
| card padding | one font size on every side |
| card radius | `12` px |
| card size | the *full* code's longest line + gutter + padding, centred on the canvas |
| gutter | `2.5em` wide, `1em` right padding, the number right-aligned |
| line numbers | `#666` at `opacity 0.5` |
| caret | `|`, `#d4d4d4`, opacity from the blink curve |
| token colours | keyword `#569cd6`, string `#ce9178`, comment `#6a9955`, number `#b5cea8`, function `#dcdcaa`, default `#d4d4d4` |
| component background | `#0d1117` (the source demo's backdrop); the card's own fill is `background` |

Each token is drawn as its own absolutely-positioned `<text>` node; whitespace runs only advance the
pen, they are never drawn. That is what keeps indentation exact without relying on the renderer's
own whitespace handling.

### The default sample

```ts
// components-generator
export const greet = (name: string) => {
  const message = `Hello, ${name}!`;
  console.log(message);
};

greet("Remotion");
```

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `code` | `String` | the source's `DEMO_CODE` (above) | Full code string; typing reveals it left to right. |
| `language` | `String` | `"tsx"` | Prism language id, kept for API compatibility. The tokenizer is language-agnostic and ignores it, so an unknown language can never fail. |
| `typing_speed` | `f32` | `0.3` | Frames per character; below `1.0` types several characters per frame. `0.0` types everything at once. |
| `delay` | `usize` | `0` | Frames to wait before typing starts. The caret still blinks. |
| `show_line_numbers` | `bool` | `true` | Show the right-aligned line-number gutter. |
| `background` | `String` | `"#1e1e1e"` | Editor card fill colour. |
| `font_size` | `Option<f32>` | `None` | Code font size in px; `None` resolves to `ctx.width() * 0.02`. |

## How to render it

```sh
# just this component
cargo run --release --features cli,typing-code-block -- render --components typing-code-block -o out.mp4

# as part of a selection
cargo run --release --features cli,typing-code-block,text-reveal -- render --components text-reveal,typing-code-block -o out.mp4

# inspect / preview / a still frame
cargo run --release --features cli,typing-code-block -- --components typing-code-block snapshot
cargo run --release --features cli,typing-code-block -- --components typing-code-block --list
```

Library use:

```rust
use fframes_components::components::typing_code_block::{
    get_typing_code_block_duration, tokenize, typed_char_count, TypingCodeBlock,
    TypingCodeBlockConfig,
};

let duration = get_typing_code_block_duration(148, 0.3, 0);
let component = TypingCodeBlock::new(TypingCodeBlockConfig {
    typing_speed: 1.0,
    ..TypingCodeBlockConfig::default()
});
```

## Duration

```text
get_typing_code_block_duration(code_len, typing_speed, delay)
    = ceil(delay + code_len * typing_speed)
duration() = get_typing_code_block_duration(...) + 90
```

* `code_len` is a **character** count (`code.chars().count()`), matching the character-by-character
  reveal. The source uses `code.length`, which counts UTF-16 code units; the two agree for every
  character outside the astral planes.
* The `90` is the source's `TYPING_CODE_TAIL_FRAMES`: without a hold, the finished code could never
  be read, and the caret could never be seen blinking over it.
* Default: `ceil(148 × 0.3)` = `45` typing frames + `90` tail = **135 frames** = **4.50 s** at
  30 fps.
* `delay` shifts the typing inside the component's slot; it is already part of the formula, so the
  slot grows by exactly `delay` frames.

`typed_char_count(frame, delay, typing_speed, char_count)` is the reveal formula itself
(`min(char_count, max(0, floor((frame - delay) / typing_speed)))`), and
`typed_slice(code, typed_count)` is the visible prefix.

## Fidelity notes

* **The highlighter is an approximation, not Prism.** The source runs `prism-react-renderer` with
  the `vsDark` theme and a full grammar per language. This port ships a small dependency-free
  tokenizer (`tokenize`) that recognises:
  * `//` line comments and `/* … */` block comments (which may span lines — every line they cover
    is coloured as a comment);
  * `"…"`, `'…'` and backtick literals, with backslash escapes;
  * decimal / hex / binary / octal numbers, including a fraction and an exponent;
  * a merged C-like and Python-ish keyword set (`KEYWORDS`, 71 words);
  * an identifier immediately followed by `(` as a function.
  Everything else is the default colour. The colours approximate `vsDark`; they are not read from
  the theme.
* **What the tokenizer does not do** (all of it deliberate): no `#` comments (Python and shell
  users lose them — the tokenizer is language-agnostic by contract), no JSX/HTML tags or
  attributes, no decorators, no `f"…"` string prefixes (`f` is a plain identifier, the literal
  after it is still a string), no `.5` (a leading dot is punctuation) and no `1.` (a trailing dot
  is punctuation), no nested highlighting inside a template literal (`${name}` is part of the
  string token, where `vsDark` colours the braces and the identifier), no operator/punctuation
  classes, and no `language`-specific rules at all.
* **Only calls are coloured as functions, not declarations.** `vsDark` colours the name in
  `function greet()` and `const greet = () => …` as a function (a `#dcdcaa` identifier), while this
  tokenizer only recognises an identifier immediately followed by `(`. So `greet` in the default
  sample's declaration line is drawn in the default colour, and `greet("Remotion")` and
  `console.log(…)` are drawn as functions.
* **The `language` field is inert.** It is kept so the API matches the source's props; passing
  `"python"` changes nothing about the output.
* **The card is sized to its content, the source is not.** The source's outer `div` is a block, and
  its demo passes `width: "80%"` plus `overflow: hidden`; this library has no caller-provided width,
  so the card is measured to the full code's longest line (+ gutter + padding) and centred. A code
  sample wider than the canvas therefore produces a card that runs off the edge; SVG here has no
  clip, so **nothing is truncated or hidden** — it simply overflows.
* **The card does not resize as it types.** Its size comes from the *full* code, so the window is
  already the right size on frame 0 and never jumps.
* **No editor scrollbar, no selection, no focus ring, no soft wrap.** The source sets
  `white-space: "pre"`, so it never wrapped either; there is no scroll region here at all.
* **The monospaced family may not be monospaced.** The crate resolves `Fonts::mono` and
  `Fonts::sans` to the same embedded DM Sans face today (`embedded_meters`), so code does not line
  up in columns the way a real mono face would. The layout is still measured, so tokens never
  overlap; add a monospaced `.ttf` to `media/` and point `Fonts::mono` at it for the real thing.
  This is a crate-wide note, not something this component can fix.
* **Characters, not UTF-16 units.** `typed_slice` cuts with `char_indices`, so it never splits a
  character and never panics on a multibyte boundary. The source's `slice` counts UTF-16 code
  units, so an astral character (an emoji) takes two of its steps and one of this port's. The
  reveal *rate* is identical; only the step count for astral characters differs.
* **The caret is a `<text>` node.** The source renders an inline-block `<span>` after the last
  token; here the caret is measured to the end of the last visible line and drawn with the same
  opacity curve. The 2-frame `EASE_OUT` ramp in and out of each half-cycle is the source's formula
  verbatim, including being dark at exactly the halfway frame.
* **Timing is the source's, to the frame.** The reveal and the duration use the same
  `floor` / `ceil` of the same binary floating-point arithmetic as the source, including its
  quirks: `3 / 0.3` is exactly `10.0` and `30 * 0.3` is exactly `9.0`, so frame 3 shows 10
  characters and a 30-character sample takes 9 frames. The fields here are `f32` where the
  source's numbers are JavaScript `f64`, so a product that lands within an ulp of an integer
  could in principle round the other way; for the default sample both give 45 typing frames.
* **A non-positive font size is clamped to 1 px.** CSS would accept `font-size: 0`; SVG needs a
  positive size, and a zero-height line box has no meaningful baseline. This is only reachable by
  passing `font_size: Some(0.0)` (or a negative one), or by rendering on a zero-width canvas.
* **Nothing else was lost.** The source uses no blur, no 3D, no blend mode and no canvas-only
  effect.

## Credit

Ported from `TypingCodeBlock.tsx` in the Remotion component library by the **remocn** authors
(<https://github.com/Remocn/remocn>, MIT). The typing formula, the caret curve, the editor chrome
metrics, the `DEMO_CODE` sample and the defaults are theirs; this is a reimplementation against
fframes' SVG + Rust API and is not affiliated with Remotion. The syntax highlighter is this port's
own simplified approximation of `prism-react-renderer`'s `vsDark` output.
