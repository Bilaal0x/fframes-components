# Text Swap

`text-swap` — replace one line of text with another on exit-then-enter scheduling, in five presets.

## What it does

The old line's segments travel out, and the new line starts entering at
`exitDuration + (segments - 1) * exitStagger - overlap + microDelay`, so the two cross over rather
than cutting. A preset decides whether the whole line travels as one block or every word travels on
its own, how long each phase takes, which easing drives it, and how far it moves.

| preset | unit | exit | enter | reads as |
| --- | --- | --- | --- | --- |
| `fly-through` | block | 20 frames, `EXIT_RUSH` | 16 frames, `ENTER_SOFT` | A perspective rush out under an 18-sample shutter, then a small 0.82 → 1 drop in. |
| `fade-through` | block | 8 frames, `MODERATE` | 13 frames, `MODERATE` | A short fade out 4px up, a longer 6px drop in from 0.99. |
| `crossfade` | word | 15 frames, `MODERATE` | 21 frames, `MODERATE` | Every word swaps on its own stagger, 6px up / 8px down. |
| `shared-axis-y` | block | 10 frames, `MODERATE` | 14 frames, `MODERATE` | The old line rises 24px out, the new one drops 24px in on the same axis. |
| `shared-axis-z` | block | 11 frames, `MODERATE` | 16 frames, `MODERATE` | The old line scales out to 1.06, the new one scales in from 0.9. |

The timing, with the default options and the `fly-through` preset:

| frames | what happens |
| --- | --- |
| 0 → 14.4 | The old line is fully opaque; `travel` runs 0 → 0.72 under `EXIT_RUSH`. |
| 0 → 20 | The wireframe of the exit: `travel` reaches 1, the perspective scale climbs from 1 to just under 12, and the 18 shutter samples smear the motion. |
| 14.4 → 20 | `exitFadeStart` (0.72 of the exit) is passed, so the opacity falls linearly to 0. |
| 21 → 37 | `enterStart = 20 + 0 − 1 + 2`, then the new line fades in from `enterScale` 0.82 while `enterY` 0 brings it to rest. The two windows overlap by `overlap` frames. |

The shutter is the `fly-through` exit's motion blur: 18 samples spread between last frame's travel
and this frame's, each scaled by `perspectiveScale(st, exitScale)` and faded by the same
`exitFadeStart` curve measured on its own travel value.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `from_text` | `String` | `"The Old Way of Thinking"` | The line that leaves. |
| `to_text` | `String` | `"The New Way of Building"` | The line that arrives. |
| `transition` | `SwapPreset` | `SwapPreset::FlyThrough` | Which of the five presets drives the swap. |
| `exit_stagger` | `usize` | `1` | Frames between exit segments (word-unit transitions). |
| `enter_stagger` | `usize` | `2` | Frames between enter segments (word-unit transitions). |
| `overlap` | `usize` | `1` | Enter starts this many frames before the exit fully ends. |
| `micro_delay` | `usize` | `2` | Extra frames after the computed enter start. |
| `speed` | `f32` | `1.0` | Scales all durations; `> 1` is faster. |
| `delay` | `usize` | `0` | Frames to wait before the swap starts. |
| `font_size` | `f32` | `72.0` | Font size in px. |
| `font_weight` | `u16` | `600` | Font weight. |
| `tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
| `color` | `String` | `"#e6edf3"` | Text fill. |

The preset table itself is public as `text_swap::PRESETS` (five `TextSwapPreset` rows with `unit`,
`exit_duration`, `enter_duration`, `exit_ease`, `enter_ease`, `exit_y`, `enter_y`, `exit_scale`,
`enter_scale`, `exit_blur`, `enter_blur`, `exit_fade_start` and `shutter`), and
`SwapPreset::def()` returns the row a preset selects.

## How to render it

```sh
# just this component
cargo run --release -- render --components text-swap -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,text-swap -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components text-swap snapshot
cargo run --release -- --components text-swap --list
```

## Duration

```text
exitDuration  = preset.exit_duration  / speed
enterDuration = preset.enter_duration / speed
exitSegments  = unit == word ? words(from_text) : 1
enterSegments = unit == word ? words(to_text)   : 1
enterStart    = max(0, exitDuration + (exitSegments - 1) * exit_stagger - overlap + micro_delay)
duration      = ceil(max(exitDuration + (exitSegments - 1) * exit_stagger,
                         enterStart + enterDuration + (enterSegments - 1) * enter_stagger))
```

Ported from the source's `getTextSwapDuration` and exposed as
`text_swap::get_text_swap_duration` (with `enter_start` for the enter frame alone).

Defaults, both lines five words and 30 fps:

| preset | frames | seconds |
| --- | --- | --- |
| `fly-through` | 37 | 1.23 s |
| `fade-through` | 22 | 0.73 s |
| `crossfade` | 49 | 1.63 s |
| `shared-axis-y` | 25 | 0.83 s |
| `shared-axis-z` | 28 | 0.93 s |

## Fidelity notes

- **Blend mode.** The 18 shutter samples are drawn with `mix-blend-mode: plus-lighter` in the
  source, which *adds* them; SVG has no blend mode here, so each of the 18 copies is drawn at
  `1/18` opacity and ordinary alpha compositing stands in for the additive sum. Overlapping
  samples therefore read dimmer than the source's saturating add.
- **Blur.** `filter: blur(Npx)` — the `exitBlur` / `enterBlur` column of the preset table (0.5/9
  for fly-through, 2 for fade-through's enter, 1/2 for shared-axis-z) — is **not rendered**. The
  values are carried in `PRESETS` for fidelity, but SVG filters are not available in this
  renderer, so the exit's motion smear comes from the 18 shutter samples alone.
- **The word-unit `crossfade` layout.** The source puts every segment in the same CSS grid cell
  (`grid-area: 1 / 1`) with `justify-items: center`, so with the word unit every word is centred
  on the canvas and the words render stacked on top of each other. This port lays the words out on
  their measured line and animates each word in place, which is what the preset's per-word stagger
  intends; the timing and all transforms are otherwise unchanged.
- **`transform-origin`.** SVG has no transform origin, so each segment is emitted as
  `translate(pivotX, pivotY + y) scale(sx, sy) translate(-pivotX, -pivotY)`: the canvas centre for
  block units, the word's own box centre for word units.
- **Measurement.** Both lines are measured with the face embedded in the crate
  (`ctx.text_width`) rather than from the DOM, and laid out as a single `white-space: nowrap` line.
- A non-positive or non-finite `speed` falls back to `1` instead of producing an infinite timeline.
- The source demo's `#0d1117` backdrop is painted by `background()`.

## Credit

Ported from `TextSwap.tsx` in <https://github.com/Remocn/remocn> (MIT).
