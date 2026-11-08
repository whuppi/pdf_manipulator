//! `page_operators`, `form_operators` and the content cache against the
//! whole-buffer content paths they replace.

use super::content_stream::{known_divergence_given, merge_tj, reference, short};
use super::filter_readers::{HELD, PEAK};
use super::support::{corpus_pdfs, pdf_from_objects, stream_object};
use crate::content::{parse_content_stream, Operator};
use crate::document::PdfDocument;
use crate::host::content::cache::{ContentCache, CONTENT_CACHE_BYTES};
use crate::host::content::parts::TEE_HIGH_WATER;
use crate::object::{Object, ObjectRef};
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn collect<I: Iterator<Item = crate::error::Result<Operator>>>(ops: I) -> Vec<Operator> {
    ops.map(|op| op.expect("an operator")).collect()
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// A one-page document whose /Contents is `contents` (a dictionary entry text) and whose other objects are `extra`, numbered from 4.
fn page_with(contents: &str, extra: &[Vec<u8>]) -> Vec<u8> {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!("3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents {contents} >>\nendobj\n").into_bytes(),
    ];
    objects.extend_from_slice(extra);
    pdf_from_objects(&objects)
}

/// No Dart battery sees it: no Dart API returns the operator sequence of a page or form.
#[test]
fn page_and_form_operators_equal_the_whole_buffer_parse_on_every_fixture() {
    // The reference parser is the slow side; a stream beyond this is left to
    // the memory proofs below.
    const REFERENCE_LIMIT: usize = 384 * 1024;
    let (mut pdfs, mut pages, mut forms, mut operators, mut excluded, mut too_large) =
        (0, 0, 0, 0, 0, 0);
    let mut mismatches = Vec::new();
    for path in corpus_pdfs() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Ok(doc) = PdfDocument::open(&path) else { continue };
        pdfs += 1;
        for page in 0..doc.page_count().unwrap_or(0) {
            let label = format!("{name} page {page}");
            let Ok(content) = doc.get_page_content_data(page) else {
                if doc.page_operators(page).is_ok() {
                    mismatches.push(format!("{label}: today errors, page_operators does not"));
                }
                continue;
            };
            if content.len() > REFERENCE_LIMIT {
                too_large += 1;
                continue;
            }
            let (expected, _) = reference(&content);
            if let Some(cause) = known_divergence_given(&content, &expected) {
                println!("EXCLUDED {label}: {cause}");
                excluded += 1;
                continue;
            }
            let mut ops = match doc.page_operators(page) {
                Ok(ops) => ops,
                Err(e) => {
                    mismatches.push(format!("{label}: page_operators fails: {e}"));
                    continue;
                },
            };
            let got = merge_tj(collect(ops.by_ref()));
            pages += 1;
            operators += got.len();
            if ops.damage().parts_unreadable != 0 {
                mismatches.push(format!("{label}: {} parts unreadable", ops.damage().parts_unreadable));
            }
            if got != expected {
                let at = got.iter().zip(&expected).position(|(a, b)| a != b).unwrap_or(got.len().min(expected.len()));
                mismatches.push(format!(
                    "{label}: {} operators, expected {}; first difference at {at}: got {}, expected {}",
                    got.len(),
                    expected.len(),
                    got.get(at).map_or("none".into(), short),
                    expected.get(at).map_or("none".into(), short),
                ));
            }
        }
        // Every Form XObject of the document, twice: the second read is the cache's.
        for id in doc.all_object_ids() {
            if id == 0 || doc.container_stream_of(id).is_some() || doc.is_free_entry(id) {
                continue;
            }
            let r = ObjectRef::new(id, 0);
            let Ok(object @ Object::Stream { .. }) = doc.load_object(r) else { continue };
            let is_form = object.as_dict().and_then(|d| d.get("Subtype")).and_then(|s| s.as_name()) == Some("Form");
            if !is_form {
                continue;
            }
            let label = format!("{name} form {id}");
            // A stream that cannot be decoded has no operators to compare.
            let Ok(data) = object.decode_stream_data() else { continue };
            if data.len() > REFERENCE_LIMIT {
                too_large += 1;
                continue;
            }
            let (expected, _) = reference(&data);
            if known_divergence_given(&data, &expected).is_some() {
                excluded += 1;
                continue;
            }
            // The second read is the cache's, for the forms small enough to be cached.
            let rounds: &[&str] = if data.len() <= 64 * 1024 { &["miss", "hit"] } else { &["miss"] };
            for &round in rounds {
                let mut ops = doc.form_operators(r).expect("a form opens");
                let got = merge_tj(collect(ops.by_ref()));
                if got != expected {
                    mismatches.push(format!("{label} ({round}): {} operators, expected {}", got.len(), expected.len()));
                }
                operators += got.len();
            }
            forms += 1;
        }
    }
    println!(
        "{pdfs} pdfs, {pages} pages, {forms} forms, {operators} operators compared, {excluded} excluded for a documented divergence, {too_large} over {REFERENCE_LIMIT} bytes left out, {} mismatches",
        mismatches.len()
    );
    for m in &mismatches {
        println!("MISMATCH {m}");
    }
    assert!(pages > 0 && forms > 0, "{pages} pages and {forms} forms compared");
    assert!(mismatches.is_empty(), "{mismatches:#?}");
}

