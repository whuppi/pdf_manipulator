//! The leaks and wrong outputs redaction had left after the font scrub, each
//! saved with garbage collection on and off and read back: strings written
//! inline in marked content, the widths of a CID font, a Type 3 glyph no
//! `/Differences` name reaches, an image edit on a `Do` the redaction renamed,
//! and the totals of the report. No battery reads a font's `/W`, the bytes of
//! a content stream or an engine count the Dart report does not carry.

use super::redact_path_save::{assert_absent_everywhere, save};
use super::redact_save::{obj, stream, write_pdf};
use super::support::{contains, open_editor_from_bytes};
use crate::PdfDocument;

const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

fn page_with(content: &[u8], properties: &str, extra: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let mut objects = vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
                 /Resources << /Font << /F1 5 0 R >> {properties} >> /Contents 4 0 R >>"
            ),
        ),
        stream(4, content),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ];
    objects.extend(extra);
    write_pdf(&objects)
}

fn redact(source: &[u8], tag: &str, garbage_collect: bool) -> Vec<u8> {
    let mut editor = open_editor_from_bytes(source, tag);
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    save(&mut editor, garbage_collect)
}

/// The decoded content of page 0.
fn content_of(out: &[u8]) -> Vec<u8> {
    PdfDocument::from_bytes(out.to_vec()).unwrap().get_page_content_data(0).unwrap()
}

/// `/ActualText`, `/Alt` and `/E` written in the property list of a `BDC`
/// whose marked content lost a glyph leave the file; those of marked content
/// the redaction did not touch stay, and the content keeps its public text.
#[test]
fn inline_marked_content_strings_of_removed_text_leave_the_file() {
    let source = page_with(
        b"/Span << /ActualText (SECRET-INLINE) /Alt (SECRET-INLINE-ALT) /E (SECRET-INLINE-E) \
          /Lang (en) >> BDC BT /F1 12 Tf 1 0 0 1 20 150 Tm (QXZ) Tj ET EMC\n\
          /Span << /ActualText (PUBLIC-INLINE) /Alt (PUBLIC-INLINE-ALT) >> BDC \
          BT /F1 12 Tf 1 0 0 1 20 60 Tm (PUBLIC) Tj ET EMC\n",
        "",
        vec![],
    );
    for garbage_collect in [true, false] {
        let out = redact(&source, "redact_gaps_inline", garbage_collect);
        for secret in [&b"SECRET-INLINE"[..], b"SECRET-INLINE-ALT", b"SECRET-INLINE-E"] {
            assert_absent_everywhere(&out, secret, garbage_collect);
        }
        let content = content_of(&out);
        assert!(contains(&content, b"PUBLIC-INLINE"), "gc={garbage_collect}: public text went");
        assert!(contains(&content, b"PUBLIC-INLINE-ALT"), "gc={garbage_collect}: public alt went");
        assert!(contains(&content, b"/Lang"), "gc={garbage_collect}: the other keys went");
        let doc = PdfDocument::from_bytes(out).unwrap();
        assert!(doc.extract_text(0).unwrap().contains("PUBLIC"), "gc={garbage_collect}");
    }
}

