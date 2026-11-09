//! A full rewrite that prunes resources cuts embedded font programs down to
//! the glyphs the document shows, and every glyph id stays.
//!
//! Each proof builds small documents in memory around fonts generated here:
//! a TrueType font of 24 glyphs with a `cmap`, a CID-keyed CFF font of 24
//! charstrings whose charset renumbers the CIDs, both drawn as pseudo-random
//! stars so unused glyphs cost bytes. The three kinds the pass handles are
//! each put through every proof: a simple TrueType font, a CIDFontType2
//! under Identity-H and a CIDFontType0 under Identity-H.
//!
//! No Dart battery can observe this: the batteries see a file that is
//! smaller and renders and extracts as before, and none of them can tell
//! which glyph a font program still holds, whether a glyph shown only in a
//! form or an appearance stream survived, or why a font was left whole.

use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::object::Object;
use crate::rendering::{PageRenderer, RenderOptions};
use std::collections::BTreeSet;

pub(super) const GLYPHS: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Simple,
    CidTrueType,
    CidCff,
}

const KINDS: [Kind; 3] = [Kind::Simple, Kind::CidTrueType, Kind::CidCff];

// ── generated fonts ──────────────────────────────────────────────────

struct Lcg(u32);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }
}

/// A star: `points` vertices around (500, 300), radii from a seeded run.
fn star(seed: u32, points: usize) -> Vec<(i32, i32)> {
    let mut lcg = Lcg(seed.wrapping_mul(2_654_435_761).wrapping_add(7));
    (0..points)
        .map(|k| {
            let angle = k as f64 * std::f64::consts::TAU / points as f64;
            let radius = 150.0 + (lcg.next() % 250) as f64;
            ((500.0 + radius * angle.cos()) as i32, (300.0 + radius * angle.sin()) as i32)
        })
        .collect()
}

fn be16(v: i64) -> [u8; 2] {
    (v as i16).to_be_bytes()
}

fn simple_glyph(points: &[(i32, i32)]) -> Vec<u8> {
    let xs = points.iter().map(|p| p.0);
    let ys = points.iter().map(|p| p.1);
    let mut out = Vec::new();
    out.extend(be16(1));
    out.extend(be16(xs.clone().min().unwrap() as i64));
    out.extend(be16(ys.clone().min().unwrap() as i64));
    out.extend(be16(xs.max().unwrap() as i64));
    out.extend(be16(ys.max().unwrap() as i64));
    out.extend(be16(points.len() as i64 - 1));
    out.extend(be16(0));
    out.extend(std::iter::repeat(0x01).take(points.len()));
    let mut at = 0;
    for p in points {
        out.extend(be16((p.0 - at) as i64));
        at = p.0;
    }
    let mut at = 0;
    for p in points {
        out.extend(be16((p.1 - at) as i64));
        at = p.1;
    }
    out
}

/// A composite glyph of `parts`, each placed at an offset.
fn composite_glyph(parts: &[u16]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(be16(-1));
    out.extend([0u8; 8]);
    for (i, part) in parts.iter().enumerate() {
        let more = if i + 1 < parts.len() { 0x20 } else { 0 };
        out.extend((0x0003u16 | more).to_be_bytes());
        out.extend(part.to_be_bytes());
        out.extend(be16(40 * i as i64));
        out.extend(be16(0));
    }
    out
}

fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn sfnt(tables: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 1, 0, 0];
    out.extend((tables.len() as u16).to_be_bytes());
    out.extend([0u8; 6]);
    out.resize(12 + 16 * tables.len(), 0);
    for (i, (tag, data)) in tables.iter().enumerate() {
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let at = 12 + 16 * i;
        out[at..at + 4].copy_from_slice(*tag);
        out[at + 4..at + 8].copy_from_slice(&checksum(data).to_be_bytes());
        let table_at = out.len() as u32;
        out[at + 8..at + 12].copy_from_slice(&table_at.to_be_bytes());
        out[at + 12..at + 16].copy_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(data);
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

/// What shapes a generated TrueType font.
#[derive(Clone, Copy)]
pub(super) struct TrueType {
    long_loca: bool,
    /// Every glyph carries one trailing byte, so no length is even.
    odd_glyphs: bool,
    /// The last glyph is a composite of glyphs 1 and 2.
    composite: bool,
}

pub(super) const PLAIN: TrueType = TrueType { long_loca: false, odd_glyphs: false, composite: false };

pub(super) fn truetype(shape: TrueType) -> Vec<u8> {
    let mut glyphs: Vec<Vec<u8>> = vec![simple_glyph(&[(50, 0), (450, 0), (450, 700), (50, 700)])];
    for gid in 1..GLYPHS as u32 {
        glyphs.push(simple_glyph(&star(gid, 40)));
    }
    if shape.composite {
        *glyphs.last_mut().unwrap() = composite_glyph(&[1, 2]);
    }
    if shape.odd_glyphs {
        for glyph in &mut glyphs {
            glyph.push(0);
        }
    }
    let (mut glyf, mut offsets) = (Vec::new(), Vec::new());
    for glyph in &glyphs {
        offsets.push(glyf.len());
        glyf.extend_from_slice(glyph);
        if !shape.odd_glyphs && glyf.len() % 2 == 1 {
            glyf.push(0);
        }
    }
    offsets.push(glyf.len());
    let mut loca = Vec::new();
    for &offset in &offsets {
        if shape.long_loca {
            loca.extend((offset as u32).to_be_bytes());
        } else {
            loca.extend(((offset / 2) as u16).to_be_bytes());
        }
    }

    let mut head = Vec::new();
    head.extend([0, 1, 0, 0, 0, 1, 0, 0]);
    head.extend([0u8; 4]);
    head.extend(0x5F0F_3CF5u32.to_be_bytes());
    head.extend([0, 0]);
    head.extend(1000u16.to_be_bytes());
    head.extend([0u8; 16]);
    head.extend(be16(0));
    head.extend(be16(0));
    head.extend(be16(1000));
    head.extend(be16(1000));
    head.extend([0, 0, 0, 8, 0, 2]);
    head.extend(be16(shape.long_loca as i64));
    head.extend(be16(0));

    let mut hhea = vec![0, 1, 0, 0];
    for v in [800, -200, 0, 600, 0, 0, 1000, 1, 0, 0, 0, 0, 0, 0, 0] {
        hhea.extend(be16(v));
    }
    hhea.extend((GLYPHS as u16).to_be_bytes());

    let mut maxp = vec![0, 1, 0, 0];
    maxp.extend((GLYPHS as u16).to_be_bytes());
    for v in [40u16, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 2, 1] {
        maxp.extend(v.to_be_bytes());
    }

    let mut hmtx = Vec::new();
    for _ in 0..GLYPHS {
        hmtx.extend(600u16.to_be_bytes());
        hmtx.extend(be16(0));
    }

    // one format 4 segment: 'A'.. → glyphs 1..
    let (start, end) = (0x41u16, 0x41 + GLYPHS as u16 - 2);
    let mut sub = Vec::new();
    for v in [4u16, 0, 0, 4, 4, 1, 0, end, 0xFFFF, 0, start, 0xFFFF] {
        sub.extend(v.to_be_bytes());
    }
    sub.extend((1u16.wrapping_sub(start)).to_be_bytes());
    sub.extend(1u16.to_be_bytes());
    sub.extend([0u8; 4]);
    let length = sub.len() as u16;
    sub[2..4].copy_from_slice(&length.to_be_bytes());
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 1, 0, 0, 0, 12];
    cmap.extend(sub);

    let mut post = vec![0, 3, 0, 0];
    post.extend([0u8; 28]);

    sfnt(&[
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
        (b"post", post),
    ])
}

fn cff_number(value: i32) -> Vec<u8> {
    if (-107..=107).contains(&value) {
        vec![(value + 139) as u8]
    } else {
        let b = (value as i16).to_be_bytes();
        vec![28, b[0], b[1]]
    }
}

fn cff_index(items: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (items.len() as u16).to_be_bytes().to_vec();
    if items.is_empty() {
        return out;
    }
    // the narrowest offsets that hold the INDEX, as a font tool writes it
    let total = items.iter().map(|i| i.len()).sum::<usize>() + 1;
    let width = (1..=4).find(|w| total < 1usize << (8 * w)).unwrap();
    out.push(width as u8);
    let mut offset = 1u32;
    out.extend(&offset.to_be_bytes()[4 - width..]);
    for item in items {
        offset += item.len() as u32;
        out.extend(&offset.to_be_bytes()[4 - width..]);
    }
    for item in items {
        out.extend_from_slice(item);
    }
    out
}

fn cff_int(value: usize) -> Vec<u8> {
    let mut out = vec![29];
    out.extend((value as i32).to_be_bytes());
    out
}

/// The CID of glyph `gid` in the generated CFF font.
const fn cff_cid(gid: usize) -> usize {
    if gid == 0 {
        0
    } else {
        gid + 100
    }
}

/// A CID-keyed CFF font: charset renumbering, an FDArray with a private
/// dictionary and a local subroutine INDEX after it, all behind the
/// CharStrings INDEX.
fn cff() -> Vec<u8> {
    let strings: Vec<Vec<u8>> = (0..GLYPHS)
        .map(|gid| {
            let points = if gid == 0 {
                vec![(50, 0), (450, 0), (450, 700), (50, 700)]
            } else {
                star(gid as u32 + 1000, 40)
            };
            let mut out = Vec::new();
            let mut at = (0, 0);
            for (i, p) in points.iter().enumerate() {
                out.extend(cff_number(p.0 - at.0));
                out.extend(cff_number(p.1 - at.1));
                out.push(if i == 0 { 21 } else { 5 });
                at = *p;
            }
            out.push(14);
            out
        })
        .collect();
    let header = vec![1, 0, 4, 4];
    let names = cff_index(&[b"TestCID".to_vec()]);
    let text = cff_index(&[b"Adobe".to_vec(), b"Identity".to_vec()]);
    let globals = cff_index(&[]);

    let top = |charset: usize, charstrings: usize, array: usize, select: usize| -> Vec<u8> {
        let mut dict = Vec::new();
        for v in [391, 392, 0] {
            dict.extend(cff_int(v));
        }
        dict.extend([12, 30]);
        dict.extend(cff_int(GLYPHS));
        dict.extend([12, 34]);
        for (value, op) in [(charset, vec![15]), (charstrings, vec![17]), (array, vec![12, 36]), (select, vec![12, 37])] {
            dict.extend(cff_int(value));
            dict.extend(op);
        }
        cff_index(&[dict])
    };
    let head_len = header.len() + names.len() + top(0, 0, 0, 0).len() + text.len() + globals.len();

    let mut charset = vec![0];
    for gid in 1..GLYPHS {
        charset.extend((cff_cid(gid) as u16).to_be_bytes());
    }
    let mut select = vec![0];
    select.extend(std::iter::repeat(0).take(GLYPHS));
    let charstrings = cff_index(&strings);
    let mut private = Vec::new();
    private.extend(cff_int(600));
    private.push(20);
    private.extend(cff_int(0));
    private.push(21);
    private.extend(cff_int(private.len() + 6));
    private.push(19);
    let subrs = cff_index(&[vec![11]]);
    let font_dict = |private_at: usize| -> Vec<u8> {
        let mut dict = cff_int(private.len());
        dict.extend(cff_int(private_at));
        dict.push(18);
        cff_index(&[dict])
    };

    let charset_at = head_len;
    let select_at = charset_at + charset.len();
    let charstrings_at = select_at + select.len();
    let array_at = charstrings_at + charstrings.len();
    let private_at = array_at + font_dict(0).len();

    let mut out = header;
    out.extend(names);
    out.extend(top(charset_at, charstrings_at, array_at, select_at));
    out.extend(text);
    out.extend(globals);
    assert_eq!(out.len(), head_len);
    out.extend(charset);
    out.extend(select);
    out.extend(charstrings);
    out.extend(font_dict(private_at));
    out.extend(private);
    out.extend(subrs);
    out
}

