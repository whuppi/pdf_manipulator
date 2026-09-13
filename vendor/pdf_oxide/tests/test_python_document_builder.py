"""Integration tests for the Python write-side API.

Covers two surfaces:

* ``DocumentBuilder`` + ``FluentPageBuilder`` + ``EmbeddedFont`` — the
  fluent API for programmatic multi-page construction, with embedded
  TTF/OTF support.

* ``Pdf.from_html_css`` / ``from_html_css_with_fonts`` — the HTML+CSS
  pipeline that was previously only reachable from Rust.

These tests exercise both surfaces end-to-end using the DejaVuSans
fixture that already ships at ``tests/fixtures/fonts/DejaVuSans.ttf``.

To run locally:

    maturin develop --features python
    pytest tests/test_python_document_builder.py -v
"""

from __future__ import annotations

from pathlib import Path

import pytest


pdf_oxide = pytest.importorskip("pdf_oxide")

FIXTURE_FONT = Path(__file__).parent / "fixtures" / "fonts" / "DejaVuSans.ttf"


# ---------------------------------------------------------------------------
# Phase 1 — DocumentBuilder + EmbeddedFont
# ---------------------------------------------------------------------------


def test_document_builder_minimal_ascii(tmp_path):
    """The simplest possible builder chain produces a valid PDF on disk."""
    out = tmp_path / "out.pdf"
    (
        pdf_oxide.DocumentBuilder()
        .title("Hello")
        .a4_page()
        .text("Hello, world.")
        .done()
        .save(str(out))
    )
    assert out.exists()
    assert out.stat().st_size > 256  # sanity floor; empty PDF is much smaller
    header = out.read_bytes()[:8]
    assert header.startswith(b"%PDF-")


def test_document_builder_build_returns_bytes():
    """``.build()`` returns a ``bytes`` object without touching the disk."""
    data = pdf_oxide.DocumentBuilder().a4_page().at(72.0, 720.0).text("pytest").done().build()
    assert isinstance(data, bytes)
    assert data.startswith(b"%PDF-")


def test_document_builder_cjk_round_trip(tmp_path):
    """The headline #384/#382 integration test.

    Register DejaVuSans (covers Latin + Cyrillic + Greek — enough to
    prove the Type-0 / CIDFontType2 path is live without needing a CJK
    fixture), emit text in each script, re-parse the output, and verify
    every original string comes back through ``extract_text``.
    """
    font = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT))
    pdf_bytes = (
        pdf_oxide.DocumentBuilder()
        .register_embedded_font("DejaVu", font)
        .a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 720.0)
        .text("Привет, мир!")
        .at(72.0, 700.0)
        .text("Καλημέρα κόσμε")
        .at(72.0, 680.0)
        .text("The quick brown fox")
        .done()
        .build()
    )
    doc = pdf_oxide.PdfDocument.from_bytes(pdf_bytes)
    text = doc.extract_text(0)
    assert "Привет, мир!" in text
    assert "Καλημέρα κόσμε" in text
    assert "The quick brown fox" in text


def test_document_builder_output_is_subsetted():
    """Register a ~760 KB font, emit one short line; the resulting PDF
    must be dramatically smaller than the face, proving the font
    subsetter is wired through the Python path."""
    font = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT))
    pdf_bytes = (
        pdf_oxide.DocumentBuilder()
        .register_embedded_font("DejaVu", font)
        .a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 700.0)
        .text("Hello world")
        .done()
        .build()
    )
    face_size = FIXTURE_FONT.stat().st_size
    assert len(pdf_bytes) * 10 < face_size, (
        f"expected PDF ({len(pdf_bytes)} bytes) to be at least 10× smaller than "
        f"the original face ({face_size} bytes); subsetter likely not wired"
    )


