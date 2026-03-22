//! AcroForm field extraction from PDF documents.
//!
//! Reads form fields (text inputs, checkboxes, radio buttons, dropdowns)
//! and their current values from the PDF's AcroForm dictionary.

use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};

/// Type of form field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFieldType {
    /// Text input field
    Text,
    /// Checkbox
    Checkbox,
    /// Radio button
    RadioButton,
    /// Dropdown / combo box
    Choice,
    /// Push button
    Button,
    /// Signature field
    Signature,
    /// Unknown field type
    Unknown,
}

/// A single form field extracted from the PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    /// Field name (fully qualified)
    pub name: String,
    /// Field type
    pub field_type: FormFieldType,
    /// Current value (if any)
    pub value: Option<String>,
    /// Default value
    pub default_value: Option<String>,
    /// Whether the field is read-only
    pub read_only: bool,
    /// Whether the field is required
    pub required: bool,
    /// Page number (1-based) if determinable
    pub page_number: Option<u32>,
}

/// Extract all AcroForm fields from a PDF document.
pub fn extract_form_fields(doc: &Document) -> Vec<FormField> {
    let mut fields = Vec::new();

    // Get the AcroForm dictionary from the document catalog
    let catalog = match doc.catalog() {
        Ok(c) => c,
        Err(_) => return fields,
    };

    let acroform = match catalog.get(b"AcroForm") {
        Ok(obj) => resolve(doc, obj),
        Err(_) => return fields,
    };

    let acroform_dict = match acroform.as_dict() {
        Ok(d) => d,
        Err(_) => return fields,
    };

    // Get the Fields array
    let fields_array = match acroform_dict.get(b"Fields") {
        Ok(obj) => resolve(doc, obj),
        Err(_) => return fields,
    };

    let fields_arr = match fields_array.as_array() {
        Ok(a) => a,
        Err(_) => return fields,
    };

    // Process each field reference
    for field_ref in fields_arr {
        let field_obj = resolve(doc, field_ref);
        if let Ok(dict) = field_obj.as_dict() {
            extract_field(doc, dict, "", &mut fields);
        }
    }

    fields
}

/// Recursively extract a field and its children (for hierarchical fields).
fn extract_field(
    doc: &Document,
    dict: &lopdf::Dictionary,
    parent_name: &str,
    fields: &mut Vec<FormField>,
) {
    // Get partial name
    let partial_name = extract_string(dict, b"T").unwrap_or_default();
    let full_name = if parent_name.is_empty() {
        partial_name.clone()
    } else if partial_name.is_empty() {
        parent_name.to_string()
    } else {
        format!("{parent_name}.{partial_name}")
    };

    // Check for Kids (child fields)
    if let Ok(kids_obj) = dict.get(b"Kids") {
        let kids = resolve(doc, kids_obj);
        if let Ok(kids_arr) = kids.as_array() {
            for kid_ref in kids_arr {
                let kid_obj = resolve(doc, kid_ref);
                if let Ok(kid_dict) = kid_obj.as_dict() {
                    extract_field(doc, kid_dict, &full_name, fields);
                }
            }
            // If the field has kids and no own widget, don't add it as a leaf
            if dict.get(b"Subtype").is_err() {
                return;
            }
        }
    }

    // Determine field type
    let field_type = determine_field_type(dict);

    // Get value
    let value = extract_string(dict, b"V");
    let default_value = extract_string(dict, b"DV");

    // Get flags
    let ff = dict
        .get(b"Ff")
        .ok()
        .and_then(|o| {
            if let Object::Integer(i) = resolve(doc, o) {
                Some(*i as u32)
            } else {
                None
            }
        })
        .unwrap_or(0);

    let read_only = (ff & 1) != 0;
    let required = (ff & 2) != 0;

    fields.push(FormField {
        name: full_name,
        field_type,
        value,
        default_value,
        read_only,
        required,
        page_number: None,
    });
}

/// Determine the field type from the /FT entry.
fn determine_field_type(dict: &lopdf::Dictionary) -> FormFieldType {
    match dict.get(b"FT") {
        Ok(obj) => {
            if let Object::Name(name) = obj {
                match name.as_slice() {
                    b"Tx" => FormFieldType::Text,
                    b"Btn" => {
                        // Distinguish checkbox vs radio vs push button
                        let ff = dict
                            .get(b"Ff")
                            .ok()
                            .and_then(|o| {
                                if let Object::Integer(i) = o {
                                    Some(*i as u32)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        if (ff & 0x10000) != 0 {
                            FormFieldType::Button // push button
                        } else if (ff & 0x8000) != 0 {
                            FormFieldType::RadioButton
                        } else {
                            FormFieldType::Checkbox
                        }
                    }
                    b"Ch" => FormFieldType::Choice,
                    b"Sig" => FormFieldType::Signature,
                    _ => FormFieldType::Unknown,
                }
            } else {
                FormFieldType::Unknown
            }
        }
        Err(_) => FormFieldType::Unknown,
    }
}

/// Extract a string value from a dictionary field.
fn extract_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key).ok().and_then(|obj| match obj {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
    })
}

/// Resolve an object reference, following indirect references.
fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_field_type_default() {
        let dict = lopdf::Dictionary::new();
        assert_eq!(determine_field_type(&dict), FormFieldType::Unknown);
    }

    #[test]
    fn test_form_field_type_text() {
        let mut dict = lopdf::Dictionary::new();
        dict.set("FT", Object::Name(b"Tx".to_vec()));
        assert_eq!(determine_field_type(&dict), FormFieldType::Text);
    }

    #[test]
    fn test_form_field_type_checkbox() {
        let mut dict = lopdf::Dictionary::new();
        dict.set("FT", Object::Name(b"Btn".to_vec()));
        assert_eq!(determine_field_type(&dict), FormFieldType::Checkbox);
    }

    #[test]
    fn test_form_field_type_radio() {
        let mut dict = lopdf::Dictionary::new();
        dict.set("FT", Object::Name(b"Btn".to_vec()));
        dict.set("Ff", Object::Integer(0x8000));
        assert_eq!(determine_field_type(&dict), FormFieldType::RadioButton);
    }

    #[test]
    fn test_extract_string_value() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(
            "V",
            Object::String(b"Hello".to_vec(), lopdf::StringFormat::Literal),
        );
        assert_eq!(extract_string(&dict, b"V"), Some("Hello".to_string()));
    }

    #[test]
    fn test_extract_string_missing() {
        let dict = lopdf::Dictionary::new();
        assert_eq!(extract_string(&dict, b"V"), None);
    }
}
