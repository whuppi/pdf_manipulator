//! XFA (XML Forms Architecture) parser.
//!
//! This module parses XFA form data from PDF documents. XFA is an XML-based
//! form specification used in some PDFs, particularly government and financial forms.
//!
//! # XFA Packet Structure
//!
//! XFA forms contain several XML packets:
//! - **template**: Form structure and field definitions
//! - **datasets**: Form data values
//! - **config**: Configuration settings
//! - **form**: Runtime form state
//!
//! # Limitations
//!
//! This parser supports static XFA conversion only:
//! - Extracts field definitions and values
//! - Does NOT support dynamic XFA features (scripts, calculations)
//! - Does NOT support complex layouts (tables, grids)

use crate::error::{Error, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
// ── pdf_manipulator patch: packets are read from a reader ──
use std::io::BufRead;
// ── end pdf_manipulator patch ──

/// US Letter page width in points (8.5 inches × 72 points/inch).
const LETTER_WIDTH: f32 = 612.0;
/// US Letter page height in points (11 inches × 72 points/inch).
const LETTER_HEIGHT: f32 = 792.0;

/// XFA field type extracted from template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XfaFieldType {
    /// Text field (textEdit)
    Text,
    /// Numeric field (numericEdit)
    Numeric,
    /// Date field (dateTimeEdit)
    DateTime,
    /// Checkbox (checkButton)
    Checkbox,
    /// Radio button group (choiceList with appearance=minimal)
    RadioGroup,
    /// Drop-down list (choiceList)
    DropDown,
    /// List box (choiceList with appearance=full)
    ListBox,
    /// Button (button)
    Button,
    /// Signature field (signature)
    Signature,
    /// Image field (imageEdit)
    Image,
    /// Barcode field (barcode)
    Barcode,
    /// Unknown field type
    Unknown(String),
}

impl XfaFieldType {
    /// Parse from XFA UI element name.
    pub fn from_xfa_name(name: &str) -> Self {
        match name {
            "textEdit" => Self::Text,
            "numericEdit" => Self::Numeric,
            "dateTimeEdit" => Self::DateTime,
            "checkButton" => Self::Checkbox,
            "choiceList" => Self::DropDown, // Default, may be ListBox based on appearance
            "button" => Self::Button,
            "signature" => Self::Signature,
            "imageEdit" => Self::Image,
            "barcode" => Self::Barcode,
            _ => Self::Unknown(name.to_string()),
        }
    }
}

/// An option in a choice field (dropdown, list box, radio group).
#[derive(Debug, Clone)]
pub struct XfaOption {
    /// Display text
    pub text: String,
    /// Value when selected
    pub value: String,
}

/// A field extracted from XFA template.
#[derive(Debug, Clone)]
pub struct XfaField {
    /// Field name
    pub name: String,
    /// Full path/binding (e.g., `topmostSubform[0].Page1[0].field1[0]`)
    pub binding: String,
    /// Field type
    pub field_type: XfaFieldType,
    /// Tooltip/caption
    pub tooltip: Option<String>,
    /// Caption text
    pub caption: Option<String>,
    /// Default value
    pub default_value: Option<String>,
    /// Current value from datasets
    pub value: Option<String>,
    /// Options for choice fields
    pub options: Vec<XfaOption>,
    /// Is field required
    pub required: bool,
    /// Is field read-only
    pub readonly: bool,
    /// Maximum length (for text fields)
    pub max_length: Option<u32>,
    /// Width hint
    pub width: Option<f32>,
    /// Height hint
    pub height: Option<f32>,
    /// X position hint
    pub x: Option<f32>,
    /// Y position hint
    pub y: Option<f32>,
}

impl XfaField {
    /// Create a new XFA field.
    pub fn new(name: impl Into<String>, binding: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            binding: binding.into(),
            field_type: XfaFieldType::Text,
            tooltip: None,
            caption: None,
            default_value: None,
            value: None,
            options: Vec::new(),
            required: false,
            readonly: false,
            max_length: None,
            width: None,
            height: None,
            x: None,
            y: None,
        }
    }
}