/// The same strings in the `/Properties` entry a `BDC` names: an indirect
/// list, a direct one, and one shared by a sequence under the box and one
/// outside it. The sequence that lost a glyph draws a copy without the
/// strings; the other keeps naming the original.
#[test]
fn named_marked_content_strings_of_removed_text_leave_the_file() {
    let source = page_with(
        b"/Span /MC0 BDC BT /F1 12 Tf 1 0 0 1 20 150 Tm (QXZ) Tj ET EMC\n\
          /Span /MC1 BDC BT /F1 12 Tf 1 0 0 1 20 140 Tm (QXZ) Tj ET EMC\n\
          /Span /MC2 BDC BT /F1 12 Tf 1 0 0 1 20 145 Tm (QXZ) Tj ET EMC\n\
          /Span /MC2 BDC BT /F1 12 Tf 1 0 0 1 20 60 Tm (PUBLIC) Tj ET EMC\n\
          /Span /MC3 BDC BT /F1 12 Tf 1 0 0 1 20 40 Tm (OTHER) Tj ET EMC\n",
        "/Properties << /MC0 20 0 R /MC1 << /ActualText (SECRET-DIRECT) /Lang (en) >> \
         /MC2 21 0 R /MC3 22 0 R >>",
        vec![
            obj(20, "<< /ActualText (SECRET-NAMED) /Alt (SECRET-NAMED-ALT) /E (SECRET-NAMED-E) >>"),
            obj(21, "<< /ActualText (SHARED-ACTUAL) /MCID 7 >>"),
            obj(22, "<< /ActualText (PUBLIC-NAMED) >>"),
        ],
    );
    for garbage_collect in [true, false] {
        let out = redact(&source, "redact_gaps_named", garbage_collect);
        let case = format!("gc={garbage_collect}");
        for secret in [&b"SECRET-NAMED"[..], b"SECRET-NAMED-ALT", b"SECRET-NAMED-E", b"SECRET-DIRECT"] {
            assert_absent_everywhere(&out, secret, garbage_collect);
        }
        let content = content_of(&out);
        let text = String::from_utf8_lossy(&content);
        assert!(contains(&out, b"PUBLIC-NAMED"), "{case}: the public list went");
        assert!(contains(&out, b"SHARED-ACTUAL"), "{case}: the list the public text names went");
        assert_eq!(text.matches("/MC2 BDC").count(), 1, "{case}: {text}");
        assert_eq!(text.matches("/MC3 BDC").count(), 1, "{case}: {text}");
        assert_eq!(text.matches("/MC0 BDC").count(), 0, "{case}: {text}");
        assert_eq!(text.matches("/MC1 BDC").count(), 0, "{case}: {text}");
        let doc = PdfDocument::from_bytes(out).unwrap();
        assert!(doc.extract_text(0).unwrap().contains("PUBLIC"), "{case}");
    }
}

/// The strings of marked content inside a form the redaction copies leave
/// the file too: the copy is written by the plan its own dry run made.
#[test]
fn marked_content_strings_inside_a_redacted_form_leave_the_file() {
    let form = super::support::stream_object(
        6,
        "/Type /XObject /Subtype /Form /BBox [0 0 300 300] /Resources << /Font << /F1 5 0 R >> \
         /Properties << /MC0 7 0 R >> >>",
        b"/Span << /ActualText (SECRET-FORM) >> BDC BT /F1 12 Tf 1 0 0 1 20 150 Tm (QXZ) Tj ET EMC\n\
          /Span /MC0 BDC BT /F1 12 Tf 1 0 0 1 20 145 Tm (QXZ) Tj ET EMC\n\
          /Span << /ActualText (PUBLIC-FORM) >> BDC BT /F1 12 Tf 1 0 0 1 20 60 Tm (PUBLIC) Tj ET EMC\n",
    );
    let source = page_with(
        b"/Fm0 Do\n",
        "/XObject << /Fm0 6 0 R >>",
        vec![(6, form), obj(7, "<< /Alt (SECRET-FORM-NAMED) >>")],
    );
    for garbage_collect in [true, false] {
        let out = redact(&source, "redact_gaps_form", garbage_collect);
        assert_absent_everywhere(&out, b"SECRET-FORM", garbage_collect);
        assert_absent_everywhere(&out, b"SECRET-FORM-NAMED", garbage_collect);
        assert!(contains(&out, b"PUBLIC-FORM"), "gc={garbage_collect}: public text went");
    }
}

