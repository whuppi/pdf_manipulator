//! A page cut into bands renders the pixels the whole canvas does.
//!
//! Each fixture is rendered with the band height forced to 1, 17 and 256 rows
//! and as one band, at 36, 72, 150 and 300 dpi, on a transparent background so
//! a pixel's alpha is its coverage. The bytes must be those of one render of
//! the whole canvas: no pixel may differ, whatever its coverage. Each case
//! prints its count of differing pixels, and the first one with its colour in
//! both renders.

use super::support::{pdf_from_objects, stream_object};
use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::rendering::{PageRenderer, RenderOptions};

pub(super) struct Fixture {
    pub(super) name: &'static str,
    pub(super) content: String,
    pub(super) resources: String,
    pub(super) annots: String,
    /// Objects numbered from 5.
    pub(super) extra: Vec<Vec<u8>>,
}

impl Fixture {
    pub(super) fn new(name: &'static str, content: &str) -> Self {
        Self {
            name,
            content: content.to_string(),
            resources: String::new(),
            annots: String::new(),
            extra: Vec::new(),
        }
    }

    pub(super) fn pdf(&self) -> Vec<u8> {
        let mut objects = vec![
            b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
            format!(
                "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R \
                 /Resources << {} >> {} >>\nendobj\n",
                self.resources, self.annots
            )
            .into_bytes(),
            stream_object(4, "", self.content.as_bytes()),
        ];
        objects.extend(self.extra.iter().cloned());
        pdf_from_objects(&objects)
    }
}

pub(super) const HELVETICA: &str = "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>";

