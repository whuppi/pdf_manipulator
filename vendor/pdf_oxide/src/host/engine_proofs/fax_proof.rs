//! The fax rows the cursor reads are the rows the whole decode carries.
//!
//! For every K the cursor serves, and for `/BlackIs1` on and off and
//! `/EncodedByteAlign` on, the row cursor (`FaxRowEngine` over a `BitFeed`,
//! behind `ImageRows`) is compared row for row against `decompress_ccitt` of
//! the same codestream — the whole-buffer decode is the oracle, kept for that
//! and for the paths that are not on the host. Then a G3 stencil reads and
//! renders the picture its whole decode draws: the same page with the stencil
//! expressed as raw samples paints the same pixels the page whose stencil
//! arrives CCITT-coded paints.
//!
//! No Dart battery can observe this: the batteries render and extract end to
//! end, and what reaches them may not change however the rows were decoded;
//! only these row values and render pixels can separate the cursor from its
//! oracle. That is why the proofs open documents here.

use super::band_proof::group4;
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::decoders::ccitt::{BitFeed, FaxRowEngine};
use crate::decoders::CcittParams;
use crate::document::PdfDocument;
use crate::extractors::ccitt_bilevel::decompress_ccitt;
use crate::host::images::stream::{ImageRows, Output};
use crate::object::ObjectRef;
use crate::rendering::{PageRenderer, RenderOptions};
use std::collections::HashMap;
use std::io::Cursor;

const WIDTH: usize = 24;
const HEIGHT: u32 = 4;
const ROW_BYTES: usize = 3;

/// The Modified-Huffman run codes the fixtures need, straight from ITU-T
/// T.4 — the same values `WHITE_CODES` / `BLACK_CODES` in
/// `src/decoders/ccitt.rs` carry. Runs stay small so a fixture needs only
/// terminating codes.
pub(super) fn mh(run: usize, black: bool) -> String {
    if black {
        match run {
            1 => "010".to_string(),
            2 => "11".to_string(),
            3 => "10".to_string(),
            4 => "011".to_string(),
            5 => "0011".to_string(),
            6 => "0010".to_string(),
            _ => panic!("the fixtures use inked runs of at most 6 pixels"),
        }
    } else {
        match run {
            1 => "000111".to_string(),
            2 => "0111".to_string(),
            3 => "1000".to_string(),
            4 => "1011".to_string(),
            5 => "1100".to_string(),
            6 => "1110".to_string(),
            7 => "1111".to_string(),
            8 => "10011".to_string(),
            9 => "10100".to_string(),
            10 => "00111".to_string(),
            11 => "01000".to_string(),
            12 => "001000".to_string(),
            _ => panic!("the fixtures use white runs of at most 12 pixels"),
        }
    }
}

/// Packs an MSB-first bit string into bytes, zero-padding the last.
pub(super) fn bits_to_bytes(bits: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut acc = 0u8;
    let mut n = 0u8;
    for ch in bits.chars().filter(|c| *c == '0' || *c == '1') {
        acc = (acc << 1) | (ch == '1') as u8;
        n += 1;
        if n == 8 {
            bytes.push(acc);
            acc = 0;
            n = 0;
        }
    }
    if n > 0 {
        bytes.push(acc << (8 - n));
    }
    bytes
}

/// One Group 3 line: a one-dimensional line coded by its white→black runs,
/// or the two-dimensional line identical to the row before it (a tag bit of
/// `0` and one T.6 V0 code).
pub(super) enum G3Line {
    Runs(Vec<usize>),
    Same,
}

/// A Group 3 stream of `lines`: a `Runs` line carries a tag bit of `1`
/// (one-dimensional) before its MH codes; a `Same` line a tag bit of `0`.
/// Lines run back-to-back (no EOL); when `byte_align`, each line after the
/// first is padded out to a byte boundary, as `/EncodedByteAlign` declares.
pub(super) fn group3(lines: &[G3Line], byte_align: bool) -> Vec<u8> {
    let mut bits = String::new();
    for (i, line) in lines.iter().enumerate() {
        if byte_align && i > 0 && bits.len() % 8 != 0 {
            bits.push_str(&"0".repeat(8 - bits.len() % 8));
        }
        match line {
            G3Line::Runs(runs) => {
                bits.push('1');
                let mut black = false;
                for &run in runs {
                    bits.push_str(&mh(run, black));
                    black = !black;
                }
            },
            G3Line::Same => {
                bits.push('0');
                bits.push('1');
            },
        }
    }
    bits_to_bytes(&bits)
}

