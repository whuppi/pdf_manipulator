// The op-unit layer: every bridge op is a named, individually addressable
// unit (entry + handler + linker anchor) instead of an arm in one match.
//
// Why this shape: reachability is the whole trim story. A match arm is
// invisible to linkers and to per-op feature gates; a unit is one row in
// the registry, one cfg gate, and one exported anchor symbol. The same
// structure serves three consumers:
//   1. per-op cargo features (compile-time cut — registry.rs rows),
//   2. a future Dart static-linking toolchain (dart-lang/sdk#49418):
//      Dart-side references to the anchors + --gc-sections drop whole
//      unlinked units,
//   3. a future wasm component model: units map one-to-one onto typed
//      interface functions.
// The op still TRAVELS as data through the single bridge door (required:
// requests must cross worker/thread boundaries, be replayable, and keep
// the 3-export ABI). The unit layer carries reachability; the door
// carries bytes. Do not merge the two back into a match.

use crate::host::binary_codec::Request;
use crate::host::bridge_api::{BoxedReader, BoxedWriter};
use crate::host::lane_state::LaneState;

pub(crate) mod builder;
pub(crate) mod convert;
pub(crate) mod doc;
pub(crate) mod editor;
pub(crate) mod fonts;
pub(crate) mod pdfa;
pub(crate) mod registry;
pub(crate) mod render;
pub(crate) mod signatures;

/// Everything a handler may need, bundled so every op shares one calling
/// convention (uniform middleware, uniform registry signature).
pub(crate) struct OpCtx<'a, 'req> {
    pub state: &'a mut LaneState,
    pub req: &'a Request<'req>,
    pub source_bytes: Option<&'req [u8]>,
    pub sources: &'a mut Vec<BoxedReader>,
    pub sinks: &'a mut Vec<BoxedWriter>,
}

impl OpCtx<'_, '_> {
    /// Takes ownership of the source at `idx` (None when absent).
    pub(crate) fn take_source(&mut self, idx: usize) -> Option<BoxedReader> {
        if idx < self.sources.len() {
            Some(self.sources.remove(idx))
        } else {
            None
        }
    }

    /// Takes ownership of the sink at `idx` (None when absent).
    pub(crate) fn take_sink(&mut self, idx: usize) -> Option<BoxedWriter> {
        if idx < self.sinks.len() {
            Some(self.sinks.remove(idx))
        } else {
            None
        }
    }

    /// Takes the op's DATA source (image bytes, embedded file, merge
    /// document). Pinned ops re-create the document reader at sources[0],
    /// so their data rides at sources[1]; non-pinned ops carry data at
    /// sources[0].
    pub(crate) fn take_data_reader(&mut self) -> Option<BoxedReader> {
        if self.sources.len() > 1 {
            Some(self.sources.remove(1))
        } else if !self.sources.is_empty() {
            Some(self.sources.remove(0))
        } else {
            None
        }
    }
}

/// One dispatchable op: its wire name and handler. Instances live in the
/// unit files and are collected by registry.rs.
pub(crate) struct OpEntry {
    pub name: &'static str,
    pub handle: fn(&mut OpCtx<'_, '_>) -> Vec<u8>,
}

/// Declares one op unit: the registry entry plus its linker anchor.
///
/// The anchor is an exported no-op symbol, one per op — inert today. It
/// exists so a future static-linking toolchain can observe per-op
/// reachability from the Dart side and garbage-collect unreferenced
/// units. Keep the `pdf_op_<name>_anchor` naming: it is the contract the
/// Dart-side wiring will reference when that toolchain lands.
macro_rules! op_unit {
    ($entry:ident, $name:literal, $anchor:ident, $handler:expr) => {
        #[no_mangle]
        pub extern "C" fn $anchor() {}

        pub(crate) static $entry: crate::host::ops::OpEntry =
            crate::host::ops::OpEntry { name: $name, handle: $handler };
    };
}
pub(crate) use op_unit;
