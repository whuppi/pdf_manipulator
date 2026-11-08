//! What a saved file holds of a redacted page, and what the page renders as.
//! A mark that is not applied is saved as a /Redact annotation and changes no
//! content; an applied mark removes its content and draws its box.

use super::support::{contains, open_editor_from_bytes, stream_object};
use crate::editor::{DocumentEditor, SaveOptions};
use crate::object::{Object, ObjectRef};
use crate::rendering::{render_page, RenderOptions};
use crate::PdfDocument;

pub(super) const PAGE: f32 = 300.0;
pub(super) const DPI: u32 = 150;
/// The box every proof redacts: it holds the secret line, the link, the
/// widget and the note of the page, and nothing of the public line.
const BOX: [f32; 4] = [15.0, 135.0, 130.0, 170.0];
const RED: [f32; 3] = [1.0, 0.0, 0.0];

pub(super) fn obj(id: u32, body: &str) -> (u32, Vec<u8>) {
    (id, format!("{id} 0 obj\n{body}\nendobj\n").into_bytes())
}

pub(super) fn stream(id: u32, data: &[u8]) -> (u32, Vec<u8>) {
    (id, stream_object(id, "", data))
}

/// Writes objects by their ids in order. The ids need not be consecutive.
pub(super) fn write_pdf(objs: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let mut sorted: Vec<&(u32, Vec<u8>)> = objs.iter().collect();
    sorted.sort_by_key(|(id, _)| *id);
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = std::collections::BTreeMap::new();
    for (id, bytes) in sorted {
        offsets.insert(*id, out.len());
        out.extend_from_slice(bytes);
    }
    let size = objs.iter().map(|(id, _)| *id).max().unwrap_or(0) + 1;
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for id in 1..size {
        match offsets.get(&id) {
            Some(off) => out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes()),
            None => out.extend_from_slice(b"0000000000 65535 f \n"),
        }
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n").as_bytes(),
    );
    out
}

/// Widths for the 95 codes of a Helvetica-like font: every glyph 600/1000 em.
fn widths() -> String {
    format!("[{}]", vec!["600"; 95].join(" "))
}

/// The page every proof shares. Objects: 1 catalog, 2 pages, 3 page, 5 font,
/// 6 link and 8 widget and 10 note under the box, each with a twin outside it
/// (7, 9, 11). The page's /Contents is `contents`, and `streams` and `extra`
/// are the other objects, with ids above 11 or on the content ids.
fn page(contents: &str, streams: Vec<(u32, Vec<u8>)>, extra: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let mut objs = vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [8 0 R 9 0 R] >> >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
                 /Resources << /Font << /F1 5 0 R >> >> /Contents {contents} \
                 /Annots [6 0 R 7 0 R 8 0 R 9 0 R 10 0 R 11 0 R] >>"
            ),
        ),
        obj(
            5,
            &format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
                 /FirstChar 32 /LastChar 126 /Widths {} >>",
                widths()
            ),
        ),
        obj(
            6,
            "<< /Type /Annot /Subtype /Link /Rect [20 140 120 165] /Border [0 0 0] /Contents (LINK-INSIDE) >>",
        ),
        obj(
            7,
            "<< /Type /Annot /Subtype /Link /Rect [20 20 120 40] /Border [0 0 0] /Contents (LINK-OUTSIDE) >>",
        ),
        obj(
            8,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (fieldin) /V (FIELDVALUE) \
             /Rect [20 142 120 162] /F 4 /P 3 0 R >>",
        ),
        obj(
            9,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (fieldout) /V (KEEPVALUE) \
             /Rect [20 20 120 40] /F 4 /P 3 0 R >>",
        ),
        obj(10, "<< /Type /Annot /Subtype /Text /Rect [20 150 40 170] /Contents (NOTE-INSIDE) >>"),
        obj(11, "<< /Type /Annot /Subtype /Text /Rect [200 20 220 40] /Contents (NOTE-OUTSIDE) >>"),
    ];
    objs.extend(streams);
    objs.extend(extra);
    write_pdf(&objs)
}

