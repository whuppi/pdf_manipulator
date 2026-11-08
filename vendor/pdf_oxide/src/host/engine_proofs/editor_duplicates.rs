//! A full rewrite with `merge_duplicates` writes one copy of identical
//! images, ICC profiles and embedded font programs.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, EncryptionConfig, SaveOptions};
use crate::object::{Object, ObjectRef};
use std::collections::{BTreeMap, BTreeSet};

const IMAGE: &str = "/Type /XObject /Subtype /Image /Width 4 /Height 4 /BitsPerComponent 8";

/// Bytes that do not compress, so a re-deflate never changes a stream.
fn noise(len: usize, seed: u64) -> Vec<u8> {
    let mut state = 0x9e37_79b9_7f4a_7c15_u64 ^ seed;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}

/// A one-page document whose page has `resources`, followed by `extra`
/// objects numbered from 4.
fn document(resources: &str, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] /Resources << {resources} >> >>\nendobj\n"
        )
        .into_bytes(),
    ];
    objects.extend_from_slice(extra);
    pdf_from_objects(&objects)
}

fn merging() -> SaveOptions {
    SaveOptions { merge_duplicates: true, ..SaveOptions::full_rewrite() }
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

fn dict_of(obj: &Object) -> Option<&std::collections::HashMap<String, Object>> {
    match obj {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream { dict, .. } => Some(dict),
        _ => None,
    }
}

fn is_image(obj: &Object) -> bool {
    matches!(obj, Object::Stream { dict, .. }
        if dict.get("Subtype").and_then(|s| s.as_name()) == Some("Image"))
}

fn image_count(doc: &PdfDocument) -> usize {
    objects(doc).iter().filter(|(_, obj)| is_image(obj)).count()
}

/// The page's resource names of `class` to the object ids they name.
fn named(doc: &PdfDocument, class: &str) -> BTreeMap<String, u32> {
    let page = objects(doc)
        .into_iter()
        .find(|(_, obj)| {
            dict_of(obj).and_then(|d| d.get("Type")).and_then(|t| t.as_name()) == Some("Page")
        })
        .expect("the output has a page")
        .1;
    let resources = dict_of(&page).unwrap().get("Resources").unwrap();
    let resources = doc.resolve_object(resources).unwrap();
    let entries = resources.as_dict().unwrap().get(class).unwrap();
    let entries = doc.resolve_object(entries).unwrap();
    entries
        .as_dict()
        .unwrap()
        .iter()
        .map(|(name, value)| (name.clone(), value.as_reference().unwrap().id))
        .collect()
}

/// Every reference of every object names an object the file holds.
fn assert_closed(doc: &PdfDocument) {
    let ids: BTreeSet<u32> = doc
        .all_object_ids()
        .into_iter()
        .filter(|&id| !doc.is_free_entry(id))
        .collect();
    for id in &ids {
        let mut refs = Vec::new();
        doc.collect_refs_of(*id, &mut refs);
        for target in refs {
            assert!(ids.contains(&target), "object {id} names {target}, which the file lacks");
        }
    }
}

fn rgb(extra: &str) -> String {
    format!("{IMAGE} /ColorSpace /DeviceRGB {extra}")
}

fn two_images(first: &str, second: &str, data: &[u8]) -> Vec<u8> {
    document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[stream_object(4, first, data), stream_object(5, second, data)],
    )
}

/// No Dart battery sees it: Dart reads a PDF through its API, which resolves a reference and shows the same image for both; only the raw object graph says whether two references share one object.
#[test]
fn identical_images_merge_and_every_reference_names_the_kept_one() {
    let data = noise(48, 1);
    let source = two_images(&rgb(""), &rgb(""), &data);
    let output = saved(source.clone(), merging());
    assert_eq!(image_count(&output), 1);
    let names = named(&output, "XObject");
    assert_eq!(names["A"], names["B"]);
    assert_closed(&output);

    let kept = saved(source, SaveOptions::full_rewrite());
    assert_eq!(image_count(&kept), 2, "the option is off by default");
}

/// No Dart battery sees it: Dart never reads the dictionary of an image stream, so two images that differ only in a structure key look alike through its API.
#[test]
fn images_with_a_struct_parent_stay_two() {
    let data = noise(48, 2);
    let same = two_images(&rgb("/StructParent 0"), &rgb("/StructParent 0"), &data);
    assert_eq!(image_count(&saved(same, merging())), 2);
    let differ = two_images(&rgb("/StructParent 0"), &rgb("/StructParent 1"), &data);
    assert_eq!(image_count(&saved(differ, merging())), 2);
    let plural = two_images(&rgb("/StructParents 0"), &rgb("/StructParents 0"), &data);
    assert_eq!(image_count(&saved(plural, merging())), 2);
}

