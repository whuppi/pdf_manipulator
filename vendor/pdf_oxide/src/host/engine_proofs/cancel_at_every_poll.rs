//! A cancel at any poll of a mutating editor op leaves the editor as it was.
//!
//! Every mutating op that takes a `CancelCheck` is run under a check that
//! cancels at the `n`th poll, for every `n` from the first poll to the last
//! one a full run makes, and then once past the last. The editor must write
//! the bytes it wrote before the run and hold the objects it held, or the op
//! must have succeeded. The two ops are `reduceImages` and `convertToPdfA`;
//! the other mutating ops take no check, so they cannot stop midway.

use super::band_proof::detailed_rgb;
use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::error::{Error, Result};
use crate::host::images::execute::reduce_images;
use crate::host::images::policy::{Chroma, Policy};
use std::io::Write;

/// What an editor holds that a save writes: the bytes of a full save and the
/// ids of the objects it has staged. Two saves of an untouched editor write
/// the same bytes, so a difference is a change the op left behind.
fn snapshot(editor: &mut DocumentEditor) -> (Vec<u8>, Vec<u32>) {
    let bytes = editor.save_to_bytes().expect("the editor saves");
    let mut staged: Vec<u32> = editor.modified_objects_mut().keys().copied().collect();
    staged.sort_unstable();
    (bytes, staged)
}

fn is_cancelled(error: &Error) -> bool {
    matches!(error, Error::Io(e) if e.to_string() == "operation cancelled")
}

/// Runs `op` on a fresh editor under a check that cancels at poll `n`, for
/// every `n` up to the polls of a full run, and past it.
fn at_every_poll<T>(
    fresh: impl Fn() -> DocumentEditor,
    op: impl Fn(&mut DocumentEditor, &CancelCheck) -> Result<T>,
) {
    let mut editor = fresh();
    let before = snapshot(&mut editor);
    let counting = CancelCheck::tripping_at(usize::MAX);
    op(&mut editor, &counting).expect("a full run succeeds");
    let polls = counting.polls();
    assert!(polls > 1, "a full run polls more than once: {polls}");
    assert!(snapshot(&mut editor) != before, "a full run changes the editor");

    for n in 1..=polls {
        let mut editor = fresh();
        let before = snapshot(&mut editor);
        let check = CancelCheck::tripping_at(n);
        let error = match op(&mut editor, &check) {
            Ok(_) => panic!("a cancel at poll {n} of {polls} did not stop the op"),
            Err(error) => error,
        };
        assert!(is_cancelled(&error), "poll {n} of {polls} ended with {error:?}");
        assert!(
            snapshot(&mut editor) == before,
            "poll {n} of {polls} left the editor changed"
        );
    }

    let mut editor = fresh();
    let check = CancelCheck::tripping_at(polls + 1);
    op(&mut editor, &check).expect("a cancel past the last poll never lands");
}

fn flate(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(data).expect("compresses");
    z.finish().expect("finishes")
}

/// One page drawing three images that each reduce: RGB, gray and RGB with a
/// soft mask, so the op stages an image and a mask object.
fn images_pdf() -> Vec<u8> {
    const W: u32 = 128;
    const H: u32 = 96;
    let rgb = detailed_rgb(W, H);
    let gray = flate(&rgb.iter().step_by(3).copied().collect::<Vec<_>>());
    let mask: Vec<u8> = (0..64 * 48u32).map(|i| (i * 7 % 251) as u8).collect();
    let dims = format!("/Width {W} /Height {H}");
    let objects = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 200] /Contents 4 0 R \
          /Resources << /XObject << /Im0 5 0 R /Im1 6 0 R /Im2 7 0 R >> >> >>\nendobj\n"
            .to_vec(),
        stream_object(
            4,
            "",
            b"q 32 0 0 24 10 10 cm /Im0 Do Q\nq 32 0 0 24 110 10 cm /Im1 Do Q\n\
              q 32 0 0 24 210 10 cm /Im2 Do Q\n",
        ),
        stream_object(
            5,
            &format!("/Type /XObject /Subtype /Image {dims} /ColorSpace /DeviceRGB /BitsPerComponent 8"),
            &rgb,
        ),
        stream_object(
            6,
            &format!(
                "/Type /XObject /Subtype /Image {dims} /ColorSpace /DeviceGray \
                 /BitsPerComponent 8 /Filter /FlateDecode"
            ),
            &gray,
        ),
        stream_object(
            7,
            &format!(
                "/Type /XObject /Subtype /Image {dims} /ColorSpace /DeviceRGB \
                 /BitsPerComponent 8 /SMask 8 0 R"
            ),
            &rgb,
        ),
        stream_object(
            8,
            "/Type /XObject /Subtype /Image /Width 64 /Height 48 /ColorSpace /DeviceGray \
             /BitsPerComponent 8",
            &mask,
        ),
    ];
    pdf_from_objects(&objects)
}

fn editor_of(bytes: Vec<u8>) -> DocumentEditor {
    let doc = PdfDocument::from_bytes(bytes).expect("the fixture parses");
    DocumentEditor::from_document(doc).expect("the editor opens")
}

/// No Dart battery sees it: whether a cancel at each poll of the engine's
/// loop leaves the editor's staged objects untouched is a property of the
/// engine's loop, which a battery reaches only at the polls a timing happens
/// to hit.
#[test]
fn reduce_images_cancelled_at_any_poll_stages_nothing() {
    let policy = Policy {
        color_ppi: Some(72.0),
        gray_ppi: Some(72.0),
        mono_ppi: Some(72.0),
        downsample_threshold: 1.5,
        jpeg_quality: 90,
        allow_lossy: true,
        convert_cmyk_to_rgb: false,
        min_pixels: 32,
        min_savings: 0.0,
        chroma: Chroma::Full,
        recompress_jpeg: false,
    };
    let bytes = images_pdf();
    at_every_poll(
        || editor_of(bytes.clone()),
        |editor, cancel| reduce_images(editor, &policy, cancel),
    );
}

/// Two pages of vector content, with no metadata, no output intent and no
/// PDF/A identification, so every fix the converter makes has something to do.
#[cfg(feature = "pdfa")]
fn pdfa_pdf() -> Vec<u8> {
    let objects = [
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(4, "", b"0 0 1 rg 20 20 100 100 re f\n"),
        b"5 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 6 0 R >>\nendobj\n"
            .to_vec(),
        stream_object(6, "", b"1 0 0 rg 40 40 80 80 re f\n"),
    ];
    pdf_from_objects(&objects)
}

#[cfg(feature = "pdfa")]
/// No Dart battery sees it: the conversion commits its fixes by saving and
/// re-parsing the editor's document in place, and validates the committed
/// document afterwards, so a cancel at a poll after the commit is a state a
/// battery cannot place; only a check that cancels at an exact poll can.
#[test]
fn convert_to_pdf_a_cancelled_at_any_poll_changes_nothing() {
    let bytes = pdfa_pdf();
    at_every_poll(
        || {
            // A staged edit of the caller's, so the editor has state of its own to lose.
            let mut editor = editor_of(bytes.clone());
            editor.set_page_rotation(1, 90).expect("the page turns");
            editor
        },
        |editor, cancel| crate::host::dispatch::edit_convert_to_pdf_a(editor, 2, cancel),
    );
}
