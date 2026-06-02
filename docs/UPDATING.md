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
| Before committing any Rust patch | [S7 — Verify our-code warnings](#s7--verify-our-code-warnings) |

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
make build-wasm
make clean
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

cargo test --lib
cd ../..
make build-wasm
make check
```

**Mark every upstream change** with `── pdf_manipulator patch ──`
boundaries. Code in `host/` doesn't need markers (the module itself
is the marker).

**Verify zero warnings:** `make analyze` checks Rust warnings in our
patched lines automatically (see [S7](#s7--verify-our-code-warnings)).

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
| 7 | Build | `cargo test --lib` → `make build-wasm` → `make check` |

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
make build-wasm
ls -lh web_assets/pdf_oxide*
git add web_assets/
```

---

## S6 — Release

### The one rule

**NEVER push directly to `dev` or `prod`. NEVER bypass PR merge
requirements.** Admin bypass exists as a GitHub safety valve — not a
shortcut. Every change goes through a PR with CI checks. No exceptions,
no "just this one quick fix," no cherry-picks to protected branches.

**Every change needs a PR — no exceptions.** If a PR was already merged
and you have more changes for the same topic, create a NEW PR. Don't
assume the old PR's branch will carry new commits — squash-merge
discards the branch. Don't assume local commits are on the remote —
check `git log origin/dev` before claiming something is shipped. The
rule: if `git diff origin/dev -- <file>` shows changes, those changes
need a PR.

### Changelog

| File | Purpose |
|---|---|
| `CHANGELOG.md` | Stable releases (pub.dev) |
| `CHANGELOG.pre.md` | Prereleases (dev testing) |

`pubspec.yaml` stays `version: 0.0.0` in git. CI stamps the real
version at publish time from the tag.

### How the release pipeline works

The release is a two-workflow chain. Both steps are automatic once
you push the changelog — no manual workflow runs needed.

```
Changelog push to dev or prod
  → create-release.yml (auto-triggers on CHANGELOG.md / CHANGELOG.pre.md change)
    → scans changelog for versions without a GitHub Release
    → creates tag + GitHub Release for each new version
    → tag push triggers publish.yml automatically
      → 6 compile jobs in parallel (macOS/iOS/Android/Linux/Windows/WASM)
      → upload binaries to GitHub Release
      → stamp version from tag into pubspec.yaml + version.dart
      → dart pub publish to pub.dev
```

Both workflows are idempotent. Rerun either at any time — existing
releases are skipped, pub.dev rejects duplicate versions.

### Stable release

```
1. Add ## X.Y.Z at top of CHANGELOG.md
2. git log v<PREV>..HEAD --oneline --no-decorate → paste in entry
3. PR to prod → merge
4. (automatic) create-release.yml → tag + GitHub Release
5. (automatic) publish.yml → compile + upload + pub.dev
```

### Prerelease

```
1. Add ## X.Y.Z-dev.N at top of CHANGELOG.pre.md
2. PR to dev → merge (or push to dev via PR)
3. (automatic) create-release.yml → tag + GitHub Release
4. (automatic) publish.yml → compile + upload + pub.dev
```

### Hotfix

```
1. Branch from release tag: git checkout -b hotfix/vX.Y.Z vPREV
2. Fix, add changelog entry, push
3. PR to prod → merge → CI tags + publishes (automatic)
4. Cherry-pick fix to dev (via PR)
```

### CI workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | PR to prod/dev | `make analyze` + `make test-unit` + `make test-ops-native` |
| `pr-lint.yml` | PR to prod/dev | Conventional commit title + promotion chain |
| `full-test.yml` | `ready-to-test` label | 10 jobs: 4 pkg + 6 integration. Web jobs have separate named steps per mode (JSPI/Atomics/OPFS). |
| `create-release.yml` | Push to dev/prod that changes `CHANGELOG.md` or `CHANGELOG.pre.md`, or `workflow_dispatch` | Scan changelog → tag + GitHub Release (idempotent) |
| `publish.yml` | Tag push (`v*`), or `workflow_dispatch` with tag name | Compile all 6 targets + upload to Release + publish to pub.dev (idempotent) |
| `flutter-upgrade.yml` | Daily schedule or `workflow_dispatch` | Check for new Flutter stable, open/update upgrade PR on `chore/flutter-upgrade` branch |
| `triage.yml` | Issues/PRs opened | Auto-label and assign |

### Failure recovery

| Failure | Fix |
|---|---|
| `create-release.yml` failed | Rerun via Actions → Create Release → Run workflow |
| pub.dev publish failed (but binaries OK) | Rerun `publish.yml` with the tag (pub.dev rejects duplicates, safe to retry) |
| Tag exists but no Release | Run `create-release.yml` via workflow_dispatch |
| Release exists but no binaries | Rerun `publish.yml` via workflow_dispatch with tag |
| `publish.yml` shows "workflow file issue" | Check for `inputs.tag` in top-level expressions — use `github.event.inputs.tag` instead |

### Compile failed — rebuild a release with fixed code

`publish.yml` checks out the **tag's code** for compilation. The compile
actions (`uses: ./.github/actions/compile-*`) also resolve from the
tag's checkout. Rerunning `publish.yml` with the same tag reuses the
same broken code — the fix on `dev` is invisible to the tag.

**Two options:**

**Option A — Bump version (new prerelease).** Code changed, so the
version changes. Clean and correct.

```
1. Fix the compile issue, merge to dev via PR
2. Add ## X.Y.Z-dev.N+1 at top of CHANGELOG.pre.md
3. Merge to dev
4. (automatic) create-release.yml → new tag → publish.yml → compile
```

**Option B — Rebuild same version (delete + recreate tag).** Use when
the code fix doesn't change the package's behavior (CI/workflow fixes,
compile script fixes). Same version, fresh tag on the right commit.

```
1. Fix the compile issue, merge to dev via PR
2. Delete the broken release + tag:
     gh release delete vX.Y.Z --repo whuppi/pdf_manipulator --yes
     gh api repos/whuppi/pdf_manipulator/git/refs/tags/vX.Y.Z -X DELETE
3. Retrigger:
     gh workflow run create-release.yml --repo whuppi/pdf_manipulator -f branch=dev
4. create-release.yml sees the version has no release → creates fresh
   tag at dev HEAD → publish.yml fires → compiles with the fix
```

**Pick A when:** the fix changes runtime behavior, adds features, or
consumers should know the binary differs.

**Pick B when:** the fix is purely CI/build infrastructure. The package
source is identical — only the build tooling changed. No reason to
burn a version number.

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

## S7 — Verify our-code warnings

`make analyze` includes a Rust warning check for both vendored crates:

1. Runs `cargo check` with all features (same set as CI release builds)
2. Uses `--message-format=json` to get warnings even from cached builds
3. Derives the upstream base tag from the branch name automatically
   (`pdf_manipulator/0.3.55-patches` → `v0.3.55`). No hardcoded tag —
   renaming the branch in S1 step 4 is all that's needed.
4. Diffs against the base tag to find lines we changed
5. Fails if any warning falls inside our changed lines

Checks both `vendor/pdf_oxide` and `vendor/office_oxide`.

```sh
make analyze
```

If the Rust check fails, it prints the exact file:line and warning.
Fix them before committing.

The feature set (`RUST_FEATURES` in the Makefile) must match
`compile_rust.sh`'s feature definitions — if they diverge, CI catches
warnings that `make analyze` misses.

### Common warning types and fixes

| Warning | Cause | Fix |
|---|---|---|
| `missing documentation for a …` | Public item without `///` doc | Add a one-line `///` comment |
| `unused import: X` | Patch removed usages but kept the import | Remove `X` from the `use` line |
| `unused variable: x` | Param unused behind a `#[cfg]` gate | Prefix with underscore: `_x` |
| `variable does not need to be mutable` | `let mut x` but `x` never mutated in this cfg | Restructure into per-cfg blocks, or remove `mut` |
| `type X is more private than item Y` | `pub` fn takes `pub(crate)` args | Narrow the fn to `pub(crate)`, or widen the arg type |
| `function X is never used` | Patch replaced callers with a new variant | Prefix with underscore: `_fn_name` |
| `unused Result that must be used` | `.write_all(…)` without `?` | Add `?` to propagate the error |

---

## Troubleshooting

| Symptom | Check |
|---|---|
| Build hook fails on consumer machine | Binary missing for platform — check GitHub Release assets |
| WASM test fails | Rebuild: `make build-wasm` |
| Native passes, web fails | bridge_api.rs WASM path missing the op |
| wire_sync_test fails | EngineOp without matching bridge_api.rs arm |
| "Handle not found" | Handle disposed or never opened |
| "Symbol not found" at runtime | Binary older than source — rebuild |
