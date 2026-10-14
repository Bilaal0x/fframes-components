# Credits

`fframes-components` is an independent Rust and SVG reimplementation of animations originally authored for Remotion. Upstream component attribution is listed in the main [README](README.md).

## Libraries

- [Remotion](https://www.remotion.dev/) — framework used by the source implementations.
- [fframes](https://github.com/dmtrKovalenko/fframes), MIT — Rust rendering framework used by this crate.
- [culori](https://github.com/Evercoder/culori), MIT — basis for the Oklch colour paths in `src/ui/color.rs`.

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
