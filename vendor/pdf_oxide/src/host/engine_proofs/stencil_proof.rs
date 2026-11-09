//! The stencil masks' rows are read a band at a time; they are the rows the
//! whole decode carried.
//!
//! For raw samples, Flate, Flate with a PNG predictor and CCITT Group 3 and
//! Group 4, each under the default `/Decode` and `/Decode [1 0]`, the luma
//! rows `open_stencil` serves row by row are the rows the whole decode's
//! samples give (the whole decode is the oracle, reimplemented here over
//! the same codestream), a stencil painted in bands paints the pixels one
//! whole-page render paints, and an image whose `/Mask` is a coded stencil
//! paints the pixels the same image with the mask's samples written raw
//! paints.
//!
//! No Dart battery can observe this: the batteries render and extract end
//! to end, and what reaches them may not change however the rows were
//! read; only these row values, the twin pages and the forced band
//! heights can separate the row cursor from its oracle. That is why the
//! proofs open documents here.

use super::band_proof::{group4, packed_bits, prove as prove_bands, Fixture};
use super::fax_proof::{group3, G3Line};
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::decoders::ccitt::{BitFeed, FaxRowEngine};
use crate::decoders::CcittParams;
use crate::document::PdfDocument;
use crate::host::images::stream::ImageRows;
use crate::object::ObjectRef;
use crate::rendering::{PageRenderer, RenderOptions};
const WIDTH: u32 = 24;
const HEIGHT: u32 = 4;
const ROW_BYTES: usize = 3;

/// The filters the proof reads a stencil through, the ones the row cursor
/// opens a band at a time. `Reversed` carries the CCITT `/K`, and the same
/// variant name is read twice, once with each `/Decode` polarity.
enum StencilKind {
    /// No filter: 1-bit samples packed MSB first.
    Raw,
    /// [FlateDecode] over the raw samples.
    Flate,
    /// [FlateDecode] over PNG-predictor rows of the raw samples.
    FlatePredictor,
    /// [CCITTFaxDecode]; the value is `/K` (0 pure 1-D Group 3, -1 Group 4).
    Ccitt(i64),
}

impl StencilKind {
    fn name(&self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Flate => "flate",
            Self::FlatePredictor => "flate predictor",
            Self::Ccitt(0) => "ccitt group 3",
            Self::Ccitt(-1) => "ccitt group 4",
            Self::Ccitt(_) => "ccitt",
        }
    }

    fn coded(&self) -> bool {
        !matches!(self, Self::Raw | Self::Flate | Self::FlatePredictor)
    }
}

fn picture() -> Vec<Vec<bool>> {
    // A width that pads into whole bytes and a small height: every row
    // boundary, byte boundary and row group the cursor serves is in it.
    // Each row is white→black run pairs (the run lengths small enough that
    // Group 3 codes them with terminating codes) and ends black, and each
    // row differs from the one before it.
    const RUNS: [[(usize, usize); 3]; 4] = [
        [(3, 6), (3, 6), (3, 3)],
        [(2, 6), (4, 6), (3, 3)],
        [(5, 6), (1, 5), (1, 6)],
        [(6, 4), (2, 6), (3, 3)],
    ];
    (0..HEIGHT as usize)
        .map(|y| {
            let mut row = Vec::new();
            for &(white, black) in &RUNS[y] {
                row.extend(std::iter::repeat(false).take(white));
                row.extend(std::iter::repeat(true).take(black));
            }
            assert_eq!(row.len(), WIDTH as usize);
            row
        })
        .collect()
}

/// The stencil stream's dictionary and data: raw sample bits, their Flate,
/// their PNG-predicted Flate, or a CCITT codestream over the picture.
/// `reversed` puts `/Decode [1 0]` on the mask.
fn stencil_stream(kind: &StencilKind, reversed: bool) -> (String, Vec<u8>) {
    let decode = if reversed { " /Decode [1 0]" } else { "" };
    // No `<<`: the PDF builders around it open and close the dictionary.
    let words = |filter: &str| {
        format!(
            "/Type /XObject /Subtype /Image /ImageMask true /Width {WIDTH} \
             /Height {HEIGHT} /BitsPerComponent 1 {filter}{decode} ",
        )
    };
    let raw = packed_bits(&picture(), false);
    match kind {
        StencilKind::Raw => (words(""), raw),
        StencilKind::Flate => (words("/Filter /FlateDecode "), flate_of(&raw)),
        StencilKind::FlatePredictor => {
            // One PNG-predictor row a source row: a filter tag of 0 (none)
            // and the row's bytes. `/Columns` and `/BitsPerComponent` size
            // the predictor's rows.
            let mut predicted = Vec::new();
            for row in raw.chunks(ROW_BYTES) {
                predicted.push(0);
                predicted.extend_from_slice(row);
            }
            (
                words(
                    "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Colors 1 \
                     /BitsPerComponent 1 /Columns 24 >> ",
                ),
                flate_of(&predicted),
            )
        },
        StencilKind::Ccitt(k) => {
            let data = if *k == -1 { group4(&picture()) } else { group3_picture() };
            (
                words(&format!(
                    "/Filter /CCITTFaxDecode /DecodeParms << /K {k} /Columns {WIDTH} \
                     /Rows {HEIGHT} >> "
                )),
                data,
            )
        },
    }
}

