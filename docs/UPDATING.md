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
wasm-bindgen-cli version is read from `vendor/pdf_oxide/Cargo.lock`
at build time — never hardcoded. `compile_rust.sh` auto-installs the
matching version before WASM builds.

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

## Flutter version pinning

`.fvmrc` (root + `example/.fvmrc`) is the single source of truth for
the Flutter SDK version. Never hardcode the version anywhere else.

`flutter-upgrade.yml` runs daily, detects new Flutter stable releases,
and opens a draft PR on `chore/flutter-upgrade` that bumps both files.
Review, test, merge when ready.

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

### Branch model

| Branch | Purpose | Merge method INTO this branch |
|---|---|---|
| feature branches | in-progress work | — |
| `dev` | integration + prereleases | **Squash and merge** (clean up feature work) |
| `prod` | stable releases | **Create a merge commit** (preserve SHA chain) |

**Why merge commit for dev→prod:** squash and rebase both create new
SHAs. Prod and dev diverge. Next promotion shows the entire history as
"new." Merge commit preserves the original SHAs so both branches share
the same commit objects.

### The rules

- **NEVER push directly to `dev` or `prod`.** Every change goes through
  a PR. No exceptions.
- **NEVER force-push protected branches** unless syncing prod to dev
  after a divergence (and only with the documented procedure below).
- **NEVER run destructive git commands** (`reset --hard`, `clean -fd`,
  `stash drop`, `gh pr close --delete-branch`) without explicit
  permission.

### Changelog files

| File | Purpose |
|---|---|
| `CHANGELOG.md` | Stable releases — triggers release on `prod` |
| `CHANGELOG.pre.md` | Prereleases — triggers release on `dev` |

Add a `## X.Y.Z` heading at the top. Write a human summary. CI
handles commit lists, filtering, and publishing. `pubspec.yaml` stays
`version: 0.0.0` in git — CI stamps the real version from the tag.

### The release pipeline

All logic lives in `tool/release.sh` (7 modes). The workflow
(`create-release.yml`) is pure job orchestration.

```
Push to dev (CHANGELOG.pre.md) or prod (CHANGELOG.md)
  │
  ├─ 1. gate     → release.sh --gate
  │               checks if the right changelog file changed
  │               dev only reacts to CHANGELOG.pre.md
  │               prod only reacts to CHANGELOG.md
  │
  ├─ 2. discover → release.sh --discover
  │               finds version, validates branch/type match
  │               stamps version + deregisters submodules
  │               creates orphan tag commit + GitHub Release
  │
  ├─ 3. compile  → checkout tag, build all 6 platforms in parallel
  │
  ├─ 4. upload   → upload binaries to GitHub Release
  │               release.sh --add-git-install (install snippet)
  │               release.sh --update-tag-hashes (asset hashes into tag)
  │
  └─ 5. publish  → ⏸ PAUSE (GitHub Environment approval gate)
                    release.sh --stamp-changelog (filtered CHANGELOG.md)
                    dart pub publish
                    release.sh --add-pub-install (install snippet)
```

After step 4, the tag has: stamped version + raw vendor source +
asset hashes. `git: ref: <tag>` users get verified binary downloads.

Concurrency is version-level — two different versions can release in
parallel. Same version pushed twice: the newer run cancels the stale one.

Every step is idempotent on rerun.

### Prerelease

```
1. Add ## X.Y.Z-dev.N at top of CHANGELOG.pre.md
2. PR to dev → squash and merge
3. (automatic) gate → discover → compile → upload
4. (manual) approve "publish" environment → pub.dev
```

### Stable release

```
1. Add ## X.Y.Z at top of CHANGELOG.md
2. PR to dev → squash and merge (dev ignores stable changelog — no release triggered)
3. PR from dev → prod → create a merge commit (NOT squash, NOT rebase)
4. (automatic) gate → discover → compile → upload
5. (manual) approve "publish" environment → pub.dev
```

### Manual re-trigger

If a release needs re-triggering (e.g. after a fix to the pipeline):

```sh
# Must use --ref to run on the correct branch
gh workflow run "Release" --repo whuppi/pdf_manipulator --ref dev --field branch=dev
gh workflow run "Release" --repo whuppi/pdf_manipulator --ref prod --field branch=prod
```

`--ref` controls which branch the workflow runs ON. `--field branch`
is the input the script reads. Both must match. Without `--ref`, the
workflow runs on the default branch regardless of the input.

### Delete and recreate a release

When a release needs to be rebuilt (broken binaries, missing assets):

```sh
gh release delete vX.Y.Z --repo whuppi/pdf_manipulator --yes
git push origin --delete refs/tags/vX.Y.Z
gh workflow run "Release" --repo whuppi/pdf_manipulator --ref <branch> --field branch=<branch>
```

### Syncing prod to dev (after divergence)

If prod diverges from dev (e.g. accidental squash merge on a promotion
PR), force-sync prod to dev:

```sh
# 1. Temporarily allow force-push on prod
gh api repos/whuppi/pdf_manipulator/branches/prod/protection -X PUT \
  -F "required_status_checks[strict]=true" \
  -F "required_status_checks[checks][][context]=Conventional Commit" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_status_checks[checks][][context]=Full Test Gate" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_status_checks[checks][][context]=CI Gate" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_pull_request_reviews[dismiss_stale_reviews]=true" \
  -F "required_pull_request_reviews[require_code_owner_reviews]=true" \
  -F "required_pull_request_reviews[required_approving_review_count]=1" \
  -F "enforce_admins=false" -F "restrictions=null" -F "allow_force_pushes=true" \
  --silent

# 2. Force-sync
git push origin dev:prod --force-with-lease

# 3. Disable force-push
gh api repos/whuppi/pdf_manipulator/branches/prod/protection -X PUT \
  -F "required_status_checks[strict]=true" \
  -F "required_status_checks[checks][][context]=Conventional Commit" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_status_checks[checks][][context]=Full Test Gate" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_status_checks[checks][][context]=CI Gate" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_pull_request_reviews[dismiss_stale_reviews]=true" \
  -F "required_pull_request_reviews[require_code_owner_reviews]=true" \
  -F "required_pull_request_reviews[required_approving_review_count]=1" \
  -F "enforce_admins=false" -F "restrictions=null" -F "allow_force_pushes=false" \
  --silent
```

### Failure recovery

| Failure | Fix |
|---|---|
| Compile failed (infra) | Rerun via workflow_dispatch (idempotent) |
| Compile failed (code bug) | Fix on dev via PR, bump prerelease version |
| Upload failed | Rerun — clobber overwrites |
| Publish failed | Rerun — approval gate shows again |
| Tag exists but no Release | Rerun via workflow_dispatch |
| Wrong release notes | Delete release + tag, re-trigger |

### CI workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `ci.yml` | PR to prod/dev | `make analyze` + `make test-unit` + `make test-ops-native` |
| `pr-lint.yml` | PR to prod/dev | Conventional commit title + promotion chain |
| `full-test.yml` | `ready-to-test` label | 10 jobs: 4 pkg + 6 integration |
| `create-release.yml` | Push to dev/prod changing changelog, or `workflow_dispatch` | Full release pipeline (7 steps) |
| `flutter-upgrade.yml` | Daily or `workflow_dispatch` | Auto-detect new Flutter stable |
| `triage.yml` | Issues/PRs | Auto-label, auto-assign, dependabot notifications |

### Git hooks

| Hook | Enforces |
|---|---|
| `commit-msg` | Conventional Commits format, blocks merge commits |
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
