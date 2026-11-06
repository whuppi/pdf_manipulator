# t-569: a wall that proves the incremental save's refusal list complete

Result: the wall is in place and **nothing is dropped today**. Every field of `DocumentEditor` is classified; every `carried` and `refused` field that an editor call can stage has a test; the incremental save carried or refused every one of them.

Changes (all in `vendor/pdf_oxide`, carried by `t-569-fork.patch`; no package file changed, so there is no `t-569-pkg.patch`):

- `src/editor/document_editor.rs`: `DocumentEditor::incremental_census` (test-only, between patch markers). Its pattern names every field with no `..`.
- `src/host/engine_proofs/incremental_wall.rs` (new) and one `mod` line in `engine_proofs/mod.rs`: 25 tests.

## 1. The field census

`incremental_census` is `#[cfg(test)]` and sits after the `Checkpoint` struct. A field added to `DocumentEditor` without a row fails to compile (section 4). `every_edit_field_has_a_kind_test` then fails if a `carried` or `refused` field has no kind test, or a test names a field under another class.

| field | class | why |
|---|---|---|
| `source` | source | the opened document; the append copies every original byte |
| `source_path` | source | where the document was opened from |
| `modified_objects` | carried | written as the append's replaced objects (incremental_objects) |
| `new_objects` | refused | 'form fields or objects were added'; no editor call stages it today |
| `next_object_id` | cache | an id counter; the append reads it only for /Size and a new /Info id |
| `modified_info` | carried | written as the append's /Info object (build_info_object) |
| `page_order` | refused | 'pages were removed or reordered' (the identity check) |
| `original_page_count` | source | the page count at open; the identity check compares against it |
| `is_modified` | cache | a dirty flag; the append writes what is staged, not this flag |
| `modified_content` | refused | 'page content was drawn, erased or rebuilt' |
| `overlay_additions` | refused | 'page content was drawn, erased or rebuilt'; no editor call stages it today |
| `resource_manager` | cache | names resources for hand-built content; the save never reads it, and that content is staged in modified_content |
| `structure_modified` | refused | 'page content was drawn, erased or rebuilt' |
| `modified_annotations` | refused | 'annotations were changed or flattened' |
| `modified_page_props` | carried | applied to the page object in the append (incremental_objects) |
| `erase_regions` | refused | 'page content was drawn, erased or rebuilt' |
| `flatten_annotations_pages` | refused | 'annotations were changed or flattened' |
| `apply_redactions_pages` | refused | 'content was redacted or scrubbed' |
| `redaction_regions` | refused | 'content was redacted or scrubbed' |
| `redacted_pages` | refused | 'content was redacted or scrubbed' |
| `redacted_orphan_ids` | refused | 'content was redacted or scrubbed' |
| `redacted_image_originals` | refused | not named by the refusal; set only with redacted_pages, which is |
| `image_modifications` | refused | 'page content was drawn, erased or rebuilt' |
| `flatten_forms_pages` | refused | 'annotations were changed or flattened' |
| `remove_acroform` | refused | 'form fields were removed' |
| `flatten_warnings` | cache | diagnostics of a flatten, which is refused |
| `embedded_files` | refused | 'files were embedded' |
| `modified_form_fields` | carried | an existing field's edits flush into the append's replaced objects (flush_form_fields_to_modified_objects); a new field is refused ('form fields or objects were added') |
| `deleted_form_fields` | refused | 'form fields were removed' |
| `acroform_modified` | carried | the flag that runs the form flush in incremental_objects |
| `merged_sources` | refused | 'pages were merged in' |
| `merged_page_props` | refused | keyed by merged page, which exists only after a merge: 'pages were merged in' |
| `merged_form_fields` | refused | 'pages were merged in' |
| `merged_acroform_dr` | refused | not named by the refusal; set only by a merge, which fills merged_sources |
| `merged_acroform_da` | refused | not named by the refusal; set only by a merge, which fills merged_sources |
| `source_page_refs` | cache | the source's page refs, computed on demand |
| `checkpoint` | cache | the undo state of a running operation; empty between operations |

Three rows are classed by their companion, not by a term of their own in `incremental_refusal`; none is an edit that could be dropped alone:

- `redacted_image_originals` is written only by `apply_redactions_destructive`, in the same statement block as `redacted_pages`, which the refusal names.
- `merged_acroform_dr` and `merged_acroform_da` are written only by a merge, which fills `merged_sources`.
- `merged_page_props` is keyed by merged page index, which exists only after a merge.

`modified_form_fields` is `carried` for an existing field (value and every property setter flush into replaced objects) and `refused` for a new field (`is_new`). The test `BOTH` list says so.

## 2. The behaviour, per kind

Each test stages one kind on a two-page text fixture (`two_pages`) or a one-field form fixture (`form`) and saves through `host::incremental::write`, the function the bridge calls.