fn fixtures() -> Vec<Fixture> {
    let mut all = Vec::new();

    all.push(Fixture::new(
        "paths",
        "0.9 0.2 0.2 rg 10.3 10.7 60.5 40.2 re f\n\
         0.2 0.4 0.9 rg 80 20 m 130 20 l 105 70 l f\n\
         0.1 0.7 0.3 rg 20 80 m 60 140 100 140 140 80 c 100 100 60 100 20 80 c f\n\
         0.5 0.5 0.1 rg 150 100 m 190 100 l 190 140 l 150 140 l h 160 110 m 180 110 l \
         180 130 l 160 130 l h f*\n\
         0 0 0 rg 100 100 m 120 190 l 135 100 l 170 160 l 105 150 l h f\n\
         0.3 g 0 120.5 200 0.5 re f\n\
         0 0.3 0.6 rg 99.5 0 1 200 re f\n\
         q 0.866 0.5 -0.5 0.866 100 100 cm 0.6 0.1 0.6 rg 0 0 80 3 re f Q\n\
         q 0.17 0.98 -0.98 0.17 150 10 cm 0.1 0.5 0.5 rg 0 0 120 2.5 re f Q\n",
    ));

    all.push(Fixture::new(
        "strokes",
        "2.7 w 1 J 1 j 0 0.5 0 RG 10 10 m 190 190 l S\n\
         5 w 0 J 0 j 10 M 0.5 0 0 RG 20 180 m 60 120 l 100 180 l S\n\
         0 w 0 0 0 RG 10 30 m 190 52 l S\n\
         0.3 w 0 0 1 RG 5 190 m 195 20 l S\n\
         10 w 2 J 2 j 0.2 0.2 0.8 RG 20 100 m 70 190 130 10 180 100 c S\n\
         1.5 w 0 J 0 j 0 0 0 RG 30.5 30.5 100 100 re S\n\
         3 w 1 J 1 j 0.7 0.1 0.1 RG 150 150 m 190 190 l 150 190 l h S\n\
         8 w 0 J 0 j 1 M 0 0.4 0.4 RG 60 60 m 150 80 l 70 100 l S\n",
    ));

    all.push(Fixture::new(
        "dashes",
        "[6 3] 0 d 2 w 0 0 0 RG 10 20 m 190 40 l S\n\
         [1 4] 2 d 1 J 3 w 0.8 0 0 RG 10 60 m 190 100 l S\n\
         [10 5 2 5] 0 d 1.5 w 0 J 0 0.5 0 RG 20 190 m 180 10 l S\n\
         [3] 1 d 2 w 0 0 0.8 RG 100 100 m 100 190 150 190 190 100 c S\n\
         [4 2] 0 d 1 w 0 0 0 RG 0.866 0.5 -0.5 0.866 100 100 cm 0 0 80 40 re S\n",
    ));

    let mut text = Fixture::new(
        "text",
        "BT /F1 24 Tf 10 170 Td (Banded Text) Tj ET\n\
         BT /F1 9 Tf 10 150 Td (The quick brown fox jumps over the lazy dog) Tj ET\n\
         BT /F1 36 Tf 0.866 0.5 -0.5 0.866 40 40 Tm 0.1 0.2 0.7 rg (Rot) Tj ET\n\
         BT /F1 18 Tf 1 Tr 1 w 10 100 Td 0 0 0 RG (Stroke) Tj ET\n\
         BT /F1 22 Tf 2 Tr 1.5 w 100 110 Td 0.9 0.8 0.1 rg 0.5 0 0 RG (Both) Tj ET\n\
         BT /F1 14 Tf 0 Tr 10 70 Td 0 g [(A) -300 (B) 200 (C) -100 (Spaced)] TJ ET\n\
         BT /F1 11 Tf 10 50 Td 14 TL (First) Tj (Second) ' T* (Third) Tj ET\n",
    );
    text.resources = HELVETICA.to_string();
    all.push(text);

    let mut clips = Fixture::new(
        "clips",
        "q 100 20 m 150 20 180 50 180 100 c 180 150 150 180 100 180 c 50 180 20 150 20 100 c \
         20 50 50 20 100 20 c h W n 1 0 0 rg 0 0 200 200 re f 0 0 1 rg 0 90 200 20 re f Q\n\
         q 20 20 80 80 re W n 70 70 80 80 re W n 0 0.6 0 rg 0 0 200 200 re f Q\n\
         q BT /F1 60 Tf 7 Tr 10 130 Td (CLIP) Tj ET 0.8 0.1 0.8 rg 0 0 200 200 re f Q\n\
         q 10 10 m 190 10 l 100 190 l h W* n 0.1 0.1 0.1 RG 4 w 0 0 m 200 200 l S Q\n",
    );
    clips.resources = HELVETICA.to_string();
    all.push(clips);

    let function = "<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>";
    let mut axial = Fixture::new(
        "axial",
        "q 20 20 160 160 re W n /S1 sh Q\n\
         q 10 10 m 190 10 l 100 190 l h W n /S2 sh Q\n",
    );
    axial.resources = format!(
        "/Shading << /S1 << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [10 10 190 190] \
         /Function {function} /Extend [true true] >> \
         /S2 << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [40 60 160 90] \
         /Function {function} /Extend [false true] >> >>"
    );
    all.push(axial);

    let mut radial = Fixture::new(
        "radial",
        "q 0 0 200 200 re W n /S1 sh Q\n\
         q 10 100 180 90 re W n /S2 sh Q\n",
    );
    radial.resources = format!(
        "/Shading << /S1 << /ShadingType 3 /ColorSpace /DeviceRGB /Coords [100 100 10 100 100 90] \
         /Function {function} /Extend [false false] >> \
         /S2 << /ShadingType 3 /ColorSpace /DeviceRGB /Coords [90 120 5 100 110 60] \
         /Function {function} /Extend [true false] >> >>"
    );
    all.push(radial);

    all.push(mesh_fixture());
    all.push(forms_fixture());
    all.push(annotation_fixture());
    all.push(image_fixture());
    all
}