/// A picture, four rows of 24 pixels: bands, a gap and a stripe.
pub(super) fn picture_lines() -> Vec<G3Line> {
    use G3Line::*;
    vec![Runs(vec![3, 2, 4, 5, 10]), Same, Runs(vec![5, 6, 11, 2]), Same]
}

/// One `Runs` row's picture, as the black/white pixels its runs spell.
fn runs_to_row(runs: &[usize]) -> Vec<bool> {
    let mut row = Vec::with_capacity(WIDTH);
    let mut black = false;
    for &run in runs {
        row.extend(std::iter::repeat(black).take(run));
        black = !black;
    }
    assert_eq!(row.len(), WIDTH, "a fixture row spells its full width");
    row
}

/// The rows via the cursor: [`FaxRowEngine`] over a [`BitFeed`] holding
/// `bytes`, driven into every row the picture has. The `black_is_1` flip the
/// whole decode applies to its output is folded here, so the sides agree on
/// what a `1` bit says. White pads the rows past the stream, as the oracle
/// does to a `/Rows` it knows.
fn cursor_rows(bytes: Vec<u8>, params: &CcittParams, height: u32) -> Vec<u8> {
    let mut engine = FaxRowEngine::new(params).expect("the params decode");
    let mut feed = BitFeed::new(Box::new(Cursor::new(bytes)));
    let row_bytes = (params.columns as usize).div_ceil(8);
    let mut packed = Vec::new();
    let mut row = vec![0u8; row_bytes];
    while engine.pull_row(&mut feed, Some(height), &mut row).expect("the cursor decodes") {
        if params.black_is_1 {
            for b in &mut row {
                *b = !*b;
            }
        }
        packed.extend_from_slice(&row);
    }
    if let Some(rows) = params.rows {
        packed.resize(rows as usize * row_bytes, 0);
    }
    packed
}

/// The cursor and the whole decode sit behind the same wire: a battery can
/// only see rendered pages and extracted files, which agree whenever the two
/// decode alike, and can never pull the two decoders apart row by row.
#[test]
fn cursor_rows_equal_the_whole_decode() {
    let lines = picture_lines();
    // The picture as pixels: each `Runs` row, then the row it repeats.
    let mut rows: Vec<Vec<bool>> = Vec::new();
    for line in &lines {
        let row = match line {
            G3Line::Runs(runs) => runs_to_row(runs),
            G3Line::Same => rows.last().expect("a repeat follows its row").clone(),
        };
        rows.push(row);
    }

    // K = -1, Group 4: the T.6 stream `fax::encoder` writes for the picture.
    let encoded = group4(&rows);
    let params = CcittParams { k: -1, columns: WIDTH as u32, rows: Some(HEIGHT), ..Default::default() };
    assert_eq!(
        cursor_rows(encoded.clone(), &params, HEIGHT),
        decompress_ccitt(&encoded, &params).expect("the oracle decodes"),
        "K = -1, the encoder's stream"
    );

    // K = -1, byte-aligned: each coded row of a hand-built stream begins a
    // byte, five all-white V0 rows.
    let aligned = bits_to_bytes(&"1".repeat(5));
    let params = CcittParams {
        k: -1,
        columns: WIDTH as u32,
        rows: Some(5),
        encoded_byte_align: true,
        ..Default::default()
    };
    assert_eq!(
        cursor_rows(aligned.clone(), &params, 5),
        decompress_ccitt(&aligned, &params).expect("the oracle decodes"),
        "K = -1, byte-aligned rows"
    );

    // K = 0 (pure 1-D): `BlackIs1` off and on, and byte-aligned.
    let encoded = group3(&lines, false);
    for black_is_1 in [false, true] {
        let params = CcittParams {
            k: 0,
            columns: WIDTH as u32,
            rows: Some(HEIGHT),
            black_is_1,
            ..Default::default()
        };
        assert_eq!(
            cursor_rows(encoded.clone(), &params, HEIGHT),
            decompress_ccitt(&encoded, &params).expect("the oracle decodes"),
            "K = 0, BlackIs1 {black_is_1}"
        );
    }
    let byte_aligned = group3(&lines, true);
    let params = CcittParams {
        k: 0,
        columns: WIDTH as u32,
        rows: Some(HEIGHT),
        encoded_byte_align: true,
        ..Default::default()
    };
    assert_eq!(
        cursor_rows(byte_aligned.clone(), &params, HEIGHT),
        decompress_ccitt(&byte_aligned, &params).expect("the oracle decodes"),
        "K = 0, byte-aligned"
    );

    // K = 2 (mixed 1-D/2-D): rows one and three code one-dimensionally, rows
    // two and four repeat them, raw and byte-aligned.
    for byte_align in [false, true] {
        let encoded = group3(&lines, byte_align);
        let params = CcittParams {
            k: 2,
            columns: WIDTH as u32,
            rows: Some(HEIGHT),
            encoded_byte_align: byte_align,
            ..Default::default()
        };
        assert_eq!(
            cursor_rows(encoded.clone(), &params, HEIGHT),
            decompress_ccitt(&encoded, &params).expect("the oracle decodes"),
            "K = 2, byte-aligned {byte_align}"
        );
    }
}

