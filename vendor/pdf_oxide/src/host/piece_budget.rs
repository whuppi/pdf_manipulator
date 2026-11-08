//! What one page's text costs per text piece: the declared constants.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A page's text table is whole by nature, like the xref: the geometric
//! reading order is a sort over all of a page's pieces, so no text leaves until
//! every piece is read. The engine's per-op promise is therefore the fixed
//! per-op ceiling (16 MiB over the baseline) plus these numbers of bytes for
//! each piece of the one page being read. A piece is one text run (one show
//! operator) of at most eight glyphs, as the memprobe pages `fonts` and
//! `tagged` build them. Each constant is asserted by a proof in
//! engine_proofs/mem_page_text.rs and used by the limit rows of
//! tool/memprobe/src/main.rs, so it cannot grow quietly.

/// Bytes the stamping pass of a page takes at its peak for each run, over what
/// the document held before it.
pub const STAMP_BYTES_PER_PIECE: usize = 256;

/// Bytes `load_fonts` takes at its peak for each font the page names.
pub const LOAD_FONTS_BYTES_PER_FONT: usize = 144;

/// Bytes a whole text extraction of a page takes at its peak for each run,
/// over what the process held before it.
pub const EXTRACT_BYTES_PER_PIECE: usize = 720;

/// Bytes a whole text extraction takes at its peak for each font a page names,
/// beyond the runs that show them: the font read for its first use, its CMap,
/// the name the spans of the font share, and what the caches hold of it.
/// `LOAD_FONTS_BYTES_PER_FONT` is only the part of it `load_fonts` takes.
pub const EXTRACT_BYTES_PER_FONT: usize = 432;

/// What a whole text extraction of one page may take at its peak over what the
/// process held before it, beyond the fixed per-op ceiling: the bytes for each
/// of its `runs` pieces and each of its `fonts`.
pub const fn page_extra(runs: usize, fonts: usize) -> usize {
    runs * EXTRACT_BYTES_PER_PIECE + fonts * EXTRACT_BYTES_PER_FONT
}

/// Bytes the structure-order assembly of a tagged page takes at its peak for
/// each run, over the page's runs: the ranges of the marked-content ids, the
/// page's walk of its structure, the actions and the ids it has emitted.
pub const ASSEMBLY_BYTES_PER_PIECE: usize = 360;

/// Bytes each of the two span passes that look for a run drawn twice takes at
/// its peak for each run, over the page's spans: the flat table of the places
/// a text sits and the chain of its rectangles or positions. Each was a map of
/// strings of its own (about 500 and 300 bytes a run) before.
pub const DEDUPE_BYTES_PER_PIECE: usize = 96;
