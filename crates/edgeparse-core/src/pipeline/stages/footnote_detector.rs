//! Stage 27: Footnote Detection
//!
//! Identifies footnote paragraphs near the bottom of pages. Footnotes
//! typically have smaller font size than body text and start with
//! superscript number markers (e.g., "1", "1.", "¹").

use regex::Regex;
use std::sync::LazyLock;

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;

/// Maximum fraction of page height from the bottom where footnotes can appear.
const FOOTNOTE_ZONE_FRACTION: f64 = 0.35;

/// Footnotes must have font size at most this fraction of the dominant body font size.
const FONT_SIZE_RATIO: f64 = 0.85;

/// Regex for footnote markers at the start of text.
static FOOTNOTE_MARKER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[\s]*(\d{1,3}[\.\)\s]|[¹²³⁴⁵⁶⁷⁸⁹⁰]+[\.\)\s]?|\*{1,3}|†|‡|§)").unwrap()
});

/// Detect and mark footnotes across all pages.
pub fn detect_footnotes(pages: &mut [Vec<ContentElement>]) {
    let body_font_size = dominant_font_size(pages);
    if body_font_size <= 0.0 {
        return;
    }

    for page in pages.iter_mut() {
        let page_top = page_max_y(page);
        let page_bottom = page_min_y(page);
        let page_height = page_top - page_bottom;
        if page_height <= 0.0 {
            continue;
        }

        let footnote_y_threshold = page_bottom + page_height * FOOTNOTE_ZONE_FRACTION;

        for elem in page.iter_mut() {
            if let ContentElement::Paragraph(p) = elem {
                // Skip if already classified as header/footer/heading
                if matches!(
                    p.base.semantic_type,
                    SemanticType::Header | SemanticType::Footer | SemanticType::Heading
                ) {
                    continue;
                }

                let font_size = p.base.font_size.unwrap_or(0.0);
                let bbox = &p.base.bbox;

                // Must be in the bottom zone of the page
                if bbox.top_y > footnote_y_threshold {
                    continue;
                }

                // Must have smaller font size than body text
                if font_size > body_font_size * FONT_SIZE_RATIO {
                    continue;
                }

                // Must start with a footnote marker
                let text = p.base.value();
                if FOOTNOTE_MARKER_RE.is_match(&text) {
                    p.base.semantic_type = SemanticType::Note;
                }
            }
        }
    }
}

/// Find the dominant (most common) font size across all pages.
fn dominant_font_size(pages: &[Vec<ContentElement>]) -> f64 {
    let mut sizes: Vec<f64> = Vec::new();
    for page in pages {
        for elem in page {
            if let ContentElement::Paragraph(p) = elem {
                if let Some(fs) = p.base.font_size {
                    if fs > 0.0 {
                        sizes.push(fs);
                    }
                }
            }
        }
    }
    if sizes.is_empty() {
        return 0.0;
    }

    // Find mode by rounding to nearest 0.5pt
    let mut freq: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
    for &s in &sizes {
        let key = (s * 2.0).round() as i64;
        *freq.entry(key).or_default() += 1;
    }
    let best_key = freq.into_iter().max_by_key(|(_, c)| *c).unwrap().0;
    best_key as f64 / 2.0
}

/// Get the maximum Y coordinate across all elements on a page.
fn page_max_y(page: &[ContentElement]) -> f64 {
    page.iter()
        .map(|e| e.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Get the minimum Y coordinate across all elements on a page.
fn page_min_y(page: &[ContentElement]) -> f64 {
    page.iter()
        .map(|e| e.bbox().bottom_y)
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::SemanticType;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::semantic::SemanticParagraph;
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    fn make_para(text: &str, font_size: f64, bottom_y: f64, top_y: f64) -> ContentElement {
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(Some(1), 72.0, bottom_y, 300.0, top_y),
            font_name: "Arial".to_string(),
            font_size,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "000000".to_string(),
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
        };
        let line = TextLine {
            bbox: chunk.bbox.clone(),
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: line.bbox.clone(),
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let col = TextColumn {
            bbox: block.bbox.clone(),
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        let base = crate::models::semantic::SemanticTextNode {
            bbox: col.bbox.clone(),
            index: None,
            level: None,
            semantic_type: SemanticType::Paragraph,
            correct_semantic_score: None,
            columns: vec![col],
            font_weight: Some(400.0),
            font_size: Some(font_size),
            text_color: None,
            italic_angle: None,
            font_name: Some("Arial".to_string()),
            text_format: None,
            max_font_size: Some(font_size),
            background_color: None,
            is_hidden_text: false,
        };
        ContentElement::Paragraph(SemanticParagraph {
            base,
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    #[test]
    fn test_footnote_detected() {
        // Body text at top, footnote at bottom
        let body1 = make_para("This is the main body text.", 12.0, 400.0, 414.0);
        let body2 = make_para("Another paragraph.", 12.0, 380.0, 394.0);
        let footnote = make_para("1. See reference A.", 8.0, 50.0, 58.0);
        let mut pages = vec![vec![body1, body2, footnote]];

        detect_footnotes(&mut pages);

        if let ContentElement::Paragraph(p) = &pages[0][2] {
            assert_eq!(p.base.semantic_type, SemanticType::Note);
        } else {
            panic!("expected Paragraph");
        }
    }

    #[test]
    fn test_body_text_not_marked_footnote() {
        let body = make_para("This is the main body text.", 12.0, 400.0, 414.0);
        let mut pages = vec![vec![body]];

        detect_footnotes(&mut pages);

        if let ContentElement::Paragraph(p) = &pages[0][0] {
            assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
        }
    }

    #[test]
    fn test_footnote_with_unicode_marker() {
        let body1 = make_para("Main text here.", 12.0, 400.0, 414.0);
        let body2 = make_para("More body text.", 12.0, 380.0, 394.0);
        let footnote = make_para("¹ Cross-reference.", 9.0, 60.0, 69.0);
        let mut pages = vec![vec![body1, body2, footnote]];

        detect_footnotes(&mut pages);

        if let ContentElement::Paragraph(p) = &pages[0][2] {
            assert_eq!(p.base.semantic_type, SemanticType::Note);
        }
    }

    #[test]
    fn test_small_text_at_top_not_footnote() {
        let small_top = make_para("1. Small text at top.", 8.0, 700.0, 708.0);
        let body = make_para("Main body text.", 12.0, 400.0, 414.0);
        let mut pages = vec![vec![small_top, body]];

        detect_footnotes(&mut pages);

        // The small text is at the top — should NOT be marked as footnote
        if let ContentElement::Paragraph(p) = &pages[0][0] {
            assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
        }
    }
}
