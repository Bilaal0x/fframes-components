# Squeeze In

`squeeze-in` — a line is wiped in from the left character by character, then closes up tight.

## What it does

The reveal is a clip, not a per-character fade: the run is masked by a rectangle whose right edge
sweeps the whole line over `chars * stagger` frames (30 for the default 10-character line), so each
character appears at its final opacity, already in place, as the edge passes it. The sweep is linear
in *width*, so the characters emerge in reading order at a rate set by their measured advances
rather than one per `stagger` frame; `stagger` sets the length of the window.

The squeeze is the second move. From `reveal_end` on, the gap each character carries in front of it
closes from `squeeze * font_size` (0.03 `em`, 3.24 px at 1080p) to 0 over `squeeze_frames` (16)
under `EASE_OUT_CUBIC`, while the whole run is shifted left by half of the gap it has closed so the
line stays centred as it tightens. Characters are measured with the face embedded in the crate at
their resting advances and re-spaced by the shrinking gap.

Beat by beat, with the defaults (10 characters, on the 1080p canvas where the reference px are drawn
at 1.5x):

| frames (30 fps) | beat |
| --- | --- |
| 0 | The wipe rectangle has zero width: nothing of the run is drawn. The gaps are at their widest, 3.24 px (0.03 em). |
| 15 | The wipe's right edge is halfway across the run's measured width. |
| 30 | `reveal_end`: the edge reaches the end of the run and the last character is revealed. The squeeze clock starts on this same frame, at 0. |
| 38 | `EASE_OUT_CUBIC` is 0.875 through the squeeze: the gaps are down to 0.41 px and the run has shifted 12.8 px left. |
| 46 | The gaps are 0 and the run has shifted half the closed gap, 14.58 px, left: the squeeze is done. |
| 46 – 90 | The 45-frame tail (1.50 s) holds the tightened line. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"SQUEEZE IN"` | The line to reveal. |
| `font_size` | `f32` | `72.0` | Reference-canvas font size in px. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `font_weight` | `u16` | `400` | Font weight. |
| `stagger` | `usize` | `3` | Frames per character of the reveal window (`chars * stagger` in total). |
| `squeeze` | `f32` | `0.03` | Gap every character closes by, in `em`, once revealed. |
| `squeeze_frames` | `usize` | `16` | Frames the squeeze takes. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `45` | Frames held after the squeeze finishes. |
| `tracking_em` | `f32` | `-0.02` | Letter spacing in `em`. Negative tightens. |

The source's props are `text`, `fontSize`, `color`, `fontWeight`, `stagger`, `squeeze` and `speed`;
its squeeze window is the module constant `SQUEEZE_FRAMES = 16`, its tracking is
`letterSpacing: "-0.02em"` and its line height is 1.1, both literals. `squeeze_frames`, `tail` and
`tracking_em` expose those here, and `tail` is this port's own.

`squeeze_in::reveal_end` and `squeeze_in::squeeze_channels` (with its `SqueezeChannels` return of
`gap`, `anchor_x` and `wipe_percent`) are public, so the timeline and the run's three channels can
be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components squeeze-in -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,squeeze-in -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components squeeze-in snapshot
cargo run --release -- --components squeeze-in --list
```

## Duration

```text
reveal_end = chars * stagger
duration() = ceil((reveal_end + squeeze_frames) / speed) + tail
```

`chars` is the Unicode-scalar count of `text` (`text.chars().count()`). With the defaults:
`30 + 16 = 46` frames of movement `+ 45` tail = **91 frames**, 3.03 s at 30 fps. `tail` is not
divided by `speed`, and an empty `text` is just the squeeze window: `16 + 45 = 61` frames.

The source's own `squeezeInLength(text, stagger)` is `chars * stagger + SQUEEZE_FRAMES`, the same
46-frame movement window; its demo composition is a flat 60 frames (`config.ts`), so its demo holds
the tightened line for 14 frames where this `duration()` adds a 45-frame tail.

## Fidelity notes

- **The wipe.** The source clips the run with `clip-path: inset(-0.2em R% -0.25em -0.1em)`, where
  `R` runs `100 → 0`. SVG's `clip-path` takes shapes rather than insets, so this port masks the run
  with a `<clipPath>` rectangle and animates its width, anchored to the canvas centre. The visible
  effect is the same left-to-right wipe; the source's overflow allowances above and below the line
  box do not exist here (the rect spans the full canvas height), so glyphs are never clipped
  vertically.
- **The clip follows the run.** The rect is anchored at the laid-out run's left edge and is as wide
  as the run plus its live gaps, so the wipe boundary *is* the run at every point of the squeeze —
  the source's `clip-path` follows the translated span in exactly the same way.
- **Measurement.** Each character is drawn as its own `<text>` node at its measured advance plus the
  live gap, with `tracking_em` applied per node, so adjacent characters are not kerned against each
  other and the line is centred from the measured advances. The baseline is placed as for a 1.1 line
  height, as the source's `lineHeight: 1.1` does.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 72` draws at 72 px on the reference canvas and 108 px on
  1080p, and `squeeze: 0.03` is a 3.24 px gap there, so the default line closes 29.2 px of gap.
- **Degenerate values.** A non-positive or non-finite `speed` falls back to `1`, a zero
  `squeeze_frames` is floored to one frame, and `squeeze` is not clamped, so a negative value opens
  the gaps as the timeline runs instead of closing them (the source's control limits it to
  `0..=0.2`).
- **Backdrop and text colour.** The source demo paints `#ffffff` behind `#171717` text and its demo
  line is `"Remocn"`; this port keeps the house dark style instead — `color` defaults to `#e6edf3`,
  the default line is `"SQUEEZE IN"`, and `background()` returns `#0d1117` in place of the source's
  white demo canvas.

## Credit

Ported from `squeeze-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
