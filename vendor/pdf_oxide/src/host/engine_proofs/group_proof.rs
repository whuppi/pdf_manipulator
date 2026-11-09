//! Groups, soft masks, blends, patterns, Type 3 glyphs and text clips cut
//! into bands render the pixels one band does.
//!
//! The same proof as `band_proof`: each fixture is rendered with the band
//! height forced to 1, 17 and 256 rows and as one band, at 36, 72, 150 and
//! 300 dpi, and every pixel must equal that of one render of the whole canvas.
//! The fixtures reach every offscreen buffer the renderer makes: transparency
//! groups isolated and not and with knockout, soft masks of both kinds with
//! `/BC` and `/TR`, every non-separable blend mode, groups nested eight deep,
//! tiling patterns whose cell is on each side of the raster budget, shading
//! patterns, Type 3 glyph procedures and each text clip mode.

use super::band_proof::{prove, Fixture, HELVETICA};
use super::support::stream_object;

const AXIAL: &str = "/Shading << /Sh << /ShadingType 2 /ColorSpace /DeviceRGB \
     /Coords [0 0 200 200] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0.2 0] \
     /C1 [0 0.3 1] /N 1 >> /Extend [true true] >> >>";

const HALF: &str = "/ExtGState << /Half << /ca 0.5 /CA 0.5 >> >>";

fn form(number: u32, bbox: &str, group: &str, resources: &str, content: &str) -> Vec<u8> {
    stream_object(
        number,
        &format!(
            "/Type /XObject /Subtype /Form /BBox [{bbox}] {group} /Resources << {resources} >>"
        ),
        content.as_bytes(),
    )
}

fn dictionary(number: u32, body: &str) -> Vec<u8> {
    format!("{number} 0 obj\n<< {body} >>\nendobj\n").into_bytes()
}

/// Isolated, non-isolated and knockout groups, each painted with a constant
/// alpha over a background, one of them turned.
fn groups() -> Fixture {
    let inside = "1 0 0 rg 0 0 60 50 re f 0 0.8 0 rg 30 20 60 50 re f /Half gs 0 0 1 rg 15 10 50 50 re f";
    let mut fixture = Fixture::new(
        "groups",
        "0.9 0.9 0.6 rg 0 0 200 200 re f 0.1 0.1 0.8 rg 0 90 200 20 re f 0.8 0.2 0.5 rg 0 0 30 200 re f\n\
         q /Half gs 1 0 0 1 20 10 cm /G1 Do Q\n\
         q /Half gs 1 0 0 1 110 10 cm /G2 Do Q\n\
         q /Half gs 1 0 0 1 20 130 cm /G3 Do Q\n\
         q /Half gs 0.8 0.6 -0.6 0.8 120 100 cm /G4 Do Q\n\
         q /Half gs 1 0 0 1 20 70 cm /G1 Do Q\n",
    );
    fixture.resources = format!("/XObject << /G1 5 0 R /G2 6 0 R /G3 7 0 R /G4 8 0 R >> {HALF}");
    fixture.extra = vec![
        form(5, "0 0 90 70", "/Group << /S /Transparency /I true >>", HALF, inside),
        form(6, "0 0 90 70", "/Group << /S /Transparency /I false >>", HALF, inside),
        form(7, "0 0 90 70", "/Group << /S /Transparency /I false /K true >>", HALF, inside),
        form(8, "0 0 90 70", "/Group << /S /Transparency /I true /K true >>", HALF, inside),
    ];
    fixture
}

