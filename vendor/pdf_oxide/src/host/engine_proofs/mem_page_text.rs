//! The text of one extreme page is read in bounded memory: the pages the
//! memprobe rows `extractAll fonts` (20,000 text runs, one font each) and
//! `extractAll tagged` (600,000 structure elements on one page) are built at
//! any size here, and what a page of N runs holds is asserted as N·K + C.

use super::own_process::alone;
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::host::heap;
use crate::layout::TextSpan;

/// The first font object of [`fonts_page`]; font `i` is object `FIRST + 2 * i`.
const FIRST_FONT: usize = 5;

/// One page of `runs` text runs, each in a simple font of its own with a
/// ToUnicode CMap of its own (64 mappings that differ per font), the
/// shape of the memprobe `fonts` input.
pub(super) fn fonts_page(runs: usize) -> Vec<u8> {
    let names: String = (0..runs).map(|i| format!("/F{i} {} 0 R ", FIRST_FONT + 2 * i)).collect();
    let content: String = (0..runs)
        .map(|i| format!("BT /F{i} 10 Tf 10 {} Td (AB) Tj ET\n", 780 - i % 700))
        .collect();
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
    for i in 0..runs {
        let font = FIRST_FONT + 2 * i;
        objects.push(
            format!(
                "{font} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Probe{i} /FirstChar 65 \
                 /LastChar 66 /Widths [600 600] /ToUnicode {} 0 R >>\nendobj\n",
                font + 1
            )
            .into_bytes(),
        );
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
             1 begincodespacerange\n<00> <FF>\nendcodespacerange\n64 beginbfchar\n",
        );
        for code in 0..64u32 {
            cmap.push_str(&format!(
                "<{:02X}> <{:04X}>\n",
                code + 0x20,
                0x4E00 + ((i as u32 * 64 + code) % 20_000)
            ));
        }
        cmap.push_str("endbfchar\nendcmap end end\n");
        objects.push(stream_object(font as u32 + 1, "", cmap.as_bytes()));
    }
    pdf_from_objects(&objects)
}

/// One tagged page of `elements` paragraphs under sections of 100, each
/// marking one run of text `t<mcid>`, the shape of the memprobe `tagged`
/// input without its article thread.
pub(super) fn tagged_page(elements: usize) -> Vec<u8> {
    const PER_SECTION: usize = 100;
    let sections = elements.div_ceil(PER_SECTION);
    let document = 7;
    let first_section = 8;
    let first_paragraph = first_section + sections;
    let mut content = String::new();
    for mcid in 0..elements {
        content.push_str(&format!(
            "/P <</MCID {mcid}>> BDC BT /F1 6 Tf 10 {} Td (t{mcid}) Tj ET EMC\n",
            780 - mcid % 760
        ));
    }
    let section_refs: String = (0..sections).map(|s| format!("{} 0 R ", first_section + s)).collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 6 0 R >>\nendobj\n"
            .to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /StructParents 0 \
          /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", content.as_bytes()),
        b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        format!("6 0 obj\n<< /Type /StructTreeRoot /K {document} 0 R >>\nendobj\n").into_bytes(),
        format!(
            "{document} 0 obj\n<< /Type /StructElem /S /Document /P 6 0 R /K [{section_refs}] >>\nendobj\n"
        )
        .into_bytes(),
    ];
    for s in 0..sections {
        let kids: String = (0..PER_SECTION)
            .filter(|k| s * PER_SECTION + k < elements)
            .map(|k| format!("{} 0 R ", first_paragraph + s * PER_SECTION + k))
            .collect();
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /StructElem /S /Sect /P {document} 0 R /K [{kids}] >>\nendobj\n",
                first_section + s
            )
            .into_bytes(),
        );
    }
    for e in 0..elements {
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /StructElem /S /P /P {} 0 R /Pg 3 0 R /K {e} >>\nendobj\n",
                first_paragraph + e,
                first_section + e / PER_SECTION
            )
            .into_bytes(),
        );
    }
    pdf_from_objects(&objects)
}


/// The bytes the spans hold: each record and every buffer it owns.
fn span_bytes(spans: &[TextSpan]) -> usize {
    spans.len() * std::mem::size_of::<TextSpan>()
        + spans
            .iter()
            .map(|s| {
                s.text.capacity()
                    + 4 * (s.char_widths.capacity() + s.char_x_offsets.capacity())
            })
            .sum::<usize>()
}

