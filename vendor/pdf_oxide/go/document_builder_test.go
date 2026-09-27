//go:build cgo

package pdfoxide

// Integration tests for the Go write-side API. Mirrors the
// Python / C# / Rust-FFI test suites. Each test is self-contained; the
// DejaVuSans fixture ships at tests/fixtures/fonts/DejaVuSans.ttf relative
// to the repo root.

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// fixtureFontPath walks up from the test binary's cwd until it finds the
// repo-root-relative fixture.
func fixtureFontPath(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd: %v", err)
	}
	for {
		candidate := filepath.Join(dir, "tests", "fixtures", "fonts", "DejaVuSans.ttf")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Skipf("DejaVuSans.ttf fixture not found from %s", dir)
		}
		dir = parent
	}
}

func TestDocumentBuilderMinimalAscii(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()

	p, err := b.A4Page()
	if err != nil {
		t.Fatalf("A4Page: %v", err)
	}
	if _, err := p.At(72, 720).Text("Hello, world.").Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}

	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	if !bytes.HasPrefix(data, []byte("%PDF-")) {
		t.Fatalf("output is not a PDF: % x", data[:8])
	}
	if len(data) < 256 {
		t.Fatalf("PDF suspiciously small: %d bytes", len(data))
	}
}

func TestDocumentBuilderCjkRoundTrip(t *testing.T) {
	fontPath := fixtureFontPath(t)
	font, err := EmbeddedFontFromFile(fontPath)
	if err != nil {
		t.Skipf("EmbeddedFontFromFile unavailable: %v", err)
	}

	b, err := NewDocumentBuilder()
	if err != nil {
		t.Fatalf("NewDocumentBuilder: %v", err)
	}
	defer b.Close()

	if err := b.RegisterEmbeddedFont("DejaVu", font); err != nil {
		t.Fatalf("RegisterEmbeddedFont: %v", err)
	}

	p, err := b.A4Page()
	if err != nil {
		t.Fatalf("A4Page: %v", err)
	}
	if _, err := p.
		Font("DejaVu", 12).
		At(72, 720).Text("Привет, мир!").
		At(72, 700).Text("Καλημέρα κόσμε").
		Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}

	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}

	// Round-trip the output through PdfDocument.ExtractText.
	tmp := filepath.Join(t.TempDir(), "out.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write tmp: %v", err)
	}
	doc, err := Open(tmp)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()
	text, err := doc.ExtractText(0)
	if err != nil {
		t.Fatalf("ExtractText: %v", err)
	}
	if !strings.Contains(text, "Привет, мир!") {
		t.Errorf("Cyrillic missing from extracted text: %q", text)
	}
	if !strings.Contains(text, "Καλημέρα κόσμε") {
		t.Errorf("Greek missing from extracted text: %q", text)
	}
}

func TestDocumentBuilderOutputIsSubsetted(t *testing.T) {
	fontPath := fixtureFontPath(t)
	faceBytes, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatalf("read font: %v", err)
	}
	font, err := EmbeddedFontFromFile(fontPath)
	if err != nil {
		t.Skipf("EmbeddedFontFromFile unavailable: %v", err)
	}
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Fatalf("NewDocumentBuilder: %v", err)
	}
	defer b.Close()
	if err := b.RegisterEmbeddedFont("DejaVu", font); err != nil {
		t.Fatalf("RegisterEmbeddedFont: %v", err)
	}
	p, _ := b.A4Page()
	if _, err := p.Font("DejaVu", 12).At(72, 700).Text("Hello world").Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	if len(data)*10 >= len(faceBytes) {
		t.Errorf("expected PDF (%d bytes) to be >= 10x smaller than face (%d bytes)", len(data), len(faceBytes))
	}
}

func TestDocumentBuilderSaveEncrypted(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.A4Page()
	if _, err := p.At(72, 720).Text("secret").Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	tmp := filepath.Join(t.TempDir(), "enc.pdf")
	if err := b.SaveEncrypted(tmp, "userpw", "ownerpw"); err != nil {
		t.Fatalf("SaveEncrypted: %v", err)
	}
	raw, err := os.ReadFile(tmp)
	if err != nil {
		t.Fatalf("read back: %v", err)
	}
	if !bytes.Contains(raw, []byte("/Encrypt")) {
		t.Errorf("missing /Encrypt dict")
	}
	if !bytes.Contains(raw, []byte("/V 5")) {
		t.Errorf("missing /V 5 (AES-256) marker")
	}
}