/// One content stream as object 4, the secret and public lines and an inline image.
fn content() -> Vec<u8> {
    b"BT /F1 12 Tf 1 0 0 1 20 150 Tm (TOPSECRET) Tj 1 0 0 1 20 60 Tm (PUBLIC) Tj ET\n\
      q 60 0 0 60 200 200 cm BI /W 2 /H 2 /BPC 8 /CS /G ID \x10\x80\x40\xf0 EI Q\n"
        .to_vec()
}

fn fixture() -> Vec<u8> {
    page("4 0 R", vec![stream(4, &content())], vec![])
}

/// Saves uncompressed with every object top-level, so the bytes of each
/// object are in the file and can be compared.
fn save(editor: &mut DocumentEditor, garbage_collect: bool) -> Vec<u8> {
    editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            garbage_collect,
            object_streams: false,
            ..SaveOptions::full_rewrite()
        })
        .unwrap()
}

/// The ids of the objects a file writes as `N 0 obj`, in file order.
fn object_ids(bytes: &[u8]) -> Vec<u32> {
    let mut ids = Vec::new();
    for at in bytes.windows(6).enumerate().filter(|(_, w)| *w == b" 0 obj").map(|(i, _)| i) {
        let digits: Vec<u8> = bytes[..at].iter().rev().take_while(|b| b.is_ascii_digit()).copied().collect();
        let digits: Vec<u8> = digits.into_iter().rev().collect();
        if let Some(id) = std::str::from_utf8(&digits).ok().and_then(|s| s.parse().ok()) {
            ids.push(id);
        }
    }
    ids
}

/// The text of page 0 of a saved file, read back through the reader.
fn page0_text(bytes: &[u8]) -> String {
    PdfDocument::from_bytes(bytes.to_vec()).unwrap().extract_text(0).unwrap_or_default()
}

/// The /Rect values of the page's /Annots in a saved file, in order.
fn annotation_rects(bytes: &[u8]) -> Vec<Vec<f64>> {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    let page = doc.get_page_ref(0).unwrap();
    let page = doc.load_object(page).unwrap();
    let Some(Object::Array(annots)) = page.as_dict().and_then(|d| d.get("Annots")).cloned() else {
        return Vec::new();
    };
    annots
        .iter()
        .filter_map(|a| a.as_reference())
        .map(|r| {
            let annot = doc.load_object(r).unwrap();
            let Some(Object::Array(rect)) = annot.as_dict().and_then(|d| d.get("Rect")).cloned()
            else {
                return Vec::new();
            };
            rect.iter()
                .filter_map(|n| n.as_real().or_else(|| n.as_integer().map(|i| i as f64)))
                .collect()
        })
        .collect()
}

/// The body of the object whose text holds `marker`: the bytes between its
/// `obj` line and its `endobj`.
fn object_body(bytes: &[u8], marker: &[u8]) -> Vec<u8> {
    let at = bytes.windows(marker.len()).position(|w| w == marker).expect("marker in output");
    let start = bytes[..at].windows(4).rposition(|w| w == b"obj\n").expect("object start") + 4;
    let end = at + bytes[at..].windows(6).position(|w| w == b"endobj").expect("object end");
    bytes[start..end].to_vec()
}

pub(super) fn render(bytes: &[u8]) -> image::RgbaImage {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    let rendered = render_page(
        &doc,
        0,
        &RenderOptions::with_dpi(DPI).with_transparent_background(),
    )
    .unwrap();
    image::load_from_memory(&rendered.data).unwrap().to_rgba8()
}

/// The pixel at page point (`x`, `y`), y counted up from the bottom edge.
fn pixel_at(img: &image::RgbaImage, x: f32, y: f32) -> [u8; 4] {
    let scale = DPI as f32 / 72.0;
    img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0
}

/// Every pixel outside `bbox` widened by `margin` points is equal in `a` and `b`.
fn assert_equal_outside(a: &image::RgbaImage, b: &image::RgbaImage, bbox: [f32; 4], margin: f32) {
    assert_eq!(a.dimensions(), b.dimensions());
    let scale = DPI as f32 / 72.0;
    for y in 0..a.height() {
        for x in 0..a.width() {
            let px = x as f32 / scale;
            let py = PAGE - y as f32 / scale;
            let inside = px >= bbox[0] - margin
                && px <= bbox[2] + margin
                && py >= bbox[1] - margin
                && py <= bbox[3] + margin;
            if !inside {
                assert_eq!(
                    a.get_pixel(x, y),
                    b.get_pixel(x, y),
                    "pixel ({x}, {y}) outside the region differs"
                );
            }
        }
    }
}

