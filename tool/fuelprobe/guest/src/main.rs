//! Fuel probe guest: one engine operation on one generated fixture, run
//! under `CancelCheck::probe()` inside wasmtime.
//!
//! `fuelprobe-guest --list` prints the cases, one `op fixture` per line;
//! `fuelprobe-guest <op> <fixture>` builds the fixture and runs the op. Every
//! poll of the check calls the host's `fuelprobe.poll`. The guest polls once
//! after the fixture is built and its documents are open (the host starts
//! counting there) and once when the op has returned (the host ends there), so
//! the work between the last poll the engine made and the end of the op is a
//! gap like any other.

mod gen;

use gen::Shell;
use pdf_oxide::cancel::CancelCheck;
use pdf_oxide::document::PdfDocument;
use pdf_oxide::editor::{DocumentEditor, SaveOptions};
use pdf_oxide::host::dispatch;
use pdf_oxide::host::positioned_write::CountingWriter;
use pdf_oxide::host::probe;

type Failure = String;

/// The cases, in the order the host runs them.
const CASES: &[(&str, &str)] = &[
    ("open", "damaged"),
        ("extractAll", "pages"),
    ("extractAll", "text"),
    ("search", "pages"),
    ("search", "text"),
    ("render", "paths"),
    ("render", "bigpage"),
    ("render", "pages"),
    ("extractImages", "images"),
    ("compress", "pages"),
    ("compress", "dups"),
    ("compress", "fonts"),
    ("compress", "images"),
    ("reduceImages", "images"),
    ("validatePdfA", "pages"),
    ("validatePdfA", "tagged"),
    ("convertToPdfA", "pages"),
    ("applyRedactions", "regions"),
    ("redact", "text"),
    ("flattenForms", "form"),
    ("flattenAnnotations", "annotations"),
    ("merge", "pages"),
    ("mergeSelected", "pages"),
    ("split", "pages"),
    ("saveFull", "pages"),
    ("saveFull", "tagged"),
    ("saveIncremental", "pages"),
    ("classifyPages", "pages"),
    ("bookmarkRanges", "outline"),
    ("addStamp", "pages"),
    ("pageImages", "long"),
    ("render", "knockout"),
    ("render", "type3"),
    ("render", "inlineImage"),
    ("render", "knockoutPaints"),
];

/// The fixtures' sizes, each chosen so the op does well over 10^9 units of
/// work (the host fails a row that does less).
mod size {
    pub const DAMAGED_PAGES: usize = 60_000;
    pub const PAGES: usize = 3_000;
    pub const PAGE_LINES: usize = 12;
    /// Pages of the extract, search, classify and conversion cases: a page of
    /// text costs those far more than a save does.
    pub const TEXT_PAGES: usize = 400;
    pub const SAVE_PAGES: usize = 4_000;
    pub const MERGE_PAGES: usize = 8_000;
    pub const INCREMENTAL_PAGES: usize = 9_000;
    /// A text page the extractor reads whole: its layout passes cost more than a line.
    pub const TEXT_LINES: usize = 1_000;
    pub const TEXT_PAGES_OF_LINES: usize = 20;
    pub const CONVERT_PAGES: usize = 700;
    pub const REDACT_LINES: usize = 20_000;
    pub const PATHS: usize = 3_000;
    pub const BIGPAGE: u32 = 1_500;
    pub const BIGPAGE_PATHS: usize = 2_000;
    pub const RENDER_PAGES: usize = 50;
    pub const EXTRACT_IMAGES: usize = 600;
    pub const IMAGES: usize = 6_000;
    pub const IMAGE_SIDE: u32 = 32;
    pub const DUPS: usize = 6_000;
    pub const FONTS: usize = 600;
    pub const TAGGED: usize = 60_000;
    pub const TAGGED_SAVE: usize = 5_000;
    pub const REGIONS: usize = 600;
    pub const REGION_PAGES: usize = 300;
    pub const FORM: usize = 4_000;
    pub const ANNOTATIONS: usize = 10_000;
    pub const OUTLINE: usize = 500;
    /// A stamp costs a page parse and a staged annotation each.
    pub const STAMP_PAGES: usize = 1_000;
    /// Save, transform and restore triples on the one page of an image listing.
    pub const LONG_PAGE_TRIPLES: usize = 300_000;
    /// Triples of the form or glyph a render reads whole.
    pub const WHOLE_FORM_TRIPLES: usize = 300_000;
    /// Side of the one inline image of a render: several megabytes of data.
    pub const INLINE_IMAGE_SIDE: u32 = 5_000;
    /// Paint operators of a knockout group the render replays one by one.
    pub const KNOCKOUT_PAINTS: usize = 8_000;
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--list") {
        for (op, fixture) in CASES {
            println!("{op} {fixture}");
        }
        return;
    }
    let [op, fixture] = args.as_slice() else {
        eprintln!("usage: fuelprobe-guest --list | <op> <fixture>");
        std::process::exit(2);
    };
    if let Err(why) = run(op, fixture) {
        eprintln!("fuelprobe-guest: {op} {fixture}: {why}");
        std::process::exit(1);
    }
}

