//! Content stream parser.
//!
//! This module parses PDF content streams into a sequence of operators.
//! Content streams are fundamentally different from the main PDF structure:
//! they use a postfix notation where operands come before operators.
//!
//! Example content stream:
//! ```text
//! BT
//!   /F1 12 Tf
//!   100 700 Td
//!   (Hello, World!) Tj
//! ET
//! ```

use crate::content::operators::{Operator, TextElement};
use crate::error::Result;
use crate::object::Object;
use crate::parser::parse_object;
use nom::bytes::complete::take_while1;
use nom::character::complete::multispace0;
use nom::IResult;
use nom::Parser;
use smallvec::SmallVec;
use std::collections::HashMap;

/// Default maximum number of operators to parse from a single content
/// stream. Prevents pathological inputs (e.g., Isartor 6.1.12) from
/// consuming unbounded time and memory.
///
/// Callers can override via [`set_max_ops_per_stream`] to raise the
/// cap (or set `usize::MAX` for effectively unbounded — use with
/// caution on adversarial PDFs).
const MAX_OPERATORS: usize = 1_000_000;

/// Global cap override for content-stream operator count. `0`
/// means "use [`MAX_OPERATORS`] default"; any other value is the
/// effective cap. Atomic so it's safe to set from one thread while
/// extraction runs on another (e.g. parallel-page extraction).
static MAX_OPERATORS_OVERRIDE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Set the global content-stream operator cap. `None` keeps the
/// default of `MAX_OPERATORS` (1,000,000). `Some(n)` overrides to `n`
/// — pass `Some(usize::MAX)` for effectively unbounded.
///
/// Returns the previous override (or `None` if the default was active).
/// The override is process-global; setting it on one thread affects all
/// concurrent extractions.
///
/// **Use case**: large technical PDFs (textbooks, ISO standards) that
/// have legitimate content streams exceeding 1,000,000 operators. The
/// default cap exists to bound the cost of adversarial inputs; raise
/// it when you know the inputs are trusted.
pub fn set_max_ops_per_stream(limit: Option<usize>) -> Option<usize> {
    let new = limit.unwrap_or(0);
    let prev = MAX_OPERATORS_OVERRIDE.swap(new, std::sync::atomic::Ordering::SeqCst);
    if prev == 0 {
        None
    } else {
        Some(prev)
    }
}

/// Current effective operator cap. Reads the override if set; otherwise
/// returns [`MAX_OPERATORS`]. Internal hot-path helper.
#[inline]
fn effective_max_operators() -> usize {
    let override_val = MAX_OPERATORS_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed);
    if override_val == 0 {
        MAX_OPERATORS
    } else {
        override_val
    }
}

/// Maximum consecutive parse errors (byte skips) before bailing out.
///
/// If we skip this many bytes without finding a valid operator, the
/// remaining data is likely junk, not a parseable content stream.
const MAX_CONSECUTIVE_ERRORS: usize = 1024;

/// Emit the operator-cap-exceeded warning at the actual *effective* cap
/// (which may have been overridden via `set_max_ops_per_stream`). PDF
/// Spec Annex C documents implementation limits; the cap exists to
/// bound parser cost on adversarial inputs.
#[inline]
fn push_operator_cap_warning() {
    let cap = effective_max_operators();
    let msg = format!("Content stream exceeded {cap} operators, truncating");
    log::warn!("{msg}");
    crate::extractors::warnings::push_global_warning(crate::extractors::warnings::Warning {
        category: crate::extractors::warnings::WarningCategory::OperatorCapExceeded,
        page: None,
        message: msg,
        spec_section: Some("Annex C"),
    });
}

/// Parse a content stream into a sequence of operators.
///
/// Content streams use postfix notation where operands precede the operator.
/// For example: `100 200 Td` means "move text position to (100, 200)".
///
/// Includes safety limits: bails out after `MAX_OPERATORS` operators or
/// `MAX_CONSECUTIVE_ERRORS` consecutive parse failures.
// ── pdf_manipulator patch: the renderers still take a page's bytes ──
///
/// Text, path and image extraction, compliance and the form finder read
/// `PdfDocument::page_operators` instead. This stays, with
/// `PdfDocument::get_page_content_data`, only for the renderers, which move
/// to the operator source next.
// ── end pdf_manipulator patch ──
pub fn parse_content_stream(data: &[u8]) -> Result<Vec<Operator>> {
    let estimated_capacity = data.len() / 20;
    let mut operators = Vec::with_capacity(estimated_capacity.min(100_000));
    let mut input = data;
    let mut consecutive_errors: usize = 0;
    // ── pdf_manipulator patch: a cancel stops the parse within a stride of tokens ──
    let mut stride = crate::cancel::Stride::job();
    let mut cancelled = None;
    // ── end pdf_manipulator patch ──

    // Parse until we consume all input
    while !input.is_empty() {
        // Skip whitespace
        if let Ok((rest, _)) = multispace0::<&[u8], nom::error::Error<&[u8]>>.parse(input) {
            input = rest;
        }

        // Check if we're done
        if input.is_empty() {
            break;
        }

        // Parse one operator with its operands
        // ── pdf_manipulator patch: a cancel stops the parse within a stride of tokens ──
        let parsed = parse_operator_with_operands(input, &mut stride, &mut cancelled);
        if let Some(error) = cancelled.take() {
            return Err(error);
        }
        match parsed {
            // ── end pdf_manipulator patch ──
            Ok((rest, op)) => {
                operators.push(op);
                input = rest;
                consecutive_errors = 0;

                if operators.len() >= effective_max_operators() {
                    push_operator_cap_warning();
                    break;
                }
            },
            Err(_) => {
                consecutive_errors += 1;
                // ── pdf_manipulator patch: a cancel stops the parse within a stride of tokens ──
                stride.tick()?;
                // ── end pdf_manipulator patch ──
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    log::warn!(
                        "Content stream had {} consecutive parse errors, bailing out ({} bytes remaining)",
                        MAX_CONSECUTIVE_ERRORS,
                        input.len()
                    );
                    break;
                }
                // If we can't parse, skip the problematic byte and continue
                // This makes us more resilient to malformed streams
                if input.len() > 1 {
                    input = &input[1..];
                } else {
                    break;
                }
            },
        }
    }

    Ok(operators)
}

// ── pdf_manipulator patch: the byte-slice text, path and image parsers are gone ──
// Text, path and image extraction read `host::content::Operators` in its
// `Text`, `Paths` and `Images` modes: one forward pass over the stream.
// `parse_content_stream` stays for the renderers until they move too.
// ── end pdf_manipulator patch ──

/// Parse a single operator with its operands.
///
/// Returns the remaining input and the parsed operator.
///
/// Uses `SmallVec<[Object; 6]>` for the operand buffer to avoid heap
/// allocation for the common case (most PDF operators have 0-6 operands).
/// Only spills to the heap for rare operators with more than 6 operands.
// ── pdf_manipulator patch: a cancel stops the parse within a stride of tokens ──
///
/// Polls `stride` once per operand and per operator; a cancel is left in
/// `cancelled` and ends the read with a nom failure, which the nom error type
/// has no room to carry.
fn parse_operator_with_operands<'a>(
    input: &'a [u8],
    stride: &mut crate::cancel::Stride<'_>,
    cancelled: &mut Option<crate::error::Error>,
) -> IResult<&'a [u8], Operator> {
    // ── end pdf_manipulator patch ──
    // Collect operands until we hit an operator name.
    // SmallVec<[Object; 6]>: stack-allocated for <= 6 operands (covers all
    // standard PDF operators: cm/Tm need 6, most need 0-4). Only spills to
    // heap for pathological content (e.g., deeply nested arrays in Other).
    let mut operands: SmallVec<[Object; 6]> = SmallVec::new();
    let mut remaining = input;

    loop {
        // ── pdf_manipulator patch: a cancel stops the parse within a stride of tokens ──
        if let Err(error) = stride.tick() {
            *cancelled = Some(error);
            return Err(nom::Err::Failure(nom::error::Error::new(
                remaining,
                nom::error::ErrorKind::Eof,
            )));
        }
        // ── end pdf_manipulator patch ──
        // Skip whitespace
        let (inp, _) = multispace0.parse(remaining)?;
        remaining = inp;

        if remaining.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                remaining,
                nom::error::ErrorKind::Eof,
            )));
        }

        // Check if this looks like an operator name (alphabetic characters)
        // Operators are typically 1-3 letter keywords
        if is_operator_start(remaining[0]) {
            let (rest, op_name) = parse_operator_name(remaining)?;

            // Special handling for inline images (BI...ID...EI sequence)
            if op_name == "BI" {
                // Parse inline image: BI <dict entries> ID <binary data> EI
                return parse_inline_image(rest);
            }

            let op = build_operator(op_name, operands);
            return Ok((rest, op));
        }

        // Otherwise, try to parse an operand (PDF object)
        let (inp, obj) = parse_object(remaining)?;
        operands.push(obj);
        remaining = inp;
    }
}

