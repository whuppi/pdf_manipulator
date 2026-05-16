# Updating pdf_manipulator

Maintenance procedures for the package. For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).

pdf_manipulator wraps a vendored fork of [pdf_oxide](https://github.com/nickhimself/pdf_oxide) (Rust engine) via FFI (native) and WASM (web). The fork lives at `vendor/pdf_oxide/` on a patch branch. We track upstream plus our own additions:

| Source | Why we track it |
|---|---|
| pdf_oxide upstream tags | The Rust engine — page manipulation, text extraction, rendering, signatures |
| pdf_oxide C header (our fork with patches) | 29 additional C-ABI functions not yet in upstream |
| Rust toolchain | Cross-compilation for 13 native targets + WASM |
| WASM binary (`web_assets/`) | Compiled from the patched Rust, committed in git, shipped to web consumers |
| Pre-built native binaries (GitHub Releases) | Per-platform `.dylib` / `.a` / `.so` / `.dll` downloaded by the build hook — consumers need zero Rust. CI (`release.yml`) cross-compiles 13 targets on 3 runners and uploads to the GitHub Release automatically. |

---

## When to update

| Trigger | Procedure |
|---|---|
| pdf_oxide releases a new version | [S1 — Bump upstream](#s1--bump-upstream) |
| A Rust patch needs updating or adding | [S2 — Edit Rust patches](#s2--edit-rust-patches) |
| Adding a new FFI function | [S3 — Add FFI function](#s3--add-ffi-function) |
| Rebuilding WASM | [S4 — Rebuild WASM](#s4--rebuild-wasm) |
| Rebuilding native binaries | [S5 — Rebuild native binaries](#s5--rebuild-native-binaries) |
| Adding a new platform | [S6 — Add platform](#s6--add-platform) |
| Consumer reports a missing function | [S7 — Diagnose missing function](#s7--diagnose-missing-function) |

---

## S1 — Bump upstream

When pdf_oxide tags a new release:

```sh
cd vendor/pdf_oxide
git fetch origin
git log --oneline origin/main..HEAD   # see our patches

# Rebase our patch branch onto the new tag
git rebase --onto vX.Y.Z go/v0.3.47  # old base → new base

# If conflicts: resolve per-file, prioritize upstream, re-apply our additions
# The patches are additive (new functions); conflicts are rare

cd ../..
git add vendor/pdf_oxide

# Regenerate Dart bindings from the (possibly updated) C header
dart run ffigen --config ffigen.yaml

# Check for new upstream functions we should expose
git diff lib/src/ffi/native_bindings.g.dart | head -80

# If upstream added functions that replace our patches:
# 1. Remove the patch from vendor/pdf_oxide (header + ffi.rs + editor)
# 2. Remove the "LOCAL PATCH" comment block
# 3. Re-run ffigen
# 4. Update the Rust Patches table below

dart analyze .
dart test

# Update provenance (bottom of this file)
# Update CHANGELOG.md
# Version is read from pubspec.yaml — no hardcoded constant in hook/build.dart
```

After bumping, always run [S4](#s4--rebuild-wasm). Native binaries are compiled and uploaded by CI when you push the version bump to main ([S5](#s5--rebuild-native-binaries)).

---

## S2 — Edit Rust patches

Our fork carries patches on branch `pdf_manipulator/0.3.47-patches` (3 commits atop `go/v0.3.47`). To modify a patch:

```sh
cd vendor/pdf_oxide

# Edit the Rust source
# - C header declarations:  include/pdf_oxide_c/pdf_oxide.h
# - FFI implementations:    src/ffi.rs
# - Editor methods:         src/editor/document_editor.rs
# - New modules:            src/editor/ (e.g. image_optimizer.rs)

# Every patch MUST have this comment block:
#   LOCAL PATCH — pdf_manipulator/0.3.47-patches
#   <one-line description>
#   Removal trigger: <what upstream change makes this unnecessary>

# Verify it compiles
cargo build --lib --features "icc,legacy-crypto,rendering,signatures"

# Amend the relevant patch commit (interactive rebase if needed)
git add -A
git commit --amend  # or git rebase -i to pick the right commit

cd ../..
git add vendor/pdf_oxide

# Regenerate Dart bindings
dart run ffigen --config ffigen.yaml

dart analyze .
dart test
```

After editing patches, always run [S4](#s4--rebuild-wasm). Native binaries are compiled and uploaded by CI when you push the changes to main ([S5](#s5--rebuild-native-binaries)).

---

## S3 — Add FFI function

End-to-end checklist for exposing a new pdf_oxide function to Dart:

1. **Rust side** — add the `#[no_mangle] pub extern "C" fn` in `vendor/pdf_oxide/src/ffi.rs` (if not already upstream)
2. **C header** — add the declaration in `vendor/pdf_oxide/include/pdf_oxide_c/pdf_oxide.h` with the `LOCAL PATCH` comment block
3. **Regenerate bindings** — `dart run ffigen --config ffigen.yaml`
4. **Dart FFI wrapper** — add a safe wrapper in `lib/src/ffi/bindings.dart` (handles `Pointer`, null checks, error codes)
5. **Web WASM wrapper** — add the matching method in `lib/src/platform/_web.dart` → `WasmPdfDocument`
6. **Platform interface** — add the method to `lib/src/platform/pdf_platform.dart`
7. **Public API** — expose via `Pdf`, `PdfEditor`, or `PdfBuilder` in `lib/src/`
8. **Export** — ensure the public type is exported from `lib/pdf_manipulator.dart`
9. **Tests** — add test in `test/`
10. **Rebuild** — [S4](#s4--rebuild-wasm) if Rust changed; native binaries are handled by CI on push to main ([S5](#s5--rebuild-native-binaries))

Never hand-edit `lib/src/ffi/native_bindings.g.dart`. That file is ffigen output.

---

## S4 — Rebuild WASM

After any Rust-side change (upstream bump, patch edit, new function):

```sh
# Prerequisites (one-time):
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.121

./tool/build_wasm.sh

# Verify output
ls -lh web_assets/pdf_oxide*

# Commit the updated WASM artifacts
git add web_assets/
```

The script runs `cargo build --target wasm32-unknown-unknown --features wasm --no-default-features --release` then `wasm-bindgen --target web`. Output goes to `web_assets/` and is committed in git.

---

## S5 — Rebuild native binaries

Native binaries are compiled and uploaded automatically by CI. Contributors do not need to compile and commit binaries manually.

**How it works:** `release.yml` fires on every push to `main` that changes `pubspec.yaml`. If the version is new (no existing git tag), CI cross-compiles 13 native targets on 3 runners (macOS, Linux, Windows), runs tests on 5 platforms, creates a git tag, and uploads pre-built binaries to the corresponding GitHub Release.

| Runner | Targets |
|---|---|
| macOS (macos-14) | macOS arm64/x64, iOS arm64/sim-arm64/sim-x64, Android arm64/arm/x64/x86 (needs NDK) |
| Linux (ubuntu-latest) | Linux x64/arm64 |
| Windows (windows-latest) | Windows x64 (MSVC) |

**To trigger a release after a Rust-side change:**

```sh
# 1. Bump the version in pubspec.yaml
# 2. Commit all changes (vendor/pdf_oxide, web_assets/, Dart code)
# 3. Push to main
# 4. release.yml handles compilation + tag + GitHub Release upload
```

**For local development:** contributors compile from source automatically when running `dart test`. The build hook (`hook/build.dart`) detects `vendor/pdf_oxide/Cargo.toml` and runs `cargo build` — no manual compilation step needed. This requires a Rust toolchain (`rustup.rs`).

---

## S6 — Add platform

1. Add the Rust target: `rustup target add <triple>`
2. Add the cross-compilation step to `release.yml` (under the appropriate runner job)
3. Add the platform mapping in `hook/build.dart` (`_platformBinaries` map)
4. Test locally: `dart test` on the new platform (build hook compiles from source)
5. Push to main with a version bump — CI compiles and uploads the new binary to the GitHub Release
6. Update `docs/CAPABILITY_ROADMAP.md` infrastructure table

---

## S7 — Diagnose missing function

When a consumer reports "function X not found" or a runtime symbol-lookup failure:

```sh
# 1. Check if the function exists in the C header
grep "function_name" vendor/pdf_oxide/include/pdf_oxide_c/pdf_oxide.h

# 2. Check if ffigen picked it up
grep "function_name" lib/src/ffi/native_bindings.g.dart

# 3. Check if the symbol is in the compiled binary (after running dart test to trigger a local build)
nm -gU .dart_tool/native_assets/*/out/*/libpdf_oxide.dylib | grep "function_name"

# 4. If nm shows nothing: the Rust code doesn't export it
#    → Add [S3](#s3--add-ffi-function) with #[no_mangle] in ffi.rs

# 5. If nm shows it but Dart can't find it:
#    → The header is missing the declaration
#    → Add to pdf_oxide.h, re-run ffigen

# 6. If it's a web-only failure:
#    → Check WasmPdfDocument / WasmPdfEditor in _web.dart
#    → Check worker.js message dispatch
```

---

## Rust patches — the full inventory

Our fork (`pdf_manipulator/0.3.47-patches` branch, commits atop `go/v0.3.47`) adds 29 C-ABI functions and 8 Rust-level additions (including `document_editor_add_image_stamp` at the C-ABI level, `AppearanceStreamBuilder::for_image_stamp` and `ImageStampData` in the Rust writer, and `StampAnnotation.image_data` + `with_image()` on the stamp struct). These are functions pdf_oxide implements internally but had not yet exposed through its C header at tag v0.3.47.

### C header patches (in `include/pdf_oxide_c/pdf_oxide.h`)

| Function | What it does | Removal trigger |
|---|---|---|
| `pdf_document_open_from_bytes` | Open a PDF from a byte buffer instead of a file path | Upstream adds to C header |
| `pdf_document_open_from_bytes_with_password` | Open an encrypted PDF from bytes | Upstream adds to C header |
| `pdf_document_open_with_password` | Open an encrypted PDF from a file path | Upstream adds to C header |
| `document_editor_delete_page` | Remove a page by index | Upstream adds to C header |
| `document_editor_move_page` | Reorder a page (from index → to index) | Upstream adds to C header |
| `document_editor_extract_pages_to_bytes` | Extract a subset of pages as a new PDF byte buffer | Upstream adds to C header |
| `document_editor_save_encrypted_to_bytes` | Save the edited document with AES encryption to bytes | Upstream adds to C header |
| `document_editor_get_page_rotation` | Query a page's rotation in degrees | Upstream adds to C header |
| `document_editor_set_page_rotation` | Set a page's rotation | Upstream adds to C header |
| `document_editor_erase_region` | White-out a rectangular area on a page | Upstream adds to C header |
| `document_editor_crop_margins` | Crop all pages by the given margins | Upstream adds to C header |
| `document_editor_convert_to_pdf_a` | Convert the document to a PDF/A conformance level | Upstream adds to C header |
| `document_editor_set_form_field_value` | Set a named form field's value | Upstream adds to C header |
| `document_editor_flatten_annotations` | Flatten annotations on a single page | Upstream adds to C header |
| `document_editor_flatten_all_annotations` | Flatten annotations on every page | Upstream adds to C header |
| `document_editor_add_watermark` | Add a text watermark annotation to a page (font, rotation, opacity, color) | Upstream adds editor-level watermark |
| `document_editor_optimize_images` | Convert non-JPEG images to JPEG when smaller; returns count of optimized images | Upstream adds image optimization to editor |
| `pdf_validate_pdf_a_level` | Validate PDF/A compliance at a given level; returns results handle | Upstream adds to C header |
| `pdf_pdf_a_is_compliant` | Query whether the validation passed | Upstream adds to C header |
| `pdf_pdf_a_error_count` | Number of compliance errors found | Upstream adds to C header |
| `pdf_pdf_a_warning_count` | Number of compliance warnings found | Upstream adds to C header |
| `pdf_pdf_a_results_free` | Free the validation results handle | Upstream adds to C header |
| `document_editor_save_encrypted_full` | Save with algorithm choice (4 types) + 8 permission flags | Upstream adds full-featured encrypted save |
| `document_editor_add_watermark_positioned` | Add watermark with x, y, width, height, font name | Upstream adds positioned watermark |
| `document_editor_add_stamp` | Add stamp annotation (16 built-in types + custom) | Upstream adds stamp to editor |
| `document_editor_add_image_stamp` | Add image stamp with embedded JPEG/PNG in appearance stream XObject | Upstream adds image stamp to C header |
| `document_editor_resize_image` | Resize an image on a page (for DPI control) | Upstream adds resize_image to C header |
| `pdf_document_get_permissions` | Read 8 permission flags from encrypted PDF | Upstream adds to C header |
| `pdf_document_get_encryption_algorithm` | Read encryption algorithm (RC4-40/128, AES-128/256) | Upstream adds to C header |

### Rust-level patches

| Addition | File | What it does | Removal trigger |
|---|---|---|---|
| `PdfDocument::get_permissions()` | `src/document.rs` | Read encryption permissions via the handler | Upstream adds to public API |
| `PdfDocument::get_encryption_algorithm()` | `src/document.rs` | Read encryption algorithm via the handler | Upstream adds to public API |

### Original Rust-level patches (in `src/`)

| Addition | File | What it does | Removal trigger |
|---|---|---|---|
| `DocumentEditor::add_annotation()` | `src/editor/document_editor.rs` | Add an annotation to a page — used by the C-ABI watermark function | Upstream adds annotation-addition to editor API |
| `DocumentEditor::optimize_images()` | `src/editor/document_editor.rs` | Optimize images via the image_optimizer module | Upstream adds image optimization to editor API |
| `editor::image_optimizer` module | `src/editor/image_optimizer.rs` (new file) | Non-JPEG to JPEG conversion when resulting size is smaller | Upstream adds equivalent |
| `AppearanceStreamBuilder::for_image_stamp()` | `src/writer/appearance_stream.rs` | Embed image as XObject in stamp annotation appearance stream | Upstream adds image stamp appearance |
| `StampAnnotation.image_data` field + `with_image()` | `src/writer/stamp.rs` | Carry optional image bytes on a stamp annotation; build AP dict with XObject | Upstream adds image stamp support |
| `ImageStampData` struct | `src/writer/appearance_stream.rs` | Raw image bytes carrier for image stamps | Upstream adds equivalent |

### Patch discipline

- Every patch carries a `LOCAL PATCH — pdf_manipulator/0.3.47-patches` comment with a one-line description and a removal trigger.
- Patches are purely additive — they add new `#[no_mangle]` functions and new public methods. They never modify existing upstream code.
- When upstream adds an equivalent function, delete the patch, re-run ffigen, verify tests pass.

---

## Reading the failure modes

| Failure | First check |
|---|---|
| Build hook fails on consumer machine | Binary not found for platform — check `hook/build.dart` `_platformBinaries` map and GitHub Release assets |
| WASM test fails | `worker.js` out of sync with `WasmPdfDocument` API — rebuild WASM ([S4](#s4--rebuild-wasm)) and update `_web.dart` |
| ffigen produces warnings | Darwin system macros leaking into the header — check `ffigen.yaml` excludes and compiler opts |
| Native test passes but web test fails | `WasmPdfDocument` method name mismatch vs native `PdfPlatform` — check `_web.dart` |
| Editor handle error "not found" | Handle was disposed or never opened — check the handle map lifecycle in the worker isolate |
| "Symbol not found" at runtime | The compiled binary is older than the header — push a version bump to main so CI rebuilds ([S5](#s5--rebuild-native-binaries)), or run `dart test` locally to recompile from source |
| `dart run ffigen` exits with errors | The C header has syntax issues — check the latest patch commit |
| Android build fails with linker errors | NDK path wrong or target not installed — check `ANDROID_NDK_HOME` and `rustup target list` |

---

## Provenance

| Item | Value |
|---|---|
| Upstream repo | `https://github.com/nickhimself/pdf_oxide` |
| Upstream base tag | `go/v0.3.47` (2026-05-12) |
| Fork branch | `pdf_manipulator/0.3.47-patches` |
| Patch commits | 3 (header declarations, editor functions, watermark + image optimization + PDF/A validation) |
| Last refresh | 2026-05-14 |
| Build hook | Dual-path: contributor compiles from source (`cargo build`), consumer downloads from GitHub Releases. Version read from `pubspec.yaml`. |
| wasm-bindgen version | `0.2.121` |

Update this table after every upstream bump or patch change.

---

> **Upstream bumps rebase our patch branch. Rust patches are additive-only with tagged removal triggers. Rebuild WASM after every Rust change; native binaries are cross-compiled by CI (`release.yml`) on 3 runners for 13 targets and uploaded to GitHub Releases automatically. ffigen regenerates Dart bindings — never hand-edit. The build hook is dual-path: contributors compile from source via `cargo build`; consumers download pre-built binaries from GitHub Releases (zero Rust required).**