/// The runs of the tagged pages the proofs build.
const RUNS: usize = 20_000;

/// A tagged page of `RUNS` runs and the spans another copy of it extracts, so
/// the document a proof measures starts with no page cached.
fn tagged_and_spans() -> (PdfDocument, Vec<TextSpan>) {
    let bytes = tagged_page(RUNS);
    let spans = PdfDocument::from_bytes(bytes.clone()).unwrap().extract_spans(0).unwrap();
    assert_eq!(spans.len(), RUNS);
    (PdfDocument::from_bytes(bytes).unwrap(), spans)
}

/// After the stamping pass of a page of 20,000 runs ends, the document holds
/// at most 64 bytes a run more than before it (the offsets the pass writes
/// into the spans) plus 3 MiB: the characters it read are not kept beside
/// the spans. Before, the page's characters stayed in `page_chars_cache`,
/// about 390 bytes a run. No Dart battery can observe this: the extracted
/// text is the same whether the characters are kept or read again, and
/// only the allocator counter of the engine sees what the document holds.
#[test]
fn the_stamping_pass_keeps_no_characters_after_it_ends() {
    alone(module_path!(), "the_stamping_pass_keeps_no_characters_after_it_ends", || {
        const K: usize = 64;
        const C: usize = 3 * 1024 * 1024;
        let (doc, mut spans) = tagged_and_spans();
        let before = heap::live();
        doc.stamp_char_x_offsets(0, &mut spans, &CancelCheck::none());
        let held = heap::live().saturating_sub(before);
        println!("{RUNS} runs: the document holds {held} bytes after the pass");
        assert!(held <= RUNS * K + C, "the pass left {held} bytes held, over {RUNS}*{K}+{C}");
    });
}

/// `extract_spans` of a page of 20,000 runs leaves the document and its
/// caller holding the spans once, and at most 3 MiB more: the cache that
/// serves a repeat call does not keep a second copy of a page it cannot
/// hold. Before, the cache kept a copy of every page (about 245 bytes a
/// run) beside the one handed out. No Dart battery can observe this: a
/// repeat call returns the same spans either way.
#[test]
fn extract_spans_holds_the_page_once() {
    alone(module_path!(), "extract_spans_holds_the_page_once", || {
        const C: usize = 3 * 1024 * 1024;
        let (doc, _) = tagged_and_spans();
        let before = heap::live();
        let spans = doc.extract_spans(0).unwrap();
        let held = heap::live().saturating_sub(before);
        let own = span_bytes(&spans);
        println!("{RUNS} runs: the spans take {own} bytes, the caller and the document hold {held}");
        assert!(held <= own + C, "{held} bytes held for spans of {own} bytes, over +{C}");
    });
}

/// Assembling the text of a tagged page of 20,000 runs in structure order,
/// given its spans and with the elements it reads in the object cache, takes
/// at most [`crate::host::piece_budget::ASSEMBLY_BYTES_PER_PIECE`] bytes a run and 256 KiB over what it was given: the spans are
/// grouped by marked-content id as ranges of one list, not a list of their
/// own per id, and no copy of their text is kept per id. No Dart battery can
/// observe this: the text is the same word for word, and only the allocator
/// counter of the engine sees what the assembly holds.
#[test]
fn tagged_assembly_holds_no_bucket_or_text_copy_per_marked_content_id() {
    alone(
        module_path!(),
        "tagged_assembly_holds_no_bucket_or_text_copy_per_marked_content_id",
        || {
            const K: usize = crate::host::piece_budget::ASSEMBLY_BYTES_PER_PIECE;
            const C: usize = 256 * 1024;
            let (doc, spans) = tagged_and_spans();
            let tree = doc.struct_tree_trustworthy().expect("the page is tagged");
            // The elements the walk reads are in the object cache, which has a
            // budget of its own, before the assembly is measured.
            drop(crate::structure::traversal::page_structure(&doc, &tree, 0).unwrap());
            let (text, growth) = heap::growth(|| {
                doc.extract_text_structure_order_cached_with_spans(0, &tree, spans, true).unwrap()
            });
            println!("{RUNS} runs: the assembly grew by {growth} bytes");
            assert!(text.starts_with("t0\n") || text.starts_with("t0"), "{:?}", &text[..8]);
            assert!(growth <= RUNS * K + C, "grew by {growth} bytes, over {RUNS}*{K}+{C}");
        },
    );
}