/// Check if a byte could start an operator name.
///
/// Operators start with alphabetic characters or special characters like ' or "
fn is_operator_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'\'' || byte == b'"' || byte == b'*'
}

/// Parse an operator name from the input.
///
/// Operator names are typically 1-3 letter alphabetic sequences, but can include:
/// - Single quote (') for the Quote operator
/// - Double quote (") for the DoubleQuote operator
/// - Star (*) for T* operator
fn parse_operator_name(input: &[u8]) -> IResult<&[u8], &str> {
    let (input, name_bytes) =
        take_while1(|c: u8| c.is_ascii_alphanumeric() || c == b'\'' || c == b'"' || c == b'*')
            .parse(input)?;

    let name = std::str::from_utf8(name_bytes)
        .map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))?;

    Ok((input, name))
}

/// Build an operator from its name and operands.
///
/// This function converts the raw operator name and operands into a strongly-typed
/// Operator enum variant. It handles type conversions and validates operand counts.
///
/// Accepts `SmallVec<[Object; 6]>` to avoid heap allocation for the common case
/// (most PDF operators have 0-6 operands). The operands are consumed and dropped
/// after extraction.
// ── pdf_manipulator patch: the streaming Operators build operators here ──
pub(crate) fn build_operator(name: &str, operands: SmallVec<[Object; 6]>) -> Operator {
    // ── end pdf_manipulator patch ──
    match name {
        // Text positioning
        "Td" => {
            let tx = get_number(&operands, 0).unwrap_or(0.0);
            let ty = get_number(&operands, 1).unwrap_or(0.0);
            Operator::Td { tx, ty }
        },
        "TD" => {
            let tx = get_number(&operands, 0).unwrap_or(0.0);
            let ty = get_number(&operands, 1).unwrap_or(0.0);
            Operator::TD { tx, ty }
        },
        "Tm" => {
            let a = get_number(&operands, 0).unwrap_or(1.0);
            let b = get_number(&operands, 1).unwrap_or(0.0);
            let c = get_number(&operands, 2).unwrap_or(0.0);
            let d = get_number(&operands, 3).unwrap_or(1.0);
            let e = get_number(&operands, 4).unwrap_or(0.0);
            let f = get_number(&operands, 5).unwrap_or(0.0);
            Operator::Tm { a, b, c, d, e, f }
        },
        "T*" => Operator::TStar,

        // Text showing
        "Tj" => {
            let text = get_string(&operands, 0).unwrap_or_default();
            Operator::Tj { text }
        },
        "TJ" => {
            let elements = if let Some(array) = get_array(&operands, 0) {
                array
                    .iter()
                    .filter_map(|obj| match obj {
                        Object::String(s) => Some(TextElement::String(s.clone())),
                        Object::Integer(i) => Some(TextElement::Offset(*i as f32)),
                        Object::Real(r) => Some(TextElement::Offset(*r as f32)),
                        _ => None,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            Operator::TJ { array: elements }
        },
        "'" => {
            let text = get_string(&operands, 0).unwrap_or_default();
            Operator::Quote { text }
        },
        "\"" => {
            let word_space = get_number(&operands, 0).unwrap_or(0.0);
            let char_space = get_number(&operands, 1).unwrap_or(0.0);
            let text = get_string(&operands, 2).unwrap_or_default();
            Operator::DoubleQuote {
                word_space,
                char_space,
                text,
            }
        },

        // Text state
        "Tc" => {
            let char_space = get_number(&operands, 0).unwrap_or(0.0);
            Operator::Tc { char_space }
        },
        "Tw" => {
            let word_space = get_number(&operands, 0).unwrap_or(0.0);
            Operator::Tw { word_space }
        },
        "Tz" => {
            let scale = get_number(&operands, 0).unwrap_or(100.0);
            Operator::Tz { scale }
        },
        "TL" => {
            let leading = get_number(&operands, 0).unwrap_or(0.0);
            Operator::TL { leading }
        },
        "Tf" => {
            let font = get_name(&operands, 0).unwrap_or("").to_string();
            let size = get_number(&operands, 1).unwrap_or(12.0);
            Operator::Tf { font, size }
        },
        "Tr" => {
            let render = get_integer(&operands, 0).unwrap_or(0) as u8;
            Operator::Tr { render }
        },
        "Ts" => {
            let rise = get_number(&operands, 0).unwrap_or(0.0);
            Operator::Ts { rise }
        },

        // Graphics state
        "q" => Operator::SaveState,
        "Q" => Operator::RestoreState,
        "cm" => {
            let a = get_number(&operands, 0).unwrap_or(1.0);
            let b = get_number(&operands, 1).unwrap_or(0.0);
            let c = get_number(&operands, 2).unwrap_or(0.0);
            let d = get_number(&operands, 3).unwrap_or(1.0);
            let e = get_number(&operands, 4).unwrap_or(0.0);
            let f = get_number(&operands, 5).unwrap_or(0.0);
            Operator::Cm { a, b, c, d, e, f }
        },

        // Color
        "rg" => {
            let r = get_number(&operands, 0).unwrap_or(0.0);
            let g = get_number(&operands, 1).unwrap_or(0.0);
            let b = get_number(&operands, 2).unwrap_or(0.0);
            Operator::SetFillRgb { r, g, b }
        },
        "RG" => {
            let r = get_number(&operands, 0).unwrap_or(0.0);
            let g = get_number(&operands, 1).unwrap_or(0.0);
            let b = get_number(&operands, 2).unwrap_or(0.0);
            Operator::SetStrokeRgb { r, g, b }
        },
        "g" => {
            let gray = get_number(&operands, 0).unwrap_or(0.0);
            Operator::SetFillGray { gray }
        },
        "G" => {
            let gray = get_number(&operands, 0).unwrap_or(0.0);
            Operator::SetStrokeGray { gray }
        },
        "k" => {
            // Set CMYK fill color
            let c = get_number(&operands, 0).unwrap_or(0.0);
            let m = get_number(&operands, 1).unwrap_or(0.0);
            let y = get_number(&operands, 2).unwrap_or(0.0);
            let k = get_number(&operands, 3).unwrap_or(0.0);
            Operator::SetFillCmyk { c, m, y, k }
        },
        "K" => {
            // Set CMYK stroke color
            let c = get_number(&operands, 0).unwrap_or(0.0);
            let m = get_number(&operands, 1).unwrap_or(0.0);
            let y = get_number(&operands, 2).unwrap_or(0.0);
            let k = get_number(&operands, 3).unwrap_or(0.0);
            Operator::SetStrokeCmyk { c, m, y, k }
        },

        // Color space operators
        "cs" => {
            // Set fill color space: name cs
            let name = get_name(&operands, 0).unwrap_or("DeviceGray").to_string();
            Operator::SetFillColorSpace { name }
        },
        "CS" => {
            // Set stroke color space: name CS
            let name = get_name(&operands, 0).unwrap_or("DeviceGray").to_string();
            Operator::SetStrokeColorSpace { name }
        },
        "sc" => {
            // Set fill color: c1 c2 ... cn sc
            // Number of components depends on current color space
            let components: Vec<f32> = operands
                .iter()
                .filter_map(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    _ => None,
                })
                .collect();
            Operator::SetFillColor { components }
        },
        "SC" => {
            // Set stroke color: c1 c2 ... cn SC
            let components: Vec<f32> = operands
                .iter()
                .filter_map(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    _ => None,
                })
                .collect();
            Operator::SetStrokeColor { components }
        },
        "scn" => {
            // Set fill color with pattern support: c1 c2 ... cn [name] scn
            // Last operand may be a name for pattern color spaces
            let name = if let Some(Object::Name(n)) = operands.last() {
                Some(n.clone())
            } else {
                None
            };
            let components: Vec<f32> = operands
                .iter()
                .filter_map(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    Object::Name(_) => None, // Skip pattern name
                    _ => None,
                })
                .collect();
            Operator::SetFillColorN {
                components,
                name: name.map(Box::new),
            }
        },
        "SCN" => {
            // Set stroke color with pattern support: c1 c2 ... cn [name] SCN
            let name = if let Some(Object::Name(n)) = operands.last() {
                Some(n.clone())
            } else {
                None
            };
            let components: Vec<f32> = operands
                .iter()
                .filter_map(|obj| match obj {
                    Object::Real(r) => Some(*r as f32),
                    Object::Integer(i) => Some(*i as f32),
                    Object::Name(_) => None, // Skip pattern name
                    _ => None,
                })
                .collect();
            Operator::SetStrokeColorN {
                components,
                name: name.map(Box::new),
            }
        },

        // Text object
        "BT" => Operator::BeginText,
        "ET" => Operator::EndText,

        // XObject
        "Do" => {
            // Per ISO 32000-1:2008 §7.8.2, operands "shall immediately precede"
            // their operator and none "shall be left over" once it executes.
            // Do takes exactly one operand (the XObject name); if stray operands
            // accumulated ahead of it (e.g. a dropped/malformed `cm` before this
            // `Do`), the name is still the one immediately preceding the operator,
            // i.e. the LAST element, not necessarily the first.
            let name = get_name(&operands, operands.len().saturating_sub(1))
                .unwrap_or("")
                .to_string();
            Operator::Do { name }
        },

        // Path construction
        "m" => {
            let x = get_number(&operands, 0).unwrap_or(0.0);
            let y = get_number(&operands, 1).unwrap_or(0.0);
            Operator::MoveTo { x, y }
        },
        "l" => {
            let x = get_number(&operands, 0).unwrap_or(0.0);
            let y = get_number(&operands, 1).unwrap_or(0.0);
            Operator::LineTo { x, y }
        },
        "c" => {
            // Cubic Bézier curve
            let x1 = get_number(&operands, 0).unwrap_or(0.0);
            let y1 = get_number(&operands, 1).unwrap_or(0.0);
            let x2 = get_number(&operands, 2).unwrap_or(0.0);
            let y2 = get_number(&operands, 3).unwrap_or(0.0);
            let x3 = get_number(&operands, 4).unwrap_or(0.0);
            let y3 = get_number(&operands, 5).unwrap_or(0.0);
            Operator::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
            }
        },
        "v" => {
            // Bézier curve (first control point = current point)
            let x2 = get_number(&operands, 0).unwrap_or(0.0);
            let y2 = get_number(&operands, 1).unwrap_or(0.0);
            let x3 = get_number(&operands, 2).unwrap_or(0.0);
            let y3 = get_number(&operands, 3).unwrap_or(0.0);
            Operator::CurveToV { x2, y2, x3, y3 }
        },
        "y" => {
            // Bézier curve (second control point = end point)
            let x1 = get_number(&operands, 0).unwrap_or(0.0);
            let y1 = get_number(&operands, 1).unwrap_or(0.0);
            let x3 = get_number(&operands, 2).unwrap_or(0.0);
            let y3 = get_number(&operands, 3).unwrap_or(0.0);
            Operator::CurveToY { x1, y1, x3, y3 }
        },
        "h" => Operator::ClosePath,
        "re" => {
            let x = get_number(&operands, 0).unwrap_or(0.0);
            let y = get_number(&operands, 1).unwrap_or(0.0);
            let width = get_number(&operands, 2).unwrap_or(0.0);
            let height = get_number(&operands, 3).unwrap_or(0.0);
            Operator::Rectangle {
                x,
                y,
                width,
                height,
            }
        },
        "S" => Operator::Stroke,
        // ISO 32000-1:2008 Table 60 (pdf.md:9532): `s` is "the same effect as
        // ── pdf_manipulator patch: the comment named a path parser that is gone ──
        // the sequence h S". Path extraction decomposes it, but this table —
        // which the streaming parser and therefore annotation appearance
        // streams go through — had no entry at all, so the operator was
        // dropped and the path never stroked. Widget borders are commonly
        // drawn exactly this way (`0 G 0.5 0.5 149 21 re s`), which is why
        // AcroForm field borders were missing across the corpus.
        // ── end pdf_manipulator patch ──
        "s" => Operator::CloseAndStroke,
        "f" | "F" => Operator::Fill, // "F" is obsolete equivalent of "f" (nonzero winding fill)
        "f*" => Operator::FillEvenOdd,
        "b" => Operator::CloseFillStroke,
        "b*" => Operator::CloseFillStrokeEvenOdd,
        "B" => Operator::FillStroke,
        "B*" => Operator::FillStrokeEvenOdd,
        "n" => Operator::EndPath,
        "W" => Operator::ClipNonZero,
        "W*" => Operator::ClipEvenOdd,

        // Graphics state operators
        "w" => {
            let width = get_number(&operands, 0).unwrap_or(1.0);
            Operator::SetLineWidth { width }
        },
        "d" => {
            // d operator: array phase
            // Example: [3 2] 0 d means 3 on, 2 off, starting at phase 0
            let array = if let Some(Object::Array(arr)) = operands.first() {
                arr.iter()
                    .filter_map(|obj| match obj {
                        Object::Integer(i) => Some(*i as f32),
                        Object::Real(r) => Some(*r as f32),
                        _ => None,
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let phase = get_number(&operands, 1).unwrap_or(0.0);
            Operator::SetDash { array, phase }
        },
        "J" => {
            // J operator: integer J
            // 0=butt cap, 1=round cap, 2=projecting square cap
            let cap_style = get_integer(&operands, 0).unwrap_or(0) as u8;
            Operator::SetLineCap { cap_style }
        },
        "j" => {
            // j operator: integer j
            // 0=miter join, 1=round join, 2=bevel join
            let join_style = get_integer(&operands, 0).unwrap_or(0) as u8;
            Operator::SetLineJoin { join_style }
        },
        "M" => {
            // M operator: number M
            // Miter limit (ratio of miter length to line width)
            let limit = get_number(&operands, 0).unwrap_or(10.0);
            Operator::SetMiterLimit { limit }
        },
        "ri" => {
            // ri operator: name ri
            // Rendering intent: /AbsoluteColorimetric, /RelativeColorimetric, /Saturation, or /Perceptual
            let intent = get_name(&operands, 0)
                .unwrap_or("RelativeColorimetric")
                .to_string();
            Operator::SetRenderingIntent { intent }
        },
        "i" => {
            // i operator: number i
            // Flatness tolerance (0-100)
            let tolerance = get_number(&operands, 0).unwrap_or(1.0);
            Operator::SetFlatness { tolerance }
        },
        "gs" => {
            // gs operator: name gs
            // Set extended graphics state from resource dictionary
            let dict_name = get_name(&operands, 0).unwrap_or("").to_string();
            Operator::SetExtGState { dict_name }
        },
        "sh" => {
            // sh operator: name sh
            // Paint shading pattern (gradient)
            let name = get_name(&operands, 0).unwrap_or("").to_string();
            Operator::PaintShading { name }
        },

        // Marked content operators (for tagged PDF structure)
        // PDF Spec: ISO 32000-1:2008, Section 14.6
        "BMC" => {
            // Begin marked content: tag BMC
            let tag = get_name(&operands, 0).unwrap_or("").to_string();
            Operator::BeginMarkedContent { tag }
        },
        "BDC" => {
            // Begin marked content with properties: tag properties BDC
            // properties can be a dictionary or a name (reference to /Properties resource)
            let tag = get_name(&operands, 0).unwrap_or("").to_string();
            let properties = Box::new(operands.get(1).cloned().unwrap_or(Object::Null));
            Operator::BeginMarkedContentDict { tag, properties }
        },
        "EMC" => {
            // End marked content: EMC (no operands)
            Operator::EndMarkedContent
        },

        // Unknown operator — convert SmallVec to Vec for the boxed storage.
        // This path is rare (only for unrecognized operators), so the
        // conversion cost is negligible.
        _ => Operator::Other {
            name: name.to_string(),
            operands: Box::new(operands.into_vec()),
        },
    }
}

// Helper functions to extract operands

fn get_number(operands: &[Object], index: usize) -> Option<f32> {
    operands.get(index).and_then(|obj| match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r as f32),
        _ => None,
    })
}

