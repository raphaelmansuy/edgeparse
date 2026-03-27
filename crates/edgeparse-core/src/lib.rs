//! EdgeParse Core Library
//!
//! High-performance PDF-to-structured-data extraction engine.
//! Implements a 20-stage processing pipeline for extracting text, tables,
//! images, and semantic structure from PDF documents.

#![warn(missing_docs)]

pub mod api;
pub mod models;
pub mod output;
pub mod pdf;
pub mod pipeline;
pub mod utils;

#[cfg(feature = "hybrid")]
pub mod hybrid;

pub mod tagged;

use crate::api::config::ProcessingConfig;
use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::pdf::chunk_parser::extract_page_chunks;
use crate::pdf::page_info;
#[cfg(not(target_arch = "wasm32"))]
use crate::pdf::raster_table_ocr::{
    recover_dominant_image_text_chunks, recover_page_raster_table_cell_text,
    recover_raster_table_borders,
};
use crate::pipeline::orchestrator::{run_pipeline, PipelineState};
use crate::tagged::struct_tree::build_mcid_map;
use std::time::Instant;

/// Main entry point: convert a PDF file to structured data.
///
/// # Arguments
/// * `input_path` - Path to the input PDF file
/// * `config` - Processing configuration
///
/// # Returns
/// * `Result<PdfDocument>` - The extracted structured document
///
/// # Errors
/// Returns an error if the PDF cannot be loaded or processed.
#[cfg(not(target_arch = "wasm32"))]
pub fn convert(
    input_path: &std::path::Path,
    config: &ProcessingConfig,
) -> Result<PdfDocument, EdgePdfError> {
    let timing_enabled = timing_enabled();
    let total_start = Instant::now();

    let phase_start = Instant::now();
    let raw_doc = pdf::loader::load_pdf(input_path, config.password.as_deref())?;
    log_phase_duration(timing_enabled, "load_pdf", phase_start);

    // Extract per-page geometry (MediaBox, CropBox, rotation) for use throughout the pipeline.
    let phase_start = Instant::now();
    let page_info_list = page_info::extract_page_info(&raw_doc.document);
    log_phase_duration(timing_enabled, "extract_page_info", phase_start);

    // Extract text chunks from each page
    let pages_map = raw_doc.document.get_pages();
    // Index by 1-based page number for fast lookup during optional OCR recovery.
    // Keep this out of the default fast path when OCR is disabled.
    let page_info_by_number: Vec<Option<&page_info::PageInfo>> =
        if config.raster_table_ocr_enabled() {
            let mut index = vec![None; pages_map.len().saturating_add(1)];
            for info in &page_info_list {
                if let Some(slot) = index.get_mut(info.page_number as usize) {
                    *slot = Some(info);
                }
            }
            index
        } else {
            Vec::new()
        };
    let mut page_contents = Vec::with_capacity(pages_map.len());

    let phase_start = Instant::now();
    for (&page_num, &page_id) in &pages_map {
        let page_chunks = extract_page_chunks(&raw_doc.document, page_num, page_id)?;
        let mut recovered_text_chunks = Vec::new();
        let mut recovered_tables = Vec::new();
        if config.raster_table_ocr_enabled() {
            if let Some(Some(page_info)) = page_info_by_number.get(page_num as usize) {
                recovered_text_chunks = recover_dominant_image_text_chunks(
                    input_path,
                    &page_info.crop_box,
                    page_num,
                    &page_chunks.text_chunks,
                    &page_chunks.image_chunks,
                );
                recovered_tables = recover_raster_table_borders(
                    input_path,
                    &page_info.crop_box,
                    page_num,
                    &page_chunks.text_chunks,
                    &page_chunks.image_chunks,
                );
            }
        }
        let mut elements: Vec<ContentElement> = page_chunks
            .text_chunks
            .into_iter()
            .map(ContentElement::TextChunk)
            .collect();
        elements.extend(recovered_text_chunks.into_iter().map(ContentElement::TextChunk));

        elements.extend(
            page_chunks
                .image_chunks
                .into_iter()
                .map(ContentElement::Image),
        );
        elements.extend(
            page_chunks
                .line_chunks
                .into_iter()
                .map(ContentElement::Line),
        );
        elements.extend(
            page_chunks
                .line_art_chunks
                .into_iter()
                .map(ContentElement::LineArt),
        );
        elements.extend(
            recovered_tables
                .into_iter()
                .map(ContentElement::TableBorder),
        );

        page_contents.push(elements);
    }
    log_phase_duration(timing_enabled, "extract_page_chunks", phase_start);

    // Run the processing pipeline
    let phase_start = Instant::now();
    let mcid_map = build_mcid_map(&raw_doc.document);
    let mut pipeline_state = PipelineState::with_mcid_map(page_contents, config.clone(), mcid_map)
        .with_page_info(page_info_list);
    run_pipeline(&mut pipeline_state)?;
    log_phase_duration(timing_enabled, "run_pipeline", phase_start);

    // Build the output document
    let file_name = input_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.pdf")
        .to_string();

    let mut doc = PdfDocument::new(file_name);
    doc.source_path = Some(input_path.display().to_string());
    doc.number_of_pages = pages_map.len() as u32;
    doc.author = raw_doc.metadata.author;
    doc.title = raw_doc.metadata.title;
    doc.creation_date = raw_doc.metadata.creation_date;
    doc.modification_date = raw_doc.metadata.modification_date;

    let phase_start = Instant::now();
    if config.raster_table_ocr_enabled() {
        for (page_idx, page) in pipeline_state.pages.iter_mut().enumerate() {
            if let Some(page_info) = pipeline_state.page_info.get(page_idx) {
                recover_page_raster_table_cell_text(
                    input_path,
                    &page_info.crop_box,
                    page_info.page_number,
                    page,
                );
            }
        }
    }
    log_phase_duration(
        timing_enabled,
        "recover_page_raster_table_cell_text",
        phase_start,
    );

    // Flatten pipeline output into document kids
    let phase_start = Instant::now();
    for page in pipeline_state.pages {
        doc.kids.extend(page);
    }
    log_phase_duration(timing_enabled, "flatten_document", phase_start);
    log_phase_duration(timing_enabled, "convert_total", total_start);

    Ok(doc)
}

