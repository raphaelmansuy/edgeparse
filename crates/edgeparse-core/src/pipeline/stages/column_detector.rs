//! Multi-Column Layout Detection
//!
//! Detects multi-column layouts on each page by analyzing horizontal gaps
//! between content elements. Assigns a `column_index` to each element.
//! This stage runs before reading order to help with proper sequencing.

use crate::models::content::ContentElement;

/// Minimum gap width (in points) between columns to consider them separate.
/// Set to 6pt to detect narrow column gutters as small as 9pt (after the
/// 3pt bbox erosion applied to each side of each element's bounding box).
const MIN_COLUMN_GAP: f64 = 6.0;

/// Erosion applied to each side of every element's bbox before building the
/// histogram.  This counteracts the "bbox inflation" in PDF TextChunks where
/// right_x overshoots the actual visible glyph by a few points due to advance
/// width calculations.  Net effect: the column gap appears `2 × BBOX_SHRINK`
/// wider in the histogram, making narrow gutters (≥ 9pt) detectable.
const BBOX_SHRINK: f64 = 3.0;

/// Result of column detection for a single page.
#[derive(Debug)]
pub struct ColumnLayout {
    /// Number of columns detected.
    pub num_columns: usize,
    /// X-coordinate boundaries between columns (column separators).
    pub boundaries: Vec<f64>,
}

/// Detect column layout for each page and assign column indices.
///
/// Returns the detected column layouts, one per page.
pub fn detect_columns(pages: &mut [Vec<ContentElement>]) -> Vec<ColumnLayout> {
    pages
        .iter_mut()
        .map(|page| {
            let layout = detect_page_columns(page);
            assign_column_indices(page, &layout);
            layout
        })
        .collect()
}

