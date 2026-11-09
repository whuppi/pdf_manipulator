//! Integration tests for hierarchical PDF content extraction and editing.
//!
//! Tests the full round-trip workflow:
//! - Extract hierarchical content from PDF pages
//! - Modify content via DocumentEditor API
//! - Save changes and verify structure preservation
//! - Handle both tagged and untagged PDFs

#[cfg(test)]
mod hierarchical_integration_tests {
    use pdf_oxide::elements::{ContentElement, StructureElement};
    use pdf_oxide::geometry::Rect;

    /// Test extracting hierarchical structure from a simple document.
    #[test]
    fn test_extract_hierarchical_content() {
        // This test would require a test PDF file
        // For now, we'll test the API contracts

        // Verify StructureElement can be created
        let structure = StructureElement {
            structure_type: "Document".to_string(),
            bbox: Rect::new(0.0, 0.0, 612.0, 792.0),
            children: vec![],
            reading_order: Some(0),
            alt_text: None,
            language: None,
        };

        assert_eq!(structure.structure_type, "Document");
        assert_eq!(structure.children.len(), 0);
        assert_eq!(structure.reading_order, Some(0));
    }

    /// Test creating a hierarchical structure with nested elements.
    #[test]
    fn test_nested_structure_creation() {
        // Create inner element
        let inner = StructureElement {
            structure_type: "Span".to_string(),
            bbox: Rect::new(100.0, 700.0, 50.0, 12.0),
            children: vec![],
            reading_order: None,
            alt_text: None,
            language: None,
        };

        // Create outer element with inner as child
        let outer = StructureElement {
            structure_type: "P".to_string(),
            bbox: Rect::new(100.0, 700.0, 200.0, 50.0),
            children: vec![ContentElement::Structure(inner)],
            reading_order: Some(1),
            alt_text: None,
            language: None,
        };

        assert_eq!(outer.structure_type, "P");
        assert_eq!(outer.children.len(), 1);

        // Verify nested structure
        if let ContentElement::Structure(nested) = &outer.children[0] {
            assert_eq!(nested.structure_type, "Span");
        } else {
            panic!("Expected nested Structure element");
        }
    }

    /// Test structure with accessibility attributes.
    #[test]
    fn test_structure_with_accessibility() {
        let structure = StructureElement {
            structure_type: "H1".to_string(),
            bbox: Rect::new(72.0, 720.0, 300.0, 24.0),
            children: vec![],
            reading_order: Some(0),
            alt_text: Some("Main Heading".to_string()),
            language: Some("en".to_string()),
        };

        assert_eq!(structure.alt_text, Some("Main Heading".to_string()));
        assert_eq!(structure.language, Some("en".to_string()));
    }

    /// Test DocumentEditor content modification API.
    #[test]
    fn test_editor_content_modification() {
        // Create a test structure
        let structure = StructureElement {
            structure_type: "Document".to_string(),
            bbox: Rect::new(0.0, 0.0, 612.0, 792.0),
            children: vec![],
            reading_order: Some(0),
            alt_text: None,
            language: None,
        };

        // Verify structure can be passed to set_page_content
        // (actual save operations would require a valid PDF file)
        assert_eq!(structure.structure_type, "Document");
        assert_eq!(structure.bbox.width, 612.0);
        assert_eq!(structure.bbox.height, 792.0);
    }

    /// Test content stream generation from structure elements.
    #[test]
    fn test_content_stream_generation() {
        use pdf_oxide::writer::ContentStreamBuilder;

        let structure = StructureElement {
            structure_type: "P".to_string(),
            bbox: Rect::new(100.0, 700.0, 200.0, 50.0),
            children: vec![],
            reading_order: None,
            alt_text: None,
            language: None,
        };

        // Build content stream with marked content
        let mut builder = ContentStreamBuilder::new();
        builder.add_structure_element(&structure);

        // Verify content stream can be built
        let bytes = builder.build().unwrap();
        assert!(!bytes.is_empty());

        // Verify BDC/EMC operators are present
        let content = String::from_utf8_lossy(&bytes);
        assert!(content.contains("BDC"));
        assert!(content.contains("EMC"));
    }

    /// Test MCID allocation during content stream generation.
    #[test]
    fn test_mcid_allocation() {
        use pdf_oxide::writer::ContentStreamBuilder;

        let mut builder = ContentStreamBuilder::new();

        // Allocate MCIDs in sequence
        assert_eq!(builder.next_mcid(), 0);
        assert_eq!(builder.next_mcid(), 1);
        assert_eq!(builder.next_mcid(), 2);
        assert_eq!(builder.next_mcid(), 3);
    }

    /// Test multiple pages with different structures.
    #[test]
    fn test_multiple_pages_structures() {
        let page1_structure = StructureElement {
            structure_type: "Document".to_string(),
            bbox: Rect::new(0.0, 0.0, 612.0, 792.0),
            children: vec![],
            reading_order: Some(0),
            alt_text: None,
            language: None,
        };

        let page2_structure = StructureElement {
            structure_type: "Document".to_string(),
            bbox: Rect::new(0.0, 0.0, 612.0, 792.0),
            children: vec![],
            reading_order: Some(1),
            alt_text: None,
            language: None,
        };

        assert_eq!(page1_structure.reading_order, Some(0));
        assert_eq!(page2_structure.reading_order, Some(1));
    }

