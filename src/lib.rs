//! # fframes-components
//!
//! An isolated, mix-and-match library of animated video components for
//! [fframes](https://github.com/dmtrKovalenko/fframes), ported from the Remotion component
//! library at <https://github.com/Remocn/remocn>.
//!
//! ## What it is
//!
//! Each component is **isolated**: it owns its own configuration struct, lives in its own module,
//! and is compiled in only when you ask for it. You then pick which ones you want, either at
//! compile time (Cargo features) or at render time (`--components`).
//!
//! ```no_run
//! use fframes_components::{ComponentsVideo, select};
//!
//! // Compile-time: only the enabled Cargo features exist in the catalog.
//! // Runtime: pick what actually renders.
//! let video = ComponentsVideo::from_spec("type-writer,text-reveal,glitch-cycle").unwrap();
//! println!("{}", video.timeline_report());
//! ```
//!
//! ## Selection
//!
//! | how | where | effect |
//! | --- | --- | --- |
//! | Cargo features | `Cargo.toml` | which components get **compiled** |
//! | `--components a,b` | the CLI | which compiled components **render**, and in what order |
//! | `--list` | the CLI | print every compiled component with its id and options |
//!
//! A component that is not compiled in does not exist: `selection()` reports it as unknown and
//! `--list` does not show it. That keeps unused components out of the binary.
//!
//! ## Components
//!
//! Every component's module carries its own `README.md` documenting every option with its type,
//! default and meaning. Start at [`catalog()`] for the list this build contains.
//!
//! ## Credit
//!
//! The animations, their timing constants, easing vocabulary and defaults are ported from the
//! Remotion component library by the **remocn** authors (<https://github.com/Remocn/remocn>, MIT).
//! This crate is a reimplementation against fframes' SVG + Rust API and is not affiliated with
//! Remotion. See `CREDITS.md`.

#![deny(missing_docs)]

pub mod blur;
pub mod core;
pub mod ctx;
pub mod meter;
pub mod motion;
pub mod text_spec;
pub mod ui;

pub mod components {
    //! One module per component, each behind its own Cargo feature.
    //!
    //! Turn on a component feature (or `full`) to compile a module in.

    /// Cinematic UI components, each behind its own feature.
    pub mod showcase;

    #[cfg(feature = "bottom-up-letters")]
    pub mod bottom_up_letters;
    #[cfg(feature = "carousel-3d")]
    pub mod carousel_3d;
    #[cfg(feature = "confetti")]
    pub mod confetti;
    #[cfg(feature = "cursor-flyover")]
    pub mod cursor_flyover;
    #[cfg(feature = "echo-stack")]
    pub mod echo_stack;
    #[cfg(feature = "glitch-cycle")]
    pub mod glitch_cycle;
    #[cfg(feature = "inflating-text")]
    pub mod inflating_text;
    #[cfg(feature = "lines-to-text")]
    pub mod lines_to_text;
    #[cfg(feature = "multi-text-typewriter")]
    pub mod multi_text_typewriter;
    #[cfg(feature = "per-character-rise")]
    pub mod per_char_rise;
    #[cfg(feature = "period-drop")]
    pub mod period_drop;
    #[cfg(feature = "short-slide-right")]
    pub mod short_slide_right;
    #[cfg(feature = "slide-in-text")]
    pub mod slide_in_text;
    #[cfg(feature = "soft-blur-in")]
    pub mod soft_blur_in;
    #[cfg(feature = "spring-scale-in")]
    pub mod spring_scale_in;
    #[cfg(feature = "squeeze-in")]
    pub mod squeeze_in;
    #[cfg(feature = "staggered-fade-up")]
    pub mod staggered_fade_up;
    #[cfg(feature = "text-build")]
    pub mod text_build;
    #[cfg(feature = "text-messages")]
    pub mod text_messages;
    #[cfg(feature = "text-reveal")]
    pub mod text_reveal;
    #[cfg(feature = "text-swap")]
    pub mod text_swap;
    #[cfg(feature = "text-swell")]
    pub mod text_swell;
    #[cfg(feature = "top-down-letters")]
    pub mod top_down_letters;
    #[cfg(feature = "tracking-in")]
    pub mod tracking_in;
    #[cfg(feature = "type-writer")]
    pub mod type_writer;
    #[cfg(feature = "typing-code-block")]
    pub mod typing_code_block;
    #[cfg(feature = "word-by-word")]
    pub mod word_by_word;
    #[cfg(feature = "word-flip")]
    pub mod word_flip;
}

