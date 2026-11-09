//! Cross-reference table reconstruction for damaged PDFs.
//!
//! When the xref table is corrupted, missing, or incomplete, this module
//! provides functionality to reconstruct it by scanning the entire PDF file
//! for object markers.
//!
//! This is a fallback mechanism used only when standard xref parsing fails.

use crate::error::{Error, Result};
use crate::object::{Object, ObjectRef};
use crate::xref::{CrossRefTable, XRefEntry};
use std::collections::HashMap;
// ── pdf_manipulator patch: file-level scans run in bounded windows ──
use crate::host::scan::{self, Flow, OfSeek};
use std::io::{Read, Seek, SeekFrom};
// ── end pdf_manipulator patch ──

/// Reconstruct the cross-reference table by scanning the entire PDF file.
///
/// This function scans for "N G obj" patterns throughout the file and builds
/// an xref table from the discovered objects. It also attempts to find the
/// trailer dictionary and identify the catalog.
///
// ── pdf_manipulator patch: file-level scans run in bounded windows ──
/// # Performance
///
/// The file is read once, in 64 KiB windows; what is held is one window and
/// an entry per object found, never the file.
///
// ── end pdf_manipulator patch ──
/// # Returns
///
/// The reconstructed cross-reference table, the trailer, and any SYNTHETIC
/// objects the caller must inject (they have no byte offset in the file).
/// Synthetic objects appear only when the file's own Catalog / page-tree root did
/// not survive and one had to be rebuilt from the orphaned pages; the vector is
/// empty in every ordinary reconstruction.
///
/// # Errors
///
/// Returns an error if:
/// - The file cannot be read
/// - No objects are found during scanning
/// - No catalog, page-tree root, or page object survived to rebuild from
///
/// # Example
///
/// ```no_run
/// # use std::fs::File;
/// # use std::io::BufReader;
/// # use pdf_oxide::xref_reconstruction::reconstruct_xref;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let file = File::open("damaged.pdf")?;
/// let mut reader = BufReader::new(file);
/// let (xref, trailer, _synthetic) = reconstruct_xref(&mut reader)?;
/// println!("Reconstructed {} objects", xref.len());
/// # Ok(())
/// # }
/// ```
pub fn reconstruct_xref<R: Read + Seek>(
    reader: &mut R,
) -> Result<(CrossRefTable, Object, Vec<(ObjectRef, Object)>)> {
    // ── pdf_manipulator patch: the cancellable twin does the work ──
    reconstruct_xref_cancellable(reader, &crate::cancel::CancelCheck::none())
    // ── end pdf_manipulator patch ──
}

