# Short Slide Right

`short-slide-right` — the whole line slides in from the left while its words fade in one after
another behind it.

## What it does

Three clocks run at once, all under the same `EASE_CHAR` curve (`cubic-bezier(0.2, 0.8, 0.2, 1)`)
the source uses for every channel here. The *line* owns the travel: over `travel_frames` (8) its
horizontal offset runs from `-distance` (24 reference px) to 0. The *line* also owns the softness,
which falls from `blur_from` (1.2) to 0 over the longer `enter_frames` (16). The *words* own the
opacity: word `i` fades in over `word_frames` (6) starting at frame `i * stagger` (3).

Splitting the channels that way is what makes the words appear to catch up with the line rather
than ride it: at frame 2 the line is already 77% of the way in and only 0.93 px soft, while word 0
is still half faded and word 1 has not started.

Beat by beat, with the defaults (5 words, on the 1080p canvas where the reference px are drawn at
1.5x):

| frames (30 fps) | beat |
| --- | --- |
| 0 | The line starts 36 px (24) left of its place, soft over a 1.8 px (1.2) radius; no word is visible. |
| 2 | The line is 8.4 px left of its place and 0.93 px soft. "Slide" is half faded in. |
| 4 | The line is 1.9 px from its place and 0.42 px soft. "in", "from", "the" and "left" start at frames 3, 6, 9 and 12. |
| 6 | "Slide" is opaque. |
| 8 | The line has arrived: its offset is 0, with only 0.10 px of softness left. |
| 12 | The last word ("left") starts its own 6-frame fade. |
| 16 | The line is sharp: the blur clock is the longest of the three. |
| 18 | The last word lands: `entry_frames = 6 + 4 * 3`. |
| 18 – 62 | The 45-frame tail (1.50 s) holds the finished line. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text.text` | `String` | `"Slide in from the left"` | The line to reveal. |
| `text.font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `text.font_weight` | `u16` | `600` | Font weight. |
| `text.color` | `String` | `"#e6edf3"` | Text fill. |
| `text.max_width` | `f32` | `960.0` | Wrap width in reference-canvas px. |
| `text.line_height` | `f32` | `1.2` | Line height as a multiple of `font_size`. |
| `text.tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
| `distance` | `f32` | `24.0` | Reference-canvas px the line travels in. |
| `stagger` | `usize` | `3` | Frames between consecutive word starts. |
| `enter_frames` | `usize` | `16` | Frames the line's softness takes to clear. |
| `travel_frames` | `usize` | `8` | Frames the line takes to reach its place. |
| `word_frames` | `usize` | `6` | Frames each word takes to fade in. |
| `blur_from` | `f32` | `1.2` | Starting softness radius in reference-canvas px. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `45` | Frames held after the last word lands. |
| `reduced_motion` | `bool` | `false` | Collapse travel and softness; keep the fades. |
| `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for a real gaussian blur. |

`text` is the shared [`TextSpec`](../text_spec.rs) block, so the styling defaults are the
library-wide ones; the values above are the ones this component sets, with `text.color` coming from
`TextSpec::default()`. The source's props are `text`, `distance`, `staggerDelay`, `fontSize`,
`color`, `fontWeight` and `speed`; its three clocks (16, 8 and 6 frames) and its `1.2` px blur are
literals in the component, with `staggerDelay` as this port's `stagger` and `blur_from` as its blur.
`tail`, `reduced_motion` and `simulate_blur` are this port's own.

`short_slide_right::entry_frames`, `short_slide_right::line_channels` (returning
`(x, blur)`) and `short_slide_right::word_opacity` are public, so the timeline, the line's two
channels and one word's fade can be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components short-slide-right -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,short-slide-right -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components short-slide-right snapshot
cargo run --release -- --components short-slide-right --list
```

## Duration

```text
words        = whitespace-separated word count of text.text
word_end     = words == 0 ? 0 : word_frames + (words - 1) * stagger            (= 18)
entry_frames = max(enter_frames, travel_frames, word_end)                       (= 18)
duration()   = ceil(entry_frames / speed) + tail
```

With the defaults (5 words): `max(16, 8, 18) = 18` entry frames `+ 45` tail = **63 frames**, 2.10 s
at 30 fps. The word stagger is the longest channel here; with a single word the line's 16-frame blur
clock sets the entry instead (and a wordless `text` gives `max(16, 8, 0) = 16`). `tail` is not
divided by `speed`.

The source has no entry-length helper at all — the channels simply clamp where they clamp — and its
demo composition is a flat 60 frames (`config.ts`); this `duration()` is the longest channel plus
the hold.

## Fidelity notes

- **Blur is approximated, not rendered.** The source applies `filter: blur(1.2px)` to the *whole
  line*; SVG has no gaussian path in this renderer, so `simulate_blur` (default `true`) stands in
  with `BLUR_GHOST_COPIES` = 2 extra copies per *word*, offset along the horizontal travel axis by
  `BLUR_GHOST_SPREAD_RATIO` (0.5) of the radius — one at `+0.25 * blur`, one at `-0.5 * blur`, i.e.
  `+0.45 px` and `-0.9 px` at the default 1.8 px starting radius — each drawn behind the sharp copy at
  `min(1, blur / (BLUR_GHOST_REFERENCE_PX (12) * unit)) * BLUR_GHOST_MAX_ALPHA (0.45) / 2` times the
  word's own opacity. Because the copies follow each word rather than the run, the stand-in does not
  smear across the gaps between words as the source's line-level filter would. At a 1.2 reference px
  radius the effect barely reads either way; set `simulate_blur: false` for a purely positional
  slide.
- **Layout and wrapping.** The source renders every word in one inline line inside a full-canvas
  flex box, with `marginRight: 0.25em` after every word and no wrap width of its own. This port
  measures each word with the face embedded in the crate, wraps greedily at `text.max_width`
  (960 reference px), centres each line and centres the block vertically; inter-word gaps come from
  the font's own space advance (≈0.26 em) and no gap follows the last word.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, `distance: 24` is a 36 px slide, and `blur_from: 1.2` is a 1.8 px radius.
- **Degenerate values.** A non-positive or non-finite `speed` falls back to `1` instead of freezing
  or reversing the animation, and a zero `travel_frames`, `enter_frames` or `word_frames` is floored
  to one frame (so every channel finishes on frame 1 and the words are still staggered).
  `reduced_motion` zeroes the travel and the softness but keeps the word fades.
- **Backdrop and text colour.** The source demo paints `#ffffff` behind `#171717` text and its demo
  line is `"Move with intent."`; this port keeps the house dark style instead — `color` defaults to
  `#e6edf3`, the default line is `"Slide in from the left"`, and `background()` returns `#0d1117` in
  place of the source's white demo canvas.

## Credit

Ported from `short-slide-right/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
