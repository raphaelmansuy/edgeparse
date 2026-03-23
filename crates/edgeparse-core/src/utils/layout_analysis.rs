//! Layout analysis utilities — page geometry classification, margin detection,
//! and content density analysis for multi-column and complex layouts.

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;

/// Classification of a page layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutType {
    /// Single column of text.
    SingleColumn,
    /// Two columns side by side.
    TwoColumn,
    /// Three or more columns.
    MultiColumn,
    /// Primarily a table-based layout.
    Tabular,
    /// Mixed layout (images + text, irregular).
    Mixed,
    /// Empty page or only decorations.
    Empty,
}

/// Detected page margins.
#[derive(Debug, Clone, PartialEq)]
pub struct PageMargins {
    /// Top margin distance from the page edge.
    pub top: f64,
    /// Bottom margin distance from the page edge.
    pub bottom: f64,
    /// Left margin distance from the page edge.
    pub left: f64,
    /// Right margin distance from the page edge.
    pub right: f64,
}

/// Content density info for a page.
#[derive(Debug, Clone)]
pub struct ContentDensity {
    /// Total area covered by content elements.
    pub content_area: f64,
    /// Total page area.
    pub page_area: f64,
    /// Ratio of content to page area (0.0 to 1.0).
    pub density: f64,
    /// Number of content elements.
    pub element_count: usize,
}

/// Classify the layout type of a page based on its content elements.
pub fn classify_layout(elements: &[ContentElement], page_width: f64) -> LayoutType {
    if elements.is_empty() {
        return LayoutType::Empty;
    }

    // Count element types
    let table_count = elements
        .iter()
        .filter(|e| matches!(e, ContentElement::Table(_)))
        .count();

    if table_count > 0 && table_count * 3 >= elements.len() {
        return LayoutType::Tabular;
    }

    // Detect columns by analyzing X positions
    let x_ranges: Vec<(f64, f64)> = elements
        .iter()
        .map(|e| {
            let b = e.bbox();
            (b.left_x, b.right_x)
        })
        .collect();

    let columns = detect_column_count(&x_ranges, page_width);

    match columns {
        0 | 1 => {
            // Check if mixed (has images interspersed with text)
            let has_images = elements
                .iter()
                .any(|e| matches!(e, ContentElement::Image(_) | ContentElement::Figure(_)));
            let has_text = elements.iter().any(|e| {
                matches!(
                    e,
                    ContentElement::Paragraph(_)
                        | ContentElement::TextBlock(_)
                        | ContentElement::Heading(_)
                )
            });
            if has_images && has_text {
                LayoutType::Mixed
            } else {
                LayoutType::SingleColumn
            }
        }
        2 => LayoutType::TwoColumn,
        _ => LayoutType::MultiColumn,
    }
}

/// Detect page margins from content bounding boxes.
pub fn detect_margins(
    elements: &[ContentElement],
    page_width: f64,
    page_height: f64,
) -> PageMargins {
    if elements.is_empty() {
        return PageMargins {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        };
    }

    let content_bbox = content_bounding_box(elements);

    PageMargins {
        left: content_bbox.left_x.max(0.0),
        right: (page_width - content_bbox.right_x).max(0.0),
        // PDF coordinates: bottom_y is lower, top_y is upper
        bottom: content_bbox.bottom_y.max(0.0),
        top: (page_height - content_bbox.top_y).max(0.0),
    }
}

/// Compute content density for a page.
pub fn compute_density(
    elements: &[ContentElement],
    page_width: f64,
    page_height: f64,
) -> ContentDensity {
    let page_area = page_width * page_height;
    if page_area <= 0.0 {
        return ContentDensity {
            content_area: 0.0,
            page_area: 0.0,
            density: 0.0,
            element_count: elements.len(),
        };
    }

    let content_area: f64 = elements
        .iter()
        .map(|e| {
            let b = e.bbox();
            b.width() * b.height()
        })
        .sum();

    ContentDensity {
        content_area,
        page_area,
        density: (content_area / page_area).min(1.0),
        element_count: elements.len(),
    }
}