/// One poll of the probe check: the host's mark.
fn mark() {
    let _ = CancelCheck::probe().is_cancelled();
}

fn fail<E: std::fmt::Debug>(what: &'static str) -> impl FnOnce(E) -> Failure {
    move |e| format!("{what}: {e:?}")
}

fn doc(bytes: Vec<u8>) -> Result<PdfDocument, Failure> {
    probe::open_source(bytes, &CancelCheck::probe()).map_err(fail("open"))
}

fn editor(bytes: Vec<u8>) -> Result<DocumentEditor, Failure> {
    DocumentEditor::from_document(doc(bytes)?).map_err(fail("editor"))
}

fn sink() -> CountingWriter<std::io::Sink> {
    CountingWriter::new(std::io::sink())
}

fn run(op: &str, fixture: &str) -> Result<(), Failure> {
    let cancel = CancelCheck::probe();
    let _job = pdf_oxide::cancel::enter(&cancel);
    match (op, fixture) {
        ("open", "damaged") => {
            let mut shell = Shell::new();
            shell.fanout = 100;
            for i in 0..size::DAMAGED_PAGES {
                let content = gen::text_lines(2, i * 2);
                shell.page((612, 792), &content, "", "");
            }
            let bytes = shell.finish_damaged();
            mark();
            let mut document = doc(bytes)?;
            dispatch::open_document(&mut document).map_err(fail("openDocument"))?;
            mark();
        },
        ("extractAll", fixture) => {
            let bytes = text_or_pages(fixture)?;
            let mut document = doc(bytes)?;
            mark();
            dispatch::extract_text(&mut document, None, "plainText", &cancel)
                .map_err(fail("extract"))?;
            mark();
        },
        ("search", fixture) => {
            let bytes = text_or_pages(fixture)?;
            let mut document = doc(bytes)?;
            mark();
            dispatch::search_text(&mut document, "kappa", None, &cancel).map_err(fail("search"))?;
            mark();
        },
        ("render", "paths") => {
            let mut shell = Shell::new();
            shell.page((2000, 2000), &gen::paths(size::PATHS, 2000), "", "");
            render(shell.finish(), &[0], 100, &cancel)?;
        },
        ("render", "bigpage") => {
            let mut shell = Shell::new();
            let content = gen::paths(size::BIGPAGE_PATHS, size::BIGPAGE);
            shell.page((size::BIGPAGE, size::BIGPAGE), &content, "", "");
            render(shell.finish(), &[0], 72, &cancel)?;
        },
        ("render", "pages") => {
            let pages: Vec<usize> = (0..size::RENDER_PAGES).collect();
            let bytes = gen::pages(size::RENDER_PAGES, size::PAGE_LINES).finish();
            render(bytes, &pages, 72, &cancel)?;
        },
        ("extractImages", "images") => {
            let bytes = gen::images(size::EXTRACT_IMAGES, size::IMAGE_SIDE, 100, false).finish();
            let document = doc(bytes)?;
            let pages: Vec<usize> = (0..document.page_count().map_err(fail("count"))?).collect();
            mark();
            probe::extract_images_streamed(&document, &pages, &mut std::io::sink(), &cancel)
                .map_err(fail("extractImages"))?;
            mark();
        },
        ("compress", "pages") => compress(gen::pages(size::PAGES, size::PAGE_LINES).finish(), false, &cancel)?,
        ("compress", "dups") => {
            compress(gen::images(size::DUPS, size::IMAGE_SIDE, 100, true).finish(), false, &cancel)?
        },
        ("compress", "fonts") => compress(gen::fonts(size::FONTS).finish(), false, &cancel)?,
        ("compress", "images") => {
            compress(gen::images(size::IMAGES, size::IMAGE_SIDE, 100, false).finish(), true, &cancel)?
        },
        ("reduceImages", "images") => {
            let bytes = gen::images(size::IMAGES, size::IMAGE_SIDE, 100, false).finish();
            let mut editor = editor(bytes)?;
            mark();
            dispatch::edit_reduce_images(&mut editor, image_policy(), &cancel)
                .map_err(fail("reduceImages"))?;
            mark();
        },
        ("validatePdfA", "pages") => validate(gen::pages(size::PAGES, size::PAGE_LINES).finish(), &cancel)?,
        ("validatePdfA", "tagged") => validate(gen::tagged(size::TAGGED).finish(), &cancel)?,
        ("convertToPdfA", "pages") => {
            let mut editor = editor(gen::pages(size::CONVERT_PAGES, size::PAGE_LINES).finish())?;
            mark();
            dispatch::edit_convert_to_pdf_a(&mut editor, 2, &cancel).map_err(fail("convert"))?;
            mark();
        },
        ("applyRedactions", "regions") => {
            let bytes = gen::pages(size::REGION_PAGES, size::PAGE_LINES).finish();
            let mut editor = editor(bytes)?;
            for i in 0..size::REGIONS {
                let (x, y) = (72.0 + (i % 3) as f32 * 170.0, 100.0 + (i / 3 % 60) as f32 * 12.0);
                dispatch::edit_add_redaction(&mut editor, i % size::REGION_PAGES, [x, y, 60.0, 9.0], Some([0.0, 0.0, 0.0]))
                    .map_err(fail("addRedaction"))?;
            }
            mark();
            dispatch::edit_apply_redactions(&mut editor, &cancel).map_err(fail("applyRedactions"))?;
            mark();
        },
        ("redact", "text") => {
            let mut shell = Shell::new();
            shell.page((612, 792), &gen::text_lines(size::REDACT_LINES, 0), "", "");
            let mut editor = editor(shell.finish())?;
            dispatch::edit_add_redaction(&mut editor, 0, [60.0, 300.0, 400.0, 200.0], Some([0.0, 0.0, 0.0]))
                .map_err(fail("addRedaction"))?;
            mark();
            dispatch::edit_apply_redactions(&mut editor, &cancel).map_err(fail("applyRedactions"))?;
            mark();
        },
        ("flattenForms", "form") => {
            let mut editor = editor(gen::form(size::FORM).finish())?;
            mark();
            dispatch::edit_flatten_forms(&mut editor, None).map_err(fail("flattenForms"))?;
            save(&mut editor, &cancel)?;
            mark();
        },
        ("flattenAnnotations", "annotations") => {
            let mut editor = editor(gen::annotations(size::ANNOTATIONS).finish())?;
            mark();
            dispatch::edit_flatten_annotations(&mut editor, None).map_err(fail("flattenAnnotations"))?;
            save(&mut editor, &cancel)?;
            mark();
        },
        ("merge", "pages") => {
            let mut editor = editor(gen::pages(10, 2).finish())?;
            let other = gen::pages(size::MERGE_PAGES, size::PAGE_LINES).finish();
            mark();
            probe::merge_pages(&mut editor, other, None, &cancel).map_err(fail("merge"))?;
            mark();
        },
        ("mergeSelected", "pages") => {
            let mut editor = editor(gen::pages(10, 2).finish())?;
            let other = gen::pages(size::MERGE_PAGES, size::PAGE_LINES).finish();
            let pages: Vec<usize> = (0..size::MERGE_PAGES).step_by(2).collect();
            mark();
            probe::merge_pages(&mut editor, other, Some(&pages), &cancel).map_err(fail("merge"))?;
            mark();
        },
        ("split", "pages") => {
            let mut editor = editor(gen::pages(size::SAVE_PAGES, size::PAGE_LINES).finish())?;
            let pages: Vec<usize> = (0..size::SAVE_PAGES).step_by(2).collect();
            mark();
            probe::extract_pages(&mut editor, &pages, &mut sink(), &cancel).map_err(fail("split"))?;
            mark();
        },
        ("saveFull", fixture) => {
            let bytes = if fixture == "tagged" {
                gen::tagged(size::TAGGED_SAVE).finish()
            } else {
                gen::pages(size::SAVE_PAGES, size::PAGE_LINES).finish()
            };
            let mut editor = editor(bytes)?;
            mark();
            save(&mut editor, &cancel)?;
            mark();
        },
        ("saveIncremental", "pages") => {
            let mut editor = editor(gen::pages(size::INCREMENTAL_PAGES, size::PAGE_LINES).finish())?;
            for page in 0..size::INCREMENTAL_PAGES {
                editor.set_page_rotation(page, 90).map_err(fail("rotate"))?;
            }
            mark();
            probe::save_incremental(&mut editor, &mut sink(), &SaveOptions::incremental(), &cancel)
                .map_err(fail("saveIncremental"))?;
            mark();
        },
        ("classifyPages", "pages") => {
            let mut document = doc(gen::pages(size::TEXT_PAGES, size::PAGE_LINES).finish())?;
            mark();
            dispatch::classify_document(&mut document, &cancel).map_err(fail("classify"))?;
            mark();
        },
        ("bookmarkRanges", "outline") => {
            let mut document = doc(gen::outline(size::OUTLINE).finish())?;
            mark();
            dispatch::plan_split_by_bookmarks(&mut document, &cancel).map_err(fail("bookmarks"))?;
            mark();
        },
        ("addStamp", "pages") => {
            let mut editor = editor(gen::pages(size::STAMP_PAGES, size::PAGE_LINES).finish())?;
            let pages: Vec<usize> = (0..size::STAMP_PAGES).collect();
            mark();
            dispatch::edit_add_stamp(&mut editor, &pages, "Approved", 10.0, 10.0, 60.0, 20.0, 1.0)
                .map_err(fail("addStamp"))?;
            mark();
        },
        ("pageImages", "long") => {
            let mut shell = Shell::new();
            shell.page((612, 792), &gen::graphics(size::LONG_PAGE_TRIPLES), "", "");
            let mut editor = editor(shell.finish())?;
            mark();
            dispatch::edit_page_images(&mut editor, 0).map_err(fail("pageImages"))?;
            mark();
        },
        ("render", "knockout") => render(gen::knockout_form(size::WHOLE_FORM_TRIPLES).finish(), &[0], 72, &cancel)?,
        ("render", "type3") => render(gen::type3_glyph(size::WHOLE_FORM_TRIPLES).finish(), &[0], 72, &cancel)?,
        ("render", "inlineImage") => render(gen::inline_image(size::INLINE_IMAGE_SIDE).finish(), &[0], 72, &cancel)?,
        ("render", "knockoutPaints") => render(gen::knockout_paints(size::KNOCKOUT_PAINTS).finish(), &[0], 72, &cancel)?,
        _ => return Err("no such case".into()),
    }
    Ok(())
}

