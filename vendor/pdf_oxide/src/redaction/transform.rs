//! Write-time redaction of one page's content.
//!
//! The page's operators stream through [`RedactPass`] one at a time: each is
//! decided by the text engine and its survivors are written at once, so the
//! rewritten content never exists whole in memory. The same pass runs into a
//! byte counter when a redaction is queued and into the file when it is
//! saved; both produce the same [`RedactionReport`].

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{Read, Write};

mod meshes;
mod paths;

pub use meshes::{MeshPattern, MeshSource};
#[cfg(feature = "rendering")]
pub(crate) use meshes::{filtered_dict, write_filtered};

use paths::{is_path_op, paint_of, redact_path, PathRedaction};
use super::classify::{classify, Classification};
use super::font_scrub::FontCodes;
#[cfg(feature = "rendering")]
use super::image_prune::invert_affine;
use super::image_prune::classify_image_placement;
use super::options::{RedactionOptions, RedactionReport};
#[cfg(feature = "rendering")]
use super::overlay::resolved_fill;
use super::overlay::region_overlay_ops;
use super::region::RegionSet;
use super::serialize::serialize_operator;
use super::text_engine::{FontMetrics, TextPass};
use crate::content::operators::Operator;
use crate::content::graphics_state::Matrix;
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::host::content::{InlineImages, Mode, Operators};
#[cfg(feature = "rendering")]
use crate::host::images::redact_copy::covered_span;
use crate::object::{Object, ObjectRef};

/// How deep forms may nest inside a redacted form. A deeper tree refuses at
/// the dry run, as a cycle does.
pub const MAX_FORM_DEPTH: usize = 24;

/// The page's image XObjects, by their resource name: the lookup a `Do`
/// decides from. A page whose resource dictionaries the engine cannot read
/// resolves to nothing: every `Do` passes through and nothing is redacted.
pub struct ImageResources {
    /// Resource name → the XObject's reference, for XObjects the engine
    /// reads as images (`/Subtype /Image` or `/ImageMask true`).
    pub images: HashMap<String, ObjectRef>,
    /// Resource name → the XObject's reference, for Form XObjects.
    pub forms: HashMap<String, ObjectRef>,
    /// Resource name → the mesh shading (types 4 to 7) a `sh` paints.
    pub meshes: HashMap<String, MeshSource>,
    /// Resource name → the shading pattern whose shading is a mesh.
    pub patterns: HashMap<String, MeshPattern>,
    /// Resource name → the line style of an ExtGState the page names. A name
    /// whose dictionary cannot be read maps to `None`, and a `gs` of it refuses.
    pub line_styles: HashMap<String, Option<LineStyle>>,
    /// Resource name → the `/MCID` of a marked-content property list the
    /// stream names with `BDC`.
    pub mcids: HashMap<String, u32>,
    /// Resource name → the marked-content property list a `BDC` names.
    pub properties: HashMap<String, PropertyList>,
}

/// The keys of a marked-content property list that hold text of the content
/// they mark.
pub const MARKED_STRINGS: [&str; 3] = ["ActualText", "Alt", "E"];

/// A `/Properties` entry.
#[derive(Clone, Debug)]
pub struct PropertyList {
    /// The list's object when it is indirect.
    pub reference: Option<ObjectRef>,
    /// The list, its values as the resources hold them.
    pub dict: HashMap<String, Object>,
}

impl PropertyList {
    /// Whether the list carries any of [`MARKED_STRINGS`].
    fn has_strings(&self) -> bool {
        MARKED_STRINGS.iter().any(|key| self.dict.contains_key(*key))
    }

    /// The list without its text keys.
    fn without_strings(&self) -> HashMap<String, Object> {
        let mut dict = self.dict.clone();
        for key in MARKED_STRINGS {
            dict.remove(key);
        }
        dict
    }
}

/// The /Properties name a copy of the list `name` is written under: the
/// first `name_rp`, with `x` added while the resources already hold it.
fn property_copy_name(images: &ImageResources, name: &str) -> String {
    let mut copy = format!("{name}_rp");
    while images.properties.contains_key(&copy) {
        copy.push('x');
    }
    copy
}

/// A marked-content sequence open at the operator being read.
struct MarkedSeq {
    mcid: Option<u32>,
    /// For a `BDC` that carries [`MARKED_STRINGS`]: its place among those
    /// `BDC`s of the stream, and the /Properties name when the list is named.
    strings: Option<(u32, Option<String>)>,
    /// Whether the sequence lost or changed a mark.
    tainted: bool,
}

impl ImageResources {
    /// An empty registry: every `Do` passes through unchanged, and a `gs`
    /// refuses, since no ExtGState can be read.
    pub fn none() -> Self {
        Self {
            images: HashMap::new(),
            forms: HashMap::new(),
            meshes: HashMap::new(),
            patterns: HashMap::new(),
            line_styles: HashMap::new(),
            mcids: HashMap::new(),
            properties: HashMap::new(),
        }
    }
}

/// A form XObject as the pass reads it.
pub struct FormView {
    /// The form's `/Matrix`.
    pub matrix: Matrix,
    /// The form's `/BBox`, in form space.
    pub bbox: Rect,
    /// The form's own resources, as the pass decides from them.
    pub resources: ImageResources,
    /// The metrics of the fonts the form's resources name.
    pub fonts: Box<dyn FontMetrics>,
}

/// Opens the forms a pass meets. The document supplies it; the pass holds no
/// document of its own.
pub trait FormLoader {
    /// The form's matrix, box, resources and fonts.
    ///
    /// # Errors
    /// [`Error::Unsupported`] when the form cannot be read in full: a form
    /// that cannot be read cannot be shown to be outside a region.
    fn view(&self, form: ObjectRef) -> Result<FormView>;

    /// The form's content operators.
    ///
    /// # Errors
    /// When the form's stream cannot be opened.
    fn content(&self, form: ObjectRef) -> Result<Operators<Box<dyn Read>>>;
}

