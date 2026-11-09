//! A CMap range the file declares is sized in checked arithmetic and its
//! mappings are stored with fallible allocation.

use crate::fonts::cmap::parse_tounicode_cmap;

fn cmap(body: &str) -> Vec<u8> {
    format!("begincmap\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n{body}\nendcmap\n")
        .into_bytes()
}

/// A bfrange array over the whole 32-bit code space is read, and one whose
/// end sits below its start is dropped, without `end - start + 1`
/// overflowing: in a debug build each used to panic ("attempt to subtract /
/// add with overflow"). No Dart battery can observe this: a panic in a lane
/// is reported as a failed job whatever the CMap, so only the parser's own
/// result tells the two apart.
#[test]
fn a_hostile_bfrange_array_size_is_refused_without_overflow() {
    let whole = parse_tounicode_cmap(&cmap(
        "1 beginbfrange\n<00000000> <FFFFFFFF> [<0041>]\nendbfrange",
    ))
    .expect("a range over the code space is not an error");
    assert_eq!(whole.get(&0).as_deref(), Some("A"));
    assert_eq!(whole.len(), 1, "only the one declared destination is stored");

    let backwards = parse_tounicode_cmap(&cmap("1 beginbfrange\n<0010> <0001> [<0041>]\nendbfrange"))
        .expect("an inverted range is not an error");
    assert!(backwards.is_empty());
}

/// A table that cannot take the entries asked of it is a typed error that
/// names the size, not an abort in `reserve`.
#[test]
fn an_impossible_reservation_is_a_typed_error() {
    let mut chars = std::collections::HashMap::new();
    let err = crate::fonts::cmap::reserve_entries(&mut chars, usize::MAX).unwrap_err();
    assert!(matches!(err, crate::error::Error::Font(_)), "{err:?}");
}
