//go:build cgo

// Cross-language benchmark — Go binding.
//
// Emits NDJSON matching the Rust baseline (bench/rust_bench.rs) so results
// can be aggregated and compared. Cgo-only: uses SearchAll and other
// surfaces that are not ported to the purego backend.
//
// Run with:
//
//	cd go && go build -o ../target/go_bench ../bench/go_bench
//	LD_LIBRARY_PATH=go/lib/linux_amd64 target/go_bench bench_fixtures/tiny.pdf ...
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	pdfoxide "github.com/yfedoseev/pdf_oxide/go"
)

const iterations = 5

type fixtureResult struct {
	Language       string `json:"language"`
	Fixture        string `json:"fixture"`
	SizeBytes      int64  `json:"sizeBytes"`
	OpenNs         int64  `json:"openNs"`
	ExtractPage0Ns int64  `json:"extractPage0Ns"`
	ExtractAllNs   int64  `json:"extractAllNs"`
	SearchNs       int64  `json:"searchNs"`
	PageCount      int    `json:"pageCount"`
	TextLen        int    `json:"textLen"`
}

func benchFixture(path string) (*fixtureResult, error) {
	info, err := os.Stat(path)
	if err != nil {
		return nil, fmt.Errorf("stat: %w", err)
	}

	// Warm-up pass (not measured) — exercises every code path we're about to
	// measure so JIT/lazy init costs are amortized away.
	{
		doc, err := pdfoxide.Open(path)
		if err != nil {
			return nil, fmt.Errorf("warmup open: %w", err)
		}
		_, _ = doc.ExtractText(0)
		_, _ = doc.SearchAll("the", false)
		doc.Close()
	}

	// Open (average across iterations).
	var openTotal time.Duration
	for i := 0; i < iterations; i++ {
		start := time.Now()
		doc, err := pdfoxide.Open(path)
		openTotal += time.Since(start)
		if err != nil {
			return nil, fmt.Errorf("open: %w", err)
		}
		doc.Close()
	}
	openNs := openTotal.Nanoseconds() / iterations

	// Extract text page 0 (average across iterations on a single open doc).
	doc, err := pdfoxide.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open: %w", err)
	}
	defer doc.Close()

	pageCount, err := doc.PageCount()
	if err != nil {
		return nil, fmt.Errorf("PageCount: %w", err)
	}

	var p0Total time.Duration
	var textLen int
	for i := 0; i < iterations; i++ {
		start := time.Now()
		text, err := doc.ExtractText(0)
		p0Total += time.Since(start)
		if err != nil {
			return nil, fmt.Errorf("ExtractText: %w", err)
		}
		textLen = len(text)
	}
	extractPage0Ns := p0Total.Nanoseconds() / iterations

	// Extract all pages (single run).
	start := time.Now()
	for i := 0; i < pageCount; i++ {
		if _, err := doc.ExtractText(i); err != nil {
			return nil, fmt.Errorf("ExtractText(%d): %w", i, err)
		}
	}
	extractAllNs := time.Since(start).Nanoseconds()

	// SearchAll for a common word (single run).
	start = time.Now()
	if _, err := doc.SearchAll("the", false); err != nil {
		return nil, fmt.Errorf("SearchAll: %w", err)
	}
	searchNs := time.Since(start).Nanoseconds()

	return &fixtureResult{
		Language:       "go",
		Fixture:        filepath.Base(path),
		SizeBytes:      info.Size(),
		OpenNs:         openNs,
		ExtractPage0Ns: extractPage0Ns,
		ExtractAllNs:   extractAllNs,
		SearchNs:       searchNs,
		PageCount:      pageCount,
		TextLen:        textLen,
	}, nil
}

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "usage: go_bench <fixture.pdf>...")
		os.Exit(1)
	}
	enc := json.NewEncoder(os.Stdout)
	for _, path := range os.Args[1:] {
		r, err := benchFixture(path)
		if err != nil {
			fmt.Fprintf(os.Stderr, "go_bench failed for %s: %v\n", path, err)
			os.Exit(2)
		}
		if err := enc.Encode(r); err != nil {
			fmt.Fprintf(os.Stderr, "encode failed: %v\n", err)
			os.Exit(2)
		}
	}
}
