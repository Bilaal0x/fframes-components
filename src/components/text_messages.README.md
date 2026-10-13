# Text Messages

`text-messages` — an animated phone chat conversation, one message at a time, with typing
indicators and a delivery receipt.

Ported from `TextMessages.tsx` (see **Credit** at the bottom).

## What it does

The whole thread plays out from the local frame index. Every beat below is the default
configuration (8 messages, `start_delay: 30`, `gap_frames: 20`, `typing_frames: 55`), at 30 fps.

| frames | beat |
| --- | --- |
| 0 – 14 | The status bar (from `-14px`), contact header (from `-10px`) and input bar (from `+12px`) fade in and rise/settle into place. The transforms use `ease-out-cubic`, the opacities are linear. |
| 8 – 22 | The "Today 6:42 PM" divider pill fades in. |
| 30 – 85 | A "typing…" bubble shows for the first incoming message. Its three dots each hop 6px with a 28.5-frame period, offset by 1.05 rad, so they ripple left to right and rest between hops. The header status reads "typing…" for exactly this window. |
| 85 | Bubble 1 pops in: a spring (damping 15, mass 0.72, stiffness 170) carries a 32px rise and a `0.82 → 1.0` scale, while a 6-frame `ease-out-cubic` fade brings it in. It settles around frame 105. |
| 105 | Bubble 2 (`me`, blue, right-aligned) pops in — sent messages never get a typing indicator. |
| 125 – 180 | Typing indicator for bubble 3, then bubble 3 at 180. |
| 200 | Bubble 4 (`me`). |
| 220 – 275 | Typing indicator for bubble 5, then bubble 5 at 275. Bubble 5 is the two-line `"Also… we got you a cake.\nDon't ask."`. |
| 295 | Bubble 6 (`me`), two lines. |
| 315 – 370 | Typing indicator for bubble 7, then bubble 7 at 370. |
| 390 – 414 | The last bubble (`me`) pops in. |
| 422 – 434 | "Delivered" fades in under that bubble, 32 frames after it mounted. |
| 434 – 509 | Hold. The composition ends at frame 509. |

Each incoming message is preceded by its own typing indicator; each sent message is not. The
typing indicator replaces the bubble while it is up — only one of the two is ever drawn.

## Options

| option | type | default | meaning |
| --- | --- | --- | --- |
| `messages` | `Vec<ChatMessage>` | the source's 8-message demo conversation | Conversation, in chronological order. |
| `contact_name` | `String` | `"Maya"` | Contact display name shown in the header. |
| `avatar_initials` | `Option<String>` | `None` | Initials in the avatar circle. `None` derives them from `contact_name` (first letter of each word, uppercased, truncated to 2 characters). |
| `typing_frames` | `usize` | `55` | Frames a "typing…" indicator shows before each incoming message. `0` disables typing indicators entirely. |
| `gap_frames` | `usize` | `20` | Frames between consecutive bubbles. |
| `start_delay` | `usize` | `30` | Frames before the conversation starts. |
| `anchor` | `Anchor` | `Anchor::Top` | Where the thread grows from. |
| `reduced_motion` | `bool` | `false` | Collapse the 32px rise and the `0.82 → 1.0` scale; keep every fade. |
| `fit_to_thread` | `bool` | `true` | Scale the whole conversation down (never up) so it fits between the header and the input bar. See **Fidelity notes**. |
| `scale` | `f32` | `1.0` | Multiplies every source px metric (on top of the canvas scaling). |

Nested option types:

| type | fields | meaning |
| --- | --- | --- |
| `ChatMessage` | `from: ChatSender`, `text: String` | One message. `\n` inside `text` starts a new line; long lines are wrapped at 76% of the thread width. |
| `ChatSender` | `Them`, `Me` | Who sent it: `Them` is left-aligned in a white card with a hairline border, `Me` is right-aligned in primary blue. |
| `Anchor` | `Top`, `Bottom` | `Top` stacks downward from the header (the source default); `Bottom` measures the stack and pins it above the input bar. |
| `ChatTiming` | `start_delay: usize` (30), `gap_frames: usize` (20), `typing_frames: usize` (55) | The timing triple the pure schedule functions read. |

### Ported helpers