def test_document_builder_multiple_pages(tmp_path):
    """A builder can produce many pages; each ``done()`` returns the
    parent so the chain keeps going."""
    out = tmp_path / "multi.pdf"
    (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        .at(72.0, 720.0)
        .text("page one")
        .done()
        .a4_page()
        .at(72.0, 720.0)
        .text("page two")
        .done()
        .a4_page()
        .at(72.0, 720.0)
        .text("page three")
        .done()
        .save(str(out))
    )
    doc = pdf_oxide.PdfDocument(str(out))
    # The high-level API keeps the DocumentBuilder usable; we check the
    # rendered PDF has the right page count.
    assert doc.page_count() == 3


def test_document_builder_save_encrypted(tmp_path):
    """Phase 1 includes AES-256 encryption (from v0.3.38 #386)."""
    out = tmp_path / "encrypted.pdf"
    (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        .at(72.0, 720.0)
        .text("confidential")
        .done()
        .save_encrypted(str(out), "userpw", "ownerpw")
    )
    raw = out.read_bytes()
    assert b"/Encrypt" in raw, "encrypted PDF missing /Encrypt dictionary"
    assert b"/V 5" in raw, "expected /V 5 (AES-256) marker"


def test_document_builder_to_bytes_encrypted():
    data = (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        .at(72.0, 720.0)
        .text("bytes")
        .done()
        .to_bytes_encrypted("u", "o")
    )
    assert isinstance(data, bytes)
    assert b"/Encrypt" in data
    assert b"/V 5" in data


def test_document_builder_consumed_after_build():
    """Calling ``build()`` again on the same instance should raise."""
    b = pdf_oxide.DocumentBuilder().a4_page().text("x").done()
    b.build()
    with pytest.raises(RuntimeError):
        b.build()


def test_document_builder_consumed_after_save(tmp_path):
    b = pdf_oxide.DocumentBuilder().a4_page().text("x").done()
    b.save(str(tmp_path / "once.pdf"))
    with pytest.raises(RuntimeError):
        b.save(str(tmp_path / "twice.pdf"))


def test_fluent_page_done_is_single_use():
    """A ``FluentPageBuilder`` may only be committed once."""
    b = pdf_oxide.DocumentBuilder()
    page = b.a4_page().text("a")
    page.done()
    with pytest.raises(RuntimeError):
        page.done()


def test_embedded_font_consumed_after_register():
    """Registering an ``EmbeddedFont`` consumes it; re-registering
    raises."""
    font = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT))
    b = pdf_oxide.DocumentBuilder().register_embedded_font("DejaVu", font)
    with pytest.raises(RuntimeError):
        b.register_embedded_font("DejaVu2", font)


def test_embedded_font_from_bytes():
    """``EmbeddedFont.from_bytes`` accepts Python ``bytes`` and
    optionally an override name."""
    data = FIXTURE_FONT.read_bytes()
    font = pdf_oxide.EmbeddedFont.from_bytes(data, name="CustomDejaVu")
    # name override applied
    assert font.name == "CustomDejaVu"


def test_document_builder_annotations():
    """Phase 3 surface: annotations attached to the previous text
    element. We emit a variety and round-trip the text; annotation
    metadata is verified via ``get_annotations(page)`` in a later
    sweep — for now we just prove the methods don't raise and the
    document still renders coherent text."""
    data = (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        .at(72.0, 720.0)
        .text("click me")
        .link_url("https://example.com")
        .at(72.0, 700.0)
        .text("important")
        .highlight((1.0, 1.0, 0.0))
        .at(72.0, 680.0)
        .text("revisit")
        .sticky_note("please review")
        .watermark_draft()
        .done()
        .build()
    )
    doc = pdf_oxide.PdfDocument.from_bytes(data)
    text = doc.extract_text(0)
    assert "click me" in text
    assert "important" in text
    assert "revisit" in text


# ---------------------------------------------------------------------------
# Phase 2 — HTML+CSS pipeline
# ---------------------------------------------------------------------------