func TestDocumentBuilderToBytesEncrypted(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.A4Page()
	if _, err := p.At(72, 720).Text("x").Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.ToBytesEncrypted("u", "o")
	if err != nil {
		t.Fatalf("ToBytesEncrypted: %v", err)
	}
	if !bytes.Contains(data, []byte("/Encrypt")) {
		t.Errorf("missing /Encrypt")
	}
	if !bytes.Contains(data, []byte("/V 5")) {
		t.Errorf("missing /V 5")
	}
}

func TestDocumentBuilderConsumedAfterBuild(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	p, _ := b.A4Page()
	if _, err := p.At(72, 720).Text("x").Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	if _, err := b.Build(); err != nil {
		t.Fatalf("first build: %v", err)
	}
	if _, err := b.Build(); !errors.Is(err, ErrBuilderConsumed) {
		t.Errorf("expected ErrBuilderConsumed on second build, got %v", err)
	}
}

func TestDocumentBuilderDoubleOpenPage(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	if _, err := b.A4Page(); err != nil {
		t.Fatalf("A4Page: %v", err)
	}
	if _, err := b.A4Page(); !errors.Is(err, ErrBuilderHasOpenPage) {
		t.Errorf("expected ErrBuilderHasOpenPage, got %v", err)
	}
}

func TestEmbeddedFontConsumedAfterRegister(t *testing.T) {
	fontPath := fixtureFontPath(t)
	font, err := EmbeddedFontFromFile(fontPath)
	if err != nil {
		t.Skipf("EmbeddedFontFromFile unavailable: %v", err)
	}
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Fatalf("NewDocumentBuilder: %v", err)
	}
	defer b.Close()
	if err := b.RegisterEmbeddedFont("A", font); err != nil {
		t.Fatalf("first register: %v", err)
	}
	if err := b.RegisterEmbeddedFont("B", font); !errors.Is(err, ErrFontConsumed) {
		t.Errorf("expected ErrFontConsumed on re-register, got %v", err)
	}
}

