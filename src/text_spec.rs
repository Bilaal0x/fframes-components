//! Shared configuration for the components that reveal a line of words.

/// Text styling and layout options the word-reveal components share.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpec {
    /// The line to render.
    pub text: String,
    /// Font size in px.
    pub font_size: f32,
    /// Font weight.
    pub font_weight: u16,
    /// Text fill.
    pub color: String,
    /// Wrap width in px.
    pub max_width: f32,
    /// Line height as a multiple of `font_size`.
    pub line_height: f32,
    /// Letter spacing in `em`. Negative tightens.
    pub tracking_em: f32,
}

impl Default for TextSpec {
    fn default() -> Self {
        Self {
            text: "Words appear one after another in sequence".to_owned(),
            font_size: 64.0,
            font_weight: 700,
            color: "#e6edf3".to_owned(),
            max_width: 900.0,
            line_height: 1.25,
            tracking_em: -0.02,
        }
    }
}

impl TextSpec {
    /// How many words the line has.
    pub fn word_count(&self) -> usize {
        self.text.split_whitespace().count()
    }

    /// Line step in px.
    pub fn line_step(&self) -> f32 {
        self.font_size * self.line_height
    }
}
