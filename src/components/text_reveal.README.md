# Text Reveal

`text-reveal` — a cinematic four-beat lead-word reveal.

## What it does

The first word of the line is the **lead**. It fades up huge and centred, drifts closer still and
hangs, then recedes to its final size on a short, heavily eased-in clock. From the same frame the
whole line slides left on a much longer clock while the trailing words push in one at a time from
the right.

The scale pivots on the lead word's centre at the text baseline (`transform-origin:
{leadRatio * 100}% {baseline}px` in the source), so the zoom reads as centred even while the line is
sliding. With the default options the four beats are:

| beat | frames | clock | what happens |
| --- | --- | --- | --- |
| 1 intro | 0 → 6 | `EASE_ZOOM_IN` | The lead word's opacity runs 0 → 1 while its scale is pinned at `initial_scale` = **2.3**. |
| 2 hold | 6 → 18 | `PUSH_EASE` | The lead word drifts from 2.3 to `2.3 × 1.06` = **2.438** and hangs there. |
| 3 recede | 18 → 32 | `EASE_ZOOM_IN` | The lead word recedes from 2.438 back to **1.0** on the short clock. |
| 4 assemble | 18 → 48 | `EASE_ZOOM_IN` | The whole line slides from `lineWidth × (0.5 − leadRatio)` to **0** on the long clock. |

The trailing words start at `recedeStart + wordDelay + j × wordStagger` — frames 25, 29 and 33 for
the three trailing words of the default line — and each pushes from `wordPushEm × fontSize` to 0
over `wordDuration` frames on `WORD_EASE`, with a deliberately tiny `wordFade` (2 frame) opacity
ramp underneath: they are *pushed*, not faded. They stay hidden until `wordDelay` frames into the
recede, so nothing appears while the lead word is still travelling.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Make Something People Love"` | One line of text; the FIRST word is the lead word. |
| `intro_duration` | `usize` | `6` | Beat 1: frames to fade the lead word in at `initial_scale`. |
| `hold_duration` | `usize` | `12` | Beat 2: frames drifting from `initial_scale` to `initial_scale * push_scale`. |
| `recede_duration` | `usize` | `14` | Beat 3: frames receding from full scale back to 1. |
| `assemble_duration` | `usize` | `30` | Beat 4: frames for the whole line to slide left into rest. |
| `word_delay` | `usize` | `7` | Trailing words stay hidden this many frames into the recede. |
| `word_duration` | `usize` | `14` | Frames each trailing word takes to push into its slot. |
| `word_stagger` | `usize` | `4` | Frames between trailing word starts. |
| `word_fade` | `usize` | `2` | Frames each trailing word's fade takes — deliberately tiny: they are PUSHED, not faded. |
| `word_push_em` | `f32` | `0.5` | How far right of its slot each trailing word starts, in `em`. |
| `initial_scale` | `f32` | `2.3` | Scale the lead word fades in at. |
| `push_scale` | `f32` | `1.06` | Extra scale the lead word drifts to before receding. |
| `delay` | `usize` | `0` | Frames to wait before the reveal starts. |
| `font_size` | `f32` | `72.0` | Font size in px. |
| `font_weight` | `u16` | `600` | Font weight. |
| `tracking_em` | `f32` | `-0.03` | Letter spacing in `em`. Negative tightens. |
| `color` | `String` | `"#e6edf3"` | Text fill. |

The source component's `text` prop has no default (it is required); this port defaults to the line
the source's own demo passes.

## How to render it

```sh
# just this component
cargo run --release -- render --components text-reveal -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,text-swap -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components text-reveal snapshot
cargo run --release -- --components text-reveal --list
```

## Duration

```text
trailing  = max(0, words(text) - 1)
recedeStart = intro_duration + hold_duration
wordsEnd  = trailing == 0 ? 0
                           : word_delay + (trailing - 1) * word_stagger + word_duration
duration  = recedeStart + max(assemble_duration, wordsEnd, recede_duration)
```

Ported from the source's `getTextRevealDuration` and exposed as
`text_reveal::get_text_reveal_duration`. The source component adds no hold tail of its own.

Default (`"Make Something People Love"`, 3 trailing words): `18 + max(30, 7 + 8 + 14, 14)` =
**48 frames = 1.60 s at 30 fps**.

## Fidelity notes

- **Measurement.** The source measures `lineWidth`, the lead word's centre and the baseline from
  the DOM behind `delayRender`. This port measures with the face embedded in the crate:
  `leadRatio = (lead_width / 2) / line_width`, the lead centre is the line's left edge plus
  `lead_width / 2`, and `slideDistance = line_width * (0.5 - leadRatio)` — which is exactly the
  negative of the lead word's centre offset from the line's centre, so at frame 0 the lead word's
  centre lands on the canvas centre, as in the source.
- **Baseline.** The source relies on CSS `line-height: normal`. The port asks the face for its own
  line height (`TextMeter::line_height`) and centres that box, instead of assuming a ratio.
- **`transform-origin`.** SVG has no transform origin, so the pivot is emitted explicitly: the line
  is one `<g>` with `translate(pivotX + slideX, baseline) scale(scale) translate(-pivotX, -baseline)`.
- **Tracking and measurement.** `letter-spacing` is applied as SVG `letter-spacing` on each
  `<text>`, but the measured advances do not include it (the same approximation the rest of this
  library makes). At the default `-0.03em` the difference is a fraction of a pixel per character.
- Nothing else is lost: this component uses no blur, no 3D transform, no blend mode and no filter.
- The source demo's `#0d1117` backdrop is painted by `background()`.
- Degenerate zero-length beats are floored to one frame, so a config can never emit a NaN
  transform.

## Credit

Ported from `TextReveal.tsx` in <https://github.com/Remocn/remocn> (MIT).
