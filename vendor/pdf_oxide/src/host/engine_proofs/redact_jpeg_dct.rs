//! The DCT-domain JPEG copy, proved by the pixels jpeg-decoder reads from it.
//! No Dart battery observes these: the coefficient reader is internal to the
//! redaction and reaches no Dart value, so only the decoded pixels of the saved
//! file show whether a block is right.
//!
//! The copy's coefficients are the source's, so a copy with no covered block
//! decodes to the source's exact pixels: any coefficient the reader gets wrong
//! shows as a pixel difference. A covered copy is exact outside the blocks it
//! replaced (and the samples that upsampling reads from them), and flat inside.
//!
//! The fixtures are libjpeg-turbo 3.2.0 output, so the encoder that wrote them
//! shares nothing with the reader. Each file's command is in `fixtures/`:
//!   gray_base.jpg       cjpeg -quality 90 -sample 1x1 g.pgm
//!   gray_prog.jpg       cjpeg -quality 90 -progressive g.pgm
//!   gray_scans.jpg      jpegtran -optimize -scans scans_gray.txt gray_base.jpg
//!   rgb444.jpg          cjpeg -quality 90 -sample 1x1 c.ppm
//!   rgb444_q100.jpg     cjpeg -quality 100 -sample 1x1 c.ppm (DC categories 9 to 11)
//!   rgb420.jpg          cjpeg -quality 90 -sample 2x2 c.ppm
//!   rgb420_prog.jpg     cjpeg -quality 90 -progressive -sample 2x2 c.ppm
//!   rgb420_scans.jpg    jpegtran -optimize -scans scans_rgb420.txt rgb420.jpg
//!   rgb420_restart.jpg  jpegtran -restart 3 rgb420.jpg
//!   rgb420_prog_restart.jpg  jpegtran -progressive -restart 2 rgb420.jpg
//!   cmyk.jpg            magick c.ppm -colorspace CMYK -quality 90 -sampling-factor 2x2 cmyk.jpg
//!                       (Adobe transform 2: YCCK)
//!   arith.jpg           jpegtran -arithmetic rgb420.jpg (SOF9, refused)
//! `c.ppm` is 64x48 RGB with `((x*4+y*2)%256, (x*y)%256, (y*5+x)%256)` per
//! pixel; `g.pgm` is 64x48 gray with `(x*3+y*5+(x*y)%7*11)%256`.
//! The scan scripts are in `fixtures/scans_*.txt`.

use std::io::Cursor;

use bytes::Bytes;
use jpeg_encoder::{ColorType, Encoder, SamplingFactor};

use crate::cancel::CancelCheck;
use crate::extractors::images::{cmyk_pixel_to_rgb, decode_cmyk_jpeg_to_rgb};
use crate::host::images::dct::{Cover, Jpeg};
use crate::object::StreamBody;

fn fixture(name: &str) -> Vec<u8> {
    let bytes: &[u8] = match name {
        "gray_base.jpg" => include_bytes!("fixtures/gray_base.jpg"),
        "gray_prog.jpg" => include_bytes!("fixtures/gray_prog.jpg"),
        "gray_scans.jpg" => include_bytes!("fixtures/gray_scans.jpg"),
        "rgb444.jpg" => include_bytes!("fixtures/rgb444.jpg"),
        "rgb444_q100.jpg" => include_bytes!("fixtures/rgb444_q100.jpg"),
        "rgb420.jpg" => include_bytes!("fixtures/rgb420.jpg"),
        "rgb420_prog.jpg" => include_bytes!("fixtures/rgb420_prog.jpg"),
        "rgb420_scans.jpg" => include_bytes!("fixtures/rgb420_scans.jpg"),
        "rgb420_restart.jpg" => include_bytes!("fixtures/rgb420_restart.jpg"),
        "rgb420_prog_restart.jpg" => include_bytes!("fixtures/rgb420_prog_restart.jpg"),
        "cmyk.jpg" => include_bytes!("fixtures/cmyk.jpg"),
        "arith.jpg" => include_bytes!("fixtures/arith.jpg"),
        _ => panic!("no fixture named {name}"),
    };
    bytes.to_vec()
}

