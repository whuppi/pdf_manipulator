//! What a saved file holds of a redacted image: the covered pixels are the
//! fill, the rest is the source's pixels, and the source's covered bytes are
//! in no stream of the file. Each proof saves with garbage collection on and
//! off and reads the copy back through the reader.

use std::io::Write;

use super::support::{contains, open_editor_from_bytes, pdf_from_objects, stream_object};
use crate::editor::{DocumentEditor, SaveOptions};
use crate::extractors::images::extract_image_from_xobject_at;
use crate::host::images::encode::{encode_ccitt_g4, encode_jpeg};
use crate::host::images::policy::Subsampling;
use crate::object::ObjectRef;
use crate::rendering::{render_page, RenderOptions};
use crate::PdfDocument;

const W: usize = 8;
const H: usize = 8;
const RED: [f32; 3] = [1.0, 0.0, 0.0];
/// The image covers page points 50..250 on both axes; the box covers its
/// lower-left quadrant, `u` and `v` in 0..0.5, with the mark's padding.
const BOX: [f32; 4] = [50.0, 50.0, 150.0, 150.0];

fn obj(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

/// Eight by eight RGB, every pixel its own colour: a copy that is wrong in
/// any pixel shows, and no 6-byte run of it repeats.
fn source_rgb() -> Vec<u8> {
    let mut out = Vec::with_capacity(W * H * 3);
    for y in 0..H {
        for x in 0..W {
            out.extend_from_slice(&[(x * 30 + 1) as u8, (y * 30 + 2) as u8, ((x + y) * 15 + 3) as u8]);
        }
    }
    out
}

/// The bytes of pixels 1 and 2 of row 5: the covered interior of the source,
/// which must be in no stream of the saved file.
fn secret_run(rgb: &[u8]) -> Vec<u8> {
    let at = (5 * W + 1) * 3;
    rgb[at..at + 6].to_vec()
}

/// A page with one image XObject `/Im1` (object 5) and the given content,
/// with `extra` objects from id 6 on, in id order.
fn one_image_page(image_dict: &str, image_data: &[u8], content: &str, extra: Vec<Vec<u8>>) -> Vec<u8> {
    let mut objs = vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream_object(4, "", content.as_bytes()),
        stream_object(5, image_dict, image_data),
    ];
    objs.extend(extra);
    pdf_from_objects(&objs)
}

const RGB_DICT: &str = "/Type /XObject /Subtype /Image /Width 8 /Height 8 \
                        /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode";
const CONTENT: &str = "q 200 0 0 200 50 50 cm /Im1 Do Q\n";

/// Saves with the page's content streamed and every object top-level.
fn save(editor: &mut DocumentEditor, garbage_collect: bool) -> Vec<u8> {
    editor
        .save_to_bytes_with_options(SaveOptions {
            compress: false,
            garbage_collect,
            object_streams: false,
            ..SaveOptions::full_rewrite()
        })
        .unwrap()
}

/// The ids a file writes as `N 0 obj`.
fn object_ids(bytes: &[u8]) -> Vec<u32> {
    let mut ids = Vec::new();
    for at in bytes.windows(6).enumerate().filter(|(_, w)| *w == b" 0 obj").map(|(i, _)| i) {
        let digits: Vec<u8> = bytes[..at].iter().rev().take_while(|b| b.is_ascii_digit()).copied().collect();
        let digits: Vec<u8> = digits.into_iter().rev().collect();
        if let Some(id) = std::str::from_utf8(&digits).ok().and_then(|s| s.parse().ok()) {
            ids.push(id);
        }
    }
    ids
}

/// The id and decoded data of every stream the file writes.
fn decoded_streams(out: &[u8]) -> Vec<(u32, Vec<u8>)> {
    let doc = PdfDocument::from_bytes(out.to_vec()).unwrap();
    object_ids(out)
        .into_iter()
        .filter_map(|id| {
            let object = doc.load_object(ObjectRef::new(id, 0)).ok()?;
            Some((id, object.decode_stream_data().ok()?))
        })
        .collect()
}

/// Asserts no stream of the file decodes to contain `secret`, naming the
/// object that does.
fn assert_no_stream_holds(out: &[u8], secret: &[u8], what: &str) {
    for (id, data) in decoded_streams(out) {
        assert!(!contains(&data, secret), "{what}: object {id} holds the secret bytes");
    }
}

