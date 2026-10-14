//! The compile-time component registry.
//!
//! Every entry is gated on its own Cargo feature, so a build contains exactly the components you
//! asked for. The list order is the default playback order and the order `--components` uses.
//!
//! Each entry is emitted inside its own block expression. That scoping matters: without it every
//! entry would define a module-level `ENTRY` static, so enabling a second component feature would
//! fail with "the name `ENTRY` is defined multiple times".

use crate::core::{CatalogEntry, Component};

macro_rules! entries {
    ($($feature:literal => $ty:path;)*) => {
        /// Every component compiled into this build, in default playback order.
        pub static REGISTRY: &[&CatalogEntry] = &[
            $(
                #[cfg(feature = $feature)]
                {
                    /// The catalog entry for this component.
                    static ENTRY: CatalogEntry = CatalogEntry {
                        meta_fn: <$ty>::meta_static,
                        build_fn: || Box::new(<$ty>::default()),
                    };
                    &ENTRY
                },
            )*
        ];
    };
}

entries! {
    "per-character-rise" => crate::components::per_char_rise::PerCharacterRise;
    "soft-blur-in" => crate::components::soft_blur_in::SoftBlurIn;
    "bottom-up-letters" => crate::components::bottom_up_letters::BottomUpLetters;
    "top-down-letters" => crate::components::top_down_letters::TopDownLetters;
    "spring-scale-in" => crate::components::spring_scale_in::SpringScaleIn;
    "staggered-fade-up" => crate::components::staggered_fade_up::StaggeredFadeUp;
    "short-slide-right" => crate::components::short_slide_right::ShortSlideRight;
    "squeeze-in" => crate::components::squeeze_in::SqueezeIn;
    "tracking-in" => crate::components::tracking_in::TrackingIn;
    "echo-stack" => crate::components::echo_stack::EchoStack;
    "period-drop" => crate::components::period_drop::PeriodDrop;
    "confetti" => crate::components::confetti::Confetti;
    "word-by-word" => crate::components::word_by_word::WordByWord;
    "slide-in-text" => crate::components::slide_in_text::SlideInText;
    "text-reveal" => crate::components::text_reveal::TextReveal;
    "text-swell" => crate::components::text_swell::TextSwell;
    "text-build" => crate::components::text_build::TextBuild;
    "text-swap" => crate::components::text_swap::TextSwap;
    "inflating-text" => crate::components::inflating_text::InflatingText;
    "type-writer" => crate::components::type_writer::TypeWriter;
    "multi-text-typewriter" => crate::components::multi_text_typewriter::MultiTextTypewriter;
    "typing-code-block" => crate::components::typing_code_block::TypingCodeBlock;
    "word-flip" => crate::components::word_flip::WordFlip;
    "glitch-cycle" => crate::components::glitch_cycle::GlitchCycle;
    "lines-to-text" => crate::components::lines_to_text::LinesToText;
    "text-messages" => crate::components::text_messages::TextMessages;
    "carousel-3d" => crate::components::carousel_3d::Carousel3D;
    "cursor-flyover" => crate::components::cursor_flyover::CursorFlyover;
    "cinematic-text-swap" => crate::components::showcase::text_swap::TextSwap;
    "text-rewrite" => crate::components::showcase::text_rewrite::TextRewrite;
    "word-gather" => crate::components::showcase::word_gather::WordGather;
    "check-cycle" => crate::components::showcase::check_cycle::CheckCycle;
    "text-highlight" => crate::components::showcase::text_highlight::TextHighlight;
    "logo-assemble" => crate::components::showcase::logo_assemble::LogoAssemble;
    "channel-thread" => crate::components::showcase::channel_thread::ChannelThread;
    "reel-collage" => crate::components::showcase::reel_collage::ReelCollage;
    "prompt-zoom" => crate::components::showcase::prompt_zoom::PromptZoom;
    "answer-highlight" => crate::components::showcase::answer_highlight::AnswerHighlight;
    "youtube-subscribe-nudge" => crate::components::showcase::youtube_subscribe_nudge::YouTubeSubscribeNudge;
}

// Assert the shape the registry relies on, so a missing `meta_static` or `Default` is a clear
// error rather than a confusing one at the macro expansion site.
#[allow(dead_code)]
fn _assert_component_shape<T: Component + Default>() {}
