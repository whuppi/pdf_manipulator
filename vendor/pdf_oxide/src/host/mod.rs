//! pdf_manipulator host layer.
//!
//! This entire module is added by pdf_manipulator for the Flutter bridge.
//! It is NOT part of upstream pdf_oxide.
//!
//! Target-shared modules carry the operation logic and wire format;
//! `native/` (executor threads, condvar I/O) and `wasm/` (JS-callback I/O)
//! are the per-target edges. Each module's own header states its job.

pub mod attachments;
pub mod binary_codec;
pub mod bridge_api;
pub mod constants;
pub mod content;
pub(crate) mod deflate;
pub mod dispatch;
pub(crate) mod duplicates;
pub(crate) mod form_choice;
#[cfg(test)]
mod engine_proofs;
#[cfg(test)]
pub(crate) mod flat_extras_seam;
pub mod caches;
pub mod heap;
pub mod group_table;
pub mod piece_budget;
pub mod engine_state;
/// Per-op dispatch units (entry + handler + linker anchor).
pub mod ops;
pub(crate) mod page_prune;
#[cfg(feature = "rendering")]
pub(crate) mod chunked;
#[cfg(feature = "rendering")]
pub(crate) mod extract_stream;
#[cfg(feature = "rendering")]
pub(crate) mod render_stream;
pub(crate) mod resource_prune;
pub(crate) mod row_space;
pub(crate) mod ruling_grid;
pub(crate) mod scan;
pub(crate) mod forward;
pub(crate) mod span_index;
#[cfg(feature = "test-support")]
pub mod probe;
// Every host the tests run on has a thread CPU clock; wasm32 has none.
#[cfg(any(unix, windows))]
pub mod cpu_clock;
pub mod positioned_write;
pub mod font_optimizer;
pub(crate) mod font_budget;
pub(crate) mod font_subset;
pub mod images;
pub mod image_embed;
pub(crate) mod staged;
pub(crate) mod incremental;
#[cfg(feature = "rendering")]
pub(crate) mod named_glyphs;
pub(crate) mod ink_index;
pub(crate) mod object_streams;
#[cfg(feature = "signatures")]
pub mod sign;
#[cfg(feature = "signatures")]
pub(crate) mod verify;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