fn program(kind: Kind, shape: TrueType) -> Vec<u8> {
    match kind {
        Kind::Simple | Kind::CidTrueType => truetype(shape),
        Kind::CidCff => cff(),
    }
}

// ── documents ────────────────────────────────────────────────────────

/// What a document shows, as 1-based glyph numbers ('A' is 1).
#[derive(Clone, Default)]
struct Shows {
    page: Vec<usize>,
    form: Vec<usize>,
    annotation: Vec<usize>,
    /// An AcroForm whose `/DR` holds the font and a text field using it.
    form_field: bool,
}

fn plain(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

/// The string operand that shows `glyphs` in a font of `kind`.
fn shown(kind: Kind, glyphs: &[usize]) -> String {
    match kind {
        Kind::Simple => format!("({})", glyphs.iter().map(|g| (b'A' + *g as u8 - 1) as char).collect::<String>()),
        Kind::CidTrueType => format!("<{}>", glyphs.iter().map(|g| format!("{g:04X}")).collect::<String>()),
        Kind::CidCff => format!("<{}>", glyphs.iter().map(|g| format!("{:04X}", cff_cid(*g))).collect::<String>()),
    }
}

fn pdf(kind: Kind, font_program: Vec<u8>, shows: &Shows) -> Vec<u8> {
    let file_dict = match kind {
        Kind::CidCff => "/Subtype /CIDFontType0C".to_string(),
        _ => format!("/Length1 {}", font_program.len()),
    };
    let file_key = if kind == Kind::CidCff { "FontFile3" } else { "FontFile2" };
    let first = if kind == Kind::CidCff { cff_cid(1) } else { 1 };
    let mut unicode = format!(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CMapName /T def /CMapType 2 def\n\
         1 begincodespacerange <0000> <FFFF> endcodespacerange\n\
         1 beginbfrange <{first:04X}> <{:04X}> <0041> endbfrange\nendcmap end end\n",
        first + GLYPHS - 2
    );
    if kind == Kind::Simple {
        unicode.clear();
    }
    let font = match kind {
        Kind::Simple => format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /TestFont /FirstChar 65 /LastChar {} \
             /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor 6 0 R >>",
            65 + GLYPHS - 2,
            vec!["600"; GLYPHS - 1].join(" ")
        ),
        _ => "<< /Type /Font /Subtype /Type0 /BaseFont /TestFont /Encoding /Identity-H \
              /DescendantFonts [10 0 R] /ToUnicode 11 0 R >>"
            .to_string(),
    };
    let descendant = match kind {
        Kind::CidTrueType => "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /TestFont /CIDToGIDMap /Identity \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor 6 0 R /DW 600 >>",
        Kind::CidCff => "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /TestFont \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor 6 0 R /DW 600 >>",
        Kind::Simple => "<< >>",
    };
    let content = |glyphs: &[usize], y: u32| {
        format!("BT /F1 36 Tf 10 {y} Td {} Tj ET\n", shown(kind, glyphs))
    };
    let mut annots = vec!["9 0 R"];
    if shows.form_field {
        annots.push("13 0 R");
    }
    let acroform = if shows.form_field {
        "/AcroForm << /DR << /Font << /F1 5 0 R >> >> /DA (/F1 0 Tf 0 g) /Fields [13 0 R] >>"
    } else {
        ""
    };
    let page_text = content(&shows.page, 70) + "/Fm1 Do\n";
    let mut objects = vec![
        plain(1, &format!("<< /Type /Catalog /Pages 2 0 R {acroform} >>")),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Contents 4 0 R \
                 /Resources << /Font << /F1 5 0 R >> /XObject << /Fm1 8 0 R >> >> /Annots [{}] >>",
                annots.join(" ")
            ),
        ),
        stream_object(4, "", page_text.as_bytes()),
        plain(5, &font),
        plain(
            6,
            &format!(
                "<< /Type /FontDescriptor /FontName /TestFont /Flags 32 /FontBBox [0 0 1000 1000] \
                 /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /{file_key} 7 0 R >>"
            ),
        ),
        stream_object(7, &file_dict, &font_program),
        stream_object(
            8,
            "/Type /XObject /Subtype /Form /BBox [0 0 200 120] /Resources << /Font << /F1 5 0 R >> >>",
            content(&shows.form, 20).as_bytes(),
        ),
        plain(9, "<< /Type /Annot /Subtype /Square /Rect [100 10 190 60] /AP << /N 12 0 R >> >>"),
        plain(10, descendant),
        stream_object(11, "", unicode.as_bytes()),
        stream_object(
            12,
            "/Type /XObject /Subtype /Form /BBox [0 0 90 50] /Resources << /Font << /F1 5 0 R >> >>",
            content(&shows.annotation, 10).as_bytes(),
        ),
    ];
    if shows.form_field {
        objects.push(plain(
            13,
            "<< /Type /Annot /Subtype /Widget /FT /Tx /T (field) /Rect [100 70 190 110] \
             /DA (/F1 12 Tf 0 g) /AP << /N 14 0 R >> >>",
        ));
        objects.push(stream_object(
            14,
            "/Type /XObject /Subtype /Form /BBox [0 0 90 40] /Resources << /Font << /F1 5 0 R >> >>",
            content(&[1], 5).as_bytes(),
        ));
    }
    pdf_from_objects(&objects)
}