fn flate_of(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("compresses");
    encoder.finish().expect("finishes")
}

/// The same picture as a pure 1-D Group 3 stream, each line coded by its
/// white→black runs.
fn group3_picture() -> Vec<u8> {
    let lines: Vec<G3Line> = picture()
        .iter()
        .map(|row| {
            let mut runs: Vec<usize> = Vec::new();
            let mut black = row[0];
            let mut run = 0;
            for &pixel in row {
                if pixel != black {
                    runs.push(run);
                    black = !black;
                    run = 0;
                }
                run += 1;
            }
            runs.push(run);
            G3Line::Runs(runs)
        })
        .collect();
    group3(&lines, false)
}

/// The stencil's sample bits as the whole decode of the same coded stream
/// carried them: the samples the encoding spells (`packed_bits`, returned
/// by stripping the encoding) for the no-codec filters, and the fax cursor
/// the whole path pulled rows through for CCITT — the decoder writes `1`
/// where an inked pixel sits, one complement from the sample on the page
/// (`/BlackIs1` composes with it, and no fixture sets it).
fn decoded_samples(kind: &StencilKind, coded: &[u8]) -> Vec<u8> {
    match kind {
        StencilKind::Raw | StencilKind::Flate | StencilKind::FlatePredictor => {
            let _ = coded;
            packed_bits(&picture(), false)
        },
        StencilKind::Ccitt(k) => {
            let params = CcittParams {
                k: *k,
                columns: WIDTH,
                rows: Some(HEIGHT),
                ..Default::default()
            };
            let mut engine = FaxRowEngine::new(&params).expect("the params decode");            let mut feed = BitFeed::new(Box::new(std::io::Cursor::new(coded.to_vec())));
            let mut packed = Vec::new();
            let mut row = vec![0u8; ROW_BYTES];
            while engine.pull_row(&mut feed, Some(HEIGHT), &mut row).expect("the cursor decodes") {
                let samples: Vec<u8> = row.iter().map(|byte| !*byte).collect();
                packed.extend_from_slice(&samples);
            }
            packed
        },
    }
}

/// The luma row the mask's mapping reads: a set sample bit is `one` (255
/// through a reversed `/Decode`, 0 otherwise), a clear bit the other.
fn luma_row_of(samples: &[u8], y: u32, reversed: bool) -> Vec<u8> {
    let one = if reversed { 255 } else { 0 };
    (0..WIDTH as usize)
        .map(|x| {
            let bit = (samples[y as usize * ROW_BYTES + x / 8] >> (7 - (x % 8))) & 1;
            if bit == 1 {
                one
            } else {
                255 - one
            }
        })
        .collect()
}

/// A one-page document whose stencil `6` is the object `dict` describes, or
/// an image masked by it through `/Mask` when `masked`.
fn stencil_pdf(dict: &str, data: Vec<u8>, masked: bool) -> Vec<u8> {
    let (name, content) = if masked {
        (
            "Im",
            "0.4 0.5 0.6 rg 0 0 200 200 re f\n \
             q 120 0 0 90 10 100 cm /Im Do Q\nq 70 70 -50 50 120 20 cm /Im Do Q\n",
        )
    } else {
        (
            "St",
            "0.4 0.5 0.6 rg q 120 0 0 90 10 100 cm /St Do Q\n \
             q 70 70 -50 50 120 20 cm /St Do Q\n",
        )
    };
    let objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] \
             /Resources << /XObject << /{name} 5 0 R >> >> /Contents 4 0 R >>\nendobj\n"
        )
        .into_bytes(),
        stream_object(4, "", content.as_bytes()),
        format!("5 0 obj\n<< {dict}/Length {} >>\nstream\n", data.len()).into_bytes(),
        data,
        b"\nendstream\nendobj\n".to_vec(),
    ];
    pdf_from_objects(&objects)
}

/// The rows `open_stencil` serves are the rows the whole decode's samples
/// carried: each filter's luma, under each polarity, compared row by row
/// against the oracle. The rows are internal values a battery cannot see,
/// so only a row-by-row hold can show the cursor and the oracle agree.
#[test]
fn the_stencil_rows_the_cursor_reads_are_the_whole_decode() {
    let kinds = [
        StencilKind::Raw,
        StencilKind::Flate,
        StencilKind::FlatePredictor,
        StencilKind::Ccitt(0),
        StencilKind::Ccitt(-1),
    ];
    for kind in kinds {
        for reversed in [false, true] {
            let (dict, data) = stencil_stream(&kind, reversed);
            let doc = PdfDocument::from_bytes(stencil_pdf(&dict, data.clone(), false))
                .expect("fixture parses");
            let xobject = doc.load_object(ObjectRef::new(5, 0)).expect("the mask object");
            let mut mask = ImageRows::open_stencil(&doc, &xobject).expect("the cursor opens");
            let oracle = decoded_samples(&kind, &data);
            for y in 0..HEIGHT {
                let mut actual = vec![0u8; WIDTH as usize];
                mask.read(&doc, &mut actual).expect("a row");
                assert_eq!(
                    actual,
                    luma_row_of(&oracle, y, reversed),
                    "{} under /Decode {} row {y}",
                    kind.name(),
                    if reversed { "[1 0]" } else { "default" }
                );
            }
            let mut over = vec![0u8; WIDTH as usize];
            mask.read(&doc, &mut over).expect_err("no rows past the stencil");
        }
    }
}