/// Mark, apply, save: the secret is gone from the text and the bytes, the
/// box is drawn in the region, and the annotations under it are gone.
/// Both garbage-collect settings give the same page.
#[test]
fn applied_mark_removes_the_secret_with_garbage_collection_on_and_off() {
    for garbage_collect in [true, false] {
        let src = fixture();
        let mut editor = open_editor_from_bytes(&src, "redact_applied");
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        let report = editor
            .apply_redactions_destructive(Default::default())
            .unwrap();
        assert_eq!(report.regions, 1);
        assert_eq!(report.glyphs_removed, 9);
        let out = save(&mut editor, garbage_collect);

        assert!(!contains(&out, b"TOPSECRET"), "gc={garbage_collect}: secret in the bytes");
        let text = page0_text(&out);
        assert!(!text.contains("TOPSECRET") && text.contains("PUBLIC"), "text: {text}");
        assert!(!contains(&out, b"FIELDVALUE"), "gc={garbage_collect}: widget value kept");
        assert!(!contains(&out, b"LINK-INSIDE") && !contains(&out, b"NOTE-INSIDE"));
        assert_eq!(
            annotation_rects(&out),
            vec![
                vec![20.0, 20.0, 120.0, 40.0],
                vec![20.0, 20.0, 120.0, 40.0],
                vec![200.0, 20.0, 220.0, 40.0],
            ],
            "gc={garbage_collect}: only the twins outside the box stay"
        );
    }
}

/// A mark that is not applied: the save keeps the secret, draws no box in the
/// content, and writes the mark as a /Redact annotation with its /Rect and /IC.
/// The render is the source's pixels everywhere.
#[test]
fn unapplied_mark_keeps_the_content_and_writes_a_redact_annotation() {
    let src = fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_unapplied");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    let out = save(&mut editor, true);

    let text = page0_text(&out);
    assert!(text.contains("TOPSECRET"), "an unapplied mark must not remove text: {text}");
    assert!(contains(&out, b"/Subtype /Redact"), "no /Redact annotation written");
    assert!(contains(&out, b"/IC"), "the mark's fill colour is not written");
    assert!(annotation_rects(&out).contains(&BOX.iter().map(|v| f64::from(*v)).collect()));
    assert_eq!(before, render(&out), "an unapplied mark changes the render");
}

/// A damaged page: the apply refuses, nothing is recorded, and the save writes
/// the page as the source had it, the text included.
#[test]
fn refused_apply_records_nothing_and_the_save_keeps_the_page() {
    let damaged = b"q ) ) Q BT /F1 12 Tf 1 0 0 1 20 150 Tm (TOPSECRET) Tj ET\n";
    let src = page("4 0 R", vec![stream(4, damaged)], vec![]);
    let mut editor = open_editor_from_bytes(&src, "redact_refused");
    editor.add_redaction(0, BOX, None).unwrap();
    assert!(editor.apply_redactions_destructive(Default::default()).is_err());
    let out = save(&mut editor, true);
    assert!(contains(&out, b"TOPSECRET"), "the refused page must keep its content");
}

/// The text the page shows is read back through the reader, with garbage
/// collection on and off, and the secret is in no decoded stream of the file.
#[test]
fn the_secret_is_in_no_stream_of_the_saved_file() {
    for garbage_collect in [true, false] {
        let src = fixture();
        let mut editor = open_editor_from_bytes(&src, "redact_streams");
        editor.add_redaction(0, BOX, None).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, garbage_collect);

        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        for id in object_ids(&out) {
            let object = doc.load_object(ObjectRef::new(id, 0)).unwrap();
            if let Ok(data) = object.decode_stream_data() {
                assert!(
                    !contains(&data, b"TOPSECRET"),
                    "gc={garbage_collect}: object {id} decodes to the secret"
                );
            }
        }
        assert!(!contains(&out, b"TOPSECRET"));
    }
}

