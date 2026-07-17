// The registry backend — the ONE swappable file of the op-unit layer.
//
// Today: an explicit table. Dropping an op is deleting (or cfg-gating)
// one row plus its unit; LTO erases everything only that unit reached.
// When Dart static linking (dart-lang/sdk#49418) or wasm component
// linking arrives, this table is what gets replaced by linker-driven
// collection — nothing else in the layer changes.

use crate::host::ops::{builder, convert, doc, editor, fonts, pdfa, render, signatures, OpEntry};

static OPS: &[&OpEntry] = &[
    // builder
    &builder::ADD_PAGE,
    &builder::CREATE,
    &builder::DISPOSE,
    &builder::PAGE_DONE,
    &builder::PAGE_OP,
    &builder::SAVE,
    &builder::SET_METADATA,
    // convert (office capability)
    &convert::CONVERT_TO,
    &convert::CONVERT_TO_PDF,
    // doc
    &doc::CLASSIFY_DOCUMENT,
    &doc::CLASSIFY_PAGE,
    &doc::DISPOSE,
    &doc::EXTRACT,
    &doc::OPEN,
    &doc::PLAN_SPLIT_BY_BOOKMARKS,
    &doc::SEARCH,
    // editor
    &editor::DISPOSE,
    &editor::EXTRACT_PAGES,
    &editor::GET_METADATA,
    &editor::IS_MODIFIED,
    &editor::MERGE_FROM,
    &editor::MUTATE,
    &editor::OPEN,
    &editor::PAGE_MEDIA_BOX,
    &editor::REDACTION_COUNT,
    &editor::SAVE,
    // fonts
    &fonts::REGISTER_FALLBACK_FONT,
    // pdfa capability
    &pdfa::VALIDATE_PDF_A,
    &pdfa::VALIDATE_PDF_UA,
    // render capability
    &render::EXTRACT_IMAGES,
    &render::RENDER,
    // signatures capability
    &signatures::GET_SIGNATURES,
    &signatures::SIGN,
    &signatures::VERIFY_SIGNATURES,
];

/// Looks up the unit for a wire op name. None means the op is unknown to
/// this build — either a typo or a unit this build excluded.
pub(crate) fn find(op: &str) -> Option<&'static OpEntry> {
    OPS.iter().find(|e| e.name == op).copied()
}
