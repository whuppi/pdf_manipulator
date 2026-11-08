//! What a saved file keeps of the font entries and the structure strings of
//! redacted content: the widths and the text of a code that only the removed
//! glyphs showed, the glyph procedure of such a code in a Type 3 font, and the
//! `/ActualText` and `/Alt` of the structure element that owned the marked
//! content. Every page here is saved with garbage collection on and off.

use super::redact_path_save::{assert_absent_everywhere, save};
use super::redact_save::{obj, stream, write_pdf};
use super::support::{contains, open_editor_from_bytes};
use crate::fonts::cmap::parse_tounicode_cmap;
use crate::object::{Object, ObjectRef};
use crate::PdfDocument;
use std::collections::HashMap;

const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// Widths of `A` to `Z`: 500 plus the letter's place, so each is its own.
fn widths() -> String {
    format!("[{}]", (0..26).map(|n| (500 + n).to_string()).collect::<Vec<_>>().join(" "))
}

/// A `/ToUnicode` CMap mapping each of `A` to `Z` to itself.
fn to_unicode() -> Vec<u8> {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
         1 begincodespacerange <00> <FF> endcodespacerange\n26 beginbfchar\n",
    );
    for code in 65..=90u32 {
        cmap.push_str(&format!("<{code:04X}> <{code:04X}>\n"));
    }
    cmap.push_str("endbfchar\nendcmap end end\n");
    cmap.into_bytes()
}

/// Page 0 shows `QXZ` under the box and `PUBLIC` outside it; page 1 shows `Z`.
/// Both draw font 5. With `acroform` the AcroForm offers the font to its
/// fields, which shows codes that do not exist yet.
fn shared_font(acroform: bool) -> Vec<u8> {
    let catalog = if acroform {
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [] /DR << /Font << /F1 5 0 R >> >> >> >>"
    } else {
        "<< /Type /Catalog /Pages 2 0 R >>"
    };
    write_pdf(&[
        obj(1, catalog),
        obj(2, "<< /Type /Pages /Kids [3 0 R 7 0 R] /Count 2 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(
            4,
            b"BT /F1 12 Tf 1 0 0 1 20 150 Tm (QXZ) Tj 1 0 0 1 20 60 Tm (PUBLIC) Tj ET\n",
        ),
        obj(
            5,
            &format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /FirstChar 65 /LastChar 90 \
                 /Widths {} /ToUnicode 6 0 R >>",
                widths()
            ),
        ),
        stream(6, &to_unicode()),
        obj(
            7,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 8 0 R >>",
        ),
        stream(8, b"BT /F1 12 Tf 1 0 0 1 20 100 Tm (Z) Tj ET\n"),
    ])
}

/// The reference and dictionary of font `/F1` of page `page`.
fn font_of(doc: &PdfDocument, page: usize) -> (ObjectRef, HashMap<String, Object>) {
    let page_ref = doc.get_page_ref(page).unwrap();
    let page = doc.load_object(page_ref).unwrap();
    let resources = doc.resolve_object(page.as_dict().unwrap().get("Resources").unwrap()).unwrap();
    let fonts = doc.resolve_object(resources.as_dict().unwrap().get("Font").unwrap()).unwrap();
    let entry = fonts.as_dict().unwrap().get("F1").unwrap();
    let reference = entry.as_reference().expect("the font is an indirect object");
    (reference, doc.load_object(reference).unwrap().as_dict().unwrap().clone())
}

fn widths_of(doc: &PdfDocument, font: &HashMap<String, Object>) -> Vec<f64> {
    let list = doc.resolve_object(font.get("Widths").unwrap()).unwrap();
    list.as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_real().or_else(|| w.as_integer().map(|i| i as f64)).unwrap())
        .collect()
}

fn mapped(doc: &PdfDocument, font: &HashMap<String, Object>, letter: char) -> Option<String> {
    let reference = font.get("ToUnicode").unwrap().as_reference().unwrap();
    let data = doc.load_object(reference).unwrap().decode_stream_data().unwrap();
    let cmap = parse_tounicode_cmap(&data).unwrap();
    cmap.get(&(letter as u32)).map(|text| text.to_string())
}

/// Letters by their place in `A` to `Z`.
const Q: usize = 16;
const X: usize = 23;
const Z: usize = 25;
const P: usize = 15;

/// The font of a redacted page and of an unredacted one is one object. The
/// codes only the secret showed (`Q`, `X`) leave its `/Widths` and
/// `/ToUnicode`; `Z`, which the unredacted page shows, and `P`, which the
/// redacted page still shows, keep both, and the unredacted page extracts as
/// it did. No battery reads a font's `/Widths` or the CMap bytes of a file.
#[test]
fn a_shared_font_loses_only_the_codes_no_surviving_text_shows() {
    let source = shared_font(false);
    let before = PdfDocument::from_bytes(source.clone()).unwrap().extract_text(1).unwrap();
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "font_scrub_shared");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        let report = editor.apply_redactions_destructive(Default::default()).unwrap();
        assert_eq!(report.fonts_scrubbed, 1, "the report names the font");
        let out = save(&mut editor, garbage_collect);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();

        let (reference, font) = font_of(&doc, 0);
        assert_eq!(font_of(&doc, 1).0, reference, "gc={garbage_collect}: the pages share the font");
        let widths = widths_of(&doc, &font);
        assert_eq!(widths[Q], 0.0, "gc={garbage_collect}: the width of Q stays");
        assert_eq!(widths[X], 0.0, "gc={garbage_collect}: the width of X stays");
        assert_eq!(widths[Z], 525.0, "gc={garbage_collect}: the width of Z went");
        assert_eq!(widths[P], 515.0, "gc={garbage_collect}: the width of P went");
        assert_eq!(mapped(&doc, &font, 'Q'), None, "gc={garbage_collect}: Q is still mapped");
        assert_eq!(mapped(&doc, &font, 'X'), None, "gc={garbage_collect}: X is still mapped");
        assert_eq!(mapped(&doc, &font, 'Z').as_deref(), Some("Z"));
        assert_eq!(mapped(&doc, &font, 'P').as_deref(), Some("P"));
        // The first CMap, with its entry for Q, is in no object of the file.
        assert_absent_everywhere(&out, b"<0051>", garbage_collect);
        assert_absent_everywhere(&out, b"<0058>", garbage_collect);
        assert_eq!(doc.extract_text(1).unwrap(), before, "gc={garbage_collect}");
        assert!(doc.extract_text(0).unwrap().contains("PUBLIC"));
    }
}

