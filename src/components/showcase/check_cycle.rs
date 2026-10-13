//! A sentence cycles words through a slot beside a checked box.

use fframes::Svgr;

use crate::core::{Component, ComponentMeta};
use crate::ctx::RenderCtx;
use crate::motion::CubicBezier;
use crate::ui::{Mode, ReferenceFrame, UiTheme, text};

/// The reference scene's width, in reference-canvas px — the source's `REF_W`.
pub const REF_W: f32 = 718.0;
/// The reference scene's height, in reference-canvas px — the source's `REF_H`.
pub const REF_H: f32 = 394.0;

/// Type size of both halves of the sentence, in reference-canvas px. Fitted per line: 21.62–21.97.
pub const SIZE: f32 = 21.78;
/// Where the lead-in's advance ends: it is right-aligned to this scene x.
pub const LEAD_END: f32 = 340.04;
/// The lead-in's baseline, in scene y.
pub const LEAD_BASE: f32 = 203.01;
/// Where the word's advance starts, in scene x.
pub const WORD_X: f32 = 379.1;
/// The word's baseline, in scene y — 1.5 px below the lead-in's, on every word shown.
pub const WORD_BASE: f32 = 204.97;

/// The checkbox, as scene `x`, `y`, `width`, `height`, corner radius.
pub const BOX: [f32; 5] = [345.11, 183.71, 27.03, 27.59, 6.53];
/// The tick, as three points in the box's own coordinates.
pub const TICK: [[f32; 2]; 3] = [[5.468, 13.785], [11.827, 18.695], [20.72, 8.888]];
/// The tick's stroke width.
pub const TICK_W: f32 = 2.72;

/// The slot the words roll through, as scene `[top, bottom]`. A hard clip, top and bottom: the
/// feather the source fitted came back at 2.1 px and 1.0 px, which is the capture's blur, not a
/// fade.
pub const SLOT: [f32; 2] = [171.37, 223.62];

/// How far the strip travels on each roll, in turn — 44.19 px, then 42.27 px, then again.
pub const TRAVEL: [f32; 2] = [44.19, 42.27];
/// When the first roll starts, in seconds.
pub const ROLL_AT: f32 = 0.0324;
/// How often a roll comes, in seconds — 0.667 s, twenty frames at 30 fps.
pub const ROLL_EVERY: f32 = 0.667;
/// How long one roll takes, in seconds.
pub const ROLL_FOR: f32 = 0.5;
/// The roll's curve: `cubic-bezier(0.65, 0, 0.35, 1)`, the in-out cubic.
pub const ROLL_EASE: CubicBezier = CubicBezier::new(0.65, 0.0, 0.35, 1.0);

/// How long the last word holds after its roll starts, before the step back, in seconds.
pub const HOLD: f32 = 1.004;

/// The step back's pivot, in scene x and y — the frame's own centre, as the source names it.
pub const PIVOT: [f32; 2] = [359.14, 197.52];
/// The scale the line steps back to.
pub const ZOOM_TO: f32 = 0.8;
/// How long the step back takes, in seconds.
pub const ZOOM_FOR: f32 = 0.6;
/// The step back's curve: the same in-out cubic as the roll.
pub const ZOOM_EASE: CubicBezier = CubicBezier::new(0.65, 0.0, 0.35, 1.0);
/// Seconds of rest at the smaller size before the scene ends.
pub const ZOOM_REST: f32 = 0.2;

/// The composition's frame rate: the source is authored at 30 fps.
pub const FPS: f32 = 30.0;

/// The weight every run is painted at — Inter 500, which the source fitted to 0.45 px RMS.
pub const FONT_WEIGHT: u16 = 500;

/// The tolerance the source's own schedule comparisons settle to — the source rounds its `TRAVEL`
/// steps to hundredths, so sums of them agree to about this much.
#[cfg(test)]
const EPSILON: f32 = 0.02;

/// Clamp to `0..=1`, as the source's own `unit` helper does.
fn unit(value: f32) -> f32 {
    crate::motion::clamp01(value)
}