/// Convert a PDF from an in-memory byte slice to structured data.
///
/// This is the WASM-compatible entry point. It replaces all filesystem
/// operations with in-memory equivalents and skips raster table OCR.
///
/// # Arguments
/// * `data` — raw PDF bytes (e.g., from a `Uint8Array` in JavaScript)
/// * `file_name` — display name (used in `PdfDocument.file_name`)
/// * `config` — processing configuration
///
/// # Returns
/// Structured document or error.
///
/// # Errors
/// Returns an error if the PDF cannot be parsed or processed.
pub fn convert_bytes(
    data: &[u8],
    file_name: &str,
    config: &ProcessingConfig,
) -> Result<PdfDocument, EdgePdfError> {
    let raw_doc = pdf::loader::load_pdf_from_bytes(data, config.password.as_deref())?;

    let page_info_list = page_info::extract_page_info(&raw_doc.document);

    let pages_map = raw_doc.document.get_pages();
    let mut page_contents = Vec::with_capacity(pages_map.len());

    for (&page_num, &page_id) in &pages_map {
        let page_chunks = extract_page_chunks(&raw_doc.document, page_num, page_id)?;

        // Raster table OCR requires external pdfimages binary — skip in memory-only mode
        let recovered_tables = Vec::new();

        let mut elements: Vec<ContentElement> = page_chunks
            .text_chunks
            .into_iter()
            .map(ContentElement::TextChunk)
            .collect();

        elements.extend(
            page_chunks
                .image_chunks
                .into_iter()
                .map(ContentElement::Image),
        );
        elements.extend(
            page_chunks
                .line_chunks
                .into_iter()
                .map(ContentElement::Line),
        );
        elements.extend(
            page_chunks
                .line_art_chunks
                .into_iter()
                .map(ContentElement::LineArt),
        );
        elements.extend(
            recovered_tables
                .into_iter()
                .map(ContentElement::TableBorder),
        );

        page_contents.push(elements);
    }

    let mcid_map = build_mcid_map(&raw_doc.document);
    let mut pipeline_state = PipelineState::with_mcid_map(page_contents, config.clone(), mcid_map)
        .with_page_info(page_info_list);
    run_pipeline(&mut pipeline_state)?;

    let mut doc = PdfDocument::new(file_name.to_string());
    doc.number_of_pages = pages_map.len() as u32;
    doc.author = raw_doc.metadata.author;
    doc.title = raw_doc.metadata.title;
    doc.creation_date = raw_doc.metadata.creation_date;
    doc.modification_date = raw_doc.metadata.modification_date;

    for page in pipeline_state.pages {
        doc.kids.extend(page);
    }

    Ok(doc)
}