/// The structure walk of a tagged page of 16,384 references, once the
/// elements it reads are in the object cache, takes at most 142 bytes a
/// reference plus 16 KiB: the structure type of a reference is the one string
/// every element of the type shares, not a string of its own. Before, each
/// reference carried a string of its own. No Dart battery can observe this:
/// the type reads the same either way, and only the allocator counter of
/// the engine sees what the walk holds.
#[test]
fn a_structure_walk_holds_no_string_per_reference() {
    alone(module_path!(), "a_structure_walk_holds_no_string_per_reference", || {
        const N: usize = 16_384;
        const K: usize = 142;
        const C: usize = 16 * 1024;
        let doc = PdfDocument::from_bytes(tagged_page(N)).unwrap();
        let tree = doc.struct_tree_trustworthy().expect("the page is tagged");
        drop(crate::structure::traversal::page_structure(&doc, &tree, 0).unwrap());
        let (structure, growth) =
            heap::growth(|| crate::structure::traversal::page_structure(&doc, &tree, 0).unwrap());
        assert_eq!(structure.content.len(), N);
        println!("{N} references: the walk grew by {growth} bytes");
        assert!(growth <= N * K + C, "grew by {growth} bytes, over {N}*{K}+{C}");
    });
}

/// Putting the runs of a two-column page of 20,000 runs in XY-cut order takes
/// at most 200 bytes a run (the cut's own partition) and 256 KiB over the runs: the order is read as
/// indices and applied to the runs where they are, with no copy of them.
/// Before, the runs were copied for the cut and moved out again into a list
/// of their own, about 490 bytes a run. No Dart battery can observe this: the
/// order is the same word for word, and only the allocator counter of the
/// engine sees what the ordering holds.
#[test]
fn xy_cut_orders_the_runs_without_a_copy_of_them() {
    alone(module_path!(), "xy_cut_orders_the_runs_without_a_copy_of_them", || {
        const K: usize = 200;
        const C: usize = 256 * 1024;
        let mut spans: Vec<TextSpan> = (0..RUNS)
            .map(|i| {
                let (column, line) = (i % 2, i / 2);
                TextSpan {
                    text: format!("c{column}l{line}"),
                    bbox: crate::geometry::Rect::new(
                        50.0 + 300.0 * column as f32,
                        10_000.0 - 12.0 * line as f32,
                        200.0,
                        10.0,
                    ),
                    ..TextSpan::default()
                }
            })
            .collect();
        let ((), growth) = heap::growth(|| PdfDocument::order_by_xy_cut(&mut spans));
        println!("{RUNS} runs: the ordering grew by {growth} bytes");
        assert_eq!(spans.len(), RUNS);
        let first: Vec<&str> = spans.iter().take(3).map(|s| s.text.as_str()).collect();
        assert_eq!(first, ["c0l0", "c0l1", "c0l2"], "the left column is read first");
        assert_eq!(spans[RUNS / 2].text, "c1l0", "the right column follows it");
        assert!(growth <= RUNS * K + C, "grew by {growth} bytes, over {RUNS}*{K}+{C}");
    });
}

/// What the document holds once `f` has run and what it returned is dropped.
fn held_after<T>(f: impl FnOnce() -> T) -> usize {
    let before = heap::live();
    drop(f());
    heap::live().saturating_sub(before)
}

/// `pages` pages of `runs` runs each, in one font: run `r` of page `p` reads
/// `p<p>r<r>`.
fn plain_pages(pages: usize, runs: usize) -> Vec<u8> {
    let kids: String = (0..pages).map(|p| format!("{} 0 R ", 4 + 2 * p)).collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {pages} >>\nendobj\n").into_bytes(),
        b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ];
    for p in 0..pages {
        let content: String = (0..runs)
            .map(|r| format!("BT /F1 6 Tf 10 {} Td (p{p}r{r}) Tj ET\n", 780 - r % 760))
            .collect();
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>\nendobj\n",
                4 + 2 * p,
                5 + 2 * p
            )
            .into_bytes(),
        );
        objects.push(stream_object(5 + 2 * p as u32, "", content.as_bytes()));
    }
    pdf_from_objects(&objects)
}

