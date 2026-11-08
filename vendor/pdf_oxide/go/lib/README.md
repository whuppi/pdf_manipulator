# Native Libraries — installed on demand

Starting with **v0.3.31**, native `pdf_oxide` libraries are no longer
committed to this module. Instead they're downloaded from GitHub Releases
on demand by a small Go installer, which removes ~310 MB of per-release
repository bloat (Rust staticlibs for 6 platforms).

## Install (one-time per machine)

```bash
# Always resolves to the matching FFI assets for the installer's own version.
go run github.com/yfedoseev/pdf_oxide/go/cmd/install@latest
# Or pin to a specific version:
go run github.com/yfedoseev/pdf_oxide/go/cmd/install@v0.3.38
```

The installer detects `GOOS`/`GOARCH`, downloads the matching asset from
`https://github.com/yfedoseev/pdf_oxide/releases/download/v0.3.38/…`, and
extracts `libpdf_oxide.a` + `pdf_oxide.h` into `os.UserCacheDir()/pdf_oxide/v0.3.38/`
— that is:

| OS       | Default path                                |
| -------- | ------------------------------------------- |
| Linux    | `~/.cache/pdf_oxide/v0.3.38/`               |
| macOS    | `~/Library/Caches/pdf_oxide/v0.3.38/`       |
| Windows  | `%LocalAppData%\pdf_oxide\v0.3.38\`         |

Override with `-dir <path>`. (Earlier releases used `~/.pdf_oxide/` on
all platforms; v0.3.38 switched to `os.UserCacheDir()` to follow the
per-platform convention Go's own toolchain uses for `GOCACHE`.)

It then prints the `CGO_CFLAGS` / `CGO_LDFLAGS` you need to export (Linux example):

```
export CGO_CFLAGS="-I$HOME/.cache/pdf_oxide/v0.3.38/include"
export CGO_LDFLAGS="$HOME/.cache/pdf_oxide/v0.3.38/lib/linux_amd64/libpdf_oxide.a -lm -lpthread -ldl -lrt -lgcc_s -lutil -lc"
```

After that, `go build` / `go test` work normally.

## purego (CGO_ENABLED=0) — shared library instead of staticlib

v0.3.38 also ships a pure-Go backend using
[ebitengine/purego](https://github.com/ebitengine/purego). It `dlopen`s the
native library at runtime — no C toolchain at build time. Use the installer's
`-shared` flag to fetch the `.so`/`.dylib`/`.dll` variant:

```bash
go run github.com/yfedoseev/pdf_oxide/go/cmd/install@latest -shared
```

The installer prints:

```
export CGO_ENABLED=0
export PDF_OXIDE_LIB_PATH=$HOME/.cache/pdf_oxide/v0.3.38/lib/linux_amd64/libpdf_oxide.so
```

Build selection is automatic: Go's built-in `cgo` build constraint is set
whenever `CGO_ENABLED=1`, so the CGo backend (`//go:build cgo`) is picked
by default and the purego backend (`//go:build !cgo`) is picked when
`CGO_ENABLED=0`. No custom tags required.

Coverage note: the purego backend currently implements the read-side
document API (text/Markdown/HTML/plain-text per page and all pages,
fonts, annotations, page elements, search, page dimensions, logging).
Editor, write-side builder, barcodes, signatures, TSA, rendering, OCR,
and forms remain CGo-only — using them under `CGO_ENABLED=0` yields a
compile error.

## Alternative: `go generate`

If you prefer to wire installation into your own project's build, add this
to any `.go` file in your project:

```go
//go:generate go run github.com/yfedoseev/pdf_oxide/go/cmd/install@latest --write-flags=.
```

Running `go generate ./...` then drops a `cgo_flags.go` next to your
`//go:generate` directive with the right `#cgo LDFLAGS` baked in for your
machine's install path. That file is per-machine — add it to `.gitignore`.

## Development / monorepo builds

If you're working inside the `pdf_oxide` monorepo and have already run
`cargo build --release --lib`, build the Go module with the `pdf_oxide_dev`
tag to use the workspace `target/` path directly:

```bash
cd go && go build -tags pdf_oxide_dev ./...
```

No installer needed in that mode.

## Windows ARM64

Windows ARM64 currently ships a dynamic `pdf_oxide.dll` (not a staticlib)
because Rust's `aarch64-pc-windows-gnullvm` target is still Tier 3. Go
binaries for this platform must ship `pdf_oxide.dll` alongside the
executable at runtime.