fn one_page_pdf(page_box: &str, content: &str, resources: &str, fifth: &str, data: &[u8]) -> Vec<u8> {
    let objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox {page_box} \
             /Resources << {resources} >> /Contents 4 0 R >>\nendobj\n"
        )
        .into_bytes(),
        stream_object(4, "", content.as_bytes()),
        format!("5 0 obj\n{fifth} /Length {} >>\nstream\n", data.len()).into_bytes(),
        data.to_vec(),
        b"\nendstream\nendobj\n".to_vec(),
    ];
    pdf_from_objects(&objects)
}

/// The rows `ImageRows` serves are internal values: a battery observes the
/// PNG the extractor writes, which is identical whatever the rows behind it
/// say, so only a row-by-row hold on the cursor can show it decodes what
/// the whole decode does.
#[test]
fn the_image_rows_the_cursor_reads_are_the_whole_decode() {
    // A G3 1-D image, `/BlackIs1` off and on. The rows `ImageRows` serves
    // must be what the whole-buffer decode unpacks: a luma row a pixel byte,
    // 0xFF white and 0x00 inked. A battery watches only the rendered page;
    // the row values behind it are the cursor's side and can drift from the
    // whole decode in a battery's blind spot.
    for black_is_1 in [false, true] {
        let encoded = group3(&picture_lines(), false);
        let params_text = format!(
            "/K 0 /Columns {WIDTH} /Rows {HEIGHT} {}",
            if black_is_1 { "/BlackIs1 true" } else { "" }
        );
        let pdf = one_page_pdf(
            "[0 0 120 90]",
            "q 120 0 0 90 0 0 cm /Im Do Q\n",
            "/XObject << /Im 5 0 R >>",
            &format!(
                "<< /Type /XObject /Subtype /Image /Width {WIDTH} /Height {HEIGHT} \
                 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter /CCITTFaxDecode \
                 /DecodeParms << {params_text} >>"
            ),
            &encoded,
        );
        let doc = PdfDocument::from_bytes(pdf).expect("fixture parses");
        let xobject = doc.load_object(ObjectRef::new(5, 0)).expect("the image object");
        let mut rows = ImageRows::open(&doc, &xobject, &HashMap::new(), None, Output::Luma, None)
            .expect("the cursor opens");
        assert_eq!(
            rows.size(),
            (WIDTH as u32, HEIGHT),
            "the cursor serves the image, a row at a time"
        );

        let params = CcittParams {
            k: 0,
            columns: WIDTH as u32,
            rows: Some(HEIGHT),
            black_is_1,
            ..Default::default()
        };
        let oracle = decompress_ccitt(&encoded, &params).expect("the whole decode answers");
        for y in 0..HEIGHT {
            // The oracle holds packed rows (1 = ink); the cursor serves luma
            // rows.
            let mut expected = Vec::with_capacity(WIDTH);
            for x in 0..WIDTH {
                let bit = (oracle[y as usize * ROW_BYTES + x / 8] >> (7 - (x % 8))) & 1;
                expected.push(if bit == 1 { 0x00 } else { 0xFF });
            }
            let mut actual = vec![0u8; WIDTH];
            rows.read(&doc, &mut actual).expect("a row");
            assert_eq!(actual, expected, "the rows the cursor reads, image path");
        }
        let mut over = vec![0u8; WIDTH];
        rows.read(&doc, &mut over).expect_err("no rows past the image");
    }
}