/// One overwritten image placement, recorded when the mark is called and
/// written after the page's content. `fills` are `(rect, rgb)` pairs in first
/// intersect-win order: which region's fill a covered pixel takes.
#[derive(Clone, Debug, PartialEq)]
pub struct ImagePlacement {
    /// The `/Name` the pass writes the copy under (never a source name).
    pub name: String,
    /// The source image XObject.
    pub source: ObjectRef,
    /// The covered rect, `u0 v0 u1 v1`, fractions of the image's unit
    /// square, lower-left origin in image space: `u0` and `v0` the lower
    /// left corner, `u1` and `v1` the upper right.
    pub u0: f32,
    /// See [`ImagePlacement::u0`].
    pub v0: f32,
    /// See [`ImagePlacement::u0`].
    pub u1: f32,
    /// See [`ImagePlacement::u0`].
    pub v1: f32,
    /// `((u0, v0, u1, v1), rgb)` for every region the placement covers.
    pub fills: Vec<([f32; 4], [f32; 3])>,
}

impl ImagePlacement {
    /// The placement's covered rect as `[u0, v0, u1, v1]`.
    pub fn covered_rect(&self) -> [f32; 4] {
        [self.u0, self.v0, self.u1, self.v1]
    }
}

/// One copy of a mesh shading with its covered primitives dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshCopy {
    /// The resource name the pass writes the copy under.
    pub name: String,
    /// The source shading stream.
    pub source: ObjectRef,
    /// The matrix from shading space to page space the primitives were
    /// decided under, and the save decides them again under.
    pub to_page: Matrix,
    /// How many primitives the copy keeps.
    pub kept: usize,
    /// How many primitives the copy drops.
    pub dropped: usize,
}

/// One copy of a shading pattern whose shading is a [`MeshCopy`].
#[derive(Clone, Debug, PartialEq)]
pub struct PatternCopy {
    /// The resource name the pass writes the copy under.
    pub name: String,
    /// The source pattern dictionary.
    pub source: ObjectRef,
    /// The index in [`Scope::meshes`] of the shading the copy names.
    pub mesh: usize,
}

/// One specialized copy of a form: its content redacted under the matrix
/// that places it on the page.
#[derive(Clone, Debug, PartialEq)]
pub struct FormCopy {
    /// The resource name the pass writes the copy under.
    pub name: String,
    /// The source form.
    pub source: ObjectRef,
    /// The form's matrix composed with the CTM at its `Do`: the matrix the
    /// form's content was decided under.
    pub ctm: Matrix,
    /// The line style the form is drawn under, which its content inherits.
    pub style: LineStyle,
    /// What the form's own pass queued.
    pub scope: Scope,
}

/// Redacts the content of the form copy `form`: the same pass the page's
/// dry run made over it, writing its operators to `out`.
///
/// # Errors
/// As [`RedactPass::run`].
pub fn redact_form<R: Read, W: Write>(
    regions: &RegionSet,
    opts: &RedactionOptions,
    view: &FormView,
    loader: &dyn FormLoader,
    form: &FormCopy,
    content: Operators<R>,
    out: &mut W,
) -> Result<PassOutcome> {
    RedactPass::new(regions, opts, &*view.fonts, &view.resources)
        .with_forms(loader)
        .with_marked_plan(&form.scope.tainted_marked)
        .starting(form.ctm, vec![form.source], &form.style)
        .run_form(content, out)
}

/// Everything one content stream's pass queued and kept, which is the same
/// for the page and for each form copy.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scope {
    /// One per overwritten image copy, in decision order.
    pub placements: Vec<ImagePlacement>,
    /// One per mesh copy a `sh` or a shading pattern uses.
    pub meshes: Vec<MeshCopy>,
    /// One per shading pattern copy.
    pub patterns: Vec<PatternCopy>,
    /// One per form copy.
    pub forms: Vec<FormCopy>,
    /// The XObject names the stream's surviving `Do`s still use.
    pub kept_xobject_names: HashSet<String>,
    /// The /Shading names a surviving `sh` still uses, and those a `sh`
    /// replaced or dropped: a replaced name stays only when also kept.
    pub kept_shadings: HashSet<String>,
    /// See [`Scope::kept_shadings`].
    pub replaced_shadings: HashSet<String>,
    /// The /Pattern names a surviving colour operator still uses, and those
    /// one replaced.
    pub kept_patterns: HashSet<String>,
    /// See [`Scope::kept_patterns`].
    pub replaced_patterns: HashSet<String>,
    /// By font resource name, in name order, the codes the stream removed and
    /// the codes it still shows.
    pub fonts: Vec<(String, FontCodes)>,
    /// The `/MCID`s of the marked content that lost or changed a mark.
    pub mcids: BTreeSet<u32>,
    /// The places, among the stream's `BDC` operators that carry
    /// [`MARKED_STRINGS`], of those whose marked content lost or changed a
    /// mark. The save writes these without the strings.
    pub tainted_marked: BTreeSet<u32>,
    /// One per property list a tainted `BDC` named: the copy without the
    /// strings that the tainted `BDC`s name instead.
    pub properties: Vec<PropertyCopy>,
    /// The /Properties names a surviving `BDC` still uses, and those a
    /// copy replaced: a replaced name stays only when also kept.
    pub kept_properties: HashSet<String>,
    /// See [`Scope::kept_properties`].
    pub replaced_properties: HashSet<String>,
}

/// A marked-content property list written again without its text keys.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyCopy {
    /// The /Properties name the tainted `BDC`s use for the copy.
    pub name: String,
    /// The source list's name.
    pub source: String,
    /// The list without [`MARKED_STRINGS`].
    pub dict: HashMap<String, Object>,
}

impl Scope {
    /// Every image placement of this scope and of the forms below it.
    pub fn all_placements(&self) -> Vec<&ImagePlacement> {
        let mut out: Vec<&ImagePlacement> = self.placements.iter().collect();
        for form in &self.forms {
            out.extend(form.scope.all_placements());
        }
        out
    }
}

/// The line-style entries of an ExtGState the stroke bound reads: `/LW`,
/// `/LJ`, `/ML` and `/LC`. An entry the dictionary does not set is `None`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    /// Line width (`/LW`).
    pub lw: Option<f32>,
    /// Line join style (`/LJ`): 0 miter, 1 round, 2 bevel.
    pub lj: Option<u8>,
    /// Miter limit (`/ML`).
    pub ml: Option<f32>,
    /// Line cap style (`/LC`): 0 butt, 1 round, 2 projecting square.
    pub lc: Option<u8>,
}

/// Rounds the u/v keys of a covered rect: two `Do`s of one source over the
/// same rect share one copy.
#[cfg(feature = "rendering")]
fn rect_key(u0: f32, v0: f32, u1: f32, v1: f32) -> u128 {
    (u0.to_bits() as u128) << 96
        | (v0.to_bits() as u128) << 64
        | (u1.to_bits() as u128) << 32
        | v1.to_bits() as u128
}

