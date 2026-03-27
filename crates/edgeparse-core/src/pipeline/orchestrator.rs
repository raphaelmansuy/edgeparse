//! Pipeline orchestrator — runs all processing stages in sequence.
//!
//! ```text
//!   PDF bytes
//!     │
//!     ▼
//!  ┌─────────────────────────────────────────────┐
//!  │ Stage 0:  Page Range Filtering               │
//!  │ Stage 1b: Watermark Removal                  │
//!  │ Stage 2:  Content Filtering + FFFD Replace   │
//!  └──────────────────┬──────────────────────────┘
//!                     │  raw TextChunks, Lines, Images
//!                     ▼
//!  ┌─────────────────────────────────────────────┐
//!  │ Stage 3-4: Border Table Detection            │
//!  │ Stage 4b:  Content → Table Cells             │
//!  │ Stage 4c:  Boxed Heading Promoter            │
//!  │ Stage 4d:  Pre-Cluster Table Release         │
//!  └──────────────────┬──────────────────────────┘
//!                     │  TextChunks + TableBorders
//!                     ▼
//!  ┌─────────────────────────────────────────────┐
//!  │ Stage 5b: Column Detection                   │
//!  │ Stage 6:  TextChunk → TextLine Grouping      │
//!  │ Stage 6.5: List Detection Pass 1 (TextLine)  │
//!  │ Stage 7:  TextLine → TextBlock Grouping      │
//!  │ Stage 7b: Cluster (Borderless) Tables        │
//!  └──────────────────┬──────────────────────────┘
//!                     │  TextBlocks + Tables + Lists
//!                     ▼
//!  ┌─────────────────────────────────────────────┐
//!  │ Stage 8:  Header / Footer Detection          │
//!  │ Stage 9:  List Detection Pass 1 (Block)      │
//!  │ Stage 10: Paragraph Detection                │
//!  │ Stage 10b: Figure Detection                  │
//!  │ Stage 12: Heading Detection                  │
//!  └──────────────────┬──────────────────────────┘
//!                     │  Semantic elements
//!                     ▼
//!  ┌─────────────────────────────────────────────┐
//!  │ Stage 11:  List Detection Pass 2 (Paragraph) │
//!  │ Stage 13:  ID Assignment                     │
//!  │ Stage 14:  Caption + Footnote + TOC Linking  │
//!  │ Stage 15:  Cross-Page Table Linking          │
//!  │ Stage 17:  Nesting Levels                    │
//!  │ Stage 18:  Reading Order Sort                │
//!  │ Stage 19:  Content Sanitization              │
//!  └──────────────────┬──────────────────────────┘
//!                     │
//!                     ▼
//!              PdfDocument (ready for output)
//! ```

use crate::api::config::ProcessingConfig;
use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::pdf::page_info::PageInfo;
use crate::pipeline::logging::PipelineTimer;
use crate::pipeline::parallel::{par_map_pages, par_map_pages_indexed};
use crate::pipeline::stages::boxed_heading_promoter;
use crate::pipeline::stages::caption_linker;
use crate::pipeline::stages::cluster_table_detector;
use crate::pipeline::stages::column_detector;
use crate::pipeline::stages::content_filter;
use crate::pipeline::stages::content_sanitizer;
use crate::pipeline::stages::cross_page_linker;
use crate::pipeline::stages::figure_detector;
use crate::pipeline::stages::footnote_detector;
use crate::pipeline::stages::header_footer;
use crate::pipeline::stages::heading_detector;
use crate::pipeline::stages::id_assignment;
use crate::pipeline::stages::list_detector;
use crate::pipeline::stages::list_pass2;
use crate::pipeline::stages::nesting_level;
use crate::pipeline::stages::paragraph_detector;
use crate::pipeline::stages::reading_order;
use crate::pipeline::stages::table_content_assigner;
use crate::pipeline::stages::table_detector;
use crate::pipeline::stages::text_block_grouper;
use crate::pipeline::stages::text_line_grouper;
use crate::pipeline::stages::toc_detector;
use crate::pipeline::stages::watermark_detector;
use crate::tagged::struct_tree::McidMap;
use crate::utils::page_range;
use crate::EdgePdfError;

/// Per-page content during pipeline processing.
pub type PageContent = Vec<ContentElement>;

