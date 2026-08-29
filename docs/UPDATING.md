# Updating pdf_manipulator

Maintenance recipes. For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md).
For capability status see [`CAPABILITY_ROADMAP.md`](CAPABILITY_ROADMAP.md).

---

## Vendored forks

Two git submodules, each a fork of the upstream repo with a named
patch branch.

| Crate | Upstream | Fork | Branch | Base tag | Submodule |
|---|---|---|---|---|---|
| pdf_oxide | [`yfedoseev/pdf_oxide`](https://github.com/yfedoseev/pdf_oxide) | [`whuppi/pdf_oxide`](https://github.com/whuppi/pdf_oxide) | `pdf_manipulator/0.3.73-patches` | `v0.3.73` | `vendor/pdf_oxide/` |
| office_oxide | [`yfedoseev/office_oxide`](https://github.com/yfedoseev/office_oxide) | [`whuppi/office_oxide`](https://github.com/whuppi/office_oxide) | `office_kit/0.1.3-patches` | `v0.1.3` | `vendor/office_oxide/` |

pdf_oxide depends on office_oxide as a path dependency
(`office_oxide = { path = "../office_oxide" }`).
wasm-bindgen + wasm-opt run as library calls inside the fork's
`bindgen_runner` workspace crate, version-pinned by the same
`vendor/pdf_oxide/Cargo.lock` as the engine's wasm-bindgen dependency
— nothing to install, nothing to hardcode.

### Bumping a fork's base tag — update every place it lives

A base tag lives in more than one spot; change all of them together or they
drift (a stale `baseTag` once slipped past review here). For pdf_oxide moving
to a new upstream `vX.Y.Z`:

1. the fork's patch branch — rename to `pdf_manipulator/X.Y.Z-patches`; its
   name is what `analyze.sh` reads when the submodule is on that branch
2. `build.json` → `baseTag` — `analyze.sh`'s off-branch fallback for its
   warning-diff. CI checks the submodule out detached, so this is the value
   CI actually uses; it MUST equal the branch's version
3. the **Branch** and **Base tag** columns in the table above

office_oxide is the same, under its own branch name and `v0.1.3`.

### The fork contract

Each fork carries exactly three things — anything else is debris and
gets deleted on sight:

| Ref | Why it exists |
|---|---|
| `main` | Clean mirror of upstream main. Synced in S1 step 5; never carries our commits. |
| The patch branch | All our patches, rebased onto the base tag. The only branch the submodule points at. |
| `v*` tags | Rebase bases. `make analyze` derives the base tag from the patch-branch name and diffs against it, so the tag must exist on the fork. |

Debris that does NOT belong on a fork: `dependabot/*` branches,
upstream feature/release branches (fork-time copies, instantly stale),
and upstream's Go-binding `go/*` tags. Delete with
`git push origin --delete <ref>` — every such ref still exists on
upstream, so deletion loses nothing.

Never `git push --mirror` a working fork: mirror mode deletes every
remote ref that doesn't exist locally, including `main` and any patch
branch not currently checked out.

### Disable Actions on the fork

A vendored fork is consumed as **source** — the parent repo's own CI
(`make test-rust` / `build-wasm` / `check`) is the gate. The fork's inherited
upstream workflows (Release, language-binding CI, CodeQL, OpenSSF Scorecard,
scheduled scans) validate nothing this repo uses. On each fork, once:

**Settings → Actions → General → "Disable actions for this repository".**

- **Free-tier drain.** Public-repo Actions are free but not unthrottled — a
  fork's heavy Rust/scan pipelines burn org-wide runner allocation and can
  throttle the whole org's hosted runners (every repo's jobs stuck "Waiting for
  a runner"). One vendored Rust fork here ran 40 workflows in two weeks,
  producing nothing the parent consumes.
- **Accidental publish.** S1 pushes the patch branch and a `v*` tag to the fork
  on every bump; an upstream `on: push tags` Release pipeline fires on that tag
  and can cut a GitHub release / publish a package from your mirror. Disabling
  defuses it.
- **Off by default.** The fork is home; upstream is just the base. Routine fork
  work — patches, rebases, tag-moves — never needs the fork's own CI; the
  parent's CI is the gate. The only exception is a deliberate, standalone
  upstream PR (occasional, never during a fix): flip Actions on for that one PR,
  then back off. Off is the resting state.

Disable at the **setting** level — never delete the workflow YAMLs. Deleting
them diverges the mirror from upstream and breaks the clean rebase-on-tag in
S1; the files stay byte-identical to upstream and just never fire.

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
| Preview the pub.dev changelog | [S8 — Preview changelog](#s8--preview-changelog-before-pubdev-publish) |
| Add a test fixture | [S9 — Add a test fixture](#s9--add-a-test-fixture) |

---

## Flutter version pinning

`.fvmrc` (root + `example/.fvmrc`) is the single source of truth for
the Flutter SDK version. Never hardcode the version anywhere else.

`upgrade-check.yml` runs daily and splits the work by risk into two draft
PRs. The `pins` job re-hashes the current pins to catch a repoint, then bumps
every pinned version Dependabot can't see (the Flutter SDK, the tools and
verified binaries in `tool/versions.env`, the zizmor + actionlint gate pins,
binary sha256s recomputed from the upstream assets) onto `chore/pins`. The
`lockfiles` job refreshes `pubspec.lock` onto `chore/lockfiles`. Review, test,
merge each when ready.

---

## S1 — Bump upstream

### 1. Discover what's new

```sh
cd vendor/pdf_oxide
git fetch upstream --tags
git log --oneline vOLD..vNEW
git diff vOLD..vNEW -- src/document.rs src/editor/document_editor.rs | grep "^+.*pub fn"
```

Read the log for fixes that overlap our patches — upstream regularly
lands the same class of fix (appearance streams, word spacing). Where
upstream's version supersedes a patch of ours, the rebase drops ours
and takes upstream.

### 2. Check conflict risk

Run over every file we patch. The markers ARE the list — never
maintain one by hand:

```sh
for f in Cargo.toml $(grep -rl "pdf_manipulator patch" src/ --include="*.rs"); do
  count=$(git diff vOLD..vNEW -- "$f" | wc -l | tr -d ' ')
  [ "$count" -gt "0" ] && echo "RISK : $f ($count lines)" || echo "clean: $f"
done
```

### 3. Rebase

```sh
git rebase vNEW
(cd ../.. && make test-rust)
```

Resolve conflicts commit by commit. `Cargo.lock` conflicts: take the
base (`git checkout vNEW -- Cargo.lock`) and let cargo re-add our
feature deps on the next build. `make test-rust` runs the full `cargo
test` (not `--lib`) for both crates — the `tests/` tree catches
signature drift the lib tests miss.

### 4. Rename branch

```sh
git branch -m pdf_manipulator/OLD-patches pdf_manipulator/NEW-patches
git push origin pdf_manipulator/NEW-patches
git push origin --delete pdf_manipulator/OLD-patches
```

### 5. Sync the fork mirror

```sh
git push origin refs/remotes/upstream/main:refs/heads/main
git push origin refs/tags/vNEW
```

Keeps the fork contract (see "Vendored forks" above): `main` stays a
clean mirror, and the new base tag exists on the fork so `make
analyze` and future rebases can resolve it from a fork-only clone.
The `main` push is a fast-forward; if it isn't, the mirror drifted —
investigate before forcing.

### 6. Rebuild + verify

```sh
cd ../..
make build-wasm
make clean
make check
```

### 7. Commit

```sh
git add vendor/pdf_oxide web_assets/
git commit -m "build: sync upstream vNEW + rebuild WASM"
```

---

## S2 — Edit patches

Our `host/` directory is entirely our code — edit freely, no markers
needed (the module itself is the marker). Everything outside `host/`
is upstream code: **mark every change there** with paired
`── pdf_manipulator patch ──` / `── end pdf_manipulator patch ──`
boundaries. The markers are the authoritative inventory of what we
patch:

```sh
cd vendor/pdf_oxide
grep -rl "pdf_manipulator patch" src/ --include="*.rs"
cd ../..

make test-rust
make build-wasm
make check
```

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

## S5b — Measure sizes

`make shake-audit` is the single builder/measurer for every size claim;
`make verify-readme-sizes` (chained onto it, also standalone) asserts the
README's numbers against the record in `tool/.shake_sizes.json`.

```sh
make shake-audit                        # native full + core, symbols, probes
SHAKE_AUDIT_WASM=1 make shake-audit     # + core-only wasm raw/gzipped
SHAKE_AUDIT_CAPS=1 make shake-audit     # + per-capability costs (5 builds)
make verify-readme-sizes                # README numbers vs the record
```

Run the flagged modes after an engine bump and before a release; the
README's size table and capability costs come from them. Sizes are quoted
in decimal MB (1 MB = 1,000,000 bytes).

**Measurement method — trust only the linker map.** For per-module byte
attribution use `RUSTFLAGS="-C link-arg=-Wl,-map,<file>"` on a forced
relink and aggregate the map's per-symbol sizes. cargo-bloat builds its
own differently-configured artifact, and Mach-O `nm` has no symbol sizes
(address-delta guesses fabricate numbers — a "925 KB table" once measured
that way was really 19 KB of source). Never edit `tool/shake_audit.sh`
while an audit is running: bash re-reads the file by offset.

---

## S6 — Release

### Branch model

The two-lane branch model (`dev` prereleases, `prod` stable) and the
squash-vs-merge-commit rule are the shared model. See
whuppi/ci/docs/ARCHITECTURE.md "The versioned-release model + the stamping
rule". The pdf-specific promotion steps are in `### Prerelease` / `### Stable
release` below.

### The rules

- **NEVER push directly to `dev` or `prod`.** Every change goes through
  a PR. No exceptions.
- **NEVER force-push protected branches** unless syncing prod to dev
  after a divergence (and only with the documented procedure below).
- **NEVER run destructive git commands** (`reset --hard`, `clean -fd`,
  `stash drop`, `gh pr close --delete-branch`) without explicit
  permission.

### Changelog files

`CHANGELOG.md` (stable, on `prod`) and `CHANGELOG.pre.md` (prerelease, on
`dev`), the `## X.Y.Z` heading convention, and the pubspec-stays-`0.0.0`
stamping rule are the shared model. See whuppi/ci/docs/ARCHITECTURE.md "The
versioned-release model + the stamping rule".

### The release pipeline

The gate → discover → compile → upload → publish orchestration (changelog
gate, version discovery, the approval-gate pause, `pub publish`, version-level
concurrency, idempotent reruns) is the shared release engine. See
whuppi/ci/docs/ARCHITECTURE.md "The release surface".

What pdf's release adds on top:

- **Native compile matrix** — the compile step checks out the tag and builds
  all 6 platforms in parallel.
- **Submodule deregistration** — discover deregisters the vendored submodules
  so the tag ships raw vendor source.
- **Asset hashes into the tag** — after upload, `--update-tag-hashes` writes
  the binary hashes back into the tag, so `git: ref: <tag>` users get verified
  binary downloads.

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

If prod diverges from dev (e.g. an accidental squash merge on a
promotion PR), force-sync it. Prod normally forbids force-push, so the
procedure is **allow → sync → re-forbid** — and steps 1 and 3 are the
*same* protection-PUT call with only the final `allow_force_pushes`
flag flipped (`true`, then `false`).

```sh
# Step 1 — allow force-push (allow_force_pushes=true):
gh api repos/whuppi/pdf_manipulator/branches/prod/protection -X PUT \
  -F "required_status_checks[strict]=true" \
  -F "required_status_checks[checks][][context]=checks / Conventional Commit" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_status_checks[checks][][context]=Full Test Gate" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_status_checks[checks][][context]=CI Gate" -F "required_status_checks[checks][][app_id]=15368" \
  -F "required_pull_request_reviews[dismiss_stale_reviews]=true" \
  -F "required_pull_request_reviews[require_code_owner_reviews]=true" \
  -F "required_pull_request_reviews[required_approving_review_count]=2" \
  -F "enforce_admins=false" -F "restrictions=null" -F "allow_force_pushes=true" \
  --silent

# Step 2 — force-sync:
git push origin dev:prod --force-with-lease

# Step 3 — re-forbid: rerun the Step 1 command with allow_force_pushes=false
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

See whuppi/ci/docs/ARCHITECTURE.md for the shared workflow mechanics (capability
model, runner model, reusable workflows). The pdf-local workflow table is in
ARCHITECTURE.md "CI/CD architecture".

### Git hooks

| Hook | Enforces |
|---|---|
| `commit-msg` | Conventional Commits format, blocks merge commits |
| `pre-commit` | Rejects `git add -f` of gitignored files |

### CODEOWNERS

| Path | Owner |
|---|---|
| `*` (default — everything else) | `@whuppi/pdf-manipulator-maintainers` |
| `CHANGELOG.md`, `CHANGELOG.pre.md` | `@chaudharydeepanshu` |
| `pubspec.yaml` | `@chaudharydeepanshu` |
| `.github/`, `.githooks/` | `@chaudharydeepanshu` |

Last match wins (gitignore semantics): the release/config paths override the default-team rule.

---

## S7 — Verify our-code warnings

`make analyze` includes a Rust warning check for both vendored crates:

1. Runs `cargo check` with all features (same set as CI release builds)
2. Uses `--message-format=json` to get warnings even from cached builds
3. Derives the upstream base tag from the branch name automatically
   (`pdf_manipulator/0.3.73-patches` → `v0.3.73`). No hardcoded tag —
   renaming the branch in S1 step 4 is all that's needed.
4. Diffs against the base tag to find lines we changed
5. Fails if any warning falls inside our changed lines

Checks both `vendor/pdf_oxide` and `vendor/office_oxide`.

```sh
make analyze
```

If the Rust check fails, it prints the exact file:line and warning.
Fix them before committing.

The feature set lives once in `build.json` (`features.native` /
`features.wasm`); `tool/compile.dart`, the build hook, `make analyze`,
and `make test-rust` all read it, so the built, analyzed, and tested
feature sets can't drift apart.

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

## S8 — Preview changelog before pub.dev publish

The filtered changelog is printed to CI logs in the "Preview pub.dev
changelog" step of the upload-assets job — right before the publish
approval gate. Open the workflow run, expand that step, and review
the full changelog that will be published.

To preview locally, run the same mode the publish job runs. `release.sh`
lives in the shared [whuppi/ci](https://github.com/whuppi/ci) repo, not
this one — CI reaches it through the `release-tool` action, so locally you
point at a checkout of that repo (a sibling of this one). Run from this
package's root, and set `GITHUB_REPOSITORY` — the script requires it while
loading, whichever mode you ask for, and exits without it. Some modes want
more: `--check-versions` also needs `BRANCH` (`dev` or `prod`, picking the
lane it checks). `--help` lists every mode.

```sh
GITHUB_REPOSITORY=whuppi/pdf_manipulator \
  bash ../ci/tool/ci/release.sh --stamp-changelog vX.Y.Z
cat CHANGELOG.md
git checkout CHANGELOG.md   # restore
```

---

## S9 — Add a test fixture

1. Add a `FixtureSpec` to `test/fixtures/catalog.dart` — name, why,
   declared truths, and a dart-pdf builder (the independent producer;
   never this package's own builder — the foreign-diet rule in
   ARCHITECTURE.md's testing section).
2. `make fixtures` (test targets run it automatically; the stamp
   regenerates everything because the catalog changed).
3. Import `test/fixtures/generated/fixtures.dart` and assert against
   the emitted `f<Name>Truth` constants.

Fixtures that no cross-platform library can produce (e.g. encrypted)
are generated ONCE externally and committed under
`test/fixtures/third_party/` with full provenance in the file header
(see `tp_encrypted.dart`). Deliberately broken byte sequences are
hand-authored in `test/fixtures/handwritten.dart`.

---

## Troubleshooting

| Symptom | Check |
|---|---|
| Build hook fails on consumer machine | Binary missing for target — check GitHub Release assets |
| WASM test fails | Rebuild: `make build-wasm` |
| Native passes, web fails | bridge_api.rs WASM path missing the op |
| wire_sync_test fails | EngineOp without matching bridge_api.rs arm |
| "Handle not found" | Handle disposed or never opened |
| "Symbol not found" at runtime | Binary older than source — rebuild |
| `make verify-android` fails without Rust | Hook tries compile → `cargo` not found → error with install URL |
| First web build takes minutes at "bindgen + optimize" | One-time: cargo builds `bindgen_runner` (binaryen compiles from source); cached afterwards |
| `make verify-linux` fails without GTK | Makefile errors with `apt-get install` command (dev) or auto-installs (CI) |
| `flutter build --release` fails but debug works | Build hook routes differently in release — check `hook/link.dart` exists |
| Google Play rejects APK "16 KB page size" | Rust cdylib needs `-Wl,-z,max-page-size=16384` in `build.rs`. Cargo doesn't inherit NDK's 16 KB default. Any new Rust crate producing a cdylib for Android needs this. See `vendor/pdf_oxide/build.rs`. |

---
