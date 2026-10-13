# Type Writer

Types its text one character at a time at a fixed per-character cadence, optionally fumbling a
wrong character now and then (and backspacing it before typing the right one), holding the
finished string, then either stopping or replaying the whole session. A blinking `|` cursor rides
the end of the last line.

Component id: `type-writer` · Cargo feature: `type-writer` · Module: `src/components/type_writer.rs`

## What it does

The animation is driven by a **simulated event timeline**, exactly like the source component's
`simulateTypeWriter`. `simulate_type_writer` replays the entire session up front and records the
string that is on screen at every frame; rendering a frame is then just "find the last event whose
frame has already passed". Typing, pausing, typo correction and clearing the screen all fall out of
that one table, which is why the timing is identical to the source at every frame.

Beat by beat, with the default options (one 56-character string, 3 frames per character, no delay):

| frame | what is on screen |
| --- | --- |
| `0` | the empty screen; the cursor is present but faded out |
| `1`–`2` | the cursor ramps up to full opacity (`EASE_OUT` over 2 frames) |
| `3` | `H` |
| `6` | `He` |
| … | one character every 3 frames, `\n` included in the count |
| `168` | the whole string is on screen (56 characters × 3 frames) |
| `168`–`197` | the hold tail: the finished text is on screen and the cursor keeps blinking (or disappears when `show_cursor_after_complete = false`) |
| `198` | end of the component |

With `delay = 10` nothing is typed for the first 10 frames (the cursor still blinks); the session
then starts at frame 10 and the component is 10 frames longer.

With `looping = true` the clock is taken modulo the session length, so the timeline replays from
frame 0 every `total_duration` frames. A non-looping run clamps the clock at the session length
instead, which is what makes `complete` (and therefore `show_cursor_after_complete`) meaningful.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `texts` | `Vec<String>` | `["Hello, World!\nThis text is typed character by character."]` | Strings typed one after another, each `\n` starting a new line. |
| `type_speed` | `usize` | `3` | Frames per typed character. |
| `delete_speed` | `usize` | `2` | Frames per deleted character. |
| `pause_after_type` | `usize` | `30` | Frames held after a string finishes typing. |
| `pause_after_delete` | `usize` | `15` | Frames held after a string is fully deleted. |
| `error_rate` | `f32` | `0.0` | `0..=1` per-character probability of a typo that is corrected. |
| `error_correct_delay` | `usize` | `5` | Frames a wrong character sits before being backspaced. |
| `seed` | `String` | `"typewriter"` | Seed for deterministic typos and wrong characters. |
| `cursor` | `bool` | `true` | Draw the blinking `\|` cursor. |
| `blink_speed` | `usize` | `30` | Full blink cycle in frames (half on, half off). |
| `show_cursor_after_complete` | `bool` | `true` | Keep the cursor blinking once the session has finished. |
| `delay` | `usize` | `0` | Frames to wait before typing starts. |
| `looping` | `bool` | `false` | Replay the whole timeline indefinitely (the source `loop` prop — `loop` is a Rust keyword). |
| `delete_before_next` | `bool` | `true` | Delete the previous string before typing the next one. When `false` the strings append to each other; a looping append run only clears at the end of the list. |
| `tail` | `usize` | `30` | Frames held after the session ends. **Library addition**, not in the source component. |

### Fixed styling

The source component takes a free-form CSS `style` prop, which this library does not model, so the
styling is fixed:

| property | value |
| --- | --- |
| font size | `42` px |
| font weight | `400` |
| family | `ctx.mono()` (the monospaced family; `Fonts::mono`, default `"DM Sans"`) |
| line height | `1.5` × font size |
| fill | `#e6edf3` |
| cursor glyph | `\|`, same fill |
| background | `#0d1117` |

### Typo alphabet

`KEYBOARD_CHARS` is the source list verbatim — 67 characters:
`abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,!? ` (the last one is a space).

## How to render it

```sh
# just this component
cargo run --release -- render --components type-writer -o out.mp4

# as part of a selection
cargo run --release -- render --components text-reveal,type-writer -o out.mp4

# inspect / preview / a still frame
cargo run --release -- --components type-writer snapshot
cargo run --release -- --components type-writer --list
```