/// Compute the bounding box enclosing all elements.
fn content_bounding_box(elements: &[ContentElement]) -> BoundingBox {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for e in elements {
        let b = e.bbox();
        min_x = min_x.min(b.left_x);
        min_y = min_y.min(b.bottom_y);
        max_x = max_x.max(b.right_x);
        max_y = max_y.max(b.top_y);
    }

    BoundingBox::new(None, min_x, min_y, max_x, max_y)
}

/// Detect the number of columns from X-ranges of elements.
fn detect_column_count(x_ranges: &[(f64, f64)], page_width: f64) -> usize {
    if x_ranges.is_empty() || page_width <= 0.0 {
        return 0;
    }

    // Divide page into bins and count elements per bin
    let bin_count = 60;
    let bin_width = page_width / bin_count as f64;
    let mut bins = vec![0u32; bin_count];

    for (left, right) in x_ranges {
        let start_bin = ((*left / bin_width) as usize).min(bin_count - 1);
        let end_bin = ((*right / bin_width) as usize).min(bin_count - 1);
        for bin in &mut bins[start_bin..=end_bin] {
            *bin += 1;
        }
    }

    // Find the first and last bins that have content — ignore margin areas
    let threshold = (x_ranges.len() as f64 * 0.1) as u32;
    let first_content = bins.iter().position(|&c| c > threshold);
    let last_content = bins.iter().rposition(|&c| c > threshold);

    let (first, last) = match (first_content, last_content) {
        (Some(f), Some(l)) if f < l => (f, l),
        _ => return 1, // all content in same bin or no content
    };

    // Count internal gaps (empty bins between first and last content bins)
    let mut gap_count = 0;
    let mut in_gap = false;
    for &count in &bins[first..=last] {
        if count <= threshold {
            if !in_gap {
                gap_count += 1;
                in_gap = true;
            }
        } else {
            in_gap = false;
        }
    }

    // Number of columns = number of internal gaps + 1
    gap_count + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text_at(x: f64, y: f64, w: f64, h: f64) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: "text".to_string(),
            bbox: BoundingBox::new(None, x, y, x + w, y + h),
            font_name: "F".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000".to_string(),
            contrast_ratio: 21.0,
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

    #[test]
    fn test_classify_empty() {
        assert_eq!(classify_layout(&[], 612.0), LayoutType::Empty);
    }

    #[test]
    fn test_classify_single_column() {
        let elements = vec![
            make_text_at(72.0, 100.0, 468.0, 12.0),
            make_text_at(72.0, 120.0, 468.0, 12.0),
            make_text_at(72.0, 140.0, 468.0, 12.0),
        ];
        let layout = classify_layout(&elements, 612.0);
        assert_eq!(layout, LayoutType::SingleColumn);
    }

    #[test]
    fn test_detect_margins() {
        let elements = vec![make_text_at(72.0, 72.0, 468.0, 648.0)];
        let margins = detect_margins(&elements, 612.0, 792.0);
        assert!((margins.left - 72.0).abs() < 0.1);
        assert!((margins.right - 72.0).abs() < 0.1);
        assert!((margins.bottom - 72.0).abs() < 0.1);
        assert!((margins.top - 72.0).abs() < 0.1);
    }

    #[test]
    fn test_compute_density() {
        let elements = vec![make_text_at(0.0, 0.0, 100.0, 50.0)];
        let density = compute_density(&elements, 200.0, 100.0);
        assert!((density.density - 0.25).abs() < 0.01); // 5000 / 20000
        assert_eq!(density.element_count, 1);
    }

    #[test]
    fn test_column_detection() {
        // Two columns: left (72-290) and right (322-540)
        let elements = vec![
            make_text_at(72.0, 100.0, 218.0, 12.0),
            make_text_at(72.0, 120.0, 218.0, 12.0),
            make_text_at(322.0, 100.0, 218.0, 12.0),
            make_text_at(322.0, 120.0, 218.0, 12.0),
        ];
        let layout = classify_layout(&elements, 612.0);
        assert_eq!(layout, LayoutType::TwoColumn);
    }
}
