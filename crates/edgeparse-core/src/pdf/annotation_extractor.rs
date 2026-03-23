//! PDF annotation extraction.
//!
//! Reads annotations (highlights, comments, stamps, etc.)
//! from each page's /Annots array.

use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};

/// Annotation type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnnotationType {
    /// Text (sticky note)
    Text,
    /// Highlight
    Highlight,
    /// Underline
    Underline,
    /// Strikeout
    StrikeOut,
    /// Free text annotation
    FreeText,
    /// Link (see hyperlink extractor)
    Link,
    /// Stamp
    Stamp,
    /// Ink (freehand drawing)
    Ink,
    /// File attachment
    FileAttachment,
    /// Pop-up (associated with another annotation)
    Popup,
    /// Other/unknown
    Other(String),
}

/// A single annotation extracted from the PDF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfAnnotation {
    /// Annotation type
    pub annotation_type: AnnotationType,
    /// Content/text of the annotation
    pub contents: Option<String>,
    /// Author/title
    pub author: Option<String>,
    /// Page number (1-based)
    pub page_number: u32,
    /// Bounding rectangle [x1, y1, x2, y2]
    pub rect: Option<[f64; 4]>,
    /// Subject line
    pub subject: Option<String>,
    /// Creation date string
    pub creation_date: Option<String>,
    /// Modification date string
    pub modification_date: Option<String>,
}

/// Extract annotations from all pages of a PDF document.
pub fn extract_annotations(doc: &Document) -> Vec<PdfAnnotation> {
    let mut annotations = Vec::new();

    let pages = doc.get_pages();
    let mut page_ids: Vec<(u32, lopdf::ObjectId)> = pages.into_iter().collect();
    page_ids.sort_by_key(|(num, _)| *num);

    for (page_num, page_id) in page_ids {
        let page_dict = match doc.get_object(page_id).and_then(|o| o.as_dict()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let annots_obj = match page_dict.get(b"Annots") {
            Ok(obj) => resolve(doc, obj),
            Err(_) => continue,
        };

        let annots_array = match annots_obj.as_array() {
            Ok(a) => a,
            Err(_) => continue,
        };

        for annot_ref in annots_array {
            let annot_obj = resolve(doc, annot_ref);
            if let Ok(dict) = annot_obj.as_dict() {
                if let Some(annot) = parse_annotation(dict, page_num) {
                    annotations.push(annot);
                }
            }
        }
    }

    annotations
}

/// Parse a single annotation dictionary.
fn parse_annotation(dict: &lopdf::Dictionary, page_number: u32) -> Option<PdfAnnotation> {
    let annotation_type = match dict.get(b"Subtype") {
        Ok(Object::Name(name)) => match name.as_slice() {
            b"Text" => AnnotationType::Text,
            b"Highlight" => AnnotationType::Highlight,
            b"Underline" => AnnotationType::Underline,
            b"StrikeOut" => AnnotationType::StrikeOut,
            b"FreeText" => AnnotationType::FreeText,
            b"Link" => AnnotationType::Link,
            b"Stamp" => AnnotationType::Stamp,
            b"Ink" => AnnotationType::Ink,
            b"FileAttachment" => AnnotationType::FileAttachment,
            b"Popup" => AnnotationType::Popup,
            other => AnnotationType::Other(String::from_utf8_lossy(other).to_string()),
        },
        _ => return None,
    };

    let contents = get_string(dict, b"Contents");
    let author = get_string(dict, b"T");
    let subject = get_string(dict, b"Subj");
    let creation_date = get_string(dict, b"CreationDate");
    let modification_date = get_string(dict, b"M");
    let rect = get_rect(dict);

    Some(PdfAnnotation {
        annotation_type,
        contents,
        author,
        page_number,
        rect,
        subject,
        creation_date,
        modification_date,
    })
}

/// Get a string value from a dictionary.
fn get_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key).ok().and_then(|obj| match obj {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
    })
}

/// Get a rectangle [x1, y1, x2, y2] from the /Rect entry.
fn get_rect(dict: &lopdf::Dictionary) -> Option<[f64; 4]> {
    let rect_obj = dict.get(b"Rect").ok()?;
    let arr = rect_obj.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let mut result = [0.0f64; 4];
    for (i, obj) in arr.iter().enumerate().take(4) {
        result[i] = match obj {
            Object::Real(f) => *f,
            Object::Integer(i) => *i as f64,
            _ => return None,
        };
    }
    Some(result)
}

/// Resolve an indirect object reference.
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
    fn test_parse_annotation_text() {
        let mut dict = lopdf::Dictionary::new();
        dict.set("Subtype", Object::Name(b"Text".to_vec()));
        dict.set(
            "Contents",
            Object::String(b"A comment".to_vec(), lopdf::StringFormat::Literal),
        );
        let annot = parse_annotation(&dict, 1).unwrap();
        assert_eq!(annot.annotation_type, AnnotationType::Text);
        assert_eq!(annot.contents, Some("A comment".to_string()));
        assert_eq!(annot.page_number, 1);
    }

    #[test]
    fn test_parse_annotation_highlight() {
        let mut dict = lopdf::Dictionary::new();
        dict.set("Subtype", Object::Name(b"Highlight".to_vec()));
        let annot = parse_annotation(&dict, 3).unwrap();
        assert_eq!(annot.annotation_type, AnnotationType::Highlight);
        assert_eq!(annot.page_number, 3);
    }

    #[test]
    fn test_parse_annotation_no_subtype() {
        let dict = lopdf::Dictionary::new();
        assert!(parse_annotation(&dict, 1).is_none());
    }

    #[test]
    fn test_get_rect() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(
            "Rect",
            Object::Array(vec![
                Object::Real(10.0),
                Object::Real(20.0),
                Object::Real(100.0),
                Object::Real(50.0),
            ]),
        );
        let rect = get_rect(&dict).unwrap();
        assert_eq!(rect, [10.0, 20.0, 100.0, 50.0]);
    }
}
