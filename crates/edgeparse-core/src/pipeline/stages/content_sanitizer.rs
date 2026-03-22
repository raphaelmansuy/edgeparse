//! Stage 19: Content Sanitization
//!
//! Applies PII masking and Unicode normalization to all text content
//! when sanitization is enabled in the processing config.

use crate::models::content::ContentElement;
use crate::utils::sanitizer::{self, SanitizationRule};

/// Sanitize all text content in the document.
pub fn sanitize_content(pages: &mut [Vec<ContentElement>], sanitize: bool) {
    if !sanitize {
        return;
    }

    let rules = sanitizer::default_rules();

    for page in pages.iter_mut() {
        for elem in page.iter_mut() {
            sanitize_element(elem, &rules);
        }
    }
}

/// Sanitize text within a single content element.
fn sanitize_element(elem: &mut ContentElement, rules: &[SanitizationRule]) {
    match elem {
        ContentElement::TextChunk(c) => {
            c.value = sanitizer::sanitize_text(&c.value, rules);
            c.value = sanitizer::normalize_unicode(&c.value);
        }
        ContentElement::TextLine(l) => {
            for chunk in &mut l.text_chunks {
                chunk.value = sanitizer::sanitize_text(&chunk.value, rules);
                chunk.value = sanitizer::normalize_unicode(&chunk.value);
            }
        }
        ContentElement::TextBlock(b) => {
            for line in &mut b.text_lines {
                for chunk in &mut line.text_chunks {
                    chunk.value = sanitizer::sanitize_text(&chunk.value, rules);
                    chunk.value = sanitizer::normalize_unicode(&chunk.value);
                }
            }
        }
        ContentElement::Paragraph(p) => {
            sanitize_semantic_node_columns(&mut p.base.columns, rules);
        }
        ContentElement::Heading(h) => {
            sanitize_semantic_node_columns(&mut h.base.base.columns, rules);
        }
        ContentElement::List(l) => {
            for item in &mut l.list_items {
                sanitize_token_rows(&mut item.label.content, rules);
                sanitize_token_rows(&mut item.body.content, rules);
            }
        }
        ContentElement::TableBorder(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    for token in &mut cell.content {
                        token.base.value = sanitizer::sanitize_text(&token.base.value, rules);
                        token.base.value = sanitizer::normalize_unicode(&token.base.value);
                    }
                }
            }
        }
        _ => {}
    }
}

/// Sanitize text in TextColumn structures.
fn sanitize_semantic_node_columns(
    columns: &mut [crate::models::text::TextColumn],
    rules: &[SanitizationRule],
) {
    for col in columns.iter_mut() {
        for block in &mut col.text_blocks {
            for line in &mut block.text_lines {
                for chunk in &mut line.text_chunks {
                    chunk.value = sanitizer::sanitize_text(&chunk.value, rules);
                    chunk.value = sanitizer::normalize_unicode(&chunk.value);
                }
            }
        }
    }
}

/// Sanitize text in token rows (used in list labels/bodies).
fn sanitize_token_rows(
    rows: &mut [crate::models::table::TableTokenRow],
    rules: &[SanitizationRule],
) {
    for row in rows.iter_mut() {
        for token in row.iter_mut() {
            token.base.value = sanitizer::sanitize_text(&token.base.value, rules);
            token.base.value = sanitizer::normalize_unicode(&token.base.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn text_chunk(value: &str) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: value.to_string(),
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 20.0),
            index: None,
            level: None,
            mcid: None,
            page_number: Some(1),
            font_size: 12.0,
            font_weight: 400.0,
            font_name: "Arial".to_string(),
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
        })
    }

    #[test]
    fn test_sanitize_email_in_text_chunk() {
        let mut pages = vec![vec![text_chunk("Contact john@example.com")]];
        sanitize_content(&mut pages, true);
        if let ContentElement::TextChunk(c) = &pages[0][0] {
            assert!(c.value.contains("[EMAIL]"));
            assert!(!c.value.contains("john@example.com"));
        }
    }

    #[test]
    fn test_no_sanitize_when_disabled() {
        let mut pages = vec![vec![text_chunk("Contact john@example.com")]];
        sanitize_content(&mut pages, false);
        if let ContentElement::TextChunk(c) = &pages[0][0] {
            assert!(c.value.contains("john@example.com"));
        }
    }

    #[test]
    fn test_sanitize_url() {
        let mut pages = vec![vec![text_chunk("Visit https://example.com/page")]];
        sanitize_content(&mut pages, true);
        if let ContentElement::TextChunk(c) = &pages[0][0] {
            assert!(c.value.contains("[URL]"));
        }
    }
}
