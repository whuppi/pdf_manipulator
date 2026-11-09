//! What a saved file holds of the mesh shadings and forms under a region, and
//! what the page renders as. A mesh primitive is a byte record in the
//! shading's stream: the record of a covered primitive exists only there, so
//! its bytes in the saved file show a primitive that was not dropped. A form's
//! covered coordinates are written once, in a mark of the form under the box.

use super::redact_path_save::{
    assert_absent_everywhere, assert_fill_inside_source_outside, save, BOX, RED,
};
use super::redact_save::{obj, render, stream, write_pdf, DPI};
use super::support::{contains, open_editor_from_bytes, stream_object};
use crate::object::ObjectRef;
use crate::rendering::{render_page, RenderOptions};
use crate::PdfDocument;

/// A mesh vertex record: an edge flag of 0, the coordinates as hundredths of
/// a point in 16 bits, and 8-bit colour components.
fn vertex(x: f32, y: f32, rgb: [u8; 3]) -> Vec<u8> {
    let be = |c: f32| ((c * 100.0).round() as u16).to_be_bytes();
    [&[0u8][..], &be(x), &be(y), &rgb].concat()
}

/// A triangle of three such records.
fn triangle(points: [(f32, f32); 3], rgb: [[u8; 3]; 3]) -> Vec<u8> {
    (0..3).flat_map(|i| vertex(points[i].0, points[i].1, rgb[i])).collect()
}

/// A square Coons patch with straight edges, its corner at (`x`, `y`) and its
/// side `s`: the flag, twelve boundary points counterclockwise from the corner
/// and four corner colours.
fn patch(x: f32, y: f32, s: f32, rgb: [[u8; 3]; 4]) -> Vec<u8> {
    let t = s / 3.0;
    let points = [
        (x, y),
        (x, y + t),
        (x, y + 2.0 * t),
        (x, y + s),
        (x + t, y + s),
        (x + 2.0 * t, y + s),
        (x + s, y + s),
        (x + s, y + 2.0 * t),
        (x + s, y + t),
        (x + s, y),
        (x + 2.0 * t, y),
        (x + t, y),
    ];
    let be = |c: f32| ((c * 100.0).round() as u16).to_be_bytes();
    let mut out = vec![0u8];
    for (px, py) in points {
        out.extend_from_slice(&be(px));
        out.extend_from_slice(&be(py));
    }
    for colour in rgb {
        out.extend_from_slice(&colour);
    }
    out
}

const MESH_DICT: &str = "/ColorSpace /DeviceRGB /BitsPerCoordinate 16 /BitsPerComponent 8 \
                         /BitsPerFlag 8 /Decode [0 655.35 0 655.35 0 1 0 1 0 1]";

/// A page painting the mesh object 5 with `sh` (or as a pattern fill), then an
/// axial shading through a clip that crosses the box.
fn mesh_page(shading_type: u8, data: &[u8], contents: &str, extra: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    // The pattern names the mesh, so it is in the file only for the page that fills with it.
    let pattern = contents.contains("/P1");
    let patterns = if pattern { "/Pattern << /P1 6 0 R >> " } else { "" };
    let sh1 = if pattern { "" } else { "/Sh1 5 0 R " };
    let mut objects = vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        obj(
            3,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
                 /Resources << {patterns}/Shading << {sh1}/Sh2 << /ShadingType 2 \
                 /ColorSpace /DeviceRGB /Coords [0 0 300 0] /Function << /FunctionType 2 \
                 /Domain [0 1] /C0 [1 0.6 0] /C1 [0 0.6 1] /N 1 >> >> >> >> >>"
            ),
        ),
        stream(4, contents.as_bytes()),
        (5, stream_object(5, &format!("/ShadingType {shading_type} {MESH_DICT}"), data)),
    ];
    if pattern {
        objects.push(obj(6, "<< /PatternType 2 /Shading 5 0 R >>"));
    }
    objects.extend(extra);
    write_pdf(&objects)
}

const CLIPPED_AXIAL: &str = "q 100 120 100 80 re W n /Sh2 sh Q\n";

/// The first vertex record of the triangle under the box.
fn covered_triangle() -> Vec<u8> {
    triangle([(30.5, 145.25), (90.75, 150.5), (60.25, 162.75)], [[0x11, 0x22, 0x33], [0x44, 0x55, 0x66], [0x77, 0x88, 0x99]])
}

fn kept_triangle() -> Vec<u8> {
    triangle([(200.0, 20.0), (260.0, 20.0), (230.0, 60.0)], [[255, 0, 0], [0, 255, 0], [0, 0, 255]])
}

/// Redacts the box of `src` and saves it with garbage collection on and off;
/// `check` is given each saved file.
fn redacted(src: &[u8], tag: &str, check: impl Fn(&[u8], bool)) {
    for garbage_collect in [true, false] {
        let mut editor = open_editor_from_bytes(src, tag);
        editor.add_redaction(0, BOX, Some(RED)).unwrap();
        editor.apply_redactions_destructive(Default::default()).unwrap();
        check(&save(&mut editor, garbage_collect), garbage_collect);
    }
}

