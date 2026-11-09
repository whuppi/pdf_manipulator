//! Test seam: objects the next flat set carries beside its first appearance.
//!
//! No public path flattens a widget with a Type 3 fallback font, so a proof of
//! the extra-object scrub supplies one here; the flat set takes them as it
//! takes the fallback font graph of a CJK value.

use crate::object::Object;

thread_local! {
    pub(crate) static TEST_FLAT_EXTRAS: std::cell::RefCell<Vec<(u32, Object)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
