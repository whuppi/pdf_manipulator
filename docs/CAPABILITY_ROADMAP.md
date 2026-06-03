# Capability Roadmap

Every Rust engine capability mapped to its Dart surface.

**O(1) memory I/O — non-negotiable.** Every shipped op streams through
bounded buffers. Every PLANNED op must do the same. The test guards
(TestSource 64KB, TestSink 256KB) enforce this mechanically — see
[`ARCHITECTURE.md`](ARCHITECTURE.md) §6.

Five files, strict rules:

| File | Role | Rule |
|---|---|---|
| `pdf_doc.dart` | Read-only queries | No mutations |
| `pdf_editor.dart` | Mutations only | No read/export ops even if Rust has them on editor |
| `pdf_builder.dart` | Create from scratch | No reading existing PDFs |
| `pdf_standalone.dart` | Source in, sink out, no handle | Non-mutating one-shot ops |
| `pdf_sugar.dart` | Convenience wrappers | Over editor/builder only, never standalone (rare exception allowed) |

---

## PdfDoc — read-only queries

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Page count | `current_page_count` | `pageCount` | DONE |
| Version | `version` | `version` | DONE |
| Page list (dimensions, rotation) | `get_page_media_box`, `get_page_rotation` | `pages` (decoded on open) | DONE |
| Title / Author / Subject / Keywords | `title`, `author`, `subject`, `keywords` | decoded on open | DONE |
| Encryption info | via open result | `isEncrypted`, `encryptionAlgorithm`, `permissions` | DONE |
| Is tagged | via open result | `isTagged` | DONE |
| Extract text | `extract_page_text` | `extract()` | DONE |
| Search text | `search_text` | `search()` | DONE |
| Render pages | `render_pages_streamed` | `render()` | DONE |
| Extract images | `extract_images_streamed` | `extractImages()` | DONE |
| Get signatures | `get_signatures` | `getSignatures()` | DONE |
| Verify signatures | `verify_signatures` | `verifySignatures()` | DONE |
| Validate PDF/A | `validate_pdfa` | `validatePdfA()` | DONE |
| Validate PDF/UA | `validate_pdfua` | `validatePdfUa()` | DONE |
| Classify page | `classify_page` | `classifyPage()` | DONE |
| Classify document | `classify_document` | `classifyDocument()` | DONE |
| Plan split by bookmarks | `plan_split_by_bookmarks` | `planSplitByBookmarks()` | DONE |
| Get page crop box | `get_page_crop_box` | — | PLANNED |
| Has XFA forms | `has_xfa` | — | PLANNED |
| Analyze XFA | `analyze_xfa` | — | PLANNED |
| Get form fields (list all) | `get_form_fields` | — | PLANNED |
| Get form field value | `get_form_field_value` | — | PLANNED |
| Has form field | `has_form_field` | — | PLANNED |
| Get page images (list metadata) | `get_page_images` | — | PLANNED |
| Producer / Creator metadata | `producer`, `creator` | — | PLANNED |
| Creation date | `creation_date` | — | PLANNED |

---