/// The four non-separable blend modes and some separable ones, painted on a
/// page and as a form.
fn blends() -> Fixture {
    let mut fixture = Fixture::new(
        "blends",
        "0.9 0.5 0.1 rg 0 0 100 200 re f 0.1 0.6 0.8 rg 100 0 100 200 re f\n\
         0.3 0.8 0.3 rg 0 70 200 60 re f 0.7 0.2 0.7 rg 40 0 30 200 re f\n\
         q /Hue gs 0.9 0.2 0.2 rg 10 10 80 80 re f Q\n\
         q /Sat gs 0.2 0.9 0.2 rg 110 10 80 80 re f Q\n\
         q /Col gs 0.2 0.2 0.9 rg 10 110 80 80 re f Q\n\
         q /Lum gs 0.9 0.9 0.2 rg 110 110 80 80 re f Q\n\
         q /Mul gs 0.8 0.5 0.9 rg 50 50 100 100 re f Q\n\
         q /Scr gs 0.4 0.4 0.9 rg 20 20 60 160 re f Q\n\
         q /Dif gs 0.9 0.9 0.9 rg 130 20 50 160 re f Q\n\
         q /Ovl gs 0.5 0.9 0.5 rg 60 150 120 30 re f Q\n\
         q /Hue gs 1 0 0 1 30 60 cm /G1 Do Q\n\
         q /Lum gs 0.6 0.4 -0.4 0.6 100 40 cm /G1 Do Q\n\
         q /Col gs 1 0 0 1 90 130 cm /G2 Do Q\n",
    );
    fixture.resources = "/XObject << /G1 5 0 R /G2 6 0 R >> /ExtGState << \
         /Hue << /BM /Hue >> /Sat << /BM /Saturation >> /Col << /BM /Color >> \
         /Lum << /BM /Luminosity >> /Mul << /BM /Multiply >> /Scr << /BM /Screen >> \
         /Dif << /BM /Difference >> /Ovl << /BM /Overlay >> >>"
        .to_string();
    fixture.extra = vec![
        form(
            5,
            "0 0 90 70",
            "/Group << /S /Transparency /I true >>",
            "",
            "0.9 0.1 0.1 rg 0 0 60 50 re f 0.1 0.9 0.1 rg 30 20 60 50 re f",
        ),
        form(
            6,
            "0 0 80 60",
            "",
            "/ExtGState << /Mul << /BM /Multiply >> >>",
            "0.2 0.4 0.9 rg 0 0 50 60 re f /Mul gs 0.9 0.8 0.1 rg 30 0 50 60 re f",
        ),
    ];
    fixture
}

/// Soft masks of both kinds with `/BC` and `/TR`.
fn soft_masks() -> Fixture {
    let mut fixture = Fixture::new(
        "softmasks",
        "0.3 0.3 0.9 rg 0 0 200 200 re f\n\
         q /SA gs 1 0.8 0 rg 10 10 180 80 re f Q\n\
         q /SL gs 0.9 0.1 0.1 rg 10 110 180 80 re f Q\n\
         q /ST gs 0.1 0.8 0.2 rg 60 40 130 130 re f Q\n\
         q /SA gs 1 0 0 1 100 20 cm /Fm Do Q\n\
         q /SL gs 0.7 0.7 -0.7 0.7 60 30 cm /Fm Do Q\n",
    );
    fixture.resources = "/XObject << /Fm 7 0 R >> /ExtGState << \
         /SA << /SMask << /Type /Mask /S /Alpha /G 5 0 R >> >> \
         /SL << /SMask << /Type /Mask /S /Luminosity /G 6 0 R /BC [0.3] >> >> \
         /ST << /SMask << /Type /Mask /S /Luminosity /G 6 0 R /BC [0.8] \
         /TR << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [0] /N 1 >> >> >> >>"
        .to_string();
    fixture.extra = vec![
        form(
            5,
            "0 0 200 200",
            "/Group << /S /Transparency /CS /DeviceGray >>",
            HALF,
            "0 g 20 20 60 60 re f /Half gs 100 20 m 180 20 l 140 100 l f 1 g 40 120 120 50 re f",
        ),
        form(
            6,
            "10 10 190 190",
            "/Group << /S /Transparency /CS /DeviceGray >>",
            AXIAL,
            "q 0 100 200 100 re W n /Sh sh Q 0.5 g 40 140 120 40 re f 0.2 g 30 30 100 50 re f",
        ),
        form(
            7,
            "0 0 80 90",
            "/Group << /S /Transparency /I true >>",
            "",
            "0.9 0.2 0.1 rg 0 0 80 50 re f 0.1 0.2 0.9 rg 20 30 60 60 re f",
        ),
    ];
    fixture
}