/// Pixels with structure in every channel, so a wrong coefficient shows.
fn source_pixels(width: usize, height: usize, channels: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * channels);
    for y in 0..height {
        for x in 0..width {
            for c in 0..channels {
                out.push(((x * 5 + y * 3 + c * 70 + (x * y) % 11 * 9) % 256) as u8);
            }
        }
    }
    out
}

/// A CMYK JPEG written by jpeg-encoder: Adobe transform 0, the plain CMYK case
/// the fixtures do not carry.
fn encode_cmyk() -> Vec<u8> {
    let data = source_pixels(64, 48, 4);
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, 90);
    encoder.set_sampling_factor(SamplingFactor::R_4_2_0);
    encoder.encode(&data, 64, 48, ColorType::Cmyk).expect("the CMYK fixture encodes");
    out
}

/// The decoded samples of a JPEG, as jpeg-decoder reads them.
fn decode(bytes: &[u8]) -> (usize, usize, Vec<u8>) {
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    let pixels = decoder.decode().expect("the JPEG decodes");
    let info = decoder.info().expect("the JPEG has info");
    (usize::from(info.width), usize::from(info.height), pixels)
}

/// The copy of `source`. Each `(rect, fill)` covers the pixel rect `x0 x1 y0 y1`
/// in decoded samples; a block takes the fill of the last entry that meets it,
/// the rule `redact_copy` applies with the list in drawing order.
fn copy(source: &[u8], regions: &[([usize; 4], Vec<u32>)]) -> Vec<u8> {
    let body = StreamBody::Inline(Bytes::from(source.to_vec()));
    let jpeg = Jpeg::read(&body).expect("the source reads");
    let (x0, x1, y0, y1) = regions.iter().fold((usize::MAX, 0, usize::MAX, 0), |a, (r, _)| {
        (a.0.min(r[0]), a.1.max(r[1]), a.2.min(r[2]), a.3.max(r[3]))
    });
    let fill_at = |bx0: usize, bx1: usize, by0: usize, by1: usize| {
        regions.iter().rev().find_map(|(r, fill)| {
            let meets = r[0] < bx1 && r[1] > bx0 && r[2] < by1 && r[3] > by0;
            meets.then(|| fill.clone())
        })
    };
    let cover = Cover {
        x0: if regions.is_empty() { 0 } else { x0 },
        x1: if regions.is_empty() { 0 } else { x1 },
        y0: if regions.is_empty() { 0 } else { y0 },
        y1: if regions.is_empty() { 0 } else { y1 },
        fill_at: &fill_at,
    };
    let mut out = Vec::new();
    jpeg.write(&body, &cover, &CancelCheck::none(), &mut out)
        .expect("the copy writes");
    out
}

/// The RGB the product shows for straight CMYK `ink`: the extractor's conversion.
fn cmyk_rgb(ink: &[u32; 4]) -> Vec<u32> {
    let [r, g, b] = cmyk_pixel_to_rgb(ink[0] as u8, ink[1] as u8, ink[2] as u8, ink[3] as u8);
    vec![u32::from(r), u32::from(g), u32::from(b)]
}

/// The pixels the product reads from a JPEG: the extractor's CMYK decode for a
/// CMYK file, which undoes jpeg-decoder's inversion, and jpeg-decoder's own for
/// the rest.
fn product_rgb(bytes: &[u8]) -> (usize, usize, Vec<u8>) {
    let (w, h, _) = decode(bytes);
    (w, h, decode_cmyk_jpeg_to_rgb(bytes).expect("the CMYK JPEG decodes through the product"))
}

