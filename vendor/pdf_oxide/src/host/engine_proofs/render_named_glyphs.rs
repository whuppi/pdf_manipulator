//! The renderer draws the glyphs the content names, never a GSUB substitution.
//!
//! DejaVu Sans carries a `liga` feature whose ligature glyphs (U+FB00..FB04)
//! a shaper substitutes for "ff", "fi", "fl", "ffi" and "ffl". A page that
//! shows the glyphs of "f" and "i" names those two glyphs; each proof renders
//! such a page next to a reference page that shows every glyph by a `Tj` of
//! its own, where no ligature can form, and compares the pixels.
//!
//! No Dart battery can observe this: it needs the rendering of two pages
//! compared pixel for pixel, and an oracle (a page whose glyphs are shown
//! one at a time) that no end-to-end call produces.

use super::support::{pdf_from_objects, stream_object};
use crate::document::PdfDocument;
use crate::rendering::{PageRenderer, RenderOptions};

const DEJAVU: &[u8] = include_bytes!("../../fonts/assets/DejaVuSans.ttf");

/// Every glyph advances 15 pt at 24 pt: the PDF's width is the position, and
/// the product is exact in binary, so a one-`Tj` page and a per-glyph page
/// place their glyphs on the same coordinates.
const WIDTH: u32 = 625;

fn plain(id: u32, body: &str) -> Vec<u8> {
    format!("{id} 0 obj\n{body}\nendobj\n").into_bytes()
}

fn face() -> ttf_parser::Face<'static> {
    ttf_parser::Face::parse(DEJAVU, 0).expect("DejaVu parses")
}

fn gid(ch: char) -> u16 {
    face().glyph_index(ch).unwrap_or_else(|| panic!("no glyph for {ch:?}")).0
}

fn hex(gids: &[u16]) -> String {
    gids.iter().map(|g| format!("{g:04X}")).collect()
}

fn descriptor(flags: u32) -> String {
    format!(
        "<< /Type /FontDescriptor /FontName /DejaVuSans /Flags {flags} \
         /FontBBox [-1000 -300 2000 1000] /ItalicAngle 0 /Ascent 900 /Descent -200 \
         /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>"
    )
}

fn page(content: &str, font: &str, extra: Vec<Vec<u8>>) -> Vec<u8> {
    let mut objects = vec![
        plain(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        plain(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        plain(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 140] /Contents 4 0 R \
             /Resources << /Font << /F1 5 0 R >> >> >>",
        ),
        stream_object(4, "", content.as_bytes()),
        plain(5, font),
    ];
    objects.extend(extra);
    pdf_from_objects(&objects)
}

/// A simple TrueType font (every width `WIDTH`) over `font`, in objects 5 to
/// 7: `flags` are its descriptor flags and `encoding` its /Encoding entry
/// (or nothing).
fn simple_page_of(content: &str, font: &[u8], flags: u32, encoding: &str) -> Vec<u8> {
    let widths = vec![WIDTH.to_string(); 95].join(" ");
    page(
        content,
        &format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /DejaVuSans /FirstChar 32 \
             /LastChar 126 /Widths [{widths}] {encoding} /FontDescriptor 6 0 R >>"
        ),
        vec![
            plain(6, &descriptor(flags)),
            stream_object(7, &format!("/Length1 {}", font.len()), font),
        ],
    )
}

/// The DejaVu program, non-symbolic, WinAnsi.
fn simple_page(content: &str) -> Vec<u8> {
    simple_page_of(content, DEJAVU, 32, "/Encoding /WinAnsiEncoding")
}