/// A page that paints one Form XObject of `runs` runs, with a font of its
/// own, at `draws` places.
fn form_text_page(draws: usize, runs: usize) -> Vec<u8> {
    let page: String = (0..draws).map(|d| format!("q 1 0 0 1 {d} 0 cm /Fm Do Q\n")).collect();
    let form: String = (0..runs)
        .map(|r| format!("BT /F1 6 Tf 10 {} Td (f{r}) Tj ET\n", 780 - r % 760))
        .collect();
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
          /Resources << /XObject << /Fm 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", page.as_bytes()),
        stream_object(
            5,
            "/Type /XObject /Subtype /Form /BBox [0 0 612 792] \
             /Resources << /Font << /F1 6 0 R >> >>",
            form.as_bytes(),
        ),
        b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ])
}

/// A page that paints `forms` Form XObjects, each of one 640 by 640 grey image
/// of its own (400 KiB of pixels once decoded, a few bytes in the file).
fn form_image_page(forms: usize) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(&vec![0u8; 640 * 640]).unwrap();
    let pixels = encoder.finish().unwrap();
    let names: String = (0..forms).map(|f| format!("/Fm{f} {} 0 R ", 5 + 2 * f)).collect();
    let page: String = (0..forms).map(|f| format!("/Fm{f} Do\n")).collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
             /Resources << /XObject << {names}>> >> /Contents 4 0 R >>\nendobj\n"
        )
        .into_bytes(),
        stream_object(4, "", page.as_bytes()),
    ];
    for f in 0..forms {
        let form = 5 + 2 * f as u32;
        objects.push(stream_object(
            form,
            &format!(
                "/Type /XObject /Subtype /Form /BBox [0 0 612 792] \
                 /Resources << /XObject << /Im {} 0 R >> >>",
                form + 1
            ),
            b"q 100 0 0 100 10 10 cm /Im Do Q",
        ));
        objects.push(stream_object(
            form + 1,
            "/Type /XObject /Subtype /Image /Width 640 /Height 640 /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /Filter /FlateDecode",
            &pixels,
        ));
    }
    pdf_from_objects(&objects)
}

/// After `extract_spans` of a page of 20,000 runs, the document holds at
/// most the 512 KiB the cache of spans may hold, and 1 MiB besides: the
/// spans of a page above the budget are handed to the caller and not kept.
/// Before, the last two pages read were kept whatever they took. No Dart
/// battery can observe this: a repeat call returns the same spans either way.
#[test]
fn the_cache_of_spans_keeps_no_page_above_its_budget() {
    alone(module_path!(), "the_cache_of_spans_keeps_no_page_above_its_budget", || {
        const C: usize = 1024 * 1024;
        let (doc, _) = tagged_and_spans();
        let held = held_after(|| doc.extract_spans(0).unwrap());
        println!("{RUNS} runs: the document holds {held} bytes after the call");
        assert!(held <= crate::host::font_budget::PAGE_SPANS_BYTES + C, "{held} bytes held");
    });
}

/// After `extract_chars` of a page of 20,000 runs (about 7 MiB of
/// characters), the document holds at most the 256 KiB the cache of
/// characters may hold, and 1 MiB besides. Before, the last two pages read
/// were kept whatever they took. No Dart battery can observe this: a repeat
/// call returns the same characters either way.
#[test]
fn the_cache_of_characters_keeps_no_page_above_its_budget() {
    alone(module_path!(), "the_cache_of_characters_keeps_no_page_above_its_budget", || {
        const C: usize = 1024 * 1024;
        let (doc, _) = tagged_and_spans();
        let held = held_after(|| doc.extract_chars(0).unwrap());
        println!("{RUNS} runs: the document holds {held} bytes after the call");
        assert!(held <= crate::host::font_budget::PAGE_CHARS_BYTES + C, "{held} bytes held");
    });
}