fn pruning() -> SaveOptions {
    SaveOptions { prune_resources: true, ..SaveOptions::full_rewrite() }
}

fn plain_save() -> SaveOptions {
    SaveOptions { prune_resources: false, ..SaveOptions::full_rewrite() }
}

fn save(source: &[u8], options: SaveOptions) -> Vec<u8> {
    DocumentEditor::from_bytes(source.to_vec()).unwrap().save_to_bytes_with_options(options).unwrap()
}

/// The raster of page 0 at 100 dpi.
fn render(bytes: &[u8]) -> Vec<u8> {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    let mut renderer = PageRenderer::new(RenderOptions::with_dpi(100));
    let mut raster = Vec::new();
    let _ = renderer
        .render_bands(&doc, 0, &|_, height| 0..height, &CancelCheck::none(), &mut |_, band| {
            raster.extend_from_slice(band.data());
            Ok(())
        })
        .expect("renders");
    raster
}

fn text(bytes: &[u8]) -> String {
    PdfDocument::from_bytes(bytes.to_vec()).unwrap().extract_text(0).unwrap()
}

/// The font program of the document: its decoded bytes and its stored length.
fn stored(bytes: &[u8]) -> (Vec<u8>, u64) {
    let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    for id in doc.all_object_ids() {
        let Ok(object @ Object::Stream { .. }) = doc.load_object(crate::object::ObjectRef::new(id, 0)) else {
            continue;
        };
        let Object::Stream { dict, body } = &object else { continue };
        let is_font = dict.contains_key("Length1")
            || dict.get("Subtype").and_then(|s| s.as_name()) == Some("CIDFontType0C");
        if is_font {
            return (object.decode_stream_data().unwrap(), body.len());
        }
    }
    panic!("no font program in the document");
}

/// Whether glyph `gid` of a font program has an outline.
fn has_outline(kind: Kind, program: &[u8], gid: usize) -> bool {
    struct Sink;
    impl ttf_parser::OutlineBuilder for Sink {
        fn move_to(&mut self, _: f32, _: f32) {}
        fn line_to(&mut self, _: f32, _: f32) {}
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {}
        fn close(&mut self) {}
    }
    match kind {
        Kind::CidCff => ttf_parser::cff::Table::parse(program)
            .is_some_and(|t| t.outline(ttf_parser::GlyphId(gid as u16), &mut Sink).is_ok()),
        _ => ttf_parser::Face::parse(program, 0)
            .unwrap()
            .outline_glyph(ttf_parser::GlyphId(gid as u16), &mut Sink)
            .is_some(),
    }
}

fn basic() -> Shows {
    Shows { page: vec![1, 3, 5], form: vec![11, 12], annotation: vec![21, 22], form_field: false }
}

// ── proofs ───────────────────────────────────────────────────────────

/// Proves the pass makes the file smaller, keeps the raster of the page at
/// 100 dpi byte for byte (RMSE 0) and keeps the extracted text, for each of
/// the three kinds. No Dart-level test can observe it: one sees the bytes
/// in and out, and cannot tell a saving that comes from the font program
/// from one the rest of the save made; this compares against the same save
/// with the pass off, and reads the program back.
#[test]
fn smaller_pixel_identical_text_equal() {
    for kind in KINDS {
        let source = pdf(kind, program(kind, PLAIN), &basic());
        let cut = save(&source, pruning());
        let whole = save(&source, plain_save());
        assert!(cut.len() < whole.len(), "{kind:?}: {} bytes is not under {}", cut.len(), whole.len());
        let (before, after) = (render(&source), render(&cut));
        assert!(
            before.iter().any(|&b| b != before[0]),
            "{kind:?}: the page renders blank, so equal rasters prove nothing"
        );
        assert!(before == after, "{kind:?}: the cut document does not render as the source does");
        assert_eq!(text(&source), text(&cut), "{kind:?}: the extracted text changed");
        assert!(!text(&cut).is_empty(), "{kind:?}: nothing extracted, the proof would hold for any font");
        let (program_cut, _) = stored(&cut);
        let (program_whole, _) = stored(&whole);
        assert!(program_cut.len() < program_whole.len(), "{kind:?}: the program was not cut");
    }
}

