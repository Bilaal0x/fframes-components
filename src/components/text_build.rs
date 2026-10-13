//! # Text Build
//!
//! The words of a line land one at a time and every word already on screen moves to make room
//! for the next one. The first word only drifts a little on the cross axis; each later word
//! enters over `push_duration` from the right (`axis = x`) or from above (`axis = y`), and the
//! placed words travel from the previous centred layout to the next one during the same window,
//! with a soft pulse mid-move. The whole build uses the library's single `EASE_OUT` entrance
//! curve, so the "word arrives" gesture carries the same weight here as in every other reveal.
//!
//! ## Options
//!
//! | option | type | default | meaning |
//! | --- | --- | --- | --- |
//! | `text` | `String` | `"Build The Future Together"` | The line to build. |
//! | `axis` | [`BuildAxis`] | `X` | Layout axis: later words enter from the right (`x`) or above (`y`). |
//! | `first_duration` | `usize` | `10` | Frames before the second word starts pushing in. |
//! | `push_duration` | `usize` | `13` | Frames each later word takes to push in, and placed words to reflow. |
//! | `entry_offset` | `Option<f32>` | `None` | Entry distance on the primary axis, in px; `None` derives `88.0` (`x`) / `28.0` (`y`). |
//! | `gap` | `Option<f32>` | `None` | Gap between placed words, in px; `None` derives `10.0` (`x`) / `12.0` (`y`). |
//! | `entry_blur` | `f32` | `3.5` | Entry softness radius in px. |
//! | `first_drift` | `Option<f32>` | `None` | First word's drift on the cross axis, in px; `None` derives `6.0` (`x`) / `-14.0` (`y`). |
//! | `reflow_blur` | `f32` | `0.8` | Softness pulse radius while placed words reflow, in px. |
//! | `speed` | `f32` | `1.0` | Scales all durations; `> 1` is faster. Non-positive values fall back to `1.0`. |
//! | `delay` | `usize` | `0` | Frames to wait before the build starts. |
//! | `font_size` | `f32` | `72.0` | Font size in px. |
//! | `font_weight` | `u16` | `600` | Font weight. |
//! | `color` | `String` | `"#e6edf3"` | Text fill. |
//! | `tail` | `usize` | `45` | Frames held after the last word lands. |
//! | `simulate_blur` | `bool` | `true` | Draw the low-opacity secondary copy that stands in for a real gaussian blur. |
//!
//! ## Rendering
//!
//! ```text
//! cargo run --release -- --components text-build -o out.mp4
//! ```
//!
//! ## Fidelity
//!
//! Two deviations from the source, both documented in the module `README.md`:
//!
//! - SVG has no gaussian blur this renderer can use, so `filter: blur(Npx)` is **approximated**
//!   by a low-opacity secondary copy displaced along the entry axis, fading out as the word
//!   sharpens. It reads as softness, not as an isotropic gaussian. Set `simulate_blur: false` to
//!   drop the copy.
//! - The source's `translate3d(...)` is a 2D translation in disguise here: SVG cannot do 3D, and
//!   the translate is used linearly, so nothing about the look is lost.
//!
//! ## Credit
//!
//! Ported from `TextBuild.tsx` in <https://github.com/Remocn/remocn> (MIT).

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::{RenderCtx, centered_positions, interpolate3, text_node_centered, words};
use crate::motion::{EASE_OUT, clamp01, lerp};

/// Letter spacing the source hard-codes, in `em`.
pub const LETTER_SPACING_EM: f32 = -0.03;

/// Scale a word starts at and grows out of as it lands.
pub const ENTRY_SCALE: f32 = 0.992;

/// Softness radius at which the secondary copy reaches [`BLUR_GHOST_MAX_ALPHA`], in px.
pub const BLUR_GHOST_REFERENCE_PX: f32 = 10.0;

/// Opacity of the secondary copy at [`BLUR_GHOST_REFERENCE_PX`].
pub const BLUR_GHOST_MAX_ALPHA: f32 = 0.35;