## PdfEditor — mutations only

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Set title | `set_title` | `setTitle()` | DONE |
| Set author | `set_author` | `setAuthor()` | DONE |
| Set subject | `set_subject` | `setSubject()` | DONE |
| Set keywords | `set_keywords` | `setKeywords()` | DONE |
| Get title | `title` | `getTitle()` | DONE |
| Get author | `author` | `getAuthor()` | DONE |
| Get subject | `subject` | `getSubject()` | DONE |
| Get keywords | `keywords` | `getKeywords()` | DONE |
| Scrub metadata | via bridge | `scrubMetadata()` | DONE |
| Rotate page | `rotate_page_by` | `rotatePage()` | DONE |
| Rotate all pages | `rotate_all_pages` | `rotateAllPages()` | DONE |
| Delete page | via bridge | `deletePage()` | DONE |
| Move page | via bridge | `movePage()` | DONE |
| Select pages | `select_pages` | `selectPages()` | DONE |
| Merge from another PDF | `merge_from_reader` | `mergeFrom()` | DONE |
| Optimize images | via bridge | `optimizeImages()` | DONE |
| Unembed standard fonts | via bridge | `unembedStandardFonts()` | DONE |
| Add watermark | via bridge | `addWatermark()` | DONE |
| Add stamp | via bridge | `addStamp()` | DONE |
| Add image stamp | via bridge | `addImageStamp()` | DONE |
| Embed file | `embed_file` | `embedFile()` | DONE |
| Erase regions | `erase_regions` | `eraseRegions()` | DONE |
| Flatten forms | `flatten_forms` | `flattenForms()` | DONE |
| Flatten all annotations | `flatten_all_annotations` | `flattenAllAnnotations()` | DONE |
| Set form field value | `set_form_field_value` | `setFormFieldValue()` | DONE |
| Crop margins | `crop_margins` | `cropMargins()` | DONE |
| Resize image | `resize_image` | `resizeImage()` | DONE |
| Convert to PDF/A | via bridge | `convertToPdfA()` | DONE |
| Add redaction | `add_redaction` | `addRedaction()` | DONE |
| Redaction count | `redaction_count` | `redactionCount()` | DONE |
| Apply redactions | `apply_all_redactions` | `applyRedactions()` | DONE |
| Get page media box | `get_page_media_box` | `getPageMediaBox()` | DONE |
| Is modified | `is_modified` | `isModified` | DONE |
| Page count | `current_page_count` | `pageCount` | DONE |
| Version | `version` | `version` | DONE |
| Save | `write_full_to_writer` | `save()` | DONE |
| Set producer | `set_producer` | — | PLANNED |
| Set creation date | `set_creation_date` | — | PLANNED |
| Set page media box | `set_page_media_box` | — | PLANNED |
| Set page crop box | `set_page_crop_box` | — | PLANNED |
| Set page rotation | `set_page_rotation` | — | PLANNED |
| Flatten forms on single page | `flatten_forms_on_page` | — | PLANNED |
| Flatten annotations on single page | `flatten_page_annotations` | — | PLANNED |
| Clear erase regions | `clear_erase_regions` | — | PLANNED |
| Merge selective pages from | `merge_pages_from` | — | PLANNED |
| Apply redactions destructive | `apply_redactions_destructive` | — | PLANNED |
| Sanitize document | `sanitize_document` | — | PLANNED |
| Reposition image | `reposition_image` | — | PLANNED |
| Set image bounds | `set_image_bounds` | — | PLANNED |
| Remove form field | `remove_form_field` | — | PLANNED |
| Set form field readonly | `set_form_field_readonly` | — | PLANNED |
| Set form field required | `set_form_field_required` | — | PLANNED |
| Set form field tooltip | `set_form_field_tooltip` | — | PLANNED |
| Set form field rect | `set_form_field_rect` | — | PLANNED |
| Set form field max length | `set_form_field_max_length` | — | PLANNED |
| Set form field alignment | `set_form_field_alignment` | — | PLANNED |
| Set form field background color | `set_form_field_background_color` | — | PLANNED |
| Set form field border color | `set_form_field_border_color` | — | PLANNED |
| Set form field border width | `set_form_field_border_width` | — | PLANNED |
| Set form field default appearance | `set_form_field_default_appearance` | — | PLANNED |
| Set form field flags | `set_form_field_flags` | — | PLANNED |
| Convert XFA to AcroForm | `convert_xfa_to_acroform` | — | PLANNED |
| Embed file with options | `embed_file_with_options` | — | PLANNED |
| Export form data FDF | `export_form_data_fdf` | — | PLANNED |
| Export form data XFDF | `export_form_data_xfdf` | — | PLANNED |

---

