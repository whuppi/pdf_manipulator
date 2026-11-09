//! The `watermark` op places its ink where the position on the wire says.
//!
//! Each proof opens a one-page editor and applies the op through the host's
//! request door (`handle_request`, `editorMutate` with `editOp` `watermark`),
//! with the position fields in the names the Dart codec sends. It saves the
//! page, renders it and finds the bounding box of the ink: a corner position
//! puts the ink in the margin band of that corner, a tiled position puts ink
//! in every cell, an exact position keeps the ink inside the given rectangle,
//! and a corner the engine does not know is refused.
//!
//! No Dart battery can observe this: a battery sees the saved bytes or a
//! render through the same lanes, and neither tells it where a watermark was
//! asked to go against where its ink landed unless it measures the ink, and
//! the wire-field names the engine reads are not visible from Dart at all.

use crate::cancel::CancelCheck;
use crate::host::binary_codec::{ErrorKind, Request};
use crate::host::bridge_api::handle_request;
use crate::host::engine_state::EngineState;
use crate::rendering::{render_page, RenderOptions};
use crate::PdfDocument;

use super::support::pdf_from_objects;

const PAGE_W: f32 = 600.0;
const PAGE_H: f32 = 800.0;
/// One pixel a point.
const DPI: u32 = 72;

enum Field<'a> {
    Bool(bool),
    Int(i32),
    Real(f64),
    Text(&'a str),
}

/// A request frame as the Dart codec lays it out.
fn frame(op: &str, fields: &[(&str, Field)]) -> Vec<u8> {
    let mut out = vec![op.len() as u8];
    out.extend_from_slice(op.as_bytes());
    out.extend_from_slice(&(fields.len() as u16).to_le_bytes());
    for (key, value) in fields {
        out.push(key.len() as u8);
        out.extend_from_slice(key.as_bytes());
        match value {
            Field::Bool(b) => {
                out.push(4);
                out.push(*b as u8);
            },
            Field::Int(n) => {
                out.push(1);
                out.extend_from_slice(&n.to_le_bytes());
            },
            Field::Real(x) => {
                out.push(3);
                out.extend_from_slice(&x.to_le_bytes());
            },
            Field::Text(s) => {
                out.push(5);
                out.extend_from_slice(&(s.len() as u32).to_le_bytes());
                out.extend_from_slice(s.as_bytes());
            },
        }
    }
    out
}

fn blank_page() -> Vec<u8> {
    pdf_from_objects(&[
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] >>\nendobj\n"
        )
        .into_bytes(),
    ])
}

/// What the lane answers to a request.
fn answer(state: &mut EngineState, request: &[u8], source: Option<&[u8]>) -> Vec<u8> {
    handle_request(state, request, source, Vec::new(), Vec::new(), &CancelCheck::none())
}

/// The watermark op with `position` fields added to a large, opaque,
/// unrotated black watermark on every page of a fresh one-page document. Returns the
/// lane's answer and the saved bytes (None when the op was refused).
fn apply(position: &[(&str, Field)]) -> (Vec<u8>, Option<Vec<u8>>) {
    let mut state = EngineState::new();
    let opened = answer(&mut state, &frame("editorOpen", &[]), Some(&blank_page()));
    assert_eq!(opened[0], 1, "the editor opens");
    let handle = {
        let mut as_request = vec![0u8];
        as_request.extend_from_slice(&opened[1..]);
        Request::parse(&as_request).unwrap().get_i32("handleId").unwrap()
    };

    let mut fields: Vec<(&str, Field)> = vec![
        ("handleId", Field::Int(handle)),
        ("editOp", Field::Text("watermark")),
        ("pagesAll", Field::Bool(true)),
        ("text", Field::Text("WM")),
        ("fontSize", Field::Real(40.0)),
        ("rotation", Field::Real(0.0)),
        ("opacity", Field::Real(1.0)),
        ("r", Field::Real(0.0)),
        ("g", Field::Real(0.0)),
        ("b", Field::Real(0.0)),
        ("layer", Field::Text("foreground")),
    ];
    fields.extend(position.iter().map(|(k, v)| {
        (*k, match v {
            Field::Bool(b) => Field::Bool(*b),
            Field::Int(n) => Field::Int(*n),
            Field::Real(x) => Field::Real(*x),
            Field::Text(s) => Field::Text(s),
        })
    }));
    let response = answer(&mut state, &frame("editorMutate", &fields), None);
    if response[0] != 1 {
        return (response, None);
    }
    let saved = state.editors.get_mut(&(handle as u32)).unwrap().save_to_bytes().unwrap();
    (response, Some(saved))
}