/// The XObjects page `index` names, by resource name.
fn page_xobjects(doc: &PdfDocument, index: usize) -> Vec<(String, ObjectRef)> {
    let page = doc.load_object(doc.get_page_ref(index).unwrap()).unwrap();
    let resources = doc.resolve_object(page.as_dict().unwrap().get("Resources").unwrap()).unwrap();
    let Some(xobjects) = resources.as_dict().and_then(|d| d.get("XObject")).cloned() else {
        return Vec::new();
    };
    let xobjects = doc.resolve_object(&xobjects).unwrap();
    xobjects
        .as_dict()
        .map(|d| {
            d.iter()
                .filter_map(|(name, value)| value.as_reference().map(|r| (name.clone(), r)))
                .collect()
        })
        .unwrap_or_default()
}

/// Width, height and RGB8 pixels of an image XObject, decoded the way the
/// renderer's extractor decodes it.
fn decoded_rgb(doc: &PdfDocument, reference: ObjectRef) -> (u32, u32, Vec<u8>) {
    let object = doc.load_object(reference).unwrap();
    let image = extract_image_from_xobject_at(Some(doc), &object, Some(reference), None, None).unwrap();
    let dynamic = image.to_dynamic_image().unwrap();
    (dynamic.width(), dynamic.height(), dynamic.to_rgb8().into_raw())
}

fn pixel(rgb: &[u8], x: usize, y: usize) -> [u8; 3] {
    let at = (y * W + x) * 3;
    [rgb[at], rgb[at + 1], rgb[at + 2]]
}

/// [`pixel`] for an image `width` samples across.
fn pixel_of(rgb: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
    let at = (y * width + x) * 3;
    [rgb[at], rgb[at + 1], rgb[at + 2]]
}

/// The covered interior is the fill, the far side is the source. The band
/// between them is the mark's padding and the image's edge, left unchecked.
fn assert_fill_and_source(copy: &[u8], source: &[u8], gc: bool) {
    for y in 0..H {
        for x in 0..W {
            let interior = x <= 2 && y >= 5;
            let outside = x >= 6 || y <= 2;
            if interior {
                assert_eq!(pixel(copy, x, y), [255, 0, 0], "gc={gc}: ({x}, {y}) is not the fill");
            } else if outside {
                assert_eq!(pixel(copy, x, y), pixel(source, x, y), "gc={gc}: ({x}, {y}) changed");
            }
        }
    }
}

/// The page's single image, after a save: its reference.
fn the_one_image(doc: &PdfDocument) -> ObjectRef {
    let xobjects = page_xobjects(doc, 0);
    assert_eq!(xobjects.len(), 1, "the page names {xobjects:?}");
    xobjects[0].1
}

/// Applies the box over the image in the page's content and saves.
fn redact_and_save(src: &[u8], page: usize, gc: bool) -> Vec<u8> {
    let mut editor = open_editor_from_bytes(src, "redact_image");
    editor.add_redaction(page, BOX, Some(RED)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    save(&mut editor, gc)
}

/// A raw Flate RGB image, its covered quadrant filled: the copy's covered
/// interior is the fill, its far side the source, and the source's covered
/// bytes are in no decoded stream of the file.
#[test]
fn raw_flate_rgb_covered_pixels_are_the_fill_and_the_source_bytes_are_gone() {
    let rgb = source_rgb();
    let src = one_image_page(RGB_DICT, &zlib(&rgb), CONTENT, vec![]);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let (w, h, copy) = decoded_rgb(&doc, the_one_image(&doc));
        assert_eq!((w as usize, h as usize), (W, H));
        assert_fill_and_source(&copy, &rgb, gc);
        assert_no_stream_holds(&out, &secret_run(&rgb), &format!("gc={gc}"));
        // Nothing else references the source image, so its object is not written.
        assert!(!object_ids(&out).contains(&5), "gc={gc}: the redacted source image (5) is still written");
    }
}

