//! Shared rendering context, text metering and layout helpers for every component.

use std::sync::OnceLock;

use fframes::{FontQuery, Frame};

use crate::core::Fonts;
use crate::meter::TextMeter;

/// A mutable borrow of the fframes frame.
///
/// Wrapped in its own type because `&mut Frame` is invariant: the wrapper keeps the lifetime in
/// one place, so a [`RenderCtx`] can hold the frame for the duration of a single `show` call
/// without that lifetime leaking into every component signature.
pub struct FrameRef<'f> {
    frame: &'f mut Frame,
}

impl<'f> FrameRef<'f> {
    /// Borrow a frame mutably.
    pub fn new(frame: &'f mut Frame) -> Self {
        Self { frame }
    }

    /// The borrowed frame.
    pub fn frame(&self) -> &Frame {
        self.frame
    }

    /// The borrowed frame, mutably.
    pub fn frame_mut(&mut self) -> &mut Frame {
        self.frame
    }

    /// Measure a string through fframes' own font source.
    ///
    /// This exists for components that need the renderer's measured width. It requires the
    /// measured string to outlive the frame borrow, so most components use
    /// [`RenderCtx::text_width`] instead, which measures from the embedded face and has no such
    /// constraint.
    pub fn text_width<'a>(
        &mut self,
        ctx: &fframes::FFramesContext<'a, '_>,
        query: FontQuery<'a>,
        value: &'a str,
    ) -> Option<usize> {
        self.frame.text_width(ctx, query, value)
    }
}

impl std::fmt::Debug for FrameRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameRef")
            .field("index", &self.frame.index)
            .finish()
    }
}

/// Info about the component currently being rendered.
#[derive(Debug, Clone, Copy)]
pub struct ComponentFrame {
    /// Frame index relative to the start of this component.
    pub local: f32,
    /// Frame index within the whole video.
    pub global: usize,
    /// Video frame rate.
    pub fps: f32,
    /// Total frames this component occupies.
    pub duration: usize,
    /// Pixel width of the canvas.
    pub width: f32,
    /// Pixel height of the canvas.
    pub height: f32,
}

impl ComponentFrame {
    /// Local frame as a whole frame index.
    pub fn local_index(&self) -> usize {
        self.local.max(0.0) as usize
    }

    /// Seconds elapsed inside this component.
    pub fn seconds(&self) -> f32 {
        self.local / self.fps
    }
}

/// One embedded face, together with the family names and weight the renderer resolves it by.
///
/// A family reaches a component as a `font-family` string plus a `font-weight`; a renderer picks
/// the face whose family matches and whose weight is closest. This type carries exactly those two
/// keys — read from the font file itself rather than guessed from its file name — so measurement
/// can pick the same face the renderer will draw with.
#[derive(Debug, Clone)]
pub struct FaceMeter {
    families: Vec<String>,
    weight: u16,
    meter: TextMeter,
}

impl FaceMeter {
    /// Probe a face's own `name` and `OS/2` tables for the keys a renderer resolves it by.
    ///
    /// The primary family is the typographic one (name ID 16) falling back to the legacy family
    /// (ID 1), which is what `usvgr`'s font database does. The legacy name is kept as a second
    /// key so a face that a renderer really does expose as its own family (`"Inter SemiBold"`)
    /// can still be asked for by that name.
    pub fn probe(meter: TextMeter) -> Self {
        let mut families: Vec<String> = Vec::new();
        for family in [meter.family_name(), meter.legacy_family_name()]
            .into_iter()
            .flatten()
        {
            let family = family.trim().to_owned();
            if !family.is_empty() && !families.iter().any(|known| known == &family) {
                families.push(family);
            }
        }
        Self {
            families,
            weight: meter.weight(),
            meter,
        }
    }

    /// Every family name this face answers to, most renderer-relevant first.
    pub fn families(&self) -> &[String] {
        &self.families
    }

    /// The family name a renderer resolves this face under.
    pub fn family(&self) -> &str {
        self.families.first().map_or("", String::as_str)
    }

    /// The face's weight class.
    pub fn weight(&self) -> u16 {
        self.weight
    }

    /// The meter for this face.
    pub fn meter(&self) -> &TextMeter {
        &self.meter
    }

