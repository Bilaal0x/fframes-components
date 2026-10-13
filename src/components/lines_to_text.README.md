# Lines to Text

Abstract stroke bars drift like a data field, converge into striped blocks sized to each text
line, then resolve into readable type through a shared clip edge — the wipe reveals glyphs on its
left while consuming the bar block on its right, so the lines literally become the text.

## What it does

With the 30 fps defaults — `drift_frames 12`, `converge_duration 14`, `resolve_duration 8`,
`hold_frames 48`, `exit_duration 18`, 3 lines, 3 bars per line (9 bars, 100 frames total):

| frames | what happens |
| --- | --- |
| 0–12 | The line field drifts. Nine white bars sit at seeded left offsets and lengths, spread over `block_height * 1.2` of vertical jitter, each wandering on its own fixed sine: `sin(gi * 2.13 + frame * 0.09) * 9`. |
| 12–26 | Each bar converges into a stripe of its line, staggered 2 frames apart (`gi * 2`). Left, scale and y all interpolate from the field state to the target with `cubic-bezier(0.2, 0.6, 0.35, 1)`. |
| 22–30 | The resolve wipe runs, one line every 2 frames, starting 4 frames before the converge ends so each line turns into type on arrival. The wipe edge sweeps left to right: the bar block is consumed from its left as the text is revealed from its left. |
| 30–78 | The resolved text holds. |
| 78–96 | The exit mirrors the whole morph with the same curve: bars ease back out into the drifting field while the text is consumed by the reverse wipe. |
| 96–100 | The field state is fully restored (see the duration note below). |

Because the exit ends where frame 0 began, the component is loopable.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `lines` | `Vec<String>` | `["LINES", "TO", "TEXT"]` | Text lines, top to bottom. Rendered uppercase. |
| `drift_frames` | `usize` | `12` | Frames the line field drifts before converging. |
| `converge_duration` | `usize` | `14` | Frames the converge morph takes. Durations are clamped to at least 1. |
| `resolve_duration` | `usize` | `8` | Frames one line's text resolve takes. |
| `hold_frames` | `usize` | `48` | Frames the resolved text holds before the exit. |
| `exit_duration` | `usize` | `18` | Frames the mirror exit back to lines takes. |
| `bars_per_line` | `usize` | `3` | Striped bars per text line. Clamped to at least 1. |
| `start_frame` | `usize` | `0` | Frame the morph starts at. |

There are no styling options: the source derives everything from the canvas, and so does the port.

| derived value | formula | at 1080p |
| --- | --- | --- |
| `font_size` | `max(1, round(height * 0.12))` | 130 px |
| `line_step` | `round(font_size * 1.16)` | 151 px |
| `letter_spacing` | `-0.02` em | -0.02 em |
| `bar_height` | `max(2, round(font_size * 0.22))` | 29 px |
| `bar_gap` | `round(font_size * 0.05)` | 7 px |
| `block_height` | `(lines.len() - 1) * line_step + font_size` | 432 px |

Bar and text fills are `#ffffff`; the background is `#e8e9ec`. The text is set in `Fonts::sans`
(the embedded `DM Sans`) at weight 800.

## How to render it

```sh
# just this component
cargo run --release --features cli -- render --components lines-to-text -o out.mp4

# as part of a selection
cargo run --release --features cli -- render --components word-by-word,lines-to-text -o out.mp4

# inspect / preview / a still frame
cargo run --release --features cli -- --components lines-to-text snapshot
cargo run --release --features cli -- --components lines-to-text --list
```

## Duration

```text
lines_to_text_duration(drift, converge, resolve, hold, exit)
    = max(1, drift + converge + resolve + hold + exit)
```

This is the source's `linesToTextDuration` verbatim, and it is exposed as a public function. With
the defaults it is `12 + 14 + 8 + 48 + 18 = 100` frames — **3.33 s at 30 fps**.

Note the four-frame overlap: the resolve starts at
`resolve_start = start_frame + drift + converge - 4`, so the morph actually finishes at frame
`exit_start + exit_duration = 96` while the helper reports 100. The last four frames hold the fully
mirrored field state. That is the source's own arithmetic, kept as-is; `start_frame` shifts every
channel and is *not* added to the helper, again as in the source.

## Fidelity notes

- **The resolve wipe is a real SVG clip.** The source clips the text with
  `clip-path: inset(0 (1 - p2) * 100% 0 0)` on a full-width, centre-aligned block, whose visible
  region is `x in [0, width * p2]`. The port emits exactly that region as a `<clipPath>` holding a
  rect `x = 0, width = width * p2` over the full canvas height, and references it from the `<text>`
  node. The clip edge cuts through glyphs mid-stroke, as the source's does — truncating the string
  instead would only ever reveal whole characters.
- **The bar scale is computed, not transformed.** The source uses `transform: scaleX(s)` with
  `transform-origin: 0% 50%`. SVG `scale()` about the rect's left edge is what that means, so the
  port computes the rect directly: `x = left + bar_width * p2`, `width = bar_width * (1 - p2)`,
  where `bar_width = canvas_width * scale`. This is exact and avoids any transform-origin
  ambiguity.
- **Line widths are measured with the real font.** The source measures with canvas `measureText` in
  an `800`-weight system sans stack, adds the letter-spacing correction
  `chars * fontSize * letterSpacing`, and falls back to `chars * fontSize * 0.62 + correction` when
  there is no DOM. The port measures the actual uppercase string with the face embedded in this
  crate and applies the same tracking correction; the fallback is never needed here because
  measurement never fails (`RenderCtx::text_width`). The widths are therefore a few pixels off a
  browser's `measureText` for a different font stack — in exchange they are deterministic and
  identical on every machine. Widths are measured once per component instance and reused.
- **The font stack is `Fonts::sans`.** The source hard-codes
  `-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Helvetica Neue', sans-serif`. A host that wants
  the source's look should point `Fonts::sans` at a comparable grotesque; `DM Sans` is the crate
  default.
- **`willChange` hints are dropped** — they are compositor hints with no SVG equivalent and no
  visual effect.
- **`textRendering: geometricPrecision` is not reproduced.** It is a browser rasterisation hint;
  the renderer's own text rasterisation applies.
- **One `<g>` with `<defs>`, the bars, then the text nodes.** The source nests absolutely
  positioned `<div>`s in an `AbsoluteFill` in that same order, so the paint order matches: bars
  first, text on top.
- No blur, blend mode, shadow or 3D is used by the source, so nothing else had to be emulated.

## Credit

Ported from `LinesToText.tsx` in the Remotion component library by the **remocn** authors,
<https://github.com/Remocn/remocn> (MIT). The morph, its timing constants, the converge easing
`cubic-bezier(0.2, 0.6, 0.35, 1)`, the field layout formulas and the resolve/exit structure are
theirs; this is a reimplementation against fframes' SVG + Rust API.
