# Carousel 3D (`carousel-3d`)

A ring of cards spinning in 3D space, viewed through a perspective camera. Card `i` sits at angle
`i * 360/N + spin` with `x = sin * r`, `z = cos * r`, so the ring turns once every
`revolution_frames` frames, and the camera transform is the inverse of the configured
`CarouselPose`, which is what makes the world slide toward the viewer when the pose moves.

fframes renders SVG, which has no 3D — no `perspective`, no `preserve-3d`, no `rotateY`, no
`matrix3d`. The **ring maths, timing, painter ordering and card size are ported exactly** and
projected with a real perspective divide; each card is then drawn as a **2.5D approximation**, a
rounded rect plus its label scaled and squashed by the projection. See the fidelity notes for the
exact split.

## What it does, beat by beat

Defaults: `cards = ["1" .. "8"]`, `radius = 60`, `card_width = 40`, `card_height = None`,
`revolution_frames = 300`, `spin_offset = 0`, `pose` all zeros, `perspective = 1000`,
`card_facing = Outward`. On the 1920x1080 canvas `vmin = 10.8`, so `radius = 648px`,
`w = h = 432px` and the label is `43.2px`.

| frame | what happens |
| --- | --- |
| 0 | Card 1 has `theta = 0`: it is the front card, dead centre, at `x = 0`, `z = 648`, projected scale `1000 / (1000 - 648) = 2.84`. Cards 2 and 8 sit at `theta = ±45`, `x = ±458` in world space, projected to `x = 960 ± 846` at scale `1.85`, squashed to `cos 45 = 0.707` of their width. Cards 4 and 6 are exactly edge-on (`theta = ±90`, width 0) and are hidden; cards 3, 5 and 7 face away from the camera and are hidden. |
| 1 – 299 | The ring turns `360 / 300 = 1.2` degrees per frame. Each card sweeps from the front to edge-on (its squash falling from 1 to 0) and is then hidden; the next card rises out of the hidden half at the other side. Card 1 is back at the front on frame 300, one full revolution later. |
| 300 | The duration: exactly one revolution. Past it the host timeline ends. |

`CarouselPose` moves the camera, not the ring: the emitted camera is the pose's inverse, so a
negative `z` pulls the camera back away from the ring (`pose.z = -100` is "100 vmin back"), and
`rotate_y` / `rotate_z` turn the whole ring in the frame. The source demo animates
`pose = { z: -100 * travel, rotateZ: 180 * travel }` over its own keyframes; the port's pose is a
static configuration value, so reproduce that by rebuilding the component or by rendering the ring
spin alone.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `cards` | `Vec<String>` | `["1", "2", "3", "4", "5", "6", "7", "8"]` | The card labels, one per ring slot. The source takes React nodes; this port takes label strings. |
| `radius` | `f32` | `60.0` | Ring radius in `vmin`. |
| `card_width` | `f32` | `40.0` | Card width in `vmin`. |
| `card_height` | `Option<f32>` | `None` | Card height in `vmin`; `None` matches the width. |
| `revolution_frames` | `usize` | `300` | Frames per full revolution of the ring. |
| `spin_offset` | `f32` | `0.0` | Starting angle of the ring, in degrees — use it to give two carousels different starting positions instead of spinning in lockstep. |
| `pose.x` | `f32` | `0.0` | Camera step translation along x, in `vmin`. |
| `pose.y` | `f32` | `0.0` | Camera step translation along y, in `vmin`. |
| `pose.z` | `f32` | `0.0` | Camera step translation along z, in `vmin`; negative pulls the camera back. |
| `pose.rotate_x` | `f32` | `0.0` | Camera rotation about x in degrees (applied first). |
| `pose.rotate_y` | `f32` | `0.0` | Camera rotation about y in degrees (applied second). |
| `pose.rotate_z` | `f32` | `0.0` | Camera rotation about z in degrees (applied third). |
| `perspective` | `f32` | `1000.0` | Perspective distance in px. |
| `card_facing` | `CardFacing` | `Outward` | `Outward` faces the ring's centre away from the camera's near half; `Inward` flips which half of the ring shows a face. |
| `card_fill` | `String` | `"#22303f"` | Card fill. |
| `card_text` | `String` | `"#e6edf3"` | Card label fill. |

