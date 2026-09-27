//! B6 / #364 — FlateDecode content stream must decompress correctly across
//! the stream-framing patterns PDF producers use in the wild.
//!
//! MS Reporting Services PDFs (nougat_026.pdf in the Kreuzberg corpus) are
//! reported to decompress to 128 bytes of repeating garbage where `pdftotext`
//! extracts normal prose. The issue hypothesises stream-boundary off-by-one
//! handling around CRLF terminators and/or a partial-recovery path in the
//! FlateDecoder that accepts mid-stream-error output.
//!
//! Without the actual nougat_026 fixture we cannot reproduce the exact byte
//! pattern. These synthetic tests instead pin every boundary shape that the
//! parser is expected to accept per ISO 32000-1 §7.3.8.1:
//!
//!   1. `stream\r\n ... \r\nendstream`   (MS Reporting Services / producer-default)
//!   2. `stream\n ... \nendstream`       (Unix-style)
//!   3. `stream\r\n ... endstream`       (no trailing EOL before `endstream`)
//!   4. `stream\r\n ... \r\nendstream`   with /Length supplied as an
//!      indirect reference — a case where the parser must resolve through
//!      `find_endstream` because the lexical pass sees `5 0 R` and cannot
//!      call `as_integer()` on a Reference.
//!
//! Each sub-test compresses the same multi-kB text-operator payload and
//! verifies that text extraction yields the first and last line, so a
//! silent truncation to "Line 00" only (the #364 failure mode) surfaces
//! immediately.
use flate2::write::ZlibEncoder;
use flate2::Compression;
use pdf_oxide::PdfDocument;
use std::io::Write;

/// A multi-kB content stream with distinctive first/last lines so the test
/// can detect truncation. Absolute text matrix (`Tm`) is used so each line
/// lands on-page regardless of the previous text state.
fn content_stream_text() -> String {
    let mut content = String::new();
    for i in 0..64 {
        let y = 800 - i * 12;
        content.push_str(&format!(
            "BT /F0 12 Tf 1 0 0 1 100 {y} Tm (Line {i:02}: quick brown fox jumps over lazy dog) Tj ET\n"
        ));
    }
    content
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// Write one PDF object record: `{id} 0 obj\n{body}\nendobj\n`.
fn push_obj(out: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &[u8]) {
    offsets.push(out.len());
    let id = offsets.len() - 1;
    out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(b"\nendobj\n");
}

/// Append a FlateDecode-compressed stream object whose `stream`/`endstream`
/// framing is controlled by `open_eol` / `close_eol`. `length_ref` overrides
/// the in-dict /Length with the literal string provided (used to write an
/// indirect-reference form like `5 0 R`).
fn push_stream(
    out: &mut Vec<u8>,
    offsets: &mut Vec<usize>,
    compressed: &[u8],
    open_eol: &[u8],
    close_eol: &[u8],
    length_literal: Option<&str>,
) {
    offsets.push(out.len());
    let id = offsets.len() - 1;
    out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    let length_str = length_literal
        .map(|s| s.to_string())
        .unwrap_or_else(|| compressed.len().to_string());
    out.extend_from_slice(
        format!("<< /Length {length_str} /Filter /FlateDecode >>\nstream").as_bytes(),
    );
    out.extend_from_slice(open_eol);
    out.extend_from_slice(compressed);
    out.extend_from_slice(close_eol);
    out.extend_from_slice(b"endstream\nendobj\n");
}

fn finish_pdf(out: &mut Vec<u8>, offsets: &[usize]) {
    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &offsets[1..] {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
}

/// Header + catalog/pages/page objects shared across fixtures.
fn pdf_preamble() -> (Vec<u8>, Vec<usize>) {
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = vec![0];
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    push_obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    push_obj(&mut out, &mut offsets, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    push_obj(
        &mut out,
        &mut offsets,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] \
           /Resources << /Font << /F0 5 0 R >> >> /Contents 4 0 R >>",
    );
    (out, offsets)
}

fn font_obj(out: &mut Vec<u8>, offsets: &mut Vec<usize>) {
    push_obj(out, offsets, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
}

fn assert_both_ends_present(text: &str, variant: &str) {
    assert!(
        text.contains("Line 00"),
        "[{variant}] missing first line — stream-open EOL mis-skipped. Text: {text:?}"
    );
    assert!(
        text.contains("Line 63"),
        "[{variant}] missing last line — stream was truncated (the #364 symptom). Text: {text:?}"
    );
    let distinct_words: std::collections::HashSet<&str> = text
        .split_whitespace()
        .filter(|w| w.chars().all(|c| c.is_ascii_alphabetic()))
        .collect();
    assert!(
        distinct_words.len() >= 5,
        "[{variant}] output looks like decompression garbage: only {} distinct alphabetic words. Text: {text:?}",
        distinct_words.len()
    );
}

#[test]
fn crlf_framed_flate_content_stream_decompresses_cleanly() {
    let content = content_stream_text();
    assert!(content.len() > 2000, "content stream must be multi-kB");
    let compressed = zlib_compress(content.as_bytes());

    let (mut out, mut offsets) = pdf_preamble();
    push_stream(&mut out, &mut offsets, &compressed, b"\r\n", b"\r\n", None);
    font_obj(&mut out, &mut offsets);
    finish_pdf(&mut out, &offsets);

    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &out).unwrap();
    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");
    assert_both_ends_present(&text, "CRLF + CRLF");
}

#[test]
fn lf_framed_flate_content_stream_decompresses_cleanly() {
    let compressed = zlib_compress(content_stream_text().as_bytes());
    let (mut out, mut offsets) = pdf_preamble();
    push_stream(&mut out, &mut offsets, &compressed, b"\n", b"\n", None);
    font_obj(&mut out, &mut offsets);
    finish_pdf(&mut out, &offsets);

    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &out).unwrap();
    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");
    assert_both_ends_present(&text, "LF + LF");
}

#[test]
fn crlf_open_no_close_eol_decompresses_cleanly() {
    // Producers sometimes omit the CR/LF before `endstream`. Per spec, /Length
    // counts only the stream data bytes, so missing trailing whitespace should
    // be fine if /Length is exact.
    let compressed = zlib_compress(content_stream_text().as_bytes());
    let (mut out, mut offsets) = pdf_preamble();
    push_stream(&mut out, &mut offsets, &compressed, b"\r\n", b"", None);
    font_obj(&mut out, &mut offsets);
    finish_pdf(&mut out, &offsets);

    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &out).unwrap();
    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");
    assert_both_ends_present(&text, "CRLF + no-close-EOL");
}

