//! Integration tests for the full pipeline and output rendering.

#[cfg(test)]
mod integration_tests {
    use edgeparse_core::api::config::ProcessingConfig;
    use edgeparse_core::models::bbox::BoundingBox;
    use edgeparse_core::models::chunks::TextChunk;
    use edgeparse_core::models::content::ContentElement;
    use edgeparse_core::models::document::PdfDocument;
    use edgeparse_core::models::enums::{PdfLayer, TextFormat, TextType};
    use edgeparse_core::output::{html, json, markdown, text};
    use edgeparse_core::pipeline::orchestrator::{run_pipeline, PipelineState};

    fn make_text_chunk(
        val: &str,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        font_size: f64,
    ) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: val.to_string(),
            bbox: BoundingBox::new(Some(0), x, y, x + w, y + h),
            font_name: "Helvetica".to_string(),
            font_size,
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
            page_number: Some(0),
            level: None,
            mcid: None,
        })
    }

    #[test]
    fn test_full_pipeline_empty() {
        let mut state = PipelineState::new(vec![], ProcessingConfig::default());
        let result = run_pipeline(&mut state);
        assert!(result.is_ok());
        assert!(state.pages.is_empty());
    }

    #[test]
    fn test_full_pipeline_single_page() {
        let page = vec![
            make_text_chunk("Hello", 72.0, 750.0, 50.0, 12.0, 12.0),
            make_text_chunk(" world", 122.0, 750.0, 50.0, 12.0, 12.0),
            make_text_chunk("Second line.", 72.0, 730.0, 100.0, 12.0, 12.0),
        ];
        let mut state = PipelineState::new(vec![page], ProcessingConfig::default());
        let result = run_pipeline(&mut state);
        assert!(result.is_ok());
        // Pipeline should have processed elements (may merge into text lines/blocks/paragraphs)
        assert!(!state.pages.is_empty());
    }

    #[test]
    fn test_full_pipeline_multi_page() {
        let page1 = vec![make_text_chunk(
            "Page one content",
            72.0,
            750.0,
            150.0,
            12.0,
            12.0,
        )];
        let page2 = vec![make_text_chunk(
            "Page two content",
            72.0,
            750.0,
            150.0,
            12.0,
            12.0,
        )];
        let mut state = PipelineState::new(vec![page1, page2], ProcessingConfig::default());
        let result = run_pipeline(&mut state);
        assert!(result.is_ok());
        assert_eq!(state.pages.len(), 2);
    }

    #[test]
    fn test_json_roundtrip() {
        let mut doc = PdfDocument::new("test.pdf".to_string());
        doc.title = Some("Test Document".to_string());
        doc.number_of_pages = 1;
        doc.author = Some("Tester".to_string());

        let json_str = json::to_json_string(&doc).unwrap();
        assert!(json_str.contains("Test Document"));
        assert!(json_str.contains("Tester"));

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["title"], "Test Document");
    }

    #[test]
    fn test_markdown_output() {
        let mut doc = PdfDocument::new("test.pdf".to_string());
        doc.title = Some("My Report".to_string());
        let md = markdown::to_markdown(&doc).unwrap();
        assert!(md.contains("# My Report"));
    }

    #[test]
    fn test_html_output() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let html_str = html::to_html(&doc).unwrap();
        assert!(html_str.contains("<html"));
        assert!(html_str.contains("</html>"));
    }

    #[test]
    fn test_text_output() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let txt = text::to_text(&doc).unwrap();
        // Should at least not panic
        assert!(txt.is_empty() || !txt.is_empty());
    }

    #[test]
    fn test_pipeline_with_page_range() {
        let pages = vec![
            vec![make_text_chunk("Page 1", 72.0, 750.0, 50.0, 12.0, 12.0)],
            vec![make_text_chunk("Page 2", 72.0, 750.0, 50.0, 12.0, 12.0)],
            vec![make_text_chunk("Page 3", 72.0, 750.0, 50.0, 12.0, 12.0)],
        ];
        let mut config = ProcessingConfig::default();
        config.pages = Some("1,3".to_string());
        let mut state = PipelineState::new(pages, config);
        let result = run_pipeline(&mut state);
        assert!(result.is_ok());
        // Should have filtered to 2 pages
        assert_eq!(state.pages.len(), 2);
    }
}