/// Whether the render has a pixel painted at page point (`x`, `y`).
fn painted(img: &image::RgbaImage, x: f32, y: f32) -> bool {
    let scale = DPI as f32 / 72.0;
    img.get_pixel((x * scale) as u32, ((300.0 - y) * scale) as u32).0[3] > 0
}

/// A type 4 mesh with a triangle under the box and one outside it, and an axial
/// shading through a clip that crosses the box: the covered triangle's bytes
/// leave the file with the original shading, the other triangle stays, and the
/// page renders as the fill inside and the source outside. The original
/// stream is the only place the covered record exists, so no battery over the
/// pixels or the extracted text can see it.
#[test]
fn mesh_triangle_under_the_box_is_dropped_and_the_rest_renders_as_the_source() {
    let data = [kept_triangle(), covered_triangle()].concat();
    let src = mesh_page(4, &data, &format!("/Sh1 sh\n{CLIPPED_AXIAL}"), Vec::new());
    let before = render(&src);
    assert!(painted(&before, 230.0, 33.0), "the fixture draws the outside triangle");
    redacted(&src, "redact_mesh4", |out, garbage_collect| {
        let covered = &covered_triangle()[..7];
        assert_absent_everywhere(out, covered, garbage_collect);
        assert_absent_everywhere(out, &covered_triangle()[7..14], garbage_collect);
        assert!(contains(out, &kept_triangle()[..7]), "gc={garbage_collect}: the outside triangle is gone");
        assert_fill_inside_source_outside(&before, &render(out));
    });
}

/// A mesh whose every primitive is under the box loses its `sh`: no
/// shading object is left to hold the primitives.
#[test]
fn mesh_fully_under_the_box_is_dropped_whole() {
    let src = mesh_page(4, &covered_triangle(), "/Sh1 sh\n", Vec::new());
    redacted(&src, "redact_mesh_whole", |out, garbage_collect| {
        assert_absent_everywhere(out, &covered_triangle()[..7], garbage_collect);
        assert!(!contains(out, b"/Sh1"), "gc={garbage_collect}: the sh is still named");
    });
}

/// A type 6 patch mesh with a patch under the box and one outside it.
#[test]
fn coons_patch_under_the_box_is_dropped_and_the_rest_renders_as_the_source() {
    let rgb = [[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]];
    let kept = patch(200.0, 100.0, 30.0, rgb);
    let covered = patch(40.5, 138.25, 30.0, [[0x11, 0x22, 0x33], [0x44, 0x55, 0x66], [0x77, 0x88, 0x99], [0xAA, 0xBB, 0xCC]]);
    let src = mesh_page(6, &[kept.clone(), covered.clone()].concat(), "/Sh1 sh\n", Vec::new());
    let before = render(&src);
    assert!(painted(&before, 215.0, 115.0), "the fixture draws the outside patch");
    redacted(&src, "redact_mesh6", |out, garbage_collect| {
        assert_absent_everywhere(out, &covered[..5], garbage_collect);
        assert_absent_everywhere(out, &covered[49..], garbage_collect);
        assert!(contains(out, &kept[..5]), "gc={garbage_collect}: the outside patch is gone");
        assert_fill_inside_source_outside(&before, &render(out));
    });
}

/// A mesh used as a shading pattern fill: the pattern is copied over a copy of
/// the mesh without the covered triangle, and the page-wide fill that paints
/// it is cut around the box.
#[test]
fn mesh_pattern_fill_loses_the_covered_triangle() {
    let data = [kept_triangle(), covered_triangle()].concat();
    let src = mesh_page(4, &data, "/Pattern cs /P1 scn 0 0 300 300 re f\n", Vec::new());
    let before = render(&src);
    redacted(&src, "redact_mesh_pattern", |out, garbage_collect| {
        assert_absent_everywhere(out, &covered_triangle()[..7], garbage_collect);
        assert!(contains(out, &kept_triangle()[..7]), "gc={garbage_collect}: the outside triangle is gone");
        assert_fill_inside_source_outside(&before, &render(out));
    });
}

/// Covered coordinates of the form fixture: the right edge of the straddling
/// rectangle and a rectangle wholly under the box, in the inner form's space.
const COVERED: &[&str] = &["22.500000", "-19.500000", "301.000000"];
/// Coordinates that stay: the outer form's rectangle and the straddling
/// rectangle's outside edge.
const PUBLIC: &[&str] = &["401.000000", "-89.750000"];

