# Staggered Fade Up

`staggered-fade-up` — words fade in and rise one after another on a plain linear clock.

## What it does

Word `i` starts at frame `i * stagger` (4) and runs for `word_frames` (12). Both channels are
linear — the source passes no `easing` to either `interpolate`, so this port's plain `clamp01` ramp
matches it exactly rather than approximating it: opacity climbs 0 → 1 while the word rises the last
of `distance` onto its resting baseline. Everything is clamped before a word's turn, so an unrevealed
word waits `distance` px low in zero opacity, and a word that has landed never moves again. There is
no easing curve anywhere in the component, which is what separates it from the eased word reveals in
this library, and there is no exit.

The line is laid out by the shared wrapper: every word is measured with the face embedded in the
crate, wrapped greedily at `text.max_width`, centred line by line, with the whole block centred
vertically.

Beat by beat, with the defaults (8 words, 30 fps; distances in canvas px on the 1080p canvas,
reference px in brackets):

| frames (30 fps) | beat |
| --- | --- |
| 0 | "Staggered" appears 30 px (20) below its resting baseline, opacity 0. Every later word waits there with it. |
| 6 | "Staggered" is halfway: opacity 0.50, 15 px (10) below. |
| 12 | "Staggered" is at rest, opacity 1. |
| 28 | The last word ("time", index 7) starts — `7 * 4`. |
| 40 | The last word lands at `12 + 7 * 4`: the entrance ends. |
| 40 – 79 | The finished line holds for the 40-frame tail (1.33 s). |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text.text` | `String` | `"Staggered fade up, one word at a time"` | The line to reveal. |
| `text.font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `text.font_weight` | `u16` | `600` | Font weight. |
| `text.color` | `String` | `"#e6edf3"` | Text fill. |
| `text.max_width` | `f32` | `960.0` | Wrap width in reference-canvas px. |
| `text.line_height` | `f32` | `1.2` | Line height as a multiple of `font_size`. |
| `text.tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
| `stagger` | `usize` | `4` | Frames between consecutive word starts. |
| `distance` | `f32` | `20.0` | Reference-canvas px each word rises. |
| `word_frames` | `usize` | `12` | Frames each word takes to land. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `40` | Frames held after the last word lands. |
| `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |

`text` is the shared [`TextSpec`](../text_spec.rs) block, so the styling defaults are the
library-wide ones; the values above are the ones this component sets, with `text.color` coming from
`TextSpec::default()`. The source's props are `text`, `staggerDelay`, `distance`, `fontSize`,
`color`, `fontWeight` and `speed`, with the 12-frame window hardcoded in its `interpolate` calls and
no hold after the line; `staggerDelay` is this port's `stagger`, and `word_frames` and `tail` expose
the window and the hold.

`staggered_fade_up::entry_frames` and `staggered_fade_up::word_channels` are public, so the timeline
and one word's opacity / offset can be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components staggered-fade-up -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,staggered-fade-up -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components staggered-fade-up snapshot
cargo run --release -- --components staggered-fade-up --list
```

## Duration

```text
entry_frames = words == 0 ? 0 : ceil((word_frames + (words - 1) * stagger) / speed)
duration()   = entry_frames + tail
```

`words` is the whitespace-separated word count of `text` (`TextSpec::word_count`). With the defaults
(8 words): `12 + 7 * 4 = 40` entry frames `+ 40` tail = **80 frames**, 2.67 s at 30 fps. `tail` is
not divided by `speed`, so it is a hold in real frames at any speed, and an empty `text` yields
`duration() = tail`.

## Fidelity notes

- **Layout and wrapping.** The source renders every word in one inline flow inside a full-canvas flex
  box, with `marginRight: "0.25em"` after every word and no wrap width of its own, so the line wraps
  at the composition width, centred including that trailing gap. This port measures each word from
  the face embedded in the crate, wraps greedily at `text.max_width`, centres each line and centres
  the block vertically. Inter-word gaps come from the font's own space advance rather than that
  margin, no gap follows the last word, and the wrap point can differ from the browser's by a word.
- **Word splitting.** The source splits on the literal space character (`text.split(" ")`); this port
  splits on any run of whitespace (`ctx::words` / `ctx::layout_paragraph`), so text containing tabs,
  newlines or doubled spaces yields more words here — and since the timeline is built from
  `word_count()`, it grows with them too.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, `distance: 20` is a 30 px rise, and `max_width: 960` wraps at 1440 px at 1080p.
- **`reduced_motion`** collapses the travel to zero — the words still fade in on their un-eased
  stagger, but nothing moves.
- **Degenerate values.** A non-positive or non-finite `speed` falls back to `1` instead of freezing
  or reversing the animation, and a zero `word_frames` is floored to one frame.
- **Backdrop and text colour.** The source demo paints `#ffffff` behind `#171717` text and its demo
  line is `"Ship faster with remocn"`; this port keeps the house dark style instead — `color`
  defaults to `#e6edf3`, the default line is `"Staggered fade up, one word at a time"`, and
  `background()` returns `#0d1117` in place of the source's white demo canvas.

## Credit

Ported from `staggered-fade-up/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