/// No Dart battery sees it: a skipped content part leaves no trace in the rendered result beyond the missing marks.
#[test]
fn an_unreadable_part_is_skipped_and_counted_as_the_whole_buffer_path_skips_it() {
    let good_a = stream_object(4, "", b"BT (one) Tj ET");
    let bad = stream_object(5, "/Filter /NoSuchFilter", b"data");
    let good_b = stream_object(6, "", b"BT (three) Tj ET");
    // 99 0 R names no object: it loads as null, an empty part.
    let bytes = page_with("[4 0 R 5 0 R 99 0 R 6 0 R]", &[good_a, bad, good_b]);
    let doc = PdfDocument::from_bytes(bytes).unwrap();
    let today = doc.get_page_content_data(0).unwrap();
    let expected = parse_content_stream(&today).unwrap();
    let mut ops = doc.page_operators(0).unwrap();
    let got = collect(ops.by_ref());
    assert_eq!(got, expected);
    assert_eq!(got.iter().filter(|o| matches!(o, Operator::Tj { .. })).count(), 2);
    assert_eq!(ops.damage().parts_unreadable, 1);

    // A single stream that cannot be decoded is an error, as it is today.
    let single = page_with("4 0 R", &[stream_object(4, "/Filter /NoSuchFilter", b"data")]);
    let doc = PdfDocument::from_bytes(single).unwrap();
    assert!(doc.get_page_content_data(0).is_err());
    assert!(doc.page_operators(0).is_err());

    // A page with no /Contents has no operators.
    let blank = PdfDocument::from_bytes(pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 10 10] >>\nendobj\n".to_vec(),
    ]))
    .unwrap();
    assert_eq!(blank.page_operators(0).unwrap().count(), 0);
}

/// No Dart battery sees it: how a token that runs across two content parts reads is an engine detail.
#[test]
fn a_token_split_across_two_parts_reads_as_the_newline_join_reads() {
    let a = stream_object(4, "", b"BT (Hel");
    let b = stream_object(5, "", b"lo) Tj ET");
    let doc = PdfDocument::from_bytes(page_with("[4 0 R 5 0 R]", &[a, b])).unwrap();
    let today = doc.get_page_content_data(0).unwrap();
    assert_eq!(today, b"BT (Hel\nlo) Tj ET\n");
    let expected = parse_content_stream(&today).unwrap();
    let got = collect(doc.page_operators(0).unwrap());
    println!("split token, today: {expected:?}");
    assert_eq!(got, expected);
    assert!(
        got.iter().any(|op| matches!(op, Operator::Tj { text } if text == b"Hel\nlo")),
        "the string holds the newline the join put in it: {got:?}"
    );
}

fn form_pdf(count: usize, size: usize) -> Vec<u8> {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>\nendobj\n".to_vec(),
    ];
    for i in 0..count {
        // Lines of a kilobyte of text each: the bytes are the point, not the operator count.
        let mut content = Vec::with_capacity(size + 1200);
        let mut n = i * 1000;
        while content.len() < size {
            content.extend_from_slice(format!("BT ({}{}) Tj ET\n", n, "x".repeat(1000)).as_bytes());
            n += 1;
        }
        let compressed = zlib(&content);
        objects.push(
            [
                format!(
                    "{} 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] /Filter /FlateDecode /Length {} >>\nstream\n",
                    4 + i,
                    compressed.len()
                )
                .into_bytes(),
                compressed,
                b"\nendstream\nendobj\n".to_vec(),
            ]
            .concat(),
        );
    }
    pdf_from_objects(&objects)
}

/// No Dart battery sees it: it reads the engine's content cache, which no Dart call exposes.
#[test]
fn the_content_cache_serves_hits_and_never_charges_more_than_its_budget() {
    // Direct: 200 entries of 64 KiB against a 4 MiB budget.
    let mut cache = ContentCache::new();
    for i in 0..200u32 {
        assert!(cache.insert(ObjectRef::new(i, 0), Arc::new(vec![i as u8; 64 * 1024])));
        assert!(cache.used() <= CONTENT_CACHE_BYTES, "after {i}: {}", cache.used());
    }
    assert!(cache.len() < 200 && cache.len() >= 60, "{} entries", cache.len());
    assert!(cache.get(ObjectRef::new(0, 0)).is_none(), "the oldest was evicted");
    assert!(cache.get(ObjectRef::new(199, 0)).is_some());
    // A hit marks the entry as used, so it outlives the ones used after it.
    let keep = ObjectRef::new(150, 0);
    cache.get(keep);
    for i in 200..260u32 {
        cache.insert(ObjectRef::new(i, 0), Arc::new(vec![0u8; 64 * 1024]));
    }
    assert!(cache.get(keep).is_some(), "a recently used entry survives");
    assert!(!cache.insert(ObjectRef::new(9999, 0), Arc::new(vec![0u8; 5 * 1024 * 1024])));

    // Through the document: 200 forms of 64 KiB, each read twice.
    let doc = PdfDocument::from_bytes(form_pdf(200, 64 * 1024)).unwrap();
    let mut first = Vec::new();
    for i in 0..200u32 {
        let r = ObjectRef::new(4 + i, 0);
        let ops = collect(doc.form_operators(r).unwrap());
        assert!(!ops.is_empty());
        first.push(ops);
        let used = doc.content_cache.lock().unwrap().used();
        assert!(used <= CONTENT_CACHE_BYTES, "form {i}: {used} bytes charged");
    }
    // The most recent forms are cached: their second read is the same operators.
    for i in (190..200u32).rev() {
        let r = ObjectRef::new(4 + i, 0);
        assert!(doc.content_cache.lock().unwrap().get(r).is_some(), "form {i} should be cached");
        assert_eq!(collect(doc.form_operators(r).unwrap()), first[i as usize], "form {i} served from the cache");
    }
    let cache = doc.content_cache.lock().unwrap();
    println!("200 forms: {} cached, {} bytes charged", cache.len(), cache.used());
}