/// The ink's bounding box in page points, `[left, bottom, right, top]`, and
/// the rendered image. A pixel is ink when it is not transparent.
fn ink(saved: &[u8]) -> ([f32; 4], image::RgbaImage) {
    let doc = PdfDocument::from_bytes(saved.to_vec()).unwrap();
    let rendered =
        render_page(&doc, 0, &RenderOptions::with_dpi(DPI).with_transparent_background()).unwrap();
    let img = image::load_from_memory(&rendered.data).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (PAGE_W as u32, PAGE_H as u32), "one pixel a point");
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, px) in img.enumerate_pixels() {
        if px.0[3] > 0 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    assert!(x0 < x1, "the page has ink");
    ([x0 as f32, PAGE_H - y1 as f32, x1 as f32, PAGE_H - y0 as f32], img)
}

/// Whether any pixel of `img` inside the page-point box is ink.
fn has_ink(img: &image::RgbaImage, [l, b, r, t]: [f32; 4]) -> bool {
    (l as u32..r as u32).any(|x| {
        ((PAGE_H - t) as u32..(PAGE_H - b) as u32).any(|y| img.get_pixel(x, y).0[3] > 0)
    })
}

fn assert_inside(got: [f32; 4], want: [f32; 4], what: &str) {
    const SLACK: f32 = 1.0;
    assert!(
        got[0] >= want[0] - SLACK
            && got[1] >= want[1] - SLACK
            && got[2] <= want[2] + SLACK
            && got[3] <= want[3] + SLACK,
        "{what}: ink {got:?} is not inside {want:?}"
    );
}

