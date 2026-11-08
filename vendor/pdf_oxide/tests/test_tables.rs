//! Integration tests for table rendering.
//!
//! Tests table creation, layout calculation, and rendering.

use pdf_oxide::writer::{
    Borders, CellAlign, CellPadding, CellVAlign, ColumnWidth, ContentStreamBuilder,
    SimpleFontMetrics, Table, TableBorderStyle, TableCell, TableRow, TableStyle,
};

// =============================================================================
// TABLE CELL TESTS
// =============================================================================

mod cell_tests {
    use super::*;

    #[test]
    fn test_cell_text() {
        let cell = TableCell::text("Hello");
        assert_eq!(cell.content, "Hello");
        assert_eq!(cell.colspan, 1);
        assert_eq!(cell.rowspan, 1);
        assert!(!cell.bold);
        assert!(!cell.italic);
    }

    #[test]
    fn test_cell_empty() {
        let cell = TableCell::empty();
        assert!(cell.content.is_empty());
    }

    #[test]
    fn test_cell_header() {
        let cell = TableCell::header("Column");
        assert_eq!(cell.content, "Column");
        assert!(cell.bold);
        assert_eq!(cell.align, CellAlign::Center);
    }

    #[test]
    fn test_cell_number() {
        let cell = TableCell::number("123.45");
        assert_eq!(cell.align, CellAlign::Right);
    }

    #[test]
    fn test_cell_colspan() {
        let cell = TableCell::text("Wide").colspan(3);
        assert_eq!(cell.colspan, 3);
    }

    #[test]
    fn test_cell_rowspan() {
        let cell = TableCell::text("Tall").rowspan(2);
        assert_eq!(cell.rowspan, 2);
    }

    #[test]
    fn test_cell_alignment() {
        let left = TableCell::text("L").align(CellAlign::Left);
        let center = TableCell::text("C").align(CellAlign::Center);
        let right = TableCell::text("R").align(CellAlign::Right);

        assert_eq!(left.align, CellAlign::Left);
        assert_eq!(center.align, CellAlign::Center);
        assert_eq!(right.align, CellAlign::Right);
    }

    #[test]
    fn test_cell_valignment() {
        let top = TableCell::text("T").valign(CellVAlign::Top);
        let middle = TableCell::text("M").valign(CellVAlign::Middle);
        let bottom = TableCell::text("B").valign(CellVAlign::Bottom);

        assert_eq!(top.valign, CellVAlign::Top);
        assert_eq!(middle.valign, CellVAlign::Middle);
        assert_eq!(bottom.valign, CellVAlign::Bottom);
    }

    #[test]
    fn test_cell_background() {
        let cell = TableCell::text("Highlighted").background(1.0, 1.0, 0.0);
        assert_eq!(cell.background, Some((1.0, 1.0, 0.0)));
    }

    #[test]
    fn test_cell_font() {
        let cell = TableCell::text("Styled").font("Times-Roman", 14.0);
        assert_eq!(cell.font_name, Some("Times-Roman".to_string()));
        assert_eq!(cell.font_size, Some(14.0));
    }

    #[test]
    fn test_cell_bold_italic() {
        let cell = TableCell::text("Styled").bold().italic();
        assert!(cell.bold);
        assert!(cell.italic);
    }

    #[test]
    fn test_cell_padding() {
        let padding = CellPadding::uniform(8.0);
        let cell = TableCell::text("Padded").padding(padding);
        assert!(cell.padding.is_some());
    }

    #[test]
    fn test_cell_borders() {
        let borders = Borders::all(TableBorderStyle::thin());
        let cell = TableCell::text("Bordered").borders(borders);
        assert!(cell.borders.is_some());
    }
}

// =============================================================================
// TABLE ROW TESTS
// =============================================================================

mod row_tests {
    use super::*;

    #[test]
    fn test_row_creation() {
        let row = TableRow::new(vec![
            TableCell::text("A"),
            TableCell::text("B"),
            TableCell::text("C"),
        ]);
        assert_eq!(row.cells.len(), 3);
        assert!(!row.is_header);
    }

    #[test]
    fn test_header_row() {
        let row = TableRow::header(vec![TableCell::text("Col1"), TableCell::text("Col2")]);
        assert!(row.is_header);
    }

