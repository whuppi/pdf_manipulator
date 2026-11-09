//go:build cgo

package pdfoxide

// Feature-guard tests — mirror of C# OcrEngineTests.cs / Node.js feature-guard.test.mjs
//
// When the native lib is compiled without rendering / signatures / barcodes
// the FFI returns error code 8 (_ERR_UNSUPPORTED). These tests accept both
// outcomes so the suite passes against both full-features and bare-features
// builds:
//
//   feature ON  → operation succeeds, assertion holds
//   feature OFF → t.Skipf("unavailable in this build: ...")
//
// isUnsupportedError is also used by render_options_test.go to guard its
// success-path assertions.

import (
	"strings"
	"testing"
)

// isUnsupportedError returns true when the error indicates a feature was not
// compiled into the native library (FFI code 8 / _ERR_UNSUPPORTED).
func isUnsupportedError(err error) bool {
	if err == nil {
		return false
	}
	msg := strings.ToLower(err.Error())
	return strings.Contains(msg, "unsupported") ||
		strings.Contains(msg, "error code 8") ||
		strings.Contains(msg, "not compiled") ||
		strings.Contains(msg, "5000")
}

// ── Rendering ────────────────────────────────────────────────────────────────

func TestFeatureGuard_RenderPage(t *testing.T) {
	path := createTestPDF(t, "# Guard\n\nBody.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()

	img, err := doc.RenderPageWithOptions(0, RenderOptions{})
	if err != nil {
		if isUnsupportedError(err) {
			t.Skipf("RenderPageWithOptions unavailable in this build: %v", err)
		}
		t.Fatalf("unexpected render error: %v", err)
	}
	defer img.Close()
	if len(img.Data()) == 0 {
		t.Fatal("rendered image has no data")
	}
}

// ── Signatures ────────────────────────────────────────────────────────────────

func TestFeatureGuard_SignatureCount(t *testing.T) {
	path := createTestPDF(t, "# Sig\n\nBody.")
	doc, err := Open(path)
	if err != nil {
		t.Fatalf("Open: %v", err)
	}
	defer doc.Close()

	count, err := doc.SignatureCount()
	if err != nil {
		if isUnsupportedError(err) {
			t.Skipf("SignatureCount unavailable in this build: %v", err)
		}
		t.Fatalf("unexpected error: %v", err)
	}
	if count < 0 {
		t.Fatalf("expected non-negative signature count, got %d", count)
	}
}

// ── Barcodes ──────────────────────────────────────────────────────────────────

func TestFeatureGuard_GenerateBarcode(t *testing.T) {
	const barcodeCode128 = 0 // format: 0=Code128 per GenerateBarcode docs
	bc, err := GenerateBarcode("HELLO", barcodeCode128, 300)
	if err != nil {
		if isUnsupportedError(err) {
			t.Skipf("GenerateBarcode unavailable in this build: %v", err)
		}
		t.Fatalf("unexpected error: %v", err)
	}
	defer bc.Close()
	png, err := bc.PNGData()
	if err != nil {
		t.Fatalf("PNGData: %v", err)
	}
	if len(png) < 8 {
		t.Fatal("PNG too short")
	}
}