Published constants: `MIN_PERSPECTIVE_SCALE = 0.05`, `MAX_PERSPECTIVE_SCALE = 20.0` (the
perspective divide's clamp), `CARD_FONT_SIZE_VMIN = 4.0`, `CARD_FONT_WEIGHT = 700` and
`EDGE_ON_EPSILON = 1e-4` (the squash below which a card counts as edge-on and is hidden). The
source's other card numbers are structural and not configurable in the source either: the corner
radius is `vmin` and the label is `vmin * 4` at weight 700.

## How to render it

```sh
# just this component (the feature has to be compiled in)
cargo run --release --features carousel-3d -- render --components carousel-3d -o out.mp4

# as part of a selection
cargo run --release --features carousel-3d,text-swell -- render --components text-swell,carousel-3d -o out.mp4

# a still frame / the option list
cargo run --release --features carousel-3d -- --components carousel-3d snapshot
cargo run --release --features carousel-3d -- --components carousel-3d --list
```

## Duration

```text
duration = max(1, revolution_frames)
```

The source has **no duration helper**: `Carousel3D` runs for as long as the composition it is
dropped into. One full revolution is the natural length, so this port uses `revolution_frames`,
clamped up to 1 so a component can never occupy zero frames.

* Default: **300 frames** = 10.0 s at 30 fps (`revolutionFrames = fps * 10`, as the source's
  recipe comment suggests).
* `revolution_frames = 0` is treated as 1 frame.

## Fidelity notes

**Exact** (ported formula for formula, all unit tested):

* the ring geometry: `theta = i * (360 / N) + spin`, `x = sin(theta) * vmin * radius`,
  `z = cos(theta) * vmin * radius` (x takes sin, z takes cos);
* the timing: `spin = spin_offset + frame * 360 / max(1, revolution_frames)`, and the pose's
  Euler angles in degrees applied X then Y then Z, exactly as the source's
  `Euler(x, y, z, "XYZ")`;
* the camera: the inverse of the pose matrix, `[R^T | -R^T t]`, applied to every card position;
* the card size: `w = vmin * card_width`, `h = vmin * (card_height ?? card_width)`, corner radius
  `vmin`, label `vmin * 4` at weight 700;
* the perspective divide `perspective / (perspective - depth)`;
* the painter ordering: cards are drawn far to near, sorted by their ring `z`.

**Approximated** (SVG has no 3D, so the *look* is emulated):

1. **Cards are squashed, not rotated.** `rotateY(theta)` becomes a horizontal `scaleX` of
   `|cos theta|` about the card's own centre. That is the correct foreshortening for a flat,
   double-sided card, and the label squashes with the card exactly as it would in 3D, but there is
   no true perspective rotation: the card's corners never converge, and the round corners stay
   circular rather than becoming elliptical.
2. **Which half of the ring is drawn.** A card is hidden when `cos(theta + flip) <= 0` — past 90
   degrees from facing the camera, where its front has turned away. `CardFacing::Outward` (the
   default) shows the near half of the ring; `Inward` flips it to the far half. The source's CSS
   does not set `backface-visibility`, so it draws *both* halves (the far half mirrored); hiding
   the far half is this port's 2.5D stand-in for "you only ever see a card's front". Because
   `|cos theta|` already reaches 0 at exactly 90 degrees, the hidden card is a zero-width sliver
   when it disappears, so the transition is smooth.
3. **The label is never mirrored.** In real 3D an `Inward` card shows its back to the camera and
   its text would read mirrored; the port keeps the label readable and lets `card_facing` choose
   which half of the ring is visible instead.
4. **No box shadow.** The source's
   `boxShadow: 0 {vmin}px {vmin * 10}px rgba(0,0,0,0.5)` is a blurred drop shadow. SVG filters are
   not executable in this pipeline (see `PORTING.md`), and a hard-edged shadow would look worse
   than none, so no shadow is emitted.
5. **No `matrix3d`, no `perspective`, no `preserve-3d`, no `rotate(0.01deg)`.** The source emits
   these for the browser's compositor (the 0.01 degree rotation forces GPU rasterization); the
   port emits plain 2D `translate` + `scale` groups. The source's `zeroNoise` step only tidied the
   printed `matrix3d` digits and does not affect the maths.
6. **Cards at or behind the camera plane are skipped.** If `perspective - depth <= 0` the divide
   is meaningless (the source's CSS would project such a card with a negative scale, which is the
   classic "behind the camera" artifact). The port drops the card and clamps the scale to
   `0.05 ..= 20.0` otherwise, so an extreme `pose.z` can never emit a `NaN` transform.
7. **No background.** The source component paints none (the demo's `#111` and its gradient card
   fill belong to the host); this port returns `background() = None` and a flat `card_fill`. The
   demo's `linear-gradient(160deg, #22303f, #101820)` and its 1px border are not reproduced — an
   SVG `linearGradient` could, but it is not part of the component's own config.

## Credit

Ported from `Carousel3D.tsx` in <https://github.com/Remocn/remocn> (MIT).