`refused_*`: the save returns `Refused`, the message contains the reason text for that kind, and no byte was written. A success fails with "reported success and dropped the edit".

| test | staged edit | field(s) |
|---|---|---|
| `refused_remove_page` | `remove_page(0)` | `page_order` |
| `refused_reorder_pages` | `move_page(0, 1)` | `page_order` |
| `refused_duplicate_page` | `duplicate_page(0)` | `page_order` |
| `refused_merge` | `merge_from_bytes` | `merged_sources` |
| `refused_merge_form` | merge of a document with an AcroForm (`/Fields`, `/DR`, `/DA`) | `merged_form_fields`, `merged_acroform_dr`, `merged_acroform_da` |
| `refused_merged_page_rotation` | merge, then rotate the merged page | `merged_page_props` |
| `refused_page_content` | `set_page_content` | `modified_content`, `structure_modified` |
| `refused_erase_region` | `erase_region` | `erase_regions` |
| `refused_image_modification` | `reposition_image` | `image_modifications` |
| `refused_annotation` | `add_page_annotation` (watermark) | `modified_annotations` |
| `refused_flatten_annotations` | `flatten_page_annotations` | `flatten_annotations_pages` |
| `refused_flatten_forms_on_page` | `flatten_forms_on_page` | `flatten_forms_pages` |
| `refused_flatten_forms` | `flatten_forms` | `remove_acroform` |
| `refused_redaction_mark` | `add_redaction` | `redaction_regions` |
| `refused_redaction_applied` | `add_redaction` + `apply_redactions_destructive` | `apply_redactions_pages`, `redacted_pages`, `redacted_orphan_ids`, `redacted_image_originals` |
| `refused_embedded_file` | `embed_file` | `embedded_files` |
| `refused_new_form_field` | `add_form_field` | `modified_form_fields` (new field) |
| `refused_removed_form_field` | `remove_form_field` | `deleted_form_fields` |

`carried_*`: the same edit is staged on two editors; one saves incrementally, one with a full rewrite (`compress: false`, `object_streams: false`). Both outputs are reopened and compared on the page count, each page's operators (`page_operators`, as `Debug` strings), and the objects the edit changed. The test also asserts that the incremental output differs from the source in what it reads, so a no-op edit cannot pass.

| test | staged edit | compared |
|---|---|---|
| `carried_info` | `set_title`, `set_author` | `/Title`, `/Author` of the trailer's `/Info` |
| `carried_page_props` | rotation of page 0; media box and crop box of page 1 | `/Rotate`, `/MediaBox`, `/CropBox` of both pages |
| `carried_objects` | `insert_modified` of the catalog with `/Lang` | `/Lang`, `/Pages` of the catalog |
| `carried_form_field_value` | `set_form_field_value` | `/V` of the field object |
| `carried_form_field_property` | tooltip, max length, flags | `/TU`, `/MaxLen`, `/Ff` |
| `carried_form_field_property_removal` | `remove_form_field_tooltip`, `remove_form_field_max_length` | `/TU`, `/MaxLen` |

Two fields have no test because no editor call stages them: `new_objects` and `overlay_additions` (nothing in the crate pushes to or inserts into them). They are listed in `UNSTAGEABLE`, which the census test requires to be `refused`. If a call that stages one is added, `incremental_refusal` already names it, and the census test will not ask for a test until the field leaves `UNSTAGEABLE`.

## 3. Nothing dropped today

Every `carried` test passed on its first run, including the property removal and the page-box probes I added to look for a drop. Every `refused` test passed. No reproducer.

Observation, not a drop: `commit_in_place` and `replace_source_bytes` (the PDF/A converter) replace `source` with rewritten bytes. An incremental save after one appends to those bytes, which already hold the edits, so nothing is lost. `source` stays classed `source`.

## 4. The two walls failing

Census: I added `injected_probe: bool` to `DocumentEditor` (and to its three constructors), and built the tests:

```
error[E0027]: pattern does not mention field `injected_probe`
   --> src/editor/document_editor.rs:954:13
    |
954 |           let DocumentEditor {
    |  _____________^
955 | |             source: _,
956 | |             source_path: _,
957 | |             modified_objects: _,
...   |
991 | |             checkpoint: _,
992 | |         } = self;
    | |_________^ missing field `injected_probe`
    |
help: include the missing field in the pattern
    |
991 -             checkpoint: _,
992 -         } = self;
```

Refusal list: I deleted the `embedded_files` branch from `incremental_refusal` and ran the module:

```
    thread 'host::engine_proofs::incremental_wall::refused_embedded_file' panicked at src/host/engine_proofs/incremental_wall.rs:81:18:
    embedded file: the incremental save reported success and dropped the edit

     Summary [   0.243s] 25 tests run: 24 passed, 1 failed, 10338 skipped
        FAIL [   0.117s] (25/25) pdf_oxide host::engine_proofs::incremental_wall::refused_embedded_file
```