// ── pdf_manipulator patch: a cancel stops the scan at the next hit ──
/// [`reconstruct_xref`] that stops with the cancelled error once `cancel`
/// reports cancelled, polled every few thousand hits of the scan.
pub fn reconstruct_xref_cancellable<R: Read + Seek>(
    reader: &mut R,
    cancel: &crate::cancel::CancelCheck,
) -> Result<(CrossRefTable, Object, Vec<(ObjectRef, Object)>)> {
    // ── end pdf_manipulator patch ──
    log::info!("Reconstructing xref table by scanning file...");

    let mut xref = CrossRefTable::new();
    let mut objects_found = 0;
    // ── pdf_manipulator patch: file-level scans run in bounded windows ──
    let mut fold = TrailerFold::default();
    let mut trailers = scan::Trailers::default();
    let mut input = OfSeek(&mut *reader);

    // Scan for "N G obj" patterns, and for the trailers on the way.
    // Pattern: one or more digits, whitespace, one or more digits, whitespace, "obj"
    let words: [&[u8]; 4] =
        [scan::pat::OBJ, scan::pat::TRAILER, scan::pat::STARTXREF, scan::pat::EOF_MARK];
    let mut stride = crate::cancel::Stride::new(cancel);
    scan::scan(&mut input, 0, &words, |hit, r| {
        stride.tick()?;
        if hit.pattern != 0 {
            trailers.hit(hit, r, hit.pattern == 1, &mut |start, parsed| fold.accept(start, parsed))?;
            return Ok(Flow::Continue);
        }
        let Some(header) = scan::header_before(hit, r, scan::Spacing::Any, scan::Before::Anything)?
        else {
            return Ok(Flow::Continue);
        };
        let offset = header.start;

        // Parse object and generation numbers
        let obj_num: u32 = match std::str::from_utf8(&header.number)
            .ok()
            .and_then(|s| s.parse().ok())
        {
            Some(n) => n,
            None => {
                log::warn!("Failed to parse object number at offset {}", offset);
                return Ok(Flow::Continue);
            },
        };

        let gen_num: u16 = match std::str::from_utf8(&header.generation)
            .ok()
            .and_then(|s| s.parse().ok())
        {
            Some(n) => n,
            None => {
                log::warn!("Failed to parse generation number at offset {}", offset);
                return Ok(Flow::Continue);
            },
        };

        // SPEC COMPLIANCE FIX: Validate that this is actually an object header
        // PDF Spec: ISO 32000-1:2008, Section 7.5.4 - Cross-Reference Table
        //
        // Previous implementation would add ANY "N G obj" pattern to the xref table,
        // even if it appeared inside strings, comments, or corrupted data.
        //
        // This creates security risks:
        // 1. False positives can point to invalid object locations
        // 2. Can cause crashes when trying to parse non-object data as objects
        // 3. Malicious PDFs can craft fake object headers to confuse parsers
        //
        // Correct behavior: Validate that the pattern is followed by valid object syntax

        // Check if the next bytes after "obj" form a valid PDF object
        // We do basic validation: look for dictionary start, array start, or primitive values
        if let Some(next_byte) = scan::first_non_space(hit, r, hit.offset + 3)? {
            // Valid object should start with:
            // - << (dictionary)
            // - [ (array)
            // - < (hex string or dict - ambiguous at this point)
            // - ( (literal string)
            // - / (name)
            // - t, f, n (true, false, null)
            // - digit or - (number)
            let is_valid_object_start =
                matches!(next_byte, b'<' | b'[' | b'(' | b'/' | b't' | b'f' | b'n' | b'-')
                    || next_byte.is_ascii_digit();

            if !is_valid_object_start {
                log::debug!(
                    "Skipping false positive object header at offset {} (next byte: 0x{:02x} '{}')",
                    offset,
                    next_byte,
                    if next_byte.is_ascii_graphic() {
                        next_byte as char
                    } else {
                        '?'
                    }
                );
                return Ok(Flow::Continue);
            }

            log::debug!("Validated object {} gen {} at offset {}", obj_num, gen_num, offset);
        }

        // Add to xref table
        let entry = XRefEntry::uncompressed(offset, gen_num);
        xref.add_entry(obj_num, entry);
        objects_found += 1;
        Ok(Flow::Continue)
    })?;
    trailers.finish(&mut input, &mut |start, parsed| fold.accept(start, parsed))?;
    // ── end pdf_manipulator patch ──

    log::info!("Reconstructed xref with {} objects", objects_found);

    if objects_found == 0 {
        return Err(Error::InvalidPdf("No objects found during xref reconstruction".to_string()));
    }

    // Try to find the trailer dictionary
    let (trailer, synthetic) = pick_trailer(fold, reader, &xref)?;

    Ok((xref, trailer, synthetic))
}

// ── pdf_manipulator patch: file-level scans run in bounded windows ──
/// The trailers a scan has parsed, folded as they come: the last one with a
/// `/Root`, and the latest `/Encrypt`, `/ID` and `/Info` of the others.
#[derive(Default)]
struct TrailerFold {
    /// The chosen /Root-bearing trailer plus the byte offset it was parsed
    /// from (the scan yields trailers in ascending file order, so a later
    /// offset = a more recent incremental update).
    best: Option<(Object, u64)>,
    /// /Encrypt /ID /Info salvaged from /Root-less parsed trailers, each
    /// tracked with the offset it came from. If no /Root-bearing trailer
    /// exists and we synthesize a minimal one, an encrypted file's /Encrypt
    /// (and /ID, used for the encryption key) would otherwise be lost, making
    /// the document undecryptable. Per ISO 32000-1 §7.5.5 the most recent
    /// occurrence wins — including over a /Root-bearing trailer that appears
    /// earlier in the file.
    salvaged: HashMap<String, (Object, u64)>,
}