/// What the pass hands the caller instead of a decided operator.
enum Decision {
    /// The operator goes; nothing is written.
    Dropped,
    /// The operator is written as the rewritten one it carries.
    UsedAs(Operator),
}

/// What one run of the pass produced: its report and the queue the save
/// writes from, and the objects the queue replaced.
#[derive(Clone, Debug, PartialEq)]
pub struct PassOutcome {
    /// The counts the call reports.
    pub report: RedactionReport,
    /// The copies queued and the names kept.
    pub scope: Scope,
    /// The object ids of every source the pass overwrote, dropped or
    /// copied: images, mesh shadings, shading patterns and forms, in this
    /// stream and in the forms below it.
    pub originals: Vec<u32>,
}

impl PassOutcome {
    /// Whether the pass changed nothing: no mark removed or cut, no copy
    /// queued.
    pub fn is_unchanged(&self) -> bool {
        let report = RedactionReport {
            regions: self.report.regions,
            ..RedactionReport::default()
        };
        self.report == report
            && self.scope.placements.is_empty()
            && self.scope.meshes.is_empty()
            && self.scope.patterns.is_empty()
            && self.scope.forms.is_empty()
    }
}

/// A source's matrix as a key: two uses of one source under the same matrix
/// share one copy.
type CopyKey = (u32, [u32; 6]);

fn copy_key(source: ObjectRef, m: &Matrix) -> CopyKey {
    (
        source.id,
        [m.a, m.b, m.c, m.d, m.e, m.f].map(f32::to_bits),
    )
}

/// Redacts one page's operators into a writer.
pub struct RedactPass<'a> {
    regions: &'a RegionSet,
    opts: &'a RedactionOptions,
    text: TextPass<'a>,
    fonts: &'a dyn FontMetrics,
    images: &'a ImageResources,
    /// Opens the forms the stream draws; without it a form `Do` refuses.
    loader: Option<&'a dyn FormLoader>,
    /// The CTM the stream starts under: the page's identity, or the matrix
    /// a form copy is specialized for. Shading patterns map through it.
    base: Matrix,
    /// The forms being redacted above this stream, for the cycle guard.
    active: Vec<ObjectRef>,
    /// The copies the pass queues and the names it keeps.
    scope: Scope,
    /// `source → name → (rect, name)`: a copy already made of the same
    /// source over the same rect, which a second placement shares.
    #[cfg(feature = "rendering")]
    copies: HashMap<u32, HashMap<u128, String>>,
    /// Form and pattern copies already made, by source and matrix.
    copy_names: HashMap<CopyKey, String>,
    /// Mesh copies already made, by source and matrix, as indices in the scope.
    #[cfg(feature = "rendering")]
    mesh_copies: HashMap<CopyKey, usize>,
    /// The sources the pass overwrote, dropped or copied.
    originals: Vec<u32>,
    /// The report totals of the forms below this stream.
    nested: RedactionReport,
    /// The names handed out so far, so each copy's name is new.
    names_made: usize,
    imgs_removed: usize,
    imgs_modified: usize,
    inline_modified: usize,
    /// The rewritten form of the operator being decided, reused.
    ops: Vec<Operator>,
    /// The bytes of one rewritten operator, reused.
    bytes: Vec<u8>,
    /// The construction and clip operators of the path being read, held
    /// until its paint operator decides them.
    path: Vec<Operator>,
    paths_pruned: usize,
    specialized: usize,
    /// The marked-content sequences open at the operator being read.
    marked: Vec<MarkedSeq>,
    /// How many `BDC`s that carry [`MARKED_STRINGS`] the stream has shown.
    strings_seen: u32,
    /// The `BDC`s the save writes without their strings: the places the
    /// dry run recorded in [`Scope::tainted_marked`].
    marked_plan: Option<&'a BTreeSet<u32>>,
}

/// Records a source id once in `originals`, the set the page's overwritten
/// and dropped images keep.
fn note_original(originals: &mut Vec<u32>, id: u32) {
    if !originals.contains(&id) {
        originals.push(id);
    }
}

impl<'a> RedactPass<'a> {
    /// A pass that redacts `regions` with `opts`' padding and overlay, glyph
    /// widths from `fonts`, and the page's image XObjects by `images`.
    pub fn new(
        regions: &'a RegionSet,
        opts: &'a RedactionOptions,
        fonts: &'a dyn FontMetrics,
        images: &'a ImageResources,
    ) -> Self {
        Self {
            regions,
            opts,
            text: TextPass::new(regions, opts.edge_padding, fonts),
            fonts,
            images,
            loader: None,
            base: Matrix::identity(),
            active: Vec::new(),
            scope: Scope::default(),
            #[cfg(feature = "rendering")]
            copies: HashMap::new(),
            copy_names: HashMap::new(),
            #[cfg(feature = "rendering")]
            mesh_copies: HashMap::new(),
            originals: Vec::new(),
            nested: RedactionReport::default(),
            names_made: 0,
            imgs_removed: 0,
            imgs_modified: 0,
            inline_modified: 0,
            ops: Vec::new(),
            bytes: Vec::new(),
            path: Vec::new(),
            paths_pruned: 0,
            specialized: 0,
            marked: Vec::new(),
            strings_seen: 0,
            marked_plan: None,
        }
    }

    /// Lets the pass redact the forms its content draws, opened by `loader`.
    pub fn with_forms(mut self, loader: &'a dyn FormLoader) -> Self {
        self.loader = Some(loader);
        self
    }

    /// Makes the pass write the `BDC`s at the places of `plan`, the
    /// [`Scope::tainted_marked`] of a dry run over the same stream, without
    /// their [`MARKED_STRINGS`]. Without a plan every `BDC` is written as it
    /// was read.
    pub fn with_marked_plan(mut self, plan: &'a BTreeSet<u32>) -> Self {
        self.marked_plan = Some(plan);
        self
    }

    /// Starts the pass under `base` instead of the identity, as the content
    /// of a form placed on the page by that matrix; `active` are the forms
    /// being redacted around it. The line style the form is drawn under
    /// carries in with `style`.
    fn starting(mut self, base: Matrix, active: Vec<ObjectRef>, style: &LineStyle) -> Self {
        self.text = TextPass::new(self.regions, self.opts.edge_padding, self.fonts).with_ctm(base);
        self.text.set_line_style(style);
        self.base = base;
        self.active = active;
        self
    }

