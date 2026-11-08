//! What reduceImages writes: the rows of each image go through the row
//! cursor, the row resampler and a row encoder, and the pixels that come out
//! are the ones the whole-image decoder and `resample_continuous` gave before.
//!
//! A Dart battery sees the sizes and the report but not the decoder the rows
//! replace, so only here can the two be compared.

use super::band_proof::{bilevel, detailed_rgb, packed_bits};
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::decoders::CcittParams;
use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::extractors::ccitt_bilevel::decompress_ccitt;
use crate::extractors::images::{extract_image_from_xobject, ImageData, PixelFormat};
use crate::host::images::execute::reduce_images;
use crate::host::images::policy::{Chroma, Policy};
use crate::host::images::report::{Action, Report};
use crate::host::images::resample::{resample_bilevel, resample_continuous};
use crate::object::Object;
use std::io::Write;

const W: u32 = 256;
const H: u32 = 192;

/// What a case's samples are for the reference.
#[derive(Clone, Copy, PartialEq)]
enum Samples {
    Gray,
    Rgb,
    Cmyk,
    Bilevel,
}

/// One image of the fixture, drawn `pt` points wide and high: 288 pixels an
/// inch for `(64, 48)`, so the policy's 72 brings it to the size in points.
struct Case {
    name: &'static str,
    dict: String,
    data: Vec<u8>,
    pt: (u32, u32),
    samples: Samples,
    /// The soft mask this image names, its object written after it.
    smask: Option<(String, Vec<u8>)>,
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(data).expect("compresses");
    z.finish().expect("finishes")
}

fn cases() -> Vec<Case> {
    let rgb = detailed_rgb(W, H);
    let case = |name, dict: &str, data, pt, samples| Case {
        name,
        dict: dict.to_string(),
        data,
        pt,
        samples,
        smask: None,
    };
    let picture = bilevel(W, H);
    let palette: String = (0..16u8)
        .flat_map(|i| [i * 16, 255 - i * 16, i * 8])
        .map(|b| format!("{b:02x}"))
        .collect();
    let jpeg = {
        let mut bytes = Vec::new();
        jpeg_encoder::Encoder::new(&mut bytes, 90)
            .encode(&rgb, W as u16, H as u16, jpeg_encoder::ColorType::Rgb)
            .expect("encodes");
        bytes
    };
    let dims = format!("/Width {W} /Height {H}");
    let mut smasked = case(
        "rgb with a soft mask",
        &format!("{dims} /ColorSpace /DeviceRGB /BitsPerComponent 8"),
        rgb.clone(),
        (64, 48),
        Samples::Rgb,
    );
    smasked.smask = Some((
        "/Width 128 /Height 96 /ColorSpace /DeviceGray /BitsPerComponent 8".to_string(),
        (0..128 * 96u32).map(|i| (i * 7 % 251) as u8).collect(),
    ));
    vec![
        case("rgb", &format!("{dims} /ColorSpace /DeviceRGB /BitsPerComponent 8"), rgb.clone(), (64, 48), Samples::Rgb),
        case(
            "gray, flate",
            &format!("{dims} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode"),
            flate(&rgb.iter().step_by(3).copied().collect::<Vec<_>>()),
            (64, 48),
            Samples::Gray,
        ),
        case(
            "cmyk",
            &format!("{dims} /ColorSpace /DeviceCMYK /BitsPerComponent 8"),
            rgb.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], p[0] ^ p[1]]).collect(),
            (64, 48),
            Samples::Cmyk,
        ),
        case(
            "indexed, four bits",
            &format!("{dims} /ColorSpace [/Indexed /DeviceRGB 15 <{palette}>] /BitsPerComponent 4"),
            (0..W / 2 * H).map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8).collect(),
            (64, 48),
            Samples::Rgb,
        ),
        case(
            "bilevel, one bit",
            &format!("{dims} /ColorSpace /DeviceGray /BitsPerComponent 1"),
            packed_bits(&picture, false),
            (64, 48),
            Samples::Bilevel,
        ),
        case(
            "stencil",
            &format!("{dims} /ImageMask true /BitsPerComponent 1"),
            packed_bits(&picture, false),
            (64, 48),
            Samples::Bilevel,
        ),
        case(
            "jpeg, a quarter",
            &format!("{dims} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"),
            jpeg.clone(),
            (64, 48),
            Samples::Rgb,
        ),
        case(
            "jpeg, three eighths",
            &format!("{dims} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"),
            jpeg,
            (84, 63),
            Samples::Rgb,
        ),
        smasked,
    ]
}

