//! pdf_manipulator host layer.
//!
//! This entire module is added by pdf_manipulator for the Flutter bridge.
//! It is NOT part of upstream pdf_oxide.
//!
//! Target-shared modules carry the operation logic and wire format;
//! `native/` (lane threads, condvar I/O) and `wasm/` (JS-callback I/O)
//! are the per-target edges. Each module's own header states its job.

pub mod binary_codec;
pub mod bridge_api;
pub mod constants;
pub mod dispatch;
pub mod lane_state;
pub mod positioned_write;
pub mod font_optimizer;
#[cfg(feature = "rendering")]
pub mod image_optimizer;
#[cfg(feature = "signatures")]
pub mod sign;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