/// A box over the whole image: no image is drawn, and the source's bytes are
/// in no stream, with garbage collection on and off.
#[test]
fn fully_covered_image_leaves_no_image_on_the_page() {
    let rgb = source_rgb();
    let src = one_image_page(RGB_DICT, &zlib(&rgb), CONTENT, vec![]);
    for gc in [true, false] {
        let mut editor = open_editor_from_bytes(&src, "redact_full_image");
        editor.add_redaction(0, [40.0, 40.0, 260.0, 260.0], Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        let out = save(&mut editor, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        assert!(page_xobjects(&doc, 0).is_empty(), "gc={gc}: the page still names an image");
        assert!(!contains(&out, b"/Im1 Do"), "gc={gc}: the image's Do is kept");
        assert_no_stream_holds(&out, &secret_run(&rgb), &format!("gc={gc}"));
    }
}

/// An image with an /SMask: the copy's mask is rewritten opaque in the covered
/// rect, the mask's far side is the source's mask, and the image copy names
/// the new mask.
#[test]
fn smask_is_copied_opaque_in_the_covered_rect() {
    let rgb = source_rgb();
    let mask: Vec<u8> = (0..W * H).map(|i| if i % W <= 2 && i / W >= 5 { 17 } else { 128 }).collect();
    let image_dict = format!("{RGB_DICT} /SMask 6 0 R");
    let mask_obj = stream_object(
        6,
        "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray \
         /BitsPerComponent 8 /Filter /FlateDecode",
        &zlib(&mask),
    );
    let src = one_image_page(&image_dict, &zlib(&rgb), CONTENT, vec![mask_obj]);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let image = doc.load_object(the_one_image(&doc)).unwrap();
        let smask = image
            .as_dict()
            .and_then(|d| d.get("SMask"))
            .and_then(|o| o.as_reference())
            .expect("the copy keeps an /SMask");
        let (_, _, opaque) = decoded_rgb(&doc, smask);
        for y in 0..H {
            for x in 0..W {
                let v = opaque[(y * W + x) * 3];
                if x <= 2 && y >= 5 {
                    assert_eq!(v, 255, "gc={gc}: mask ({x}, {y}) is not opaque");
                } else if x >= 6 || y <= 2 {
                    assert_eq!(v, 128, "gc={gc}: mask ({x}, {y}) changed");
                }
            }
        }
        let (_, _, copy) = decoded_rgb(&doc, the_one_image(&doc));
        assert_fill_and_source(&copy, &rgb, gc);
    }
}

/// An image under an explicit stencil `/Mask`: sample 0 is the base image
/// showing (ISO 32000-1 §8.9.6.3, default `/Decode`). The copy's covered mask
/// samples are 0, so the filled base shows there: the render inside the box is
/// the fill, the render outside equals the source's, and the mask's bits
/// outside the box are the source's.
#[test]
fn stencil_mask_covered_samples_show_the_filled_base() {
    let rgb = source_rgb();
    // One byte a row, bit 1 masked out: the source's cells alternate between
    // the picture and the page behind it.
    let mask_bits: Vec<u8> = (0..H).map(|y| if y % 2 == 0 { 0b1010_1010 } else { 0b0101_0101 }).collect();
    let image_dict = format!("{RGB_DICT} /Mask 6 0 R");
    let mask_obj = stream_object(
        6,
        "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true /Filter /FlateDecode",
        &zlib(&mask_bits),
    );
    let src = one_image_page(&image_dict, &zlib(&rgb), CONTENT, vec![mask_obj]);
    let before = render(&src);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let image = doc.load_object(the_one_image(&doc)).unwrap();
        let mask_ref = image
            .as_dict()
            .and_then(|d| d.get("Mask"))
            .and_then(|o| o.as_reference())
            .expect("the copy keeps a /Mask");
        let copy_mask = doc.load_object(mask_ref).unwrap().decode_stream_data().unwrap();
        let (_, _, copy) = decoded_rgb(&doc, the_one_image(&doc));
        assert_fill_and_source(&copy, &rgb, gc);
        for y in 0..H {
            for x in 0..W {
                let bit = |bytes: &[u8]| (bytes[y] >> (7 - x)) & 1;
                if x <= 2 && y >= 5 {
                    assert_eq!(bit(&copy_mask), 0, "gc={gc}: mask ({x}, {y}) does not show the base");
                } else if x >= 6 || y <= 2 {
                    assert_eq!(bit(&copy_mask), bit(&mask_bits), "gc={gc}: mask ({x}, {y}) changed");
                }
            }
        }
        let after = render(&out);
        for y in 0..H {
            for x in 0..W {
                // Cell centres in page points: image column x spans 50 + 25x.
                let (px, py) = (62.5 + 25.0 * x as f32, 237.5 - 25.0 * y as f32);
                if x <= 2 && y >= 5 {
                    assert_eq!(pixel_at(&after, px, py), [255, 0, 0, 255], "gc={gc}: ({x}, {y}) is not the fill");
                } else if x >= 6 || y <= 2 {
                    assert_eq!(pixel_at(&after, px, py), pixel_at(&before, px, py), "gc={gc}: ({x}, {y}) changed");
                }
            }
        }
        assert_no_stream_holds(&out, &secret_run(&rgb), &format!("gc={gc}"));
    }
}