/// Displacement of the secondary copy per px of softness radius.
pub const BLUR_GHOST_SPREAD_RATIO: f32 = 0.6;

/// Below this softness radius no secondary copy is drawn (the source's `blur > 0.01`).
pub const BLUR_GHOST_MIN_PX: f32 = 0.01;

/// Frames held after the last word lands by [`TextBuildConfig::default`].
pub const DEFAULT_TAIL: usize = 45;

/// The axis the build lays words out on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildAxis {
    /// Words occupy width-sized slots and enter from the right. The source's default.
    #[default]
    X,
    /// Words occupy `font_size`-tall slots and enter from above.
    Y,
}

/// Configuration for [`TextBuild`].
#[derive(Debug, Clone, PartialEq)]
pub struct TextBuildConfig {
    /// The line to build.
    pub text: String,
    /// Layout axis: later words enter from the right (`x`) or above (`y`).
    pub axis: BuildAxis,
    /// Frames before the second word starts pushing in.
    pub first_duration: usize,
    /// Frames each later word takes to push in, and placed words to reflow.
    pub push_duration: usize,
    /// Entry distance on the primary axis, in px; `None` derives `88.0` (`x`) / `28.0` (`y`).
    pub entry_offset: Option<f32>,
    /// Gap between placed words, in px; `None` derives `10.0` (`x`) / `12.0` (`y`).
    pub gap: Option<f32>,
    /// Entry softness radius in px.
    pub entry_blur: f32,
    /// First word's drift on the cross axis, in px; `None` derives `6.0` (`x`) / `-14.0` (`y`).
    pub first_drift: Option<f32>,
    /// Softness pulse radius while placed words reflow, in px.
    pub reflow_blur: f32,
    /// Scales all durations; `> 1` is faster. Non-positive values fall back to `1.0`.
    pub speed: f32,
    /// Frames to wait before the build starts.
    pub delay: usize,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Text fill.
    pub color: String,
    /// Frames held after the last word lands.
    pub tail: usize,
    /// Draw the low-opacity secondary copy that stands in for a real gaussian blur.
    ///
    /// This library cannot blur, so the softness is suggested rather than rendered. Turn this
    /// off for a crisp build that only moves.
    pub simulate_blur: bool,
}

impl Default for TextBuildConfig {
    fn default() -> Self {
        Self {
            text: "Build The Future Together".to_owned(),
            axis: BuildAxis::X,
            first_duration: 10,
            push_duration: 13,
            entry_offset: None,
            gap: None,
            entry_blur: 3.5,
            first_drift: None,
            reflow_blur: 0.8,
            speed: 1.0,
            delay: 0,
            font_size: 72.0,
            font_weight: 600,
            color: "#e6edf3".to_owned(),
            tail: DEFAULT_TAIL,
            simulate_blur: true,
        }
    }
}

impl TextBuildConfig {
    /// The entry distance in px, falling back to the axis default.
    pub fn resolved_entry_offset(&self) -> f32 {
        self.entry_offset
            .unwrap_or_else(|| derived_entry_offset(self.axis))
    }

    /// The gap in px, falling back to the axis default.
    pub fn resolved_gap(&self) -> f32 {
        self.gap.unwrap_or_else(|| derived_gap(self.axis))
    }

    /// The first word's cross-axis drift in px, falling back to the axis default.
    pub fn resolved_first_drift(&self) -> f32 {
        self.first_drift
            .unwrap_or_else(|| derived_first_drift(self.axis))
    }
}

/// Entry distance for an axis when the config leaves it unset: `88` (`x`) / `28` (`y`).
pub const fn derived_entry_offset(axis: BuildAxis) -> f32 {
    match axis {
        BuildAxis::X => 88.0,
        BuildAxis::Y => 28.0,
    }
}

/// Gap for an axis when the config leaves it unset: `10` (`x`) / `12` (`y`).
pub const fn derived_gap(axis: BuildAxis) -> f32 {
    match axis {
        BuildAxis::X => 10.0,
        BuildAxis::Y => 12.0,
    }
}