fn get_integer(operands: &[Object], index: usize) -> Option<i64> {
    operands.get(index).and_then(|obj| obj.as_integer())
}

fn get_string(operands: &[Object], index: usize) -> Option<Vec<u8>> {
    operands
        .get(index)
        .and_then(|obj| obj.as_string().map(|s| s.to_vec()))
}

fn get_name(operands: &[Object], index: usize) -> Option<&str> {
    operands.get(index).and_then(|obj| obj.as_name())
}

fn get_array(operands: &[Object], index: usize) -> Option<&Vec<Object>> {
    operands.get(index).and_then(|obj| obj.as_array())
}

/// Parse an inline image sequence (BI...ID...EI).
///
/// PDF Spec: ISO 32000-1:2008, Section 8.9.7 - Inline Images
///
/// Inline images have the format:
/// BI <key value> <key value> ... ID <binary data> EI
///
/// The dictionary uses abbreviated keys:
/// - W: Width
/// - H: Height
/// - CS: ColorSpace
/// - BPC: BitsPerComponent
/// - F: Filter
/// - DP: DecodeParms
/// - I: Interpolate
///
/// The challenge is finding the EI operator in the binary data, as the bytes
/// for "EI" could appear in the image data itself. Per spec, EI must be:
/// - Preceded by whitespace (space, tab, CR, LF)
/// - Followed by whitespace or end of stream
fn parse_inline_image(input: &[u8]) -> IResult<&[u8], Operator> {
    let mut dict = HashMap::new();
    let mut remaining = input;

    // Step 1: Parse the inline image dictionary (key-value pairs)
    loop {
        // Skip whitespace
        let (inp, _) = multispace0.parse(remaining)?;
        remaining = inp;

        if remaining.is_empty() {
            return Err(nom::Err::Error(nom::error::Error::new(
                remaining,
                nom::error::ErrorKind::Eof,
            )));
        }

        // Check if we've reached "ID" (start of image data)
        if remaining.len() >= 2 && &remaining[0..2] == b"ID" {
            // Check that ID is followed by whitespace or is at end
            if remaining.len() == 2 || remaining.len() > 2 && is_whitespace(remaining[2]) {
                remaining = &remaining[2..];
                break;
            }
        }

        // Parse a key (name object, often abbreviated)
        let (inp, key_obj) = parse_object(remaining)?;
        remaining = inp;

        // Skip whitespace after key
        let (inp, _) = multispace0.parse(remaining)?;
        remaining = inp;

        // Parse the corresponding value
        let (inp, value_obj) = parse_object(remaining)?;
        remaining = inp;

        // Add to dictionary
        if let Some(key_str) = key_obj.as_name() {
            dict.insert(key_str.to_string(), value_obj);
        }
    }

    // Step 2: Skip whitespace after ID
    let (inp, _) = multispace0.parse(remaining)?;
    remaining = inp;

    // Step 3: Read binary image data until we find EI
    // EI must be preceded and followed by whitespace
    let (_inp, data) = find_and_extract_image_data(remaining)?;
    let data_len = data.len();
    remaining = &remaining[data_len..];

    // Step 4: Skip past the EI operator
    // Find EI preceded by whitespace and skip it
    let (_inp, ei_pos) = find_ei_operator(remaining)?;
    remaining = &remaining[ei_pos + 2..]; // Skip past whitespace and "EI"

    // Step 5: Return the InlineImage operator
    Ok((
        remaining,
        Operator::InlineImage {
            dict: Box::new(dict),
            data,
        },
    ))
}