/// After a search of 12 pages of 3,000 runs, the document holds at most the
/// 1 MiB the search index may hold, and 1 MiB besides, though the indexes of
/// the 12 pages take about 1.4 MiB. Before, the index of every page searched
/// was kept. No Dart battery can observe this: a search finds the same hits
/// whether a page's index is kept or built again.
#[test]
fn the_search_index_is_bounded_by_the_bytes_it_takes() {
    alone(module_path!(), "the_search_index_is_bounded_by_the_bytes_it_takes", || {
        const C: usize = 1024 * 1024;
        let doc = PdfDocument::from_bytes(plain_pages(12, 3_000)).unwrap();
        let held = held_after(|| {
            crate::search::TextSearcher::search(&doc, "p11r2999", &Default::default()).unwrap()
        });
        println!("12 pages of 3000 runs: the document holds {held} bytes after the search");
        assert!(held <= crate::host::font_budget::SEARCH_INDEX_BYTES + C, "{held} bytes held");
    });
}

/// After the pages of a document whose page dictionaries take 1.6 MiB each
/// are read, the document holds at most the 8 MiB of its object cache, the
/// 256 KiB its cache of pages may hold, and 1 MiB besides. Before, the
/// last 64 pages were kept whatever they took. No Dart battery can observe
/// this: a page reads the same whether it is kept or read again.
#[test]
fn the_cache_of_pages_is_bounded_by_the_bytes_the_dictionaries_take() {
    alone(
        module_path!(),
        "the_cache_of_pages_is_bounded_by_the_bytes_the_dictionaries_take",
        || {
            const PAGES: usize = 12;
            const C: usize = 1024 * 1024;
            let junk = "0 ".repeat(40_000);
            let kids: String = (0..PAGES).map(|p| format!("{} 0 R ", 3 + p)).collect();
            let mut objects = vec![
                b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
                format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {PAGES} >>\nendobj\n")
                    .into_bytes(),
            ];
            for p in 0..PAGES {
                objects.push(
                    format!(
                        "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                         /Junk [{junk}] >>\nendobj\n",
                        3 + p
                    )
                    .into_bytes(),
                );
            }
            let doc = PdfDocument::from_bytes(pdf_from_objects(&objects)).unwrap();
            let held = held_after(|| {
                for p in 0..PAGES {
                    drop(doc.get_page(p).unwrap());
                }
            });
            println!("{PAGES} pages: the document holds {held} bytes after reading them");
            assert!(
                held <= 8 * 1024 * 1024
                    + crate::host::font_budget::PAGE_OBJECT_BYTES
                    + C,
                "{held} bytes held"
            );
        },
    );
}

/// After a page that paints a Form XObject of 1,500 runs at 8 places is
/// extracted, the document holds at most the 256 KiB the cache of Form spans
/// may hold, and 1 MiB besides: the spans of a Form above the budget are
/// not kept. Before, the spans of up to 1,024 paintings were kept whatever
/// they took. No Dart battery can observe this: the spans of a repeat
/// painting are the same whether they are kept or extracted again.
#[test]
fn the_cache_of_form_spans_keeps_no_form_above_its_budget() {
    alone(module_path!(), "the_cache_of_form_spans_keeps_no_form_above_its_budget", || {
        const C: usize = 1024 * 1024;
        let doc = PdfDocument::from_bytes(form_text_page(8, 1_500)).unwrap();
        let held = held_after(|| {
            let spans = doc.extract_spans(0).unwrap();
            assert!(!spans.is_empty());
            spans
        });
        println!("8 paintings of 1500 runs: the document holds {held} bytes after the call");
        assert!(
            held <= crate::host::font_budget::PAGE_SPANS_BYTES
                + crate::host::font_budget::FORM_SPANS_BYTES
                + C,
            "{held} bytes held"
        );
    });
}

/// After the images of a page whose 8 Form XObjects hold an image of 400 KiB
/// each are extracted, the document holds at most the 256 KiB the cache of
/// Form images may hold, and 1 MiB besides: the images of a Form above the
/// budget are not kept. Before, the images of up to 1,024 Forms were kept
/// whatever they took. No Dart battery can observe this: the images of a Form
/// painted again are the same whether they are kept or extracted again.
#[test]
fn the_cache_of_form_images_keeps_no_form_above_its_budget() {
    alone(module_path!(), "the_cache_of_form_images_keeps_no_form_above_its_budget", || {
        const C: usize = 1024 * 1024;
        let doc = PdfDocument::from_bytes(form_image_page(8)).unwrap();
        let held = held_after(|| {
            let images = doc.extract_images(0).unwrap();
            assert_eq!(images.len(), 8);
            images
        });
        println!("8 forms of 400 KiB: the document holds {held} bytes after the call");
        assert!(held <= crate::host::font_budget::FORM_IMAGES_BYTES + C, "{held} bytes held");
    });
}

