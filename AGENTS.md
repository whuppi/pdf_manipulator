<!--
============================================================================
AUTO-GENERATED — DO NOT EDIT
============================================================================
This file is rendered by:
  /Users/deepanshu/personal1/whuppi/.claude/scripts/stamp-agents.sh
from:
  /Users/deepanshu/personal1/whuppi/AGENTS.template.md
  with per-repo data inlined in the stamper itself.

To change content:
  - Workspace-wide: edit AGENTS.template.md, then re-run the stamper.
  - One repo only:  edit the `repo_data` case for "pdf_manipulator" in stamp-agents.sh,
                    then re-run the stamper.
Manual edits to this file will be overwritten on the next stamp.
============================================================================
-->

# pdf_manipulator

> **Public AI agent contract** for pdf_manipulator — read by Cursor, OpenAI Codex, Aider, Devin, JetBrains Junie, and any AI tool that follows the [agents.md](https://agents.md) convention.
>
> Claude Code reads the deeper workspace config at `whuppi/.claude/rules/` and `whuppi/.claude/memory/` automatically — this AGENTS.md exists for every *other* AI tool.
>
> Stamped from `whuppi/AGENTS.template.md`. Per-repo content lives in the placeholder sections; everything else is identical workspace-wide.

---

## What this tool does

**pdf_manipulator** is a cross-platform PDF manipulation package for Dart & Flutter — instance-based API (`final pdf = Pdf()`) with `DataSource` in, `DataSink` out, streaming through bounded buffers (64KB read / 256KB write). Merge, split, compress, encrypt, render, extract text, search, sign, validate, convert, stamp, build from scratch. Powered by vendored forks of pdf_oxide + office_oxide (Rust) at `vendor/`. Lane architecture: one shared Dart Router routing jobs to isolated lanes — detached Rust threads on native, Web Workers + WASM on web (3 I/O modes: JSPI, Atomics, OPFS) — every operation runs off the main thread, with per-op cancellation and instant dispose. No `dart:io` in the barrel. 5-step asset resolution: cached → download → compile → submodule → error.

This repo is one tool inside the **whuppi** workspace — a multi-tool monorepo. The workspace ships shared engineering standards, code conventions, brand identity, and build patterns that apply across every tool. They're documented in three layers:

- **Repo-specific architecture, design, reference:** `./docs/`
- **Workspace human-readable standards:** `../docs/` (when this repo is cloned as part of the whuppi workspace) — engineering principles, decision frameworks, secret/CI patterns
- **Workspace AI-only directives:** `../.claude/rules/` (Claude Code reads these automatically; other AI tools can read them as supplementary context)

If you're working on this tool standalone (cloned outside the workspace), the in-repo `./docs/` is your authority; ignore the workspace pointers.

---

## Build and test commands

Run these after every code change. A failing test or analyzer error means the task is not done — don't suppress with `// ignore:`, `# noqa`, or `--no-verify`. Fix the underlying issue.

```bash
# Setup (needs Rust — https://rustup.rs, FVM — https://fvm.app)
git clone --recursive https://github.com/whuppi/pdf_manipulator
cd pdf_manipulator
fvm install                     # downloads SDK pinned in .fvmrc
make check                      # analyze + test-guards + tests (native + 3 web modes) + example

# Without FVM (override SDK commands)
make check DART=dart FLUTTER=flutter

# Individual targets
make analyze                    # Dart + Rust warnings (all features)
make test-ops-native            # native FFI ops tests
make test-ops-web               # web ops tests (JSPI, Atomics, OPFS)
make test-example               # integration tests (macOS + 3 web modes)
make build-wasm                 # rebuild WASM after Rust changes
```

---

## Code style

Match the style of existing code in this repo first. Workspace-wide standards live at:

- **Engineering standards** (seven questions before every decision, env-blind code, twelve-factor checklist): `../docs/universal/development-standards.md`
- **Secrets and environments** (GitHub Environments, branch=env, security walls, files-not-env-vars): `../docs/universal/secrets-and-environments.md`
- **Python tools** (SDK/CLI/MCP three-layer pattern, ruff config, hatchling): `../.claude/rules/python-shared/sdk-cli-mcp-pattern.md`
- **Flutter packages** (opaque boundaries, async at edges, dependency flow): `../.claude/rules/flutter-shared/package-design.md`
- **Comments and doc-comments** (what earns a comment, what doesn't): `../.claude/rules/universal/comments.md`
- **Renaming anything** (sweep all references in one session): `../.claude/rules/universal/rename-hygiene.md`

When in doubt, read existing code in this repo and match it. Per-repo style consistency beats general-best-practice consistency.

---

## Tool-specific notes

**Instance-based API.** `final pdf = Pdf()` — all methods on the instance. Every engine method returns a `PdfTask<T>` (a `Future` plus `cancel()`). `pdf.dispose()` kills every lane instantly. `pdf.edit(source)` for batch editing, `pdf.build()` for creating from scratch. I/O is `DataSource` (random-access reads) and `DataSink` (sequential writes) — the transport streams through fixed 64KB/256KB buffers regardless of file size.

**5-step build hook.** cached (hash verified) → download from GitHub Releases → compile from vendor source → init submodules + compile → error. SHA-256 hashes in `lib/src/hook/asset_hashes.dart`. Constants in `build.json`.

**Vendor submodules with patches.** `vendor/pdf_oxide/` and `vendor/office_oxide/` are git submodules with named patch branches. Full inventory in `docs/UPDATING.md`.

**Conventional commits required.** PR titles must follow `feat:` / `fix:` / `chore:` etc. Enforced by CI (`pr-lint.yml`) and local hook (`.githooks/commit-msg`).

**CI/CD.** Capability-based CI architecture — 13 independent capability actions provisioned by `make-target` orchestrator. Matrix manifests declare capabilities per job. All workflows accept runner override via `workflow_dispatch`. Release pipeline: `create-release.yml` (gate → discover → compile matrix → upload → publish). All release logic in `tool/release.sh`.

---

## Data, secrets, and gitignore

This repo's `.gitignore` is stamped from `../.gitignore.template` (workspace canonical). It already covers:

- `data/.env` and every other `.env` flavor (only `.env.example` / `.env.template` / `.env.sample` are committed)
- `data/auth/` (captured tokens, cookies, OAuth credentials)
- `data/db/*.sqlite*` (full app state — irreplaceable)
- `cookies*.json`, `*.token`, `*.pem`, `*.key`
- `output/`, `debug/`, `logs/`, `cache/`

Never commit a sensitive file even if it's somehow not gitignored — surface to the maintainer instead. The gitignore is defense-in-depth, not the only check.

---

## Working with AI agents

- **Run the test suite before claiming completion.** Always.
- **Don't add `TODO` comments as a substitute for fixing things.** If you found it, you own it — fix in this pass or surface to the maintainer.
- **Don't add backwards-compat shims** for code that hasn't shipped. Code assumes the latest schema and contracts; migrations handle old data once.
- **Don't refactor "for cleanliness" without a stated reason.** Surface the suggestion before changing surrounding code.
- **No co-authored-by AI in commits.** The maintainer is the author.
- **Never force-push protected branches** (`prod`, `main`, `dev`). Never skip pre-commit hooks.

For the engineering philosophy that informs every line of code in this workspace, see `../.claude/rules/universal/dc-engineering-philosophy.md` if available.

---

*This file is stamped from `whuppi/AGENTS.template.md`. The placeholder sections (`{{...}}`) are the only parts customized per repo. Re-stamping refreshes the shared content; per-repo placeholders are preserved.*