/// The PDF: one page that draws every case; the case at index `i` is object
/// `first_object(i)`.
fn pdf(cases: &[Case]) -> (Vec<u8>, Vec<u32>) {
    let mut objects = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
    ];
    let mut content = String::new();
    let mut names = String::new();
    let mut extra = Vec::new();
    let mut ids = Vec::new();
    let mut number = 5;
    for (i, case) in cases.iter().enumerate() {
        content.push_str(&format!(
            "q {} 0 0 {} {} {} cm /Im{i} Do Q\n",
            case.pt.0,
            case.pt.1,
            10 + i % 4 * 100,
            10 + i / 4 * 100
        ));
        names.push_str(&format!("/Im{i} {number} 0 R "));
        ids.push(number);
        let smask = case.smask.as_ref().map(|_| format!("/SMask {} 0 R", number + 1)).unwrap_or_default();
        extra.push(stream_object(
            number,
            &format!("/Type /XObject /Subtype /Image {} {smask}", case.dict),
            &case.data,
        ));
        number += 1;
        if let Some((dict, data)) = &case.smask {
            extra.push(stream_object(number, &format!("/Type /XObject /Subtype /Image {dict}"), data));
            number += 1;
        }
    }
    objects.push(
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 600] /Contents 4 0 R \
             /Resources << /XObject << {names}>> >> >>\nendobj\n"
        )
        .into_bytes(),
    );
    objects.push(stream_object(4, "", content.as_bytes()));
    objects.extend(extra);
    (pdf_from_objects(&objects), ids)
}

fn policy(lossy: bool) -> Policy {
    Policy {
        color_ppi: Some(72.0),
        gray_ppi: Some(72.0),
        mono_ppi: Some(72.0),
        downsample_threshold: 1.5,
        jpeg_quality: 90,
        allow_lossy: lossy,
        convert_cmyk_to_rgb: false,
        min_pixels: 32,
        min_savings: 0.0,
        chroma: Chroma::Full,
        recompress_jpeg: false,
    }
}

trait Staged {
    /// The replacement the reducer staged for object `id`, if any.
    fn staged_object(&mut self, id: u32) -> Option<Object>;
}

impl Staged for DocumentEditor {
    fn staged_object(&mut self, id: u32) -> Option<Object> {
        self.modified_objects_mut().get(&id).cloned()
    }
}

fn reduced(cases: &[Case], policy: &Policy) -> (DocumentEditor, Report, Vec<u32>) {
    let (bytes, ids) = pdf(cases);
    let doc = PdfDocument::from_bytes(bytes).expect("fixture parses");
    let mut editor = DocumentEditor::from_document(doc).expect("editor");
    let report = reduce_images(&mut editor, policy, &CancelCheck::none()).expect("reduces");
    (editor, report, ids)
}

/// The samples the whole-image decoder gives for the stored image: what the
/// reducer decoded before it read rows.
fn whole_samples(doc: &PdfDocument, id: u32, samples: Samples) -> (u32, u32, Vec<u8>) {
    let object = doc.load_object(crate::object::ObjectRef::new(id, 0)).expect("object");
    let image = extract_image_from_xobject(Some(doc), &object, None, None).expect("decodes");
    let (w, h) = (image.width(), image.height());
    let data = match samples {
        Samples::Cmyk => match image.data() {
            ImageData::Raw { pixels, format: PixelFormat::CMYK } => pixels.clone(),
            _ => panic!("CMYK did not decode to CMYK"),
        },
        Samples::Rgb => image.to_dynamic_image().expect("image").to_rgb8().into_raw(),
        Samples::Gray | Samples::Bilevel => image.to_dynamic_image().expect("image").to_luma8().into_raw(),
    };
    (w, h, data)
}