/// The spans of a page of 20,000 runs in one font take at most 225 bytes a
/// run and 64 KiB: the runs share the name of their font, one allocation, and
/// do not each hold a copy. Before, each held its own, and the spans took
/// about 237 bytes a run. No Dart battery can observe this: the name reads
/// the same shared or copied, and only the allocator counter of the engine
/// sees what the spans hold.
#[test]
fn the_runs_of_one_font_share_its_name() {
    alone(module_path!(), "the_runs_of_one_font_share_its_name", || {
        const K: usize = 225;
        const C: usize = 64 * 1024;
        let (doc, _) = tagged_and_spans();
        let before = heap::live();
        let spans = doc.extract_spans(0).unwrap();
        let held = heap::live().saturating_sub(before);
        println!("{RUNS} runs: the caller and the document hold {held} bytes");
        assert_eq!(spans[0].font_name, "Helvetica");
        assert!(spans.iter().all(|s| s.font_name == spans[0].font_name));
        assert!(held <= RUNS * K + C, "{held} bytes held, over {RUNS}*{K}+{C}");
    });
}

/// A tagged page of `runs` runs and the spans another copy of it extracts, so
/// the document a proof measures starts with no page cached.
fn tagged_and_spans_of(runs: usize) -> (PdfDocument, Vec<TextSpan>) {
    let bytes = tagged_page(runs);
    let spans = PdfDocument::from_bytes(bytes.clone()).unwrap().extract_spans(0).unwrap();
    assert_eq!(spans.len(), runs);
    (PdfDocument::from_bytes(bytes).unwrap(), spans)
}

/// What the stamping pass takes at its peak over what the document held
/// before it, for a tagged page of `runs` runs, and the glyphs of its spans.
fn stamping_peak_and_glyphs(runs: usize) -> (usize, usize) {
    let (doc, mut spans) = tagged_and_spans_of(runs);
    let glyphs = spans.iter().map(|s| s.text.chars().count()).sum();
    let ((), growth) =
        heap::growth(|| doc.stamp_char_x_offsets(0, &mut spans, &CancelCheck::none()));
    (growth, glyphs)
}

fn stamping_peak(runs: usize) -> usize {
    stamping_peak_and_glyphs(runs).0
}

/// The stamping pass of a page grows with its glyphs, not with the room a
/// list of glyphs was given: a list that doubles held 2,097,152 slots for the
/// 1,079,000 glyphs of 170,000 runs, and the peak at twice the runs was 3.35
/// times the peak at the runs. The fixture's glyphs grow faster than its runs
/// (about 499,000 at 85,000 runs), so the peak is compared per glyph: at twice
/// the runs it is at most 1.05 times what it is at the runs. No Dart battery
/// can observe this: the offsets stamped are the same at every size, and only
/// the allocator counter of the engine sees the peak.
#[test]
fn the_stamping_pass_grows_linearly_with_the_page() {
    alone(module_path!(), "the_stamping_pass_grows_linearly_with_the_page", || {
        let (at_n, glyphs_n) = stamping_peak_and_glyphs(85_000);
        let (at_2n, glyphs_2n) = stamping_peak_and_glyphs(170_000);
        let (per_n, per_2n) = (at_n as f64 / glyphs_n as f64, at_2n as f64 / glyphs_2n as f64);
        println!(
            "stamping peak: {at_n} bytes for {glyphs_n} glyphs ({per_n:.1} a glyph), {at_2n} for {glyphs_2n} ({per_2n:.1})"
        );
        assert!(
            per_2n <= 1.05 * per_n,
            "{per_2n:.1} bytes a glyph at twice the runs is over 1.05 times {per_n:.1}"
        );
    });
}