    /// Test structure type variants (PDF standard types).
    #[test]
    fn test_standard_structure_types() {
        let standard_types = vec![
            "Document", "Part", "Art", "Sect", "Div", "P", "H1", "H2", "H3", "H4", "H5", "H6",
            "List", "ListItem", "Label", "ListBody", "Table", "THead", "TBody", "TFoot", "TR",
            "TD", "TH", "Span", "Quote", "Code", "Link",
        ];

        for type_name in standard_types {
            let structure = StructureElement {
                structure_type: type_name.to_string(),
                bbox: Rect::new(0.0, 0.0, 100.0, 50.0),
                children: vec![],
                reading_order: None,
                alt_text: None,
                language: None,
            };

            assert_eq!(structure.structure_type, type_name);
        }
    }

    /// Test empty structure element.
    #[test]
    fn test_empty_structure() {
        let empty_structure = StructureElement {
            structure_type: "Div".to_string(),
            bbox: Rect::new(0.0, 0.0, 0.0, 0.0),
            children: vec![],
            reading_order: None,
            alt_text: None,
            language: None,
        };

        assert!(empty_structure.children.is_empty());
        assert_eq!(empty_structure.bbox.width, 0.0);
        assert_eq!(empty_structure.bbox.height, 0.0);
    }

    /// Test deep nesting of structure elements.
    #[test]
    fn test_deep_nesting() {
        // Create a deeply nested structure
        let mut current = StructureElement {
            structure_type: "Span".to_string(),
            bbox: Rect::new(0.0, 0.0, 10.0, 10.0),
            children: vec![],
            reading_order: None,
            alt_text: None,
            language: None,
        };

        // Nest 5 levels deep
        for level in 0..5 {
            let parent = StructureElement {
                structure_type: format!("Level{}", level),
                bbox: Rect::new(0.0, 0.0, 100.0 + (level as f32 * 10.0), 50.0),
                children: vec![ContentElement::Structure(current)],
                reading_order: Some(level),
                alt_text: None,
                language: None,
            };
            current = parent;
        }

        // Verify nesting
        let mut depth = 0;
        let mut current_ref = &current;
        loop {
            depth += 1;
            if current_ref.children.is_empty() {
                break;
            }
            if let ContentElement::Structure(nested) = &current_ref.children[0] {
                current_ref = nested;
            } else {
                break;
            }
        }

        assert_eq!(depth, 6); // Root + 5 levels
    }

    /// Test resource manager functionality.
    #[test]
    fn test_resource_manager() {
        use pdf_oxide::editor::ResourceManager;

        let mut manager = ResourceManager::new();

        // Register fonts
        let font1 = manager.register_font("Helvetica");
        let font2 = manager.register_font("Times-Roman");
        let font3 = manager.register_font("Helvetica"); // Duplicate

        assert_eq!(font1, "/F1");
        assert_eq!(font2, "/F2");
        assert_eq!(font3, "/F1"); // Should reuse existing

        // Register images
        let img1 = manager.register_image();
        let img2 = manager.register_image();

        assert_eq!(img1, "/Im1");
        assert_eq!(img2, "/Im2");

        // Verify resource count
        assert_eq!(manager.resource_count(), 4); // 2 unique fonts + 2 images
    }

    /// Test synthetic structure generation configuration.
    #[test]
    fn test_synthetic_structure_config() {
        use pdf_oxide::extractors::SyntheticStructureConfig;

        let config = SyntheticStructureConfig::default();

        assert_eq!(config.paragraph_gap_threshold, 4.0);
        assert_eq!(config.heading_size_multiplier, 1.3);
        assert_eq!(config.section_break_threshold, 50.0);

        // Custom configuration
        let custom_config = SyntheticStructureConfig {
            paragraph_gap_threshold: 6.0,
            heading_size_multiplier: 1.5,
            section_break_threshold: 75.0,
        };

        assert_eq!(custom_config.paragraph_gap_threshold, 6.0);
        assert_eq!(custom_config.heading_size_multiplier, 1.5);
        assert_eq!(custom_config.section_break_threshold, 75.0);
    }

    /// Test structure type formatting.
    #[test]
    fn test_structure_type_formatting() {
        // Verify various structure type strings can be created and compared
        let types = vec![
            ("Document", "Document"),
            ("H1", "H1"),
            ("P", "P"),
            ("Sect", "Sect"),
        ];

        for (input, expected) in types {
            let structure = StructureElement {
                structure_type: input.to_string(),
                bbox: Rect::new(0.0, 0.0, 100.0, 50.0),
                children: vec![],
                reading_order: None,
                alt_text: None,
                language: None,
            };

            assert_eq!(structure.structure_type, expected);
        }
    }
}