/// Split the comma-separated `words` prop the way the source does: trim, drop empties, and never
/// return an empty list.
#[must_use]
pub fn word_list(words: &str) -> Vec<String> {
    let list: Vec<String> = words
        .split(',')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    if list.is_empty() {
        vec![String::new()]
    } else {
        list
    }
}

/// Strip travel at `now` seconds, for a list of `n` words.
///
/// Each roll contributes its own [`TRAVEL`] step on the shared in-out cubic, so the strip's travel
/// is a sum of eased steps rather than one curve.
#[must_use]
pub fn strip_at(now: f32, n: usize) -> f32 {
    let mut total = 0.0;
    for k in 0..n.saturating_sub(1) {
        let p = unit((now - (ROLL_AT + k as f32 * ROLL_EVERY)) / ROLL_FOR);
        total += TRAVEL[k % TRAVEL.len()] * ROLL_EASE.ease(p);
    }
    total
}

/// Where word `k` rests on the strip.
///
/// This is a second sum of the same [`TRAVEL`] table: if it and [`strip_at`] ever disagree the
/// scene stops a pixel off, forever.
#[must_use]
pub fn rest_of(k: usize) -> f32 {
    let mut total = 0.0;
    for j in 0..k {
        total += TRAVEL[j % TRAVEL.len()];
    }
    total
}

/// When the step back starts, in seconds, for `n` words.
#[must_use]
pub fn zoom_start(n: usize) -> f32 {
    ROLL_AT + n.saturating_sub(2) as f32 * ROLL_EVERY + HOLD
}

/// Camera scale at `now` seconds: 1, easing down to [`ZOOM_TO`].
#[must_use]
pub fn zoom_at(now: f32, n: usize) -> f32 {
    1.0 - (1.0 - ZOOM_TO) * ZOOM_EASE.ease(unit((now - zoom_start(n)) / ZOOM_FOR))
}

/// Seconds from the first frame to the end: rolls, hold, step back, rest.
#[must_use]
pub fn scene_length(n: usize) -> f32 {
    zoom_start(n) + ZOOM_FOR + ZOOM_REST
}

/// The frame count the source's composition occupies for `n` words: `ceil(scene_length * 30)`.
///
/// For the default six-word copy this is 136, which is `config.ts`'s `durationInFrames`.
#[must_use]
pub fn scene_frames(n: usize) -> usize {
    (scene_length(n) * FPS).ceil() as usize
}

/// The y a word's baseline is drawn at, in scene y, when the strip has travelled `strip`.
#[must_use]
pub fn word_y(k: usize, strip: f32) -> f32 {
    WORD_BASE + rest_of(k) - strip
}

/// The zoomed scene x of a point, about [`PIVOT`].
#[must_use]
pub fn zoomed_x(x: f32, scale: f32) -> f32 {
    PIVOT[0] + (x - PIVOT[0]) * scale
}

/// The zoomed scene y of a point, about [`PIVOT`].
#[must_use]
pub fn zoomed_y(y: f32, scale: f32) -> f32 {
    PIVOT[1] + (y - PIVOT[1]) * scale
}

/// The slot rectangle in scene coordinates once the line has stepped back to `scale`.
///
/// The source's clip is a child of the scaled group, and a `clipPath` child is in the coordinate
/// system of the element that references it — so the rectangle is pre-scaled about [`PIVOT`] here
/// and the clip lands where the source's does.
#[must_use]
pub fn slot_rect(scale: f32) -> [f32; 4] {
    let top = zoomed_y(SLOT[0], scale);
    let bottom = zoomed_y(SLOT[1], scale);
    [0.0, top, REF_W, bottom - top]
}

/// Whether a word at scene y `y` is worth painting: off the slot entirely means nothing to draw.
#[must_use]
pub fn word_is_visible(y: f32) -> bool {
    (y - WORD_BASE).abs() <= SLOT[1] - SLOT[0] + SIZE
}

/// Whether the source's `plus-lighter` cross-fade would be visible at this device pixel ratio.
///
/// The two copies are one device pixel apart and weighted `1 - f` and `f`. At a device pixel ratio
/// of 1 — and a canvas of 1920x1080 px is the canvas, there is no second scale for it to live in —
/// `floor` and the fraction collapse onto the baseline itself, both weights become `0.5`, and the
/// pair sums to a single fully opaque draw. So the faithful output there is one copy, and this
/// predicate says so.
#[must_use]
pub fn blend_is_visible(device_pixel_ratio: f32) -> bool {
    device_pixel_ratio > 1.0
}

