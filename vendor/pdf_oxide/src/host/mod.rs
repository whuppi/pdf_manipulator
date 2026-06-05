//! pdf_manipulator host layer.
//!
//! This entire module is added by pdf_manipulator for the Flutter bridge.
//! It is NOT part of upstream pdf_oxide.
//!
//! Shared (both targets):
//!   `constants`    — I/O buffer capacities
//!   `dispatch`     — all operation logic, calls engine APIs
//!   `binary_codec` — parse/encode the binary wire format
//!   `bridge_api`   — request routing, handle maps, entry points
//!
//! Native only (`#[cfg(not(wasm32))]`):
//!   `native/` — arena, thread pool, condvar reader/writer, shared buffer
//!
//! WASM only (`#[cfg(wasm32)]`):
//!   `wasm/` — JS-callback reader/writer

pub mod binary_codec;
pub mod bridge_api;
pub mod constants;
pub mod dispatch;
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
