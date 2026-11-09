//! The editor's page index over a nested page tree.

use super::support::open_editor_from_bytes;
use crate::editor::EditableDocument;

/// Three pages under a nested tree: root `[inner(4, 5), 6]`.
fn nested_page_tree_pdf_bytes() -> Vec<u8> {
    let objs: Vec<&[u8]> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n",
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 3 >>\nendobj\n",
        b"3 0 obj\n<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 5 0 R] /Count 2 >>\nendobj\n",
        b"4 0 obj\n<< /Type /Page /Parent 3 0 R /MediaBox [0 0 100 100] >>\nendobj\n",
        b"5 0 obj\n<< /Type /Page /Parent 3 0 R /MediaBox [0 0 100 100] >>\nendobj\n",
        b"6 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n",
    ];
    let mut body = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for o in &objs {
        offsets.push(body.len());
        body.extend_from_slice(o);
    }
    let xref_offset = body.len();
    let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
    for off in &offsets {
        tail.push_str(&format!("{off:010} 00000 n \n"));
    }
    tail.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
        objs.len() + 1
    ));
    [body, tail.into_bytes()].concat()
}

/// No Dart battery sees it: it reads the editor's private output-to-source page index.
#[test]
fn output_index_follows_removes_and_moves() {
    let mut editor = open_editor_from_bytes(&nested_page_tree_pdf_bytes(), "output_index");
    assert_eq!(editor.output_to_source_index(1), 1);

    editor.remove_page(1).unwrap();
    assert_eq!(editor.output_to_source_index(0), 0);
    assert_eq!(editor.output_to_source_index(1), 2);

    editor.move_page(0, 1).unwrap();
    assert_eq!(editor.output_to_source_index(0), 2);
    assert_eq!(editor.output_to_source_index(1), 0);

    // Past the end falls back to the output index itself.
    assert_eq!(editor.output_to_source_index(5), 5);
}

// ── a merge records ids and streams the merged document at save ──

/// One page whose /Contents is `contents`, with the content streams `parts` as objects 4, 5, ….
fn page_with_contents(contents: &str, parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    use super::support::{pdf_from_objects, stream_object};
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents {contents} >>\nendobj\n")
            .into_bytes(),
    ];
    for (i, (dict, data)) in parts.iter().enumerate() {
        objects.push(stream_object(4 + i as u32, dict, data));
    }
    pdf_from_objects(&objects)
}

fn flate(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(1));
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// No Dart battery sees it: the page's images come back the same whether its content was held whole or read as operators, and the heap one call raises is reported by no wire op.
#[test]
fn the_images_of_a_page_with_a_huge_content_are_found_without_holding_the_content() {
    let image = b"q 10 0 0 20 5 6 cm /Im1 Do Q\n";
    let filler = b"q Q\n".repeat(4 * 1024 * 1024);
    let mut content = image.to_vec();
    content.extend_from_slice(&filler);
    let mut editor =
        open_editor_from_bytes(&page_with_contents("4 0 R", &[("/Filter /FlateDecode", flate(&content))]), "huge_content");
    let (images, grew) = crate::host::heap::growth(|| editor.get_page_images(0).unwrap());
    assert!(grew < content.len() / 4, "the read held {grew} bytes for a content of {}", content.len());
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].name, "Im1");
    assert_eq!(images[0].bounds, [5.0, 6.0, 10.0, 20.0]);
}

/// No Dart battery sees it: the parts of a page's /Contents array join the same whether they were concatenated or read in turn; a part that cannot be decoded is skipped, as it always was.
#[test]
fn the_images_of_a_page_with_a_contents_array_are_found_and_a_broken_part_is_skipped() {
    let mut editor = open_editor_from_bytes(
        &page_with_contents(
            "[4 0 R 5 0 R 6 0 R]",
            &[
                ("", b"q 10 0 0 20 5 6 cm /Im1 Do".to_vec()),
                ("/Filter /FlateDecode", b"not a zlib stream".to_vec()),
                ("", b"Q q 3 0 0 4 1 2 cm /Im2 Do Q".to_vec()),
            ],
        ),
        "contents_array",
    );
    let images = editor.get_page_images(0).unwrap();
    let names: Vec<_> = images.iter().map(|i| i.name.as_str()).collect();
    assert_eq!(names, ["Im1", "Im2"]);
    assert_eq!(images[1].bounds, [1.0, 2.0, 3.0, 4.0]);
}
