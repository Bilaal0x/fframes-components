# Slide In Text — `slide-in-text`

Words slide in from one edge of the frame and ease into their resting places, one after another.

## What it does

Every word travels the same vector — `left { x: -1, y: 0 }`, `right { x: 1, y: 0 }`,
`top { x: 0, y: -1 }`, `bottom { x: 0, y: 1 }` — scaled by `distance`, and shares the library's
single `EASE_OUT` curve (`cubic-bezier(0.23, 1, 0.32, 1)`), so each word moves as a unit while
the stagger cascades them. Opacity rides the very same eased progress, so a word is fully faded
in exactly when it stops moving.

Beat by beat, with the default configuration (5 words, `direction = bottom`, `duration = 30`,
`stagger = 1`, `delay = 0`, `distance = 40`, 30 fps):

| frame | what is on screen |
| --- | --- |
| 0 | "Slides" appears 40 px below its resting place, opacity 0, and starts rising. |
| 1–4 | "in", "from", "an", "edge" follow, one per frame. |
| 3 | "Slides" is ~24 px below, opacity 0.40 (the ease-out has already covered 40% of the travel). |
| 8 | "Slides" is ~8 px below, opacity 0.79. |
| 15 | "Slides" is ~1.4 px below, opacity 0.97 — effectively landed. |
| 30 | "Slides" has arrived: 0 travel, opacity 1. |
| 34 | The last word ("edge") lands — `30 + 4 * 1`. |
| 35–79 | The finished line holds for the 45-frame tail (1.5 s at 30 fps). |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text.text` | `String` | `"Slides in from an edge"` | The line to reveal. |
| `text.font_size` | `f32` | `64.0` | Font size in px. |
| `text.font_weight` | `u16` | `700` | Font weight. |
| `text.color` | `String` | `"#e6edf3"` | Text fill. |
| `text.max_width` | `f32` | `900.0` | Wrap width in px. The words are laid out on one line until they no longer fit, then wrapped. |
| `text.line_height` | `f32` | `1.25` | Line height as a multiple of `font_size`. |
| `text.tracking_em` | `f32` | `-0.02` | Letter spacing in `em`. Negative tightens. |
| `direction` | `SlideDirection` | `Bottom` | Edge the words enter from: `Left`, `Right`, `Top` or `Bottom`. `top` slides down, `bottom` slides up. |
| `duration` | `usize` | `30` | Frames each word takes to arrive. |
| `stagger` | `usize` | `1` | Frames between consecutive word starts. |
| `delay` | `usize` | `0` | Frames to wait before the first word starts. |
| `distance` | `f32` | `40.0` | Pixels each word travels in from its resting place. |
| `tail` | `usize` | `45` | Frames held after the last word lands. |
| `reduced_motion` | `bool` | `false` | Collapse travel to 0; keep the fades. |

`text` is the shared [`TextSpec`](../text_spec.rs) block, so the styling defaults are the
library-wide ones. The source's `style` prop maps onto it.

## How to render it

```sh
# just this component
cargo run --release -- render --components slide-in-text -o out.mp4

# as part of a selection
cargo run --release -- render --components word-by-word,slide-in-text -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components slide-in-text snapshot
cargo run --release -- --components slide-in-text --list
```

The component only exists when its Cargo feature is on, so a build that is not already using
`--features full` adds it (plus `cli` for the renderer binary) explicitly:

```sh
cargo run --release --features cli,slide-in-text -- render --components slide-in-text -o out.mp4
```

## Duration

```text
entry_frames(word_count, duration, stagger) = word_count == 0 ? 0 : duration + (word_count - 1) * stagger
duration()                                  = entry_frames(...) + tail
```

With the defaults (5 words): `30 + 4 * 1 = 34` entry frames `+ 45` tail = **79 frames**, 2.63 s
at 30 fps. `delay` is deliberately not part of this figure, matching the library's other
word-reveal components.

## Fidelity notes

- **Nothing is faked.** The source's `translate(x, y)` is a plain 2D translation, the offset
  vector is used linearly, and the entrance curve is the library's own `EASE_OUT` — SVG can
  express all of it, so there is no approximation here.
- The source leaves the words in the inline flow of a `<span>`; this port lays them out itself
  with the shared wrapper so every word's centre is known. Word widths come from the face
  embedded in this crate and do not include the negative tracking, so a wrapped line can break a
  few pixels away from the browser's break point.
- Words are split on any run of whitespace (the shared `ctx::words`), where the source splits on
  the literal space character; text containing tabs or newlines yields more words here.
- `distance` is a scalar along the direction vector, so `Left`/`Right` travel horizontally,
  `Top`/`Bottom` vertically, and `reduced_motion: true` collapses the travel while keeping the
  eased fade.

## Credit

Ported from `SlideInText.tsx` in the Remotion component library by the **remocn** authors,
<https://github.com/Remocn/remocn> (MIT). This port reimplements the animation against the
fframes SVG + Rust API and is not affiliated with Remotion.
