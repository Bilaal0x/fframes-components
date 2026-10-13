//! Concrete light and dark design-token palettes for the component collection.
//!
//! The values are literal strings, including the shared primary accent and ten-pixel radius.

/// Which of shadcn's two palettes a component paints from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// shadcn's light palette — the registry's own default.
    #[default]
    Light,
    /// shadcn's dark palette, for rendering onto a dark frame.
    Dark,
}

/// Shared UI design tokens as concrete colours.
///
/// Every field is a concrete colour string, never a `var(--token)`: a Remotion bundle (and this
/// crate) has none of an app's CSS, so `var()` resolves to nothing and cannot be interpolated.
#[derive(Debug, Clone, PartialEq)]
pub struct UiTheme {
    /// The page behind everything.
    pub background: String,
    /// Primary text.
    pub foreground: String,
    /// A surface sitting on the page.
    pub card: String,
    /// Text on [`UiTheme::card`].
    pub card_foreground: String,
    /// A floating surface.
    pub popover: String,
    /// Text on [`UiTheme::popover`].
    pub popover_foreground: String,
    /// The primary accent.
    pub primary: String,
    /// Text on [`UiTheme::primary`].
    pub primary_foreground: String,
    /// A subtle fill.
    pub secondary: String,
    /// Text on [`UiTheme::secondary`].
    pub secondary_foreground: String,
    /// The quietest fill.
    pub muted: String,
    /// Secondary text and leading icons.
    pub muted_foreground: String,
    /// The hover wash.
    pub accent: String,
    /// Text on [`UiTheme::accent`].
    pub accent_foreground: String,
    /// The error colour.
    pub destructive: String,
    /// Text on [`UiTheme::destructive`].
    pub destructive_foreground: String,
    /// The hairline: separation is borders, not shadows.
    pub border: String,
    /// The hairline of an input.
    pub input: String,
    /// The focus ring.
    pub ring: String,
    /// Corner radius in reference-canvas px.
    pub radius: f32,
    /// The face every text component paints with, when a caller wants to re-skin a whole
    /// timeline from one value. `None` keeps the face a component was designed around, which is
    /// what the source's `"Default"` font option means.
    pub font_family: Option<String>,
}

impl UiTheme {
    /// The light palette.
    #[must_use]
    pub fn light() -> Self {
        Self {
            background: "#faf9f6".to_owned(),
            foreground: "#141414".to_owned(),
            card: "#ffffff".to_owned(),
            card_foreground: "#141414".to_owned(),
            popover: "#ffffff".to_owned(),
            popover_foreground: "#141414".to_owned(),
            primary: "#3072db".to_owned(),
            primary_foreground: "#ffffff".to_owned(),
            secondary: "#f2f0eb".to_owned(),
            secondary_foreground: "#141414".to_owned(),
            muted: "#f2f0eb".to_owned(),
            muted_foreground: "#6e6a63".to_owned(),
            accent: "#f2f0eb".to_owned(),
            accent_foreground: "#141414".to_owned(),
            destructive: "#d92d20".to_owned(),
            destructive_foreground: "#ffffff".to_owned(),
            border: "#d9d9d9".to_owned(),
            input: "#d9d9d9".to_owned(),
            ring: "#3072db".to_owned(),
            radius: 10.0,
            font_family: None,
        }
    }

    /// shadcn's dark palette, exactly as `defaultDarkTheme` in the source.
    #[must_use]
    pub fn dark() -> Self {
        Self {
            background: "#0a0a0b".to_owned(),
            foreground: "#fafafa".to_owned(),
            card: "#141417".to_owned(),
            card_foreground: "#fafafa".to_owned(),
            popover: "#141417".to_owned(),
            popover_foreground: "#fafafa".to_owned(),
            primary: "#3072db".to_owned(),
            primary_foreground: "#ffffff".to_owned(),
            secondary: "#1d1d21".to_owned(),
            secondary_foreground: "#fafafa".to_owned(),
            muted: "#1b1b1f".to_owned(),
            muted_foreground: "#a1a1aa".to_owned(),
            accent: "#1d1d21".to_owned(),
            accent_foreground: "#fafafa".to_owned(),
            destructive: "#f97066".to_owned(),
            destructive_foreground: "#fafafa".to_owned(),
            border: "#26272b".to_owned(),
            input: "#26272b".to_owned(),
            ring: "#3072db".to_owned(),
            radius: 10.0,
            font_family: None,
        }
    }

    /// Resolve the concrete palette for a mode when there is no override.
    #[must_use]
    pub fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Light => Self::light(),
            Mode::Dark => Self::dark(),
        }
    }
}

impl Default for UiTheme {
    fn default() -> Self {
        // Components use the light palette unless configured otherwise.
        Self::light()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_are_distinct_but_share_the_accent() {
        let light = UiTheme::light();
        let dark = UiTheme::dark();
        assert_ne!(light.background, dark.background);
        assert_eq!(light.primary, dark.primary);
        assert_eq!(light.radius, dark.radius);
    }

    #[test]
    fn the_default_is_the_light_palette() {
        assert_eq!(UiTheme::default(), UiTheme::light());
        assert_eq!(UiTheme::for_mode(Mode::default()), UiTheme::light());
        assert_eq!(Mode::default(), Mode::Light);
    }

    #[test]
    fn every_token_is_a_concrete_colour() {
        for theme in [UiTheme::light(), UiTheme::dark()] {
            for token in [
                &theme.background,
                &theme.foreground,
                &theme.card,
                &theme.primary,
                &theme.muted,
                &theme.border,
                &theme.ring,
            ] {
                assert!(
                    token.starts_with('#'),
                    "{token} should be a concrete hex colour"
                );
                assert!(
                    !token.contains("var("),
                    "{token} must not be a CSS variable"
                );
            }
        }
    }
}
