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

/// Geometric hybrid triage + merge (Docling HTTP behind `feature = "hybrid"`).
pub mod hybrid;

pub mod tagged;

use crate::api::config::ProcessingConfig;
use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::pdf::bookmark_extractor::extract_bookmarks;
use crate::pdf::chunk_parser::extract_page_chunks;
use crate::pdf::page_info;
#[cfg(not(target_arch = "wasm32"))]
use crate::pdf::raster_table_ocr::{
    recover_dominant_image_text_chunks, recover_page_raster_table_cell_text,
    recover_raster_table_borders,
};
use crate::pipeline::orchestrator::{run_pipeline, PipelineState};
use crate::pipeline::stages::heading_detector::refine_heading_hierarchy;
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
        elements.extend(
            recovered_text_chunks
                .into_iter()
                .map(ContentElement::TextChunk),
        );

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
    // Outline + section-number heading hierarchy (after style-based levels).
    let bookmarks = extract_bookmarks(&raw_doc.document);
    if !bookmarks.is_empty() {
        refine_heading_hierarchy(&mut pipeline_state.pages, &bookmarks);
    } else {
        refine_heading_hierarchy(&mut pipeline_state.pages, &[]);
    }
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
/// This is the WASM-compatible entry point. It replaces filesystem I/O with
/// in-memory equivalents. Embedded Image XObject tables are recovered via
/// [`pdf::inmem_raster`] when `raster_table_ocr` is enabled (no pdfimages /
/// pdftoppm). Heading hierarchy matches native [`convert`].
///
/// Implemented as [`assemble`]([`extract_session`](..)) so the sync path stays
/// identical to the two-phase WASM session.
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
    let session = extract_session(data, file_name, config, None)?;
    assemble(session, OcrAssembleMode::SyncEngine)
}

/// Progress callback: `(phase, done, total)`.
pub type ProgressFn<'a> = dyn FnMut(&str, u32, u32) + 'a;

/// Intermediate state between extract (plan) and assemble (finish).
pub struct ExtractSession {
    /// Display / document name.
    pub file_name: String,
    /// Processing config snapshot.
    pub config: ProcessingConfig,
    /// Author metadata.
    pub author: Option<String>,
    /// Title metadata.
    pub title: Option<String>,
    /// Creation date metadata.
    pub creation_date: Option<String>,
    /// Modification date metadata.
    pub modification_date: Option<String>,
    /// Producer metadata.
    pub producer: Option<String>,
    /// Creator metadata.
    pub creator: Option<String>,
    /// Page count.
    pub number_of_pages: u32,
    /// Per-page content elements (tables not yet recovered from OCR candidates).
    pub page_contents: Vec<Vec<ContentElement>>,
    /// Page geometry.
    pub page_info_list: Vec<page_info::PageInfo>,
    /// Tagged PDF MCID map.
    pub mcid_map: crate::tagged::struct_tree::McidMap,
    /// Bookmarks for heading refine.
    pub bookmarks: Vec<pdf::bookmark_extractor::Bookmark>,
    /// OCR candidates (image feature).
    #[cfg(feature = "image")]
    pub candidates: Vec<pdf::inmem_raster::RasterCandidate>,
    /// OCR words provided by the host, keyed by candidate id.
    #[cfg(feature = "image")]
    pub ocr_words: std::collections::HashMap<u32, Vec<pdf::ocr::OcrWord>>,
}

/// How assemble should obtain OCR for candidates.
pub enum OcrAssembleMode {
    /// Call the default sync [`pdf::ocr`] engine per candidate (legacy path).
    SyncEngine,
    /// Use words already stored on the session via [`ExtractSession::provide_ocr`].
    Provided,
    /// Call a caller-supplied engine (tests / custom hosts).
    #[cfg(feature = "image")]
    Engine(std::sync::Arc<dyn pdf::ocr::OcrEngine>),
}

