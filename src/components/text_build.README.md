# Text Build — `text-build`

The words of a line land one at a time, and every word already on screen moves to make room for
the next one.

## What it does

The first word only drifts `first_drift` on the cross axis while it fades in. Every later word
enters over `push_duration` from the right (axis `x`) or from above (axis `y`), and during the
very same window the words already placed travel from the previous centred layout to the next
one, with a softness pulse that peaks in the middle of the move. The whole build uses the
library's single `EASE_OUT` entrance curve, so the "word arrives" gesture carries the same
weight here as in every other reveal. Each word also grows out of `ENTRY_SCALE` (0.992), and the
line carries the source's hard-coded `-0.03em` tracking.

Beat by beat, with the default configuration (4 words, axis `x`, `first_duration = 10`,
`push_duration = 13`, `speed = 1`, `font_size = 72`, 30 fps). Positions are x offsets from the
canvas centre; the four final values are the centred layout of 4 words:

| word | starts (frame) | lands (frame) | initial x | final x |
| --- | --- | --- | --- | --- |
| "Build" | 0 | 13 | centre (drifts up from 6 px low) | −343.2 |
| "The" | 10 | 23 | +88.0 | −186.0 |
| "Future" | 23 | 36 | +88.0 | −1.8 |
| "Together" | 36 | 49 | +88.0 | +273.5 |

| frame | what is on screen |
| --- | --- |
| 0 | "Build" appears at the centre, 6 px low, ~3.5 px soft, opacity 0, scale 0.992. |
| 6 | "Build" is all but landed: 0.2 px of drift left, 0.12 px of softness, opacity 0.97. |
| 10 | "The" appears 88 px right of the canvas centre. "Build" starts sliding left, from the centre towards −68 px. |
| 13 | "Build" has landed in the two-word layout; "The" is still pushing in. The reflow softness peaks around frame 16.5 (0.8 px) and is gone by frame 23. |
| 23 | "The" reaches +89.2 px — the two-word layout — and "Future" appears at +88 px. Both placed words start the next reflow. |
| 36 | "Together" appears at +88 px; every placed word makes its last move. |
| 49 | The whole line is placed: −343.2, −186.0, −1.8, +273.5 (an 854.9 px run, well inside the canvas). |
| 50–94 | The landed line holds for the 45-frame tail (1.5 s at 30 fps). |

Every word after the first enters from the same point — `entry_offset` px right of the canvas
centre on axis `x`, or above it on axis `y` — and then travels to its slot in the growing
layout. That is the source's behaviour, and it is why the line seems to unroll out of one point
while the placed words drift steadily leftward to keep themselves centred.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Build The Future Together"` | The line to build. |
| `axis` | `BuildAxis` | `X` | Layout axis: `X` lays words out by measured width, `Y` gives every word a `font_size`-tall slot. Later words enter from the right (`X`) or from above (`Y`). |
| `first_duration` | `usize` | `10` | Frames before the second word starts pushing in. |
| `push_duration` | `usize` | `13` | Frames each later word takes to push in, and placed words to reflow. Each word's first entry window is also this long. |
| `entry_offset` | `Option<f32>` | `None` | Entry distance on the primary axis, in px. `None` derives `88.0` (`X`) / `28.0` (`Y`). |
| `gap` | `Option<f32>` | `None` | Gap between placed words, in px. `None` derives `10.0` (`X`) / `12.0` (`Y`). |
| `entry_blur` | `f32` | `3.5` | Entry softness radius in px. |
| `first_drift` | `Option<f32>` | `None` | First word's drift on the cross axis, in px. `None` derives `6.0` (`X`) / `-14.0` (`Y`). |
| `reflow_blur` | `f32` | `0.8` | Softness pulse radius while placed words reflow, in px; it peaks mid-move. |
| `speed` | `f32` | `1.0` | Scales all durations; `> 1` is faster. A non-positive value falls back to `1.0`. |
| `delay` | `usize` | `0` | Frames to wait before the build starts. |
| `font_size` | `f32` | `72.0` | Font size in px. |
| `font_weight` | `u16` | `600` | Font weight. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `tail` | `usize` | `45` | Frames held after the last word lands. |
| `simulate_blur` | `bool` | `true` | Draw the low-opacity secondary copy that stands in for a real gaussian blur. |

