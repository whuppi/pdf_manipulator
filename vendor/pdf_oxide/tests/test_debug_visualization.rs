//! Tests for debug visualization functionality.
//!
//! These tests require the "rendering" feature to be enabled.

#![cfg(feature = "rendering")]

use pdf_oxide::api::{DebugOptions, DebugVisualizer, ElementColors, Pdf};

/// Helper function to create a simple test PDF.
fn create_test_pdf() -> Vec<u8> {
    let pdf = Pdf::from_text("Test document for debugging.\n\nPage 1 content.")
        .expect("Failed to create PDF");
    pdf.into_bytes()
}

mod debug_options {
    use super::*;

    #[test]
    fn test_debug_options_default() {
        let opts = DebugOptions::default();
        assert!(opts.show_text_bounds);
        assert!(opts.show_image_bounds);
        assert!(opts.show_path_bounds);
        assert!(opts.show_table_bounds);
        assert!(!opts.show_structure_bounds);
        assert!(!opts.label_elements);
        assert_eq!(opts.dpi, 150);
        assert!((opts.line_width - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_debug_options_text_only() {
        let opts = DebugOptions::text_only();
        assert!(opts.show_text_bounds);
        assert!(!opts.show_image_bounds);
        assert!(!opts.show_path_bounds);
        assert!(!opts.show_table_bounds);
        assert!(!opts.show_structure_bounds);
    }

    #[test]
    fn test_debug_options_all() {
        let opts = DebugOptions::all();
        assert!(opts.show_text_bounds);
        assert!(opts.show_image_bounds);
        assert!(opts.show_path_bounds);
        assert!(opts.show_table_bounds);
        assert!(opts.show_structure_bounds);
    }
}

mod element_colors {
    use super::*;

    #[test]
    fn test_element_colors_default() {
        let colors = ElementColors::default();
        // Text is red
        assert_eq!(colors.text[0], 1.0);
        assert_eq!(colors.text[1], 0.0);
        assert_eq!(colors.text[2], 0.0);
        assert_eq!(colors.text[3], 0.5);

        // Image is green
        assert_eq!(colors.image[0], 0.0);
        assert_eq!(colors.image[1], 1.0);
        assert_eq!(colors.image[2], 0.0);
        assert_eq!(colors.image[3], 0.5);

        // Path is blue
        assert_eq!(colors.path[0], 0.0);
        assert_eq!(colors.path[1], 0.0);
        assert_eq!(colors.path[2], 1.0);
        assert_eq!(colors.path[3], 0.5);

        // Table is yellow
        assert_eq!(colors.table[0], 1.0);
        assert_eq!(colors.table[1], 1.0);
        assert_eq!(colors.table[2], 0.0);
        assert_eq!(colors.table[3], 0.5);

        // Structure is magenta
        assert_eq!(colors.structure[0], 1.0);
        assert_eq!(colors.structure[1], 0.0);
        assert_eq!(colors.structure[2], 1.0);
        assert_eq!(colors.structure[3], 0.5);
    }
}

mod visualizer {
    use super::*;

    #[test]
    fn test_visualizer_creation() {
        let options = DebugOptions::default();
        let _visualizer = DebugVisualizer::new(options);
        // Verify it was created successfully (no panic)
    }

    #[test]
    fn test_visualizer_default() {
        let _visualizer = DebugVisualizer::default();
        // Default visualizer should be created (no panic)
    }

    #[test]
    fn test_render_debug_page() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_debug_render_input.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        let visualizer = DebugVisualizer::new(DebugOptions::default());
        let result = visualizer.render_debug_page(&mut pdf, 0);

        assert!(result.is_ok(), "Debug page render should succeed");

        let image = result.unwrap();
        assert!(image.width > 0);
        assert!(image.height > 0);
        assert!(!image.data.is_empty());

        // PNG magic bytes
        assert_eq!(&image.data[0..4], &[0x89, 0x50, 0x4E, 0x47]);

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }

    #[test]
    fn test_render_debug_page_to_file() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_debug_file_input.pdf");
        let output_path = temp_dir.join("test_debug_output.png");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        let visualizer = DebugVisualizer::new(DebugOptions::default());
        let result = visualizer.render_debug_page_to_file(&mut pdf, 0, &output_path);

        assert!(result.is_ok(), "Debug page render to file should succeed");
        assert!(output_path.exists(), "Output file should exist");

        let file_bytes = std::fs::read(&output_path).expect("Failed to read output file");
        assert!(!file_bytes.is_empty());
        // PNG magic bytes
        assert_eq!(&file_bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]);

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
        let _ = std::fs::remove_file(&output_path);
    }

    #[test]
    fn test_render_debug_page_with_custom_options() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_debug_custom_input.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");

        // Custom options - higher DPI, different line width
        let mut options = DebugOptions::all();
        options.dpi = 300;
        options.line_width = 2.0;

        let visualizer = DebugVisualizer::new(options);
        let result = visualizer.render_debug_page(&mut pdf, 0);

        assert!(result.is_ok());
        let image = result.unwrap();
        // Higher DPI should produce a larger image
        assert!(image.width > 0);
        assert!(image.height > 0);

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }
}

mod json_export {
    use super::*;

    #[test]
    fn test_export_elements_json() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_json_export_input.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");
        let page = pdf.page(0).expect("Failed to get page");

        let visualizer = DebugVisualizer::default();
        let result = visualizer.export_elements_json(&page);

        assert!(result.is_ok(), "JSON export should succeed");

        let json = result.unwrap();
        assert!(!json.is_empty());
        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should be valid JSON");
        assert!(parsed.is_array(), "JSON should be an array");

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }
}

mod svg_export {
    use super::*;

    #[test]
    fn test_export_elements_svg() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_svg_export_input.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");
        let page = pdf.page(0).expect("Failed to get page");

        let visualizer = DebugVisualizer::default();
        let result = visualizer.export_elements_svg(&page, 612.0, 792.0);

        assert!(result.is_ok(), "SVG export should succeed");

        let svg = result.unwrap();
        assert!(!svg.is_empty());
        // Should start with SVG tag
        assert!(svg.starts_with("<svg"), "Should start with SVG tag");
        assert!(svg.ends_with("</svg>"), "Should end with </svg>");

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }

    #[test]
    fn test_export_elements_svg_custom_dimensions() {
        let bytes = create_test_pdf();

        let temp_dir = std::env::temp_dir();
        let input_path = temp_dir.join("test_svg_custom_input.pdf");
        std::fs::write(&input_path, &bytes).expect("Failed to write temp PDF");

        let mut pdf = Pdf::open(&input_path).expect("Failed to open PDF");
        let page = pdf.page(0).expect("Failed to get page");

        let visualizer = DebugVisualizer::default();
        // A4 dimensions in points
        let result = visualizer.export_elements_svg(&page, 595.0, 842.0);

        assert!(result.is_ok());
        let svg = result.unwrap();
        assert!(svg.contains("width=\"595\""));
        assert!(svg.contains("height=\"842\""));

        // Cleanup
        let _ = std::fs::remove_file(&input_path);
    }
}