/// Types 4 to 7 in one page, each in its own quarter, every shape crossing
/// the cuts.
fn mesh_fixture() -> Fixture {
    let decode = "/Decode [0 200 0 200 0 1 0 1 0 1]";
    let common = "/ColorSpace /DeviceRGB /BitsPerCoordinate 8 /BitsPerComponent 8";

    let vertex = |flag: Option<u8>, x: u8, y: u8, rgb: [u8; 3]| {
        let mut v = Vec::new();
        v.extend(flag);
        v.extend([x, y]);
        v.extend(rgb);
        v
    };

    let mut type4 = Vec::new();
    type4.extend(vertex(Some(0), 10, 20, [255, 0, 0]));
    type4.extend(vertex(Some(0), 120, 10, [0, 255, 0]));
    type4.extend(vertex(Some(0), 60, 150, [0, 0, 255]));
    type4.extend(vertex(Some(1), 200, 140, [255, 255, 0]));
    type4.extend(vertex(Some(2), 220, 60, [0, 255, 255]));

    let mut type5 = Vec::new();
    for (row, y) in [20u8, 110, 200].iter().enumerate() {
        for (col, x) in [20u8, 130, 240].iter().enumerate() {
            let rgb = [(row * 120) as u8, (col * 120) as u8, 200 - (row * col * 40) as u8];
            type5.extend(vertex(None, *x, *y, rgb));
        }
    }

    let point = |x: u8, y: u8| vec![x, y];
    let coons: [(u8, u8); 12] = [
        (40, 40),
        (30, 100),
        (50, 160),
        (40, 220),
        (100, 230),
        (160, 210),
        (220, 220),
        (230, 160),
        (210, 100),
        (220, 40),
        (160, 30),
        (100, 50),
    ];
    let tensor: [(u8, u8); 4] = [(90, 90), (90, 170), (170, 170), (170, 90)];
    let colours = [[255u8, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];
    let patch = |with_interior: bool| {
        let mut p = vec![0u8];
        for (x, y) in coons {
            p.extend(point(x, y));
        }
        if with_interior {
            for (x, y) in tensor {
                p.extend(point(x, y));
            }
        }
        for c in colours {
            p.extend(c);
        }
        p
    };

    let mut fixture = Fixture::new(
        "mesh",
        "/M4 sh\n\
         q 0.5 0 0 0.5 100 0 cm /M5 sh Q\n\
         q 0.5 0 0 0.5 0 100 cm /M6 sh Q\n\
         q 0.5 0 0 0.5 100 100 cm /M7 sh Q\n",
    );
    fixture.resources = "/Shading << /M4 5 0 R /M5 6 0 R /M6 7 0 R /M7 8 0 R >>".to_string();
    fixture.extra = vec![
        stream_object(5, &format!("/ShadingType 4 {common} /BitsPerFlag 8 {decode}"), &type4),
        stream_object(
            6,
            &format!("/ShadingType 5 {common} /VerticesPerRow 3 {decode}"),
            &type5,
        ),
        stream_object(
            7,
            &format!("/ShadingType 6 {common} /BitsPerFlag 8 {decode}"),
            &patch(false),
        ),
        stream_object(
            8,
            &format!("/ShadingType 7 {common} /BitsPerFlag 8 {decode}"),
            &patch(true),
        ),
    ];
    fixture
}

/// Forms that straddle, sit inside and miss the cuts, one of them a
/// transparency group.
fn forms_fixture() -> Fixture {
    let mut fixture = Fixture::new(
        "forms",
        "q 1 0 0 1 10 10 cm /Fm1 Do Q\n\
         q 1 0 0 1 80 100 cm /Fm1 Do Q\n\
         q 0.7 0.7 -0.7 0.7 100 30 cm /Fm1 Do Q\n\
         q 1 0 0 1 20 140 cm /Fm2 Do Q\n\
         q 1 0 0 1 400 400 cm /Fm1 Do Q\n",
    );
    fixture.resources = "/XObject << /Fm1 5 0 R /Fm2 6 0 R >> \
         /ExtGState << /Half << /ca 0.5 >> >>"
        .to_string();
    fixture.extra = vec![
        stream_object(
            5,
            "/Type /XObject /Subtype /Form /BBox [0 0 60 60]",
            b"0.2 0.6 0.9 rg 0 0 60 60 re f 1 0 0 rg 20.5 20.5 30 30 re f 0 G 2 w 5 5 m 55 55 l S",
        ),
        stream_object(
            6,
            "/Type /XObject /Subtype /Form /BBox [0 0 80 40] /Group << /S /Transparency /I true >>",
            b"0.9 0.1 0.1 rg 0 0 50 40 re f 0.1 0.1 0.9 rg 30 0 50 40 re f",
        ),
    ];
    fixture
}

fn annotation_fixture() -> Fixture {
    let mut fixture = Fixture::new("annotations", "0.95 g 0 0 200 200 re f\n");
    fixture.annots = "/Annots [ 5 0 R 7 0 R ]".to_string();
    fixture.extra = vec![
        b"5 0 obj\n<< /Type /Annot /Subtype /Square /Rect [20 20 120 80] /F 4 \
          /AP << /N 6 0 R >> >>\nendobj\n"
            .to_vec(),
        stream_object(
            6,
            "/Type /XObject /Subtype /Form /BBox [0 0 100 60]",
            b"1 0 0 RG 3 w 1.5 1.5 97 57 re S 0.9 0.9 0 rg 10 10 30 20 re f",
        ),
        b"7 0 obj\n<< /Type /Annot /Subtype /Square /Rect [110 100 190 190] /F 4 \
          /AP << /N 8 0 R >> >>\nendobj\n"
            .to_vec(),
        stream_object(
            8,
            "/Type /XObject /Subtype /Form /BBox [0 0 80 90]",
            b"0 0.5 0 RG 2 w 1 1 78 88 re S 0 0.5 0 rg 10 10 m 70 10 l 40 80 l f",
        ),
    ];
    fixture
}

/// An upscaled image and a rotated one, the image path as it stands.
fn image_fixture() -> Fixture {
    let mut pixels = Vec::new();
    for y in 0..4u8 {
        for x in 0..4u8 {
            pixels.extend([x * 80, y * 80, 255 - x * 40 - y * 40]);
        }
    }
    let mut fixture = Fixture::new(
        "images",
        "q 100.4 0 0 100.6 20.3 70.2 cm /Im1 Do Q\n\
         q 50 30 -30 50 150 20 cm /Im1 Do Q\n\
         q 0 0 0.5 rg 40 0 0 40 20 20 cm /Mk Do Q\n",
    );
    fixture.resources = "/XObject << /Im1 5 0 R /Mk 6 0 R >>".to_string();
    fixture.extra = vec![
        stream_object(
            5,
            "/Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceRGB \
             /BitsPerComponent 8",
            &pixels,
        ),
        stream_object(
            6,
            "/Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true \
             /BitsPerComponent 1",
            &[0xAA, 0x55, 0xAA, 0x55, 0xF0, 0xF0, 0x0F, 0x0F],
        ),
    ];
    fixture
}

/// Deterministic pixels with enough detail at every scale that a wrong row,
/// a wrong column or a half-sample shift changes a byte.
pub(super) fn detailed_rgb(width: u32, height: u32) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
    for y in 0..height {
        for x in 0..width {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = (state >> 59) as u32;
            let stripe = if (x / 9 + y / 7) % 2 == 0 { 40 } else { 0 };
            pixels.push(((x * 255 / width) + noise + stripe).min(255) as u8);
            pixels.push(((y * 255 / height) + noise).min(255) as u8);
            pixels.push((((x + y) * 255 / (width + height)) + stripe).min(255) as u8);
        }
    }
    pixels
}

