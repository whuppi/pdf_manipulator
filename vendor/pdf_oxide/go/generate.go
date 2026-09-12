//go:build !pdf_oxide_dev

// Convenience: `go generate github.com/yfedoseev/pdf_oxide/go` triggers the
// installer which downloads the native lib for this platform from GitHub
// Releases and prints the CGO_CFLAGS / CGO_LDFLAGS to export.
//
// In practice, users run the installer directly (see README) rather than
// via `go generate` on the module cache (which is read-only). This
// directive is here for parity with the Kreuzberg-style flow and for the
// `//go:generate go run ...@latest --write-flags=.` pattern in consumer
// projects.

//go:generate go run github.com/yfedoseev/pdf_oxide/go/cmd/install

package pdfoxide
