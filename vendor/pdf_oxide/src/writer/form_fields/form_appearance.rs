//! Form field appearance stream generation.
//!
//! Generates visual appearance streams for interactive form fields.
//! These streams define how fields appear when printed or displayed.
//!
//! Note: When `NeedAppearances` is set in the AcroForm dictionary,
//! PDF viewers will regenerate appearances. This module provides
//! fallback appearances for compatibility.

use crate::geometry::Rect;

/// Generator for form field appearance streams.
///
/// Creates PDF content streams that define the visual appearance of form fields.
#[derive(Debug, Clone, Default)]
pub struct FormAppearanceGenerator {
    /// Border width
    border_width: f32,
    /// Border color (RGB)
    border_color: Option<(f32, f32, f32)>,
    /// Background color (RGB)
    background_color: Option<(f32, f32, f32)>,
}

impl FormAppearanceGenerator {
    /// Create a new appearance generator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set border style.
    pub fn with_border(mut self, width: f32, r: f32, g: f32, b: f32) -> Self {
        self.border_width = width;
        self.border_color = Some((r, g, b));
        self
    }

    /// Set background color.
    pub fn with_background(mut self, r: f32, g: f32, b: f32) -> Self {
        self.background_color = Some((r, g, b));
        self
    }

    /// Generate appearance stream for a text field.
    ///
    /// # Arguments
    ///
    /// * `rect` - Field bounding rectangle
    /// * `text` - Current text value
    /// * `font_name` - Font resource name (e.g., "/Helv")
    /// * `font_size` - Font size in points
    /// * `text_color` - RGB color (0.0-1.0)
    pub fn text_field_appearance(
        &self,
        rect: Rect,
        text: &str,
        font_name: &str,
        font_size: f32,
        text_color: (f32, f32, f32),
    ) -> String {
        let mut stream = String::new();

        let width = rect.width;
        let height = rect.height;

        // Background
        if let Some((r, g, b)) = self.background_color {
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&format!("0 0 {} {} re f\n", width, height));
        }

        // Border
        if let Some((r, g, b)) = self.border_color {
            if self.border_width > 0.0 {
                stream.push_str(&format!("{} {} {} RG\n", r, g, b));
                stream.push_str(&format!("{} w\n", self.border_width));
                let half = self.border_width / 2.0;
                stream.push_str(&format!(
                    "{} {} {} {} re S\n",
                    half,
                    half,
                    width - self.border_width,
                    height - self.border_width
                ));
            }
        }

