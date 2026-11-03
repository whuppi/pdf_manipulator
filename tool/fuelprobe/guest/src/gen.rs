//! PDF writers for the fixtures: objects are numbered from 1, offsets are
//! computed, streams are deflated by the caller when it wants them to be.

use std::fmt::Write as _;
use std::io::Write as _;

pub struct Pdf {
    objects: Vec<Vec<u8>>,
}

impl Pdf {
    pub fn new() -> Self {
        Self { objects: Vec::new() }
    }

    /// The id of an object whose body comes later.
    pub fn reserve(&mut self) -> u32 {
        self.objects.push(Vec::new());
        self.objects.len() as u32
    }

    pub fn set(&mut self, id: u32, body: impl Into<Vec<u8>>) {
        self.objects[id as usize - 1] = body.into();
    }

    pub fn add(&mut self, body: impl Into<Vec<u8>>) -> u32 {
        self.objects.push(body.into());
        self.objects.len() as u32
    }

    pub fn add_stream(&mut self, dict: &str, data: &[u8]) -> u32 {
        self.add(stream(dict, data))
    }

    /// A file with a sound cross-reference table.
    pub fn finish(self, root: u32) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::with_capacity(self.objects.len());
        for (index, body) in self.objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_at = out.len();
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
        for offset in offsets {
            let _ = writeln!(table, "{offset:010} 00000 n ");
        }
        out.extend_from_slice(table.as_bytes());
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {root} 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
                self.objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }

    /// The same objects with no cross-reference table and a `startxref` that
    /// points into the middle of an object: a reader has to rebuild the table
    /// from the objects themselves.
    pub fn finish_damaged(self, root: u32) -> Vec<u8> {
        let mut out = b"%PDF-1.7\n".to_vec();
        for (index, body) in self.objects.iter().enumerate() {
            out.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {root} 0 R >>\nstartxref\n23\n%%EOF\n",
                self.objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

pub fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut out = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    out.extend_from_slice(data);
    out.extend_from_slice(b"\nendstream");
    out
}

pub fn flate(data: &[u8]) -> Vec<u8> {
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    z.write_all(data).expect("deflates");
    z.finish().expect("finishes")
}

/// Pseudo-random numbers that do not depend on the platform.
pub struct Lcg(pub u32);

impl Lcg {
    pub fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }
}

/// A document with one shared Helvetica (`/F1`), pages added one at a time.
pub struct Shell {
    pub pdf: Pdf,
    catalog: u32,
    pages: u32,
    pub font: u32,
    kids: Vec<u32>,
    /// Pages to a `/Pages` node below the root; 0 keeps the tree flat.
    pub fanout: usize,
    chunks: Vec<(u32, Vec<u32>)>,
    /// Further entries of the catalog, written when the file is finished.
    pub catalog_extra: String,
}

impl Shell {
    pub fn new() -> Self {
        let mut pdf = Pdf::new();
        let catalog = pdf.reserve();
        let pages = pdf.reserve();
        let font = pdf.add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>");
        Self { pdf, catalog, pages, font, kids: Vec::new(), fanout: 0, chunks: Vec::new(), catalog_extra: String::new() }
    }

    pub fn catalog_id(&self) -> u32 {
        self.catalog
    }

    pub fn pages_id(&self) -> u32 {
        self.pages
    }

    /// The id the next page will have.
    pub fn next_page_id(&self) -> u32 {
        self.pdf.objects.len() as u32 + 2
    }