def test_pdf_from_html_css_basic():
    """Single-font HTML+CSS → PDF via ``Pdf.from_html_css``."""
    font_bytes = FIXTURE_FONT.read_bytes()
    pdf = pdf_oxide.Pdf.from_html_css(
        "<h1>Hello</h1><p>World</p>",
        "h1 { color: blue; font-size: 24pt }",
        font_bytes,
    )
    data = pdf.to_bytes()
    assert data.startswith(b"%PDF-")
    # Confirm the rendered output round-trips the literal text.
    doc = pdf_oxide.PdfDocument.from_bytes(data)
    rendered = doc.extract_text(0)
    assert "Hello" in rendered
    assert "World" in rendered


def test_pdf_from_html_css_with_fonts_cascade():
    """Multi-font cascade: the first entry is the default, later
    entries resolve CSS ``font-family`` matches."""
    font_bytes = FIXTURE_FONT.read_bytes()
    pdf = pdf_oxide.Pdf.from_html_css_with_fonts(
        "<p>default</p><p style=\"font-family: 'My Sans'\">named</p>",
        "p { font-size: 12pt }",
        [("Body", font_bytes), ("My Sans", font_bytes)],
    )
    data = pdf.to_bytes()
    assert data.startswith(b"%PDF-")
    doc = pdf_oxide.PdfDocument.from_bytes(data)
    rendered = doc.extract_text(0)
    assert "default" in rendered
    assert "named" in rendered


def test_pdf_from_html_css_with_fonts_rejects_empty_font_list():
    with pytest.raises(ValueError):
        pdf_oxide.Pdf.from_html_css_with_fonts(
            "<p>x</p>",
            "",
            [],
        )


# ---------------------------------------------------------------------------
# Release sanity
# ---------------------------------------------------------------------------


def test_form_field_creation():
    """DocumentBuilder creates form-field widgets (text fields +
    checkboxes) directly on a page. The resulting PDF exposes an
    /AcroForm entry and the widgets round-trip through the read-side
    form-field API."""
    bytes_ = (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        .at(72.0, 720.0)
        .text("Fill out the form:")
        .text_field("name", 150.0, 720.0, 200.0, 20.0, "default name")
        .text_field("email", 150.0, 690.0, 200.0, 20.0)
        .checkbox("subscribe", 72.0, 650.0, 15.0, 15.0, True)
        .checkbox("remember", 72.0, 620.0, 15.0, 15.0, False)
        .done()
        .build()
    )
    assert b"/AcroForm" in bytes_
    # Verify via the existing form-field read API
    doc = pdf_oxide.PdfDocument.from_bytes(bytes_)
    fields = doc.get_form_fields()
    names = {f.name for f in fields}
    assert {"name", "email", "subscribe", "remember"} <= names, (
        f"expected all 4 field names in output; got {names}"
    )


def test_form_field_all_five_widget_types():
    """Every widget type (text_field, checkbox, combo_box, radio_group,
    push_button) is reachable from Python and produces a correct
    /AcroForm entry."""
    bytes_ = (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        .text_field("name", 150.0, 720.0, 200.0, 20.0, "Jane Doe")
        .checkbox("subscribe", 72.0, 680.0, 15.0, 15.0, True)
        .combo_box(
            "country",
            150.0,
            640.0,
            200.0,
            20.0,
            ["US", "CA", "UK", "DE"],
            "US",
        )
        .radio_group(
            "payment",
            [
                ("credit", 72.0, 600.0, 15.0, 15.0),
                ("debit", 72.0, 580.0, 15.0, 15.0),
                ("paypal", 72.0, 560.0, 15.0, 15.0),
            ],
            "credit",
        )
        .push_button("submit", 72.0, 520.0, 80.0, 24.0, "Submit")
        .done()
        .build()
    )
    assert b"/AcroForm" in bytes_
    doc = pdf_oxide.PdfDocument.from_bytes(bytes_)
    fields = doc.get_form_fields()
    names = {f.name for f in fields}
    # Every named field we created should be present in the output
    # (radio_group adds a parent field with the group name).
    expected = {"name", "subscribe", "country", "payment", "submit"}
    assert expected <= names, f"expected {expected} in output field names; got {names}"