/// A bilevel picture, `true` black: a checker with a disc cut out of it.
pub(super) fn bilevel(width: u32, height: u32) -> Vec<Vec<bool>> {
    let (cx, cy, r) = (width as i64 / 2, height as i64 / 2, width.min(height) as i64 / 3);
    (0..height as i64)
        .map(|y| {
            (0..width as i64)
                .map(|x| {
                    let checker = (x / 6 + y / 6) % 2 == 0;
                    let disc = (x - cx).pow(2) + (y - cy).pow(2) < r * r;
                    checker ^ disc
                })
                .collect()
        })
        .collect()
}

/// The picture as CCITT Group 4 (T.6) data.
pub(super) fn group4(picture: &[Vec<bool>]) -> Vec<u8> {
    let width = picture[0].len() as u32;
    let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
    for row in picture {
        let pels = row
            .iter()
            .map(|&black| if black { fax::Color::Black } else { fax::Color::White });
        encoder.encode_line(pels, width).expect("encodes");
    }
    encoder.finish().expect("finishes").finish()
}

/// The picture as an embedded JBIG2 stream: a page-information segment and
/// one immediate generic region coded with MMR, which is T.6.
pub(super) fn jbig2(picture: &[Vec<bool>]) -> Vec<u8> {
    let (width, height) = (picture[0].len() as u32, picture.len() as u32);
    let mut page = Vec::new();
    for v in [width, height, 0, 0] {
        page.extend_from_slice(&v.to_be_bytes());
    }
    page.extend_from_slice(&[0, 0, 0]);
    let mut region = Vec::new();
    for v in [width, height, 0, 0] {
        region.extend_from_slice(&v.to_be_bytes());
    }
    region.extend_from_slice(&[0, 0x01]);
    region.extend_from_slice(&group4(picture));
    let mut out = Vec::new();
    for (number, kind, data) in [(0u32, 48u8, page), (1u32, 38u8, region)] {
        out.extend_from_slice(&number.to_be_bytes());
        out.extend_from_slice(&[kind, 0x00, 0x01]);
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(&data);
    }
    out
}