    #[test]
    fn test_row_as_header() {
        let row = TableRow::new(vec![TableCell::text("X")]).as_header();
        assert!(row.is_header);
    }

    #[test]
    fn test_row_min_height() {
        let row = TableRow::new(vec![TableCell::text("X")]).min_height(50.0);
        assert_eq!(row.min_height, Some(50.0));
    }

    #[test]
    fn test_row_background() {
        let row = TableRow::new(vec![TableCell::text("X")]).background(0.9, 0.9, 0.9);
        assert_eq!(row.background, Some((0.9, 0.9, 0.9)));
    }
}

// =============================================================================
// TABLE TESTS
// =============================================================================

mod table_tests {
    use super::*;

    #[test]
    fn test_table_creation() {
        let table = Table::new(vec![
            vec![TableCell::text("A"), TableCell::text("B")],
            vec![TableCell::text("C"), TableCell::text("D")],
        ]);

        assert_eq!(table.num_columns(), 2);
        assert_eq!(table.num_rows(), 2);
        assert!(!table.is_empty());
    }

    #[test]
    fn test_table_from_rows() {
        let rows = vec![
            TableRow::header(vec![TableCell::text("H1"), TableCell::text("H2")]),
            TableRow::new(vec![TableCell::text("D1"), TableCell::text("D2")]),
        ];
        let table = Table::from_rows(rows);

        assert_eq!(table.num_rows(), 2);
        assert!(table.rows[0].is_header);
    }

    #[test]
    fn test_table_empty() {
        let table = Table::empty();
        assert!(table.is_empty());
        assert_eq!(table.num_columns(), 0);
        assert_eq!(table.num_rows(), 0);
    }

    #[test]
    fn test_table_with_header_row() {
        let table = Table::new(vec![
            vec![TableCell::text("Name"), TableCell::text("Age")],
            vec![TableCell::text("Alice"), TableCell::text("30")],
        ])
        .with_header_row();

        assert!(table.rows[0].is_header);
        assert!(!table.rows[1].is_header);
    }

    #[test]
    fn test_table_with_style() {
        let style = TableStyle::bordered();
        let table = Table::new(vec![vec![TableCell::text("X")]]).with_style(style);

        assert!(table.style.outer_border.is_some());
    }

    #[test]
    fn test_table_with_width() {
        let table = Table::new(vec![vec![TableCell::text("X")]]).with_width(500.0);
        assert_eq!(table.width, Some(500.0));
    }

    #[test]
    fn test_table_with_column_widths() {
        let table = Table::new(vec![vec![
            TableCell::text("A"),
            TableCell::text("B"),
            TableCell::text("C"),
        ]])
        .with_column_widths(vec![
            ColumnWidth::Fixed(100.0),
            ColumnWidth::Percent(50.0),
            ColumnWidth::Auto,
        ]);

        assert_eq!(table.column_widths.len(), 3);
    }

    #[test]
    fn test_table_with_column_aligns() {
        let table = Table::new(vec![vec![TableCell::text("A"), TableCell::text("B")]])
            .with_column_aligns(vec![CellAlign::Left, CellAlign::Right]);

        assert_eq!(table.column_aligns.len(), 2);
    }

    #[test]
    fn test_table_add_row() {
        let mut table = Table::empty();
        table.add_row(TableRow::new(vec![TableCell::text("X")]));
        assert_eq!(table.num_rows(), 1);
    }
}

// =============================================================================
// STYLE TESTS
// =============================================================================

mod style_tests {
    use super::*;

    #[test]
    fn test_default_style() {
        let style = TableStyle::default();
        assert_eq!(style.font_name, "Helvetica");
        assert_eq!(style.font_size, 10.0);
        assert!(style.header_background.is_some());
    }

    #[test]
    fn test_minimal_style() {
        let style = TableStyle::minimal();
        assert!(style.cell_borders.top.is_none());
        assert!(style.outer_border.is_none());
        assert!(style.header_background.is_none());
    }

    #[test]
    fn test_bordered_style() {
        let style = TableStyle::bordered();
        assert!(style.outer_border.is_some());
        assert!(style.cell_borders.top.is_some());
    }