/// The widget with a value under the box leaves the field tree with its field,
/// and its twin outside stays with its value.
#[test]
fn widget_with_a_value_under_the_box_leaves_with_its_field() {
    let src = fixture();
    let mut editor = open_editor_from_bytes(&src, "redact_widget");
    editor.add_redaction(0, BOX, None).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, true);
    assert!(!contains(&out, b"fieldin"), "the field under the box is kept");
    assert!(!contains(&out, b"FIELDVALUE"));
    assert!(contains(&out, b"fieldout") && contains(&out, b"KEEPVALUE"));
}

/// The link and the note under the box are gone. Their twins outside it are
/// written byte for byte as a control save of the same page writes them.
#[test]
fn link_and_note_twins_outside_the_box_are_kept_byte_for_byte() {
    let src = fixture();
    let mut control = open_editor_from_bytes(&src, "redact_control");
    let control_out = save(&mut control, false);

    let mut editor = open_editor_from_bytes(&src, "redact_twins");
    editor.add_redaction(0, BOX, None).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, false);

    assert!(!contains(&out, b"LINK-INSIDE") && !contains(&out, b"NOTE-INSIDE"));
    for marker in [&b"(LINK-OUTSIDE)"[..], b"(NOTE-OUTSIDE)"] {
        assert_eq!(object_body(&out, marker), object_body(&control_out, marker));
    }
}

/// The render outside the padded box is equal to the render before redaction,
/// and inside it is the fill colour.
#[test]
fn render_outside_the_box_is_the_source_and_inside_is_the_fill() {
    let src = fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_render");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, true);

    assert_equal_outside(&before, &render(&out), BOX, 3.0);
    let inside = pixel_at(&render(&out), (BOX[0] + BOX[2]) / 2.0, (BOX[1] + BOX[3]) / 2.0);
    assert_eq!(inside, [255, 0, 0, 255], "the box is not drawn in the fill colour");
}

/// Identity-H text: the secret run is gone, the public run stays.
#[test]
fn identity_h_secret_run_is_removed_and_the_public_run_stays() {
    let content = b"BT /F1 12 Tf 1 0 0 1 100 220 Tm <004100420043> Tj ET\n\
                    BT /F1 12 Tf 1 0 0 1 100 100 Tm <004F004B> Tj ET\n";
    let cmap = "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
        /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
        1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n\
        5 beginbfchar\n<0041> <0053>\n<0042> <0045>\n<0043> <0043>\n<004F> <004F>\n<004B> <004B>\n\
        endbfchar\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend";
    let src = write_pdf(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream(4, content),
        obj(5, "<< /Type /Font /Subtype /Type0 /BaseFont /IDFix /Encoding /Identity-H \
                /DescendantFonts [6 0 R] /ToUnicode 8 0 R >>"),
        obj(
            6,
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /IDFix \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor 7 0 R /DW 1000 /W [65 [1000] 66 [1000] 67 [1000] 79 [1000] 75 [1000]] \
             /CIDToGIDMap /Identity >>",
        ),
        obj(
            7,
            "<< /Type /FontDescriptor /FontName /IDFix /Flags 4 /FontBBox [0 -200 1000 800] \
             /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 >>",
        ),
        stream(8, cmap.as_bytes()),
    ]);
    let mut editor = open_editor_from_bytes(&src, "redact_identity");
    editor.add_redaction(0, [95.0, 210.0, 160.0, 240.0], None).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, true);
    let text = page0_text(&out);
    assert!(!text.contains("SEC"), "text: {text}");
    assert!(text.contains("OK"), "text: {text}");
}

/// Inline image and text under the box, the overlay on: the text is gone, the
/// fill is drawn in the box, and the inline image's bytes outside are unchanged.
#[test]
fn inline_image_page_draws_the_overlay_and_keeps_the_image() {
    let src = fixture();
    let before = render(&src);
    let mut editor = open_editor_from_bytes(&src, "redact_inline");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, false);

    let text = page0_text(&out);
    assert!(!text.contains("TOPSECRET") && text.contains("PUBLIC"), "text: {text}");
    assert!(contains(&out, b"\x10\x80\x40\xf0"), "the inline image's bytes are not kept");
    let after = render(&out);
    let inside = pixel_at(&after, (BOX[0] + BOX[2]) / 2.0, (BOX[1] + BOX[3]) / 2.0);
    assert_eq!(inside, [255, 0, 0, 255], "the overlay is not drawn in the box");
    assert_equal_outside(&before, &after, BOX, 3.0);
}
