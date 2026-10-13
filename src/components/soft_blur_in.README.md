# Soft Blur In

`soft-blur-in` — characters fade up from soft focus one at a time, each resolving as it lands.

## What it does

Character `i` starts at frame `i * stagger`. One `EASE_MATERIAL_OUT` clock
(`cubic-bezier(0.22, 1, 0.36, 1)`) drives opacity and softness over `char_duration_frames` (27):
opacity rises 0 → 1 while the softness radius falls from `blur_from` to 0. The vertical travel is
its own ramp over the shorter `char_travel_frames` (9), so a glyph is already in place while it is
still soft and still faint. Everything is clamped before the character's turn, so an unrevealed
glyph waits at full softness and full travel in zero opacity.

Beat by beat, with the defaults (19 characters, on the 1080p canvas where the reference px are drawn
at 1.5x):

| frames (30 fps) | beat |
| --- | --- |
| 0 | "S" appears 24 px below its resting place, soft over an 18 px radius, opacity 0.0. Every later character waits there in the same state. |
| 1 – 18 | "o", "f", … "." start, one per frame. |
| 9 | "S" has covered its 24 px and is at rest, opacity 0.87 and still 2.4 px soft — the travel clock is 9 frames, the fade 27. |
| 27 | "S" reaches full opacity and 0 px of softness: sharp. The last character (".") has also finished its travel by now. |
| 45 | The last character is opaque and sharp: `duration()`. |

The softness is drawn, not filtered: while the radius exceeds 0.05 px each glyph is drawn once
sharply and twice more as low-opacity copies offset along the vertical travel axis, one at
`+0.25 * blur` and one at `-0.5 * blur`.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Softly, then sharp."` | The line to reveal. |
| `blur_from` | `f32` | `12.0` | Starting softness radius in reference-canvas px; eases to 0. |
| `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `font_weight` | `u16` | `600` | Font weight. |
| `stagger` | `usize` | `1` | Frames between consecutive character starts. |
| `char_duration_frames` | `usize` | `27` | Frames each character takes to fade in and sharpen. |
| `char_travel_frames` | `usize` | `9` | Frames each character takes to finish rising. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `0` | Frames held after the last character lands. |
| `tracking_em` | `f32` | `-0.05` | Letter spacing in `em`. Negative tightens. |
| `reduced_motion` | `bool` | `false` | Collapse travel and softness; keep the fades. |
| `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for a real gaussian blur. |

The source's `blur` prop is this port's `blur_from`. The per-character clocks (27, 9) and the 16 px
of travel are constants in the source; here the clocks are options and the 16 reference px of travel
is fixed, as it is there.

`soft_blur_in::entry_frames`, `soft_blur_in::character_channels` (returning `SoftBlurChannels`) and
`crate::blur::blur_ghost_alpha` are public, so the timeline and the stand-in's opacity can be
sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components soft-blur-in -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,soft-blur-in -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components soft-blur-in snapshot
cargo run --release -- --components soft-blur-in --list
```

## Duration

```text
entry_frames = chars == 0 ? 0 : ceil((char_duration_frames + (chars - 1) * stagger) / speed)
duration()   = entry_frames + tail
```

`chars` is the Unicode-scalar count of `text` (`text.chars().count()`). With the defaults
(19 characters): `27 + 18 * 1 = 45` entry frames `+ 0` tail = **45 frames**, 1.50 s at 30 fps.

The source's demo composition is a flat 60 frames (`config.ts`), so it holds the settled line for
the other 15; this `duration()` is the movement plus `tail`.

## Fidelity notes

- **Blur is approximated, not rendered.** The source applies CSS `filter: blur(Npx)`, an isotropic
  gaussian over the glyph. SVG has no gaussian path in this renderer, so `simulate_blur` (default
  `true`) stands in with `BLUR_GHOST_COPIES` = 2 extra copies of the glyph, offset along the
  vertical travel axis by `BLUR_GHOST_SPREAD_RATIO` (0.5) of the radius — one at `+0.25 * blur`,
  one at `-0.5 * blur` — each drawn behind the sharp copy at
  `min(1, blur / (BLUR_GHOST_REFERENCE_PX * unit)) * BLUR_GHOST_MAX_ALPHA (0.45) / 2` times the
  glyph's own opacity. That reads as softness and resolves cleanly as the radius reaches 0, but it
  is a two-copy stand-in, not an isotropic gaussian; nothing is drawn once the radius drops below
  `BLUR_GHOST_MIN_PX` (0.05). Set `simulate_blur: false` to drop it and keep only the travel and the
  fade.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, and `blur_from: 12` is an 18 px radius at 1080p. The stand-in's reference radius follows
  the same factor, so the ghost budget is spent exactly when a glyph starts at full softness.
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
- **Backdrop and text colour.** The source demo paints `#ffffff` behind `#171717` text and its demo
  line is `"Think different."`; this port keeps the house dark style instead — `color` defaults to
  `#e6edf3`, the demo line is `"Softly, then sharp."`, and `background()` returns `#0d1117` in place
  of the source's white demo canvas.

## Credit

Ported from `soft-blur-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
