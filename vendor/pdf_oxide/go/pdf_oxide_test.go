//go:build cgo

package pdfoxide

import (
	"errors"
	"os"
	"path/filepath"
	"sync"
	"testing"
)

// createTestPDF generates a small PDF from Markdown and writes it to a temp
// file. Tests that need a real PDF on disk use this instead of hardcoded
// fixture paths.
func createTestPDF(t *testing.T, content string) string {
	t.Helper()
	pdf, err := FromMarkdown(content)
	if err != nil {
		t.Skipf("FromMarkdown unavailable in this build: %v", err)
	}
	defer pdf.Close()

	dir := t.TempDir()
	path := filepath.Join(dir, "test.pdf")
	if err := pdf.Save(path); err != nil {
		t.Fatalf("Save failed: %v", err)
	}
	return path
}

// ─── Error handling ─────────────────────────────────────────────────────────

func TestOpen_NonExistentFile(t *testing.T) {
	_, err := Open("/nonexistent/path/to/file.pdf")
	if err == nil {
		t.Fatal("expected error for non-existent file")
	}
	if !errors.Is(err, ErrDocumentNotFound) && !errors.Is(err, ErrInvalidPath) {
		t.Errorf("expected ErrDocumentNotFound or ErrInvalidPath, got %v", err)
	}
}

func TestOpen_ClosedDocument(t *testing.T) {
	doc := &PdfDocument{closed: true}

	if _, err := doc.PageCount(); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("PageCount: expected ErrDocumentClosed, got %v", err)
	}
	if _, _, err := doc.Version(); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("Version: expected ErrDocumentClosed, got %v", err)
	}
	if _, err := doc.HasStructureTree(); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("HasStructureTree: expected ErrDocumentClosed, got %v", err)
	}
	if _, err := doc.ExtractText(0); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("ExtractText: expected ErrDocumentClosed, got %v", err)
	}
	if _, err := doc.Fonts(0); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("Fonts: expected ErrDocumentClosed, got %v", err)
	}
	if _, err := doc.Annotations(0); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("Annotations: expected ErrDocumentClosed, got %v", err)
	}
	if _, err := doc.PageElements(0); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("PageElements: expected ErrDocumentClosed, got %v", err)
	}
	if _, err := doc.SearchAll("x", false); !errors.Is(err, ErrDocumentClosed) {
		t.Errorf("SearchAll: expected ErrDocumentClosed, got %v", err)
	}
}

func TestEditor_ClosedEditor(t *testing.T) {
	editor := &DocumentEditor{closed: true}

	if _, err := editor.IsModified(); !errors.Is(err, ErrEditorClosed) {
		t.Errorf("IsModified: expected ErrEditorClosed, got %v", err)
	}
	if _, _, err := editor.Version(); !errors.Is(err, ErrEditorClosed) {
		t.Errorf("Version: expected ErrEditorClosed, got %v", err)
	}
	if _, err := editor.Title(); !errors.Is(err, ErrEditorClosed) {
		t.Errorf("Title: expected ErrEditorClosed, got %v", err)
	}
	if err := editor.SetTitle("x"); !errors.Is(err, ErrEditorClosed) {
		t.Errorf("SetTitle: expected ErrEditorClosed, got %v", err)
	}
	if err := editor.ApplyMetadata(Metadata{Title: "x"}); !errors.Is(err, ErrEditorClosed) {
		t.Errorf("ApplyMetadata: expected ErrEditorClosed, got %v", err)
	}
}

func TestFromMarkdown_EmptyContent(t *testing.T) {
	// Empty content may succeed or fail depending on the native layer;
	// when it fails it should be a well-typed error.
	pdf, err := FromMarkdown("")
	if err == nil {
		pdf.Close()
	} else if !errors.Is(err, ErrEmptyContent) && !errors.Is(err, ErrInternal) {
		t.Errorf("unexpected error type: %v", err)
	}
}

func TestError_SentinelUnwrap(t *testing.T) {
	// ffiError should wrap the canonical sentinel so errors.Is works.
	err := ffiError(2)
	if !errors.Is(err, ErrDocumentNotFound) {
		t.Errorf("errors.Is(ffiError(2), ErrDocumentNotFound) = false, want true")
	}
	if errors.Is(err, ErrInvalidPath) {
		t.Errorf("errors.Is(ffiError(2), ErrInvalidPath) = true, want false")
	}

	// errors.As should give access to the Code.
	var e *Error
	if !errors.As(err, &e) {
		t.Fatal("errors.As failed to extract *Error")
	}
	if e.Code != 2 {
		t.Errorf("Code = %d, want 2", e.Code)
	}
}

// ─── End-to-end happy path ──────────────────────────────────────────────────