/// No Dart battery sees it: Dart never reads the dictionary of an image stream, so an optional-content key is invisible through its API.
#[test]
fn images_in_an_optional_content_group_stay_two() {
    let data = noise(48, 3);
    let source = document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[
            stream_object(4, &rgb("/OC 6 0 R"), &data),
            stream_object(5, &rgb("/OC 6 0 R"), &data),
            b"6 0 obj\n<< /Type /OCG /Name (layer) >>\nendobj\n".to_vec(),
        ],
    );
    assert_eq!(image_count(&saved(source, merging())), 2);
}

/// No Dart battery sees it: whether /Length is a number or a reference is a detail of the file that no Dart read shows.
#[test]
fn dictionaries_that_differ_only_in_length_merge() {
    let data = noise(48, 4);
    let indirect = [
        format!("5 0 obj\n<< {} /Length 6 0 R >>\nstream\n", rgb("")).into_bytes(),
        data.clone(),
        b"\nendstream\nendobj\n".to_vec(),
    ]
    .concat();
    let source = document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[
            stream_object(4, &rgb(""), &data),
            indirect,
            format!("6 0 obj\n{}\nendobj\n", data.len()).into_bytes(),
        ],
    );
    let output = saved(source, merging());
    assert_eq!(image_count(&output), 1);
    let names = named(&output, "XObject");
    assert_eq!(names["A"], names["B"]);
    assert_closed(&output);
}

/// No Dart battery sees it: the claim is about which objects the file holds, and Dart reads a colour space through the image, never as an object.
#[test]
fn images_naming_identical_icc_streams_merge_with_them() {
    let data = noise(48, 5);
    let icc = noise(300, 6);
    let source = document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[
            stream_object(4, &format!("{IMAGE} /ColorSpace [/ICCBased 6 0 R]"), &data),
            stream_object(5, &format!("{IMAGE} /ColorSpace [/ICCBased 7 0 R]"), &data),
            stream_object(6, "/N 3 /Alternate /DeviceRGB", &icc),
            stream_object(7, "/N 3 /Alternate /DeviceRGB", &icc),
        ],
    );
    let output = saved(source, merging());
    assert_eq!(image_count(&output), 1);
    let profiles = objects(&output)
        .iter()
        .filter(|(_, obj)| {
            matches!(obj, Object::Stream { dict, .. }
                if dict.contains_key("N") && dict.contains_key("Alternate"))
        })
        .count();
    assert_eq!(profiles, 1);
    assert_closed(&output);
}

/// No Dart battery sees it: Dart never reads a colour space's profile stream, so two profiles that differ in data look alike through its API.
#[test]
fn images_naming_different_icc_streams_stay_two() {
    let data = noise(48, 7);
    let source = document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[
            stream_object(4, &format!("{IMAGE} /ColorSpace [/ICCBased 6 0 R]"), &data),
            stream_object(5, &format!("{IMAGE} /ColorSpace [/ICCBased 7 0 R]"), &data),
            stream_object(6, "/N 3", &noise(300, 8)),
            stream_object(7, "/N 3", &noise(300, 9)),
        ],
    );
    let output = saved(source, merging());
    assert_eq!(image_count(&output), 2);
    assert_closed(&output);
}

/// No Dart battery sees it: Dart never reads a soft mask as an object, so two masks of equal size and different data look alike through its API.
#[test]
fn images_with_soft_masks_of_different_data_stay_two() {
    let data = noise(48, 10);
    let mask = format!("{IMAGE} /ColorSpace /DeviceGray");
    let source = document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[
            stream_object(4, &rgb("/SMask 6 0 R"), &data),
            stream_object(5, &rgb("/SMask 7 0 R"), &data),
            stream_object(6, &mask, &noise(16, 11)),
            stream_object(7, &mask, &noise(16, 12)),
        ],
    );
    let output = saved(source, merging());
    assert_eq!(image_count(&output), 4);
    assert_closed(&output);
}

/// No Dart battery sees it: Dart never reads a soft mask as an object, so it cannot tell one shared mask from two.
#[test]
fn images_with_identical_soft_masks_merge_with_them() {
    let data = noise(48, 13);
    let mask = format!("{IMAGE} /ColorSpace /DeviceGray");
    let source = document(
        "/XObject << /A 4 0 R /B 5 0 R >>",
        &[
            stream_object(4, &rgb("/SMask 6 0 R"), &data),
            stream_object(5, &rgb("/SMask 7 0 R"), &data),
            stream_object(6, &mask, &noise(16, 14)),
            stream_object(7, &mask, &noise(16, 14)),
        ],
    );
    let output = saved(source, merging());
    assert_eq!(image_count(&output), 2);
    assert_closed(&output);
}

