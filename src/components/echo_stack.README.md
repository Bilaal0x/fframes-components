# Echo Stack

`echo-stack` — a word resolves out of a blur inside a drifting stack of outlined echoes.

## What it does

Three clocks run at once:

| clock | window | what it does |
| --- | --- | --- |
| main | frames 0→14, `EASE_OUT_CUBIC` | the filled word blurs from 12 reference px to 0, fades in, scales `1.04 → 1` |
| unfold | frames 8→26, `EASE_OUT_CUBIC` | the echo spacing opens from 0.6 of a pitch to a full `ECHO_STACK_PITCH` (1.05 `em`) |
| fade in | frames 8→24, `EASE_OUT_CUBIC` | the echoes reach their full `echo_opacity` |

The rows are outlined copies of the word laid out one `ECHO_STACK_PITCH` apart, and the row closest
to the word is the one the falloff hides: nothing within `HIDDEN` (0.55 pitches) is drawn at all,
because it would sit behind the filled word. From there the opacity ramps up to one pitch and decays
out to `echoes + 0.5` pitches, so a three-echo stack draws rows from about 0.55 to 3.5 pitches away
in each direction.

After the unfold the stack keeps moving: the phase advances by one pitch every `cycle` frames —
upwards by default, downwards with `direction: Down` — and rows recycle as they pass the centre, so
the drift never ends. The word's own scale stays at 1 after frame 14, so the drift is the whole
animation from there on; `duration()` simply cuts it off at the tail.

Beat by beat, with the defaults (30 fps; canvas px on 1080p, reference px in brackets; one pitch is
267.75 px, 178.5 reference px, at the default 255 px font):

| frames (30 fps) | beat |
| --- | --- |
| 0 | The word is invisible and soft over 18 px (12) at scale 1.04. No echo row is drawn: the fade-in has not started. |
| 8 | The unfold and the echo fade-in begin; the word is at opacity 0.88 and 2.25 px (1.5) soft. |
| 14 | The word has resolved: opacity 1, sharp, scale 1. |
| 17 | Mid-unfold the rows sit 0.95 of their final spacing from the word; the nearest drawn rows are 0.72 and 1.28 pitches out at 0.10 and 0.28 opacity. |
| 24 | The echoes reach their peak `echo_opacity` (0.35). |
| 26 | The stack is at full pitch — every row sits `k ± phase` pitches from the word — and the entrance is over. |
| 26 – 70 | The drift: one pitch every 60 frames upward, so the phase goes from −0.43 to −0.68 between frames 26 and 41; rows recycle forever. `duration()` cuts the loop off at frame 71. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Bold."` | The word to stack. |
| `font_size` | `f32` | `170.0` | Reference-canvas font size in px. |
| `font_weight` | `u16` | `800` | Font weight. |
| `color` | `String` | `"#e6edf3"` | The word's fill. |
| `echo_color` | `String` | `""` | Echo outline colour. Empty means `color`. |
| `echoes` | `usize` | `3` | How many echo rows reach out from the centre. |
| `echo_opacity` | `f32` | `0.35` | Peak opacity of an echo row. |
| `stroke_width` | `f32` | `1.5` | Echo outline thickness in reference-canvas px. |
| `direction` | `EchoDirection` | `EchoDirection::Up` | Which way the stack drifts. |
| `cycle` | `f32` | `60.0` | Frames per pitch of drift. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `45` | Frames held before the stack's own loop is cut off. |
| `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for the main word's gaussian blur. |

Every one of the source's props is here — `text`, `fontSize`, `fontWeight`, `color`, `echoColor`,
`echoes`, `echoOpacity`, `strokeWidth`, `direction`, `cycle` and `speed` — with the same defaults;
`tail` and `simulate_blur` are this port's own (the module's own option table omits
`simulate_blur`, but the field exists and defaults to `true`). The source's `fontFamily` prop is
not exposed: this library draws with the face embedded in the crate, as every other component here
does.

`echo_stack::ECHO_STACK_PITCH`, `HIDDEN`, `MAIN`, `UNFOLD`, `FADE_IN`, `START_SPREAD`,
`MAX_BLUR`, `EchoDirection::sign`, `echo_stack::normalised_cycle`, `echo_stack::main_channels`
(with its `MainChannels` return), `echo_stack::echo_fade` and `echo_stack::echo_rows` (with its
`EchoRow` rows) are public, so the stack can be sampled a row at a time without building the
component.

## How to render it

```sh
# just this component
cargo run --release -- render --components echo-stack -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,echo-stack -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components echo-stack snapshot
cargo run --release -- --components echo-stack --list
```

## Duration

```text
duration() = ceil(max(UNFOLD[1], FADE_IN[1], MAIN[1]) / speed) + tail
           = ceil(max(26, 24, 14) / 1) + 45