    /// Whether this face answers to `family` (case-insensitive, as font databases match).
    pub fn matches_family(&self, family: &str) -> bool {
        let family = family.trim();
        self.families
            .iter()
            .any(|known| known.eq_ignore_ascii_case(family))
    }

    /// How far this face's weight is from a request, with ties resolving downward.
    ///
    /// The second element is `1` for a face heavier than the request and `0` otherwise, so a
    /// request halfway between two faces (`500` and `700` for `600`) sorts the lighter one first.
    fn weight_distance(&self, weight: u16) -> (u16, u8) {
        (weight.abs_diff(self.weight), u8::from(self.weight > weight))
    }
}

/// The font meters a render resolves against.
///
/// Both come from faces embedded in this crate, so measurement is identical everywhere and does
/// not depend on the renderer having loaded system fonts.
#[derive(Debug, Default)]
pub struct TextMeters {
    /// The face used by every sans-serif component.
    pub sans: Option<TextMeter>,
    /// The face used by the monospaced components.
    pub mono: Option<TextMeter>,
    /// Extra faces, each addressed by the family and weight its own file declares.
    faces: Vec<FaceMeter>,
}

impl TextMeters {
    /// Build both meters from the bytes of the two faces.
    pub fn from_bytes(sans: Option<Vec<u8>>, mono: Option<Vec<u8>>) -> Self {
        Self {
            sans: sans.and_then(|bytes| TextMeter::new("sans", bytes)),
            mono: mono.and_then(|bytes| TextMeter::new("mono", bytes)),
            faces: Vec::new(),
        }
    }

    /// Register one more embedded face, addressed by the family and weight inside its file.
    pub fn with_face(mut self, meter: TextMeter) -> Self {
        self.faces.push(FaceMeter::probe(meter));
        self
    }

    /// Every registered weight-aware face, in registration order.
    pub fn faces(&self) -> &[FaceMeter] {
        &self.faces
    }

    /// The meter to use for a resolved family, falling back to the sans face.
    ///
    /// A family that was registered with weights (Inter, Montserrat) resolves to its *regular*
    /// face, so measuring a family without naming a weight measures what a renderer draws for a
    /// weight-less run. Families registered without weights — DM Sans, and whatever a host puts in
    /// [`Fonts::sans`] — behave exactly as they always have: sans, or mono for the mono family.
    pub fn for_family(&self, family: &str, fonts: &Fonts) -> Option<&TextMeter> {
        if let Some(face) = self.nearest_face(family, 400) {
            return Some(face.meter());
        }
        if family == fonts.mono {
            self.mono.as_ref().or(self.sans.as_ref())
        } else {
            self.sans.as_ref()
        }
    }

    /// The registered face of `family` closest to `weight`, or `None` when the family has none.
    fn nearest_face(&self, family: &str, weight: u16) -> Option<&FaceMeter> {
        self.faces
            .iter()
            .filter(|face| face.matches_family(family))
            .min_by_key(|face| face.weight_distance(weight))
    }

    /// The meter for a family *and* a weight: the nearest registered weight of that family.
    ///
    /// Falls back to the family's default face (through [`TextMeters::for_family`], which is how
    /// the families registered without weights — DM Sans today — are reached), and the caller
    /// decides what to do when even that is missing. Ties between two equally distant weights
    /// resolve downward, to the lighter face.
    pub fn for_family_weight(
        &self,
        family: &str,
        weight: u16,
        fonts: &Fonts,
    ) -> Option<&TextMeter> {
        match self.nearest_face(family, weight) {
            Some(face) => Some(face.meter()),
            None => self.for_family(family, fonts),
        }
    }
}

/// What a component is handed on every frame.
///
/// Every string this context hands to a component is **owned**, and measurement goes through
/// [`TextMeters`] rather than `Frame::text_width`. Both choices exist for one reason: the
/// returned SVG tree may borrow from the context or from the component, and keeping the context
/// free of borrowed strings means those two borrows never have to be reconciled.
pub struct RenderCtx<'a> {
    /// The fframes frame, for components that need frame-level APIs.
    pub frame: FrameRef<'a>,
    /// The resolved font family names.
    pub fonts: Fonts,
    /// The font meters measurement goes through.
    pub meters: &'a TextMeters,
    /// Geometry and timing of the component being rendered.
    pub info: ComponentFrame,
}