/// Find the EI operator in the input, which must be preceded by whitespace.
/// Returns the position of the whitespace before EI.
fn find_ei_operator(input: &[u8]) -> IResult<&[u8], usize> {
    for i in 0..input.len().saturating_sub(2) {
        // Check if we have whitespace followed by "EI"
        if is_whitespace(input[i]) && input.len() > i + 2 && &input[i + 1..i + 3] == b"EI" {
            // Check that EI is followed by whitespace, end of stream, or another operator
            if input.len() == i + 3 || is_whitespace_or_delimiter(input[i + 3]) {
                return Ok((input, i));
            }
        }
    }

    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Tag)))
}

/// Extract image data up to (but not including) the whitespace before EI.
fn find_and_extract_image_data(input: &[u8]) -> IResult<&[u8], Vec<u8>> {
    let (inp, ei_pos) = find_ei_operator(input)?;
    Ok((inp, input[..ei_pos].to_vec()))
}

/// Check if a byte is whitespace (null, tab, LF, FF, CR, space — PDF spec Table 1).
fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b'\x00' | b'\t' | b'\r' | b'\n' | b'\x0C' | b' ')
}

/// Check if a byte is whitespace or a PDF delimiter.
fn is_whitespace_or_delimiter(byte: u8) -> bool {
    is_whitespace(byte)
        || matches!(byte, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%')
}

// ── Nom-based operand skippers (test-only, superseded by raw variants) ─────

#[cfg(test)]
fn skip_operand_token(input: &[u8]) -> IResult<&[u8], ()> {
    if input.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Eof)));
    }

    match input[0] {
        b'0'..=b'9' | b'.' | b'+' | b'-' => skip_number(input),
        b'(' => skip_literal_string(input),
        b'<' if input.len() > 1 && input[1] == b'<' => skip_dict(input),
        b'<' => skip_hex_string(input),
        b'/' => skip_name(input),
        b'[' => skip_array(input),
        _ => Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char))),
    }
}

#[cfg(test)]
fn skip_number(input: &[u8]) -> IResult<&[u8], ()> {
    let mut i = 0;
    if i < input.len() && (input[i] == b'+' || input[i] == b'-') {
        i += 1;
    }
    let start = i;
    let mut has_dot = false;
    while i < input.len() {
        if input[i].is_ascii_digit() {
            i += 1;
        } else if input[i] == b'.' && !has_dot {
            has_dot = true;
            i += 1;
        } else {
            break;
        }
    }
    if i == start {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit)));
    }
    Ok((&input[i..], ()))
}

#[cfg(test)]
fn skip_literal_string(input: &[u8]) -> IResult<&[u8], ()> {
    let mut i = 1; // past opening '('
    let mut depth: u32 = 1;
    while i < input.len() && depth > 0 {
        match input[i] {
            b'\\' if i + 1 < input.len() => i += 2,
            b'(' => {
                depth += 1;
                i += 1;
            },
            b')' => {
                depth -= 1;
                i += 1;
            },
            _ => i += 1,
        }
    }
    if depth != 0 {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)));
    }
    Ok((&input[i..], ()))
}

#[cfg(test)]
fn skip_hex_string(input: &[u8]) -> IResult<&[u8], ()> {
    let mut i = 1; // past opening '<'
    while i < input.len() {
        if input[i] == b'>' {
            return Ok((&input[i + 1..], ()));
        }
        i += 1;
    }
    Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)))
}

#[cfg(test)]
fn skip_name(input: &[u8]) -> IResult<&[u8], ()> {
    let mut i = 1; // past '/'
    while i < input.len() && !is_whitespace_or_delimiter(input[i]) {
        i += 1;
    }
    Ok((&input[i..], ()))
}

#[cfg(test)]
fn skip_array(input: &[u8]) -> IResult<&[u8], ()> {
    let mut i = 1; // past opening '['
    let mut depth: u32 = 1;
    while i < input.len() && depth > 0 {
        match input[i] {
            b'[' => {
                depth += 1;
                i += 1;
            },
            b']' => {
                depth -= 1;
                i += 1;
            },
            b'(' => {
                // Skip nested literal string
                i += 1;
                let mut str_depth: u32 = 1;
                while i < input.len() && str_depth > 0 {
                    match input[i] {
                        b'\\' if i + 1 < input.len() => i += 2,
                        b'(' => {
                            str_depth += 1;
                            i += 1;
                        },
                        b')' => {
                            str_depth -= 1;
                            i += 1;
                        },
                        _ => i += 1,
                    }
                }
            },
            b'<' if i + 1 < input.len() && input[i + 1] == b'<' => {
                // Skip nested dict <<...>>
                i += 2;
                let mut dict_depth: u32 = 1;
                while i + 1 < input.len() && dict_depth > 0 {
                    if input[i] == b'<' && input[i + 1] == b'<' {
                        dict_depth += 1;
                        i += 2;
                    } else if input[i] == b'>' && input[i + 1] == b'>' {
                        dict_depth -= 1;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
            },
            b'<' => {
                // Skip nested hex string
                i += 1;
                while i < input.len() && input[i] != b'>' {
                    i += 1;
                }
                if i < input.len() {
                    i += 1;
                }
            },
            _ => i += 1,
        }
    }
    if depth != 0 {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)));
    }
    Ok((&input[i..], ()))
}