/// First word's cross-axis drift for an axis when the config leaves it unset:
/// `6` (`x`) / `-14` (`y`).
pub const fn derived_first_drift(axis: BuildAxis) -> f32 {
    match axis {
        BuildAxis::X => 6.0,
        BuildAxis::Y => -14.0,
    }
}

/// Total frames until the last word lands (entry only; add a hold tail).
///
/// Zero words take no frames. The source calls this `getTextBuildDuration`; note that it folds
/// in neither `speed` nor `delay`, exactly like the source.
pub fn get_text_build_duration(text: &str, first_duration: usize, push_duration: usize) -> usize {
    let count = words(text).len();
    if count == 0 {
        return 0;
    }
    first_duration + (count - 1) * push_duration
}

/// The frame, scaled by `speed`, at which word `index` starts entering.
///
/// Word 0 starts immediately; each later word starts one push window after the previous one, the
/// first of them at `first`.
pub fn entry_start(index: usize, first: f32, push: f32) -> f32 {
    if index == 0 {
        0.0
    } else {
        first + (index - 1) as f32 * push
    }
}

/// The eased 0..1 progress of a window that starts at `start` and runs for `push` frames.
pub fn entry_progress(f: f32, start: f32, push: f32) -> f32 {
    EASE_OUT.ease(clamp01((f - start) / push.max(f32::EPSILON)))
}

/// A usable speed multiplier: `speed` when positive, `1.0` otherwise.
///
/// The source divides by `speed` unguarded; a non-positive value here would make every frame
/// time infinite or `NaN`, so it falls back to no scaling instead.
pub fn effective_speed(speed: f32) -> f32 {
    if speed > 0.0 { speed } else { 1.0 }
}

/// Centre-of-word offset for word `index` in the layout with `placed` words on screen.
///
/// On axis `x` a word occupies its measured width; on axis `y` every word occupies a
/// `font_size`-tall slot. An index that has no slot resolves to `0.0` rather than panicking.
pub fn layout_pos(
    sizes: &[f32],
    placed: usize,
    index: usize,
    gap: f32,
    font_size: f32,
    axis: BuildAxis,
) -> f32 {
    let positions = if axis == BuildAxis::X {
        centered_positions(sizes, placed, gap)
    } else {
        let slots = vec![font_size; sizes.len()];
        centered_positions(&slots, placed, gap)
    };
    positions.get(index).copied().unwrap_or(0.0)
}

/// Opacity of the secondary softness copy for a softness radius in px.
///
/// Proportional to the radius, capped at [`BLUR_GHOST_MAX_ALPHA`].
pub fn blur_ghost_alpha(blur_px: f32) -> f32 {
    clamp01(blur_px / BLUR_GHOST_REFERENCE_PX) * BLUR_GHOST_MAX_ALPHA
}

/// Displacement of the secondary softness copy for a softness radius in px.
pub fn blur_ghost_spread(blur_px: f32) -> f32 {
    blur_px * BLUR_GHOST_SPREAD_RATIO
}

/// Where the secondary softness copy sits relative to the sharp word.
///
/// It trails the motion, so it is displaced along the axis the words enter on: to the right on
/// axis `x`, above on axis `y`.
pub fn ghost_offset(axis: BuildAxis, blur_px: f32) -> (f32, f32) {
    let spread = blur_ghost_spread(blur_px);
    match axis {
        BuildAxis::X => (spread, 0.0),
        BuildAxis::Y => (0.0, -spread),
    }
}

/// One word drawn centred on the origin of a group placed at `(x, y)` and scaled about it.
#[allow(clippy::too_many_arguments)]
fn word_node(
    word: &str,
    x: f32,
    y: f32,
    scale: f32,
    opacity: f32,
    family: &str,
    font_size: f32,
    font_weight: u16,
    baseline_dy: f32,
    color: &str,
) -> Svgr<'static> {
    let node = text_node_centered(
        word,
        0.0,
        baseline_dy,
        family.to_owned(),
        font_size,
        font_weight,
        LETTER_SPACING_EM,
        color.to_owned(),
        opacity,
    );
    let transform = format!("translate({x:.4} {y:.4}) scale({scale:.6})");
    fframes::svgr!(<g transform={transform}>{node}</g>)
}