impl<'a> RenderCtx<'a> {
    /// Build a context for one component frame.
    ///
    /// The fframes context is deliberately not stored: `FFramesContext` is invariant over its
    /// own lifetime, so holding it would force every component signature to name that lifetime.
    /// Measurement goes through `meters` instead, and media access is the host's concern.
    pub fn new(
        frame: &'a mut Frame,
        fonts: Fonts,
        meters: &'a TextMeters,
        info: ComponentFrame,
    ) -> Self {
        Self {
            frame: FrameRef::new(frame),
            fonts,
            meters,
            info,
        }
    }

    /// Canvas width in pixels.
    pub fn width(&self) -> f32 {
        self.info.width
    }

    /// Canvas height in pixels.
    pub fn height(&self) -> f32 {
        self.info.height
    }

    /// Local frame index as a float.
    pub fn frame(&self) -> f32 {
        self.info.local
    }

    /// `min(width, height) / 100`, the design unit the 3D-ish components scale against.
    pub fn vmin(&self) -> f32 {
        self.width().min(self.height()) / 100.0
    }

    /// The sans family name.
    pub fn sans(&self) -> String {
        self.fonts.sans.clone()
    }

    /// The monospaced family name.
    pub fn mono(&self) -> String {
        self.fonts.mono.clone()
    }

    /// The meter for a resolved family.
    pub fn meter(&self, family: &str) -> Option<&TextMeter> {
        self.meters.for_family(family, &self.fonts)
    }

    /// The meter for a resolved family drawn at `weight`.
    ///
    /// Components that emit a `font-weight` must measure through this rather than
    /// [`RenderCtx::meter`], or they lay text out with the regular face and draw it with another.
    pub fn meter_at(&self, family: &str, weight: u16) -> Option<&TextMeter> {
        self.meters.for_family_weight(family, weight, &self.fonts)
    }

    /// Width of `text` at `size` px in `family`.
    ///
    /// Falls back to a per-character estimate when the family has no embedded face, so layout
    /// never collapses.
    pub fn text_width(&self, family: &str, size: f32, text: &str) -> f32 {
        match self.meter(family) {
            Some(meter) => meter.text_width(text, size),
            None => text.chars().count() as f32 * size * 0.55,
        }
    }

    /// Width of one space glyph at `size` px in `family`.
    pub fn space_width(&self, family: &str, size: f32) -> f32 {
        match self.meter(family) {
            Some(meter) => meter.space_width(size),
            None => size * 0.26,
        }
    }

    /// Ascent of `family` at `size` px, falling back to a typical cap ratio.
    pub fn ascent(&self, family: &str, size: f32) -> f32 {
        match self.meter(family) {
            Some(meter) => meter.ascent(size),
            None => size * 0.75,
        }
    }

    /// Width of `text` at `size` px in `family` drawn at `weight`.
    ///
    /// Same contract as [`RenderCtx::text_width`], except the face is the nearest registered
    /// weight of `family`. A family registered without weights (DM Sans, and the mono slot) falls
    /// back to its single face, so this is a strict generalisation of `text_width`.
    pub fn text_width_at(&self, family: &str, weight: u16, size: f32, text: &str) -> f32 {
        match self.meter_at(family, weight) {
            Some(meter) => meter.text_width(text, size),
            None => text.chars().count() as f32 * size * 0.55,
        }
    }

    /// Width of one space glyph at `size` px in `family` drawn at `weight`.
    pub fn space_width_at(&self, family: &str, weight: u16, size: f32) -> f32 {
        match self.meter_at(family, weight) {
            Some(meter) => meter.space_width(size),
            None => size * 0.26,
        }
    }

    /// Ascent of `family` at `size` px drawn at `weight`.
    pub fn ascent_at(&self, family: &str, weight: u16, size: f32) -> f32 {
        match self.meter_at(family, weight) {
            Some(meter) => meter.ascent(size),
            None => size * 0.75,
        }
    }

    /// Distance from the top of a line box to its baseline, for a vertically centred block.
    ///
    /// Components draw text with a baseline `y`, so this converts "centre this line" into a
    /// baseline offset that accounts for real font metrics.
    pub fn baseline_offset(&self, family: &str, size: f32, line_height: f32) -> f32 {
        let Some(meter) = self.meter(family) else {
            return size * 0.34;
        };
        let ascent = meter.ascent(size);
        let descent = meter.descent(size);
        (line_height - (ascent + descent)) / 2.0 + ascent
    }