/// How many copies of a run the source paints at this device pixel ratio.
///
/// The port always answers `1`: it draws into the canvas, where the ratio is 1, and one copy is
/// then *identical* to the pair rather than an approximation of it. This is the function that
/// would return `2`, with weights `1 - f` and `f`, for a host rendering above ratio 1.
#[must_use]
pub fn run_copies(device_pixel_ratio: f32) -> usize {
    if blend_is_visible(device_pixel_ratio) {
        2
    } else {
        1
    }
}

/// The source's SVG viewport for a `width × height` canvas.
///
/// The source fits its measured `718 × 394` reference recording to the frame with
/// `u = min(width / REF_W, height / REF_H)` and centres the box, so every interior number in this
/// module is a *reference* pixel and this is the only place the canvas size enters the geometry.
/// On 1920x1080 that is `u = 2.6744` — not the `1.5` that `vmin / REFERENCE_VMIN` gives, because
/// the reference here is a recording, not a design canvas. See
/// [`crate::ui::reference`] for the full note.
#[must_use]
pub fn scene_view(width: f32, height: f32) -> ReferenceFrame {
    ReferenceFrame::fit_in(width, height, REF_W, REF_H)
}

/// Configuration for [`CheckCycle`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckCycleConfig {
    /// The part of the sentence that never changes.
    pub headline: String,
    /// The words that finish it, in order, comma separated.
    pub words: String,
    /// Which shadcn palette to paint from.
    pub mode: Mode,
    /// A whole token set, replacing the palette for `mode`.
    pub theme: Option<UiTheme>,
    /// The face to paint in. `None` resolves through the theme, then the crate's own face.
    pub font_family: Option<String>,
    /// Composition length in frames.
    pub duration: usize,
}

impl Default for CheckCycleConfig {
    fn default() -> Self {
        // `config.ts`'s demo defaults, field for field.
        Self {
            headline: "Create your own".to_owned(),
            words: "Intros, Demos, Launches, Changelogs, Tutorials, And more".to_owned(),
            mode: Mode::Light,
            theme: None,
            font_family: None,
            duration: 136,
        }
    }
}

/// The **Check Cycle** component.
#[derive(Debug, Clone, Default)]
pub struct CheckCycle {
    /// Component options.
    pub config: CheckCycleConfig,
}

impl CheckCycle {
    /// Build with the given options.
    #[must_use]
    pub fn new(config: CheckCycleConfig) -> Self {
        Self { config }
    }

    /// Static metadata, callable without an instance (the catalog needs this).
    #[must_use]
    pub fn meta_static() -> ComponentMeta {
        ComponentMeta {
            id: "check-cycle",
            name: "Check Cycle",
            summary: "Words roll one by one through a slot beside a ticked checkbox, then the line steps back",
            feature: "check-cycle",
        }
    }
}

/// Wrap a node in a scale about a point, collapsing to the node itself at rest.
fn scaled_about(node: Svgr<'static>, center_x: f32, center_y: f32, scale: f32) -> Svgr<'static> {
    if (scale - 1.0).abs() < 1e-4 {
        return node;
    }
    let transform = format!(
        "translate({center_x} {center_y}) scale({scale}) translate({} {})",
        -center_x, -center_y
    );
    fframes::svgr!(<g transform={transform}>{node}</g>)
}

/// The tick's polyline points, in the scene's own coordinates, as the SVG `points` attribute wants
/// them.
#[must_use]
fn tick_points() -> String {
    let mut out = String::new();
    for (index, [a, b]) in TICK.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{},{}", BOX[0] + a, BOX[1] + b));
    }
    out
}

impl Component for CheckCycle {
    fn meta(&self) -> ComponentMeta {
        Self::meta_static()
    }

    fn duration(&self) -> usize {
        self.config.duration
    }