/// The Text Build component.
#[derive(Debug, Clone, Default)]
pub struct TextBuild {
    /// Component options.
    pub config: TextBuildConfig,
}

impl TextBuild {
    /// Build with the given options.
    pub fn new(config: TextBuildConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (used by the catalog).
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "text-build",
            name: "Text Build",
            summary: "Words land one by one, pushing the placed words aside.",
            feature: "text-build",
        }
    }
}

impl Component for TextBuild {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        get_text_build_duration(
            &self.config.text,
            self.config.first_duration,
            self.config.push_duration,
        ) + self.config.tail
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let family = ctx.fonts.sans.clone();
        let line = words(&config.text);
        if line.is_empty() {
            return Svgr::empty();
        }

        let size = config.font_size;
        let sizes: Vec<f32> = line
            .iter()
            .map(|word| ctx.text_width(&family, size, word))
            .collect();
        let axis = config.axis;
        let offset = config.resolved_entry_offset();
        let gap = config.resolved_gap();
        let drift = config.resolved_first_drift();

        let speed = effective_speed(config.speed);
        let f = (ctx.frame() - config.delay as f32).max(0.0) / speed;
        let first = config.first_duration as f32 / speed;
        let push = config.push_duration as f32 / speed;

        let center_x = ctx.width() / 2.0;
        let center_y = ctx.height() / 2.0;
        // The words are centred on their own boxes; SVG draws them from a baseline.
        let baseline_dy = ctx.baseline_offset(&family, size, size) - size / 2.0;

        let mut nodes: Vec<Svgr<'_>> = Vec::new();
        for (index, word) in line.iter().enumerate() {
            let start = entry_start(index, first, push);
            if f < start {
                // Not on screen yet.
                continue;
            }

            let entry_p = entry_progress(f, start, push);
            let primary_from = if index == 0 {
                0.0
            } else if axis == BuildAxis::X {
                offset
            } else {
                -offset
            };
            let mut primary = lerp(
                primary_from,
                layout_pos(&sizes, index + 1, index, gap, size, axis),
                entry_p,
            );
            let cross = if index == 0 {
                lerp(drift, 0.0, entry_p)
            } else {
                0.0
            };
            let mut blur = config.entry_blur * (1.0 - entry_p);
            let opacity = entry_p;
            let scale = ENTRY_SCALE + (1.0 - ENTRY_SCALE) * entry_p;

            // Every later word's push window moves this word from the previous centred layout
            // to the next one, with a mid-move softness pulse.
            for later in (index + 1)..line.len() {
                let later_start = entry_start(later, first, push);
                if f < later_start {
                    break;
                }
                let reflow_p = entry_progress(f, later_start, push);
                primary = lerp(
                    layout_pos(&sizes, later, index, gap, size, axis),
                    layout_pos(&sizes, later + 1, index, gap, size, axis),
                    reflow_p,
                );
                blur = config.reflow_blur * interpolate3(reflow_p);
            }

            let x = center_x + if axis == BuildAxis::X { primary } else { cross };
            let y = center_y + if axis == BuildAxis::X { cross } else { primary };

            // The softness stand-in: one trailing copy, behind the sharp word.
            if config.simulate_blur && blur > BLUR_GHOST_MIN_PX {
                let (ghost_x, ghost_y) = ghost_offset(axis, blur);
                nodes.push(word_node(
                    word,
                    x + ghost_x,
                    y + ghost_y,
                    scale,
                    opacity * blur_ghost_alpha(blur),
                    &family,
                    size,
                    config.font_weight,
                    baseline_dy,
                    &config.color,
                ));
            }

            nodes.push(word_node(
                word,
                x,
                y,
                scale,
                opacity,
                &family,
                size,
                config.font_weight,
                baseline_dy,
                &config.color,
            ));
        }