    /// A page of `size` points drawing `content`. `resources` joins the
    /// resource dictionary beside `/Font`, `extra` the page dictionary.
    pub fn page(&mut self, size: (u32, u32), content: &[u8], resources: &str, extra: &str) -> u32 {
        let contents = self.pdf.add_stream("", content);
        let parent = if self.fanout > 0 {
            if self.kids.len() % self.fanout == 0 {
                let id = self.pdf.reserve();
                self.chunks.push((id, Vec::new()));
            }
            self.chunks.last().unwrap().0
        } else {
            self.pages
        };
        let page = self.pdf.add(format!(
            "<< /Type /Page /Parent {parent} 0 R /MediaBox [0 0 {} {}] /Contents {contents} 0 R \
             /Resources << /Font << /F1 {} 0 R >> {resources} >> {extra} >>",
            size.0, size.1, self.font
        ));
        self.kids.push(page);
        if let Some(chunk) = self.chunks.last_mut().filter(|_| self.fanout > 0) {
            chunk.1.push(page);
        }
        page
    }

    pub fn page_count(&self) -> usize {
        self.kids.len()
    }

    pub fn finish_parts(&mut self) {
        let mut roots: Vec<u32> = self.kids.clone();
        if self.fanout > 0 {
            roots = self.chunks.iter().map(|c| c.0).collect();
            for (id, pages) in std::mem::take(&mut self.chunks) {
                let list: Vec<String> = pages.iter().map(|k| format!("{k} 0 R")).collect();
                self.pdf.set(
                    id,
                    format!(
                        "<< /Type /Pages /Parent {} 0 R /Kids [{}] /Count {} >>",
                        self.pages,
                        list.join(" "),
                        pages.len()
                    ),
                );
            }
        }
        let kids: Vec<String> = roots.iter().map(|k| format!("{k} 0 R")).collect();
        self.pdf.set(
            self.pages,
            format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), self.kids.len()),
        );
        self.pdf.set(
            self.catalog,
            format!("<< /Type /Catalog /Pages {} 0 R {} >>", self.pages, self.catalog_extra),
        );
    }

    pub fn finish(mut self) -> Vec<u8> {
        self.finish_parts();
        let root = self.catalog;
        self.pdf.finish(root)
    }

    pub fn finish_damaged(mut self) -> Vec<u8> {
        self.finish_parts();
        let root = self.catalog;
        self.pdf.finish_damaged(root)
    }
}

// ── fixtures ─────────────────────────────────────────────────────────

const WORDS: [&str; 8] = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "theta", "kappa"];

/// `lines` lines of text from (72, 760) down, each a `Tj` of five words.
pub fn text_lines(lines: usize, first: usize) -> Vec<u8> {
    let mut out = String::from("BT /F1 9 Tf\n");
    let mut lcg = Lcg(first as u32 + 1);
    for line in 0..lines {
        let y = 760 - (line % 60) as i32 * 12;
        let x = 72 + (line / 60 % 3) as i32 * 170;
        let mut text = String::new();
        for _ in 0..5 {
            text.push_str(WORDS[lcg.next() as usize % WORDS.len()]);
            text.push(' ');
        }
        let _ = write!(out, "1 0 0 1 {x} {y} Tm ({text}{}) Tj\n", first + line);
    }
    out.push_str("ET\n");
    out.into_bytes()
}

/// `n` pages of text and one rectangle each.
pub fn pages(n: usize, lines: usize) -> Shell {
    let mut shell = Shell::new();
    for i in 0..n {
        let mut content = text_lines(lines, i * lines);
        content.extend_from_slice(b"72 600 200 40 re S\n");
        shell.page((612, 792), &content, "", "");
    }
    shell
}

/// One page of `n` stroked line segments and filled boxes.
pub fn paths(n: usize, size: u32) -> Vec<u8> {
    let mut lcg = Lcg(7);
    let mut out = String::from("0.5 w\n");
    for i in 0..n {
        let (x, y) = (lcg.next() % size, lcg.next() % size);
        let (x2, y2) = (lcg.next() % size, lcg.next() % size);
        if i % 4 == 0 {
            let _ = writeln!(out, "{} {} {} rg {x} {y} 6 6 re f", i % 3, i % 2, i % 5 / 4);
        } else {
            let _ = writeln!(out, "{x} {y} m {x2} {y2} l S");
        }
    }
    out.into_bytes()
}

