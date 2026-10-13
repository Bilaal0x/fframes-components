# Bottom Up Letters

`bottom-up-letters` — letters slide up into place from below on a wide staircase stagger.

## What it does

Character `i` starts at frame `i * stagger` (the source's `staggerDelay`) — 3 frames apart by
default, three times the one-frame stagger of `per-character-rise` — and travels 46 reference px
against that component's 32, which is what turns the same idea into a visible staircase. Opacity
runs over `char_duration_frames` (12) under `EASE_LETTER_SOFT`
(`cubic-bezier(0.18, 1, 0.32, 1)`), a front-loaded curve: a letter is already 0.94 opaque five
frames in. The vertical travel is a ramp over the shorter `char_travel_frames` (7). Both channels
are clamped before the character's turn, so an unrevealed letter waits 46 reference px low in zero
opacity.

Beat by beat, with the defaults (18 characters, on the 1080p canvas where the reference px are drawn
at 1.5x):

| frames (30 fps) | beat |
| --- | --- |
| 0 | "B" appears 69 px below its resting place at opacity 0.0. Every later letter waits there with it. |
| 1 | "B" is 59 px low at opacity 0.40 — the letter-soft curve is almost all front. |
| 3 | "o" starts. "B" is 39 px low at opacity 0.81. |
| 6 | "t" starts. "B" is 10 px low at opacity 0.97. |
| 7 | "B" lands, opacity 0.98. |
| 12 | "B" is fully opaque. |
| 51 | The last character (".") starts, 17 staggers after the first. |
| 58 | The last character lands. |
| 63 | The last character is fully opaque: `duration()`. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Bottom up letters."` | The line to reveal. |
| `stagger` | `usize` | `3` | Frames between consecutive character starts. |
| `distance` | `f32` | `46.0` | Reference-canvas px each character rises from. Positive rises from below. |
| `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `font_weight` | `u16` | `600` | Font weight. |
| `char_duration_frames` | `usize` | `12` | Frames each character takes to fade in. |
| `char_travel_frames` | `usize` | `7` | Frames each character takes to finish rising. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `0` | Frames held after the last character lands. |
| `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
| `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |

The source's `staggerDelay` is this port's `stagger`. The per-letter clocks (12, 7) and the
`letterSpacing: "-0.05em"` are hard-coded in the source; here the clocks and the tracking are
options, with defaults that reproduce the source.

`crate::motion::entry_frames` is public, and `crate::motion::letter_channels` /
`config_channels` sample one character's opacity and offset —
[`top_down_letters`](top_down_letters.rs) shares them with the travel sign flipped.

## How to render it

```sh
# just this component
cargo run --release -- render --components bottom-up-letters -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,bottom-up-letters -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components bottom-up-letters snapshot
cargo run --release -- --components bottom-up-letters --list
```

## Duration

```text
entry_frames = chars == 0 ? 0 : ceil((char_duration_frames + (chars - 1) * stagger) / speed)
duration()   = entry_frames + tail
```

`chars` is the Unicode-scalar count of `text` (`text.chars().count()`). With the defaults
(18 characters): `12 + 17 * 3 = 63` entry frames `+ 0` tail = **63 frames**, 2.10 s at 30 fps —
the longest of the four letter-by-letter reveals in this family, against 35 for
`per-character-rise`, 45 for `soft-blur-in` and 60 for `top-down-letters`.

The source's demo composition is a flat 60 frames (`config.ts`) around its 5-character `"Shift"`
demo — 24 frames of movement and a 36-frame hold; this `duration()` is the movement plus `tail`, so
the 18-character default line is longer than that composition.

## Fidelity notes

- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, and `distance: 46` travels 69 px at 1080p.
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
- **Backdrop, colour and demo line.** The source demo paints `#ffffff` behind `#171717` text over
  the line `"Shift"`; this port keeps the house dark style instead — `color` defaults to `#e6edf3`,
  the demo line is `"Bottom up letters."`, and `background()` returns `#0d1117` in place of the
  source's white demo canvas.

## Credit

Ported from `bottom-up-letters/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