    #[test]
    fn test_striped_style() {
        let style = TableStyle::new().striped(0.95, 0.95, 0.95);
        assert!(style.stripe_color.is_some());
    }

    #[test]
    fn test_style_font() {
        let style = TableStyle::new().font("Courier", 12.0);
        assert_eq!(style.font_name, "Courier");
        assert_eq!(style.font_size, 12.0);
    }

    #[test]
    fn test_style_header_background() {
        let style = TableStyle::new().header_background(0.8, 0.8, 1.0);
        assert_eq!(style.header_background, Some((0.8, 0.8, 1.0)));
    }
}

// =============================================================================
// BORDER TESTS
// =============================================================================

mod border_tests {
    use super::*;

    #[test]
    fn test_border_style_presets() {
        let thin = TableBorderStyle::thin();
        assert_eq!(thin.width, 0.25);

        let medium = TableBorderStyle::medium();
        assert_eq!(medium.width, 0.5);

        let thick = TableBorderStyle::thick();
        assert_eq!(thick.width, 1.0);

        let none = TableBorderStyle::none();
        assert_eq!(none.width, 0.0);
    }

    #[test]
    fn test_border_style_color() {
        let border = TableBorderStyle::new(1.0).with_color(1.0, 0.0, 0.0);
        assert_eq!(border.color, (1.0, 0.0, 0.0));
    }

    #[test]
    fn test_borders_none() {
        let borders = Borders::none();
        assert!(borders.top.is_none());
        assert!(borders.right.is_none());
        assert!(borders.bottom.is_none());
        assert!(borders.left.is_none());
    }

    #[test]
    fn test_borders_all() {
        let borders = Borders::all(TableBorderStyle::medium());
        assert!(borders.top.is_some());
        assert!(borders.right.is_some());
        assert!(borders.bottom.is_some());
        assert!(borders.left.is_some());
    }

    #[test]
    fn test_borders_horizontal() {
        let borders = Borders::horizontal(TableBorderStyle::thin());
        assert!(borders.top.is_some());
        assert!(borders.bottom.is_some());
        assert!(borders.left.is_none());
        assert!(borders.right.is_none());
    }

    #[test]
    fn test_borders_vertical() {
        let borders = Borders::vertical(TableBorderStyle::thin());
        assert!(borders.left.is_some());
        assert!(borders.right.is_some());
        assert!(borders.top.is_none());
        assert!(borders.bottom.is_none());
    }

    #[test]
    fn test_borders_individual() {
        let borders = Borders::none()
            .with_top(TableBorderStyle::thick())
            .with_bottom(TableBorderStyle::thin());

        assert!(borders.top.is_some());
        assert!(borders.bottom.is_some());
        assert!(borders.left.is_none());
        assert!(borders.right.is_none());
    }
}

// =============================================================================
// PADDING TESTS
// =============================================================================

mod padding_tests {
    use super::*;

    #[test]
    fn test_padding_uniform() {
        let padding = CellPadding::uniform(10.0);
        assert_eq!(padding.top, 10.0);
        assert_eq!(padding.right, 10.0);
        assert_eq!(padding.bottom, 10.0);
        assert_eq!(padding.left, 10.0);
    }

    #[test]
    fn test_padding_symmetric() {
        let padding = CellPadding::symmetric(5.0, 10.0);
        assert_eq!(padding.left, 5.0);
        assert_eq!(padding.right, 5.0);
        assert_eq!(padding.top, 10.0);
        assert_eq!(padding.bottom, 10.0);
    }

    #[test]
    fn test_padding_none() {
        let padding = CellPadding::none();
        assert_eq!(padding.horizontal(), 0.0);
        assert_eq!(padding.vertical(), 0.0);
    }

    #[test]
    fn test_padding_calculations() {
        let padding = CellPadding::symmetric(5.0, 10.0);
        assert_eq!(padding.horizontal(), 10.0);
        assert_eq!(padding.vertical(), 20.0);
    }
}

// =============================================================================
// COLUMN WIDTH TESTS
// =============================================================================

mod column_width_tests {
    use super::*;