/// No Dart battery sees it: it counts what the engine's tee holds, which no Dart call exposes.
#[test]
fn a_form_larger_than_the_budget_is_never_admitted_and_the_tee_never_outgrows_the_budget() {
    // 10 MiB of path operators in a Flate stream.
    let doc = PdfDocument::from_bytes(form_pdf(1, 10 * 1024 * 1024)).unwrap();
    let r = ObjectRef::new(4, 0);
    TEE_HIGH_WATER.with(|h| h.set(0));
    let count = doc.form_operators(r).unwrap().count();
    assert!(count > 10_000, "{count} operators");
    let high = TEE_HIGH_WATER.with(|h| h.get());
    println!("10 MiB form: the tee held at most {high} bytes");
    assert!(high <= CONTENT_CACHE_BYTES, "the tee held {high} bytes");
    assert!(doc.content_cache.lock().unwrap().get(r).is_none(), "a form over the budget is not cached");
    assert_eq!(doc.content_cache.lock().unwrap().used(), 0);
    assert_eq!(doc.form_operators(r).unwrap().count(), count, "and it streams every time");
}

/// No Dart battery sees it: the memory a page's operators hold is the engine's own allocator counter, and Dart sees only the extracted result.
#[test]
fn a_page_of_96_mib_of_flate_content_iterates_in_under_a_mebibyte_of_growth() {
    const TOTAL: usize = 96 * 1024 * 1024;
    let line = b"BT /F1 12 Tf 1 0 0 1 10 20 Tm (hello) Tj ET\n";
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    // Whole lines only, so no token is cut where one block meets the next.
    let block: Vec<u8> = line.repeat(64 * 1024 / line.len());
    for _ in 0..TOTAL / block.len() {
        encoder.write_all(&block).unwrap();
    }
    let compressed = encoder.finish().unwrap();
    let doc = PdfDocument::from_bytes(page_with(
        "4 0 R",
        &[stream_object(4, "/Filter /FlateDecode", &compressed)],
    ))
    .unwrap();
    drop(compressed);

    let mut ops = doc.page_operators(0).unwrap();
    // The decoder's tables and the lexer window exist before the baseline.
    for _ in 0..1000 {
        ops.next().unwrap().unwrap();
    }
    let base = HELD.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let mut count = 1000usize;
    for op in ops.by_ref() {
        op.unwrap();
        count += 1;
    }
    let growth = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    println!("96 MiB page: {count} operators, peak growth {growth} bytes");
    assert!(count > 3_000_000, "{count} operators");
    assert!(ops.damage().is_clean(), "{:?}", ops.damage());
    assert!(growth < 1024 * 1024, "iterating 96 MiB grew the process by {growth} bytes");
}

/// No Dart battery sees it: the document's structured warnings are an engine-side list, and what Dart sees is the same bytes with or without the warning.
#[test]
fn a_checksum_mismatch_warning_names_its_stream_in_the_documents_sink() {
    let mut text = Vec::new();
    while text.len() < 300_000 {
        text.extend_from_slice(b"BT /F1 12 Tf (checksum) Tj ET\n");
    }
    let mut compressed = zlib(&text);
    let last = compressed.len() - 1;
    compressed[last] ^= 0x01; // the Adler-32 trailer
    let doc = PdfDocument::from_bytes(page_with(
        "4 0 R",
        &[stream_object(4, "/Filter /FlateDecode", &compressed)],
    ))
    .unwrap();
    let decoded = doc.load_object(ObjectRef::new(4, 0)).unwrap().decode_stream_data().unwrap();
    assert_eq!(decoded, text, "every byte is kept");
    let warnings = doc.structured_warnings();
    let named: Vec<_> = warnings
        .iter()
        .filter(|w| w.message.contains("checksum mismatch, data kept") && w.message.contains("object 4 0"))
        .collect();
    assert_eq!(named.len(), 1, "{warnings:?}");
}