/// A CIDFontType2 under Identity-H (every width `WIDTH`) over the embedded
/// DejaVu program, with `map` as its /CIDToGIDMap entry; the stream, when
/// there is one, is object 8.
fn cid_page(content: &str, map: &str, map_stream: Option<Vec<u8>>) -> Vec<u8> {
    let mut extra = vec![
        plain(6, &descriptor(4)),
        stream_object(7, &format!("/Length1 {}", DEJAVU.len()), DEJAVU),
    ];
    if let Some(bytes) = map_stream {
        extra.push(stream_object(8, "", &bytes));
    }
    extra.push(plain(
        9,
        &format!(
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /DejaVuSans /CIDToGIDMap {map} \
             /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
             /FontDescriptor 6 0 R /DW {WIDTH} >>"
        ),
    ));
    page(
        content,
        "<< /Type /Font /Subtype /Type0 /BaseFont /DejaVuSans /Encoding /Identity-H \
         /DescendantFonts [9 0 R] >>",
        extra,
    )
}

/// The page as raw RGBA rows, and its width in pixels.
fn render_sized(bytes: Vec<u8>) -> (Vec<u8>, usize) {
    let doc = PdfDocument::from_bytes(bytes).expect("page parses");
    let image = PageRenderer::new(RenderOptions::with_dpi(100).as_raw())
        .render_page(&doc, 0)
        .expect("page renders");
    (image.data, image.width as usize)
}

fn render(bytes: Vec<u8>) -> Vec<u8> {
    render_sized(bytes).0
}

/// A render that painted anything: some pixel is not the white background.
fn inked(raster: &[u8]) -> bool {
    raster.iter().any(|&b| b != 255)
}

/// Lines of text at 24 pt, a line per `y`.
fn lines(shows: &[(u32, String)]) -> String {
    shows.iter().map(|(y, show)| format!("BT /F1 24 Tf 10 {y} Td {show} Tj ET\n")).collect()
}

/// The same lines with every glyph's code string shown by a `Tj` of its own.
fn per_glyph(shows: &[(u32, Vec<String>)]) -> String {
    shows
        .iter()
        .map(|(y, glyphs)| {
            let tjs: String = glyphs.iter().map(|g| format!("{g} Tj ")).collect();
            format!("BT /F1 24 Tf 10 {y} Td {tjs}ET\n")
        })
        .collect()
}

/// A simple TrueType page showing "fi", "fl" and "office" in one `Tj` each
/// renders the pixels of the page showing each letter by its own `Tj`.
/// The renderer used to decode the text to Unicode and shape it with the
/// font's GSUB, so the two letters "fi" painted the one `fi` ligature glyph
/// although the content names the glyphs of "f" and "i". No Dart-level test
/// can observe it: it needs the two renders compared pixel for pixel against
/// a per-glyph page that no end-to-end call produces.
#[test]
fn simple_truetype_draws_two_glyphs_for_f_and_i() {
    let words = ["fi", "fl", "office"];
    let ys = [100u32, 60, 20];
    let one_tj: Vec<(u32, String)> =
        words.iter().zip(ys).map(|(w, y)| (y, format!("({w})"))).collect();
    let each: Vec<(u32, Vec<String>)> = words
        .iter()
        .zip(ys)
        .map(|(w, y)| (y, w.chars().map(|c| format!("({c})")).collect()))
        .collect();

    let shown = render(simple_page(&lines(&one_tj)));
    let reference = render(simple_page(&per_glyph(&each)));
    assert!(inked(&reference), "the reference page is blank");
    assert!(inked(&shown), "the page is blank");
    assert!(
        shown == reference,
        "\"fi\", \"fl\" and \"office\" shown in one Tj paint other pixels than the same letters shown one by one"
    );
}

/// A CIDFontType2 page (Identity-H, Identity CIDToGIDMap) showing the glyph
/// ids of f, i, l, o, c, e renders the pixels of the page showing each glyph
/// id by its own `Tj`. The renderer used to route an Identity-H TrueType font
/// whose program has a Unicode cmap through its decoded Unicode and the
/// shaper, so the ids of "f" and "i" painted the `fi` ligature. No
/// Dart-level test can observe it: it needs the two renders compared pixel
/// for pixel against a per-glyph page that no end-to-end call produces.
#[test]
fn cid_truetype_draws_the_glyph_ids_it_names() {
    let ids: Vec<u16> = "filoce".chars().map(gid).collect();
    let shown = render(cid_page(&lines(&[(80, format!("<{}>", hex(&ids)))]), "/Identity", None));
    let each: Vec<String> = ids.iter().map(|&g| format!("<{}>", hex(&[g]))).collect();
    let reference = render(cid_page(&per_glyph(&[(80, each)]), "/Identity", None));
    assert!(inked(&reference), "the reference page is blank");
    assert!(inked(&shown), "the page is blank");
    assert!(shown == reference, "the glyph ids of f i l o c e paint other pixels than each shown alone");
}

