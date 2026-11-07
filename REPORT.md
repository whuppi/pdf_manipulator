# t-585 report — `docs/CAPABILITY_ROADMAP.md` for 6.0.0

Only `docs/CAPABILITY_ROADMAP.md` changes; it is carried by `t-585-pkg.patch`.
No test suite was run: the change is a doc, and no code, config or test moved.

## Choices where the plan left one open

1. **Submodule.** `vendor/pdf_oxide` was empty, so `host/dispatch.rs` could not be read. I ran `git submodule update --init vendor/pdf_oxide` (checked out at the pinned commit `9a349d1`). The gitlink is unchanged and `git status` shows nothing for it.
2. **`registerFallbackFont`.** The plan's door list omits it, but `lib/src/ops/pdf.dart` declares it, so it has a row in the doors table (Rust: `register_fallback_font`).
3. **Extra sub-tables.** `PdfEditor` has its own table (reads of staged state, `appendPages`, queued redactions, save, dispose) and `PdfFormFieldEditor` and `PdfPageBuilder` each have a sub-table. Save and `dispose` of the handles are rows because only a handle has them.
4. **Rust column for Dart-composed reads.** `text`, `bitmap` and `appendText` have no op of their own. The Rust column names the engine function they reach (`extract_text_framed`, `render_pages_streamed`, `PageOp::Inline`). The Rust column for `appendPages` and `merge` is `merge_document`, the engine function `handle_editor_merge_from` calls.
5. **Summary.** The old totals (119 done, 34 planned) did not match the old tables, which held one PLANNED row. The new totals are counted from the new tables: 133 done, 1 planned. Runtime, build and the later sections are not counted, as before.
6. **Runtime row.** "Three web I/O modes (JSPI / Atomics / OPFS)" became "Four web I/O modes (Atomics / JSPI / OPFS / Asyncify)", as the plan says.
7. **Deferred section.** The date is dropped from its heading (the plan bans dates). Names are renamed; meanings are unchanged.
8. **Test infrastructure gaps.** The "Field-action and link no-op" row named `PdfInvalidArgument` and removed calls. It now says that scripts and links ride with the field or text, so the unattached call cannot be written, and the engine still refuses an unattached `PageOp`. Status stays DONE. In the memory row `extractPages` is now "the page extract behind `split`"; `extractPages` remains the probe's own op name in `tool/memprobe`.
9. **Links.** `#binary-size--feature-trimming` pointed at no heading; it is now `#binary-size--trim` (the heading is "Binary size — trim").
10. **`DataAsset`.** The Build & distribution text named it, an SDK type that is not in the repo. It now reads "a plain data asset".
11. **Status notes.** The proofs (fixtures and values) are carried from the old notes with new names. I checked that each fixture still exists under `test/`. I did not re-run the batteries.

## 5.x rows folded into a 6.0.0 row (no row of their own)

| 5.x row | Folded into |
|---|---|
| PdfDoc: Title / Author / Subject / Keywords, Producer / Creator, Creation date | `metadata` |
| PdfDoc: Encryption info (`isEncrypted`, `encryptionAlgorithm`, `permissions`) | `encryption` |
| PdfDoc: Page list; Get page crop box | `pageInfo` |
| PdfDoc: Extract text | `extractText` and `text` |
| PdfDoc: Classify page | `classifyPages` |
| PdfDoc: Plan split by bookmarks | `bookmarkRanges` |
| PdfDoc: Has XFA; Analyze XFA | `xfa` |
| PdfDoc: Get form field value; Has form field | `formFields` |
| PdfDoc: Get page images (list metadata) | `PdfEditor.pageImages` |
| PdfEditor: Set/Get title, author, subject, keywords; Set producer, creation date; Get producer, creation date | `setMetadata` and `metadata` |
| PdfEditor: Scrub metadata | `sanitize` |
| PdfEditor: Rotate page; Rotate all pages | `rotatePages` |
| PdfEditor: Set page rotation | `setRotation` |
| PdfEditor: Get/Set page media box, Get/Set page crop box | `pageInfo`, `setMediaBox`, `setCropBox` |
| PdfEditor: Delete page | `removePages` |
| PdfEditor: Select pages | `keepPages`, `reorderPages` |
| PdfEditor: Merge from another PDF; Merge selective pages | `appendPages` |
| PdfEditor: Embed file; Embed file with options | `addAttachment` |
| PdfEditor: Flatten forms; on single page | `flattenForms` (`pages:`) |
| PdfEditor: Flatten all annotations; on single page | `flattenAnnotations` (`pages:`) |
| PdfEditor: Erase regions; Clear erase regions | `redact` (`overlay: none`) and `clearRedactions` |
| PdfEditor: Add redaction | `markRedaction` |
| PdfEditor: Resize image; Reposition image; Set image bounds | `setImageBounds` |
| PdfEditor: Set form field value, remove, and the other `setFormField…` | the `PdfFormFieldEditor` rows |
| PdfEditor: Page count; Version; Is modified | the `PdfEditor` rows |
| PdfEditor: Save; Incremental save | `PdfEditorHandle.save`, `PdfSave.full`, `PdfSave.incremental`, `PdfSave.incrementalOrRewrite` |
| PdfEditor: Export form data FDF; XFDF | one `exportFormData` row on `PdfDocument` |
| PdfBuilder: Field keystroke / format / validate / calculate | the `scripts:` argument of `textField`, `checkbox`, `comboBox`, `radioGroup` |
| PdfBuilder: Link URL; Link page | the `link:` argument of `text` and `appendText` |
| PdfBuilder: Font; Newline; Horizontal rule … | kept, renamed (`setFont`, `moveTo`) |
| PdfStandalone: Convert to format; Convert to PDF | `convertToOffice`; `convertFromOffice` |
| PdfStandalone: Extract pages | `keepPages` (edit) and `split` (door) |
| PdfSugar: `splitBySize`, `splitByBookmarks` | `split` |
| PdfSugar: `extractPages`, `deletePages`, `reorderPages`, `movePage`, `rotatePages`, `rotateAllPages`, `flattenForms`, `applyRedactions`, `embedFile`, `eraseRegions`, `addStamp`, `addImageStamp`, `watermark`, `convertToPdfA` | the edit rows, reached through `editOnce` or `rewrite` |
| PdfSugar: `encrypt`, `decrypt` | `resave` with `PdfSave.full(encryption:)` |

Removed with no replacement call, so no row: Page done, New page same size (MIGRATION: a page ends when the next is added).