There is no `TextSpec` block here: the source takes flat `fontSize` / `fontWeight` props rather
than a CSS style block, so the port keeps them flat and adds the colour the source hard-codes.

## How to render it

```sh
# just this component
cargo run --release -- render --components text-build -o out.mp4

# as part of a selection
cargo run --release -- render --components word-by-word,text-build -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components text-build snapshot
cargo run --release -- --components text-build --list
```

The component only exists when its Cargo feature is on, so a build that is not already using
`--features full` adds it (plus `cli` for the renderer binary) explicitly:

```sh
cargo run --release --features cli,text-build -- render --components text-build -o out.mp4
```

## Duration

```text
get_text_build_duration(text, first_duration, push_duration)
    = words(text).count() == 0 ? 0 : first_duration + (count - 1) * push_duration
duration() = get_text_build_duration(...) + tail
```

With the defaults (4 words): `10 + 3 * 13 = 49` entry frames `+ 45` tail = **94 frames**, 3.13 s
at 30 fps. The 45-frame tail is a port addition — the source exports the entry-only helper and
leaves the hold to the composition — so the landed line stays readable instead of being cut on
the frame the last word lands.

Matching the source's exported helper, `speed` and `delay` are **not** folded into this figure:
with `speed = 2` the build finishes at frame ~25 and simply holds longer, and with `speed < 1`
the build would run past the end of the slot, so raise `tail` or the video's slot for it.

## Fidelity notes

- **Blur is approximated, not rendered.** SVG has no gaussian blur this renderer can apply, so
  the source's `filter: blur(Npx)` is faked with a secondary copy of the word, displaced along
  the axis the words enter on (`blur * 0.6` px: right on axis `X`, up on axis `Y`) and drawn at
  `min(1, blur / 10) * 0.35` opacity behind the sharp copy. The sharp word is drawn at its full,
  un-dimmed opacity. It reads as softness, but it is a single trailing copy, not an isotropic
  gaussian, so `entry_blur: 3.5` looks less soft than the source and the 0.8 px reflow pulse is
  barely visible. Set `simulate_blur: false` to drop the copy entirely.
- The secondary copy is drawn only when the radius exceeds 0.01 px, mirroring the source's
  `blur > 0.01` guard.
- **Word widths ignore the tracking.** The source measures with canvas `measureText` after
  setting `ctx.letterSpacing = -0.03em * fontSize`, so its measured widths include the tracking;
  `ctx.text_width` measures the raw advances. At 72 px that is up to ~2.2 px per character, so a
  word's slot, the gap between neighbours and the final centred run can differ from the browser's
  by a few pixels (the drawn text still carries the tracking). The layout is internally
  consistent, which is what the animation needs.
- **The softness pulse resets when a new word starts.** `blur = reflow_blur * interpolate3(rp)`
  is 0 at the start of every push window and again at its end, so a word's softness can drop
  abruptly from the previous window's pulse as the next one begins. That is verbatim source
  behaviour, not a port artefact.
- **No 3D.** The source writes `translate3d(x, y, 0)`; SVG cannot do 3D, and the z component is
  always 0, so it becomes a plain 2D `translate` and nothing about the look is lost.
- Vertical centring uses the embedded face's metrics — the baseline sits at
  `centre + (ascent − descent) / 2`, i.e. +24.55 px at 72 px — where the source centres the span's
  CSS line box. The two agree to within about a pixel.
- A word before its start frame is not emitted at all; the source renders it as an `opacity: 0`
  span at the centre, which paints nothing either way.
- Words are split on any run of whitespace after trimming, which is the source's
  `text.trim().split(/\s+/)` exactly.
- The source's `FONT_STACK` resolves to the platform UI font; this library draws with the face
  embedded in the crate, so a port renders in the same font everywhere.

## Credit

Ported from `TextBuild.tsx` in the Remotion component library by the **remocn** authors,
<https://github.com/Remocn/remocn> (MIT). This port reimplements the animation against the
fframes SVG + Rust API and is not affiliated with Remotion.
