//! The engine caches and registrations: a font loaded inside a font load,
//! and a fallback font registered twice.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::fonts::cmap::cmap_cache_size;
use crate::fonts::form_fallback::{self, Fallback};
use crate::fonts::global_cache::global_font_cache_stats;
use crate::host::engine_state::EngineState;

fn plain(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

const IDENTITY_CMAP: &[u8] = b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
    1 begincodespacerange <00> <FF> endcodespacerange \
    1 beginbfrange <00> <FF> <0000> endbfrange endcmap end end";

/// A page of three fonts: a Type 3 font whose glyph procedure shows a glyph of
/// Helvetica; Helvetica, which the executor's cache holds; and Courier, which a
/// Form XObject names and extraction loads in the middle of the page. Every
/// font but the Type 3 one has a ToUnicode CMap, which the CMap cache holds.
fn fonts_loading_fonts() -> Vec<u8> {
    pdf_from_objects(&[
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R \
             /Resources << /Font << /T3 5 0 R /F1 8 0 R >> /XObject << /X1 10 0 R >> >> >>",
        ),
        stream_object(
            4,
            "",
            b"BT /T3 40 Tf 20 30 Td (aa) Tj ET BT /F1 12 Tf 20 80 Td (Hi) Tj ET /X1 Do",
        ),
        plain(
            5,
            "<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] \
             /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << /a 6 0 R >> \
             /Encoding << /Type /Encoding /Differences [97 /a] >> \
             /FirstChar 97 /LastChar 97 /Widths [1000] \
             /Resources << /Font << /H 7 0 R >> >> >>",
        ),
        stream_object(6, "", b"1000 0 d0 BT /H 800 Tf 100 100 Td (A) Tj ET"),
        plain(7, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        plain(8, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /ToUnicode 9 0 R >>"),
        stream_object(9, "", IDENTITY_CMAP),
        stream_object(
            10,
            "/Type /XObject /Subtype /Form /BBox [0 0 200 100] \
             /Resources << /Font << /F2 11 0 R >> >>",
            b"BT /F2 12 Tf 100 80 Td (Yo) Tj ET",
        ),
        plain(11, "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /ToUnicode 9 0 R >>"),
    ])
}

/// A lane whose page names a Type 3 font that loads Helvetica in its glyph
/// procedure, a Helvetica of its own and a Courier inside a Form XObject
/// renders it, with ink where only the nested Helvetica can paint, and extracts
/// all its text: fonts load inside the extraction of a page and inside a glyph,
/// the executor's caches fill, and no cache borrow is held across a load. No Dart
/// battery can observe this: a RefCell double borrow is a panic that depends on
/// the order of cache accesses inside nested font loads, and only the engine's
/// own fixture, with an engine state installed, reaches it.
#[test]
fn fonts_that_load_fonts_render_and_extract_inside_a_lane() {
    let _lane = EngineState::new();
    let doc = PdfDocument::from_bytes(fonts_loading_fonts()).unwrap();

    let text = doc.extract_text(0).unwrap();
    for shown in ["aa", "Hi", "Yo"] {
        assert!(text.contains(shown), "extracted {text:?}, missing {shown:?}");
    }
    assert!(global_font_cache_stats().0 >= 2, "the lane's font cache holds the page's fonts");
    assert!(cmap_cache_size() >= 1, "the lane's CMap cache holds their CMap");

    #[cfg(feature = "rendering")]
    {
        use crate::rendering::{PageRenderer, RenderOptions};
        let mut renderer = PageRenderer::new(RenderOptions::with_dpi(72));
        let rendered = renderer.render_page(&doc, 0).unwrap();
        let page = image::load_from_memory(&rendered.data).unwrap().to_rgba8();
        let ink = (20..60)
            .flat_map(|x| (30..70).map(move |y| (x, y)))
            .filter(|&(x, y)| page.get_pixel(x, y).0[..3].iter().all(|&c| c < 64))
            .count();
        assert!(ink > 0, "the glyph procedure's Helvetica painted nothing");
    }
}

/// Registering a fallback font twice keeps the second, and the form-flatten
/// lookup returns it. No Dart battery can observe this: the router replays one
/// registration to every lane, and which bytes a lane holds shows only in the
/// font the engine embeds, which a form fixture would have to render to tell.
#[test]
fn a_fallback_font_registered_twice_is_the_second() {
    let _lane = EngineState::new();
    crate::host::dispatch::register_fallback_font("cjk", vec![1, 2, 3]).unwrap();
    crate::host::dispatch::register_fallback_font("cjk", vec![9, 8]).unwrap();

    let registered = crate::host::caches::with(|e| e.registered(Fallback::Cjk));
    assert_eq!(registered.as_deref(), Some(&[9u8, 8][..]));
    let resolved = form_fallback::resolve_font_bytes(Fallback::Cjk).expect("a registered font");
    assert_eq!(&*resolved, &[9u8, 8][..]);
    assert!(crate::host::caches::with(|e| e.registered(Fallback::Emoji)).is_none());
}
