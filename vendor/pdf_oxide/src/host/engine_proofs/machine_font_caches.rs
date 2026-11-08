//! The machine's font caches are the process's bytes, never an executor's.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::host::engine_state::EngineState;
use crate::host::native::executor::{reset_executor, ExecutorController, Mail};
use crate::host::heap;
use crate::rendering::{PageRenderer, RenderOptions};
use std::thread;

fn plain(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

/// A page with a font it does not embed, so text in it takes its font from
/// the machine.
fn page_in_a_machine_font(content: &str) -> Vec<u8> {
    pdf_from_objects(&[
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
        ),
        stream_object(4, "", content.as_bytes()),
        plain(5, "<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /Encoding /WinAnsiEncoding >>"),
    ])
}

fn render(pdf: &[u8]) {
    let doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
    PageRenderer::new(RenderOptions::with_dpi(72)).render_page(&doc, 0).unwrap();
}

/// A lane renders text in a machine font and is reset: the machine's font
/// bytes the render put in the caches count in the process's figure and not
/// in the lane's, so the lane's heap after the reset is where it was before
/// the job, while the process holds the font. Without the guard the lane's
/// figure stays a font above where it began and the lane is retired for
/// bytes every lane shares. No Dart battery can observe this: the counter is
/// the engine's own and no wire op reads it.
#[test]
fn a_machine_font_the_lane_cached_is_not_the_lanes_heap() {
    // The system font database is built by whichever thread renders first.
    thread::spawn(|| drop(PageRenderer::new(RenderOptions::with_dpi(72)))).join().unwrap();
    let pdf = page_in_a_machine_font("BT /F1 18 Tf 20 50 Td (Hello) Tj ET");
    let blank = page_in_a_machine_font("0 0 10 10 re f");

    thread::spawn(move || {
        let mut state = EngineState::new();
        let (sender, _mailbox) = crossbeam_channel::unbounded::<Mail>();
        let controller = ExecutorController::new(sender);
        render(&blank);
        reset_executor(&mut state, &controller);
        let before = heap::live();
        let process_before = heap::process_live();

        render(&pdf);
        assert!(
            heap::process_live() > process_before + 1024,
            "the render cached no machine font, so the test proves nothing"
        );
        reset_executor(&mut state, &controller);
        let after = heap::live();
        assert!(
            after.abs_diff(before) <= 4096,
            "the lane held {before} bytes before its job and {after} after its reset"
        );
    })
    .join()
    .unwrap();
}