mod catalog;

pub use crate::catalog::REGISTRY;
pub use crate::core::{
    CANVAS_FPS, CANVAS_HEIGHT, CANVAS_WIDTH, Canvas, CatalogEntry, Component, ComponentFactory,
    ComponentMeta, ComponentsVideo, Fonts, Horizontal, Landscape, Portrait, Slot, UnknownComponent,
    Vertical, catalog, find, selection,
};
pub use crate::ctx::{
    ComponentFrame, FaceMeter, FrameRef, LayoutCache, LineLayout, PlacedLine, PlacedWord,
    RenderCtx, TextMeters, centered_positions, char_centers, chars, escape_xml, font_weight_attr,
    interpolate3, layout_paragraph, svg_transform, words,
};
pub use crate::meter::TextMeter;
pub use crate::motion::{CubicBezier, Spring};
pub use crate::text_spec::TextSpec;

/// Build a landscape (16:9) video from a `--components` selection string.
///
/// A convenience alias for [`ComponentsVideo::from_spec`] on the [`Landscape`] canvas. For the
/// 9:16 canvas, annotate the binding instead:
///
/// ```no_run
/// use fframes_components::{ComponentsVideo, Vertical};
///
/// let tall: Vertical = ComponentsVideo::from_spec("text-messages").unwrap();
/// ```
pub fn video_from_spec(spec: &str) -> Result<Horizontal, UnknownComponent> {
    ComponentsVideo::from_spec(spec)
}

/// Build a portrait (9:16) video from a `--components` selection string.
pub fn vertical_video_from_spec(spec: &str) -> Result<Vertical, UnknownComponent> {
    ComponentsVideo::from_spec(spec)
}

/// Build components for a selection, in catalog order.
pub fn build_selection(spec: &str) -> Result<Vec<Box<dyn Component>>, UnknownComponent> {
    Ok(selection(spec)?.iter().map(|entry| entry.build()).collect())
}

// Every media file embedded in this library, so a host video can build its media provider.
//
// The generated struct's fields are named after the files and cannot carry doc comments, so the
// missing-docs lint is allowed for this one module.
#[allow(missing_docs)]
mod embedded_media {
    fframes::include_media_dir!(pub struct ComponentsMedia, "media");
}
pub use embedded_media::ComponentsMedia;

/// The font family the original embedded face registers as.
pub const EMBEDDED_FONT_FAMILY: &str = "DM Sans";

/// The family the embedded Inter weights (400, 500, 600, 700) register as.
///
/// The Inter 4.1 static instances are separate files per weight, but the renderer resolves all
/// four as family `Inter` and picks between them by `font-weight`, so components emit this one
/// family and the numeric weight.
pub const INTER_FAMILY: &str = "Inter";

/// The family the embedded Montserrat weights (700, 800, 900) register as.
///
/// Same story as [`INTER_FAMILY`]: the static instances all resolve as `Montserrat`, selected by
/// `font-weight`.
pub const MONTSERRAT_FAMILY: &str = "Montserrat";

/// Every face in `media/` that is *not* the original DM Sans, as `(diagnostic label, bytes)`.
///
/// Keeping the list in one place means the meter registry and the tests that prove the family
/// names cannot drift apart.
fn embedded_weighted_faces(media: &ComponentsMedia) -> [(&'static str, &'static [u8]); 7] {
    [
        ("Inter Regular", media.inter_regular_ttf),
        ("Inter Medium", media.inter_medium_ttf),
        ("Inter SemiBold", media.inter_semibold_ttf),
        ("Inter Bold", media.inter_bold_ttf),
        ("Montserrat Bold", media.montserrat_bold_ttf),
        ("Montserrat ExtraBold", media.montserrat_extrabold_ttf),
        ("Montserrat Black", media.montserrat_black_ttf),
    ]
}