/// A stencil rendered in bands paints the pixels one whole-page render
/// does, for every filter the row cursor opens, at every forced band
/// height. No Dart battery sees it: the band height is an internal knob no
/// API sets.
#[test]
fn stencil_bands_equal_the_whole_canvas_on_every_filter() {
    let kinds = [
        StencilKind::Raw,
        StencilKind::Flate,
        StencilKind::FlatePredictor,
        StencilKind::Ccitt(0),
        StencilKind::Ccitt(-1),
    ];
    for kind in kinds {
        for reversed in [false, true] {
            let (dict, data) = stencil_stream(&kind, reversed);
            let mut fixture = Fixture::new(
                kind.name(),
                "0.4 0.5 0.6 rg q 120 0 0 90 10 100 cm /St Do Q\n \
                 q 70 70 -50 50 120 20 cm /St Do Q\n",
            );
            fixture.resources = "/XObject << /St 5 0 R >>".to_string();
            fixture.extra = vec![stream_object(5, &dict, &data)];
            prove_bands(&fixture);
        }
    }
}

/// An image whose `/Mask` is a coded stencil paints the pixels the same
/// image with the mask's whole-decode samples written raw paints: every
/// coded filter next to its raw twin, at 72 dpi. No Dart battery sees it:
/// the two legs differ only in how the mask's rows reached the engine.
#[test]
fn a_masked_image_renders_whatever_coded_its_stencil() {
    let kinds = [
        StencilKind::Raw,
        StencilKind::Flate,
        StencilKind::FlatePredictor,
        StencilKind::Ccitt(0),
        StencilKind::Ccitt(-1),
    ];
    for kind in kinds {
        for reversed in [false, true] {
            // The coded leg: the mask as the fixture carries it.
            let (coded_dict, coded_data) = stencil_stream(&kind, reversed);
            let coded = PdfDocument::from_bytes(stencil_pdf(&coded_dict, coded_data.clone(), true))
                .expect("fixture parses");
            // The twin: the same samples written raw, unfiltered.
            let samples = decoded_samples(&kind, &coded_data);
            let plain_dict = format!(
                "/Type /XObject /Subtype /Image /ImageMask true /Width {WIDTH} \
                 /Height {HEIGHT} /BitsPerComponent 1{} ",
                if reversed { " /Decode [1 0]" } else { "" }
            );
            let plain =
                PdfDocument::from_bytes(stencil_pdf(&plain_dict, samples, true)).expect("fixture parses");
            let plain_rows = render_page(&plain);
            let coded_rows = render_page(&coded);
            assert_eq!(
                coded_rows, plain_rows,
                "the {} coded mask paints its raw twin's pixels (reversed {reversed})",
                kind.name()
            );
        }
    }
}

/// A stencil larger than its device footprint is reduced to the footprint
/// through the resampler, and a band holds the reduced rows. The stencil here
/// is 4x its footprint at 72 dpi and 8x at 36 dpi, and fits its footprint at
/// 300 dpi; the bands equal the whole page at every dpi and band height.
#[test]
fn a_stencil_larger_than_its_footprint_bands_equal_the_whole_canvas() {
    const BIG_W: u32 = 480;
    const BIG_H: u32 = 360;
    let row_bytes = (BIG_W as usize).div_ceil(8);
    let mut data = vec![0u8; row_bytes * BIG_H as usize];
    for y in 0..BIG_H as usize {
        for x in 0..BIG_W as usize {
            if (x / 7 + y / 5) % 2 == 0 {
                data[y * row_bytes + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    let dict = format!(
        "/Type /XObject /Subtype /Image /ImageMask true /Width {BIG_W} /Height {BIG_H} \
         /BitsPerComponent 1"
    );
    let mut fixture = Fixture::new("stencil_reduced", "0.4 0.5 0.6 rg q 120 0 0 90 10 100 cm /Big Do Q\n");
    fixture.resources = "/XObject << /Big 5 0 R >>".to_string();
    fixture.extra = vec![stream_object(5, &dict, &data)];
    prove_bands(&fixture);
}

fn render_page(doc: &PdfDocument) -> Vec<u8> {
    let mut renderer = PageRenderer::new(RenderOptions::with_dpi(72).with_transparent_background());
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
}