    /// Writes the page's rewritten content to `out`: its operators inside
    /// `q … Q`, so the overlays cannot inherit a CTM the content left
    /// behind, then one overlay per region.
    ///
    /// # Errors
    /// - [`Error::Unsupported`] when the content is damaged, or shows text in
    ///   a font whose glyphs cannot be pruned, or holds an image whose
    ///   copy the engine cannot write. Whatever reached `out` before
    ///   the error is not a valid redaction; the caller discards it.
    /// - [`Error::Io`] when `out` fails.
    pub fn run<R: Read, W: Write>(
        self,
        content: Operators<R>,
        out: &mut W,
    ) -> Result<PassOutcome> {
        self.run_then(content, Operators::new(std::io::empty(), Mode::Full), out)
    }

    /// As [`Self::run`] over `content` followed by `then`: the page's own
    /// operators and the operators an edit appends to it (a flattened
    /// widget), which the pass decides as one stream.
    ///
    /// # Errors
    /// As [`Self::run`].
    pub fn run_then<R: Read, T: Read, W: Write>(
        mut self,
        content: Operators<R>,
        then: Operators<T>,
        out: &mut W,
    ) -> Result<PassOutcome> {
        out.write_all(b"q\n")?;
        self.process(content, out)?;
        self.process(then, out)?;
        out.write_all(b"Q\n")?;
        for region in &self.regions.regions {
            out.write_all(&region_overlay_ops(region, self.opts))?;
        }
        Ok(self.finish())
    }

    /// Writes a form's rewritten content to `out`: the operators alone, since
    /// the form is drawn inside the `q … Q` its `Do` implies, and no overlay,
    /// since the page draws them.
    fn run_form<R: Read, W: Write>(
        mut self,
        content: Operators<R>,
        out: &mut W,
    ) -> Result<PassOutcome> {
        self.process(content, out)?;
        Ok(self.finish())
    }

