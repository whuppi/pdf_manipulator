//! Runtime-registered fallback fonts for form-value baking.
//!
//! The registry lets the app supply TTF bytes at runtime instead of
//! compiling them in via `cjk-form-fonts` — the consumer decides whether
//! to carry the ~4 MB font, and only apps that need CJK/emoji form fill
//! pay for it. `form_fallback::resolve_font_bytes` consults this registry
//! before any embedded bytes.
//!
//! Statics are per linked instance: one registration covers every lane
//! thread on native; each web worker holds its own instance, so the Dart
//! router replays registrations to every lane it spawns.

use std::sync::OnceLock;

use crate::fonts::form_fallback::Fallback;

static CJK: OnceLock<Vec<u8>> = OnceLock::new();
static EMOJI: OnceLock<Vec<u8>> = OnceLock::new();

fn slot(kind: Fallback) -> &'static OnceLock<Vec<u8>> {
    match kind {
        Fallback::Cjk => &CJK,
        Fallback::Emoji => &EMOJI,
    }
}

/// Store the font for `kind`. First registration wins; re-registering is a
/// no-op (the router replays the same bytes to every lane, so later calls
/// are duplicates, not updates).
pub fn register(kind: Fallback, bytes: Vec<u8>) {
    let _ = slot(kind).set(bytes);
}

/// The registered font for `kind`, if any. `'static` because the backing
/// `OnceLock` lives for the whole instance.
pub fn registered(kind: Fallback) -> Option<&'static [u8]> {
    slot(kind).get().map(|v| v.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_then_lookup_and_first_wins() {
        register(Fallback::Emoji, vec![1, 2, 3]);
        assert_eq!(registered(Fallback::Emoji), Some(&[1u8, 2, 3][..]));
        register(Fallback::Emoji, vec![9]);
        assert_eq!(registered(Fallback::Emoji), Some(&[1u8, 2, 3][..]));
    }
}
