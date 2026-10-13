# Word Flip

`word-flip` — a headline that types itself out, then flips its key word through a replacement list.

## What it does

The line is `` `${prefix} ${words[0]} ${suffix}` ``. It types itself out character by character
behind a blinking caret that follows the newest keystroke, then the designated word cycles through
the rest of the list on a flip: an accelerating throw out and a decelerating settle in, overlapping
so the two words hand over at identity transforms.

Nothing reflows. Every candidate word is measured once and the flip slot reserves
`max(wordWidths)` from the first keystroke, so the line never moves when the word changes.

With the default options at 30 fps (30 typed characters, `cps` 9 → 10/3 frames per character):

| frame | what happens |
| --- | --- |
| 4 | The first keystroke lands (`typeStart`). |
| 4 → 100.72 | One character every 10/3 frames, each jittered by `keystrokeOffset(i, 0.18)` frames and fading in over `charFade` = 6 frames. The caret walks with them. |
| 100.72 → 106.72 | The last character (`o` of "Portfolio") fades in; `typingEnd` = 106.72. |
| 106.72 → 112.72 | The `pause` (6 frames) before the first flip; the caret has parked at the end of the line and keeps blinking at 2 Hz. |
| 112.72 | Flip 1: "Modern" throws out over 9 frames (up 1.29em, 0.98 scale, a 90° flip) while "Stunning" settles in from 0.135em below, starting 3 frames before the throw ends (`overlap`). |
| 147.72 | Flip 2: "Stunning" → "Minimal". |
| 162.72 | Flip 2's settle-in completes; the headline is at rest. The schedule would fire again at 182.72, but `flipsStarted` is capped at the last word, so nothing changes. |
| 162.72 → 230 | The headline holds. The timeline still reserves all three cycles — the source counts `words.len() * cycle` whether or not every cycle runs — plus the 12 frame tail. |

While typing, each character fades in on its own clock. Once the flips start the prefix and suffix
are drawn fully opaque and the slot shows the outgoing word, the incoming word, or both at once.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `prefix` | `String` | `"Looking For A"` | Text typed before the flipping word. |
| `words` | `Vec<String>` | `["Modern", "Stunning", "Minimal"]` | Replacement words. The first is typed; each flip advances one entry, stopping at the last. |
| `suffix` | `String` | `"Portfolio"` | Text typed after the flipping word. |
| `cps` | `f32` | `9.0` | Typing speed in characters per second. |
| `type_start` | `f32` | `4.0` | Frames before the first keystroke lands. |
| `char_fade` | `f32` | `6.0` | Fade-in duration per character, in frames. |
| `jitter` | `f32` | `0.18` | Keystroke jitter as a fraction of a character slot. |
| `pause` | `f32` | `6.0` | Frames between the last character and the first flip. |
| `cycle` | `f32` | `35.0` | Frames per flip cycle (one exit plus one enter). |
| `exit_duration` | `f32` | `9.0` | Exit throw duration, in frames. |
| `enter_duration` | `f32` | `9.0` | Settle-in duration, in frames. |
| `overlap` | `f32` | `3.0` | Frames the settle-in overlaps the exit throw. |
| `caret` | `bool` | `true` | Show the blinking caret; it keeps blinking after typing completes. |
| `font_size` | `f32` | `72.0` | Font size in px. |
| `font_weight` | `u16` | `600` | Font weight. |
| `tracking_em` | `f32` | `-0.03` | Headline tracking in `em`. Negative tightens. |
| `color` | `String` | `"#f4f4f8"` | Base text colour. |
| `accent_color` | `String` | `"#7c5cff"` | Colour of the flipping word and the caret bar. |

The source's `WordFlipDurationOptions` is ported too, with the source's defaults and its `tail`:

| option | type | default |
| --- | --- | --- |
| `prefix` | `String` | `"Looking For A"` |
| `words` | `Vec<String>` | `["Modern", "Stunning", "Minimal"]` |
| `suffix` | `String` | `"Portfolio"` |
| `cps` | `f32` | `9.0` |
| `type_start` | `f32` | `4.0` |
| `char_fade` | `f32` | `6.0` |
| `jitter` | `f32` | `0.18` |
| `pause` | `f32` | `6.0` |
| `cycle` | `f32` | `35.0` |
| `fps` | `f32` | `30.0` |
| `tail` | `f32` | `12.0` |

## How to render it

```sh
# just this component
cargo run --release -- render --components word-flip -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,word-flip -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components word-flip snapshot
cargo run --release -- --components word-flip --list
```

## Duration

```text
framesPerChar = fps / cps
last          = max(0, totalChars - 1)
typingEnd     = type_start + (last + keystrokeOffset(last, jitter)) * framesPerChar + char_fade
duration      = ceil(typingEnd + pause + words.len() * cycle + tail)
```

where `totalChars = chars(`${prefix} ${words[0]} ${suffix}`)` and
`keystrokeOffset(i, jitter) = jitter * (sin(i * 12.9898) * 0.6 + sin(i * 4.1414) * 0.4)`.

Ported from the source's `getWordFlipDuration` and exposed as
`word_flip::get_word_flip_duration` (with `typing_end_frame` and `keystroke_offset` alongside it).

Defaults: 30 characters → `typingEnd` = 106.723, then `+ 6 + 3 × 35 + 12` = 229.723 →
**230 frames = 7.67 s at 30 fps**.

## Fidelity notes

- **SVG cannot do `rotateX`.** The source throws each word out on a real 3D rotation —
  `rotateX(90deg)` about the baseline inside a `perspective: 6.5em` container. The port emulates it
  as an orthographic vertical squash: `scaleY = cos(angle)` about the same pivot, composed with the
  same `translateY` (−1.29em out, +0.135em in) and uniform scale (0.98 out, 1.0 in). At 90° the
  word compresses to an invisible edge line instead of foreshortening toward the pivot, and the
  perspective term is dropped — with it, the top edge would be pushed toward the pivot, which is
  the same compression the squash already applies. The `squash()` helper holds this and is
  unit-tested.
- **Blur.** The exit's motion blur (`exitSpeed(u) * 0.085em`, keyed to the derivative of
  `EASE_DEPART` rather than to progress) is **not rendered**: SVG has no blur path in this
  renderer. `word_flip::exit_blur_px` computes it exactly as the source does and is unit-tested,
  and nothing is faked in its place.
- **Kerning.** Once the flips start, the source renders the prefix and suffix as single text runs,
  so inter-character kerning is preserved; this port draws every character as its own `<text>` node
  (as the source's typing phase does), so kerning between adjacent characters is not reproduced.
  Positions, the slot and the caret are self-consistent with what is drawn.
- **Measurement.** `slotWidth = max(wordWidths)` and the per-character pen positions come from the
  face embedded in the crate instead of the DOM. `letter-spacing` is folded into each measured
  advance in both places, so the caret and the slot line up with the rendered glyphs.
- **`fit_scale` is always 1** in practice: the slot is the widest candidate word, so no word is ever
  wider than it. The formula is ported and unit-tested anyway.
- A zero or negative `cps` falls back to one frame per character, a non-positive `cycle` pins the
  flip counter, and a zero `char_fade` is floored to one frame, so a degenerate config can never
  emit NaN transforms.
- The source demo's `#0b0b10` backdrop is painted by `background()`.

## Credit

Ported from `WordFlip.tsx` in <https://github.com/Remocn/remocn> (MIT).