impl TrailerFold {
    fn accept(&mut self, trailer_start: u64, parsed: Result<Object>) {
        log::debug!("Found trailer keyword at offset {}", trailer_start);
        match parsed {
            Ok(obj) => {
                // Only accept a parsed trailer that actually carries /Root.
                // A Linearized file's sparse end-of-file trailer legitimately
                // omits /Root — the Catalog is reachable via the linearization
                // parameters / first xref chain, not the trailing trailer
                // (issue #509). Accepting a /Root-less trailer here would
                // short-circuit Catalog discovery and fail downstream with
                // "Trailer missing /Root entry". The *last* /Root-bearing
                // trailer still wins for /Root itself.
                if obj.as_dict().is_some_and(|d| d.get("Root").is_some()) {
                    self.best = Some((obj, trailer_start));
                } else {
                    if let Some(d) = obj.as_dict() {
                        for key in ["Encrypt", "ID", "Info"] {
                            if let Some(v) = d.get(key) {
                                self.salvaged
                                    .insert(key.to_string(), (v.clone(), trailer_start));
                            }
                        }
                    }
                    log::debug!(
                        "Parsed trailer at offset {} has no /Root — skipping (Catalog located by object scan; /Encrypt /ID /Info preserved)",
                        trailer_start
                    );
                }
            },
            Err(e) => {
                log::warn!("Failed to parse trailer dictionary at offset {}: {}", trailer_start, e);
            },
        }
    }
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: file-level scans run in bounded windows ──
/// Pick the trailer dictionary from the ones the scan found.
///
/// Prefers the last /Root-bearing trailer. If there is none, attempts to
/// reconstruct a minimal trailer by finding the catalog object.
fn pick_trailer<R: Read + Seek>(
    fold: TrailerFold,
    reader: &mut R,
    xref: &CrossRefTable,
) -> Result<(Object, Vec<(ObjectRef, Object)>)> {
    // Per ISO 32000-1:2008 Section 7.5.5, the most recent trailer (from the
    // latest incremental update) takes precedence. Using the first trailer can
    // miss /Encrypt entries added in later revisions.
    let TrailerFold { best: best_trailer, salvaged } = fold;
// ── end pdf_manipulator patch ──
    if let Some((mut trailer, best_off)) = best_trailer {
        // Merge salvaged /Encrypt /ID /Info from /Root-less trailers using
        // most-recent-occurrence-wins (ISO 32000-1 §7.5.5): a salvaged value
        // overrides the /Root-bearing trailer's only when it was parsed from
        // a *later* offset (a newer incremental update — e.g. a sparse
        // trailer that adds encryption or rotates the file ID), and always
        // fills a key the /Root-bearing trailer lacks. An earlier /Root-less
        // value never clobbers a newer explicit one.
        if !salvaged.is_empty() {
            if let Object::Dictionary(d) = &mut trailer {
                for (key, (value, off)) in &salvaged {
                    match d.get(key) {
                        Some(_) if *off <= best_off => {}, // existing is newer/equal
                        _ => {
                            d.insert(key.clone(), value.clone());
                        },
                    }
                }
            }
        }
        log::info!("Successfully parsed trailer dictionary (last /Root-bearing occurrence)");
        // A parsed /Root-bearing trailer needs no synthesis.
        return Ok((trailer, Vec::new()));
    }

    // No /Root-bearing trailer found — synthesize one by scanning objects
    // for /Type /Catalog (handles Linearized files whose only trailer is the
    // sparse, /Root-less end-of-file trailer).
    log::info!(
        "No /Root-bearing trailer found; reconstructing minimal trailer via Catalog scan..."
    );
    let salvaged_values: HashMap<String, Object> =
        salvaged.into_iter().map(|(k, (v, _))| (k, v)).collect();
    reconstruct_minimal_trailer(reader, xref, &salvaged_values)
}

/// Reconstruct a minimal trailer dictionary.
///
/// Scans objects to find the catalog (object with /Type /Catalog) and
/// creates a minimal trailer with the required entries, plus any
/// `salvaged` entries (/Encrypt, /ID, /Info) carried over from a
/// /Root-less parsed trailer so encrypted documents remain decryptable.
fn reconstruct_minimal_trailer<R: Read + Seek>(
    reader: &mut R,
    xref: &CrossRefTable,
    salvaged: &HashMap<String, Object>,
) -> Result<(Object, Vec<(ObjectRef, Object)>)> {
    log::debug!("Scanning objects to find catalog...");

    // We need to find the catalog object
    // The catalog is an object with /Type /Catalog
    let mut catalog_ref = None;

    // Scan objects looking for the catalog. `all_object_numbers()` is
    // `HashMap`-backed, so iterating it directly is nondeterministic: a
    // bounded scan over an arbitrary subset can miss the Catalog on
    // different runs (even for a ~114-object Linearized file).
    // `smallest_object_numbers` is deterministic, visits low-numbered
    // objects first (where the Catalog conventionally lives), and bounds the
    // candidate set *before* sorting so a maliciously sparse/huge xref stays
    // O(n log MAX_SCAN) time / O(MAX_SCAN) memory.
    const MAX_SCAN: usize = 4096;
    let obj_nums = xref.smallest_object_numbers(MAX_SCAN);
    let mut checked = 0usize;
    for obj_num in obj_nums {
        if checked >= MAX_SCAN {
            break;
        }

        if let Some(entry) = xref.get(obj_num) {
            if !entry.in_use {
                continue;
            }
            checked += 1;

            // Try to load and check this object
            match load_object_at_offset(reader, entry.offset) {
                Ok(obj) => {
                    if is_catalog(&obj) {
                        log::info!("Found catalog: object {} gen {}", obj_num, entry.generation);
                        catalog_ref = Some((obj_num, entry.generation));
                        break;
                    }
                },
                Err(e) => {
                    log::debug!(
                        "Failed to load object {} at offset {}: {}",
                        obj_num,
                        entry.offset,
                        e
                    );
                    continue;
                },
            }
        }
    }

    // No Catalog anywhere in the surviving objects. This is the truncated-file
    // case (a web crawl capped mid-stream, an incremental update lost its tail):
    // the Catalog and page-tree root lived at the end and are gone, but the page
    // objects themselves survived. Rebuild a Catalog from those pages rather than
    // declaring the whole document a total loss. `synthetic` carries the objects
    // we invent - they have no byte offset, so the caller injects them into the
    // object cache.
    let (root_ref, synthetic) = match catalog_ref {
        Some((cat_num, cat_gen)) => (ObjectRef::new(cat_num, cat_gen), Vec::new()),
        None => synthesize_catalog_from_pages(reader, xref)?,
    };

    // Create minimal trailer dictionary
    let mut trailer_dict = HashMap::new();
    trailer_dict.insert("Root".to_string(), Object::Reference(root_ref));
    trailer_dict.insert("Size".to_string(), Object::Integer(xref.len() as i64));

    // Carry over /Encrypt, /ID, /Info salvaged from a skipped /Root-less
    // trailer. Never clobber the Root/Size we just computed.
    for (key, value) in salvaged {
        if key != "Root" && key != "Size" {
            trailer_dict.insert(key.clone(), value.clone());
        }
    }

    Ok((Object::Dictionary(trailer_dict), synthetic))
}

/// Rebuild a Catalog (and, if needed, a page-tree root) from the surviving page
/// objects of a truncated file, returning the Root reference and the SYNTHETIC
/// objects to inject.
///
/// Two cases, in order of fidelity:
///  1. A `/Type /Pages` node survived (the page-tree root, or any internal node).
///     Prefer a root - a `/Pages` with no `/Parent` - and point a synthesized
///     Catalog at it, preserving the file's own tree and its inherited attributes.
///  2. No `/Pages` survived: collect every `/Type /Page` object and hang them off
///     a synthesized flat `/Pages` node, then a Catalog. Page order follows object
///     number (the conventional page order). The flat node carries a default
///     `/MediaBox` so a page that relied on inheritance from its lost parent still
///     has a media box to fall back to.
///
/// Returns `Error::InvalidPdf` only when NEITHER a `/Pages` nor any `/Type /Page`
/// survived - there is genuinely nothing to show.
fn synthesize_catalog_from_pages<R: Read + Seek>(
    reader: &mut R,
    xref: &CrossRefTable,
) -> Result<(ObjectRef, Vec<(ObjectRef, Object)>)> {
    // Free object numbers for the objects we invent: above every surviving one.
    //
    // Both operands come from the file's own object numbering, and this path
    // runs only on files that are already malformed — so hostile input is
    // guaranteed here rather than merely possible. `max_obj + 1` on a file
    // containing `4294967295 0 obj` panics in debug and wraps in release (no
    // profile sets `overflow-checks`), and the wrapped number then collides
    // with a real object, placing the synthetic page tree on top of it.
    let max_obj = xref.all_object_numbers().max().unwrap_or(0);
    let Some(catalog_num) = max_obj.checked_add(1) else {
        return Err(Error::InvalidPdf(
            "Cannot reconstruct: the file's object numbering leaves no free number \
             for a synthesized Catalog"
                .to_string(),
        ));
    };

    // Deterministic, low-first scan (the same bound the Catalog scan uses).
    const MAX_SCAN: usize = 4096;
    let obj_nums = xref.smallest_object_numbers(MAX_SCAN);

    // Look for a surviving /Type /Pages node - prefer a genuine ROOT (no /Parent).
    let mut pages_root: Option<u32> = None;
    let mut pages_any: Option<u32> = None;
    let mut page_objs: Vec<u32> = Vec::new();
    for obj_num in obj_nums {
        let Some(entry) = xref.get(obj_num) else {
            continue;
        };
        if !entry.in_use {
            continue;
        }
        let Ok(obj) = load_object_at_offset(reader, entry.offset) else {
            continue;
        };
        let Some(dict) = obj.as_dict() else { continue };
        match dict.get("Type").and_then(|t| t.as_name()) {
            Some("Pages") => {
                pages_any.get_or_insert(obj_num);
                if dict.get("Parent").is_none() {
                    pages_root.get_or_insert(obj_num);
                }
            },
            Some("Page") => page_objs.push(obj_num),
            _ => {},
        }
    }

    // Case 1: a /Pages node survived - point a Catalog at the best one.
    if let Some(root) = pages_root.or(pages_any) {
        log::info!("Recovery: synthesizing Catalog -> surviving /Pages object {root}");
        let catalog = catalog_dict(ObjectRef::new(root, 0));
        return Ok((
            ObjectRef::new(catalog_num, 0),
            vec![(ObjectRef::new(catalog_num, 0), catalog)],
        ));
    }

    // Case 2: no uncompressed page survived. Before giving up, look inside any
    // surviving object streams (/Type /ObjStm): PDF 1.5+ files routinely pack the
    // Catalog, page-tree nodes and page dictionaries into ObjStms, which the
    // offset scan above cannot see (it only finds `N G obj` markers). The objects
    // parsed out carry their real numbers, so injecting them lets their refs -
    // including /Contents streams, which live UNCOMPRESSED in the rebuilt xref -
    // resolve normally.
    if page_objs.is_empty() {
        if let Some(result) = recover_from_objstms(reader, xref, catalog_num) {
            return Ok(result);
        }
        return Err(Error::InvalidPdf(
            "Could not find catalog or any page in reconstructed xref".to_string(),
        ));
    }
    page_objs.sort_unstable();
    log::info!("Recovery: synthesizing flat /Pages over {} orphan pages", page_objs.len());
    let Some(pages_num) = max_obj.checked_add(2) else {
        return Err(Error::InvalidPdf(
            "Cannot reconstruct: the file's object numbering leaves no free number \
             for a synthesized /Pages node"
                .to_string(),
        ));
    };
    let kids: Vec<Object> = page_objs
        .iter()
        .map(|&n| Object::Reference(ObjectRef::new(n, 0)))
        .collect();
    let mut pages = HashMap::new();
    pages.insert("Type".to_string(), Object::Name("Pages".to_string()));
    pages.insert("Count".to_string(), Object::Integer(page_objs.len() as i64));
    pages.insert("Kids".to_string(), Object::Array(kids));
    // Fallback media box for any page that inherited its size from the lost parent.
    pages.insert(
        "MediaBox".to_string(),
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]),
    );
    let catalog = catalog_dict(ObjectRef::new(pages_num, 0));
    Ok((
        ObjectRef::new(catalog_num, 0),
        vec![
            (ObjectRef::new(pages_num, 0), Object::Dictionary(pages)),
            (ObjectRef::new(catalog_num, 0), catalog),
        ],
    ))
}

