// SPDX-License-Identifier: MIT OR Apache-2.0
//go:build pdf_oxide_dev

// Package pdfoxide provides Go bindings to the pdf_oxide Rust PDF toolkit.
//
// This file (cgo_dev.go) holds dev-mode linker flags used when building
// inside the pdf_oxide monorepo after `cargo build --release --lib
// [--target <triple>]`. Paths are relative to this file's directory
// (`${SRCDIR}`) and point at the Cargo workspace's per-target output dir.
//
// Usage:
//
//	cd go && go build -tags pdf_oxide_dev ./...
//
// Production consumers never hit this file — the default build has no tag,
// in which case CGO_LDFLAGS must come from the environment (set by the
// install CLI) or from a locally-generated cgo_flags.go.
package pdfoxide

/*
#cgo linux,amd64 LDFLAGS: ${SRCDIR}/../target/release/libpdf_oxide.a -lm -lpthread -ldl -lrt -lgcc_s -lutil -lc
#cgo linux,arm64 LDFLAGS: ${SRCDIR}/../target/aarch64-unknown-linux-gnu/release/libpdf_oxide.a -lm -lpthread -ldl -lrt -lgcc_s -lutil -lc
#cgo darwin,amd64 LDFLAGS: ${SRCDIR}/../target/release/libpdf_oxide.a -framework CoreFoundation -framework Security -framework SystemConfiguration -liconv -lresolv
#cgo darwin,arm64 LDFLAGS: ${SRCDIR}/../target/aarch64-apple-darwin/release/libpdf_oxide.a -framework CoreFoundation -framework Security -framework SystemConfiguration -liconv -lresolv
#cgo windows,amd64 LDFLAGS: ${SRCDIR}/../target/x86_64-pc-windows-gnu/release/libpdf_oxide.a -lws2_32 -luserenv -lbcrypt -ladvapi32 -lcrypt32 -lsynchronization -lntdll -lkernel32 -lntoskrnl -lole32 -lshell32
*/
import "C"