    /// Decides and writes every operator of `content`.
    fn process<R: Read, W: Write>(
        &mut self,
        content: Operators<R>,
        out: &mut W,
    ) -> Result<()> {
        let mut content = content.with_inline_images(InlineImages::Collect);
        for item in content.by_ref() {
            // ── pdf_manipulator patch: a cancel stops the pass within one operator ──
            crate::cancel::poll()?;
            // ── end pdf_manipulator patch ──
            let op = item?;
            if is_path_op(&op) {
                self.path.push(op);
                continue;
            }
            if paint_of(&op).is_some() {
                let pending = std::mem::take(&mut self.path);
                let decided = redact_path(
                    &pending,
                    &op,
                    self.text.gstate(),
                    &self.text.ctm(),
                    self.regions,
                    self.opts.edge_padding,
                )?;
                match decided {
                    PathRedaction::Keep => {
                        for held in &pending {
                            self.emit(out, held)?;
                        }
                        self.emit(out, &op)?;
                    },
                    PathRedaction::Rewritten(written) => {
                        self.paths_pruned += 1;
                        self.touch_marked();
                        for written in &written {
                            self.emit(out, written)?;
                        }
                    },
                }
                continue;
            }
            if !self.path.is_empty() {
                return Err(Error::Unsupported(
                    "a path is not ended by a paint operator; refusing to redact a path that \
                     cannot be read"
                        .to_string(),
                ));
            }
            match &op {
                Operator::BeginMarkedContent { .. } => self.marked.push(MarkedSeq {
                    mcid: None,
                    strings: None,
                    tainted: false,
                }),
                Operator::BeginMarkedContentDict { tag, properties } => {
                    let mcid = match &**properties {
                        Object::Dictionary(dict) => dict
                            .get("MCID")
                            .and_then(Object::as_integer)
                            .and_then(|mcid| u32::try_from(mcid).ok()),
                        Object::Name(name) => self.images.mcids.get(name.as_str()).copied(),
                        _ => None,
                    };
                    let strings = self.marked_strings(properties);
                    self.marked.push(MarkedSeq { mcid, strings, tainted: false });
                    if let Some(written) = self.rewrite_marked(tag, properties) {
                        self.emit(out, &written)?;
                        continue;
                    }
                },
                Operator::EndMarkedContent => {
                    if let Some(seq) = self.marked.pop() {
                        self.keep_properties_of(&seq);
                    }
                },
                _ => {},
            }
            if let Operator::SetExtGState { dict_name } = &op {
                match self.images.line_styles.get(dict_name.as_str()) {
                    Some(Some(style)) => self.text.set_line_style(style),
                    _ => {
                        return Err(Error::Unsupported(format!(
                            "the ExtGState /{dict_name} cannot be read; refusing to redact a \
                             page whose line style is unknown"
                        )))
                    },
                }
            }
            // An image `Do` and an inline image are decided here: the
            // placement, the overlay fill it carries, and the mechanics of
            // the copy. A `Do` of whatever else — a form (T6c), a glyph's
            // resource the pass cannot resolve — passes through and keeps
            // its name on the page. A form `Do`, a `sh` of a mesh and a
            // colour operator naming a mesh pattern are decided the same way.
            let replacement = match &op {
                Operator::Do { name } => {
                    if let Some(source) = self.images.images.get(name.as_str()).copied() {
                        self.decide_do(source, name.as_str())?
                    } else if let Some(source) = self.images.forms.get(name.as_str()).copied() {
                        self.decide_form(source, name.as_str())?
                    } else {
                        None
                    }
                },
                Operator::PaintShading { name } => self.decide_shading(name)?,
                Operator::SetFillColorN { components, name: Some(name) } => {
                    self.decide_pattern(name, |name| Operator::SetFillColorN {
                        components: components.clone(),
                        name: Some(Box::new(name)),
                    })?
                },
                Operator::SetStrokeColorN { components, name: Some(name) } => {
                    self.decide_pattern(name, |name| Operator::SetStrokeColorN {
                        components: components.clone(),
                        name: Some(Box::new(name)),
                    })?
                },
                Operator::InlineImage { dict, data } => self.decide_inline(dict, data)?,
                _ => None,
            };
            match replacement {
                Some(Decision::Dropped) => {
                    self.touch_marked();
                    continue;
                },
                Some(Decision::UsedAs(written)) => {
                    self.touch_marked();
                    self.bytes.clear();
                    serialize_operator(&mut self.bytes, &written);
                    out.write_all(&self.bytes)?;
                    continue;
                },
                None => {},
            }
            self.ops.clear();
            let glyphs_before = self.text.result().glyphs_removed;
            self.text.apply(&op, &mut self.ops);
            if self.text.result().glyphs_removed > glyphs_before {
                self.touch_marked();
            }
            for written in &self.ops {
                if let Operator::Do { name } = written {
                    self.scope.kept_xobject_names.insert(name.clone());
                }
                self.bytes.clear();
                serialize_operator(&mut self.bytes, written);
                out.write_all(&self.bytes)?;
            }
            if self.text.result().unsupported_font {
                return Err(unsupported_font());
            }
        }
        if !content.damage().is_clean() {
            return Err(Error::Unsupported(
                "the content stream is damaged; refusing to redact a stream whose \
                 operators cannot be read in full"
                    .to_string(),
            ));
        }
        if !self.path.is_empty() {
            return Err(Error::Unsupported(
                "a path is not ended by a paint operator; refusing to redact a path that \
                 cannot be read"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// The outcome of the stream the pass has read: its own counts plus those
    /// of the form copies it queued.
    fn finish(mut self) -> PassOutcome {
        for seq in std::mem::take(&mut self.marked) {
            self.keep_properties_of(&seq);
        }
        let totals = self.text.result();
        self.scope.fonts = totals
            .fonts
            .iter()
            .map(|(name, codes)| (name.clone(), codes.clone()))
            .collect();
        let scrubbed = self
            .scope
            .fonts
            .iter()
            .filter(|(_, codes)| codes.has_redaction_only_code())
            .count();
        PassOutcome {
            report: RedactionReport {
                regions: self.regions.len(),
                glyphs_removed: totals.glyphs_removed + self.nested.glyphs_removed,
                annotations_removed: 0,
                fonts_scrubbed: scrubbed + self.nested.fonts_scrubbed,
                bytes_removed: totals.bytes_removed + self.nested.bytes_removed,
                images_modified: self.imgs_modified
                    + self.inline_modified
                    + self.nested.images_modified,
                images_removed: self.imgs_removed + self.nested.images_removed,
                paths_pruned: self.paths_pruned + self.nested.paths_pruned,
                xobjects_specialized: self.specialized + self.nested.xobjects_specialized,
                ..RedactionReport::default()
            },
            scope: self.scope,
            originals: self.originals,
        }
    }

    /// Notes that every marked-content sequence open now lost or changed a
    /// mark, so the structure elements that own them lose their strings.
    fn touch_marked(&mut self) {
        let images = self.images;
        for seq in &mut self.marked {
            self.scope.mcids.extend(seq.mcid);
            if seq.tainted {
                continue;
            }
            seq.tainted = true;
            let Some((place, named)) = &seq.strings else { continue };
            self.scope.tainted_marked.insert(*place);
            let Some(name) = named else { continue };
            self.scope.replaced_properties.insert(name.clone());
            if self.scope.properties.iter().any(|copy| copy.source == *name) {
                continue;
            }
            if let Some(list) = images.properties.get(name) {
                if let Some(reference) = list.reference {
                    note_original(&mut self.originals, reference.id);
                }
                self.scope.properties.push(PropertyCopy {
                    name: property_copy_name(images, name),
                    source: name.clone(),
                    dict: list.without_strings(),
                });
            }
        }
    }

    /// The place and the /Properties name of a `BDC` that carries
    /// [`MARKED_STRINGS`], counted among such `BDC`s; `None` for any other.
    fn marked_strings(&mut self, properties: &Object) -> Option<(u32, Option<String>)> {
        let named = match properties {
            Object::Dictionary(dict) => {
                if !MARKED_STRINGS.iter().any(|key| dict.contains_key(*key)) {
                    return None;
                }
                None
            },
            Object::Name(name) => {
                if !self.images.properties.get(name.as_str()).is_some_and(PropertyList::has_strings) {
                    return None;
                }
                Some(name.clone())
            },
            _ => return None,
        };
        let place = self.strings_seen;
        self.strings_seen += 1;
        Some((place, named))
    }

    /// The `BDC` the plan writes in place of the one just read: its strings
    /// dropped from an inline list, a named list replaced by its copy.
    fn rewrite_marked(&self, tag: &str, properties: &Object) -> Option<Operator> {
        let plan = self.marked_plan?;
        let (place, named) = self.marked.last()?.strings.as_ref()?;
        if !plan.contains(place) {
            return None;
        }
        let properties = match (properties, named) {
            (Object::Dictionary(dict), None) => {
                let mut dict = dict.clone();
                for key in MARKED_STRINGS {
                    dict.remove(key);
                }
                Object::Dictionary(dict)
            },
            (_, Some(name)) => Object::Name(property_copy_name(self.images, name)),
            _ => return None,
        };
        Some(Operator::BeginMarkedContentDict {
            tag: tag.to_string(),
            properties: Box::new(properties),
        })
    }

    /// Notes that a named list is still used by a sequence that lost nothing.
    fn keep_properties_of(&mut self, seq: &MarkedSeq) {
        if let (false, Some((_, Some(name)))) = (seq.tainted, &seq.strings) {
            self.scope.kept_properties.insert(name.clone());
        }
    }

    /// A resource name for a copy of `name`, not one the stream's resources
    /// or the copies queued so far already hold: the first `name_rN` past
    /// the names made.
    fn fresh_name(&mut self, name: &str) -> String {
        let stem = name.trim_start_matches('/');
        loop {
            let candidate = format!("{stem}_r{}", self.names_made);
            self.names_made += 1;
            let taken = self.images.images.contains_key(&candidate)
                || self.images.forms.contains_key(&candidate)
                || self.images.meshes.contains_key(&candidate)
                || self.images.patterns.contains_key(&candidate)
                || self.scope.placements.iter().any(|p| p.name == candidate)
                || self.scope.meshes.iter().any(|m| m.name == candidate)
                || self.scope.patterns.iter().any(|p| p.name == candidate)
                || self.scope.forms.iter().any(|f| f.name == candidate);
            if !taken {
                return candidate;
            }
        }
    }

    /// Writes one operator to `out` through the pass's byte buffer.
    fn emit<W: Write>(&mut self, out: &mut W, op: &Operator) -> Result<()> {
        self.bytes.clear();
        serialize_operator(&mut self.bytes, op);
        out.write_all(&self.bytes)?;
        Ok(())
    }

    /// Decides one image `Do` against the regions through the composed CTM:
    /// the placement queued, and `None` when the `Do` passes through (the
    /// kept one is written byte for byte; the caller re-serialises it).
    #[cfg(feature = "rendering")]
    fn decide_do(&mut self, source: ObjectRef, name: &str) -> Result<Option<Decision>> {
        use super::image_prune::ImageRedaction;
        let ctm = self.text.ctm();
        match classify_image_placement(&ctm, self.regions, self.opts.edge_padding) {
            ImageRedaction::Keep => Ok(None),
            ImageRedaction::DeleteFull => {
                self.imgs_removed += 1;
                note_original(&mut self.originals, source.id);
                Ok(Some(Decision::Dropped))
            },
            ImageRedaction::Overwrite { u0, v0, u1, v1 } => {
                let key = rect_key(u0, v0, u1, v1);
                // The same image over the same covered rect shares one copy;
                // a different rect is a copy of its own.
                let shared = self
                    .copies
                    .get(&source.id)
                    .and_then(|m| m.iter().find(|(k, _)| **k == key))
                    .map(|(_, n)| n.clone());
                let copy_name = match shared {
                    Some(n) => n,
                    None => {
                        let copy_name = self.fresh_name(name);
                        let fills = self.region_fills(&ctm)?;
                        self.scope.placements.push(ImagePlacement {
                            name: copy_name.clone(),
                            source,
                            u0,
                            v0,
                            u1,
                            v1,
                            fills,
                        });
                        note_original(&mut self.originals, source.id);
                        self.imgs_modified += 1;
                        self.copies
                            .entry(source.id)
                            .or_default()
                            .insert(key, copy_name.clone());
                        copy_name
                    },
                };
                Ok(Some(Decision::UsedAs(Operator::Do {
                    name: copy_name,
                })))
            },
        }
    }

    /// Decides one image `Do` in a build without rendering: an image that
    /// meets a region cannot be overwritten, so the pass refuses; one outside
    /// every region passes through.
    #[cfg(not(feature = "rendering"))]
    fn decide_do(&mut self, _source: ObjectRef, _name: &str) -> Result<Option<Decision>> {
        use super::image_prune::ImageRedaction;
        match classify_image_placement(&self.text.ctm(), self.regions, self.opts.edge_padding) {
            ImageRedaction::Keep => Ok(None),
            _ => Err(rendering_left_out("an image")),
        }
    }

    /// Decides one form `Do` through the composed matrix of the form and the
    /// CTM: outside its box the `Do` passes, inside it is dropped, and across
    /// it the form is redacted as a copy specialized for that matrix. A form
    /// the copy would leave as it is passes unchanged.
    fn decide_form(&mut self, source: ObjectRef, name: &str) -> Result<Option<Decision>> {
        let Some(loader) = self.loader else {
            return Err(Error::Unsupported(
                "a form is drawn but no form can be opened; refusing to redact a page whose \
                 forms cannot be read"
                    .to_string(),
            ));
        };
        let view = loader.view(source)?;
        let composed = view.matrix.multiply(&self.text.ctm());
        match classify(&view.bbox, &composed, self.regions, self.opts.edge_padding) {
            Classification::Outside => Ok(None),
            Classification::Inside => {
                self.paths_pruned += 1;
                note_original(&mut self.originals, source.id);
                Ok(Some(Decision::Dropped))
            },
            Classification::Straddle => {
                let key = copy_key(source, &composed);
                if let Some(shared) = self.copy_names.get(&key) {
                    return Ok(Some(Decision::UsedAs(Operator::Do { name: shared.clone() })));
                }
                if self.active.contains(&source) {
                    return Err(Error::Unsupported(
                        "a form draws itself; refusing to redact a cycle of forms".to_string(),
                    ));
                }
                if self.active.len() >= MAX_FORM_DEPTH {
                    return Err(Error::Unsupported(format!(
                        "forms nest more than {MAX_FORM_DEPTH} deep; refusing to redact them"
                    )));
                }
                let gs = self.text.gstate();
                let style = LineStyle {
                    lw: Some(gs.line_width),
                    lj: Some(gs.line_join),
                    ml: Some(gs.miter_limit),
                    lc: Some(gs.line_cap),
                };
                let mut active = self.active.clone();
                active.push(source);
                let inner = RedactPass::new(self.regions, self.opts, &*view.fonts, &view.resources)
                    .with_forms(loader)
                    .starting(composed, active, &style)
                    .run_form(loader.content(source)?, &mut std::io::sink())?;
                if inner.is_unchanged() {
                    return Ok(None);
                }
                self.absorb(&inner.report);
                self.specialized += 1;
                note_original(&mut self.originals, source.id);
                for id in &inner.originals {
                    note_original(&mut self.originals, *id);
                }
                let copy_name = self.fresh_name(name);
                self.copy_names.insert(key, copy_name.clone());
                self.scope.forms.push(FormCopy {
                    name: copy_name.clone(),
                    source,
                    ctm: composed,
                    style,
                    scope: inner.scope,
                });
                Ok(Some(Decision::UsedAs(Operator::Do { name: copy_name })))
            },
        }
    }

    /// Adds the counts of a form copy's pass to this stream's totals.
    fn absorb(&mut self, report: &RedactionReport) {
        self.nested.glyphs_removed += report.glyphs_removed;
        self.nested.bytes_removed += report.bytes_removed;
        self.nested.images_modified += report.images_modified;
        self.nested.images_removed += report.images_removed;
        self.nested.paths_pruned += report.paths_pruned;
        self.nested.xobjects_specialized += report.xobjects_specialized;
        self.nested.fonts_scrubbed += report.fonts_scrubbed;
    }

    /// Decides a `sh` of the mesh `name` under the CTM: it passes when no
    /// primitive meets a region, is dropped when every one does, and
    /// otherwise names a copy without the primitives that do.
    #[cfg(feature = "rendering")]
    fn decide_shading(&mut self, name: &str) -> Result<Option<Decision>> {
        let images = self.images;
        let Some(mesh) = images.meshes.get(name) else {
            return Ok(None);
        };
        let to_page = self.text.ctm();
        let found = meshes::filter(mesh, &to_page, self.regions, self.opts.edge_padding, &mut std::io::sink())?;
        if found.dropped == 0 {
            self.scope.kept_shadings.insert(name.to_string());
            return Ok(None);
        }
        self.scope.replaced_shadings.insert(name.to_string());
        note_original(&mut self.originals, mesh.reference.id);
        self.paths_pruned += 1;
        if found.kept == 0 {
            return Ok(Some(Decision::Dropped));
        }
        let index = self.mesh_copy(name, mesh.reference, to_page, found);
        Ok(Some(Decision::UsedAs(Operator::PaintShading {
            name: self.scope.meshes[index].name.clone(),
        })))
    }

    /// Decides a `sh` of the mesh `name` in a build without rendering: a mesh
    /// that may meet a region cannot be filtered, so the pass refuses; one
    /// outside every region passes.
    #[cfg(not(feature = "rendering"))]
    fn decide_shading(&mut self, name: &str) -> Result<Option<Decision>> {
        let images = self.images;
        let Some(mesh) = images.meshes.get(name) else {
            return Ok(None);
        };
        meshes::refuse_if_met(mesh, &self.text.ctm(), self.regions, self.opts.edge_padding)?;
        self.scope.kept_shadings.insert(name.to_string());
        Ok(None)
    }

    /// Decides a colour operator naming the pattern `name` in a build
    /// without rendering: a pattern whose mesh may meet a region refuses, any
    /// other passes.
    #[cfg(not(feature = "rendering"))]
    fn decide_pattern(
        &mut self,
        name: &str,
        _rename: impl FnOnce(String) -> Operator,
    ) -> Result<Option<Decision>> {
        let images = self.images;
        if let Some(pattern) = images.patterns.get(name) {
            let to_page = pattern.matrix.multiply(&self.base);
            meshes::refuse_if_met(&pattern.mesh, &to_page, self.regions, self.opts.edge_padding)?;
        }
        self.scope.kept_patterns.insert(name.to_string());
        Ok(None)
    }

    /// Decides a colour operator naming the pattern `name`: a pattern whose
    /// mesh loses no primitive passes, any other names a copy of the pattern
    /// over a copy of the mesh, which may be empty. `rename` rebuilds the
    /// operator under the copy's name.
    #[cfg(feature = "rendering")]
    fn decide_pattern(
        &mut self,
        name: &str,
        rename: impl FnOnce(String) -> Operator,
    ) -> Result<Option<Decision>> {
        let images = self.images;
        let Some(pattern) = images.patterns.get(name) else {
            self.scope.kept_patterns.insert(name.to_string());
            return Ok(None);
        };
        let to_page = pattern.matrix.multiply(&self.base);
        let found = meshes::filter(
            &pattern.mesh,
            &to_page,
            self.regions,
            self.opts.edge_padding,
            &mut std::io::sink(),
        )?;
        if found.dropped == 0 {
            self.scope.kept_patterns.insert(name.to_string());
            return Ok(None);
        }
        self.scope.replaced_patterns.insert(name.to_string());
        let key = copy_key(pattern.reference, &to_page);
        let copy_name = match self.copy_names.get(&key) {
            Some(shared) => shared.clone(),
            None => {
                note_original(&mut self.originals, pattern.reference.id);
                note_original(&mut self.originals, pattern.mesh.reference.id);
                self.paths_pruned += 1;
                let mesh = self.mesh_copy(name, pattern.mesh.reference, to_page, found);
                let copy_name = self.fresh_name(name);
                self.copy_names.insert(key, copy_name.clone());
                self.scope.patterns.push(PatternCopy {
                    name: copy_name.clone(),
                    source: pattern.reference,
                    mesh,
                });
                self.specialized += 1;
                copy_name
            },
        };
        Ok(Some(Decision::UsedAs(rename(copy_name))))
    }

    /// The index in the scope of the copy of the mesh `source` filtered under
    /// `to_page`, made if the stream has none yet.
    #[cfg(feature = "rendering")]
    fn mesh_copy(
        &mut self,
        stem: &str,
        source: ObjectRef,
        to_page: Matrix,
        found: crate::rendering::mesh_shading::MeshFilter,
    ) -> usize {
        let key = copy_key(source, &to_page);
        if let Some(&index) = self.mesh_copies.get(&key) {
            return index;
        }
        let name = self.fresh_name(stem);
        self.scope.meshes.push(MeshCopy {
            name,
            source,
            to_page,
            kept: found.kept,
            dropped: found.dropped,
        });
        self.specialized += 1;
        self.mesh_copies.insert(key, self.scope.meshes.len() - 1);
        self.scope.meshes.len() - 1
    }

    /// Every region that meets the placement through `ctm`, as its covered
    /// rect in image u/v space and the overlay fill it carries; first hit
    /// wins for a pixel.
    #[cfg(feature = "rendering")]
    fn region_fills(&self, ctm: &Matrix) -> Result<Vec<([f32; 4], [f32; 3])>> {
        let Some(inv) = invert_affine(ctm) else {
            // The mapping is unreliable: the whole unit square is under one
            // fill, the same over-redaction the classifier's own singular
            // case takes (never under-redact).
            return Ok(vec![(
                [0.0, 0.0, 1.0, 1.0],
                resolved_fill(&self.regions.regions[0], self.opts).unwrap_or([0.0, 0.0, 0.0]),
            )]);
        };
        let unit = crate::geometry::Rect::from_points(0.0, 0.0, 1.0, 1.0);
        let footprint = super::classify::transform_bbox(&unit, ctm);
        let clamp01 = |v: f32| v.clamp(0.0, 1.0);
        let mut out = Vec::new();
        for (index, region) in self.regions.regions.iter().enumerate() {
            let padded = region.padded_rect(self.opts.edge_padding);
            if !padded.intersects(&footprint) {
                continue;
            }
            let mut u0 = f32::INFINITY;
            let mut v0 = f32::INFINITY;
            let mut u1 = f32::NEG_INFINITY;
            let mut v1 = f32::NEG_INFINITY;
            for (px, py) in [
                (padded.left(), padded.top()),
                (padded.right(), padded.top()),
                (padded.right(), padded.bottom()),
                (padded.left(), padded.bottom()),
            ] {
                let p = inv.transform_point(px, py);
                u0 = u0.min(p.x);
                v0 = v0.min(p.y);
                u1 = u1.max(p.x);
                v1 = v1.max(p.y);
            }
            out.push((
                [clamp01(u0), clamp01(v0), clamp01(u1), clamp01(v1)],
                self.region_fill(index),
            ));
        }
        Ok(out)
    }

    /// The overlay colour a region redacts under: the mark's `/IC` colour,
    /// else the configured default when overlay drawing is on, else black —
    /// the plan's "else sample zero" for the pixels a mark of no colour
    /// covers.
    #[cfg(feature = "rendering")]
    fn region_fill(&self, region: usize) -> [f32; 3] {
        resolved_fill(&self.regions.regions[region], self.opts).unwrap_or([0.0, 0.0, 0.0])
    }

    /// Decides one inline image in a build without rendering: as
    /// [`Self::decide_do`], an inline image that meets a region refuses.
    #[cfg(not(feature = "rendering"))]
    fn decide_inline(
        &mut self,
        _dict: &std::collections::HashMap<String, Object>,
        _data: &[u8],
    ) -> Result<Option<Decision>> {
        use super::image_prune::ImageRedaction;
        match classify_image_placement(&self.text.ctm(), self.regions, self.opts.edge_padding) {
            ImageRedaction::Keep => Ok(None),
            _ => Err(rendering_left_out("an inline image")),
        }
    }

    /// Decides one inline image: decode it whole (they are small), fill the
    /// covered pixels, re-emit the bytes the copy stores. The re-emit is
    /// raw when the source was, Flate when it was filtered.
    #[cfg(feature = "rendering")]
    fn decide_inline(
        &mut self,
        dict: &std::collections::HashMap<String, Object>,
        data: &[u8],
    ) -> Result<Option<Decision>> {
        use super::image_prune::ImageRedaction;
        let ctm = self.text.ctm();
        match classify_image_placement(&ctm, self.regions, self.opts.edge_padding) {
            ImageRedaction::Keep => Ok(None),
            ImageRedaction::DeleteFull => {
                self.imgs_removed += 1;
                Ok(Some(Decision::Dropped))
            },
            ImageRedaction::Overwrite { u0, v0, u1, v1 } => {
                let fills = self.region_fills(&ctm)?;
                let filled = fill_inline(dict, data, u0, v0, u1, v1, &fills)?;
                self.inline_modified += 1;
                Ok(Some(Decision::UsedAs(Operator::InlineImage {
                    dict: Box::new(filled.0),
                    data: filled.1,
                })))
            },
        }
    }
}

fn unsupported_font() -> Error {
    Error::Unsupported(
        "destructive text redaction of composite/Type0/unknown font content is not yet \
         supported; refusing rather than risk leaving recoverable text"
            .to_string(),
    )
}

/// The refusal of a build without rendering: image and mesh redaction need
/// the codecs and the mesh reader that only rendering carries.
#[cfg(not(feature = "rendering"))]
pub(crate) fn rendering_left_out(what: &str) -> Error {
    Error::Unsupported(format!(
        "this build leaves out rendering, which image and mesh redaction needs; refusing to \
         redact a page where {what} meets a redaction box"
    ))
}

/// An inline image's covered pixels filled: `(dict, data)` in the re-emitted
/// form. The pixels decode through the extractor the renderer paints them
/// with, the fill is the overlay colour the region under a pixel carries,
/// and the copy is the raster's own bytes: DeviceGray or DeviceRGB at 8
/// bits, `/Decode` dropped (the samples are the physical colour now), a raw
/// byte stream when the source was raw and a Flate one when it was filtered.
#[cfg(feature = "rendering")]
fn fill_inline(
    dict: &HashMap<String, Object>,
    data: &[u8],
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
    fills: &[([f32; 4], [f32; 3])],
) -> Result<(HashMap<String, Object>, Vec<u8>)> {
    let expanded = crate::extractors::images::expand_inline_image_dict(dict.clone());
    let filtered = !crate::object::extract_filter_names(expanded.get("Filter").unwrap_or(&Object::Null))
        .is_empty();
    let synthetic = Object::stream(expanded, bytes::Bytes::from(data.to_vec()));
    let image = crate::extractors::images::extract_image_from_xobject_at(
        None,
        &synthetic,
        None,
        None,
        None,
    )?;
    let dynamic = image.to_dynamic_image()?;
    let (w, h) = (dynamic.width(), dynamic.height());
    let (space, pixels) = match &dynamic {
        image::DynamicImage::ImageLuma8(plain) => ("DeviceGray", plain.to_vec()),
        other => ("DeviceRGB", other.to_rgb8().to_vec()),
    };
    let channels: usize = match space {
        "DeviceGray" => 1,
        _ => 3,
    };
    let (x0, x1, y0, y1) = covered_span(u0, v0, u1, v1, w, h);
    let mut out = pixels;
    let luma = |rgb: [f32; 3]| (0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]).clamp(0.0, 1.0);
    for y in y0..y1 {
        for x in x0..x1 {
            let u = (x as f64 + 0.5) / w as f64;
            let v = 1.0 - (y as f64 + 0.5) / h as f64;
            // ── pdf_manipulator patch: the last region in drawing order is drawn on top ──
            let Some((_, rgb)) = fills.iter().rev().find(|(rect, _)| {
                let [ru0, rv0, ru1, rv1] = rect;
                f64::from(*ru0) <= u
                    && u < f64::from(*ru1)
                    && f64::from(*rv0) <= v
                    && v < f64::from(*rv1)
            }) else {
                continue;
            };
            // ── end pdf_manipulator patch ──
            let at = (y * w as usize + x) * channels;
            if channels == 1 {
                out[at] = (luma(*rgb) * 255.0).round() as u8;
            } else {
                out[at] = (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8;
                out[at + 1] = (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8;
                out[at + 2] = (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    // The copy's dictionary names only what its raster needs: the size, the
    // colour space, 8 bits and its filter. The source's /Decode does not
    // carry over, since the samples are the physical colour now.
    let mut out_dict = HashMap::new();
    out_dict.insert("W".to_string(), Object::Integer(u64::from(w) as i64));
    out_dict.insert("H".to_string(), Object::Integer(u64::from(h) as i64));
    out_dict.insert("BPC".to_string(), Object::Integer(8));
    out_dict.insert("CS".to_string(), Object::Name(space.to_string()));
    if filtered {
        out_dict.insert("F".to_string(), Object::Name("FlateDecode".to_string()));
        let encoded = crate::host::images::encode::encode_flate_predicted(
            &out,
            w,
            h,
            u8::try_from(channels).map_err(|_| Error::Image("inline image channels".to_string()))?,
        )?;
        Ok((out_dict, encoded.bytes))
    } else {
        Ok((out_dict, out))
    }
}


