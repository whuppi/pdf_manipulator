//! Page facts, the page index and the object cache's size estimate.

use crate::document::{estimated_object_size, PageFacts, PdfDocument};
use crate::object::{Object, ObjectRef};

/// Build a minimal PDF with a multi-page structure (given page count).
fn build_multi_page_pdf(page_count: usize) -> Vec<u8> {
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets: Vec<usize> = Vec::new();

    // Object 1: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // Object 2: Pages (we'll build the Kids array)
    offsets.push(pdf.len());
    let kids_str: String = (0..page_count)
        .map(|i| format!("{} 0 R", i + 3))
        .collect::<Vec<_>>()
        .join(" ");
    let pages_obj = format!(
        "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
        kids_str, page_count
    );
    pdf.extend_from_slice(pages_obj.as_bytes());

    // Objects 3..3+page_count: Page objects (no /Contents, blank pages)
    for _i in 0..page_count {
        offsets.push(pdf.len());
        let page_obj = format!(
            "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
            offsets.len()
        );
        pdf.extend_from_slice(page_obj.as_bytes());
    }

    let xref_off = pdf.len();
    let total_objs = offsets.len() + 1; // +1 for object 0
    pdf.extend_from_slice(format!("xref\n0 {}\n", total_objs).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        pdf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            total_objs, xref_off
        )
        .as_bytes(),
    );

    pdf
}

/// No Dart battery sees it: it compares the object cache's size estimate with an independent sum.
#[test]
fn test_estimate_size_counts_a_page_dictionary_heap() {
    use std::collections::HashMap;
    use std::mem::size_of;
    let mut font = HashMap::new();
    font.insert("F1".to_string(), Object::Reference(ObjectRef::new(5, 0)));
    let mut resources = HashMap::new();
    resources.insert("Font".to_string(), Object::Dictionary(font));
    let mut page = HashMap::new();
    page.insert("Type".to_string(), Object::Name("Page".to_string()));
    page.insert("Parent".to_string(), Object::Reference(ObjectRef::new(2, 0)));
    page.insert(
        "MediaBox".to_string(),
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]),
    );
    page.insert("Resources".to_string(), Object::Dictionary(resources));
    let page = Object::Dictionary(page);

    // Computed independently of the estimator: 4 entries need 8 slots,
    // 1 entry needs 1 slot; a slot is a String, an Object and a control
    // byte. Key and name buffers are exactly as long as their text.
    let slot = size_of::<String>() + size_of::<Object>() + 1;
    let expected = size_of::<Object>()
        + 8 * slot
        + ("Type".len() + "Parent".len() + "MediaBox".len() + "Resources".len())
        + "Page".len()
        + 4 * size_of::<Object>()
        + (slot + "Font".len())
        + (slot + "F1".len());
    let got = estimated_object_size(&page);
    let ratio = got as f64 / expected as f64;
    assert!(
        (1.0 / 1.5..=1.5).contains(&ratio),
        "estimate {got} vs independent sum {expected}"
    );
}

/// A Pages root (MediaBox, Rotate 90) holding an inner Pages node
/// (CropBox) with two leaves, the second overriding MediaBox.
fn nested_page_tree_pdf() -> Vec<u8> {
    let bodies = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 2 /MediaBox [0 0 300 400] /Rotate 90 >>",
        "<< /Type /Pages /Parent 2 0 R /Kids [4 0 R 5 0 R] /Count 2 /CropBox [10 10 200 300] >>",
        "<< /Type /Page /Parent 3 0 R >>",
        "<< /Type /Page /Parent 3 0 R /MediaBox [0 0 500 600] >>",
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, body).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", bodies.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            bodies.len() + 1,
            xref_at
        )
        .as_bytes(),
    );
    out
}

/// No Dart battery sees it: it asserts `page_facts` leaves the page cache empty, and Dart cannot read the cache.
#[test]
fn test_page_facts_match_get_page_accessors() {
    let doc = PdfDocument::from_bytes(nested_page_tree_pdf()).unwrap();
    let refs = doc.all_page_refs().unwrap();
    assert_eq!(refs, vec![ObjectRef::new(4, 0), ObjectRef::new(5, 0)]);
    let facts: Vec<PageFacts> = refs.iter().map(|r| doc.page_facts(*r).unwrap()).collect();
    assert_eq!(
        doc.cached_page_count(),
        0,
        "page_facts must not fill the page cache"
    );
    for (i, f) in facts.iter().enumerate() {
        let (x0, y0, x1, y1) = doc.get_page_media_box(i).unwrap();
        assert_eq!(f.media_box, Some([x0, y0, x1, y1]), "media box of page {i}");
        assert_eq!(f.rotation(), doc.get_page_rotation(i).unwrap(), "rotation of page {i}");
        assert_eq!(f.crop_box, Some([10.0, 10.0, 200.0, 300.0]), "crop box of page {i}");
    }
    assert_eq!(facts[0].media_box, Some([0.0, 0.0, 300.0, 400.0]));
    assert_eq!(facts[1].media_box, Some([0.0, 0.0, 500.0, 600.0]));
    assert_eq!(facts[0].rotation(), 90);
}

/// No Dart battery sees it: it asserts the page cache holds a window of pages, and Dart cannot read the cache.
#[test]
fn test_indexed_pages_equal_traversed_pages_and_cache_is_a_window() {
    let traversed = PdfDocument::from_bytes(nested_page_tree_pdf()).unwrap();
    let indexed = PdfDocument::from_bytes(nested_page_tree_pdf()).unwrap();
    indexed.index_all_pages().unwrap();
    for i in 0..2 {
        assert_eq!(
            indexed.get_page(i).unwrap(),
            traversed.get_page(i).unwrap(),
            "page {i} from the index must carry the same inherited attributes"
        );
    }

    // More pages than the cache's byte budget holds.
    const PAGES: usize = 4_000;
    let many = PdfDocument::from_bytes(build_multi_page_pdf(PAGES)).unwrap();
    for i in 0..PAGES {
        many.get_page(i).unwrap();
    }
    assert!(
        many.cached_page_count() < PAGES,
        "the page cache must hold the bytes of its budget, not every page"
    );
    assert!(many.page_tree_is_indexed());
}

/// No Dart battery sees it: it reads the document's page cache, page index or size estimate, which no Dart call returns.
#[test]
fn test_all_page_refs_walks_inner_nodes_with_matching_count() {
    let bodies = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>",
        "<< /Type /Pages /Parent 2 0 R /Kids [5 0 R] /Count 1 >>",
        "<< /Type /Pages /Parent 2 0 R /Kids [6 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 3 0 R >>",
        "<< /Type /Page /Parent 4 0 R >>",
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in bodies.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", i + 1, body).as_bytes());
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", bodies.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            bodies.len() + 1,
            xref_at
        )
        .as_bytes(),
    );
    let doc = PdfDocument::from_bytes(out).unwrap();
    assert_eq!(
        doc.all_page_refs().unwrap(),
        vec![ObjectRef::new(5, 0), ObjectRef::new(6, 0)]
    );
    assert_eq!(doc.page_count().unwrap(), 2);
}
