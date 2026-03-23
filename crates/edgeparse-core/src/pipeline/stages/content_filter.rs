//! Stage 2: Content Filtering
//!
//! Removes hidden text, off-page content, tiny text, and invisible OCG content
//! based on the FilterConfig settings.

use crate::api::filter::FilterConfig;
use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;

/// Minimum font size threshold — text below this is considered "tiny".
const MIN_FONT_SIZE: f64 = 1.0;

/// Minimum contrast ratio — text below this is considered hidden.
const MIN_CONTRAST_RATIO: f64 = 1.5;

/// Filter content elements according to content safety rules.
///
/// Returns a filtered vector with unwanted elements removed.
pub fn filter_content(
    elements: Vec<ContentElement>,
    filter_config: &FilterConfig,
    page_bbox: &BoundingBox,
) -> Vec<ContentElement> {
    elements
        .into_iter()
        .filter(|e| should_keep(e, filter_config, page_bbox))
        .collect()
}

/// Decide whether to keep a content element.
fn should_keep(element: &ContentElement, filter: &FilterConfig, page_bbox: &BoundingBox) -> bool {
    match element {
        ContentElement::TextChunk(tc) => {
            // Filter hidden text (low contrast)
            if filter.filter_hidden_text && tc.contrast_ratio < MIN_CONTRAST_RATIO {
                return false;
            }

            // Filter tiny text
            if filter.filter_tiny_text && tc.font_size < MIN_FONT_SIZE {
                return false;
            }

            // Filter off-page content
            if filter.filter_out_of_page && is_off_page(&tc.bbox, page_bbox) {
                return false;
            }

            // Filter hidden OCG content
            if filter.filter_hidden_ocg && !tc.ocg_visible {
                return false;
            }

            true
        }
        // For now, keep all non-text elements
        _ => true,
    }
}

/// Check if a bounding box is outside the page bounds.
fn is_off_page(element_bbox: &BoundingBox, page_bbox: &BoundingBox) -> bool {
    // Element is off-page if it doesn't overlap with the page at all
    element_bbox.right_x < page_bbox.left_x
        || element_bbox.left_x > page_bbox.right_x
        || element_bbox.top_y < page_bbox.bottom_y
        || element_bbox.bottom_y > page_bbox.top_y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_chunk(value: &str, font_size: f64, contrast: f64, x: f64, y: f64) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: value.to_string(),
            bbox: BoundingBox::new(Some(1), x, y, x + 100.0, y + font_size),
            font_name: "Helvetica".to_string(),
            font_size,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: contrast,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        })
    }

    fn make_hidden_ocg_chunk(value: &str) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: value.to_string(),
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 12.0),
            font_name: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: false,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        })
    }

    #[test]
    fn test_filter_keeps_normal_text() {
        let elements = vec![make_chunk("Hello", 12.0, 21.0, 100.0, 700.0)];
        let page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);
        let filter = FilterConfig::default();
        let result = filter_content(elements, &filter, &page);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_removes_hidden_text() {
        let elements = vec![
            make_chunk("Visible", 12.0, 21.0, 100.0, 700.0),
            make_chunk("Hidden", 12.0, 1.0, 100.0, 680.0),
        ];
        let page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);
        let filter = FilterConfig::default();
        let result = filter_content(elements, &filter, &page);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_removes_tiny_text() {
        let elements = vec![
            make_chunk("Normal", 12.0, 21.0, 100.0, 700.0),
            make_chunk("Tiny", 0.5, 21.0, 100.0, 680.0),
        ];
        let page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);
        let filter = FilterConfig::default();
        let result = filter_content(elements, &filter, &page);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_removes_off_page() {
        let elements = vec![
            make_chunk("On page", 12.0, 21.0, 100.0, 700.0),
            make_chunk("Off page", 12.0, 21.0, -200.0, 700.0),
        ];
        let page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);
        let filter = FilterConfig::default();
        let result = filter_content(elements, &filter, &page);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_removes_hidden_ocg() {
        let elements = vec![
            make_chunk("Visible", 12.0, 21.0, 100.0, 700.0),
            make_hidden_ocg_chunk("Hidden OCG"),
        ];
        let page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);
        let filter = FilterConfig::default();
        let result = filter_content(elements, &filter, &page);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_filter_disabled_keeps_all() {
        let elements = vec![
            make_chunk("Normal", 12.0, 21.0, 100.0, 700.0),
            make_chunk("Hidden", 12.0, 1.0, 100.0, 680.0),
            make_chunk("Tiny", 0.5, 21.0, 100.0, 660.0),
        ];
        let page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);
        let filter = FilterConfig::all_off();
        let result = filter_content(elements, &filter, &page);
        assert_eq!(result.len(), 3);
    }
}