    /// [`RenderCtx::baseline_offset`] against the face for `weight`.
    pub fn baseline_offset_at(
        &self,
        family: &str,
        weight: u16,
        size: f32,
        line_height: f32,
    ) -> f32 {
        let Some(meter) = self.meter_at(family, weight) else {
            return size * 0.34;
        };
        let ascent = meter.ascent(size);
        let descent = meter.descent(size);
        (line_height - (ascent + descent)) / 2.0 + ascent
    }
}

/// Split text into words the way the source library does (`trim` + split on whitespace).
pub fn words(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Split text into characters, unicode-scalar-safe (`Array.from(text)`).
pub fn chars(text: &str) -> Vec<char> {
    text.chars().collect()
}

/// A laid-out word: its text and the centre x it should be drawn at.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedWord {
    /// The word's characters.
    pub text: String,
    /// Centre x, relative to the run's centre.
    pub x: f32,
    /// Measured width in px.
    pub width: f32,
}

/// One laid-out line.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLine {
    /// The words on this line.
    pub words: Vec<PlacedWord>,
    /// The line's full width in px.
    pub width: f32,
}

impl PlacedLine {
    /// The line's left edge, given the line is centred at `center_x`.
    pub fn left(&self, center_x: f32) -> f32 {
        center_x - self.width / 2.0
    }
}

/// A wrapped paragraph, ready to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct LineLayout {
    /// One entry per line, top to bottom.
    pub lines: Vec<PlacedLine>,
    /// Line step in px (`font_size * line_height`).
    pub line_step: f32,
    /// Baseline of line `0` relative to the block's vertical centre.
    pub first_baseline_dy: f32,
}

impl LineLayout {
    /// Baseline y of line `index` when the block is vertically centred on `center_y`.
    pub fn baseline_y(&self, index: usize, center_y: f32) -> f32 {
        center_y + self.first_baseline_dy + index as f32 * self.line_step
    }

    /// Every word in reading order, with its running word index and absolute baseline y.
    ///
    /// The index is the word's position in the whole paragraph, not its position on its own line:
    /// the per-word reveals stagger off it, and a reveal that restarted at every line break would
    /// put the first word of line 2 ahead of the last word of line 1.
    pub fn all_words(&self, center_y: f32) -> Vec<(usize, &PlacedWord, f32)> {
        let mut out = Vec::new();
        let mut index = 0;
        for (line_index, line) in self.lines.iter().enumerate() {
            let y = self.baseline_y(line_index, center_y);
            for word in &line.words {
                out.push((index, word, y));
                index += 1;
            }
        }
        out
    }

    /// Total words across every line.
    pub fn word_count(&self) -> usize {
        self.lines.iter().map(|line| line.words.len()).sum()
    }
}

/// A thread-safe cache for a measured [`LineLayout`].
///
/// Frames are rendered concurrently, so this must be `Sync`. `OnceLock` gives exactly the
/// semantics wanted: the first measurement wins and is shared by every later frame.
#[derive(Debug, Default)]
pub struct LayoutCache(OnceLock<LineLayout>);

impl LayoutCache {
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// The cached layout, if a measurement has happened.
    pub fn get(&self) -> Option<&LineLayout> {
        self.0.get()
    }

    /// Resolve the layout, measuring once per component instance.
    pub fn resolve(
        &self,
        ctx: &RenderCtx<'_>,
        family: &str,
        text: &str,
        font_size: f32,
        max_width: f32,
        line_height: f32,
    ) -> LineLayout {
        if let Some(layout) = self.0.get() {
            return layout.clone();
        }
        let layout = layout_paragraph(ctx, family, text, font_size, max_width, line_height);
        let _ = self.0.set(layout.clone());
        layout
    }
}

impl Clone for LayoutCache {
    fn clone(&self) -> Self {
        Self(OnceLock::new())
    }
}