/// The stamping pass of a tagged page of 170,000 runs (about 1,079,000
/// glyphs before the overlap filter) takes at its peak at most
/// [`crate::host::piece_budget::STAMP_BYTES_PER_PIECE`] bytes a run and 256 KiB:
/// each glyph is a record of
/// [`crate::extractors::text::ALIGNED_CHAR_BYTES`] bytes with no heap object
/// behind it, not a 128-byte `TextChar` with a font name of its own. No Dart
/// battery can observe this: the offsets stamped are the same either way.
#[test]
fn the_stamping_pass_holds_a_declared_number_of_bytes_a_piece() {
    alone(module_path!(), "the_stamping_pass_holds_a_declared_number_of_bytes_a_piece", || {
        const RUNS_HERE: usize = 170_000;
        const C: usize = 256 * 1024;
        let growth = stamping_peak(RUNS_HERE);
        let k = crate::host::piece_budget::STAMP_BYTES_PER_PIECE;
        println!("{RUNS_HERE} runs: the stamping pass peaked {growth} bytes over what the document held");
        assert!(growth <= RUNS_HERE * k + C, "{growth} bytes, over {RUNS_HERE}*{k}+{C}");
    });
}

/// `load_fonts` of a page naming `fonts` fonts: what it takes at its peak over
/// what the document held before.
fn load_fonts_peak(fonts: usize) -> usize {
    let doc = PdfDocument::from_bytes(fonts_page(fonts)).unwrap();
    let page = doc.get_page(0).unwrap();
    let resources = page.as_dict().unwrap().get("Resources").unwrap().clone();
    let mut extractor = crate::extractors::TextExtractor::new();
    extractor.set_document(&doc);
    let (result, growth) = heap::growth(|| doc.load_fonts(&resources, &mut extractor));
    result.unwrap();
    growth
}

/// Naming N fonts costs `load_fonts` at most
/// [`crate::host::piece_budget::LOAD_FONTS_BYTES_PER_FONT`] bytes a font and 1 MiB: the page's
/// /Font dictionary is walked once, not cloned, and no copy of its names or
/// entries is made beyond the sorted list of references the order of loading
/// needs and the name a font left unread waits under. Before, it copied the
/// dictionary and its names several times, about 1,200 bytes a font. No Dart
/// battery can observe this: the fonts resolve the same either way.
#[test]
fn load_fonts_holds_a_declared_number_of_bytes_a_font() {
    alone(module_path!(), "load_fonts_holds_a_declared_number_of_bytes_a_font", || {
        const FONTS: usize = 20_000;
        const C: usize = 1024 * 1024;
        let growth = load_fonts_peak(FONTS);
        let k = crate::host::piece_budget::LOAD_FONTS_BYTES_PER_FONT;
        println!("{FONTS} fonts: load_fonts peaked {growth} bytes");
        assert!(growth <= FONTS * k + C, "{growth} bytes, over {FONTS}*{k}+{C}");
    });
}

/// `load_fonts` grows with the fonts the page names: the peak for 2N fonts is
/// at most 2.1 times the peak for N.
#[test]
fn load_fonts_grows_linearly_with_the_fonts() {
    alone(module_path!(), "load_fonts_grows_linearly_with_the_fonts", || {
        let at_n = load_fonts_peak(10_000);
        let at_2n = load_fonts_peak(20_000);
        println!("load_fonts peak: {at_n} bytes for 10000 fonts, {at_2n} for 20000");
        assert!(at_2n as f64 <= 2.1 * at_n as f64, "{at_2n} is over 2.1 times {at_n}");
    });
}

/// A page that shows `AB` twice at nearly one place (the overlap filter drops
/// the second), then a Form XObject whose /BBox is a corner of the page,
/// painting one run inside it and one far outside it (the clip drops that one).
fn overlap_and_clip_page() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
          /Resources << /Font << /F1 6 0 R >> /XObject << /Fm 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(
            4,
            "",
            b"BT /F1 12 Tf 100 700 Td (AB) Tj ET\nBT /F1 12 Tf 100.3 700 Td (AB) Tj ET\n\
              BT /F1 12 Tf 100 650 Td (C D) Tj ET\n/Fm Do\n",
        ),
        stream_object(
            5,
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /Font << /F1 6 0 R >> >>",
            b"BT /F1 10 Tf 10 10 Td (in) Tj ET\nBT /F1 10 Tf 300 300 Td (out) Tj ET\n",
        ),
        b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ])
}