/// Pipeline state passed between stages.
pub struct PipelineState {
    /// Per-page content elements
    pub pages: Vec<PageContent>,
    /// Processing configuration
    pub config: ProcessingConfig,
    /// MCID map from structure tree (tagged PDFs).
    /// Maps (page_number, mcid) → tag info for heading detection.
    pub mcid_map: Option<McidMap>,
    /// Per-page geometry (MediaBox, width, height). Index matches pages.
    pub page_info: Vec<PageInfo>,
}

impl PipelineState {
    /// Create a new pipeline state from raw page content.
    pub fn new(pages: Vec<PageContent>, config: ProcessingConfig) -> Self {
        Self {
            pages,
            config,
            mcid_map: None,
            page_info: Vec::new(),
        }
    }

    /// Create a new pipeline state with an MCID map from a tagged PDF.
    pub fn with_mcid_map(
        pages: Vec<PageContent>,
        config: ProcessingConfig,
        mcid_map: McidMap,
    ) -> Self {
        let mcid_map = if mcid_map.is_empty() {
            None
        } else {
            Some(mcid_map)
        };
        Self {
            pages,
            config,
            mcid_map,
            page_info: Vec::new(),
        }
    }

    /// Create a new pipeline state with page geometry.
    pub fn with_page_info(mut self, page_info: Vec<PageInfo>) -> Self {
        self.page_info = page_info;
        self
    }

    /// Total number of content elements across all pages.
    pub fn total_elements(&self) -> usize {
        self.pages.iter().map(|p| p.len()).sum()
    }
}