## PdfBuilder — create from scratch

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Create builder | via bridge | `Pdf.build()` | DONE |
| Add page (custom size) | via bridge | `addPage()` | DONE |
| Add A4 page | — (dart convenience) | `addA4Page()` | DONE |
| Add Letter page | — (dart convenience) | `addLetterPage()` | DONE |
| Set title | via bridge | `setTitle()` | DONE |
| Set author | via bridge | `setAuthor()` | DONE |
| Set subject | via bridge | `setSubject()` | DONE |
| Set keywords | via bridge | `setKeywords()` | DONE |
| Text | via bridge | `page.text()` | DONE |
| Heading | via bridge | `page.heading()` | DONE |
| Paragraph | via bridge | `page.paragraph()` | DONE |
| Image | via bridge | `page.image()` | DONE |
| Watermark | via bridge | `page.watermark()` | DONE |
| Font | via bridge | `page.font()` | DONE |
| Space | via bridge | `page.space()` | DONE |
| Horizontal rule | via bridge | `page.horizontalRule()` | DONE |
| Newline | via bridge | `page.newline()` | DONE |
| New page same size | via bridge | `page.newPageSameSize()` | DONE |
| Text field | via bridge | `page.textField()` | DONE |
| Checkbox | via bridge | `page.checkbox()` | DONE |
| Combo box | via bridge | `page.comboBox()` | DONE |
| Push button | via bridge | `page.pushButton()` | DONE |
| Signature field | via bridge | `page.signatureField()` | DONE |
| Radio group | via bridge | `page.radioGroup()` | PLANNED |
| Field keystroke | via bridge | `page.fieldKeystroke()` | DONE |
| Field format | via bridge | `page.fieldFormat()` | DONE |
| Field validate | via bridge | `page.fieldValidate()` | DONE |
| Field calculate | via bridge | `page.fieldCalculate()` | DONE |
| Link URL | via bridge | `page.linkUrl()` | DONE |
| Link page | via bridge | `page.linkPage()` | DONE |
| Footnote | via bridge | `page.footnote()` | DONE |
| Columns | via bridge | `page.columns()` | DONE |
| Page done | via bridge | `page.done()` | DONE |
| Save | via bridge | `save()` | DONE |

---

## PdfStandalone — source in, sink out, no handle

| Capability | Rust | Dart | Status |
|---|---|---|---|
| Sign PDF (PKCS12 / PEM) | `handle_sign` | `sign()` | DONE |
| Convert to format (DOCX/PPTX/XLSX) | `handle_convert_to` | `convertTo()` | DONE |
| Convert to PDF | `handle_convert_to_pdf` | `convertToPdf()` | DONE |
| Extract pages | `handle_editor_extract_pages` | `extractPages()` | DONE |

---

## PdfSugar — convenience wrappers over editor/builder

| Sugar method | Wraps | Status |
|---|---|---|
| `merge` | editor: edit → mergeFrom × N → save | DONE |
| `split` | editor: extractPages × N chunks | DONE |
| `splitBySize` | editor: extractPages with binary search | DONE |
| `splitByBookmarks` | doc: planSplitByBookmarks → extractPages | DONE |
| `extractPages` | editor: edit → selectPages → save | DONE |
| `deletePages` | editor: edit → deletePage × N → save | DONE |
| `reorderPages` | editor: edit → selectPages → save | DONE |
| `movePage` | editor: edit → movePage → save | DONE |
| `rotatePages` | editor: edit → rotatePage × N → save | DONE |
| `rotateAllPages` | editor: edit → rotateAllPages → save | DONE |
| `flattenForms` | editor: edit → flattenForms → save | DONE |
| `applyRedactions` | editor: edit → applyRedactions → save | DONE |
| `compress` | editor: edit → optimizeImages → save(compress) | DONE |
| `embedFile` | editor: edit → embedFile → save | DONE |
| `eraseRegions` | editor: edit → eraseRegions → save | DONE |
| `addStamp` | editor: edit → addStamp → save | DONE |
| `addImageStamp` | editor: edit → addImageStamp → save | DONE |
| `watermark` | editor: edit → addWatermark(-1) → save | DONE |
| `encrypt` | editor: edit → save(encryption) | DONE |
| `decrypt` | editor: edit(pw) → save(removeEncryption) | DONE |
| `convertToPdfA` | editor: edit → convertToPdfA → save | DONE |
| `imagesToPdf` | builder: build → addPage+image × N → save | DONE |

---

## Summary

| Category | Done | Planned |
|---|---|---|
| PdfDoc | 17 | 9 |
| PdfEditor | 35 | 29 |
| PdfBuilder | 32 | 1 |
| PdfStandalone | 4 | 0 |
| PdfSugar | 22 | 0 |
| **Total** | **110** | **39** |

---

## Out of scope

| Feature | Why not |
|---|---|
| OCR | Requires Tesseract or similar — not a PDF primitive |
| Table extraction | Heuristic-heavy — better served by dedicated libraries |
| PDF viewer widget | Use pdfx or flutter_pdfview — they're built for viewing, we're built for manipulation |
