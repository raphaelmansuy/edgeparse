//! Page geometry extraction — MediaBox, CropBox, Rotation.

use lopdf::{Document, Object};

use crate::models::bbox::BoundingBox;

/// Physical page information extracted from the PDF.
#[derive(Debug, Clone)]
pub struct PageInfo {
    /// Zero-based page index.
    pub index: usize,
    /// One-based page number.
    pub page_number: u32,
    /// MediaBox — the full physical page size.
    pub media_box: BoundingBox,
    /// CropBox — the visible area (defaults to MediaBox if absent).
    pub crop_box: BoundingBox,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotation: i64,
    /// Page width in points.
    pub width: f64,
    /// Page height in points.
    pub height: f64,
}

/// Extract page info for all pages in the document.
pub fn extract_page_info(doc: &Document) -> Vec<PageInfo> {
    let pages = doc.get_pages();
    let mut infos = Vec::with_capacity(pages.len());

    let mut sorted_pages: Vec<_> = pages.into_iter().collect();
    sorted_pages.sort_by_key(|&(num, _)| num);

    for (idx, (page_num, page_id)) in sorted_pages.into_iter().enumerate() {
        let page_dict = match doc.get_object(page_id).and_then(|o| o.as_dict().cloned()) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let media_box = extract_rect(doc, &page_dict, b"MediaBox")
            .unwrap_or_else(|| BoundingBox::new(None, 0.0, 0.0, 612.0, 792.0)); // US Letter default

        let crop_box =
            extract_rect(doc, &page_dict, b"CropBox").unwrap_or_else(|| media_box.clone());

        let rotation = page_dict
            .get(b"Rotate")
            .ok()
            .and_then(|o| resolve_integer(doc, o))
            .unwrap_or(0);

        let width = media_box.width();
        let height = media_box.height();

        infos.push(PageInfo {
            index: idx,
            page_number: page_num,
            media_box,
            crop_box,
            rotation,
            width,
            height,
        });
    }

    infos
}

/// Extract a rectangle array [llx, lly, urx, ury] from a page dictionary,
/// walking up the page tree if not found on the page itself.
fn extract_rect(doc: &Document, dict: &lopdf::Dictionary, key: &[u8]) -> Option<BoundingBox> {
    if let Ok(obj) = dict.get(key) {
        if let Some(bbox) = parse_rect_array(doc, obj) {
            return Some(bbox);
        }
    }
    // Try parent
    if let Ok(parent_ref) = dict.get(b"Parent") {
        if let Ok((_, parent_obj)) = doc.dereference(parent_ref) {
            if let Ok(parent_dict) = parent_obj.as_dict() {
                return extract_rect(doc, parent_dict, key);
            }
        }
    }
    None
}

fn parse_rect_array(doc: &Document, obj: &Object) -> Option<BoundingBox> {
    let arr = match obj {
        Object::Array(a) => a.clone(),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_array().ok().cloned())?,
        _ => return None,
    };

    if arr.len() < 4 {
        return None;
    }

    let vals: Vec<f64> = arr.iter().filter_map(|o| resolve_number(doc, o)).collect();

    if vals.len() < 4 {
        return None;
    }

    Some(BoundingBox::new(None, vals[0], vals[1], vals[2], vals[3]))
}

fn resolve_number(doc: &Document, obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(f) => Some(*f),
        Object::Integer(i) => Some(*i as f64),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| resolve_number(doc, o)),
        _ => None,
    }
}

fn resolve_integer(doc: &Document, obj: &Object) -> Option<i64> {
    match obj {
        Object::Integer(i) => Some(*i),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| resolve_integer(doc, o)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document() {
        let doc = Document::new();
        let infos = extract_page_info(&doc);
        assert!(infos.is_empty());
    }

    #[test]
    fn test_page_info_defaults() {
        let info = PageInfo {
            index: 0,
            page_number: 1,
            media_box: BoundingBox::new(None, 0.0, 0.0, 612.0, 792.0),
            crop_box: BoundingBox::new(None, 0.0, 0.0, 612.0, 792.0),
            rotation: 0,
            width: 612.0,
            height: 792.0,
        };
        assert_eq!(info.width, 612.0);
        assert_eq!(info.height, 792.0);
        assert_eq!(info.rotation, 0);
    }

    #[test]
    fn test_rotated_page() {
        let info = PageInfo {
            index: 0,
            page_number: 1,
            media_box: BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0),
            crop_box: BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0),
            rotation: 90,
            width: 595.0,
            height: 842.0,
        };
        assert_eq!(info.rotation, 90);
    }
}
