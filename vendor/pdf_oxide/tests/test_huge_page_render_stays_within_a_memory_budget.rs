//! A page's raster is bounded by a budget, not by the file.
//!
//! Nothing in a PDF bounds `page_box x scale`. `docs/spec/pdf.md` leaves the
//! MediaBox extent entirely in the file's hands — Table 30 (pdf.md:5760)
//! defines it as the rectangle that "shall define the boundaries of the
//! physical medium on which the page shall be displayed or printed", with no
//! ceiling — so a conforming file may declare a page whose raster is hundreds
//! of megapixels.
//!
//! Annex C.1 (pdf.md:41166) names the constraint this runs into: "The amount
//! of memory available to a conforming reader limits the number of
//! memory-consuming objects that can be held simultaneously", and the note at
//! C.2 (pdf.md:41183) adds that "Memory limits are often exceeded before
//! architectural limits ... are reached". The spec therefore treats staying
//! inside available memory as the reader's own responsibility; it gives no
//! page size a reader may assume it will never see.
//!
//! One real 6 MB document declares a 12608 x 16806 pt page carrying a 211.9
//! megapixel image. At 72 dpi that is an 847 MB pixmap before any working
//! copy; rendering it reached 11.6 GB and the process was killed. An OOM kill
//! is a signal, so the caller never gets a `Result` and the host process dies
//! outright — a thumbnailer or web service handling untrusted input has no
//! defence, and the WASM and mobile targets have far less headroom than the
//! machine that died.
//!
//! So the renderer never holds the raster whole: it renders the page in
//! bands that stay inside a fixed budget and hands their rows to a sink, at
//! the size the caller asked for. The page is neither scaled down nor
//! cropped, and its size decides how long the render takes, not how much
//! memory it needs.

#![cfg(feature = "rendering")]

use pdf_oxide::cancel::CancelCheck;
use pdf_oxide::rendering::{render_page, render_page_rows, RenderOptions, RowSink};
use pdf_oxide::PdfDocument;

/// A one-page document whose MediaBox is `w` x `h` points, with a black
/// square filling the middle third of the page.
///
/// The mark is placed proportionally so that a *scaled* render still contains
/// it while a render that merely allocated a smaller buffer — cropping the
/// page to its top-left corner — would not.
fn page_sized(w: u32, h: u32) -> Vec<u8> {
    let content = format!("0 0 0 rg {} {} {} {} re f\n", w / 3, h / 3, w / 3, h / 3).into_bytes();
    let mut pdf = Vec::new();
    let mut off = [0usize; 6];
    macro_rules! push {
        ($s:expr) => {
            pdf.extend_from_slice($s.as_bytes())
        };
    }
    push!("%PDF-1.7\n");
    off[1] = pdf.len();
    push!("1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    off[2] = pdf.len();
    push!("2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    off[3] = pdf.len();
    push!(format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] \
         /Contents 4 0 R /Resources << >> >>\nendobj\n"
    ));
    off[4] = pdf.len();
    push!(format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()));
    pdf.extend_from_slice(&content);
    push!("endstream\nendobj\n");
    let xref = pdf.len();
    push!("xref\n0 5\n0000000000 65535 f \r\n".to_string());
    for id in 1..=4 {
        push!(format!("{:010} 00000 n \r\n", off[id]));
    }
    push!(format!("trailer\n<< /Size 5 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"));
    pdf
}

/// A sink that keeps no rows: it counts them, notes the largest piece it was
/// handed and how many pixels are substantially darker than white.
struct Tally {
    /// The raster's width in pixels, to turn a piece's length into rows.
    width: usize,
    rows: usize,
    largest_piece: usize,
    dark: u64,
}

impl RowSink for Tally {
    fn rows(&mut self, y: u32, rgba_straight: &[u8]) -> pdf_oxide::error::Result<()> {
        assert_eq!(y as usize, self.rows, "rows arrive top to bottom without a gap");
        self.largest_piece = self.largest_piece.max(rgba_straight.len());
        self.rows += rgba_straight.len() / 4 / self.width;
        for p in rgba_straight.chunks_exact(4) {
            if p[3] > 0 && (u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2])) / 3 < 128 {
                self.dark += 1;
            }
        }
        Ok(())
    }
}

/// Renders the page into a [`Tally`] for a raster `width` pixels wide.
fn rendered_rows(pdf: Vec<u8>, opts: &RenderOptions, width: usize) -> (Tally, (u32, u32)) {
    let doc = PdfDocument::from_bytes(pdf).expect("synthetic PDF parses");
    let mut tally = Tally {
        width,
        rows: 0,
        largest_piece: 0,
        dark: 0,
    };
    let size = render_page_rows(&doc, 0, opts, &mut tally, &CancelCheck::none()).expect("renders");
    (tally, size)
}

fn rendered_dimensions(pdf: Vec<u8>, opts: &RenderOptions) -> (u32, u32) {
    let doc = PdfDocument::from_bytes(pdf).expect("synthetic PDF parses");
    let img = render_page(&doc, 0, opts).expect("page renders");
    (img.width, img.height)
}

#[test]
fn test_enormous_page_renders_at_its_natural_size_in_bounded_bands() {
    // 5000 x 5000 pt at 72 dpi is 25 megapixels: a 100 MB raster if it were
    // held whole.
    let opts = RenderOptions::with_dpi(72);
    let (tally, (w, h)) = rendered_rows(page_sized(5000, 5000), &opts, 5000);

    assert_eq!((w, h), (5000, 5000), "the page renders at the size asked for");
    assert_eq!(tally.rows, 5000, "every row of the page reaches the sink");
    assert!(
        tally.largest_piece <= 2 * 1024 * 1024,
        "a band of {} bytes exceeds the 2 MiB band budget",
        tally.largest_piece
    );
}

#[test]
fn test_enormous_page_is_rendered_whole_rather_than_scaled_or_cropped() {
    // The whole page must be in the picture at the requested scale. The
    // fixture's mark covers the middle ninth of the page, so a correct render
    // inks about 1/9 of the raster; one scaled into the wrong place, or
    // cropped to a corner, inks none of it or all of it.
    let opts = RenderOptions::with_dpi(72);
    let (tally, (w, h)) = rendered_rows(page_sized(5000, 5000), &opts, 5000);

    let coverage = tally.dark as f64 / (f64::from(w) * f64::from(h));
    assert!(
        (0.10..0.12).contains(&coverage),
        "expected the centre mark to cover about 1/9 of the raster, got {coverage:.4}"
    );
}

#[test]
fn test_ordinary_page_is_left_at_its_natural_size() {
    // US Letter at the default 150 dpi: 612 x 792 pt, rounded up to whole
    // pixels.
    let scale = RenderOptions::default().dpi as f32 / 72.0;
    let expected = ((612.0 * scale).ceil() as u32, (792.0 * scale).ceil() as u32);

    assert_eq!(
        rendered_dimensions(page_sized(612, 792), &RenderOptions::default()),
        expected,
        "a page renders at its natural size"
    );
}