/// A minimal `<< /Type /Catalog /Pages ref >>`.
fn catalog_dict(pages: ObjectRef) -> Object {
    let mut d = HashMap::new();
    d.insert("Type".to_string(), Object::Name("Catalog".to_string()));
    d.insert("Pages".to_string(), Object::Reference(pages));
    Object::Dictionary(d)
}

/// Recover pages packed inside object streams when no uncompressed page survived.
///
/// Decompresses every surviving `/Type /ObjStm` and injects ALL objects it
/// contains (at their real numbers) so their cross-references resolve, then
/// anchors a Root: a real Catalog if one was packed in, otherwise a synthesized
/// Catalog over the ObjStm's own `/Pages` root or a flat `/Pages` of the packed
/// page dictionaries. `None` when no ObjStm yields a page.
fn recover_from_objstms<R: Read + Seek>(
    reader: &mut R,
    xref: &CrossRefTable,
    catalog_num: u32,
) -> Option<(ObjectRef, Vec<(ObjectRef, Object)>)> {
    const MAX_SCAN: usize = 4096;
    let mut injected: Vec<(ObjectRef, Object)> = Vec::new();
    let mut catalog: Option<u32> = None;
    let mut pages_root: Option<u32> = None;
    let mut pages_any: Option<u32> = None;
    let mut page_objs: Vec<u32> = Vec::new();

    for obj_num in xref.smallest_object_numbers(MAX_SCAN) {
        let Some(entry) = xref.get(obj_num) else {
            continue;
        };
        if !entry.in_use {
            continue;
        }
        let Ok(container) = load_object_at_offset(reader, entry.offset) else {
            continue;
        };
        // Only /Type /ObjStm streams carry other objects.
        let is_objstm = container
            .as_dict()
            .and_then(|d| d.get("Type"))
            .and_then(|t| t.as_name())
            == Some("ObjStm");
        if !is_objstm {
            continue;
        }
        let Ok(contained) = crate::objstm::parse_object_stream(&container) else {
            continue;
        };
        // `parse_object_stream` returns a HashMap, and the selections below are
        // all `get_or_insert` — first one seen wins. Walking it in hash order
        // therefore let the same damaged bytes recover a *different* /Root
        // between runs of the same binary. Ascending object number is the order
        // the sibling non-object-stream path already uses
        // (`smallest_object_numbers`), so use it here too.
        let mut contained: Vec<(u32, Object)> = contained.into_iter().collect();
        contained.sort_by_key(|(num, _)| *num);
        for (num, obj) in contained {
            match obj
                .as_dict()
                .and_then(|d| d.get("Type"))
                .and_then(|t| t.as_name())
            {
                Some("Catalog") => {
                    catalog.get_or_insert(num);
                },
                Some("Pages") => {
                    pages_any.get_or_insert(num);
                    if obj.as_dict().and_then(|d| d.get("Parent")).is_none() {
                        pages_root.get_or_insert(num);
                    }
                },
                Some("Page") => page_objs.push(num),
                _ => {},
            }
            injected.push((ObjectRef::new(num, 0), obj));
        }
    }

    // A real Catalog was packed in - use it directly.
    if let Some(cat) = catalog {
        log::info!("Recovery: Catalog {cat} recovered from an object stream");
        return Some((ObjectRef::new(cat, 0), injected));
    }

    // Free object numbers for anything we synthesize must clear EVERY injected
    // number too, not just the uncompressed max (compressed objects can outrank
    // it), or a synthetic Catalog could shadow a real recovered object.
    // Same hazard as `catalog_num` above: an injected object numbered at the
    // top of the range must not wrap when we reach past it. Saturating is the
    // right disposition here — this helper returns Option and its caller
    // simply declines to synthesize, rather than failing the whole recovery.
    let free_base = injected
        .iter()
        .map(|(r, _)| r.id)
        .max()
        .map_or(catalog_num, |m| m.max(catalog_num.saturating_sub(1)).saturating_add(1));

    // Else anchor a synthesized Catalog on the best page tree we found.
    if let Some(root) = pages_root.or(pages_any) {
        injected.push((ObjectRef::new(free_base, 0), catalog_dict(ObjectRef::new(root, 0))));
        return Some((ObjectRef::new(free_base, 0), injected));
    }
    if page_objs.is_empty() {
        return None;
    }
    page_objs.sort_unstable();
    log::info!("Recovery: flat /Pages over {} ObjStm-packed pages", page_objs.len());
    let synth_catalog = free_base;
    let pages_num = free_base + 1;
    let kids: Vec<Object> = page_objs
        .iter()
        .map(|&n| Object::Reference(ObjectRef::new(n, 0)))
        .collect();
    let mut pages = HashMap::new();
    pages.insert("Type".to_string(), Object::Name("Pages".to_string()));
    pages.insert("Count".to_string(), Object::Integer(page_objs.len() as i64));
    pages.insert("Kids".to_string(), Object::Array(kids));
    pages.insert(
        "MediaBox".to_string(),
        Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]),
    );
    injected.push((ObjectRef::new(pages_num, 0), Object::Dictionary(pages)));
    injected.push((ObjectRef::new(synth_catalog, 0), catalog_dict(ObjectRef::new(pages_num, 0))));
    Some((ObjectRef::new(synth_catalog, 0), injected))
}