/// Proves a glyph shown only inside a form XObject or an annotation
/// appearance stream survives, and that every glyph no content shows is
/// gone. No Dart-level test can observe it: the answer is which outlines
/// the font program in the output still holds, and a Dart test has only
/// the rendered page, which would pass if the pass kept everything.
#[test]
fn glyphs_of_forms_and_annotations_survive() {
    for kind in KINDS {
        let source = pdf(kind, program(kind, PLAIN), &basic());
        let (program_cut, _) = stored(&save(&source, pruning()));
        let shown_gids: BTreeSet<usize> = [1, 3, 5, 11, 12, 21, 22].into_iter().collect();
        for gid in 1..GLYPHS {
            assert_eq!(
                has_outline(kind, &program_cut, gid),
                shown_gids.contains(&gid),
                "{kind:?}: glyph {gid}"
            );
        }
    }
}

/// Proves a composite glyph keeps its components: the only glyph shown is
/// the composite, its components are shown by nothing, and it renders as it
/// did. No Dart-level test can observe it: the components are glyph ids no
/// content names, so only the program's own `glyf` says they are still there.
#[test]
fn composite_components_stay() {
    for kind in [Kind::Simple, Kind::CidTrueType] {
        let shape = TrueType { composite: true, ..PLAIN };
        let shows = Shows { page: vec![GLYPHS - 1], ..Shows::default() };
        let source = pdf(kind, program(kind, shape), &shows);
        let cut = save(&source, pruning());
        let (program_cut, _) = stored(&cut);
        assert!(has_outline(kind, &program_cut, GLYPHS - 1));
        assert!(has_outline(kind, &program_cut, 1), "{kind:?}: a component of the composite is gone");
        assert!(has_outline(kind, &program_cut, 2), "{kind:?}: a component of the composite is gone");
        assert!(!has_outline(kind, &program_cut, 3));
        assert!(render(&source) == render(&cut), "{kind:?}: the composite renders differently");
    }
}

/// Proves a font with a long `loca` (32-bit offsets) is cut the same way
/// and renders as before. No Dart-level test can observe it: which `loca`
/// format a program uses is not visible outside the program.
#[test]
fn long_loca_is_cut() {
    for kind in [Kind::Simple, Kind::CidTrueType] {
        let shape = TrueType { long_loca: true, ..PLAIN };
        let source = pdf(kind, program(kind, shape), &basic());
        let cut = save(&source, pruning());
        assert!(cut.len() < save(&source, plain_save()).len());
        assert!(render(&source) == render(&cut));
        assert!(!has_outline(kind, &stored(&cut).0, 7));
    }
}

/// Proves a font a form field can use (the AcroForm's `/DR` holds it, a
/// widget's appearance uses it) is not subset: the field's appearance may
/// be regenerated with any glyph. The program is the source's, byte for
/// byte. No Dart-level test can observe it: a refused cut leaves the
/// rendered page and the text as a cut would, so only the program's bytes
/// tell the two apart.
#[test]
fn form_field_font_is_not_cut() {
    for kind in KINDS {
        let shows = Shows { form_field: true, ..basic() };
        let source = pdf(kind, program(kind, PLAIN), &shows);
        let (before, _) = stored(&source);
        let (after, _) = stored(&save(&source, pruning()));
        assert!(before == after, "{kind:?}: a font in the AcroForm's /DR was cut");
    }
}

/// Proves a font whose subset would not be smaller is kept as the source
/// has it: with every glyph shown there is nothing to drop and the output
/// is the very file the save writes without the pass; with every glyph of
/// odd length, the even padding the rebuild writes makes the subset larger,
/// and it is still not written. No Dart-level test can observe it: the file
/// is the same size whether a larger subset was written and then discarded
/// or never written, and only the bytes of the program separate a kept
/// original from a rewrite of it.
#[test]
fn a_subset_that_is_not_smaller_is_not_written() {
    use crate::host::font_subset::truetype;

    let everything = Shows { page: (1..GLYPHS).collect(), ..Shows::default() };
    for kind in KINDS {
        let source = pdf(kind, program(kind, PLAIN), &everything);
        let cut = save(&source, pruning());
        let whole = save(&source, plain_save());
        assert!(cut == whole, "{kind:?}: a program with nothing to drop was rewritten");
    }

    let odd = TrueType { odd_glyphs: true, ..PLAIN };
    let font = truetype(odd);
    let all: BTreeSet<u16> = (0..GLYPHS as u16).collect();
    let rebuilt = truetype::subset(&font, &all).expect("the font can be rewritten");
    let deflated = |bytes: &[u8]| {
        let mut out = Vec::new();
        let raw = crate::host::deflate::Encoding::Raw;
        crate::host::deflate::recode(std::io::Cursor::new(bytes), raw, 9, &mut out).unwrap();
        out.len()
    };
    assert!(deflated(&rebuilt) >= deflated(&font), "the odd-length font does not make the proof's point");
    for kind in [Kind::Simple, Kind::CidTrueType] {
        let source = pdf(kind, program(kind, odd), &everything);
        assert!(save(&source, pruning()) == save(&source, plain_save()), "{kind:?}: a larger subset was written");
    }
}