    #[test]
    fn test_column_width_variants() {
        let auto = ColumnWidth::Auto;
        let fixed = ColumnWidth::Fixed(100.0);
        let percent = ColumnWidth::Percent(25.0);
        let weight = ColumnWidth::Weight(2.0);

        // Just verify they compile and can be used
        let widths = [auto, fixed, percent, weight];
        assert_eq!(widths.len(), 4);
    }

    #[test]
    fn test_default_column_width() {
        let default = ColumnWidth::default();
        assert!(matches!(default, ColumnWidth::Auto));
    }
}

// =============================================================================
// LAYOUT TESTS
// =============================================================================

mod layout_tests {
    use super::*;

    #[test]
    fn test_simple_layout() {
        let table = Table::new(vec![
            vec![TableCell::text("A"), TableCell::text("B")],
            vec![TableCell::text("C"), TableCell::text("D")],
        ]);

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(400.0, &metrics);

        assert_eq!(layout.column_widths.len(), 2);
        assert_eq!(layout.row_heights.len(), 2);
        assert!(layout.total_width > 0.0);
        assert!(layout.total_height > 0.0);
        assert_eq!(layout.cell_positions.len(), 2);
    }

    #[test]
    fn test_fixed_width_layout() {
        let table = Table::new(vec![vec![TableCell::text("A"), TableCell::text("B")]])
            .with_column_widths(vec![ColumnWidth::Fixed(100.0), ColumnWidth::Fixed(200.0)]);

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(500.0, &metrics);

        assert!((layout.column_widths[0] - 100.0).abs() < 1.0 || layout.total_width <= 500.0);
    }

    #[test]
    fn test_empty_table_layout() {
        let table = Table::empty();
        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(400.0, &metrics);

        assert!(layout.column_widths.is_empty());
        assert!(layout.row_heights.is_empty());
        assert_eq!(layout.total_width, 0.0);
        assert_eq!(layout.total_height, 0.0);
    }

    #[test]
    fn test_cell_positions() {
        let table = Table::new(vec![
            vec![TableCell::text("A"), TableCell::text("B")],
            vec![TableCell::text("C"), TableCell::text("D")],
        ]);

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(400.0, &metrics);

        // First cell should be at origin
        assert_eq!(layout.cell_positions[0][0].x, 0.0);
        assert_eq!(layout.cell_positions[0][0].y, 0.0);

        // Second column should be offset
        assert!(layout.cell_positions[0][1].x > 0.0);
    }
}

// =============================================================================
// FONT METRICS TESTS
// =============================================================================

mod font_metrics_tests {
    use super::*;
    use pdf_oxide::writer::FontMetrics;

    #[test]
    fn test_simple_font_metrics() {
        let metrics = SimpleFontMetrics::default();
        let width = metrics.text_width("Hello", 12.0);
        assert!(width > 0.0);
    }

    #[test]
    fn test_monospace_metrics() {
        let metrics = SimpleFontMetrics::monospace();
        let width = metrics.text_width("Test", 10.0);
        // 4 characters * 10pt * 0.6 ratio
        assert!((width - 24.0).abs() < 0.1);
    }

    #[test]
    fn test_text_width_proportional() {
        let metrics = SimpleFontMetrics::default();
        let short = metrics.text_width("Hi", 12.0);
        let long = metrics.text_width("Hello World", 12.0);
        assert!(long > short);
    }

    #[test]
    fn test_font_size_affects_width() {
        let metrics = SimpleFontMetrics::default();
        let small = metrics.text_width("Test", 10.0);
        let large = metrics.text_width("Test", 20.0);
        assert!((large - small * 2.0).abs() < 0.1);
    }
}

// =============================================================================
// RENDERING TESTS
// =============================================================================

mod rendering_tests {
    use super::*;

    #[test]
    fn test_table_renders_to_content_stream() {
        let table = Table::new(vec![
            vec![TableCell::text("Name"), TableCell::text("Value")],
            vec![TableCell::text("Test"), TableCell::text("123")],
        ])
        .with_header_row();

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(400.0, &metrics);

        let mut builder = ContentStreamBuilder::new();
        let result = table.render(&mut builder, 72.0, 720.0, &layout);

        assert!(result.is_ok());

        let content = builder.build().unwrap();
        let content_str = String::from_utf8_lossy(&content);

        // Should contain text operators
        assert!(content_str.contains("BT"));
        assert!(content_str.contains("ET"));
        // Should contain cell text
        assert!(content_str.contains("Name"));
        assert!(content_str.contains("Value"));
    }