#[test]
fn indirect_length_crlf_framed_stream_decompresses_cleanly() {
    // MS Reporting Services (and many other producers) emit streams with
    // `/Length X 0 R` pointing at a separate integer object. The parser's
    // lexical pass sees a Reference for /Length and cannot call `as_integer()`
    // on it, so it has to scan for `endstream` — which is the most fragile
    // path through `parse_stream_data` in `src/parser.rs`.
    let content = content_stream_text();
    let compressed = zlib_compress(content.as_bytes());
    let length_val = compressed.len();

    // We cannot reuse `pdf_preamble` because object IDs need to accommodate a
    // separate Length object. Object layout:
    //   1 Catalog, 2 Pages, 3 Page, 4 Contents stream, 5 Font, 6 Length
    let mut out: Vec<u8> = Vec::new();
    let mut offsets: Vec<usize> = vec![0];
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    push_obj(&mut out, &mut offsets, b"<< /Type /Catalog /Pages 2 0 R >>");
    push_obj(&mut out, &mut offsets, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    push_obj(
        &mut out,
        &mut offsets,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 900] \
           /Resources << /Font << /F0 5 0 R >> >> /Contents 4 0 R >>",
    );
    // Object 4: stream with /Length 6 0 R (indirect)
    push_stream(&mut out, &mut offsets, &compressed, b"\r\n", b"\r\n", Some("6 0 R"));
    font_obj(&mut out, &mut offsets);
    // Object 6: the length integer.
    push_obj(&mut out, &mut offsets, format!("{length_val}").as_bytes());
    finish_pdf(&mut out, &offsets);

    let tmp = tempfile::NamedTempFile::new().expect("temp");
    std::fs::write(tmp.path(), &out).unwrap();
    let doc = PdfDocument::open(tmp.path()).expect("open");
    let text = doc.extract_text(0).expect("extract");
    assert_both_ends_present(&text, "indirect-Length + CRLF");
}
