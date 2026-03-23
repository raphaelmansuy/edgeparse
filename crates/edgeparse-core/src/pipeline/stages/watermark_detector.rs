//! Watermark Detection
//!
//! Identifies text that appears at the same position across many pages
//! (e.g., "DRAFT", "CONFIDENTIAL") and marks it as a watermark.

use std::collections::HashMap;

use crate::models::content::ContentElement;

/// Minimum fraction of pages a text must appear on to be classified as a watermark.
const WATERMARK_PAGE_FRACTION: f64 = 0.75;

/// Minimum number of pages required before watermark detection kicks in.
const MIN_PAGES_FOR_DETECTION: usize = 3;

/// Position tolerance — texts within this distance are considered "same position".
const POSITION_TOLERANCE: f64 = 5.0;

/// A fingerprint for a repeated text element.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct TextFingerprint {
    /// Rounded x position
    x_bucket: i32,
    /// Rounded y position
    y_bucket: i32,
    /// The text content
    content: String,
}

/// Detect watermarks across pages by finding text that repeats at the
/// same position on most pages. Returns indices of watermark text chunks
/// per page.
pub fn detect_watermarks(pages: &[Vec<ContentElement>]) -> Vec<Vec<usize>> {
    let total_pages = pages.len();
    if total_pages < MIN_PAGES_FOR_DETECTION {
        return pages.iter().map(|_| Vec::new()).collect();
    }

    // Count fingerprint occurrences across pages
    let mut fingerprint_counts: HashMap<TextFingerprint, usize> = HashMap::new();

    for page in pages {
        // Use a set per page to count each fingerprint at most once per page
        let mut seen_on_page: std::collections::HashSet<TextFingerprint> =
            std::collections::HashSet::new();
        for element in page {
            if let Some(fp) = make_fingerprint(element) {
                if seen_on_page.insert(fp.clone()) {
                    *fingerprint_counts.entry(fp).or_insert(0) += 1;
                }
            }
        }
    }

    // Find fingerprints that appear on >= WATERMARK_PAGE_FRACTION of pages
    let threshold = (total_pages as f64 * WATERMARK_PAGE_FRACTION).ceil() as usize;
    let watermark_fps: std::collections::HashSet<TextFingerprint> = fingerprint_counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(fp, _)| fp)
        .collect();

    if watermark_fps.is_empty() {
        return pages.iter().map(|_| Vec::new()).collect();
    }

    // Collect indices of watermark elements per page
    pages
        .iter()
        .map(|page| {
            page.iter()
                .enumerate()
                .filter_map(|(idx, element)| {
                    if let Some(fp) = make_fingerprint(element) {
                        if watermark_fps.contains(&fp) {
                            return Some(idx);
                        }
                    }
                    None
                })
                .collect()
        })
        .collect()
}

/// Remove watermark elements from pages in-place.
pub fn remove_watermarks(pages: &mut [Vec<ContentElement>]) {
    let watermark_indices = detect_watermarks(pages);

    for (page, indices) in pages.iter_mut().zip(watermark_indices.iter()) {
        if indices.is_empty() {
            continue;
        }
        let idx_set: std::collections::HashSet<usize> = indices.iter().copied().collect();
        let elements = std::mem::take(page);
        *page = elements
            .into_iter()
            .enumerate()
            .filter(|(i, _)| !idx_set.contains(i))
            .map(|(_, e)| e)
            .collect();
    }
}

/// Create a fingerprint from a text chunk element.
fn make_fingerprint(element: &ContentElement) -> Option<TextFingerprint> {
    match element {
        ContentElement::TextChunk(tc) => {
            let content = tc.value.trim().to_string();
            if content.is_empty() {
                return None;
            }
            Some(TextFingerprint {
                x_bucket: (tc.bbox.left_x / POSITION_TOLERANCE).round() as i32,
                y_bucket: (tc.bbox.bottom_y / POSITION_TOLERANCE).round() as i32,
                content,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text(value: &str, x: f64, y: f64) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: value.to_string(),
            bbox: BoundingBox::new(Some(1), x, y, x + 100.0, y + 12.0),
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
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        })
    }

    #[test]
    fn test_no_watermarks_below_min_pages() {
        let pages = vec![
            vec![make_text("DRAFT", 100.0, 400.0)],
            vec![make_text("DRAFT", 100.0, 400.0)],
        ];
        let wm = detect_watermarks(&pages);
        assert!(wm.iter().all(|v| v.is_empty()));
    }

    #[test]
    fn test_detect_draft_watermark() {
        let pages = vec![
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("Hello", 50.0, 700.0),
            ],
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("World", 50.0, 700.0),
            ],
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("Page3", 50.0, 700.0),
            ],
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("Page4", 50.0, 700.0),
            ],
        ];
        let wm = detect_watermarks(&pages);
        // "DRAFT" appears on all 4 pages at same position → watermark
        for page_wm in &wm {
            assert_eq!(page_wm.len(), 1);
            assert_eq!(page_wm[0], 0); // first element is "DRAFT"
        }
    }

    #[test]
    fn test_unique_content_not_watermark() {
        let pages = vec![
            vec![make_text("Intro", 50.0, 700.0)],
            vec![make_text("Chapter 1", 50.0, 700.0)],
            vec![make_text("Chapter 2", 50.0, 700.0)],
        ];
        let wm = detect_watermarks(&pages);
        assert!(wm.iter().all(|v| v.is_empty()));
    }

    #[test]
    fn test_remove_watermarks() {
        let mut pages = vec![
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("Hello", 50.0, 700.0),
            ],
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("World", 50.0, 700.0),
            ],
            vec![
                make_text("DRAFT", 200.0, 400.0),
                make_text("Content", 50.0, 700.0),
            ],
        ];
        remove_watermarks(&mut pages);
        // DRAFT should be removed from each page
        for page in &pages {
            assert_eq!(page.len(), 1);
        }
    }
}