/// Eight groups, each in the last, alternately isolated, with knockout at the
/// third and sixth and a soft mask at the fifth.
fn nested() -> Fixture {
    let mut fixture = Fixture::new("nested", "0.95 g 0 0 200 200 re f q 1 0 0 1 12 12 cm /N1 Do Q\n");
    fixture.resources = "/XObject << /N1 5 0 R >>".to_string();
    let mut objects = Vec::new();
    for level in 1..=8u32 {
        let size = 176.0 - 14.0 * level as f32;
        let colour = [0.9 - 0.08 * level as f32, 0.2 + 0.09 * level as f32, 0.5 + 0.04 * level as f32];
        let isolated = if level % 2 == 1 { "true" } else { "false" };
        let knockout = if level == 3 || level == 6 { " /K true" } else { "" };
        let mask = if level == 5 { "/SM gs " } else { "" };
        let next = if level < 8 {
            format!("q {mask}0.97 0.12 -0.12 0.97 7 5 cm /N{} Do Q", level + 1)
        } else {
            "0 0 0 rg 4 4 12 12 re f".to_string()
        };
        let content = format!(
            "{:.2} {:.2} {:.2} rg 0 0 {size:.1} {size:.1} re f /Half gs 0 0 0 rg 3 3 9 9 re f \
             1 1 1 rg {:.1} 3 9 9 re f /Full gs {next}",
            colour[0], colour[1], colour[2], size - 12.0
        );
        let resources = format!(
            "/XObject << /N{} {} 0 R >> /ExtGState << /Half << /ca 0.6 >> /Full << /ca 1 >> \
             /SM << /SMask << /Type /Mask /S /Luminosity /G 13 0 R >> >> >>",
            level + 1,
            5 + level
        );
        objects.push(form(
            4 + level,
            &format!("0 0 {size:.1} {size:.1}"),
            &format!("/Group << /S /Transparency /I {isolated}{knockout} >>"),
            &resources,
            &content,
        ));
    }
    objects.push(form(
        13,
        "0 0 100 100",
        "/Group << /S /Transparency /CS /DeviceGray >>",
        AXIAL,
        "0.4 g 0 0 100 100 re f q 0 0 100 60 re W n /Sh sh Q",
    ));
    fixture.extra = objects;
    fixture
}

/// Coloured and uncoloured tiling patterns with small cells, turned cells, and
/// a cell of 180 by 180 points: its raster is under the 2 MiB budget at the
/// lower dpis and over it at the higher ones.
fn tiling() -> Fixture {
    let mut fixture = Fixture::new(
        "tiling",
        "/Pattern cs /P4 scn 0 0 200 200 re f\n\
         /Pattern cs /P1 scn 10 10 90 90 re f\n\
         /P2 scn 110 10 80 90 re f\n\
         /CSU cs 0.2 0.7 0.3 /P3 scn 10 110 90 80 re f\n\
         /Pattern cs /P2 scn BT /F1 44 Tf 100 140 Td (Pat) Tj ET\n\
         /Pattern CS /P1 SCN 8 w 20 20 m 180 190 l S\n",
    );
    fixture.resources = format!(
        "{HELVETICA} /Pattern << /P1 5 0 R /P2 6 0 R /P3 7 0 R /P4 8 0 R >> \
         /ColorSpace << /CSU [/Pattern /DeviceRGB] >>"
    );
    let pattern = |number: u32, extra: &str, content: &[u8]| {
        stream_object(
            number,
            &format!("/Type /Pattern /PatternType 1 /TilingType 1 /Resources << >> {extra}"),
            content,
        )
    };
    fixture.extra = vec![
        pattern(
            5,
            "/PaintType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Matrix [1 0 0 1 3 4]",
            b"1 0 0 rg 0 0 5 5 re f 0 0 1 rg 5 5 5 5 re f",
        ),
        pattern(
            6,
            "/PaintType 1 /BBox [0 0 14 14] /XStep 16 /YStep 16 /Matrix [0.866 0.5 -0.5 0.866 0 0]",
            b"0.1 0.6 0.2 rg 0 0 14 14 re f 1 1 0 RG 1.5 w 0 0 m 14 14 l S",
        ),
        pattern(
            7,
            "/PaintType 2 /BBox [0 0 12 12] /XStep 12 /YStep 12",
            b"0 0 8 8 re f 6 6 m 12 12 l 12 6 l f",
        ),
        pattern(
            8,
            "/PaintType 1 /BBox [0 0 180 180] /XStep 190 /YStep 190 /Matrix [1 0 0 1 5 5]",
            b"0.85 0.9 1 rg 0 0 180 180 re f 0.3 0.3 0.7 RG 3 w 10 10 160 160 re S \
              0.7 0.4 0.2 rg 40 40 m 140 60 l 90 150 l f 0 g 0 0 m 180 180 l S",
        ),
    ];
    fixture
}

