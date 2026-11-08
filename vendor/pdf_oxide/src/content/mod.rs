//! PDF content stream parsing and execution.
//!
//! This module handles parsing and processing of PDF content streams,
//! which contain sequences of operators that define page appearance.
//!
//! Phase 4

pub mod graphics_state;
pub mod operators;
pub mod parser;

pub use graphics_state::{GraphicsState, GraphicsStateStack, Matrix};
pub use operators::{Operator, TextElement};
// ── pdf_manipulator patch: the byte-slice text, path and image parsers are gone ──
pub use parser::parse_content_stream;
// ── end pdf_manipulator patch ──
