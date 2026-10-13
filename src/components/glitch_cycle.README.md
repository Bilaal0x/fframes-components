# Glitch Cycle

Cycles a list of strings with a scramble transition between them.

## What it does

Each string owns one slot of `item_duration` frames. Inside a slot, a keyframe envelope over
`glitch` says what fraction of the characters is scrambled, sampled evenly across the slot.

With the defaults (`item_duration 60`, `glitch [1, 0, 0, 0.1, 0, 0, 1]`, `delay 0`,
4 strings, 240 frames total):

| frame | what is on screen |
| --- | --- |
| 0 | Slot 0 (`BOOT SEQUENCE`) enters at full scramble, fading in over the first 5 frames. |
| 0–10 | The scramble decays from 1.0 to 0.0: the text resolves out of noise. |
| 10–20 | Clean, no flicker. |
| 20–30 | A residual flicker: a keyframe of 0.1 scrambles ~10% of the characters. |
| 30–40 | Clean again. |
| 40–50 | Clean (the fifth keyframe is 0.0). |
| 50–60 | The scramble climbs back to 1.0, so slot 1 begins fully scrambled. |
| 60 | Slot 1 (`CALIBRATING`) starts — the swap happens inside a full scramble. |
| 60–240 | The same envelope repeats for slots 2 and 3, then the cycle wraps to slot 0. |

Nothing else moves: no travel, no scale, no second competing motion. The flicker *is* the motion.

Because every scramble seed contains the frame, the glyphs re-roll on every frame — deterministic
for a given frame, but flickering. Two renders of the same frame are identical.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `texts` | `Vec<String>` | `["BOOT SEQUENCE", "CALIBRATING", "ALL SYSTEMS GO", "READY"]` | Strings cycled through, one slot each. |
| `item_duration` | `usize` | `60` | Frames per text slot. Clamped to at least 1. |
| `glitch` | `Vec<f32>` | `[1.0, 0.0, 0.0, 0.1, 0.0, 0.0, 1.0]` | Scramble amount keyframes, sampled evenly across each slot. `1.0` scrambles every character, `0.0` leaves the text clean. |
| `delay` | `usize` | `0` | Frames to wait before cycling starts. |
| `font_size` | `f32` | `64.0` | Font size in px. |
| `font_weight` | `u16` | `700` | Font weight. |
| `color` | `String` | `"#e6edf3"` | Text fill. |
| `tracking_em` | `f32` | `0.02` | Letter spacing in `em`. |

Fixed rendering choices: the family is the monospaced one (`Fonts::mono`, `"DM Sans"` by default),
the background is `#0d1117`, the string is centred horizontally and vertically on the canvas, and
the scramble pool is the source's 91-character `GLITCH_CHARS`:

```text
ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?/~`
```

## How to render it

```sh
# just this component
cargo run --release --features cli -- render --components glitch-cycle -o out.mp4

# as part of a selection
cargo run --release --features cli -- render --components text-reveal,glitch-cycle -o out.mp4

# inspect / preview / a still frame
cargo run --release --features cli -- --components glitch-cycle snapshot
cargo run --release --features cli -- --components glitch-cycle --list
```

## Duration

```text
glitch_cycle_duration(texts.len(), item_duration, delay)
    = max(1, delay + texts.len() * max(1, item_duration))
```

The source has no duration helper (it is an unconditional component), so the port renders one pass
over every string. With the defaults that is `0 + 4 * 60 = 240` frames — **8.00 s at 30 fps**.

`delay` frames of blank output come first: the scramble and the opacity ramp both key off
`relativeFrame = max(0, frame - delay)`, exactly as the source does.

## Fidelity notes

- **One text node, not one span per character.** The source builds a string and joins it; the port
  emits exactly one centred `<text>` node whose content is that string. Spaces are preserved in the
  string (the `whiteSpace: "pre"` equivalent); the svgr macro rejects `xml:space`, so the string is
  passed through untouched and never re-trimmed.
- **The scramble is a per-character decision on the whole string**, not per-node markup: no glyph
  keeps its own position, so there is no per-character node to animate.
- **The family is `Fonts::mono`.** By default that resolves to the face embedded in this crate
  (`DM Sans`), which is *not* monospaced — the source's `fontFamily: monospace`. Point
  `Fonts::mono` at a real monospaced `.ttf` in `media/` for the intended look; nothing else has to
  change.
- **The source's `style` prop is flattened into config fields.** The source takes an arbitrary
  `React.CSSProperties`; this port models the four values its demo uses (`font_size`,
  `font_weight`, `color`, `tracking_em`) as options with the same defaults.
- **Vertical centring uses real font metrics.** The baseline is placed with
  `RenderCtx::baseline_offset` for a `line-height: 1` line box centred on the canvas, so the string
  is optically centred rather than approximated.
- No blur, blend mode or 3D is used by the source, so nothing had to be emulated or dropped.
- The pseudo-random stream is the ported xorshift derivation in `crate::motion::random`, seeded
  with the same strings (`glitch-{frame}-{index}-{charIdx}`, `char-{frame}-{index}-{charIdx}`), so
  the same character is scrambled and the same glyph is picked as in the source. Note that this
  generator is *not* uniform for nearly-identical seeds — short seeds that differ only in their
  last byte produce clustered values. That is the source's behaviour, kept deliberately; do not
  "fix" it, or the scramble threshold stops meaning the same thing.

## Credit

Ported from `GlitchCycle.tsx` in the Remotion component library by the **remocn** authors,
<https://github.com/Remocn/remocn> (MIT). The animation, its defaults, keyframe list, glyph pool
and seeding scheme are theirs; this is a reimplementation against fframes' SVG + Rust API.
