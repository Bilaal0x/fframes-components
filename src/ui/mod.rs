//! Shared UI primitives used by the cinematic component collection.
//!
//! The modules provide concrete theme tokens, colour interpolation, timeline helpers, reference
//! canvas transforms, measured text placement, and SVG blur approximations.

pub mod blur;
pub mod color;
pub mod reference;
pub mod text;
pub mod theme;
pub mod timeline;

pub use color::{
    Oklch, Rgba, clamp_chroma, format_rgb, mix_oklch, oklch_to_rgb, parse_color, rgb_to_oklch,
    to_hex, with_alpha,
};
pub use reference::ReferenceFrame;
pub use text::{
    baseline_for_centred_box, default_face, resolve_face, text_node, text_node_left, tracked_width,
    tracked_width_at, unit,
};
pub use theme::{Mode, UiTheme};
pub use timeline::{
    StateTransition, Step, clamp01, current_state, reveal_count, revealed_text, state_transition,
};