pub(super) fn packed_bits(picture: &[Vec<bool>], black_is_one: bool) -> Vec<u8> {
    let row_bytes = picture[0].len().div_ceil(8);
    let mut out = vec![0u8; row_bytes * picture.len()];
    for (y, row) in picture.iter().enumerate() {
        for (x, &black) in row.iter().enumerate() {
            if black == black_is_one {
                out[y * row_bytes + x / 8] |= 0x80 >> (x % 8);
            }
        }
    }
    out
}

/// The transforms an image meets, one fixture each. Together with the dpis
/// they reach every class: a pure scale down and up, a quarter turn, an
/// eighth turn, a shear, a vertical and a horizontal flip.
const TRANSFORMS: [(&str, &str); 6] = [
    ("scale", "150 0 0 112 25 40"),
    ("rot90", "0 150 -112 0 140 25"),
    ("rot45", "106 106 -80 80 100 20"),
    ("shear", "150 0 40 112 20 40"),
    ("flip_v", "150 0 0 -112 25 150"),
    ("flip_h", "-150 0 0 112 175 40"),
];

fn image_object(number: u32, dict: &str, data: &[u8]) -> Vec<u8> {
    stream_object(number, &format!("/Type /XObject /Subtype /Image {dict}"), data)
}

/// Every image kind at every transform class, as the plan's identity proof
/// asks: raw samples, a soft mask, a stencil `/Mask`, a colour key, an inline
/// image, a stencil painted in the fill colour, and each codec.
fn image_fixtures() -> Vec<Fixture> {
    let rgb = detailed_rgb(400, 300);
    let mut all = Vec::new();

    for (name, matrix) in TRANSFORMS {
        let mut fixture =
            Fixture::new(name, &format!("q {matrix} cm /Im Do Q\n"));
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            "/Width 400 /Height 300 /ColorSpace /DeviceRGB /BitsPerComponent 8",
            &rgb,
        )];
        all.push(fixture);
    }

    // Gray and sub-byte samples through a Flate filter, and an Indexed palette.
    {
        use std::io::Write;
        let gray: Vec<u8> = (0..300u32 * 200).map(|i| ((i % 300) * 255 / 300) as u8 ^ (i / 300) as u8).collect();
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&gray).expect("compresses");
        let flate = z.finish().expect("finishes");
        let two_bit: Vec<u8> = (0..(300 / 4) * 200).map(|i| (i * 37 % 251) as u8).collect();
        let palette: Vec<u8> = (0..16u8).flat_map(|i| [i * 16, 255 - i * 16, i * 8]).collect();
        let nibbles: Vec<u8> = (0..(300 / 2) * 200).map(|i| (i * 53 % 256) as u8).collect();
        let mut fixture = Fixture::new(
            "kinds",
            "q 90 0 0 60 10 120 cm /G Do Q\nq 90 0 0 60 105 120 cm /T Do Q\n\
             q 60 40 -40 60 60 20 cm /P Do Q\n",
        );
        fixture.resources = "/XObject << /G 5 0 R /T 6 0 R /P 7 0 R >>".to_string();
        fixture.extra = vec![
            image_object(
                5,
                "/Width 300 /Height 200 /ColorSpace /DeviceGray /BitsPerComponent 8 \
                 /Filter /FlateDecode",
                &flate,
            ),
            image_object(
                6,
                "/Width 300 /Height 200 /ColorSpace /DeviceGray /BitsPerComponent 2 \
                 /Decode [1 0]",
                &two_bit,
            ),
            image_object(
                7,
                &format!(
                    "/Width 300 /Height 200 /ColorSpace [/Indexed /DeviceRGB 15 <{}>] \
                     /BitsPerComponent 4",
                    palette.iter().map(|b| format!("{b:02x}")).collect::<String>()
                ),
                &nibbles,
            ),
        ];
        all.push(fixture);
    }

    // A soft mask of another size, under a scale and under an eighth turn.
    {
        let soft: Vec<u8> = (0..100u32 * 80).map(|i| ((i % 100) * 255 / 100) as u8 ^ ((i / 100) * 3) as u8).collect();
        let mut fixture = Fixture::new(
            "softmask",
            "0.8 0.8 0.2 rg 0 0 200 200 re f\n\
             q 120 0 0 90 10 100 cm /Im Do Q\nq 70 70 -50 50 120 20 cm /Im Do Q\n",
        );
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![
            image_object(
                5,
                "/Width 400 /Height 300 /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask 6 0 R",
                &rgb,
            ),
            image_object(6, "/Width 100 /Height 80 /ColorSpace /DeviceGray /BitsPerComponent 8", &soft),
        ];
        all.push(fixture);
    }

    // A stencil /Mask with no colour space, which the stencil reader takes.
    {
        let picture = bilevel(64, 48);
        let mut fixture = Fixture::new(
            "stencilmask",
            "0.2 0.6 0.8 rg 0 0 200 200 re f\n\
             q 120 0 0 90 10 100 cm /Im Do Q\nq 70 70 -50 50 120 20 cm /Im Do Q\n",
        );
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![
            image_object(
                5,
                "/Width 400 /Height 300 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Mask 6 0 R",
                &rgb,
            ),
            image_object(
                6,
                "/Width 64 /Height 48 /ImageMask true /BitsPerComponent 1",
                &packed_bits(&picture, false),
            ),
        ];
        all.push(fixture);
    }

    // A colour key.
    {
        let mut fixture = Fixture::new(
            "colourkey",
            "0.9 0.3 0.3 rg 0 0 200 200 re f\n\
             q 120 0 0 90 10 100 cm /Im Do Q\nq 70 70 -50 50 120 20 cm /Im Do Q\n",
        );
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            "/Width 400 /Height 300 /ColorSpace /DeviceRGB /BitsPerComponent 8 \
             /Mask [0 120 0 255 0 255]",
            &rgb,
        )];
        all.push(fixture);
    }

    // An inline image, hex coded, at a scale and at an eighth turn.
    {
        let data: String = detailed_rgb(24, 18).iter().map(|b| format!("{b:02x}")).collect();
        let fixture = Fixture::new(
            "inline",
            &format!(
                "q 120 0 0 90 10 100 cm BI /W 24 /H 18 /CS /RGB /BPC 8 /F /AHx ID {data}> EI Q\n\
                 q 70 70 -50 50 120 20 cm BI /W 24 /H 18 /CS /RGB /BPC 8 /F /AHx ID {data}> EI Q\n"
            ),
        );
        all.push(fixture);
    }

    // A stencil painted in the fill colour.
    {
        let picture = bilevel(200, 150);
        let mut fixture = Fixture::new(
            "stencil",
            "0.7 0.1 0.4 rg q 150 0 0 112 25 40 cm /St Do Q\n\
             0.1 0.5 0.2 rg q 70 70 -50 50 120 20 cm /St Do Q\n",
        );
        fixture.resources = "/XObject << /St 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            "/Width 200 /Height 150 /ImageMask true /BitsPerComponent 1",
            &packed_bits(&picture, false),
        )];
        all.push(fixture);
    }

    // The codecs, each at a scale and at an eighth turn.
    let codec_content = "q 150 0 0 112 25 40 cm /Im Do Q\nq 70 70 -50 50 120 20 cm /Im Do Q\n";
    {
        let mut jpeg = Vec::new();
        jpeg_encoder::Encoder::new(&mut jpeg, 85)
            .encode(&detailed_rgb(320, 240), 320, 240, jpeg_encoder::ColorType::Rgb)
            .expect("encodes");
        let mut fixture = Fixture::new("dct", codec_content);
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            "/Width 320 /Height 240 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode",
            &jpeg,
        )];
        all.push(fixture);
    }
    {
        let jp2: &[u8] = include_bytes!("../../../tests/fixtures/jpx/sample_gray.jp2");
        let decoded = crate::decoders::jpx::decode_jpx_at(jp2, None).expect("decodes");
        let mut fixture = Fixture::new("jpx", codec_content);
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            &format!(
                "/Width {} /Height {} /Filter /JPXDecode",
                decoded.width, decoded.height
            ),
            jp2,
        )];
        all.push(fixture);
    }
    {
        let picture = bilevel(256, 192);
        let mut fixture = Fixture::new("ccitt", codec_content);
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            "/Width 256 /Height 192 /ColorSpace /DeviceGray /BitsPerComponent 1 \
             /Filter /CCITTFaxDecode /DecodeParms << /K -1 /Columns 256 /Rows 192 >>",
            &group4(&picture),
        )];
        all.push(fixture);
        let mut fixture = Fixture::new("jbig2", codec_content);
        fixture.resources = "/XObject << /Im 5 0 R >>".to_string();
        fixture.extra = vec![image_object(
            5,
            "/Width 256 /Height 192 /ColorSpace /DeviceGray /BitsPerComponent 1 \
             /Filter /JBIG2Decode",
            &jbig2(&picture),
        )];
        all.push(fixture);
    }
    all
}