/// A page drawing form 5 at half size, and when `second_page` a page 2 that
/// draws it the same way. Form 5 draws a public rectangle and form 6, which has
/// its own matrix, and form 6 draws a rectangle across the left edge of the box
/// and one wholly inside it.
fn form_fixture(second_page: bool) -> Vec<u8> {
    let kids = if second_page { "[3 0 R 8 0 R]" } else { "[3 0 R]" };
    let count = if second_page { 2 } else { 1 };
    let mut objects = vec![
        obj(1, "<< /Type /Catalog /Pages 2 0 R >>"),
        obj(2, &format!("<< /Type /Pages /Kids {kids} /Count {count} >>")),
        obj(
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R \
             /Resources << /XObject << /Fm1 5 0 R >> >> >>",
        ),
        stream(4, b"q 0.5 0 0 0.5 0 0 cm /Fm1 Do Q\n"),
        (
            5,
            stream_object(
                5,
                "/Type /XObject /Subtype /Form /BBox [0 0 600 600] \
                 /Resources << /XObject << /Fm2 6 0 R >> >>",
                b"0 0 1 rg 401.000000 41.000000 80.000000 80.000000 re f /Fm2 Do\n",
            ),
        ),
        (
            6,
            stream_object(
                6,
                "/Type /XObject /Subtype /Form /BBox [-100 0 600 600] /Matrix [1 0 0 1 100 0] \
                 /Resources << >>",
                b"1 0.5 0 rg -89.750000 290.750000 m 22.500000 290.750000 l 22.500000 321.250000 l \
                  -89.750000 321.250000 l h f 0 0.5 0 rg -19.500000 301.000000 40.000000 20.000000 re f\n",
            ),
        ),
    ];
    if second_page {
        objects.push(obj(
            8,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 9 0 R \
             /Resources << /XObject << /Fm1 5 0 R >> >> >>",
        ));
        objects.push(stream(9, b"q 0.5 0 0 0.5 0 0 cm /Fm1 Do Q\n"));
    }
    write_pdf(&objects)
}

/// A form two deep across the box: its covered coordinates are gone from the
/// whole file, the rest renders as the source outside the box, and the box
/// renders as the fill.
#[test]
fn nested_form_across_the_box_loses_its_covered_marks() {
    let src = form_fixture(false);
    let before = render(&src);
    redacted(&src, "redact_form_nested", |out, garbage_collect| {
        for secret in COVERED {
            assert_absent_everywhere(out, secret.as_bytes(), garbage_collect);
        }
        for public in PUBLIC {
            assert!(contains(out, public.as_bytes()), "gc={garbage_collect}: {public} is gone");
        }
        assert_fill_inside_source_outside(&before, &render(out));
    });
}

/// The same form on a page nobody redacts stays whole: the original form
/// objects are in the file because page 2 names them, page 2 renders as it
/// did, and page 1 holds copies with none of the covered coordinates.
#[test]
fn form_shared_with_an_unredacted_page_stays_and_the_redacted_page_gets_copies() {
    let src = form_fixture(true);
    let render_at = |bytes: &[u8], page: usize| {
        let doc = PdfDocument::from_bytes(bytes.to_vec()).unwrap();
        let rendered =
            render_page(&doc, page, &RenderOptions::with_dpi(DPI).with_transparent_background()).unwrap();
        image::load_from_memory(&rendered.data).unwrap().to_rgba8()
    };
    let before_second = render_at(&src, 1);
    redacted(&src, "redact_form_shared", |out, garbage_collect| {
        for secret in COVERED {
            assert!(contains(out, secret.as_bytes()), "gc={garbage_collect}: the shared form lost {secret}");
        }
        assert_eq!(render_at(out, 1), before_second, "gc={garbage_collect}: page 2 changed");

        // Page 1's forms: the copies, followed from its resources.
        let doc = PdfDocument::from_bytes(out.to_vec()).unwrap();
        let first = doc.get_page(0).unwrap();
        let named_form = |holder: &crate::object::Object| -> ObjectRef {
            let resources = doc.resolve_object(holder.as_dict().unwrap().get("Resources").unwrap()).unwrap();
            let xobjects = doc.resolve_object(resources.as_dict().unwrap().get("XObject").unwrap()).unwrap();
            let entries = xobjects.as_dict().unwrap();
            assert_eq!(entries.len(), 1, "gc={garbage_collect}: the resources name {entries:?}");
            entries.values().next().unwrap().as_reference().unwrap()
        };
        let outer_ref = named_form(&first);
        assert_ne!(outer_ref.id, 5, "gc={garbage_collect}: page 1 still draws the shared form");
        let outer = doc.load_object(outer_ref).unwrap();
        let inner_ref = named_form(&outer);
        assert_ne!(inner_ref.id, 6, "gc={garbage_collect}: the copy still draws the shared inner form");
        let inner = doc.load_object(inner_ref).unwrap();
        let text = inner.decode_stream_data().unwrap();
        for secret in COVERED {
            assert!(!contains(&text, secret.as_bytes()), "gc={garbage_collect}: the copy holds {secret}");
        }
        assert!(contains(&text, b"-89.750000"), "gc={garbage_collect}: the copy lost its outside edge");
    });
}