        // Text
        if !text.is_empty() {
            let (r, g, b) = text_color;
            let padding = 2.0;
            let y_pos = (height - font_size) / 2.0; // Center vertically

            stream.push_str("BT\n");
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&format!("{} {} Tf\n", font_name, font_size));
            stream.push_str(&format!("{} {} Td\n", padding, y_pos));
            stream.push_str(&format!("({}) Tj\n", escape_pdf_string(text)));
            stream.push_str("ET\n");
        }

        stream
    }

    /// Generate appearance stream for a checkbox (checked state).
    ///
    /// # Arguments
    ///
    /// * `rect` - Field bounding rectangle
    /// * `check_color` - RGB color for the checkmark
    pub fn checkbox_on_appearance(&self, rect: Rect, check_color: (f32, f32, f32)) -> String {
        let mut stream = String::new();

        let width = rect.width;
        let height = rect.height;

        // Background
        if let Some((r, g, b)) = self.background_color {
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&format!("0 0 {} {} re f\n", width, height));
        }

        // Border
        if let Some((r, g, b)) = self.border_color {
            if self.border_width > 0.0 {
                stream.push_str(&format!("{} {} {} RG\n", r, g, b));
                stream.push_str(&format!("{} w\n", self.border_width));
                let half = self.border_width / 2.0;
                stream.push_str(&format!(
                    "{} {} {} {} re S\n",
                    half,
                    half,
                    width - self.border_width,
                    height - self.border_width
                ));
            }
        }

        // Checkmark
        let (r, g, b) = check_color;
        let margin = width * 0.2;
        stream.push_str(&format!("{} {} {} RG\n", r, g, b));
        stream.push_str(&format!("{} w\n", width * 0.1));
        stream.push_str(&format!(
            "{} {} m {} {} l {} {} l S\n",
            margin,
            height * 0.5, // Start left middle
            width * 0.4,
            margin, // Bottom center
            width - margin,
            height - margin // Top right
        ));

        stream
    }

    /// Generate appearance stream for a checkbox (unchecked state).
    pub fn checkbox_off_appearance(&self, rect: Rect) -> String {
        let mut stream = String::new();

        let width = rect.width;
        let height = rect.height;

        // Background
        if let Some((r, g, b)) = self.background_color {
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&format!("0 0 {} {} re f\n", width, height));
        }

        // Border
        if let Some((r, g, b)) = self.border_color {
            if self.border_width > 0.0 {
                stream.push_str(&format!("{} {} {} RG\n", r, g, b));
                stream.push_str(&format!("{} w\n", self.border_width));
                let half = self.border_width / 2.0;
                stream.push_str(&format!(
                    "{} {} {} {} re S\n",
                    half,
                    half,
                    width - self.border_width,
                    height - self.border_width
                ));
            }
        }

        stream
    }

    /// Generate appearance stream for a radio button (selected state).
    ///
    /// # Arguments
    ///
    /// * `rect` - Field bounding rectangle
    /// * `indicator_color` - RGB color for the filled circle
    pub fn radio_on_appearance(&self, rect: Rect, indicator_color: (f32, f32, f32)) -> String {
        let mut stream = String::new();

        let width = rect.width;
        let height = rect.height;
        let cx = width / 2.0;
        let cy = height / 2.0;
        let radius = (width.min(height) / 2.0) - 1.0;

        // Background circle
        if let Some((r, g, b)) = self.background_color {
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&circle_path(cx, cy, radius));
            stream.push_str("f\n");
        }

        // Border circle
        if let Some((r, g, b)) = self.border_color {
            if self.border_width > 0.0 {
                stream.push_str(&format!("{} {} {} RG\n", r, g, b));
                stream.push_str(&format!("{} w\n", self.border_width));
                stream.push_str(&circle_path(cx, cy, radius));
                stream.push_str("S\n");
            }
        }

        // Inner filled circle (indicator)
        let (r, g, b) = indicator_color;
        let inner_radius = radius * 0.5;
        stream.push_str(&format!("{} {} {} rg\n", r, g, b));
        stream.push_str(&circle_path(cx, cy, inner_radius));
        stream.push_str("f\n");

        stream
    }

    /// Generate appearance stream for a radio button (unselected state).
    pub fn radio_off_appearance(&self, rect: Rect) -> String {
        let mut stream = String::new();

        let width = rect.width;
        let height = rect.height;
        let cx = width / 2.0;
        let cy = height / 2.0;
        let radius = (width.min(height) / 2.0) - 1.0;

        // Background circle
        if let Some((r, g, b)) = self.background_color {
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&circle_path(cx, cy, radius));
            stream.push_str("f\n");
        }

        // Border circle
        if let Some((r, g, b)) = self.border_color {
            if self.border_width > 0.0 {
                stream.push_str(&format!("{} {} {} RG\n", r, g, b));
                stream.push_str(&format!("{} w\n", self.border_width));
                stream.push_str(&circle_path(cx, cy, radius));
                stream.push_str("S\n");
            }
        }

        stream
    }

    /// Generate appearance stream for a push button.
    ///
    /// # Arguments
    ///
    /// * `rect` - Field bounding rectangle
    /// * `caption` - Button caption text
    /// * `font_name` - Font resource name
    /// * `font_size` - Font size in points
    /// * `text_color` - RGB color for caption
    pub fn button_appearance(
        &self,
        rect: Rect,
        caption: &str,
        font_name: &str,
        font_size: f32,
        text_color: (f32, f32, f32),
    ) -> String {
        let mut stream = String::new();

        let width = rect.width;
        let height = rect.height;

        // Background (gradient effect with lighter top)
        if let Some((r, g, b)) = self.background_color {
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&format!("0 0 {} {} re f\n", width, height));
        }

        // Border with 3D effect
        if let Some((r, g, b)) = self.border_color {
            if self.border_width > 0.0 {
                // Dark bottom/right edge
                stream.push_str(&format!("{} {} {} RG\n", r * 0.5, g * 0.5, b * 0.5));
                stream.push_str(&format!("{} w\n", self.border_width));
                stream.push_str(&format!("0 0 m {} 0 l S\n", width)); // Bottom
                stream.push_str(&format!("{} 0 m {} {} l S\n", width, width, height)); // Right

                // Light top/left edge
                stream.push_str(&format!(
                    "{} {} {} RG\n",
                    (r + 1.0).min(1.0) * 0.9,
                    (g + 1.0).min(1.0) * 0.9,
                    (b + 1.0).min(1.0) * 0.9
                ));
                stream.push_str(&format!("0 {} m {} {} l S\n", height, width, height)); // Top
                stream.push_str(&format!("0 0 m 0 {} l S\n", height)); // Left
            }
        }

        // Caption (centered)
        if !caption.is_empty() {
            let (r, g, b) = text_color;
            // Estimate text width (rough approximation)
            let approx_char_width = font_size * 0.6;
            let text_width = caption.len() as f32 * approx_char_width;
            let x_pos = (width - text_width) / 2.0;
            let y_pos = (height - font_size) / 2.0;

            stream.push_str("BT\n");
            stream.push_str(&format!("{} {} {} rg\n", r, g, b));
            stream.push_str(&format!("{} {} Tf\n", font_name, font_size));
            stream.push_str(&format!("{} {} Td\n", x_pos.max(2.0), y_pos));
            stream.push_str(&format!("({}) Tj\n", escape_pdf_string(caption)));
            stream.push_str("ET\n");
        }

        stream
    }
}