/// Greedy line wrapping over measured words.
pub fn layout_paragraph(
    ctx: &RenderCtx<'_>,
    family: &str,
    text: &str,
    font_size: f32,
    max_width: f32,
    line_height: f32,
) -> LineLayout {
    let raw: Vec<&str> = text.split_whitespace().collect();
    let line_step = font_size * line_height;
    if raw.is_empty() {
        return LineLayout {
            lines: Vec::new(),
            line_step,
            first_baseline_dy: 0.0,
        };
    }

    let space = ctx.space_width(family, font_size);
    let mut lines: Vec<PlacedLine> = Vec::new();
    let mut current: Vec<PlacedWord> = Vec::new();
    let mut current_width = 0.0_f32;

    for word in raw {
        let width = ctx.text_width(family, font_size, word);
        if current.is_empty() {
            current_width = width;
            current.push(PlacedWord {
                text: word.to_owned(),
                x: 0.0,
                width,
            });
            continue;
        }
        if current_width + space + width > max_width {
            lines.push(finish_line(std::mem::take(&mut current), space));
            current_width = width;
        } else {
            current_width += space + width;
        }
        current.push(PlacedWord {
            text: word.to_owned(),
            x: 0.0,
            width,
        });
    }
    if !current.is_empty() {
        lines.push(finish_line(current, space));
    }

    let line_count = lines.len();
    let block_height = line_step * line_count as f32;
    let first_baseline_dy = -block_height / 2.0 + ctx.baseline_offset(family, font_size, line_step);

    LineLayout {
        lines,
        line_step,
        first_baseline_dy,
    }
}

fn finish_line(mut words: Vec<PlacedWord>, space: f32) -> PlacedLine {
    let total: f32 =
        words.iter().map(|w| w.width).sum::<f32>() + space * words.len().saturating_sub(1) as f32;
    let mut cursor = -total / 2.0;
    for word in &mut words {
        word.x = cursor + word.width / 2.0;
        cursor += word.width + space;
    }
    PlacedLine {
        words,
        width: total,
    }
}

/// Centre-of-each-word positions for the first `count` words of `sizes`, centred around 0.
///
/// Ported from the source library's `centeredPositions` — used by the layouts that reflow
/// words as each new one lands.
pub fn centered_positions(sizes: &[f32], count: usize, gap: f32) -> Vec<f32> {
    if count == 0 || sizes.is_empty() {
        return Vec::new();
    }
    let count = count.min(sizes.len());
    let placed = &sizes[..count];
    let total: f32 = placed.iter().sum::<f32>() + gap * (count.saturating_sub(1)) as f32;
    let mut cursor = -total / 2.0;
    placed
        .iter()
        .map(|size| {
            let center = cursor + size / 2.0;
            cursor += size + gap;
            center
        })
        .collect()
}

/// Centre x of every character of a single unwrapped line, relative to the run's centre.
///
/// The per-character typography components need each glyph's own centre because every glyph
/// carries its own transform. A run laid out at its left edge would drift as soon as one glyph
/// scales, so the whole run is centred here by accumulating measured advances.
///
/// Returns `(centres, total_width)`. `centres` is empty when `line` is empty.
pub fn char_centers(ctx: &RenderCtx<'_>, family: &str, line: &str, size: f32) -> (Vec<f32>, f32) {
    let glyphs: Vec<char> = line.chars().collect();
    let widths: Vec<f32> = glyphs
        .iter()
        .map(|ch| {
            let mut buffer = [0_u8; 4];
            ctx.text_width(family, size, ch.encode_utf8(&mut buffer))
        })
        .collect();
    let total: f32 = widths.iter().sum();
    let mut cursor = -total / 2.0;
    let centres = widths
        .iter()
        .map(|width| {
            let centre = cursor + width / 2.0;
            cursor += width;
            centre
        })
        .collect();
    (centres, total)
}

/// 0 -> 1 -> 0 across `t` (the source library's three-point mid-move pulse).
pub fn interpolate3(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 }
}

/// `translate(x y)` / `scale(s)` / `rotate(deg)` as one SVG transform list.
pub fn svg_transform(x: f32, y: f32, scale: f32, rotate_deg: f32) -> String {
    let mut parts = Vec::new();
    if x.abs() > 1e-4 || y.abs() > 1e-4 {
        parts.push(format!("translate({x:.4} {y:.4})"));
    }
    if (scale - 1.0).abs() > 1e-5 {
        parts.push(format!("scale({scale:.6})"));
    }
    if rotate_deg.abs() > 1e-4 {
        parts.push(format!("rotate({rotate_deg:.4})"));
    }
    if parts.is_empty() {
        "none".to_owned()
    } else {
        parts.join(" ")
    }
}

