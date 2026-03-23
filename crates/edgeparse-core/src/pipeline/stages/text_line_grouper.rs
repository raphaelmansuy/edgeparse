//! Stage 6: Text Line Grouping
//!
//! Groups TextChunks that share a common baseline into TextLines.
//!
//! ```text
//!   Chunks on page (unsorted):
//!     [A](x=10,y=100)  [B](x=200,y=100)  [C](x=10,y=80)  [†](x=220,y=105)
//!
//!   Step 1: Y-bucket + X sort
//!     bucket 0 (y≈100): A(x=10), B(x=200), †(x=220)   ← same Y-band
//!     bucket 1 (y≈80):  C(x=10)
//!
//!   Step 2: Sequential merge (Y-close + X-adjacent)
//!     Line 1: [A] [B] [†]     ← merged into one TextLine
//!     Line 2: [C]             ← separate line
//! ```
//!
//! Algorithm:
//!   1. Compute the median font size of all chunks on the page.  Use
//!      `median_font × BUCKET_RATIO` as a shared bucket size so that chunks
//!      with different font sizes (body text vs. superscripts) land in the same
//!      Y-bucket and are sorted in left-to-right order within that bucket.
//!   2. Sort by (Y-bucket ASC, left_x ASC).  This ensures a raised "†" marker
//!      (higher center_y) sorts AFTER "Canjie Luo" (same bucket, larger X),
//!      fixing the isolation bug in the original Y-desc sort.
//!   3. Sequential scan: merge adjacent chunks that are Y-close and X-adjacent.
//!      Additionally accept tiny chunks (≤ 4 chars) that are in the next bucket
//!      and directly to the right as super/subscripts.

use crate::models::chunks::TextChunk;
use crate::models::content::ContentElement;
use crate::models::text::TextLine;
use crate::pipeline::stages::column_detector::ColumnLayout;

/// Shared bucket height as a fraction of the PAGE median font size.
/// Must satisfy:  superscript_rise/font  <  BUCKET_RATIO  <  line_spacing/font.
/// Typical values: rise ~0.35–0.50, spacing ~1.0–1.5 → 0.55 is safe.
const BUCKET_RATIO: f64 = 0.55;

/// Maximum vertical gap between baselines to be on the same line (ratio of font size).
const BASELINE_TOLERANCE_RATIO: f64 = 0.30;

/// Maximum horizontal gap between chunks to be on the same line (ratio of font size).
const MAX_HORIZONTAL_GAP_RATIO: f64 = 1.2;

/// NEIGHBORS_TEXT_CHUNKS_EPSILON — merge threshold as fraction of height.
/// Must be exactly 0.1: this threshold determines which adjacent
/// glyph fragments are character parts (merged, no space) vs. word boundaries
/// (left separate, space inserted later by needs_space at 0.17×fontSize).
/// A value of 0.25 was too aggressive — it merged chunks 2-3pt apart (which
/// ARE word boundaries at 12pt font) into single spaceless strings like
/// "CombinatorialCosmology" instead of keeping them separate for space insertion.
const NEIGHBORS_EPSILON: f64 = 0.1;

/// Baseline tolerance for mergeCloseTextChunks — fraction of height.
/// Relaxed from 0.01 to 0.05 to handle PDFs with per-glyph positioning where
/// individual characters have slight baseline jitter (common in design-heavy
/// PDFs and presentations).
const BASELINE_MERGE_TOLERANCE: f64 = 0.05;

/// areCloseNumbers default epsilon — absolute tolerance for style comparison.
/// Relaxed from 1e-4 to 0.5 for font_weight and 0.01 for font_size/italic_angle.
/// Some PDFs report slightly different font weights for each glyph operation
/// (e.g., 399.997 vs 400.003), which prevents merging under the old strict threshold.
const CLOSE_NUMBER_EPSILON: f64 = 0.01;
const WEIGHT_MERGE_EPSILON: f64 = 0.5;

/// Determine which column a chunk belongs to based on its center_x.
fn column_index(chunk: &TextChunk, layout: Option<&ColumnLayout>) -> usize {
    match layout {
        Some(l) if !l.boundaries.is_empty() => {
            let cx = chunk.bbox.center_x();
            l.boundaries
                .iter()
                .position(|&b| cx < b)
                .unwrap_or(l.boundaries.len())
        }
        _ => 0,
    }
}

