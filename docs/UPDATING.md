# Updating pdf_manipulator

Maintenance procedures for the package. For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).

pdf_manipulator wraps two vendored forks via FFI (native) and WASM (web):
- [pdf_oxide](https://github.com/yfedoseev/pdf_oxide) (PDF engine) — fork at [`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide), submodule at `vendor/pdf_oxide/`
- [office_oxide](https://github.com/yfedoseev/office_oxide) (DOCX/PPTX/XLSX conversion) — fork at [`whuppi/office_oxide`](https://github.com/whuppi/office_oxide), submodule at `vendor/office_oxide/`

| Source | Why we track it |
|---|---|
| pdf_oxide upstream tags | The Rust engine — page manipulation, text extraction, rendering, signatures |
| `host/dispatch.rs` (our code) | Shared dispatch logic — both platforms call the same typed functions |
| `host/native/` (our code) | FFI entry points (`ffi_api.rs`), binary encoders (`ffi_encode.rs`), I/O transport |
| `host/web/` (our code) | WASM entry points (`wasm_api.rs`), JsValue encoders (`wasm_encode.rs`) |
| `src/wasm.rs` (author + our additions) | WASM type wrappers + our `with_doc()` helper + encryption bindings |
| Rust toolchain | Cross-compilation for 13 native targets + WASM |
| WASM binary (`web_assets/`) | Compiled from the patched Rust, committed in git, shipped to web consumers |
| Pre-built native binaries (GitHub Releases) | Per-platform `.dylib` / `.a` / `.so` / `.dll` downloaded by the build hook |

---

## When to update

| Trigger | Procedure |
|---|---|
| pdf_oxide releases a new version | [S1 — Bump upstream](#s1--bump-upstream) |
| A Rust patch needs updating or adding | [S2 — Edit Rust patches](#s2--edit-rust-patches) |
| Adding a new read/stream operation | [S3 — Add read operation](#s3--add-read-operation) |
| Adding a new editor mutation | [S4 — Add editor mutation](#s4--add-editor-mutation) |
| Rebuilding WASM | [S5 — Rebuild WASM](#s5--rebuild-wasm) |
| Releasing a new version | [S6 — Release pipeline](#s6--release-pipeline) |

---

## S1 — Bump upstream

When pdf_oxide tags a new release:

### Step 1 — Discover what's new

```sh
cd vendor/pdf_oxide
git fetch upstream

git tag --sort=-version:refname | head -5
git log --oneline vOLD..vNEW
git diff --stat vOLD..vNEW | tail -5

# New WASM methods:
git diff vOLD..vNEW -- src/wasm.rs | grep "js_name" | head -20

# New engine API:
git diff vOLD..vNEW -- src/document.rs src/editor/document_editor.rs | grep "^+.*pub fn"
```

### Step 2 — Check conflict risk in our patched files

```sh
for f in Cargo.toml src/host/dispatch.rs src/host/native/ffi_api.rs \
         src/host/native/ffi_encode.rs src/host/web/wasm_api.rs \
         src/host/web/wasm_encode.rs src/wasm.rs \
         src/editor/document_editor.rs; do
  count=$(git diff vOLD..vNEW -- "$f" | wc -l | tr -d ' ')
  if [ "$count" -gt "0" ]; then
    echo "CONFLICT RISK: $f ($count diff lines)"
  else
    echo "CLEAN: $f"
  fi
done
```

### Step 3 — Rebase patches onto new tag

```sh
git rebase vNEW

# Conflict resolution rules:
# - src/wasm.rs: keep upstream new methods + our with_doc() helper +
#   isEncrypted/requiresPassword/getEncryptionAlgorithm/getPermissionBits.
#   The dispatch_* methods are in host/web/wasm_api.rs (separate file).
# - src/editor/document_editor.rs: upstream may add new struct fields —
#   add them to our from_reader() with defaults.
# - host/ files (dispatch, ffi_api, ffi_encode, wasm_api, wasm_encode):
#   entirely our code, no upstream conflict expected.

cargo test --lib --release   # verify Rust tests pass
```

### Step 4 — Rename branch + rebuild

```sh
git branch -m pdf_manipulator/OLD-patches pdf_manipulator/NEW-patches
git push origin pdf_manipulator/NEW-patches
git push origin --delete pdf_manipulator/OLD-patches
```

### Step 5 — Rebuild and verify Dart

```sh
cd ../..   # back to pdf_manipulator root

make wasm                    # rebuild WASM
git add vendor/pdf_oxide web_assets/

rm -rf .dart_tool/hooks_runner   # clear stale native cache

make check                   # native tests
make check-all               # native + web tests
```

### Step 6 — Commit

```sh
git commit -m "build: sync upstream vNEW + rebuild WASM"
# Update this file's Provenance table with new branch name + base tag
```

---

## S2 — Edit Rust patches

Our `host/` directory is entirely our code — not upstream patches. Edit freely:

```sh
cd vendor/pdf_oxide

# Edit the files you need:
# - host/dispatch.rs          — shared operation logic
# - host/native/ffi_api.rs    — C extern entry points
# - host/native/ffi_encode.rs — result → binary encoders
# - host/web/wasm_api.rs      — #[wasm_bindgen] entry points
# - host/web/wasm_encode.rs   — result → JsValue encoders
# - src/wasm.rs                — WASM type wrappers (careful — mix of author + ours)
# - src/editor/document_editor.rs — engine-level additions

cargo test   # verify Rust compiles + passes

cd ../..
make wasm    # rebuild WASM if any Rust changed
make check   # native Dart tests
```

**Critical: commit AND push the submodule, or CI will fail.**

```sh
# 1. Commit inside the submodule
cd vendor/pdf_oxide
git add -A && git commit -m "patch: <description>"

# 2. Push to the fork remote
git push origin <branch-name>

# 3. Update parent submodule pointer
cd ../..
git add vendor/pdf_oxide
git commit -m "build: update submodule — <description>"
```

---

## S3 — Add read operation

End-to-end checklist for a new read/stream operation (e.g. a new `extractFoo`):

| Step | File | What to do |
|---|---|---|
| 1 | `host/dispatch.rs` | Add typed function + result struct |
| 2 | `host/native/ffi_encode.rs` | Add `encode_foo()` (dispatch result → binary) |
| 3 | `host/web/wasm_encode.rs` | Add `foo_to_js()` (dispatch result → JsValue) |
| 4 | `host/native/ffi_api.rs` | Add case in `dispatch_read_op` (call dispatch + encode) |
| 5 | `host/web/wasm_api.rs` | Add `#[wasm_bindgen]` method (call wasm_encode) |
| 6 | `src/wasm.rs` | Nothing if `wasm_api.rs` handles it via `with_doc()` |
| 7 | `protocol/op.dart` | Add `EngineOp` value |
| 8 | `protocol/codec.dart` | Add request builder (`fooOp()`) + response decoder (`decodeFoo()`) |
| 9 | `native/coordinator.dart` | Add case in dispatch switch |
| 10 | `web_assets/worker.js` | Add case in dispatch switch |
| 11 | `native/wire.dart` | Add `wireDecodeFoo()` (binary → typed, calls codec) |
| 12 | `web/wire.dart` | Add `wireDecodeFoo()` (Map → typed, calls codec) |
| 13 | `native/bridge.dart` | Add method, call `wireDecodeFoo(result)` |
| 14 | `web/bridge.dart` | Add method, call `wire.wireDecodeFoo(r)` |
| 15 | `transport/pdf_bridge.dart` | Add abstract method |
| 16 | `ops/pdf.dart` | Add public method |
| 17 | Tests | Native + web |
| 18 | Build | `cargo test` → `make wasm` → `make check-all` |

The wire_sync_test catches parity drift — if you miss step 9 or 10, the test fails.

---

## S4 — Add editor mutation

Edit ops also go through `dispatch.rs`. Same pattern as read ops but no encode step (edits return `Result<()>`, not structured data):

| Step | File | What to do |
|---|---|---|
| 1 | `host/dispatch.rs` | Add `edit_*()` typed function |
| 2 | `host/native/ffi_api.rs` | Add case in `dispatch_edit_op` (binary params → dispatch call) |
| 3 | `host/web/wasm_api.rs` | Add `#[wasm_bindgen]` method (call dispatch) |
| 4 | `web_assets/worker.js` | Add case in `applyEditOp` (one-liner, calls `doc.dispatchEditXxx()`) |
| 5 | `protocol/codec.dart` | Add param encoder if needed |
| 6 | `native/coordinator.dart` | Usually goes through existing `editorMutate` — no change |
| 7 | `native/bridge.dart` | Add `_mutate(opCode, params)` call |
| 8 | `web/bridge.dart` | Add matching method |
| 9 | `ops/pdf_editor.dart` | Add public method |
| 10 | Tests | Native + web |

---

## S5 — Rebuild WASM

After any Rust-side change:

```sh
make wasm

# Verify output
ls -lh web_assets/pdf_oxide*

# Commit
git add web_assets/
```

---

## S6 — Release pipeline

Version and changelog are manually written by the maintainer. CI handles tagging and publishing.

### Two changelog files

| File | Purpose | CI trigger |
|---|---|---|
| `CHANGELOG.md` | Stable releases — what pub.dev shows | `auto-tag.yml` on prod push |
| `CHANGELOG.pre.md` | Prereleases — testing builds | `publish-prerelease.yml` on dev push |

`pubspec.yaml` stays `version: 0.0.0` in git. CI stamps the real version from the changelog at publish time.

### Stable release

```
1. Add ## X.Y.Z entry at the top of CHANGELOG.md
2. Run: dart run tool/commits.dart v<PREVIOUS_VERSION>
   Copy the <details> block into your entry
3. Commit + push to dev
4. PR dev → prod, merge
5. auto-tag.yml reads CHANGELOG.md → tags vX.Y.Z
6. release.yml fires → stamps version → compiles → GitHub Release → pub.dev
```

### Prerelease

```
1. Add ## X.Y.Z-dev.N entry at the top of CHANGELOG.pre.md
2. Run: dart run tool/commits.dart v<PREVIOUS_TAG>
   Copy the <details> block into your entry
3. Commit + push to dev
4. publish-prerelease.yml reads CHANGELOG.pre.md → tags vX.Y.Z-dev.N
5. release.yml fires → stamps version → uses CHANGELOG.pre.md as the published changelog
```

### Hotfix

```
1. git checkout -b hotfix/vX.Y.Z vPREVIOUS_TAG   (branch from release tag)
2. Fix the bug
3. Add ## X.Y.Z entry at the top of CHANGELOG.md
4. Run: dart run tool/commits.dart vPREVIOUS_TAG
5. Commit + push the hotfix branch
6. PR hotfix/vX.Y.Z → prod, merge
7. auto-tag.yml tags → release.yml publishes
8. Cherry-pick the fix code to dev if needed (not the changelog commit)
```

### CI workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | PR to prod/dev | Analyze + macOS test |
| `pr-checks.yml` | PR to prod/dev | Conventional commit title + promotion chain + workflow security |
| `full-test.yml` | `ready-to-test` label | 6-platform test |
| `create-release.yml` | Push to dev/prod (changelog changed) OR `workflow_dispatch` | Creates GitHub Release + tag for new versions. Idempotent — skips existing. |
| `release.yml` | Tag push (`v*`) OR `workflow_dispatch` | Stamps version → compiles all targets → uploads to GitHub Release → pub.dev. Idempotent — safe to rerun. |

### Failure recovery

| Failure | Fix |
|---|---|
| `create-release.yml` failed | Rerun via Actions → Create Release → Run workflow. Idempotent. |
| `release.yml` compile failed | Fix the code, then rerun via Actions → Release → Run workflow → enter tag. |
| `release.yml` pub.dev publish failed | Rerun via workflow_dispatch. pub.dev rejects duplicates (safe). |
| Tag exists but no GitHub Release | Run `create-release.yml` via workflow_dispatch — it checks for the Release, not the tag. |
| GitHub Release exists but no binaries | Rerun `release.yml` via workflow_dispatch — uploads overwrite existing assets. |

### Git hooks

| Hook | What it enforces |
|---|---|
| `commit-msg` | Conventional Commits format. Blocks non-release merge commits. |
| `pre-commit` | Rejects `git add -f` of gitignored files. |

### CODEOWNERS protection

| Path | Owner | Effect |
|---|---|---|
| `CHANGELOG.md`, `CHANGELOG.pre.md` | `@chaudharydeepanshu` | Contributors can't edit changelogs without maintainer approval |
| `pubspec.yaml` | `@chaudharydeepanshu` | Contributors can't change version |
| `.github/`, `.githooks/` | `@chaudharydeepanshu` | CI/hook changes need maintainer approval |

### Helper tool

```bash
dart run tool/commits.dart v1.0.0       # commits since tag v1.0.0
dart run tool/commits.dart v1.1.0-dev.0 # commits since that prerelease
dart run tool/commits.dart abc1234      # commits since any git ref
```

Prints a `<details>` block to stdout. Copy into your changelog entry.

---

## Reading the failure modes

| Failure | First check |
|---|---|
| Build hook fails on consumer machine | Binary not found for platform — check `hook/build.dart` and GitHub Release assets |
| WASM test fails | `worker.js` dispatch out of sync — rebuild WASM (`make wasm`) |
| Native test passes but web fails | Dispatch parity issue — check `dispatch.rs` page routing + `wasm_encode.rs` result shape |
| Wire sync test fails | EngineOp enum has an op not in `coordinator.dart` or `worker.js` — add the missing case |
| Editor handle error "not found" | Handle disposed or never opened — check coordinator.dart lifecycle |
| "Symbol not found" at runtime | Compiled binary older than source — `dart test` recompiles from source |
| Web returns wrong data (e.g. 260 chars instead of full text) | Dispatch page routing — check `dispatch.rs` handles `None` vs `Some(page)` correctly |
| Web search crash (JSNull) | Result shape mismatch — ensure `wasm_encode.rs` flattens bbox to flat x,y,w,h |

---

## Provenance

### pdf_oxide (PDF engine)

| Item | Value |
|---|---|
| Upstream repo | [`yfedoseev/pdf_oxide`](https://github.com/yfedoseev/pdf_oxide) |
| Fork repo | [`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide) |
| Fork branch | `pdf_manipulator/0.3.55-patches` |
| Upstream base tag | `v0.3.55` |
| Submodule path | `vendor/pdf_oxide/` |
| wasm-bindgen version | `0.2.121` |

### office_oxide (DOCX/PPTX/XLSX conversion)

| Item | Value |
|---|---|
| Upstream repo | [`yfedoseev/office_oxide`](https://github.com/yfedoseev/office_oxide) |
| Fork repo | [`whuppi/office_oxide`](https://github.com/whuppi/office_oxide) |
| Fork branch | `pdf_manipulator/0.1.2-patches` |
| Upstream base tag | `v0.1.2` |
| Submodule path | `vendor/office_oxide/` |
| Patch | Streaming OPC writer — `Write`-only (no `Seek`), enables streaming DOCX/PPTX/XLSX output |

Fork convention: `main` on each fork stays a clean mirror of upstream. Patches live on the named branch. When upstream releases, rebase onto the new tag and rename the branch ([S1](#s1--bump-upstream)). Same procedure applies to both submodules.

---

## Our additions to the engine

### `host/` directory — entirely ours

| File | What it does |
|---|---|
| `host/dispatch.rs` | Shared typed dispatch functions — both native and web call these |
| `host/constants.rs` | Buffer sizes |
| `host/native/ffi_api.rs` | C extern entry points — deserialize binary, call dispatch, encode result |
| `host/native/ffi_encode.rs` | Dispatch result → binary bytes (Dart wire.dart decodes) |
| `host/native/arena.rs` | Per-op bumpalo arena |
| `host/native/callback_reader.rs` | Read+Seek via condvar |
| `host/native/callback_writer.rs` | Write via condvar |
| `host/native/shared_buffer.rs` | Shared memory layout |
| `host/native/thread_pool.rs` | Fixed-size thread pool + cancel |
| `host/web/wasm_api.rs` | #[wasm_bindgen] dispatch entry points |
| `host/web/wasm_encode.rs` | Dispatch result → JsValue (matches Dart codec shape) |

### Additions to author files

| File | What we added |
|---|---|
| `src/wasm.rs` | `with_doc()` helper for mutex locking, `isEncrypted`, `requiresPassword`, `getEncryptionAlgorithm`, `getPermissionBits`, `isModified(&mut self)`, streaming sign functions, buffer capacity constants |
| `src/editor/document_editor.rs` | `scan_content_stream_names`, `prune_page_resources` (wired into GC save), resource pruning during full rewrite, incremental save support via `save_mode` parameter, `add_content_stream_watermark` + `under_content` HashMap for background watermarks prepended before page Contents |

### Patch discipline

- `host/` is entirely ours — no upstream conflict risk.
- Additions to `src/wasm.rs` are clearly separated (helper methods, encryption bindings, streaming sign).
- Engine-level patches are additive — never modify existing upstream code.
- When upstream absorbs a feature we patched, delete our patch during rebase.

---

> **Two submodules (pdf_oxide + office_oxide), same fork convention. Upstream bumps rebase our patch branch. `host/` is entirely ours (dispatch + native + web). All ops (read + edit) go through dispatch.rs. Rebuild WASM after every Rust change. The wire_sync_test catches parity drift. Release pipeline is fully automated.**
