# Multi Text Typewriter

Cycles through several strings: each one is typed character by character, held, then backspaced
away before the next one starts. After the last string the cycle would restart from the first, so
the component plays as an endless loop. A blinking `|` cursor rides the end of the line and keeps
blinking on the unclamped clock — through the deletions and through the empty gaps between strings.

Component id: `multi-text-typewriter` · Cargo feature: `multi-text-typewriter` ·
Module: `src/components/multi_text_typewriter.rs`

## What it does

The animation is driven by a **simulated event timeline**, exactly like the source component's
`simulateCycle`. `simulate_cycle` replays one complete type-hold-backspace cycle up front and
records the string that is on screen at every frame; rendering a frame is then just "find the last
event whose frame has already passed". There is no randomness, so the timeline is trivially
frame-deterministic.

Beat by beat, with the default options (`"Design."`, `"Develop."`, `"Deploy."`, `"Repeat."`,
3 frames per typed character, 2 per backspace):

| frame | what is on screen |
| --- | --- |
| `0` | the empty screen; the cursor is present but faded out |
| `1`–`2` | the cursor ramps up to full opacity (`EASE_OUT` over 2 frames) |
| `3` | `D` |
| `21` | `Design.` complete |
| `21`–`51` | the 30 frame hold after typing |
| `53`–`65` | the seven backspaces, one every 2 frames |
| `65`–`80` | the 15 frame gap on the empty screen |
| `83` | `D` of `Develop.` |
| `104` | `Develop.` complete |
| `150` | the screen is empty again |
| `168` | `D` of `Deploy.`; `186` completes it; `230` empties it |
| `248` | `R` of `Repeat.`; `266` completes it; `310` empties it |
| `310`–`325` | the final 15 frame gap |
| `325` | end of the component; a longer slot would wrap back to frame 0 |

With `delay = 10` nothing is typed for the first 10 frames (the cursor still blinks); the cycle
then starts at frame 10 and the component is 10 frames longer.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `texts` | `Vec<String>` | `["Design.", "Develop.", "Deploy.", "Repeat."]` | Strings cycled through: type, hold, backspace, next. |
| `type_speed` | `usize` | `3` | Frames per typed character. |
| `delete_speed` | `usize` | `2` | Frames per backspaced character. |
| `pause_after_type` | `usize` | `30` | Frames the completed line holds before deleting starts. |
| `pause_after_delete` | `usize` | `15` | Frames the empty line holds before the next string starts. |
| `cursor` | `bool` | `true` | Draw the blinking `\|` cursor. |
| `blink_speed` | `usize` | `30` | Full blink cycle in frames (half on, half off). |
| `delay` | `usize` | `0` | Frames to wait before typing starts. |

### Fixed styling

The source component takes a free-form CSS `style` prop, which this library does not model, so the
styling is fixed:

| property | value |
| --- | --- |
| font size | `56` px |
| font weight | `700` |
| family | `ctx.mono()` (the monospaced family; `Fonts::mono`, default `"DM Sans"`) |
| line height | `1.2` × font size (only matters if a string contains `\n`) |
| fill | `#e6edf3` |
| cursor glyph | `\|`, same fill |
| background | `#0d1117` |

## How to render it

```sh
# just this component
cargo run --release -- render --components multi-text-typewriter -o out.mp4

# as part of a selection
cargo run --release -- render --components multi-text-typewriter,type-writer -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components multi-text-typewriter snapshot
cargo run --release -- --components multi-text-typewriter --list
```

The component only exists when its Cargo feature is on, so a build that is not already using
`--features full` adds it (plus `cli` for the renderer binary) explicitly:

```sh
cargo run --release --features cli,multi-text-typewriter -- render --components multi-text-typewriter -o out.mp4
```

Library use:

```rust
use fframes_components::components::multi_text_typewriter::{
    MultiTextTypewriter, MultiTextTypewriterConfig, simulate_cycle,
};

let timeline = simulate_cycle(&MultiTextTypewriterConfig::default());
let component = MultiTextTypewriter::new(MultiTextTypewriterConfig {
    delay: 15,
    ..MultiTextTypewriterConfig::default()
});
```

## Duration

```text
total_duration = time after the last string's pause_after_delete
duration()     = max(1, delay + total_duration)
```

The trailing pause **is** part of the cycle — looping has to hold the empty screen before
restarting rather than snapping straight into the first word. The source returns
`Math.round(time)`; the clock only ever moves in whole frames here, so `total_duration` is exactly
`time`.

With the defaults (29 characters over four strings): `80 + 85 + 80 + 80` = **325 frames** =
**10.83 s** at 30 fps. Per string the cost is `len * type_speed + pause_after_type +
len * delete_speed + pause_after_delete`; with the defaults that is `5 * len + 45`, which is why
the eight-character `Develop.` costs 85 and the rest 80.

The assembled video gives each component exactly `duration()` frames, so one slot plays exactly one
cycle and ends on the empty screen. The modulo clock (`clock % total_duration`) is what the source
uses, and it means the component still behaves correctly if a host renders it past its own cycle
length: the cycle simply repeats.

## Fidelity notes

* **No wrapping.** The source renders the string into one `white-space: pre-wrap` line box, so a
  long string re-wraps at the container width; a `\n` starts a new line. SVG `<text>` does not
  reflow, so this port splits on `\n` and draws one `<text>` node per line without ever wrapping.
  A line longer than the canvas runs off the edge instead of wrapping.
* **The block is centred, the source is not.** The block's left edge is pinned to the width the
  longest string will need — plus the cursor's advance when the cursor is drawn — and that block is
  centred on the canvas, so characters grow to the right from a fixed left edge instead of the line
  re-centring on every keystroke. The source just flows inside whatever container the caller
  provides, so its alignment is the caller's choice.
* **The cursor is a text node.** The source renders an inline-block `<span>` whose opacity is the
  blink value; this port draws a `|` `<text>` node at the end of the last line with the same
  opacity, and omits it entirely when `cursor = false`. The blink formula is the source's verbatim
  (2-frame `EASE_OUT` ramp in and out of each half-cycle, dark at exactly the halfway frame), driven
  by the *unclamped* clock, so the cursor keeps blinking through the deletions and the gaps.
* **Deleting is unicode-safe.** The source deletes with `current.slice(0, -1)` and measures with
  `current.length`, both of which count UTF-16 code units, so an emoji (a surrogate pair) can be
  half-deleted or typed in two steps. This port keeps the on-screen string in a `Vec<char>` and
  counts characters, so a character is typed and deleted as one unit. For the demo strings, which
  are pure ASCII, the two behave identically.
* **No hold tail.** The source has no notion of a component duration; the Remotion composition's
  length is chosen outside the component. This port uses one full cycle, which always ends on the
  empty screen and so needs no extra hold. Add more strings, or raise `pause_after_delete`, to hold
  the frame longer.
* **Nothing else was lost.** No blur, 3D, blend mode or canvas-only effect is used by the source
  component.
* **Style prop.** The source's `style` override is not exposed; see the fixed styling table above.

## Credit

Ported from `MultiTextTypewriter.tsx` in the Remotion component library by the **remocn** authors
(<https://github.com/Remocn/remocn>, MIT). The timing constants, the blink curve and the defaults
are theirs; this is a reimplementation against fframes' SVG + Rust API and is not affiliated with
Remotion.