/// The two pages the proof compares differ only in how the stencil's rows
/// reach the engine, which the render output hides whenever the two legs
/// decode alike: the equality of the render pixels is the statement a
/// battery cannot distinguish a broken G3 decode within.
#[test]
fn the_g3_stencil_reads_and_renders_the_whole_decode() {
    // A G3 stencil (/K 0 and /K 2), the default /Decode. What repaints it
    // must be what the whole decode draws: one page with the stencil coded,
    // one with the whole decode's samples written as raw stencil samples,
    // and both render the same pixels. `open_stencil`'s rows are read too:
    // the mask's luma a band samples (255 where the pixel is marked, 0
    // where its sample says paint) has no other way to say what the whole
    // decode carried.
    for k in [0, 2] {
        let encoded = group3(&picture_lines(), false);
        let params = CcittParams {
            k,
            columns: WIDTH as u32,
            rows: Some(HEIGHT),
            ..Default::default()
        };
        let oracle = decompress_ccitt(&encoded, &params).expect("the whole decode answers");

        // The whole decode's rows in image-mask sample space: 0 marks the
        // page, so a pixel whose sample reads 0 is one the decoder inked.
        let inked = |row: &[u8], x: usize| (row[x / 8] >> (7 - (x % 8))) & 1 == 1;
        let mut raw_samples = vec![0u8; ROW_BYTES * HEIGHT as usize];
        for y in 0..HEIGHT as usize {
            for x in 0..WIDTH {
                if !inked(&oracle[y * ROW_BYTES..], x) {
                    raw_samples[y * ROW_BYTES + x / 8] |= 0x80 >> (x % 8);
                }
            }
        }

        let coded_pdf = one_page_pdf(
            "[0 0 120 90]",
            "0.3 0.7 0.2 rg q 48 0 0 16 20 30 cm /St Do Q\n",
            "/XObject << /St 5 0 R >>",
            &format!(
                "<< /Type /XObject /Subtype /Image /ImageMask true /Width {WIDTH} \
                 /Height {HEIGHT} /BitsPerComponent 1 /Filter /CCITTFaxDecode \
                 /DecodeParms << /K {k} /Columns {WIDTH} /Rows {HEIGHT} >>"
            ),
            &encoded,
        );
        let plain_pdf = one_page_pdf(
            "[0 0 120 90]",
            "0.3 0.7 0.2 rg q 48 0 0 16 20 30 cm /St Do Q\n",
            "/XObject << /St 5 0 R >>",
            &format!(
                "<< /Type /XObject /Subtype /Image /ImageMask true /Width {WIDTH} \
                 /Height {HEIGHT} /BitsPerComponent 1"
            ),
            &raw_samples,
        );
        let coded_doc = PdfDocument::from_bytes(coded_pdf).expect("fixture parses");
        let plain_doc = PdfDocument::from_bytes(plain_pdf).expect("fixture parses");

        // `open_stencil`'s luma masks: byte per pixel, 0xFF where the pixel
        // is marked (its sample says put the base image there) — the
        // complement of the marked samples.
        let xobject = coded_doc.load_object(ObjectRef::new(5, 0)).expect("the stencil object");
        let mut mask = ImageRows::open_stencil(&coded_doc, &xobject).expect("the cursor opens");
        assert_eq!(mask.size(), (WIDTH as u32, HEIGHT));
        for y in 0..HEIGHT {
            let mut actual = vec![0u8; WIDTH];
            mask.read(&coded_doc, &mut actual).expect("a row");
            let mut expected = Vec::with_capacity(WIDTH);
            for x in 0..WIDTH {
                // The mask marks the base image where the sample is inked
                // (the whole decode's `1`), and covers where it is not: a
                // marked pixel passes the base (255), the rest do not.
                expected.push(if inked(&oracle[y as usize * ROW_BYTES..], x) {
                    255
                } else {
                    0
                });
            }
            assert_eq!(actual, expected, "the rows the cursor reads, stencil path");
        }
        let mut over = vec![0u8; WIDTH];
        mask.read(&coded_doc, &mut over).expect_err("no rows past the stencil");

        // The two pages paint the same pixels.
        let mut renderer =
            PageRenderer::new(RenderOptions::with_dpi(72).with_transparent_background());
        let mut render = |doc: &PdfDocument, _tag: &str| {
            let mut raster = Vec::new();
            let _ = renderer
                .render_bands(
                    doc,
                    0,
                    &|_, height| 0..height,
                    &CancelCheck::none(),
                    &mut |_, band| {
                        raster.extend_from_slice(band.data());
                        Ok(())
                    },
                )
                .expect("renders");
            raster
        };
        let plain = render(&plain_doc, "the raw samples");
        let coded = render(&coded_doc, &format!("coded K={k}"));
        assert_eq!(coded, plain, "the stencil paints the whole decode's picture");
    }
}

