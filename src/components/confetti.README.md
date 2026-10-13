# Confetti

`confetti` — a deterministic burst of drifting confetti pieces, fired from a point.

## What it does

Every piece is launched on its own angle and speed and then falls under gravity while fluttering
around its own axis. There is no simulation and no per-frame state: each piece's position, rotation
and flutter is a closed-form function of the burst time, so the same frame always renders the same
pixels and the whole burst can be scrubbed.

| channel | formula |
| --- | --- |
| launch velocity | `(cos θ * speed * power + drift, sin θ * speed * power)`, reference px per frame |
| position | `origin + v * t`, plus `0.5 * gravity * t²` on `y` |
| rotation | `rot0 + spin * t` radians |
| flutter | `cos(rot0 + spin * t * 1.6)`, used as the horizontal scale |
| opacity | ramp in over 4 frames, hold, then ramp out over the last 30% of `lifetime` |

Pieces are born at their own speed multiplier in `0.35..1.0` and their own size multiplier in
`0.7..1.3`, drawn from a seeded `mulberry32` generator in exactly the source's draw order — so a
given seed reproduces the source's scatter piece for piece.

Beat by beat, with the defaults (140 pieces, 30 fps):

| frames | beat |
| --- | --- |
| 0 | The burst fires. Every piece is at the origin at opacity 0. |
| 4 | The fade-in is complete; pieces are spread by their launch velocities. |
| 4 – 63 | Flight: `gravity` bends every path down, and each piece spins on its own axis. |
| 63 | The fade-out begins (70% of `lifetime`). |
| 90 | `lifetime` ends: every piece is at opacity 0. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `particle_count` | `usize` | `140` | How many pieces the burst fires. |
| `colors` | `Vec<String>` | *(empty)* | Palette the pieces draw from. Empty means `DEFAULT_COLORS` (6 colours). |
| `origin_x` | `f32` | `0.5` | Burst origin across the canvas, `0..1`. |
| `origin_y` | `f32` | `0.5` | Burst origin down the canvas, `0..1`. |
| `start_frame` | `usize` | `0` | Frame the burst fires on. |
| `lifetime` | `usize` | `90` | Frames the burst lives for. |
| `power` | `f32` | `17.0` | Launch speed, reference px per frame. |
| `gravity` | `f32` | `0.45` | Downward acceleration, reference px per frame². |
| `size` | `f32` | `13.0` | Base piece size in reference px. |
| `seed` | `u32` | `1` | PRNG seed; the same seed always renders the same burst. |
| `aspect` | `f32` | `0.62` | Piece height as a share of its width. |
| `corner_radius` | `f32` | `2.0` | Piece corner radius in reference px. |

`DEFAULT_COLORS` is `#1d9bf0`, `#ff5da2`, `#ffd23f`, `#22c55e`, `#a855f7`, `#ff7a45`.

The particle set is built once per component instance and cached in a `OnceLock`, so `show` neither
allocates nor rebuilds it. `confetti::make_particles`, `particle_opacity` and `particle_pose` are
public, so a burst can be sampled without building the component.

## How to render it

```sh
# just this component
cargo run --release -- render --components confetti -o out.mp4

# as part of a selection
cargo run --release -- render --components period-drop,confetti -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components confetti snapshot
cargo run --release -- --components confetti --list
```

## Duration

```text
duration = start_frame + lifetime
```

With the defaults: **90 frames**, 3.00 s at 30 fps.

## Fidelity notes

- **Scaling.** The source is authored on a 1280x720 canvas and scales its particle sizes and
  physics by `height / 720`. The port uses `ctx.vmin() / 7.2` (`REFERENCE_VMIN` in `../motion.rs`):
  the same factor on the reference canvas, and canvas-independent everywhere else. Option values are
  therefore reference-canvas px — `size: 13` draws at 19.5 px on the default 1920x1080, and
  `power: 17` launches at 25.5 px per frame.
- **Backdrop.** The source's demo canvas is its own; this port paints the house dark `#0d1117` and
  keeps the source's six-colour palette exactly.
- **No clip.** Pieces leave the frame as they fly; the source does not clip them and neither does
  the port — the renderer simply drops what is off-canvas.
- **The `start_frame` guard.** Before the burst and after `lifetime` the component returns an empty
  tree, matching the source's `return null`.

## Credit

Ported from `confetti/index.tsx` in <https://github.com/Remocn/remocn> (MIT).