/// Escape the five XML special characters for inclusion in an SVG text node.
pub fn escape_xml(value: &str) -> std::borrow::Cow<'_, str> {
    if value
        .bytes()
        .any(|b| matches!(b, b'&' | b'<' | b'>' | b'"' | b'\''))
    {
        let mut out = String::with_capacity(value.len() + 8);
        for ch in value.chars() {
            match ch {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                other => out.push(other),
            }
        }
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

/// A `<text>` element drawn with `text-anchor="middle"` at a measured centre.
///
/// Component fonts in this library use negative tracking, and SVG applies `letter-spacing` from
/// the first glyph's advance, so the visual centre of a tracked run shifts by roughly
/// `tracking / 2` per inter-glyph gap. `x` is corrected to land the run truly centred.
pub fn centred_text(text: &str, center_x: f32, font_size: f32, tracking_em: f32) -> (String, f32) {
    let chars = text.chars().count().max(1);
    let correction = -tracking_em * font_size * chars as f32 / 2.0;
    (text.to_owned(), center_x + correction)
}

/// The `font-weight` value to write into a `<text>` node.
///
/// It has to be a *string*. `fframes` builds its compile-time SVG tree with a numeric attribute
/// keeping the number apart from its text form, and `usvgr`'s `resolve_font_weight` reads the
/// attribute through that text form — a `u16` arrives as an empty string, the match falls through
/// to the default, and a component that asks for weight 700 silently draws the regular face.
/// Spelling the number out keeps the weight the renderer resolves equal to the weight the meters
/// measure. One face per family (DM Sans) cannot tell the difference; a family with four weights
/// (Inter) draws the wrong one without this.
#[must_use]
pub fn font_weight_attr(weight: u16) -> String {
    weight.to_string()
}

/// A `<text>` node centred on `center_x`, with tracking compensation applied.
#[allow(clippy::too_many_arguments)]
pub fn text_node_centered(
    content: &str,
    center_x: f32,
    y: f32,
    family: String,
    font_size: f32,
    weight: u16,
    tracking_em: f32,
    fill: String,
    opacity: f32,
) -> fframes::Svgr<'static> {
    let (content, x) = centred_text(content, center_x, font_size, tracking_em);
    let weight = font_weight_attr(weight);
    fframes::svgr!(
        <text x={x} y={y} font-family={family} font-size={font_size} font-weight={weight}
              letter-spacing={tracking_em} fill={fill} opacity={opacity}
              text-anchor="middle">{content}</text>
    )
}

