//! A full rewrite with `prune_resources` drops the entries of a resource
//! dictionary that no content using it names, and with them the objects
//! only those entries reached.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::object::{Object, ObjectRef};
use std::collections::{BTreeSet, HashMap};

type Dict = HashMap<String, Object>;

fn pruning() -> SaveOptions {
    SaveOptions { prune_resources: true, ..SaveOptions::full_rewrite() }
}

fn plain(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

fn content(id: u32, data: &str) -> Vec<u8> {
    stream_object(id, "", data.as_bytes())
}

fn font(id: u32, name: &str) -> Vec<u8> {
    plain(id, &format!("<< /Type /Font /Subtype /Type1 /BaseFont /{name} >>"))
}

/// An image whose width tells it from the others.
fn image(id: u32, width: u32) -> Vec<u8> {
    stream_object(
        id,
        &format!(
            "/Type /XObject /Subtype /Image /Width {width} /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray"
        ),
        &[7],
    )
}

fn form(id: u32, extra: &str, data: &str) -> Vec<u8> {
    stream_object(id, &format!("/Type /XObject /Subtype /Form /BBox [0 0 10 10] {extra}"), data.as_bytes())
}

/// Catalog, page tree and one page (3) whose contents are object 4,
/// followed by `extra` objects numbered from 5.
fn document(page: &str, contents: Vec<u8>, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(3, &format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R {page} >>")),
        contents,
    ];
    objects.extend_from_slice(extra);
    pdf_from_objects(&objects)
}

fn saved(source: Vec<u8>, options: SaveOptions) -> PdfDocument {
    let mut editor = DocumentEditor::from_bytes(source).unwrap();
    PdfDocument::from_bytes(editor.save_to_bytes_with_options(options).unwrap()).unwrap()
}

fn objects(doc: &PdfDocument) -> Vec<(u32, Object)> {
    doc.all_object_ids()
        .into_iter()
        .filter(|&id| id != 0 && !doc.is_free_entry(id))
        .filter_map(|id| doc.load_object(ObjectRef::new(id, 0)).ok().map(|obj| (id, obj)))
        .collect()
}

fn dict_of(obj: &Object) -> Option<&Dict> {
    match obj {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream { dict, .. } => Some(dict),
        _ => None,
    }
}

fn resolved(doc: &PdfDocument, value: &Object) -> Option<Dict> {
    dict_of(&doc.resolve_object(value).ok()?).cloned()
}

fn first_page(doc: &PdfDocument) -> Dict {
    objects(doc)
        .into_iter()
        .filter_map(|(id, obj)| dict_of(&obj).cloned().map(|d| (id, d)))
        .filter(|(_, d)| d.get("Type").and_then(|t| t.as_name()) == Some("Page"))
        .min_by_key(|(id, _)| *id)
        .expect("the output has a page")
        .1
}

/// The names of subdictionary `class` of `resources`; empty when it is absent.
fn entries(doc: &PdfDocument, resources: &Dict, class: &str) -> BTreeSet<String> {
    resources
        .get(class)
        .and_then(|sub| resolved(doc, sub))
        .map(|sub| sub.keys().cloned().collect())
        .unwrap_or_default()
}

fn page_entries(doc: &PdfDocument, class: &str) -> BTreeSet<String> {
    let page = first_page(doc);
    let resources = resolved(doc, page.get("Resources").expect("the page has resources")).unwrap();
    entries(doc, &resources, class)
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn font_programs(doc: &PdfDocument) -> BTreeSet<String> {
    objects(doc)
        .iter()
        .filter_map(|(_, obj)| dict_of(obj))
        .filter(|d| d.get("Type").and_then(|t| t.as_name()) == Some("Font"))
        .filter_map(|d| d.get("BaseFont").and_then(|b| b.as_name()).map(str::to_string))
        .collect()
}

fn image_widths(doc: &PdfDocument) -> BTreeSet<i64> {
    objects(doc)
        .iter()
        .filter_map(|(_, obj)| dict_of(obj))
        .filter(|d| d.get("Subtype").and_then(|s| s.as_name()) == Some("Image"))
        .filter_map(|d| d.get("Width").and_then(|w| w.as_integer()))
        .collect()
}

fn count_of_type(doc: &PdfDocument, kind: &str) -> usize {
    objects(doc)
        .iter()
        .filter_map(|(_, obj)| dict_of(obj))
        .filter(|d| d.get("Type").and_then(|t| t.as_name()) == Some(kind))
        .count()
}

fn used_and_unused() -> Vec<u8> {
    document(
        "/Resources << /Font << /F1 5 0 R /F2 6 0 R >> /XObject << /Im1 7 0 R /Im2 8 0 R >> >>",
        content(4, "BT /F1 12 Tf (x) Tj ET q 1 0 0 1 0 0 cm /Im1 Do Q"),
        &[font(5, "Used"), font(6, "Unused"), image(7, 3), image(8, 7)],
    )
}

/// A page with fonts `F1` and `F2` and the given content object (4), which
/// names only `F1`.
fn two_fonts(contents: Vec<u8>, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut objects = vec![font(5, "First"), font(6, "Second")];
    objects.extend_from_slice(extra);
    document("/Resources << /Font << /F1 5 0 R /F2 6 0 R >> >>", contents, &objects)
}

/// The save keeps both fonts of [`two_fonts`]: the content that names `F1`
/// cannot be trusted to name every font it uses.
fn assert_whole(source: Vec<u8>) {
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1", "F2"]));
    assert_eq!(font_programs(&output), names(&["First", "Second"]));
}

/// No Dart battery sees it: Dart reads a page through its API, which shows what a page paints and never which objects the file holds, so a font nothing paints is invisible there.
#[test]
fn an_unused_font_and_image_are_dropped_and_not_written() {
    let output = saved(used_and_unused(), pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1"]));
    assert_eq!(page_entries(&output, "XObject"), names(&["Im1"]));
    assert_eq!(font_programs(&output), names(&["Used"]));
    assert_eq!(image_widths(&output), BTreeSet::from([3]));
}

/// No Dart battery sees it: the option's default is a property of the engine's save, and a Dart read shows the same page either way.
#[test]
fn the_option_is_off_by_default() {
    let output = saved(used_and_unused(), SaveOptions::full_rewrite());
    assert_eq!(page_entries(&output, "Font"), names(&["F1", "F2"]));
    assert_eq!(font_programs(&output), names(&["Used", "Unused"]));
    assert_eq!(image_widths(&output), BTreeSet::from([3, 7]));
}

/// No Dart battery sees it: a resource dictionary two pages share is one object in the file, which a Dart read of either page hides.
#[test]
fn a_dictionary_two_pages_share_keeps_the_union() {
    let source = pdf_from_objects(&[
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"),
        plain(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 5 0 R /Resources 7 0 R >>"),
        plain(4, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 6 0 R /Resources 7 0 R >>"),
        content(5, "BT /F1 12 Tf ET"),
        content(6, "BT /F2 12 Tf ET"),
        plain(7, "<< /Font << /F1 8 0 R /F2 9 0 R /F3 10 0 R >> >>"),
        font(8, "First"),
        font(9, "Second"),
        font(10, "Third"),
    ]);
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1", "F2"]));
    assert_eq!(font_programs(&output), names(&["First", "Second"]));
}

/// No Dart battery sees it: whether a Form XObject carries resources of its own is a detail of its dictionary that no Dart read shows.
#[test]
fn a_form_without_resources_keeps_the_names_it_uses_from_the_page() {
    let source = document(
        "/Resources << /Font << /F1 5 0 R /F2 6 0 R >> /XObject << /Fm1 7 0 R >> >>",
        content(4, "/Fm1 Do"),
        &[font(5, "Unused"), font(6, "Used"), form(7, "", "BT /F2 9 Tf ET")],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F2"]));
    assert_eq!(font_programs(&output), names(&["Used"]));
}

/// No Dart battery sees it: a form's own resources are cut once however many pages paint it, a count no Dart read shows.
#[test]
fn a_form_two_pages_paint_keeps_the_names_it_uses() {
    let source = pdf_from_objects(&[
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 5 0 R \
             /Resources << /XObject << /Fm1 7 0 R >> >> >>",
        ),
        plain(
            4,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 6 0 R \
             /Resources << /XObject << /Fm1 7 0 R >> >> >>",
        ),
        content(5, "/Fm1 Do"),
        content(6, "/Fm1 Do"),
        form(7, "/Resources << /Font << /F1 8 0 R /F2 9 0 R >> >>", "BT /F1 9 Tf ET"),
        font(8, "Used"),
        font(9, "Unused"),
    ]);
    let output = saved(source, pruning());
    assert_eq!(font_programs(&output), names(&["Used"]));
    let (_, form) = objects(&output)
        .into_iter()
        .find(|(_, obj)| dict_of(obj).is_some_and(|d| d.get("Subtype").and_then(|s| s.as_name()) == Some("Form")))
        .expect("the form is written");
    let own = resolved(&output, dict_of(&form).unwrap().get("Resources").unwrap()).unwrap();
    assert_eq!(entries(&output, &own, "Font"), names(&["F1"]));
}

/// No Dart battery sees it: the glyph procedures of a Type 3 font are streams only the raw object graph shows.
#[test]
fn a_type3_glyph_procedure_keeps_the_names_it_uses() {
    let source = document(
        "/Resources << /Font << /T3 5 0 R >> /XObject << /Im1 7 0 R /Im2 8 0 R >> >>",
        content(4, "BT /T3 12 Tf (a) Tj ET"),
        &[
            plain(
                5,
                "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1 1] /FontMatrix [1 0 0 1 0 0] \
                 /CharProcs << /a 6 0 R >> /FirstChar 97 /LastChar 97 /Widths [1] >>",
            ),
            content(6, "1 0 d0 /Im1 Do"),
            image(7, 3),
            image(8, 7),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "XObject"), names(&["Im1"]));
    assert_eq!(image_widths(&output), BTreeSet::from([3]));
}

/// No Dart battery sees it: marked-content property lists are resources no Dart API reads.
#[test]
fn marked_content_properties_named_by_bdc_and_dp_stay() {
    let source = document(
        "/Resources << /Properties << /MC0 5 0 R /MC1 6 0 R /MC2 7 0 R >> >>",
        content(4, "/OC /MC0 BDC 0 0 1 1 re f EMC /Tag /MC2 DP"),
        &[
            plain(5, "<< /Type /OCG /Name (a) >>"),
            plain(6, "<< /Type /OCG /Name (b) >>"),
            plain(7, "<< /Type /OCG /Name (c) >>"),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Properties"), names(&["MC0", "MC2"]));
    assert_eq!(count_of_type(&output, "OCG"), 2);
}

/// No Dart battery sees it: an inline image's color space name is read from the content stream, which no Dart API exposes.
#[test]
fn an_inline_images_color_space_name_stays() {
    let source = document(
        "/Resources << /ColorSpace << /Cs1 [/CalGray << /WhitePoint [0.95 1 1.09] >>] \
         /Cs2 [/CalGray << /WhitePoint [0.95 1 1.09] >>] >> >>",
        content(4, "q 1 0 0 1 0 0 cm BI /W 1 /H 1 /BPC 8 /CS /Cs1 ID A EI Q"),
        &[],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "ColorSpace"), names(&["Cs1"]));
}

/// No Dart battery sees it: a tiling pattern's own resource dictionary is reachable only through the raw object graph.
#[test]
fn a_patterns_own_resources_are_pruned_by_its_content() {
    let source = document(
        "/Resources << /Pattern << /P1 5 0 R >> /Font << /F9 8 0 R >> >>",
        content(4, "/Pattern cs /P1 scn 0 0 10 10 re f"),
        &[
            stream_object(
                5,
                "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 4 4] \
                 /XStep 4 /YStep 4 /Resources << /Font << /F1 6 0 R /F2 7 0 R >> >>",
                b"BT /F1 4 Tf (a) Tj ET",
            ),
            font(6, "PatternUsed"),
            font(7, "PatternUnused"),
            font(8, "PageUnused"),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(font_programs(&output), names(&["PatternUsed"]));
    assert_eq!(page_entries(&output, "Pattern"), names(&["P1"]));
    assert!(page_entries(&output, "Font").is_empty());
    let (_, pattern) = objects(&output)
        .into_iter()
        .find(|(_, obj)| dict_of(obj).is_some_and(|d| d.get("PatternType").is_some()))
        .expect("the pattern is written");
    let own = resolved(&output, dict_of(&pattern).unwrap().get("Resources").unwrap()).unwrap();
    assert_eq!(entries(&output, &own, "Font"), names(&["F1"]));
    assert_eq!(pattern.decode_stream_data().unwrap(), b"BT /F1 4 Tf (a) Tj ET");
}

/// No Dart battery sees it: an annotation's appearance stream and its resources are objects no Dart API reads.
#[test]
fn an_appearance_streams_resources_are_pruned_by_its_content() {
    let source = document(
        "/Annots [5 0 R] /Resources << >>",
        content(4, ""),
        &[
            plain(5, "<< /Type /Annot /Subtype /Square /Rect [0 0 5 5] /AP << /N 6 0 R >> >>"),
            form(6, "/Resources << /Font << /F1 7 0 R /F2 8 0 R >> >>", "BT /F1 4 Tf ET"),
            font(7, "ApUsed"),
            font(8, "ApUnused"),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(font_programs(&output), names(&["ApUsed"]));
}

/// No Dart battery sees it: some viewers read an appearance stream without resources of its own against the page's, and a Dart read shows neither which fonts the file keeps nor how a viewer resolves them.
#[test]
fn an_appearance_stream_without_resources_keeps_the_page_font_it_uses() {
    let source = document(
        "/Annots [5 0 R] /Resources << /Font << /F1 7 0 R /F2 8 0 R >> >>",
        content(4, ""),
        &[
            plain(5, "<< /Type /Annot /Subtype /Square /Rect [0 0 5 5] /AP << /N 6 0 R >> >>"),
            form(6, "", "BT /F1 4 Tf ET"),
            font(7, "ApUsed"),
            font(8, "ApUnused"),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1"]));
    assert_eq!(font_programs(&output), names(&["ApUsed"]));
}

/// No Dart battery sees it: a content stream the engine cannot decode still renders as far as it can, and a Dart read does not say which fonts the file keeps.
#[test]
fn an_unopenable_content_stream_leaves_its_dictionary_whole() {
    assert_whole(two_fonts(stream_object(4, "/Filter /FlateDecode", b"not zlib data"), &[]));
}

/// No Dart battery sees it: the read error of a content stream half way through is internal to the source, and a Dart read shows only what the engine could draw.
#[test]
fn a_content_stream_that_fails_half_way_leaves_its_dictionary_whole() {
    // `BT /F1 12 Tf ET` and a newline, then zero groups (NUL bytes, which
    // are white-space), then a byte no ASCII85 stream holds.
    let mut packed = ascii85(b"BT /F1 12 Tf ET\n");
    packed.extend(b"z".repeat(20_000));
    packed.push(b'{');
    assert_whole(two_fonts(stream_object(4, "/Filter /ASCII85Decode", &packed), &[]));
}

/// `data` (a multiple of four bytes) as ASCII85 groups.
fn ascii85(data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len() % 4, 0);
    let mut out = Vec::new();
    for group in data.chunks(4) {
        let mut value = u32::from_be_bytes(group.try_into().unwrap());
        let mut digits = [0u8; 5];
        for digit in digits.iter_mut().rev() {
            *digit = (value % 85) as u8 + b'!';
            value /= 85;
        }
        out.extend_from_slice(&digits);
    }
    out
}

/// No Dart battery sees it: a part of an array of content streams that cannot be decoded is skipped by the reader, which no Dart read reports.
#[test]
fn an_array_of_contents_with_an_unreadable_part_leaves_its_dictionary_whole() {
    let source = pdf_from_objects(&[
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents [4 0 R 7 0 R] \
             /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>",
        ),
        content(4, "BT /F1 12 Tf ET"),
        font(5, "First"),
        font(6, "Second"),
        stream_object(7, "/Filter /FlateDecode", b"not zlib data"),
    ]);
    assert_whole(source);
}

/// No Dart battery sees it: the content parser's recovery from bad bytes is internal, and a Dart read shows only what the engine could draw.
#[test]
fn a_content_stream_the_parser_gave_up_on_leaves_its_dictionary_whole() {
    let junk = ")".repeat(3000);
    assert_whole(two_fonts(content(4, &format!("{junk}\nBT /F1 12 Tf ET")), &[]));
}

/// No Dart battery sees it: a byte the parser skipped is counted inside the source, which no Dart read reports.
#[test]
fn a_content_stream_with_a_skipped_byte_leaves_its_dictionary_whole() {
    assert_whole(two_fonts(content(4, "BT /F1 12 Tf ET ] q Q"), &[]));
}

/// No Dart battery sees it: a string the content ends inside of is damage the source reports, which no Dart read shows.
#[test]
fn a_content_stream_ending_inside_a_string_leaves_its_dictionary_whole() {
    assert_whole(two_fonts(content(4, "BT /F1 12 Tf ET (never closed"), &[]));
}

/// No Dart battery sees it: an inline image without an end is damage the source reports, which no Dart read shows.
#[test]
fn a_content_stream_ending_inside_an_inline_image_leaves_its_dictionary_whole() {
    assert_whole(two_fonts(content(4, "BT /F1 12 Tf ET BI /W 1 /H 1 /BPC 8 /CS /G ID A"), &[]));
}

/// No Dart battery sees it: an operator with more operands than any takes loses its first, which the source counts and no Dart read reports.
#[test]
fn an_operator_with_too_many_operands_leaves_its_dictionary_whole() {
    let operands = "1 ".repeat(40);
    assert_whole(two_fonts(content(4, &format!("BT /F1 12 Tf ET {operands}m")), &[]));
}

/// No Dart battery sees it: a long array sent out in chunks that turns out not to be a TJ array is damage the source counts, which no Dart read reports.
#[test]
fn a_long_array_of_another_operator_leaves_its_dictionary_whole() {
    let elements = "1 ".repeat(5000);
    assert_whole(two_fonts(content(4, &format!("BT /F1 12 Tf ET [{elements}] 0 d")), &[]));
}

/// No Dart battery sees it: a name an operator held and no arm of it uses may be a resource the source misread, which no Dart read exposes.
#[test]
fn a_name_no_operator_arm_consumed_that_is_a_resource_key_leaves_its_dictionary_whole() {
    assert_whole(two_fonts(content(4, "BT /F2 /F1 12 Tf ET"), &[]));
}

/// No Dart battery sees it: a name that is no key of the page's resources says nothing about them.
#[test]
fn a_name_no_operator_arm_consumed_that_is_no_resource_key_prunes() {
    let output = saved(two_fonts(content(4, "/Spare BT /F1 12 Tf ET"), &[]), pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1"]));
    assert_eq!(font_programs(&output), names(&["First"]));
}

/// No Dart battery sees it: whether a `Do` resolves against the page's resources is a detail of the file.
#[test]
fn a_do_whose_name_resolves_nowhere_pins_nothing() {
    let output =
        saved(two_fonts(content(4, "/Nowhere Do BT /F1 12 Tf ET"), &[]), pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1"]));
    assert_eq!(font_programs(&output), names(&["First"]));
}

/// No Dart battery sees it: a form that paints itself is a cycle in the object graph, which a Dart read of the page does not show.
#[test]
fn forms_that_paint_each_other_keep_their_resources_whole() {
    let source = document(
        "/Resources << /XObject << /Fm1 5 0 R >> >>",
        content(4, "/Fm1 Do"),
        &[
            form(
                5,
                "/Resources << /XObject << /Fm2 6 0 R >> /Font << /F1 7 0 R /F2 8 0 R >> >>",
                "/Fm2 Do BT /F1 4 Tf ET",
            ),
            form(
                6,
                "/Resources << /XObject << /Fm1 5 0 R >> /Font << /G1 9 0 R /G2 10 0 R >> >>",
                "/Fm1 Do BT /G1 4 Tf ET",
            ),
            font(7, "FirstUsed"),
            font(8, "FirstUnused"),
            font(9, "SecondUsed"),
            font(10, "SecondUnused"),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(
        font_programs(&output),
        names(&["FirstUsed", "FirstUnused", "SecondUsed", "SecondUnused"])
    );
}

/// No Dart battery sees it: how deep forms nest is a property of the file's object graph.
#[test]
fn forms_nested_beyond_the_limit_keep_the_resources_of_the_cut() {
    const DEPTH: u32 = 70;
    let mut objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R \
             /Resources << /XObject << /N 7 0 R >> >> >>",
        ),
        content(4, "/N Do"),
        font(5, "Used"),
        font(6, "Unused"),
    ];
    for k in 1..=DEPTH {
        let id = 6 + k;
        let next = if k < DEPTH { format!("/XObject << /N {} 0 R >>", id + 1) } else { String::new() };
        objects.push(form(
            id,
            &format!("/Seq {k} /Resources << {next} /Font << /Fa 5 0 R /Fb 6 0 R >> >>"),
            "/N Do BT /Fa 4 Tf ET",
        ));
    }
    let output = saved(pdf_from_objects(&objects), pruning());
    let fonts_of = |seq: i64| -> BTreeSet<String> {
        let (_, form) = objects_with_seq(&output, seq);
        let own = resolved(&output, dict_of(&form).unwrap().get("Resources").unwrap()).unwrap();
        entries(&output, &own, "Font")
    };
    assert_eq!(fonts_of(1), names(&["Fa"]));
    assert_eq!(fonts_of(63), names(&["Fa"]));
    assert_eq!(fonts_of(64), names(&["Fa", "Fb"]));
    assert_eq!(fonts_of(65), names(&["Fa", "Fb"]));
}

fn objects_with_seq(doc: &PdfDocument, seq: i64) -> (u32, Object) {
    objects(doc)
        .into_iter()
        .find(|(_, obj)| {
            dict_of(obj).and_then(|d| d.get("Seq")).and_then(|s| s.as_integer()) == Some(seq)
        })
        .expect("the form is written")
}

/// No Dart battery sees it: whether a form's own resources can be read is a detail of its dictionary that no Dart read shows.
#[test]
fn a_form_whose_resources_cannot_be_read_keeps_its_painters_whole() {
    let source = document(
        "/Resources << /Font << /F1 5 0 R /F2 6 0 R >> /XObject << /Fm1 7 0 R >> >>",
        content(4, "/Fm1 Do BT /F1 12 Tf ET"),
        &[font(5, "First"), font(6, "Second"), form(7, "/Resources 8 0 R", "BT /F2 9 Tf ET"), plain(8, "42")],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Font"), names(&["F1", "F2"]));
}

/// No Dart battery sees it: the AcroForm's default resources are a catalog entry, and a Dart read of the form shows its fields, not the dictionary they fall back on.
#[test]
fn the_acroform_default_resources_are_unchanged() {
    let source = pdf_from_objects(&[
        plain(
            1,
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [] \
             /DR << /Font << /Helv 5 0 R /Extra 7 0 R >> >> >> >>",
        ),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R /Resources << /Font << /Other 6 0 R >> >> >>"),
        content(4, "BT /Other 12 Tf ET"),
        font(5, "Helv"),
        font(6, "Other"),
        font(7, "Extra"),
    ]);
    let output = saved(source, pruning());
    assert_eq!(font_programs(&output), names(&["Helv", "Other", "Extra"]));
}

/// No Dart battery sees it: whether the default resources and a page share one font dictionary object is a detail of the file.
#[test]
fn a_font_dictionary_shared_with_the_acroform_stays_whole() {
    let source = pdf_from_objects(&[
        plain(
            1,
            "<< /Type /Catalog /Pages 2 0 R /AcroForm << /Fields [] /DR << /Font 7 0 R >> >> >>",
        ),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R /Resources << /Font 7 0 R >> >>"),
        content(4, "BT /Other 12 Tf ET"),
        font(5, "Helv"),
        font(6, "Other"),
        plain(7, "<< /Helv 5 0 R /Other 6 0 R >>"),
    ]);
    let output = saved(source, pruning());
    assert_eq!(font_programs(&output), names(&["Helv", "Other"]));
}

/// No Dart battery sees it: which objects point at one resource dictionary is the file's structure, not something a Dart read shows.
#[test]
fn a_dictionary_an_unwalked_object_also_points_at_stays_whole() {
    let source = document(
        "/Resources 7 0 R",
        content(4, "BT /F1 12 Tf ET"),
        &[
            font(5, "First"),
            font(6, "Second"),
            plain(7, "<< /Font << /F1 5 0 R /F2 6 0 R >> >>"),
            form(8, "/Resources 7 0 R", "BT /F2 9 Tf ET"),
        ],
    );
    let output = saved(source, pruning());
    assert_eq!(font_programs(&output), names(&["First", "Second"]));
}

/// No Dart battery sees it: a shading dictionary and the color spaces it names are objects and names no Dart API reads.
#[test]
fn a_shading_keeps_the_color_space_it_names() {
    let source = document(
        "/Resources << /Shading << /Sh1 5 0 R >> /ColorSpace << /Cs1 [/CalGray << /WhitePoint [0.95 1 1.09] >>] \
         /Cs2 [/CalGray << /WhitePoint [0.95 1 1.09] >>] >> >>",
        content(4, "/Sh1 sh"),
        &[plain(
            5,
            "<< /ShadingType 2 /ColorSpace /Cs1 /Coords [0 0 1 1] \
             /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >>",
        )],
    );
    let output = saved(source, pruning());
    assert_eq!(page_entries(&output, "Shading"), names(&["Sh1"]));
    assert_eq!(page_entries(&output, "ColorSpace"), names(&["Cs1"]));
}

/// No Dart battery sees it: whether the save edits a page's content while writing is internal to the engine, and a Dart read shows the page's fonts as drawn.
#[test]
fn a_page_the_save_edits_keeps_its_resources() {
    let mut editor = DocumentEditor::from_bytes(used_and_unused()).unwrap();
    editor.erase_region(0, [0.0, 0.0, 1.0, 1.0]).unwrap();
    let bytes = editor.save_to_bytes_with_options(pruning()).unwrap();
    let output = PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(font_programs(&output), names(&["Used", "Unused"]));
}

/// A one-page document to merge in: page resources of fonts and images, one of each used.
fn merged_source(resources: &str, extra: &[Vec<u8>], contents: &str) -> Vec<u8> {
    let mut objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            &format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Contents 4 0 R {resources} >>"),
        ),
        content(4, contents),
        font(5, "MergedUsed"),
        font(6, "MergedUnused"),
        image(7, 3),
        image(8, 7),
    ];
    objects.extend_from_slice(extra);
    pdf_from_objects(&objects)
}

fn merged_output(source: &[u8]) -> PdfDocument {
    let mut editor =
        DocumentEditor::from_bytes(crate::api::Pdf::from_text("base page").unwrap().into_bytes())
            .unwrap();
    assert_eq!(editor.merge_from_bytes(source).unwrap(), 1);
    PdfDocument::from_bytes(editor.save_to_bytes_with_options(pruning()).unwrap()).unwrap()
}

fn merged_page_entries(doc: &PdfDocument, class: &str) -> BTreeSet<String> {
    let page = doc.get_page(1).unwrap();
    let resources = resolved(doc, dict_of(&page).unwrap().get("Resources").unwrap()).unwrap();
    entries(doc, &resources, class)
}

/// No Dart battery sees it: a Dart read shows what a merged page paints, never which objects the file holds.
#[test]
fn a_merged_pages_unused_font_and_image_are_dropped_and_the_used_ones_kept() {
    let source = merged_source(
        "/Resources << /Font << /F1 5 0 R /F2 6 0 R >> /XObject << /Im1 7 0 R /Im2 8 0 R >> >>",
        &[],
        "BT /F1 12 Tf (x) Tj ET q 1 0 0 1 0 0 cm /Im1 Do Q",
    );
    let output = merged_output(&source);
    assert_eq!(merged_page_entries(&output, "Font"), names(&["F1"]));
    assert_eq!(merged_page_entries(&output, "XObject"), names(&["Im1"]));
    let page = output.get_page(1).unwrap();
    let resources = resolved(&output, dict_of(&page).unwrap().get("Resources").unwrap()).unwrap();
    let fonts = resolved(&output, resources.get("Font").unwrap()).unwrap();
    let used = resolved(&output, fonts.get("F1").unwrap()).unwrap();
    assert_eq!(used.get("BaseFont").and_then(|b| b.as_name()), Some("MergedUsed"));
    let images = resolved(&output, resources.get("XObject").unwrap()).unwrap();
    let image = resolved(&output, images.get("Im1").unwrap()).unwrap();
    assert_eq!(image.get("Width").and_then(|w| w.as_integer()), Some(3));
    assert!(!font_programs(&output).contains("MergedUnused"));
    assert_eq!(image_widths(&output), BTreeSet::from([3]));
}

/// No Dart battery sees it: resource dictionaries held by reference, and a form's own, are objects of the file that a Dart read does not show.
#[test]
fn a_merged_pages_indirect_resources_and_form_are_pruned_with_their_ids_remapped() {
    let source = merged_source(
        "/Resources 9 0 R",
        &[
            plain(9, "<< /Font << /F1 5 0 R /F2 6 0 R >> /XObject << /Fm1 10 0 R >> >>"),
            form(
                10,
                "/Resources << /XObject << /Im1 7 0 R /Im2 8 0 R >> >>",
                "q 1 0 0 1 0 0 cm /Im1 Do Q",
            ),
        ],
        "BT /F1 12 Tf ET /Fm1 Do",
    );
    let output = merged_output(&source);
    assert_eq!(merged_page_entries(&output, "Font"), names(&["F1"]));
    assert_eq!(merged_page_entries(&output, "XObject"), names(&["Fm1"]));
    assert!(font_programs(&output).contains("MergedUsed"));
    assert!(!font_programs(&output).contains("MergedUnused"));
    assert_eq!(image_widths(&output), BTreeSet::from([3]));
    let (_, form) = objects(&output)
        .into_iter()
        .find(|(_, obj)| dict_of(obj).is_some_and(|d| d.get("Subtype").and_then(|s| s.as_name()) == Some("Form")))
        .expect("the form is written");
    assert_eq!(form.decode_stream_data().unwrap(), b"q 1 0 0 1 0 0 cm /Im1 Do Q");
    let own = resolved(&output, dict_of(&form).unwrap().get("Resources").unwrap()).unwrap();
    assert_eq!(entries(&output, &own, "XObject"), names(&["Im1"]));
}

/// No Dart battery sees it: a merged page's content that cannot be trusted keeps its fonts, which no Dart read shows.
#[test]
fn a_merged_pages_damaged_content_leaves_its_dictionary_whole() {
    let source = merged_source(
        "/Resources << /Font << /F1 5 0 R /F2 6 0 R >> >>",
        &[],
        "BT /F1 12 Tf ET (never closed",
    );
    let output = merged_output(&source);
    assert_eq!(merged_page_entries(&output, "Font"), names(&["F1", "F2"]));
}