/// The size and samples of a staged image, decoded the way a reader does.
fn written_samples(doc: &PdfDocument, object: &Object, samples: Samples) -> (u32, u32, Vec<u8>) {
    let Object::Stream { dict, body } = object else {
        panic!("a staged image is a stream");
    };
    let dimension = |key: &str| dict.get(key).and_then(|o| o.as_integer()).expect(key) as u32;
    let (w, h) = (dimension("Width"), dimension("Height"));
    match samples {
        Samples::Bilevel => {
            let params = CcittParams { columns: w, rows: Some(h), k: -1, ..Default::default() };
            let packed = decompress_ccitt(&body.read_whole_raw().expect("bytes"), &params).expect("ccitt");
            let row_bytes = (w as usize).div_ceil(8);
            let data = (0..h as usize)
                .flat_map(|y| (0..w as usize).map(move |x| (y, x)))
                .map(|(y, x)| if (packed[y * row_bytes + x / 8] >> (7 - x % 8)) & 1 == 1 { 0 } else { 255 })
                .collect();
            (w, h, data)
        },
        Samples::Cmyk => (w, h, object.decode_stream_data().expect("samples").to_vec()),
        Samples::Rgb | Samples::Gray => {
            if dict.get("Filter").and_then(|f| f.as_name()) == Some("DCTDecode") {
                let image = extract_image_from_xobject(Some(doc), object, None, None).expect("decodes");
                let dynamic = image.to_dynamic_image().expect("image");
                let data = if samples == Samples::Gray { dynamic.to_luma8().into_raw() } else { dynamic.to_rgb8().into_raw() };
                (w, h, data)
            } else {
                (w, h, object.decode_stream_data().expect("samples").to_vec())
            }
        },
    }
}

fn rmse(a: &[u8], b: &[u8]) -> f64 {
    assert_eq!(a.len(), b.len());
    (a.iter().zip(b).map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2)).sum::<f64>() / a.len() as f64).sqrt()
}

/// The samples the previous reducer made of the image at its target size.
fn reference(doc: &PdfDocument, id: u32, case: &Case, target: (u32, u32)) -> Vec<u8> {
    let samples = case.samples;
    // The whole-image decoder has no path for a stencil: it paints, so the
    // picture's black is the sample 0.
    let (w, h, whole) = if case.dict.contains("/ImageMask") {
        let picture = bilevel(W, H);
        (W, H, picture.iter().flatten().map(|&black| if black { 0 } else { 255 }).collect())
    } else {
        whole_samples(doc, id, samples)
    };
    match samples {
        Samples::Bilevel => resample_bilevel(&whole, w, h, target.0, target.1),
        Samples::Gray => resample_continuous(&whole, w, h, 1, target.0, target.1),
        Samples::Rgb => resample_continuous(&whole, w, h, 3, target.0, target.1),
        Samples::Cmyk => resample_continuous(&whole, w, h, 4, target.0, target.1),
    }
    .expect("resamples")
}

/// The most RMSE a case may have against the previous reducer: the row
/// resampler rounds as the whole-image one does, so a raw-sample image agrees
/// to the last level; a JPEG decoded at a reduced scale differs by what the
/// inverse DCT at that scale makes of it against a full decode resampled.
fn allowed(case: &Case) -> f64 {
    if case.name.starts_with("jpeg") {
        6.0
    } else {
        0.5
    }
}