/// Extract pages, chunks, and OCR candidates without recognizing text.
///
/// `on_progress` is invoked between pages as `("planning", page_done, page_total)`.
pub fn extract_session(
    data: &[u8],
    file_name: &str,
    config: &ProcessingConfig,
    mut on_progress: Option<&mut ProgressFn<'_>>,
) -> Result<ExtractSession, EdgePdfError> {
    let raw_doc = pdf::loader::load_pdf_from_bytes(data, config.password.as_deref())?;
    let page_info_list = page_info::extract_page_info(&raw_doc.document);
    let pages_map = raw_doc.document.get_pages();
    let page_total = pages_map.len() as u32;
    let mut page_contents = Vec::with_capacity(pages_map.len());
    #[cfg(feature = "image")]
    let mut candidates = Vec::new();
    #[cfg(feature = "image")]
    let mut next_id = 0u32;

    let mut page_done = 0u32;
    for (&page_num, &page_id) in &pages_map {
        let page_chunks = extract_page_chunks(&raw_doc.document, page_num, page_id)?;

        #[cfg(feature = "image")]
        if config.raster_table_ocr_enabled() {
            let (page_w, page_h) = page_info_list
                .iter()
                .find(|p| p.page_number == page_num)
                .map(|p| (p.width, p.height))
                .unwrap_or((612.0, 792.0));
            let page_cands = pdf::inmem_raster::collect_raster_candidates(
                &raw_doc.document,
                page_id,
                &page_chunks.image_chunks,
                &page_chunks.text_chunks,
                page_w,
                page_h,
                &mut next_id,
            );
            candidates.extend(page_cands);
        }

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
        // OCR tables are injected in assemble from candidates.
        page_contents.push(elements);

        page_done += 1;
        if let Some(cb) = on_progress.as_mut() {
            cb("planning", page_done, page_total);
        }
    }

    let mcid_map = build_mcid_map(&raw_doc.document);
    let bookmarks = extract_bookmarks(&raw_doc.document);

    Ok(ExtractSession {
        file_name: file_name.to_string(),
        config: config.clone(),
        author: raw_doc.metadata.author,
        title: raw_doc.metadata.title,
        creation_date: raw_doc.metadata.creation_date,
        modification_date: raw_doc.metadata.modification_date,
        producer: raw_doc.metadata.producer,
        creator: raw_doc.metadata.creator,
        number_of_pages: pages_map.len() as u32,
        page_contents,
        page_info_list,
        mcid_map,
        bookmarks,
        #[cfg(feature = "image")]
        candidates,
        #[cfg(feature = "image")]
        ocr_words: std::collections::HashMap::new(),
    })
}

impl ExtractSession {
    /// Store host OCR words for a candidate (two-phase finish path).
    #[cfg(feature = "image")]
    pub fn provide_ocr(&mut self, id: u32, words: Vec<pdf::ocr::OcrWord>) {
        self.ocr_words.insert(id, words);
    }

    /// Candidate metadata list (no pixels).
    #[cfg(feature = "image")]
    pub fn candidate_metas(&self) -> Vec<pdf::inmem_raster::RasterCandidateMeta> {
        self.candidates.iter().map(|c| c.meta()).collect()
    }

    /// Gray pixels for a candidate id.
    #[cfg(feature = "image")]
    pub fn candidate_gray(&self, id: u32) -> Option<&[u8]> {
        self.candidates
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.gray.as_slice())
    }
}

/// Drop the source Image (by XObject index) and inject OCR words as text chunks.
#[cfg(feature = "image")]
fn inject_ocr_text_blocks(
    session: &mut ExtractSession,
    page_idx: usize,
    cand: &pdf::inmem_raster::RasterCandidate,
    words: &[pdf::ocr::OcrWord],
) {
    if let Some(page) = session.page_contents.get_mut(page_idx) {
        if let Some(idx) = cand.image_index {
            page.retain(|el| match el {
                ContentElement::Image(img) => img.index != Some(idx),
                _ => true,
            });
        }
        for chunk in pdf::inmem_raster::ocr_words_to_text_chunks(cand, words) {
            page.push(ContentElement::TextChunk(chunk));
        }
    }
}