/// A CCITT image (not a stencil) whose `/DecodeParms` array holds its
/// parameters by reference renders the page its inline form renders. The
/// image path resolves each array entry before it reads the parameters, so
/// the reference is a dictionary the decode sees, not an unresolved entry.
/// No Dart battery sees it: the batteries build no image whose parameters
/// sit behind a reference inside an array.
#[test]
fn a_ccitt_image_decode_params_array_by_reference_renders_as_inline() {
    fn gray_ccitt_pdf(indirect_entry: bool) -> Vec<u8> {
        let entry = if indirect_entry { "6 0 R" } else { "<< /K -1 /Columns 8 /Rows 1 >>" };
        let mut objects = vec![
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 80 10] \
              /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
                .to_vec(),
            stream_object(4, "", b"q 80 0 0 10 0 0 cm /Im0 Do Q\n"),
            format!(
                "5 0 obj\n<< /Type /XObject /Subtype /Image /Width 8 /Height 1 \
                 /BitsPerComponent 1 /ColorSpace /DeviceGray \
                 /Filter [/ASCIIHexDecode /CCITTFaxDecode] /DecodeParms [null {entry}] \
                 /Length 5 >>\nstream\n31C0>\nendstream\nendobj\n"
            )
            .into_bytes(),
        ];
        if indirect_entry {
            objects.push(b"6 0 obj\n<< /K -1 /Columns 8 /Rows 1 >>\nendobj\n".to_vec());
        }
        pdf_from_objects(&objects)
    }

    let render = |pdf: Vec<u8>| {
        let doc = PdfDocument::from_bytes(pdf).expect("fixture parses");
        let mut renderer =
            PageRenderer::new(RenderOptions::with_dpi(72).with_transparent_background());
        let mut raster = Vec::new();
        let _ = renderer
            .render_bands(
                &doc,
                0,
                &|_, height| 0..height,
                &CancelCheck::none(),
                &mut |_, band| {
                    raster.extend_from_slice(band.data());
                    Ok(())
                },
            )
            .expect("renders");
        raster
    };
    let inline = render(gray_ccitt_pdf(false));
    assert!(inline.iter().any(|&b| b != 0), "the inline form paints the picture");
    assert_eq!(render(gray_ccitt_pdf(true)), inline, "the by-reference array renders as inline");
}