/// Every image is rewritten at the size of its placement, lossless, and its
/// pixels are those of the previous reducer. The table is printed with
/// `--no-capture`.
#[test]
fn rows_give_the_pixels_the_whole_image_pipeline_gave() {
    let cases = cases();
    let (mut editor, report, ids) = reduced(&cases, &policy(false));
    assert_eq!(report.images.len(), cases.len());
    println!("{:<24} {:>9} {:>8}", "case", "size", "RMSE");
    for (i, case) in cases.iter().enumerate() {
        let outcome = &report.images[i];
        assert!(
            matches!(outcome.action, Action::Downsampled),
            "{}: {:?} {:?}",
            case.name,
            outcome.action,
            outcome.keep_reason
        );
        let staged = editor.staged_object(ids[i]).unwrap_or_else(|| panic!("{} is staged", case.name));
        let (w, h, written) = written_samples(editor.source(), &staged, case.samples);
        assert_eq!((w, h), case.pt, "{}: the size of its placement", case.name);
        let expected = reference(editor.source(), ids[i], case, (w, h));
        let error = rmse(&written, &expected);
        println!("{:<24} {:>4}x{:<4} {:>8.3}", case.name, w, h, error);
        assert!(error <= allowed(case), "{}: RMSE {error}", case.name);
    }
}

/// A soft mask is read at its own size and written at the parent's.
#[test]
fn a_soft_mask_is_resampled_to_the_new_size_of_its_parent() {
    let cases = cases();
    let (mut editor, report, ids) = reduced(&cases, &policy(false));
    let index = cases.iter().position(|c| c.smask.is_some()).expect("a masked case");
    assert!(matches!(report.images[index].action, Action::Downsampled));
    let mask = editor.staged_object(ids[index] + 1).expect("the mask is staged");
    let (w, h, written) = written_samples(editor.source(), &mask, Samples::Gray);
    assert_eq!((w, h), cases[index].pt);
    let (mw, mh, whole) = whole_samples(editor.source(), ids[index] + 1, Samples::Gray);
    let expected = resample_continuous(&whole, mw, mh, 1, w, h).expect("resamples");
    let error = rmse(&written, &expected);
    println!("{:<24} {:>4}x{:<4} {:>8.3}", "soft mask", w, h, error);
    assert!(error <= 0.5, "RMSE {error}");
}

/// The lossy policy writes JPEG rows: the output decodes at the new size and
/// is close to the lossless result.
#[test]
fn jpeg_output_decodes_at_the_new_size_and_is_close_to_the_lossless_rows() {
    let cases = cases();
    let (mut lossy, _, ids) = reduced(&cases, &policy(true));
    let (mut lossless, _, _) = reduced(&cases, &policy(false));
    for (i, case) in cases.iter().enumerate() {
        if !matches!(case.samples, Samples::Rgb | Samples::Gray) || case.name.starts_with("indexed") {
            continue;
        }
        let staged = lossy.staged_object(ids[i]).unwrap_or_else(|| panic!("{} is staged", case.name));
        let Object::Stream { dict, .. } = &staged else { panic!("a stream") };
        assert_eq!(dict.get("Filter").and_then(|f| f.as_name()), Some("DCTDecode"), "{}", case.name);
        let (w, h, written) = written_samples(lossy.source(), &staged, case.samples);
        assert_eq!((w, h), case.pt, "{}", case.name);
        let exact = lossless.staged_object(ids[i]).expect("staged");
        let (_, _, rows) = written_samples(lossless.source(), &exact, case.samples);
        let error = rmse(&written, &rows);
        println!("{:<24} jpeg vs flate {:>8.3}", case.name, error);
        assert!(error <= 14.0, "{}: RMSE {error}", case.name);
    }
}

/// The first-run presets: a stored JPEG that needs no new size is kept
/// unless `recompress_jpeg` is on, and then it is rewritten.
#[test]
fn a_jpeg_that_is_not_downsampled_is_kept_unless_recompression_is_on() {
    let mut cases = cases();
    cases.retain(|c| c.name == "jpeg, a quarter");
    cases[0].pt = (256, 192);
    let (mut editor, report, ids) = reduced(&cases, &policy(true));
    assert!(editor.staged_object(ids[0]).is_none(), "{:?}", report.images[0].keep_reason);
    let recompress = Policy { recompress_jpeg: true, jpeg_quality: 40, ..policy(true) };
    let (mut editor, report, ids) = reduced(&cases, &recompress);
    assert!(matches!(report.images[0].action, Action::Recompressed), "{:?}", report.images[0].keep_reason);
    assert!(editor.staged_object(ids[0]).is_some());
}