/// Build the font meters from the faces embedded in this crate.
///
/// The sans and mono slots keep pointing at exactly the bytes they always have (DM Sans Medium),
/// so every component ported before the Inter/Montserrat faces existed still measures and draws
/// precisely as it did. The other seven files are registered as *weighted* faces on top of that:
/// [`RenderCtx::text_width_at`] resolves them by family and weight, and
/// [`TextMeters::for_family_weight`] is where that selection happens.
///
/// A file that fails to parse is skipped rather than panicking, so a broken font cannot take the
/// whole library down at startup.
pub fn embedded_meters() -> TextMeters {
    let Ok(media) = ComponentsMedia::new() else {
        return TextMeters::default();
    };

    let sans = media.dmsans_medium_ttf.to_vec();
    let mut meters = TextMeters::from_bytes(Some(sans.clone()), Some(sans));
    for (label, bytes) in embedded_weighted_faces(&media) {
        if let Some(meter) = TextMeter::new(label, bytes.to_vec()) {
            meters = meters.with_face(meter);
        }
    }
    meters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_kebab_case() {
        let mut seen = std::collections::HashSet::new();
        for entry in catalog() {
            let id = entry.meta().id;
            assert!(seen.insert(id), "duplicate component id: {id}");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "id is not kebab-case: {id}"
            );
            assert!(!entry.meta().summary.is_empty(), "{id} has no summary");
            assert!(
                entry.meta().feature.starts_with("blur-slide-text")
                    || !entry.meta().feature.is_empty(),
                "{id} has no feature"
            );
        }
    }

    #[test]
    fn every_catalog_entry_builds_and_reports_a_duration() {
        for entry in catalog() {
            let component = entry.build();
            assert_eq!(component.meta().id, entry.meta().id);
            assert!(
                component.duration() > 0,
                "{} has zero duration",
                entry.meta().id
            );
        }
    }

    #[test]
    fn select_all_matches_the_catalog() {
        let all = selection("all").expect("all selects");
        assert_eq!(all.len(), catalog().len());
        let empty = selection("").expect("empty selects everything");
        assert_eq!(empty.len(), catalog().len());
    }

    #[test]
    fn select_unknown_component_is_an_error() {
        let err = selection("nope-not-a-component").unwrap_err();
        assert_eq!(err.0, "nope-not-a-component");
        assert!(err.to_string().contains("unknown component"));
    }

    #[test]
    fn select_is_case_insensitive_and_deduplicates() {
        let Some(first) = catalog().first() else {
            return;
        };
        let id = first.meta().id;
        let chosen = selection(&format!("{},{}", id.to_uppercase(), id)).unwrap();
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].meta().id, id);
    }

    #[test]
    fn selection_follows_catalog_order_not_user_order() {
        if catalog().len() < 2 {
            return;
        }
        let first = catalog()[0].meta().id;
        let second = catalog()[1].meta().id;
        let chosen = selection(&format!("{second},{first}")).unwrap();
        let ids: Vec<_> = chosen.iter().map(|c| c.meta().id).collect();
        assert_eq!(ids, vec![first, second]);
    }

    #[test]
    fn timeline_is_contiguous_and_covers_every_frame() {
        let video: Horizontal = ComponentsVideo::from_spec("all").expect("all");
        let slots = video.slots();
        assert_eq!(slots.len(), video.components().len());
        let mut expected = 0;
        for slot in slots {
            assert_eq!(slot.start, expected, "timeline has a gap or overlap");
            assert!(
                !slot.is_empty(),
                "empty slot for {}",
                video.components()[slot.index].meta().id
            );
            expected = slot.end;
        }
        assert_eq!(video.total_frames(), expected);
    }

    #[test]
    fn slot_at_covers_the_whole_timeline() {
        let video: Horizontal = ComponentsVideo::from_spec("all").expect("all");
        for frame in 0..video.total_frames() {
            let slot = video
                .slot_at(frame)
                .unwrap_or_else(|| panic!("frame {frame} is not covered"));
            assert!(frame >= slot.start && frame < slot.end);
        }
        assert!(video.slot_at(video.total_frames()).is_none());
    }

    #[test]
    fn timeline_report_lists_every_component() {
        let video: Horizontal = ComponentsVideo::from_spec("all").expect("all");
        let report = video.timeline_report();
        for component in video.components() {
            assert!(
                report.contains(component.meta().id),
                "report is missing {}",
                component.meta().id
            );
        }
    }

    /// The id of whatever this build happens to contain, so these canvas tests do not depend on
    /// one particular feature being enabled. `cargo test --no-default-features --features <one>`
    /// is part of the component contract, and a hard-coded id breaks it.
    fn any_component_id() -> String {
        catalog()
            .first()
            .map(|entry| entry.meta().id.to_owned())
            .expect("a build with no components at all")
    }

    #[test]
    fn landscape_and_portrait_report_their_own_geometry() {
        let id = any_component_id();
        let wide: Horizontal = ComponentsVideo::from_spec(&id).expect("selection");
        let tall: Vertical = ComponentsVideo::from_spec(&id).expect("selection");

        assert_eq!((wide.width(), wide.height()), (1920, 1080));
        assert_eq!((tall.width(), tall.height()), (1080, 1920));
        assert_eq!(wide.fps(), tall.fps());

        // The trait constants the renderer reads must agree with the accessors.
        assert_eq!(<Horizontal as fframes::Video>::WIDTH, 1920);
        assert_eq!(<Horizontal as fframes::Video>::HEIGHT, 1080);
        assert_eq!(<Vertical as fframes::Video>::WIDTH, 1080);
        assert_eq!(<Vertical as fframes::Video>::HEIGHT, 1920);
    }

    #[test]
    fn the_canvas_does_not_change_the_timeline() {
        // Components are the same code on either canvas; only the geometry they lay out against
        // changes, so the frame budget must be identical.
        let wide: Horizontal = ComponentsVideo::from_spec("all").expect("all");
        let tall: Vertical = ComponentsVideo::from_spec("all").expect("all");

        assert_eq!(wide.total_frames(), tall.total_frames());
        assert_eq!(wide.slots(), tall.slots());
    }

    #[test]
    fn timeline_report_names_the_canvas() {
        let id = any_component_id();
        let tall: Vertical = ComponentsVideo::from_spec(&id).expect("selection");
        let report = tall.timeline_report();
        assert!(
            report.contains("1080x1920"),
            "the report must state the portrait geometry: {report}"
        );
        assert!(report.contains("portrait 9:16"), "{report}");
    }

    #[test]
    fn embedded_meters_register_every_face_at_its_declared_weight() {
        let meters = embedded_meters();

        // The 43 components that predate the new faces still resolve to DM Sans. `Fonts::default`
        // points both slots at the same DM Sans bytes, so the mono slot answers for "DM Sans".
        assert_eq!(
            meters
                .for_family("DM Sans", &Fonts::default())
                .map(TextMeter::name),
            Some("mono")
        );

        let mut found: Vec<(String, u16)> = meters
            .faces()
            .iter()
            .map(|face| (face.family().to_owned(), face.weight()))
            .collect();
        found.sort();
        println!("embedded weighted faces: {found:?}");

        assert_eq!(
            found,
            vec![
                (INTER_FAMILY.to_owned(), 400),
                (INTER_FAMILY.to_owned(), 500),
                (INTER_FAMILY.to_owned(), 600),
                (INTER_FAMILY.to_owned(), 700),
                (MONTSERRAT_FAMILY.to_owned(), 700),
                (MONTSERRAT_FAMILY.to_owned(), 800),
                (MONTSERRAT_FAMILY.to_owned(), 900),
            ],
            "the family constants must match what the files declare"
        );

        // Weight-aware measurement resolves to the right file, and the old API is untouched.
        for (family, weight, expected) in [
            (INTER_FAMILY, 400, "Inter Regular"),
            (INTER_FAMILY, 600, "Inter SemiBold"),
            (INTER_FAMILY, 700, "Inter Bold"),
            (MONTSERRAT_FAMILY, 800, "Montserrat ExtraBold"),
            (MONTSERRAT_FAMILY, 900, "Montserrat Black"),
        ] {
            let meters = embedded_meters();
            let Some(meter) = meters.for_family_weight(family, weight, &Fonts::default()) else {
                panic!("{family} {weight} has no meter");
            };
            assert_eq!(meter.name(), expected, "{family} {weight}");
        }
    }
}
