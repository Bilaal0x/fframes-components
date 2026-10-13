//! Deterministic text measurement from the font file itself.
//!
//! fframes' `Frame::text_width` borrows the measured string for the frame's whole lifetime, which
//! cannot work when a [`crate::RenderCtx`] owns its frame and components measure their own config
//! strings. Measuring with `ttf-parser` against an embedded face sidesteps that entirely: no
//! borrow of the frame, identical results on every machine, and no dependency on whether the
//! renderer has loaded its fonts yet.
//!
//! The face is parsed on each measurement rather than stored, which keeps this type free of
//! self-referential borrows. Measurements happen once per component at most (components cache
//! their resolved layout), so the parse cost is paid a handful of times per render.

use ttf_parser::{Face, GlyphId, name_id};

/// The bytes of a font file, able to measure text at any size.
#[derive(Debug, Clone)]
pub struct TextMeter {
    bytes: Vec<u8>,
    name: String,
}

impl TextMeter {
    /// Wrap the bytes of a font file.
    pub fn new(name: impl Into<String>, bytes: Vec<u8>) -> Option<Self> {
        {
            let face = Face::parse(&bytes, 0).ok()?;
            // Touch the face so obviously invalid files are rejected up front.
            let _ = face.units_per_em();
        }
        Some(Self {
            bytes,
            name: name.into(),
        })
    }

    /// A label for diagnostics.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Borrow the font bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The family name a renderer resolves this face under.
    ///
    /// This mirrors what the renderer's own font database does: the *typographic* family (name
    /// table ID 16) wins when the file defines one, otherwise the legacy family (ID 1) is used.
    /// A static instance that only sets ID 1 therefore registers as its own family, e.g.
    /// "Inter SemiBold" instead of "Inter" at weight 600.
    pub fn family_name(&self) -> Option<String> {
        self.face_name(name_id::TYPOGRAPHIC_FAMILY)
            .or_else(|| self.face_name(name_id::FAMILY))
    }

    /// The legacy family name (name table ID 1), whatever the typographic family says.
    pub fn legacy_family_name(&self) -> Option<String> {
        self.face_name(name_id::FAMILY)
    }

    /// The weight class the face declares in `OS/2`, or 400 when it declares none.
    pub fn weight(&self) -> u16 {
        self.with_face(|face| match face.tables().os2 {
            // A missing or zeroed weight class means "regular", not "weightless".
            Some(os2) => os2.weight().to_number().max(1),
            None => 400,
        })
        .unwrap_or(400)
    }

    fn face_name(&self, wanted: u16) -> Option<String> {
        self.with_face(|face| {
            face.names()
                .into_iter()
                .filter(|name| name.name_id == wanted)
                .find_map(|name| name.to_string())
        })
        .flatten()
    }