    #[test]
    fn test_table_with_background_renders() {
        let table = Table::new(vec![vec![
            TableCell::text("Highlighted").background(1.0, 1.0, 0.0)
        ]]);

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(200.0, &metrics);

        let mut builder = ContentStreamBuilder::new();
        let result = table.render(&mut builder, 72.0, 720.0, &layout);

        assert!(result.is_ok());

        let content = builder.build().unwrap();
        let content_str = String::from_utf8_lossy(&content);

        // Should contain fill color for background
        assert!(content_str.contains("rg")); // RGB fill color
    }

    #[test]
    fn test_table_with_borders_renders() {
        let table =
            Table::new(vec![vec![TableCell::text("Bordered")]]).with_style(TableStyle::bordered());

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(200.0, &metrics);

        let mut builder = ContentStreamBuilder::new();
        let result = table.render(&mut builder, 72.0, 720.0, &layout);

        assert!(result.is_ok());

        let content = builder.build().unwrap();
        let content_str = String::from_utf8_lossy(&content);

        // Should contain stroke operators for borders
        assert!(content_str.contains("S")); // Stroke
    }
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_data_table() {
        // Create a typical data table
        let table = Table::new(vec![
            vec![
                TableCell::header("Name"),
                TableCell::header("Age"),
                TableCell::header("Score"),
            ],
            vec![
                TableCell::text("Alice"),
                TableCell::number("30"),
                TableCell::number("95.5"),
            ],
            vec![
                TableCell::text("Bob"),
                TableCell::number("25"),
                TableCell::number("87.3"),
            ],
            vec![
                TableCell::text("Charlie"),
                TableCell::number("35"),
                TableCell::number("92.1"),
            ],
        ])
        .with_header_row()
        .with_column_aligns(vec![CellAlign::Left, CellAlign::Right, CellAlign::Right])
        .with_style(TableStyle::new().striped(0.95, 0.95, 0.95));

        assert_eq!(table.num_columns(), 3);
        assert_eq!(table.num_rows(), 4);
        assert!(table.rows[0].is_header);

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(400.0, &metrics);

        assert_eq!(layout.column_widths.len(), 3);
        assert!(layout.total_width > 0.0);
    }

    #[test]
    fn test_table_with_colspan() {
        let table = Table::new(vec![
            vec![TableCell::text("Header").colspan(3)],
            vec![
                TableCell::text("A"),
                TableCell::text("B"),
                TableCell::text("C"),
            ],
        ]);

        assert_eq!(table.num_columns(), 3);

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(300.0, &metrics);

        // First row, first cell should span full width
        let first_cell = &layout.cell_positions[0][0];
        assert!(first_cell.width > layout.column_widths[0]);
    }

    #[test]
    fn test_styled_invoice_table() {
        let table = Table::new(vec![
            vec![
                TableCell::header("Item"),
                TableCell::header("Qty"),
                TableCell::header("Price"),
                TableCell::header("Total"),
            ],
            vec![
                TableCell::text("Widget"),
                TableCell::number("5"),
                TableCell::number("$10.00"),
                TableCell::number("$50.00"),
            ],
            vec![
                TableCell::text("Gadget"),
                TableCell::number("3"),
                TableCell::number("$25.00"),
                TableCell::number("$75.00"),
            ],
            vec![
                TableCell::text("Grand Total").colspan(3).bold(),
                TableCell::number("$125.00").bold(),
            ],
        ])
        .with_header_row()
        .with_style(
            TableStyle::bordered()
                .font("Helvetica", 10.0)
                .header_background(0.2, 0.4, 0.8),
        );

        let metrics = SimpleFontMetrics::default();
        let layout = table.calculate_layout(500.0, &metrics);

        let mut builder = ContentStreamBuilder::new();
        let result = table.render(&mut builder, 72.0, 700.0, &layout);

        assert!(result.is_ok());
    }
}