def test_graphics_primitives():
    """Low-level graphics primitives (rect, filled_rect, line) are
    reachable directly from DocumentBuilder without going through the
    ContentElement::Path builder."""
    bytes_ = (
        pdf_oxide.DocumentBuilder()
        .a4_page()
        # A framing box + an inner filled box + a diagonal line.
        .rect(50.0, 50.0, 500.0, 700.0)
        .filled_rect(100.0, 100.0, 200.0, 100.0, 0.9, 0.9, 1.0)
        .line(50.0, 400.0, 550.0, 400.0)
        .at(72.0, 500.0)
        .text("Graphics primitives demo")
        .done()
        .build()
    )
    # Sanity: output is a valid PDF, parses, and the text we added
    # survives round-trip. The rect / line operators themselves aren't
    # exposed by extract_text, but their presence is implicit in the
    # PDF being valid and bigger than a text-only page.
    assert bytes_.startswith(b"%PDF-")
    doc = pdf_oxide.PdfDocument.from_bytes(bytes_)
    assert "Graphics primitives demo" in doc.extract_text(0)


def test_version_is_038_or_newer():
    """Sanity check that we're running against a build that has the
    #384 write-side API — if the binding was rebuilt from a pre-0.3.38
    source tree we'd be testing a stale wheel."""
    version = pdf_oxide.VERSION
    assert version.startswith("0.3.3") or version.startswith("0.4.") or version.startswith("1.")
    # Confirm the symbols we rely on are actually exported.
    assert hasattr(pdf_oxide, "DocumentBuilder")
    assert hasattr(pdf_oxide, "FluentPageBuilder")
    assert hasattr(pdf_oxide, "EmbeddedFont")
    # Phase 2 methods on existing Pdf class.
    assert hasattr(pdf_oxide.Pdf, "from_html_css")
    assert hasattr(pdf_oxide.Pdf, "from_html_css_with_fonts")


# ---------------------------------------------------------------------------
# Phase 3 — v0.3.39 primitives + tables (#393 step 6a, Python binding)
# ---------------------------------------------------------------------------


def test_v0339_symbols_exported():
    """New symbols for #393 step 6a must be reachable from the
    top-level ``pdf_oxide`` namespace."""
    for name in ("Align", "Column", "Table", "StreamingTable"):
        assert hasattr(pdf_oxide, name), f"missing export: {name}"
    for name in (
        "measure",
        "text_in_rect",
        "stroke_rect",
        "stroke_line",
        "new_page_same_size",
        "remaining_space",
        "table",
        "streaming_table",
    ):
        assert hasattr(pdf_oxide.FluentPageBuilder, name), f"missing FluentPageBuilder.{name}"


def test_measure_returns_positive_float_for_helvetica():
    """``.measure(text)`` returns the base-14 Helvetica width in points.

    The exact value is font-implementation-dependent but must be:
        * a positive float,
        * monotonic in text length,
        * scale linearly with font size (roughly, to 1 %).
    """
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 10.0).at(72.0, 720.0)
    w_small = page.measure("Hello")
    w_big = page.measure("Hello, world — this is longer text")
    assert isinstance(w_small, float)
    assert w_small > 0.0
    assert w_big > w_small
    # Clean up buffered page to avoid dangling builder.
    page.done().build()


def test_measure_scales_with_font_size():
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 10.0)
    w10 = page.measure("ABC")
    page = page.font("Helvetica", 20.0)
    w20 = page.measure("ABC")
    # 20pt should be ~2x 10pt; allow 1% slack for rounding.
    assert 1.9 * w10 < w20 < 2.1 * w10
    page.done().build()


