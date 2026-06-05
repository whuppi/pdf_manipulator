//! CMap multi-byte parsing correctness tests (§9.7.5 / §9.10.3).
//!
//! Focuses on three specific parsing correctness requirements:
//!
//! 1. **Array-form `beginbfrange`**: `<src_start> <src_end> [<dst1> <dst2> ...]`
//!    Each array entry maps `src_start + i → dst_i`.  Used for ligatures and
//!    irregular CJK sub-ranges.
//!
//! 2. **Multi-byte hex strings**: `<4E2D>` is a single 2-byte code 0x4E2D, not
//!    bytes 0x4E and 0x2D read separately.
//!
//! 3. **`begincodespacerange` drives byte-width**: When the codespace declares
//!    2-byte codes (`<0000> <FFFF>`), `LazyCMap::code_width()` must return 2 so
//!    that the text extractor switches from 1-byte to 2-byte character reading.

use pdf_oxide::fonts::cmap::{parse_tounicode_cmap, LazyCMap};

// ============================================================================
// Fix 1: Array form of beginbfrange
// ============================================================================

#[test]
fn test_bfrange_array_form_basic() {
    // CMap with: beginbfrange <0041> <0043> [<FF21> <FF22> <FF23>] endbfrange
    // Code 0x0041 → U+FF21 (Ａ), 0x0042 → U+FF22 (Ｂ), 0x0043 → U+FF23 (Ｃ)
    let data = b"beginbfrange\n<0041> <0043> [<FF21> <FF22> <FF23>]\nendbfrange";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x0041), Some(&"\u{FF21}".to_string()), "0x41 → Fullwidth A (U+FF21)");
    assert_eq!(cmap.get(&0x0042), Some(&"\u{FF22}".to_string()), "0x42 → Fullwidth B (U+FF22)");
    assert_eq!(cmap.get(&0x0043), Some(&"\u{FF23}".to_string()), "0x43 → Fullwidth C (U+FF23)");
}

#[test]
fn test_bfrange_array_form_ligatures() {
    // PDF spec §9.10.3 example: <005F> <0061> [<00660066> <00660069> <00660066006C>]
    // Codes 0x5F→"ff", 0x60→"fi", 0x61→"ffl"
    let data = b"beginbfrange\n<005F> <0061> [<00660066> <00660069> <00660066006C>]\nendbfrange";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x5F), Some(&"ff".to_string()), "code 0x5F → \"ff\"");
    assert_eq!(cmap.get(&0x60), Some(&"fi".to_string()), "code 0x60 → \"fi\"");
    assert_eq!(cmap.get(&0x61), Some(&"ffl".to_string()), "code 0x61 → \"ffl\"");
}

#[test]
fn test_bfrange_array_form_cjk() {
    // 2-byte source codes with 2-byte destinations — typical CJK ToUnicode CMap snippet
    // beginbfrange <4E00> <4E02> [<4E00> <4E01> <4E02>]
    let data = b"beginbfrange\n<4E00> <4E02> [<4E00> <4E01> <4E02>]\nendbfrange";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x4E00), Some(&"\u{4E00}".to_string()), "一 identity");
    assert_eq!(cmap.get(&0x4E01), Some(&"\u{4E01}".to_string()), "丁 identity");
    assert_eq!(cmap.get(&0x4E02), Some(&"\u{4E02}".to_string()), "丂 identity");
}

// ============================================================================
// Fix 1 complement: Linear form of beginbfrange still works after the change
// ============================================================================

#[test]
fn test_bfrange_linear_form_still_works() {
    // beginbfrange <0041> <0045> <0061> endbfrange
    // 0x41→'a', 0x42→'b', 0x43→'c', 0x44→'d', 0x45→'e'
    let data = b"beginbfrange\n<0041> <0045> <0061>\nendbfrange";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x41), Some(&"a".to_string()));
    assert_eq!(cmap.get(&0x42), Some(&"b".to_string()));
    assert_eq!(cmap.get(&0x43), Some(&"c".to_string()));
    assert_eq!(cmap.get(&0x44), Some(&"d".to_string()));
    assert_eq!(cmap.get(&0x45), Some(&"e".to_string()));
}

// ============================================================================
// Fix 2: Multi-byte hex code parsing in bfchar
// ============================================================================

#[test]
fn test_bfchar_two_byte_src_code() {
    // beginbfchar <4E2D> <4E2D> endbfchar
    // Character code 0x4E2D maps to U+4E2D (中)
    let data = b"beginbfchar\n<4E2D> <4E2D>\nendbfchar";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x4E2D), Some(&"\u{4E2D}".to_string()), "code 0x4E2D → U+4E2D (中)");
    // Make sure we did NOT insert the individual bytes as separate entries
    // (would happen if the src hex `4E2D` were split into bytes 0x4E and 0x2D)
    assert!(
        cmap.get(&0x4E).is_none() || cmap.get(&0x4E) != Some(&"\u{4E2D}".to_string()),
        "byte 0x4E must not produce 中"
    );
}

