# Inflating Text (`inflating-text`)

Letters anticipate with a tiny squash, then inflate past their rest size on a damped-sine kick,
wobble once below rest and settle elastically, tinting toward an accent colour at peak inflation
and lifting a fraction of an em while they are big. After the line holds, the letters deflate out
one after another and fade away.

## What it does, beat by beat

Defaults: `text = "Make It Pop"` (11 characters), `stagger = 2`, `delay = 0`, `hold = 20`,
`deflate_stagger = 2`, `inflate_scale = 1.22`, `font_size = 110`.

| frames | what happens |
| --- | --- |
| 0 – 4 | Letter 0's **anticipation**: `sin(pi * u)` squashes it 8% wider and 8% shorter, peaking at frame 2 and releasing at frame 4. |
| 4 – 18 | Letter 0's **inflate**: the normalised damped sine `exp(-2u) * sin(2*pi*u) / peak` takes it to 1.22x at u = 0.2 (frame ~6.8), back through rest at u = 0.5 (frame 11), down to ~0.92x at u = 0.7 (frame ~13.8) and exactly back to rest at frame 18. |
| 0 – 38 | Every other letter repeats the same two beats, started at `2 * i` frames: the last letter (index 10) starts at frame 20 and finishes its swell at frame 38. |
| throughout | While a letter is away from rest it rotates up to 4 degrees (direction from `random("inflate-{i}")`, peak at mid-inflate), lifts up to `0.05 * font_size` = 5.5px, and tints from `#f5f5f7` toward `#ffd166` by `|scale - 1| / (inflate_scale - 1)` — full accent exactly at the peak. |
| 38 – 58 | **Hold**: the whole line sits inflated and settled. |
| 58 – 86 | **Deflate tail**: letter `i` starts at `58 + 2i`, and over 8 frames each shrinks to 0.85x and fades to 0. The last letter (index 10) starts at 78 and lands exactly on frame 86, which is the duration. |

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Make It Pop"` | The line to animate, split unicode-safe per character. |
| `font_size` | `f32` | `110.0` | Font size in px. |
| `font_weight` | `u16` | `800` | Font weight. |
| `inflate_scale` | `f32` | `1.22` | Peak scale a letter overshoots to mid-inflate. |
| `stagger` | `usize` | `2` | Frames between consecutive letters starting their swell. |
| `delay` | `usize` | `0` | Frames before the first letter starts. |
| `color` | `String` | `"#f5f5f7"` | Base fill of the letters. |
| `accent_color` | `String` | `"#ffd166"` | Letters tint toward this colour at peak inflation. |
| `tracking_em` | `f32` | `-0.02` | Letter spacing in `em` (the source `letterSpacing: "-0.02em"`). |
| `hold` | `usize` | `20` | Frames the whole line holds, inflated, before the deflate tail. |
| `deflate_stagger` | `usize` | `2` | Frames between consecutive letters starting to deflate. |
| `reduced_motion` | `bool` | `false` | Collapse every size/position channel; the tint and the fades stay. |

Published constants, all from the source: `ANTICIPATE_FRAMES = 4`, `INFLATE_FRAMES = 14`,
`HOLD_FRAMES = 20`, `DEFLATE_FRAMES = 8`, `SQUASH = 0.08`, `WOBBLE_DEG = 4.0`,
`BOUNCE_EM = 0.05`, plus `GOOEY_BLUR_CAP`, `SPEED_NORM` and `GOOEY_MATRIX`, which are recorded
but not used — see the fidelity notes.

## How to render it

```sh
# just this component (the feature has to be compiled in)
cargo run --release --features inflating-text -- render --components inflating-text -o out.mp4

# as part of a selection
cargo run --release --features inflating-text,text-reveal -- render --components text-reveal,inflating-text -o out.mp4

# a still frame / the option list
cargo run --release --features inflating-text -- --components inflating-text snapshot
cargo run --release --features inflating-text -- --components inflating-text --list
```

## Duration

```text
count     = characters in `text`
lastStart = delay + (count - 1) * stagger
total     = lastStart + ANTICIPATE_FRAMES + INFLATE_FRAMES + hold
            + (count - 1) * deflate_stagger + DEFLATE_FRAMES
duration  = max(1, total)
```

`get_inflating_text_duration(text, stagger, delay, hold, deflate_stagger)` implements exactly
that. The source takes the same four as an options object defaulting to
`{ stagger: 2, delay: 0, hold: 20, deflateStagger: 2 }`, and `max(1, ceil(total))` is `max(1,
total)` here because every term is a whole number of frames.

* Default (`"Make It Pop"`): `20 + 4 + 14 + 20 + 10 * 2 + 8` = **86 frames** = 2.87 s at 30 fps.
* An empty `text` lasts **1 frame**.

## Fidelity notes

1. **The gooey filter is omitted.** The source wraps the line in
   `feGaussianBlur` -> `feColorMatrix` with
   `values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 19 -9"`, keyed to the summed per-letter scale
   velocity of the fastest neighbouring pair (`SPEED_NORM = 0.3`) and capped at
   `GOOEY_BLUR_CAP = 0.04 * font_size`. fframes renders SVG and filters are not executable in
   this pipeline, so no filter is emitted: neighbouring silhouettes never melt together. The
   filter's three constants are published (`GOOEY_MATRIX`, `GOOEY_BLUR_CAP`, `SPEED_NORM`) for
   the record, and the velocity maths that only existed to key the filter is not ported.
   Everything else is exact: the anticipation squash, the kick, the wobble, the bounce, the tint
   and the deflate tail all match the source formula for formula.
2. **The transform pivot.** The source's inline-block letters carry
   `transform-origin: 50% 50%`, i.e. the centre of the em box. The port draws each glyph with its
   baseline at the local origin and pivots the `rotate`/`scale` on the em box's centre
   (`(ascent - descent) / 2` above the baseline), so rotation and scale are anchored exactly where
   CSS anchors them.
3. **`hold` and `deflate_stagger` are used in both places.** The source's component hardcodes
   `HOLD_FRAMES` and steps the deflate tail by `stagger`, while `getInflatingTextDuration` takes
   `hold` and `deflateStagger` as options; at the defaults (`stagger = deflateStagger = 2`,
   `hold = HOLD_FRAMES = 20`) they agree. The port uses `hold` and `deflate_stagger` in the
   render *and* the duration, so the two can never disagree when a caller changes one.
4. **Letter advances are measured, not laid out by a browser.** The source measures integer DOM
   metrics of `inline-block` spans; the port measures each character's advance from the face
   embedded in this crate and advances the cursor by `advance + tracking_em * font_size`. The
   result is the same order and spacing (a few px either way on rounding), and deterministic.
5. **`0 / 0` beats.** A zero-frame phase in the source produces `NaN`; every duration here is a
   whole number of frames and `hold`/`deflate_stagger` of 0 simply collapse the phase, with no
   `NaN` reaching a transform.
6. **No background.** The source component paints none; the demo composition's `#101014` fill
   belongs to the host. This port returns `None`, so the video's background (black by default)
   shows through — set a background on the host video for the demo's look.

## Credit

Ported from `InflatingText.tsx` in <https://github.com/Remocn/remocn> (MIT).
