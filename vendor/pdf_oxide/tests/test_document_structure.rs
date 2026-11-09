//! Integration tests for document structure features.
//!
//! Tests for bookmarks/outlines, hyperlinks/annotations, and page templates.

use pdf_oxide::geometry::Rect;
use pdf_oxide::object::ObjectRef;
use pdf_oxide::writer::{
    AnnotationBuilder, Artifact, ArtifactAlignment, ArtifactElement, ArtifactStyle, BorderStyle,
    FitMode, HighlightMode, LinkAction, LinkAnnotation, OutlineBuilder, OutlineDestination,
    OutlineItem, OutlineStyle, PageNumberFormat, PageTemplate, Placeholder, PlaceholderContext,
};

// =============================================================================
// OUTLINE BUILDER TESTS
// =============================================================================

mod outline_tests {
    use super::*;

    #[test]
    fn test_outline_item_basic() {
        let item = OutlineItem::new("Chapter 1", 0);

        assert_eq!(item.title, "Chapter 1");
        assert!(item.open);
        assert!(matches!(item.destination, OutlineDestination::Page(0)));
    }

    #[test]
    fn test_outline_item_with_style() {
        let style = OutlineStyle::new().bold().italic().color(1.0, 0.0, 0.0);
        let item = OutlineItem::new("Important", 0).with_style(style);

        assert!(item.style.bold);
        assert!(item.style.italic);
        assert_eq!(item.style.color, Some((1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_outline_style_flags() {
        assert_eq!(OutlineStyle::new().flags(), 0);
        assert_eq!(OutlineStyle::new().italic().flags(), 1);
        assert_eq!(OutlineStyle::new().bold().flags(), 2);
        assert_eq!(OutlineStyle::new().bold().italic().flags(), 3);
    }

    #[test]
    fn test_outline_builder_flat() {
        let mut builder = OutlineBuilder::new();
        builder.item("Introduction", 0);
        builder.item("Chapter 1", 1);
        builder.item("Chapter 2", 5);
        builder.item("Conclusion", 10);

        assert_eq!(builder.len(), 4);
        assert!(!builder.is_empty());
    }

    #[test]
    fn test_outline_builder_nested() {
        let mut builder = OutlineBuilder::new();

        // Build hierarchy:
        // - Part 1
        //   - Chapter 1
        //     - Section 1.1
        //     - Section 1.2
        //   - Chapter 2
        // - Part 2

        builder.item("Part 1", 0);
        builder.child("Chapter 1", 1);
        builder.child("Section 1.1", 2);
        builder.pop(); // Back to Chapter 1
        builder.child("Section 1.2", 3);
        builder.pop(); // Back to Chapter 1
        builder.pop(); // Back to Part 1
        builder.child("Chapter 2", 4);
        builder.root();
        builder.item("Part 2", 5);

        let items = builder.items();
        assert_eq!(items.len(), 2); // Part 1 and Part 2

        // Part 1 has 2 children: Chapter 1 and Chapter 2
        assert_eq!(items[0].children.len(), 2);

        // Chapter 1 has 2 children: Section 1.1 and Section 1.2
        assert_eq!(items[0].children[0].children.len(), 2);
    }

    #[test]
    fn test_outline_uri_destination() {
        let item = OutlineItem::with_destination(
            "Visit Website",
            OutlineDestination::Uri("https://example.com".to_string()),
        );

        assert!(matches!(item.destination, OutlineDestination::Uri(_)));
    }

    #[test]
    fn test_outline_page_fit_destination() {
        let item = OutlineItem::with_destination(
            "Go to Section",
            OutlineDestination::PageFit {
                page: 3,
                fit: FitMode::FitH(Some(500.0)),
            },
        );

        match &item.destination {
            OutlineDestination::PageFit { page, fit } => {
                assert_eq!(*page, 3);
                assert!(matches!(fit, FitMode::FitH(Some(500.0))));
            },
            _ => panic!("Expected PageFit destination"),
        }
    }

    #[test]
    fn test_outline_build_objects() {
        let mut builder = OutlineBuilder::new();
        builder.item("Page 1", 0);
        builder.child("Section A", 0);
        builder.pop();
        builder.item("Page 2", 1);

        let page_refs = vec![ObjectRef::new(100, 0), ObjectRef::new(101, 0)];
        let result = builder.build(&page_refs, 200);

        assert!(result.is_some());
        let result = result.unwrap();

        // Should have root + 3 items (Page 1, Section A, Page 2)
        assert!(result.objects.len() >= 4);
        assert_eq!(result.root_ref.id, 200);
    }

    #[test]
    fn test_outline_closed_items() {
        let mut item = OutlineItem::new("Collapsed", 0).with_open(false);
        item.add_child(OutlineItem::new("Hidden 1", 1));
        item.add_child(OutlineItem::new("Hidden 2", 2));

        assert!(!item.open);
        assert_eq!(item.children.len(), 2);
    }

    #[test]
    fn test_fit_mode_variants() {
        // Test various fit modes
        let _fit = FitMode::Fit;
        let _fit_h = FitMode::FitH(Some(100.0));
        let _fit_v = FitMode::FitV(None);
        let _fit_r = FitMode::FitR {
            left: 0.0,
            bottom: 0.0,
            right: 612.0,
            top: 792.0,
        };
        let _fit_b = FitMode::FitB;
        let _fit_bh = FitMode::FitBH(Some(400.0));
        let _fit_bv = FitMode::FitBV(None);
        let _xyz = FitMode::XYZ {
            left: Some(72.0),
            top: Some(720.0),
            zoom: Some(1.5),
        };
    }
}

// =============================================================================
// ANNOTATION BUILDER TESTS
// =============================================================================

mod annotation_tests {
    use super::*;

    #[test]
    fn test_link_annotation_uri() {
        let link =
            LinkAnnotation::uri(Rect::new(72.0, 720.0, 100.0, 12.0), "https://rust-lang.org");

        assert!(matches!(link.action, LinkAction::Uri(_)));
        assert_eq!(link.rect.x, 72.0);
        assert_eq!(link.rect.width, 100.0);
    }

    #[test]
    fn test_link_annotation_goto() {
        let link = LinkAnnotation::goto_page(Rect::new(0.0, 0.0, 50.0, 10.0), 5);

        match link.action {
            LinkAction::GoTo { page, fit } => {
                assert_eq!(page, 5);
                assert!(fit.is_none());
            },
            _ => panic!("Expected GoTo action"),
        }
    }

    #[test]
    fn test_link_annotation_named() {
        let link = LinkAnnotation::goto_named(Rect::new(0.0, 0.0, 50.0, 10.0), "chapter1");

        assert!(matches!(link.action, LinkAction::GoToNamed(_)));
    }

    #[test]
    fn test_link_with_border() {
        let link = LinkAnnotation::uri(Rect::new(0.0, 0.0, 100.0, 20.0), "https://example.com")
            .with_border(BorderStyle::solid(2.0).with_radius(3.0));

        assert_eq!(link.border.width, 2.0);
        assert_eq!(link.border.horizontal_radius, 3.0);
    }

    #[test]
    fn test_link_with_color() {
        let link = LinkAnnotation::uri(Rect::new(0.0, 0.0, 100.0, 20.0), "https://example.com")
            .with_color(0.0, 0.0, 1.0); // Blue

        assert_eq!(link.color, Some((0.0, 0.0, 1.0)));
    }

    #[test]
    fn test_link_with_highlight() {
        let link = LinkAnnotation::uri(Rect::new(0.0, 0.0, 100.0, 20.0), "https://example.com")
            .with_highlight(HighlightMode::Push);

        assert!(matches!(link.highlight, HighlightMode::Push));
    }

    #[test]
    fn test_border_style_none() {
        let border = BorderStyle::none();
        assert_eq!(border.width, 0.0);
    }

    #[test]
    fn test_border_style_dashed() {
        let border = BorderStyle::dashed(1.0, 3.0, 2.0);
        assert_eq!(border.width, 1.0);
        assert_eq!(border.dash, Some((3.0, 2.0)));
    }

    #[test]
    fn test_highlight_mode_names() {
        assert_eq!(HighlightMode::None.pdf_name(), "N");
        assert_eq!(HighlightMode::Invert.pdf_name(), "I");
        assert_eq!(HighlightMode::Outline.pdf_name(), "O");
        assert_eq!(HighlightMode::Push.pdf_name(), "P");
    }

    #[test]
    fn test_annotation_builder() {
        let mut builder = AnnotationBuilder::new();
        assert!(builder.is_empty());

        builder.uri(Rect::new(72.0, 720.0, 100.0, 12.0), "https://example.com");
        builder.goto(Rect::new(72.0, 700.0, 50.0, 12.0), 2);

        assert_eq!(builder.len(), 2);
        assert!(!builder.is_empty());
    }

    #[test]
    fn test_annotation_build() {
        let mut builder = AnnotationBuilder::new();
        builder.uri(Rect::new(0.0, 0.0, 100.0, 20.0), "https://rust-lang.org");

        let page_refs = vec![ObjectRef::new(10, 0)];
        let annotations = builder.build(&page_refs);

        assert_eq!(annotations.len(), 1);
        let annot = &annotations[0];

        assert!(annot.contains_key("Type"));
        assert!(annot.contains_key("Subtype"));
        assert!(annot.contains_key("Rect"));
        assert!(annot.contains_key("A")); // Action for URI link
    }

    #[test]
    fn test_link_with_quad_points() {
        let quad = vec![
            72.0, 720.0, // bottom-left
            172.0, 720.0, // bottom-right
            172.0, 732.0, // top-right
            72.0, 732.0, // top-left
        ];

        let link = LinkAnnotation::uri(Rect::new(72.0, 720.0, 100.0, 12.0), "https://example.com")
            .with_quad_points(quad.clone());

        assert_eq!(link.quad_points, Some(quad));
    }
}

// =============================================================================
// PAGE TEMPLATE TESTS
// =============================================================================

mod template_tests {
    use super::*;

    #[test]
    fn test_placeholder_tokens() {
        assert_eq!(Placeholder::PageNumber.token(), "{page}");
        assert_eq!(Placeholder::TotalPages.token(), "{pages}");
        assert_eq!(Placeholder::Date.token(), "{date}");
        assert_eq!(Placeholder::Time.token(), "{time}");
        assert_eq!(Placeholder::Title.token(), "{title}");
        assert_eq!(Placeholder::Author.token(), "{author}");
    }

    #[test]
    fn test_placeholder_parse() {
        let text = "Page {page} of {pages} - {date}";
        let placeholders = Placeholder::parse_all(text);

        assert_eq!(placeholders.len(), 3);
        assert_eq!(placeholders[0].1, Placeholder::PageNumber);
        assert_eq!(placeholders[1].1, Placeholder::TotalPages);
        assert_eq!(placeholders[2].1, Placeholder::Date);
    }

    #[test]
    fn test_hf_element_alignments() {
        let left = ArtifactElement::left("Left text");
        assert_eq!(left.alignment, ArtifactAlignment::Left);

        let center = ArtifactElement::center("Center text");
        assert_eq!(center.alignment, ArtifactAlignment::Center);

        let right = ArtifactElement::right("Right text");
        assert_eq!(right.alignment, ArtifactAlignment::Right);
    }

    #[test]
    fn test_hf_element_resolve() {
        let element = ArtifactElement::center("Page {page} of {pages}");
        let context = PlaceholderContext::new(5, 20);

        let resolved = element.resolve(&context);
        assert_eq!(resolved, "Page 5 of 20");
    }

    #[test]
    fn test_hf_element_resolve_metadata() {
        let element = ArtifactElement::center("{title} by {author}");
        let context = PlaceholderContext::new(1, 1)
            .with_title("My Document")
            .with_author("John Doe");

        let resolved = element.resolve(&context);
        assert_eq!(resolved, "My Document by John Doe");
    }

    #[test]
    fn test_header_footer_creation() {
        let hf = Artifact::new()
            .with_left("Document Title")
            .with_center("Confidential")
            .with_right("{page}");

        assert!(hf.left.is_some());
        assert!(hf.center.is_some());
        assert!(hf.right.is_some());
        assert!(!hf.is_empty());
    }

    #[test]
    fn test_header_footer_shorthand() {
        let left_hf = Artifact::left("Left only");
        assert!(left_hf.left.is_some());
        assert!(left_hf.center.is_none());
        assert!(left_hf.right.is_none());

        let center_hf = Artifact::center("Center only");
        assert!(center_hf.center.is_some());

        let right_hf = Artifact::right("Right only");
        assert!(right_hf.right.is_some());
    }

    #[test]
    fn test_header_footer_style() {
        let style = ArtifactStyle::new()
            .font("Times-Roman", 10.0)
            .bold()
            .color(0.5, 0.5, 0.5)
            .with_separator(0.5);

        let hf = Artifact::center("Header").with_style(style);

        assert_eq!(hf.style.font_name, "Times-Roman");
        assert_eq!(hf.style.font_size, 10.0);
        assert!(hf.style.separator_line);
    }

    #[test]
    fn test_header_footer_offset() {
        let hf = Artifact::center("Header").with_offset(50.0);
        assert_eq!(hf.offset, 50.0);
    }

    #[test]
    fn test_page_template_basic() {
        let template = PageTemplate::new()
            .header(Artifact::center("My Document"))
            .footer(Artifact::right("{page} of {pages}"));

        assert!(template.header.is_some());
        assert!(template.footer.is_some());
        assert!(!template.is_empty());
    }

    #[test]
    fn test_page_template_skip_first() {
        let template = PageTemplate::new()
            .header(Artifact::center("Header"))
            .skip_first_page();

        assert!(template.get_header(1).is_none());
        assert!(template.get_header(2).is_some());
    }

    #[test]
    fn test_page_template_different_first_page() {
        let template = PageTemplate::new()
            .header(Artifact::center("Regular Header"))
            .first_page_header(Artifact::center("Title Page Header"));

        let first = template.get_header(1).unwrap();
        let second = template.get_header(2).unwrap();

        assert_eq!(first.center.as_ref().unwrap().text, "Title Page Header");
        assert_eq!(second.center.as_ref().unwrap().text, "Regular Header");
    }

    #[test]
    fn test_page_template_margins() {
        let template = PageTemplate::new().margins(100.0, 100.0);

        assert_eq!(template.margin_left, 100.0);
        assert_eq!(template.margin_right, 100.0);
    }

    #[test]
    fn test_page_number_formats() {
        assert_eq!(PageNumberFormat::page_x(), "Page {page}");
        assert_eq!(PageNumberFormat::page_x_of_y(), "Page {page} of {pages}");
        assert_eq!(PageNumberFormat::x_slash_y(), "{page} / {pages}");
        assert_eq!(PageNumberFormat::number_only(), "{page}");
        assert_eq!(PageNumberFormat::dashed(), "- {page} -");
    }

    #[test]
    fn test_placeholder_context_date_time() {
        let context = PlaceholderContext::new(1, 10);

        // Date should be in YYYY-MM-DD format
        assert!(context.date.len() == 10);
        assert!(context.date.contains('-'));

        // Time should be in HH:MM format
        assert!(context.time.len() == 5);
        assert!(context.time.contains(':'));
    }

    #[test]
    fn test_page_template_footer_variations() {
        // Test different footer configurations
        let footer1 = Artifact::center(PageNumberFormat::page_x_of_y());
        let footer2 = Artifact::new().with_left("{date}").with_right("{page}");

        assert!(footer1.center.is_some());
        assert!(footer2.left.is_some());
        assert!(footer2.right.is_some());
    }

    #[test]
    fn test_header_footer_elements() {
        let hf = Artifact::new()
            .with_left("Left")
            .with_center("Center")
            .with_right("Right");

        let elements = hf.elements();
        assert_eq!(elements.len(), 3);
    }

    #[test]
    fn test_hf_element_with_style() {
        let style = ArtifactStyle::new().font("Courier", 8.0);
        let element = ArtifactElement::center("Code").with_style(style);

        assert!(element.style.is_some());
        let s = element.style.as_ref().unwrap();
        assert_eq!(s.font_name, "Courier");
        assert_eq!(s.font_size, 8.0);
    }
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_document_structure() {
        // Create outline with multiple levels
        let mut outline = OutlineBuilder::new();
        outline.item("Introduction", 0);
        outline.item("Chapter 1: Getting Started", 1);
        outline.child("Installation", 1);
        outline.child("First Steps", 2);
        outline.pop();
        outline.pop();
        outline.item("Chapter 2: Advanced Topics", 3);
        outline.item("Conclusion", 4);

        // Create annotations for a page
        let mut annotations = AnnotationBuilder::new();
        annotations.uri(Rect::new(72.0, 720.0, 200.0, 12.0), "https://docs.rs/pdf_oxide");
        annotations.goto(Rect::new(72.0, 700.0, 100.0, 12.0), 3);

        // Create page template
        let template = PageTemplate::new()
            .header(Artifact::new().with_left("{title}").with_right("{date}"))
            .footer(Artifact::center("{page} / {pages}"))
            .first_page_header(Artifact::center("Document Title"))
            .skip_first_page();

        // Verify structures
        assert_eq!(outline.len(), 4);
        assert_eq!(annotations.len(), 2);
        assert!(!template.is_empty());

        // Verify first page behavior
        assert!(template.get_header(1).is_some()); // Uses first_page_header
        assert!(template.get_footer(1).is_none()); // Skipped

        // Verify subsequent pages
        assert!(template.get_header(2).is_some());
        assert!(template.get_footer(2).is_some());
    }

    #[test]
    fn test_outline_with_all_destination_types() {
        let mut builder = OutlineBuilder::new();

        // Page destination
        builder.add_item(OutlineItem::new("Page 1", 0));

        // Page with fit mode
        builder.add_item(OutlineItem::with_destination(
            "Zoomed View",
            OutlineDestination::PageFit {
                page: 1,
                fit: FitMode::XYZ {
                    left: Some(72.0),
                    top: Some(720.0),
                    zoom: Some(2.0),
                },
            },
        ));

        // Named destination
        builder.add_item(OutlineItem::with_destination(
            "Named Link",
            OutlineDestination::Named("chapter2".to_string()),
        ));

        // URI link
        builder.add_item(OutlineItem::with_destination(
            "External Link",
            OutlineDestination::Uri("https://example.com".to_string()),
        ));

        assert_eq!(builder.len(), 4);
    }

    #[test]
    fn test_styled_outline() {
        let mut builder = OutlineBuilder::new();

        // Regular item
        builder.add_item(OutlineItem::new("Normal", 0));

        // Bold item
        builder
            .add_item(OutlineItem::new("Bold Section", 1).with_style(OutlineStyle::new().bold()));

        // Italic item
        builder.add_item(OutlineItem::new("Note", 2).with_style(OutlineStyle::new().italic()));

        // Colored item
        builder.add_item(
            OutlineItem::new("Important", 3)
                .with_style(OutlineStyle::new().bold().color(1.0, 0.0, 0.0)),
        );

        // Closed item
        builder.add_item(OutlineItem::new("Collapsed", 4).with_open(false));

        assert_eq!(builder.len(), 5);
    }

    #[test]
    fn test_annotation_types() {
        let mut builder = AnnotationBuilder::new();

        // External URI
        builder.add_link(
            LinkAnnotation::uri(Rect::new(0.0, 0.0, 100.0, 20.0), "https://example.com")
                .with_highlight(HighlightMode::Invert),
        );

        // Internal page link
        builder.add_link(
            LinkAnnotation::goto_page(Rect::new(0.0, 30.0, 100.0, 20.0), 5)
                .with_border(BorderStyle::solid(1.0)),
        );

        // Named destination
        builder.add_link(
            LinkAnnotation::goto_named(Rect::new(0.0, 60.0, 100.0, 20.0), "appendix")
                .with_color(0.0, 0.5, 0.0),
        );

        assert_eq!(builder.len(), 3);
    }
}
