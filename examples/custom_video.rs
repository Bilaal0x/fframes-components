//! Assemble a video from a chosen set of components, in code.
//!
//! ```text
//! # landscape 16:9 (the default)
//! cargo run --release --features cli --example custom_video -- --components type-writer,glitch-cycle
//!
//! # portrait 9:16
//! cargo run --release --features cli --example custom_video -- --portrait --components text-messages
//!
//! # describe the assembled timeline and exit
//! cargo run --release --features cli --example custom_video -- --describe
//! ```
//!
//! This uses the library API rather than the `fframes-components` binary. It shows the three
//! selection styles (a spec string, resolved catalog entries, or your own `Component` values), how
//! to pick the canvas, and where to point the components at a different font family.

use fframes::{
    EncoderOptions, RenderOptions, StaticMediaProvider,
    cli::{self, clap},
};
use fframes_components::{
    Canvas, ComponentsMedia, ComponentsVideo, Fonts, Horizontal, Vertical, catalog, selection,
};
use std::process::ExitCode;

#[derive(Debug, clap::Args)]
struct Args {
    /// Which components to render: `all`, or a comma-separated list of ids.
    #[arg(long, default_value = "all", global = true)]
    components: String,

    /// Build on the 9:16 portrait canvas (1080x1920) instead of 16:9 landscape (1920x1080).
    #[arg(long, global = true)]
    portrait: bool,

    /// Print the assembled timeline and exit without rendering.
    #[arg(long, global = true)]
    describe: bool,
}

fn main() -> ExitCode {
    let args = cli::parse::<Args>();
    let spec = args.app.components.clone();

    // The canvas is a *type*, not a runtime setting: fframes reads the render target from the
    // `Video` trait's associated constants, which cannot vary per instance. So the two
    // orientations are two types, `Horizontal` and `Vertical`.
    if args.app.portrait {
        let tall: Vertical = match ComponentsVideo::from_spec(&spec) {
            Ok(video) => video,
            Err(err) => return report(&err.to_string()),
        };
        describe(&tall, &spec);
        if args.app.describe {
            return ExitCode::SUCCESS;
        }
        render(tall, args)
    } else {
        let wide: Horizontal = match ComponentsVideo::from_spec(&spec) {
            Ok(video) => video,
            Err(err) => return report(&err.to_string()),
        };
        describe(&wide, &spec);
        if args.app.describe {
            return ExitCode::SUCCESS;
        }
        render(wide, args)
    }
}

/// Log what was selected, using the library's catalog rather than the CLI.
fn describe<C: Canvas>(video: &ComponentsVideo<C>, spec: &str) {
    println!("{}", video.timeline_report());

    // Style 2 — resolve the spec to catalog entries yourself, for example to log what was picked
    // or to build only a subset.
    match selection(spec) {
        Ok(entries) => {
            let ids: Vec<_> = entries.iter().map(|entry| entry.meta().id).collect();
            println!(
                "selection resolved to {} entries: {}",
                ids.len(),
                ids.join(", ")
            );
        }
        Err(err) => eprintln!("warning: {err}"),
    }

    println!(
        "this build has {} of the library's components compiled in",
        catalog().len()
    );

    // Style 3 — build a video from components you constructed yourself. Any type implementing
    // `fframes_components::Component` works, including ones defined outside this crate; the crate
    // README has a worked example.
    //
    //     let mine: Vec<Box<dyn fframes_components::Component>> =
    //         vec![Box::new(MyComponent::default())];
    //     let video = ComponentsVideo::from_components(mine);

    // To draw the mono components in a real monospace face, include one in this crate's media
    // provider and set the family name that is inside the file:
    //
    //     let video = video.with_fonts(Fonts { mono: "JetBrains Mono".to_owned(), ..Default::default() });
    //
    // `Fonts::default()` uses the face embedded in `fframes-components`, which is what the
    // components measure against, so measurement and drawing stay in step.
    let _fonts = Fonts::default();
}

/// Hand the assembled video to `fframes::cli` and run the requested command.
fn render<C: Canvas>(video: ComponentsVideo<C>, args: cli::Cli<Args>) -> ExitCode
where
    ComponentsVideo<C>: fframes::Video + Send + Sync,
{
    let media = ComponentsMedia::prepare().expect("embedded media");
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
    .run()
}

/// Print an error and fail.
fn report(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}
