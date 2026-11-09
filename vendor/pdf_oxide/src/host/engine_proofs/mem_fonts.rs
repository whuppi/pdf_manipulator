//! A page that names thousands of fonts is extracted holding a few of them at
//! a time, and the caches that keep fonts are bounded by the bytes they hold.

use super::filter_readers::{HELD, PEAK};
use super::own_process::alone;
use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use std::sync::atomic::Ordering;

const FONTS: usize = 1500;
/// The ToUnicode mappings of each font: every code of its one-byte space.
const MAPPINGS: u32 = 256;
/// The first font object; font `i` is object `FIRST + 2 * i`, its CMap the next.
const FIRST: usize = 5;

/// The character font `i` maps `code` to: mappings differ from font to font, so
/// no two CMaps are the same stream.
fn mapped(i: usize, code: u32) -> char {
    char::from_u32(0x4E00 + ((i as u32 * MAPPINGS + (code - 0x20)) % 20_000)).unwrap()
}

/// One page showing each of `FONTS` simple fonts once, with the two letters
/// `AB`. Every font has a ToUnicode CMap of its own. Returns the file and the
/// bytes its fonts and CMaps take in it.
fn fixture() -> (Vec<u8>, usize) {
    let names: String = (0..FONTS).map(|i| format!("/F{i} {} 0 R ", FIRST + 2 * i)).collect();
    let content: String =
        (0..FONTS).map(|i| format!("BT /F{i} 10 Tf 10 {} Td (AB) Tj ET\n", 780 - i % 700)).collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /Font << {names}>> >> /Contents 4 0 R >>\nendobj\n"
        )
        .into_bytes(),
        stream_object(4, "", content.as_bytes()),
    ];
    let widths = "600 ".repeat(MAPPINGS as usize);
    let mut in_file = 0usize;
    for i in 0..FONTS {
        let font = FIRST + 2 * i;
        let dict = format!(
            "{font} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Probe{i} /FirstChar 0 \
             /LastChar {} /Widths [{widths}] /ToUnicode {} 0 R >>\nendobj\n",
            MAPPINGS - 1,
            font + 1
        );
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
             1 begincodespacerange\n<00> <FF>\nendcodespacerange\n",
        );
        cmap.push_str(&format!("{MAPPINGS} beginbfchar\n"));
        for code in 0x20..0x20 + MAPPINGS {
            cmap.push_str(&format!("<{:02X}> <{:04X}>\n", code & 0xFF, mapped(i, code) as u32));
        }
        cmap.push_str("endbfchar\nendcmap end end\n");
        in_file += dict.len() + cmap.len();
        objects.push(dict.into_bytes());
        objects.push(stream_object(font as u32 + 1, "", cmap.as_bytes()));
    }
    (pdf_from_objects(&objects), in_file)
}

/// Extracting a page that names 1,500 fonts, each with a ToUnicode CMap of its
/// own, grows the process by 14 MiB, and by 74 MiB when the page loads every
/// font it names, and by 60 MiB when the font caches have no byte bound. The text of the fonts read
/// late in the page is still decoded by their own CMaps. No Dart battery can
/// observe this: the allocator counter is the engine's own, and the extracted
/// text is the same whether the fonts are held for the page or released as it
/// goes. It runs in a process of its own, since the counter counts every
/// thread.
#[test]
fn a_page_of_thousands_of_fonts_is_extracted_holding_few_of_them() {
    alone(module_path!(), "a_page_of_thousands_of_fonts_is_extracted_holding_few_of_them", || {
        let (bytes, in_file) = fixture();
        let doc = PdfDocument::from_bytes(bytes).unwrap();
        let base = HELD.load(Ordering::Relaxed);
        PEAK.store(base, Ordering::Relaxed);
        let text = doc.extract_text(0).unwrap();
        let growth = PEAK.load(Ordering::Relaxed).saturating_sub(base);
        println!("{FONTS} fonts, {in_file} bytes in the file: peak growth {growth} bytes");
        for i in [0, 1, FONTS / 2, FONTS - 1] {
            let pair: String = [mapped(i, 0x41), mapped(i, 0x42)].iter().collect();
            assert!(text.contains(&pair), "font {i}: {pair:?} is missing from the text");
        }
        assert!(
            growth < 30 * 1024 * 1024,
            "extracting {FONTS} fonts grew the process by {growth} bytes"
        );
    });
}
