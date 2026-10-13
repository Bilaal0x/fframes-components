# Word by Word

Words appear one after another with staggered timing, each rising a short distance while fading
in. One shared ease-out curve drives both channels per word, so each word moves as a unit — and it
is the same curve every other reveal in this library uses, so a word here and a word in
Slide In Text land with identical weight.

## What it does

| frames (30 fps) | beat |
| --- | --- |
| `0` | Word 0 begins. `delay` shifts the whole sequence. |
| `0 … duration` | Word 0 rises `distance` px and fades 0 → 1 on `EASE_OUT`. |
| `+ stagger` per word | Each subsequent word starts `stagger` frames after the one before it. |
| after the last word lands | The line holds for `tail` frames, then the segment ends. |

The entrance starts `duration + (words - 1) * stagger` frames after the first word. Word `i`
starts exactly `i * stagger` frames after word 0, so the cascade is even regardless of word
length. Progress is clamped, so a word is fully at rest once its window closes and stays there.

Words wrap onto multiple lines when the run exceeds `max_width`; each line is centred on the
canvas and the whole block is centred vertically.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text.text` | `String` | `"Words appear one after another in sequence"` | The line to reveal. |
| `text.font_size` | `f32` | `64.0` | Font size in px. |
| `text.font_weight` | `u16` | `700` | Font weight. |
| `text.color` | `String` | `"#e6edf3"` | Text fill. |
| `text.max_width` | `f32` | `900.0` | Wrap width in px. |
| `text.line_height` | `f32` | `1.25` | Line height as a multiple of `font_size`. |
| `text.tracking_em` | `f32` | `-0.02` | Letter spacing in `em`. Negative tightens. |
| `duration` | `usize` | `30` | Frames each word takes to enter. |
| `stagger` | `usize` | `3` | Frames between consecutive word starts. |
| `delay` | `usize` | `0` | Frames to wait before the first word starts. |
| `distance` | `f32` | `20.0` | Pixels each word rises while fading in. |
| `tail` | `usize` | `72` | Frames held after the last word lands. |
| `reduced_motion` | `bool` | `false` | Collapse the travel to zero; keep the fades. |

Set `reduced_motion` to keep the word-by-word fade while removing all movement, which is the
accessible variant: the line still assembles in sequence, but nothing travels.

## Rendering

```sh
# just this component
cargo run --release -- render --components word-by-word render -o out.mp4

# as part of a selection (always rendered in catalog order)
cargo run --release -- render --components word-by-word,text-reveal,glitch-cycle render -o out.mp4

# a still frame, and the problems found in it
cargo run --release -- --components word-by-word frame 1s,50%,end

# one contact sheet of the motion
cargo run --release -- --components word-by-word strip -n 8

# the timeline this build assembles
cargo run --release -- --components word-by-word timeline
```

Replace `cargo run --release --` with `fframes-components` if the binary is on your `PATH`. Add
`--renderer cpu` to render on the CPU instead of the GPU for a single run.

## Duration

```
duration() = duration + (word_count - 1) * stagger + tail
```

`word_count` is the number of whitespace-separated words in `text`. With the defaults
(7 words, `duration` 30, `stagger` 3, `tail` 72):

```
30 + (7 - 1) * 3 + 72 = 120 frames = 4.00 s at 30 fps
```

An empty or whitespace-only `text` yields `tail` frames, not an underflow.

## Fidelity

Ported faithfully. The source animates inline `<span>`s with CSS transforms; this port measures
each word from the embedded font file and places it with an explicit SVG `<text>` at a measured
centre, which is why the layout is deterministic across machines.

The one visible difference is that the source renders the gap between words as a text node inside
the line, so the browser's own kerning applies across it. This port measures the space glyph and
places each word independently, so inter-word spacing comes from the font's space advance rather
than the renderer's layout. At the sizes these components are used that is sub-pixel.

## Credit

Ported from `WordByWord.tsx` in <https://github.com/Remocn/remocn> (MIT). See the crate
`CREDITS.md`.