/// The same image on two pages, the box on page 0 only: page 1 keeps the
/// source image's own object, byte for byte, and page 0 names a copy. The
/// original is written because page 1 still references it.
#[test]
fn image_shared_with_an_unredacted_page_keeps_its_original() {
    let rgb = source_rgb();
    let data = zlib(&rgb);
    let src = pdf_from_objects(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R 6 0 R] /Count 2 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
        ),
        stream_object(4, "", CONTENT.as_bytes()),
        stream_object(5, RGB_DICT, &data),
        obj(
            6,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] \
             /Resources << /XObject << /Im1 5 0 R >> >> /Contents 7 0 R >>",
        ),
        stream_object(7, "", CONTENT.as_bytes()),
    ]);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let kept = the_one_image(&doc);
        let other = page_xobjects(&doc, 1);
        assert_eq!(other.len(), 1, "gc={gc}: page 1 names {other:?}");
        assert_eq!(other[0].1, ObjectRef::new(5, 0), "gc={gc}: page 1 lost the source image");
        assert_ne!(kept, ObjectRef::new(5, 0), "gc={gc}: page 0 still names the source image");
        let (_, _, untouched) = decoded_rgb(&doc, other[0].1);
        assert_eq!(untouched, rgb, "gc={gc}: page 1's image is not the source's pixels");
        let (_, _, copy) = decoded_rgb(&doc, kept);
        assert_fill_and_source(&copy, &rgb, gc);
    }
}

/// A DCT image of 32 by 32, four by four blocks: the box covers the lower-left
/// quadrant, so the replaced blocks are the lower-left ones and the copy's far
/// side is the source. The covered interior is the fill within the codec's
/// error, and the source's JPEG bytes are in no stream of the file. (An 8 by 8
/// image is one block, which the copy replaces whole, so it cannot show a far side.)
#[test]
fn dct_image_covered_pixels_are_the_fill_within_the_codec() {
    const N: usize = 32;
    let rgb: Vec<u8> = (0..N * N)
        .flat_map(|i| {
            let (x, y) = (i % N, i / N);
            [((x * 7 + y * 3) % 256) as u8, ((x * 5 + y * 11) % 256) as u8, ((x * y) % 256) as u8]
        })
        .collect();
    let jpeg = encode_jpeg(&rgb, N as u32, N as u32, 3, 90, Subsampling::Full).unwrap();
    let dict = "/Type /XObject /Subtype /Image /Width 32 /Height 32 \
                /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode";
    let src = one_image_page(dict, &jpeg, CONTENT, vec![]);
    // The source is lossy, so its far side is compared with the source's own decode:
    // the copy keeps those blocks' coefficients, so the two decodes are equal.
    let source_doc = PdfDocument::from_bytes(src.clone()).unwrap();
    let (_, _, source_pixels) = decoded_rgb(&source_doc, the_one_image(&source_doc));
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let (w, h, copy) = decoded_rgb(&doc, the_one_image(&doc));
        assert_eq!((w as usize, h as usize), (N, N), "gc={gc}: the copy's size");
        for y in 0..N {
            for x in 0..N {
                let got = pixel_of(&copy, N, x, y);
                if x <= 13 && y >= 18 {
                    for c in 0..3 {
                        assert!([255u8, 0, 0][c].abs_diff(got[c]) <= 32, "gc={gc}: ({x}, {y}) = {got:?}");
                    }
                } else if x >= 24 || y < 8 {
                    let want = pixel_of(&source_pixels, N, x, y);
                    assert_eq!(got, want, "gc={gc}: ({x}, {y}) is not the source's decode");
                }
            }
        }
        assert!(!contains(&out, &jpeg), "gc={gc}: the source's JPEG is kept");
    }
}

