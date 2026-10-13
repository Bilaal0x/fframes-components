# Text Swell (`text-swell`)

A title reveal built around one idea: the lead word is pushed aside by the words that follow it.

## What it does, beat by beat

Defaults: `text = "Big Ideas Move Fast"` (lead word + 3 trailing), `font_size = 72`,
`rise_duration = 10`, `approach_delay = 14`, `approach_duration = 20`, `word_delay = 27`,
`word_stagger = 14`, `word_push_duration = 12`, `hold_duration = 6`, `recede_duration = 18`.

| frames | what happens |
| --- | --- |
| 0 – 10 | **Beat 1 — the lead rises.** `RISE_EASE` takes the lead word from 0.7 em below the baseline up to its rest position. It is alone and dead centre. |
| 14 – 34 | **Beat 2 — the line floats forward.** `APPROACH_EASE` scales the line from 1 toward `front_scale` (2.1), capped so the widest moment stays inside `FIT = 0.97` of the frame width. Still alone, the lead stays centred as it grows. |
| 27 – 39 | **Beat 3a — the second word cuts in.** It starts 0.15 em to the right and pushes in on `PUSH_EASE`, while its letters bounce up off the baseline one after another (`letter_stagger = 2`, `letter_rise = 3`, `letter_fall = 6`, peak `+0.23x` scale) about each letter's own centre on the baseline. |
| 27 – 67 | **Beat 3b — the shove.** Word `j` (0-based, after the lead) starts its push at `27 + 14j`, and every landed word shoves the line left by exactly its share of the width it added: `share[j] = (wordWidth[j+1] + spaceWidth) / totalAdded`, accumulated as `shoved = sum(share[j] * PUSH_EASE(p))` and applied as `tx = scale * (lineWidth / 2 - leadCenter) * (1 - shoved)`. Words after the first (`bounce_words = 1`) push in without a letter bounce. |
| 67 – 73 | **Beat 4a — hold.** The last word has landed at frame 67 (`27 + 2 * 14 + 12`); the line holds 6 frames at full scale. |
| 73 – 91 | **Beat 4b — recede.** `EASE_ZOOM_IN` takes the line from the front scale back to 1 and it settles at its final size. Frame 91 is the duration. |