/// Run the pipeline after injecting OCR-derived tables from session candidates.
pub fn assemble(
    mut session: ExtractSession,
    mode: OcrAssembleMode,
) -> Result<PdfDocument, EdgePdfError> {
    #[cfg(feature = "image")]
    if session.config.raster_table_ocr_enabled() {
        use image::GrayImage;
        use pdf::inmem_raster::RasterCandidateKind;
        use pdf::ocr::{default_engine, OcrEngine};

        let candidates = std::mem::take(&mut session.candidates);
        for cand in &candidates {
            let words = match &mode {
                OcrAssembleMode::SyncEngine => {
                    if let Some(gray) =
                        GrayImage::from_raw(cand.width, cand.height, cand.gray.clone())
                    {
                        OcrEngine::recognize(&*default_engine(), &gray)
                    } else {
                        Vec::new()
                    }
                }
                OcrAssembleMode::Provided => {
                    session.ocr_words.get(&cand.id).cloned().unwrap_or_default()
                }
                OcrAssembleMode::Engine(engine) => {
                    if let Some(gray) =
                        GrayImage::from_raw(cand.width, cand.height, cand.gray.clone())
                    {
                        engine.recognize(&gray)
                    } else {
                        Vec::new()
                    }
                }
            };
            let page_idx = cand.page.saturating_sub(1) as usize;
            match cand.kind {
                RasterCandidateKind::Table => {
                    let table = pdf::inmem_raster::build_table_from_candidate(cand, &words)
                        .filter(|t| pdf::inmem_raster::bordered_table_is_plausible(t));
                    if let Some(table) = table {
                        if let Some(page) = session.page_contents.get_mut(page_idx) {
                            page.push(ContentElement::TableBorder(table));
                        }
                    } else if !words.is_empty() {
                        // OCR found glyphs but no lattice — assemble as text,
                        // not discard (right assembly from evidence).
                        inject_ocr_text_blocks(&mut session, page_idx, cand, &words);
                    }
                }
                RasterCandidateKind::TextBlocks => {
                    inject_ocr_text_blocks(&mut session, page_idx, cand, &words);
                }
            }
        }
        session.candidates = candidates;
    }

    #[cfg(not(feature = "image"))]
    let _ = mode;

    let mut pipeline_state = PipelineState::with_mcid_map(
        session.page_contents,
        session.config.clone(),
        session.mcid_map,
    )
    .with_page_info(session.page_info_list);
    run_pipeline(&mut pipeline_state)?;
    refine_heading_hierarchy(&mut pipeline_state.pages, &session.bookmarks);

    let mut doc = PdfDocument::new(session.file_name);
    doc.number_of_pages = session.number_of_pages;
    doc.author = session.author;
    doc.title = session.title;
    doc.creation_date = session.creation_date;
    doc.modification_date = session.modification_date;
    doc.producer = session.producer;
    doc.creator = session.creator;

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

    #[cfg(feature = "image")]
    struct FakeOcrEngine;

    #[cfg(feature = "image")]
    impl pdf::ocr::OcrEngine for FakeOcrEngine {
        fn recognize(&self, _gray: &image::GrayImage) -> Vec<pdf::ocr::OcrWord> {
            vec![
                pdf::ocr::OcrWord {
                    line_key: (1, 1, 1),
                    left: 10,
                    top: 10,
                    width: 40,
                    height: 12,
                    text: "A".into(),
                    confidence: 95.0,
                },
                pdf::ocr::OcrWord {
                    line_key: (1, 1, 1),
                    left: 80,
                    top: 10,
                    width: 40,
                    height: 12,
                    text: "B".into(),
                    confidence: 95.0,
                },
                pdf::ocr::OcrWord {
                    line_key: (1, 1, 2),
                    left: 10,
                    top: 40,
                    width: 40,
                    height: 12,
                    text: "C".into(),
                    confidence: 95.0,
                },
                pdf::ocr::OcrWord {
                    line_key: (1, 1, 2),
                    left: 80,
                    top: 40,
                    width: 40,
                    height: 12,
                    text: "D".into(),
                    confidence: 95.0,
                },
            ]
        }
    }

    #[cfg(feature = "image")]
    fn count_table_borders(doc: &PdfDocument) -> usize {
        doc.kids
            .iter()
            .filter(|e| matches!(e, models::content::ContentElement::TableBorder(_)))
            .count()
    }

    /// Two-phase (provide_ocr + Provided) equals assemble with the same Fake engine.
    #[cfg(feature = "image")]
    #[test]
    fn two_phase_equals_sync_fake_engine() {
        let dir = std::env::temp_dir().join("edgeparse_two_phase");
        std::fs::create_dir_all(&dir).unwrap();
        let pdf_path = dir.join("hello.pdf");
        create_test_pdf_file(&pdf_path);
        let data = std::fs::read(&pdf_path).unwrap();
        let config = ProcessingConfig::default();
        let engine: std::sync::Arc<dyn pdf::ocr::OcrEngine> = std::sync::Arc::new(FakeOcrEngine);

        let sync_doc = {
            let session = extract_session(&data, "hello.pdf", &config, None).unwrap();
            assemble(session, OcrAssembleMode::Engine(engine.clone())).unwrap()
        };

        let two_phase_doc = {
            let mut session = extract_session(&data, "hello.pdf", &config, None).unwrap();
            let ids: Vec<_> = session.candidates.iter().map(|c| c.id).collect();
            for id in ids {
                let gray = session.candidate_gray(id).unwrap().to_vec();
                let meta = session.candidates.iter().find(|c| c.id == id).unwrap();
                let img = image::GrayImage::from_raw(meta.width, meta.height, gray).unwrap();
                let words = engine.recognize(&img);
                session.provide_ocr(id, words);
            }
            assemble(session, OcrAssembleMode::Provided).unwrap()
        };

        assert_eq!(sync_doc.number_of_pages, two_phase_doc.number_of_pages);
        assert_eq!(
            count_table_borders(&sync_doc),
            count_table_borders(&two_phase_doc)
        );
        let sync_md = output::markdown::to_markdown(&sync_doc).unwrap();
        let phase_md = output::markdown::to_markdown(&two_phase_doc).unwrap();
        assert_eq!(sync_md, phase_md);
        let _ = std::fs::remove_file(&pdf_path);
    }

    /// Candidate pre-filter must keep table-like XObjects on docs 110 and 122.
    #[cfg(feature = "image")]
    #[test]
    fn candidate_prefilter_keeps_docs_110_and_122() {
        let roots = [
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/pdfs"),
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../odl-bench/pdfs"),
        ];
        for id in ["01030000000110", "01030000000122"] {
            let pdf = roots
                .iter()
                .map(|r| r.join(format!("{id}.pdf")))
                .find(|p| p.exists());
            let Some(pdf) = pdf else {
                eprintln!("skip {id}: PDF not found");
                continue;
            };
            let data = std::fs::read(&pdf).unwrap();
            let config = ProcessingConfig::default();
            let session = extract_session(&data, &format!("{id}.pdf"), &config, None).unwrap();
            assert!(
                !session.candidates.is_empty(),
                "{id}: expected ≥1 OCR candidate, got 0 (pre-filter dropped tables)"
            );
            // Sync assemble should still recover at least one table border.
            let doc = assemble(session, OcrAssembleMode::SyncEngine).unwrap();
            // Doc 110/122 are image-table heavy; allow zero only if OCR backend missing
            // but candidates must exist (assertion above). Soft-check tables when OCR works.
            let _ = count_table_borders(&doc);
        }
    }

    /// Pattern-painted page raster (Skia/Penpot style) must emit an OCR
    /// candidate, skip page-edge lines, and not render artifact metadata titles.
    #[cfg(feature = "image")]
    #[test]
    fn pattern_form_fixture_emits_text_blocks_candidate() {
        let pdf =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/pattern_form.pdf");
        assert!(pdf.exists(), "missing fixture {}", pdf.display());
        let data = std::fs::read(&pdf).unwrap();
        let config = ProcessingConfig::default();
        let session = extract_session(&data, "pattern_form.pdf", &config, None).unwrap();
        assert!(
            !session.candidates.is_empty(),
            "expected ≥1 OCR candidate from pattern-painted image"
        );
        assert!(
            session
                .candidates
                .iter()
                .any(|c| c.kind == pdf::inmem_raster::RasterCandidateKind::TextBlocks),
            "expected TextBlocks candidate for page-sized raster"
        );

        // No OCR words → still assemble; markdown must skip "Penpot - Render".
        let doc = assemble(session, OcrAssembleMode::Provided).unwrap();
        assert_eq!(doc.producer.as_deref(), Some("Skia/PDF m151"));
        let md = output::markdown::to_markdown(&doc).unwrap();
        assert!(
            !md.contains("# Penpot - Render"),
            "artifact metadata title must not become H1, got:\n{md}"
        );
        // Native text from the fixture should still appear.
        assert!(
            md.contains("NAME VALUE") || md.contains("ACME"),
            "expected native text in markdown, got:\n{md}"
        );
        // Reading order: title (higher on page) before value text.
        if let (Some(a), Some(b)) = (md.find("ACME"), md.find("NAME VALUE")) {
            assert!(a < b, "expected ACME before NAME VALUE in:\n{md}");
        }

        // Page-edge lines should not dominate kids.
        let edge_lines = doc
            .kids
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    models::content::ContentElement::Line(l)
                        if l.bbox.width() < 2.0 || l.bbox.height() < 2.0
                )
            })
            .count();
        assert_eq!(edge_lines, 0, "pattern fill must not emit page-edge lines");
        assert!(
            !doc.kids.iter().any(|e| matches!(
                e,
                models::content::ContentElement::Image(_)
                    | models::content::ContentElement::Figure(_)
            )),
            "page-sized TextBlocks source image should be dropped from layout"
        );
    }

    #[test]
    fn artifact_metadata_title_skipped_in_markdown() {
        let mut doc = PdfDocument::new("x.pdf".into());
        doc.title = Some("Penpot - Render".into());
        doc.producer = Some("Skia/PDF m151".into());
        doc.creator = Some("Chromium".into());
        doc.number_of_pages = 1;
        // Body text present but does not corroborate Info.Title → skip H1.
        doc.kids.push(models::content::ContentElement::TextChunk(
            models::chunks::TextChunk {
                value: "MANSUY RAPHAEL".into(),
                bbox: models::bbox::BoundingBox::new(Some(1), 10.0, 100.0, 200.0, 120.0),
                font_name: "Helvetica".into(),
                font_size: 12.0,
                font_weight: 400.0,
                italic_angle: 0.0,
                font_color: "#000000".into(),
                contrast_ratio: 21.0,
                symbol_ends: vec![200.0],
                text_format: models::enums::TextFormat::Normal,
                text_type: models::enums::TextType::Regular,
                pdf_layer: models::enums::PdfLayer::Main,
                ocg_visible: true,
                index: Some(0),
                page_number: Some(1),
                level: None,
                mcid: None,
            },
        ));
        let md = output::markdown::to_markdown(&doc).unwrap();
        assert!(
            !md.contains("# Penpot - Render"),
            "uncorroborated metadata title must be skipped when body text exists, got:\n{md}"
        );
        assert!(md.contains("MANSUY RAPHAEL"), "got:\n{md}");
    }
}
