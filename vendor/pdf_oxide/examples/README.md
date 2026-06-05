# PDF Library Examples

This directory contains examples demonstrating various features of pdf_oxide.

## Available Examples

Run examples with:
```bash
cargo run --example <example_name> -- [args]
```

### Basic Examples
- **basic_usage.rs** - Simple text extraction
- **extract_text.rs** - Text extraction with options
- **check_form_tables.rs** - Form field inspection

### Analysis Examples
- **analyze_font_f1.rs** - Font analysis and debugging
- **analyze_pdf_structure.rs** - PDF structure inspection

### Python Examples
Located in `python/` subdirectory.

## Quick Start

```bash
# Extract text from a PDF
cargo run --example basic_usage -- path/to/document.pdf

# Check form fields
cargo run --example check_form_tables -- path/to/form.pdf
```

## Documentation

For comprehensive documentation, see:
- API docs: https://docs.rs/pdf_oxide
- Main README: [../README.md](../README.md)
- Contributing guide: [../CONTRIBUTING.md](../CONTRIBUTING.md)

## License

All examples are licensed under MIT OR Apache-2.0, same as the main library.
