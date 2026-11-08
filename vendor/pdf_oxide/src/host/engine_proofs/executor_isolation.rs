//! Two executors share nothing the engine caches, and a reset empties one.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::extractors::warnings::{push_global_warning, snapshot_global_warnings, Warning, WarningCategory};
use crate::fonts::cmap::cmap_cache_size;
use crate::fonts::form_fallback::Fallback;
use crate::fonts::global_cache::global_font_cache_stats;
use crate::host::engine_state::EngineState;
use crate::host::native::executor::{reset_executor, ExecutorController, Mail};
use crate::host::{caches, heap};
use std::sync::mpsc;
use std::thread;

fn plain(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

/// A page of text in Helvetica with a ToUnicode CMap, so reading it fills the
/// font cache and the CMap cache.
fn page_with_a_font() -> Vec<u8> {
    pdf_from_objects(&[
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
        ),
        stream_object(4, "", b"BT /F1 12 Tf 20 50 Td (Hello) Tj ET"),
        plain(5, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 6 0 R >>"),
        stream_object(
            6,
            "",
            b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
              1 begincodespacerange <00> <FF> endcodespacerange \
              1 beginbfrange <00> <FF> <0000> endbfrange endcmap end end",
        ),
    ])
}

fn read_text(pdf: &[u8]) {
    let doc = PdfDocument::from_bytes(pdf.to_vec()).unwrap();
    doc.extract_text(0).unwrap();
}

/// Executor 1 registers a fallback font, reads a document and raises a warning;
/// executor 2, made while executor 1 holds all of that, sees none of it; a reset of
/// executor 1 through the executor's own reset path then empties the registration,
/// both caches and the warning sink, and gives back the heap the job took.
/// No Dart battery can observe this: the caches and the sink are engine
/// statics behind no wire op, and the heap figure is the engine's own counter.
#[test]
fn an_executor_owns_its_caches_and_a_reset_empties_them() {
    let pdf = page_with_a_font();
    // The process-wide tables the first read builds are allocated by whichever
    // thread reads first; building them here keeps them out of executor 1's heap.
    read_text(&pdf);

    let (loaded_tx, loaded_rx) = mpsc::channel::<()>();
    let (checked_tx, checked_rx) = mpsc::channel::<()>();

    let executor_1 = thread::spawn(move || {
        let mut state = EngineState::new();
        let (sender, _mailbox) = crossbeam_channel::unbounded::<Mail>();
        let controller = ExecutorController::new(sender);
        // A thread's first read allocates its thread-locals once and keeps
        // them (49 KiB, the same after one reset as after three); one read
        // and reset here put them in the figure the later reset is held to.
        read_text(&pdf);
        reset_executor(&mut state, &controller);
        let before = heap::live();

        caches::with(|e| e.register(Fallback::Cjk, vec![7; 300]));
        push_global_warning(Warning {
            category: WarningCategory::Font,
            page: None,
            message: "executor 1".into(),
            spec_section: None,
        });
        read_text(&pdf);
        assert!(global_font_cache_stats().0 > 0, "reading the page cached no font");
        assert!(cmap_cache_size() > 0, "reading the page cached no CMap");
        loaded_tx.send(()).unwrap();
        checked_rx.recv().unwrap();

        assert!(caches::with(|e| e.registered(Fallback::Cjk)).is_some());
        assert!(global_font_cache_stats().0 > 0);
        reset_executor(&mut state, &controller);

        assert!(caches::with(|e| e.registered(Fallback::Cjk)).is_none());
        assert_eq!(global_font_cache_stats().0, 0, "the reset left fonts cached");
        assert_eq!(cmap_cache_size(), 0, "the reset left CMaps cached");
        assert!(snapshot_global_warnings().is_empty(), "the reset left a warning");
        let after = heap::live();
        assert!(
            after.abs_diff(before) <= 4096,
            "executor 1 held {before} bytes before its job and {after} after its reset"
        );
    });

    let executor_2 = thread::spawn(move || {
        loaded_rx.recv().unwrap();
        let _state = EngineState::new();
        assert!(caches::with(|e| e.registered(Fallback::Cjk)).is_none());
        assert_eq!(global_font_cache_stats().0, 0, "executor 2 sees executor 1's fonts");
        assert_eq!(cmap_cache_size(), 0, "executor 2 sees executor 1's CMaps");
        checked_tx.send(()).unwrap();
    });

    executor_2.join().unwrap();
    executor_1.join().unwrap();
}
