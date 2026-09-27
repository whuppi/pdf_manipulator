//! Widget-appearance regeneration shared between the form flattener
//! (`DocumentEditor`) and the page renderer.
//!
//! This file is part of the pdf_manipulator fork (NOT upstream). The appearance
//! generation below used to live as private methods on `DocumentEditor`,
//! reachable only by the flatten path. The page renderer holds a `&PdfDocument`
//! (not a `DocumentEditor`) and needs the same regeneration, so that a reopened
//! filled form rasterizes its `/V` value instead of the stale `/AP` placeholder
//! the form writer left behind (ISO 32000-1:2008 §12.7.3.3, Table 226 — a
//! present `/AP` otherwise takes precedence over `/DA`, so a `/NeedAppearances
//! true` document must be regenerated). The logic only ever read the document,
//! so it moves verbatim to `impl PdfDocument`; `DocumentEditor` now delegates
//! here. The move also drops the editor's one-level `resolve_obj`, which
//! reinvented `PdfDocument::resolve_object`.

use std::collections::HashMap;

use crate::annotations::Annotation;
use crate::document::PdfDocument;
use crate::error::Result;
use crate::object::Object;

/// A regenerated widget appearance: the content-stream bytes, its bounding box
/// (`[0 0 w h]`), and the `/Resources` needed to draw it. Both the flattener
/// (which wraps it into an `AnnotationAppearance`) and the renderer (which draws
/// it as a synthetic form XObject) consume this.
pub(crate) struct GeneratedAppearance {
    pub content: Vec<u8>,
    pub bbox: [f32; 4],
    pub resources: Option<Object>,
}

impl PdfDocument {
    /// Whether the document-wide AcroForm has `/NeedAppearances true`. When set,
    /// a present `/AP` may be a stale placeholder the form writer never
    /// refreshed; appearances must be regenerated from `/V`+`/DA` rather than
    /// trusted. Our save path sets this flag whenever a field value is written,
    /// so it is present after a save+reopen.
    pub(crate) fn acroform_needs_appearances(&self) -> bool {
        let cat = match self.catalog() {
            Ok(c) => c,
            Err(_) => return false,
        };
        let af_ref = match cat.as_dict().and_then(|d| d.get("AcroForm")) {
            Some(o) => o,
            None => return false,
        };
        let af = self.resolve_object(af_ref).unwrap_or(Object::Null);
        matches!(
            af.as_dict().and_then(|d| d.get("NeedAppearances")),
            Some(Object::Boolean(true))
        )
    }

    /// The interactive form's default resource dictionary (`/DR`), resolved.
    fn acroform_default_resources(&self) -> Option<Object> {
        let cat = self.catalog().ok()?;
        let af = self.resolve_object(cat.as_dict()?.get("AcroForm")?).ok()?;
        let dr = af.as_dict()?.get("DR")?.clone();
        self.resolve_object(&dr).ok()
    }

    /// The field's effective default-appearance string: the widget's own `/DA`,
    /// else the document-wide AcroForm `/DA` (ISO 32000-1 §12.7.3.3, inheritable).
    pub(crate) fn effective_da(&self, annotation: &Annotation) -> Option<String> {
        if let Some(Object::String(s)) = annotation.raw_dict.as_ref().and_then(|d| d.get("DA")) {
            return Some(String::from_utf8_lossy(s).into_owned());
        }
        let cat = self.catalog().ok()?;
        let af = self.resolve_object(cat.as_dict()?.get("AcroForm")?).ok()?;
        match af.as_dict()?.get("DA") {
            Some(Object::String(s)) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        }
    }