/// Shading patterns as the fill of a rect, a path, text and a stroke.
fn shading_patterns() -> Fixture {
    let mut fixture = Fixture::new(
        "shading_patterns",
        "/Pattern cs /SP1 scn 10 10 90 90 re f\n\
         /SP2 scn 110 10 m 190 10 l 150 100 l f\n\
         /SP1 scn BT /F1 60 Tf 10 120 Td (Sh) Tj ET\n\
         /Pattern CS /SP2 SCN 10 w 20 190 m 180 110 l S\n\
         /SP2 scn 100 110 90 40 re f\n",
    );
    fixture.resources = format!("{HELVETICA} /Pattern << /SP1 5 0 R /SP2 6 0 R >>");
    let function = "<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >>";
    fixture.extra = vec![
        dictionary(
            5,
            &format!(
                "/PatternType 2 /Matrix [1 0 0 1 0 0] /Shading << /ShadingType 2 \
                 /ColorSpace /DeviceRGB /Coords [10 10 190 190] /Function {function} \
                 /Extend [true true] >>"
            ),
        ),
        dictionary(
            6,
            &format!(
                "/PatternType 2 /Matrix [0.9 0.3 -0.3 0.9 20 0] /Shading << /ShadingType 3 \
                 /ColorSpace /DeviceRGB /Coords [100 60 5 100 60 90] /Function {function} \
                 /Extend [false true] >>"
            ),
        ),
    ];
    fixture
}

/// Type 3 text at several sizes and turns, glyphs of both `d0` and `d1`.
fn type3() -> Fixture {
    let mut fixture = Fixture::new(
        "type3",
        "BT /T3 60 Tf 10 130 Td 0.2 0.5 0.8 rg (abab) Tj ET\n\
         BT /T3 24 Tf 10 100 Td 0.8 0.1 0.1 rg (baba) Tj ET\n\
         BT /T3 40 Tf 0.866 0.5 -0.5 0.866 40 20 Tm 0.1 0.6 0.2 rg (abab) Tj ET\n\
         BT /T3 90 Tf 100 30 Td 0.9 0.7 0.1 rg (b) Tj ET\n",
    );
    fixture.resources = "/Font << /T3 5 0 R >>".to_string();
    fixture.extra = vec![
        dictionary(
            5,
            "/Type /Font /Subtype /Type3 /FontBBox [0 0 100 100] /FontMatrix [0.01 0 0 0.01 0 0] \
             /CharProcs << /sq 6 0 R /tri 7 0 R >> \
             /Encoding << /Type /Encoding /Differences [97 /sq /tri] >> \
             /FirstChar 97 /LastChar 98 /Widths [100 100]",
        ),
        stream_object(6, "", b"100 0 0 0 100 100 d1 10 10 80 80 re f 30 30 40 40 re f*"),
        stream_object(
            7,
            "",
            b"100 0 d0 1 0 0 rg 0 0 m 100 0 l 50 100 l f 0 0 1 RG 4 w 20 10 m 80 10 l 50 80 l h S",
        ),
    ];
    fixture
}

/// Every text clip mode, with a shading and a fill painted through the clip.
fn text_clips() -> Fixture {
    let mut fixture = Fixture::new(
        "textclips",
        "q BT /F1 70 Tf 4 Tr 0.2 0.5 0.9 rg 10 130 Td (Ab) Tj ET 0.9 0.2 0.2 rg 0 120 200 40 re f Q\n\
         q BT /F1 70 Tf 5 Tr 3 w 0 0.5 0 RG 100 130 Td (Cd) Tj ET /Sh sh Q\n\
         q BT /F1 70 Tf 6 Tr 2 w 0.5 0 0.5 RG 0.9 0.8 0.1 rg 10 50 Td (Ef) Tj ET 0 0 200 100 re f Q\n\
         q BT /F1 70 Tf 7 Tr 100 50 Td (Gh) Tj ET /Sh sh Q\n\
         q BT /F1 40 Tf 7 Tr 0.866 0.5 -0.5 0.866 20 5 Tm (rot) Tj ET /Sh sh Q\n",
    );
    fixture.resources = format!("{HELVETICA} {AXIAL}");
    fixture
}

