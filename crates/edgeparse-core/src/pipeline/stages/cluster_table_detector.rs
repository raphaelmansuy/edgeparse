//! Stage 7b: Cluster (Borderless) Table Detection
//!
//! Detects tables without visible grid lines by analysing spatial alignment of
//! text chunks across all TextBlocks on a page.
//! Modelled after the reference `ClusterTableConsumer → TableRecognitionArea`.
//!
//! **Algorithm summary**
//! 1. Collect every TextChunk from every TextLine in every TextBlock.
//! 2. Group chunks by baseline Y (same horizontal text line).
//! 3. Within each baseline group, find *column segments* by detecting large
//!    horizontal gaps (gap > COLUMN_GAP_FACTOR × font_size).
//!    Threshold from the reference implementation: DEFAULT_FONT_CHAR_SPACING_INTERVAL[1] +
//!    FONT_WHITESPACE_COMPARISON_THRESHOLD = 0.67 + 0.33 = 1.0.
//! 4. Baseline groups with ≥ MIN_COLUMNS segments → row candidates.
//! 5. Consecutive row candidates with consistent column structure → table.
//! 6. Validate and emit TableBorder elements.

use std::collections::HashSet;

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::models::table::{
    TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType,
};

// ── Constants ────────────────────────────────────────────────────────────────

/// Minimum gap / max_font_size to consider two adjacent chunks in different
/// column segments. the reference implementation: 0.67 + 0.33 = 1.0.
const COLUMN_GAP_FACTOR: f64 = 1.0;

/// Baseline tolerance for "same row" — fraction of font size.
const ONE_LINE_TOLERANCE: f64 = 0.9;

/// Maximum vertical gap (× font size) before the recognition area is broken.
const TABLE_GAP_FACTOR: f64 = 3.0;

/// Minimum column segments to form a table row.
const MIN_COLUMNS: usize = 2;

/// Minimum data rows (excluding header).
const MIN_DATA_ROWS: usize = 2;
/// Strong multi-column layouts can legitimately contain a header plus a single
/// data row (for example benchmark summary tables with one measured row).
const MIN_DATA_ROWS_STRONG_LAYOUT: usize = 1;

/// Minimum matching cells per data row.
const MIN_CELLS_PER_ROW: usize = 2;

/// Column alignment tolerance in font-size units for data row segment matching.
const COL_ALIGN_TOLERANCE: f64 = 2.0;

/// Maximum table rows (real tables rarely exceed this; prevents false positives
/// from two-column page layouts being detected as tables).
const MAX_TABLE_ROWS: usize = 30;

/// Minimum fraction of data rows that must have the SAME segment count as the
/// header row.  Real tables have very consistent column structure (>60%); two-
/// column prose produces variable segment counts per "row".
const MIN_CONSISTENT_ROW_FRACTION: f64 = 0.5;
/// Maximum word count for any single cell segment.  Segments with more than
/// this many whitespace-separated words are paragraph text (flowing prose),
/// not table cells.  Two-column academic body text commonly produces segments
/// of 7-12 words; real table cells are short (a number, a name, a few words).
/// The reference implementation avoids this problem because its cluster-table detection works at the
/// raw-TextChunk level (before paragraph formation); Rust works at the
/// TextBlock level, so inter-column gaps in a 2-column layout look like table
/// column gaps.  Reduced from 5→4 to catch more 2-column false positives
/// where short academic lines have 4-5 words per column.
const MAX_CELL_WORDS: usize = 4;
/// Maximum words allowed in the long value cell of a structured 2-column
/// key/value row.  Real descriptor tables often place a short field name on the
/// left and a sentence fragment on the right.
const MAX_KEY_VALUE_CELL_WORDS: usize = 40;
/// Maximum total word count across ALL cells in a table.  Real borderless
/// tables contain short labels and numbers (total < 200 words).  Two-column
/// academic layouts that slip past the per-cell filter accumulate hundreds of
/// words.
const MAX_TABLE_TOTAL_WORDS: usize = 200;
/// Higher allowance for genuinely wide tables with explanatory cells.
const MAX_WIDE_TABLE_TOTAL_WORDS: usize = 900;
/// Higher allowance for structured 2-column key/value tables.
const MAX_KEY_VALUE_TABLE_TOTAL_WORDS: usize = 500;
/// Maximum fraction of page area a cluster table may cover.  Tables that span
/// nearly the entire page are almost certainly false positives — the cluster
/// detector has absorbed a whole page of body text.  The reference pre-filter
/// (`areSuspiciousTextChunks`) prevents this from ever happening by only
/// considering pages where chunks exhibit side-by-side alignment gaps ≥ 3×chunk-height.
const MAX_PAGE_COVERAGE: f64 = 0.65;
/// Wide, multi-row tables can legitimately occupy most of a landscape page.
const MAX_WIDE_TABLE_PAGE_COVERAGE: f64 = 0.78;

/// Minimum columns for 2-column table — 2-column borderless tables that contain
/// prose (sentences with punctuation) are almost always false positives from
/// side-by-side text columns.  Require 3+ columns when the average cell text
/// looks like prose.
const MIN_COLUMNS_PROSE_GUARD: usize = 3;

/// Average cell text length (chars) above which a 2-column table is suspected
/// to be a 2-column text layout rather than a real table.
const PROSE_AVG_CELL_LENGTH: f64 = 15.0;
/// Header baseline alignment probability threshold (reference: HEADERS_PROBABILITY_THRESHOLD).
const HEADERS_PROB_THRESHOLD: f64 = 0.75;

// ── Public entry-point ────────────────────────────────────────────────────────

/// Public entry-point — processes one page of elements.
pub fn detect_cluster_tables(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    if elements.is_empty() {
        return elements;
    }

    // Collect every non-empty TextChunk from every TextLine in every TextBlock.
    let mut all_chunks: Vec<ChunkRef> = Vec::new();
    for (block_idx, el) in elements.iter().enumerate() {
        match el {
            ContentElement::TextBlock(block) => {
                if block.is_hidden_text {
                    continue;
                }
                for line in &block.text_lines {
                    if line.is_hidden_text {
                        continue;
                    }
                    collect_line_chunks(&mut all_chunks, line, block_idx);
                }
            }
            ContentElement::TextLine(line) => {
                if line.is_hidden_text {
                    continue;
                }
                collect_line_chunks(&mut all_chunks, line, block_idx);
            }
            _ => {}
        }
    }

    if all_chunks.len() < MIN_COLUMNS * 2 {
        return elements;
    }

    // Group chunks by baseline → find row candidates.
    let row_candidates = group_chunks_into_row_candidates(&all_chunks);

    // Detect tables from chunk alignment first.
    let mut tables = if row_candidates.len() > MIN_DATA_ROWS_STRONG_LAYOUT {
        find_cluster_tables(&row_candidates)
    } else {
        Vec::new()
    };
    let occupied_indices: HashSet<usize> = tables
        .iter()
        .flat_map(|ct| ct.consumed_block_indices.iter().copied())
        .collect();
    tables.extend(find_flow_key_value_tables(&elements, &occupied_indices));
    let occupied_indices: HashSet<usize> = tables
        .iter()
        .flat_map(|ct| ct.consumed_block_indices.iter().copied())
        .collect();
    tables.extend(find_parallel_flow_key_value_tables(
        &elements,
        &occupied_indices,
    ));
    let occupied_indices: HashSet<usize> = tables
        .iter()
        .flat_map(|ct| ct.consumed_block_indices.iter().copied())
        .collect();
    tables.extend(find_caption_compact_two_column_tables(
        &elements,
        &occupied_indices,
    ));
    let tables = augment_panel_cluster_tables(&elements, tables);
    let tables = augment_grouped_header_cluster_tables(&elements, tables);

    if tables.is_empty() {
        return elements;
    }

    // Page-coverage guard: reject cluster tables that cover too much of the page.
    // Compute page extents from all elements on this page.
    let page_min_x = elements
        .iter()
        .map(|e| e.bbox().left_x)
        .fold(f64::MAX, f64::min);
    let page_max_x = elements
        .iter()
        .map(|e| e.bbox().right_x)
        .fold(f64::MIN, f64::max);
    let page_min_y = elements
        .iter()
        .map(|e| e.bbox().bottom_y)
        .fold(f64::MAX, f64::min);
    let page_max_y = elements
        .iter()
        .map(|e| e.bbox().top_y)
        .fold(f64::MIN, f64::max);
    let page_area = (page_max_x - page_min_x).max(1.0) * (page_max_y - page_min_y).max(1.0);

    let tables: Vec<ClusterTable> = tables
        .into_iter()
        .filter(|ct| {
            let tb = &ct.table_border;
            let tb_area = tb.bbox.width().max(0.0) * tb.bbox.height().max(0.0);
            let coverage = tb_area / page_area;
            coverage <= max_page_coverage_for_table(tb) && !is_sparse_ocr_layout_table(tb)
        })
        .collect();

    if tables.is_empty() {
        return elements;
    }

    // Collect all consumed indices.
    let mut consumed: Vec<bool> = vec![false; elements.len()];
    let mut new_table_borders: Vec<(usize, TableBorder)> = Vec::new();

    for ct in &tables {
        for &idx in &ct.consumed_block_indices {
            consumed[idx] = true;
        }
        // Insert table at the position of the first consumed paragraph.
        let insert_pos = ct.consumed_block_indices.iter().copied().min().unwrap_or(0);
        new_table_borders.push((insert_pos, ct.table_border.clone()));
    }

    // Sort tables by insert position.
    new_table_borders.sort_by_key(|(pos, _)| *pos);

    // Rebuild element list.
    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());
    let mut table_iter = new_table_borders.into_iter().peekable();

    for (i, elem) in elements.into_iter().enumerate() {
        // Insert tables at their designated positions.
        while let Some(&(pos, _)) = table_iter.peek() {
            if pos == i {
                let (_, tb) = table_iter.next().unwrap();
                result.push(ContentElement::TableBorder(tb));
            } else {
                break;
            }
        }
        if !consumed[i] {
            result.push(elem);
        }
    }
    // Any remaining tables (shouldn't happen but defensive).
    for (_, tb) in table_iter {
        result.push(ContentElement::TableBorder(tb));
    }

    result
}

// ── Internal types ──────────────────────────────────────────────────────────

/// A single text chunk extracted from a TextLine, with its source block index.
struct ChunkRef {
    left_x: f64,
    right_x: f64,
    /// Baseline from the parent TextLine (more accurate than chunk bbox.bottom_y).
    baseline: f64,
    font_size: f64,
    text: String,
    page_number: Option<u32>,
    block_index: usize,
}

/// A group of adjacent TextChunks with no large horizontal gap between them.
/// Corresponds to one "cell" in a table row.
#[derive(Debug, Clone)]
struct CellSegment {
    left_x: f64,
    right_x: f64,
    baseline: f64,
    font_size: f64,
    /// Concatenated text of all chunks in this segment.
    text: String,
    page_number: Option<u32>,
    /// Source TextBlock indices.
    block_indices: Vec<usize>,
}

/// A row of column segments — one horizontal line that looks like a table row.
struct RowCandidate {
    baseline: f64,
    font_size: f64,
    /// Segments sorted left-to-right.
    segments: Vec<CellSegment>,
    /// All source TextBlock indices referenced by segments in this row.
    block_indices: Vec<usize>,
}

/// Column boundary derived from the header row.
#[derive(Debug, Clone)]
struct ColumnBound {
    left: f64,
    right: f64,
}

/// A recognised borderless table.
struct ClusterTable {
    consumed_block_indices: Vec<usize>,
    table_border: TableBorder,
}

