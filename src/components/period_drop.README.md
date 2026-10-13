# Period Drop

`period-drop` — a word rises letter by letter, then a coloured dot falls onto it and bounces twice.

## What it does

Two moves share the beat. First the letters: every character climbs out of its own clipping box
from 115% of the font size below its line, each starting `rise_stagger` (2) frames after the one
before it and taking `rise_frames` (12) under `EASE_OUT_QUART`, so the word is fully up on frame
`rise_stagger * (chars - 1) + rise_frames` (22). Then the dot: it is released at `drop_start` (20)
at `-drop_height` of the canvas height, falls for `fall_frames` (12) and lands on the baseline.

The physics is the source's own, not an eased imitation. Gravity is set by the fall time
(`g = 2 / fall²`, so the landing speed is exactly `2 / fall`), each bounce leaves the floor at
`restitution` (0.35) times the speed it arrived with, and the deformation is the impact squash —
decaying as `e^(-τ / recover)`, with `recover` 1.1 frames — plus the in-flight stretch, which tracks
the instantaneous speed. `dot_scale` returns `scale_x` as the reciprocal of `scale_y`, so the dot
preserves its area, and a last wobble below `0.005` is snapped to zero so it rests exactly. The dot
is anchored at its bottom on the baseline, which is the point it squashes about, and sits a small gap
after the run.

Beat by beat, with the defaults (30 fps, `"remocn"`, on the 1080p canvas, where `unit` is 1.5 and
the font draws at 240 px; the drop is 669.6 px):

| frames (30 fps) | beat |
| --- | --- |
| 0 | Every letter sits 276 px (115% of 240 px) below its line, outside its clip box: nothing is visible. The dot is not drawn. |
| 10 | The last letter ("n", index 5) starts to climb at `rise_stagger * 5`. |
| 20 | `drop_start`: the dot appears at the top of its fall, 0.62 of the canvas height above the line. |
| 22 | The last letter lands: the rise is over. |
| 31.75 | A quarter of a frame before contact the dot is at full falling speed, stretched to `scale_y` 1.29. |
| 32 | The first impact, at `drop_start + fall_frames`. The squash takes over on the contact frame at full depth: `scale_y` 0.78, `scale_x` 1.28. |
| 36.2 | The apex of the first hop: 0.1225 of the drop, 82 px above the line. |
| 40.4 | The second impact, at 35% of the landing speed and a shallower squash (`scale_y` 0.92). |
| 41.87 | The apex of the second hop: 0.015 of the drop, 10 px above the line. |
| 43.34 | The third and last impact (`scale_y` 0.97). |
| 47 | The squash has decayed below the 0.005 snap threshold and the dot rests exactly — the source's own `periodDropLength`. |

`motion_frames` is 44 — the ceiling of the last impact — so the component's movement window closes
three frames before frame 47; the 45-frame tail covers the difference.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"remocn"` | The word the dot follows. |
| `font_size` | `f32` | `160.0` | Reference-canvas font size in px. |
| `font_weight` | `u16` | `800` | Font weight. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `dot_color` | `String` | `"#d97757"` | The dot's fill. |
| `dot_size` | `f32` | `0.2` | The dot's diameter in `em`. |
| `rise_frames` | `usize` | `12` | Frames each letter takes to climb. |
| `rise_stagger` | `usize` | `2` | Frames between consecutive letter starts. |
| `drop_start` | `f32` | `20.0` | Frame the dot is released on. |
| `fall_frames` | `f32` | `12.0` | Frames the dot takes to fall. |
| `restitution` | `f32` | `0.35` | Speed kept at each bounce. |
| `bounces` | `usize` | `2` | How many bounces the dot takes. |
| `drop_height` | `f32` | `0.62` | Drop height as a share of the canvas height. |
| `stretch` | `f32` | `0.3` | `scale_y` gained at full speed. |
| `squash` | `f32` | `0.22` | `scale_y` lost at a full-speed impact. |
| `recover` | `f32` | `1.1` | Frames for an impact squash to decay by 1/e. |
| `speed` | `f32` | `1.0` | Scales the timeline; `> 1` is faster. |
| `tail` | `usize` | `45` | Frames held after the dot comes to rest. |

The source's props are `text`, `fontSize`, `fontWeight`, `color`, `dotColor`, `dotSize` and `speed`;
every timing constant — `riseFrames` 12, `riseStagger` 2, `dropStart` 20, `fallFrames` 12,
`restitution` 0.35, `bounces` 2, `dropHeight` 0.62, `stretch` 0.3, `squash` 0.22 and `recover` 1.1 —
lives in one `periodDropTiming` table there and is exposed here as options, unchanged. The source's
`fontFamily` prop is not exposed: this library draws with the face embedded in the crate. `tail` is
this port's own.

`period_drop::rise_offset`, `period_drop::impacts`, `period_drop::dot_scale`,
`period_drop::dot_state` (with its `DotState` return of `visible`, `y`, `scale_x` and `scale_y`) and
`period_drop::motion_frames` are public, so the timeline and the dot can be sampled without building
the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components period-drop -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,period-drop -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components period-drop snapshot
cargo run --release -- --components period-drop --list
```

