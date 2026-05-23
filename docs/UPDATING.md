# Updating pdf_manipulator

Maintenance procedures for the package. For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).

pdf_manipulator wraps a vendored fork of [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) (Rust engine) via FFI (native) and WASM (web). The fork lives at [`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide) with patches on the [`pdf_manipulator/0.3.53-patches`](https://github.com/whuppi/pdf_oxide/tree/pdf_manipulator/0.3.53-patches) branch. The git submodule at `vendor/pdf_oxide/` points to this branch.

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
| Releasing a new version | [S5 — Release pipeline](#s5--release-pipeline) |
| Adding a new platform | [S6 — Add platform](#s6--add-platform) |
| Consumer reports a missing function | [S7 — Diagnose missing function](#s7--diagnose-missing-function) |
| Using a new Dart/Flutter feature | [S8 — Update SDK constraint](#s8--update-sdk-constraint) |

---

## S1 — Bump upstream

When pdf_oxide tags a new release:

### Step 1 — Discover what's new

```sh
cd vendor/pdf_oxide
git fetch upstream

# What tags exist?
git tag --sort=-version:refname | head -5

# What changed since our base? (replace OLD with current base tag)
git log --oneline vOLD..vNEW          # commit messages
git diff --stat vOLD..vNEW | tail -5  # file summary

# New WASM functions (what web can call):
git diff vOLD..vNEW -- src/wasm.rs | grep "js_name" | head -20

# New bridge-relevant Rust API:
git diff vOLD..vNEW -- src/document.rs src/editor/document_editor.rs | grep "^+.*pub fn"
```

### Step 2 — Check which of our patched files upstream also changed

```sh
# Our patches touch these files — check each for upstream changes:
for f in Cargo.toml src/bridge/ffi_api.rs src/editor/document_editor.rs \
         src/editor/mod.rs src/ffi.rs src/signatures/sign_bytes.rs src/wasm.rs; do
  count=$(git diff vOLD..vNEW -- "$f" | wc -l | tr -d ' ')
  if [ "$count" -gt "0" ]; then
    echo "CONFLICT RISK: $f ($count diff lines)"
  else
    echo "CLEAN: $f"
  fi
done

# If upstream added a function that does the same thing as one of our patches,
# the upstream version wins. Delete our patch during conflict resolution.
```

### Step 3 — Rebase patches onto new tag

```sh
git log --oneline upstream/main..HEAD   # see our patches

git rebase vNEW
# (rebases all our patches onto the new tag)

# Conflict resolution rules:
# - C header (include/pdf_oxide_c/pdf_oxide.h): always take upstream.
#   Our additions are usually absorbed by upstream in later releases.
# - Cargo.toml: keep our feature additions (e.g. "rendering" in wasm),
#   drop any that are now always-on deps in upstream.
# - src/document.rs: keep BOTH (upstream new methods + our local patches).
#   Watch for missing closing braces after merge.
# - src/signatures/sign_bytes.rs: if upstream refactored the time code,
#   apply our WASM cfg-gate to the new location.
# - src/wasm.rs: keep BOTH (upstream new features + our standalone sign fns).
#   Watch for missing closing braces after merge.
# - src/editor/document_editor.rs: upstream may add new fields to
#   DocumentEditor struct — add them to our from_reader() with defaults.

# After all conflicts resolved:
cargo build --lib --release   # verify native compiles
cargo test --lib --release    # verify Rust tests pass
```

### Step 4 — Rename branch

The branch name encodes the upstream version we're based on. Rename after every sync:

```sh
# Rename local branch
git branch -m pdf_manipulator/OLD-patches pdf_manipulator/NEW-patches

# Push new branch, delete old
git push origin pdf_manipulator/NEW-patches
git push origin --delete pdf_manipulator/OLD-patches
```

### Step 5 — Rebuild WASM and verify Dart

```sh
# Rebuild WASM binary
bash tool/build_wasm.sh

# Back to parent repo
cd ../..

# Update submodule pointer
git add vendor/pdf_oxide web_assets/

# Clear build cache (stale native lib from old upstream)
rm -rf .dart_tool/hooks_runner

# Verify everything
dart analyze lib/ test/
dart test test/types/ test/protocol/ test/ops/native_runner_test.dart
dart test test/ops/web_runner_test.dart -p chrome
```

### Step 6 — Commit and finalize

```sh
# Commit parent with updated submodule + WASM
git commit -m "build: sync upstream vNEW + rebuild WASM"

# Update this file:
# - Branch name references (intro paragraph + provenance table)
# - Upstream base tag in provenance table
# - S1 examples (vOLD..vNEW references)
# Update CAPABILITY_ROADMAP.md with any new capabilities
```

After Rust changes, rebuild WASM ([S4](#s4--rebuild-wasm)). Version bumps, changelog, and native binary releases are handled automatically by release-please and the tag-triggered release pipeline ([S5](#s5--release-pipeline)).

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

After editing patches, always run [S4](#s4--rebuild-wasm). Native binaries are compiled and released automatically by the tag-triggered pipeline ([S5](#s5--release-pipeline)).

**Critical: commit AND push the submodule, or CI will fail.** Local edits to `vendor/pdf_oxide/` work locally (cargo compiles from the dirty working tree) but CI checks out the committed submodule pointer. If the patches aren't committed and pushed to the fork, CI gets the old code and fails with symbol-not-found errors. The three-step sequence:

```sh
# 1. Commit inside the submodule
cd vendor/pdf_oxide
git add -A
git commit -m "patch: <description>"

# 2. Push to the fork remote
git push origin pdf_manipulator/0.3.47-patches

# 3. Back to parent — update submodule pointer and open PR
cd ../..
git add vendor/pdf_oxide
git commit -m "build: update submodule to include <description>"
git push origin <your-feature-branch>
# Then open a PR to dev
```

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
10. **Rebuild** — [S4](#s4--rebuild-wasm) if Rust changed; native binaries are released automatically by the tag-triggered pipeline ([S5](#s5--release-pipeline))

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

## S5 — Release pipeline

Versioning, changelog, tagging, compilation, and publishing are fully automated via release-please + tag-triggered CI. No manual version bumps, no manual changelog editing, no manual tagging.

### How it works — two release channels

**Prerelease (dev branch):**

1. Work on feature branches with conventional commit titles (`feat:`, `fix:`, etc.)
2. Open PRs to `dev` — CI runs analyze + macOS test + PR title validation
3. Squash-merge — PR title becomes the commit on dev
4. release-please reads the commits, opens a prerelease Release PR (bumps to `1.1.0-dev.0`, writes CHANGELOG)
5. Merge the Release PR → tag `v1.1.0-dev.0` → full pipeline → prerelease published

**Stable release (prod branch):**

1. Open PR from `dev` → `prod` with a title summarizing what dev accumulated (e.g. `feat: watermark + compression + stamps`)
2. Squash-merge — that title becomes the commit on prod
3. release-please reads it, opens a stable Release PR (bumps to `1.1.0`, writes CHANGELOG)
4. You can edit the Release PR's CHANGELOG before merging to polish the wording
5. Merge the Release PR → tag `v1.1.0` → full pipeline → stable release published

**Why this works cleanly:** each branch tracks its own commit history independently. Squash-merge works on both paths — the PR title is always the changelog entry. No commits leak between branches.

### CI/CD workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | PR to prod/dev | Analyze + macOS test (fast, automatic) |
| `pr-checks.yml` | PR to prod/dev | Conventional commit title + promotion chain + workflow security lint |
| `full-test.yml` | Maintainer adds `ready-to-test` label | 6-platform test (macOS, Linux, Windows, Android, iOS, Web). Required before merge. |
| `release-please.yml` | Push to prod/dev | Opens/updates Release PRs with version bump + CHANGELOG |
| `release.yml` | Tag push (`v*`) | Compile all targets → GitHub Release → pub.dev publish. No re-test (same commit already passed full-test). |

### Cross-compilation runners

| Runner | Targets |
|---|---|
| macOS (macos-14) | macOS arm64/x64, iOS arm64/sim-arm64/sim-x64 |
| Linux (ubuntu-latest) | Linux x64/arm64, Android arm64/arm/x64/x86, WASM |
| Windows (windows-latest) | Windows x64 |

### Branch protection

Both `prod` and `dev` are protected: no direct push (even admins), PRs required, 3 status checks must pass (Analyze, Test, Conventional Commit), squash-merge only, force push blocked.

### Security

- Default workflow token is read-only; workflows opt-in to write permissions
- Fork PRs get read-only tokens (GitHub enforced)
- pub.dev publish uses OIDC (no long-lived secrets)
- Only tag pushes trigger the release pipeline; only release-please creates tags

### For local development

Contributors compile from source automatically when running `dart test`. The build hook (`hook/build.dart`) detects `vendor/pdf_oxide/Cargo.toml` and runs `cargo build` — no manual compilation step needed. This requires a Rust toolchain (`rustup.rs`).

---

## S6 — Add platform

1. Add the Rust target: `rustup target add <triple>`
2. Add the cross-compilation step to `release.yml` (under the appropriate runner job)
3. Add the platform mapping in `hook/build.dart` (`_platformBinaries` map)
4. Test locally: `dart test` on the new platform (build hook compiles from source)
5. Merge to dev — release-please handles version bump, CI compiles and uploads the new binary on release
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

## S8 — Update SDK constraint

The `environment.sdk` in `pubspec.yaml` must match the minimum Dart SDK the package actually requires. When to update:

| You added | Minimum SDK | Why |
|---|---|---|
| Build hooks (`hooks: build: true`) | `>=3.10.0` | Build hooks shipped in Dart 3.10 |
| `@Native` FFI annotations | `>=3.4.0` | `@Native` stabilized in Dart 3.4 |
| New Dart language features | Check [Dart changelog](https://dart.dev/get-dart/archive) | Each feature has a minimum SDK |

**How to check what the current minimum should be:**

```sh
# 1. Look at pubspec.yaml — what features does the package use?
#    hooks: build: true → needs 3.10+
#    @Native annotations → needs 3.4+
#    The highest minimum wins.

# 2. Verify the constraint
grep "sdk:" pubspec.yaml
#    Should show >= the highest requirement

# 3. If unsure, try building with an older SDK version
#    The Dart team publishes minimum SDK requirements per feature
#    in the changelog at https://dart.dev/get-dart/archive
```

**Current constraint:** `>=3.10.0 <4.0.0` (required by `hooks: build: true`).

When bumping the constraint, note the new minimum SDK in your commit message — release-please will include it in the changelog automatically.

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

- Every patch carries a `LOCAL PATCH` comment with a one-line description and a removal trigger.
- Patches are purely additive — they add new functions and new public methods. They never modify existing upstream code.
- When upstream adds an equivalent function, delete our patch during the rebase conflict resolution. The upstream version wins.

---

## Reading the failure modes

| Failure | First check |
|---|---|
| Build hook fails on consumer machine | Binary not found for platform — check `hook/build.dart` `_platformBinaries` map and GitHub Release assets |
| WASM test fails | `worker.js` out of sync with `WasmPdfDocument` API — rebuild WASM ([S4](#s4--rebuild-wasm)) and update `_web.dart` |
| ffigen produces warnings | Darwin system macros leaking into the header — check `ffigen.yaml` excludes and compiler opts |
| Native test passes but web test fails | `WasmPdfDocument` method name mismatch vs native `PdfPlatform` — check `_web.dart` |
| Editor handle error "not found" | Handle was disposed or never opened — check the handle map lifecycle in the worker isolate |
| "Symbol not found" at runtime | The compiled binary is older than the header — push a version bump to prod so CI rebuilds ([S5](#s5--rebuild-native-binaries)), or run `dart test` locally to recompile from source |
| `dart run ffigen` exits with errors | The C header has syntax issues — check the latest patch commit |
| Android build fails with linker errors | NDK path wrong or target not installed — check `ANDROID_NDK_HOME` and `rustup target list` |

---

## Provenance

| Item | Value |
|---|---|
| Upstream repo | [`yfedoseev/pdf_oxide`](https://github.com/yfedoseev/pdf_oxide) |
| Fork repo | [`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide) |
| Fork branch | [`pdf_manipulator/0.3.53-patches`](https://github.com/whuppi/pdf_oxide/tree/pdf_manipulator/0.3.53-patches) |
| Upstream base tag | `v0.3.53` |
| wasm-bindgen version | `0.2.121` |

Fork convention: `main` on the fork stays a clean mirror of upstream. Patches live on the named branch (`pdf_manipulator/X.Y.Z-patches`). The branch name encodes the upstream version — rename it on every sync (S1 Step 4). When upstream releases a new version, rebase the patch branch onto the new tag (see [S1](#s1--bump-upstream)).

Update this table after every upstream bump.

---

> **Upstream bumps rebase our patch branch. Rust patches are additive-only with tagged removal triggers. Rebuild WASM after every Rust change. Versioning, changelog, tagging, native compilation, and pub.dev publishing are fully automated: release-please opens Release PRs from conventional commits; merging one creates a tag; the tag triggers 6-platform test → compile → GitHub Release → pub publish. ffigen regenerates Dart bindings — never hand-edit. The build hook is dual-path: contributors compile from source via `cargo build`; consumers download pre-built binaries from GitHub Releases (zero Rust required).**