/// A copy of `source` with `fill` over the pixel rect `x0..x1`, `y0..y1`, read by
/// `read`, holds `expect` (one value per channel of `read`) inside the rect, and
/// the source's own read outside the replaced blocks.
fn check_covered(
    name: &str,
    source: &[u8],
    subsample: usize,
    fill: &[u32],
    expect: &[u32],
    read: fn(&[u8]) -> (usize, usize, Vec<u8>),
) {
    let comps = expect.len();
    let (x0, x1, y0, y1) = (9usize, 27usize, 5usize, 14usize);
    let (w, h, want) = read(source);
    let (_, _, got) = read(&copy(source, &[([x0, x1, y0, y1], fill.to_vec())]));
    // The replaced blocks, in luma pixels: the covered rect widened to its
    // blocks of the coarsest grid, plus the two samples upsampling reads.
    let grid = 8 * subsample;
    let changed = |lo: usize, hi: usize, size: usize| {
        let a = (lo / grid) * grid;
        let b = (hi.div_ceil(grid) * grid).min(size);
        (a.saturating_sub(2), (b + 2).min(size))
    };
    let (cx0, cx1) = changed(x0, x1, w);
    let (cy0, cy1) = changed(y0, y1, h);
    let mut worst_inside = 0i32;
    for y in 0..h {
        for x in 0..w {
            for c in 0..comps {
                let at = (y * w + x) * comps + c;
                let covered = x >= x0 && x < x1 && y >= y0 && y < y1;
                if covered {
                    worst_inside = worst_inside.max((i32::from(got[at]) - expect[c] as i32).abs());
                } else if !(x >= cx0 && x < cx1 && y >= cy0 && y < cy1) {
                    assert_eq!(got[at], want[at], "{name}: pixel ({x}, {y}) channel {c} outside the replaced blocks");
                }
            }
        }
    }
    assert!(worst_inside <= 2, "{name}: a covered sample is {worst_inside} away from the fill");
    eprintln!("redact_jpeg_dct: {name}: covered samples at most {worst_inside} from the fill");
}

/// Every fixture with no covered block decodes to its source's exact pixels.
#[test]
fn identity_copy_decodes_to_the_source_pixels() {
    let mut cases: Vec<(String, Vec<u8>)> = [
        "gray_base.jpg",
        "gray_prog.jpg",
        "gray_scans.jpg",
        "rgb444.jpg",
        "rgb420.jpg",
        "rgb420_prog.jpg",
        "rgb420_scans.jpg",
        "rgb420_restart.jpg",
        "rgb420_prog_restart.jpg",
        "cmyk.jpg",
    ]
    .iter()
    .map(|n| (n.to_string(), fixture(n)))
    .collect();
    cases.push(("cmyk_transform_0 (jpeg-encoder)".to_string(), encode_cmyk()));
    for (name, source) in cases {
        let (w, h, want) = decode(&source);
        let (cw, ch, got) = decode(&copy(&source, &[]));
        assert_eq!((cw, ch), (w, h), "{name}: the copy's size");
        assert!(got == want, "{name}: the copy decodes to other pixels than its source");
    }
}

/// A file from the crate's own JPEG writer (`encode_jpeg`, 4:4:4 and 4:2:0 at
/// quality 90, 32 by 32) copies to its source's pixels.
#[test]
fn identity_copy_of_the_crate_encoder_decodes_to_the_source_pixels() {
    use crate::host::images::encode::encode_jpeg;
    use crate::host::images::policy::Subsampling;
    const N: usize = 32;
    let rgb = source_pixels(N, N, 3);
    for (label, sub) in [("4:4:4", Subsampling::Full), ("4:2:0", Subsampling::Half)] {
        let source = encode_jpeg(&rgb, N as u32, N as u32, 3, 90, sub).expect("encodes");
        let (w, h, want) = decode(&source);
        let (cw, ch, got) = decode(&copy(&source, &[]));
        assert_eq!((cw, ch), (w, h), "{label}: the copy's size");
        if let Some(at) = got.iter().zip(&want).position(|(a, b)| a != b) {
            panic!("{label}: first copy sample {at} is {}, source {}", got[at], want[at]);
        }
    }
}

/// Covered copies of gray, RGB 4:4:4 and 4:2:0 (baseline, progressive and
/// non-interleaved refinement scans), are flat inside and exact outside. The
/// CMYK files, a YCCK one and a plain CMYK one, are read through the product and
/// hold the straight ink's RGB, which is not the RGB of its complement.
#[test]
fn covered_copy_is_flat_inside_and_exact_outside() {
    check_covered("gray_prog.jpg", &fixture("gray_prog.jpg"), 1, &[150], &[150], decode);
    check_covered("gray_scans.jpg", &fixture("gray_scans.jpg"), 1, &[150], &[150], decode);
    check_covered("rgb444.jpg", &fixture("rgb444.jpg"), 1, &[200, 30, 90], &[200, 30, 90], decode);
    check_covered("rgb444_q100.jpg", &fixture("rgb444_q100.jpg"), 1, &[200, 30, 90], &[200, 30, 90], decode);
    check_covered("rgb420_prog.jpg", &fixture("rgb420_prog.jpg"), 2, &[200, 30, 90], &[200, 30, 90], decode);
    check_covered("rgb420_scans.jpg", &fixture("rgb420_scans.jpg"), 2, &[200, 30, 90], &[200, 30, 90], decode);
    let ink = [200u32, 30, 90, 40];
    check_covered("cmyk.jpg (YCCK)", &fixture("cmyk.jpg"), 2, &ink, &cmyk_rgb(&ink), product_rgb);
    check_covered("cmyk_transform_0 (jpeg-encoder)", &encode_cmyk(), 2, &ink, &cmyk_rgb(&ink), product_rgb);
}

