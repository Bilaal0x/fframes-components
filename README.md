# fframes-components

A curated library of reusable animated components for [fframes](https://crates.io/crates/fframes).
Every component is behind its own Cargo feature, so consumers compile only what they select.

The crate is configured for publication on crates.io.

## Add one component

Add only `logo-assemble` from crates.io:

```toml
[dependencies]
fframes-components = {
    version = "0.1.0",
    default-features = false,
    features = ["logo-assemble"]
}
```

Or use Cargo:

```sh
cargo add fframes-components@0.1.0 --no-default-features --features logo-assemble
```

Use the selected component directly:

```rust
use fframes_components::{ComponentsVideo, Horizontal};
use fframes_components::components::showcase::logo_assemble::{
    LogoAssemble,
    LogoAssembleConfig,
};

let logo = LogoAssemble::new(LogoAssembleConfig {
    brand_name: "my-brand".to_owned(),
    ..LogoAssembleConfig::default()
});

let video: Horizontal = ComponentsVideo::from_components(vec![Box::new(logo)]);
```

Unused component modules are excluded at compile time. The default feature set is empty.

## Components

- `answer-highlight`
- `bottom-up-letters`
- `carousel-3d`
- `channel-thread`
- `check-cycle`
- `cinematic-text-swap`
- `confetti`
- `cursor-flyover`
- `echo-stack`
- `glitch-cycle`
- `inflating-text`
- `lines-to-text`
- `logo-assemble`
- `multi-text-typewriter`
- `per-character-rise`
- `period-drop`
- `prompt-zoom`
- `reel-collage`
- `short-slide-right`
- `slide-in-text`
- `soft-blur-in`
- `spring-scale-in`
- `squeeze-in`
- `staggered-fade-up`
- `text-build`
- `text-highlight`
- `text-messages`
- `text-reveal`
- `text-rewrite`
- `text-swap`
- `text-swell`
- `top-down-letters`
- `tracking-in`
- `type-writer`
- `typing-code-block`
- `word-by-word`
- `word-flip`
- `word-gather`

`cinematic-text-swap` is the preset-driven cinematic transition. `text-swap` is the original lightweight text-swap component. The distinct IDs avoid a feature and catalog collision.

Enable every curated component with the `full` feature.

## Optional CLI

The `cli` feature adds the standalone renderer and preview binary:

```sh
cargo run --release \
  --no-default-features \
  --features cli,logo-assemble \
  -- --components logo-assemble render -o logo.mp4
```

Library consumers do not need the CLI, Vulkan renderer, or native player dependencies.

## Demo site

The static component gallery lives in [`docs/`](docs/) and uses only repository-relative paths.
To preview it locally:

```sh
cd docs
python -m http.server 8000
```

Then open <http://localhost:8000>.

To deploy it with GitHub Pages:

1. Commit and push the `docs/` directory to the repository's default branch.
2. Open the GitHub repository and go to **Settings → Pages**.
3. Under **Build and deployment**, choose **Deploy from a branch**.
4. Select the default branch, choose the `/docs` folder, and click **Save**.
5. Wait for the Pages deployment to finish. The site will be available at
   `https://bilaal0x.github.io/fframes-components/`.

Future changes to `docs/` deploy automatically whenever that branch is pushed.

## Credits

The Rust implementation is MIT licensed. Components were ported from [remocn](https://github.com/Remocn/remocn) and [snapcn](https://github.com/shadcn-labs/snapcn), with the upstream namespaces removed from the public component IDs. Font and embedded demo-image details are listed in [CREDITS.md](CREDITS.md).