/// The default position keeps the ink in the middle of the page. It pins the
/// baseline the other positions are told apart from.
#[test]
fn centre_puts_the_ink_in_the_middle() {
    let (_, saved) = apply(&[("posType", Field::Int(0))]);
    let (bbox, _) = ink(&saved.unwrap());
    assert_inside(bbox, [PAGE_W * 0.1, PAGE_H * 0.3, PAGE_W * 0.9, PAGE_H * 0.7], "centre");
    let (cx, cy) = ((bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0);
    assert!((cx - PAGE_W / 2.0).abs() < 30.0 && (cy - PAGE_H / 2.0).abs() < 30.0,
        "centre: ink centre ({cx}, {cy}) is not the page centre");
}

/// Each corner puts the ink in that corner's margin band: clear of the
/// margins on its two edges, and within the corner box the position names.
/// Dart cannot see this: the corner arrives by name on the wire and only a
/// measured render tells a corner placement from the centre fallback.
#[test]
fn corners_put_the_ink_in_their_margin_band() {
    const MX: f32 = 30.0;
    const MY: f32 = 50.0;
    let (bw, bh) = (PAGE_W * 0.3, PAGE_H * 0.08);
    let cases = [
        ("topLeft", [MX, PAGE_H - MY - bh, MX + bw, PAGE_H - MY]),
        ("topRight", [PAGE_W - MX - bw, PAGE_H - MY - bh, PAGE_W - MX, PAGE_H - MY]),
        ("bottomLeft", [MX, MY, MX + bw, MY + bh]),
        ("bottomRight", [PAGE_W - MX - bw, MY, PAGE_W - MX, MY + bh]),
    ];
    for (corner, band) in cases {
        let (_, saved) = apply(&[
            ("posType", Field::Int(1)),
            ("corner", Field::Text(corner)),
            ("marginX", Field::Real(MX as f64)),
            ("marginY", Field::Real(MY as f64)),
        ]);
        let (bbox, _) = ink(&saved.unwrap());
        assert_inside(bbox, band, corner);
    }
}

/// A tiled position repeats the watermark in every cell of the grid, and no
/// cell's ink leaves the cell. Dart cannot see this: it reads the grid size
/// from `columns` and `rows`, which only a render of every cell tells apart
/// from the engine's own default grid.
#[test]
fn tiled_puts_ink_in_every_cell() {
    let (_, saved) = apply(&[
        ("posType", Field::Int(2)),
        ("columns", Field::Int(3)),
        ("rows", Field::Int(2)),
    ]);
    let (bbox, img) = ink(&saved.unwrap());
    let (cw, ch) = (PAGE_W / 3.0, PAGE_H / 2.0);
    for row in 0..2 {
        for col in 0..3 {
            let cell = [
                col as f32 * cw,
                row as f32 * ch,
                (col + 1) as f32 * cw,
                (row + 1) as f32 * ch,
            ];
            assert!(has_ink(&img, cell), "tiled: no ink in cell ({col}, {row}) {cell:?}");
        }
    }
    assert_inside(bbox, [0.0, 0.0, PAGE_W, PAGE_H], "tiled");

    // The engine's own grid when the fields are ignored is three columns by
    // four rows, whose cells hold ink too: only the number of separate ink
    // bands across and down tells the asked grid from it.
    let column_ink: Vec<bool> =
        (0..PAGE_W as u32).map(|x| (0..PAGE_H as u32).any(|y| img.get_pixel(x, y).0[3] > 0)).collect();
    let row_ink: Vec<bool> =
        (0..PAGE_H as u32).map(|y| (0..PAGE_W as u32).any(|x| img.get_pixel(x, y).0[3] > 0)).collect();
    assert_eq!(bands(&column_ink), 3, "tiled: ink bands across");
    assert_eq!(bands(&row_ink), 2, "tiled: ink bands down");
}

/// The runs of ink in a projection, runs closer than a glyph gap merged.
fn bands(projection: &[bool]) -> usize {
    const GAP: usize = 20;
    let mut count = 0;
    let mut since_ink = GAP;
    for &inked in projection {
        if inked {
            if since_ink >= GAP {
                count += 1;
            }
            since_ink = 0;
        } else {
            since_ink += 1;
        }
    }
    count
}

/// An exact position keeps the ink inside the rectangle given. Dart cannot
/// see this: `posX`, `posY`, `posW` and `posH` are wire names, and only a
/// measured render tells the rectangle from the engine's fallback box.
#[test]
fn exact_keeps_the_ink_inside_the_rect() {
    let rect = [100.0, 200.0, 350.0, 280.0];
    let (_, saved) = apply(&[
        ("posType", Field::Int(3)),
        ("posX", Field::Real(rect[0] as f64)),
        ("posY", Field::Real(rect[1] as f64)),
        ("posW", Field::Real((rect[2] - rect[0]) as f64)),
        ("posH", Field::Real((rect[3] - rect[1]) as f64)),
    ]);
    let (bbox, _) = ink(&saved.unwrap());
    assert_inside(bbox, rect, "exact");
}

/// A corner name the engine does not know is an invalid-argument answer, not
/// a placement. Dart cannot see this: its codec only ever sends the four
/// names, so the refusal is reachable only by a frame the codec never builds.
#[test]
fn unknown_corner_is_refused() {
    let (response, saved) = apply(&[
        ("posType", Field::Int(1)),
        ("corner", Field::Text("middleLeft")),
        ("marginX", Field::Real(20.0)),
        ("marginY", Field::Real(20.0)),
    ]);
    assert!(saved.is_none(), "an unknown corner is refused");
    assert_eq!(response[0], 0, "an error response");
    assert_eq!(response[1], ErrorKind::InvalidArgument as u8, "typed InvalidArgument");
}
