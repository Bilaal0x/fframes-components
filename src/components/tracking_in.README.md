# Tracking In

`tracking-in` — a single line closes up its letter spacing while sharpening out of a 12 px blur.

## What it does

One spring drives everything spatial. `t` is the spring's raw value, running 0 → 1 as it settles, and
the letter spacing is a map of it from `start_tracking` (0.5 `em`, wide open) to `end_tracking`
(-0.03 `em`, tightened) while the softness falls from `start_blur` to 0 on the same clock. Opacity is
a separate, shorter linear ramp over `opacity_frames` (15), so the line is fully visible while the
letters are still closing and the line is still soft. The spring is underdamped — `damping` 18,
`stiffness` 90 and mass 1 — so it passes its target, peaks and comes back; the range maps are
unclamped (`map_range`), so the tracking closes a hair past its resting value and the softness passes
0 before both relax. That excursion is the source's read and is deliberately preserved.

The line is drawn as one centred `<text>` node, as in the source's `whiteSpace: "nowrap"` span, plus
the stand-in copies described under Fidelity notes.

Beat by beat, with the defaults (30 fps; softness in canvas px on the 1080p canvas, reference px in
brackets):

| frames (30 fps) | beat |
| --- | --- |
| 0 | The line appears at 0.5 `em` tracking, soft over an 18 px (12) radius, opacity 0. |
| 5 | spring 0.483: tracking 0.244 `em`, softness 9.3 px (6.2), opacity 0.33. |
| 10 | spring 0.847: tracking 0.051 `em`, softness 2.7 px (1.8), opacity 0.67. |
| 15 | The opacity ramp completes. spring 0.966: tracking -0.012 `em`, softness 0.6 px (0.4) — fully visible, still closing and still soft. |
| 20 | spring 0.994: tracking -0.027 `em`, softness 0.1 px. |
| 28 | The port's settle frame: the first frame the spring is within `1e-4` of 1.0. Tracking -0.030 `em`, softness 0. |
| 31 | The overshoot peaks at spring 1.00008: tracking -0.03004 `em`, a hair tighter than its resting value, with the softness range passing 0. |
| 28 – 72 | The settled line holds for the 45-frame tail (1.50 s). |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"TRACKING IN"` | The line to settle. |
| `start_tracking` | `f32` | `0.5` | Starting letter spacing in `em`. |
| `start_blur` | `f32` | `12.0` | Starting softness radius in reference-canvas px. |
| `end_tracking` | `f32` | `-0.03` | Resting letter spacing in `em`. |
| `font_size` | `f32` | `96.0` | Reference-canvas font size in px. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `font_weight` | `u16` | `700` | Font weight. |
| `damping` | `f32` | `18.0` | Spring damping. |
| `stiffness` | `f32` | `90.0` | Spring stiffness. |
| `opacity_frames` | `usize` | `15` | Frames the opacity ramp takes. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `45` | Frames held after the line settles. |
| `simulate_blur` | `bool` | `true` | Draw the secondary copies that stand in for a real gaussian blur. |

`text`, `startTracking`, `startBlur`, `fontSize`, `color`, `fontWeight` and `speed` are the source's
props. Its spring (`damping` 18, `stiffness` 90, Remotion's default mass 1), its `-0.03em` resting
tracking and its 15-frame opacity ramp are literals in the source; `damping`, `stiffness`,
`end_tracking` and `opacity_frames` expose them here, and `tail` and `simulate_blur` are this port's
own. Mass stays fixed at 1, as in the source.

`tracking_in::spring_settle_frames` and `tracking_in::tracking_channels` are public, and
`TrackingInConfig::spring()` returns the `Spring` they solve, so the timeline and the three channels
can be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components tracking-in -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,tracking-in -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components tracking-in snapshot
cargo run --release -- --components tracking-in --list
```

## Duration

```text
spring_frames = first frame where |Spring::value(frame) - 1.0| <= 1e-4
duration()    = ceil(spring_frames / speed) + tail
```

`spring_settle_frames(config.spring(), 1e-4)` scans from frame 0 for that first frame; for the
default spring it is **frame 28** (frame 27 sits `1.1e-4` away from 1.0, frame 28 `1.4e-5` away),
giving `28 + 45 = 73` frames — **2.43 s at 30 fps**. The spring is solved analytically at `FPS` = 30
(`../motion.rs`), where the source reads `fps` from the composition; being underdamped it keeps
oscillating past that frame, but the remaining excursion is below `1e-4` of the spring's range.

The source's demo composition is a flat 90 frames (`config.ts`), so it holds the settled line for the
other 62; this `duration()` is the movement plus `tail`.

## Fidelity notes

- **Blur is approximated, not rendered.** The source applies `filter: blur(Npx)`, an isotropic
  gaussian over the whole line; SVG has no gaussian path in this renderer, so `simulate_blur` (default
  `true`) stands in with the constants borrowed from [`soft_blur_in`](soft_blur_in.rs):
  `BLUR_GHOST_COPIES` = 2 extra copies of the line, offset from the baseline by
  `BLUR_GHOST_SPREAD_RATIO` (0.5) of the radius — one at `+0.25 * blur`, one at `-0.5 * blur`, i.e.
  `+4.5 px` and `-9 px` at the default 18 px radius — each drawn behind the sharp copy at
  `min(1, blur / (BLUR_GHOST_REFERENCE_PX * unit)) * BLUR_GHOST_MAX_ALPHA (0.45) / 2` times the
  line's own opacity (0.225 each at full softness). That reads as softness and resolves cleanly as
  the radius reaches 0, but it is a two-copy stand-in, not an isotropic gaussian; nothing is drawn
  once the radius drops below `BLUR_GHOST_MIN_PX` (0.05). Set `simulate_blur: false` to drop it and
  keep only the tracking and the fade.
- **The overshoot is preserved.** Remotion's `interpolate` clamps its input but not the spring's
  value, so the source's line passes its resting tracking and comes back; this port maps the spring
  with the unclamped `map_range` for the same reason (`interpolate` would have flattened the
  excursion). The peak is spring 1.00008 on frame 31, i.e. -0.03004 `em` of tracking against a
  -0.03 `em` rest — about 0.006 px of extra travel at the default size — and the softness range
  crosses 0 with it.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 96` draws at 96 px on the reference canvas and 144 px on
  1080p, and `start_blur: 12` is an 18 px radius at 1080p. The `em` tracking values scale with the
  font, so they need no separate factor.
- **Degenerate values.** A non-positive or non-finite `speed` falls back to `1` instead of freezing
  or reversing the animation, and a zero `opacity_frames` is floored to one frame. The settle scan
  covers ten seconds of frames, so a spring that has not settled by then reports frame 299 rather
  than running away; a non-positive `stiffness` is instantly at rest and settles on frame 1.
- **Backdrop and text colour.** The source demo paints `#ffffff` behind `#171717` text and its demo
  line is `"tracking in"`; this port keeps the house dark style instead — `color` defaults to
  `#e6edf3`, the default line is `"TRACKING IN"`, and the component paints no backdrop of its own
  and `background()` returns `#0d1117`.

## Credit

Ported from `tracking-in/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