fn render(doc: &PdfDocument, dpi: u32, rows: Option<u32>) -> (u32, u32, Vec<u8>) {
    let mut renderer = PageRenderer::new(RenderOptions::with_dpi(dpi).with_transparent_background());
    renderer.forced_rows = rows;
    let mut raster = Vec::new();
    let (width, height) = renderer
        .render_bands(
            doc,
            0,
            &|_, height| 0..height,
            &CancelCheck::none(),
            &mut |_, band| {
                raster.extend_from_slice(band.data());
                Ok(())
            },
        )
        .expect("renders");
    (width, height, raster)
}

#[derive(Default)]
struct Diff {
    differing: u64,
    max_channel: u8,
    first: Option<(u32, u32, [u8; 4], [u8; 4])>,
}

fn pixel(raster: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = (y as usize * width as usize + x as usize) * 4;
    raster[at..at + 4].try_into().expect("four bytes")
}

fn compare(width: u32, height: u32, whole: &[u8], cut: &[u8]) -> Diff {
    let mut diff = Diff::default();
    for y in 0..height {
        for x in 0..width {
            let (a, b) = (pixel(whole, width, x, y), pixel(cut, width, x, y));
            if a != b {
                diff.differing += 1;
                diff.first.get_or_insert((x, y, a, b));
                for channel in 0..4 {
                    diff.max_channel = diff.max_channel.max(a[channel].abs_diff(b[channel]));
                }
            }
        }
    }
    diff
}