/// A `<text>` node anchored at its left edge.
#[allow(clippy::too_many_arguments)]
pub fn text_node_left(
    content: String,
    x: f32,
    y: f32,
    family: String,
    font_size: f32,
    weight: u16,
    tracking_em: f32,
    fill: String,
    opacity: f32,
) -> fframes::Svgr<'static> {
    let weight = font_weight_attr(weight);
    fframes::svgr!(
        <text x={x} y={y} font-family={family} font-size={font_size} font-weight={weight}
              letter-spacing={tracking_em} fill={fill} opacity={opacity}>{content}</text>
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_splits_like_the_source() {
        assert_eq!(words("Hello, World!"), vec!["Hello,", "World!"]);
        assert_eq!(words("  a   b  "), vec!["a", "b"]);
        assert!(words("   ").is_empty());
    }

    #[test]
    fn centered_positions_matches_the_ported_formula() {
        let sizes = [10.0, 20.0, 30.0];
        let all = centered_positions(&sizes, 3, 4.0);
        assert_eq!(all.len(), 3);
        // total = 60 + 8 = 68, cursor starts at -34.
        // centre0 = -34 + 5 = -29; cursor -> -20
        // centre1 = -20 + 10 = -10; cursor -> 4
        // centre2 = 4 + 15 = 19
        assert!((all[0] + 29.0).abs() < 1e-5, "got {}", all[0]);
        assert!((all[1] + 10.0).abs() < 1e-5, "got {}", all[1]);
        assert!((all[2] - 19.0).abs() < 1e-5, "got {}", all[2]);
        assert!((centered_positions(&sizes, 1, 4.0)[0]).abs() < 1e-5);
    }

    #[test]
    fn finish_line_centres_words() {
        let words = vec![
            PlacedWord {
                text: "a".into(),
                x: 0.0,
                width: 10.0,
            },
            PlacedWord {
                text: "b".into(),
                x: 0.0,
                width: 10.0,
            },
        ];
        let line = finish_line(words, 4.0);
        assert!((line.width - 24.0).abs() < 1e-5);
        assert!((line.words[0].x + 7.0).abs() < 1e-5);
        assert!((line.words[1].x - 7.0).abs() < 1e-5);
    }

    #[test]
    fn interpolate3_pulses() {
        assert!((interpolate3(0.0) - 0.0).abs() < 1e-6);
        assert!((interpolate3(0.5) - 1.0).abs() < 1e-6);
        assert!((interpolate3(1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn escape_xml_only_escapes_when_needed() {
        assert_eq!(escape_xml("plain"), "plain");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("<x>"), "&lt;x&gt;");
    }

    #[test]
    fn tracking_correction_centres_a_negative_track() {
        let (_, x) = centred_text("hello", 100.0, 72.0, -0.03);
        assert!(x > 100.0, "expected a rightward correction, got {x}");
        let (_, x0) = centred_text("hello", 100.0, 72.0, 0.0);
        assert!((x0 - 100.0).abs() < 1e-6);
    }

    #[test]
    fn baseline_offsets_step_by_line_height() {
        let layout = LineLayout {
            lines: Vec::new(),
            line_step: 80.0,
            first_baseline_dy: 10.0,
        };
        assert!((layout.baseline_y(0, 540.0) - 550.0).abs() < 1e-6);
        assert!((layout.baseline_y(2, 540.0) - 710.0).abs() < 1e-6);
    }

    #[test]
    fn all_words_indexes_across_line_breaks() {
        // Two lines, three words total. The index must keep counting across the break, because
        // the per-word reveals stagger off it: a restart at line 2 would put its first word ahead
        // of line 1's last word.
        let line = |text: &str, x: f32| PlacedLine {
            words: vec![PlacedWord {
                text: text.to_owned(),
                x,
                width: 10.0,
            }],
            width: 10.0,
        };
        let layout = LineLayout {
            lines: vec![line("alpha", 0.0), line("beta", 0.0), line("gamma", 0.0)],
            line_step: 80.0,
            first_baseline_dy: 10.0,
        };

        let words = layout.all_words(540.0);
        assert_eq!(words.len(), 3);
        assert_eq!(
            words.iter().map(|(index, ..)| *index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        // The baselines still step by line, one `line_step` apart.
        assert_eq!(words[0].2, 550.0);
        assert_eq!(words[1].2, 630.0);
        assert_eq!(words[2].2, 710.0);
    }

    #[test]
    fn caches_are_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<LayoutCache>();
        assert_sync::<TextMeters>();
    }

    /// A meter for one of the files in `media/`, as the crate's own registry builds them.
    fn embedded(file: &str) -> TextMeter {
        let bytes = std::fs::read(format!("{}/media/{file}", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|e| panic!("media/{file} is in the repo: {e}"));
        TextMeter::new(file, bytes).expect("the embedded font parses")
    }

    /// The DM Sans face plus the seven Inter/Montserrat instances, exactly as `embedded_meters`
    /// assembles them.
    fn registry() -> TextMeters {
        let dm_sans = std::fs::read(format!(
            "{}/media/DMSans-Medium.ttf",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("the original embedded font is still in the repo");
        let mut meters = TextMeters::from_bytes(Some(dm_sans.clone()), Some(dm_sans));
        for file in [
            "Inter-Regular.ttf",
            "Inter-Medium.ttf",
            "Inter-SemiBold.ttf",
            "Inter-Bold.ttf",
            "Montserrat-Bold.ttf",
            "Montserrat-ExtraBold.ttf",
            "Montserrat-Black.ttf",
        ] {
            meters = meters.with_face(embedded(file));
        }
        meters
    }

    fn family_of<'m>(meters: &'m TextMeters, family: &str, weight: u16) -> &'m str {
        meters
            .for_family_weight(family, weight, &Fonts::default())
            .expect("the family is registered")
            .name()
    }

    #[test]
    fn every_face_is_registered_under_one_weighted_family() {
        let meters = registry();
        let fonts = Fonts::default();
        let inter: Vec<(String, u16)> = meters
            .faces()
            .iter()
            .filter(|face| face.matches_family("Inter"))
            .map(|face| (face.family().to_owned(), face.weight()))
            .collect();
        assert_eq!(inter.len(), 4, "all four Inter instances are registered");
        assert!(
            inter.iter().all(|(family, _)| family == "Inter"),
            "the renderer resolves every Inter instance as \"Inter\": {inter:?}"
        );
        assert_eq!(
            inter.iter().map(|(_, weight)| *weight).collect::<Vec<_>>(),
            vec![400, 500, 600, 700]
        );

        // A requested weight lands on the file with exactly that weight class.
        assert_eq!(family_of(&meters, "Inter", 600), "Inter-SemiBold.ttf");
        assert_eq!(family_of(&meters, "Inter", 400), "Inter-Regular.ttf");
        assert_eq!(family_of(&meters, "Inter", 700), "Inter-Bold.ttf");

        // Montserrat only ships 700/800/900.
        assert_eq!(
            family_of(&meters, "Montserrat", 900),
            "Montserrat-Black.ttf"
        );
        assert_eq!(family_of(&meters, "Montserrat", 700), "Montserrat-Bold.ttf");

        // A family with no weighted faces falls back to its only face. `Fonts::default` points
        // both slots at the same DM Sans bytes, so the mono slot answers for "DM Sans".
        assert_eq!(family_of(&meters, "DM Sans", 700), "mono");
        assert_eq!(family_of(&meters, "DM Sans", 400), "mono");
        // And a family nobody embedded falls back to the sans face too.
        assert_eq!(family_of(&meters, "Comic Sans", 400), "sans");
        assert!(meters.for_family_weight("Inter", 400, &fonts).is_some());
    }

    #[test]
    fn a_weighted_family_measures_as_its_own_regular_face() {
        let meters = registry();
        let fonts = Fonts::default();

        // Naming no weight must not silently measure DM Sans for a family this crate embeds.
        assert_eq!(
            meters.for_family("Inter", &fonts).map(TextMeter::name),
            Some("Inter-Regular.ttf")
        );
        assert_eq!(
            meters.for_family("Montserrat", &fonts).map(TextMeter::name),
            Some("Montserrat-Bold.ttf"),
            "Montserrat has no 400, so its lightest face is the regular one"
        );
        // Font databases match family names case-insensitively.
        assert_eq!(
            meters.for_family("inter", &fonts).map(TextMeter::name),
            Some("Inter-Regular.ttf")
        );
        // The pre-existing families are untouched.
        assert_eq!(
            meters.for_family("DM Sans", &fonts).map(TextMeter::name),
            Some("mono")
        );
    }

    #[test]
    fn nearest_weight_resolves_ties_downward() {
        let meters = registry();
        let fonts = Fonts::default();
        let at = |weight| {
            meters
                .for_family_weight("Montserrat", weight, &fonts)
                .expect("registered")
                .name()
        };
        // 700 and 800 are one step from 750 either way; the lighter face wins.
        assert_eq!(at(750), "Montserrat-Bold.ttf");
        assert_eq!(at(849), "Montserrat-ExtraBold.ttf");
        // 850 is again a tie, and again the lighter face wins.
        assert_eq!(at(850), "Montserrat-ExtraBold.ttf");
        assert_eq!(at(851), "Montserrat-Black.ttf");
        // Below the range clamps to the lightest face, above it to the heaviest.
        assert_eq!(at(100), "Montserrat-Bold.ttf");
        assert_eq!(at(1000), "Montserrat-Black.ttf");
        assert_eq!(at(0), "Montserrat-Bold.ttf");
    }

    #[test]
    fn a_weighted_measurement_uses_the_weighted_face() {
        let meters = registry();
        // The four Inter weights have different advances, so a weight request must change the
        // measured width; a single-face family must not.
        let width = |family: &str, weight: u16| {
            meters
                .for_family_weight(family, weight, &Fonts::default())
                .expect("registered")
                .text_width("Hamburgefonstiv", 100.0)
        };
        let regular = width("Inter", 400);
        assert!(
            width("Inter", 700) > regular,
            "Inter Bold should measure wider than Inter Regular: {} vs {regular}",
            width("Inter", 700)
        );
        assert!(
            width("Montserrat", 900) > width("Montserrat", 700),
            "Montserrat Black should measure wider than Montserrat Bold"
        );
        assert!(
            (width("DM Sans", 400) - width("DM Sans", 900)).abs() < f32::EPSILON,
            "DM Sans has one face, so its weight cannot change the measurement"
        );
    }
}