## Proof script 1 — every backticked token

`/tmp/claude-0/-home-user-repo/d9f648ce-dbc8-50fb-8cf2-640ba0bd0e5d/scratchpad/check_names.py` (scratch, not committed). It pulls every backticked token from the doc, greps each identifier in `lib`, `vendor/pdf_oxide/src/host`, `vendor/pdf_oxide/src`, `tool`, `hook`, `build.json`, `Makefile` and `test` (never `docs/` or `README.md`, so the doc cannot prove itself), resolves file names with `find`, and checks each `#anchor` and relative link against the headings.

```python
import re, subprocess, sys, os
doc = "docs/CAPABILITY_ROADMAP.md"
text = open(doc).read()
tokens = sorted(set(re.findall(r"`([^`\n]+)`", text)))
def grep(pat, *paths):
    r = subprocess.run(["grep", "-rqF", "--exclude-dir=.git", "--", pat, *paths], capture_output=True)
    return r.returncode == 0
found, missing = [], []
for t in tokens:
    # path-like
    if re.fullmatch(r"[\w./-]+\.(dart|rs|md|json|sh|yaml)", t):
        base = os.path.basename(t)
        hit = subprocess.run(["find", ".", "-path", "./.git", "-prune", "-o", "-name", base, "-print"], capture_output=True, text=True).stdout.strip()
        (found if hit or os.path.exists(t) else missing).append((t, "file " + (hit.split("\n")[0] if hit else "")))
        continue
    # identifiers inside the token
    ids = re.findall(r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*", t)
    ids = [i for i in ids if len(i) > 2]
    if not ids:
        continue
    ok_all = True; where = []
    for i in ids:
        ws = [w for w in ("lib", "vendor/pdf_oxide/src/host", "vendor/pdf_oxide/src", "tool", "hook", "build.json", "Makefile", "test") if os.path.exists(w)]
        w = next((w for w in ws if grep(i, w)), None)
        if w is None:
            ok_all = False; where.append(i + " NOT FOUND")
        else:
            where.append(f"{i} in {w}")
    (found if ok_all else missing).append((t, "; ".join(where)))
print("FOUND", len(found))
for t, w in found: print("  ok  ", t, "->", w)
print("MISSING", len(missing))
for t, w in missing: print("  MISS", t, "->", w)
# anchors
heads = {}
for line in text.split("\n"):
    m = re.match(r"^#+ (.*)", line)
    if m:
        a = re.sub(r"[^\w\- ]", "", m.group(1).lower()).replace(" ", "-")
        heads[a] = 1
bad = 0
for m in re.finditer(r"\]\(([^)]+)\)", text):
    l = m.group(1)
    if l.startswith("http"): continue
    path, _, anc = l.partition("#")
    if path:
        p = os.path.join("docs", path)
        if not os.path.exists(p): print("BAD LINK FILE", l); bad += 1; continue
        if anc:
            t2 = open(p).read(); h2 = set()
            for line in t2.split("\n"):
                mm = re.match(r"^#+ (.*)", line)
                if mm: h2.add(re.sub(r"[^\w\- ]", "", mm.group(1).lower()).replace(" ", "-"))
            if anc not in h2: print("BAD ANCHOR", l); bad += 1
    elif anc not in heads: print("BAD ANCHOR", l); bad += 1
print("links bad:", bad)
```

Output:

```
FOUND 444
  ok   'Ada' -> Ada in vendor/pdf_oxide/src/host
  ok   .bookmarks -> bookmarks in lib
  ok   .maxBytes -> maxBytes in lib
  ok   .none() -> none in lib
  ok   /AcroForm /Fields -> AcroForm in lib; Fields in lib
  ok   /Annots -> Annots in lib
  ok   /CropBox -> CropBox in lib
  ok   /MaxLen -> MaxLen in lib
  ok   /PageLabels -> PageLabels in vendor/pdf_oxide/src/host
  ok   /Rect -> Rect in lib
  ok   /T (fullname) /V (Ada) -> fullname in test; Ada in vendor/pdf_oxide/src/host
  ok   /{font} {size} Tf {r} {g} {b} rg -> font in lib; size in lib
  ok   <field name="fullname"><value>Ada</value></field> -> field in lib; name in lib; fullname in test; value in lib; Ada in vendor/pdf_oxide/src/host; value in lib; field in lib
  ok   ARCHITECTURE.md -> file ./vendor/pdf_oxide/docs/ARCHITECTURE.md
  ok   ArgumentError -> ArgumentError in lib
  ok   CARGO_PROFILE_RELEASE_* -> CARGO_PROFILE_RELEASE_ in lib
  ok   CodeAsset -> CodeAsset in hook
  ok   DataSink -> DataSink in lib
  ok   DataSource -> DataSource in lib
  ok   DateTime? -> DateTime in lib
  ok   FdfWriter::from_fields -> FdfWriter::from_fields in vendor/pdf_oxide/src/host
  ok   JsAsset -> JsAsset in hook
  ok   MARKER-TWO -> MARKER in vendor/pdf_oxide/src; TWO in vendor/pdf_oxide/src/host
  ok   MAX_EXECUTORS_GLOBAL -> MAX_EXECUTORS_GLOBAL in vendor/pdf_oxide/src/host
  ok   MemorySink -> MemorySink in lib
  ok   PageOp -> PageOp in lib
  ok   PageOp::At -> PageOp::At in vendor/pdf_oxide/src/host
  ok   PageOp::Checkbox -> PageOp::Checkbox in vendor/pdf_oxide/src/host
  ok   PageOp::Columns -> PageOp::Columns in vendor/pdf_oxide/src/host
  ok   PageOp::ComboBox -> PageOp::ComboBox in vendor/pdf_oxide/src/host
  ok   PageOp::FilledRect -> PageOp::FilledRect in vendor/pdf_oxide/src/host
  ok   PageOp::Font -> PageOp::Font in vendor/pdf_oxide/src/host
  ok   PageOp::Footnote -> PageOp::Footnote in vendor/pdf_oxide/src/host
  ok   PageOp::FreeText -> PageOp::FreeText in vendor/pdf_oxide/src/host
  ok   PageOp::Heading -> PageOp::Heading in vendor/pdf_oxide/src/host
  ok   PageOp::HorizontalRule -> PageOp::HorizontalRule in vendor/pdf_oxide/src/host
  ok   PageOp::Image -> PageOp::Image in vendor/pdf_oxide/src/host
  ok   PageOp::ImageArtifact -> PageOp::ImageArtifact in vendor/pdf_oxide/src/host
  ok   PageOp::Inline -> PageOp::Inline in vendor/pdf_oxide/src/host
  ok   PageOp::Newline -> PageOp::Newline in vendor/pdf_oxide/src/host
  ok   PageOp::OnClose -> PageOp::OnClose in vendor/pdf_oxide/src/host
  ok   PageOp::OnOpen -> PageOp::OnOpen in vendor/pdf_oxide/src/host
  ok   PageOp::Paragraph -> PageOp::Paragraph in vendor/pdf_oxide/src/host
  ok   PageOp::PushButton -> PageOp::PushButton in vendor/pdf_oxide/src/host
  ok   PageOp::RadioGroup -> PageOp::RadioGroup in vendor/pdf_oxide/src/host
  ok   PageOp::SignatureField -> PageOp::SignatureField in vendor/pdf_oxide/src/host
  ok   PageOp::Space -> PageOp::Space in vendor/pdf_oxide/src/host
  ok   PageOp::Stamp -> PageOp::Stamp in vendor/pdf_oxide/src/host
  ok   PageOp::StickyNote -> PageOp::StickyNote in vendor/pdf_oxide/src/host
  ok   PageOp::StreamingTableBlock -> PageOp::StreamingTableBlock in vendor/pdf_oxide/src/host
  ok   PageOp::StrokeLine -> PageOp::StrokeLine in vendor/pdf_oxide/src/host
  ok   PageOp::StrokeRect -> PageOp::StrokeRect in vendor/pdf_oxide/src/host
  ok   PageOp::StyledText -> PageOp::StyledText in vendor/pdf_oxide/src/host
  ok   PageOp::Text -> PageOp::Text in vendor/pdf_oxide/src/host
  ok   PageOp::TextField -> PageOp::TextField in vendor/pdf_oxide/src/host
  ok   PageOp::TextInRect -> PageOp::TextInRect in vendor/pdf_oxide/src/host
  ok   PageOp::Watermark -> PageOp::Watermark in vendor/pdf_oxide/src/host
  ok   Pdf -> Pdf in lib
  ok   Pdf.build -> Pdf in lib; build in lib
  ok   Pdf.editOnce -> Pdf in lib; editOnce in lib
  ok   Pdf.readOnce -> Pdf in lib; readOnce in lib
  ok   Pdf.rewrite -> Pdf in lib; rewrite in lib
  ok   PdfALevel -> PdfALevel in lib
  ok   PdfAlignment.center -> PdfAlignment in lib; center in lib
  ok   PdfBuilder -> PdfBuilder in lib
  ok   PdfBuilderHandle -> PdfBuilderHandle in lib
  ok   PdfBuilderHandle.dispose -> PdfBuilderHandle in lib; dispose in lib
  ok   PdfBuilderHandle.save -> PdfBuilderHandle in lib; save in lib
  ok   PdfDocument -> PdfDocument in lib
  ok   PdfDocument.annotations -> PdfDocument in lib; annotations in lib
  ok   PdfDocument.attachments -> PdfDocument in lib; attachments in lib
  ok   PdfDocument.extractAttachment -> PdfDocument in lib; extractAttachment in lib
  ok   PdfDocumentHandle -> PdfDocumentHandle in lib
  ok   PdfEditor -> PdfEditor in lib
  ok   PdfEditorHandle -> PdfEditorHandle in lib
  ok   PdfEditorHandle.dispose -> PdfEditorHandle in lib; dispose in lib
  ok   PdfEditorHandle.save -> PdfEditorHandle in lib; save in lib
  ok   PdfEdits -> PdfEdits in lib
  ok   PdfEncryption.keep() -> PdfEncryption in lib; keep in lib
  ok   PdfEncryption.password -> PdfEncryption in lib; password in lib
  ok   PdfEncryption.remove() -> PdfEncryption in lib; remove in lib
  ok   PdfEncryptionInfo -> PdfEncryptionInfo in lib
  ok   PdfEngineFailed -> PdfEngineFailed in lib
  ok   PdfError -> PdfError in lib
  ok   PdfFieldScripts -> PdfFieldScripts in lib
  ok   PdfFormDataFormat.fdf -> PdfFormDataFormat in lib; fdf in lib
  ok   PdfFormFieldEditor -> PdfFormFieldEditor in lib
  ok   PdfImagePage -> PdfImagePage in lib
  ok   PdfImagePolicy.lossless -> PdfImagePolicy in lib; lossless in lib
  ok   PdfImageReport -> PdfImageReport in lib
  ok   PdfIncrementalRefused -> PdfIncrementalRefused in lib
  ok   PdfInput -> PdfInput in lib
  ok   PdfLink -> PdfLink in lib
  ok   PdfMarkup -> PdfMarkup in lib
  ok   PdfMetadata -> PdfMetadata in lib
  ok   PdfNotFound -> PdfNotFound in lib
  ok   PdfPageBuilder -> PdfPageBuilder in lib
  ok   PdfPageOrder -> PdfPageOrder in lib
  ok   PdfPageOutOfRange -> PdfPageOutOfRange in lib
  ok   PdfPageSize -> PdfPageSize in lib
  ok   PdfPoint -> PdfPoint in lib
  ok   PdfReads -> PdfReads in lib
  ok   PdfRect -> PdfRect in lib
  ok   PdfRedactionOverlay -> PdfRedactionOverlay in lib
  ok   PdfRedactionReport -> PdfRedactionReport in lib
  ok   PdfSave -> PdfSave in lib
  ok   PdfSave.full() -> PdfSave in lib; full in lib
  ok   PdfSave.full(encryption:) -> PdfSave in lib; full in lib; encryption in lib
  ok   PdfSave.incremental() -> PdfSave in lib; incremental in lib
  ok   PdfSave.incrementalOrRewrite() -> PdfSave in lib; incrementalOrRewrite in lib
  ok   PdfSplitRule.every -> PdfSplitRule in lib; every in lib
  ok   PdfStamp -> PdfStamp in lib
  ok   PdfTask.cancel() -> PdfTask in lib; cancel in lib
  ok   PdfTextStyle -> PdfTextStyle in lib
  ok   PdfUnsupported -> PdfUnsupported in lib
  ok   Set<PdfFormFieldFlag> -> Set in lib; PdfFormFieldFlag in lib
  ok   StateError -> StateError in lib
  ok   StaticLinking -> StaticLinking in lib
  ok   UPDATING.md -> file ./docs/UPDATING.md
  ok   WasmAsset -> WasmAsset in hook
  ok   XfdfWriter::from_fields -> XfdfWriter::from_fields in vendor/pdf_oxide/src/host
  ok   ['Checkbox', 'Text'] -> Checkbox in lib; Text in lib
  ok   [list] -> list in lib
  ok   _ERR_UNSUPPORTED -> _ERR_UNSUPPORTED in vendor/pdf_oxide/src
  ok   addAttachment -> addAttachment in lib
  ok   addImagePage -> addImagePage in lib
  ok   addImageStamp -> addImageStamp in lib
  ok   addPage -> addPage in lib
  ok   addStamp -> addStamp in lib
  ok   addWatermark -> addWatermark in lib
  ok   agree -> agree in lib
  ok   algorithm -> algorithm in lib
  ok   alignment -> alignment in lib
  ok   altText -> altText in lib
  ok   annotations -> annotations in lib
  ok   appendPages -> appendPages in lib
  ok   appendText -> appendText in lib
  ok   applyRedactions -> applyRedactions in lib
  ok   apply_page_op -> apply_page_op in vendor/pdf_oxide/src/host
  ok   attachmentPdf -> attachmentPdf in test
  ok   attachments -> attachments in lib
  ok   auto -> auto in lib
  ok   bitmap -> bitmap in lib
  ok   bookmarkRanges -> bookmarkRanges in lib
  ok   bounds -> bounds in lib
  ok   build -> build in lib
  ok   build.json -> file ./build.json
  ok   build: -> build in lib
  ok   build: size -> build in lib; size in lib
  ok   buildTo -> buildTo in lib
  ok   catch_unwind -> catch_unwind in lib
  ok   checkbox -> checkbox in lib
  ok   classifyDocument -> classifyDocument in lib
  ok   classifyPages -> classifyPages in lib
  ok   classify_document -> classify_document in vendor/pdf_oxide/src/host
  ok   classify_page -> classify_page in vendor/pdf_oxide/src/host
  ok   clearRedactions -> clearRedactions in lib
  ok   columns -> columns in lib
  ok   comboBox -> comboBox in lib
  ok   compress -> compress in lib
  ok   convertFromOffice -> convertFromOffice in lib
  ok   convertToOffice -> convertToOffice in lib
  ok   convertToPdfA -> convertToPdfA in lib
  ok   convertXfaToAcroForm -> convertXfaToAcroForm in lib
  ok   convert_from_format_writer -> convert_from_format_writer in vendor/pdf_oxide/src/host
  ok   convert_to_format_writer -> convert_to_format_writer in vendor/pdf_oxide/src/host
  ok   convert_xfa_to_acroform -> convert_xfa_to_acroform in vendor/pdf_oxide/src/host
  ok   creationDate -> creationDate in lib
  ok   cropBox -> cropBox in lib
  ok   cropMargins -> cropMargins in lib
  ok   croppedPagePdf -> croppedPagePdf in test
  ok   deflate: false -> deflate in lib; false in lib
  ok   detector -> detector in lib
  ok   dispose() -> dispose in lib
  ok   doc.render(...) -> doc in lib; render in lib
  ok   doc_annotations -> doc_annotations in vendor/pdf_oxide/src/host
  ok   doc_attachments -> doc_attachments in vendor/pdf_oxide/src/host
  ok   doc_export_form_data -> doc_export_form_data in vendor/pdf_oxide/src/host
  ok   doc_extract_attachment -> doc_extract_attachment in vendor/pdf_oxide/src/host
  ok   doc_form_fields -> doc_form_fields in vendor/pdf_oxide/src/host
  ok   doc_xfa -> doc_xfa in vendor/pdf_oxide/src/host
  ok   ebook -> ebook in lib
  ok   edit -> edit in lib
  ok   editOnce -> editOnce in lib
  ok   edit_add_image_stamp -> edit_add_image_stamp in vendor/pdf_oxide/src/host
  ok   edit_add_redaction -> edit_add_redaction in vendor/pdf_oxide/src/host
  ok   edit_add_stamp -> edit_add_stamp in vendor/pdf_oxide/src/host
  ok   edit_apply_redactions -> edit_apply_redactions in vendor/pdf_oxide/src/host
  ok   edit_clear_form_field_value -> edit_clear_form_field_value in vendor/pdf_oxide/src/host
  ok   edit_clear_redactions -> edit_clear_redactions in vendor/pdf_oxide/src/host
  ok   edit_convert_to_pdf_a -> edit_convert_to_pdf_a in vendor/pdf_oxide/src/host
  ok   edit_crop_margins -> edit_crop_margins in vendor/pdf_oxide/src/host
  ok   edit_delete_pages -> edit_delete_pages in vendor/pdf_oxide/src/host
  ok   edit_embed_file -> edit_embed_file in vendor/pdf_oxide/src/host
  ok   edit_flatten_annotations -> edit_flatten_annotations in vendor/pdf_oxide/src/host
  ok   edit_flatten_forms -> edit_flatten_forms in vendor/pdf_oxide/src/host
  ok   edit_get_metadata -> edit_get_metadata in vendor/pdf_oxide/src/host
  ok   edit_is_modified -> edit_is_modified in vendor/pdf_oxide/src/host
  ok   edit_move_page -> edit_move_page in vendor/pdf_oxide/src/host
  ok   edit_page_images -> edit_page_images in vendor/pdf_oxide/src/host
  ok   edit_page_info -> edit_page_info in vendor/pdf_oxide/src/host
  ok   edit_redact -> edit_redact in vendor/pdf_oxide/src/host
  ok   edit_redaction_count -> edit_redaction_count in vendor/pdf_oxide/src/host
  ok   edit_reduce_images -> edit_reduce_images in vendor/pdf_oxide/src/host
  ok   edit_remove_form_field -> edit_remove_form_field in vendor/pdf_oxide/src/host
  ok   edit_rotate_pages -> edit_rotate_pages in vendor/pdf_oxide/src/host
  ok   edit_sanitize -> edit_sanitize in vendor/pdf_oxide/src/host
  ok   edit_save_with_options -> edit_save_with_options in vendor/pdf_oxide/src/host
  ok   edit_select_pages -> edit_select_pages in vendor/pdf_oxide/src/host
  ok   edit_set_checkbox_field_value -> edit_set_checkbox_field_value in vendor/pdf_oxide/src/host
  ok   edit_set_crop_box -> edit_set_crop_box in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_alignment -> edit_set_form_field_alignment in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_appearance -> edit_set_form_field_appearance in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_background_color -> edit_set_form_field_background_color in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_border_color -> edit_set_form_field_border_color in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_border_width -> edit_set_form_field_border_width in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_bounds -> edit_set_form_field_bounds in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_choice -> edit_set_form_field_choice in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_choices -> edit_set_form_field_choices in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_flags -> edit_set_form_field_flags in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_max_length -> edit_set_form_field_max_length in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_readonly -> edit_set_form_field_readonly in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_required -> edit_set_form_field_required in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_tooltip -> edit_set_form_field_tooltip in vendor/pdf_oxide/src/host
  ok   edit_set_form_field_value -> edit_set_form_field_value in vendor/pdf_oxide/src/host
  ok   edit_set_image_bounds -> edit_set_image_bounds in vendor/pdf_oxide/src/host
  ok   edit_set_media_box -> edit_set_media_box in vendor/pdf_oxide/src/host
  ok   edit_set_metadata -> edit_set_metadata in vendor/pdf_oxide/src/host
  ok   edit_set_rotation -> edit_set_rotation in vendor/pdf_oxide/src/host
  ok   edit_unembed_standard_fonts -> edit_unembed_standard_fonts in vendor/pdf_oxide/src/host
  ok   edit_watermark -> edit_watermark in vendor/pdf_oxide/src/host
  ok   editor.field(name) -> editor in lib; field in lib; name in lib
  ok   encryption -> encryption in lib
  ok   example_trimmed/ -> example_trimmed in Makefile
  ok   exportFormData -> exportFormData in lib
  ok   extract -> extract in lib
  ok   extractAttachment -> extractAttachment in lib
  ok   extractImages -> extractImages in lib
  ok   extractText -> extractText in lib
  ok   extract_images_streamed -> extract_images_streamed in vendor/pdf_oxide/src/host
  ok   extract_text_framed -> extract_text_framed in vendor/pdf_oxide/src/host
  ok   fFormFields -> fFormFields in test
  ok   fFormFieldsProps -> fFormFieldsProps in test
  ok   fThreePageMarkers -> fThreePageMarkers in test
  ok   fieldCount -> fieldCount in lib
  ok   fieldTypes -> fieldTypes in lib
  ok   flattenAnnotations -> flattenAnnotations in lib
  ok   flattenForms -> flattenForms in lib
  ok   flutter build -> flutter in lib; build in lib
  ok   flutter pub run pdf_manipulator:setup -> flutter in lib; pub in lib; run in lib; pdf_manipulator in lib; setup in lib
  ok   footnote -> footnote in lib
  ok   formFields -> formFields in lib
  ok   freeText -> freeText in lib
  ok   fullname -> fullname in test
  ok   get_signatures -> get_signatures in vendor/pdf_oxide/src/host
  ok   given -> given in lib
  ok   glyphsRemoved > 0 -> glyphsRemoved in lib
  ok   handle_builder_add_image_page -> handle_builder_add_image_page in vendor/pdf_oxide/src/host
  ok   handle_builder_add_page -> handle_builder_add_page in vendor/pdf_oxide/src/host
  ok   handle_builder_create -> handle_builder_create in vendor/pdf_oxide/src/host
  ok   handle_builder_save -> handle_builder_save in vendor/pdf_oxide/src/host
  ok   handle_builder_set_metadata -> handle_builder_set_metadata in vendor/pdf_oxide/src/host
  ok   handle_editor_extract_pages -> handle_editor_extract_pages in vendor/pdf_oxide/src/host
  ok   handle_editor_open -> handle_editor_open in vendor/pdf_oxide/src/host
  ok   handle_sign -> handle_sign in vendor/pdf_oxide/src/host
  ok   heading -> heading in lib
  ok   hello notes -> hello in lib; notes in vendor/pdf_oxide/src/host
  ok   hook/build.dart -> file ./hook/build.dart
  ok   hook/link.dart -> file ./hook/link.dart
  ok   hooks: user_defines: -> hooks in lib; user_defines in lib
  ok   horizontalRule -> horizontalRule in lib
  ok   host/incremental.rs -> file ./vendor/pdf_oxide/src/host/incremental.rs
  ok   image -> image in lib
  ok   imagesToPdf -> imagesToPdf in lib
  ok   info -> info in lib
  ok   isModified -> isModified in lib
  ok   isReadOnly -> isReadOnly in lib
  ok   isRequired -> isRequired in lib
  ok   isTagged -> isTagged in lib
  ok   kMaxExecutors -> kMaxExecutors in lib
  ok   keep -> keep in lib
  ok   keep: -> keep in lib
  ok   keep: [render] -> keep in lib; render in lib
  ok   keep: auto -> keep in lib; auto in lib
  ok   keepPages -> keepPages in lib
  ok   label -> label in lib
  ok   lib/src/hook/engine_build.dart -> file ./lib/src/hook/engine_build.dart
  ok   lib/src/hook/pdf_config.dart -> file ./lib/src/types/pdf_config.dart
  ok   lib/src/hook/user_defines.dart -> file ./lib/src/hook/user_defines.dart
  ok   lib/src/keep/capabilities.dart -> file ./lib/src/keep/capabilities.dart
  ok   lib/src/keep/detector.dart -> file ./lib/src/keep/detector.dart
  ok   line -> line in lib
  ok   link: -> link in lib
  ok   lossless -> lossless in lib
  ok   main() -> main in lib
  ok   make check -> make in lib; check in lib
  ok   make shake-audit -> make in lib; shake in lib; audit in vendor/pdf_oxide/src/host
  ok   make test-example-trimmed -> make in lib; test in lib; example in lib; trimmed in lib
  ok   make test-memory -> make in lib; test in lib; memory in lib
  ok   markRedaction -> markRedaction in lib
  ok   markup: -> markup in lib
  ok   maxLength -> maxLength in lib
  ok   merge -> merge in lib
  ok   merge_document -> merge_document in vendor/pdf_oxide/src/host
  ok   meta ^1.19 -> meta in lib
  ok   metadata -> metadata in lib
  ok   minimalPdf -> minimalPdf in test
  ok   movePage -> movePage in lib
  ok   moveTo -> moveTo in lib
  ok   newline -> newline in lib
  ok   null -> null in lib
  ok   ocr -> ocr in vendor/pdf_oxide/src
  ok   ocr-tract -> ocr in vendor/pdf_oxide/src; tract in lib
  ok   office -> office in lib
  ok   onClose -> onClose in lib
  ok   onOpen -> onOpen in lib
  ok   opacity -> opacity in lib
  ok   open -> open in lib
  ok   open_document -> open_document in vendor/pdf_oxide/src/host
  ok   ops/registry.rs -> file ./vendor/pdf_oxide/src/host/ops/registry.rs
  ok   opt-level=z -> opt in lib; level in lib
  ok   pageCount -> pageCount in lib
  ok   pageImages -> pageImages in lib
  ok   pageInfo -> pageInfo in lib
  ok   page_info -> page_info in lib
  ok   pages -> pages in lib
  ok   panic=abort -> panic in lib; abort in lib
  ok   panic=unwind -> panic in lib; unwind in lib
  ok   paragraph -> paragraph in lib
  ok   pdf.dart -> file ./lib/src/ops/pdf.dart
  ok   pdf_builder.dart -> file ./lib/src/ops/pdf_builder.dart
  ok   pdf_doc.dart -> file ./lib/src/ops/pdf_doc.dart
  ok   pdf_editor.dart -> file ./lib/src/ops/pdf_editor.dart
  ok   pdf_edits.dart -> file ./lib/src/ops/pdf_edits.dart
  ok   pdf_jobs.dart -> file ./lib/src/ops/pdf_jobs.dart
  ok   pdf_manipulator -> pdf_manipulator in lib
  ok   pdf_op_<name>_anchor -> pdf_op_ in vendor/pdf_oxide/src/host; name in lib; _anchor in lib
  ok   pdf_oxide -> pdf_oxide in lib
  ok   pdf_reads.dart -> file ./lib/src/ops/pdf_reads.dart
  ok   permissions -> permissions in lib
  ok   plan_split_by_bookmarks -> plan_split_by_bookmarks in vendor/pdf_oxide/src/host
  ok   postMessage -> postMessage in lib
  ok   print -> print in lib
  ok   pushButton -> pushButton in lib
  ok   radioGroup -> radioGroup in lib
  ok   read -> read in lib
  ok   readOnce -> readOnce in lib
  ok   record_use -> record_use in lib
  ok   record_use_shim*.dart -> record_use_shim in lib; dart in lib
  ok   rect -> rect in lib
  ok   redact -> redact in lib
  ok   redact(page, regions, overlay: const PdfRedactionOverlay.none()) -> redact in lib; page in lib; regions in lib; overlay in lib; const in lib; PdfRedactionOverlay in lib; none in lib
  ok   redactionCount -> redactionCount in lib
  ok   reduceImages -> reduceImages in lib
  ok   registerFallbackFont -> registerFallbackFont in lib
  ok   registerFallbackFont(bytes) -> registerFallbackFont in lib; bytes in lib
  ok   register_fallback_font -> register_fallback_font in vendor/pdf_oxide/src/host
  ok   release -> release in lib
  ok   release-small -> release in lib; small in lib
  ok   remove -> remove in lib
  ok   removePages -> removePages in lib
  ok   render -> render in lib
  ok   render_pages_streamed -> render_pages_streamed in vendor/pdf_oxide/src/host
  ok   reorderPages -> reorderPages in lib
  ok   requiresPassword -> requiresPassword in lib
  ok   resave -> resave in lib
  ok   resolveKeepPlan -> resolveKeepPlan in lib
  ok   resolveWeb() -> resolveWeb in hook
  ok   rewrite -> rewrite in lib
  ok   rotatePages -> rotatePages in lib
  ok   sanitize -> sanitize in lib
  ok   sanitize(stripJavaScript: false) -> sanitize in lib; stripJavaScript in lib; false in lib
  ok   screen -> screen in lib
  ok   scripts: -> scripts in lib
  ok   search -> search in lib
  ok   search_text -> search_text in vendor/pdf_oxide/src/host
  ok   setAlignment -> setAlignment in lib
  ok   setAppearance -> setAppearance in lib
  ok   setBackgroundColor -> setBackgroundColor in lib
  ok   setBorderColor -> setBorderColor in lib
  ok   setBorderWidth -> setBorderWidth in lib
  ok   setBounds -> setBounds in lib
  ok   setCropBox -> setCropBox in lib
  ok   setFlags -> setFlags in lib
  ok   setFont -> setFont in lib
  ok   setImageBounds -> setImageBounds in lib
  ok   setMaxLength -> setMaxLength in lib
  ok   setMediaBox -> setMediaBox in lib
  ok   setMetadata -> setMetadata in lib
  ok   setMetadata(await editor.metadata()) -> setMetadata in lib; await in lib; editor in lib; metadata in lib
  ok   setReadOnly -> setReadOnly in lib
  ok   setRequired -> setRequired in lib
  ok   setRotation -> setRotation in lib
  ok   setTooltip -> setTooltip in lib
  ok   setValue(PdfFormFieldValue.checked(...)) -> setValue in lib; PdfFormFieldValue in lib; checked in lib
  ok   setValue(PdfFormFieldValue.choice(...)) -> setValue in lib; PdfFormFieldValue in lib; choice in lib
  ok   setValue(PdfFormFieldValue.choices(...)) -> setValue in lib; PdfFormFieldValue in lib; choices in lib
  ok   setValue(PdfFormFieldValue.none()) -> setValue in lib; PdfFormFieldValue in lib; none in lib
  ok   setValue(PdfFormFieldValue.text(...)) -> setValue in lib; PdfFormFieldValue in lib; text in lib
  ok   setup -> setup in lib
  ok   setup --force web -> setup in lib; force in lib; web in lib
  ok   setup <target> -> setup in lib; target in lib
  ok   setup web -> setup in lib; web in lib
  ok   sign -> sign in lib
  ok   signatureField -> signatureField in lib
  ok   signatures -> signatures in lib
  ok   sinkFor -> sinkFor in lib
  ok   space -> space in lib
  ok   spatial_table_detector -> spatial_table_detector in vendor/pdf_oxide/src/host
  ok   split -> split in lib
  ok   stamp -> stamp in lib
  ok   stickyNote -> stickyNote in lib
  ok   stripAttachments -> stripAttachments in lib
  ok   stripJavaScript -> stripJavaScript in lib
  ok   stripMetadata -> stripMetadata in lib
  ok   table -> table in lib
  ok   table-ml -> table in lib
  ok   table_extractor -> table_extractor in vendor/pdf_oxide/src
  ok   text -> text in lib
  ok   text/plain -> text in lib; plain in lib
  ok   textField -> textField in lib
  ok   textIn -> textIn in lib
  ok   tooltip -> tooltip in lib
  ok   trim-detector: compare -> trim in lib; detector in lib; compare in lib
  ok   true -> true in lib
  ok   unembedStandardFonts -> unembedStandardFonts in lib
  ok   user_defines -> user_defines in lib
  ok   user_defines.dart -> file ./lib/src/hook/user_defines.dart
  ok   validatePdfA -> validatePdfA in lib
  ok   validatePdfUa -> validatePdfUa in lib
  ok   validate_pdf_a -> validate_pdf_a in vendor/pdf_oxide/src/host
  ok   validate_pdf_ua -> validate_pdf_ua in vendor/pdf_oxide/src/host
  ok   value -> value in lib
  ok   vendor/pdf_oxide/src/host/ops/ -> vendor in lib; pdf_oxide in lib; src in lib; host in lib; ops in lib
  ok   verifySignatures -> verifySignatures in lib
  ok   verify_signatures -> verify_signatures in vendor/pdf_oxide/src/host
  ok   version -> version in lib
  ok   wasm-ocr -> wasm in lib; ocr in vendor/pdf_oxide/src
  ok   watermark -> watermark in lib
  ok   xfa -> xfa in lib
  ok   xfaFormPdf -> xfaFormPdf in test
  ok   xfdf -> xfdf in lib
  ok   {multiline} -> multiline in lib
  ok   {readOnly, required} -> readOnly in lib; required in lib
MISSING 0
links bad: 0
```

## Proof script 2 — the Rust and Dart columns, row by row

Scratch `check_columns.py`. Each Rust cell must be a `fn`, `enum` or `struct` under `vendor/pdf_oxide/src` (a `PageOp::X` must appear in `host/`; `host/incremental.rs` must exist). Each Dart cell must be declared in `lib/src/ops` or `lib/src/types`.

```python
import re, subprocess
text = open("docs/CAPABILITY_ROADMAP.md").read()
def sh(cmd): return subprocess.run(cmd, shell=True, capture_output=True, text=True).stdout
bad = 0; n = 0
for line in text.split("\n"):
    c = [x.strip() for x in line.strip().strip("|").split("|")]
    if not line.startswith("|") or len(c) != 4 or c[0] in ("Capability",) or set(c[0]) <= set("-"): continue
    rust, dart = c[1], c[2]
    for tok in re.findall(r"`([^`]+)`", rust):
        if tok in ("host/incremental.rs",):
            ok = bool(sh("ls vendor/pdf_oxide/src/host/incremental.rs")); label = tok
        elif "::" in tok:
            ok = bool(sh(f"grep -rF -- '{tok}' vendor/pdf_oxide/src/host | head -1")); label = tok
        else:
            ok = bool(sh(f"grep -rE -- '(fn|enum|struct) {tok}\\b' vendor/pdf_oxide/src | head -1")); label = "fn " + tok
        n += 1
        print(("ok   " if ok else "MISS ") + "rust", label)
        bad += not ok
    for tok in re.findall(r"`([^`]+)`", dart):
        first = re.match(r"(?:(\w+)\.)?(\w+)", tok)
        owner, name = first.group(1), first.group(2)
        if name == "Pdf" and owner is None:
            continue
        pat = rf"(^|[ .>])(get )?{name}\b *(\(|=|;|,|<)|final [\w<>?, ]+ {name};|\b{name}\(|\b{name}\b *[,;(]"
        decl = sh(f"grep -rEn -- '{pat}' lib/src/ops lib/src/types | head -1")
        n += 1
        print(("ok   " if decl else "MISS ") + "dart", tok)
        bad += not decl
print("checked", n, "missing", bad)
```

Output:

```
ok   rust fn open_document
ok   dart pageCount
ok   rust fn open_document
ok   dart version
ok   rust fn open_document
ok   dart metadata
ok   rust fn open_document
ok   dart encryption
ok   rust fn open_document
ok   dart isTagged
ok   rust fn page_info
ok   dart pageInfo
ok   rust fn extract_text_framed
ok   dart extractText
ok   rust fn extract_text_framed
ok   dart text
ok   rust fn search_text
ok   dart search
ok   rust fn render_pages_streamed
ok   dart render
ok   rust fn render_pages_streamed
ok   dart bitmap
ok   rust fn extract_images_streamed
ok   dart extractImages
ok   rust fn get_signatures
ok   dart signatures
ok   rust fn verify_signatures
ok   dart verifySignatures
ok   rust fn validate_pdf_a
ok   dart validatePdfA
ok   rust fn validate_pdf_ua
ok   dart validatePdfUa
ok   rust fn classify_page
ok   dart classifyPages
ok   rust fn classify_document
ok   dart classifyDocument
ok   rust fn plan_split_by_bookmarks
ok   dart bookmarkRanges
ok   rust fn doc_xfa
ok   dart xfa
ok   rust fn doc_attachments
ok   dart attachments
ok   rust fn doc_extract_attachment
ok   dart extractAttachment
ok   rust fn doc_annotations
ok   dart annotations
ok   rust fn doc_form_fields
ok   dart formFields
ok   rust fn doc_export_form_data
ok   dart exportFormData
ok   rust fn edit_set_metadata
ok   dart setMetadata
ok   rust fn edit_rotate_pages
ok   dart rotatePages
ok   rust fn edit_set_rotation
ok   dart setRotation
ok   rust fn edit_set_media_box
ok   dart setMediaBox
ok   rust fn edit_set_crop_box
ok   dart setCropBox
ok   rust fn edit_delete_pages
ok   dart removePages
ok   rust fn edit_select_pages
ok   dart keepPages
ok   rust fn edit_select_pages
ok   dart reorderPages
ok   rust fn edit_move_page
ok   dart movePage
ok   rust fn edit_watermark
ok   dart addWatermark
ok   rust fn edit_add_stamp
ok   dart addStamp
ok   rust fn edit_add_image_stamp
ok   dart addImageStamp
ok   rust fn edit_flatten_forms
ok   dart flattenForms
ok   rust fn edit_flatten_annotations
ok   dart flattenAnnotations
ok   rust fn edit_convert_to_pdf_a
ok   dart convertToPdfA
ok   rust fn edit_redact
ok   dart redact
ok   rust fn edit_reduce_images
ok   dart reduceImages
ok   rust fn edit_unembed_standard_fonts
ok   dart unembedStandardFonts
ok   rust fn edit_embed_file
ok   dart addAttachment
ok   rust fn edit_crop_margins
ok   dart cropMargins
ok   rust fn edit_set_image_bounds
ok   dart setImageBounds
ok   rust fn edit_sanitize
ok   dart sanitize
ok   rust fn edit_get_metadata
ok   dart pageCount
ok   rust fn edit_get_metadata
ok   dart version
ok   rust fn edit_is_modified
ok   dart isModified
ok   rust fn edit_get_metadata
ok   dart metadata
ok   rust fn edit_page_info
ok   dart pageInfo
ok   rust fn edit_page_images
ok   dart pageImages
ok   rust fn merge_document
ok   dart appendPages
ok   rust fn edit_add_redaction
ok   dart markRedaction
ok   rust fn edit_redaction_count
ok   dart redactionCount
ok   rust fn edit_apply_redactions
ok   dart applyRedactions
ok   rust fn edit_clear_redactions
ok   dart clearRedactions
ok   rust fn edit_save_with_options
ok   dart PdfEditorHandle.save
ok   rust fn edit_save_with_options
ok   dart PdfSave.full(encryption:)
ok   rust host/incremental.rs
ok   dart PdfSave.incremental()
ok   rust host/incremental.rs
ok   rust fn edit_save_with_options
ok   dart PdfSave.incrementalOrRewrite()
ok   dart PdfEditorHandle.dispose
ok   rust fn edit_set_form_field_value
ok   dart setValue(PdfFormFieldValue.text(...))
ok   rust fn edit_set_checkbox_field_value
ok   dart setValue(PdfFormFieldValue.checked(...))
ok   rust fn edit_set_form_field_choice
ok   dart setValue(PdfFormFieldValue.choice(...))
ok   rust fn edit_set_form_field_choices
ok   dart setValue(PdfFormFieldValue.choices(...))
ok   rust fn edit_clear_form_field_value
ok   dart setValue(PdfFormFieldValue.none())
ok   rust fn edit_remove_form_field
ok   dart remove
ok   rust fn edit_set_form_field_readonly
ok   dart setReadOnly
ok   rust fn edit_set_form_field_required
ok   dart setRequired
ok   rust fn edit_set_form_field_tooltip
ok   dart setTooltip
ok   rust fn edit_set_form_field_bounds
ok   dart setBounds
ok   rust fn edit_set_form_field_max_length
ok   dart setMaxLength
ok   rust fn edit_set_form_field_alignment
ok   dart setAlignment
ok   rust fn edit_set_form_field_background_color
ok   dart setBackgroundColor
ok   rust fn edit_set_form_field_border_color
ok   dart setBorderColor
ok   rust fn edit_set_form_field_border_width
ok   dart setBorderWidth
ok   rust fn edit_set_form_field_appearance
ok   dart setAppearance
ok   rust fn edit_set_form_field_flags
ok   dart setFlags
ok   rust fn handle_builder_create
ok   dart Pdf.build
ok   rust fn handle_builder_set_metadata
ok   dart setMetadata
ok   rust fn handle_builder_add_page
ok   dart addPage
ok   rust fn handle_builder_add_image_page
ok   dart addImagePage
ok   rust fn handle_builder_save
ok   dart PdfBuilderHandle.save
ok   dart PdfBuilderHandle.dispose
ok   rust PageOp::Font
ok   dart setFont
ok   rust PageOp::At
ok   dart moveTo
ok   rust PageOp::Text
ok   rust PageOp::StyledText
ok   dart text
ok   rust PageOp::Inline
ok   rust PageOp::StyledText
ok   dart appendText
ok   rust PageOp::Heading
ok   dart heading
ok   rust PageOp::Paragraph
ok   dart paragraph
ok   rust PageOp::Space
ok   dart space
ok   rust PageOp::Newline
ok   dart newline
ok   rust PageOp::HorizontalRule
ok   dart horizontalRule
ok   rust PageOp::TextInRect
ok   dart textIn
ok   rust PageOp::Image
ok   rust PageOp::ImageArtifact
ok   dart image
ok   rust PageOp::FilledRect
ok   rust PageOp::StrokeRect
ok   dart rect
ok   rust PageOp::StrokeLine
ok   dart line
ok   rust PageOp::Watermark
ok   dart watermark
ok   rust PageOp::Stamp
ok   dart stamp
ok   rust PageOp::StickyNote
ok   dart stickyNote
ok   rust PageOp::FreeText
ok   dart freeText
ok   rust PageOp::Footnote
ok   dart footnote
ok   rust PageOp::Columns
ok   dart columns
ok   rust PageOp::StreamingTableBlock
ok   dart table
ok   rust PageOp::TextField
ok   dart textField
ok   rust PageOp::Checkbox
ok   dart checkbox
ok   rust PageOp::ComboBox
ok   dart comboBox
ok   rust PageOp::RadioGroup
ok   dart radioGroup
ok   rust PageOp::PushButton
ok   dart pushButton
ok   rust PageOp::SignatureField
ok   dart signatureField
ok   rust PageOp::OnOpen
ok   dart onOpen
ok   rust PageOp::OnClose
ok   dart onClose
ok   rust fn open_document
ok   dart open
ok   rust fn handle_editor_open
ok   dart edit
ok   rust fn handle_builder_create
ok   dart build
ok   rust fn open_document
ok   dart read
ok   rust fn edit_save_with_options
ok   dart rewrite
ok   rust fn handle_builder_save
ok   dart buildTo
ok   rust fn open_document
ok   dart readOnce
ok   rust fn edit_save_with_options
ok   dart editOnce
ok   rust fn open_document
ok   dart info
ok   rust fn edit_save_with_options
ok   dart resave
ok   rust fn merge_document
ok   dart merge
ok   rust fn handle_editor_extract_pages
ok   dart split
ok   rust fn edit_reduce_images
ok   rust fn edit_save_with_options
ok   dart compress
ok   rust fn handle_builder_add_image_page
ok   dart imagesToPdf
ok   rust fn handle_sign
ok   dart sign
ok   rust fn convert_to_format_writer
ok   dart convertToOffice
ok   rust fn convert_from_format_writer
ok   dart convertFromOffice
ok   rust fn convert_xfa_to_acroform
ok   dart convertXfaToAcroForm
ok   rust fn register_fallback_font
ok   dart registerFallbackFont
checked 270 missing 0
```