/// Load an object at a specific byte offset.
///
/// This is a standalone function that doesn't require the full PdfDocument
/// context, used during trailer reconstruction.
fn load_object_at_offset<R: Read + Seek>(reader: &mut R, offset: u64) -> Result<Object> {
    // ── pdf_manipulator patch: file-level scans run in bounded windows ──
    scan::load_object(&mut OfSeek(reader), offset)
    // ── end pdf_manipulator patch ──
}

/// Check if an object is the document catalog.
///
/// The catalog has /Type /Catalog in its dictionary.
fn is_catalog(obj: &Object) -> bool {
    if let Some(dict) = obj.as_dict() {
        if let Some(type_obj) = dict.get("Type") {
            if let Some(type_name) = type_obj.as_name() {
                return type_name == "Catalog";
            }
        }
    }
    false
}

/// Search for an object near a given offset.
///
/// When the reconstructed xref has slightly incorrect offsets, this function
/// searches within a ±1KB window to find the actual object.
pub fn search_nearby_for_object<R: Read + Seek>(
    reader: &mut R,
    obj_id: u32,
    approx_offset: u64,
) -> Result<Object> {
    log::debug!("Searching for object {} near offset {}", obj_id, approx_offset);

    // Search ±1KB from the expected offset
    let search_range = 1024u64;
    let start = approx_offset.saturating_sub(search_range);
    let end = approx_offset + search_range;

    reader.seek(SeekFrom::Start(start))?;
    let mut buffer = vec![0u8; (end - start) as usize];
    let bytes_read = reader.read(&mut buffer)?;
    let buffer = &buffer[..bytes_read];

    // Look for "N G obj" marker
    let pattern = format!(r"{} \d+ obj", obj_id);
    let re = match regex::bytes::Regex::new(&pattern) {
        Ok(r) => r,
        Err(_) => return Err(Error::ObjectNotFound(obj_id, 0)),
    };

    if let Some(mat) = re.find(buffer) {
        let obj_offset = start + mat.start() as u64;
        log::debug!(
            "Found object {} at offset {} (expected {})",
            obj_id,
            obj_offset,
            approx_offset
        );

        return load_object_at_offset(reader, obj_offset);
    }

    Err(Error::ObjectNotFound(obj_id, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_reconstruct_simple_pdf() {
        let pdf_data = b"%PDF-1.4\n\
            1 0 obj\n\
            << /Type /Catalog /Pages 2 0 R >>\n\
            endobj\n\
            2 0 obj\n\
            << /Type /Pages /Count 0 /Kids [] >>\n\
            endobj\n\
            trailer\n\
            << /Root 1 0 R /Size 3 >>\n\
            startxref\n\
            0\n\
            %%EOF";

        let mut cursor = Cursor::new(pdf_data);
        let result = reconstruct_xref(&mut cursor);

        assert!(result.is_ok());
        let (xref, trailer, _synthetic) = result.unwrap();

        // Should find objects 1 and 2
        assert!(xref.contains(1));
        assert!(xref.contains(2));

        // Trailer should have Root entry
        if let Some(dict) = trailer.as_dict() {
            assert!(dict.contains_key("Root"));
        } else {
            panic!("Trailer is not a dictionary");
        }
    }

    #[test]
    fn test_is_catalog() {
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("Catalog".to_string()));
        let catalog = Object::Dictionary(dict);

        assert!(is_catalog(&catalog));

        let not_catalog = Object::Integer(42);
        assert!(!is_catalog(&not_catalog));
    }

    #[test]
    fn test_reconstruct_no_objects() {
        let pdf_data = b"%PDF-1.4\n\
            This is not a valid PDF with objects\n\
            %%EOF";

        let mut cursor = Cursor::new(pdf_data);
        let result = reconstruct_xref(&mut cursor);

        assert!(result.is_err());
    }
}
