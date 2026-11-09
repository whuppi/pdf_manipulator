//! Choice-field values: a field's options, its flags and the `/I` entry a
//! list box carries beside `/V`.
//!
//! A choice field's `/Opt` lists each option as a text string (the export
//! value a `/V` holds is also what a viewer shows) or as a pair
//! `[export display]`. `/Opt`, `/Ff` and `/V` are inheritable, so each is read
//! from the field's own dictionary and then up its `/Parent` chain.
//!
//! The editor's flush writes `/V` and calls [`selected_indexes`] for the `/I`
//! entry; the host's value edits call [`options_of`] to refuse a value the
//! field does not offer; the widget appearance reads the same options.

use crate::document::PdfDocument;
use crate::editor::form_fields::FormFieldValue;
use crate::extractors::forms::field_flags;
use crate::object::{decode_pdf_text_string, Object};
use std::collections::HashMap;

/// How far up a field's `/Parent` chain an inheritable key is looked for.
const PARENT_DEPTH: usize = 16;

/// One option of a choice field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceOption {
    /// What `/V` holds when the option is selected.
    pub export: String,
    /// What a viewer shows for the option.
    pub display: String,
}

/// The value of `key` on the field's own dictionary or on its nearest
/// ancestor that sets it, with an indirect reference resolved.
pub(crate) fn inherited(
    doc: &PdfDocument,
    field: &HashMap<String, Object>,
    key: &str,
) -> Option<Object> {
    let mut dict = field.clone();
    for _ in 0..PARENT_DEPTH {
        if let Some(value) = dict.get(key) {
            return doc.resolve_object(value).ok();
        }
        let parent = dict.get("Parent")?.as_reference()?;
        dict = doc.load_object(parent).ok()?.as_dict()?.clone();
    }
    None
}

/// The field's effective `/Ff` bits.
pub(crate) fn flags_of(doc: &PdfDocument, field: &HashMap<String, Object>) -> u32 {
    match inherited(doc, field, "Ff") {
        Some(Object::Integer(n)) => n as u32,
        _ => 0,
    }
}

fn text_of(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes) => Some(decode_pdf_text_string(bytes)),
        Object::Name(name) => Some(name.clone()),
        _ => None,
    }
}

/// The field's options, in `/Opt` order; empty when it has none.
pub(crate) fn options_of(doc: &PdfDocument, field: &HashMap<String, Object>) -> Vec<ChoiceOption> {
    let Some(Object::Array(entries)) = inherited(doc, field, "Opt") else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let entry = doc.resolve_object(entry).ok()?;
            match &entry {
                Object::Array(pair) => {
                    let export = text_of(&doc.resolve_object(pair.first()?).ok()?)?;
                    let display = pair
                        .get(1)
                        .and_then(|d| doc.resolve_object(d).ok())
                        .and_then(|d| text_of(&d))
                        .unwrap_or_else(|| export.clone());
                    Some(ChoiceOption { export, display })
                },
                other => {
                    let text = text_of(other)?;
                    Some(ChoiceOption { export: text.clone(), display: text })
                },
            }
        })
        .collect()
}

/// Whether `flags` describe a combo box (one line, a drop-down).
pub(crate) fn is_combo(flags: u32) -> bool {
    flags & field_flags::COMBO != 0
}

/// The values a choice value selects, in the order given.
fn selected_values(value: &FormFieldValue) -> Vec<&str> {
    match value {
        FormFieldValue::Choice(s) | FormFieldValue::Text(s) => vec![s.as_str()],
        FormFieldValue::MultiChoice(values) => values.iter().map(String::as_str).collect(),
        FormFieldValue::Boolean(_) | FormFieldValue::None => Vec::new(),
    }
}

/// The `/I` entry for a list box whose value is `value`: the indexes, in
/// ascending order, of the options it selects. `None` when the field is not a
/// list box (a combo box has no `/I`) or the value selects no listed option.
pub(crate) fn selected_indexes(
    doc: &PdfDocument,
    field: &HashMap<String, Object>,
    value: &FormFieldValue,
) -> Option<Object> {
    if is_combo(flags_of(doc, field)) {
        return None;
    }
    let options = options_of(doc, field);
    let selected = selected_values(value);
    let indexes: Vec<Object> = options
        .iter()
        .enumerate()
        .filter(|(_, option)| selected.contains(&option.export.as_str()))
        .map(|(at, _)| Object::Integer(at as i64))
        .collect();
    (!indexes.is_empty()).then(|| Object::Array(indexes))
}