/// Proves a font with a `GSUB` table is left whole: a shaper may substitute
/// a glyph that no code names, so no glyph set can be complete. No
/// Dart-level test can observe it: the rendered page is the same whether
/// the font was cut or not, as long as the substitution does not fire.
#[test]
fn a_font_with_substitutions_is_not_cut() {
    let mut tables = Vec::new();
    let font = truetype(PLAIN);
    let count = u16::from_be_bytes([font[4], font[5]]) as usize;
    for i in 0..count {
        let at = 12 + 16 * i;
        let tag: [u8; 4] = font[at..at + 4].try_into().unwrap();
        let offset = u32::from_be_bytes(font[at + 8..at + 12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(font[at + 12..at + 16].try_into().unwrap()) as usize;
        tables.push((tag, font[offset..offset + length].to_vec()));
    }
    tables.push((*b"GSUB", vec![0, 1, 0, 0, 0, 10, 0, 10, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0]));
    tables.sort_by_key(|t| t.0);
    let refs: Vec<(&[u8; 4], Vec<u8>)> = tables.iter().map(|(tag, data)| (tag, data.clone())).collect();
    let with_gsub = sfnt(&refs);
    for kind in [Kind::Simple, Kind::CidTrueType] {
        let source = pdf(kind, with_gsub.clone(), &basic());
        let (after, _) = stored(&save(&source, pruning()));
        assert!(after == with_gsub, "{kind:?}: a font with GSUB was cut");
    }
}

/// Proves two identical programs merged into one by `merge_duplicates`
/// hold the glyphs of both fonts: the glyphs only the second font shows
/// survive. No Dart-level test can observe it: the merge and the cut are
/// each invisible in the rendered page, and only a program cut to one
/// font's glyphs, and a page that then paints the other's, tells them apart.
#[test]
fn merged_twins_hold_both_glyph_sets() {
    let kind = Kind::Simple;
    let font = program(kind, PLAIN);
    let source = twin_fonts(&font);
    let options = SaveOptions { merge_duplicates: true, ..pruning() };
    let cut = save(&source, options);
    let (program_cut, _) = stored(&cut);
    for gid in [1, 2, 3, 4] {
        assert!(has_outline(kind, &program_cut, gid), "glyph {gid} of one twin is gone");
    }
    assert!(!has_outline(kind, &program_cut, 9));
    assert!(render(&source) == render(&cut));
}

/// A page showing glyphs 1 and 2 with one font and 3 and 4 with another
/// whose program is the same bytes.
fn twin_fonts(font: &[u8]) -> Vec<u8> {
    let descriptor = |file: u32| {
        format!(
            "<< /Type /FontDescriptor /FontName /TestFont /Flags 32 /FontBBox [0 0 1000 1000] \
             /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 {file} 0 R >>"
        )
    };
    let font_dict = |descriptor: u32| {
        format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /TestFont /FirstChar 65 /LastChar {} \
             /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor {descriptor} 0 R >>",
            65 + GLYPHS - 2,
            vec!["600"; GLYPHS - 1].join(" ")
        )
    };
    let objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R /F2 6 0 R >> >> >>",
        ),
        stream_object(4, "", b"BT /F1 36 Tf 10 70 Td (AB) Tj /F2 36 Tf 10 20 Td (CD) Tj ET\n"),
        plain(5, &font_dict(7)),
        plain(6, &font_dict(8)),
        plain(7, &descriptor(9)),
        plain(8, &descriptor(10)),
        stream_object(9, &format!("/Length1 {}", font.len()), font),
        stream_object(10, &format!("/Length1 {}", font.len()), font),
    ];
    pdf_from_objects(&objects)
}

// ── fonts with GSUB ──────────────────────────────────────────────────

const DEJAVU: &[u8] = include_bytes!("../../fonts/assets/DejaVuSans.ttf");

/// The glyph id of `ch` in `face`.
fn gid_of(face: &ttf_parser::Face, ch: char) -> u16 {
    face.glyph_index(ch).unwrap_or_else(|| panic!("no glyph for {ch:?}")).0
}