/// Detect columns on a single page using a Y-baseline histogram approach.
///
/// For each 1-point X bin, counts how many distinct Y baselines (text lines)
/// have element coverage at that X position. Real column gaps are wide X regions
/// where very few text lines pass through the X strip. This approach is immune
/// to the "cascade merge" problem of the old interval-merging algorithm.
///
/// A bin is treated as a "gap bin" when its baseline count is at most
/// `max(1, total_baselines / 8)` — allowing up to ~12% of page baselines to
/// cross the gap, which handles figure labels or captions that occasionally
/// span the column gutter without masking the gap.
///
/// Spanning elements (those covering > 60% of the page width, e.g. titles,
/// footers, wide charts/tables) are excluded from bin coverage.
fn detect_page_columns(page: &[ContentElement]) -> ColumnLayout {
    use std::collections::HashSet;

    if page.is_empty() {
        return ColumnLayout {
            num_columns: 1,
            boundaries: vec![],
        };
    }

    // Compute page extents
    let page_left = page
        .iter()
        .map(|e| e.bbox().left_x)
        .fold(f64::INFINITY, f64::min);
    let page_right = page
        .iter()
        .map(|e| e.bbox().right_x)
        .fold(f64::NEG_INFINITY, f64::max);
    let page_top = page
        .iter()
        .map(|e| e.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max);
    let page_bottom = page
        .iter()
        .map(|e| e.bbox().bottom_y)
        .fold(f64::INFINITY, f64::min);
    let page_height = page_top - page_bottom;
    if page_height <= 0.0 {
        return ColumnLayout {
            num_columns: 1,
            boundaries: vec![],
        };
    }
    let page_width = (page_right - page_left).max(1.0);

    // An element is "spanning" if it covers > 60% of the page width.
    let span_threshold = page_width * 0.6;

    // Collect non-spanning elements (those that belong to a single column).
    let non_spanning: Vec<_> = page
        .iter()
        .filter(|e| {
            let b = e.bbox();
            let w = b.right_x - b.left_x;
            w > 0.0 && w <= span_threshold
        })
        .collect();

    if non_spanning.len() < 2 {
        return ColumnLayout {
            num_columns: 1,
            boundaries: vec![],
        };
    }

    // Build a 1-point-resolution X histogram.
    // For each bin, record the set of distinct Y baselines (quantized to 3pt
    // buckets to group chunks on the same text line) that have coverage there.
    let x_origin = page_left.floor();
    let n_bins = ((page_right.ceil() - x_origin) as usize + 1).max(1);
    let mut bin_baselines: Vec<HashSet<i64>> = vec![HashSet::new(); n_bins];
    let mut all_baselines: HashSet<i64> = HashSet::new();

    for elem in &non_spanning {
        let b = elem.bbox();
        // Quantize center-Y to 3pt buckets → same text line ≈ same key
        let y_key = (((b.top_y + b.bottom_y) * 0.5) / 3.0).round() as i64;
        all_baselines.insert(y_key);
        // Apply BBOX_SHRINK to both edges to counteract bbox inflation in PDF
        // TextChunks: text runs' right_x can overshoot the actual visible glyph
        // by a few points, which artificially fills the column gutter.  Shrinking
        // makes the effective gap appear wider and easier to detect.
        let eff_left = b.left_x + BBOX_SHRINK;
        let eff_right = b.right_x - BBOX_SHRINK;
        if eff_right <= eff_left {
            continue; // element too narrow after erosion
        }
        let left_bin = ((eff_left - x_origin).floor().max(0.0) as usize).min(n_bins - 1);
        let right_bin = ((eff_right - x_origin).ceil().max(0.0) as usize).min(n_bins);
        for cell in &mut bin_baselines[left_bin..right_bin] {
            cell.insert(y_key);
        }
    }

    let total_baselines = all_baselines.len();
    if total_baselines < 4 {
        if let Some(boundary) = fallback_center_gap_boundary(&non_spanning, page_left, page_right) {
            return ColumnLayout {
                num_columns: 2,
                boundaries: vec![boundary],
            };
        }
        if std::env::var("COLUMN_DEBUG").is_ok() {
            eprintln!(
                "[COLUMN] sparse baselines total={} non_spanning={} page_width={:.1}",
                total_baselines,
                non_spanning.len(),
                page_width
            );
        }
        // Too few baselines to reliably detect columns.
        return ColumnLayout {
            num_columns: 1,
            boundaries: vec![],
        };
    }

    // Convert to per-bin coverage counts for gap analysis.
    let coverage_counts: Vec<usize> = bin_baselines.iter().map(|s| s.len()).collect();
    let max_cov = coverage_counts.iter().copied().max().unwrap_or(0);
    if max_cov == 0 {
        if std::env::var("COLUMN_DEBUG").is_ok() {
            eprintln!(
                "[COLUMN] zero coverage max_cov=0 non_spanning={}",
                non_spanning.len()
            );
        }
        return ColumnLayout {
            num_columns: 1,
            boundaries: vec![],
        };
    }

    // A bin is a "gap bin" when its coverage ≤ gap_threshold.
    // Use 25% of the peak column coverage: this tolerates figure labels and
    // chart numbers whose bboxes span the column gutter, while still detecting
    // the gap cleanly when the two columns each have high (≥ 50%) coverage.
    let gap_threshold = (max_cov / 4).max(1);

    // Require that the bins just outside the gap have coverage > half_cov,
    // confirming there is real column content on both sides of the gap.
    let half_cov = max_cov / 2;

    let min_gap_bins = MIN_COLUMN_GAP as usize;
    let edge_margin = page_width * 0.08; // ignore gaps within 8% of page edges

    let mut boundaries = Vec::new();
    let mut gap_start: Option<usize> = None;

    for (i, &cov) in coverage_counts.iter().enumerate() {
        if cov <= gap_threshold {
            if gap_start.is_none() {
                gap_start = Some(i);
            }
        } else {
            if let Some(start) = gap_start {
                let gap_len = i - start;
                if gap_len >= min_gap_bins && cov > half_cov {
                    // Verify left side has high coverage too (look back up to 10 bins)
                    let left_ok = if start > 0 {
                        let check_start = start.saturating_sub(10);
                        coverage_counts[check_start..start]
                            .iter()
                            .any(|&c| c > half_cov)
                    } else {
                        false
                    };
                    if left_ok {
                        let gap_center = x_origin + (start + i) as f64 * 0.5;
                        if gap_center > page_left + edge_margin
                            && gap_center < page_right - edge_margin
                        {
                            boundaries.push(gap_center);
                        }
                    }
                }
            }
            gap_start = None;
        }
    }
    // A trailing low-coverage run extending to the right edge is the right margin — skip it.

    let dense_single_band = max_cov * 100 >= total_baselines.saturating_mul(85);
    if dense_single_band {
        boundaries.clear();
    }

    if boundaries.is_empty() && !dense_single_band {
        if let Some(boundary) = fallback_center_gap_boundary(&non_spanning, page_left, page_right) {
            boundaries.push(boundary);
        }
    }

    if std::env::var("COLUMN_DEBUG").is_ok() {
        eprintln!(
            "[COLUMN] total_baselines={} max_cov={} gap_threshold={} boundaries={:?}",
            total_baselines, max_cov, gap_threshold, boundaries
        );
    }

    ColumnLayout {
        num_columns: boundaries.len() + 1,
        boundaries,
    }
}