/// A page extracted from XFA template.
#[derive(Debug, Clone)]
pub struct XfaPage {
    /// Page name
    pub name: String,
    /// Fields on this page
    pub fields: Vec<XfaField>,
    /// Width in points
    pub width: f32,
    /// Height in points
    pub height: f32,
}

impl Default for XfaPage {
    fn default() -> Self {
        Self {
            name: String::new(),
            fields: Vec::new(),
            width: LETTER_WIDTH,
            height: LETTER_HEIGHT,
        }
    }
}

/// Parsed XFA form.
#[derive(Debug, Clone, Default)]
pub struct XfaForm {
    /// Form name/title
    pub name: Option<String>,
    /// Pages in the form
    pub pages: Vec<XfaPage>,
    /// All fields (flattened)
    pub fields: Vec<XfaField>,
    /// Field values from datasets (binding -> value)
    pub dataset_values: HashMap<String, String>,
    // ── pdf_manipulator patch: the packets are read as events, so the form keeps no raw XML ──
    // ── end pdf_manipulator patch ──
}

impl XfaForm {
    /// Get a field by name.
    pub fn get_field(&self, name: &str) -> Option<&XfaField> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get a field by binding path.
    pub fn get_field_by_binding(&self, binding: &str) -> Option<&XfaField> {
        self.fields.iter().find(|f| f.binding == binding)
    }

    /// Get total number of fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

// ── pdf_manipulator patch: the state a packet's events carry, and which section they are in ──
/// The part of an XDP an event belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Template,
    Datasets,
}

/// What reading a template carries from one event to the next.
#[derive(Default)]
struct TemplateState {
    current_field: Option<XfaField>,
    current_page: Option<XfaPage>,
    in_items: bool,
    option_text: String,
    option_value: String,
}

/// What reading datasets carries from one event to the next.
#[derive(Default)]
struct DatasetsState {
    path_stack: Vec<String>,
    current_text: String,
}
// ── end pdf_manipulator patch ──

/// XFA parser.
pub struct XfaParser {
    /// Parsed form
    form: XfaForm,
    /// Current parsing context stack (element names)
    context_stack: Vec<String>,
    /// Current binding path parts
    binding_parts: Vec<String>,
}

impl Default for XfaParser {
    fn default() -> Self {
        Self::new()
    }
}

impl XfaParser {
    /// Create a new parser.
    pub fn new() -> Self {
        Self {
            form: XfaForm::default(),
            context_stack: Vec::new(),
            binding_parts: Vec::new(),
        }
    }

    // ── pdf_manipulator patch: XFA packets are read as XML events, from a reader, never held as one string ──
    /// Parse XFA data from raw bytes.
    ///
    /// The XFA data may be a single XML stream or an array of packets.
    pub fn parse(&mut self, xfa_data: &[u8]) -> Result<XfaForm> {
        self.parse_reader(xfa_data)
    }

    /// Parse XFA data read from `reader`, one XML event at a time.
    pub fn parse_reader<R: BufRead>(&mut self, reader: R) -> Result<XfaForm> {
        self.feed(reader)?;
        Ok(self.finish())
    }

    /// Feed one packet of XFA data (a complete XDP, or a `template` or
    /// `datasets` packet) to the parser. Call [`Self::finish`] once every
    /// packet is fed.
    pub fn feed<R: BufRead>(&mut self, reader: R) -> Result<()> {
        self.drive(reader, None)
    }

    /// Take the form the packets fed so far describe, with the dataset values
    /// applied to its fields.
    pub fn finish(&mut self) -> XfaForm {
        self.apply_dataset_values();
        std::mem::take(&mut self.form)
    }

    /// Parse XFA packets from a PDF XFA array.
    ///
    /// XFA in PDFs can be an array: [name1, stream1, name2, stream2, ...]
    pub fn parse_packets(&mut self, packets: &[(String, Vec<u8>)]) -> Result<XfaForm> {
        for (name, data) in packets {
            match name.as_str() {
                "template" => self.drive(data.as_slice(), Some(Section::Template))?,
                "datasets" => self.drive(data.as_slice(), Some(Section::Datasets))?,
                // Other packets (config, form, etc.) are not needed for static conversion
                _ => {},
            }
        }
        Ok(self.finish())
    }

