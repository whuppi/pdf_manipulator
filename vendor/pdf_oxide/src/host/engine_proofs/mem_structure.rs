//! The structure tree is walked, not parsed whole: a page's structure and the
//! ActualText index come out of one walk that holds the open elements' child
//! lists, and a tree of any size ends, cycles included.

use super::alone::run_alone;
use super::filter_readers::thread_growth;
use super::support::pdf_from_objects;
use crate::document::PdfDocument;
use crate::structure::traversal::{build_actualtext_index, page_structure, streamed_actualtext_index};
use crate::structure::{
    find_table_elements_all_pages, open_structure_tree, traverse_structure_tree_all_pages,
    StructChild, StructElem,
};

/// A tagged document of `pages` empty pages and `elements` paragraphs under
/// sections of 100: paragraph `e` is on page `e % pages` and names MCID
/// `e / pages`.
fn tagged(pages: usize, elements: usize) -> Vec<u8> {
    const PER_SECTION: usize = 100;
    let sections = elements / PER_SECTION;
    let first_page = 4;
    let document = first_page + pages;
    let first_section = document + 1;
    let first_paragraph = first_section + sections;
    let kids: String = (0..pages).map(|p| format!("{} 0 R ", first_page + p)).collect();
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 3 0 R >>\nendobj\n"
            .to_vec(),
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {pages} >>\nendobj\n").into_bytes(),
        format!("3 0 obj\n<< /Type /StructTreeRoot /K {document} 0 R >>\nendobj\n").into_bytes(),
    ];
    for p in 0..pages {
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n",
                first_page + p
            )
            .into_bytes(),
        );
    }
    let section_refs: String = (0..sections).map(|s| format!("{} 0 R ", first_section + s)).collect();
    objects.push(
        format!("{document} 0 obj\n<< /Type /StructElem /S /Document /P 3 0 R /K [{section_refs}] >>\nendobj\n")
            .into_bytes(),
    );
    for s in 0..sections {
        let refs: String = (0..PER_SECTION)
            .map(|k| format!("{} 0 R ", first_paragraph + s * PER_SECTION + k))
            .collect();
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /StructElem /S /Sect /P {document} 0 R /K [{refs}] >>\nendobj\n",
                first_section + s
            )
            .into_bytes(),
        );
    }
    for e in 0..elements {
        objects.push(
            format!(
                "{} 0 obj\n<< /Type /StructElem /S /P /Pg {} 0 R /K {} >>\nendobj\n",
                first_paragraph + e,
                first_page + e % pages,
                e / pages
            )
            .into_bytes(),
        );
    }
    pdf_from_objects(&objects)
}

fn count_elements(elem: &StructElem) -> usize {
    1 + elem
        .children
        .iter()
        .map(|c| match c {
            StructChild::StructElem(e) => count_elements(e),
            _ => 0,
        })
        .sum::<usize>()
}

/// A tree of 200,000 paragraphs, twenty times the old cap, is walked for one
/// page with the thread holding under 24 MiB over where it began (the document's 8 MiB object
/// cache is most of it), while the same tree held whole (`structure_tree()`) is more than
/// three times what the walk held. The page
/// gets exactly its 1,000 paragraphs, in order. No Dart battery can observe
/// this: Dart reads the same text either way, and what the walk holds is
/// only visible to the allocator counter.
#[test]
fn a_tree_of_200000_elements_is_walked_for_one_page_in_bounded_memory() {
    run_alone("host::engine_proofs::mem_structure::alone::a_tree_of_200000_elements_is_walked_for_one_page_in_bounded_memory");
}


/// A tree larger than the old element cap is returned whole, under a budget
/// too: the cap that dropped it is gone. Replaces
/// `parse_budget_bounded_caps_element_count`, which pinned that cap. No Dart
/// battery can observe this: the budgeted parse is reached only by an engine
/// caller, and Dart extraction does not set one.
#[test]
fn a_tree_past_the_old_cap_is_returned_whole_even_under_a_budget() {
    run_alone("host::engine_proofs::mem_structure::alone::a_tree_past_the_old_cap_is_returned_whole_even_under_a_budget");
}