Everything the animation computes is available as a pure function, so the timing can be reused or
tested without a renderer:

| function | what |
| --- | --- |
| `compute_chat_schedule(&[ChatMessage], ChatTiming) -> Vec<ChatScheduleEntry>` | One entry per message with its `entry` (mount) frame and `typing_from` (indicator mount) frame. |
| `get_text_messages_duration(&[ChatMessage], ChatTiming) -> usize` | Total frame count including the end hold. |
| `bubble_entrance(local_frame, fps, reduced) -> BubbleEntrance` | `{ opacity, translate_y, scale }` for a bubble. |
| `typing_dot_lift(frame, fps, dot_index) -> f32` | The dot's upward hop in source px, `0..=-6`. |
| `chrome_fade(frame, from_y) -> ChromeFade` | `{ opacity, translate_y }` for one chrome row. |
| `delivered_opacity(local_frame) -> f32` | The "Delivered" receipt's opacity. |
| `typing_active(&[ChatScheduleEntry], frame) -> bool` | Whether the header should read "typing…". |
| `last_mine_index(&[ChatMessage]) -> Option<usize>` | The message that gets the receipt. |
| `derive_initials(&str) -> String` | The avatar initials derived from a contact name. |

Constants: `ENTRANCE_FRAMES = 24`, `DELIVERED_LAG_FRAMES = 8`, `DELIVERED_FADE_FRAMES = 12`,
`END_HOLD_FRAMES = 75`, `REFERENCE_VMIN = 10.8`.

## How to render it

```sh
# the native 9:16 aspect (recommended for this component)
cargo run --release --features text-messages,cli -- render --portrait \
    --components text-messages render -o story.mp4

# landscape 16:9 — the component scales itself down to fit
cargo run --release --features text-messages,cli -- render --components text-messages -o out.mp4

# as part of a selection
cargo run --release --features text-messages,word-by-word,cli -- render \
    --components word-by-word,text-messages -o out.mp4

# inspect / preview / a still frame
cargo run --release --features text-messages,cli -- --portrait --components text-messages snapshot
cargo run --release --features text-messages -- --portrait --components text-messages --list
```

## Duration

```text
duration = start_delay + END_HOLD_FRAMES                                    if messages is empty
duration = last.entry + ENTRANCE_FRAMES + delivered_tail + END_HOLD_FRAMES  otherwise
```

where `last.entry` is the mount frame of the last message from
`compute_chat_schedule`, and

```text
delivered_tail = DELIVERED_LAG_FRAMES + DELIVERED_FADE_FRAMES (= 20)  if any message is from Me
               = 0                                                    otherwise
```

With the defaults: the last bubble mounts at frame 390, so
`390 + 24 + 20 + 75 = 509` frames — **16.97 s at 30 fps**.

## Canvas: use 9:16

**This component was authored for 1080x1920, so the portrait canvas is its native aspect.** On the
9:16 canvas it needs no fit-down at all: `vmin` is `1080/100 = 10.8`, so every source px metric
comes out at exactly `1.0` and the whole demo conversation fits at the source's own scale.

```sh
# the native aspect — recommended
cargo run --release -- render --portrait --components text-messages render -o story.mp4

# landscape still works; the component scales itself down to fit the 737px thread viewport
cargo run --release -- render --components text-messages render -o wide.mp4
```

| canvas | `vmin` | thread viewport | what happens |
| --- | --- | --- | --- |
| `--portrait` (1080x1920) | 10.8 | 1577px | Exact source metrics. The 985px conversation sits in the thread with room to spare. |
| default (1920x1080) | 10.8 | 737px | `fit_to_thread` scales the layout by ≈`0.79` so the conversation still fits. |

The timeline is identical on both canvases (509 frames): the component's code does not change, only
the geometry it lays out against. `inspect` reports no `text_off_canvas` on either.

In Rust, the canvas is a type:

```rust
use fframes_components::{ComponentsVideo, Vertical};

let tall: Vertical = ComponentsVideo::from_spec("text-messages")?;
```

## Fidelity notes

* **Canvas.** Every source px metric is multiplied by `ctx.vmin() / 10.8`, which is exactly `1.0`
  when the canvas' short edge is 1080 — true for both 1920x1080 and 1080x1920. The chrome is laid
  out against `ctx.width()`/`ctx.height()`, so on the portrait canvas the phone UI is a full-bleed
  portrait column, exactly as the source composes it.