/// Two overlapping boxes on a raw image: the overlap takes the later box's fill,
/// and each box alone takes its own. The list is in drawing order, so the later
/// box is the one drawn on top.
#[test]
fn raw_image_shared_pixel_takes_the_later_region_fill() {
    const BLUE: [f32; 3] = [0.0, 0.0, 1.0];
    let rgb = source_rgb();
    let src = one_image_page(RGB_DICT, &zlib(&rgb), CONTENT, vec![]);
    let mut editor = open_editor_from_bytes(&src, "redact_image_overlap");
    editor.add_redaction(0, BOX, Some(RED)).unwrap();
    editor.add_redaction(0, [100.0, 50.0, 200.0, 150.0], Some(BLUE)).unwrap();
    editor.apply_redactions_destructive(Default::default()).unwrap();
    let out = save(&mut editor, false);
    let doc = PdfDocument::from_bytes(out).unwrap();
    let (_, _, copy) = decoded_rgb(&doc, the_one_image(&doc));
    // The image is 8 by 8 over 25 points a sample: columns 0 to 3 are under
    // box A, columns 2 to 5 under box B, so columns 2 and 3 are the overlap.
    assert_eq!(pixel(&copy, 1, 6), [255, 0, 0], "box A alone");
    assert_eq!(pixel(&copy, 3, 6), [0, 0, 255], "the overlap is not the later box's fill");
    assert_eq!(pixel(&copy, 5, 6), [0, 0, 255], "box B alone");
}

/// A CCITT image whose /Decode inverts its bits: the copy's far side is the
/// picture the source decodes to, which only holds when the copy drops the
/// /Decode its rows already applied.
#[test]
fn ccitt_inverted_decode_far_side_matches_the_source_picture() {
    let gray: Vec<u8> = (0..W * H).map(|i| if (i % W + i / W) % 3 == 0 { 0 } else { 255 }).collect();
    let fax = encode_ccitt_g4(&gray, W as u32, H as u32).unwrap();
    let dict = "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ColorSpace /DeviceGray \
                /BitsPerComponent 1 /Filter /CCITTFaxDecode \
                /DecodeParms << /K -1 /Columns 8 /Rows 8 >> /Decode [1 0]";
    let src = one_image_page(dict, &fax, CONTENT, vec![]);
    let source_doc = PdfDocument::from_bytes(src.clone()).unwrap();
    let source_ref = the_one_image(&source_doc);
    let (_, _, source) = decoded_rgb(&source_doc, source_ref);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let (_, _, copy) = decoded_rgb(&doc, the_one_image(&doc));
        for y in 0..H {
            for x in 0..W {
                if x >= 6 || y <= 2 {
                    assert_eq!(pixel(&copy, x, y), pixel(&source, x, y), "gc={gc}: ({x}, {y}) changed");
                }
            }
        }
    }
}

/// An image with a colour-key /Mask whose range holds the fill colour: the
/// fill is nudged out of the range, so the covered pixels show the fill and
/// the far side is the source.
#[test]
fn colour_key_mask_does_not_hide_the_fill() {
    let rgb = source_rgb();
    let dict = format!("{RGB_DICT} /Mask [255 255 0 0 0 0]");
    let src = one_image_page(&dict, &zlib(&rgb), CONTENT, vec![]);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let (_, _, copy) = decoded_rgb(&doc, the_one_image(&doc));
        for y in 0..H {
            for x in 0..W {
                let got = pixel(&copy, x, y);
                if x <= 2 && y >= 5 {
                    assert!(got[0] >= 250 && got[1] <= 5 && got[2] <= 5, "gc={gc}: ({x}, {y}) = {got:?}");
                } else if x >= 6 || y <= 2 {
                    assert_eq!(got, pixel(&rgb, x, y), "gc={gc}: ({x}, {y}) changed");
                }
            }
        }
    }
}

/// The same image placed twice on one page, the box over the first placement
/// only: the first places a copy, the second keeps the source name, so the
/// source image is written and its bytes are the source's.
#[test]
fn image_placed_twice_on_one_page_keeps_the_source_for_the_uncovered_placement() {
    let rgb = source_rgb();
    let content = "q 200 0 0 200 50 50 cm /Im1 Do Q\nq 40 0 0 40 255 255 cm /Im1 Do Q\n";
    let src = one_image_page(RGB_DICT, &zlib(&rgb), content, vec![]);
    for gc in [true, false] {
        let out = redact_and_save(&src, 0, gc);
        let doc = PdfDocument::from_bytes(out.clone()).unwrap();
        let xobjects = page_xobjects(&doc, 0);
        assert_eq!(xobjects.len(), 2, "gc={gc}: the page names {xobjects:?}");
        let kept = xobjects
            .iter()
            .find(|(name, _)| name == "Im1")
            .expect("the uncovered placement keeps its source name");
        assert_eq!(kept.1, ObjectRef::new(5, 0), "gc={gc}: the uncovered placement lost the source");
        let (_, _, untouched) = decoded_rgb(&doc, kept.1);
        assert_eq!(untouched, rgb, "gc={gc}: the source's pixels changed");
        let copy = xobjects.iter().find(|(name, _)| name != "Im1").expect("a copy").1;
        let (_, _, copied) = decoded_rgb(&doc, copy);
        assert_fill_and_source(&copied, &rgb, gc);
    }
}