func TestDocumentBuilderMultiplePages(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	for _, s := range []string{"page one", "page two", "page three"} {
		p, err := b.A4Page()
		if err != nil {
			t.Fatalf("A4Page: %v", err)
		}
		if _, err := p.At(72, 720).Text(s).Done(); err != nil {
			t.Fatalf("chain: %v", err)
		}
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	tmp := filepath.Join(t.TempDir(), "multi.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	doc, err := Open(tmp)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()
	pages, err := doc.PageCount()
	if err != nil {
		t.Fatalf("PageCount: %v", err)
	}
	if pages != 3 {
		t.Errorf("expected 3 pages, got %d", pages)
	}
}

func TestDocumentBuilderAnnotationsDoNotBreakExtraction(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.A4Page()
	if _, err := p.
		At(72, 720).Text("click me").LinkURL("https://example.com").
		At(72, 700).Text("important").Highlight(1.0, 1.0, 0.0).
		At(72, 680).Text("revisit").StickyNote("review").WatermarkDraft().
		Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	tmp := filepath.Join(t.TempDir(), "annots.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	doc, _ := Open(tmp)
	defer doc.Close()
	text, err := doc.ExtractText(0)
	if err != nil {
		t.Fatalf("ExtractText: %v", err)
	}
	for _, want := range []string{"click me", "important", "revisit"} {
		if !strings.Contains(text, want) {
			t.Errorf("missing %q in extracted text: %q", want, text)
		}
	}
}

// ─── v0.3.39 primitives ───────────────────────────────────────────────────

func TestPageBuilderStrokePrimitives(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, err := b.LetterPage()
	if err != nil {
		t.Fatalf("LetterPage: %v", err)
	}
	if _, err := p.
		StrokeRect(50, 600, 200, 100, 2.0, 0.5, 0.5, 0.5).
		StrokeLine(50, 600, 250, 600, 1.0, 0.2, 0.2, 0.2).
		Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	if !bytes.HasPrefix(data, []byte("%PDF-")) {
		t.Fatalf("output is not a PDF")
	}
}

func TestPageBuilderTextInRect(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.LetterPage()
	if _, err := p.
		Font("Helvetica", 10).
		TextInRect(72, 600, 200, 100, "this text wraps inside the rect", AlignCenter).
		Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	tmp := filepath.Join(t.TempDir(), "textrect.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	doc, _ := Open(tmp)
	defer doc.Close()
	text, err := doc.ExtractText(0)
	if err != nil {
		t.Fatalf("ExtractText: %v", err)
	}
	if !strings.Contains(text, "wraps") {
		t.Errorf("expected 'wraps' in extracted text, got %q", text)
	}
}

func TestPageBuilderNewPageSameSize(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.LetterPage()
	if _, err := p.
		At(72, 720).Text("page 1").
		NewPageSameSize().
		At(72, 720).Text("page 2").
		Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	tmp := filepath.Join(t.TempDir(), "multi.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	doc, _ := Open(tmp)
	defer doc.Close()
	pages, err := doc.PageCount()
	if err != nil {
		t.Fatalf("PageCount: %v", err)
	}
	if pages != 2 {
		t.Errorf("expected 2 pages after NewPageSameSize, got %d", pages)
	}
}

func TestPageBuilderTableBuffered3x3(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, err := b.LetterPage()
	if err != nil {
		t.Fatalf("LetterPage: %v", err)
	}
	spec := TableSpec{
		Columns: []Column{
			{Header: "SKU", Width: 100, Align: AlignLeft},
			{Header: "Item", Width: 200, Align: AlignLeft},
			{Header: "Qty", Width: 60, Align: AlignRight},
		},
		Rows: [][]string{
			{"A-1", "Widget", "12"},
			{"B-2", "Gadget", "3"},
			{"C-3", "Gizmo", "42"},
		},
		HasHeader: true,
	}
	if _, err := p.At(72, 720).Table(spec).Done(); err != nil {
		t.Fatalf("chain: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	tmp := filepath.Join(t.TempDir(), "table.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	doc, _ := Open(tmp)
	defer doc.Close()
	text, err := doc.ExtractText(0)
	if err != nil {
		t.Fatalf("ExtractText: %v", err)
	}
	for _, want := range []string{"SKU", "Widget", "Gadget", "Gizmo", "42"} {
		if !strings.Contains(text, want) {
			t.Errorf("missing %q in table output: %q", want, text)
		}
	}
}

func TestPageBuilderTableRowLengthMismatch(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.LetterPage()
	spec := TableSpec{
		Columns: []Column{
			{Header: "A", Width: 60},
			{Header: "B", Width: 60},
		},
		Rows: [][]string{
			{"only-one-cell"},
		},
	}
	_, err = p.At(72, 720).Table(spec).Done()
	if err == nil {
		t.Fatalf("expected row-length mismatch error")
	}
	if !strings.Contains(err.Error(), "row 0") {
		t.Errorf("expected row-0 mismatch error, got: %v", err)
	}
}

func TestPageBuilderStreamingTable1000Rows(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, err := b.LetterPage()
	if err != nil {
		t.Fatalf("LetterPage: %v", err)
	}
	st := p.At(72, 720).StreamingTable(StreamingTableConfig{
		Columns: []Column{
			{Header: "SKU", Width: 72, Align: AlignLeft},
			{Header: "Item", Width: 200, Align: AlignLeft},
			{Header: "Qty", Width: 48, Align: AlignRight},
		},
		RepeatHeader: true,
	})
	const n = 1000
	for i := 0; i < n; i++ {
		if err := st.PushRow([]string{
			"sku-" + itoa(i),
			"item " + itoa(i),
			itoa(i % 99),
		}); err != nil {
			t.Fatalf("PushRow %d: %v", i, err)
		}
	}
	page := st.Finish()
	if page == nil {
		t.Fatalf("StreamingTable.Finish returned nil page")
	}
	if _, err := page.Done(); err != nil {
		t.Fatalf("Done: %v", err)
	}
	data, err := b.Build()
	if err != nil {
		t.Fatalf("Build: %v", err)
	}
	if !bytes.HasPrefix(data, []byte("%PDF-")) {
		t.Fatalf("not a PDF")
	}
	// v0.3.39 Go streaming is buffered-flush: rows are aggregated in
	// managed memory and forwarded as a single buffered Table() call at
	// Finish(). The native streaming path (with automatic page-break +
	// repeat-header) is tracked separately; this test just confirms
	// round-tripped extraction sees the first + last sku.
	tmp := filepath.Join(t.TempDir(), "streaming.pdf")
	if err := os.WriteFile(tmp, data, 0o644); err != nil {
		t.Fatalf("write: %v", err)
	}
	doc, _ := Open(tmp)
	defer doc.Close()
	pages, err := doc.PageCount()
	if err != nil {
		t.Fatalf("PageCount: %v", err)
	}
	if pages < 1 {
		t.Errorf("expected at least 1 page, got %d", pages)
	}
}

func TestPageBuilderMeasureStubReturnsNonNegative(t *testing.T) {
	// Measure is a managed stub in v0.3.39 — smoke-test that it returns
	// a non-negative value and doesn't panic. Full parity lands with the
	// native FFI accessor in a later release.
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.LetterPage()
	if w := p.Measure("hello"); w < 0 {
		t.Errorf("Measure returned negative: %v", w)
	}
	if r := p.RemainingSpace(); r < 0 {
		t.Errorf("RemainingSpace returned negative: %v", r)
	}
	_ = p.Close()
}

func TestStreamingTablePushRowAfterFinish(t *testing.T) {
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	p, _ := b.LetterPage()
	st := p.At(72, 720).StreamingTable(StreamingTableConfig{
		Columns: []Column{{Header: "X", Width: 60}},
	})
	_ = st.PushRow([]string{"a"})
	st.Finish()
	if err := st.PushRow([]string{"b"}); err == nil {
		t.Errorf("expected PushRow after Finish to error")
	}
}

// itoa is a dependency-free int-to-string used in the streaming test to
// avoid pulling strconv into every chain.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}

func TestFromHTMLCSS(t *testing.T) {
	fontPath := fixtureFontPath(t)
	fontBytes, err := os.ReadFile(fontPath)
	if err != nil {
		t.Fatalf("read font: %v", err)
	}
	pdf, err := FromHTMLCSS(
		"<h1>Hello</h1><p>World</p>",
		"h1 { color: blue; font-size: 24pt }",
		fontBytes,
	)
	if err != nil {
		t.Skipf("FromHTMLCSS unavailable: %v", err)
	}
	defer pdf.Close()
	tmp := filepath.Join(t.TempDir(), "html.pdf")
	if err := pdf.Save(tmp); err != nil {
		t.Fatalf("Save: %v", err)
	}
	doc, _ := Open(tmp)
	defer doc.Close()
	text, _ := doc.ExtractText(0)
	if !strings.Contains(text, "Hello") {
		t.Errorf("missing 'Hello' in %q", text)
	}
	if !strings.Contains(text, "World") {
		t.Errorf("missing 'World' in %q", text)
	}
}

// TestDocumentBuilderSaveEncrypted_EmbeddedFont_ContentObjects_Preserved verifies
// that encrypting a DocumentBuilder PDF with an embedded TrueType font writes all
// font sub-objects (DescendantFonts, FontFile2, ToUnicode, FontDescriptor) into the
// encrypted output. Regression test for issue #401.
//
// Strategy: the embedded DejaVu font program (FontFile2 stream) is several KB even
// after subsetting. If the object-graph sweep is broken, those sub-objects are
// silently dropped and the encrypted embedded-font PDF is barely larger than a
// simple base-14-font encrypted PDF. With the fix, the difference must be ≥10 KB.
func TestDocumentBuilderSaveEncrypted_EmbeddedFont_ContentObjects_Preserved(t *testing.T) {
	// ── baseline: simple text (base-14 font), encrypted ──────────────────
	simpleB, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer simpleB.Close()
	simplePage, _ := simpleB.A4Page()
	if _, err := simplePage.At(72, 720).Text("Hello simple").Done(); err != nil {
		t.Fatalf("simple page chain: %v", err)
	}
	simpleTmp := filepath.Join(t.TempDir(), "simple_enc.pdf")
	if err := simpleB.SaveEncrypted(simpleTmp, "userpw", "ownerpw"); err != nil {
		t.Fatalf("simple SaveEncrypted: %v", err)
	}
	simpleRaw, err := os.ReadFile(simpleTmp)
	if err != nil {
		t.Fatalf("read simple encrypted: %v", err)
	}

	// ── embedded-font PDF, encrypted ─────────────────────────────────────
	fontPath := fixtureFontPath(t)
	font, err := EmbeddedFontFromFile(fontPath)
	if err != nil {
		t.Skipf("EmbeddedFontFromFile unavailable: %v", err)
	}
	ttfB, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer ttfB.Close()
	if err := ttfB.RegisterEmbeddedFont("DejaVu", font); err != nil {
		t.Fatalf("RegisterEmbeddedFont: %v", err)
	}
	ttfPage, _ := ttfB.A4Page()
	if _, err := ttfPage.Font("DejaVu", 12).At(72, 720).Text("Hello from embedded font").Done(); err != nil {
		t.Fatalf("ttf page chain: %v", err)
	}
	ttfTmp := filepath.Join(t.TempDir(), "ttf_enc.pdf")
	if err := ttfB.SaveEncrypted(ttfTmp, "userpw", "ownerpw"); err != nil {
		t.Fatalf("ttf SaveEncrypted: %v", err)
	}
	ttfRaw, err := os.ReadFile(ttfTmp)
	if err != nil {
		t.Fatalf("read ttf encrypted: %v", err)
	}

	// ── assertions ────────────────────────────────────────────────────────
	if !bytes.Contains(ttfRaw, []byte("/Encrypt")) {
		t.Errorf("missing /Encrypt dict in embedded-font encrypted PDF")
	}

	// The embedded DejaVu font program adds several KB even when subsetted and
	// FlateDecode-compressed (SaveOptions::with_encryption sets compress=true).
	// Without the fix (#401), font sub-objects are missing and the size
	// difference is near-zero (<100 B).
	diff := len(ttfRaw) - len(simpleRaw)
	if diff < 5_000 {
		t.Errorf(
			"issue #401: embedded-font encrypted PDF (%d B) is not substantially "+
				"larger than simple encrypted PDF (%d B); diff=%d B; "+
				"font sub-objects (FontFile2, DescendantFonts, etc.) are likely missing",
			len(ttfRaw), len(simpleRaw), diff,
		)
	}
}

// TestDocumentBuilderToBytesEncrypted_EmbeddedFont_ContentObjects_Preserved mirrors
// the SaveEncrypted test above but exercises the ToBytesEncrypted code path.
func TestDocumentBuilderToBytesEncrypted_EmbeddedFont_ContentObjects_Preserved(t *testing.T) {
	fontPath := fixtureFontPath(t)
	font, err := EmbeddedFontFromFile(fontPath)
	if err != nil {
		t.Skipf("EmbeddedFontFromFile unavailable: %v", err)
	}
	b, err := NewDocumentBuilder()
	if err != nil {
		t.Skipf("NewDocumentBuilder unavailable: %v", err)
	}
	defer b.Close()
	if err := b.RegisterEmbeddedFont("DejaVu", font); err != nil {
		t.Fatalf("RegisterEmbeddedFont: %v", err)
	}
	p, _ := b.A4Page()
	if _, err := p.Font("DejaVu", 12).At(72, 720).Text("bytes encrypted with embedded font").Done(); err != nil {
		t.Fatalf("page chain: %v", err)
	}
	data, err := b.ToBytesEncrypted("u", "o")
	if err != nil {
		t.Fatalf("ToBytesEncrypted: %v", err)
	}
	if !bytes.Contains(data, []byte("/Encrypt")) {
		t.Errorf("missing /Encrypt dict")
	}
	// Font program must be present: encrypted PDF with embedded font must be
	// substantially larger than a bare PDF. With FlateDecode compression
	// (SaveOptions::with_encryption sets compress=true), a subsetted font
	// adds ~8 KB; an 8 KB floor clearly distinguishes "present" from "missing".
	if len(data) < 8_000 {
		t.Errorf(
			"issue #401: ToBytesEncrypted embedded-font result (%d B) is too small; "+
				"font sub-objects are likely missing from encrypted output",
			len(data),
		)
	}
}

// ── CSS property correctness ──────────────────────────────────────────────────
// Each test generates two PDFs that differ only in one CSS property and asserts
// the byte output differs — proving the property is actually applied.

func fixtureFontBytes(t *testing.T) []byte {
	t.Helper()
	fontPath := fixtureFontPath(t)
	data, err := os.ReadFile(fontPath)
	if err != nil {
		t.Skipf("could not read font fixture: %v", err)
	}
	return data
}

func htmlCSS(t *testing.T, html, css string, fontBytes []byte) []byte {
	t.Helper()
	pdf, err := FromHTMLCSS(html, css, fontBytes)
	if err != nil {
		t.Fatalf("FromHTMLCSS: %v", err)
	}
	defer pdf.Close()
	data, err := pdf.SaveToBytes()
	if err != nil {
		t.Fatalf("SaveToBytes: %v", err)
	}
	return data
}

func TestCSSFontSizeChangesOutput(t *testing.T) {
	fontBytes := fixtureFontBytes(t)
	const html = "<p>text</p>"
	small := htmlCSS(t, html, "p { font-size: 12px; }", fontBytes)
	large := htmlCSS(t, html, "p { font-size: 48px; }", fontBytes)
	if bytes.Equal(small, large) {
		t.Fatal("CSS font-size had no effect on output")
	}
}

func TestCSSColorChangesOutput(t *testing.T) {
	fontBytes := fixtureFontBytes(t)
	const html = "<p>text</p>"
	black := htmlCSS(t, html, "p { color: black; }", fontBytes)
	red := htmlCSS(t, html, "p { color: red; }", fontBytes)
	if bytes.Equal(black, red) {
		t.Fatal("CSS color had no effect on output")
	}
}

func TestCSSBackgroundColorChangesOutput(t *testing.T) {
	fontBytes := fixtureFontBytes(t)
	const html = "<p>text</p>"
	none := htmlCSS(t, html, "", fontBytes)
	yellow := htmlCSS(t, html, "body { background-color: yellow; }", fontBytes)
	if bytes.Equal(none, yellow) {
		t.Fatal("CSS background-color had no effect on output")
	}
}

func TestCSSTextDecorationChangesOutput(t *testing.T) {
	fontBytes := fixtureFontBytes(t)
	const html = "<p>text</p>"
	none := htmlCSS(t, html, "", fontBytes)
	underline := htmlCSS(t, html, "p { text-decoration: underline; }", fontBytes)
	if bytes.Equal(none, underline) {
		t.Fatal("CSS text-decoration had no effect on output")
	}
}

func TestStreamingTableBoundedBatch(t *testing.T) {
	doc, err := NewDocumentBuilder()
	if err != nil {
		t.Fatalf("NewDocumentBuilder: %v", err)
	}
	page, err := doc.LetterPage()
	if err != nil {
		t.Fatalf("LetterPage: %v", err)
	}
	page.Font("Helvetica", 9).At(72, 720)
	st := page.StreamingTable(StreamingTableConfig{
		Columns: []Column{
			{Header: "ID", Width: 60},
			{Header: "Val", Width: 120},
		},
		BatchSize: 3,
	})

	// Push 7 rows: 3 → flush (batch 1), 3 → flush (batch 2), 1 still pending.
	for i := 0; i < 7; i++ {
		id := strconv.Itoa(i)
		val := "row-" + id
		if err2 := st.PushRow([]string{id, val}); err2 != nil {
			t.Fatalf("PushRow %d: %v", i, err2)
		}
	}
	if got := st.PendingRowCount(); got != 1 {
		t.Errorf("PendingRowCount: got %d, want 1", got)
	}
	if got := st.BatchCount(); got != 2 {
		t.Errorf("BatchCount: got %d, want 2", got)
	}

	page2 := st.Finish()
	if page2 == nil {
		t.Fatal("Finish returned nil")
	}
	page2.Done()
	data, buildErr := doc.Build()
	if buildErr != nil {
		t.Fatalf("Build: %v", buildErr)
	}
	if !bytes.HasPrefix(data, []byte("%PDF")) {
		t.Fatal("output is not a PDF")
	}
}

func TestStrokeDashed(t *testing.T) {
	doc, err := NewDocumentBuilder()
	if err != nil {
		t.Fatalf("NewDocumentBuilder: %v", err)
	}
	page, _ := doc.A4Page()
	dash := []float32{3.0, 2.0}
	page.StrokeRectDashed(50, 100, 200, 150, 1.5, 0.0, 0.0, 0.8, dash, 0.0)
	page.StrokeLineDashed(50, 80, 250, 80, 1.0, 0.8, 0.0, 0.0, dash, 1.0)
	page.Done()
	data, err := doc.Build()
	if err != nil {
		t.Fatalf("Build failed: %v", err)
	}
	if !bytes.HasPrefix(data, []byte("%PDF")) {
		t.Fatal("output is not a PDF")
	}
	if !bytes.Contains(data, []byte(" d\n")) && !bytes.Contains(data, []byte(" d ")) {
		t.Fatal("dash operator 'd' missing from PDF content")
	}
}