/// A font the AcroForm offers its fields shows codes nobody can count, so the
/// redacted page draws a scrubbed copy and the original stays as it was for
/// the fields.
#[test]
fn a_font_the_acroform_offers_is_copied_for_the_redacted_page() {
    let source = shared_font(true);
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "font_scrub_copy");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);
        let doc = PdfDocument::from_bytes(out).unwrap();

        let (copy, scrubbed) = font_of(&doc, 0);
        let (original, kept) = font_of(&doc, 1);
        assert_ne!(copy, original, "gc={garbage_collect}: the page still draws the shared font");
        assert_eq!(widths_of(&doc, &scrubbed)[Q], 0.0);
        assert_eq!(mapped(&doc, &scrubbed, 'Q'), None);
        assert_eq!(widths_of(&doc, &kept)[Q], 516.0, "gc={garbage_collect}: the original changed");
        assert_eq!(mapped(&doc, &kept, 'Q').as_deref(), Some("Q"));
    }
}

/// A Type 3 font whose glyph `a` only the secret used: its procedure leaves
/// the file and the glyph `b` the public text uses keeps its own.
#[test]
fn a_type3_glyph_only_the_secret_used_becomes_an_empty_procedure() {
    let source = write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(
            4,
            b"BT /F1 12 Tf 1 0 0 1 20 150 Tm (a) Tj 1 0 0 1 20 60 Tm (b) Tj ET\n",
        ),
        obj(
            5,
            "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] \
             /FontMatrix [0.001 0 0 0.001 0 0] /FirstChar 97 /LastChar 98 /Widths [1000 1000] \
             /Encoding << /Type /Encoding /Differences [97 /a /b] >> \
             /CharProcs << /a 6 0 R /b 7 0 R >> /Resources << >> >>",
        ),
        stream(6, b"1000 0 0 0 1000 1000 d1 0 0 1000 1000 re f % SECRETGLYPH\n"),
        stream(7, b"1000 0 0 0 1000 1000 d1 0 0 500 500 re f % PUBLICGLYPH\n"),
    ]);
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "font_scrub_type3");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);
        assert_absent_everywhere(&out, b"SECRETGLYPH", garbage_collect);
        assert!(contains(&out, b"PUBLICGLYPH"), "gc={garbage_collect}: the public glyph is gone");
    }
}

/// A page with a structure tree: the element of the marked content under the
/// box loses its `/ActualText` and `/Alt`, the element of the public content
/// keeps them, and the lookup goes through the page's `/StructParents` in the
/// parent tree.
#[test]
fn the_structure_element_of_redacted_marked_content_loses_its_strings() {
    let source = write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R /StructTreeRoot 10 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /StructParents 0 \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(
            4,
            b"/P << /MCID 0 >> BDC BT /F1 12 Tf 1 0 0 1 20 150 Tm (QXZ) Tj ET EMC\n\
              /P << /MCID 1 >> BDC BT /F1 12 Tf 1 0 0 1 20 60 Tm (PUBLIC) Tj ET EMC\n",
        ),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        obj(10, "<< /Type /StructTreeRoot /K [11 0 R 12 0 R] /ParentTree 13 0 R >>"),
        obj(
            11,
            "<< /Type /StructElem /S /P /P 10 0 R /Pg 3 0 R /K 0 \
             /ActualText (SECRET-ACTUAL) /Alt (SECRET-ALT) /E (SECRET-EXPANSION) >>",
        ),
        obj(
            12,
            "<< /Type /StructElem /S /P /P 10 0 R /Pg 3 0 R /K 1 \
             /ActualText (PUBLIC-ACTUAL) /Alt (PUBLIC-ALT) >>",
        ),
        obj(13, "<< /Nums [0 [11 0 R 12 0 R]] >>"),
    ]);
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "font_scrub_structure");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);
        for secret in [&b"SECRET-ACTUAL"[..], b"SECRET-ALT", b"SECRET-EXPANSION"] {
            assert_absent_everywhere(&out, secret, garbage_collect);
        }
        assert!(contains(&out, b"PUBLIC-ACTUAL"), "gc={garbage_collect}: public strings went");
        assert!(contains(&out, b"PUBLIC-ALT"), "gc={garbage_collect}: public strings went");
        let doc = PdfDocument::from_bytes(out).unwrap();
        let element = doc.load_object(ObjectRef::new(11, 0)).unwrap();
        assert_eq!(
            element.as_dict().unwrap().get("S").and_then(Object::as_name),
            Some("P"),
            "gc={garbage_collect}: the element itself stays"
        );
    }
}