    /// Parse a `/DA` string into `(font resource name incl. '/', size, rgb)`.
    /// A size of 0 means auto-size; colour defaults to black.
    pub(crate) fn parse_da(da: &str) -> (String, f32, (f32, f32, f32)) {
        let toks: Vec<&str> = da.split_whitespace().collect();
        let mut font = "/Helv".to_string();
        let mut size = 0.0f32;
        let mut color = (0.0, 0.0, 0.0);
        for i in 0..toks.len() {
            match toks[i] {
                "Tf" if i >= 2 => {
                    if toks[i - 2].starts_with('/') {
                        font = toks[i - 2].to_string();
                    }
                    if let Ok(s) = toks[i - 1].parse::<f32>() {
                        size = s;
                    }
                },
                "g" if i >= 1 => {
                    if let Ok(v) = toks[i - 1].parse::<f32>() {
                        color = (v, v, v);
                    }
                },
                "rg" if i >= 3 => {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        toks[i - 3].parse::<f32>(),
                        toks[i - 2].parse::<f32>(),
                        toks[i - 1].parse::<f32>(),
                    ) {
                        color = (r, g, b);
                    }
                },
                _ => {},
            }
        }
        (font, size, color)
    }

    /// A self-contained standard-14 Helvetica font dictionary (no external
    /// dependencies, so it survives the garbage-collected full rewrite).
    fn helvetica_font_dict() -> Object {
        let mut d = HashMap::new();
        d.insert("Type".to_string(), Object::Name("Font".to_string()));
        d.insert("Subtype".to_string(), Object::Name("Type1".to_string()));
        d.insert("BaseFont".to_string(), Object::Name("Helvetica".to_string()));
        d.insert("Encoding".to_string(), Object::Name("WinAnsiEncoding".to_string()));
        Object::Dictionary(d)
    }

    /// Build the `/Resources` dict for a regenerated text appearance so the
    /// `/DA` font name resolves inside the form XObject. The form's `/DR` fonts
    /// are referenced from the AcroForm, which is dropped when flattening (so
    /// its font objects are garbage-collected). We therefore *inline* a
    /// self-contained font dict under the `/DA` name: the document's own font
    /// when it is a standard-14 Type1 (no embedded program), otherwise a
    /// Helvetica stand-in. Non-Latin fonts that need an embedded program are
    /// handled by the flattener's fallback-embedding path.
    pub(crate) fn build_text_appearance_resources(&self, da_font_name: &str) -> Object {
        let name = da_font_name.trim_start_matches('/').to_string();

        let font_dict = self
            .acroform_default_resources()
            .and_then(|dr| {
                dr.as_dict()?
                    .get("Font")
                    .map(|f| self.resolve_object(f).unwrap_or(Object::Null))
            })
            .and_then(|fonts| {
                fonts
                    .as_dict()?
                    .get(&name)
                    .map(|f| self.resolve_object(f).unwrap_or(Object::Null))
            })
            .filter(Self::is_self_contained_simple_font)
            .unwrap_or_else(Self::helvetica_font_dict);

        let mut fonts = HashMap::new();
        fonts.insert(name, font_dict);
        let mut res = HashMap::new();
        res.insert("Font".to_string(), Object::Dictionary(fonts));
        Object::Dictionary(res)
    }

    /// True for a standard-14 Type1 font dict with no embedded font program,
    /// i.e. one that can be inlined verbatim and stay self-contained.
    fn is_self_contained_simple_font(font: &Object) -> bool {
        let Some(d) = font.as_dict() else {
            return false;
        };
        if d.get("Subtype").and_then(|s| s.as_name()) != Some("Type1") {
            return false;
        }
        // A FontDescriptor implies an embedded program / extra object graph.
        !d.contains_key("FontDescriptor")
    }

    /// Regenerate a widget's appearance from its field value, `/DA`, and `/DR`
    /// (ISO 32000-1 §12.7.3.3). Returns the content-stream bytes plus the
    /// `/Resources` the content references. Text-bearing fields draw with no
    /// opaque background or border so they never paint over page content the
    /// field rect overlaps. Returns `None` for signatures / unknown fields.
    ///
    /// This is the Latin/`/DA`-font path shared with the renderer; the
    /// flattener additionally tries an embedded-font path first for CJK/emoji
    /// values the `/DA` font cannot render.
    pub(crate) fn regenerate_widget_appearance(
        &self,
        annotation: &Annotation,
    ) -> Result<Option<GeneratedAppearance>> {
        use crate::annotation_types::WidgetFieldType;
        use crate::geometry::Rect;
        use crate::writer::FormAppearanceGenerator;

        let rect = match annotation.rect {
            Some(r) => r,
            None => return Ok(None),
        };
        let width = rect[2] as f32 - rect[0] as f32;
        let height = rect[3] as f32 - rect[1] as f32;
        let geom_rect = Rect::new(0.0, 0.0, width, height);

        let field_type = annotation.field_type.as_ref();
        let shape_generator = FormAppearanceGenerator::new()
            .with_background(1.0, 1.0, 1.0)
            .with_border(1.0, 0.0, 0.0, 0.0);
        let text_generator = FormAppearanceGenerator::new();

        let (da_font, da_size, da_color) =
            Self::parse_da(&self.effective_da(annotation).unwrap_or_default());
        // Auto-size (/DA size 0) → fit the annotation height with padding.
        let font_size = if da_size > 0.0 {
            da_size
        } else {
            (height * 0.7).clamp(6.0, 12.0)
        };

        let mut text_resources: Option<Object> = None;
        let content_str = match field_type {
            Some(WidgetFieldType::Text) => {
                let text = annotation.field_value.as_deref().unwrap_or("");
                text_resources = Some(self.build_text_appearance_resources(&da_font));
                text_generator.text_field_appearance(geom_rect, text, &da_font, font_size, da_color)
            },
            Some(WidgetFieldType::Checkbox { checked }) => {
                if *checked {
                    shape_generator.checkbox_on_appearance(geom_rect, (0.0, 0.0, 0.0))
                } else {
                    shape_generator.checkbox_off_appearance(geom_rect)
                }
            },
            Some(WidgetFieldType::Radio { selected }) => {
                if selected.is_some() {
                    shape_generator.radio_on_appearance(geom_rect, (0.0, 0.0, 0.0))
                } else {
                    shape_generator.radio_off_appearance(geom_rect)
                }
            },
            Some(WidgetFieldType::Button) => {
                let caption = annotation.field_value.as_deref().unwrap_or("");
                text_resources = Some(self.build_text_appearance_resources(&da_font));
                text_generator.button_appearance(geom_rect, caption, &da_font, font_size, da_color)
            },
            Some(WidgetFieldType::Choice { selected, .. }) => {
                let text = selected.as_deref().unwrap_or("");
                text_resources = Some(self.build_text_appearance_resources(&da_font));
                text_generator.text_field_appearance(geom_rect, text, &da_font, font_size, da_color)
            },
            Some(WidgetFieldType::Signature) | Some(WidgetFieldType::Unknown) | None => {
                return Ok(None);
            },
        };

        Ok(Some(GeneratedAppearance {
            content: content_str.into_bytes(),
            bbox: [0.0, 0.0, width, height],
            resources: text_resources,
        }))
    }
}