/// A `w` x `w` raw RGB image whose samples depend on `seed`.
pub fn rgb_image(w: u32, seed: u32) -> Vec<u8> {
    let mut lcg = Lcg(seed);
    (0..w * w * 3).map(|i| ((i / 3 % w) as u8).wrapping_add(lcg.next() as u8 & 0x1F)).collect()
}

/// `n` images of `w` x `w` samples, `per_page` to a page, each drawn small so
/// a reduction has something to do. With `same`, every image is one image.
pub fn images(n: usize, w: u32, per_page: usize, same: bool) -> Shell {
    let mut shell = Shell::new();
    let mut made = 0;
    while made < n {
        let mut xobjects = String::new();
        let mut content = String::new();
        for k in 0..per_page.min(n - made) {
            let data = rgb_image(w, if same { 1 } else { (made + k) as u32 });
            let id = shell.pdf.add_stream(
                &format!(
                    "/Type /XObject /Subtype /Image /Width {w} /Height {w} \
                     /ColorSpace /DeviceRGB /BitsPerComponent 8"
                ),
                &data,
            );
            let _ = write!(xobjects, "/Im{k} {id} 0 R ");
            let (x, y) = (20 + (k % 20) as u32 * 28, 20 + (k / 20) as u32 * 28);
            let _ = writeln!(content, "q 24 0 0 24 {x} {y} cm /Im{k} Do Q");
        }
        made += per_page.min(n - made);
        shell.page((612, 792), content.as_bytes(), &format!("/XObject << {xobjects} >>"), "");
    }
    shell
}

/// `n` annotations with appearance streams over 50 pages.
pub fn annotations(n: usize) -> Shell {
    let mut shell = Shell::new();
    let pages = 50;
    let appearance = shell.pdf.add_stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 10 10]",
        b"0 0 1 rg 0 0 10 10 re f",
    );
    for _ in 0..pages {
        let ids: Vec<u32> = (0..n / pages).map(|_| shell.pdf.reserve()).collect();
        let page_id = shell.next_page_id();
        for (k, id) in ids.iter().enumerate() {
            let (x, y) = (20 + (k % 40) * 14, 20 + (k / 40) * 14);
            shell.pdf.set(
                *id,
                format!(
                    "<< /Type /Annot /Subtype /Square /Rect [{x} {y} {} {}] /P {page_id} 0 R /F 4 \
                     /C [0 0 1] /AP << /N {appearance} 0 R >> >>",
                    x + 10,
                    y + 10
                ),
            );
        }
        let refs: Vec<String> = ids.iter().map(|id| format!("{id} 0 R")).collect();
        shell.page((612, 792), b"0 0 m 1 1 l S\n", "", &format!("/Annots [{}]", refs.join(" ")));
    }
    shell
}

/// `n` text fields with a value and an appearance stream, 100 to a page.
pub fn form(n: usize) -> Shell {
    let mut shell = Shell::new();
    let mut fields = Vec::new();
    let mut made = 0;
    while made < n {
        let count = 100.min(n - made);
        let appearances: Vec<u32> = (0..count)
            .map(|_| {
                shell.pdf.add_stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 20] /Resources << >>",
                    b"BT /Helv 10 Tf 2 6 Td (value) Tj ET",
                )
            })
            .collect();
        let ids: Vec<u32> = (0..count).map(|_| shell.pdf.reserve()).collect();
        let page_id = shell.next_page_id();
        for k in 0..count {
            let (x, y) = (20 + (k % 5) * 110, 20 + (k / 5) * 30);
            shell.pdf.set(
                ids[k],
                format!(
                    "<< /Type /Annot /Subtype /Widget /FT /Tx /T (f{}) /V (value {}) \
                     /DA (/Helv 10 Tf 0 g) /Rect [{x} {y} {} {}] /P {page_id} 0 R /F 4 \
                     /AP << /N {} 0 R >> >>",
                    made + k,
                    made + k,
                    x + 100,
                    y + 20,
                    appearances[k]
                ),
            );
        }
        let refs: Vec<String> = ids.iter().map(|id| format!("{id} 0 R")).collect();
        fields.extend(refs.iter().cloned());
        made += count;
        shell.page((612, 792), b"0 0 m 1 1 l S\n", "", &format!("/Annots [{}]", refs.join(" ")));
    }
    let font = shell.font;
    shell.catalog_extra = format!(
        "/AcroForm << /Fields [{}] /DR << /Font << /Helv {font} 0 R >> >> /DA (/Helv 10 Tf 0 g) >>",
        fields.join(" ")
    );
    shell
}

