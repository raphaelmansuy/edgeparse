//! Hyperlink extraction from PDF link annotations.
//!
//! Identifies /Link annotations with /URI actions and maps them
//! to bounding box regions on each page.

use lopdf::{Document, Object};
use serde::{Deserialize, Serialize};

/// A hyperlink extracted from a PDF page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfHyperlink {
    /// The target URI
    pub uri: String,
    /// Page number (1-based)
    pub page_number: u32,
    /// Bounding rectangle [left, bottom, right, top]
    pub rect: [f64; 4],
}

/// Extract all hyperlinks from the PDF document.
pub fn extract_hyperlinks(doc: &Document) -> Vec<PdfHyperlink> {
    let mut links = Vec::new();

    let pages = doc.get_pages();
    let mut page_list: Vec<(u32, lopdf::ObjectId)> = pages.into_iter().collect();
    page_list.sort_by_key(|(num, _)| *num);

    for (page_num, page_id) in page_list {
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
                if let Some(link) = parse_link_annotation(doc, dict, page_num) {
                    links.push(link);
                }
            }
        }
    }

    links
}

/// Parse a link annotation to extract URI and rect.
fn parse_link_annotation(
    doc: &Document,
    dict: &lopdf::Dictionary,
    page_number: u32,
) -> Option<PdfHyperlink> {
    // Must be a /Link annotation
    match dict.get(b"Subtype") {
        Ok(Object::Name(name)) if name == b"Link" => {}
        _ => return None,
    }

    // Get URI from /A (action) dictionary
    let uri = extract_uri(doc, dict)?;

    // Get bounding rect
    let rect = extract_rect(dict)?;

    Some(PdfHyperlink {
        uri,
        page_number,
        rect,
    })
}

/// Extract URI from the /A action dictionary.
fn extract_uri(doc: &Document, dict: &lopdf::Dictionary) -> Option<String> {
    let action_obj = dict.get(b"A").ok()?;
    let action = resolve(doc, action_obj);
    let action_dict = action.as_dict().ok()?;

    // Check action type is /URI
    match action_dict.get(b"S") {
        Ok(Object::Name(name)) if name == b"URI" => {}
        _ => return None,
    }

    // Get the URI string
    match action_dict.get(b"URI") {
        Ok(Object::String(bytes, _)) => Some(String::from_utf8_lossy(bytes).to_string()),
        _ => None,
    }
}

/// Extract bounding rectangle from /Rect.
fn extract_rect(dict: &lopdf::Dictionary) -> Option<[f64; 4]> {
    let rect_obj = dict.get(b"Rect").ok()?;
    let arr = rect_obj.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let mut result = [0.0f64; 4];
    for (i, obj) in arr.iter().enumerate().take(4) {
        result[i] = match obj {
            Object::Real(f) => *f,
            Object::Integer(n) => *n as f64,
            _ => return None,
        };
    }
    Some(result)
}

/// Resolve an indirect reference.
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
    fn test_empty_document_no_links() {
        let doc = Document::new();
        let links = extract_hyperlinks(&doc);
        assert!(links.is_empty());
    }

    #[test]
    fn test_hyperlink_struct() {
        let link = PdfHyperlink {
            uri: "https://example.com".to_string(),
            page_number: 1,
            rect: [72.0, 700.0, 200.0, 714.0],
        };
        assert_eq!(link.uri, "https://example.com");
        assert_eq!(link.page_number, 1);
        assert_eq!(link.rect[0], 72.0);
    }

    #[test]
    fn test_extract_rect_from_dict() {
        let mut dict = lopdf::Dictionary::new();
        dict.set(
            "Rect",
            Object::Array(vec![
                Object::Real(10.0),
                Object::Integer(20),
                Object::Real(100.0),
                Object::Real(50.0),
            ]),
        );
        let rect = extract_rect(&dict).unwrap();
        assert_eq!(rect[0], 10.0);
        assert_eq!(rect[1], 20.0);
        assert_eq!(rect[2], 100.0);
        assert_eq!(rect[3], 50.0);
    }

    #[test]
    fn test_extract_rect_missing() {
        let dict = lopdf::Dictionary::new();
        assert!(extract_rect(&dict).is_none());
    }
}
