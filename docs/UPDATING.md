# Updating pdf_manipulator

Maintenance recipes. For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).
For capability status see [`CAPABILITY_ROADMAP.md`](CAPABILITY_ROADMAP.md).

---

## Vendored forks

Two git submodules, each a fork of the upstream repo with a named
patch branch. `main` on each fork stays a clean mirror of upstream.

| Crate | Upstream | Fork | Branch | Base tag | Submodule |
|---|---|---|---|---|---|
| pdf_oxide | [`yfedoseev/pdf_oxide`](https://github.com/yfedoseev/pdf_oxide) | [`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide) | `pdf_manipulator/0.3.55-patches` | `v0.3.55` | `vendor/pdf_oxide/` |
| office_oxide | [`yfedoseev/office_oxide`](https://github.com/yfedoseev/office_oxide) | [`whuppi/office_oxide`](https://github.com/whuppi/office_oxide) | `office_kit/0.1.2-patches` | `v0.1.2` | `vendor/office_oxide/` |

pdf_oxide depends on office_oxide as a path dependency
(`office_oxide = { path = "../office_oxide" }`).
wasm-bindgen version: `0.2.121`.

---

## When to update

| Trigger | Recipe |
|---|---|
| pdf_oxide upstream release | [S1 — Bump upstream](#s1--bump-upstream) |
| Edit a Rust patch | [S2 — Edit patches](#s2--edit-patches) |
| Add a read/stream op | [S3 — Add read op](#s3--add-read-op) |
| Add an editor mutation | [S4 — Add editor mutation](#s4--add-editor-mutation) |
| Rebuild WASM | [S5 — Rebuild WASM](#s5--rebuild-wasm) |
| Release a version | [S6 — Release](#s6--release) |

---

## S1 — Bump upstream

### 1. Discover what's new

```sh
cd vendor/pdf_oxide
git fetch upstream
git log --oneline vOLD..vNEW
git diff vOLD..vNEW -- src/document.rs src/editor/document_editor.rs | grep "^+.*pub fn"
```

### 2. Check conflict risk

```sh
for f in Cargo.toml src/document.rs src/editor/document_editor.rs \
         src/compliance/converter.rs src/converters/office/mod.rs \
         src/writer/pdf_writer.rs src/writer/document_builder.rs; do
  count=$(git diff vOLD..vNEW -- "$f" | wc -l | tr -d ' ')
  [ "$count" -gt "0" ] && echo "RISK: $f ($count lines)" || echo "CLEAN: $f"
done
```

### 3. Rebase

```sh
git rebase vNEW
cargo test --lib --release
```

### 4. Rename branch

```sh
git branch -m pdf_manipulator/OLD-patches pdf_manipulator/NEW-patches
git push origin pdf_manipulator/NEW-patches
git push origin --delete pdf_manipulator/OLD-patches
```

### 5. Rebuild + verify

```sh
cd ../..
./tool/build_wasm.sh
rm -rf .dart_tool/hooks_runner
make check
```

### 6. Commit

```sh
git add vendor/pdf_oxide web_assets/
git commit -m "build: sync upstream vNEW + rebuild WASM"
```

---

## S2 — Edit patches

Our `host/` directory is entirely our code. Upstream patches live in
7 files (see ARCHITECTURE.md §9). Edit either freely:

```sh
cd vendor/pdf_oxide

# Our code (host/):
#   dispatch.rs, bridge_api.rs, positioned_write.rs, sign.rs,
#   image_optimizer.rs, font_optimizer.rs, constants.rs,
#   native/*, wasm/*
#
# Upstream patches:
#   document.rs, editor/document_editor.rs,
#   compliance/converter.rs, converters/office/mod.rs,
#   writer/pdf_writer.rs, writer/document_builder.rs

cargo test
cd ../..
./tool/build_wasm.sh
make check
```

**Mark every upstream change** with `── pdf_manipulator patch ──`
boundaries. Code in `host/` doesn't need markers (the module itself
is the marker).

**Commit AND push the submodule, or CI will fail:**

```sh
cd vendor/pdf_oxide
git add -A && git commit -m "patch: <description>"
git push origin <branch-name>
cd ../..
git add vendor/pdf_oxide
git commit -m "build: update submodule — <description>"
```

---

## S3 — Add read op

| Step | File | Action |
|---|---|---|
| 1 | `host/dispatch.rs` | Typed function + result struct |
| 2 | `host/bridge_api.rs` | Match arm (call dispatch, encode response) |
| 3 | `protocol/op.dart` | Add `EngineOp` value |
| 4 | `protocol/codec.dart` | Request builder + response decoder |
| 5 | `ops/pdf_doc.dart` | Public method |
| 6 | Tests | Core + stress |
| 7 | Build | `cargo test` → `./tool/build_wasm.sh` → `make check` |

wire_sync_test catches parity drift — missing step 2 = test failure.

---

## S4 — Add editor mutation

Edit mutations are sub-dispatched inside `editorMutate`:

| Step | File | Action |
|---|---|---|
| 1 | `host/dispatch.rs` | `edit_*()` function |
| 2 | `host/bridge_api.rs` | Match arm in `handle_editor_mutate` |
| 3 | `protocol/codec.dart` | Param encoder if needed |
| 4 | `shared_bridge.dart` | `_mutate('opName', params)` on editor handle |
| 5 | `ops/pdf_editor.dart` | Public method |
| 6 | Tests | Core + stress |

---

## S5 — Rebuild WASM

```sh
./tool/build_wasm.sh
ls -lh web_assets/pdf_oxide*
git add web_assets/
```

---

## S6 — Release

### Changelog

| File | Purpose |
|---|---|
| `CHANGELOG.md` | Stable releases (pub.dev) |
| `CHANGELOG.pre.md` | Prereleases (dev testing) |

`pubspec.yaml` stays `version: 0.0.0` in git. CI stamps the real
version at publish time.

### Stable

```
1. Add ## X.Y.Z at top of CHANGELOG.md
2. git log v<PREV>..HEAD --oneline --no-decorate → paste in entry
3. Push to dev → PR to prod → merge
4. create-release.yml → tag + GitHub Release
5. release.yml → compile + upload + pub.dev
```

### Prerelease

```
1. Add ## X.Y.Z-dev.N at top of CHANGELOG.pre.md
2. Push to dev
3. create-release.yml → tag + Release
4. release.yml → compile + upload + pub.dev
```

### Hotfix

```
1. Branch from release tag: git checkout -b hotfix/vX.Y.Z vPREV
2. Fix, add changelog entry, push
3. PR to prod → merge → CI tags + publishes
4. Cherry-pick fix to dev
```

### CI workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | PR to prod/dev | `make analyze` + `make test-unit` + `make test-ops-native` |
| `pr-checks.yml` | PR to prod/dev | Conventional commit title + promotion chain |
| `full-test.yml` | `ready-to-test` label | 8 jobs: 4 pkg (macOS/Linux/Windows/web) + 4 integration (macOS/Android/iOS/web) |
| `create-release.yml` | Changelog push or `workflow_dispatch` | Tag + GitHub Release (idempotent) |
| `release.yml` | Tag push or `workflow_dispatch` | `make compile-natives/wasm` + upload + pub.dev (idempotent) |

### Failure recovery

| Failure | Fix |
|---|---|
| `create-release.yml` failed | Rerun via workflow_dispatch |
| Compile failed | Fix code, rerun with tag |
| pub.dev failed | Rerun (rejects duplicates, safe) |
| Tag without Release | Run `create-release.yml` |
| Release without binaries | Rerun `release.yml` |

### Git hooks

| Hook | Enforces |
|---|---|
| `commit-msg` | Conventional Commits format |
| `pre-commit` | Rejects `git add -f` of gitignored files |

### CODEOWNERS

| Path | Owner |
|---|---|
| `CHANGELOG.md`, `CHANGELOG.pre.md` | `@chaudharydeepanshu` |
| `pubspec.yaml` | `@chaudharydeepanshu` |
| `.github/`, `.githooks/` | `@chaudharydeepanshu` |

---

## Troubleshooting

| Symptom | Check |
|---|---|
| Build hook fails on consumer machine | Binary missing for platform — check GitHub Release assets |
| WASM test fails | Rebuild: `./tool/build_wasm.sh` |
| Native passes, web fails | bridge_api.rs WASM path missing the op |
| wire_sync_test fails | EngineOp without matching bridge_api.rs arm |
| "Handle not found" | Handle disposed or never opened |
| "Symbol not found" at runtime | Binary older than source — rebuild |