/// `n` one-line pages and a flat outline of `n` bookmarks, one for each.
pub fn outline(n: usize) -> Shell {
    let mut shell = Shell::new();
    let mut page_ids = Vec::new();
    for i in 0..n {
        let content = format!("BT /F1 12 Tf 72 720 Td (Chapter {i}) Tj ET\n");
        page_ids.push(shell.page((612, 792), content.as_bytes(), "", ""));
    }
    let root = shell.pdf.reserve();
    let items: Vec<u32> = (0..n).map(|_| shell.pdf.reserve()).collect();
    for (i, id) in items.iter().enumerate() {
        let prev = if i > 0 { format!("/Prev {} 0 R", items[i - 1]) } else { String::new() };
        let next = if i + 1 < n { format!("/Next {} 0 R", items[i + 1]) } else { String::new() };
        shell.pdf.set(
            *id,
            format!(
                "<< /Title (Chapter {i}) /Parent {root} 0 R {prev} {next} /Dest [{} 0 R /Fit] >>",
                page_ids[i]
            ),
        );
    }
    shell.pdf.set(
        root,
        format!(
            "<< /Type /Outlines /First {} 0 R /Last {} 0 R /Count {n} >>",
            items[0],
            items[n - 1]
        ),
    );
    shell.catalog_extra = format!("/Outlines {root} 0 R /PageMode /UseOutlines");
    shell
}

/// A tagged document: `n` paragraphs, 40 to a page and a section, with marked
/// content that the structure tree points at.
pub fn tagged(n: usize) -> Shell {
    let mut shell = Shell::new();
    let per_page = 40;
    let tree_root = shell.pdf.reserve();
    let document = shell.pdf.reserve();
    let mut sections: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut made = 0;
    while made < n {
        let count = per_page.min(n - made);
        let section = shell.pdf.reserve();
        let ids: Vec<u32> = (0..count).map(|_| shell.pdf.reserve()).collect();
        let page_id = shell.next_page_id();
        let mut content = String::from("BT /F1 10 Tf\n");
        for k in 0..count {
            let _ = writeln!(
                content,
                "/P << /MCID {k} >> BDC 1 0 0 1 72 {} Tm (Paragraph {}) Tj EMC",
                760 - k * 14,
                made + k
            );
            shell.pdf.set(
                ids[k],
                format!("<< /Type /StructElem /S /P /P {section} 0 R /Pg {page_id} 0 R /K {k} >>"),
            );
        }
        content.push_str("ET\n");
        shell.page(
            (612, 792),
            content.as_bytes(),
            "",
            &format!("/StructParents {}", sections.len()),
        );
        sections.push((section, ids));
        made += count;
    }
    for (section, ids) in &sections {
        let kids: Vec<String> = ids.iter().map(|k| format!("{k} 0 R")).collect();
        shell.pdf.set(
            *section,
            format!("<< /Type /StructElem /S /Sect /P {document} 0 R /K [{}] >>", kids.join(" ")),
        );
    }
    let section_refs: Vec<String> = sections.iter().map(|(s, _)| format!("{s} 0 R")).collect();
    shell.pdf.set(
        document,
        format!(
            "<< /Type /StructElem /S /Document /P {tree_root} 0 R /K [{}] >>",
            section_refs.join(" ")
        ),
    );
    let numbers: Vec<String> = sections
        .iter()
        .enumerate()
        .map(|(i, (_, ids))| {
            let refs: Vec<String> = ids.iter().map(|k| format!("{k} 0 R")).collect();
            format!("{i} [{}]", refs.join(" "))
        })
        .collect();
    shell.pdf.set(
        tree_root,
        format!(
            "<< /Type /StructTreeRoot /K {document} 0 R /ParentTree << /Nums [{}] >> >>",
            numbers.join(" ")
        ),
    );
    shell.catalog_extra =
        format!("/StructTreeRoot {tree_root} 0 R /MarkInfo << /Marked true >> /Lang (en-US)");
    shell
}