```

**71 frames**, 2.37 s at 30 fps: 26 frames of entrance plus the 45-frame tail. `tail` is not
divided by `speed`, and `echoes`, `cycle` and `direction` do not affect the length at all — the
drift is endless by design and the tail is what ends it. The source documents its own movement
length as `echoStackLength = 26`, the same frame, and its demo composition is a flat 120 frames
(`config.ts`).

## Fidelity notes

- **The outline is stroked, not filtered.** The source builds each echo from an SVG filter —
  `feMorphology` eroding `SourceAlpha`, `feComposite` subtracting it, and `feFlood` recolouring the
  ring — so that the variable font's overlapping contours cannot show through a plain text stroke.
  This renderer does not execute SVG filters, so the port strokes the echo text directly at
  `stroke_width`. Two differences follow: the source's ring is eroded *inwards* from the silhouette,
  where a stroke straddles the glyph outline (half of it outside the glyph edge), so the port's
  echoes are marginally larger and their ring reads thinner against the same silhouette; and where
  the source font has overlapping contours the port's stroke may show a seam that the source's
  filter hides.
- **Blur is approximated, not rendered.** The main word's `filter: blur(12px → 0)` is stood in for
  by `simulate_blur` (default `true`): `BLUR_GHOST_COPIES` = 2 extra copies of the word, offset from
  its baseline by `BLUR_GHOST_SPREAD_RATIO` (0.5) of the radius — one at `+0.25 * blur`, one at
  `-0.5 * blur`, i.e. `+4.5 px` and `-9 px` at the default 18 px starting radius — each drawn behind
  the sharp word at
  `min(1, blur / (BLUR_GHOST_REFERENCE_PX (12) * unit)) * BLUR_GHOST_MAX_ALPHA (0.45) / 2` times the
  word's own opacity; nothing is drawn below `BLUR_GHOST_MIN_PX` (0.05). The echoes themselves are
  never blurred, as in the source.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080 (the source
  scales by `height / 720` instead, which agrees on a 16:9 landscape canvas and follows the long
  side of a portrait one). Option values are reference-canvas px: `font_size: 170` draws at 170 px on
  the reference canvas and 255 px on 1080p, so one pitch is 267.75 px there and `stroke_width: 1.5`
  is a 2.25 px stroke.
- **Degenerate values.** A non-finite or non-positive `cycle` falls back to `60` (the source clamps
  it to at least `1`), `echoes: 0` draws no rows at all (the falloff's reach collapses to one pitch
  and every distance past it fades to 0), a non-positive `echo_opacity` skips every row, and a
  non-positive or non-finite `speed` falls back to `1`.
- **Backdrop and text colour.** The source's demo backdrop is `#0a0a0a` behind `#fafafa` text, and
  `echo_color` is empty by default, meaning the echoes take the word's colour; this port keeps that
  rule but changes the house colours — `color` defaults to `#e6edf3`, and `background()` returns
  `#0d1117` in place of the source's near-black demo canvas.

## Credit

Ported from `echo-stack/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
