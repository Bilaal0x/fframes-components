# Cursor Flyover

A "camera" flies over a UI surface: it opens on a centred overview of the whole surface, dives in
on each of four corners in turn (each dive ends with that corner filling the screen centre), then
pulls back out to a slightly smaller overview — while a cursor arrow leads the way. The cursor fades
in at each corner, and pulses on the two left-hand ones.

Component id: `cursor-flyover` · Cargo feature: `cursor-flyover` ·
Module: `src/components/cursor_flyover.rs`

## What it does

The source composes a three.js `Matrix4` from a pose, inverts it and emits it as CSS `matrix3d`
inside a `perspective: d` container. Every pose it ever builds is a **pure translation** (the
quaternions are the identity and the scales are 1), and the source's own comment says the
rotation step is skipped because it is a no-op today. So the whole camera reduces to a divide:

```text
s = d / (d + pose.z)                                  # projection_scale
screen = (center + (x - pose.x) * s, center + (y - pose.y) * s)   # project_point
```

and this port implements exactly that. No 3D library, no matrix, no approximation.

Beat by beat with the defaults (`step_duration = 45`, `transition_duration = 45`,
`cursor_transition_duration = 35`, four corners, `perspective = 1200`):

| frames | step | what is on screen |
| --- | --- | --- |
| `0`–`44` | 0 — overview | the whole surface is centred at 65% scale (`1200 / (1200 + 648)`); the cursor sits at the canvas centre, invisible |
| `45`–`89` | 1 — top-left | the camera travels from the overview pose to `(-648, -259.2, 259.2)` with `EASE_IN_OUT`; the cursor travels the same path on its own 35-frame clock, so it lands at frame 80, ten frames before the camera |
| `45`–`89` | the pulse | the cursor pulses: `scale = 1 + sin(π × t)`, peaking at **2.0** on frame 67.5 and returning to 1.0 on frame 90 (the two left-hand corners pulse; the right-hand ones do not) |
| `90` | — | the surface's top-left corner is exactly on the canvas centre, at 82% scale (`1200 / (1200 + 259.2)`) |
| `90`–`134` | 2 — top-right | travel top-left → top-right (no pulse) |
| `135`–`179` | 3 — bottom-left | travel top-right → bottom-left (pulses again) |
| `180`–`224` | 4 — bottom-right | travel bottom-left → bottom-right (no pulse) |
| `225`–`269` | 5 — zoom-out | the camera pulls back to `z = 777.6` (61% scale) and the cursor shrinks to `scale 0.5` while fading out |
| `270` | — | end of the component |

There are `1 overview + corners + 1 zoom-out` steps; step *i* owns frames `i × step_duration`
onwards, and the last step holds until the component ends. Within a step the camera only travels
for `transition_duration` frames — with the default 45 the camera arrives exactly as the next step
begins, so every move runs end to end, and a shorter `transition_duration` leaves the pose resting
for the rest of the step instead. Step 0 is the exception: it has no previous pose to travel from,
so it holds the overview for its whole duration. Note that a pose is therefore *reached* at the
step boundary, not at the start of the step whose id names it.

### The cursor leads

Because the cursor's pose clock (35) is shorter than the camera's (45), the cursor reaches each
corner before the camera does and then waits there while the surface slides under it. The cursor's
world position is *the same point* as the camera's pose at that corner, so the arrow ends up at the
canvas centre at the end of every dive — with the surface's corner underneath it. That is the
source's design, and it falls out of the same projection formula.

### What is drawn

| element | how |
| --- | --- |
| surface | a rounded rect of `img_width × img_height` scaled by `s`, centred on the projected world origin, fill `#1c1c1c`, radius `vmin × 1.5 × s` |
| label | `label`, centred on the card, fill `#e6edf3`, weight `Fonts::weight_medium`, size `vmin × 4 × s` |
| cursor | the source's arrow path `M5 3 L19 12.2 L12 13.4 L8.6 20.4 Z`, fill `#ffffff`, stroke `#000000` width `2`, in a `vmin × 5 × s × pulse` box (`24 × 24` view box), centred on its projected position, with the cursor pose's opacity |
| background | `#0a0a0a` |

Geometry, using `vmin = min(width, height) / 100` (10.8 at 1080p):

```text
img_width  = vmin * 120          = 1296
img_height = img_width * 0.4     = 518.4
zoom       = img_width * 0.2     = 259.2
overview z = img_width * 0.5     = 648
zoom-out z = img_width * 0.6     = 777.6
corner x   = ±0.5 * img_width    corner y = ±0.5 * img_height
```

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `corners` | `Vec<Corner>` | `[TopLeft, TopRight, BottomLeft, BottomRight]` | Corner sequence to dive to, in order. `Corner` is `TopLeft`, `TopRight`, `BottomLeft` or `BottomRight`. |
| `step_duration` | `usize` | `45` | Frames each step owns; the last step holds until the component ends. |
| `transition_duration` | `usize` | `45` | Frames the camera travels at the start of a step. `0` is treated as one frame. |
| `cursor_transition_duration` | `usize` | `35` | The cursor's own, shorter pose clock. `0` is treated as one frame. |
| `perspective` | `f32` | `1200.0` | CSS `perspective` distance in px. |
| `label` | `String` | `"App"` | Text of the placeholder surface card. |