// ── TrueType ─────────────────────────────────────────────────────────

pub const GLYPHS: usize = 24;

fn be16(v: i64) -> [u8; 2] {
    (v as i16).to_be_bytes()
}

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

/// A TrueType font of `GLYPHS` glyphs, 'A' onward, drawn as stars from `seed`.
pub fn truetype(seed: u32) -> Vec<u8> {
    let mut glyphs: Vec<Vec<u8>> = vec![simple_glyph(&[(50, 0), (450, 0), (450, 700), (50, 700)])];
    for gid in 1..GLYPHS as u32 {
        glyphs.push(simple_glyph(&star(gid + seed * 64, 40)));
    }
    let (mut glyf, mut offsets) = (Vec::new(), Vec::new());
    for glyph in &glyphs {
        offsets.push(glyf.len());
        glyf.extend_from_slice(glyph);
        if glyf.len() % 2 == 1 {
            glyf.push(0);
        }
    }
    offsets.push(glyf.len());
    let mut loca = Vec::new();
    for &offset in &offsets {
        loca.extend(((offset / 2) as u16).to_be_bytes());
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
    head.extend(be16(0));
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

/// `n` embedded TrueType fonts, each showing four of its glyphs, 20 to a page.
pub fn fonts(n: usize) -> Shell {
    let mut shell = Shell::new();
    let mut made = 0;
    while made < n {
        let mut font_dict = String::new();
        let mut content = String::from("BT\n");
        for k in 0..20.min(n - made) {
            let program = truetype((made + k) as u32);
            let file = shell.pdf.add_stream(&format!("/Length1 {}", program.len()), &program);
            let descriptor = shell.pdf.add(format!(
                "<< /Type /FontDescriptor /FontName /Test{} /Flags 32 /FontBBox [0 0 1000 1000] \
                 /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 {file} 0 R >>",
                made + k
            ));
            let font = shell.pdf.add(format!(
                "<< /Type /Font /Subtype /TrueType /BaseFont /Test{} /FirstChar 65 /LastChar {} \
                 /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor {descriptor} 0 R >>",
                made + k,
                65 + GLYPHS - 2,
                vec!["600"; GLYPHS - 1].join(" ")
            ));
            let _ = write!(font_dict, "/T{k} {font} 0 R ");
            let _ = writeln!(content, "/T{k} 12 Tf 1 0 0 1 72 {} Tm (ABCD) Tj", 760 - k * 30);
        }
        content.push_str("ET\n");
        made += 20.min(n - made);
        let resources = format!("/Font << {font_dict} >>");
        // The shell's own /F1 entry shares the dictionary, so the page is
        // written here with its fonts replacing it.
        let contents = shell.pdf.add_stream("", content.as_bytes());
        let page = shell.pdf.add(format!(
            "<< /Type /Page /Parent {} 0 R /MediaBox [0 0 612 792] /Contents {contents} 0 R \
             /Resources << {resources} >> >>",
            shell.pages_id()
        ));
        shell.kids_push(page);
    }
    shell
}

impl Shell {
    fn kids_push(&mut self, page: u32) {
        self.kids.push(page);
    }
}