    fn background(&self) -> Option<&'static str> {
        // The source's `AbsoluteFill` paints the theme's `background` token; the light palette's
        // value is a constant, so the static signature can name it.
        Some("#faf9f6")
    }

    fn show(&self, ctx: &mut RenderCtx<'_>) -> Svgr<'_> {
        let config = &self.config;
        let theme = config
            .theme
            .clone()
            .unwrap_or_else(|| UiTheme::for_mode(config.mode));
        let family =
            text::resolve_face(config.font_family.as_ref(), theme.font_family.as_ref(), ctx);

        let view = scene_view(ctx.width(), ctx.height());
        let list = word_list(&config.words);
        let n = list.len();

        // The source's clock: seconds inside the composition, at its own 30 fps.
        let now = ctx.frame() / FPS;
        let strip = strip_at(now, n);
        let zoom = zoom_at(now, n);

        // Faces are measured at the size the scene gives them: the `scale(u)` on the wrapper is
        // the reference scene's own fit to the canvas, so the numbers stay reference px.
        let size = SIZE;

        // `<text y>` is a **baseline** in this renderer, exactly as it is in CSS and in the
        // source `Line`, so `LEAD_BASE` and `WORD_BASE` are written to `y` unchanged.
        // (Measured, not assumed: see the module's fidelity note.)
        let lead = text::text_node(
            ctx,
            &config.headline,
            LEAD_END
                - text::tracked_width_at(ctx, &family, FONT_WEIGHT, size, &config.headline, 0.0)
                    / 2.0,
            LEAD_BASE,
            family.clone(),
            size,
            FONT_WEIGHT,
            0.0,
            theme.foreground.clone(),
            1.0,
        );
        let lead = scaled_about(lead, PIVOT[0], PIVOT[1], zoom);

        // The checkbox and its tick, in the scene's own box coordinates.
        let checkbox = fframes::svgr!(
            <g>
                <rect x={BOX[0]} y={BOX[1]} width={BOX[2]} height={BOX[3]} rx={BOX[4]}
                      fill={theme.primary.clone()} opacity={1.0} />
                <polyline points={tick_points()} fill="none"
                          stroke={theme.primary_foreground.clone()} stroke-width={TICK_W}
                          stroke-linecap="round" stroke-linejoin="round" opacity={1.0} />
            </g>
        );
        let checkbox = scaled_about(checkbox, PIVOT[0], PIVOT[1], zoom);

        // The strip: every word at its own rest on the one travel, hard-clipped to the slot. The
        // step back multiplies the glyph size and the baseline's distance from the pivot, which is
        // the same mapping the source's `scale(z)` about the pivot produces.
        let mut words: Vec<Svgr<'static>> = Vec::new();
        for (k, word) in list.iter().enumerate() {
            let y = word_y(k, strip);
            if !word_is_visible(y) {
                continue;
            }
            // `WORD_X` is where the word's advance *starts* — a left edge, not a centre.
            words.push(text::text_node_left(
                word,
                WORD_X,
                zoomed_y(y, zoom),
                family.clone(),
                size * zoom,
                FONT_WEIGHT,
                0.0,
                theme.foreground.clone(),
                1.0,
            ));
        }
        let words = Svgr::from(words);

        // The source's own SVG viewport: the reference scene's box, fitted to the canvas and
        // centred, with every interior number left in reference px. `overflow: visible` keeps type
        // that outruns the 718 px box.
        let slot = slot_rect(zoom);
        let transform = view.transform();
        fframes::svgr!(
            <g transform={transform}>
                <defs>
                    <clipPath id="check-cycle-slot">
                        <rect x={slot[0]} y={slot[1]} width={slot[2]} height={slot[3]} />
                    </clipPath>
                </defs>
                {lead}
                {checkbox}
                <g clip-path="url(#check-cycle-slot)">{words}</g>
            </g>
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_source_config_field_by_field() {
        let config = CheckCycleConfig::default();
        assert_eq!(config.headline, "Create your own");
        assert_eq!(
            config.words,
            "Intros, Demos, Launches, Changelogs, Tutorials, And more"
        );
        assert_eq!(config.mode, Mode::Light);
        assert_eq!(config.theme, None);
        assert_eq!(config.font_family, None);
        assert_eq!(config.duration, 136);
    }

    #[test]
    fn the_duration_is_the_source_composition_length() {
        assert_eq!(CheckCycle::default().duration(), 136);
        assert_eq!(CheckCycle::default().meta().id, "check-cycle");
        assert_eq!(CheckCycle::meta_static().feature, "check-cycle");
        assert_eq!(CheckCycle::default().background(), Some("#faf9f6"));
    }

    #[test]
    fn the_closed_form_reproduces_the_measured_settle() {
        // `registry/__measured__.json` records durationInFrames 136 and settle.base 129 for the
        // default six-word copy. The source's own formulas must reproduce both.
        let words = word_list(&CheckCycleConfig::default().words).len();
        assert_eq!(words, 6);
        assert_eq!(scene_frames(words), 136);

        // The measurement is a per-frame motion signal on rendered frames: a frame counts as still
        // when less than 0.05% of the pixels move by more than 2 luma. Its series for this
        // component is zero on frames 97..=111 — the hold — and runs 112..=129, one frame later at
        // the start and eighteen frames long.
        //
        // The step back starts at 0.0324 + 4 * 0.667 + 1.004 = 3.7044 s, i.e. frame 111.13. A
        // 1080p render evaluates whole frames, so frame 111 is before it. And because the in-out
        // cubic flattens at both ends, the *per-frame* change through 112..=129 reproduces the
        // measured shape on its own: 0.0085 of the type size at 112, a peak at 121, and 0.0145
        // reference px at 129 — the last frame that moves at all.
        assert!((zoom_start(words) * FPS - 111.13).abs() < 0.005);
        let scale = |frame: i32| zoom_at(frame.max(0) as f32 / FPS, words);
        let moved = |frame: i32| (scale(frame) - scale(frame - 1)) * SIZE;

        // Nothing on frame 111: it is still inside the hold.
        assert!((moved(111)).abs() < 1e-6, "{}", moved(111));
        assert!(scale(112) > ZOOM_TO);

        // The window the measurement saw, and its shape.
        let series: Vec<f32> = (112..=129).map(moved).collect();
        assert_eq!(series.len(), 18);
        assert!(
            (series[0].abs() - 0.008468).abs() < 0.0005,
            "frame 112 moved {}",
            series[0]
        );
        assert!(
            (moved(129).abs() - 0.014485).abs() < 0.0005,
            "frame 129 moved {}",
            moved(129)
        );
        let peak = series
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|(index, _)| index + 112);
        assert_eq!(peak, Some(121));
        // And it is over by frame 130, which rests at the target.
        assert!((scale(130) - ZOOM_TO).abs() < 1e-6);
        assert_eq!(scene_frames(words), 136);
    }

    #[test]
    fn the_reference_scene_is_the_sources_own() {
        assert!((REF_W - 718.0).abs() < 1e-6);
        assert!((REF_H - 394.0).abs() < 1e-6);
        assert!((SIZE - 21.78).abs() < 1e-6);
        assert!((LEAD_END - 340.04).abs() < 1e-6);
        assert!((LEAD_BASE - 203.01).abs() < 1e-6);
        assert!((WORD_X - 379.1).abs() < 1e-6);
        assert!((WORD_BASE - 204.97).abs() < 1e-6);
        assert!((BOX[0] - 345.11).abs() < 1e-6);
        assert!((BOX[1] - 183.71).abs() < 1e-6);
        assert!((BOX[2] - 27.03).abs() < 1e-6);
        assert!((BOX[3] - 27.59).abs() < 1e-6);
        assert!((BOX[4] - 6.53).abs() < 1e-6);
        assert!((TICK_W - 2.72).abs() < 1e-6);
        assert_eq!(TICK[0], [5.468, 13.785]);
        assert_eq!(TICK[2], [20.72, 8.888]);
        assert!((SLOT[0] - 171.37).abs() < 1e-6);
        assert!((SLOT[1] - 223.62).abs() < 1e-6);
        // The source's named pivot, kept verbatim. It is the `718 × 394` scene's centre to within
        // a fifth of a reference pixel, which is a third of a canvas pixel at 1080p.
        assert!((PIVOT[0] - 359.14).abs() < 1e-6);
        assert!((PIVOT[1] - 197.52).abs() < 1e-6);
        assert!((PIVOT[0] - REF_W / 2.0).abs() < 0.2);
        assert!((PIVOT[1] - REF_H / 2.0).abs() < 0.6);
        assert_eq!(FONT_WEIGHT, 500);
        assert_eq!(ROLL_EASE, CubicBezier::new(0.65, 0.0, 0.35, 1.0));
        assert_eq!(ZOOM_EASE, CubicBezier::new(0.65, 0.0, 0.35, 1.0));
    }

    #[test]
    fn the_word_list_trims_drops_empties_and_never_is_empty() {
        assert_eq!(word_list("Intros, Demos"), ["Intros", "Demos"]);
        assert_eq!(word_list(" Intros ,, Demos , "), ["Intros", "Demos"]);
        assert_eq!(word_list(""), [""]);
        assert_eq!(word_list("  ,  , "), [""]);
        // A single word has nothing to roll, but the list is still one entry.
        assert_eq!(word_list("Only").len(), 1);
    }

    #[test]
    fn the_strip_lands_every_word_exactly_on_the_slot() {
        for n in [2, 3, 5, 6] {
            let done = strip_at(60.0, n);
            assert!(
                (done - rest_of(n - 1)).abs() < EPSILON,
                "n {n}: {done} vs {}",
                rest_of(n - 1)
            );
        }
        assert!((rest_of(0)).abs() < 1e-6);
        assert!((rest_of(1) - TRAVEL[0]).abs() < 1e-6);
        assert!((rest_of(2) - (TRAVEL[0] + TRAVEL[1])).abs() < 1e-6);
        // The table cycles, so the third step is the first one again.
        assert!((rest_of(3) - (TRAVEL[0] + TRAVEL[1] + TRAVEL[0])).abs() < 1e-6);
    }

    #[test]
    fn the_strip_only_ever_rolls_forwards() {
        let mut previous = -1.0_f32;
        for frame in 0..=90 {
            let strip = strip_at(frame as f32 / FPS, 3);
            assert!(
                strip >= previous,
                "frame {frame}: {strip} went backwards from {previous}"
            );
            previous = strip;
        }
    }

    #[test]
    fn a_roll_is_half_a_second_and_starts_every_twenty_frames() {
        assert!((ROLL_FOR - 0.5).abs() < 1e-6);
        assert!(
            (ROLL_EVERY * FPS - 20.0).abs() < 0.02,
            "{}",
            ROLL_EVERY * FPS
        );
        // The second roll starts twenty frames after the first.
        let first = ROLL_AT * FPS;
        let second = (ROLL_AT + ROLL_EVERY) * FPS;
        assert!((second - first - 20.0).abs() < 0.02, "{}", second - first);
        // The roll is complete one `ROLL_FOR` after it starts, and not before.
        let mid = strip_at(ROLL_AT + ROLL_FOR / 2.0, 2);
        assert!(mid > 0.0 && mid < TRAVEL[0], "{mid}");
        assert!((strip_at(ROLL_AT + ROLL_FOR, 2) - TRAVEL[0]).abs() < EPSILON);
        // Nothing has moved before the first roll starts.
        assert!(strip_at(ROLL_AT, 6).abs() < EPSILON);
        assert!(strip_at(0.0, 6).abs() < EPSILON);
    }

    #[test]
    fn the_scene_steps_back_only_a_little_and_rests_before_the_end() {
        assert!((zoom_at(zoom_start(6) - 0.01, 6) - 1.0).abs() < EPSILON);
        assert!((zoom_at(scene_length(6), 6) - ZOOM_TO).abs() < EPSILON);
        assert!((zoom_at(scene_length(6) - ZOOM_REST, 6) - ZOOM_TO).abs() < EPSILON);
    }

    #[test]
    fn the_scene_ends_on_the_last_word() {
        assert!((strip_at(scene_length(6), 6) - rest_of(5)).abs() < EPSILON);
    }

    #[test]
    fn the_zoom_lands_between_one_and_the_target() {
        // Monotone down, never past either end.
        let mut previous = 1.0_f32;
        for step in 0..=60 {
            let now = zoom_start(6) + step as f32 * ZOOM_FOR / 60.0;
            let scale = zoom_at(now, 6);
            assert!(scale <= previous + 1e-6, "{scale} rose from {previous}");
            assert!((ZOOM_TO - 1e-6..=1.0 + 1e-6).contains(&scale), "{scale}");
            previous = scale;
        }
        // The ease leaves the mark gently: 20 ms in, it has barely moved.
        assert!(zoom_at(zoom_start(6) + 0.02, 6) > 0.99);
        // And a single word steps back too — the schedule does not depend on there being a roll.
        assert!((zoom_at(scene_length(1), 1) - ZOOM_TO).abs() < EPSILON);
    }

    #[test]
    fn the_step_back_pivots_on_the_scene_centre() {
        // At rest every point is itself; when scaled the pivot does not move.
        assert!((zoomed_y(SLOT[0], 1.0) - SLOT[0]).abs() < 1e-6);
        assert!((zoomed_y(PIVOT[1], ZOOM_TO) - PIVOT[1]).abs() < 1e-6);
        assert!((zoomed_x(PIVOT[0], ZOOM_TO) - PIVOT[0]).abs() < 1e-6);

        // The slot's top edge moves down towards the pivot as the line shrinks.
        let full = slot_rect(1.0);
        assert!((full[1] - SLOT[0]).abs() < 1e-4);
        assert!((full[3] - (SLOT[1] - SLOT[0])).abs() < 1e-4);

        let stepped = slot_rect(ZOOM_TO);
        let expected_top = PIVOT[1] + (SLOT[0] - PIVOT[1]) * ZOOM_TO;
        assert!((stepped[1] - expected_top).abs() < 1e-4, "{stepped:?}");
        assert!(stepped[1] > full[1], "the slot top should move down");
        assert!(
            stepped[3] < full[3],
            "the slot should be shorter once the line has stepped back"
        );
    }

    #[test]
    fn the_cull_keeps_the_far_copy_of_an_odd_tail_word() {
        // The source copies the strip to produce its trailing word and culls with
        // `|y - WORD_BASE| > slot height + SIZE`. The duplicate sits one travel below — well
        // inside the reach — so it is drawn, and clipped by the slot.
        assert!(word_is_visible(WORD_BASE + TRAVEL[0]));
        let reach = SLOT[1] - SLOT[0] + SIZE;
        assert!((reach - 74.03).abs() < 1e-4, "{reach}");
        assert!(!word_is_visible(WORD_BASE + reach + 0.01));
        assert!(!word_is_visible(WORD_BASE - reach - 0.01));
        assert!(word_is_visible(WORD_BASE + reach - 0.01));
    }

    #[test]
    fn every_word_of_the_default_copy_is_scheduled() {
        // Roll k starts at `ROLL_AT + k * ROLL_EVERY`, carries word k's step of `TRAVEL`, and is
        // done a `ROLL_FOR` later — no later roll has begun by then. Six words are five rolls.
        let n = 6;
        for k in 0..n - 1 {
            let start = ROLL_AT + k as f32 * ROLL_EVERY;
            let before = strip_at(start, n);
            let after = strip_at(start + ROLL_FOR, n);
            assert!(
                (after - before - TRAVEL[k % TRAVEL.len()]).abs() < EPSILON,
                "roll {k} carried {} not {}",
                after - before,
                TRAVEL[k % TRAVEL.len()]
            );
        }
        // Word 0 rests on the slot for the first frame: it has nowhere to travel from.
        assert!((word_y(0, 0.0) - WORD_BASE).abs() < 1e-6);
        // The last word is on the slot once the fifth roll has finished, and the strip has
        // travelled exactly to that word's rest.
        let settled = ROLL_AT + 4.0 * ROLL_EVERY + ROLL_FOR;
        assert!((strip_at(settled, n) - rest_of(n - 1)).abs() < EPSILON);
        assert!((word_y(n - 1, strip_at(settled, n)) - WORD_BASE).abs() < EPSILON);
    }

    #[test]
    fn the_blend_pair_collapses_at_the_canvass_own_pixel_ratio() {
        // The canvas is the device pixel ratio: one copy, which is identical to the source's pair
        // rather than an approximation of it. A hypothetical ratio above 1 would want two.
        assert!(!blend_is_visible(1.0));
        assert_eq!(run_copies(1.0), 1);
        assert_eq!(run_copies(2.0), 2);
    }

    #[test]
    fn the_word_slot_starts_to_the_right_of_the_checkbox() {
        // `WORD_X` is a **left edge**, not a centre: the source's slot starts after the box, and
        // the 7 reference px of air between them is what stops the word overlapping the checkbox.
        // The drawn left edge is `WORD_X` itself, so this is an assertion about `show`'s placement
        // and not just about the constant.
        let box_right = BOX[0] + BOX[2];
        assert!((box_right - 372.14).abs() < 0.001, "{box_right}");
        assert!((WORD_X - 379.1).abs() < 1e-6);
        assert!(WORD_X > box_right, "{WORD_X} is not right of {box_right}");
        assert!((WORD_X - box_right - 6.96).abs() < 0.001);

        // A word is drawn from its left edge at `WORD_X`. Centring it there instead — passing a
        // left edge to a helper that takes a centre — would move it left by half its width, which
        // for a word as wide as the box is the whole gap and more.
        let word_width = 40.0_f32;
        let centred_left = WORD_X - word_width / 2.0;
        assert!(
            centred_left < box_right,
            "centring at WORD_X would start the word at {centred_left}, inside the box"
        );
    }

    #[test]
    fn the_scene_fits_the_canvas_the_way_the_source_viewbox_does() {
        // The source's own `u = min(width / REF_W, height / REF_H)`, through the shared helper.
        // At 1080p that is 2.6744, *not* the `vmin / REFERENCE_VMIN` design unit of 1.5 — the
        // reference here is a measured recording, not a design canvas.
        let full = scene_view(1920.0, 1080.0);
        let expected = (1920.0_f32 / REF_W).min(1080.0 / REF_H);
        assert!((full.scale - expected).abs() < 1e-6, "{}", full.scale);
        assert!((full.scale - 2.6744).abs() < 0.001, "{}", full.scale);
        assert!((full.width - 718.0).abs() < 1e-6);
        assert!((full.height - 394.0).abs() < 1e-6);
        let design_unit = 1080.0_f32 / 100.0 / crate::motion::REFERENCE_VMIN;
        assert!((design_unit - 1.5).abs() < 1e-6);
        assert!(
            full.scale > design_unit * 1.7,
            "the two models differ by a lot"
        );

        // At the source's own composition size the fit is 1.7827, limited by the width there too.
        let source = scene_view(1280.0, 720.0);
        assert!((source.scale - 1.7827).abs() < 0.001, "{}", source.scale);
        assert!((source.top - 8.76).abs() < 0.05, "{}", source.top);

        // The box is centred: it fills one axis and is letterboxed equally on the other.
        for (width, height) in [(1920.0, 1080.0), (1280.0, 720.0), (1000.0, 1000.0)] {
            let view = scene_view(width, height);
            let drawn_w = REF_W * view.scale;
            let drawn_h = REF_H * view.scale;
            assert!((drawn_w - width).abs() < 0.01 || (drawn_h - height).abs() < 0.01);
            assert!((view.left - (width - drawn_w) / 2.0).abs() < 1e-4);
            assert!((view.top - (height - drawn_h) / 2.0).abs() < 1e-4);
        }

        // The group's transform is what `show` emits, and a reference pixel is `scale` canvas px.
        assert_eq!(
            full.transform(),
            format!(
                "translate({} {}) scale({})",
                full.left, full.top, full.scale
            )
        );
        let (x, y) = full.point(REF_W / 2.0, REF_H / 2.0);
        assert!((x - 960.0).abs() < 0.01);
        assert!((y - 540.0).abs() < 0.01);
        let (x, _) = full.point(REF_W / 2.0 + 1.0, 0.0);
        assert!((x - (960.0 + full.scale)).abs() < 0.01);
    }
}