Then every other entry, one by one. The build had a temporary `brk(name)` switch (read from the environment) in `incremental_refusal` and `incremental_objects`; each run skipped the named term(s) and recorded which tests failed. Failing tests per skipped term:

| skipped | tests that failed |
|---|---|
| `page_order` | refused_duplicate_page refused_remove_page refused_reorder_pages |
| `merged_sources,merged_form_fields` | refused_merge refused_merge_form refused_merged_page_rotation |
| `merged_form_fields` | _none (see note)_ |
| `apply_redactions_pages,redaction_regions,redacted_pages,redacted_orphan_ids` | refused_redaction_applied refused_redaction_mark |
| `redaction_regions` | refused_redaction_mark |
| `redacted_pages,redacted_orphan_ids,apply_redactions_pages` | refused_redaction_applied |
| `deleted_form_fields` | refused_removed_form_field |
| `remove_acroform` | refused_flatten_forms |
| `structure_modified,modified_content` | refused_page_content |
| `modified_content` | _none (see note)_ |
| `overlay_additions` | _none (see note)_ |
| `erase_regions` | refused_erase_region |
| `image_modifications` | refused_image_modification |
| `modified_annotations` | refused_annotation |
| `flatten_annotations_pages` | refused_flatten_annotations |
| `flatten_forms_pages` | refused_flatten_forms_on_page |
| `embedded_files` | refused_embedded_file |
| `new_objects` | _none (see note)_ |
| `new_form_field` | refused_new_form_field |
| `flush` | carried_form_field_property carried_form_field_property_removal carried_form_field_value |
| `objects` | carried_form_field_property carried_form_field_property_removal carried_form_field_value carried_objects |
| `props` | carried_page_props |
| `info` | carried_info |

Notes on the empty rows: `merged_form_fields` alone, `modified_content` alone (`structure_modified` is set by the same call), `overlay_additions` and `new_objects` leave another term that still refuses the same staged edit, or nothing can stage them. Where several terms refuse one edit, the test fails when all of them are skipped (`merged_sources` + `merged_form_fields`; the four redaction terms for `refused_redaction_applied`). The `flush`, `objects`, `props` and `info` rows are the carried side: skipping the form flush, the replaced objects, the page props, or the info object fails the matching `carried_*` tests (`objects` also drops the objects the form flush stages, so it fails the form tests too).

After each injection I restored the file from a copy and `git diff` was identical to the diff taken before the injection; no `brk` and no `injected_probe` is left.

## 5. Gate

- Rust: `Summary [ 827.894s] 10298 tests run: 10298 passed (11 slow), 65 skipped`
- The new module alone: `Summary [   0.229s] 25 tests run: 25 passed, 10338 skipped`
- `bash tool/check_patch_markers.sh`: exit 0, no output
- `bash tool/check_fork_tests.sh`: exit 0, no output
- `make test-ops-native DART=dart`: `01:13 +663 ~1: All tests passed!` (exit 0)

## 6. Choices the plan left open

- **Where the census lives.** The fields are private to `document_editor.rs`, so the destructure must be there. It is a plain `#[cfg(test)]` function between patch markers (no `#[test]` in upstream code, which `check_fork_tests.sh` check 1 forbids). The tests are in `src/host/engine_proofs/`.
- **Literal pattern, not a macro.** My first version generated the pattern and the rows from one macro. For a new field, rustc then said only "pattern requires `..` due to inaccessible fields", without the name. The literal pattern gives `E0027 … missing field`, so I kept it and the rows sit beside it. The cost is that a pattern entry and its row can drift; the census test checks the rows against the kind tests, not against the pattern.
- **Classes returned as strings** (`"carried"` …) through an associated function, so no new item is exported from `editor/mod.rs`.
- **One test per kind of edit, not per field.** A kind is what a caller does; fields set by one call share a test (the table in section 2 lists them). Page order has three tests (remove, reorder, duplicate) because the identity check can fail three ways.
- **Narrower choice on the three companion-classed fields** (section 1): `refused`, with the reason said. I did not add terms to `incremental_refusal`.
- **Reason text as the "naming the kind".** The refusal carries only the reason sentence, so each test asserts that sentence; several kinds share one.
- **Fixtures** are built in the test file from `support::pdf_from_objects`, so the module carries no `rendering` gate. I found no feature gate on the APIs it calls by reading; I did not build without features to confirm.
- **No Dart battery added.** Every test's doc comment says why Dart cannot observe it (the census reads private fields; the per-kind tests are rows of that census).
- **Injection method.** A temporary environment switch and one build per file state, instead of a rebuild per term. It was removed before the patch was taken.
- **Disk.** The Rust test build filled the disk before `make test-ops-native` could compile the native library. I deleted `vendor/pdf_oxide/target` after the Rust summary was recorded and ran the Dart gate again.
- **Commit message.** AGENTS.md forbids AI co-author lines, so the commit has none.