/// An Identity-H font whose `/W` lists the widths of `A` to `Z` (each its own,
/// 600 plus its place) and a run of codes 200 to 210 at one width. Page 0
/// shows `Q`, `X` and code 205 under the box and `P` and `Z` outside it. With
/// `acroform` the AcroForm offers the font to its fields, which shows codes
/// that cannot be counted.
fn cid_font_page(acroform: bool) -> Vec<u8> {
    let catalog = if acroform {
        "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [] /DR << /Font << /F1 5 0 R >> >> >> >>"
    } else {
        "<< /Type /Catalog /Pages 2 0 R >>"
    };
    let widths: Vec<String> = (0..26).map(|n| (600 + n).to_string()).collect();
    write_pdf(&[
        obj(1, catalog),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(
            4,
            b"BT /F1 12 Tf 1 0 0 1 20 150 Tm <0051 0058 00CD> Tj 1 0 0 1 20 60 Tm <0050 005A> Tj ET\n",
        ),
        obj(
            5,
            "<< /Type /Font /Subtype /Type0 /BaseFont /AAAAAA+Test /Encoding /Identity-H \
             /DescendantFonts [6 0 R] >>",
        ),
        obj(
            6,
            &format!(
                "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /AAAAAA+Test \
                 /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
                 /FontDescriptor 7 0 R /DW 1000 /W [65 [{}] 200 210 777] >>",
                widths.join(" ")
            ),
        ),
        obj(
            7,
            "<< /Type /FontDescriptor /FontName /AAAAAA+Test /Flags 4 /FontBBox [0 0 1000 1000] \
             /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 >>",
        ),
    ])
}

/// The width the `/W` array of the font `/F1` of page 0 gives `cid`, or `None`
/// when it lists none (the font then reads `/DW`). `from_acroform` reads the
/// font the AcroForm offers instead of the page's.
fn listed_width(doc: &PdfDocument, from_acroform: bool, cid: u32) -> Option<f64> {
    let number = |o: &crate::object::Object| o.as_real().or_else(|| o.as_integer().map(|i| i as f64));
    let font_entry = if from_acroform {
        let catalog = doc.catalog().unwrap();
        let acroform = doc.resolve_object(catalog.as_dict().unwrap().get("AcroForm").unwrap()).unwrap();
        let defaults = doc.resolve_object(acroform.as_dict().unwrap().get("DR").unwrap()).unwrap();
        let fonts = doc.resolve_object(defaults.as_dict().unwrap().get("Font").unwrap()).unwrap();
        fonts.as_dict().unwrap().get("F1").unwrap().clone()
    } else {
        let page = doc.load_object(doc.get_page_ref(0).unwrap()).unwrap();
        let resources = doc.resolve_object(page.as_dict().unwrap().get("Resources").unwrap()).unwrap();
        let fonts = doc.resolve_object(resources.as_dict().unwrap().get("Font").unwrap()).unwrap();
        fonts.as_dict().unwrap().get("F1").unwrap().clone()
    };
    let font = doc.resolve_object(&font_entry).unwrap();
    let descendants = doc.resolve_object(font.as_dict().unwrap().get("DescendantFonts").unwrap()).unwrap();
    let descendant = doc.resolve_object(&descendants.as_array().unwrap()[0]).unwrap();
    let widths = doc.resolve_object(descendant.as_dict().unwrap().get("W").unwrap()).unwrap();
    let items = widths.as_array().unwrap();
    let mut at = 0;
    while at < items.len() {
        let first = items[at].as_integer().unwrap() as u32;
        match &items[at + 1] {
            crate::object::Object::Array(run) => {
                if cid >= first && ((cid - first) as usize) < run.len() {
                    return number(&run[(cid - first) as usize]);
                }
                at += 2;
            },
            last => {
                if cid >= first && cid <= last.as_integer().unwrap() as u32 {
                    return number(&items[at + 2]);
                }
                at += 3;
            },
        }
    }
    None
}

/// The widths of the codes only the secret showed leave the descendant's
/// `/W`, those a surviving show uses (and the rest of the run they split)
/// keep theirs, and a font the AcroForm offers keeps its own `/W` while the
/// redacted page draws a scrubbed copy.
#[test]
fn a_cid_font_loses_the_widths_of_the_codes_only_the_secret_showed() {
    const Q: u32 = 81;
    const X: u32 = 88;
    const P: u32 = 80;
    const Z: u32 = 90;
    for acroform in [false, true] {
        for garbage_collect in [true, false] {
            let out = redact(&cid_font_page(acroform), "redact_gaps_cid", garbage_collect);
            let doc = PdfDocument::from_bytes(out).unwrap();
            let case = format!("acroform={acroform} gc={garbage_collect}");
            assert_eq!(listed_width(&doc, false, Q), None, "{case}: the width of Q stays");
            assert_eq!(listed_width(&doc, false, X), None, "{case}: the width of X stays");
            assert_eq!(listed_width(&doc, false, 205), None, "{case}: the width of 205 stays");
            assert_eq!(listed_width(&doc, false, P), Some(615.0), "{case}: the width of P went");
            assert_eq!(listed_width(&doc, false, Z), Some(625.0), "{case}: the width of Z went");
            assert_eq!(listed_width(&doc, false, 200), Some(777.0), "{case}: the run lost its start");
            assert_eq!(listed_width(&doc, false, 204), Some(777.0), "{case}: the run lost its head");
            assert_eq!(listed_width(&doc, false, 206), Some(777.0), "{case}: the run lost its tail");
            assert_eq!(listed_width(&doc, false, 210), Some(777.0), "{case}: the run lost its end");
            if acroform {
                assert_eq!(listed_width(&doc, true, Q), Some(616.0), "{case}: the original changed");
                assert_eq!(listed_width(&doc, true, 205), Some(777.0), "{case}: the original changed");
            }
        }
    }
}

