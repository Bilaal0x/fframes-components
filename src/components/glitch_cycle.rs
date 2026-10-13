//! # Glitch Cycle
//!
//! Cycles a list of strings, one per slot, and scrambles the characters through a keyframed
//! glitch envelope. Each slot starts fully scrambled, cleans up towards readable text, holds with
//! a small residual flicker, then scrambles back out — so the cut between two strings always
//! happens inside a full scramble and the swap never reads as a hard cut. A short ease-out
//! opacity ramp brings the first slot in.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `texts` | `Vec<String>` | `["BOOT SEQUENCE", "CALIBRATING", "ALL SYSTEMS GO", "READY"]` | Strings cycled through, one slot each. |
//! | `item_duration` | `usize` | `60` | Frames per text slot. |
//! | `glitch` | `Vec<f32>` | `[1.0, 0.0, 0.0, 0.1, 0.0, 0.0, 1.0]` | Scramble amount keyframes, sampled evenly across each slot. |
//! | `delay` | `usize` | `0` | Frames to wait before cycling starts. |
//! | `font_size` | `f32` | `64.0` | Font size in px. |
//! | `font_weight` | `u16` | `700` | Font weight. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `tracking_em` | `f32` | `0.02` | Letter spacing in `em`. |
//!
//! The family is the monospaced one ([`RenderCtx::mono`]), and the background is `#0d1117`.
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components glitch-cycle -o out.mp4
//! ```
//!
//! ## Credit
//!
//! Ported from `GlitchCycle.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, text_node_centered};
use crate::motion::{DUR_QUICK, EASE_OUT, clamp01, random, sample_keyframes};

