//! A build without rendering refuses to redact an image, an inline image or a
//! mesh that meets a box, and passes one that does not. No Dart battery sees
//! this: the trimmed engine is a different build, and the refusal is raised
//! by the pass before anything is recorded or written. The module is compiled
//! only without the `rendering` feature, so it runs under
//! `cargo test --no-default-features --features icc,legacy-crypto,native-bridge`
//! and is type-checked by every keep-set that leaves rendering out.

use std::collections::HashMap;

use crate::content::graphics_state::Matrix;
use crate::error::{Error, Result};
use crate::host::content::{Mode, Operators};
use crate::object::{Object, ObjectRef};
use crate::redaction::text_engine::FontMetrics;
use crate::redaction::transform::{ImageResources, MeshPattern, MeshSource, RedactPass};
use crate::redaction::{RedactionOptions, RedactionRegion, RegionSet};

struct Stub;

impl FontMetrics for Stub {
    fn width(&self, _font: &str, _code: u32) -> f32 {
        500.0
    }
}

fn region(x0: f32, y0: f32, x1: f32, y1: f32) -> RegionSet {
    let mut set = RegionSet::new(0);
    set.push(RedactionRegion::from_rect(x0, y0, x1, y1, Some([0.0, 0.0, 0.0])));
    set
}

fn mesh() -> MeshSource {
    let decode: Vec<Object> = [0, 100, 0, 100, 0, 1, 0, 1, 0, 1]
        .into_iter()
        .map(Object::Integer)
        .collect();
    let dict = HashMap::from([
        ("ShadingType".to_string(), Object::Integer(4)),
        ("Decode".to_string(), Object::Array(decode)),
    ]);
    MeshSource {
        reference: ObjectRef::new(7, 0),
        object: Object::stream(dict, bytes::Bytes::new()),
        shading_type: 4,
    }
}

fn resources() -> ImageResources {
    let mut resources = ImageResources::none();
    resources.images.insert("Im1".to_string(), ObjectRef::new(5, 0));
    resources.meshes.insert("Sh1".to_string(), mesh());
    resources.patterns.insert(
        "P1".to_string(),
        MeshPattern { reference: ObjectRef::new(8, 0), matrix: Matrix::identity(), mesh: mesh() },
    );
    resources
}

fn redact(content: &[u8], regions: &RegionSet) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let resources = resources();
    let opts = RedactionOptions::default();
    RedactPass::new(regions, &opts, &Stub, &resources)
        .run(Operators::new(content, Mode::Full), &mut out)?;
    Ok(out)
}

fn assert_refused(content: &[u8], regions: &RegionSet) {
    match redact(content, regions) {
        Err(Error::Unsupported(message)) => assert!(
            message.contains("this build leaves out rendering"),
            "the refusal must name the build: {message}"
        ),
        other => panic!("a covered object must refuse, got {other:?}"),
    }
}

const IMAGE: &[u8] = b"q 100 0 0 100 50 50 cm /Im1 Do Q\n";
const INLINE: &[u8] = b"q 100 0 0 100 50 50 cm\nBI /W 1 /H 1 /BPC 8 /CS /G ID \x80\nEI\nQ\n";
const SHADING: &[u8] = b"q 1 0 0 1 50 50 cm /Sh1 sh Q\n";
const PATTERN: &[u8] = b"/Pattern cs /P1 scn\n50 50 100 100 re f\n";

/// A box over part of the object: a straddle for the image and the mesh.
fn over() -> RegionSet {
    region(60.0, 60.0, 90.0, 90.0)
}

/// A box far from every object.
fn away() -> RegionSet {
    region(500.0, 500.0, 520.0, 520.0)
}

/// No battery sees this: the build that refuses leaves out rendering, and the
/// pass raises the refusal before any byte is returned.
#[test]
fn an_image_under_a_box_is_refused() {
    assert_refused(IMAGE, &over());
}

/// No battery sees this: the build that refuses leaves out rendering, and an
/// image the box covers whole is refused, not dropped.
#[test]
fn an_image_inside_a_box_is_refused() {
    assert_refused(IMAGE, &region(0.0, 0.0, 300.0, 300.0));
}

/// No battery sees this: the pass runs over bare operator bytes, so only the
/// pass shows that an uncovered image keeps its `Do`.
#[test]
fn an_image_outside_every_box_passes_unchanged() {
    let out = redact(IMAGE, &away()).unwrap();
    assert!(
        String::from_utf8_lossy(&out).contains("/Im1 Do"),
        "the Do must survive: {}",
        String::from_utf8_lossy(&out)
    );
}

/// No battery sees this: the build that refuses leaves out rendering, and the
/// pass raises the refusal before any byte is returned.
#[test]
fn an_inline_image_under_a_box_is_refused() {
    assert_refused(INLINE, &over());
}

/// No battery sees this: the pass runs over bare operator bytes, so only the
/// pass shows that an uncovered inline image is written again.
#[test]
fn an_inline_image_outside_every_box_passes() {
    let out = redact(INLINE, &away()).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("BI"), "the inline image must survive");
}

/// No battery sees this: the build that refuses leaves out rendering, and the
/// pass raises the refusal before any byte is returned.
#[test]
fn a_mesh_shading_under_a_box_is_refused() {
    assert_refused(SHADING, &over());
}

/// No battery sees this: the pass runs over bare operator bytes, so only the
/// pass shows that an uncovered mesh keeps its `sh`.
#[test]
fn a_mesh_shading_outside_every_box_passes() {
    let out = redact(SHADING, &away()).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("/Sh1 sh"), "the sh must survive");
}

/// No battery sees this: the build that refuses leaves out rendering, and the
/// pass raises the refusal before any byte is returned.
#[test]
fn a_mesh_pattern_under_a_box_is_refused() {
    assert_refused(PATTERN, &over());
}

/// No battery sees this: the pass runs over bare operator bytes, so only the
/// pass shows that an uncovered mesh pattern keeps its colour operator.
#[test]
fn a_mesh_pattern_outside_every_box_passes() {
    let out = redact(PATTERN, &away()).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("/P1 scn"), "the pattern colour must survive");
}