* **`fit_to_thread` is an addition, and it only matters on landscape.** On the source's canvas the
  thread viewport is 1577px tall and the demo conversation (about 985px) fits easily. On 1920x1080
  the viewport is only 737px, so at 1:1 the last bubbles and the "Delivered" receipt would run off
  the bottom (`inspect` reports them as `text_off_canvas`). With `fit_to_thread: true` (the
  default) every metric is additionally multiplied by `min(1, canvas_height / (stack + chrome))`,
  computed from *every* configured message, so the thread always fits and the scale never breathes
  as bubbles arrive. The fit **never scales up**, so on the portrait canvas it is inert and the
  source metrics are exact. Set `fit_to_thread: false` to disable it entirely. The fit will not
  scale below 20% of the nominal unit, so a genuinely enormous conversation on a small canvas still
  overflows rather than collapsing into illegibility.
* On landscape the fit lands on unit ≈ `0.79`: the status bar is 50px, the header 107px, the input
  bar 112px, the bubble text 24px, and the 985px conversation sits inside the 810px thread with
  "Delivered" just above the input bar. On portrait the unit is `1.0` and every metric is the
  source value. `inspect` reports no `text_off_canvas` on either.
* With the portrait viewport (1577px) and the default eight-message conversation, the thread fills
  roughly the top 70% of the canvas. That headroom is intentional — it is what a chat thread looks
  like, and adding more messages grows into it.
* **Per-corner radii.** SVG `<rect>` takes a single `rx`, so the source's
  `borderBottomRightRadius: 8` (Mine) and `borderBottomLeftRadius: 8` (Them) — the "tail" corner —
  cannot be reproduced. Every bubble is drawn with a uniform 26px radius.
* **Shadows.** `box-shadow: 0 1px 2px rgba(20, 20, 20, 0.06)` on the bubbles and the typing
  indicator is not drawn. A soft shadow needs a blur filter and this library does not use SVG
  filters; nothing is faked in its place.
* **Fonts.** The source uses the system UI stack (`-apple-system, …`). This library embeds one
  face (DM Sans), so the whole UI renders in DM Sans, `font-weight: 600` maps to the same single
  weight as 400, and the italic/serif stacks of other components do not apply here.
* **Emoji.** The demo conversation contains 🎉 🐶 😏. The embedded face has no emoji glyphs, so
  they measure as a fallback box and render as tofu; `inspect` reports
  `No fonts with a X character were found` for each one. Replace them in `messages` for a clean
  render.
* **`line-height: normal`.** Spans without an explicit line height (the status bar clock, the
  header name/status, the "Delivered" receipt, the "Message" placeholder, the "+") are laid out
  with a 1.2 line box, which is the browser default for this kind of UI font but not a value the
  source states.
* **Typing dot colour.** `#6e6a6394` is emitted as the muted foreground at 58% opacity rather than
  as an 8-digit hex, which not every SVG renderer accepts.
* **Avatar gradient.** `linear-gradient(135deg, #3072db 0%, #7c5cff 100%)` becomes an
  `objectBoundingBox` `linearGradient` from `(0,0)` to `(1,1)`.
* **Delivery receipt alignment.** CSS `text-align: right` is an SVG `text-anchor="end"`, placed at
  the bubble's right edge minus the source's 6px.
* **`anchor: Bottom`** packs the divider pill with the bubbles, exactly as the source's
  `justify-content: flex-end` flex column does, so the divider sits directly above the first
  visible bubble rather than staying under the header.
* **Measurement happens once.** The wrapped bubbles and the divider are measured on the first
  frame and cached in a `OnceLock`; the canvas size and the configuration are constant for a
  video, so later frames reuse them.
* The typing indicator carries no `margin-bottom` in the source and none here; the 22px gap
  belongs to the message bubble only.

## Credit

Ported from `TextMessages.tsx` in <https://github.com/Remocn/remocn> (MIT). The animation, its
timing constants, easing vocabulary, palette and defaults are the source's; this is a
reimplementation against fframes' SVG + Rust API and is not affiliated with Remotion.
