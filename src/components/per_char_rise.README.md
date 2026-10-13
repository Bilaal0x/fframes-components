# Per Character Rise

`per-character-rise` — every character of a single line rises into place from below, crisp and
without blur, on a one-frame stagger.

## What it does

Two clocks drive each glyph. Character `i` starts at frame `i * stagger`; its opacity runs over
`char_duration_frames` under `EASE_CHAR` (`cubic-bezier(0.2, 0.8, 0.2, 1)`) while its travel runs
over the shorter `char_travel_frames` under `EASE_CHAR_TRAVEL`
(`cubic-bezier(0.2, 0.8, 0.6, 0.85)`). Both channels are clamped at 0 before the glyph's turn, so an
unrevealed character waits at full travel and zero opacity. The two clocks are deliberately out of
step: the travel is over in 10 frames, the fade takes 21, so a character is at rest long before it
is fully opaque.

Beat by beat, with the defaults (15 characters, on the 1080p canvas where the reference px are drawn
at 1.5x):

| frames (30 fps) | beat |
| --- | --- |
| 0 | "O" appears 48 px below its resting place at opacity 0.0. Every later character waits there with it. |
| 1 – 14 | "n", "e", … "." start, one per frame. |
| 10 | "O" has covered its whole 48 px and is at rest, opacity 0.94 — its travel clock is 10 frames, its fade 21. |
| 21 | "O" reaches full opacity. |
| 24 | The last character (".") is in place. |
| 35 | The last character finishes its fade: `duration()`. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"One more thing."` | The line to reveal. |
| `distance` | `f32` | `32.0` | Reference-canvas px each character rises from. |
| `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `font_weight` | `u16` | `600` | Font weight. |
| `stagger` | `usize` | `1` | Frames between consecutive character starts. |
| `char_duration_frames` | `usize` | `21` | Frames each character takes to fade in. |
| `char_travel_frames` | `usize` | `10` | Frames each character takes to finish rising. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `0` | Frames held after the last character lands. |
| `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
| `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |

The source fixes the per-glyph timings (`charDurationFrames = 21`, `charTravelFrames = 10`,
`staggerFrames = 1`) inside the component and applies `letterSpacing: "-0.05em"` unconditionally;
this port exposes them all as options, with defaults that reproduce those constants.

`per_char_rise::entry_frames` and `per_char_rise::character_channels` (with its `RiseChannels`
return) are public, so the timeline can be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components per-character-rise -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,per-character-rise -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components per-character-rise snapshot
cargo run --release -- --components per-character-rise --list
```

## Duration

```text
entry_frames = chars == 0 ? 0 : ceil((char_duration_frames + (chars - 1) * stagger) / speed)
duration()   = entry_frames + tail
```

`chars` is the Unicode-scalar count of `text` (`text.chars().count()`). With the defaults
(15 characters): `21 + 14 * 1 = 35` entry frames `+ 0` tail = **35 frames**, 1.17 s at 30 fps.

The source's demo composition is a flat 60 frames (`config.ts`), so it holds the settled line for
the other 25; this `duration()` is the movement plus `tail`.

## Fidelity notes

- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, and `distance: 32` travels 48 px at 1080p.
- **Glyph positions.** The source leaves the characters in the inline flow of one `<span>`, where
  the browser applies `letter-spacing` between them; this port measures each character on its own
  through the face embedded in the crate (`ctx::char_centers`) and draws it as its own `<text>` node
  at that measured centre. Kerning between adjacent characters is therefore not reproduced, and
  because those measured advances do not include `tracking_em`, the line is laid out on untracked
  advances and its glyphs sit wider than the browser's tracked line would put them (the tracking
  only enters as the per-node anchor correction in `ctx::centred_text`).
- **Degenerate values.** A non-positive or non-finite `speed` falls back to `1` instead of freezing
  or reversing the animation, a zero `char_duration_frames` / `char_travel_frames` is floored to one
  frame, and an empty `text` yields `duration() = tail`.
- **Backdrop and text colour.** The source demo paints `#ffffff` behind `#171717` text; this port
  keeps the house dark style instead — `color` defaults to `#e6edf3`, and the component paints no
  backdrop of its own and `background()` returns `#0d1117`.

## Credit

Ported from `per-character-rise/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