pub(super) fn prove(fixture: &Fixture) {
    let doc = PdfDocument::from_bytes(fixture.pdf()).expect("fixture parses");
    let mut failures = Vec::new();
    for dpi in [36, 72, 150, 300] {
        let (width, height, whole) = render(&doc, dpi, Some(u32::MAX));
        assert!(whole.iter().any(|&b| b != 0), "{} at {dpi} dpi drew nothing", fixture.name);
        for rows in [Some(1), Some(17), Some(256), None] {
            let (w, h, cut) = render(&doc, dpi, rows);
            assert_eq!((w, h), (width, height));
            let diff = compare(width, height, &whole, &cut);
            println!(
                "band_proof {:<12} {dpi:>3} dpi rows {:>8}: {} px, differing {}, max channel step {}",
                fixture.name,
                rows.map_or("default".to_string(), |r| r.to_string()),
                width as u64 * height as u64,
                diff.differing,
                diff.max_channel,
            );
            if let Some((x, y, a, b)) = diff.first {
                println!("    first differing pixel ({x}, {y}): whole {a:?} cut {b:?}");
                failures.push(format!("{dpi} dpi, rows {rows:?}: {} differing", diff.differing));
            }
        }
    }
    assert!(failures.is_empty(), "{}: {failures:?}", fixture.name);
}