/// A /CIDToGIDMap stream mapping CID 1 to the glyph of "A" and CID 2 to "B"
/// is followed: showing CIDs 1 and 2 renders as the glyph ids of "A" and "B"
/// shown under the Identity map, and not as glyphs 1 and 2. The renderer
/// used to send an embedded CIDFontType2 with a Unicode cmap around its
/// CIDToGIDMap. No Dart-level test can observe it: the map's effect is the
/// pixels, compared with a page that needs no map.
#[test]
fn a_cid_to_gid_map_stream_is_followed() {
    let (a, b) = (gid('A'), gid('B'));
    let mut map = vec![0u8; 6];
    map[2..4].copy_from_slice(&a.to_be_bytes());
    map[4..6].copy_from_slice(&b.to_be_bytes());

    let shown = render(cid_page(&lines(&[(80, "<00010002>".into())]), "8 0 R", Some(map)));
    let reference = render(cid_page(&lines(&[(80, format!("<{}>", hex(&[a, b])))]), "/Identity", None));
    let unmapped = render(cid_page(&lines(&[(80, "<00010002>".into())]), "/Identity", None));
    assert!(inked(&reference), "the reference page is blank");
    assert!(inked(&shown), "the page is blank");
    assert!(shown == reference, "CIDs 1 2 do not render as the glyphs of A B");
    assert!(shown != unmapped, "the CIDToGIDMap changed nothing");
}

/// Content that shows the ligature glyph id itself (the CID path, as a
/// producer that shaped the text writes it) draws the ligature: the page is
/// not blank and differs from the page showing the glyphs of "f" and "i".
/// Dropping the shaper must not drop a ligature the content names. No
/// Dart-level test can observe it: it needs the two renders compared pixel
/// for pixel.
#[test]
fn a_producer_shaped_ligature_still_draws() {
    let ligature = gid('\u{FB01}');
    assert_ne!(ligature, gid('f'));
    let shown = render(cid_page(&lines(&[(80, format!("<{}>", hex(&[ligature])))]), "/Identity", None));
    let letters = render(cid_page(
        &lines(&[(80, format!("<{}>", hex(&[gid('f'), gid('i')])))]),
        "/Identity",
        None,
    ));
    assert!(inked(&shown), "the ligature glyph id draws nothing");
    assert!(shown != letters, "the ligature glyph id paints the pixels of f and i");
}

/// `font` with its `cmap` table replaced by one (3,1) subtable (format 6)
/// that maps the codes 32..=126 to the glyphs of the same Unicode values.
fn only_a_31_cmap(font: &[u8]) -> Vec<u8> {
    let u16_at = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]) as usize;
    let u32_at = |at: usize| u32::from_be_bytes(font[at..at + 4].try_into().unwrap()) as usize;
    let mut subtable = Vec::new();
    for value in [6u16, 10 + 2 * 95, 0, 32, 95] {
        subtable.extend_from_slice(&value.to_be_bytes());
    }
    for code in 32u8..=126 {
        subtable.extend_from_slice(&gid(code as char).to_be_bytes());
    }
    let mut cmap = Vec::new();
    for value in [0u16, 1, 3, 1] {
        cmap.extend_from_slice(&value.to_be_bytes());
    }
    cmap.extend_from_slice(&12u32.to_be_bytes());
    cmap.extend_from_slice(&subtable);

    let count = u16_at(4);
    let mut tables: Vec<([u8; 4], Vec<u8>)> = (0..count)
        .map(|i| {
            let record = 12 + 16 * i;
            let (offset, length) = (u32_at(record + 8), u32_at(record + 12));
            let tag: [u8; 4] = font[record..record + 4].try_into().unwrap();
            let data = if &tag == b"cmap" { cmap.clone() } else { font[offset..offset + length].to_vec() };
            (tag, data)
        })
        .collect();
    tables.sort_by_key(|(tag, _)| *tag);
    let mut out = font[..12].to_vec();
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    for (tag, data) in &tables {
        out.extend_from_slice(tag);
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&(offset as u32).to_be_bytes());
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        body.extend_from_slice(data);
        while body.len() % 4 != 0 {
            body.push(0);
        }
        offset = 12 + 16 * tables.len() + body.len();
    }
    out.extend_from_slice(&body);
    out
}

