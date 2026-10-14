//! Cinematic UI components.
//!
//! Each module is gated by its own Cargo feature and shares primitives from [`crate::ui`].

#[cfg(feature = "answer-highlight")]
pub mod answer_highlight;
#[cfg(feature = "channel-thread")]
pub mod channel_thread;
#[cfg(feature = "check-cycle")]
pub mod check_cycle;
#[cfg(feature = "circle-marker")]
pub mod circle_marker;
#[cfg(feature = "crossed-off")]
pub mod crossed_off;
#[cfg(feature = "logo-assemble")]
pub mod logo_assemble;
#[cfg(feature = "map-flyover")]
pub mod map_flyover;
#[cfg(feature = "prompt-zoom")]
pub mod prompt_zoom;
#[cfg(feature = "reel-collage")]
pub mod reel_collage;
#[cfg(feature = "spinning-text-wheel")]
pub mod spinning_text_wheel;
#[cfg(feature = "strike-through")]
pub mod strike_through;
#[cfg(feature = "text-highlight")]
pub mod text_highlight;
#[cfg(feature = "text-marker")]
pub mod text_marker;
#[cfg(feature = "text-rewrite")]
pub mod text_rewrite;
#[cfg(feature = "cinematic-text-swap")]
pub mod text_swap;
#[cfg(feature = "word-gather")]
pub mod word_gather;
#[cfg(feature = "youtube-comment-highlight")]
pub mod youtube_comment_highlight;
#[cfg(feature = "youtube-end-card")]
pub mod youtube_end_card;
#[cfg(feature = "youtube-subscribe-nudge")]
pub mod youtube_subscribe_nudge;