/// No Dart battery sees it: the band height is an internal knob no API sets,
/// and the property is about the pixels of one render against another.
#[test]
fn bands_equal_the_whole_canvas_on_paths_strokes_and_dashes() {
    for fixture in fixtures()
        .iter()
        .filter(|f| ["paths", "strokes", "dashes"].contains(&f.name))
    {
        prove(fixture);
    }
}

/// No Dart battery sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_text_and_clips() {
    for fixture in fixtures().iter().filter(|f| ["text", "clips"].contains(&f.name)) {
        prove(fixture);
    }
}

/// A stroke and text painted with a tiling pattern are the same pixels in
/// every cut. No Dart battery sees it: the band height is an internal knob no
/// API sets.
#[test]
fn bands_equal_the_whole_canvas_on_strokes_and_text_painted_with_a_pattern() {
    for fixture in &super::pattern_paint::fixtures() {
        prove(fixture);
    }
}

/// Text in each rendering mode 0 to 7, stroked and clipping, is the same
/// pixels in every cut. No Dart battery sees it: the band height is an
/// internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_text_in_every_rendering_mode() {
    for fixture in &super::text_render_modes::fixtures() {
        prove(fixture);
    }
}

/// No Dart battery sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_shadings() {
    for fixture in fixtures()
        .iter()
        .filter(|f| ["axial", "radial", "mesh"].contains(&f.name))
    {
        prove(fixture);
    }
}

/// No Dart battery sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_forms_annotations_and_images() {
    for fixture in fixtures()
        .iter()
        .filter(|f| ["forms", "annotations", "images"].contains(&f.name))
    {
        prove(fixture);
    }
}

/// An image read through open row cursors is the same image in every cut:
/// each transform class, each kind of mask, an inline image and each codec,
/// at every dpi and every band height. No Dart battery sees it: the band
/// height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_images_at_every_transform_class() {
    for fixture in &image_fixtures() {
        prove(fixture);
    }
}