/// An inline image whose left half is under the box: the render shows the
/// fill there and the source's pixels on the right, and the inline image's
/// covered bytes are not in the file.
#[test]
fn inline_image_left_half_is_the_fill_and_the_right_half_is_the_source() {
    // Bytes that carry no `EI` and no whitespace the parser reads as one.
    let gray: Vec<u8> = (0..W * H).map(|i| (i as u8).wrapping_mul(7).wrapping_add(33)).collect();
    let mut content = b"q 200 0 0 200 50 50 cm BI /W 8 /H 8 /BPC 8 /CS /G ID ".to_vec();
    content.extend_from_slice(&gray);
    content.extend_from_slice(b" EI Q\n");
    let src = pdf_from_objects(&[
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>",
        ),
        stream_object(4, "", &content),
    ]);
    let before = render(&src);
    let out = redact_and_save(&src, 0, false);
    let after = render(&out);
    let inside = pixel_at(&after, 80.0, 80.0);
    assert_eq!(inside, [255, 0, 0, 255], "the covered inline pixels are not the fill");
    let outside = pixel_at(&after, 200.0, 200.0);
    assert_eq!(outside, pixel_at(&before, 200.0, 200.0), "the source's pixels changed");
    // Row 5, columns 0 to 2: covered, so absent from the copy's bytes.
    assert!(!contains(&out, &gray[40..43]), "the inline image's covered bytes are kept");
}

const DPI: u32 = 150;
const PAGE: f32 = 300.0;

fn render(bytes: &[u8]) -> image::RgbaImage {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    let rendered = render_page(&doc, 0, &RenderOptions::with_dpi(DPI).with_transparent_background()).unwrap();
    image::load_from_memory(&rendered.data).unwrap().to_rgba8()
}

fn pixel_at(img: &image::RgbaImage, x: f32, y: f32) -> [u8; 4] {
    let scale = DPI as f32 / 72.0;
    img.get_pixel((x * scale) as u32, ((PAGE - y) * scale) as u32).0
}

/// A DCT image on a page, redacted, rendered at 150 dpi: inside the box, padded
/// by the codec's blocks, the page shows the fill, and far from the box it shows
/// the source's render exactly. The image spans 50 to 250 points, so the box
/// covers its lower-left quadrant. The redaction pads the box by two points, and
/// the copy replaces every 8-by-8 block the padded box meets, which reaches to
/// page x = 200 and y = 200.
#[test]
fn dct_image_render_is_the_fill_inside_the_box_and_the_source_outside() {
    const N: usize = 32;
    let rgb: Vec<u8> = (0..N * N)
        .flat_map(|i| {
            let (x, y) = (i % N, i / N);
            [((x * 7 + y * 3) % 256) as u8, ((x * 5 + y * 11) % 256) as u8, ((x * y) % 256) as u8]
        })
        .collect();
    let jpeg = encode_jpeg(&rgb, N as u32, N as u32, 3, 90, Subsampling::Full).unwrap();
    let dict = "/Type /XObject /Subtype /Image /Width 32 /Height 32 \
                /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode";
    let src = one_image_page(dict, &jpeg, CONTENT, vec![]);
    let before = render(&src);
    for gc in [true, false] {
        let after = render(&redact_and_save(&src, 0, gc));
        for py in (52..=248).step_by(4) {
            for px in (52..=248).step_by(4) {
                let (x, y) = (px as f32, py as f32);
                let got = pixel_at(&after, x, y);
                if (56.0..=144.0).contains(&x) && (56.0..=144.0).contains(&y) {
                    assert!(got[0] >= 223 && got[1] <= 32 && got[2] <= 32, "gc={gc}: ({x}, {y}) = {got:?}");
                } else if x >= 210.0 || y >= 210.0 {
                    assert_eq!(got, pixel_at(&before, x, y), "gc={gc}: ({x}, {y}) is not the source's render");
                }
            }
        }
    }
}