/// A blue image the redaction copies (a box covers part of it) and the caller
/// moves: the page renders the copy at the new bounds, as it renders an
/// image the redaction left alone. The belt sees the name the copy is drawn
/// under, which no battery can see.
#[test]
fn an_image_the_redaction_copied_moves_to_its_new_bounds() {
    let image = super::support::stream_object(
        6,
        "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB \
         /BitsPerComponent 8",
        &[0, 0, 255].repeat(4),
    );
    let source = page_with(
        b"q 50 0 0 50 10 10 cm /Im1 Do Q\n",
        "/XObject << /Im1 6 0 R >>",
        vec![(6, image)],
    );
    let blue = |p: [u8; 4]| p[2] > 200 && p[0] < 64 && p[1] < 64;
    let scale = super::redact_save::DPI as f32 / 72.0;
    let at = |img: &image::RgbaImage, x: f32, y: f32| {
        img.get_pixel((x * scale) as u32, ((super::redact_save::PAGE - y) * scale) as u32).0
    };
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "redact_gaps_moved_copy");
        editor.add_redaction(0, [20.0, 20.0, 40.0, 40.0], Some([0.0, 0.0, 0.0])).unwrap();
        editor.set_image_bounds(0, "Im1", 200.0, 200.0, 80.0, 80.0).unwrap();
        let report = editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);
        let after = super::redact_save::render(&out);
        let case = format!("gc={garbage_collect} {report:?}");
        assert!(blue(at(&after, 270.0, 270.0)), "{case}: the copy is not at its new bounds");
        assert!(!blue(at(&after, 55.0, 55.0)), "{case}: the copy is still where it was");
        assert!(!blue(at(&after, 230.0, 230.0)), "{case}: the covered part of the copy is blue");
    }
}

/// Two text widgets with empty appearances, `secret` under the box (and
/// reaching past it) and `keep` outside it, both drawn in the default font.
fn two_widgets() -> Vec<u8> {
    write_pdf(&[
        obj(
            1,
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [8 0 R 9 0 R] \
             /DR << /Font << /He 5 0 R >> >> /DA (/He 0 Tf 0 g) /NeedAppearances true >> >>",
        ),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
             /Annots [8 0 R 9 0 R] >>",
        ),
        stream(4, b"q Q\n"),
        obj(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        obj(
            8,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (secret) /Rect [20 142 200 162] \
             /F 4 /P 3 0 R /DA (/He 0 Tf 0 g) /AP << /N 6 0 R >> \
             /V <FEFF3084307E3060305F308D3046> >>",
        ),
        obj(
            9,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (keep) /Rect [20 20 200 40] \
             /F 4 /P 3 0 R /DA (/He 0 Tf 0 g) /AP << /N 7 0 R >> \
             /V <FEFF30683046304D30873046> >>",
        ),
        (
            6,
            super::support::stream_object(6, "/Type /XObject /Subtype /Form /BBox [0 0 180 20]", b"/Tx BMC\nEMC\n"),
        ),
        (
            7,
            super::support::stream_object(7, "/Type /XObject /Subtype /Form /BBox [0 0 180 20]", b"/Tx BMC\nEMC\n"),
        ),
    ])
}