/// Pre-merge adjacent TextChunks that have the same style, same baseline,
/// and are spatially close.  Matches the reference `TextProcessor.mergeCloseTextChunks()`.
///
/// This step merges character fragments like "ar" + "e" → "are" that the PDF
/// content stream emits as separate text operations with small bounding-box gaps.
/// Without this merge, the gap between fragments would be misinterpreted as a
/// word space during `TextLine::value()` computation.
fn merge_close_text_chunks(chunks: &mut Vec<TextChunk>) {
    if chunks.len() < 2 {
        return;
    }

    let mut merged: Vec<TextChunk> = Vec::with_capacity(chunks.len());
    let mut current = chunks.remove(0);

    for next in chunks.drain(..) {
        if are_same_style(&current, &next)
            && are_same_baseline(&current, &next)
            && are_neighbor_chunks(&current, &next)
        {
            current = union_text_chunks(current, next);
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);

    *chunks = merged;
}

/// The reference `areTextChunksHaveSameStyle` — checks font name, weight, size,
/// italic angle, and color for near-equality.
fn are_same_style(a: &TextChunk, b: &TextChunk) -> bool {
    a.font_name == b.font_name
        && (a.font_weight - b.font_weight).abs() <= WEIGHT_MERGE_EPSILON
        && (a.italic_angle - b.italic_angle).abs() <= CLOSE_NUMBER_EPSILON
        && a.font_color == b.font_color
        && (a.font_size - b.font_size).abs() <= CLOSE_NUMBER_EPSILON
}

/// The reference `areTextChunksHaveSameBaseLine` — baseline difference within 1% of height.
fn are_same_baseline(a: &TextChunk, b: &TextChunk) -> bool {
    let height = a.bbox.height().max(1.0);
    (a.bbox.bottom_y - b.bbox.bottom_y).abs() <= BASELINE_MERGE_TOLERANCE * height
}

/// The reference `areNeighborsTextChunks` — gap between textEnd and textStart
/// within 10% of the first chunk's height.
fn are_neighbor_chunks(a: &TextChunk, b: &TextChunk) -> bool {
    let height = a.bbox.height().max(1.0);
    (a.bbox.right_x - b.bbox.left_x).abs() <= NEIGHBORS_EPSILON * height
}

/// The reference `unionTextChunks` — merge two chunks by concatenating values,
/// unioning bounding boxes, and merging symbol_ends.
fn union_text_chunks(mut first: TextChunk, second: TextChunk) -> TextChunk {
    first.value.push_str(&second.value);
    first.bbox = first.bbox.union(&second.bbox);
    first.symbol_ends.extend(&second.symbol_ends);
    first
}

/// Group text chunks into text lines.
///
/// When `column_layout` is provided, chunks from different columns are never
/// merged into the same line, preventing two-column text from being interleaved.
pub fn group_text_lines(
    elements: Vec<ContentElement>,
    column_layout: Option<&ColumnLayout>,
) -> Vec<ContentElement> {
    // Separate text chunks from other elements.
    let mut text_chunks: Vec<TextChunk> = Vec::new();
    let mut other_elements: Vec<ContentElement> = Vec::new();

    for element in elements {
        match element {
            ContentElement::TextChunk(tc) => text_chunks.push(tc),
            other => other_elements.push(other),
        }
    }

    if text_chunks.is_empty() {
        return other_elements;
    }

    // Pre-merge adjacent same-style chunks (the reference mergeCloseTextChunks).
    // Must run BEFORE sorting/grouping so fragments like "ar"+"e" → "are"
    // are combined while still in PDF extraction order.
    merge_close_text_chunks(&mut text_chunks);

    // The reference filterConsecutiveSpaces: compress internal runs of spaces that
    // can arise when merging a chunk ending with " " and a whitespace chunk.
    for chunk in &mut text_chunks {
        chunk.compress_spaces();
    }

    // Shared bucket size: median font size × BUCKET_RATIO.
    // Using the SAME bucket size for every chunk on the page ensures that a
    // 12 pt body chunk and an 8 pt superscript chunk at +4 pt land in the
    // same bucket and are then ordered left-to-right by X.
    let shared_bucket_size = {
        let mut sizes: Vec<f64> = text_chunks.iter().map(|c| c.font_size).collect();
        sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = sizes[sizes.len() / 2];
        (median * BUCKET_RATIO).max(1.0)
    };

    // Sort by (Y-bucket ASC = top of page first, left_x ASC = left-to-right).
    text_chunks.sort_by(|a, b| {
        let ba = (-a.bbox.center_y() / shared_bucket_size).round() as i64;
        let bb = (-b.bbox.center_y() / shared_bucket_size).round() as i64;
        ba.cmp(&bb).then_with(|| {
            a.bbox
                .left_x
                .partial_cmp(&b.bbox.left_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    // Sequential grouping with the same logic as before.
    // Because the sort now puts superscripts to the RIGHT of their parent text
    // (same bucket, larger X), the h_gap check works correctly.
    //
    // Column boundary check (fallback): when column_layout has no boundaries
    // (detector couldn't find gaps due to spanning elements), use an inline
    // heuristic: if two chunks are on the same baseline but separated by more
    // than 3× the font_size, treat them as different columns. This threshold
    // matches the reference areSuspiciousTextChunks (gap > 3 × height).
    let has_column_boundaries = column_layout.is_some_and(|l| !l.boundaries.is_empty());

    let mut lines: Vec<TextLine> = Vec::new();
    let mut current_chunks: Vec<TextChunk> = vec![text_chunks.remove(0)];

    for chunk in text_chunks {
        let last = current_chunks.last().unwrap();
        let baseline_diff = (chunk.bbox.center_y() - last.bbox.center_y()).abs();
        let tolerance = last.font_size * BASELINE_TOLERANCE_RATIO;
        let h_gap = chunk.bbox.left_x - last.bbox.right_x;
        let max_gap = last.font_size * MAX_HORIZONTAL_GAP_RATIO;

        // Column boundary check: if multi-column layout is detected, never merge
        // chunks that are in different columns (prevents two-column interleaving).
        let same_column = if has_column_boundaries {
            column_index(&chunk, column_layout) == column_index(last, column_layout)
        } else {
            // Fallback: use inline heuristic for large horizontal gaps.
            // If h_gap > 2×font_size AND the chunk is not a tiny super/subscript,
            // treat as different column.  Reduced from 3.0→2.0 to avoid merging
            // two-column body text when the column detector fails to find boundaries.
            // Academic two-column gutters are typically 12–24 pt; at 10 pt font
            // typical word spacing is ≤ 4 pt, so 2.0× (20 pt) separates them.
            h_gap <= last.font_size * 2.0
                || (chunk.value.chars().count() <= 2 && h_gap <= last.font_size * 4.0)
        };

        // Standard "same baseline" merge.
        let on_same_line = same_column
            && baseline_diff <= tolerance
            && h_gap <= max_gap
            && h_gap >= -last.font_size;

        // Super/subscript merge: chunk is slightly off-baseline but directly to
        // the right — tiny marker glyph (≤ 4 chars).  Requires h_gap ≥ -2.0 so
        // we reject any chunk that starts well to the LEFT of the current line.
        let is_super_sub = !on_same_line
            && same_column
            && h_gap >= -2.0
            && h_gap <= max_gap
            && baseline_diff <= last.font_size
            && chunk.value.chars().count() <= 4;

        if on_same_line || is_super_sub {
            current_chunks.push(chunk);
        } else {
            lines.push(build_text_line(std::mem::take(&mut current_chunks)));
            current_chunks.push(chunk);
        }
    }

    if !current_chunks.is_empty() {
        lines.push(build_text_line(current_chunks));
    }

    // Post-merge pass: combine TextLines that share a baseline but landed in
    // different Y-buckets.  The bucket boundary can separate a section number
    // "7" from its heading "Variants of sj Observer Models" when they differ
    // by just a few points in center_y.  Pass the column layout so the post-
    // merge pass never re-merges text lines from different columns.
    lines = merge_adjacent_lines(lines, column_layout);

    // Convert lines to ContentElements and merge with non-text elements.
    let mut result: Vec<ContentElement> = other_elements;
    result.extend(lines.into_iter().map(ContentElement::TextLine));
    result
}

/// Merge TextLines that are on the same baseline and horizontally adjacent.
///
/// The Y-bucket sort can place same-line text in different buckets when their
/// center_y values straddle a bucket boundary (e.g. section number "7" at
/// cy=155.6 is in bucket -26 while heading "Variants..." at cy=154.0 is in
/// bucket -25).  This pass merges these accidentally-split lines.
///
/// When `column_layout` has detected column boundaries, text lines from
/// different columns are never merged, preventing two-column body text from
/// being re-interleaved by this post-grouping pass.
fn merge_adjacent_lines(
    mut lines: Vec<TextLine>,
    column_layout: Option<&ColumnLayout>,
) -> Vec<TextLine> {
    if lines.len() < 2 {
        return lines;
    }

    let has_column_boundaries = column_layout.is_some_and(|l| !l.boundaries.is_empty());

    // Sort by (page, base_line DESC = top-first, left_x ASC)
    lines.sort_by(|a, b| {
        a.bbox
            .page_number
            .cmp(&b.bbox.page_number)
            .then_with(|| {
                b.base_line
                    .partial_cmp(&a.base_line)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                a.bbox
                    .left_x
                    .partial_cmp(&b.bbox.left_x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut merged: Vec<TextLine> = Vec::with_capacity(lines.len());
    let mut current = lines.remove(0);

    for next in lines {
        let same_page = current.bbox.page_number == next.bbox.page_number;
        let baseline_diff = (current.base_line - next.base_line).abs();
        let ref_size = current.font_size.max(next.font_size).max(1.0);
        let tolerance = ref_size * BASELINE_TOLERANCE_RATIO;
        let h_gap = next.bbox.left_x - current.bbox.right_x;

        // Column boundary check: when multi-column layout is detected, never
        // merge text lines from different columns.  This prevents the post-
        // grouping pass from re-interleaving two-column body text that the
        // primary grouping pass already correctly separated.
        let same_column = if has_column_boundaries {
            let cur_cx = (current.bbox.left_x + current.bbox.right_x) * 0.5;
            let next_cx = (next.bbox.left_x + next.bbox.right_x) * 0.5;
            let col_of = |cx: f64| -> usize {
                column_layout
                    .and_then(|l| l.boundaries.iter().position(|&b| cx < b))
                    .unwrap_or_else(|| column_layout.map_or(0, |l| l.boundaries.len()))
            };
            col_of(cur_cx) == col_of(next_cx)
        } else {
            true // no column boundaries detected — allow normal merging
        };

        // Allow a larger gap for short labels (section numbers like "7", footnote
        // markers like "18") that are separated from heading/footnote text by a
        // wide space.  Normal lines use a modest 1.5× multiplier; short labels
        // (≤ 4 chars) use 3.0× to handle the ~2.5× font_size gap typically
        // inserted between a section number and its heading title.
        //
        // Special case: when BOTH lines are short (combined ≤ 15 chars), allow
        // unlimited gap.  This handles running headers like "3 4" + "Yarrow" at
        // opposite page margins (gap ~280pt).  The reference TextLineProcessor merges
        // all same-baseline chunks without any gap limit; this case covers the
        // common page-number + author-name pattern without affecting multi-column
        // body text (which has much longer lines).
        let cur_chars = current.value().chars().count();
        let next_chars = next.value().chars().count();
        let is_short_label = cur_chars <= 4 || next_chars <= 4;
        let both_short = cur_chars + next_chars <= 15;
        let font_ratio = (current.font_size.min(next.font_size)
            / current.font_size.max(next.font_size).max(1.0))
        .max(0.0);
        let allow_short_fragment_exception = font_ratio >= 0.75;
        let wide_parallel_lines = !has_column_boundaries
            && h_gap > ref_size * 1.2
            && cur_chars >= 12
            && next_chars >= 12
            && current.bbox.width() > ref_size * 6.0
            && next.bbox.width() > ref_size * 6.0;
        // The reference TextLineProcessor merges ALL same-baseline chunks without
        // any gap limit.  We approximate this for short fragments:
        //  - both_short (combined ≤ 15 chars): unlimited gap, any direction
        //  - is_short_label (either ≤ 4 chars): unlimited gap, any direction
        //    Covers page-number + header-title across full page width.
        //  - normal lines: modest 1.5× gap factor
        let has_numeric_fragment = looks_like_numeric_fragment(&current.value())
            || looks_like_numeric_fragment(&next.value());
        let gap_factor = if allow_short_fragment_exception
            && has_numeric_fragment
            && (both_short || is_short_label)
        {
            f64::MAX / (ref_size * MAX_HORIZONTAL_GAP_RATIO + 1.0)
        } else {
            // Cap at 3.0× font_size absolute maximum for normal lines to avoid
            // merging text from different columns in two-column layouts.
            // The reference areSuspiciousTextChunks uses 3× height as the column-gap indicator.
            let raw_max = ref_size * MAX_HORIZONTAL_GAP_RATIO * 1.5;
            let capped = raw_max.min(ref_size * 3.0);
            capped / (ref_size * MAX_HORIZONTAL_GAP_RATIO).max(1.0)
        };
        let max_gap = ref_size * MAX_HORIZONTAL_GAP_RATIO * gap_factor;

        // For short fragments (both_short or is_short_label), allow any
        // horizontal arrangement — they may be sorted with the right-most
        // line first (by descending baseline tie-breaking), giving a large
        // negative h_gap.  Normal lines keep the original guard.
        let h_gap_ok = if allow_short_fragment_exception
            && has_numeric_fragment
            && (both_short || is_short_label)
        {
            true
        } else {
            !wide_parallel_lines && h_gap >= -ref_size && h_gap <= max_gap
        };
        if same_column && same_page && baseline_diff <= tolerance && h_gap_ok {
            // Merge: absorb next's chunks into current, then re-sort by
            // left_x so that e.g. "3 4" (left) comes before "Yarrow" (right).
            current.text_chunks.extend(next.text_chunks);
            current.text_chunks.sort_by(|a, b| {
                a.bbox
                    .left_x
                    .partial_cmp(&b.bbox.left_x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            current.bbox = current.bbox.union(&next.bbox);
            current.font_size = current.font_size.max(next.font_size);
            current.base_line = current.bbox.bottom_y;
            current.level = common_column_level(
                current
                    .text_chunks
                    .iter()
                    .map(|chunk| chunk.level.as_deref()),
            );
        } else {
            merged.push(current);
            current = next;
        }
    }
    merged.push(current);

    merged
}

fn looks_like_numeric_fragment(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 4
        && trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c.is_ascii_whitespace())
}

/// Build a TextLine from a group of TextChunks.
fn build_text_line(chunks: Vec<TextChunk>) -> TextLine {
    let bbox = chunks
        .iter()
        .map(|c| c.bbox.clone())
        .reduce(|a, b| a.union(&b))
        .expect("build_text_line called with empty chunks");

    let font_size = chunks
        .iter()
        .map(|c| c.font_size)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(0.0);

    let base_line = bbox.bottom_y;
    let level = common_column_level(chunks.iter().map(|chunk| chunk.level.as_deref()));

    TextLine {
        bbox,
        index: None,
        level,
        font_size,
        base_line,
        slant_degree: 0.0,
        is_hidden_text: false,
        text_chunks: chunks,
        is_line_start: false,
        is_line_end: false,
        is_list_line: false,
        connected_line_art_label: None,
    }
}

fn common_column_level<'a>(levels: impl IntoIterator<Item = Option<&'a str>>) -> Option<String> {
    let mut iter = levels.into_iter().flatten();
    let first = iter.next()?;
    if iter.all(|level| level == first) {
        Some(first.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_tc(value: &str, x: f64, y: f64, width: f64, size: f64) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: value.to_string(),
            bbox: BoundingBox::new(Some(1), x, y, x + width, y + size),
            font_name: "Helvetica".to_string(),
            font_size: size,
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
    fn test_single_chunk_becomes_line() {
        let elements = vec![make_tc("Hello", 100.0, 700.0, 60.0, 12.0)];
        let result = group_text_lines(elements, None);
        assert_eq!(result.len(), 1);
        matches!(&result[0], ContentElement::TextLine(_));
    }

    #[test]
    fn test_two_chunks_same_baseline() {
        let elements = vec![
            make_tc("Hello", 100.0, 700.0, 60.0, 12.0),
            make_tc("World", 165.0, 700.0, 60.0, 12.0),
        ];
        let result = group_text_lines(elements, None);
        assert_eq!(result.len(), 1);
        if let ContentElement::TextLine(ref line) = result[0] {
            assert_eq!(line.text_chunks.len(), 2);
            assert_eq!(line.value(), "Hello World");
        } else {
            panic!("Expected TextLine");
        }
    }

    #[test]
    fn test_two_lines_different_baselines() {
        let elements = vec![
            make_tc("Line 1", 100.0, 700.0, 60.0, 12.0),
            make_tc("Line 2", 100.0, 680.0, 60.0, 12.0),
        ];
        let result = group_text_lines(elements, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_empty_input() {
        let elements: Vec<ContentElement> = vec![];
        let result = group_text_lines(elements, None);
        assert!(result.is_empty());
    }

    /// Superscript marker "†" raised ~4pt should join the adjacent text line.
    #[test]
    fn test_superscript_merges_with_parent() {
        // "Luo" at baseline y=700 (12pt font)
        // "†"   raised by 4pt → y=704 (8pt font), directly to the right of "Luo"
        let elements = vec![
            make_tc("Luo", 72.0, 700.0, 24.0, 12.0),
            make_tc("†", 100.0, 704.0, 6.0, 8.0),
        ];
        let result = group_text_lines(elements, None);
        // Should produce one TextLine containing both chunks
        assert_eq!(result.len(), 1, "superscript should merge with parent text");
        if let ContentElement::TextLine(ref line) = result[0] {
            assert_eq!(line.text_chunks.len(), 2);
        }
    }

    /// Two columns at the same Y must stay as separate TextLines.
    #[test]
    fn test_two_columns_same_y_not_merged() {
        // Left column at x=72, right column at x=320 (gap=200pt).
        // Use realistic multi-word column content (> 15 chars combined) so
        // the short-fragment running-header exception doesn't trigger.
        let elements = vec![
            make_tc("left column text data", 72.0, 700.0, 120.0, 10.0),
            make_tc("right column text data", 340.0, 700.0, 120.0, 10.0),
        ];
        let result = group_text_lines(elements, None);
        assert_eq!(result.len(), 2, "two-column text must not merge");
    }

    #[test]
    fn test_short_cross_column_fragment_not_merged() {
        let elements = vec![
            make_tc("black and white", 72.0, 700.0, 90.0, 10.0),
            make_tc("of", 340.0, 700.0, 8.0, 10.0),
        ];
        let result = group_text_lines(elements, None);
        assert_eq!(
            result.len(),
            2,
            "short fragments must not bridge cross-column gaps"
        );
    }

    #[test]
    fn test_post_merge_does_not_rejoin_parallel_columns_without_layout() {
        let left = build_text_line(vec![TextChunk {
            value: "left column bibliography entry".to_string(),
            bbox: BoundingBox::new(Some(1), 72.0, 700.0, 210.0, 710.0),
            font_name: "Helvetica".to_string(),
            font_size: 10.0,
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
        }]);
        let right = build_text_line(vec![TextChunk {
            value: "right column bibliography entry".to_string(),
            bbox: BoundingBox::new(Some(1), 226.0, 700.0, 366.0, 710.0),
            font_name: "Helvetica".to_string(),
            font_size: 10.0,
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
        }]);

        let merged = merge_adjacent_lines(vec![left, right], None);
        assert_eq!(
            merged.len(),
            2,
            "post-merge should not reconnect wide parallel columns"
        );
    }

    #[test]
    fn test_running_header_short_fragment_merges_when_font_sizes_match() {
        let elements = vec![
            make_tc("3 4", 72.0, 790.0, 18.0, 9.0),
            make_tc("Yarrow", 340.0, 790.0, 42.0, 9.0),
        ];
        let result = group_text_lines(elements, None);
        assert_eq!(
            result.len(),
            1,
            "same-size running header fragments should merge"
        );
    }

    #[test]
    fn test_tiny_page_number_does_not_merge_into_large_toc_line() {
        let elements = vec![
            make_tc(
                "2. Introduction of Product Services and Key Features",
                240.0,
                174.0,
                350.0,
                17.0,
            ),
            make_tc("6", 348.0, 193.0, 4.0, 5.0),
        ];
        let result = group_text_lines(elements, None);
        assert_eq!(
            result.len(),
            2,
            "tiny page number should stay separate from TOC text"
        );
    }
}