/// Top-level error type for EdgeParse operations.
#[derive(Debug, thiserror::Error)]
pub enum EdgePdfError {
    /// PDF loading error
    #[error("PDF loading error: {0}")]
    LoadError(String),

    /// Pipeline processing error
    #[error("Pipeline error at stage {stage}: {message}")]
    PipelineError {
        /// Pipeline stage number (1-20)
        stage: u32,
        /// Error description
        message: String,
    },

    /// Output generation error
    #[error("Output error: {0}")]
    OutputError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// lopdf error
    #[error("PDF parse error: {0}")]
    LopdfError(String),
}

impl From<lopdf::Error> for EdgePdfError {
    fn from(e: lopdf::Error) -> Self {
        EdgePdfError::LopdfError(e.to_string())
    }
}

fn timing_enabled() -> bool {
    std::env::var("EDGEPARSE_TIMING")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn log_phase_duration(enabled: bool, phase: &str, start: Instant) {
    if enabled {
        log::info!(
            "Timing {}: {:.2} ms",
            phase,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{
        content::{Content, Operation},
        dictionary, Object, Stream,
    };
    use std::io::Write;

    /// Create a synthetic PDF file for integration testing.
    fn create_test_pdf_file(path: &std::path::Path) {
        let mut doc = lopdf::Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![72.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal("Hello EdgeParse!")]),
                Operation::new("Td", vec![0.into(), Object::Real(-20.0)]),
                Operation::new("Tj", vec![Object::string_literal("Second line of text.")]),
                Operation::new("ET", vec![]),
            ],
        };

        let encoded = content.encode().unwrap();
        let content_id = doc.add_object(Stream::new(dictionary! {}, encoded));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut file = std::fs::File::create(path).unwrap();
        doc.save_to(&mut file).unwrap();
        file.flush().unwrap();
    }

    #[test]
    fn test_convert_end_to_end() {
        let dir = std::env::temp_dir().join("edgeparse_test");
        std::fs::create_dir_all(&dir).unwrap();
        let pdf_path = dir.join("test_convert.pdf");

        create_test_pdf_file(&pdf_path);

        let config = ProcessingConfig::default();
        let result = convert(&pdf_path, &config);
        assert!(result.is_ok(), "convert() failed: {:?}", result.err());

        let doc = result.unwrap();
        assert_eq!(doc.number_of_pages, 1);
        assert!(
            !doc.kids.is_empty(),
            "Expected content elements in document"
        );

        // Check that we extracted content (may be TextChunks, TextLines, or TextBlocks after pipeline)
        let mut all_text = String::new();
        for element in &doc.kids {
            match element {
                models::content::ContentElement::TextChunk(tc) => {
                    all_text.push_str(&tc.value);
                    all_text.push(' ');
                }
                models::content::ContentElement::TextLine(tl) => {
                    all_text.push_str(&tl.value());
                    all_text.push(' ');
                }
                models::content::ContentElement::TextBlock(tb) => {
                    all_text.push_str(&tb.value());
                    all_text.push(' ');
                }
                models::content::ContentElement::Paragraph(p) => {
                    all_text.push_str(&p.base.value());
                    all_text.push(' ');
                }
                models::content::ContentElement::Heading(h) => {
                    all_text.push_str(&h.base.base.value());
                    all_text.push(' ');
                }
                _ => {}
            }
        }

        assert!(
            all_text.contains("Hello"),
            "Expected 'Hello' in extracted text, got: {}",
            all_text
        );
        assert!(
            all_text.contains("Second"),
            "Expected 'Second' in extracted text, got: {}",
            all_text
        );

        // Cleanup
        let _ = std::fs::remove_file(&pdf_path);
    }
}