fn text_or_pages(fixture: &str) -> Result<Vec<u8>, Failure> {
    match fixture {
        "pages" => Ok(gen::pages(size::TEXT_PAGES, size::PAGE_LINES).finish()),
        "text" => {
            let mut shell = Shell::new();
            for i in 0..size::TEXT_PAGES_OF_LINES {
                shell.page((612, 792), &gen::text_lines(size::TEXT_LINES, i * size::TEXT_LINES), "", "");
            }
            Ok(shell.finish())
        },
        other => Err(format!("no fixture {other}")),
    }
}

fn render(bytes: Vec<u8>, pages: &[usize], dpi: u32, cancel: &CancelCheck) -> Result<(), Failure> {
    let document = doc(bytes)?;
    mark();
    probe::render_streamed(&document, pages, dpi, &mut std::io::sink(), cancel).map_err(fail("render"))?;
    mark();
    Ok(())
}

fn save(editor: &mut DocumentEditor, cancel: &CancelCheck) -> Result<(), Failure> {
    dispatch::edit_save_with_options(editor, &mut sink(), &SaveOptions::full_rewrite(), cancel)
        .map_err(fail("save"))
}

fn validate(bytes: Vec<u8>, cancel: &CancelCheck) -> Result<(), Failure> {
    let mut document = doc(bytes)?;
    mark();
    dispatch::validate_pdf_a(&mut document, 2, cancel).map_err(fail("validate"))?;
    mark();
    Ok(())
}

fn image_policy() -> dispatch::ImagePolicyArgs {
    dispatch::ImagePolicyArgs {
        color_ppi: Some(72.0),
        gray_ppi: Some(72.0),
        mono_ppi: Some(72.0),
        downsample_threshold: 1.5,
        jpeg_quality: 75,
        allow_lossy: true,
        convert_cmyk_to_rgb: false,
        min_pixels: 8,
        min_savings: 0.0,
        chroma: "auto".into(),
        recompress_jpeg: false,
    }
}

/// `compress`: the image policy (when `images`), then the save with every
/// size option on, the way the Dart call sends them.
fn compress(bytes: Vec<u8>, images: bool, cancel: &CancelCheck) -> Result<(), Failure> {
    let mut editor = editor(bytes)?;
    mark();
    if images {
        dispatch::edit_reduce_images(&mut editor, image_policy(), cancel).map_err(fail("reduceImages"))?;
    }
    let options = SaveOptions {
        merge_duplicates: true,
        prune_resources: true,
        ..SaveOptions::full_rewrite()
    };
    dispatch::edit_save_with_options(&mut editor, &mut sink(), &options, cancel).map_err(fail("save"))?;
    mark();
    Ok(())
}
