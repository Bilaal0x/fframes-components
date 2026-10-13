# Top Down Letters

`top-down-letters` — letters drop into place from above on a wide staircase stagger.

## What it does

The mirror of [`bottom_up_letters`](bottom_up_letters.rs): character `i` starts at frame
`i * stagger` (the source's `staggerDelay`), 3 frames apart by default, and drops 46 reference px
instead of rising them. Opacity runs over `char_duration_frames` (12) under `EASE_LETTER_SOFT`
(`cubic-bezier(0.18, 1, 0.32, 1)`), a front-loaded curve, and the vertical travel is a ramp over the
shorter `char_travel_frames` (7). The only difference from the bottom-up component is the sign of
that travel; the timing, the curve and the 46 px are identical. Both channels are clamped before the
character's turn, so an unrevealed letter waits 46 reference px high in zero opacity.

Beat by beat, with the defaults (17 characters, on the 1080p canvas where the reference px are drawn
at 1.5x):

| frames (30 fps) | beat |
| --- | --- |
| 0 | "T" appears 69 px above its resting place at opacity 0.0. Every later letter waits there with it. |
| 1 | "T" is still 59 px high at opacity 0.40 — the letter-soft curve is almost all front. |
| 3 | "o" starts. "T" is 39 px high at opacity 0.81. |
| 6 | "p" starts. "T" is 10 px high at opacity 0.97. |
| 7 | "T" lands, opacity 0.98. |
| 12 | "T" is fully opaque. |
| 48 | The last character (".") starts, 16 staggers after the first. |
| 55 | The last character lands. |
| 60 | The last character is fully opaque: `duration()`. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Top down letters."` | The line to reveal. |
| `stagger` | `usize` | `3` | Frames between consecutive character starts. |
| `distance` | `f32` | `46.0` | Reference-canvas px each character drops from. Positive means above the resting place. |
| `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `font_weight` | `u16` | `600` | Font weight. |
| `char_duration_frames` | `usize` | `12` | Frames each character takes to fade in. |
| `char_travel_frames` | `usize` | `7` | Frames each character takes to finish dropping. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `0` | Frames held after the last character lands. |
| `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
| `reduced_motion` | `bool` | `false` | Collapse travel; keep the fades. |

The config is structurally identical to
[`BottomUpLettersConfig`](bottom_up_letters.rs) — the two source files are the same component with
the travel sign flipped — but it is its own type with its own default demo line, because `Default`
is what the catalog builds components from. The per-letter clocks (12, 7) and the
`letterSpacing: "-0.05em"` are hard-coded in the source; here they are options.

`crate::motion::entry_frames` and `crate::motion::letter_channels` are shared with this
component (`top_down_letters` calls them with direction `-1`).

## How to render it

```sh
# just this component
cargo run --release -- render --components top-down-letters -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,top-down-letters -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components top-down-letters snapshot
cargo run --release -- --components top-down-letters --list
```

## Duration

```text
entry_frames = chars == 0 ? 0 : ceil((char_duration_frames + (chars - 1) * stagger) / speed)
duration()   = entry_frames + tail
```

`chars` is the Unicode-scalar count of `text` (`text.chars().count()`). With the defaults
(17 characters): `12 + 16 * 3 = 60` entry frames `+ 0` tail = **60 frames**, 2.00 s at 30 fps.

The source's demo composition is a flat 60 frames (`config.ts`) around its 6-character `"Signal"`
demo — 27 frames of movement and a 33-frame hold; this `duration()` is the movement plus `tail`.

## Fidelity notes

- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, and `distance: 46` drops 69 px at 1080p.
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
  the line `"Signal"`; this port keeps the house dark style instead — `color` defaults to
  `#e6edf3`, the demo line is `"Top down letters."`, and `background()` returns `#0d1117` in place
  of the source's white demo canvas.

## Credit

Ported from `top-down-letters/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