#[derive(Clone)]
#[allow(dead_code)]
struct PanelLine {
    bbox: BoundingBox,
    baseline: f64,
    font_size: f64,
    chunks: Vec<crate::models::chunks::TextChunk>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct PanelFragment {
    slot_idx: usize,
    bbox: BoundingBox,
    text: String,
}

#[derive(Clone)]
struct PanelRow {
    bbox: BoundingBox,
    cells: Vec<String>,
}

struct FlowCell {
    text: String,
    bbox: BoundingBox,
}

struct FlowRow {
    left: FlowCell,
    right: Option<FlowCell>,
    consumed_indices: Vec<usize>,
}

// ── Core algorithm ──────────────────────────────────────────────────────────

/// Group all chunks by baseline proximity, then split each group into column
/// segments by detecting large horizontal gaps.
fn group_chunks_into_row_candidates(chunks: &[ChunkRef]) -> Vec<RowCandidate> {
    if chunks.is_empty() {
        return Vec::new();
    }

    // Sort by baseline descending (top-of-page first), then left_x ascending.
    let mut sorted: Vec<&ChunkRef> = chunks.iter().collect();
    sorted.sort_by(|a, b| {
        b.baseline
            .partial_cmp(&a.baseline)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.left_x
                    .partial_cmp(&b.left_x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    // Group consecutive chunks by baseline proximity.
    let mut baseline_groups: Vec<Vec<&ChunkRef>> = Vec::new();
    let mut group_baselines: Vec<f64> = Vec::new();
    let mut group_font_sizes: Vec<f64> = Vec::new();

    for chunk in &sorted {
        let mut placed = false;
        for (gi, grp) in baseline_groups.iter_mut().enumerate() {
            let tol = ONE_LINE_TOLERANCE * chunk.font_size.min(group_font_sizes[gi]);
            if (group_baselines[gi] - chunk.baseline).abs() < tol {
                grp.push(chunk);
                placed = true;
                break;
            }
        }
        if !placed {
            baseline_groups.push(vec![chunk]);
            group_baselines.push(chunk.baseline);
            group_font_sizes.push(chunk.font_size);
        }
    }

    // For each baseline group, sort by left_x and split into column segments.
    let mut row_candidates: Vec<RowCandidate> = Vec::new();

    for (gi, group) in baseline_groups.iter().enumerate() {
        let mut sorted_group = group.to_vec();
        sorted_group.sort_by(|a, b| {
            a.left_x
                .partial_cmp(&b.left_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let font_size = group_font_sizes[gi];
        let segments = split_into_segments(&sorted_group, font_size);

        if segments.len() < MIN_COLUMNS {
            continue;
        }

        if !is_viable_row_candidate(&segments) {
            continue;
        }

        let mut block_indices: Vec<usize> = segments
            .iter()
            .flat_map(|s| s.block_indices.iter().copied())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        block_indices.sort_unstable();

        row_candidates.push(RowCandidate {
            baseline: group_baselines[gi],
            font_size,
            segments,
            block_indices,
        });
    }

    // Sort row candidates by baseline descending (top-to-bottom).
    row_candidates.sort_by(|a, b| {
        b.baseline
            .partial_cmp(&a.baseline)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    row_candidates
}

/// Split a baseline-aligned, left-to-right sorted slice of chunks into column
/// segments by detecting large horizontal gaps.
fn split_into_segments(chunks: &[&ChunkRef], font_size: f64) -> Vec<CellSegment> {
    if chunks.is_empty() {
        return Vec::new();
    }

    let mut segments: Vec<CellSegment> = Vec::new();
    let mut seg_parts: Vec<String> = vec![chunks[0].text.clone()];
    let mut seg_left = chunks[0].left_x;
    let mut seg_right = chunks[0].right_x;
    let mut seg_baseline = chunks[0].baseline;
    let mut seg_font_size = chunks[0].font_size;
    let mut seg_page = chunks[0].page_number;
    let mut seg_blocks: Vec<usize> = vec![chunks[0].block_index];

    for chunk in &chunks[1..] {
        let gap = chunk.left_x - seg_right;
        let max_font = seg_font_size.max(chunk.font_size).max(font_size).max(1.0);

        if gap > COLUMN_GAP_FACTOR * max_font {
            // Flush current segment.
            segments.push(CellSegment {
                left_x: seg_left,
                right_x: seg_right,
                baseline: seg_baseline,
                font_size: seg_font_size,
                text: seg_parts.join(" "),
                page_number: seg_page,
                block_indices: seg_blocks.clone(),
            });
            // Start a new segment.
            seg_parts = vec![chunk.text.clone()];
            seg_left = chunk.left_x;
            seg_right = chunk.right_x;
            seg_baseline = chunk.baseline;
            seg_font_size = chunk.font_size;
            seg_page = chunk.page_number;
            seg_blocks = vec![chunk.block_index];
        } else {
            // Extend current segment.
            seg_parts.push(chunk.text.clone());
            seg_right = seg_right.max(chunk.right_x);
            if !seg_blocks.contains(&chunk.block_index) {
                seg_blocks.push(chunk.block_index);
            }
        }
    }

    // Flush the last segment.
    segments.push(CellSegment {
        left_x: seg_left,
        right_x: seg_right,
        baseline: seg_baseline,
        font_size: seg_font_size,
        text: seg_parts.join(" "),
        page_number: seg_page,
        block_indices: seg_blocks,
    });

    segments
}

fn find_flow_key_value_tables(
    elements: &[ContentElement],
    occupied_indices: &HashSet<usize>,
) -> Vec<ClusterTable> {
    let mut tables = Vec::new();
    let mut i = 0usize;

    while i < elements.len() {
        if occupied_indices.contains(&i) {
            i += 1;
            continue;
        }

        let Some(first_row) = make_flow_row_candidate(elements, i, occupied_indices) else {
            i += 1;
            continue;
        };
        if !is_flow_label_text(&first_row.left.text) {
            i += 1;
            continue;
        }

        let mut rows = vec![first_row];
        let mut j = rows[0].consumed_indices.iter().copied().max().unwrap_or(i) + 1;

        while j < elements.len() {
            if occupied_indices.contains(&j) {
                break;
            }
            let Some(row) = make_flow_row_candidate(elements, j, occupied_indices) else {
                break;
            };
            if !is_flow_row_compatible(&rows[0], &row) {
                break;
            }
            j = row.consumed_indices.iter().copied().max().unwrap_or(j) + 1;
            rows.push(row);
        }

        if let Some(table) = build_flow_key_value_table(&rows) {
            tables.push(table);
            i = j;
        } else {
            i += 1;
        }
    }

    tables
}

fn find_parallel_flow_key_value_tables(
    elements: &[ContentElement],
    occupied_indices: &HashSet<usize>,
) -> Vec<ClusterTable> {
    let mut labels: Vec<(usize, String, BoundingBox)> = elements
        .iter()
        .enumerate()
        .filter(|(idx, _)| !occupied_indices.contains(idx))
        .filter_map(|(idx, elem)| {
            let text = element_text(elem)?;
            let bbox = elem.bbox().clone();
            (is_flow_label_text(&text) && bbox.width() <= 220.0).then_some((idx, text, bbox))
        })
        .collect();

    if labels.len() < 4 {
        return Vec::new();
    }

    let left_anchor = match median(labels.iter().map(|(_, _, bbox)| bbox.left_x).collect()) {
        Some(anchor) => anchor,
        None => return Vec::new(),
    };
    labels.retain(|(_, _, bbox)| (bbox.left_x - left_anchor).abs() <= 28.0);
    if labels.len() < 4 {
        return Vec::new();
    }
    labels.sort_by(|a, b| {
        b.2.top_y
            .partial_cmp(&a.2.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let min_right_threshold = labels
        .iter()
        .map(|(_, _, bbox)| bbox.right_x)
        .fold(f64::MIN, f64::max)
        .min(left_anchor + 180.0);

    let mut rows = Vec::new();
    let mut used_right_indices = HashSet::new();

    for (pos, (label_idx, label_text, label_bbox)) in labels.iter().enumerate() {
        let band_top = label_bbox.top_y + 18.0;
        let band_bottom = labels
            .get(pos + 1)
            .map(|(_, _, next_bbox)| next_bbox.top_y + 6.0)
            .unwrap_or(label_bbox.bottom_y - 120.0);

        let mut right_texts = Vec::new();
        let mut right_indices = Vec::new();
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut page_number = label_bbox.page_number;

        for (idx, elem) in elements.iter().enumerate() {
            if occupied_indices.contains(&idx)
                || used_right_indices.contains(&idx)
                || idx == *label_idx
            {
                continue;
            }

            let Some(text) = element_text(elem) else {
                continue;
            };
            let bbox = elem.bbox();
            if bbox.left_x < min_right_threshold || bbox.left_x < label_bbox.right_x + 18.0 {
                continue;
            }

            let center_y = bbox.center_y();
            if center_y > band_top || center_y <= band_bottom {
                continue;
            }

            right_texts.push(text.trim().to_string());
            right_indices.push(idx);
            min_x = min_x.min(bbox.left_x);
            min_y = min_y.min(bbox.bottom_y);
            max_x = max_x.max(bbox.right_x);
            max_y = max_y.max(bbox.top_y);
            page_number = page_number.or(bbox.page_number);
        }

        if right_texts.is_empty() {
            continue;
        }

        let joined = right_texts.join(" ");
        if joined.split_whitespace().count() < 2 {
            continue;
        }

        for idx in &right_indices {
            used_right_indices.insert(*idx);
        }
        let mut consumed_indices = vec![*label_idx];
        consumed_indices.extend(right_indices.iter().copied());
        rows.push(FlowRow {
            left: FlowCell {
                text: label_text.clone(),
                bbox: label_bbox.clone(),
            },
            right: Some(FlowCell {
                text: joined,
                bbox: BoundingBox::new(page_number, min_x, min_y, max_x, max_y),
            }),
            consumed_indices,
        });
    }

    if let Some(table) = build_flow_key_value_table(&rows) {
        vec![table]
    } else {
        Vec::new()
    }
}

fn find_caption_compact_two_column_tables(
    elements: &[ContentElement],
    occupied_indices: &HashSet<usize>,
) -> Vec<ClusterTable> {
    let mut tables = Vec::new();

    for caption_idx in 0..elements.len() {
        if occupied_indices.contains(&caption_idx) {
            continue;
        }
        let Some(caption_text) = element_text(&elements[caption_idx]) else {
            continue;
        };
        if !looks_like_table_caption(&caption_text) {
            continue;
        }

        let Some(table) =
            build_caption_compact_two_column_table(elements, caption_idx, occupied_indices)
        else {
            continue;
        };
        tables.push(table);
    }

    tables
}

fn build_caption_compact_two_column_table(
    elements: &[ContentElement],
    caption_idx: usize,
    occupied_indices: &HashSet<usize>,
) -> Option<ClusterTable> {
    let caption_bbox = elements.get(caption_idx)?.bbox();
    let mut candidates = Vec::new();

    for (idx, elem) in elements.iter().enumerate().skip(caption_idx + 1) {
        if occupied_indices.contains(&idx) {
            break;
        }
        let bbox = elem.bbox();
        if caption_bbox.bottom_y - bbox.top_y > 160.0 {
            break;
        }
        let Some(text) = element_text(elem) else {
            continue;
        };
        candidates.push((idx, text, bbox.clone()));
        if candidates.len() >= 5 {
            break;
        }
    }

    if candidates.len() < 2 {
        return None;
    }

    let (
        header_left_idx,
        header_left_text,
        header_left_bbox,
        header_right_idx,
        header_right_text,
        header_right_bbox,
        mut data_start,
    ) = if let Some((left, right)) =
        split_compact_header_row(&candidates[0].1, candidates[0].2.width())
    {
        (
            candidates[0].0,
            left.to_string(),
            BoundingBox::new(
                candidates[0].2.page_number,
                candidates[0].2.left_x,
                candidates[0].2.bottom_y,
                candidates[0].2.left_x + candidates[0].2.width() * 0.28,
                candidates[0].2.top_y,
            ),
            candidates[0].0,
            right.to_string(),
            BoundingBox::new(
                candidates[0].2.page_number,
                candidates[0].2.left_x + candidates[0].2.width() * 0.30,
                candidates[0].2.bottom_y,
                candidates[0].2.right_x,
                candidates[0].2.top_y,
            ),
            1usize,
        )
    } else {
        if candidates.len() < 3 {
            return None;
        }
        let (header_left_idx, header_left_text, header_left_bbox) = &candidates[0];
        let (header_right_idx, header_right_text, header_right_bbox) = &candidates[1];
        if !is_compact_header_text(header_left_text) || !is_compact_header_text(header_right_text) {
            return None;
        }
        if (header_left_bbox.bottom_y - header_right_bbox.bottom_y).abs() > 12.0 {
            return None;
        }
        if header_right_bbox.left_x <= header_left_bbox.right_x + 8.0 {
            return None;
        }
        (
            *header_left_idx,
            header_left_text.clone(),
            header_left_bbox.clone(),
            *header_right_idx,
            header_right_text.clone(),
            header_right_bbox.clone(),
            2usize,
        )
    };

    let mut unit_text = None;
    if let Some((_, text, bbox)) = candidates.get(data_start) {
        if bbox.left_x >= header_right_bbox.left_x - 8.0
            && text.split_whitespace().count() <= 2
            && !text.chars().any(|c| c.is_ascii_digit())
        {
            unit_text = Some((text.clone(), bbox.clone()));
            data_start += 1;
        }
    }

    let (_, data_text, data_bbox) = candidates.get(data_start)?;
    let mut rows = vec![(header_left_text.clone(), Some(header_right_text.clone()))];
    if let Some((unit, _)) = &unit_text {
        rows.push((String::new(), Some(unit.clone())));
    }
    if let Some(pairs) = split_label_value_pairs(data_text) {
        if pairs.len() < 3 {
            return None;
        }
        rows.extend(pairs.into_iter().map(|(left, right)| (left, Some(right))));
    } else if let Some(labels) = split_label_only_series(data_text) {
        if labels.len() < 3 {
            return None;
        }
        rows.extend(labels.into_iter().map(|left| (left, Some(String::new()))));
    } else {
        return None;
    }

    let page_number = header_left_bbox
        .page_number
        .or(header_right_bbox.page_number);
    let col_split = (header_left_bbox.right_x + header_right_bbox.left_x) / 2.0;
    let left_x = header_left_bbox.left_x.min(data_bbox.left_x);
    let right_x = header_right_bbox.right_x.max(data_bbox.right_x);

    let row_height = (data_bbox.height() / (rows.len().saturating_sub(1).max(1) as f64)).max(10.0);
    let table_top = header_left_bbox.top_y.max(header_right_bbox.top_y);
    let table_bottom = data_bbox.bottom_y;
    let mut y_coords = vec![table_top];
    while y_coords.len() < rows.len() {
        let next = (table_top - row_height * y_coords.len() as f64).max(table_bottom);
        y_coords.push(next);
    }
    y_coords.push(table_bottom);

    let x_coords = vec![left_x, col_split, right_x];
    let mut border_rows = Vec::new();
    for (ri, (left_text, right_text)) in rows.iter().enumerate() {
        let row_top = y_coords[ri];
        let row_bottom = y_coords[ri + 1];
        let left_bbox =
            BoundingBox::new(page_number, x_coords[0], row_bottom, x_coords[1], row_top);
        let right_bbox =
            BoundingBox::new(page_number, x_coords[1], row_bottom, x_coords[2], row_top);
        let left_tokens = if !left_text.trim().is_empty() {
            vec![make_text_token(left_text, &left_bbox)]
        } else {
            Vec::new()
        };
        let left_contents = left_tokens
            .iter()
            .map(|token| ContentElement::TextChunk(token.base.clone()))
            .collect();
        let right_tokens = right_text
            .as_ref()
            .map(|text| vec![make_text_token(text, &right_bbox)])
            .unwrap_or_default();
        let right_contents = right_tokens
            .iter()
            .map(|token| ContentElement::TextChunk(token.base.clone()))
            .collect();
        border_rows.push(TableBorderRow {
            bbox: BoundingBox::new(page_number, x_coords[0], row_bottom, x_coords[2], row_top),
            index: None,
            level: None,
            row_number: ri,
            cells: vec![
                TableBorderCell {
                    bbox: left_bbox.clone(),
                    index: None,
                    level: None,
                    row_number: ri,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: left_tokens,
                    contents: left_contents,
                    semantic_type: None,
                },
                TableBorderCell {
                    bbox: right_bbox.clone(),
                    index: None,
                    level: None,
                    row_number: ri,
                    col_number: 1,
                    row_span: 1,
                    col_span: 1,
                    content: right_tokens,
                    contents: right_contents,
                    semantic_type: None,
                },
            ],
            semantic_type: None,
        });
    }

    let mut consumed_indices = vec![header_left_idx, header_right_idx, candidates[data_start].0];
    consumed_indices.sort_unstable();
    consumed_indices.dedup();

    Some(ClusterTable {
        consumed_block_indices: consumed_indices,
        table_border: TableBorder {
            bbox: BoundingBox::new(page_number, left_x, table_bottom, right_x, table_top),
            index: None,
            level: Some("1".to_string()),
            x_coordinates: x_coords.clone(),
            x_widths: vec![0.0; x_coords.len()],
            y_coordinates: y_coords.clone(),
            y_widths: vec![0.0; y_coords.len()],
            rows: border_rows,
            num_rows: rows.len(),
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        },
    })
}

fn make_flow_row_candidate(
    elements: &[ContentElement],
    idx: usize,
    occupied_indices: &HashSet<usize>,
) -> Option<FlowRow> {
    let elem = elements.get(idx)?;
    let left_bbox = elem.bbox();
    let left_text = element_text(elem)?;

    if let Some((label, value)) = split_merged_key_value_text(&left_text, left_bbox.width()) {
        return Some(FlowRow {
            left: FlowCell {
                text: label.to_string(),
                bbox: BoundingBox::new(
                    left_bbox.page_number,
                    left_bbox.left_x,
                    left_bbox.bottom_y,
                    left_bbox.left_x + left_bbox.width() * 0.28,
                    left_bbox.top_y,
                ),
            },
            right: Some(FlowCell {
                text: value.to_string(),
                bbox: BoundingBox::new(
                    left_bbox.page_number,
                    left_bbox.left_x + left_bbox.width() * 0.30,
                    left_bbox.bottom_y,
                    left_bbox.right_x,
                    left_bbox.top_y,
                ),
            }),
            consumed_indices: vec![idx],
        });
    }

    if !is_flow_label_text(&left_text) {
        return None;
    }

    let mut consumed = vec![idx];
    let mut right_cell = None;
    let mut best_right: Option<(f64, usize, FlowCell)> = None;
    for look_ahead in 1..=12 {
        let next_idx = idx + look_ahead;
        if next_idx >= elements.len() || occupied_indices.contains(&next_idx) {
            continue;
        }
        let next = &elements[next_idx];
        let next_bbox = next.bbox();
        let vertical_gap = left_bbox.bottom_y - next_bbox.top_y;
        let overlap =
            left_bbox.top_y.min(next_bbox.top_y) - left_bbox.bottom_y.max(next_bbox.bottom_y);
        let is_rightish = next_bbox.left_x >= left_bbox.right_x + 20.0;
        let is_near = overlap >= -12.0 || vertical_gap.abs() <= 28.0;
        if is_rightish && is_near {
            if let Some(text) = element_text(next) {
                let candidate = FlowCell {
                    text,
                    bbox: next_bbox.clone(),
                };
                let score =
                    vertical_gap.abs() + (next_bbox.left_x - left_bbox.right_x).abs() * 0.05;
                match &best_right {
                    Some((best_score, _, _)) if *best_score <= score => {}
                    _ => best_right = Some((score, next_idx, candidate)),
                }
            }
        }
    }
    if let Some((_, next_idx, cell)) = best_right {
        right_cell = Some(cell);
        consumed.push(next_idx);
    } else if let Some((cell, extra_indices)) =
        collect_indented_flow_value(elements, idx, occupied_indices)
    {
        right_cell = Some(cell);
        consumed.extend(extra_indices);
    }

    Some(FlowRow {
        left: FlowCell {
            text: left_text,
            bbox: left_bbox.clone(),
        },
        right: right_cell,
        consumed_indices: consumed,
    })
}

fn collect_indented_flow_value(
    elements: &[ContentElement],
    idx: usize,
    occupied_indices: &HashSet<usize>,
) -> Option<(FlowCell, Vec<usize>)> {
    let label = elements.get(idx)?;
    let label_bbox = label.bbox();
    let label_text = element_text(label)?;
    if !is_flow_label_text(&label_text) {
        return None;
    }

    let mut texts = Vec::new();
    let mut consumed = Vec::new();
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut page_number = label_bbox.page_number;
    let mut anchor_left = None::<f64>;
    let mut prev_bottom = label_bbox.bottom_y;

    for (next_idx, next) in elements.iter().enumerate().skip(idx + 1).take(7) {
        if occupied_indices.contains(&next_idx) {
            break;
        }

        let next_bbox = next.bbox();
        let Some(text) = element_text(next) else {
            if !texts.is_empty() {
                break;
            }
            continue;
        };

        if is_flow_label_text(&text) && (next_bbox.left_x - label_bbox.left_x).abs() <= 28.0 {
            break;
        }

        let vertical_gap = prev_bottom - next_bbox.top_y;
        if !(-18.0..=120.0).contains(&vertical_gap) {
            if !texts.is_empty() {
                break;
            }
            continue;
        }

        let is_indented = next_bbox.left_x >= label_bbox.left_x + 80.0
            || next_bbox.left_x >= label_bbox.right_x + 18.0;
        if !is_indented {
            if !texts.is_empty() {
                break;
            }
            continue;
        }

        if let Some(anchor) = anchor_left {
            if (next_bbox.left_x - anchor).abs() > 36.0 {
                break;
            }
        } else {
            anchor_left = Some(next_bbox.left_x);
        }

        texts.push(text.trim().to_string());
        consumed.push(next_idx);
        min_x = min_x.min(next_bbox.left_x);
        min_y = min_y.min(next_bbox.bottom_y);
        max_x = max_x.max(next_bbox.right_x);
        max_y = max_y.max(next_bbox.top_y);
        page_number = page_number.or(next_bbox.page_number);
        prev_bottom = next_bbox.bottom_y;
    }

    if texts.is_empty() {
        return None;
    }

    let joined = texts.join(" ");
    if joined.split_whitespace().count() < 4 {
        return None;
    }

    Some((
        FlowCell {
            text: joined,
            bbox: BoundingBox::new(page_number, min_x, min_y, max_x, max_y),
        },
        consumed,
    ))
}

fn build_flow_key_value_table(rows: &[FlowRow]) -> Option<ClusterTable> {
    if rows.len() < 4 {
        return None;
    }

    let paired_rows = rows.iter().filter(|r| r.right.is_some()).count();
    if paired_rows < 3 {
        return None;
    }

    let left_xs: Vec<f64> = rows.iter().map(|r| r.left.bbox.left_x).collect();
    let left_rights: Vec<f64> = rows.iter().map(|r| r.left.bbox.right_x).collect();
    let right_lefts: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.right.as_ref().map(|c| c.bbox.left_x))
        .collect();
    let right_rights: Vec<f64> = rows
        .iter()
        .filter_map(|r| r.right.as_ref().map(|c| c.bbox.right_x))
        .collect();

    if right_lefts.len() < 3 {
        return None;
    }

    let left_anchor = median(left_xs)?;
    let left_boundary = median(left_rights)?;
    let right_anchor = median(right_lefts)?;
    let right_boundary = median(right_rights)?;

    if right_anchor <= left_boundary + 20.0 {
        return None;
    }

    if rows
        .iter()
        .filter(|row| (row.left.bbox.left_x - left_anchor).abs() <= 24.0)
        .count()
        * 10
        < rows.len() * 7
    {
        return None;
    }
    if rows
        .iter()
        .filter(|row| {
            row.right
                .as_ref()
                .is_none_or(|cell| (cell.bbox.left_x - right_anchor).abs() <= 40.0)
        })
        .count()
        * 10
        < rows.len() * 7
    {
        return None;
    }

    let min_y = rows
        .iter()
        .map(|r| {
            r.left.bbox.bottom_y.min(
                r.right
                    .as_ref()
                    .map_or(r.left.bbox.bottom_y, |c| c.bbox.bottom_y),
            )
        })
        .fold(f64::MAX, f64::min);
    let max_y = rows
        .iter()
        .map(|r| {
            r.left
                .bbox
                .top_y
                .max(r.right.as_ref().map_or(r.left.bbox.top_y, |c| c.bbox.top_y))
        })
        .fold(f64::MIN, f64::max);
    let page_number = rows.iter().find_map(|r| {
        r.left
            .bbox
            .page_number
            .or(r.right.as_ref().and_then(|c| c.bbox.page_number))
    });

    let mut y_coords = Vec::with_capacity(rows.len() + 1);
    y_coords.push(max_y);
    for pair in rows.windows(2) {
        let upper = pair[0].left.bbox.bottom_y.min(
            pair[0]
                .right
                .as_ref()
                .map_or(pair[0].left.bbox.bottom_y, |c| c.bbox.bottom_y),
        );
        let lower = pair[1].left.bbox.top_y.max(
            pair[1]
                .right
                .as_ref()
                .map_or(pair[1].left.bbox.top_y, |c| c.bbox.top_y),
        );
        y_coords.push((upper + lower) / 2.0);
    }
    y_coords.push(min_y);

    let x_coords = vec![
        left_anchor,
        (left_boundary + right_anchor) / 2.0,
        right_boundary,
    ];
    let mut border_rows = Vec::with_capacity(rows.len());
    for (ri, row) in rows.iter().enumerate() {
        let row_top = y_coords[ri];
        let row_bottom = y_coords[ri + 1];
        let mut cells = Vec::with_capacity(2);
        let left_token = make_text_token(&row.left.text, &row.left.bbox);
        let right_tokens = row
            .right
            .as_ref()
            .map(|cell| vec![make_text_token(&cell.text, &cell.bbox)])
            .unwrap_or_default();
        let right_contents = right_tokens
            .iter()
            .map(|token| ContentElement::TextChunk(token.base.clone()))
            .collect();

        cells.push(TableBorderCell {
            bbox: BoundingBox::new(page_number, x_coords[0], row_bottom, x_coords[1], row_top),
            index: None,
            level: None,
            row_number: ri,
            col_number: 0,
            row_span: 1,
            col_span: 1,
            content: vec![left_token.clone()],
            contents: vec![ContentElement::TextChunk(left_token.base)],
            semantic_type: None,
        });
        cells.push(TableBorderCell {
            bbox: BoundingBox::new(page_number, x_coords[1], row_bottom, x_coords[2], row_top),
            index: None,
            level: None,
            row_number: ri,
            col_number: 1,
            row_span: 1,
            col_span: 1,
            content: right_tokens,
            contents: right_contents,
            semantic_type: None,
        });

        border_rows.push(TableBorderRow {
            bbox: BoundingBox::new(page_number, x_coords[0], row_bottom, x_coords[2], row_top),
            index: None,
            level: None,
            row_number: ri,
            cells,
            semantic_type: None,
        });
    }

    let mut consumed: Vec<usize> = rows
        .iter()
        .flat_map(|r| r.consumed_indices.iter().copied())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    consumed.sort_unstable();

    Some(ClusterTable {
        consumed_block_indices: consumed,
        table_border: TableBorder {
            bbox: BoundingBox::new(page_number, x_coords[0], min_y, x_coords[2], max_y),
            index: None,
            level: Some("1".to_string()),
            x_coordinates: x_coords.clone(),
            x_widths: vec![0.0; x_coords.len()],
            y_coordinates: y_coords.clone(),
            y_widths: vec![0.0; y_coords.len()],
            rows: border_rows,
            num_rows: rows.len(),
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        },
    })
}

fn is_flow_row_compatible(first: &FlowRow, candidate: &FlowRow) -> bool {
    let left_delta = (candidate.left.bbox.left_x - first.left.bbox.left_x).abs();
    let right_delta = match (&first.right, &candidate.right) {
        (Some(a), Some(b)) => (b.bbox.left_x - a.bbox.left_x).abs(),
        _ => 0.0,
    };
    let gap = first.left.bbox.bottom_y - candidate.left.bbox.top_y;

    left_delta <= 24.0 && right_delta <= 40.0 && gap <= 220.0
}

fn is_flow_label_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains(['.', '?', '!']) {
        return false;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.is_empty() || words.len() > 4 {
        return false;
    }

    words.iter().all(|word| {
        let lower = word.to_ascii_lowercase();
        matches!(lower.as_str(), "and" | "of" | "to" | "in" | "&")
            || word
                .chars()
                .next()
                .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || c == '#')
    })
}

fn split_merged_key_value_text(text: &str, width: f64) -> Option<(&str, &str)> {
    if width < 260.0 {
        return None;
    }

    let trimmed = text.trim();
    let mut split_idx = None;
    let mut cursor = 0usize;
    for (i, word) in trimmed.split_whitespace().enumerate() {
        if i >= 4 {
            break;
        }
        if i > 0 && looks_like_sentence_start(word) {
            split_idx = Some(cursor.saturating_sub(1));
            break;
        }
        cursor += word.len() + 1;
    }

    let idx = split_idx?;
    let (left, right) = trimmed.split_at(idx);
    let left = left.trim();
    let right = right.trim();
    if !is_flow_label_text(left) || right.split_whitespace().count() < 4 {
        return None;
    }
    Some((left, right))
}

fn is_compact_header_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains(['.', '?', '!']) {
        return false;
    }

    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if !(2..=8).contains(&words.len()) {
        return false;
    }

    let alpha_words = words
        .iter()
        .filter(|word| word.chars().any(|c| c.is_alphabetic()))
        .count();
    if alpha_words < 2 {
        return false;
    }

    let numeric_words = words
        .iter()
        .filter(|word| word.chars().any(|c| c.is_ascii_digit()))
        .count();
    let starts_like_header = trimmed
        .chars()
        .find(|c| c.is_alphanumeric())
        .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit());

    starts_like_header
        && numeric_words <= 2
        && trimmed.chars().count() <= 64
        && !trimmed.contains(':')
}

fn split_compact_header_row(text: &str, width: f64) -> Option<(&str, &str)> {
    if let Some((left, right)) = split_merged_key_value_text(text, width) {
        if is_compact_header_text(left) && is_compact_header_text(right) {
            return Some((left, right));
        }
    }

    let trimmed = text.trim();
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.len() < 4 {
        return None;
    }

    let mut cursor = 0usize;
    for split_word_idx in 1..words.len() {
        cursor += words[split_word_idx - 1].len() + 1;
        let (left, right) = trimmed.split_at(cursor.saturating_sub(1));
        let left = left.trim();
        let right = right.trim();
        if !is_compact_header_text(left) || !is_compact_header_text(right) {
            continue;
        }

        let left_words = left.split_whitespace().count();
        let right_words = right.split_whitespace().count();
        let right_starts_title = right
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit());
        if right_starts_title && left_words >= 2 && right_words >= 2 {
            return Some((left, right));
        }
    }

    None
}

fn looks_like_sentence_start(word: &str) -> bool {
    matches!(
        word,
        "To" | "Provides"
            | "Provide"
            | "Relative"
            | "Select"
            | "Choose"
            | "Connect"
            | "Create"
            | "Monitor"
            | "Viewing"
            | "Guide"
            | "Using"
            | "The"
    )
}

fn looks_like_table_caption(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() <= 120
        && trimmed
            .get(..6)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("table "))
}

fn split_label_value_pairs(text: &str) -> Option<Vec<(String, String)>> {
    let mut current_label = Vec::new();
    let mut pairs = Vec::new();

    for token in text.split_whitespace() {
        if token.chars().any(|c| c.is_ascii_digit()) {
            if current_label.is_empty() {
                return None;
            }
            pairs.push((current_label.join(" "), token.to_string()));
            current_label.clear();
        } else {
            current_label.push(token);
        }
    }

    if !current_label.is_empty() || pairs.is_empty() {
        return None;
    }

    Some(pairs)
}

fn split_label_only_series(text: &str) -> Option<Vec<String>> {
    let labels: Vec<String> = text
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| {
            token
                .trim_matches(|c: char| c == ',' || c == ';')
                .to_string()
        })
        .collect();

    if labels.len() < 3 || labels.len() > 12 {
        return None;
    }
    if !labels.iter().all(|token| {
        let word_count = token.split_whitespace().count();
        word_count == 1
            && token.chars().count() <= 18
            && token.chars().any(|c| c.is_alphanumeric())
            && !token.contains(['.', '?', '!'])
    }) {
        return None;
    }

    Some(labels)
}

fn element_text(elem: &ContentElement) -> Option<String> {
    match elem {
        ContentElement::TextBlock(block) => {
            let text = block.value();
            (!text.trim().is_empty()).then_some(text)
        }
        ContentElement::List(list) => {
            let text = list
                .list_items
                .iter()
                .map(list_item_text)
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            (!text.trim().is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn list_item_text(item: &crate::models::list::ListItem) -> String {
    let from_body = item
        .body
        .content
        .iter()
        .flat_map(|row| row.iter())
        .map(|token| token.base.value.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if !from_body.is_empty() {
        return from_body;
    }

    item.contents
        .iter()
        .map(|elem| match elem {
            ContentElement::Paragraph(p) => p.base.value(),
            ContentElement::TextBlock(tb) => tb.value(),
            ContentElement::TextLine(tl) => tl.value(),
            ContentElement::TextChunk(tc) => tc.value.clone(),
            _ => String::new(),
        })
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(values[values.len() / 2])
}

fn is_viable_row_candidate(segments: &[CellSegment]) -> bool {
    if segments.len() < MIN_COLUMNS {
        return false;
    }

    let max_words = segments
        .iter()
        .map(|s| s.text.split_whitespace().count())
        .max()
        .unwrap_or(0);

    if max_words <= MAX_CELL_WORDS {
        return true;
    }

    if segments.len() >= 4 {
        // 4+ aligned columns are already a strong table signal; wide tables
        // regularly contain sentence-like explanatory cells.
        return true;
    }

    if segments.len() == 3 {
        let total_words: usize = segments
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum();
        return max_words <= 18 && total_words <= 48;
    }

    looks_like_key_value_row(segments)
}

fn looks_like_key_value_row(segments: &[CellSegment]) -> bool {
    if segments.len() != 2 {
        return false;
    }

    let left = &segments[0];
    let right = &segments[1];
    let left_words = left.text.split_whitespace().count();
    let right_words = right.text.split_whitespace().count();

    if left_words == 0 || left_words > MAX_CELL_WORDS {
        return false;
    }
    if right_words == 0 || right_words > MAX_KEY_VALUE_CELL_WORDS {
        return false;
    }

    let left_width = (left.right_x - left.left_x).max(1.0);
    let right_width = (right.right_x - right.left_x).max(1.0);
    if left_width >= right_width {
        return false;
    }

    let left_text = left.text.trim();
    let right_text = right.text.trim();
    if left_text.is_empty() || right_text.is_empty() {
        return false;
    }
    if left_text.contains(['.', '?', '!']) {
        return false;
    }
    if right_text.split_whitespace().count() <= 2 {
        return false;
    }

    left_text
        .chars()
        .next()
        .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit())
}

/// Main table-finding loop.
fn find_cluster_tables(row_candidates: &[RowCandidate]) -> Vec<ClusterTable> {
    let n = row_candidates.len();
    let mut used: Vec<bool> = vec![false; n];
    let mut tables: Vec<ClusterTable> = Vec::new();

    for header_idx in 0..n {
        if used[header_idx] {
            continue;
        }
        let header = &row_candidates[header_idx];

        if header.segments.len() < MIN_COLUMNS {
            continue;
        }
        if !validate_header_alignment(&header.segments) {
            continue;
        }

        // Derive column bounds from header segments.
        let columns: Vec<ColumnBound> = header
            .segments
            .iter()
            .map(|s| ColumnBound {
                left: s.left_x,
                right: s.right_x,
            })
            .collect();

        // Collect data rows below the header.
        let mut table_rows: Vec<&RowCandidate> = vec![header];
        let mut table_indices: Vec<usize> = vec![header_idx];

        for ri in (header_idx + 1)..n {
            if used[ri] {
                continue;
            }
            let row = &row_candidates[ri];

            // Vertical gap check.
            let prev_baseline = table_rows.last().unwrap().baseline;
            let gap = prev_baseline - row.baseline; // positive = below in PDF coords
            let fs = row.font_size.max(1.0);

            if gap > TABLE_GAP_FACTOR * fs {
                break; // Too far below — end of table.
            }
            if gap < -ONE_LINE_TOLERANCE * fs {
                continue; // Above last row — skip.
            }

            let matches = count_matching_segments(&row.segments, &columns, fs);
            if matches >= MIN_CELLS_PER_ROW {
                table_rows.push(row);
                table_indices.push(ri);
            } else {
                break; // Gap in alignment — stop.
            }
        }

        let min_data_rows = min_data_rows_for_header(header);
        // Validate: need header + enough data rows, not too many rows.
        if table_rows.len() < 1 + min_data_rows || table_rows.len() > MAX_TABLE_ROWS {
            continue;
        }

        // All data rows must pass the cell count check.
        let all_valid = table_rows.iter().skip(1).all(|row| {
            count_matching_segments(&row.segments, &columns, row.font_size) >= MIN_CELLS_PER_ROW
        });
        if !all_valid {
            continue;
        }

        // Consistency check: at least MIN_CONSISTENT_ROW_FRACTION of data rows
        // must have the same segment count as the header.  Real tables are
        // structurally uniform; two-column prose produces variable segments.
        let header_seg_count = header.segments.len();
        let data_rows = &table_rows[1..];
        if !data_rows.is_empty() {
            let consistent = data_rows
                .iter()
                .filter(|r| {
                    r.segments.len() == header_seg_count
                        || is_wrapped_wide_continuation_row(r, &columns, header_seg_count)
                })
                .count();
            let fraction = consistent as f64 / data_rows.len() as f64;
            if fraction < MIN_CONSISTENT_ROW_FRACTION {
                continue;
            }
        }

        if let Some(ct) = build_cluster_table(&table_rows, &columns) {
            // Post-validation 1: total word count.  Real tables are concise;
            // two-column body text that slips past the per-cell filter has
            // many more words in aggregate.
            let total_words: usize = ct
                .table_border
                .rows
                .iter()
                .flat_map(|r| r.cells.iter())
                .flat_map(|c| c.content.iter())
                .map(|tok| tok.base.value.split_whitespace().count())
                .sum();
            if total_words > max_total_words_for_table(&ct.table_border) {
                continue;
            }

            // Post-validation 2: prose guard for 2-column "tables".
            // Two-column page layouts produce 2-column cluster tables where each
            // "cell" contains a paragraph fragment.  Real 2-column tables have
            // short cells (numbers, labels).  Reject 2-column tables when the
            // average cell text length suggests prose.
            if ct.table_border.num_columns < MIN_COLUMNS_PROSE_GUARD
                && !is_key_value_table(&ct.table_border)
            {
                let cell_texts: Vec<&str> = ct
                    .table_border
                    .rows
                    .iter()
                    .flat_map(|r| r.cells.iter())
                    .flat_map(|c| c.content.iter())
                    .map(|tok| tok.base.value.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                if !cell_texts.is_empty() {
                    let avg_len = cell_texts.iter().map(|s| s.len()).sum::<usize>() as f64
                        / cell_texts.len() as f64;
                    if avg_len > PROSE_AVG_CELL_LENGTH {
                        continue;
                    }
                }
            }

            for &idx in &table_indices {
                used[idx] = true;
            }
            tables.push(ct);
        }
    }

    tables
}

fn collect_line_chunks(
    all_chunks: &mut Vec<ChunkRef>,
    line: &crate::models::text::TextLine,
    block_index: usize,
) {
    let line_baseline = line.base_line;
    let line_font_size = line.font_size.max(1.0);
    for chunk in &line.text_chunks {
        let text = chunk.value.trim().to_string();
        if text.is_empty() {
            continue;
        }
        all_chunks.push(ChunkRef {
            left_x: chunk.bbox.left_x,
            right_x: chunk.bbox.right_x,
            baseline: line_baseline,
            font_size: chunk.font_size.max(line_font_size).max(1.0),
            text,
            page_number: chunk.bbox.page_number,
            block_index,
        });
    }
}

/// Check that all header segments share approximately the same baseline.
fn validate_header_alignment(segments: &[CellSegment]) -> bool {
    if segments.len() < MIN_COLUMNS {
        return false;
    }
    let avg_bl: f64 = segments.iter().map(|s| s.baseline).sum::<f64>() / segments.len() as f64;
    let avg_fs: f64 = segments.iter().map(|s| s.font_size).sum::<f64>() / segments.len() as f64;
    let max_dev = segments
        .iter()
        .map(|s| (s.baseline - avg_bl).abs() / avg_fs.max(1.0))
        .fold(0.0_f64, f64::max);
    (1.0 - max_dev) > HEADERS_PROB_THRESHOLD
}

fn min_data_rows_for_header(header: &RowCandidate) -> usize {
    if header.segments.len() >= 4 {
        return MIN_DATA_ROWS_STRONG_LAYOUT;
    }

    MIN_DATA_ROWS
}

fn max_total_words_for_table(table: &TableBorder) -> usize {
    if table.num_columns >= 4 {
        return MAX_WIDE_TABLE_TOTAL_WORDS;
    }
    if is_key_value_table(table) {
        return MAX_KEY_VALUE_TABLE_TOTAL_WORDS;
    }

    MAX_TABLE_TOTAL_WORDS
}

fn max_page_coverage_for_table(table: &TableBorder) -> f64 {
    if table.num_columns >= 4 && table.num_rows >= 4 {
        return MAX_WIDE_TABLE_PAGE_COVERAGE;
    }

    MAX_PAGE_COVERAGE
}

fn is_key_value_table(table: &TableBorder) -> bool {
    if table.num_columns != 2 || table.num_rows < 3 {
        return false;
    }

    let mut matched_rows = 0usize;
    for row in &table.rows {
        if row.cells.len() < 2 {
            continue;
        }
        let left = cell_text(&row.cells[0]);
        let right = cell_text(&row.cells[1]);
        if is_key_value_cell_pair(&left, &right) {
            matched_rows += 1;
        }
    }

    matched_rows * 10 >= table.num_rows * 6
}

fn is_sparse_ocr_layout_table(table: &TableBorder) -> bool {
    if table.num_rows < 3 || table.num_columns < 2 {
        return false;
    }

    let mut total_cells = 0usize;
    let mut non_empty_cells = 0usize;
    let mut occupied_columns = vec![0usize; table.num_columns];
    let mut rows_with_multiple_cells = 0usize;
    let mut total_tokens = 0usize;
    let mut ocr_tokens = 0usize;
    let mut numeric_tokens = 0usize;
    let mut verbose_cells = 0usize;

    for row in &table.rows {
        let mut row_non_empty = 0usize;
        for (col_idx, cell) in row.cells.iter().enumerate() {
            total_cells += 1;
            let text = cell_text(cell);
            if text.is_empty() {
                continue;
            }
            non_empty_cells += 1;
            row_non_empty += 1;
            if let Some(count) = occupied_columns.get_mut(col_idx) {
                *count += 1;
            }
            if text.split_whitespace().count() >= 5 {
                verbose_cells += 1;
            }

            for token in &cell.content {
                let value = token.base.value.trim();
                if value.is_empty() {
                    continue;
                }
                total_tokens += 1;
                if token.base.font_name == "OCR" {
                    ocr_tokens += 1;
                }
                if value.chars().any(|ch| ch.is_ascii_digit()) {
                    numeric_tokens += 1;
                }
            }
        }
        if row_non_empty >= 2 {
            rows_with_multiple_cells += 1;
        }
    }

    if total_cells == 0 || non_empty_cells == 0 || total_tokens == 0 {
        return false;
    }

    let occupied_column_count = occupied_columns.iter().filter(|count| **count > 0).count();
    let repeated_columns = occupied_columns.iter().filter(|count| **count >= 2).count();
    let dominant_column_fill = occupied_columns.iter().copied().max().unwrap_or(0);

    let occupancy_ratio = non_empty_cells as f64 / total_cells as f64;
    let ocr_ratio = ocr_tokens as f64 / total_tokens as f64;
    let numeric_ratio = numeric_tokens as f64 / total_tokens as f64;
    let dominant_column_ratio = dominant_column_fill as f64 / non_empty_cells as f64;
    let verbose_ratio = verbose_cells as f64 / non_empty_cells as f64;

    ocr_ratio >= 0.85
        && occupancy_ratio <= 0.55
        && numeric_ratio <= 0.35
        && rows_with_multiple_cells >= 2
        && (repeated_columns < 3
            || occupied_column_count <= 2
            || dominant_column_ratio >= 0.62
            || verbose_ratio >= 0.35)
}

fn is_key_value_cell_pair(left: &str, right: &str) -> bool {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() || right.is_empty() {
        return false;
    }

    let left_words = left.split_whitespace().count();
    let right_words = right.split_whitespace().count();
    if left_words == 0 || left_words > MAX_CELL_WORDS || right_words <= 2 {
        return false;
    }

    if left.contains(['.', '?', '!']) {
        return false;
    }

    left.chars()
        .next()
        .is_some_and(|c| c.is_uppercase() || c.is_ascii_digit())
}

/// Count how many data-row segments align to any header column bound.
fn count_matching_segments(
    segments: &[CellSegment],
    columns: &[ColumnBound],
    font_size: f64,
) -> usize {
    let tol = COL_ALIGN_TOLERANCE * font_size.max(1.0);
    let mut count = 0usize;
    for seg in segments {
        let seg_center = (seg.left_x + seg.right_x) / 2.0;
        for col in columns {
            // Match by center within extended column bounds, or by left-x proximity.
            if seg_center >= col.left - tol && seg_center <= col.right + tol {
                count += 1;
                break;
            }
            if (seg.left_x - col.left).abs() <= tol {
                count += 1;
                break;
            }
        }
    }
    count
}

fn is_wrapped_wide_continuation_row(
    row: &RowCandidate,
    columns: &[ColumnBound],
    header_seg_count: usize,
) -> bool {
    if header_seg_count < 4 || row.segments.len() >= header_seg_count || row.segments.len() < 2 {
        return false;
    }

    let first_col = match columns.first() {
        Some(col) => col,
        None => return false,
    };
    let tol = COL_ALIGN_TOLERANCE * row.font_size.max(1.0);
    if row
        .segments
        .iter()
        .any(|seg| segment_matches_column(seg, first_col, tol))
    {
        return false;
    }

    row.segments.iter().all(|seg| {
        columns
            .iter()
            .skip(1)
            .any(|col| segment_matches_column(seg, col, tol))
    })
}

fn segment_matches_column(seg: &CellSegment, col: &ColumnBound, tol: f64) -> bool {
    let seg_center = (seg.left_x + seg.right_x) / 2.0;
    (seg_center >= col.left - tol && seg_center <= col.right + tol)
        || (seg.left_x - col.left).abs() <= tol
}

/// Build a TableBorder from the validated rows and column bounds.
fn build_cluster_table(
    table_rows: &[&RowCandidate],
    columns: &[ColumnBound],
) -> Option<ClusterTable> {
    let num_rows = table_rows.len();
    let num_cols = columns.len();

    if num_rows < 2 || num_cols < MIN_COLUMNS {
        return None;
    }

    // Compute bounding box and collect consumed block indices.
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_font_size = 1.0f64;
    let mut page_number: Option<u32> = None;
    let mut all_block_indices: HashSet<usize> = HashSet::new();

    for row in table_rows {
        for seg in &row.segments {
            min_x = min_x.min(seg.left_x);
            max_x = max_x.max(seg.right_x);
            min_y = min_y.min(seg.baseline);
            max_font_size = max_font_size.max(seg.font_size);
            if page_number.is_none() {
                page_number = seg.page_number;
            }
        }
        for &idx in &row.block_indices {
            all_block_indices.insert(idx);
        }
    }

    // Estimate top_y of first row from its font size.
    let max_y = table_rows
        .first()
        .map(|r| r.baseline + r.font_size * 1.2)
        .unwrap_or(min_y + max_font_size * 1.2);

    // Build column X-coordinates.
    let mut x_coords: Vec<f64> = vec![min_x];
    for i in 0..num_cols.saturating_sub(1) {
        let mid = (columns[i].right + columns[i + 1].left) / 2.0;
        x_coords.push(mid);
    }
    x_coords.push(max_x);

    // Build row Y-coordinates.
    let mut y_coords: Vec<f64> = vec![max_y];
    for i in 0..num_rows.saturating_sub(1) {
        let mid = (table_rows[i].baseline + table_rows[i + 1].baseline) / 2.0;
        y_coords.push(mid);
    }
    y_coords.push(min_y);

    // Build TableBorderRows with TableBorderCells.
    let mut border_rows: Vec<TableBorderRow> = Vec::with_capacity(num_rows);
    for (ri, row) in table_rows.iter().enumerate() {
        let row_top = y_coords[ri];
        let row_bottom = y_coords[ri + 1];
        let fs = row.font_size.max(1.0);
        let tol = COL_ALIGN_TOLERANCE * fs;

        let mut cells: Vec<TableBorderCell> = Vec::with_capacity(num_cols);
        for ci in 0..num_cols {
            let cell_left = x_coords[ci];
            let cell_right = x_coords[ci + 1];
            let cell_bbox =
                BoundingBox::new(page_number, cell_left, row_bottom, cell_right, row_top);

            let col = &columns[ci];
            let mut cell_tokens: Vec<TableToken> = Vec::new();

            for seg in &row.segments {
                let seg_center = (seg.left_x + seg.right_x) / 2.0;
                let matched = (seg_center >= col.left - tol && seg_center <= col.right + tol)
                    || (seg.left_x - col.left).abs() <= tol;
                if matched {
                    cell_tokens.push(make_token(seg));
                }
            }
            let cell_contents = cell_tokens
                .iter()
                .map(|token| ContentElement::TextChunk(token.base.clone()))
                .collect();

            cells.push(TableBorderCell {
                bbox: cell_bbox,
                index: None,
                level: None,
                row_number: ri,
                col_number: ci,
                row_span: 1,
                col_span: 1,
                content: cell_tokens,
                contents: cell_contents,
                semantic_type: None,
            });
        }

        let row_bbox = BoundingBox::new(page_number, min_x, row_bottom, max_x, row_top);
        border_rows.push(TableBorderRow {
            bbox: row_bbox,
            index: None,
            level: None,
            row_number: ri,
            cells,
            semantic_type: None,
        });
    }

    let table_bbox = BoundingBox::new(page_number, min_x, min_y, max_x, max_y);
    let table_border = TableBorder {
        bbox: table_bbox,
        index: None,
        level: Some("1".to_string()),
        x_coordinates: x_coords.clone(),
        x_widths: vec![0.0; x_coords.len()],
        y_coordinates: y_coords.clone(),
        y_widths: vec![0.0; y_coords.len()],
        rows: border_rows,
        num_rows,
        num_columns: num_cols,
        is_bad_table: false,
        is_table_transformer: false,
        previous_table: None,
        next_table: None,
    };

    let mut consumed: Vec<usize> = all_block_indices.into_iter().collect();
    consumed.sort_unstable();

    Some(ClusterTable {
        consumed_block_indices: consumed,
        table_border,
    })
}

fn augment_panel_cluster_tables(
    elements: &[ContentElement],
    tables: Vec<ClusterTable>,
) -> Vec<ClusterTable> {
    tables
        .into_iter()
        .map(|table| augment_panel_cluster_table(elements, &table).unwrap_or(table))
        .collect()
}

fn augment_grouped_header_cluster_tables(
    elements: &[ContentElement],
    tables: Vec<ClusterTable>,
) -> Vec<ClusterTable> {
    tables
        .into_iter()
        .map(|table| augment_grouped_header_cluster_table(elements, &table).unwrap_or(table))
        .collect()
}

fn augment_panel_cluster_table(
    elements: &[ContentElement],
    table: &ClusterTable,
) -> Option<ClusterTable> {
    if table.table_border.num_columns < 3 || table.consumed_block_indices.is_empty() {
        return None;
    }

    let band_indices = collect_panel_band_indices(elements, table)?;
    let slot_ranges = derive_panel_slot_ranges(elements, &band_indices, &table.table_border)?;
    if slot_ranges.len() != table.table_border.num_columns + 1 {
        return None;
    }

    let mut rows = reconstruct_panel_rows(elements, &band_indices, &slot_ranges);
    if rows.len() < table.table_border.num_rows {
        return None;
    }
    merge_panel_stub_companion_rows(&mut rows);
    merge_panel_continuation_rows(&mut rows);
    if rows.len() < 3 {
        return None;
    }

    let header_like_rows = rows
        .iter()
        .take(2)
        .filter(|row| {
            row.cells
                .iter()
                .skip(1)
                .filter(|cell| !cell.trim().is_empty())
                .count()
                >= slot_ranges.len().saturating_sub(2)
        })
        .count();
    let stub_rows = rows
        .iter()
        .filter(|row| !row.cells[0].trim().is_empty())
        .count();
    if header_like_rows == 0 || stub_rows < 2 {
        return None;
    }

    let x_coords = slot_ranges
        .iter()
        .map(|(left, _)| *left)
        .chain(slot_ranges.last().map(|(_, right)| *right))
        .collect::<Vec<_>>();
    let y_coords = build_panel_y_coordinates(&rows);
    let page_number = table.table_border.bbox.page_number;
    let min_x = *x_coords.first()?;
    let max_x = *x_coords.last()?;
    let max_y = *y_coords.first()?;
    let min_y = *y_coords.last()?;

    let mut border_rows = Vec::with_capacity(rows.len());
    for (row_idx, row) in rows.iter().enumerate() {
        let row_top = y_coords[row_idx];
        let row_bottom = y_coords[row_idx + 1];
        let mut cells = Vec::with_capacity(slot_ranges.len());
        for (col_idx, cell_text) in row.cells.iter().enumerate() {
            let bbox = BoundingBox::new(
                page_number,
                slot_ranges[col_idx].0,
                row_bottom,
                slot_ranges[col_idx].1,
                row_top,
            );
            let content = if cell_text.trim().is_empty() {
                Vec::new()
            } else {
                vec![make_text_token(cell_text.trim(), &bbox)]
            };
            let contents = content
                .iter()
                .map(|token| ContentElement::TextChunk(token.base.clone()))
                .collect();
            cells.push(TableBorderCell {
                bbox,
                index: None,
                level: None,
                row_number: row_idx,
                col_number: col_idx,
                row_span: 1,
                col_span: 1,
                content,
                contents,
                semantic_type: None,
            });
        }
        border_rows.push(TableBorderRow {
            bbox: BoundingBox::new(page_number, min_x, row_bottom, max_x, row_top),
            index: None,
            level: None,
            row_number: row_idx,
            cells,
            semantic_type: None,
        });
    }

    Some(ClusterTable {
        consumed_block_indices: band_indices,
        table_border: TableBorder {
            bbox: BoundingBox::new(page_number, min_x, min_y, max_x, max_y),
            index: None,
            level: Some("1".to_string()),
            x_coordinates: x_coords.clone(),
            x_widths: vec![0.0; x_coords.len()],
            y_coordinates: y_coords.clone(),
            y_widths: vec![0.0; y_coords.len()],
            rows: border_rows,
            num_rows: rows.len(),
            num_columns: slot_ranges.len(),
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        },
    })
}

fn augment_grouped_header_cluster_table(
    elements: &[ContentElement],
    table: &ClusterTable,
) -> Option<ClusterTable> {
    if table.table_border.num_columns < 3 || table.consumed_block_indices.is_empty() {
        return None;
    }

    let header_indices = collect_grouped_header_band_indices(elements, table)?;
    if header_indices.is_empty() {
        return None;
    }

    let slot_ranges = table
        .table_border
        .x_coordinates
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .collect::<Vec<_>>();
    if slot_ranges.len() != table.table_border.num_columns {
        return None;
    }

    let header_rows = reconstruct_panel_rows(elements, &header_indices, &slot_ranges);
    if header_rows.is_empty() || header_rows.len() > 3 {
        return None;
    }

    let max_header_fill = header_rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .filter(|cell| !cell.trim().is_empty())
                .count()
        })
        .max()
        .unwrap_or(0);
    if max_header_fill < 2 {
        return None;
    }

    let existing_rows = grouped_table_rows(table);
    if existing_rows.is_empty() {
        return None;
    }
    if header_rows.iter().any(|header| {
        existing_rows
            .first()
            .is_some_and(|row| row.cells == header.cells)
    }) {
        return None;
    }

    let mut rows = header_rows;
    rows.extend(existing_rows);
    let x_coords = table.table_border.x_coordinates.clone();
    let y_coords = build_panel_y_coordinates(&rows);
    let page_number = table.table_border.bbox.page_number;
    let min_x = *x_coords.first()?;
    let max_x = *x_coords.last()?;
    let max_y = *y_coords.first()?;
    let min_y = *y_coords.last()?;

    let mut border_rows = Vec::with_capacity(rows.len());
    for (row_idx, row) in rows.iter().enumerate() {
        let row_top = y_coords[row_idx];
        let row_bottom = y_coords[row_idx + 1];
        let mut cells = Vec::with_capacity(slot_ranges.len());
        for (col_idx, cell_text) in row.cells.iter().enumerate() {
            let bbox = BoundingBox::new(
                page_number,
                slot_ranges[col_idx].0,
                row_bottom,
                slot_ranges[col_idx].1,
                row_top,
            );
            let content = if cell_text.trim().is_empty() {
                Vec::new()
            } else {
                vec![make_text_token(cell_text.trim(), &bbox)]
            };
            let contents = content
                .iter()
                .map(|token| ContentElement::TextChunk(token.base.clone()))
                .collect();
            cells.push(TableBorderCell {
                bbox,
                index: None,
                level: None,
                row_number: row_idx,
                col_number: col_idx,
                row_span: 1,
                col_span: 1,
                content,
                contents,
                semantic_type: None,
            });
        }
        border_rows.push(TableBorderRow {
            bbox: BoundingBox::new(page_number, min_x, row_bottom, max_x, row_top),
            index: None,
            level: None,
            row_number: row_idx,
            cells,
            semantic_type: None,
        });
    }

    let mut consumed = table.consumed_block_indices.clone();
    consumed.extend(header_indices);
    consumed.sort_unstable();
    consumed.dedup();

    Some(ClusterTable {
        consumed_block_indices: consumed,
        table_border: TableBorder {
            bbox: BoundingBox::new(page_number, min_x, min_y, max_x, max_y),
            index: None,
            level: table.table_border.level.clone(),
            x_coordinates: x_coords.clone(),
            x_widths: vec![0.0; x_coords.len()],
            y_coordinates: y_coords.clone(),
            y_widths: vec![0.0; y_coords.len()],
            rows: border_rows,
            num_rows: rows.len(),
            num_columns: slot_ranges.len(),
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        },
    })
}

fn collect_grouped_header_band_indices(
    elements: &[ContentElement],
    table: &ClusterTable,
) -> Option<Vec<usize>> {
    let start_idx = *table.consumed_block_indices.iter().min()?;
    let page_number = table.table_border.bbox.page_number;
    let table_top = table.table_border.bbox.top_y;
    let row_pitch =
        (table.table_border.bbox.height() / table.table_border.num_rows.max(1) as f64).max(8.0);

    let mut indices = Vec::new();
    let mut cursor = start_idx;
    while let Some(prev_idx) = cursor.checked_sub(1) {
        let elem = elements.get(prev_idx)?;
        if !is_panel_text_candidate(elem) || elem.bbox().page_number != page_number {
            break;
        }
        let gap = elem.bbox().bottom_y - table_top;
        if !(-row_pitch..=row_pitch * 3.5).contains(&gap) {
            break;
        }
        indices.push(prev_idx);
        cursor = prev_idx;
        if indices.len() >= 6 {
            break;
        }
    }
    indices.reverse();
    Some(indices)
}

fn grouped_table_rows(table: &ClusterTable) -> Vec<PanelRow> {
    table
        .table_border
        .rows
        .iter()
        .map(|row| {
            let mut cells = vec![String::new(); table.table_border.num_columns];
            for cell in &row.cells {
                if cell.col_number < cells.len() {
                    cells[cell.col_number] = cell_text(cell);
                }
            }
            PanelRow {
                bbox: row.bbox.clone(),
                cells,
            }
        })
        .collect()
}

fn collect_panel_band_indices(
    elements: &[ContentElement],
    table: &ClusterTable,
) -> Option<Vec<usize>> {
    let start_idx = *table.consumed_block_indices.iter().min()?;
    let end_idx = *table.consumed_block_indices.iter().max()?;
    let page_number = table.table_border.bbox.page_number;
    let table_top = table.table_border.bbox.top_y;
    let table_bottom = table.table_border.bbox.bottom_y;
    let row_pitch =
        (table.table_border.bbox.height() / table.table_border.num_rows.max(1) as f64).max(10.0);

    let mut indices = Vec::new();
    let mut cursor = start_idx;
    while let Some(prev_idx) = cursor.checked_sub(1) {
        let elem = elements.get(prev_idx)?;
        if !is_panel_text_candidate(elem) || elem.bbox().page_number != page_number {
            break;
        }
        let gap = elem.bbox().bottom_y - table_top;
        if !(-row_pitch..=row_pitch * 6.0).contains(&gap) {
            break;
        }
        indices.push(prev_idx);
        cursor = prev_idx;
        if indices.len() >= 12 {
            break;
        }
    }
    indices.reverse();
    indices.extend(table.consumed_block_indices.iter().copied());

    for (next_idx, elem) in elements.iter().enumerate().skip(end_idx + 1) {
        if !is_panel_text_candidate(elem) || elem.bbox().page_number != page_number {
            break;
        }
        let gap = table_bottom - elem.bbox().top_y;
        if !(-row_pitch..=row_pitch * 3.0).contains(&gap) {
            break;
        }
        indices.push(next_idx);
        if indices.len() >= table.consumed_block_indices.len() + 4 {
            break;
        }
    }

    indices.sort_unstable();
    indices.dedup();
    Some(indices)
}

fn is_panel_text_candidate(elem: &ContentElement) -> bool {
    matches!(
        elem,
        ContentElement::TextBlock(_) | ContentElement::TextLine(_)
    )
}

fn derive_panel_slot_ranges(
    elements: &[ContentElement],
    band_indices: &[usize],
    table: &TableBorder,
) -> Option<Vec<(f64, f64)>> {
    let first_left = *table.x_coordinates.first()?;
    let first_right = *table.x_coordinates.get(1)?;
    let first_width = (first_right - first_left).max(1.0);

    let mut external_stub_left = f64::INFINITY;
    let mut external_stub_right = f64::NEG_INFINITY;
    let mut stub_right = f64::NEG_INFINITY;
    let mut first_data_left = f64::INFINITY;

    for idx in band_indices {
        let elem = &elements[*idx];
        let bbox = elem.bbox();
        if bbox.right_x <= first_left + first_width * 0.08
            && bbox.left_x >= first_left - first_width * 0.9
            && bbox.width() <= first_width * 0.35
        {
            external_stub_left = external_stub_left.min(bbox.left_x);
            external_stub_right = external_stub_right.max(bbox.right_x);
        }
        if bbox.right_x <= first_left || bbox.left_x >= first_right {
            continue;
        }
        if bbox.left_x <= first_left + first_width * 0.18
            && bbox.width() <= first_width * 0.26
            && bbox.center_x() <= first_left + first_width * 0.22
        {
            stub_right = stub_right.max(bbox.right_x);
        }

        for line in extract_panel_lines(elem) {
            for chunk in line.chunks {
                if chunk.bbox.left_x >= first_right || chunk.bbox.right_x <= first_left {
                    continue;
                }
                if chunk.bbox.left_x > first_left + first_width * 0.22 {
                    first_data_left = first_data_left.min(chunk.bbox.left_x);
                }
            }
        }
    }

    if external_stub_right.is_finite() {
        let gap = first_left - external_stub_right;
        if gap >= 4.0 {
            let mut slots = vec![(external_stub_left, first_left)];
            for pair in table.x_coordinates.windows(2) {
                slots.push((pair[0], pair[1]));
            }
            return Some(slots);
        }
    }

    if !stub_right.is_finite() || !first_data_left.is_finite() {
        return None;
    }

    let split = (stub_right + first_data_left) / 2.0;
    if split <= first_left + first_width * 0.10 || split >= first_right - first_width * 0.15 {
        return None;
    }

    let mut slots = vec![(first_left, split), (split, first_right)];
    for pair in table.x_coordinates.windows(2).skip(1) {
        slots.push((pair[0], pair[1]));
    }
    Some(slots)
}

fn reconstruct_panel_rows(
    elements: &[ContentElement],
    band_indices: &[usize],
    slot_ranges: &[(f64, f64)],
) -> Vec<PanelRow> {
    let mut rows: Vec<PanelRow> = Vec::new();

    for idx in band_indices {
        for line in extract_panel_lines(&elements[*idx]) {
            let fragments = split_panel_fragments(&line, slot_ranges);
            if fragments.is_empty() {
                continue;
            }
            let filled = fragments.len();
            let row_center = line.bbox.center_y();
            let tolerance = line.font_size.max(8.0) * 0.8;
            let target = rows
                .iter()
                .position(|row| (row.bbox.center_y() - row_center).abs() <= tolerance);

            if filled == 1
                && line.bbox.width() > (slot_ranges.last().unwrap().1 - slot_ranges[0].0) * 0.65
            {
                continue;
            }

            if let Some(row_idx) = target {
                let row = &mut rows[row_idx];
                row.bbox = row.bbox.union(&line.bbox);
                for fragment in fragments {
                    append_panel_cell(&mut row.cells[fragment.slot_idx], &fragment.text);
                }
            } else {
                let mut cells = vec![String::new(); slot_ranges.len()];
                for fragment in fragments {
                    append_panel_cell(&mut cells[fragment.slot_idx], &fragment.text);
                }
                rows.push(PanelRow {
                    bbox: line.bbox.clone(),
                    cells,
                });
            }
        }
    }

    rows.sort_by(|a, b| {
        b.bbox
            .top_y
            .partial_cmp(&a.bbox.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.into_iter()
        .filter(|row| {
            let filled = row
                .cells
                .iter()
                .filter(|cell| !cell.trim().is_empty())
                .count();
            filled >= 2
                || row
                    .cells
                    .first()
                    .is_some_and(|cell| !cell.trim().is_empty())
        })
        .collect()
}

fn merge_panel_stub_companion_rows(rows: &mut Vec<PanelRow>) {
    let mut merged: Vec<PanelRow> = Vec::with_capacity(rows.len());
    let mut idx = 0usize;
    while idx < rows.len() {
        if idx + 1 < rows.len() && should_merge_panel_stub_companions(&rows[idx], &rows[idx + 1]) {
            merged.push(combine_panel_rows(&rows[idx], &rows[idx + 1]));
            idx += 2;
            continue;
        }
        merged.push(rows[idx].clone());
        idx += 1;
    }
    *rows = merged;
}

fn merge_panel_continuation_rows(rows: &mut Vec<PanelRow>) {
    let mut merged: Vec<PanelRow> = Vec::with_capacity(rows.len());
    for row in rows.drain(..) {
        let empty_stub = row.cells.first().is_some_and(|cell| cell.trim().is_empty());
        let filled_data = row
            .cells
            .iter()
            .skip(1)
            .filter(|cell| !cell.trim().is_empty())
            .count();
        if empty_stub && filled_data >= 1 {
            if let Some(prev) = merged.last_mut() {
                let gap = prev.bbox.bottom_y - row.bbox.top_y;
                let max_gap = prev.bbox.height().max(row.bbox.height()).max(8.0) * 0.75;
                if prev
                    .cells
                    .first()
                    .is_some_and(|cell| !cell.trim().is_empty())
                    && (-2.0..=max_gap).contains(&gap)
                {
                    prev.bbox = prev.bbox.union(&row.bbox);
                    for (dst, src) in prev.cells.iter_mut().zip(row.cells.iter()) {
                        append_panel_cell(dst, src);
                    }
                    continue;
                }
            }
        }
        merged.push(row);
    }
    *rows = merged;
}

fn should_merge_panel_stub_companions(upper: &PanelRow, lower: &PanelRow) -> bool {
    let upper_stub = upper
        .cells
        .first()
        .is_some_and(|cell| !cell.trim().is_empty());
    let lower_stub = lower
        .cells
        .first()
        .is_some_and(|cell| !cell.trim().is_empty());
    let upper_data = upper
        .cells
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();
    let lower_data = lower
        .cells
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();

    let complementary = (upper_stub && upper_data == 0 && !lower_stub && lower_data >= 2)
        || (!upper_stub && upper_data >= 2 && lower_stub && lower_data == 0);
    if !complementary {
        return false;
    }

    let gap = upper.bbox.bottom_y - lower.bbox.top_y;
    let max_gap = upper.bbox.height().max(lower.bbox.height()).max(8.0) * 0.75;
    (-2.0..=max_gap).contains(&gap)
}

fn combine_panel_rows(upper: &PanelRow, lower: &PanelRow) -> PanelRow {
    let mut cells = vec![String::new(); upper.cells.len().max(lower.cells.len())];
    for (idx, dst) in cells.iter_mut().enumerate() {
        if let Some(src) = upper.cells.get(idx) {
            append_panel_cell(dst, src);
        }
        if let Some(src) = lower.cells.get(idx) {
            append_panel_cell(dst, src);
        }
    }
    PanelRow {
        bbox: upper.bbox.union(&lower.bbox),
        cells,
    }
}

fn build_panel_y_coordinates(rows: &[PanelRow]) -> Vec<f64> {
    let mut y_coords = Vec::with_capacity(rows.len() + 1);
    y_coords.push(rows.first().map(|row| row.bbox.top_y).unwrap_or(0.0));
    for pair in rows.windows(2) {
        y_coords.push((pair[0].bbox.bottom_y + pair[1].bbox.top_y) / 2.0);
    }
    y_coords.push(rows.last().map(|row| row.bbox.bottom_y).unwrap_or(0.0));
    y_coords
}

fn extract_panel_lines(elem: &ContentElement) -> Vec<PanelLine> {
    match elem {
        ContentElement::TextBlock(block) => block
            .text_lines
            .iter()
            .map(|line| PanelLine {
                bbox: line.bbox.clone(),
                baseline: line.base_line,
                font_size: line.font_size.max(1.0),
                chunks: line.text_chunks.clone(),
            })
            .collect(),
        ContentElement::TextLine(line) => vec![PanelLine {
            bbox: line.bbox.clone(),
            baseline: line.base_line,
            font_size: line.font_size.max(1.0),
            chunks: line.text_chunks.clone(),
        }],
        _ => Vec::new(),
    }
}

fn split_panel_fragments(line: &PanelLine, slot_ranges: &[(f64, f64)]) -> Vec<PanelFragment> {
    let mut groups: Vec<(usize, Vec<crate::models::chunks::TextChunk>, BoundingBox)> = Vec::new();

    for chunk in line
        .chunks
        .iter()
        .filter(|chunk| !chunk.value.trim().is_empty())
        .cloned()
    {
        let slot_idx = assign_panel_slot(&chunk.bbox, slot_ranges);
        if let Some((prev_slot, prev_chunks, prev_bbox)) = groups.last_mut() {
            let gap = chunk.bbox.left_x - prev_bbox.right_x;
            if *prev_slot == slot_idx && gap <= chunk.font_size.max(6.0) * 2.4 {
                *prev_bbox = prev_bbox.union(&chunk.bbox);
                prev_chunks.push(chunk);
                continue;
            }
        }
        groups.push((slot_idx, vec![chunk.clone()], chunk.bbox.clone()));
    }

    groups
        .into_iter()
        .filter_map(|(slot_idx, chunks, bbox)| {
            let text = crate::models::text::TextLine::concatenate_chunks(&chunks);
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| PanelFragment {
                slot_idx,
                bbox,
                text: trimmed.to_string(),
            })
        })
        .collect()
}

fn assign_panel_slot(bbox: &BoundingBox, slot_ranges: &[(f64, f64)]) -> usize {
    let mut best_idx = 0usize;
    let mut best_score = f64::NEG_INFINITY;
    let center_x = bbox.center_x();

    for (idx, (left, right)) in slot_ranges.iter().enumerate() {
        let overlap = (bbox.right_x.min(*right) - bbox.left_x.max(*left)).max(0.0);
        let score = if overlap > 0.0 {
            overlap / bbox.width().max(1.0)
        } else {
            -(center_x - ((*left + *right) / 2.0)).abs()
        };
        if score > best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    best_idx
}

fn append_panel_cell(target: &mut String, fragment: &str) {
    let trimmed = fragment.trim();
    if trimmed.is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push(' ');
    }
    target.push_str(trimmed);
}

fn cell_text(cell: &TableBorderCell) -> String {
    cell.content
        .iter()
        .map(|t| t.base.value.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Create a TableToken from a CellSegment.
fn make_token(seg: &CellSegment) -> TableToken {
    TableToken {
        base: crate::models::chunks::TextChunk {
            value: seg.text.clone(),
            bbox: BoundingBox::new(
                seg.page_number,
                seg.left_x,
                seg.baseline,
                seg.right_x,
                seg.baseline + seg.font_size,
            ),
            font_name: String::new(),
            font_size: seg.font_size,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: String::new(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: crate::models::enums::TextFormat::Normal,
            text_type: crate::models::enums::TextType::Regular,
            pdf_layer: crate::models::enums::PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: seg.page_number,
            level: None,
            mcid: None,
        },
        token_type: TableTokenType::Text,
    }
}

fn make_text_token(text: &str, bbox: &BoundingBox) -> TableToken {
    TableToken {
        base: crate::models::chunks::TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: String::new(),
            font_size: bbox.height().max(1.0),
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: String::new(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: crate::models::enums::TextFormat::Normal,
            text_type: crate::models::enums::TextType::Regular,
            pdf_layer: crate::models::enums::PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: bbox.page_number,
            level: None,
            mcid: None,
        },
        token_type: TableTokenType::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::text::{TextBlock, TextLine};

    /// Create a TextChunk at given x-range and baseline.
    fn make_chunk(
        page: u32,
        left: f64,
        right: f64,
        baseline: f64,
        fs: f64,
        text: &str,
    ) -> TextChunk {
        TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(Some(page), left, baseline, right, baseline + fs),
            font_name: String::new(),
            font_size: fs,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: String::new(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: Some(page),
            level: None,
            mcid: None,
        }
    }

    /// Create a TextLine at given baseline from a slice of (left, right, text) tuples.
    fn make_line(page: u32, baseline: f64, fs: f64, cols: &[(f64, f64, &str)]) -> TextLine {
        let chunks: Vec<TextChunk> = cols
            .iter()
            .map(|&(l, r, t)| make_chunk(page, l, r, baseline, fs, t))
            .collect();
        let min_x = cols.iter().map(|c| c.0).fold(f64::MAX, f64::min);
        let max_x = cols.iter().map(|c| c.1).fold(f64::MIN, f64::max);
        TextLine {
            bbox: BoundingBox::new(Some(page), min_x, baseline, max_x, baseline + fs),
            index: None,
            level: None,
            font_size: fs,
            base_line: baseline,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: chunks,
            is_line_start: false,
            is_line_end: false,
            is_list_line: false,
            connected_line_art_label: None,
        }
    }

    /// Create a TextBlock from a single TextLine.
    fn make_block_with_line(line: TextLine) -> ContentElement {
        let bbox = line.bbox.clone();
        let fs = line.font_size;
        let bl = line.base_line;
        ContentElement::TextBlock(TextBlock {
            bbox,
            index: None,
            level: None,
            font_size: fs,
            base_line: bl,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: false,
            has_end_line: false,
            text_alignment: None,
        })
    }

    fn make_context_block(page: u32, baseline: f64, text: &str) -> ContentElement {
        make_block_with_line(make_line(page, baseline, 10.0, &[(20.0, 560.0, text)]))
    }

    fn make_cluster_table(
        page: u32,
        x_coords: &[f64],
        row_tops: &[f64],
        row_bottoms: &[f64],
        rows: &[Vec<&str>],
        consumed_block_indices: Vec<usize>,
    ) -> ClusterTable {
        let mut border_rows = Vec::new();
        for (row_idx, cells) in rows.iter().enumerate() {
            let row_top = row_tops[row_idx];
            let row_bottom = row_bottoms[row_idx];
            let mut border_cells = Vec::new();
            for (col_idx, text) in cells.iter().enumerate() {
                let bbox = BoundingBox::new(
                    Some(page),
                    x_coords[col_idx],
                    row_bottom,
                    x_coords[col_idx + 1],
                    row_top,
                );
                let content = if text.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![make_text_token(text, &bbox)]
                };
                let contents = content
                    .iter()
                    .map(|token| ContentElement::TextChunk(token.base.clone()))
                    .collect();
                border_cells.push(TableBorderCell {
                    bbox,
                    index: None,
                    level: None,
                    row_number: row_idx,
                    col_number: col_idx,
                    row_span: 1,
                    col_span: 1,
                    content,
                    contents,
                    semantic_type: None,
                });
            }
            border_rows.push(TableBorderRow {
                bbox: BoundingBox::new(
                    Some(page),
                    x_coords[0],
                    row_bottom,
                    *x_coords.last().unwrap(),
                    row_top,
                ),
                index: None,
                level: None,
                row_number: row_idx,
                cells: border_cells,
                semantic_type: None,
            });
        }

        ClusterTable {
            consumed_block_indices,
            table_border: TableBorder {
                bbox: BoundingBox::new(
                    Some(page),
                    x_coords[0],
                    *row_bottoms.last().unwrap(),
                    *x_coords.last().unwrap(),
                    row_tops[0],
                ),
                index: None,
                level: Some("1".to_string()),
                x_coordinates: x_coords.to_vec(),
                x_widths: vec![0.0; x_coords.len()],
                y_coordinates: Vec::new(),
                y_widths: Vec::new(),
                rows: border_rows,
                num_rows: rows.len(),
                num_columns: x_coords.len() - 1,
                is_bad_table: false,
                is_table_transformer: false,
                previous_table: None,
                next_table: None,
            },
        }
    }

    #[test]
    fn test_basic_cluster_table_detection() {
        // Simulate a 3-column, 3-row table.
        // Column positions: x=50-70 (col 0), x=150-200 (col 1), x=350-400 (col 2).
        // Gap between col 0 right (70) and col 1 left (150) = 80pt / fs=10 = 8.0 > 1.0 → split.
        // Gap between col 1 right (200) and col 2 left (350) = 150pt / fs=10 = 15.0 → split.
        let page = 1u32;
        let fs = 10.0;
        let cols: &[(f64, f64, &str)] = &[
            (50.0, 70.0, "H1"),
            (150.0, 200.0, "H2"),
            (350.0, 400.0, "H3"),
        ];

        // Header row at baseline 300.
        let header_line = make_line(page, 300.0, fs, cols);
        // Data row 1 at baseline 288.
        let data1 = make_line(
            page,
            288.0,
            fs,
            &[
                (52.0, 68.0, "A1"),
                (152.0, 198.0, "A2"),
                (352.0, 398.0, "A3"),
            ],
        );
        // Data row 2 at baseline 276.
        let data2 = make_line(
            page,
            276.0,
            fs,
            &[
                (51.0, 69.0, "B1"),
                (151.0, 199.0, "B2"),
                (351.0, 399.0, "B3"),
            ],
        );

        let elements = vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(header_line),
            make_block_with_line(data1),
            make_block_with_line(data2),
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ];

        let result = detect_cluster_tables(elements);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 1, "Expected exactly 1 table");

        if let Some(ContentElement::TableBorder(tb)) = result
            .iter()
            .find(|e| matches!(e, ContentElement::TableBorder(_)))
        {
            assert_eq!(tb.num_rows, 3);
            assert_eq!(tb.num_columns, 3);
        } else {
            panic!("Expected a TableBorder");
        }
    }

    #[test]
    fn test_no_table_from_single_column() {
        // Single-segment lines should NOT form a table.
        let page = 1u32;
        let fs = 10.0;
        // Each line has only ONE chunk spanning 50-300 (no internal column gap).
        let line1 = make_line(page, 300.0, fs, &[(50.0, 300.0, "Text A")]);
        let line2 = make_line(page, 288.0, fs, &[(50.0, 300.0, "Text B")]);
        let line3 = make_line(page, 276.0, fs, &[(50.0, 300.0, "Text C")]);

        let elements = vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(line1),
            make_block_with_line(line2),
            make_block_with_line(line3),
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ];
        let result = detect_cluster_tables(elements);
        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(
            table_count, 0,
            "Single-column lines should not form a table"
        );
    }

    #[test]
    fn test_narrow_separate_blocks_form_table() {
        // Classic case: each column is its own TextBlock with one TextChunk.
        // The algorithm should group them by baseline and detect the column gap.
        let page = 1u32;
        let fs = 10.0;

        // Header row: 3 separate TextBlocks at same baseline 300.
        // gaps: 150-70=80pt, 350-200=150pt — both > 10pt (1.0×fs).
        let h0 = make_block_with_line(make_line(page, 300.0, fs, &[(50.0, 70.0, "H1")]));
        let h1 = make_block_with_line(make_line(page, 300.0, fs, &[(150.0, 200.0, "H2")]));
        let h2 = make_block_with_line(make_line(page, 300.0, fs, &[(350.0, 400.0, "H3")]));
        // Data row 1.
        let d10 = make_block_with_line(make_line(page, 288.0, fs, &[(50.0, 70.0, "A1")]));
        let d11 = make_block_with_line(make_line(page, 288.0, fs, &[(150.0, 200.0, "A2")]));
        let d12 = make_block_with_line(make_line(page, 288.0, fs, &[(350.0, 400.0, "A3")]));
        // Data row 2.
        let d20 = make_block_with_line(make_line(page, 276.0, fs, &[(50.0, 70.0, "B1")]));
        let d21 = make_block_with_line(make_line(page, 276.0, fs, &[(150.0, 200.0, "B2")]));
        let d22 = make_block_with_line(make_line(page, 276.0, fs, &[(350.0, 400.0, "B3")]));

        let elements = vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            h0,
            h1,
            h2,
            d10,
            d11,
            d12,
            d20,
            d21,
            d22,
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ];
        let result = detect_cluster_tables(elements);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(
            table_count, 1,
            "Expected 1 table from narrow separate blocks"
        );

        if let Some(ContentElement::TableBorder(tb)) = result
            .iter()
            .find(|e| matches!(e, ContentElement::TableBorder(_)))
        {
            assert_eq!(tb.num_rows, 3);
            assert_eq!(tb.num_columns, 3);
        } else {
            panic!("Expected TableBorder");
        }
    }

    #[test]
    fn test_wide_four_column_single_data_row_detected() {
        let page = 1u32;
        let fs = 10.0;

        let header = make_line(
            page,
            300.0,
            fs,
            &[
                (50.0, 90.0, "Stage"),
                (150.0, 210.0, "Function"),
                (280.0, 420.0, "Explanation"),
                (470.0, 590.0, "Benefit"),
            ],
        );
        let row = make_line(
            page,
            286.0,
            fs,
            &[
                (50.0, 95.0, "1. Project"),
                (150.0, 225.0, "Creation"),
                (
                    280.0,
                    430.0,
                    "Select document type and configure deployment",
                ),
                (
                    470.0,
                    610.0,
                    "The UI improves workflow efficiency for operators",
                ),
            ],
        );

        let result = detect_cluster_tables(vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(header),
            make_block_with_line(row),
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ]);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 1, "Expected wide 4-column table");
    }

    #[test]
    fn test_wide_four_column_textline_only_table_detected() {
        let page = 1u32;
        let fs = 10.0;

        let header = make_line(
            page,
            300.0,
            fs,
            &[
                (50.0, 90.0, "Stage"),
                (150.0, 210.0, "Function"),
                (280.0, 420.0, "Explanation"),
                (470.0, 590.0, "Benefit"),
            ],
        );
        let row = make_line(
            page,
            286.0,
            fs,
            &[
                (50.0, 95.0, "1. Project"),
                (150.0, 225.0, "Creation"),
                (
                    280.0,
                    430.0,
                    "Select document type and configure deployment",
                ),
                (
                    470.0,
                    610.0,
                    "The UI improves workflow efficiency for operators",
                ),
            ],
        );

        let result = detect_cluster_tables(vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            ContentElement::TextLine(header),
            ContentElement::TextLine(row),
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ]);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(
            table_count, 1,
            "Expected wide 4-column table from standalone text lines"
        );
    }

    #[test]
    fn test_wide_four_column_wrapped_rows_detected() {
        let page = 1u32;
        let fs = 10.0;

        let result = detect_cluster_tables(vec![
            make_context_block(
                page,
                360.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(make_line(
                page,
                320.0,
                fs,
                &[
                    (50.0, 90.0, "Stage"),
                    (150.0, 210.0, "Function"),
                    (280.0, 420.0, "Explanation"),
                    (470.0, 590.0, "Benefit"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                306.0,
                fs,
                &[
                    (50.0, 95.0, "1. Project"),
                    (150.0, 225.0, "Creation and"),
                    (280.0, 430.0, "Select document type and configure"),
                    (470.0, 610.0, "The UI helps operators move quickly"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                294.0,
                fs,
                &[
                    (150.0, 205.0, "management"),
                    (280.0, 420.0, "deployment with recommended endpoints"),
                    (470.0, 610.0, "through project creation and deployment"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                282.0,
                fs,
                &[
                    (280.0, 410.0, "and model sets"),
                    (470.0, 610.0, "with better workflow efficiency"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                268.0,
                fs,
                &[
                    (50.0, 120.0, "2. Monitoring"),
                    (150.0, 230.0, "Project monitoring"),
                    (280.0, 435.0, "Monitor deployments and detect issues"),
                    (470.0, 610.0, "Teams can identify and respond faster"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                256.0,
                fs,
                &[
                    (280.0, 430.0, "including performance degradation"),
                    (470.0, 610.0, "with clear project-level indicators"),
                ],
            )),
            make_context_block(
                page,
                220.0,
                "Context below the table to simulate a real page width",
            ),
        ]);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 1, "Expected wrapped wide table");
    }

    #[test]
    fn test_two_column_key_value_table_with_long_right_cells_detected() {
        let page = 1u32;
        let fs = 10.0;

        let row1 = make_line(
            page,
            300.0,
            fs,
            &[
                (50.0, 120.0, "Competence Area"),
                (190.0, 430.0, "Recycle reuse reduce"),
            ],
        );
        let row2 = make_line(
            page,
            286.0,
            fs,
            &[
                (50.0, 135.0, "Competence Statement"),
                (
                    190.0,
                    520.0,
                    "To know the basics of the 3 Rs and their implementation",
                ),
            ],
        );
        let row3 = make_line(
            page,
            272.0,
            fs,
            &[
                (50.0, 95.0, "Knowledge"),
                (
                    190.0,
                    520.0,
                    "Understand reducing reusing recycling and waste management",
                ),
            ],
        );

        let result = detect_cluster_tables(vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(row1),
            make_block_with_line(row2),
            make_block_with_line(row3),
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ]);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 1, "Expected key/value table");
    }

    #[test]
    fn test_two_column_key_value_table_with_indented_value_runs_detected() {
        let page = 1u32;
        let fs = 10.0;

        let elements = vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(make_line(
                page,
                310.0,
                fs,
                &[(50.0, 125.0, "Competence Area")],
            )),
            make_block_with_line(make_line(page, 310.0, fs, &[(210.0, 370.0, "#1 THE 3 RS")])),
            make_block_with_line(make_line(
                page,
                292.0,
                fs,
                &[(50.0, 150.0, "Competence Statement")],
            )),
            make_block_with_line(make_line(
                page,
                292.0,
                fs,
                &[(
                    210.0,
                    520.0,
                    "To know the basics of the 3 Rs and their implementation",
                )],
            )),
            make_block_with_line(make_line(page, 270.0, fs, &[(50.0, 95.0, "Knowledge")])),
            make_block_with_line(make_line(
                page,
                270.0,
                fs,
                &[(
                    210.0,
                    520.0,
                    "To understand the meaning of reducing reusing and recycling",
                )],
            )),
            make_block_with_line(make_line(
                page,
                258.0,
                fs,
                &[(
                    210.0,
                    500.0,
                    "To understand the importance of the 3 Rs as waste management",
                )],
            )),
            make_block_with_line(make_line(page, 236.0, fs, &[(50.0, 82.0, "Skills")])),
            make_block_with_line(make_line(
                page,
                236.0,
                fs,
                &[(
                    210.0,
                    500.0,
                    "To implement different ways of waste management into daily life",
                )],
            )),
            make_block_with_line(make_line(
                page,
                214.0,
                fs,
                &[(50.0, 170.0, "Attitudes and Values")],
            )),
            make_block_with_line(make_line(
                page,
                214.0,
                fs,
                &[(
                    210.0,
                    510.0,
                    "To educate others on the importance of sustainable waste management",
                )],
            )),
            make_context_block(
                page,
                180.0,
                "Context below the table to simulate a real page width",
            ),
        ];

        let result = detect_cluster_tables(elements);
        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 1, "Expected indented key/value table");

        let Some(ContentElement::TableBorder(tb)) = result
            .iter()
            .find(|e| matches!(e, ContentElement::TableBorder(_)))
        else {
            panic!("Expected TableBorder");
        };
        assert_eq!(tb.num_columns, 2);
        assert!(tb.num_rows >= 2);
    }

    #[test]
    fn test_two_column_key_value_table_with_column_major_order_detected() {
        let page = 1u32;
        let fs = 10.0;

        let elements = vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(make_line(
                page,
                310.0,
                fs,
                &[(50.0, 125.0, "Competence Area")],
            )),
            make_block_with_line(make_line(
                page,
                292.0,
                fs,
                &[(50.0, 150.0, "Competence Statement")],
            )),
            make_block_with_line(make_line(page, 270.0, fs, &[(50.0, 95.0, "Knowledge")])),
            make_block_with_line(make_line(page, 236.0, fs, &[(50.0, 82.0, "Skills")])),
            make_block_with_line(make_line(
                page,
                214.0,
                fs,
                &[(50.0, 170.0, "Attitudes and Values")],
            )),
            make_block_with_line(make_line(page, 310.0, fs, &[(210.0, 370.0, "#1 THE 3 RS")])),
            make_block_with_line(make_line(
                page,
                292.0,
                fs,
                &[(
                    210.0,
                    520.0,
                    "To know the basics of the 3 Rs and their implementation",
                )],
            )),
            make_block_with_line(make_line(
                page,
                270.0,
                fs,
                &[(
                    210.0,
                    520.0,
                    "To understand the meaning of reducing reusing and recycling",
                )],
            )),
            make_block_with_line(make_line(
                page,
                258.0,
                fs,
                &[(
                    210.0,
                    500.0,
                    "To understand the importance of the 3 Rs as waste management",
                )],
            )),
            make_block_with_line(make_line(
                page,
                236.0,
                fs,
                &[(
                    210.0,
                    500.0,
                    "To implement different ways of waste management into daily life",
                )],
            )),
            make_block_with_line(make_line(
                page,
                214.0,
                fs,
                &[(
                    210.0,
                    510.0,
                    "To educate others on the importance of sustainable waste management",
                )],
            )),
            make_context_block(
                page,
                180.0,
                "Context below the table to simulate a real page width",
            ),
        ];

        let result = detect_cluster_tables(elements);
        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 1, "Expected column-major key/value table");
    }

    #[test]
    fn test_three_column_panel_table_is_rebuilt_with_left_stub_column() {
        let page = 1u32;
        let fs = 10.0;

        let result = detect_cluster_tables(vec![
            make_context_block(page, 380.0, "Context above the panel"),
            make_block_with_line(make_line(page, 336.0, fs, &[(220.0, 250.0, "OCR")])),
            make_block_with_line(make_line(
                page,
                336.0,
                fs,
                &[(420.0, 520.0, "Recommendation")],
            )),
            make_block_with_line(make_line(
                page,
                336.0,
                fs,
                &[(650.0, 850.0, "Product semantic search")],
            )),
            make_block_with_line(make_line(page, 312.0, fs, &[(72.0, 110.0, "Pack")])),
            make_block_with_line(make_line(
                page,
                312.0,
                fs,
                &[(145.0, 340.0, "Character recognition")],
            )),
            make_block_with_line(make_line(
                page,
                312.0,
                fs,
                &[(390.0, 620.0, "Best-product recommendation")],
            )),
            make_block_with_line(make_line(
                page,
                312.0,
                fs,
                &[(650.0, 910.0, "Semantic product search")],
            )),
            make_block_with_line(make_line(
                page,
                286.0,
                fs,
                &[
                    (145.0, 360.0, "Application text extraction"),
                    (390.0, 625.0, "Application next-item prediction"),
                    (650.0, 910.0, "Application search to DB"),
                ],
            )),
            make_block_with_line(make_line(page, 272.0, fs, &[(72.0, 138.0, "Application")])),
            make_block_with_line(make_line(
                page,
                248.0,
                fs,
                &[
                    (145.0, 360.0, "Highlight OCR competition"),
                    (390.0, 625.0, "Highlight Kaggle medal"),
                    (650.0, 910.0, "Highlight KLUE benchmark"),
                ],
            )),
            make_block_with_line(make_line(page, 234.0, fs, &[(72.0, 120.0, "Highlight")])),
            make_context_block(page, 190.0, "Context below the panel"),
        ]);

        let Some(ContentElement::TableBorder(tb)) = result
            .iter()
            .find(|e| matches!(e, ContentElement::TableBorder(_)))
        else {
            panic!("Expected panel table");
        };

        assert_eq!(tb.num_columns, 4);
        assert!(tb.rows.len() >= 4);
        assert_eq!(cell_text(&tb.rows[0].cells[0]), "");
        assert_eq!(cell_text(&tb.rows[0].cells[1]), "OCR");
        assert_eq!(cell_text(&tb.rows[1].cells[0]), "Pack");
        assert!(cell_text(&tb.rows[2].cells[0]).contains("Application"));
        assert!(cell_text(&tb.rows[3].cells[0]).contains("Highlight"));
    }

    #[test]
    fn test_grouped_headers_are_promoted_into_existing_cluster_table() {
        let page = 1u32;
        let fs = 10.0;
        let elements = vec![
            make_context_block(page, 380.0, "Context above the grouped table"),
            make_block_with_line(make_line(page, 336.0, fs, &[(100.0, 130.0, "Properties")])),
            make_block_with_line(make_line(
                page,
                336.0,
                fs,
                &[
                    (165.0, 220.0, "Instruction"),
                    (315.0, 366.0, "Training Datasets"),
                    (402.0, 433.0, "Alignment"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                322.0,
                fs,
                &[
                    (200.0, 250.0, "Alpaca-GPT4"),
                    (250.0, 300.0, "OpenOrca"),
                    (300.0, 360.0, "Synth. Math-Instruct"),
                    (360.0, 410.0, "Orca DPO Pairs"),
                    (410.0, 470.0, "Ultrafeedback Cleaned"),
                    (470.0, 530.0, "Synth. Math-Alignment"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                300.0,
                fs,
                &[
                    (95.0, 160.0, "Total # Samples"),
                    (200.0, 230.0, "52K"),
                    (250.0, 290.0, "2.91M"),
                    (300.0, 340.0, "126K"),
                    (360.0, 390.0, "12.9K"),
                    (410.0, 450.0, "60.8K"),
                    (470.0, 500.0, "126K"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                286.0,
                fs,
                &[
                    (95.0, 185.0, "Maximum # Samples Used"),
                    (200.0, 230.0, "52K"),
                    (250.0, 290.0, "100K"),
                    (300.0, 330.0, "52K"),
                    (360.0, 390.0, "12.9K"),
                    (410.0, 450.0, "60.8K"),
                    (470.0, 505.0, "20.1K"),
                ],
            )),
            make_block_with_line(make_line(
                page,
                272.0,
                fs,
                &[
                    (95.0, 145.0, "Open Source"),
                    (200.0, 215.0, "O"),
                    (250.0, 265.0, "O"),
                    (300.0, 315.0, "✗"),
                    (360.0, 375.0, "O"),
                    (410.0, 425.0, "O"),
                    (470.0, 485.0, "✗"),
                ],
            )),
        ];
        let table = make_cluster_table(
            page,
            &[95.0, 160.0, 230.0, 290.0, 340.0, 390.0, 450.0, 505.0],
            &[310.0, 296.0, 282.0],
            &[300.0, 286.0, 272.0],
            &[
                vec![
                    "Total # Samples",
                    "52K",
                    "2.91M",
                    "126K",
                    "12.9K",
                    "60.8K",
                    "126K",
                ],
                vec![
                    "Maximum # Samples Used",
                    "52K",
                    "100K",
                    "52K",
                    "12.9K",
                    "60.8K",
                    "20.1K",
                ],
                vec!["Open Source", "O", "O", "✗", "O", "O", "✗"],
            ],
            vec![4, 5, 6],
        );

        let augmented = augment_grouped_header_cluster_table(&elements, &table)
            .expect("expected grouped-header augmentation");

        assert_eq!(augmented.table_border.num_columns, 7);
        assert_eq!(augmented.table_border.num_rows, 5);
        assert_eq!(
            cell_text(&augmented.table_border.rows[0].cells[0]),
            "Properties"
        );
        assert_eq!(
            cell_text(&augmented.table_border.rows[0].cells[1]),
            "Instruction"
        );
        assert_eq!(
            cell_text(&augmented.table_border.rows[0].cells[4]),
            "Training Datasets"
        );
        assert!(augmented.table_border.rows[0]
            .cells
            .iter()
            .any(|cell| cell_text(cell) == "Alignment"));
        assert_eq!(
            cell_text(&augmented.table_border.rows[1].cells[1]),
            "Alpaca-GPT4"
        );
        assert_eq!(
            cell_text(&augmented.table_border.rows[1].cells[6]),
            "Synth. Math-Alignment"
        );
    }

    #[test]
    fn test_caption_compact_two_column_table_with_lowercase_headers_detected() {
        let page = 1u32;
        let fs = 10.0;

        let elements = vec![
            make_context_block(
                page,
                420.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(make_line(
                page,
                400.0,
                fs,
                &[(
                    50.0,
                    360.0,
                    "Table 13.4. Typical CEC of various soil colloids",
                )],
            )),
            make_block_with_line(make_line(
                page,
                380.0,
                fs,
                &[(60.0, 155.0, "Mineral or colloid type")],
            )),
            make_block_with_line(make_line(
                page,
                380.0,
                fs,
                &[(170.0, 245.0, "CEC of pure colloid")],
            )),
            make_block_with_line(make_line(page, 364.0, fs, &[(170.0, 210.0, "cmolc/kg")])),
            make_block_with_line(make_line(
                page,
                348.0,
                fs,
                &[(
                    60.0,
                    190.0,
                    "kaolinite 10 illite 30 montmorillonite/smectite 100 vermiculite 150 humus 200",
                )],
            )),
            make_context_block(
                page,
                300.0,
                "Context below the table to simulate a real page width",
            ),
        ];

        let result = detect_cluster_tables(elements);
        let Some(ContentElement::TableBorder(tb)) = result
            .iter()
            .find(|e| matches!(e, ContentElement::TableBorder(_)))
        else {
            panic!("Expected compact caption table");
        };

        assert_eq!(tb.num_columns, 2);
        assert!(tb.num_rows >= 5);
        assert!(!tb.rows[0].cells[0].contents.is_empty());
        assert!(!tb.rows[0].cells[1].contents.is_empty());
    }

    #[test]
    fn test_caption_compact_two_column_table_with_merged_header_row_detected() {
        let page = 1u32;
        let fs = 10.0;

        let elements = vec![
            make_context_block(
                page,
                700.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(make_line(
                page,
                680.0,
                fs,
                &[(
                    50.0,
                    420.0,
                    "Table 13.2. Effect of cations on flocculation of a clay suspension",
                )],
            )),
            make_block_with_line(make_line(
                page,
                648.0,
                fs,
                &[(
                    60.0,
                    285.0,
                    "Added cation Relative Size & Settling Rates of Floccules",
                )],
            )),
            make_block_with_line(make_line(
                page,
                632.0,
                fs,
                &[(60.0, 90.0, "K+ Na+ Ca2+ Al3+ Check")],
            )),
            make_context_block(
                page,
                580.0,
                "Context below the table to simulate a real page width",
            ),
        ];

        let result = detect_cluster_tables(elements);
        let Some(ContentElement::TableBorder(tb)) = result
            .iter()
            .find(|e| matches!(e, ContentElement::TableBorder(_)))
        else {
            panic!("Expected merged-header caption table");
        };

        assert_eq!(tb.num_columns, 2);
        assert!(tb.num_rows >= 4);
        assert!(!tb.rows[0].cells[0].contents.is_empty());
        assert!(!tb.rows[0].cells[1].contents.is_empty());
    }

    #[test]
    fn test_two_column_prose_rows_still_rejected() {
        let page = 1u32;
        let fs = 10.0;

        let row1 = make_line(
            page,
            300.0,
            fs,
            &[
                (
                    50.0,
                    220.0,
                    "This paragraph explains the experimental setup",
                ),
                (
                    300.0,
                    520.0,
                    "This paragraph continues the discussion in a second column",
                ),
            ],
        );
        let row2 = make_line(
            page,
            286.0,
            fs,
            &[
                (
                    50.0,
                    225.0,
                    "Another paragraph with several prose words in the left column",
                ),
                (
                    300.0,
                    520.0,
                    "Another prose paragraph on the right that should stay text",
                ),
            ],
        );
        let row3 = make_line(
            page,
            272.0,
            fs,
            &[
                (
                    50.0,
                    220.0,
                    "Further discussion continues in narrative form",
                ),
                (
                    300.0,
                    520.0,
                    "The layout resembles two-column prose rather than a table",
                ),
            ],
        );

        let result = detect_cluster_tables(vec![
            make_context_block(
                page,
                340.0,
                "Context above the table to simulate a real page width",
            ),
            make_block_with_line(row1),
            make_block_with_line(row2),
            make_block_with_line(row3),
            make_context_block(
                page,
                240.0,
                "Context below the table to simulate a real page width",
            ),
        ]);

        let table_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .count();
        assert_eq!(table_count, 0, "Two-column prose should remain rejected");
    }

    #[test]
    #[ignore]
    fn debug_real_doc_00200_stage7_rows() {
        use std::path::Path;

        use crate::api::config::ProcessingConfig;
        use crate::models::content::ContentElement;
        use crate::pdf::chunk_parser::extract_page_chunks;
        use crate::pdf::loader::load_pdf;
        use crate::pdf::page_info;
        use crate::pipeline::stages::boxed_heading_promoter;
        use crate::pipeline::stages::column_detector;
        use crate::pipeline::stages::content_filter;
        use crate::pipeline::stages::list_detector;
        use crate::pipeline::stages::table_content_assigner;
        use crate::pipeline::stages::table_detector;
        use crate::pipeline::stages::text_block_grouper;
        use crate::pipeline::stages::text_line_grouper;

        let pdf_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/pdfs/01030000000200.pdf");
        let config = ProcessingConfig::default();
        let raw_doc = load_pdf(&pdf_path, None).unwrap();
        let page_info_list = page_info::extract_page_info(&raw_doc.document);
        let (&page_num, &page_id) = raw_doc.document.get_pages().iter().next().unwrap();
        let page_chunks = extract_page_chunks(&raw_doc.document, page_num, page_id).unwrap();
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

        elements = content_filter::filter_content(
            elements,
            &config.filter_config,
            &page_info_list[0].crop_box,
        );
        elements = table_detector::detect_table_borders(elements);
        elements = table_content_assigner::assign_content_to_tables(elements);
        elements = table_detector::filter_empty_tables(elements);
        elements = boxed_heading_promoter::promote_boxed_headings(elements);
        elements = table_detector::release_pre_cluster_tables(elements);
        let mut pages = vec![elements];
        let layouts = column_detector::detect_columns(&mut pages);
        let mut elements = text_line_grouper::group_text_lines(pages.remove(0), layouts.first());
        elements = list_detector::detect_lists(elements);
        elements = text_block_grouper::group_text_blocks(elements);

        eprintln!("stage7 elements {}", elements.len());
        for (idx, elem) in elements.iter().enumerate() {
            let bbox = elem.bbox();
            eprintln!(
                "#{idx:02} {:?} x=({:.1},{:.1}) y=({:.1},{:.1})",
                std::mem::discriminant(elem),
                bbox.left_x,
                bbox.right_x,
                bbox.bottom_y,
                bbox.top_y,
            );
            if let Some(text) = element_text(elem) {
                eprintln!("    text={text}");
            }
        }

        let mut all_chunks = Vec::new();
        for (block_idx, el) in elements.iter().enumerate() {
            match el {
                ContentElement::TextBlock(block) => {
                    if !block.is_hidden_text {
                        for line in &block.text_lines {
                            if !line.is_hidden_text {
                                collect_line_chunks(&mut all_chunks, line, block_idx);
                            }
                        }
                    }
                }
                ContentElement::TextLine(line) => {
                    if !line.is_hidden_text {
                        collect_line_chunks(&mut all_chunks, line, block_idx);
                    }
                }
                _ => {}
            }
        }
        let rows = group_chunks_into_row_candidates(&all_chunks);
        eprintln!("row candidates {}", rows.len());
        for (ri, row) in rows.iter().enumerate() {
            let texts: Vec<_> = row
                .segments
                .iter()
                .map(|seg| format!("[{:.0}-{:.0}] {}", seg.left_x, seg.right_x, seg.text))
                .collect();
            eprintln!(
                "row {ri:02} baseline {:.1} :: {}",
                row.baseline,
                texts.join(" || ")
            );
        }

        let result = detect_cluster_tables(elements);
        for elem in &result {
            if let ContentElement::TableBorder(tb) = elem {
                eprintln!(
                    "detected table rows={} cols={} width={:.1} height={:.1}",
                    tb.num_rows,
                    tb.num_columns,
                    tb.bbox.width(),
                    tb.bbox.height()
                );
                for row in &tb.rows {
                    let cells: Vec<_> = row.cells.iter().map(cell_text).collect();
                    eprintln!("row {} => {:?}", row.row_number, cells);
                }
            }
        }
        let filtered = table_detector::filter_suspicious_tables(result);
        eprintln!(
            "tables after suspicious filter {}",
            filtered
                .iter()
                .filter(|e| matches!(e, ContentElement::TableBorder(_)))
                .count()
        );
    }

    #[test]
    #[ignore]
    fn debug_real_doc_00199_stage7_rows() {
        use std::path::Path;

        use crate::api::config::ProcessingConfig;
        use crate::models::content::ContentElement;
        use crate::pdf::chunk_parser::extract_page_chunks;
        use crate::pdf::loader::load_pdf;
        use crate::pdf::page_info;
        use crate::pipeline::stages::boxed_heading_promoter;
        use crate::pipeline::stages::column_detector;
        use crate::pipeline::stages::content_filter;
        use crate::pipeline::stages::list_detector;
        use crate::pipeline::stages::table_content_assigner;
        use crate::pipeline::stages::table_detector;
        use crate::pipeline::stages::text_block_grouper;
        use crate::pipeline::stages::text_line_grouper;

        let pdf_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/pdfs/01030000000199.pdf");
        let config = ProcessingConfig::default();
        let raw_doc = load_pdf(&pdf_path, None).unwrap();
        let page_info_list = page_info::extract_page_info(&raw_doc.document);
        let (&page_num, &page_id) = raw_doc.document.get_pages().iter().next().unwrap();
        let page_chunks = extract_page_chunks(&raw_doc.document, page_num, page_id).unwrap();
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

        elements = content_filter::filter_content(
            elements,
            &config.filter_config,
            &page_info_list[0].crop_box,
        );
        elements = table_detector::detect_table_borders(elements);
        elements = table_content_assigner::assign_content_to_tables(elements);
        elements = table_detector::filter_empty_tables(elements);
        elements = boxed_heading_promoter::promote_boxed_headings(elements);
        let mut pages = vec![elements];
        let layouts = column_detector::detect_columns(&mut pages);
        let mut elements = text_line_grouper::group_text_lines(pages.remove(0), layouts.first());
        elements = list_detector::detect_lists(elements);
        elements = text_block_grouper::group_text_blocks(elements);

        eprintln!("stage7 elements {}", elements.len());
        let result = detect_cluster_tables(elements);
        for elem in &result {
            if let ContentElement::TableBorder(tb) = elem {
                let text = tb
                    .rows
                    .iter()
                    .flat_map(|row| row.cells.iter())
                    .map(cell_text)
                    .collect::<Vec<_>>()
                    .join(" ");
                let image_tokens = tb
                    .rows
                    .iter()
                    .flat_map(|row| row.cells.iter())
                    .flat_map(|cell| cell.content.iter())
                    .filter(|tok| tok.base.value == "[image]")
                    .count();
                eprintln!(
                    "detected table rows={} cols={} width={:.1} height={:.1} text_len={} image_tokens={}",
                    tb.num_rows,
                    tb.num_columns,
                    tb.bbox.width(),
                    tb.bbox.height(),
                    text.len(),
                    image_tokens
                );
            }
        }
        let filtered = table_detector::filter_suspicious_tables(result);
        eprintln!(
            "tables after suspicious filter {}",
            filtered
                .iter()
                .filter(|e| matches!(e, ContentElement::TableBorder(_)))
                .count()
        );
    }

    #[test]
    #[ignore]
    fn debug_real_doc_00200_final_document_tables() {
        use std::path::Path;

        use crate::api::config::ProcessingConfig;

        let pdf_path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../benchmark/pdfs/01030000000200.pdf");
        let doc = crate::convert(&pdf_path, &ProcessingConfig::default()).unwrap();
        eprintln!("final kids {}", doc.kids.len());
        for elem in &doc.kids {
            if let ContentElement::TableBorder(tb) = elem {
                eprintln!(
                    "final table rows={} cols={} width={:.1} height={:.1}",
                    tb.num_rows,
                    tb.num_columns,
                    tb.bbox.width(),
                    tb.bbox.height()
                );
                for row in &tb.rows {
                    let cells: Vec<_> = row.cells.iter().map(cell_text).collect();
                    eprintln!("row {} => {:?}", row.row_number, cells);
                }
            }
        }
        eprintln!(
            "final table count {}",
            doc.kids
                .iter()
                .filter(|e| matches!(e, ContentElement::TableBorder(_)))
                .count()
        );
        let md = crate::output::markdown::to_markdown(&doc).unwrap();
        eprintln!("markdown has pipe table {}", md.contains("| --- |"));
        eprintln!("{md}");
    }
}