The lead word's centre is exact throughout: it sits dead centre while it is alone and ends exactly
at the line's flexbox rest position, because the shove is pivoted at the line centre + baseline.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `text` | `String` | `"Big Ideas Move Fast"` | One line; the first word is the lead word that gets shoved aside. |
| `rise_distance_em` | `f32` | `0.7` | Beat 1: how far below rest the lead starts, in `em`. |
| `rise_duration` | `usize` | `10` | Beat 1: frames the lead's rise takes. |
| `front_scale` | `f32` | `2.1` | Beat 2: peak scale the line floats forward to. |
| `approach_delay` | `usize` | `14` | Beat 2: frames before the float forward starts. |
| `approach_duration` | `usize` | `20` | Beat 2: frames the float forward takes. |
| `word_delay` | `usize` | `27` | Beat 3: frames before the first trailing word pushes in. |
| `word_stagger` | `usize` | `14` | Beat 3: frames between consecutive words pushing in. |
| `word_push_em` | `f32` | `0.15` | Beat 3: how far right a word starts, in `em`. |
| `word_push_duration` | `usize` | `12` | Beat 3: frames one word's push takes. |
| `bounce_words` | `usize` | `1` | How many leading trailing words bounce letter-by-letter. |
| `letter_swell` | `f32` | `0.23` | Peak extra scale of a bouncing letter. |
| `letter_stagger` | `usize` | `2` | Frames between consecutive letters bouncing. |
| `letter_rise` | `usize` | `3` | Frames a letter's bounce up takes. |
| `letter_hold` | `usize` | `0` | Frames a letter holds its peak scale. |
| `letter_fall` | `usize` | `6` | Frames a letter's fall back takes. |
| `hold_duration` | `usize` | `6` | Beat 4: frames held after the last word lands. |
| `recede_duration` | `usize` | `18` | Beat 4: frames the fall back to final size takes. |
| `delay` | `usize` | `0` | Frames to wait before the reveal starts (shifts the component's clock). |
| `font_size` | `f32` | `72.0` | Font size in px. |
| `font_weight` | `u16` | `600` | Font weight. |
| `color` | `String` | `"#e6edf3"` | Text fill (the source's `lineStyle`). |
| `tracking_em` | `f32` | `-0.03` | Letter spacing in `em` (the source's `lineStyle`, `letterSpacing: "-0.03em"`). |

Named curves, all from the source: `RISE_EASE = cubic-bezier(0.2, 0.6, 0.35, 1)`,
`APPROACH_EASE = cubic-bezier(0.4, 0, 0.15, 1)`, `LETTER_EASE = cubic-bezier(0.4, 0, 0.2, 1)`,
`PUSH_EASE = cubic-bezier(0.22, 0.8, 0.36, 1)`, `FIT = 0.97`. Beat 4 reuses the library's shared
`EASE_ZOOM_IN` token (`cubic-bezier(0.5, 0, 0.05, 1)`), exactly as the source does.

## How to render it

```sh
# just this component (the feature has to be compiled in)
cargo run --release --features text-swell -- render --components text-swell -o out.mp4

# as part of a selection
cargo run --release --features text-swell,text-reveal -- render --components text-reveal,text-swell -o out.mp4

# a still frame / the option list
cargo run --release --features text-swell -- --components text-swell snapshot
cargo run --release --features text-swell -- --components text-swell --list
```

## Duration

```text
trailing = words(text) - 1
lastEnd  = trailing == 0 ? approach_delay + approach_duration
                         : word_delay + (trailing - 1) * word_stagger + word_push_duration
duration = max(lastEnd, approach_delay + approach_duration) + hold_duration + recede_duration
```

`get_text_swell_duration(text, &TextSwellTiming)` implements exactly that, with the source's
option defaults; `TextSwell::config.timing()` converts the component's own options into a
`TextSwellTiming`.

* Default (`"Big Ideas Move Fast"`, 3 trailing words): `max(27 + 2 * 14 + 12, 34) + 6 + 18` =
  `67 + 24` = **91 frames** = 3.03 s at 30 fps.
* One word: `max(34, 34) + 6 + 18` = **58 frames**.

## Fidelity notes

1. **No DOM measurement.** The source mounts a hidden, untransformed copy of the line behind
   `delayRender` and reads `offsetWidth` / `offsetTop` off it for `lineWidth`, `leadCenter`,
   `baseline`, `wordWidths` and `spaceWidth`. The port measures the same quantities with
   `RenderCtx::text_width` / `space_width` from the face embedded in this crate, so there is no
   measuring pass, no `NaN` frame if the DOM is slow, and identical output on every machine.
   `baseline` is therefore exactly `0.0`: every word is drawn in a coordinate system whose origin
   is already the line's left edge on the baseline, which is what the source's zero-height ruler
   measures. The source's DOM numbers are integers (`offsetWidth` rounds); this port does not
   round.
2. **`transform-origin` is emulated with explicit transforms.** The line is emitted as
   `translate(centre + tx, baseline) scale(finalScale) translate(-lineWidth / 2, 0)`, which is
   exactly the source's `translateX(tx) scale(s)` about the line centre + baseline; each bouncing
   letter is `translate(charCentre, baseline) scale(letterScale)`, exactly its
   `transform-origin: 50% <baseline>`.
3. **`TextSpec` is not used.** This component's text block is a single non-wrapping line with its
   own font defaults (72px / 600 / `-0.03em`), which do not match the shared `TextSpec` defaults
   (64px / 700 / `-0.02em`, wrap width, line height). The fields are declared here so the
   documented defaults match the code, and `color` / `tracking_em` come from the source's
   `lineStyle`, which the source applies unconditionally.
4. **`0 / 0` beats.** `clamp01((f - start) / duration)` with `duration = 0` is `NaN` in the
   source; the port treats a zero-frame beat as instantly complete, so no `NaN` can reach a
   transform.
5. **Zero added width.** If every trailing word and the space measure 0 (a degenerate line), the
   shares are 0 instead of `0/0`, so the shove is a no-op rather than `NaN`.
6. **The demo's extra tail.** The source's demo composition renders
   `getTextSwellDuration(text) + 30` frames to leave the settled line on screen; this component
   returns the helper's value exactly (91 by default). Add frames on the host timeline if you
   want the hold.
7. **No background and no wrap.** The source component paints no background (the demo's
   `#0d1117` belongs to the host) and sets `whiteSpace: nowrap`; the port is single-line and
   returns `background() = None`.

## Credit

Ported from `TextSwell.tsx` in <https://github.com/Remocn/remocn> (MIT).