/// Run the full 20-stage pipeline.
///
/// # Errors
/// Returns `EdgePdfError::PipelineError` if any stage fails.
pub fn run_pipeline(state: &mut PipelineState) -> Result<(), EdgePdfError> {
    macro_rules! timed_stage {
        ($timer:expr, $name:expr, $state:expr, $body:block) => {{
            if let Some(timer) = $timer.as_mut() {
                timer.start_stage($name);
            }
            let result = { $body };
            if let Some(timer) = $timer.as_mut() {
                timer.end_stage($state.total_elements());
            }
            result
        }};
    }

    let mut timer = pipeline_timing_enabled().then(PipelineTimer::new);

    log::info!(
        "Starting pipeline with {} pages, {} elements",
        state.pages.len(),
        state.total_elements()
    );

    // Stage 1: PDF Loading (already done before pipeline)

    // Stage 0b: Page Range Filtering
    timed_stage!(timer, "Stage 0b (Page Range Filtering)", state, {
        if let Some(ref range_str) = state.config.pages {
            let total = state.pages.len();
            if let Some(selected) = page_range::parse_page_range(range_str, total) {
                state.pages = page_range::filter_pages(std::mem::take(&mut state.pages), &selected);
                if !state.page_info.is_empty() {
                    state.page_info = state
                        .page_info
                        .drain(..)
                        .enumerate()
                        .filter_map(|(idx, info)| {
                            let page_num = idx + 1;
                            if selected.contains(&page_num) {
                                Some(info)
                            } else {
                                None
                            }
                        })
                        .collect();
                }
                log::info!(
                    "Page range filter: kept {} of {} pages",
                    state.pages.len(),
                    total
                );
            }
        }
    });

    // Stage 1b: Watermark Detection & Removal
    timed_stage!(timer, "Stage 1b (Watermark Removal)", state, {
        watermark_detector::remove_watermarks(&mut state.pages);
    });
    log::info!(
        "Stage 1b (Watermark Removal) complete: {} elements",
        state.total_elements()
    );

    // Stage 2: Content Filtering
    let filter_config = &state.config.filter_config;
    // Default A4 page bbox — will be refined when we track per-page MediaBox
    let default_page = BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0);

    timed_stage!(timer, "Stage 2 (Content Filtering)", state, {
        par_map_pages_indexed(&mut state.pages, |page_idx, elements| {
            let page_bbox = state
                .page_info
                .get(page_idx)
                .map(|info| info.crop_box.clone())
                .unwrap_or_else(|| default_page.clone());
            content_filter::filter_content(elements, filter_config, &page_bbox)
        });
    });
    log::info!(
        "Stage 2 (Content Filtering) complete: {} elements",
        state.total_elements()
    );

    // Stage 2b: Replace undefined characters (U+FFFD → replacement char)
    // Matches the reference TextProcessor.replaceUndefinedCharacters() called from
    // ContentFilterProcessor.  Default replacement is space " ".
    let replacement = &state.config.replace_invalid_chars;
    timed_stage!(timer, "Stage 2b (Replace Undefined Chars)", state, {
        if replacement != "\u{FFFD}" {
            par_map_pages(&mut state.pages, |mut elements| {
                for elem in &mut elements {
                    replace_fffd_in_element(elem, replacement);
                }
                elements
            });
        }
    });
    log::info!("Stage 2b (Replace Undefined Chars) complete");

    // Stage 3-4: Table Border Detection
    timed_stage!(timer, "Stage 3-4 (Table Border Detection)", state, {
        par_map_pages(&mut state.pages, table_detector::detect_table_borders);
    });
    log::info!(
        "Stage 3-4 (Table Border Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 4b: Content Assignment to Table Cells
    timed_stage!(timer, "Stage 4b (Table Content Assignment)", state, {
        par_map_pages(
            &mut state.pages,
            table_content_assigner::assign_content_to_tables,
        );
    });
    log::info!(
        "Stage 4b (Table Content Assignment) complete: {} elements",
        state.total_elements()
    );

    // Stage 4b2: Filter mostly-empty bordered tables (chart grid FPs)
    timed_stage!(timer, "Stage 4b2 (Empty Table Filter)", state, {
        par_map_pages(&mut state.pages, table_detector::filter_empty_tables);
    });
    log::info!(
        "Stage 4b2 (Empty Table Filter) complete: {} elements",
        state.total_elements()
    );

    // Stage 4c: Boxed Heading Promoter — single-cell tables with short heading text
    // are released back as free TextChunks so heading_detector can see them.
    timed_stage!(timer, "Stage 4c (Boxed Heading Promoter)", state, {
        par_map_pages(
            &mut state.pages,
            boxed_heading_promoter::promote_boxed_headings,
        );
    });
    log::info!(
        "Stage 4c (Boxed Heading Promoter) complete: {} elements",
        state.total_elements()
    );

    // Stage 4d: Release page-wide single-cell pseudo-tables before line/block
    // grouping so cluster detection can recover the underlying text layout.
    timed_stage!(timer, "Stage 4d (Pre-Cluster Table Release)", state, {
        par_map_pages(&mut state.pages, table_detector::release_pre_cluster_tables);
    });
    log::info!(
        "Stage 4d (Pre-Cluster Table Release) complete: {} elements",
        state.total_elements()
    );

    // Stage 5: Line Chunk Removal — handled by table detector (consumed lines removed)

    // Stage 5b: Multi-Column Detection
    let column_layouts = timed_stage!(timer, "Stage 5b (Column Detection)", state, {
        column_detector::detect_columns(&mut state.pages)
    });
    log::info!(
        "Stage 5b (Column Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 6: Text Line Grouping
    timed_stage!(timer, "Stage 6 (Text Line Grouping)", state, {
        par_map_pages_indexed(&mut state.pages, |page_idx, elements| {
            let layout = column_layouts.get(page_idx);
            text_line_grouper::group_text_lines(elements, layout)
        });
    });
    log::info!(
        "Stage 6 (Text Line Grouping) complete: {} elements",
        state.total_elements()
    );

    // Stage 6.5: List Detection Pass 1 (TextLine level — before block grouping)
    // Matches the reference pipeline: ListProcessor.processLists() runs on individual
    // TextLines BEFORE ParagraphProcessor.processParagraphs().  This catches
    // bibliography entries ([N] bracket notation) and other list patterns at the
    // TextLine level before they get merged into TextBlocks by Stage 7.
    timed_stage!(timer, "Stage 6.5 (List Detection Pass 1)", state, {
        par_map_pages(&mut state.pages, list_detector::detect_lists);
    });
    log::info!(
        "Stage 6.5 (List Detection Pass 1) complete: {} elements",
        state.total_elements()
    );

    // Stage 7: Text Block Grouping (paragraph detection)
    timed_stage!(timer, "Stage 7 (Text Block Grouping)", state, {
        par_map_pages(&mut state.pages, text_block_grouper::group_text_blocks);
    });
    log::info!(
        "Stage 7 (Text Block Grouping) complete: {} elements",
        state.total_elements()
    );

    // Stage 7b: Cluster (Borderless) Table Detection
    timed_stage!(timer, "Stage 7b (Cluster Table Detection)", state, {
        par_map_pages(
            &mut state.pages,
            cluster_table_detector::detect_cluster_tables,
        );
    });
    log::info!(
        "Stage 7b (Cluster Table Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 7b2: Reject table-shaped layout artifacts produced by the border
    // and cluster detectors, releasing their text back into the page flow.
    timed_stage!(timer, "Stage 7b2 (Suspicious Table Filter)", state, {
        par_map_pages(&mut state.pages, table_detector::filter_suspicious_tables);
    });
    log::info!(
        "Stage 7b2 (Suspicious Table Filter) complete: {} elements",
        state.total_elements()
    );

    // Stage 8: Header/Footer Detection (cross-page)
    // Use median page height from page info, or fallback to A4.
    let page_height = if !state.page_info.is_empty() {
        let mut heights: Vec<f64> = state.page_info.iter().map(|p| p.height).collect();
        heights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        heights[heights.len() / 2]
    } else {
        842.0
    };
    timed_stage!(timer, "Stage 8 (Header/Footer Detection)", state, {
        header_footer::detect_headers_footers(&mut state.pages, page_height);
    });
    log::info!(
        "Stage 8 (Header/Footer Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 9: List Detection Pass 1 (TextBlock/TextLine level)
    // Also runs at Stage 6.5 on TextLines; this second pass catches patterns
    // from TextBlocks that the block grouper broke apart (e.g., numbered lists).
    timed_stage!(timer, "Stage 9 (List Detection)", state, {
        par_map_pages(&mut state.pages, list_detector::detect_lists);
    });
    log::info!(
        "Stage 9 (List Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 10: Paragraph Detection
    timed_stage!(timer, "Stage 10 (Paragraph Detection)", state, {
        par_map_pages(&mut state.pages, paragraph_detector::detect_paragraphs);
    });
    log::info!(
        "Stage 10 (Paragraph Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 10b: Figure Detection
    timed_stage!(timer, "Stage 10b (Figure Detection)", state, {
        par_map_pages(&mut state.pages, figure_detector::detect_figures);
    });
    log::info!(
        "Stage 10b (Figure Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 12: Heading Detection (moved before List Pass 2 so headings are
    // tagged before list body-continuation filtering)
    timed_stage!(timer, "Stage 12 (Heading Detection)", state, {
        heading_detector::detect_headings(&mut state.pages, state.mcid_map.as_ref());
    });
    log::info!(
        "Stage 12 (Heading Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 18 (pre-pass): Reading Order Sorting before List Pass 2
    // so elements are in correct reading order for sequential list detection.
    timed_stage!(timer, "Stage 18-pre (Reading Order pre-pass)", state, {
        reading_order::sort_reading_order(&mut state.pages, &state.page_info);
    });
    log::info!(
        "Stage 18-pre (Reading Order pre-pass) complete: {} elements",
        state.total_elements()
    );

    // Stage 11: List Detection Pass 2 (Paragraph Level)
    timed_stage!(timer, "Stage 11 (List Detection Pass 2)", state, {
        par_map_pages(&mut state.pages, list_pass2::detect_paragraph_lists);
    });
    log::info!(
        "Stage 11 (List Detection Pass 2) complete: {} elements",
        state.total_elements()
    );

    // Stage 11b: Document-level common-prefix list detection (Figure N, Table N)
    timed_stage!(timer, "Stage 11b (Common-prefix Lists)", state, {
        list_pass2::detect_common_prefix_lists_document(&mut state.pages);
    });
    log::info!(
        "Stage 11b (Common-prefix Lists) complete: {} elements",
        state.total_elements()
    );

    // Stage 13: ID Assignment
    timed_stage!(timer, "Stage 13 (ID Assignment)", state, {
        id_assignment::assign_ids(&mut state.pages);
    });
    log::info!(
        "Stage 13 (ID Assignment) complete: {} elements",
        state.total_elements()
    );

    // Stage 14: Caption Linking
    timed_stage!(timer, "Stage 14 (Caption Linking)", state, {
        caption_linker::link_captions(&mut state.pages);
    });
    log::info!(
        "Stage 14 (Caption Linking) complete: {} elements",
        state.total_elements()
    );

    // Stage 14b: Footnote Detection
    timed_stage!(timer, "Stage 14b (Footnote Detection)", state, {
        footnote_detector::detect_footnotes(&mut state.pages);
    });
    log::info!(
        "Stage 14b (Footnote Detection) complete: {} elements",
        state.total_elements()
    );

    // Stage 14c: TOC Detection
    timed_stage!(timer, "Stage 14c (TOC Detection)", state, {
        toc_detector::detect_toc(&mut state.pages);
    });
    log::info!(
        "Stage 14c (TOC Detection) complete: {} elements",
        state.total_elements()
    );
    // Stage 15: Cross-Page Table Linking
    timed_stage!(timer, "Stage 15 (Cross-Page Table Linking)", state, {
        cross_page_linker::link_cross_page_tables(&mut state.pages);
    });
    log::info!(
        "Stage 15 (Cross-Page Table Linking) complete: {} elements",
        state.total_elements()
    );
    // Stage 16: Heading Level Assignment — handled by Stage 12 (heading_detector already assigns global levels)
    // Stage 17: Nesting Level Assignment
    timed_stage!(timer, "Stage 17 (Nesting Level Assignment)", state, {
        nesting_level::assign_nesting_levels(&mut state.pages);
    });
    log::info!(
        "Stage 17 (Nesting Level Assignment) complete: {} elements",
        state.total_elements()
    );

    // Stage 18: Final Reading Order Sorting (after all semantic classification)
    timed_stage!(timer, "Stage 18 (Reading Order)", state, {
        reading_order::sort_reading_order(&mut state.pages, &state.page_info);
    });
    log::info!(
        "Stage 18 (Reading Order) complete: {} elements",
        state.total_elements()
    );

    // Stage 19: Content Sanitization
    timed_stage!(timer, "Stage 19 (Content Sanitization)", state, {
        content_sanitizer::sanitize_content(&mut state.pages, state.config.sanitize);
    });
    log::info!(
        "Stage 19 (Content Sanitization) complete: {} elements",
        state.total_elements()
    );
    // Stage 20: Output Generation — to be implemented

    if let Some(timer) = timer.as_ref() {
        timer.log_summary();
    }

    log::info!("Pipeline complete");
    Ok(())
}

fn pipeline_timing_enabled() -> bool {
    std::env::var("EDGEPARSE_PIPELINE_TIMING")
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

/// Replace U+FFFD (Unicode replacement character) in a content element's text.
/// Matches the reference `TextProcessor.replaceUndefinedCharacters()`.
fn replace_fffd_in_element(elem: &mut ContentElement, replacement: &str) {
    if let ContentElement::TextChunk(c) = elem {
        if c.value.contains('\u{FFFD}') {
            c.value = c.value.replace('\u{FFFD}', replacement);
        }
    } // Only TextChunks exist at Stage 2 (before line grouping)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::config::ProcessingConfig;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    #[test]
    fn test_pipeline_state() {
        let state = PipelineState::new(vec![vec![], vec![]], ProcessingConfig::default());
        assert_eq!(state.pages.len(), 2);
        assert_eq!(state.total_elements(), 0);
    }

    #[test]
    fn test_run_empty_pipeline() {
        let mut state = PipelineState::new(vec![], ProcessingConfig::default());
        let result = run_pipeline(&mut state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_content_filter_uses_real_page_geometry() {
        let chunk = ContentElement::TextChunk(TextChunk {
            value: "Right column".to_string(),
            bbox: BoundingBox::new(Some(1), 800.0, 400.0, 900.0, 420.0),
            font_name: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "[0.0]".to_string(),
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
        });
        let page_info = vec![PageInfo {
            index: 0,
            page_number: 1,
            media_box: BoundingBox::new(None, 0.0, 0.0, 960.0, 540.0),
            crop_box: BoundingBox::new(None, 0.0, 0.0, 960.0, 540.0),
            rotation: 0,
            width: 960.0,
            height: 540.0,
        }];

        let mut state = PipelineState::new(vec![vec![chunk]], ProcessingConfig::default())
            .with_page_info(page_info);
        run_pipeline(&mut state).unwrap();

        assert!(state.total_elements() > 0);
    }
}