/// The compact glyphs the stamping pass reads are the characters it read
/// before, glyph for glyph: the same characters, at the same origins, in the
/// same order, after the same sort, overlap filter and Form clip. Nothing a
/// Dart battery sees could differ otherwise, but a battery cannot tell which
/// of two paths read the page, so this compares the two.
#[test]
fn the_compact_glyphs_are_the_characters_the_stamping_pass_read_before() {
    let pages = [
        ("overlap and clip", overlap_and_clip_page()),
        ("tagged", tagged_page(3_000)),
        ("fonts", fonts_page(300)),
    ];
    for (name, bytes) in pages {
        let doc = PdfDocument::from_bytes(bytes).unwrap();
        let cancel = CancelCheck::none();
        let chars = doc
            .extract_chars_impl(0, Default::default(), Default::default(), &cancel)
            .unwrap();
        let glyphs = doc.extract_aligned_impl(0, &cancel).unwrap();
        assert!(!glyphs.is_empty(), "{name}: no glyphs");
        assert_eq!(glyphs.len(), chars.len(), "{name}: glyph count");
        for (i, (g, c)) in glyphs.iter().zip(&chars).enumerate() {
            assert_eq!(
                (g.char, g.origin_x.to_bits(), g.origin_y.to_bits()),
                (c.char, c.origin_x.to_bits(), c.origin_y.to_bits()),
                "{name}: glyph {i}"
            );
        }
        if name == "overlap and clip" {
            let text: String = glyphs.iter().map(|g| g.char).collect();
            assert!(text.contains("in") && !text.contains("out"), "the clip kept {text:?}");
            assert_eq!(text.matches('A').count(), 1, "the overlap filter kept {text:?}");
        }
    }
}

/// A whole text extraction of a tagged page takes at its peak, over what the
/// process held before it, at most
/// [`crate::host::piece_budget::EXTRACT_BYTES_PER_PIECE`] bytes a run and 1 MiB;
/// a page of twice the runs takes at most 2.1 times as much. The memprobe rows
/// `extractAll tagged` and `tagged-2x` hold the engine to the same number.
/// No Dart battery can observe this: the text is the same word for word, and
/// only the allocator counter of the engine sees the peak.
#[test]
fn a_tagged_page_extracts_within_the_declared_bytes_a_piece() {
    alone(module_path!(), "a_tagged_page_extracts_within_the_declared_bytes_a_piece", || {
        const C: usize = 1024 * 1024;
        let growth_for = |runs: usize| {
            let doc = PdfDocument::from_bytes(tagged_page(runs)).unwrap();
            let (text, growth) = heap::growth(|| doc.extract_text(0).unwrap());
            assert!(text.starts_with("t0"), "{:?}", &text[..8]);
            growth
        };
        let (n, at_n, at_2n) = (40_000, growth_for(40_000), growth_for(80_000));
        println!("a tagged page: {at_n} bytes for {n} runs, {at_2n} for {}", 2 * n);
        let bound = crate::host::piece_budget::page_extra(2 * n, 0) + C;
        assert!(at_2n <= bound, "{at_2n} bytes for {} runs, over {bound}", 2 * n);
        assert!(at_2n as f64 <= 2.1 * at_n as f64, "{at_2n} is over 2.1 times {at_n}");
    });
}

/// The same for a page that names a font for each run: the fonts' own bytes
/// are [`crate::host::piece_budget::EXTRACT_BYTES_PER_FONT`] a font on top.
#[test]
fn a_page_of_fonts_extracts_within_the_declared_bytes_a_font() {
    alone(module_path!(), "a_page_of_fonts_extracts_within_the_declared_bytes_a_font", || {
        let growth_for = |fonts: usize| {
            let doc = PdfDocument::from_bytes(fonts_page(fonts)).unwrap();
            heap::growth(|| doc.extract_text(0).unwrap()).1
        };
        let (n, at_n, at_2n) = (10_000, growth_for(10_000), growth_for(20_000));
        println!("a fonts page: {at_n} bytes for {n} fonts, {at_2n} for {}", 2 * n);
        // The caches a document holds fill during the first page (the object
        // cache, the fonts, the text caches): the per-op ceiling of 16 MiB.
        let bound = 16 * 1024 * 1024 + crate::host::piece_budget::page_extra(2 * n, 2 * n);
        assert!(at_2n <= bound, "{at_2n} bytes for {} fonts, over {bound}", 2 * n);
        assert!(at_2n as f64 <= 2.1 * at_n as f64, "{at_2n} is over 2.1 times {at_n}");
    });
}