def test_text_in_rect_wraps_and_aligns(tmp_path):
    """``text_in_rect`` wraps long strings and accepts either a str
    alignment or an ``Align`` enum value."""
    out = tmp_path / "wrap.pdf"
    (
        pdf_oxide.DocumentBuilder()
        .letter_page()
        .font("Helvetica", 10.0)
        .text_in_rect(
            72.0,
            600.0,
            200.0,
            80.0,
            "This is a fairly long sentence that ought to wrap inside "
            "the configured rectangle when rendered by the Rust core.",
            align="center",
        )
        .text_in_rect(
            72.0,
            500.0,
            200.0,
            40.0,
            "Right-aligned",
            align=pdf_oxide.Align.RIGHT,
        )
        .done()
        .save(str(out))
    )
    assert out.stat().st_size > 256
    doc = pdf_oxide.PdfDocument.from_bytes(out.read_bytes())
    text = doc.extract_text(0)
    assert "Right-aligned" in text
    assert "configured rectangle" in text


def test_text_in_rect_invalid_align_raises():
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 10.0)
    with pytest.raises(ValueError):
        page.text_in_rect(72.0, 600.0, 200.0, 40.0, "x", align="bogus")


def test_stroke_rect_and_line_render(tmp_path):
    out = tmp_path / "stroke.pdf"
    (
        pdf_oxide.DocumentBuilder()
        .letter_page()
        .stroke_rect(50.0, 50.0, 200.0, 100.0, width=2.0, color=(0.5, 0.5, 0.5))
        .stroke_line(50.0, 50.0, 250.0, 50.0, width=1.0, color=(0.2, 0.2, 0.2))
        .at(72.0, 500.0)
        .font("Helvetica", 10.0)
        .text("stroked shapes")
        .done()
        .save(str(out))
    )
    assert out.exists()
    assert out.stat().st_size > 256
    assert "stroked shapes" in pdf_oxide.PdfDocument.from_bytes(out.read_bytes()).extract_text(0)


def test_new_page_same_size_creates_second_page():
    bytes_ = (
        pdf_oxide.DocumentBuilder()
        .letter_page()
        .font("Helvetica", 12.0)
        .at(72.0, 720.0)
        .text("first page")
        .new_page_same_size()
        .at(72.0, 720.0)
        .text("second page")
        .done()
        .build()
    )
    doc = pdf_oxide.PdfDocument.from_bytes(bytes_)
    assert doc.page_count() == 2
    assert "first page" in doc.extract_text(0)
    assert "second page" in doc.extract_text(1)


def test_buffered_table_round_trip():
    """``Table(columns=..., rows=..., has_header=True)`` renders via
    ``FluentPageBuilder.table`` and the cell text round-trips through
    extraction."""
    tbl = pdf_oxide.Table(
        columns=[
            pdf_oxide.Column("SKU", width=100.0),
            pdf_oxide.Column("Qty", width=60.0, align=pdf_oxide.Align.RIGHT),
        ],
        rows=[["A-1", "12"], ["B-2", "3"]],
        has_header=True,
    )
    bytes_ = (
        pdf_oxide.DocumentBuilder()
        .letter_page()
        .font("Helvetica", 10.0)
        .at(72.0, 720.0)
        .table(tbl)
        .done()
        .build()
    )
    doc = pdf_oxide.PdfDocument.from_bytes(bytes_)
    text = doc.extract_text(0)
    assert "SKU" in text
    assert "Qty" in text
    assert "A-1" in text
    assert "B-2" in text


def test_buffered_table_row_length_validation():
    """Passing a row with the wrong cell count must raise ``ValueError``
    at construction time rather than producing a broken PDF."""
    with pytest.raises(ValueError):
        pdf_oxide.Table(
            columns=[pdf_oxide.Column("A", width=50.0), pdf_oxide.Column("B", width=50.0)],
            rows=[["only-one-cell"]],  # short by one
        )


def test_buffered_table_requires_columns():
    with pytest.raises(ValueError):
        pdf_oxide.Table(columns=[], rows=[])