/// A font flagged symbolic whose only cmap is a (3,1) subtable draws the
/// pixels the same font flagged non-symbolic draws. Producers flag fonts
/// symbolic wrongly all the time, and every other reader falls back to the
/// (3,1) subtable by the code. No Dart-level test can observe it: the font
/// program is built here with a single cmap subtable, and only the two
/// renders compared pixel for pixel say whether the codes reached glyphs.
#[test]
fn a_symbolic_font_with_only_a_31_cmap_draws_like_the_non_symbolic_one() {
    let font = only_a_31_cmap(DEJAVU);
    let face = ttf_parser::Face::parse(&font, 0).expect("the rebuilt font parses");
    let subtables: Vec<_> = face.tables().cmap.expect("cmap").subtables.into_iter().collect();
    assert_eq!(subtables.len(), 1, "the font keeps one cmap subtable");
    assert_eq!((subtables[0].platform_id, subtables[0].encoding_id), (ttf_parser::PlatformId::Windows, 1));

    let content = lines(&[(100, "(office fi)".into()), (60, "(Aqz 09)".into())]);
    let symbolic = render(simple_page_of(&content, &font, 4, ""));
    let non_symbolic = render(simple_page_of(&content, &font, 32, "/Encoding /WinAnsiEncoding"));
    assert!(inked(&non_symbolic), "the non-symbolic page is blank");
    assert!(inked(&symbolic), "the symbolic page is blank");
    assert!(symbolic == non_symbolic, "the symbolic font paints other pixels than the non-symbolic one");
}

/// A non-embedded font (drawn with a substitute face) showing a ligature
/// code and then a letter puts the letter where it stands when the ligature
/// code is replaced by a code of the same width. The old path decoded the
/// ligature to its letters and read the widths of the next codes for them,
/// so the letter after it moved. No Dart-level test can observe it: it needs
/// the pixel columns right of the code compared across two pages.
#[test]
fn a_ligature_code_keeps_its_own_advance_on_a_substituted_font() {
    // MacRoman 0xDE is the fi ligature. The ligature code and "a" are 1000
    // wide, "x" is 300 and every other code 500, so a neighbour's width read
    // for the ligature's second letter moves the "x".
    let widths: Vec<String> = (32u32..=255)
        .map(|c| match c {
            0xDE | 0x61 => 1000,
            0x78 => 300,
            _ => 500,
        })
        .map(|w| w.to_string())
        .collect();
    let font = format!(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /MacRomanEncoding \
         /FirstChar 32 /LastChar 255 /Widths [{}] >>",
        widths.join(" ")
    );
    let render_of = |show: &str| render_sized(page(&lines(&[(80, show.into())]), &font, vec![]));
    let (ligature, width) = render_of("(\\336x)");
    let (replaced, _) = render_of("(ax)");
    // Everything right of the first code's 24 pt cell: the "x" alone.
    let from = (10 + 24) * 100 / 72 + 1;
    let right = |raster: &[u8]| -> Vec<u8> {
        raster
            .chunks(width * 4)
            .flat_map(|row| row[from * 4..].iter().copied())
            .collect()
    };
    assert!(inked(&right(&replaced)), "the letter after the code is not drawn");
    assert!(right(&ligature) == right(&replaced), "the letter after the ligature code moved");
}