/// Generate a Bezier approximation of a circle path.
fn circle_path(cx: f32, cy: f32, r: f32) -> String {
    // Magic number for Bezier circle approximation
    let k: f32 = 0.552_284_7;
    let kp = r * k;

    format!(
        "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c\n",
        cx + r,
        cy, // Start at right
        cx + r,
        cy + kp, // Control point 1
        cx + kp,
        cy + r, // Control point 2
        cx,
        cy + r, // Top
        cx - kp,
        cy + r, // Control point 1
        cx - r,
        cy + kp, // Control point 2
        cx - r,
        cy, // Left
        cx - r,
        cy - kp, // Control point 1
        cx - kp,
        cy - r, // Control point 2
        cx,
        cy - r, // Bottom
        cx + kp,
        cy - r, // Control point 1
        cx + r,
        cy - kp, // Control point 2
        cx + r,
        cy // Back to start
    )
}

// ── pdf_manipulator patch: encode to WinAnsi bytes, not UTF-8 ──
/// Escape and encode text for a `(…) Tj` literal under a single-byte
/// WinAnsiEncoding font (the /DA fonts these appearances reference).
///
/// The content stream is a byte stream: pushing a `char` above U+007F
/// into a Rust `String` emits its multi-byte UTF-8 form, which a
/// single-byte font decodes as several wrong glyphs (ß → "ÃŸ" — the
/// classic Latin-1-range mojibake). Every char is therefore written as
/// exactly one WinAnsi byte, octal-escaped outside printable ASCII.
/// Chars with no WinAnsi byte become '?' — the fallback-font path owns
/// them, and a wrong single byte would render a random glyph.
pub(crate) fn escape_pdf_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => result.push_str("\\\\"),
            '(' => result.push_str("\\("),
            ')' => result.push_str("\\)"),
            '\r' => result.push_str("\\r"),
            '\n' => result.push_str("\\n"),
            _ => match win_ansi_byte(c) {
                Some(b) if (0x20..0x7F).contains(&b) => result.push(b as char),
                Some(b) => {
                    result.push_str(&format!("\\{:03o}", b));
                },
                None => result.push('?'),
            },
        }
    }
    result
}