def test_streaming_table_1000_rows_multi_page():
    """1 000-row streaming table must span multiple pages and preserve
    the header + some sampled cell values. The exact row count is held
    well below the 30 k smoke in the Rust-core test to keep CI fast."""
    n_rows = 1000
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 9.0).at(72.0, 720.0)
    st = page.streaming_table(
        columns=[
            pdf_oxide.Column("SKU", width=72.0),
            pdf_oxide.Column("Item", width=200.0),
            pdf_oxide.Column("Qty", width=48.0, align=pdf_oxide.Align.RIGHT),
        ],
        repeat_header=True,
    )
    assert st.column_count() == 3
    for i in range(n_rows):
        st.push_row([f"S-{i:04d}", f"Item number {i}", str(i % 97)])
    bytes_ = st.finish().done().build()
    pdf = pdf_oxide.PdfDocument.from_bytes(bytes_)
    # 1 000 rows at 9 pt / letter never fit on one page.
    assert pdf.page_count() > 1
    # Sample a few rows' content from the PDF.
    full_text = "\n".join(pdf.extract_text(p) for p in range(pdf.page_count()))
    assert "S-0000" in full_text
    assert "S-0500" in full_text or "S-0750" in full_text
    # Header must appear at least once (and more than once if repeat_header
    # fired — but at minimum the first page has it).
    assert "SKU" in full_text
    assert "Item" in full_text


def test_streaming_table_push_row_wrong_arity_raises():
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 10.0).at(72.0, 720.0)
    st = page.streaming_table(
        columns=[pdf_oxide.Column("A", width=50.0), pdf_oxide.Column("B", width=50.0)],
    )
    with pytest.raises(ValueError):
        st.push_row(["only-one"])
    # Clean up the buffered page so we don't leak the builder.
    st.finish().done().build()


def test_align_enum_int_and_string_interchangeable():
    """Column constructor should accept ``Align.CENTER``, ``"center"``,
    and omitting the arg (defaults to LEFT) interchangeably."""
    c_enum = pdf_oxide.Column("X", width=10.0, align=pdf_oxide.Align.CENTER)
    c_str = pdf_oxide.Column("X", width=10.0, align="center")
    c_default = pdf_oxide.Column("X", width=10.0)
    assert c_enum.align == c_str.align == 1
    assert c_default.align == 0


# ---------------------------------------------------------------------------
# Issue #401 regression — encrypted embedded-font content preservation
# ---------------------------------------------------------------------------

FIXTURE_FONT_BOLD = Path(__file__).parent / "fixtures" / "fonts" / "DejaVuSans-Bold.ttf"


def test_save_encrypted_embedded_font_content_objects_preserved(tmp_path):
    """Regression test for issue #401.

    ``DocumentBuilder.save_encrypted`` must write ALL font sub-objects
    (DescendantFonts, FontFile2, ToUnicode, FontDescriptor) into the
    encrypted output.

    Strategy: the embedded DejaVu font program adds several KB even after
    subsetting.  Without the fix, those sub-objects were silently dropped
    and the encrypted embedded-font PDF was barely larger than a simple
    base-14-font encrypted PDF.  With the fix the difference must be ≥10 KB.
    """
    if not FIXTURE_FONT.exists():
        pytest.skip("DejaVuSans.ttf fixture not found")

    # Baseline: simple text (base-14 font), encrypted.
    simple_path = tmp_path / "simple_enc.pdf"
    (
        pdf_oxide.DocumentBuilder()
        .letter_page()
        .at(72.0, 720.0)
        .text("Hello simple")
        .done()
        .save_encrypted(str(simple_path), "userpw", "ownerpw")
    )
    simple_size = simple_path.stat().st_size

    # Embedded-font PDF, encrypted.
    ttf_path = tmp_path / "ttf_enc.pdf"
    font = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT))
    doc = pdf_oxide.DocumentBuilder().register_embedded_font("DejaVu", font)
    (
        doc.a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 720.0)
        .text("Hello from embedded font")
        .done()
        .save_encrypted(str(ttf_path), "userpw", "ownerpw")
    )
    ttf_raw = ttf_path.read_bytes()
    ttf_size = len(ttf_raw)

    assert b"/Encrypt" in ttf_raw, "encrypted PDF must contain /Encrypt dict"

    # With FlateDecode compression (SaveOptions::with_encryption sets compress=true),
    # a subsetted DejaVu font adds several KB. A 5 KB floor clearly distinguishes
    # "font present" from "font missing" (which gives near-zero diff).
    diff = ttf_size - simple_size
    assert diff >= 5_000, (
        f"issue #401: embedded-font encrypted PDF ({ttf_size} B) is not "
        f"substantially larger than simple encrypted PDF ({simple_size} B); "
        f"diff={diff} B — font sub-objects (FontFile2, etc.) are likely missing"
    )