The component only exists when its Cargo feature is on, so a build that is not already using
`--features full` adds it (plus `cli` for the renderer binary) explicitly:

```sh
cargo run --release --features cli,type-writer -- render --components type-writer -o out.mp4
```

Library use:

```rust
use fframes_components::components::type_writer::{TypeWriter, TypeWriterConfig, simulate_type_writer};

let timeline = simulate_type_writer(&TypeWriterConfig::default());
let component = TypeWriter::new(TypeWriterConfig { delay: 15, ..TypeWriterConfig::default() });
```

## Duration

```text
total_duration = last event frame of simulate_type_writer(config)
duration()     = max(1, delay + total_duration + tail)
```

* `total_duration` is the source's `getTypeWriterDuration`: the frame of the last timeline event,
  which for a non-looping run is the moment the last character lands. The pauses after a string are
  **not** part of it; they only matter when another string follows (`delete_before_next`) or when
  `looping` triggers the final clear.
* `delay` frames are prepended and `tail` frames appended, so the delayed start and the finished
  state are both visible inside the component's slot.
* Default (`delay = 0`, `tail = 30`): `168` frames of typing + `30` tail = **198 frames** =
  **6.60 s** at 30 fps.

## Fidelity notes

* **No wrapping.** The source renders each `\n`-separated line into its own line box with
  `white-space: pre-wrap`, so a long line re-wraps at the container width. SVG `<text>` does not
  reflow, so this port draws one `<text>` node per `\n`-separated line and never wraps: a line
  longer than the canvas runs off the edge. Text is never clipped or truncated.
* **The block is centred, the source is not.** The block's left edge is pinned to the width the
  fully typed text will need — plus the cursor's advance when the cursor is drawn — and that block
  is centred on the canvas. Characters therefore grow to the right from a fixed left edge instead of
  the line re-centring on every keystroke. The source just flows inside whatever container the
  caller provides, so its alignment is the caller's choice.
* **`tail` is a library addition.** The source has no notion of a component duration; the Remotion
  composition's length is chosen outside the component. Without a hold after the last character,
  `complete` would never be true inside the component's slot, and `show_cursor_after_complete =
  false` could never be seen. 30 frames (1 s at 30 fps) is enough to read the finished line.
* **The cursor is a text node.** The source renders an inline-block `<span>` whose opacity is the
  blink value; this port draws a `|` `<text>` node at the end of the last line with the same
  opacity, and omits it entirely when `cursor = false`. The blink ramp itself is the source formula
  verbatim (2-frame `EASE_OUT` ramp in and out of each half-cycle, dark at exactly the halfway
  frame), on the *unclamped* clock, so it keeps blinking through a delayed start and while looping.
* **Deleting is unicode-safe.** The source deletes with `current.slice(0, -1)` and measures with
  `current.length`, both of which count UTF-16 code units, so an emoji (a surrogate pair) can be
  half-deleted or typed in two steps. This port keeps the on-screen string in a `Vec<char>` and
  counts characters, so a character is typed and deleted as one unit. For pure ASCII — the source's
  demo text, and the whole `KEYBOARD_CHARS` alphabet — the two behave identically.
* **Typos are deterministic but not bit-identical to Remotion.** `error_rate` decisions use
  `crate::motion::random`, this library's shared xorshift32 port of Remotion's `random`, with the
  source's exact keys (`"{seed}-{str_index}-{i}"` and `"{seed}-wrong-{str_index}-{i}"`). Renders
  are therefore reproducible frame for frame, and the *timing* of a typo is identical to the
  source; whether it picks the same wrong character as a given Remotion build depends on that
  shared hash, not on this component. `error_rate` is not clamped: a value above `1.0` makes every
  character a typo (which is useful in tests).
* **Nothing else was lost.** No blur, 3D, blend mode or canvas-only effect is used by the source
  component.
* **Style prop.** The source's `style` override is not exposed; see the fixed styling table above.

## Credit

Ported from `TypeWriter.tsx` in the Remotion component library by the **remocn** authors
(<https://github.com/Remocn/remocn>, MIT). The timing constants, the typo alphabet, the blink
curve and the defaults are theirs; this is a reimplementation against fframes' SVG + Rust API and
is not affiliated with Remotion.
