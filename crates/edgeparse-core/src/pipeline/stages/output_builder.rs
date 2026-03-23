//! Stage 20 — Output Generation
//!
//! Assembles the final [`PdfDocument`] from the processed pipeline pages.
//! Flattens page contents into the document's `kids` element list and populates
//! metadata fields.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::pipeline::orchestrator::PipelineState;

/// Build a [`PdfDocument`] from the completed pipeline state.
///
/// This is Stage 20 — the final pipeline stage. It:
/// 1. Flattens all per-page content into a single `kids` list.
/// 2. Sets `number_of_pages` from the page count.
/// 3. Optionally strips header/footer elements based on config.
pub fn build_document(state: &PipelineState, file_name: &str) -> PdfDocument {
    let mut doc = PdfDocument::new(file_name.to_string());
    doc.number_of_pages = state.pages.len() as u32;

    for page in &state.pages {
        for element in page {
            if !state.config.include_header_footer
                && matches!(element, ContentElement::HeaderFooter(_))
            {
                continue;
            }
            doc.kids.push(element.clone());
        }
    }

    doc
}

/// Build a document, keeping elements separated by page. Returns a Vec of
/// per-page element vectors (useful for formats that need page boundaries).
pub fn build_paged_document(state: &PipelineState) -> Vec<Vec<ContentElement>> {
    state
        .pages
        .iter()
        .map(|page| {
            if state.config.include_header_footer {
                page.clone()
            } else {
                page.iter()
                    .filter(|e| !matches!(e, ContentElement::HeaderFooter(_)))
                    .cloned()
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::config::ProcessingConfig;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, SemanticType, TextFormat, TextType};
    use crate::models::semantic::SemanticHeaderOrFooter;

    fn text_chunk(val: &str) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: val.to_string(),
            bbox: BoundingBox::new(None, 0.0, 0.0, 100.0, 10.0),
            font_name: String::new(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: String::new(),
            contrast_ratio: 21.0,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: None,
            level: None,
            mcid: None,
        })
    }

    fn header_footer_elem() -> ContentElement {
        ContentElement::HeaderFooter(SemanticHeaderOrFooter {
            bbox: BoundingBox::new(None, 0.0, 800.0, 595.0, 842.0),
            index: None,
            level: None,
            semantic_type: SemanticType::Header,
            contents: vec![],
        })
    }

    #[test]
    fn test_build_empty_document() {
        let state = PipelineState::new(vec![], ProcessingConfig::default());
        let doc = build_document(&state, "empty.pdf");
        assert_eq!(doc.file_name, "empty.pdf");
        assert_eq!(doc.number_of_pages, 0);
        assert!(doc.kids.is_empty());
    }

    #[test]
    fn test_build_document_flattens_pages() {
        let pages = vec![
            vec![text_chunk("a"), text_chunk("b")],
            vec![text_chunk("c")],
        ];
        let state = PipelineState::new(pages, ProcessingConfig::default());
        let doc = build_document(&state, "test.pdf");
        assert_eq!(doc.number_of_pages, 2);
        assert_eq!(doc.kids.len(), 3);
    }

    #[test]
    fn test_build_document_strips_headers() {
        let pages = vec![vec![text_chunk("content"), header_footer_elem()]];
        let config = ProcessingConfig::default(); // include_header_footer = false
        let state = PipelineState::new(pages, config);
        let doc = build_document(&state, "test.pdf");
        assert_eq!(doc.kids.len(), 1); // header filtered out
    }

    #[test]
    fn test_build_document_includes_headers() {
        let pages = vec![vec![text_chunk("content"), header_footer_elem()]];
        let mut config = ProcessingConfig::default();
        config.include_header_footer = true;
        let state = PipelineState::new(pages, config);
        let doc = build_document(&state, "test.pdf");
        assert_eq!(doc.kids.len(), 2); // header included
    }

    #[test]
    fn test_build_paged_document() {
        let pages = vec![
            vec![text_chunk("a"), header_footer_elem()],
            vec![text_chunk("b")],
        ];
        let state = PipelineState::new(pages, ProcessingConfig::default());
        let paged = build_paged_document(&state);
        assert_eq!(paged.len(), 2);
        assert_eq!(paged[0].len(), 1); // header stripped
        assert_eq!(paged[1].len(), 1);
    }
}