#[test]
fn test_bfchar_two_byte_src_hiragana() {
    // beginbfchar <3042> <3042> endbfchar  (hiragana あ)
    let data = b"beginbfchar\n<3042> <3042>\nendbfchar";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x3042), Some(&"\u{3042}".to_string()), "code 0x3042 → U+3042 (あ)");
}

#[test]
fn test_bfchar_two_byte_multiple_cjk() {
    // Several CJK characters as both source and destination
    let data = b"beginbfchar\n<4E2D> <4E2D>\n<6587> <6587>\n<5B66> <5B66>\nendbfchar";
    let cmap = parse_tounicode_cmap(data).unwrap();

    assert_eq!(cmap.get(&0x4E2D), Some(&"\u{4E2D}".to_string()), "中 (0x4E2D)");
    assert_eq!(cmap.get(&0x6587), Some(&"\u{6587}".to_string()), "文 (0x6587)");
    assert_eq!(cmap.get(&0x5B66), Some(&"\u{5B66}".to_string()), "学 (0x5B66)");
}

// ============================================================================
// Fix 3: begincodespacerange drives code_width
// ============================================================================

#[test]
fn test_codespacerange_two_byte_sets_code_width() {
    // begincodespacerange <0000> <FFFF> endcodespacerange declares 2-byte codes
    let data = b"/CIDInit /ProcSet findresource begin\nbegincmap\n\
        1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
        1 beginbfchar\n<3042> <3042>\nendbfchar\nendcmap";

    let cmap = parse_tounicode_cmap(data).unwrap();
    assert_eq!(cmap.code_width, 2, "2-byte codespace must set code_width = 2");

    // Lookup still works
    assert_eq!(cmap.get(&0x3042), Some(&"\u{3042}".to_string()), "あ lookup");
}

#[test]
fn test_codespacerange_one_byte_keeps_default() {
    // begincodespacerange <00> <FF> endcodespacerange — 1-byte codes
    let data = b"1 begincodespacerange\n<00> <FF>\nendcodespacerange\n\
        1 beginbfchar\n<41> <41>\nendbfchar";

    let cmap = parse_tounicode_cmap(data).unwrap();
    assert_eq!(cmap.code_width, 1, "1-byte codespace keeps code_width = 1");
    assert_eq!(cmap.get(&0x41), Some(&"A".to_string()), "A lookup");
}

#[test]
fn test_lazycmap_code_width_two_byte() {
    // Verify LazyCMap::code_width() returns 2 for a 2-byte codespace CMap
    let cmap_data = b"1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
        2 beginbfchar\n<4E2D> <4E2D>\n<6587> <6587>\nendbfchar"
        .to_vec();

    let lazy = LazyCMap::new(cmap_data);
    assert_eq!(lazy.code_width(), 2, "LazyCMap::code_width() should return 2");
}

#[test]
fn test_lazycmap_code_width_one_byte() {
    // Verify LazyCMap::code_width() returns 1 for a 1-byte codespace CMap
    let cmap_data = b"1 begincodespacerange\n<00> <FF>\nendcodespacerange\n\
        1 beginbfchar\n<41> <41>\nendbfchar"
        .to_vec();

    let lazy = LazyCMap::new(cmap_data);
    assert_eq!(lazy.code_width(), 1, "LazyCMap::code_width() should return 1");
}

#[test]
fn test_lazycmap_code_width_default_when_no_codespace() {
    // When begincodespacerange is absent, code_width should default to 1
    let cmap_data = b"1 beginbfchar\n<41> <41>\nendbfchar".to_vec();

    let lazy = LazyCMap::new(cmap_data);
    assert_eq!(lazy.code_width(), 1, "Missing codespace defaults code_width = 1");
}

// ============================================================================
// Integration: full CMap with 2-byte codespace + 2-byte bfchar + bfrange
// ============================================================================

#[test]
fn test_full_cjk_cmap_roundtrip() {
    // Simulates a realistic ToUnicode CMap for a CJK composite font
    let cmap_data = br#"
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo
<< /Registry (Adobe)
/Ordering (UCS)
/Supplement 0
>> def
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
3 beginbfchar
<4E2D> <4E2D>
<6587> <6587>
<3042> <3042>
endbfchar
1 beginbfrange
<4E00> <4E05> <4E00>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end
"#;

    let lazy = LazyCMap::new(cmap_data.to_vec());

    // code_width must be 2
    assert_eq!(lazy.code_width(), 2, "full CJK CMap code_width = 2");

    let cmap = lazy.get().expect("CMap must parse");

    // bfchar lookups
    assert_eq!(cmap.get(&0x4E2D), Some(&"\u{4E2D}".to_string()), "中");
    assert_eq!(cmap.get(&0x6587), Some(&"\u{6587}".to_string()), "文");
    assert_eq!(cmap.get(&0x3042), Some(&"\u{3042}".to_string()), "あ");

    // bfrange lookups
    assert_eq!(cmap.get(&0x4E00), Some(&"\u{4E00}".to_string()), "一");
    assert_eq!(cmap.get(&0x4E03), Some(&"\u{4E03}".to_string()), "七");
    assert_eq!(cmap.get(&0x4E05), Some(&"\u{4E05}".to_string()), "丅");
}