    fn with_face<T>(&self, f: impl FnOnce(&Face<'_>) -> T) -> Option<T> {
        Face::parse(&self.bytes, 0).ok().map(|face| f(&face))
    }

    /// The face's `units_per_em`.
    pub fn units_per_em(&self) -> f32 {
        self.with_face(|face| f32::from(face.units_per_em()))
            .unwrap_or(1000.0)
    }

    /// Advance width of one character at `size` px.
    pub fn char_width(&self, ch: char, size: f32) -> f32 {
        self.with_face(|face| {
            let upem = f32::from(face.units_per_em());
            match face.glyph_index(ch) {
                // A missing glyph falls back to a sensible average so layout never collapses.
                None => size * 0.5,
                Some(glyph) => match face.glyph_hor_advance(glyph) {
                    Some(advance) => f32::from(advance) / upem * size,
                    None => size * 0.5,
                },
            }
        })
        .unwrap_or(size * 0.5)
    }

    /// Width of a string at `size` px, including pair kerning when the face defines it.
    pub fn text_width(&self, text: &str, size: f32) -> f32 {
        self.with_face(|face| {
            let upem = f32::from(face.units_per_em());
            let kern = face.tables().kern;
            let mut width = 0.0_f32;
            let mut previous: Option<GlyphId> = None;
            for ch in text.chars() {
                let Some(glyph) = face.glyph_index(ch) else {
                    width += size * 0.5;
                    previous = None;
                    continue;
                };
                if let (Some(prev), Some(kern)) = (previous, kern)
                    && let Some(pair) = kern
                        .subtables
                        .into_iter()
                        .filter(|s| s.horizontal && !s.variable)
                        .find_map(|s| s.glyphs_kerning(prev, glyph))
                {
                    width += f32::from(pair) / upem * size;
                }
                width += match face.glyph_hor_advance(glyph) {
                    Some(advance) => f32::from(advance) / upem * size,
                    None => size * 0.5,
                };
                previous = Some(glyph);
            }
            width
        })
        .unwrap_or_else(|| text.chars().count() as f32 * size * 0.55)
    }

    /// Width of a single space glyph at `size` px.
    pub fn space_width(&self, size: f32) -> f32 {
        self.char_width(' ', size)
    }

    /// Ascent above the baseline at `size` px.
    pub fn ascent(&self, size: f32) -> f32 {
        self.with_face(|face| f32::from(face.ascender()) / f32::from(face.units_per_em()) * size)
            .unwrap_or(size * 0.75)
    }

    /// Descent below the baseline at `size` px (positive).
    pub fn descent(&self, size: f32) -> f32 {
        self.with_face(|face| {
            f32::from(face.descender()).abs() / f32::from(face.units_per_em()) * size
        })
        .unwrap_or(size * 0.25)
    }

    /// The face's own suggested line height at `size` px.
    pub fn line_height(&self, size: f32) -> f32 {
        self.with_face(|face| {
            let upem = f32::from(face.units_per_em());
            let ascender = face.ascender();
            let descender = face.descender();
            let gap = face.line_gap();
            let height = gap
                .checked_add(ascender)
                .and_then(|v| v.checked_sub(descender))
                .unwrap_or(ascender - descender);
            f32::from(height) / upem * size
        })
        .unwrap_or(size * 1.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meter() -> TextMeter {
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/media/DMSans-Medium.ttf"
        ))
        .expect("the embedded font is in the repo");
        TextMeter::new("DM Sans", bytes).expect("the embedded font parses")
    }

    #[test]
    fn rejects_non_font_bytes() {
        assert!(TextMeter::new("junk", vec![0, 1, 2, 3]).is_none());
    }

    #[test]
    fn measures_text_monotonically() {
        let meter = meter();
        let short = meter.text_width("ab", 100.0);
        let long = meter.text_width("abcd", 100.0);
        assert!(short > 0.0);
        assert!(long > short, "{long} should exceed {short}");
    }

    #[test]
    fn scales_linearly_with_size() {
        let meter = meter();
        let at_50 = meter.text_width("Hello", 50.0);
        let at_100 = meter.text_width("Hello", 100.0);
        assert!(
            (at_100 - at_50 * 2.0).abs() < 0.5,
            "expected linear scaling, got {at_50} and {at_100}"
        );
    }

    #[test]
    fn space_is_narrower_than_a_letter() {
        let meter = meter();
        assert!(meter.space_width(100.0) < meter.char_width('m', 100.0));
    }

    #[test]
    fn metrics_are_sane() {
        let meter = meter();
        assert!(meter.units_per_em() > 0.0);
        assert!(meter.ascent(100.0) > 0.0);
        assert!(meter.descent(100.0) > 0.0);
        assert!(meter.line_height(100.0) > 0.0);
    }

    #[test]
    fn unknown_glyph_does_not_collapse_to_zero() {
        let meter = meter();
        // A private-use codepoint the face almost certainly lacks.
        assert!(meter.char_width('\u{E000}', 100.0) > 0.0);
    }

    /// Read an embedded face by file name, the way the crate's own meters do.
    fn embedded(file: &str) -> TextMeter {
        let bytes = std::fs::read(format!("{}/media/{file}", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|e| panic!("media/{file} is in the repo: {e}"));
        TextMeter::new(file, bytes).expect("the embedded font parses")
    }

    /// The family names and weights inside every newly embedded face.
    ///
    /// Static instances of one family routinely disagree about the *legacy* family name: Inter
    /// 4.1's Medium and SemiBold files declare "Inter Medium" / "Inter SemiBold" with subfamily
    /// "Regular", while Regular and Bold declare "Inter". The renderer's font database prefers
    /// the typographic family (ID 16), so all four collapse onto "Inter" with their own weight —
    /// that is the `font-family` a component must emit. The per-file legacy names are asserted
    /// too, because they are what a renderer that ignores ID 16 would need.
    #[test]
    fn reports_the_family_and_weight_of_every_new_face() {
        // (file, renderer family, legacy family, weight class)
        let expected = [
            ("Inter-Regular.ttf", "Inter", "Inter", 400_u16),
            ("Inter-Medium.ttf", "Inter", "Inter Medium", 500),
            ("Inter-SemiBold.ttf", "Inter", "Inter SemiBold", 600),
            ("Inter-Bold.ttf", "Inter", "Inter", 700),
            ("Montserrat-Bold.ttf", "Montserrat", "Montserrat", 700),
            (
                "Montserrat-ExtraBold.ttf",
                "Montserrat",
                "Montserrat ExtraBold",
                800,
            ),
            (
                "Montserrat-Black.ttf",
                "Montserrat",
                "Montserrat Black",
                900,
            ),
        ];

        for (file, family, legacy, weight) in expected {
            let meter = embedded(file);
            println!(
                "{file}: family={:?} legacy={:?} weight={}",
                meter.family_name(),
                meter.legacy_family_name(),
                meter.weight()
            );
            assert_eq!(meter.family_name().as_deref(), Some(family), "{file}");
            assert_eq!(
                meter.legacy_family_name().as_deref(),
                Some(legacy),
                "{file}"
            );
            assert_eq!(meter.weight(), weight, "{file}");
        }
    }

    /// DM Sans is a single face that must keep reporting exactly what it reported before.
    #[test]
    fn dm_sans_keeps_its_own_name_and_weight() {
        let meter = embedded("DMSans-Medium.ttf");
        println!(
            "DMSans-Medium.ttf: family={:?} legacy={:?} weight={}",
            meter.family_name(),
            meter.legacy_family_name(),
            meter.weight()
        );
        assert_eq!(meter.family_name().as_deref(), Some("DM Sans"));
        assert_eq!(
            meter.legacy_family_name().as_deref(),
            Some("DM Sans Medium")
        );
        assert_eq!(meter.weight(), 500);
    }
}