/// A structure tree that names its own ancestors ends: the document lists a
/// section twice, a section lists its parent, one element lists itself, and
/// an indirect `/K` points back up. Each element is met once. No Dart battery
/// can observe this: a cycle that did not end would hang the engine lane,
/// and a Dart test cannot build the cycle with the engine's own writer.
#[test]
fn a_cyclic_tree_ends() {
    let doc = PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 4 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /StructTreeRoot /K [5 0 R 5 0 R] >>\nendobj\n".to_vec(),
        b"5 0 obj\n<< /Type /StructElem /S /Document /K [6 0 R 6 0 R 5 0 R] >>\nendobj\n".to_vec(),
        b"6 0 obj\n<< /Type /StructElem /S /Sect /Pg 3 0 R /K [5 0 R 7 0 R 6 0 R 0] >>\nendobj\n".to_vec(),
        b"7 0 obj\n<< /Type /StructElem /S /P /Pg 3 0 R /K 8 0 R >>\nendobj\n".to_vec(),
        b"8 0 obj\n<< /Type /StructElem /S /Span /Pg 3 0 R /K 7 0 R >>\nendobj\n".to_vec(),
    ]))
    .unwrap();
    let tree = doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.root_elements.len(), 1, "the second listing of the document is a cycle");
    // Document, one Sect, one P, one Span: the later listings are skipped.
    assert_eq!(count_elements(&tree.root_elements[0]), 4);
    let header = open_structure_tree(&doc).unwrap().unwrap();
    let structure = page_structure(&doc, &header, 0).unwrap();
    assert_eq!(structure.content.len(), 1, "the Sect names MCID 0 once");
    assert!(streamed_actualtext_index(&doc, &header).unwrap().covered_mcids.is_empty());
}

/// One walk of the tree gives, for every page, what the whole-document
/// traversals give: the marked content in reading order with its roles, block
/// ids and sections (headings, lists, a table, a word break), the table
/// elements, and the ActualText index with a scope that nests and one that
/// spans two pages. No Dart battery can observe this: it compares the engine's
/// internal results with the ones it replaced, field by field.
#[test]
fn a_walk_gives_what_the_whole_tree_traversals_give() {
    let doc = PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> /StructTreeRoot 5 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        b"5 0 obj\n<< /Type /StructTreeRoot /RoleMap << /Heading1 /H1 >> /K 6 0 R >>\nendobj\n".to_vec(),
        b"6 0 obj\n<< /Type /StructElem /S /Document /K [7 0 R 9 0 R 12 0 R 17 0 R 19 0 R] >>\nendobj\n".to_vec(),
        b"7 0 obj\n<< /Type /StructElem /S /Sect /K [8 0 R 10 0 R] >>\nendobj\n".to_vec(),
        b"8 0 obj\n<< /Type /StructElem /S /Heading1 /Pg 3 0 R /K 0 >>\nendobj\n".to_vec(),
        b"9 0 obj\n<< /Type /StructElem /S /P /Pg 4 0 R /K [0 1] >>\nendobj\n".to_vec(),
        b"10 0 obj\n<< /Type /StructElem /S /L /K [11 0 R] >>\nendobj\n".to_vec(),
        b"11 0 obj\n<< /Type /StructElem /S /LI /Pg 3 0 R /K [<< /Type /MCR /MCID 1 >> << /Type /MCR /Pg 4 0 R /MCID 2 >>] >>\nendobj\n".to_vec(),
        b"12 0 obj\n<< /Type /StructElem /S /Table /K [13 0 R] >>\nendobj\n".to_vec(),
        b"13 0 obj\n<< /Type /StructElem /S /TR /K [14 0 R 15 0 R] >>\nendobj\n".to_vec(),
        b"14 0 obj\n<< /Type /StructElem /S /TD /Pg 3 0 R /K 2 >>\nendobj\n".to_vec(),
        b"15 0 obj\n<< /Type /StructElem /S /TD /Pg 4 0 R /K 3 >>\nendobj\n".to_vec(),
        b"16 0 obj\n<< /Type /StructElem /S /P /Pg 3 0 R /K 9 >>\nendobj\n".to_vec(),
        b"17 0 obj\n<< /Type /StructElem /S /WB /K [16 0 R 18 0 R] >>\nendobj\n".to_vec(),
        b"18 0 obj\n<< /Type /StructElem /S /Span /Pg 4 0 R /K 4 >>\nendobj\n".to_vec(),
        b"19 0 obj\n<< /Type /StructElem /S /Sect /ActualText (replaced) /Pg 3 0 R /K [20 0 R 21 0 R 23 0 R] >>\nendobj\n".to_vec(),
        b"20 0 obj\n<< /Type /StructElem /S /P /Pg 3 0 R /K 5 >>\nendobj\n".to_vec(),
        b"21 0 obj\n<< /Type /StructElem /S /Div /ActualText (inner) /K [22 0 R] >>\nendobj\n".to_vec(),
        b"22 0 obj\n<< /Type /StructElem /S /P /Pg 4 0 R /K [6 7] >>\nendobj\n".to_vec(),
        b"23 0 obj\n<< /Type /StructElem /S /P /Pg 4 0 R /K 8 >>\nendobj\n".to_vec(),
    ]))
    .unwrap();
    let tree = doc.structure_tree().unwrap().unwrap();
    let all = traverse_structure_tree_all_pages(&tree);
    let tables = find_table_elements_all_pages(&tree);
    let header = open_structure_tree(&doc).unwrap().unwrap();
    let mut seen = 0;
    for page in 0..2u32 {
        let walked = page_structure(&doc, &header, page).unwrap();
        let expected = all.get(&page).cloned().unwrap_or_default();
        seen += expected.len();
        assert_eq!(format!("{:?}", walked.content), format!("{:?}", expected), "page {page}");
        let expected_tables = tables.get(&page).cloned().unwrap_or_default();
        assert_eq!(format!("{:?}", walked.tables), format!("{:?}", expected_tables), "tables of page {page}");
    }
    assert!(seen >= 12, "the fixture reaches headings, lists, a table, a word break: {seen} items");
    assert!(all.values().flatten().any(|c| c.is_word_break), "the fixture has a word break");
    assert!(all.values().flatten().any(|c| c.in_table), "the fixture has a table");

    let whole = build_actualtext_index(&tree);
    let streamed = streamed_actualtext_index(&doc, &header).unwrap();
    assert_eq!(whole.covered_mcids, streamed.covered_mcids);
    assert_eq!(whole.suppress_only, streamed.suppress_only);
    assert_eq!(whole.mcid_to_actual_text, streamed.mcid_to_actual_text);
    assert!(!whole.covered_mcids.is_empty(), "the fixture has an ActualText scope");
    assert!(!whole.suppress_only.is_empty(), "the fixture has a scope across two pages");
}