        let nodes = Svgr::from(nodes);
        fframes::svgr!(<g>{nodes}</g>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_helper_matches_the_source() {
        assert_eq!(
            get_text_build_duration("Build The Future Together", 10, 13),
            49
        );
        assert_eq!(get_text_build_duration("Solo", 10, 13), 10);
        assert_eq!(get_text_build_duration("  a   b  ", 10, 13), 23);
        assert_eq!(get_text_build_duration("", 10, 13), 0);
        assert_eq!(get_text_build_duration("   ", 10, 13), 0);
    }

    #[test]
    fn the_helper_agrees_with_the_last_words_entry_start() {
        let text = "Build The Future Together";
        let count = words(text).len();
        // The last word starts at `entry_start(count - 1)` and the helper reports when it lands.
        assert_eq!(
            entry_start(count - 1, 10.0, 13.0) + 13.0,
            get_text_build_duration(text, 10, 13) as f32
        );
    }

    #[test]
    fn entry_starts_step_by_the_push_window() {
        assert!((entry_start(0, 10.0, 13.0) - 0.0).abs() < 1e-6);
        assert!((entry_start(1, 10.0, 13.0) - 10.0).abs() < 1e-6);
        assert!((entry_start(2, 10.0, 13.0) - 23.0).abs() < 1e-6);
        assert!((entry_start(3, 10.0, 13.0) - 36.0).abs() < 1e-6);
    }

    #[test]
    fn entry_progress_clamps_and_eases_out() {
        assert!((entry_progress(10.0, 10.0, 13.0) - 0.0).abs() < 1e-6);
        assert!((entry_progress(0.0, 10.0, 13.0) - 0.0).abs() < 1e-6);
        assert!((entry_progress(23.0, 10.0, 13.0) - 1.0).abs() < 1e-6);
        assert!((entry_progress(100.0, 10.0, 13.0) - 1.0).abs() < 1e-6);
        // A strong ease-out is past half way at the midpoint of its window.
        assert!(entry_progress(16.5, 10.0, 13.0) > 0.5);
        // A degenerate window must not produce NaN.
        assert!((entry_progress(5.0, 5.0, 0.0) - 0.0).abs() < 1e-6);
        assert!((entry_progress(6.0, 5.0, 0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn speed_falls_back_to_one() {
        assert!((effective_speed(2.0) - 2.0).abs() < 1e-6);
        assert!((effective_speed(1.0) - 1.0).abs() < 1e-6);
        assert!((effective_speed(0.0) - 1.0).abs() < 1e-6);
        assert!((effective_speed(-3.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn axis_defaults_are_derived_from_the_axis() {
        assert_eq!(BuildAxis::default(), BuildAxis::X);
        assert!((derived_entry_offset(BuildAxis::X) - 88.0).abs() < 1e-6);
        assert!((derived_entry_offset(BuildAxis::Y) - 28.0).abs() < 1e-6);
        assert!((derived_gap(BuildAxis::X) - 10.0).abs() < 1e-6);
        assert!((derived_gap(BuildAxis::Y) - 12.0).abs() < 1e-6);
        assert!((derived_first_drift(BuildAxis::X) - 6.0).abs() < 1e-6);
        assert!((derived_first_drift(BuildAxis::Y) + 14.0).abs() < 1e-6);

        let x = TextBuildConfig::default();
        assert!((x.resolved_entry_offset() - 88.0).abs() < 1e-6);
        assert!((x.resolved_gap() - 10.0).abs() < 1e-6);
        assert!((x.resolved_first_drift() - 6.0).abs() < 1e-6);

        let y = TextBuildConfig {
            axis: BuildAxis::Y,
            ..TextBuildConfig::default()
        };
        assert!((y.resolved_entry_offset() - 28.0).abs() < 1e-6);
        assert!((y.resolved_gap() - 12.0).abs() < 1e-6);
        assert!((y.resolved_first_drift() + 14.0).abs() < 1e-6);

        let explicit = TextBuildConfig {
            entry_offset: Some(4.0),
            gap: Some(5.0),
            first_drift: Some(6.0),
            ..TextBuildConfig::default()
        };
        assert!((explicit.resolved_entry_offset() - 4.0).abs() < 1e-6);
        assert!((explicit.resolved_gap() - 5.0).abs() < 1e-6);
        assert!((explicit.resolved_first_drift() - 6.0).abs() < 1e-6);
    }

    #[test]
    fn layout_pos_uses_widths_on_x_and_font_size_slots_on_y() {
        let sizes = [10.0, 20.0, 30.0];
        // Axis x: laid out by measured width, matching ctx::centered_positions.
        assert!((layout_pos(&sizes, 3, 0, 4.0, 20.0, BuildAxis::X) + 29.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 3, 1, 4.0, 20.0, BuildAxis::X) + 10.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 3, 2, 4.0, 20.0, BuildAxis::X) - 19.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 1, 0, 4.0, 20.0, BuildAxis::X)).abs() < 1e-5);
        // A layout of two words: total 30 + 4 = 34.
        assert!((layout_pos(&sizes, 2, 0, 4.0, 20.0, BuildAxis::X) + 12.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 2, 1, 4.0, 20.0, BuildAxis::X) - 7.0).abs() < 1e-5);

        // Axis y: every word occupies a font_size-tall slot, so the widths do not matter.
        assert!((layout_pos(&sizes, 3, 0, 4.0, 10.0, BuildAxis::Y) + 14.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 3, 1, 4.0, 10.0, BuildAxis::Y)).abs() < 1e-5);
        assert!((layout_pos(&sizes, 3, 2, 4.0, 10.0, BuildAxis::Y) - 14.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 2, 0, 4.0, 10.0, BuildAxis::Y) + 7.0).abs() < 1e-5);
        assert!((layout_pos(&sizes, 2, 1, 4.0, 10.0, BuildAxis::Y) - 7.0).abs() < 1e-5);

        // Out-of-range indices must not panic.
        assert!((layout_pos(&sizes, 1, 5, 4.0, 10.0, BuildAxis::X)).abs() < 1e-6);
        assert!((layout_pos(&[], 0, 0, 4.0, 10.0, BuildAxis::Y)).abs() < 1e-6);
    }

    #[test]
    fn blur_ghost_scales_with_the_radius_and_is_capped() {
        assert!((blur_ghost_alpha(0.0) - 0.0).abs() < 1e-6);
        assert!((blur_ghost_alpha(BLUR_GHOST_REFERENCE_PX) - BLUR_GHOST_MAX_ALPHA).abs() < 1e-6);
        assert!((blur_ghost_alpha(1000.0) - BLUR_GHOST_MAX_ALPHA).abs() < 1e-6);
        assert!((blur_ghost_spread(10.0) - 6.0).abs() < 1e-6);

        assert_eq!(ghost_offset(BuildAxis::X, 10.0), (6.0, 0.0));
        assert_eq!(ghost_offset(BuildAxis::Y, 10.0), (0.0, -6.0));
    }

    #[test]
    fn duration_includes_the_hold_tail() {
        let component = TextBuild::default();
        // The default line is 4 words: 10 + 3*13 = 49 entry frames, plus the 45 frame tail.
        assert_eq!(words(&component.config.text).len(), 4);
        assert_eq!(component.duration(), 94);
        assert_eq!(
            component.duration(),
            get_text_build_duration("Build The Future Together", 10, 13) + DEFAULT_TAIL
        );
        assert_eq!(component.meta().id, "text-build");
        assert_eq!(component.meta().feature, "text-build");
    }

    #[test]
    fn zero_word_text_does_not_underflow() {
        let component = TextBuild::new(TextBuildConfig {
            text: "   ".to_owned(),
            ..TextBuildConfig::default()
        });
        assert_eq!(component.duration(), DEFAULT_TAIL);
    }
}