fn fallback_center_gap_boundary(
    elements: &[&ContentElement],
    page_left: f64,
    page_right: f64,
) -> Option<f64> {
    if elements.len() < 4 {
        return None;
    }

    let page_width = (page_right - page_left).max(1.0);
    let min_candidate_width = page_width * 0.18;
    let mut candidates: Vec<&ContentElement> = elements
        .iter()
        .copied()
        .filter(|elem| elem.bbox().width() >= min_candidate_width)
        .collect();
    if candidates.len() < 4 {
        return None;
    }

    candidates.sort_by(|a, b| {
        a.bbox()
            .center_x()
            .partial_cmp(&b.bbox().center_x())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut best_gap = 0.0f64;
    let mut best_idx = None;
    for idx in 0..candidates.len() - 1 {
        let left_center = candidates[idx].bbox().center_x();
        let right_center = candidates[idx + 1].bbox().center_x();
        let gap = right_center - left_center;
        if gap > best_gap {
            best_gap = gap;
            best_idx = Some(idx);
        }
    }

    let idx = best_idx?;
    if best_gap < page_width * 0.08 {
        return None;
    }

    let split_x = (candidates[idx].bbox().center_x() + candidates[idx + 1].bbox().center_x()) * 0.5;
    let (left, right): (Vec<&ContentElement>, Vec<&ContentElement>) = candidates
        .into_iter()
        .partition(|elem| elem.bbox().center_x() < split_x);
    if left.len() < 2 || right.len() < 2 {
        return None;
    }

    let left_max_right = left
        .iter()
        .map(|elem| elem.bbox().right_x)
        .fold(f64::NEG_INFINITY, f64::max);
    let right_min_left = right
        .iter()
        .map(|elem| elem.bbox().left_x)
        .fold(f64::INFINITY, f64::min);
    if right_min_left - left_max_right < MIN_COLUMN_GAP {
        return None;
    }

    let left_top = left
        .iter()
        .map(|elem| elem.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max);
    let left_bottom = left
        .iter()
        .map(|elem| elem.bbox().bottom_y)
        .fold(f64::INFINITY, f64::min);
    let right_top = right
        .iter()
        .map(|elem| elem.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max);
    let right_bottom = right
        .iter()
        .map(|elem| elem.bbox().bottom_y)
        .fold(f64::INFINITY, f64::min);

    let overlap = (left_top.min(right_top) - left_bottom.max(right_bottom)).max(0.0);
    let min_height = (left_top - left_bottom)
        .min(right_top - right_bottom)
        .max(1.0);
    if overlap / min_height < 0.25 {
        return None;
    }

    Some((left_max_right + right_min_left) * 0.5)
}

/// Assign column indices to elements based on detected boundaries.
fn assign_column_indices(page: &mut [ContentElement], layout: &ColumnLayout) {
    for elem in page.iter_mut() {
        if layout.boundaries.is_empty() {
            clear_column_level(elem);
            continue;
        }

        let center_x = elem.bbox().center_x();
        let col = layout
            .boundaries
            .iter()
            .position(|&b| center_x < b)
            .unwrap_or(layout.boundaries.len());

        set_column_level(elem, Some(format!("col:{col}")));
    }
}

fn set_column_level(elem: &mut ContentElement, level: Option<String>) {
    match elem {
        ContentElement::TextChunk(chunk) => {
            chunk.level = level;
        }
        ContentElement::TextLine(line) => {
            line.level = level;
        }
        ContentElement::TextBlock(block) => {
            block.level = level;
        }
        ContentElement::Paragraph(p) => {
            p.base.level = level;
        }
        ContentElement::Heading(h) => {
            h.base.base.level = level;
        }
        ContentElement::TableBorder(t) => {
            t.level = level;
        }
        ContentElement::Image(img) => {
            img.level = level;
        }
        _ => {}
    }
}

fn clear_column_level(elem: &mut ContentElement) {
    match elem {
        ContentElement::TextChunk(chunk) => clear_if_column_level(&mut chunk.level),
        ContentElement::TextLine(line) => clear_if_column_level(&mut line.level),
        ContentElement::TextBlock(block) => clear_if_column_level(&mut block.level),
        ContentElement::Paragraph(p) => clear_if_column_level(&mut p.base.level),
        ContentElement::Heading(h) => clear_if_column_level(&mut h.base.base.level),
        ContentElement::TableBorder(t) => clear_if_column_level(&mut t.level),
        ContentElement::Image(img) => clear_if_column_level(&mut img.level),
        _ => {}
    }
}

fn clear_if_column_level(level: &mut Option<String>) {
    if level
        .as_deref()
        .is_some_and(|value| value.starts_with("col:"))
    {
        *level = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, SemanticType, TextFormat, TextType};
    use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    fn make_para_at(left_x: f64, right_x: f64, y: f64) -> ContentElement {
        let chunk = TextChunk {
            value: "text".to_string(),
            bbox: BoundingBox::new(Some(1), left_x, y, right_x, y + 12.0),
            font_name: "Arial".to_string(),
            font_size: 10.0,
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
            font_size: 10.0,
            base_line: y + 2.0,
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
            font_size: 10.0,
            base_line: y + 2.0,
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
            font_size: 10.0,
            base_line: y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: SemanticTextNode {
                bbox: col.bbox.clone(),
                index: None,
                level: None,
                semantic_type: SemanticType::Paragraph,
                correct_semantic_score: None,
                columns: vec![col],
                font_weight: Some(400.0),
                font_size: Some(10.0),
                text_color: None,
                italic_angle: None,
                font_name: Some("Arial".to_string()),
                text_format: None,
                max_font_size: Some(10.0),
                background_color: None,
                is_hidden_text: false,
            },
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    #[test]
    fn test_two_column_layout() {
        // Left column: x=72..280, Right column: x=310..520
        let e1 = make_para_at(72.0, 280.0, 700.0);
        let e2 = make_para_at(72.0, 280.0, 680.0);
        let e3 = make_para_at(310.0, 520.0, 700.0);
        let e4 = make_para_at(310.0, 520.0, 680.0);
        let mut pages = vec![vec![e1, e2, e3, e4]];

        let layouts = detect_columns(&mut pages);

        assert_eq!(layouts[0].num_columns, 2);
        assert_eq!(layouts[0].boundaries.len(), 1);
    }

    #[test]
    fn test_single_column_layout() {
        let e1 = make_para_at(72.0, 500.0, 700.0);
        let e2 = make_para_at(72.0, 500.0, 680.0);
        let mut pages = vec![vec![e1, e2]];

        let layouts = detect_columns(&mut pages);

        assert_eq!(layouts[0].num_columns, 1);
    }

    #[test]
    fn test_empty_page() {
        let mut pages: Vec<Vec<ContentElement>> = vec![vec![]];
        let layouts = detect_columns(&mut pages);
        assert_eq!(layouts[0].num_columns, 1);
    }

    #[test]
    fn test_single_column_detection_clears_stale_column_labels() {
        let mut pages = vec![vec![
            make_para_at(72.0, 500.0, 700.0),
            make_para_at(72.0, 500.0, 680.0),
        ]];

        for elem in &mut pages[0] {
            if let ContentElement::Paragraph(p) = elem {
                p.base.level = Some("col:1".to_string());
            }
        }

        let layouts = detect_columns(&mut pages);

        assert_eq!(layouts[0].num_columns, 1);
        assert!(pages[0].iter().all(|elem| match elem {
            ContentElement::Paragraph(p) => p.base.level.is_none(),
            _ => true,
        }));
    }
}
