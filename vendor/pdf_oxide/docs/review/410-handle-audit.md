# FFI Handle Dereference Audit — Issue #410

All `&mut *` / `&*` handle dereferences in `src/ffi.rs` have been replaced with
`handle_mut!(ptr)` / `handle_ref!(ptr)` macros. This table records the classification
of every `handle_mut!` site (the 58 genuinely mutating calls).

All 58 use `handle_mut!` because the underlying Rust API methods take `&mut self`
throughout — including read-looking operations like `page_count`, `get_page_rotation`,
and `validate_pdf_a`. Making those `&self` is a separate refactor tracked in #410.

## handle_mut! sites (58 total)

| Line (approx) | Function | Handle type | Why &mut needed |
|---|---|---|---|
| 502 | `document_editor_save` (macro) | `*const DocumentEditor` cast | `save()` takes `&mut self` |
| 533 | `document_editor_save` | `DocumentEditor` | `save()` takes `&mut self` |
| 567 | `document_editor_open_from_bytes` | `DocumentEditor` | builder pattern takes `&mut` |
| 601 | `document_editor_set_producer` | `DocumentEditor` | setter takes `&mut self` |
| 617 | `document_editor_get_producer` | `DocumentEditor` | getter takes `&mut self` (internal lazy load) |
| 651 | `document_editor_set_creation_date` | `DocumentEditor` | setter |
| 670 | `document_editor_get_creation_date` | `DocumentEditor` | getter takes `&mut self` |
| 730–1182 | Various `document_editor_*` | `DocumentEditor` | all `EditableDocument` trait methods take `&mut self` |
| 1280 | `pdf_save` | `Pdf` | `save()` takes `&mut self` |
| 1305 | `pdf_save_to_bytes` | `Pdf` | `save_to_bytes()` takes `&mut self` |
| 1327 | `pdf_get_page_count` | `Pdf` | `page_count()` takes `&mut self` (PDF builder API) |
| 2797/2929/2961 | Signature functions | `PdfDocument` (cast) | signature verification mutates internal state |
| 3510–3782 | `pdf_render_page*` | `PdfDocument` | `render_page()` takes `&mut PdfDocument` |
| 4394/4599 | UA / PdfA validation | `PdfDocument` | `validate_*()` takes `&mut PdfDocument` |
| 5811–6003 | `document_editor_delete_page` etc. | `DocumentEditor` | mutating page operations |
| 6134/6233 | `pdf_validate_pdf_a/x_level` | `PdfDocument` | validator takes `&mut` |
| 6315/6694/6734/6758 | More editor operations | `DocumentEditor` | mutating |
| 7349/7386 | Builder ops | inner builder | builder takes `&mut` |
| 7811 | `ffi_builder_mut` (private) | `FfiDocumentBuilder` | helper that yields `&mut` for callers |
| 8078 | `push_page_op` (private) | `FfiPageBuilder` | pushes to ops Vec |
| 9463 | `pdf_page_builder_done` | `page.parent` | writes into parent builder |

## Future work (v0.4.0)

To eliminate the UB entirely, wrap each handle type in `Mutex<T>`:
- `Box<PdfDocument>` → `Box<Mutex<PdfDocument>>`
- `Box<DocumentEditor>` → `Box<Mutex<DocumentEditor>>`
- `Box<Pdf>` → `Box<Mutex<Pdf>>`
- etc.

This requires an ABI version bump and a deadlock audit (check no FFI call path
acquires two handles in a fixed order that could deadlock with a reversed order
from another thread). See issue #410.