func TestRoundTrip_CreateOpenExtract(t *testing.T) {
	content := "# Hello World\n\nThis is a test PDF with searchable content.\n\nThe quick brown fox jumps over the lazy dog."
	path := createTestPDF(t, content)

	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open failed: %v", err)
	}
	defer doc.Close()

	count, err := doc.PageCount()
	if err != nil {
		t.Fatalf("PageCount failed: %v", err)
	}
	if count < 1 {
		t.Errorf("PageCount = %d, want >= 1", count)
	}

	major, minor, err := doc.Version()
	if err != nil {
		t.Fatalf("Version failed: %v", err)
	}
	if major == 0 && minor == 0 {
		t.Error("Version returned 0.0 — should have a real PDF version")
	}

	text, err := doc.ExtractText(0)
	if err != nil {
		t.Fatalf("ExtractText failed: %v", err)
	}
	if len(text) == 0 {
		t.Error("ExtractText returned empty — should contain at least some of the markdown text")
	}
}

func TestOpenFromBytes(t *testing.T) {
	path := createTestPDF(t, "# Byte Test\n\nContent loaded from bytes.")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}

	doc, err := OpenFromBytes(data)
	if err != nil {
		t.Fatalf("OpenFromBytes: %v", err)
	}
	defer doc.Close()

	if _, err := doc.PageCount(); err != nil {
		t.Errorf("PageCount on byte-loaded doc: %v", err)
	}
}

func TestOpenFromBytes_Empty(t *testing.T) {
	if _, err := OpenFromBytes(nil); !errors.Is(err, ErrEmptyContent) {
		t.Errorf("expected ErrEmptyContent for nil, got %v", err)
	}
	if _, err := OpenFromBytes([]byte{}); !errors.Is(err, ErrEmptyContent) {
		t.Errorf("expected ErrEmptyContent for empty slice, got %v", err)
	}
}

func TestFonts_OnCreatedDocument(t *testing.T) {
	path := createTestPDF(t, "# Fonts\n\nThis PDF uses **at least one** embedded font.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()

	fonts, err := doc.Fonts(0)
	if err != nil {
		t.Fatalf("Fonts: %v", err)
	}
	// We don't assert a specific font — just that the JSON round trip works
	// and the slice is usable (not nil-panic'ing).
	for _, f := range fonts {
		if f.Name == "" {
			t.Error("font has empty name — JSON decode failure?")
		}
	}
}

func TestAnnotations_OnCreatedDocument(t *testing.T) {
	path := createTestPDF(t, "# No Annotations\n\nThis document has no annotations.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()

	anns, err := doc.Annotations(0)
	if err != nil {
		t.Fatalf("Annotations: %v", err)
	}
	if anns == nil {
		t.Error("Annotations returned nil slice — should be empty slice")
	}
}

func TestSearchAll(t *testing.T) {
	path := createTestPDF(t, "# Search Target\n\nThe word FINDME appears exactly twice: FINDME.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()

	// This may legitimately return zero hits depending on the rendering
	// pipeline's text positioning. We just assert the error path is clean
	// and the slice is usable (not panicking on iteration).
	hits, err := doc.SearchAll("FINDME", false)
	if err != nil {
		t.Fatalf("SearchAll: %v", err)
	}
	for _, h := range hits {
		if h.Page < 0 {
			t.Errorf("negative page index in result: %+v", h)
		}
	}
}

func TestEditor_RoundTrip(t *testing.T) {
	path := createTestPDF(t, "# Editable\n\nOriginal body.")
	editor, err := OpenEditor(path)
	if err != nil {
		t.Fatalf("OpenEditor: %v", err)
	}
	defer editor.Close()

	modified, err := editor.IsModified()
	if err != nil {
		t.Fatalf("IsModified: %v", err)
	}
	if modified {
		t.Error("freshly opened editor reports IsModified = true")
	}

	if err := editor.ApplyMetadata(Metadata{
		Title:  "Round-trip test",
		Author: "pdf_oxide tests",
	}); err != nil {
		t.Fatalf("ApplyMetadata: %v", err)
	}

	outPath := filepath.Join(t.TempDir(), "out.pdf")
	if err := editor.Save(outPath); err != nil {
		t.Fatalf("Save: %v", err)
	}
	if info, err := os.Stat(outPath); err != nil || info.Size() == 0 {
		t.Fatalf("saved file invalid: %v, info=%v", err, info)
	}
}

// ─── Concurrency ────────────────────────────────────────────────────────────

func TestConcurrentReads(t *testing.T) {
	path := createTestPDF(t, "# Concurrent\n\nReader safety test.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()

	var wg sync.WaitGroup
	errs := make(chan error, 100)

	for i := 0; i < 20; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 5; j++ {
				if _, err := doc.PageCount(); err != nil {
					errs <- err
					return
				}
				if _, err := doc.ExtractText(0); err != nil {
					errs <- err
					return
				}
			}
		}()
	}

	wg.Wait()
	close(errs)
	for err := range errs {
		t.Errorf("concurrent operation failed: %v", err)
	}
}

// ─── IsClosed idempotence ───────────────────────────────────────────────────

func TestClose_Idempotent(t *testing.T) {
	path := createTestPDF(t, "# Close\n\nTest.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}

	doc.Close()
	doc.Close() // second call must not panic

	if !doc.IsClosed() {
		t.Error("IsClosed should return true after Close")
	}
}