    /// Parse the datasets packet.
    #[cfg(test)]
    fn parse_datasets(&mut self, xml: &str) -> Result<()> {
        self.drive(xml.as_bytes(), Some(Section::Datasets))
    }

    /// Reads `reader` as XML events. With `forced` the whole input is that
    /// section; without it a `template` element opens the template section and
    /// a `datasets` or `data` element the datasets section, each until its
    /// element closes. An XML error inside a section is an error; one outside
    /// any section ends the packet.
    fn drive<R: BufRead>(&mut self, reader: R, forced: Option<Section>) -> Result<()> {
        let mut reader = Reader::from_reader(reader);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut section = forced;
        let mut depth = 0usize;
        let mut template = TemplateState::default();
        let mut datasets = DatasetsState::default();
        loop {
            buf.clear();
            let event = match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(event) => event,
                Err(e) => {
                    return match section {
                        Some(Section::Template) => {
                            Err(Error::InvalidPdf(format!("XFA template parse error: {}", e)))
                        },
                        Some(Section::Datasets) => {
                            Err(Error::InvalidPdf(format!("XFA datasets parse error: {}", e)))
                        },
                        None => Ok(()),
                    };
                },
            };
            if section.is_none() {
                if let Event::Start(ref e) | Event::Empty(ref e) = event {
                    section = match e.local_name().as_ref().to_string().as_str() {
                        "template" => Some(Section::Template),
                        "datasets" | "data" => Some(Section::Datasets),
                        _ => None,
                    };
                }
                if section.is_none() {
                    continue;
                }
                depth = 0;
            }
            match section {
                Some(Section::Template) => self.template_event(&event, &mut template),
                Some(Section::Datasets) => self.datasets_event(&event, &mut datasets),
                None => {},
            }
            if forced.is_none() {
                match event {
                    Event::Start(_) => depth += 1,
                    Event::End(_) => depth = depth.saturating_sub(1),
                    _ => {},
                }
                let closed = match event {
                    Event::Empty(_) => depth == 0,
                    Event::End(_) => depth == 0,
                    _ => false,
                };
                if closed {
                    if section == Some(Section::Template) {
                        self.close_template(&mut template);
                    }
                    section = None;
                    template = TemplateState::default();
                    datasets = DatasetsState::default();
                }
            }
        }
        if section == Some(Section::Template) {
            self.close_template(&mut template);
        }
        Ok(())
    }

    /// The last page of a template, if it has fields.
    fn close_template(&mut self, state: &mut TemplateState) {
        if let Some(page) = state.current_page.take() {
            if !page.fields.is_empty() {
                self.form.pages.push(page);
            }
        }
    }

    /// One event of the template packet.
    fn template_event(&mut self, event: &Event, st: &mut TemplateState) {
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let local_name = e.local_name().as_ref().to_string();

                match local_name.as_str() {
                    "subform" => {
                        // Check if this is a page-level subform
                        let name = self.get_attribute(e, "name");
                        if let Some(name) = name {
                            self.binding_parts.push(format!("{}[0]", name));

                            // Check for page-like subforms
                            if name.contains("Page") || st.current_page.is_none() {
                                st.current_page = Some(XfaPage {
                                    name,
                                    ..Default::default()
                                });
                            }
                        }
                    },
                    "field" => {
                        let name = self.get_attribute(e, "name").unwrap_or_default();
                        let mut binding = self.binding_parts.join(".");
                        if !binding.is_empty() {
                            binding.push('.');
                        }
                        binding.push_str(&format!("{}[0]", name));

                        let mut field = XfaField::new(&name, binding);

                        // Get dimensions from attributes
                        if let Some(w) = self.get_attribute(e, "w") {
                            field.width = self.parse_dimension(&w);
                        }
                        if let Some(h) = self.get_attribute(e, "h") {
                            field.height = self.parse_dimension(&h);
                        }
                        if let Some(x) = self.get_attribute(e, "x") {
                            field.x = self.parse_dimension(&x);
                        }
                        if let Some(y) = self.get_attribute(e, "y") {
                            field.y = self.parse_dimension(&y);
                        }

                        st.current_field = Some(field);
                    },
                    "textEdit" | "numericEdit" | "dateTimeEdit" | "checkButton" | "choiceList"
                    | "button" | "signature" | "imageEdit" | "barcode" => {
                        if let Some(ref mut field) = st.current_field {
                            field.field_type = XfaFieldType::from_xfa_name(&local_name);
                        }
                    },
                    "items" => {
                        st.in_items = true;
                    },
                    _ => {},
                }

                self.context_stack.push(local_name);
            },
            Event::Text(e) => {
                let text = e.xml11_content().to_string();

                if let Some(parent) = self.context_stack.last() {
                    if st.in_items && parent == "text" {
                        st.option_text = text.clone();
                        st.option_value = text;
                    } else if let Some(ref mut field) = st.current_field {
                        match parent.as_str() {
                            "caption" | "text" => {
                                if field.caption.is_none()
                                    && !st.in_items
                                    && self.context_stack.iter().any(|s| s == "caption")
                                {
                                    field.caption = Some(text);
                                }
                            },
                            "toolTip" => {
                                field.tooltip = Some(text);
                            },
                            "value" => {
                                if field.default_value.is_none() {
                                    field.default_value = Some(text);
                                }
                            },
                            _ => {},
                        }
                    }
                }
            },
            Event::End(e) => {
                let local_name = e.local_name().as_ref().to_string();

                match local_name.as_str() {
                    "subform" => {
                        self.binding_parts.pop();
                    },
                    "field" => {
                        if let Some(field) = st.current_field.take() {
                            self.form.fields.push(field.clone());
                            if let Some(ref mut page) = st.current_page {
                                page.fields.push(field);
                            }
                        }
                    },
                    "items" => {
                        st.in_items = false;
                    },
                    "text" => {
                        if st.in_items {
                            if let Some(ref mut field) = st.current_field {
                                field.options.push(XfaOption {
                                    text: st.option_text.clone(),
                                    value: st.option_value.clone(),
                                });
                            }
                            st.option_text.clear();
                            st.option_value.clear();
                        }
                    },
                    _ => {},
                }

                self.context_stack.pop();
            },
            _ => {},
        }
    }

    /// One event of the datasets packet.
    fn datasets_event(&mut self, event: &Event, st: &mut DatasetsState) {
        match event {
            Event::Start(e) => {
                let local_name = e.local_name().as_ref().to_string();

                // Skip XFA namespace elements
                if local_name != "xfa" && local_name != "datasets" && local_name != "data" {
                    st.path_stack.push(format!("{}[0]", local_name));
                }
            },
            Event::Text(e) => {
                st.current_text = e.xml11_content().to_string();
            },
            Event::End(e) => {
                let local_name = e.local_name().as_ref().to_string();

                if local_name != "xfa" && local_name != "datasets" && local_name != "data" {
                    // Store value if we have text
                    if !st.current_text.is_empty() && !st.path_stack.is_empty() {
                        let binding = st.path_stack.join(".");
                        self.form
                            .dataset_values
                            .insert(binding, st.current_text.clone());
                    }
                    st.current_text.clear();
                    st.path_stack.pop();
                }
            },
            _ => {},
        }
    }
    // ── end pdf_manipulator patch ──

    /// Apply dataset values to fields.
    fn apply_dataset_values(&mut self) {
        for field in &mut self.form.fields {
            // Try exact binding match
            if let Some(value) = self.form.dataset_values.get(&field.binding) {
                field.value = Some(value.clone());
            } else {
                // Try partial match (XFA bindings can be complex)
                for (binding, value) in &self.form.dataset_values {
                    if binding.ends_with(&format!(".{}[0]", field.name)) {
                        field.value = Some(value.clone());
                        break;
                    }
                }
            }
        }

        // Also update page field values
        for page in &mut self.form.pages {
            for field in &mut page.fields {
                if let Some(value) = self.form.dataset_values.get(&field.binding) {
                    field.value = Some(value.clone());
                } else {
                    for (binding, value) in &self.form.dataset_values {
                        if binding.ends_with(&format!(".{}[0]", field.name)) {
                            field.value = Some(value.clone());
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Get an attribute value from an element.
    fn get_attribute<'a>(
        &self,
        e: &'a quick_xml::events::BytesStart<'a>,
        name: &str,
    ) -> Option<String> {
        for attr in e.attributes().flatten() {
            if attr.key.as_ref() == name {
                return Some(attr.value.to_string());
            }
        }
        None
    }

    /// Parse a dimension value (e.g., "72pt", "1in", "2.5cm").
    fn parse_dimension(&self, value: &str) -> Option<f32> {
        let value = value.trim();

        if let Some(stripped) = value.strip_suffix("pt") {
            stripped.parse().ok()
        } else if let Some(stripped) = value.strip_suffix("in") {
            stripped.parse::<f32>().ok().map(|v| v * 72.0)
        } else if let Some(stripped) = value.strip_suffix("cm") {
            stripped.parse::<f32>().ok().map(|v| v * 72.0 / 2.54)
        } else if let Some(stripped) = value.strip_suffix("mm") {
            stripped.parse::<f32>().ok().map(|v| v * 72.0 / 25.4)
        } else {
            // Assume points
            value.parse().ok()
        }
    }
}

/// Check if data appears to be XFA content.
pub fn is_xfa_data(data: &[u8]) -> bool {
    let s = String::from_utf8_lossy(data);
    s.contains("<template") || s.contains("<xfa:") || s.contains("<xdp:xdp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xfa_field_type_from_name() {
        assert_eq!(XfaFieldType::from_xfa_name("textEdit"), XfaFieldType::Text);
        assert_eq!(XfaFieldType::from_xfa_name("numericEdit"), XfaFieldType::Numeric);
        assert_eq!(XfaFieldType::from_xfa_name("checkButton"), XfaFieldType::Checkbox);
        assert_eq!(XfaFieldType::from_xfa_name("choiceList"), XfaFieldType::DropDown);
        assert!(matches!(XfaFieldType::from_xfa_name("unknown"), XfaFieldType::Unknown(_)));
    }

    #[test]
    fn test_xfa_field_new() {
        let field = XfaField::new("firstName", "form.page1.firstName[0]");

        assert_eq!(field.name, "firstName");
        assert_eq!(field.binding, "form.page1.firstName[0]");
        assert_eq!(field.field_type, XfaFieldType::Text);
        assert!(field.value.is_none());
    }

    #[test]
    fn test_xfa_page_default() {
        let page = XfaPage::default();

        assert!(page.name.is_empty());
        assert!(page.fields.is_empty());
        assert_eq!(page.width, 612.0);
        assert_eq!(page.height, 792.0);
    }

    #[test]
    fn test_xfa_form_get_field() {
        let mut form = XfaForm::default();
        form.fields.push(XfaField::new("test", "binding"));

        assert!(form.get_field("test").is_some());
        assert!(form.get_field("nonexistent").is_none());
    }

    #[test]
    fn test_xfa_form_get_field_by_binding() {
        let mut form = XfaForm::default();
        form.fields.push(XfaField::new("test", "root.test[0]"));

        assert!(form.get_field_by_binding("root.test[0]").is_some());
        assert!(form.get_field_by_binding("other").is_none());
    }

    #[test]
    fn test_parser_dimension_parsing() {
        let parser = XfaParser::new();

        assert_eq!(parser.parse_dimension("72pt"), Some(72.0));
        assert_eq!(parser.parse_dimension("1in"), Some(72.0));
        assert_eq!(parser.parse_dimension("2.54cm"), Some(72.0));
        assert_eq!(parser.parse_dimension("25.4mm"), Some(72.0));
        assert_eq!(parser.parse_dimension("100"), Some(100.0));
    }

    #[test]
    fn test_is_xfa_data() {
        assert!(is_xfa_data(b"<template xmlns=\"http://www.xfa.org/schema/xfa-template/2.8/\">"));
        assert!(is_xfa_data(b"<xfa:datasets>"));
        assert!(is_xfa_data(b"<xdp:xdp xmlns:xdp=\"http://ns.adobe.com/xdp/\">"));
        assert!(!is_xfa_data(b"<html><body>Not XFA</body></html>"));
        assert!(!is_xfa_data(b"Random data"));
    }

    #[test]
    fn test_parse_simple_template() {
        let template = r#"<?xml version="1.0"?>
<template xmlns="http://www.xfa.org/schema/xfa-template/3.0/">
    <subform name="form1">
        <field name="firstName" w="200pt" h="20pt">
            <ui>
                <textEdit/>
            </ui>
            <caption>
                <value><text>First Name</text></value>
            </caption>
        </field>
        <field name="lastName">
            <ui>
                <textEdit/>
            </ui>
        </field>
    </subform>
</template>"#;

        let mut parser = XfaParser::new();
        let form = parser.parse(template.as_bytes()).unwrap();

        assert_eq!(form.fields.len(), 2);
        assert_eq!(form.fields[0].name, "firstName");
        assert_eq!(form.fields[0].field_type, XfaFieldType::Text);
        assert_eq!(form.fields[0].width, Some(200.0));
        assert_eq!(form.fields[0].height, Some(20.0));
        assert_eq!(form.fields[1].name, "lastName");
    }

    #[test]
    fn test_parse_datasets() {
        let datasets = r#"<?xml version="1.0"?>
<xfa:datasets xmlns:xfa="http://www.xfa.org/schema/xfa-data/1.0/">
    <xfa:data>
        <form1>
            <firstName>John</firstName>
            <lastName>Doe</lastName>
        </form1>
    </xfa:data>
</xfa:datasets>"#;

        let mut parser = XfaParser::new();

        // First add some fields
        parser
            .form
            .fields
            .push(XfaField::new("firstName", "form1[0].firstName[0]"));
        parser
            .form
            .fields
            .push(XfaField::new("lastName", "form1[0].lastName[0]"));

        // Parse datasets and apply values
        parser.parse_datasets(datasets).unwrap();
        parser.apply_dataset_values();

        assert_eq!(parser.form.fields[0].value, Some("John".to_string()));
        assert_eq!(parser.form.fields[1].value, Some("Doe".to_string()));
    }

    #[test]
    fn test_parse_field_with_options() {
        let template = r#"<?xml version="1.0"?>
<template>
    <subform name="form1">
        <field name="country">
            <ui>
                <choiceList/>
            </ui>
            <items>
                <text>United States</text>
                <text>Canada</text>
                <text>Mexico</text>
            </items>
        </field>
    </subform>
</template>"#;

        let mut parser = XfaParser::new();
        let form = parser.parse(template.as_bytes()).unwrap();

        assert_eq!(form.fields.len(), 1);
        assert_eq!(form.fields[0].name, "country");
        assert_eq!(form.fields[0].field_type, XfaFieldType::DropDown);
        assert_eq!(form.fields[0].options.len(), 3);
        assert_eq!(form.fields[0].options[0].text, "United States");
    }

    #[test]
    fn test_parse_checkbox_field() {
        let template = r#"<?xml version="1.0"?>
<template>
    <subform name="form1">
        <field name="agree">
            <ui>
                <checkButton/>
            </ui>
            <caption>
                <value><text>I agree to the terms</text></value>
            </caption>
        </field>
    </subform>
</template>"#;

        let mut parser = XfaParser::new();
        let form = parser.parse(template.as_bytes()).unwrap();

        assert_eq!(form.fields.len(), 1);
        assert_eq!(form.fields[0].name, "agree");
        assert_eq!(form.fields[0].field_type, XfaFieldType::Checkbox);
    }

    #[test]
    fn test_xfa_option() {
        let option = XfaOption {
            text: "Display Text".to_string(),
            value: "actual_value".to_string(),
        };

        assert_eq!(option.text, "Display Text");
        assert_eq!(option.value, "actual_value");
    }
}