#[cfg(test)]
fn skip_dict(input: &[u8]) -> IResult<&[u8], ()> {
    let mut i = 2; // past opening '<<'
    let mut depth: u32 = 1;
    while i < input.len() && depth > 0 {
        if i + 1 < input.len() && input[i] == b'<' && input[i + 1] == b'<' {
            depth += 1;
            i += 2;
        } else if i + 1 < input.len() && input[i] == b'>' && input[i + 1] == b'>' {
            depth -= 1;
            i += 2;
        } else if input[i] == b'(' {
            // Skip literal string inside dict
            i += 1;
            let mut str_depth: u32 = 1;
            while i < input.len() && str_depth > 0 {
                match input[i] {
                    b'\\' if i + 1 < input.len() => i += 2,
                    b'(' => {
                        str_depth += 1;
                        i += 1;
                    },
                    b')' => {
                        str_depth -= 1;
                        i += 1;
                    },
                    _ => i += 1,
                }
            }
        } else if input[i] == b'<' {
            // Single '<' → hex string <...>
            i += 1;
            while i < input.len() && input[i] != b'>' {
                i += 1;
            }
            if i < input.len() {
                i += 1; // Skip closing '>'
            }
        } else {
            i += 1;
        }
    }
    if depth != 0 {
        return Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Char)));
    }
    Ok((&input[i..], ()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_text() {
        let stream = b"BT /F1 12 Tf 100 700 Td (Hello) Tj ET";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 5);

        assert!(matches!(ops[0], Operator::BeginText));
        assert!(matches!(ops[1], Operator::Tf { ref font, size } if font == "F1" && size == 12.0));
        assert!(matches!(ops[2], Operator::Td { tx, ty } if tx == 100.0 && ty == 700.0));
        assert!(matches!(ops[3], Operator::Tj { .. }));
        assert!(matches!(ops[4], Operator::EndText));
    }

    #[test]
    fn test_parse_text_matrix() {
        let stream = b"1 0 0 1 100 200 Tm";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);

        match &ops[0] {
            Operator::Tm { a, b, c, d, e, f } => {
                assert_eq!(*a, 1.0);
                assert_eq!(*b, 0.0);
                assert_eq!(*c, 0.0);
                assert_eq!(*d, 1.0);
                assert_eq!(*e, 100.0);
                assert_eq!(*f, 200.0);
            },
            _ => panic!("Expected Tm operator"),
        }
    }

    #[test]
    fn test_parse_tj_array() {
        let stream = b"[(Hello) -100 (World)] TJ";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);

        match &ops[0] {
            Operator::TJ { array } => {
                assert_eq!(array.len(), 3);
                assert!(matches!(array[0], TextElement::String(_)));
                assert!(matches!(array[1], TextElement::Offset(-100.0)));
                assert!(matches!(array[2], TextElement::String(_)));
            },
            _ => panic!("Expected TJ operator"),
        }
    }

    #[test]
    fn test_parse_color_operators() {
        // Add proper spacing between operators
        let stream = b"1 0 0 rg\n0 1 0 RG";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);

        match &ops[0] {
            Operator::SetFillRgb { r, g, b } => {
                assert_eq!(*r, 1.0);
                assert_eq!(*g, 0.0);
                assert_eq!(*b, 0.0);
            },
            _ => panic!("Expected rg operator"),
        }

        match &ops[1] {
            Operator::SetStrokeRgb { r, g, b } => {
                assert_eq!(*r, 0.0);
                assert_eq!(*g, 1.0);
                assert_eq!(*b, 0.0);
            },
            _ => panic!("Expected RG operator"),
        }
    }

    #[test]
    fn test_parse_graphics_state() {
        let stream = b"q 1 0 0 1 50 50 cm Q";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 3);

        assert!(matches!(ops[0], Operator::SaveState));
        assert!(matches!(ops[1], Operator::Cm { .. }));
        assert!(matches!(ops[2], Operator::RestoreState));
    }

    #[test]
    fn test_parse_t_star() {
        let stream = b"T*";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::TStar));
    }

    #[test]
    fn test_parse_text_state() {
        let stream = b"2 Tc 3 Tw 50 Tz 14 TL";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 4);

        assert!(matches!(ops[0], Operator::Tc { char_space } if char_space == 2.0));
        assert!(matches!(ops[1], Operator::Tw { word_space } if word_space == 3.0));
        assert!(matches!(ops[2], Operator::Tz { scale } if scale == 50.0));
        assert!(matches!(ops[3], Operator::TL { leading } if leading == 14.0));
    }

    #[test]
    fn test_parse_quote_operators() {
        let stream = b"(Text1) ' 1 0.5 (Text2) \"";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);

        assert!(matches!(ops[0], Operator::Quote { .. }));
        assert!(matches!(ops[1], Operator::DoubleQuote { .. }));
    }

    #[test]
    fn test_parse_path_operators() {
        let stream = b"100 200 m 150 250 l 10 10 50 50 re S";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 4);

        assert!(matches!(ops[0], Operator::MoveTo { x, y } if x == 100.0 && y == 200.0));
        assert!(matches!(ops[1], Operator::LineTo { x, y } if x == 150.0 && y == 250.0));
        assert!(matches!(ops[2], Operator::Rectangle { .. }));
        assert!(matches!(ops[3], Operator::Stroke));
    }

    #[test]
    fn test_parse_do_operator() {
        let stream = b"/Im1 Do";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);

        match &ops[0] {
            Operator::Do { name } => {
                assert_eq!(name, "Im1");
            },
            _ => panic!("Expected Do operator"),
        }
    }

    #[test]
    fn test_parse_empty_stream() {
        let stream = b"";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 0);
    }

    #[test]
    fn test_parse_whitespace_only() {
        let stream = b"   \n  \t  ";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 0);
    }

    #[test]
    fn test_parse_real_numbers() {
        let stream = b"1.5 2.7 Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);

        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert_eq!(*tx, 1.5);
                assert_eq!(*ty, 2.7);
            },
            _ => panic!("Expected Td operator"),
        }
    }

    #[test]
    fn test_content_stream_operator_limit() {
        // Build a stream with more than MAX_OPERATORS simple operators.
        // Each "q\n" is a SaveState operator (1 byte + newline).
        let count = super::MAX_OPERATORS + 1000;
        let stream: Vec<u8> = "q\n".repeat(count).into_bytes();
        let ops = parse_content_stream(&stream).unwrap();
        assert_eq!(ops.len(), super::MAX_OPERATORS);
    }

    #[test]
    fn test_content_stream_consecutive_error_bailout() {
        // A stream of junk bytes that can't be parsed as any operator.
        // The parser should bail out after MAX_CONSECUTIVE_ERRORS skips.
        let junk = vec![0xFFu8; super::MAX_CONSECUTIVE_ERRORS + 500];
        let ops = parse_content_stream(&junk).unwrap();
        assert!(ops.is_empty());
    }

    // ── pdf_manipulator patch: the section header named a parser that is gone ──
    // ── Tests for operand lookup ───────────────────────────────────────
    // ── end pdf_manipulator patch ──

    #[test]
    fn test_do_resolves_name_when_stray_operands_precede_it() {
        // A `q ... /Name Do Q` sequence whose leading numeric operands were
        // never consumed by a `cm` (missing/dropped token, or any other
        // non-conformant producer quirk). Per ISO 32000-1:2008 §7.8.2 an
        // operator's operand is whatever immediately precedes it — here
        // that's the Name, not the stray numbers ahead of it — so Do must
        // still resolve "Overlay", not silently produce "".
        // ── pdf_manipulator patch: the byte-slice text parser this test also ran is gone ──
        let stream = b"q 1 0 0 1 20 150 /Overlay Do Q";
        let ops = parse_content_stream(stream).unwrap();
        assert!(
            ops.iter()
                .any(|op| matches!(op, Operator::Do { ref name } if name == "Overlay")),
            "expected a Do operator naming \"Overlay\", got: {:?}",
            ops
        );
        // ── end pdf_manipulator patch ──
    }

    #[test]
    fn test_skip_operand_token_numbers() {
        assert_eq!(skip_operand_token(b"123 ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"-45.6 ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"+0.5 ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b".002 ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_strings() {
        assert_eq!(skip_operand_token(b"(hello) ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"(nested (parens)) ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"(escaped \\) paren) ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"<48656C6C6F> ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_names_arrays_dicts() {
        assert_eq!(skip_operand_token(b"/Name ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"[1 2 3] ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"[(text) -100] ").unwrap().0, b" ");
        assert_eq!(skip_operand_token(b"<< /K 1 >> ").unwrap().0, b" ");
    }

    // ══════════════════════════════════════════════════════════════════
    // Additional coverage tests
    // ══════════════════════════════════════════════════════════════════

    // ── Inline image parsing ────────────────────────────────────────

    #[test]
    fn test_parse_inline_image_basic() {
        // BI /W 4 /H 4 /BPC 8 /CS /DeviceGray ID <4 bytes data> EI
        let stream = b"BI /W 4 /H 4 /BPC 8 /CS /DeviceGray ID ABCD EI";
        let ops = parse_content_stream(stream).unwrap();
        // The parser should produce at least one InlineImage
        assert!(ops
            .iter()
            .any(|op| matches!(op, Operator::InlineImage { .. })));
        // Check that the inline image dict contains expected keys
        for op in &ops {
            if let Operator::InlineImage { dict, data } = op {
                assert_eq!(dict.get("W").and_then(|o| o.as_integer()), Some(4));
                assert_eq!(dict.get("H").and_then(|o| o.as_integer()), Some(4));
                assert_eq!(dict.get("BPC").and_then(|o| o.as_integer()), Some(8));
                assert!(!data.is_empty());
            }
        }
    }

    #[test]
    fn test_parse_inline_image_empty_data() {
        // Inline image with minimal data
        let stream = b"BI /W 1 /H 1 ID X EI";
        let ops = parse_content_stream(stream).unwrap();
        assert!(ops
            .iter()
            .any(|op| matches!(op, Operator::InlineImage { .. })));
    }

    #[test]
    fn test_parse_inline_image_in_stream_context() {
        // Inline image surrounded by other operators
        let stream = b"q 1 0 0 1 0 0 cm BI /W 2 /H 2 ID AB EI Q";
        let ops = parse_content_stream(stream).unwrap();
        assert!(ops.len() >= 3);
        assert!(matches!(ops[0], Operator::SaveState));
        assert!(matches!(ops[1], Operator::Cm { .. }));
        assert!(ops
            .iter()
            .any(|op| matches!(op, Operator::InlineImage { .. })));
        assert!(ops.iter().any(|op| matches!(op, Operator::RestoreState)));
    }

    #[test]
    fn test_parse_inline_image_null_before_ei() {
        // PDF spec Table 1 lists NULL (0x00) as whitespace.
        let mut stream = b"q BI /W 2 /H 2 ID AB".to_vec();
        stream.extend_from_slice(b"\x00EI Q BT (Hi) Tj ET");
        let ops = parse_content_stream(&stream).unwrap();
        assert!(ops
            .iter()
            .any(|op| matches!(op, Operator::InlineImage { .. })));
        assert!(ops.iter().any(|op| matches!(op, Operator::RestoreState)));
        // Text after the inline image must still be extracted
        assert!(ops.iter().any(|op| matches!(op, Operator::Tj { .. })));
    }

    // ── Number parsing edge cases ───────────────────────────────────

    #[test]
    fn test_parse_negative_numbers() {
        let stream = b"-100 -200 Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert_eq!(*tx, -100.0);
                assert_eq!(*ty, -200.0);
            },
            _ => panic!("Expected Td"),
        }
    }

    #[test]
    fn test_parse_decimal_numbers() {
        let stream = b"0.001 99.999 Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert!((tx - 0.001).abs() < 0.0001);
                assert!((ty - 99.999).abs() < 0.01);
            },
            _ => panic!("Expected Td"),
        }
    }

    #[test]
    fn test_parse_leading_dot_number() {
        let stream = b".5 .25 Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert!((tx - 0.5).abs() < 0.001);
                assert!((ty - 0.25).abs() < 0.001);
            },
            _ => panic!("Expected Td"),
        }
    }

    #[test]
    fn test_parse_large_numbers() {
        let stream = b"99999 88888 Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert_eq!(*tx, 99999.0);
                assert_eq!(*ty, 88888.0);
            },
            _ => panic!("Expected Td"),
        }
    }

    #[test]
    fn test_parse_zero() {
        let stream = b"0 0 Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert_eq!(*tx, 0.0);
                assert_eq!(*ty, 0.0);
            },
            _ => panic!("Expected Td"),
        }
    }

    // ── String parsing edge cases ───────────────────────────────────

    #[test]
    fn test_parse_string_with_nested_parens() {
        let stream = b"(Hello (World)) Tj";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Tj { text } => {
                assert_eq!(text, b"Hello (World)");
            },
            _ => panic!("Expected Tj"),
        }
    }

    #[test]
    fn test_parse_string_with_escape_sequences() {
        let stream = b"(Line1\\nLine2\\r\\t) Tj";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Tj { text } => {
                // The PDF parser should handle escape sequences
                assert!(!text.is_empty());
            },
            _ => panic!("Expected Tj"),
        }
    }

    #[test]
    fn test_parse_empty_string() {
        let stream = b"() Tj";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Tj { text } => {
                assert!(text.is_empty());
            },
            _ => panic!("Expected Tj"),
        }
    }

    #[test]
    fn test_parse_hex_string() {
        let stream = b"<48656C6C6F> Tj";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Tj { text } => {
                assert_eq!(text, b"Hello");
            },
            _ => panic!("Expected Tj"),
        }
    }

    #[test]
    fn test_parse_hex_string_odd_digits() {
        // Odd number of hex digits: trailing nibble should be padded with 0
        let stream = b"<ABC> Tj";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Tj { text } => {
                assert_eq!(text.len(), 2);
                assert_eq!(text[0], 0xAB);
                assert_eq!(text[1], 0xC0);
            },
            _ => panic!("Expected Tj"),
        }
    }

    #[test]
    fn test_parse_empty_hex_string() {
        let stream = b"<> Tj";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Tj { text } => {
                assert!(text.is_empty());
            },
            _ => panic!("Expected Tj"),
        }
    }

    // ── Graphics state operators ────────────────────────────────────

    #[test]
    fn test_parse_line_width() {
        let stream = b"2.5 w";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::SetLineWidth { width } if (width - 2.5).abs() < 0.001));
    }

    #[test]
    fn test_parse_line_cap() {
        let stream = b"1 J";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::SetLineCap { cap_style: 1 }));
    }

    #[test]
    fn test_parse_line_join() {
        let stream = b"2 j";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::SetLineJoin { join_style: 2 }));
    }

    #[test]
    fn test_parse_miter_limit() {
        let stream = b"10 M";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::SetMiterLimit { limit } if limit == 10.0));
    }

    #[test]
    fn test_parse_dash_pattern() {
        let stream = b"[3 2] 0 d";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetDash { array, phase } => {
                assert_eq!(array, &[3.0, 2.0]);
                assert_eq!(*phase, 0.0);
            },
            _ => panic!("Expected SetDash"),
        }
    }

    #[test]
    fn test_parse_rendering_intent() {
        let stream = b"/AbsoluteColorimetric ri";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetRenderingIntent { intent } => {
                assert_eq!(intent, "AbsoluteColorimetric");
            },
            _ => panic!("Expected SetRenderingIntent"),
        }
    }

    #[test]
    fn test_parse_flatness() {
        let stream = b"50 i";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::SetFlatness { tolerance } if tolerance == 50.0));
    }

    #[test]
    fn test_parse_ext_gstate() {
        let stream = b"/GS0 gs";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetExtGState { dict_name } => {
                assert_eq!(dict_name, "GS0");
            },
            _ => panic!("Expected SetExtGState"),
        }
    }

    #[test]
    fn test_parse_paint_shading() {
        let stream = b"/Sh0 sh";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::PaintShading { name } => {
                assert_eq!(name, "Sh0");
            },
            _ => panic!("Expected PaintShading"),
        }
    }

    // ── Color operators ─────────────────────────────────────────────

    #[test]
    fn test_parse_gray_color() {
        let stream = b"0.5 g 0.8 G";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], Operator::SetFillGray { gray } if (gray - 0.5).abs() < 0.001));
        assert!(matches!(ops[1], Operator::SetStrokeGray { gray } if (gray - 0.8).abs() < 0.001));
    }

    #[test]
    fn test_parse_cmyk_color() {
        let stream = b"0.1 0.2 0.3 0.4 k\n0.5 0.6 0.7 0.8 K";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);
        match &ops[0] {
            Operator::SetFillCmyk { c, m, y, k } => {
                assert!((c - 0.1).abs() < 0.01);
                assert!((m - 0.2).abs() < 0.01);
                assert!((y - 0.3).abs() < 0.01);
                assert!((k - 0.4).abs() < 0.01);
            },
            _ => panic!("Expected SetFillCmyk"),
        }
        match &ops[1] {
            Operator::SetStrokeCmyk { c, m, y, k } => {
                assert!((c - 0.5).abs() < 0.01);
                assert!((m - 0.6).abs() < 0.01);
                assert!((y - 0.7).abs() < 0.01);
                assert!((k - 0.8).abs() < 0.01);
            },
            _ => panic!("Expected SetStrokeCmyk"),
        }
    }

    #[test]
    fn test_parse_color_space_operators() {
        let stream = b"/DeviceRGB cs /DeviceCMYK CS";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);
        match &ops[0] {
            Operator::SetFillColorSpace { name } => assert_eq!(name, "DeviceRGB"),
            _ => panic!("Expected SetFillColorSpace"),
        }
        match &ops[1] {
            Operator::SetStrokeColorSpace { name } => assert_eq!(name, "DeviceCMYK"),
            _ => panic!("Expected SetStrokeColorSpace"),
        }
    }

    #[test]
    fn test_parse_sc_color_components() {
        let stream = b"0.1 0.2 0.3 sc";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetFillColor { components } => {
                assert_eq!(components.len(), 3);
                assert!((components[0] - 0.1).abs() < 0.01);
                assert!((components[1] - 0.2).abs() < 0.01);
                assert!((components[2] - 0.3).abs() < 0.01);
            },
            _ => panic!("Expected SetFillColor"),
        }
    }

    #[test]
    fn test_parse_sc_stroke_color_components() {
        let stream = b"0.5 0.6 SC";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetStrokeColor { components } => {
                assert_eq!(components.len(), 2);
            },
            _ => panic!("Expected SetStrokeColor"),
        }
    }

    #[test]
    fn test_parse_scn_with_pattern_name() {
        let stream = b"0.5 /Pattern1 scn";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetFillColorN { components, name } => {
                assert_eq!(components.len(), 1);
                assert!(name.is_some());
                assert_eq!(**name.as_ref().unwrap(), "Pattern1");
            },
            _ => panic!("Expected SetFillColorN"),
        }
    }

    #[test]
    fn test_parse_scn_without_pattern_name() {
        let stream = b"0.1 0.2 0.3 scn";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetFillColorN { components, name } => {
                assert_eq!(components.len(), 3);
                assert!(name.is_none());
            },
            _ => panic!("Expected SetFillColorN"),
        }
    }

    #[test]
    fn test_parse_scn_stroke_with_pattern() {
        let stream = b"0.5 /P1 SCN";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::SetStrokeColorN { components, name } => {
                assert_eq!(components.len(), 1);
                assert!(name.is_some());
            },
            _ => panic!("Expected SetStrokeColorN"),
        }
    }

    // ── Marked content operators ────────────────────────────────────

    #[test]
    fn test_parse_bmc() {
        let stream = b"/Span BMC";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::BeginMarkedContent { tag } => assert_eq!(tag, "Span"),
            _ => panic!("Expected BeginMarkedContent"),
        }
    }

    #[test]
    fn test_parse_bdc_with_dict() {
        let stream = b"/Span << /MCID 0 >> BDC";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::BeginMarkedContentDict { tag, properties } => {
                assert_eq!(tag, "Span");
                assert!(!matches!(**properties, Object::Null));
            },
            _ => panic!("Expected BeginMarkedContentDict"),
        }
    }

    #[test]
    fn test_parse_bdc_with_name_ref() {
        let stream = b"/Span /MC0 BDC";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::BeginMarkedContentDict { tag, properties } => {
                assert_eq!(tag, "Span");
                assert_eq!(properties.as_name(), Some("MC0"));
            },
            _ => panic!("Expected BeginMarkedContentDict"),
        }
    }

    #[test]
    fn test_parse_emc() {
        let stream = b"EMC";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::EndMarkedContent));
    }

    #[test]
    fn test_parse_marked_content_nesting() {
        let stream = b"/Article BMC /P << /MCID 1 >> BDC BT (text) Tj ET EMC EMC";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 7);
        assert!(matches!(ops[0], Operator::BeginMarkedContent { .. }));
        assert!(matches!(ops[1], Operator::BeginMarkedContentDict { .. }));
        assert!(matches!(ops[2], Operator::BeginText));
        assert!(matches!(ops[3], Operator::Tj { .. }));
        assert!(matches!(ops[4], Operator::EndText));
        assert!(matches!(ops[5], Operator::EndMarkedContent));
        assert!(matches!(ops[6], Operator::EndMarkedContent));
    }

    // ── Path operators ──────────────────────────────────────────────

    #[test]
    fn test_parse_bezier_curves() {
        let stream = b"10 20 30 40 50 60 c";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
            } => {
                assert_eq!(*x1, 10.0);
                assert_eq!(*y1, 20.0);
                assert_eq!(*x2, 30.0);
                assert_eq!(*y2, 40.0);
                assert_eq!(*x3, 50.0);
                assert_eq!(*y3, 60.0);
            },
            _ => panic!("Expected CurveTo"),
        }
    }

    #[test]
    fn test_parse_curve_to_v() {
        let stream = b"10 20 30 40 v";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::CurveToV { x2, y2, x3, y3 } => {
                assert_eq!(*x2, 10.0);
                assert_eq!(*y2, 20.0);
                assert_eq!(*x3, 30.0);
                assert_eq!(*y3, 40.0);
            },
            _ => panic!("Expected CurveToV"),
        }
    }

    #[test]
    fn test_parse_curve_to_y() {
        let stream = b"10 20 30 40 y";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::CurveToY { x1, y1, x3, y3 } => {
                assert_eq!(*x1, 10.0);
                assert_eq!(*y1, 20.0);
                assert_eq!(*x3, 30.0);
                assert_eq!(*y3, 40.0);
            },
            _ => panic!("Expected CurveToY"),
        }
    }

    #[test]
    fn test_parse_close_path() {
        let stream = b"h";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::ClosePath));
    }

    #[test]
    fn test_parse_fill_variants() {
        let stream = b"f\nf*\nn";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 3);
        assert!(matches!(ops[0], Operator::Fill));
        assert!(matches!(ops[1], Operator::FillEvenOdd));
        assert!(matches!(ops[2], Operator::EndPath));
    }

    #[test]
    fn test_parse_close_fill_stroke() {
        let stream = b"b";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::CloseFillStroke));
    }

    #[test]
    fn test_parse_clipping() {
        let stream = b"W\nW*";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], Operator::ClipNonZero));
        assert!(matches!(ops[1], Operator::ClipEvenOdd));
    }

    #[test]
    fn test_parse_rectangle() {
        let stream = b"10 20 100 50 re";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Rectangle {
                x,
                y,
                width,
                height,
            } => {
                assert_eq!(*x, 10.0);
                assert_eq!(*y, 20.0);
                assert_eq!(*width, 100.0);
                assert_eq!(*height, 50.0);
            },
            _ => panic!("Expected Rectangle"),
        }
    }

    // ── Text state operators ────────────────────────────────────────

    #[test]
    fn test_parse_tr_render_mode() {
        let stream = b"1 Tr";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::Tr { render: 1 }));
    }

    #[test]
    fn test_parse_ts_text_rise() {
        let stream = b"5 Ts";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::Ts { rise } if rise == 5.0));
    }

    #[test]
    fn test_parse_double_quote_operator() {
        let stream = b"1.5 0.5 (Hello) \"";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::DoubleQuote {
                word_space,
                char_space,
                text,
            } => {
                assert!((word_space - 1.5).abs() < 0.001);
                assert!((char_space - 0.5).abs() < 0.001);
                assert_eq!(text, b"Hello");
            },
            _ => panic!("Expected DoubleQuote"),
        }
    }

    #[test]
    fn test_parse_single_quote_operator() {
        let stream = b"(NextLine) '";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Quote { text } => {
                assert_eq!(text, b"NextLine");
            },
            _ => panic!("Expected Quote"),
        }
    }

    // ── Unknown / Other operators ───────────────────────────────────

    #[test]
    fn test_parse_unknown_operator() {
        let stream = b"42 XYZ";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Other { name, operands } => {
                assert_eq!(name, "XYZ");
                assert_eq!(operands.len(), 1);
            },
            _ => panic!("Expected Other operator"),
        }
    }

    #[test]
    fn test_parse_unknown_operator_no_operands() {
        let stream = b"BX";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Other { name, operands } => {
                assert_eq!(name, "BX");
                assert_eq!(operands.len(), 0);
            },
            _ => panic!("Expected Other for BX"),
        }
    }

    #[test]
    fn test_parse_compatibility_operators() {
        let stream = b"BX EX";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], Operator::Other { name, .. } if name == "BX"));
        assert!(matches!(&ops[1], Operator::Other { name, .. } if name == "EX"));
    }

    #[test]
    fn test_parse_mp_dp_marked_point() {
        let stream = b"/Tag MP /Tag2 << /K 1 >> DP";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(matches!(&ops[0], Operator::Other { name, .. } if name == "MP"));
        assert!(matches!(&ops[1], Operator::Other { name, .. } if name == "DP"));
    }

    // ── Error handling / malformed streams ───────────────────────────

    #[test]
    fn test_parse_recovers_from_garbage_bytes() {
        // Garbage followed by valid operators
        let mut stream = vec![0xFF, 0xFE, 0xFD];
        stream.extend_from_slice(b" BT (Hello) Tj ET");
        let ops = parse_content_stream(&stream).unwrap();
        // Should recover and parse the text block
        assert!(ops.iter().any(|op| matches!(op, Operator::BeginText)));
        assert!(ops.iter().any(|op| matches!(op, Operator::Tj { .. })));
    }

    #[test]
    fn test_parse_truncated_inline_image() {
        // BI without matching EI
        let stream = b"BI /W 2 /H 2 ID AAAA";
        let ops = parse_content_stream(stream).unwrap();
        // Should not crash; may produce 0 ops or recover gracefully
        // The parser should handle the missing EI
        let _ = ops; // just ensure no panic
    }

    #[test]
    fn test_parse_unbalanced_bt_et() {
        // Extra ET without matching BT
        let stream = b"ET";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Operator::EndText));
    }

    #[test]
    fn test_parse_missing_operands() {
        // Td with no operands - should use defaults
        let stream = b"Td";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert_eq!(*tx, 0.0);
                assert_eq!(*ty, 0.0);
            },
            _ => panic!("Expected Td with default values"),
        }
    }

    #[test]
    fn test_parse_stream_with_only_comments() {
        let stream = b"% This is a comment\n% Another comment\n";
        let ops = parse_content_stream(stream).unwrap();
        // Comments should be skipped, resulting in no operators
        // The parser skips unknown bytes, so comments are handled gracefully
        let _ = ops;
    }

    // ── Complex / combined streams ──────────────────────────────────

    #[test]
    fn test_parse_complete_page_stream() {
        // Simulates a realistic mini-page content stream
        let stream = b"q\n\
            1 0 0 1 72 720 cm\n\
            /GS0 gs\n\
            BT\n\
            /F1 12 Tf\n\
            0 0 Td\n\
            14 TL\n\
            (First line) Tj\n\
            T*\n\
            (Second line) Tj\n\
            ET\n\
            Q\n\
            0 0 612 792 re\n\
            S";
        let ops = parse_content_stream(stream).unwrap();
        assert!(ops.len() >= 10);

        // Check key operators are present
        assert!(matches!(ops[0], Operator::SaveState));
        assert!(matches!(ops[1], Operator::Cm { .. }));
        assert!(matches!(ops[2], Operator::SetExtGState { .. }));
        assert!(matches!(ops[3], Operator::BeginText));
        assert!(ops.iter().any(|op| matches!(op, Operator::TStar)));
        assert!(ops.iter().any(|op| matches!(op, Operator::EndText)));
        assert!(ops.iter().any(|op| matches!(op, Operator::RestoreState)));
        assert!(ops
            .iter()
            .any(|op| matches!(op, Operator::Rectangle { .. })));
        assert!(ops.iter().any(|op| matches!(op, Operator::Stroke)));
    }

    #[test]
    fn test_parse_mixed_text_and_graphics() {
        let stream = b"q\n\
            0 0 100 100 re W n\n\
            BT /F1 10 Tf 0 0 Td (Hello) Tj ET\n\
            1 0 0 rg\n\
            0 0 m 100 0 l 100 100 l h f\n\
            Q";
        let ops = parse_content_stream(stream).unwrap();
        assert!(ops.len() > 5);
        // Verify text and graphics operators are both present
        assert!(ops.iter().any(|op| matches!(op, Operator::BeginText)));
        assert!(ops.iter().any(|op| matches!(op, Operator::Tj { .. })));
        assert!(ops.iter().any(|op| matches!(op, Operator::MoveTo { .. })));
        assert!(ops.iter().any(|op| matches!(op, Operator::Fill)));
    }

    #[test]
    fn test_parse_tj_with_hex_strings_in_array() {
        let stream = b"[<48656C6C6F> -50 <576F726C64>] TJ";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::TJ { array } => {
                assert_eq!(array.len(), 3);
                match &array[0] {
                    TextElement::String(s) => assert_eq!(s, b"Hello"),
                    _ => panic!("Expected string"),
                }
                assert!(matches!(array[1], TextElement::Offset(_)));
                match &array[2] {
                    TextElement::String(s) => assert_eq!(s, b"World"),
                    _ => panic!("Expected string"),
                }
            },
            _ => panic!("Expected TJ"),
        }
    }

    #[test]
    fn test_parse_td_with_varied_whitespace() {
        // Operators separated by various whitespace types
        let stream = b"100\t200\nTd";
        let ops = parse_content_stream(stream).unwrap();
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operator::Td { tx, ty } => {
                assert_eq!(*tx, 100.0);
                assert_eq!(*ty, 200.0);
            },
            _ => panic!("Expected Td"),
        }
    }

    #[test]
    fn test_parse_multiple_bt_et_blocks() {
        let stream = b"BT (A) Tj ET BT (B) Tj ET BT (C) Tj ET";
        let ops = parse_content_stream(stream).unwrap();
        // 3 blocks * 3 ops each = 9 ops
        assert_eq!(ops.len(), 9);
        let bt_count = ops
            .iter()
            .filter(|op| matches!(op, Operator::BeginText))
            .count();
        let et_count = ops
            .iter()
            .filter(|op| matches!(op, Operator::EndText))
            .count();
        assert_eq!(bt_count, 3);
        assert_eq!(et_count, 3);
    }

    // ── Helper function tests ───────────────────────────────────────

    #[test]
    fn test_is_operator_start() {
        assert!(is_operator_start(b'B'));
        assert!(is_operator_start(b'a'));
        assert!(is_operator_start(b'\''));
        assert!(is_operator_start(b'"'));
        assert!(is_operator_start(b'*'));
        assert!(!is_operator_start(b'0'));
        assert!(!is_operator_start(b' '));
        assert!(!is_operator_start(b'('));
        assert!(!is_operator_start(b'/'));
    }

    #[test]
    fn test_is_whitespace() {
        // NUL (0x00) is one of the six PDF white-space chars (ISO 32000-1:2008 §7.2, Table 1).
        assert!(is_whitespace(0x00));
        assert!(is_whitespace(b' '));
        assert!(is_whitespace(b'\t'));
        assert!(is_whitespace(b'\r'));
        assert!(is_whitespace(b'\n'));
        assert!(is_whitespace(0x0C));
        assert!(!is_whitespace(b'A'));
        assert!(!is_whitespace(b'0'));
    }

    #[test]
    fn test_is_whitespace_or_delimiter() {
        assert!(is_whitespace_or_delimiter(b' '));
        assert!(is_whitespace_or_delimiter(b'('));
        assert!(is_whitespace_or_delimiter(b')'));
        assert!(is_whitespace_or_delimiter(b'<'));
        assert!(is_whitespace_or_delimiter(b'>'));
        assert!(is_whitespace_or_delimiter(b'['));
        assert!(is_whitespace_or_delimiter(b']'));
        assert!(is_whitespace_or_delimiter(b'{'));
        assert!(is_whitespace_or_delimiter(b'}'));
        assert!(is_whitespace_or_delimiter(b'/'));
        assert!(is_whitespace_or_delimiter(b'%'));
        assert!(!is_whitespace_or_delimiter(b'A'));
        assert!(!is_whitespace_or_delimiter(b'0'));
    }

    // ── find_ei_operator tests ──────────────────────────────────────

    #[test]
    fn test_find_ei_operator_basic() {
        let data = b"binary data \nEI ";
        let result = find_ei_operator(data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_ei_operator_at_end() {
        let data = b"data \nEI";
        let result = find_ei_operator(data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_find_ei_operator_not_found() {
        let data = b"binary data without end marker";
        let result = find_ei_operator(data);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_ei_operator_ei_without_whitespace_prefix() {
        // EI without preceding whitespace should not match
        let data = b"dataEI ";
        let result = find_ei_operator(data);
        assert!(result.is_err());
    }

    // ── Operator limit enforcement ──────────────────────────────────

    // ── Consistency tests ───────────────────────────────────────────

    // ── Edge cases for operand parsing via nom ──────────────────────

    #[test]
    fn test_skip_operand_token_dict_with_strings() {
        assert_eq!(skip_operand_token(b"<< /Key (value) >> ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_nested_array() {
        assert_eq!(skip_operand_token(b"[1 [2 3] 4] ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_array_with_hex_string() {
        assert_eq!(skip_operand_token(b"[<4142> 1] ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_array_with_dict() {
        assert_eq!(skip_operand_token(b"[<< /K 1 >>] ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_dict_with_nested_dict() {
        assert_eq!(skip_operand_token(b"<< /A << /B 1 >> >> ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_dict_with_hex_string() {
        assert_eq!(skip_operand_token(b"<< /K <4142> >> ").unwrap().0, b" ");
    }

    #[test]
    fn test_skip_operand_token_errors() {
        // Characters that don't start a valid operand
        assert!(skip_operand_token(b"").is_err());
        assert!(skip_operand_token(b"@").is_err());
    }

}
