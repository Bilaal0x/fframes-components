# Spring Scale In

`spring-scale-in` — words pop in one after another and overshoot slightly before settling.

## What it does

Word `i` starts at frame `i * stagger`. Opacity and scale share one `EASE_BACK_OUT` clock
(`cubic-bezier(0.34, 1.56, 0.64, 1)`) over `word_frames` (11): opacity rises 0 → 1 and the scale
runs `scale_from` (0.7) → 1.0. The back-out curve's first control point sits above 1, so both
channels pass their target before settling — a word grows to about 1.03 and comes back. That
overshoot, plus a 3-frame stagger, is what makes the line read as a quick ripple rather than one
pop.

Beat by beat, with the defaults (3 words, on the 1080p canvas where the reference px are drawn at
1.5x, so 72 is drawn at 108 px):

| frames (30 fps) | beat |
| --- | --- |
| 0 | "Fast." appears at 0.7 scale (76 px) and opacity 0.0. |
| 3 | "Crisp." starts. "Fast." is at 0.958 (103 px), opacity 0.86. |
| 6 | "Fluid." starts. "Fast." peaks at 1.029 (111 px) — the overshoot — and is already fully opaque. |
| 11 | "Fast." settles at scale 1.000 (108 px). |
| 14 | "Crisp." settles. |
| 17 | "Fluid." settles: the entrance ends. |
| 18 – 57 | The finished line holds for the 40-frame tail. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text.text` | `String` | `"Fast. Crisp. Fluid."` | The line to reveal. |
| `text.font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `text.font_weight` | `u16` | `600` | Font weight. |
| `text.color` | `String` | `"#e6edf3"` | Text fill. |
| `text.max_width` | `f32` | `960.0` | Wrap width in reference-canvas px. |
| `text.line_height` | `f32` | `1.2` | Line height as a multiple of `font_size`. |
| `text.tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
| `stagger` | `usize` | `3` | Frames between consecutive word starts. |
| `scale_from` | `f32` | `0.7` | Scale each word starts at. |
| `word_frames` | `usize` | `11` | Frames each word takes to land. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `40` | Frames held after the last word lands. |
| `reduced_motion` | `bool` | `false` | Collapse the scale; keep the fades. |

`text` is the shared [`TextSpec`](../text_spec.rs) block; the values above are the ones this
component sets, with `text.color` coming from `TextSpec::default()`. The source's `staggerDelay` is
this port's `stagger`, and its hard-coded 11-frame word duration and `letterSpacing: "-0.03em"` are
the `word_frames` and `tracking_em` options here.

`spring_scale_in::entry_frames` and `spring_scale_in::word_channels` are public, so the timeline and
one word's opacity / scale can be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components spring-scale-in -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,spring-scale-in -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components spring-scale-in snapshot
cargo run --release -- --components spring-scale-in --list
```

## Duration

```text
entry_frames = words == 0 ? 0 : ceil((word_frames + (words - 1) * stagger) / speed)
duration()   = entry_frames + tail
```

`words` is the whitespace-separated word count of `text` (`TextSpec::word_count`). With the
defaults (3 words): `11 + 2 * 3 = 17` entry frames `+ 40` tail = **57 frames**, 1.90 s at 30 fps.

The source's demo composition is a flat 60 frames (`config.ts`) for the same 3-word line: 17 frames
of movement and a 43-frame hold. This `duration()` is the movement plus `tail`, which is 57 with the
default 40.

## Fidelity notes

- **The overshoot is preserved.** Remotion's `interpolate` clamps its input but not the eased
  output, so the back-out excursion survives in the source; this port maps the eased value with the
  unclamped `map_range` for the same reason. `opacity` is that eased value, so it passes 1 in
  mid-flight and SVG clamps it; what visibly overshoots is the scale.
- **The scale is a font size.** The source scales each `inline-block` word about
  `transform-origin: 50% 50%`; this port lays the words out at their resting size and draws each one
  at `font_size * scale` centred on its own measured centre, which is the same gesture in SVG. The
  word's centre — and therefore the line's spacing — does not move as it scales.
- **Layout and wrapping.** The source renders every word in one inline flow and replaces the literal
  spaces with `margin-right: 0.25em`; this port measures each word from the face embedded in the
  crate, wraps greedily at `text.max_width`, centres each line and centres the block vertically.
  Inter-word gaps come from the font's own space advance rather than that margin, and the wrap point
  can differ from the browser's by a word. Words are split on any run of whitespace, where the
  source splits on the literal space character, so text containing tabs or newlines yields more
  words here.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, and `max_width: 960` wraps at 1440 px at 1080p.
- **`reduced_motion`** collapses the scale channel to its resting value (words fade in at full size,
  no overshoot) and leaves the fades and the stagger alone.
- **Degenerate values.** A non-positive or non-finite `speed` falls back to `1` instead of freezing
  or reversing the animation, a zero `word_frames` is floored to one frame, and an empty `text`
  yields `duration() = tail`.
- **Backdrop and colour.** The source demo paints `#ffffff` behind `#171717` text; this port keeps
  the house dark style instead — `color` defaults to `#e6edf3`, and the component paints no backdrop
  of its own and `background()` returns `#0d1117`.

## Credit

Ported from `spring-scale-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