/// The WinAnsiEncoding byte for a char, if it has one: ASCII and
/// U+00A0–U+00FF map to themselves; the 0x80–0x9F slots hold the
/// Windows-1252 specials (€, curly quotes, dashes, ™, …).
fn win_ansi_byte(c: char) -> Option<u8> {
    let cp = c as u32;
    if cp < 0x80 || (0xA0..=0xFF).contains(&cp) {
        return Some(cp as u8);
    }
    Some(match c {
        '\u{20AC}' => 0x80, // €
        '\u{201A}' => 0x82, // ‚
        '\u{0192}' => 0x83, // ƒ
        '\u{201E}' => 0x84, // „
        '\u{2026}' => 0x85, // …
        '\u{2020}' => 0x86, // †
        '\u{2021}' => 0x87, // ‡
        '\u{02C6}' => 0x88, // ˆ
        '\u{2030}' => 0x89, // ‰
        '\u{0160}' => 0x8A, // Š
        '\u{2039}' => 0x8B, // ‹
        '\u{0152}' => 0x8C, // Œ
        '\u{017D}' => 0x8E, // Ž
        '\u{2018}' => 0x91, // '
        '\u{2019}' => 0x92, // '
        '\u{201C}' => 0x93, // "
        '\u{201D}' => 0x94, // "
        '\u{2022}' => 0x95, // •
        '\u{2013}' => 0x96, // –
        '\u{2014}' => 0x97, // —
        '\u{02DC}' => 0x98, // ˜
        '\u{2122}' => 0x99, // ™
        '\u{0161}' => 0x9A, // š
        '\u{203A}' => 0x9B, // ›
        '\u{0153}' => 0x9C, // œ
        '\u{017E}' => 0x9E, // ž
        '\u{0178}' => 0x9F, // Ÿ
        _ => return None,
    })
}
// ── end pdf_manipulator patch ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_pdf_string() {
        assert_eq!(escape_pdf_string("Hello"), "Hello");
        assert_eq!(escape_pdf_string("Hello (World)"), "Hello \\(World\\)");
        assert_eq!(escape_pdf_string("Back\\slash"), "Back\\\\slash");
    }

    #[test]
    fn test_text_field_appearance() {
        let gen = FormAppearanceGenerator::new()
            .with_background(1.0, 1.0, 1.0)
            .with_border(1.0, 0.0, 0.0, 0.0);

        let rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        let stream = gen.text_field_appearance(rect, "Hello", "/Helv", 12.0, (0.0, 0.0, 0.0));

        assert!(stream.contains("rg")); // Fill color
        assert!(stream.contains("re")); // Rectangle
        assert!(stream.contains("BT")); // Begin text
        assert!(stream.contains("(Hello)")); // Text
        assert!(stream.contains("ET")); // End text
    }

    #[test]
    fn test_checkbox_on_appearance() {
        let gen = FormAppearanceGenerator::new()
            .with_background(1.0, 1.0, 1.0)
            .with_border(1.0, 0.0, 0.0, 0.0);

        let rect = Rect::new(0.0, 0.0, 15.0, 15.0);
        let stream = gen.checkbox_on_appearance(rect, (0.0, 0.0, 0.0));

        assert!(stream.contains("re")); // Border rectangle
        assert!(stream.contains("m")); // Move to (checkmark start)
        assert!(stream.contains("l")); // Line to (checkmark)
    }

    #[test]
    fn test_checkbox_off_appearance() {
        let gen = FormAppearanceGenerator::new()
            .with_background(1.0, 1.0, 1.0)
            .with_border(1.0, 0.0, 0.0, 0.0);

        let rect = Rect::new(0.0, 0.0, 15.0, 15.0);
        let stream = gen.checkbox_off_appearance(rect);

        assert!(stream.contains("re")); // Border rectangle
                                        // Should not contain checkmark paths
        assert!(!stream.contains("("));
    }

    #[test]
    fn test_radio_on_appearance() {
        let gen = FormAppearanceGenerator::new()
            .with_background(1.0, 1.0, 1.0)
            .with_border(1.0, 0.0, 0.0, 0.0);

        let rect = Rect::new(0.0, 0.0, 15.0, 15.0);
        let stream = gen.radio_on_appearance(rect, (0.0, 0.0, 0.0));

        assert!(stream.contains("c")); // Bezier curves for circle
        assert!(stream.contains("f")); // Fill (for inner circle)
    }

    #[test]
    fn test_radio_off_appearance() {
        let gen = FormAppearanceGenerator::new()
            .with_background(1.0, 1.0, 1.0)
            .with_border(1.0, 0.0, 0.0, 0.0);

        let rect = Rect::new(0.0, 0.0, 15.0, 15.0);
        let stream = gen.radio_off_appearance(rect);

        assert!(stream.contains("c")); // Circle outline
    }

    #[test]
    fn test_button_appearance() {
        let gen = FormAppearanceGenerator::new()
            .with_background(0.85, 0.85, 0.85)
            .with_border(1.0, 0.0, 0.0, 0.0);

        let rect = Rect::new(0.0, 0.0, 80.0, 25.0);
        let stream = gen.button_appearance(rect, "Submit", "/Helv", 12.0, (0.0, 0.0, 0.0));

        assert!(stream.contains("re")); // Background
        assert!(stream.contains("(Submit)")); // Caption
    }

    #[test]
    fn test_circle_path() {
        let path = circle_path(10.0, 10.0, 5.0);

        assert!(path.contains("m")); // Move to
        assert!(path.contains("c")); // Bezier curves
    }

    #[test]
    fn test_empty_text_field() {
        let gen = FormAppearanceGenerator::new().with_background(1.0, 1.0, 1.0);

        let rect = Rect::new(0.0, 0.0, 200.0, 20.0);
        let stream = gen.text_field_appearance(rect, "", "/Helv", 12.0, (0.0, 0.0, 0.0));

        // Should not contain text operations for empty text
        assert!(!stream.contains("BT"));
    }
}