#[cfg(feature = "cjk-form-fonts")]
/// A widget whose value is Japanese is flattened with an embedded fallback
/// font that no source object holds, and the page is redacted: the save
/// redacts the glyphs under the box instead of refusing the font, the value
/// under the box is gone from the text and from the streams, and the value
/// outside stays. No battery builds the pair on one page.
#[test]
fn a_flattened_cjk_fallback_value_under_the_box_is_redacted() {
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&two_widgets(), "redact_gaps_cjk");
        editor.flatten_forms_on_page(0).unwrap();
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);
        let case = format!("gc={garbage_collect}");
        let doc = PdfDocument::from_bytes(out).unwrap();
        let text = doc.extract_text(0).unwrap();
        assert!(!text.contains("やま") && !text.contains("たろう"), "{case}: the value reads: {text:?}");
        assert!(text.contains("とうきょう"), "{case}: the value outside does not read: {text:?}");
        // The fallback font's CMap names the value's characters by code: the
        // ones only the secret has (や ま だ た ろ) leave every decoded stream.
        for character in [0x3084u32, 0x307E, 0x3060, 0x305F, 0x308D] {
            let mapping = format!("<{character:04X}>");
            for id in 1..200 {
                let Ok(object) = doc.load_object(crate::object::ObjectRef::new(id, 0)) else { continue };
                if let Ok(data) = object.decode_stream_data() {
                    assert!(
                        !contains(&data, mapping.as_bytes()),
                        "{case}: {mapping} is in the stream of object {id}"
                    );
                }
            }
        }
    }
}

/// A Type 3 font whose `/Encoding` has no `/Differences`: code 97 and 98 reach
/// the procedures `a` and `b` through StandardEncoding. `shown` is the string
/// of the line under the box and `outside` the public line's.
fn type3_without_differences(encoding: &str, shown: &str, outside: &str) -> Vec<u8> {
    write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(
            4,
            format!(
                "BT /F1 12 Tf 1 0 0 1 20 150 Tm {shown} Tj 1 0 0 1 20 60 Tm {outside} Tj ET\n"
            )
            .as_bytes(),
        ),
        obj(
            5,
            &format!(
                "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] \
                 /FontMatrix [0.001 0 0 0.001 0 0] /FirstChar 97 /LastChar 98 \
                 /Widths [1000 1000] /Encoding {encoding} \
                 /CharProcs << /a 6 0 R /b 7 0 R >> /Resources << >> >>"
            ),
        ),
        stream(6, b"1000 0 0 0 1000 1000 d1 0 0 1000 1000 re f % SECRETGLYPH\n"),
        stream(7, b"1000 0 0 0 1000 1000 d1 0 0 500 500 re f % PUBLICGLYPH\n"),
    ])
}

/// The procedure of a Type 3 code that no `/Differences` entry names is found
/// through the base encoding: the secret glyph's procedure leaves the file and
/// the public glyph's stays.
#[test]
fn a_type3_code_without_a_differences_name_loses_its_procedure() {
    for encoding in ["<< /Type /Encoding >>", "/StandardEncoding", "/WinAnsiEncoding"] {
        let source = type3_without_differences(encoding, "(a)", "(b)");
        for garbage_collect in [true, false] {
            let out = redact(&source, "redact_gaps_type3", garbage_collect);
            assert_absent_everywhere(&out, b"SECRETGLYPH", garbage_collect);
            assert!(
                contains(&out, b"PUBLICGLYPH"),
                "encoding {encoding} gc={garbage_collect}: the public glyph is gone"
            );
        }
    }
}

/// A removed code that no encoding names reaches no procedure the engine can
/// find, so the save refuses and writes nothing.
#[test]
fn a_type3_code_no_encoding_names_refuses_the_save() {
    let source = type3_without_differences("<< /Type /Encoding >>", "(\\200)", "(b)");
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(&source, "redact_gaps_type3_refusal");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let result = editor.save_to_bytes_with_options(crate::editor::SaveOptions {
            compress: false,
            garbage_collect,
            object_streams: false,
            ..crate::editor::SaveOptions::full_rewrite()
        });
        let error = result.expect_err("the save wrote a glyph whose procedure it could not find");
        assert!(error.to_string().contains("procedure cannot be found"), "gc={garbage_collect}: {error}");
    }
}
