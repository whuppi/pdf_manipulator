//! Loops that used to stop at a fixed count now run to the end of their input.

use super::support::pdf_from_objects;
use crate::document::PdfDocument;
use crate::structure::article_threads::parse_article_threads;
use crate::structure::spatial_table_detector::split_flow_for_proof;

/// A document whose one thread chains `beads` beads over its one page.
fn threaded(beads: usize) -> Vec<u8> {
    let first = 5usize;
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /Threads [4 0 R] >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\n".to_vec(),
        format!("4 0 obj\n<< /Type /Thread /F {first} 0 R >>\nendobj\n").into_bytes(),
    ];
    for i in 0..beads {
        let id = first + i;
        let next = first + (i + 1) % beads;
        let prev = first + (i + beads - 1) % beads;
        objects.push(
            format!(
                "{id} 0 obj\n<< /Type /Bead /T 4 0 R /N {next} 0 R /V {prev} 0 R /P 3 0 R /R [0 0 10 10] >>\nendobj\n"
            )
            .into_bytes(),
        );
    }
    pdf_from_objects(&objects)
}

/// A thread of 6000 beads is read to its last bead: the old cap of 4096
/// returned 4096 of them. No Dart battery can observe this: Dart exposes no
/// article threads, and the count is the cap itself.
#[test]
fn an_article_thread_longer_than_the_old_cap_is_read_whole() {
    let doc = PdfDocument::from_bytes(threaded(6000)).unwrap();
    let threads = parse_article_threads(&doc);
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].beads.len(), 6000, "the chain was cut short");
}

/// `rows` rows each holding the columns `cols` of an `n`-column grid.
fn grid(n: usize, rows: &[&[usize]]) -> Vec<Vec<Vec<usize>>> {
    rows.iter()
        .map(|cols| (0..n).map(|c| if cols.contains(&c) { vec![c] } else { Vec::new() }).collect())
        .collect()
}

/// Two prose flows mis-clustered as one grid are split-flow at 200 columns
/// as at 8: the check used to give up above 128 columns (`u128` masks) and
/// let the grid through. No Dart battery can observe this: it needs a
/// grid of exactly this shape, and a page's span clustering never makes one.
#[test]
fn a_split_flow_grid_wider_than_128_columns_is_still_split() {
    let left: Vec<usize> = (0..100).collect();
    let right: Vec<usize> = (100..200).collect();
    let rows: Vec<&[usize]> = vec![&left, &left, &left, &left, &right, &right, &right, &right];
    assert!(split_flow_for_proof(grid(200, &rows), 200, 100), "a wide split flow was let through");
}

/// The same check at 8 columns, and a grid whose rows all join, so the
/// disjoint-set answers as the masks did.
#[test]
fn the_split_flow_check_keeps_its_answers_at_small_widths() {
    let rows: Vec<&[usize]> = vec![&[0, 1], &[0, 1], &[2, 3], &[2, 3], &[0, 1], &[2, 3]];
    assert!(split_flow_for_proof(grid(4, &rows), 4, 2));
    let joined: Vec<&[usize]> = vec![&[0, 1], &[1, 2], &[2, 3], &[0, 1], &[2, 3]];
    assert!(!split_flow_for_proof(grid(4, &joined), 4, 2));
}