/// A document showing `text` in `font` as a simple TrueType font
/// (`Kind::Simple`, WinAnsi codes) or as a CIDFontType2 under Identity-H
/// (`Kind::CidTrueType`, glyph ids as a producer shaped them: a ligature
/// character is shown by its own glyph id).
fn real_pdf(kind: Kind, font: &[u8], lines: &[&str]) -> Vec<u8> {
    let face = ttf_parser::Face::parse(font, 0).unwrap();
    let scale = 1000.0 / face.units_per_em() as f32;
    let width = |ch: char| (face.glyph_hor_advance(face.glyph_index(ch).unwrap()).unwrap() as f32 * scale).round() as u32;
    let mut content = String::new();
    let mut unicode = String::new();
    for (i, line) in lines.iter().enumerate() {
        let y = 70 - 40 * i as u32;
        let operand = match kind {
            Kind::Simple => format!("({line})"),
            _ => {
                let mut hex = String::new();
                for ch in line.chars() {
                    let gid = gid_of(&face, ch);
                    hex += &format!("{gid:04X}");
                    let decoded: String = match ch {
                        '\u{FB00}' => "ff".into(),
                        '\u{FB01}' => "fi".into(),
                        '\u{FB02}' => "fl".into(),
                        c => c.to_string(),
                    };
                    let utf16: String = decoded.encode_utf16().map(|u| format!("{u:04X}")).collect();
                    unicode += &format!("<{gid:04X}> <{utf16}>\n");
                }
                format!("<{hex}>")
            }
        };
        content += &format!("BT /F1 24 Tf 10 {y} Td {operand} Tj ET\n");
    }
    let to_unicode = format!(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n/CMapName /T def /CMapType 2 def\n\
         1 begincodespacerange <0000> <FFFF> endcodespacerange\n{} beginbfchar\n{unicode}endbfchar\n\
         endcmap end end\n",
        unicode.lines().count()
    );
    let widths = (32u8..=126).map(|c| width(c as char).to_string()).collect::<Vec<_>>().join(" ");
    let font_dict = match kind {
        Kind::Simple => format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /FirstChar 32 /LastChar 126 \
             /Widths [{widths}] /Encoding /WinAnsiEncoding /FontDescriptor 6 0 R >>"
        ),
        _ => "<< /Type /Font /Subtype /Type0 /BaseFont /DejaVuSans /Encoding /Identity-H \
              /DescendantFonts [10 0 R] /ToUnicode 11 0 R >>"
            .to_string(),
    };
    let mut objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 100] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
        ),
        stream_object(4, "", content.as_bytes()),
        plain(5, &font_dict),
        plain(
            6,
            "<< /Type /FontDescriptor /FontName /DejaVuSans /Flags 32 /FontBBox [-1000 -300 2000 1000] \
             /ItalicAngle 0 /Ascent 900 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>",
        ),
        stream_object(7, &format!("/Length1 {}", font.len()), font),
    ];
    if kind == Kind::CidTrueType {
        objects.push(plain(8, "null"));
        objects.push(plain(9, "null"));
        objects.push(plain(
            10,
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /DejaVuSans /CIDToGIDMap /Identity \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor 6 0 R /DW 600 >>",
        ));
        objects.push(stream_object(11, "", to_unicode.as_bytes()));
    }
    pdf_from_objects(&objects)
}

/// Proves a real font with a `GSUB` table (DejaVu Sans, `liga` among its
/// features) is left whole, as a simple TrueType font and as a CIDFontType2
/// under Identity-H, for text whose letters a shaper joins ("office",
/// "fluffy"). The renderer shapes the decoded text of a TrueType font with
/// its GSUB, so "fi" paints the `fi` ligature glyph although the content
/// shows only `f` and `i`; a program cut to the shown glyphs lost that
/// outline and the page rendered differently (measured with the skip
/// removed, at 100 dpi, in both kinds). No Dart-level test can observe it:
/// the page renders the same whether the font was left whole or never
/// reached by the pass, and only the program's bytes tell them apart.
#[test]
fn a_real_gsub_font_is_left_whole() {
    let shaped = ["o\u{FB00}ice \u{FB02}u\u{FB00}y", "a\u{FB01}x \u{FB00}\u{FB01} \u{FB02}ow"];
    let letters = ["office fluffy", "afix ffi flow"];
    for (kind, lines) in [(Kind::CidTrueType, &shaped), (Kind::Simple, &letters)] {
        let source = real_pdf(kind, DEJAVU, lines);
        let cut = save(&source, pruning());
        let (before, _) = stored(&source);
        let (after, _) = stored(&cut);
        assert!(before == after, "{kind:?}: a font with GSUB was cut");
        let blank = render(&source);
        assert!(blank.iter().any(|&b| b != blank[0]), "{kind:?}: the page renders blank");
        assert!(blank == render(&cut), "{kind:?}: the page renders differently");
        assert_eq!(text(&source), text(&cut), "{kind:?}: the extracted text changed");
    }
}

/// A page showing glyphs 1 and 3 ('A' and 'C') in a simple TrueType font
/// held by object 5, laid out by the caller: `pages_extra` and `page_extra`
/// are entries of the page tree node and the page, `content` the page's
/// content, `more` the objects from 8 on.
fn loose_pdf(pages_extra: &str, page_extra: &str, content: &str, more: Vec<Vec<u8>>) -> Vec<u8> {
    let font_program = program(Kind::Simple, PLAIN);
    let mut objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, &format!("<< /Type /Pages /Kids [3 0 R] /Count 1 {pages_extra} >>")),
        plain(
            3,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 120] /Contents 4 0 R {page_extra} >>"
            ),
        ),
        stream_object(4, "", content.as_bytes()),
        plain(
            5,
            &format!(
                "<< /Type /Font /Subtype /TrueType /BaseFont /TestFont /FirstChar 65 /LastChar {} \
                 /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor 6 0 R >>",
                65 + GLYPHS - 2,
                vec!["600"; GLYPHS - 1].join(" ")
            ),
        ),
        plain(
            6,
            "<< /Type /FontDescriptor /FontName /TestFont /Flags 32 /FontBBox [0 0 1000 1000] \
             /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>",
        ),
        stream_object(7, &format!("/Length1 {}", font_program.len()), &font_program),
    ];
    objects.extend(more);
    pdf_from_objects(&objects)
}

const SHOWS_AC: &str = "BT /F1 36 Tf 10 70 Td (AC) Tj ET\n";