fn font_document(first: &[u8], second: &[u8]) -> Vec<u8> {
    let descriptor = |id: u32, file: u32| {
        format!(
            "{id} 0 obj\n<< /Type /FontDescriptor /FontName /X /Flags 4 /FontFile2 {file} 0 R >>\nendobj\n"
        )
        .into_bytes()
    };
    let font = |id: u32, descriptor: u32| {
        format!(
            "{id} 0 obj\n<< /Type /Font /Subtype /TrueType /BaseFont /X /FontDescriptor {descriptor} 0 R >>\nendobj\n"
        )
        .into_bytes()
    };
    document(
        "/Font << /F1 8 0 R /F2 9 0 R >>",
        &[
            stream_object(4, "/Length1 600", first),
            stream_object(5, "/Length1 600", second),
            descriptor(6, 4),
            descriptor(7, 5),
            font(8, 6),
            font(9, 7),
        ],
    )
}

fn font_files(doc: &PdfDocument) -> BTreeSet<u32> {
    objects(doc)
        .iter()
        .filter_map(|(_, obj)| dict_of(obj)?.get("FontFile2")?.as_reference())
        .map(|r| r.id)
        .collect()
}

/// No Dart battery sees it: Dart never reads a font descriptor's file reference, so two fonts that embed one program and two that embed copies look alike through its API.
#[test]
fn identical_font_files_merge() {
    let program = noise(600, 15);
    let output = saved(font_document(&program, &program), merging());
    assert_eq!(font_files(&output).len(), 1);
    assert_closed(&output);

    let kept = saved(font_document(&program, &program), SaveOptions::full_rewrite());
    assert_eq!(font_files(&kept).len(), 2);
}

/// No Dart battery sees it: Dart never reads a font descriptor's file reference.
#[test]
fn different_font_files_stay_two() {
    let output = saved(font_document(&noise(600, 16), &noise(600, 17)), merging());
    assert_eq!(font_files(&output).len(), 2);
}

/// An AES-256 file the engine wrote, with an empty user password, whose two
/// images carry `len` bytes each.
fn encrypted_source(len: usize) -> Vec<u8> {
    let data = noise(len, 18);
    let plain = two_images(&rgb(""), &rgb(""), &data);
    let mut editor = DocumentEditor::from_bytes(plain).unwrap();
    editor
        .save_to_bytes_with_options(SaveOptions::with_encryption(EncryptionConfig::new("", "owner")))
        .unwrap()
}

fn merged_from_encrypted(len: usize) -> PdfDocument {
    let mut editor = DocumentEditor::from_bytes(encrypted_source(len)).unwrap();
    let options = SaveOptions { keep_source_encryption: false, ..merging() };
    PdfDocument::from_bytes(editor.save_to_bytes_with_options(options).unwrap()).unwrap()
}

/// No Dart battery sees it: the claim is how many image objects the saved file holds, and a Dart read of the output shows the same pages whether it holds one or two. The memory the save takes on an encrypted source is held by `mem_encrypted_duplicates` and by the `compress encrypted` row of the memory probe.
#[test]
fn an_encrypted_source_merges_a_small_image_and_a_large_one() {
    assert_eq!(image_count(&merged_from_encrypted(1000)), 1);
    let above = crate::host::deflate::MEMORY_STREAM as usize + 1000;
    assert_eq!(image_count(&merged_from_encrypted(above)), 1);
}

fn merge_source() -> Vec<u8> {
    two_images(&rgb(""), &rgb("/Interpolate true"), &noise(48, 19))
}

/// No Dart battery sees it: the claim is about one object shared by references from different sources, which no Dart read shows.
#[test]
fn an_image_merged_in_twice_is_written_once() {
    let source = document(
        "/XObject << /A 4 0 R >>",
        &[stream_object(4, &rgb(""), &noise(48, 20))],
    );
    let mut editor = DocumentEditor::from_bytes(source.clone()).unwrap();
    editor.merge_from_bytes(&source).unwrap();
    editor.merge_from_bytes(&source).unwrap();
    let bytes = editor.save_to_bytes_with_options(merging()).unwrap();
    let output = PdfDocument::from_bytes(bytes).unwrap();
    assert_eq!(image_count(&output), 1);
    assert_closed(&output);

    let mut editor = DocumentEditor::from_bytes(source.clone()).unwrap();
    editor.merge_from_bytes(&source).unwrap();
    let bytes = editor.save_to_bytes_with_options(SaveOptions::full_rewrite()).unwrap();
    assert_eq!(image_count(&PdfDocument::from_bytes(bytes).unwrap()), 2);
}

/// No Dart battery sees it: the claim is about which of two near-identical image objects the file holds, and a Dart read resolves both to a picture.
#[test]
fn images_that_differ_in_one_entry_stay_two() {
    let output = saved(merge_source(), merging());
    assert_eq!(image_count(&output), 2);
}

/// No Dart battery sees it: the output bytes of two saves are compared as raw bytes.
#[test]
fn a_merging_save_writes_the_same_bytes_twice() {
    let data = noise(48, 21);
    let source = two_images(&rgb(""), &rgb(""), &data);
    let mut editor = DocumentEditor::from_bytes(source).unwrap();
    let first = editor.save_to_bytes_with_options(merging()).unwrap();
    let second = editor.save_to_bytes_with_options(merging()).unwrap();
    assert_eq!(first, second);
}
