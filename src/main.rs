//! `fframes-components`: pick components, list them, render them on the GPU.
//!
//! ```text
//! fframes-components --list
//! fframes-components --components type-writer,glitch-cycle -o out.mp4
//! fframes-components --components all -o everything.mp4
//! fframes-components --components text-reveal --renderer gpu frame 3s
//! ```
//!
//! Every standard fframes command is available: `render` (the default), `frame`, `strip`,
//! `onion`, `svg`, `timeline`, `inspect`, `snapshot`, `audio` and `preview`. Run with `--help`.

use clap::Args;
use fframes::{
    EncoderOptions, RenderOptions, StaticMediaProvider,
    cli::{self, clap},
};
use fframes_components::{
    CANVAS_FPS, CANVAS_HEIGHT, CANVAS_WIDTH, Canvas, ComponentsMedia, ComponentsVideo, Landscape,
    Portrait, catalog,
};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, vulkan::SkiaVulkanCtx,
};
use std::process::ExitCode;

/// Flags of this binary, next to the standard ones of `fframes::cli`.
#[derive(Debug, Args)]
struct ComponentsArgs {
    /// Which components to render: `all`, or a comma-separated list of ids.
    #[arg(long, default_value = "all", global = true)]
    components: String,

    /// List every component compiled into this build, then exit.
    #[arg(long, global = true)]
    list: bool,

    /// Render on the 9:16 portrait canvas (1080x1920) instead of 16:9 landscape (1920x1080).
    #[arg(long, global = true)]
    portrait: bool,
}

fn main() -> ExitCode {
    let args = cli::parse::<ComponentsArgs>();
    let selection = args.app.components.clone();

    if args.app.list {
        print_catalog();
        return ExitCode::SUCCESS;
    }

    // The canvas is a type parameter, because fframes reads the render target from the `Video`
    // trait's associated constants. `fframes::Video` is implemented per concrete canvas, so the
    // two orientations are dispatched concretely while sharing one drawing body.
    macro_rules! run_on {
        ($canvas:ty) => {{
            match ComponentsVideo::<$canvas>::from_spec(&selection) {
                Ok(video) => run(video, args),
                Err(err) => {
                    eprintln!("error: {err}");
                    ExitCode::FAILURE
                }
            }
        }};
    }

    if args.app.portrait {
        run_on!(Portrait)
    } else {
        run_on!(Landscape)
    }
}

/// Render `video` through the standard fframes command line on the GPU.
///
/// Takes the concrete canvas type: `fframes::Video` is implemented per canvas, so a generic
/// `C: Canvas` bound does not hold without the trait gaining a blanket impl.
fn run<C: Canvas>(video: ComponentsVideo<C>, args: cli::Cli<ComponentsArgs>) -> ExitCode
where
    ComponentsVideo<C>: fframes::Video + Send + Sync,
{
    if video.components().is_empty() {
        eprintln!(
            "error: no components selected. This build has {} compiled in; pass --list to see them.",
            catalog().len()
        );
        return ExitCode::FAILURE;
    }

    let media = ComponentsMedia::prepare().expect("embedded media");
    let pipeline = SkiaPipelineConfig {
        concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
        ..Default::default()
    };
    let gpu = SkiaVulkanCtx::new(
        <ComponentsVideo<C> as fframes::Video>::WIDTH,
        <ComponentsVideo<C> as fframes::Video>::HEIGHT,
    )
    .expect("Vulkan GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[("crf", "20"), ("preset", "medium"), ("tune", "animation")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .args(args)
    .backend(SkiaFFramesRenderer::new_vulkan(&gpu, pipeline).expect("skia renderer"))
    .preview(fframes_native_player::cli_preview)
    .run()
}

/// Print every component this build contains, with the Cargo feature that enables it.
fn print_catalog() {
    let entries = catalog();
    if entries.is_empty() {
        println!(
            "No components are compiled into this build.\n\
             Enable at least one component feature, or `--features full` for all of them."
        );
        return;
    }

    println!(
        "{} components compiled into this build ({}x{} @ {} fps):\n",
        entries.len(),
        CANVAS_WIDTH,
        CANVAS_HEIGHT,
        CANVAS_FPS
    );
    for entry in entries {
        let meta = entry.meta();
        println!("  {:<24} {}", meta.id, meta.name);
        println!("  {:<24} {}", "", meta.summary);
        println!("  {:<24} cargo feature: {}\n", "", meta.feature);
    }
    println!("Render a selection:");
    println!("  fframes-components --components <id>,<id> -o out.mp4");
    println!("  fframes-components --components all -o everything.mp4");
    println!("  fframes-components --components <id> --renderer gpu inspect");
}