/// Several of them together: a group holding a text clip under a soft mask,
/// painted with a blend mode.
fn combined() -> Fixture {
    let mut fixture = Fixture::new(
        "combined",
        "0.9 0.8 0.5 rg 0 0 200 200 re f 0.3 0.6 0.4 rg 0 0 200 70 re f\n\
         q /Mix gs 1 0 0 1 10 20 cm /G1 Do Q\n\
         q /Mix gs 0.8 0.6 -0.6 0.8 100 20 cm /G1 Do Q\n",
    );
    fixture.resources = "/XObject << /G1 5 0 R >> /ExtGState << /Mix << /BM /Hue /ca 0.8 >> >>".to_string();
    fixture.extra = vec![
        form(
            5,
            "0 0 160 160",
            "/Group << /S /Transparency /I true >>",
            &format!("{HELVETICA} {AXIAL} /ExtGState << /SM << /SMask << /Type /Mask /S /Luminosity /G 6 0 R >> >> >>"),
            "/SM gs q BT /F1 80 Tf 7 Tr 5 50 Td (Gr) Tj ET /Sh sh Q 0.1 0.1 0.6 rg 0 0 160 20 re f",
        ),
        form(
            6,
            "0 0 160 160",
            "/Group << /S /Transparency /CS /DeviceGray >>",
            AXIAL,
            "0.2 g 0 0 160 160 re f 1 g 20 20 100 100 re f",
        ),
    ];
    fixture
}

/// Isolated, non-isolated and knockout groups. No Dart battery sees it: the
/// band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_isolated_and_knockout_groups() {
    prove(&groups());
}

/// The non-separable blend modes and some separable ones. No Dart battery sees
/// it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_blend_modes() {
    prove(&blends());
}

/// Soft masks of both kinds with `/BC` and `/TR`. No Dart battery sees it: the
/// band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_soft_masks() {
    prove(&soft_masks());
}

/// Groups nested eight deep, with knockout and a soft mask among them. No Dart
/// battery sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_groups_nested_eight_deep() {
    prove(&nested());
}

/// Tiling patterns with small cells, turned cells and a cell on each side of the
/// raster budget. No Dart battery sees it: the band height is an internal knob
/// no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_tiling_patterns() {
    prove(&tiling());
}

/// The pattern-cell rule has two sides. A cell whose raster fits the band
/// budget (2 MiB) is rendered once to a texture, and a turned lattice is then
/// painted flat in the cell's average colour, which is what the texture path
/// has always done. A cell over the budget is drawn tile by tile into the band,
/// so its content shows at any turn. The same pattern is rendered at 150 dpi
/// (a 1 MiB cell raster) and at 300 dpi (4 MiB).
#[test]
fn a_pattern_cell_over_the_budget_is_drawn_tile_by_tile_and_one_under_it_is_a_texture() {
    use crate::document::PdfDocument;
    use crate::rendering::{render_page, RenderOptions};

    let mut fixture = Fixture::new("cellrule", "/Pattern cs /P scn 0 0 200 200 re f\n");
    fixture.resources = "/Pattern << /P 5 0 R >>".to_string();
    fixture.extra = vec![stream_object(
        5,
        "/Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /Resources << >> \
         /BBox [0 0 180 180] /XStep 190 /YStep 190 \
         /Matrix [0.866 0.5 -0.5 0.866 0 0]",
        b"1 0 0 rg 0 0 90 180 re f 0 0 1 rg 90 0 90 180 re f",
    )];
    let doc = PdfDocument::from_bytes(fixture.pdf()).expect("fixture parses");
    let colours = |dpi: u32| {
        let rendered = render_page(&doc, 0, &RenderOptions::with_dpi(dpi).with_transparent_background())
            .expect("renders");
        let pixels = image::load_from_memory(&rendered.data).expect("PNG decodes").to_rgba8();
        // Inside the page edge, where a fill's coverage is partial.
        let mut seen = std::collections::BTreeSet::new();
        for y in 4..pixels.height() - 4 {
            for x in 4..pixels.width() - 4 {
                seen.insert(pixels.get_pixel(x, y).0);
            }
        }
        seen.len()
    };
    let (under, over) = (colours(150), colours(300));
    assert_eq!(under, 1, "a cell under the budget is a texture: a turned lattice is flat");
    assert!(over >= 2, "a cell over the budget is drawn tile by tile: its content shows ({over})");
}

/// Shading patterns as the fill of a rect, a path and text. No Dart battery
/// sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_shading_patterns() {
    prove(&shading_patterns());
}

/// Type 3 text of both glyph kinds, at several sizes and turns. No Dart battery
/// sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_type3_text() {
    prove(&type3());
}

/// Every text clip mode. No Dart battery sees it: the band height is an
/// internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_text_clips() {
    prove(&text_clips());
}

/// A group holding a text clip under a soft mask, painted with a blend mode. No
/// Dart battery sees it: the band height is an internal knob no API sets.
#[test]
fn bands_equal_the_whole_canvas_on_a_group_with_a_clip_a_mask_and_a_blend() {
    prove(&combined());
}