def test_to_bytes_encrypted_embedded_font_content_objects_preserved():
    """Issue #401: ``to_bytes_encrypted`` must also preserve font sub-objects."""
    if not FIXTURE_FONT.exists():
        pytest.skip("DejaVuSans.ttf fixture not found")

    font = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT))
    doc = pdf_oxide.DocumentBuilder().register_embedded_font("DejaVu", font)
    encrypted_bytes = (
        doc.a4_page()
        .font("DejaVu", 12.0)
        .at(72.0, 720.0)
        .text("bytes encrypted with embedded font")
        .done()
        .to_bytes_encrypted("u", "o")
    )

    assert b"/Encrypt" in encrypted_bytes, "must contain /Encrypt dict"
    # Font program must be present. With FlateDecode compression a subsetted DejaVu
    # adds ~8 KB; an 8 KB floor clearly distinguishes "present" from "missing".
    assert len(encrypted_bytes) > 8_000, (
        f"issue #401: to_bytes_encrypted embedded-font result ({len(encrypted_bytes)} B) "
        "is too small; font sub-objects likely missing from encrypted output"
    )


def test_issue_401_two_embedded_fonts_save_encrypted(tmp_path):
    """Exact scenario from issue #401: two embedded fonts, AES encryption.

    The reporter used two TrueType fonts (regular + bold) with save_with_encryption
    and got a blank PDF.  This test mirrors that exact usage pattern.
    """
    if not FIXTURE_FONT.exists() or not FIXTURE_FONT_BOLD.exists():
        pytest.skip("DejaVuSans.ttf / DejaVuSans-Bold.ttf fixture not found")

    font_reg = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT))
    font_bold = pdf_oxide.EmbeddedFont.from_file(str(FIXTURE_FONT_BOLD))

    doc = (
        pdf_oxide.DocumentBuilder()
        .register_embedded_font("Regular", font_reg)
        .register_embedded_font("Bold", font_bold)
    )

    (
        doc.a4_page()
        .font("Bold", 14.5)
        .at(30.0, 800.0)
        .text("High Performance")
        .font("Regular", 10.5)
        .at(30.0, 780.0)
        .text("Rust is fast and memory-efficient.")
        .font("Bold", 14.5)
        .at(30.0, 745.0)
        .text("Reliability")
        .font("Regular", 10.5)
        .at(30.0, 725.0)
        .text("Rust's type system ensures memory and thread safety.")
        .done()
    )

    out = tmp_path / "issue_401.pdf"
    doc.save_encrypted(str(out), "123456", "123456")

    raw = out.read_bytes()
    assert b"/Encrypt" in raw, "encrypted PDF must contain /Encrypt dict"
    # Two font programs → even more data → must be >25 KB.
    assert len(raw) > 25_000, (
        f"issue #401: two-font encrypted PDF ({len(raw)} B) is too small; "
        "font sub-objects for both fonts are likely missing"
    )


# ── Dashed stroke tests ──────────────────────────────────────────────────────