/// The glyphs a scrambled position may show, in the source's order.
pub const GLITCH_CHARS: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?/~`";

/// Configuration for [`GlitchCycle`].
#[derive(Debug, Clone, PartialEq)]
pub struct GlitchCycleConfig {
    /// Strings cycled through, one slot of `item_duration` frames each.
    pub texts: Vec<String>,
    /// Frames per text slot.
    pub item_duration: usize,
    /// Glitch amount keyframes, sampled evenly across each slot. `1.0` scrambles every character,
    /// `0.0` leaves the text clean.
    pub glitch: Vec<f32>,
    /// Frames to wait before cycling starts.
    pub delay: usize,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Text fill.
    pub color: String,
    /// Letter spacing in `em`.
    pub tracking_em: f32,
}

impl Default for GlitchCycleConfig {
    fn default() -> Self {
        Self {
            texts: vec![
                "BOOT SEQUENCE".to_owned(),
                "CALIBRATING".to_owned(),
                "ALL SYSTEMS GO".to_owned(),
                "READY".to_owned(),
            ],
            item_duration: 60,
            glitch: vec![1.0, 0.0, 0.0, 0.1, 0.0, 0.0, 1.0],
            delay: 0,
            font_size: 64.0,
            font_weight: 700,
            color: "#e6edf3".to_owned(),
            tracking_em: 0.02,
        }
    }
}

/// Frames elapsed at `frame`, ignoring anything before `delay`.
pub fn relative_frame(frame: f32, delay: usize) -> usize {
    (frame - delay as f32).max(0.0).floor() as usize
}

/// Frames one full pass over every text occupies.
pub fn glitch_cycle_duration(text_count: usize, item_duration: usize, delay: usize) -> usize {
    (delay + text_count * item_duration.max(1)).max(1)
}

/// Index into `texts` of the slot shown at `frame`.
pub fn text_index(frame: f32, text_count: usize, item_duration: usize, delay: usize) -> usize {
    if text_count == 0 {
        return 0;
    }
    (relative_frame(frame, delay) / item_duration.max(1)) % text_count
}

/// Frame offset inside the slot shown at `frame`.
pub fn slot_offset(frame: f32, item_duration: usize, delay: usize) -> usize {
    relative_frame(frame, delay) % item_duration.max(1)
}

/// Slot progress in `0.0..1.0`, the input to the glitch keyframes.
pub fn slot_progress(frame: f32, item_duration: usize, delay: usize) -> f32 {
    slot_offset(frame, item_duration, delay) as f32 / item_duration.max(1) as f32
}

/// Scramble one frame of `text`.
///
/// Spaces pass through untouched; every other character is replaced by a glyph from
/// [`GLITCH_CHARS`] when `random("glitch-{frame}-{index}-{char_idx}") < glitch_amount`. Both seeds
/// include the frame and the slot index, so the scramble is deterministic but re-rolls every
/// frame.
pub fn scramble(text: &str, relative_frame: usize, index: usize, glitch_amount: f32) -> String {
    let pool: Vec<char> = GLITCH_CHARS.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (char_idx, ch) in text.chars().enumerate() {
        if ch == ' ' {
            out.push(' ');
            continue;
        }
        if random(&format!("glitch-{relative_frame}-{index}-{char_idx}")) < glitch_amount {
            let roll = random(&format!("char-{relative_frame}-{index}-{char_idx}"));
            let pick =
                ((roll * pool.len() as f32).floor() as usize).min(pool.len().saturating_sub(1));
            out.push(pool.get(pick).copied().unwrap_or(ch));
        } else {
            out.push(ch);
        }
    }
    out
}

/// The Glitch Cycle component.
#[derive(Debug, Clone, Default)]
pub struct GlitchCycle {
    /// Component options.
    pub config: GlitchCycleConfig,
}

impl GlitchCycle {
    /// Build with the given options.
    pub fn new(config: GlitchCycleConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "glitch-cycle",
            name: "Glitch Cycle",
            summary: "Strings cycle through a keyframed character scramble.",
            feature: "glitch-cycle",
        }
    }
}

impl Component for GlitchCycle {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        glitch_cycle_duration(
            self.config.texts.len(),
            self.config.item_duration,
            self.config.delay,
        )
    }

    fn background(&self) -> Option<&'static str> {
        Some("#0d1117")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        if config.texts.is_empty() {
            return Svgr::empty();
        }

        let frame = ctx.frame();
        let item_duration = config.item_duration.max(1);
        let relative = relative_frame(frame, config.delay);
        let index = text_index(frame, config.texts.len(), item_duration, config.delay);
        let progress = slot_progress(frame, item_duration, config.delay);
        let glitch_amount = sample_keyframes(&config.glitch, progress);

        let current = config.texts.get(index).map_or("", String::as_str);
        let display = scramble(current, relative, index, glitch_amount);

        // The scramble re-rolls glyphs every frame by design, so the block is already flickering;
        // the entrance only needed an opacity ramp.
        let opacity = EASE_OUT.ease(clamp01(relative as f32 / DUR_QUICK as f32));

        let family = ctx.mono();
        let font_size = config.font_size;
        // One line box, `line-height: 1`, centred on the canvas.
        let baseline = ctx.height() / 2.0 - font_size / 2.0
            + ctx.baseline_offset(&family, font_size, font_size);

        text_node_centered(
            &display,
            ctx.width() / 2.0,
            baseline,
            family,
            font_size,
            config.font_weight,
            config.tracking_em,
            config.color.clone(),
            opacity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_covers_every_slot() {
        assert_eq!(glitch_cycle_duration(4, 60, 0), 240);
        assert_eq!(glitch_cycle_duration(4, 60, 15), 255);
        // A zero-item-duration config still advances one frame per slot.
        assert_eq!(glitch_cycle_duration(2, 0, 0), 2);
        // Never zero, so the assembled timeline always has a frame to draw.
        assert_eq!(glitch_cycle_duration(0, 60, 0), 1);
    }

    #[test]
    fn default_duration_is_four_slots() {
        let component = GlitchCycle::default();
        assert_eq!(component.duration(), 240);
        assert_eq!(component.meta().id, "glitch-cycle");
        assert_eq!(component.background(), Some("#0d1117"));
    }

    #[test]
    fn slots_advance_and_wrap() {
        // Slot 0 covers 0..60, slot 1 covers 60..120, and the cycle wraps after 240.
        assert_eq!(text_index(0.0, 4, 60, 0), 0);
        assert_eq!(text_index(59.0, 4, 60, 0), 0);
        assert_eq!(text_index(60.0, 4, 60, 0), 1);
        assert_eq!(text_index(240.0, 4, 60, 0), 0);
        assert_eq!(slot_offset(60.0, 60, 0), 0);
        assert_eq!(slot_offset(75.0, 60, 0), 15);
        assert!((slot_progress(75.0, 60, 0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn delay_shifts_and_never_underflows() {
        assert_eq!(relative_frame(0.0, 20), 0);
        assert_eq!(relative_frame(19.0, 20), 0);
        assert_eq!(relative_frame(20.0, 20), 0);
        assert_eq!(relative_frame(25.0, 20), 5);
        assert_eq!(text_index(5.0, 4, 60, 20), 0);
        assert_eq!(text_index(85.0, 4, 60, 20), 1);
    }

    #[test]
    fn empty_texts_do_not_panic() {
        assert_eq!(text_index(100.0, 0, 60, 0), 0);
        let component = GlitchCycle::new(GlitchCycleConfig {
            texts: Vec::new(),
            ..GlitchCycleConfig::default()
        });
        assert_eq!(component.duration(), 1);
    }

    #[test]
    fn char_pool_matches_the_source() {
        assert_eq!(GLITCH_CHARS.chars().count(), 91);
        assert!(GLITCH_CHARS.starts_with("ABC"));
        assert!(GLITCH_CHARS.ends_with("~`"));
    }

    #[test]
    fn a_clean_slot_is_untouched() {
        assert_eq!(scramble("ALL SYSTEMS GO", 7, 2, 0.0), "ALL SYSTEMS GO");
    }

    #[test]
    fn scramble_is_deterministic_for_a_given_frame() {
        let a = scramble("BOOT SEQUENCE", 12, 0, 1.0);
        let b = scramble("BOOT SEQUENCE", 12, 0, 1.0);
        assert_eq!(a, b);
        // Same length, and the space at index 4 survives.
        assert_eq!(a.chars().count(), "BOOT SEQUENCE".chars().count());
        assert_eq!(a.chars().nth(4), Some(' '));
        // Every glyph was re-rolled from the pool, so it is no longer the source string.
        assert_ne!(a, "BOOT SEQUENCE");
    }

    #[test]
    fn the_scramble_rerolls_with_the_frame() {
        // The seed contains the frame, which is what makes the block flicker.
        let frames: Vec<String> = (0..8).map(|f| scramble("READY", f, 3, 1.0)).collect();
        assert!(frames.iter().any(|f| *f != frames[0]));
    }

    #[test]
    fn glitch_amount_switches_one_character_at_a_time() {
        // The first character of a slot is scrambled exactly when its roll falls under the
        // keyframed amount, and the replacement comes from the `char-` seed.
        let roll = random("glitch-3-1-0");
        assert!(roll > 0.0 && roll < 1.0, "roll {roll}");

        assert_eq!(scramble("C", 3, 1, roll - 0.01), "C");

        let pool: Vec<char> = GLITCH_CHARS.chars().collect();
        let pick =
            ((random("char-3-1-0") * pool.len() as f32).floor() as usize).min(pool.len() - 1);
        assert_eq!(scramble("C", 3, 1, roll + 0.01), pool[pick].to_string());
    }

    #[test]
    fn a_full_scramble_rerolls_every_non_space() {
        let text = "C A";
        let heavy = scramble(text, 3, 1, 1.0);
        assert_eq!(heavy.chars().nth(1), Some(' '));
        assert_eq!(heavy.chars().count(), text.chars().count());
    }

    #[test]
    fn intro_opacity_ramps_over_the_quick_duration() {
        assert!((EASE_OUT.ease(clamp01(0.0 / DUR_QUICK as f32)) - 0.0).abs() < 1e-6);
        assert!((EASE_OUT.ease(clamp01(DUR_QUICK as f32 / DUR_QUICK as f32)) - 1.0).abs() < 1e-6);
    }
}
