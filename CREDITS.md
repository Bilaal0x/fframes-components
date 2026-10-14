# Credits

`fframes-components` is an independent Rust and SVG reimplementation of animations originally authored for Remotion. Upstream component attribution is listed in the main [README](README.md).

## Libraries

- [Remotion](https://www.remotion.dev/) — framework used by the source implementations.
- [fframes](https://github.com/dmtrKovalenko/fframes), MIT — Rust rendering framework used by this crate.
- [culori](https://github.com/Evercoder/culori), MIT — basis for the Oklch colour paths in `src/ui/color.rs`.
- [roughjs](https://github.com/rough-stuff/rough) 4.6.6, MIT, Copyright (c) 2019 Preet Shihn — its seeded Lehmer random generator, `_line`, `ellipse`, `_computeEllipsePoints` and `_curve` are ported to Rust in `circle-marker`, `crossed-off`, `strike-through` and `text-marker`.
- [@remotion/rough-notation](https://github.com/remotion-dev/remotion/tree/main/packages/rough-notation), MIT, by Jonny Burger and contributors — its annotation recipes (`render-annotation.tsx`: stroke layout, default options and per-stroke progress) are ported to Rust in the same four components. It derives from [rough-notation](https://github.com/rough-stuff/rough-notation), MIT, Copyright (c) 2020 Preet Shihn.

### MIT License (roughjs, rough-notation)

Copyright (c) 2019 Preet Shihn (roughjs)  
Copyright (c) 2020 Preet Shihn (rough-notation)

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Embedded fonts

| Face | Weights | Source | License |
| --- | --- | --- | --- |
| DM Sans | Medium | [Google Fonts](https://fonts.google.com/specimen/DM+Sans) | OFL 1.1 |
| Inter | Regular, Medium, SemiBold, Bold | [rsms/inter](https://github.com/rsms/inter) v4.1 | OFL 1.1 |
| Montserrat | Bold, ExtraBold, Black | [JulietaUla/Montserrat](https://github.com/JulietaUla/Montserrat) | OFL 1.1 |

## Embedded pictures

Pictures are embedded so renders remain deterministic and offline.

| Files | Used by | Source |
| --- | --- | --- |
| `media/avatar1.png`, `media/avatar2.png` | `channel-thread` | demo art supplied for this port |
| `media/placeholder-01.png` through `media/placeholder-08.png` | `logo-assemble`, `reel-collage` | original abstract placeholder art created for this crate |

## Component sources

| Component | Source |
| --- | --- |
| `youtube-subscribe-nudge` | Ported from the `YouTubeSubscribeNudge` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/youtube/youtube-subscribe-nudge) (`packages/docs/elements/youtube/youtube-subscribe-nudge/youtube-subscribe-nudge.tsx`) |
| `youtube-end-card` | Ported from the `YouTubeEndCard` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/youtube/youtube-end-card) (`packages/docs/elements/youtube/youtube-end-card/youtube-end-card.tsx`). Its LinkedIn, X, Instagram and link glyphs are [Font Awesome Free](https://fontawesome.com/license/free) brand and solid icons (CC BY 4.0), with path data taken from that source |
| `youtube-comment-highlight` | Ported from the `YouTubeCommentHighlight` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/youtube/youtube-comment-highlight) (`packages/docs/elements/youtube/youtube-comment-highlight/youtube-comment-highlight.tsx`). Its thumbs-up path data is taken from that source |
| `circle-marker` | Ported from the `CircleMarker` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/text/circle-marker) (`packages/docs/elements/text/circle-marker/circle-marker.tsx`). Its rough ring is ported from `@remotion/rough-notation`'s `circle` annotation and roughjs's `ellipse` |
| `crossed-off` | Ported from the `CrossedOff` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/text/crossed-off) (`packages/docs/elements/text/crossed-off/crossed-off.tsx`). Its rough cross is ported from `@remotion/rough-notation`'s `crossed-off` annotation and roughjs's `line` |
| `spinning-text-wheel` | Ported from the `SpinningTextWheel` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/text/spinning-text-wheel) (`packages/docs/elements/text/spinning-text-wheel/spinning-text-wheel.tsx`) |
| `text-marker` | Ported from the `TextMarker` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/text/text-marker) (`packages/docs/elements/text/text-marker/text-marker.tsx`). Its rough highlight is ported from `@remotion/rough-notation`'s `highlight` annotation and roughjs's `line` |
| `strike-through` | Ported from the `StrikeThrough` element in [Remotion Elements](https://github.com/remotion-dev/remotion/tree/main/packages/docs/elements/text/strike-through) (`packages/docs/elements/text/strike-through/strike-through.tsx`). Its rough strike is ported from `@remotion/rough-notation`'s `strike-through` annotation and roughjs's `line` |