Public helpers, all pure and unit-tested:

| item | what |
| --- | --- |
| `flyover_steps(corners, img_width, img_height, zoom)` | the source's step list |
| `get_cursor_flyover_duration(corner_count, step_duration)` | `(corner_count + 2) * step_duration` |
| `active_step(frame, step_count, step_duration)` | the last `k` with `k * step_duration <= frame` |
| `projection_scale(perspective, z)` | `perspective / (perspective + z)` |
| `project_point(x, y, camera, perspective, center_x, center_y)` | world point → canvas point |
| `camera_pose(steps, frame, step_duration, transition_duration)` | the interpolated camera |
| `cursor_pose(steps, frame, step_duration, cursor_transition_duration)` | the interpolated cursor, pulse folded into `scale` |

## How to render it

```sh
# just this component
cargo run --release --features cli,cursor-flyover -- render --components cursor-flyover -o out.mp4

# as part of a selection
cargo run --release --features cli,cursor-flyover,cursor-flyover -- render --components text-reveal,cursor-flyover -o out.mp4

# inspect / preview / a still frame
cargo run --release --features cli,cursor-flyover -- --components cursor-flyover snapshot
cargo run --release --features cli,cursor-flyover -- --components cursor-flyover --list
```

Library use:

```rust
use fframes_components::components::cursor_flyover::{
    get_cursor_flyover_duration, Corner, CursorFlyover, CursorFlyoverConfig,
};

let duration = get_cursor_flyover_duration(4, 45); // 270
let component = CursorFlyover::new(CursorFlyoverConfig {
    corners: vec![Corner::TopLeft, Corner::BottomRight],
    step_duration: 60,
    ..CursorFlyoverConfig::default()
});
```

## Duration

```text
duration() = get_cursor_flyover_duration(corners.len(), step_duration)
           = (corners.len() + 2) * step_duration
```

* The source has **no duration helper** — that is a library addition, because a component here owns
  its own slot. It is the timeline's own length: the last step holds until the end.
* Default: `(4 + 2) * 45` = **270 frames** = **9.00 s** at 30 fps.
* The source's demo composition passes `durationInFrames={300}` by hand, which is 30 frames longer
  than the timeline needs; the extra frames hold the zoom-out pose.

## Fidelity notes

* **The projection is exact.** Every pose, every step boundary, every easing curve (`EASE_IN_OUT`
  for both the camera and the cursor pose), the raw-sine pulse and the whole projection divide are
  the source's. Two details are worth spelling out:
  * the camera translation is applied *before* the perspective divide (`x - pose.x`), which is what
    makes a dive land on a corner instead of merely zooming. With `pose.x = pose.y = 0` this is the
    plain `centre + point × s` formula;
  * `s` is applied to the surface card *and* to the cursor, as CSS applies one perspective to the
    whole 3D subtree.
* **The screenshot is a placeholder card.** The source takes a React node (an `<Img>` in its demo)
  and this library cannot load images, so the surface is a rounded `#1c1c1c` rect with the `label`
  centred on it. The card's geometry, radius and projection are the source's; only its *content* is
  a stand-in.
* **Soft shadows are dropped.** The source gives the card
  `box-shadow: 0 vmin*2 vmin*6 rgba(0, 0, 0, 0.6)` and the cursor
  `filter: drop-shadow(2px 4px 6px rgba(0, 0, 0, 0.5))`. SVG filters are unavailable for practical
  purposes in this library, and neither is faked here: there is no gaussian blur and no layered
  shadow substitute. Both shadows are simply absent.
* **The label's font size is this port's choice.** The source renders an arbitrary node, so there is
  no source formula to copy. `vmin * 4 * s` keeps the label proportional to the projected card.
* **`perspective` at or below the pose depth draws nothing.** With `perspective <= 0`, or a pose that
  cancels the denominator exactly, there is no meaningful projection; the frame comes back empty
  instead of producing an infinity or a mirrored surface. The default poses never get near this.
* **`transition_duration = 0` is clamped to one frame.** The source would compute `0 / 0` (a `NaN`
  transform) on the step's first frame; here the pose snaps over one frame instead, so no `NaN`
  can reach the renderer.
* **A `Vec<Corner>` with repeats or a single corner is allowed.** The source does the same —
  duplicate corners simply dive to the same place twice, and `corners: []` gives a two-step
  overview → zoom-out timeline of `2 * step_duration` frames.
* **No 3D rotation was lost.** The source never used one; both its camera branches are translations
  of a flat surface. There is nothing to approximate in SVG's lack of `rotateX/Y`.

## Credit

Ported from `CursorFlyover.tsx` in the Remotion component library by the **remocn** authors
(<https://github.com/Remocn/remocn>, MIT). The step table, the pose values, the corner points, the
pulse rule, the cursor arrow path and the defaults are theirs; this is a reimplementation against
fframes' SVG + Rust API and is not affiliated with Remotion. The three.js camera maths is replaced
by the closed-form projection above, which the source's own poses make possible.