## Duration

```text
impacts[0]  = drop_start + fall_frames                                       (= 32)
impacts[n]  = impacts[n-1] + 2 * restitution^n * fall_frames                 (= 40.4, 43.34)
rise_end    = rise_stagger * (chars - 1) + rise_frames                       (= 22)
motion      = ceil(max(impacts.last(), rise_end) / speed)                    (= 44)
duration()  = motion + tail
```

With the defaults: the last impact at 43.34 beats the rise's 22, so `motion_frames` is 44, and
`44 + 45` = **89 frames**, 2.97 s at 30 fps. `tail` is not divided by `speed`. `bounces` adds one
`2 * restitution^n * fall_frames` hop each, so every extra bounce shortens geometrically; `text` and
the rise constants only matter while `rise_end` is the larger of the two.

The source documents its movement length as `periodDropLength = 47` — the frame the dot has finished
settling — and its demo composition is a flat 75 frames (`config.ts`); this port's `motion_frames`
stops at the last impact and leaves the settle to the tail.

## Fidelity notes

- **The letter clip.** The source wraps every letter in an `overflow: hidden` box with a
  `padding-bottom` / negative `margin-bottom` pair (`0.15em`) so descenders survive. SVG clips to a
  rectangle, so the port clips each letter to its own line box extended by the descender allowance —
  from `baseline - ascent - 0.2 * font_size` to `baseline + descent + 0.2 * font_size`, with
  `ascent` and `descent` from the face embedded in the crate and 1 px of slack either side of the
  glyph — so ascenders and descenders are never cut. The source's box has that allowance below the
  line only.
- **The dot's gap.** The port leaves `0.04 * diameter` between the run and the dot (1.92 px at the
  default 1080p size), where the source's `marginLeft: 0.04em` is 0.04 of the *font size* (9.6 px
  there). The dot itself is drawn as an `<ellipse>` whose bottom sits on the baseline, with
  `scale_x` / `scale_y` applied about that bottom point — the same pivot as the source's
  `transform-origin: 50% 100%`.
- **The rest frame.** The impact squash snaps to nothing on frame 47 (the source's own length
  constant), three frames after this port's 44-frame movement window ends. Nothing is lost: the tail
  is 45 frames long, and the source's demo pads the same beat.
- **Scaling.** The source is authored on a 1280x720 canvas, whose `min(width, height) / 100` is
  `REFERENCE_VMIN` = 7.2 (`../motion.rs`); every design px value here is multiplied at draw time by
  `ctx.vmin() / 7.2`, i.e. 1.0 on a 720-tall canvas and 1.5 on the default 1920x1080. Option values
  are reference-canvas px: `font_size: 160` draws at 160 px on the reference canvas and 240 px on
  1080p, so the dot's `dot_size: 0.2` is 48 px there. The drop height is a share of the canvas
  height on both sides, so it needs no factor.
- **Degenerate values.** `fall_frames` and `recover` are floored at `f32::EPSILON`, so a zero falls
  in one frame instead of dividing by zero; `bounces: 0` leaves the single landing at
  `drop_start + fall_frames`; a non-positive or non-finite `speed` falls back to `1`. The dot is
  never allowed above its release point (`y` is clamped at 0) and never drawn before `drop_start`.
- **Backdrop and text colour.** The source's demo backdrop is `#0a0a0a` behind `#fafafa` text, its
  component default for `dotColor` is `#D97757` while its `config.ts` demo previews `#ff4d1a`, and
  its demo line is `"remocn"`. This port keeps the default line and the component's `#d97757` dot,
  moves the text to the house `#e6edf3`, and returns `#0d1117` from `background()` in place of the
  source's near-black demo canvas.

## Credit

Ported from `period-drop/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