mod alone {
    use super::*;

    /// The body of the test of the same name above, run in a process of its
    /// own by `alone::run_alone` so that its 80 MiB do not reach the
    /// allocator counter of the proofs beside it. No Dart battery can observe
    /// this, for the reason the parent gives.
    #[test]
    #[ignore = "run in a process of its own by its parent test"]
    fn a_tree_of_200000_elements_is_walked_for_one_page_in_bounded_memory() {
    let doc = PdfDocument::from_bytes(tagged(200, 200_000)).unwrap();
    let (structure, walked) = thread_growth(|| {
        let header = open_structure_tree(&doc).unwrap().expect("a tagged document");
        page_structure(&doc, &header, 7).unwrap()
    });
    println!("200,000 elements walked for one page: peak growth {walked} bytes");
    let mcids: Vec<u32> = structure.content.iter().filter_map(|c| c.mcid).collect();
    assert_eq!(mcids, (0..1000).collect::<Vec<u32>>());
    assert!(structure.content.iter().all(|c| c.page == 7));
    // The document's object cache (8 MiB, fixed) fills as the walk loads elements, and the
    // visited ids are 8 bytes an element: none of it grows with a page's share of the tree.
    assert!(walked < 24 * 1024 * 1024, "walking 200,000 elements for one page grew the thread by {walked} bytes");

    let whole = PdfDocument::from_bytes(tagged(200, 200_000)).unwrap();
    let (tree, held) = thread_growth(|| whole.structure_tree().unwrap().expect("a tagged document"));
    assert_eq!(count_elements(&tree.root_elements[0]), 200_000 + 2000 + 1);
    println!("200,000 elements held whole: peak growth {held} bytes");
    assert!(held > 3 * walked, "the whole tree is {held} bytes and the walk {walked}: the tree is too small to prove anything");
    }

    /// The body of the test of the same name above, run in a process of its
    /// own by `alone::run_alone`. No Dart battery can observe this, for the
    /// reason the parent gives.
    #[test]
    #[ignore = "run in a process of its own by its parent test"]
    fn a_tree_past_the_old_cap_is_returned_whole_even_under_a_budget() {

    let doc = PdfDocument::from_bytes(tagged(20, 20_000)).unwrap();
    let budgeted = doc
        .structure_tree_with_budget(Some(std::time::Duration::from_secs(3600)))
        .unwrap()
        .expect("an hour is not exceeded");
    assert_eq!(count_elements(&budgeted.root_elements[0]), 20_000 + 200 + 1);
    let unbounded = doc.structure_tree().unwrap().unwrap();
    assert_eq!(count_elements(&unbounded.root_elements[0]), 20_000 + 200 + 1);
    }
}
