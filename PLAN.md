You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-585: rewrite `docs/CAPABILITY_ROADMAP.md` for the 6.0.0 API. Only that file changes.

## Why

The roadmap still maps the 5.x API: `pdf_standalone.dart`, `pdf_sugar.dart`, `PdfDoc`, `setTitle`, `deletePage`, `mergeFrom`, `embedFile`, `scrubMetadata` and the like. 6.0.0 replaced that surface. `docs/MIGRATION.md` lists every rename and removal; `README.md` ("I want to...", "Usage") shows the 6.0.0 doors; `lib/` is the truth.

## Read first, end to end

`docs/CAPABILITY_ROADMAP.md`, `docs/MIGRATION.md`, `README.md`, `docs/ARCHITECTURE.md` ("The four layers", "Source tree", "Trim"), and the public API under `lib/src/` (the `Pdf` class, `PdfDocument`, `PdfEditor`, `PdfBuilder` and its page, `pdf_reads.dart`, `pdf_edits.dart`, `pdf_jobs.dart`). Find each file yourself; ARCHITECTURE's Source tree names them.

## What the doc becomes

- The opening keeps its job: every engine capability mapped to its Dart surface, the bounded-I/O rule, and where reads and edits live. Replace the "Five files, strict rules" table with the 6.0.0 layout: the handles (`PdfDocument`, `PdfEditor`, `PdfBuilder`), the files that declare every read and every edit once (`pdf_reads.dart`, `pdf_edits.dart`), and the doors on `Pdf` (handle, scope, one-shot, several documents), as README "Usage" describes them.
- One table per surface, same columns as today (Capability | Rust | Dart | Status): reads (`PdfDocument`), edits (`PdfEdits` and what only an editor adds), the builder and its page, and the doors on `Pdf` (merge, split, compress, imagesToPdf, sign, convertToOffice, convertFromOffice, convertXfaToAcroForm, resave, info, readOnce, editOnce). The Dart column uses the 6.0.0 name exactly as `lib/` declares it. The Rust column names the `host/dispatch.rs` function or engine function the op reaches (check each).
- A 5.x capability that 6.0.0 removed or folded into another call gets no row of its own: it is the row of the call that replaced it (MIGRATION says which). Do not list removed names.
- Every PLANNED, BLOCKED, WONT_DO, "Built in the engine, not shipped", "Not planned" and "Test infrastructure gaps" row keeps its meaning; rename only the Dart names in it. The Runtime section keeps its rows (four web I/O modes).
- The Summary totals are recounted from the new tables.
- Status notes say what is true now and how it is proven (a battery or fixture), as today's notes do; no history, no dates, no "renamed from" notes (MIGRATION holds those).
- Keep the Build & distribution, Binary size — trim, and the triggers tables as they are, except for a name that no longer exists.

## Rules

- Every path, class, member and function the doc names exists. Prove it: write a small script (scratch, not committed) that pulls every backticked Dart identifier and file path from the new doc and greps `lib/`, `vendor/pdf_oxide/src/host/` and the repo for it, and paste its output (every name found) into REPORT.md. A name it cannot find is fixed or removed, never left.
- Plain global English, short sentences, present tense, no "we". Tables for mappings. Match the voice of `docs/ARCHITECTURE.md`.
- Links inside the doc resolve (check each `#anchor` against the headings).
- Change no other file.

## Done

- `git diff --stat` shows only `docs/CAPABILITY_ROADMAP.md`.
- Write `REPORT.md` at the repository root: the script and its output, each 5.x row you folded and into which row, each choice where this plan left one open.
- `git diff HEAD -- docs/CAPABILITY_ROADMAP.md > t-585-pkg.patch`, undo your working change so only the patch carries it, and commit only `t-585-pkg.patch` and `REPORT.md`.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-585`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-585`, and do not clone it.