/// A block that two regions share takes the later region's fill, and so does a
/// block only the later region meets.
#[test]
fn a_shared_block_takes_the_last_region_in_drawing_order() {
    let source = fixture("gray_base.jpg");
    // Region A, the first drawn, ends inside block column 1; region B, drawn on
    // top, starts in it. Block column 1 (x 8..16) is shared; column 2 is B's alone.
    let (_, w, got) = {
        let (w, h, _) = decode(&source);
        let (_, _, pixels) = decode(&copy(&source, &[([8, 12, 8, 16], vec![60]), ([12, 20, 8, 16], vec![200])]));
        (h, w, pixels)
    };
    for by in [1usize] {
        for bx in [1usize, 2] {
            for y in by * 8..by * 8 + 8 {
                for x in bx * 8..bx * 8 + 8 {
                    let v = got[y * w + x];
                    assert!((i32::from(v) - 200).abs() <= 2, "block ({bx}, {by}) pixel ({x}, {y}) is {v}, not the top fill");
                }
            }
        }
    }
}

/// A block of the copy decodes constant: the replaced block has no AC left.
#[test]
fn a_flat_block_decodes_constant() {
    let source = fixture("gray_base.jpg");
    let (w, _, got) = decode(&copy(&source, &[([8, 24, 8, 16], vec![40])]));
    // The covered rect is block row 1 and block columns 1 and 2 exactly.
    for by in [1usize] {
        for bx in [1usize, 2] {
            let first = got[(by * 8) * w + bx * 8];
            for y in by * 8..by * 8 + 8 {
                for x in bx * 8..bx * 8 + 8 {
                    assert_eq!(got[y * w + x], first, "block ({bx}, {by}) is not flat");
                }
            }
        }
    }
}

/// Frames the copy does not read are refused at the reader: 12-bit precision,
/// arithmetic coding (SOF9), and a quantisation table after the first scan.
#[test]
fn refuses_what_the_copy_does_not_read() {
    let source = fixture("gray_base.jpg");
    let sof = source.windows(2).position(|w| w == [0xFF, 0xC0]).expect("a baseline frame");
    let mut twelve_bit = source.clone();
    twelve_bit[sof + 4] = 12;
    let arith = fixture("arith.jpg");
    let mut late_dqt = fixture("rgb420.jpg");
    let end = late_dqt.windows(2).rposition(|w| w == [0xFF, 0xD9]).expect("an EOI");
    let mut dqt = vec![0xFF, 0xDB, 0x00, 0x43, 0x00];
    dqt.extend(std::iter::repeat(1u8).take(64));
    late_dqt.splice(end..end, dqt);
    for (bytes, why) in [(twelve_bit, "12-bit"), (arith, "arithmetic"), (late_dqt, "quant table after the first scan")] {
        let body = StreamBody::Inline(Bytes::from(bytes));
        assert!(Jpeg::read(&body).is_err(), "{why}: the frame is read");
    }
}

/// The copy of a fixture with restart markers, one the reader stops at each
/// interval, decodes to the source's pixels.
#[test]
fn restart_intervals_resynchronise_the_reader() {
    for name in ["rgb420_restart.jpg", "rgb420_prog_restart.jpg"] {
        let source = fixture(name);
        let (w, h, want) = decode(&source);
        let (cw, ch, got) = decode(&copy(&source, &[]));
        assert_eq!((cw, ch), (w, h), "{name}: the copy's size");
        assert!(got == want, "{name}: the copy decodes to other pixels than its source");
    }
}