/// A form that shows glyph 15 ('O') in the font of `loose_pdf`.
fn form_of_o(id: u32) -> Vec<u8> {
    stream_object(
        id,
        "/Type /XObject /Subtype /Form /BBox [0 0 200 120] /Resources << /Font << /F1 5 0 R >> >>",
        b"BT /F1 36 Tf 10 20 Td (O) Tj ET",
    )
}

/// Proves a page that inherits its resources from the page tree node is cut
/// to the glyphs it shows, and keeps its fonts: the glyph the page shows
/// is in the program and the others are not, and the page renders as it did.
/// No Dart-level test can observe it: it needs the program's glyphs, which
/// no rendering of the page tells apart from a program that was never cut.
#[test]
fn a_page_with_inherited_resources_is_cut() {
    let source = loose_pdf("/Resources << /Font << /F1 5 0 R >> >>", "", SHOWS_AC, Vec::new());
    let cut = save(&source, pruning());
    let (program_cut, _) = stored(&cut);
    for gid in 1..GLYPHS {
        assert_eq!(has_outline(Kind::Simple, &program_cut, gid), gid == 1 || gid == 3, "glyph {gid}");
    }
    let before = render(&source);
    assert!(before.iter().any(|&b| b != before[0]), "the page renders blank");
    assert!(before == render(&cut), "the cut document renders differently");
}

/// Proves a form no content names, in a resource dictionary the prune leaves
/// whole (an object outside the page also points at it), keeps its glyphs:
/// the form stays in the file, and a viewer that paints it needs them. No
/// Dart-level test can observe it: the answer is which outlines the program
/// holds, and the page renders the same without them.
#[test]
fn a_form_no_name_reaches_keeps_its_glyphs_where_the_dictionary_stays() {
    let resources = plain(8, "<< /Font << /F1 5 0 R >> /XObject << /Fm1 9 0 R >> >>");
    let strays = plain(10, "<< /Resources 8 0 R >>");
    let source = loose_pdf("", "/Resources 8 0 R", SHOWS_AC, vec![resources, form_of_o(9), strays]);
    let cut = save(&source, pruning());
    assert!(super::support::contains(&cut, b"/Subtype /Form"), "the form was dropped, the proof holds for nothing");
    let (program_cut, _) = stored(&cut);
    for gid in 1..GLYPHS {
        assert_eq!(
            has_outline(Kind::Simple, &program_cut, gid),
            matches!(gid, 1 | 3 | 15),
            "glyph {gid}"
        );
    }
}

/// Proves the same for a resource dictionary held directly in the page, which
/// the prune leaves whole because a name of the content is a key that no
/// operator used (`/Fm1 MP` is a marked point, not a paint of the form).
/// No Dart-level test can observe it: see the proof above.
#[test]
fn a_form_in_a_direct_dictionary_the_prune_leaves_whole_keeps_its_glyphs() {
    let content = format!("{SHOWS_AC}/Fm1 MP\n");
    let page = "/Resources << /Font << /F1 5 0 R >> /XObject << /Fm1 8 0 R >> >>";
    let source = loose_pdf("", page, &content, vec![form_of_o(8)]);
    let cut = save(&source, pruning());
    assert!(super::support::contains(&cut, b"/Subtype /Form"), "the form was dropped, the proof holds for nothing");
    let (program_cut, _) = stored(&cut);
    for gid in 1..GLYPHS {
        assert_eq!(
            has_outline(Kind::Simple, &program_cut, gid),
            matches!(gid, 1 | 3 | 15),
            "glyph {gid}"
        );
    }
}

/// Proves a page the save edits while it writes keeps its font program as
/// the source has it: the edit may show any glyph the program holds. No
/// Dart-level test can observe it: the program is the same size on screen
/// whether it was cut or kept, and only its bytes tell the two apart.
#[test]
fn a_page_the_save_edits_keeps_its_fonts_whole() {
    let source = loose_pdf("", "/Resources << /Font << /F1 5 0 R >> >>", SHOWS_AC, Vec::new());
    let mut editor = DocumentEditor::from_bytes(source.clone()).unwrap();
    editor.erase_region(0, [150.0, 100.0, 190.0, 110.0]).unwrap();
    let kept = editor.save_to_bytes_with_options(pruning()).unwrap();
    assert!(stored(&source).0 == stored(&kept).0, "the font of an edited page was cut");
    let untouched = save(&source, pruning());
    assert!(stored(&untouched).0.len() < stored(&source).0.len(), "the unedited page was not cut");
}

/// Proves the font a form field's appearance lists keeps its program whole
/// although the appearance shows nothing in it and no AcroForm `/DR` holds
/// it: the field may be regenerated with any glyph. No Dart-level test can
/// observe it: see `form_field_font_is_not_cut`.
#[test]
fn a_font_a_widget_appearance_lists_is_not_cut() {
    let widget = plain(
        8,
        "<< /Type /Annot /Subtype /Widget /FT /Tx /T (field) /Rect [100 70 190 110] /AP << /N 9 0 R >> >>",
    );
    let appearance = stream_object(
        9,
        "/Type /XObject /Subtype /Form /BBox [0 0 90 40] /Resources << /Font << /F1 5 0 R >> >>",
        b"q Q",
    );
    let source = loose_pdf(
        "",
        "/Resources << /Font << /F1 5 0 R >> >> /Annots [8 0 R]",
        SHOWS_AC,
        vec![widget, appearance],
    );
    let kept = save(&source, pruning());
    assert!(stored(&source).0 == stored(&kept).0, "the font a widget lists was cut");
}
