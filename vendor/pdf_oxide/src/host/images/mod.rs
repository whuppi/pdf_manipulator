//! Image work for the host layer: what images a document draws, how they
//! are classified, what a policy decides for them, and (behind the
//! `rendering` feature) how they are re-encoded.
//!
//! This module is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Five stages, one file each: `inventory` (which images, drawn where),
//! `classify` (what an image is), `policy` (what to do with it, a pure
//! decision), `execute` (do it), `report` (say what happened). `inventory`
//! is core because `editorPageImages` needs it without any codec.
//!
//! Two more files serve the page render rather than the reducer: `stream`
//! reads an image's rows as the page's bands ask for them, and `resample`
//! also holds the row-at-a-time resampler those rows go through.

pub mod inventory;

#[cfg(feature = "rendering")]
pub mod classify;
#[cfg(feature = "rendering")]
pub mod encode;
#[cfg(feature = "rendering")]
pub mod execute;
#[cfg(feature = "rendering")]
pub mod policy;
#[cfg(feature = "rendering")]
pub mod report;
#[cfg(feature = "rendering")]
pub mod resample;
#[cfg(feature = "rendering")]
pub mod stream;
#[cfg(feature = "rendering")]
pub mod dct;
#[cfg(feature = "rendering")]
pub mod redact_copy;