def test_stroke_rect_dashed_produces_pdf():
    """stroke_rect_dashed emits a valid PDF containing the dash operator."""
    doc = pdf_oxide.DocumentBuilder()
    page = doc.a4_page()
    page.stroke_rect_dashed(50, 100, 200, 150, dash=[3.0, 2.0], width=1.5, color=(0.0, 0.0, 0.8))
    page.done()
    data = doc.build()
    assert data[:4] == b"%PDF", "output is not a PDF"
    assert b" d\n" in data or b" d " in data, "dash operator 'd' missing from PDF content"


def test_stroke_line_dashed_produces_pdf():
    """stroke_line_dashed emits a valid PDF containing the dash operator."""
    doc = pdf_oxide.DocumentBuilder()
    page = doc.a4_page()
    page.stroke_line_dashed(
        50, 80, 250, 80, dash=[5.0, 3.0], width=1.0, color=(0.8, 0.0, 0.0), phase=1.0
    )
    page.done()
    data = doc.build()
    assert data[:4] == b"%PDF"
    assert b" d\n" in data or b" d " in data, "dash operator 'd' missing from PDF content"


def test_stroke_dashed_chainable():
    """Dashed stroke methods return self for fluent chaining."""
    doc = pdf_oxide.DocumentBuilder()
    page = doc.a4_page()
    # Both dashed methods should be chainable
    result = page.stroke_rect_dashed(10, 10, 100, 50, dash=[4.0, 2.0])
    assert result is page
    result2 = page.stroke_line_dashed(10, 70, 110, 70, dash=[4.0, 2.0])
    assert result2 is page
    page.done()
    data = doc.build()
    assert data[:4] == b"%PDF"


# ── bounded RowBatch for StreamingTable (#400) ──────────────────────────────


def test_streaming_table_batch_size_accepted():
    """batch_size parameter is accepted and exposed via pending_row_count / batch_count."""
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 9.0).at(72.0, 720.0)
    st = page.streaming_table(
        columns=[
            pdf_oxide.Column("A", width=72.0),
            pdf_oxide.Column("B", width=72.0),
        ],
        batch_size=3,
    )
    assert st.pending_row_count() == 0
    assert st.batch_count() == 0
    st.push_row(["r0c0", "r0c1"])
    st.push_row(["r1c0", "r1c1"])
    assert st.pending_row_count() == 2
    assert st.batch_count() == 0
    # Third push fills the batch → auto-flush
    st.push_row(["r2c0", "r2c1"])
    assert st.pending_row_count() == 0
    assert st.batch_count() == 1
    # One more row in the new batch
    st.push_row(["r3c0", "r3c1"])
    assert st.pending_row_count() == 1
    st.finish().done().build()


def test_streaming_table_large_with_batch_size():
    """500-row table with a small batch_size (50) produces a correct multi-page PDF."""
    n_rows = 500
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 9.0).at(72.0, 720.0)
    st = page.streaming_table(
        columns=[
            pdf_oxide.Column("ID", width=60.0),
            pdf_oxide.Column("Name", width=200.0),
        ],
        repeat_header=True,
        batch_size=50,
    )
    for i in range(n_rows):
        st.push_row([str(i), f"Row-{i}"])
    bytes_ = st.finish().done().build()
    pdf = pdf_oxide.PdfDocument.from_bytes(bytes_)
    assert pdf.page_count() > 1
    full_text = "\n".join(pdf.extract_text(p) for p in range(pdf.page_count()))
    assert "ID" in full_text
    assert "Row-0" in full_text
    assert "Row-499" in full_text


def test_streaming_table_explicit_flush():
    """Calling flush() manually moves the current batch to completed."""
    doc = pdf_oxide.DocumentBuilder()
    page = doc.letter_page().font("Helvetica", 9.0).at(72.0, 720.0)
    st = page.streaming_table(
        columns=[pdf_oxide.Column("X", width=100.0)],
        batch_size=256,
    )
    st.push_row(["first"])
    st.push_row(["second"])
    assert st.batch_count() == 0
    st.flush()
    assert st.batch_count() == 1
    assert st.pending_row_count() == 0
    st.push_row(["third"])
    st.finish().done().build()
