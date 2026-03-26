//! XY-Cut++ reading order algorithm.
//!
//! ```text
//!   ┌──────────────────────────┐
//!   │  Col A      │   Col B    │
//!   │  ┌────────┐ │ ┌────────┐ │
//!   │  │ Para 1 │ │ │ Para 3 │ │    1. Find largest vertical gap
//!   │  └────────┘ │ └────────┘ │       → split left / right
//!   │  ┌────────┐ │ ┌────────┐ │    2. Recurse on each half
//!   │  │ Para 2 │ │ │ Para 4 │ │    3. Within half: find horizontal gap
//!   │  └────────┘ │ └────────┘ │       → split top / bottom
//!   └──────────────────────────┘    4. Order: top→bottom, left→right
//!
//!   Result: Para 1, Para 2, Para 3, Para 4
//! ```

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;

/// Sort content elements using the XY-Cut++ algorithm.
///
/// The algorithm recursively partitions the page:
/// 1. Find largest horizontal gap → split into top/bottom
/// 2. Find largest vertical gap → split into left/right
/// 3. Recurse on each partition
/// 4. Order: top-to-bottom, left-to-right within each partition
pub fn xycut_sort(elements: &mut [ContentElement], page_bbox: &BoundingBox) {
    if elements.len() <= 1 {
        return;
    }
    xycut_recursive(elements, page_bbox);
}

fn xycut_recursive(elements: &mut [ContentElement], region: &BoundingBox) {
    if elements.len() <= 1 {
        return;
    }

    // Find both possible cuts
    let h_gap = find_horizontal_gap_size(elements);
    let v_gap = find_vertical_gap_size(elements);
    let vertical_analysis = v_gap.and_then(|(split_x, v_size)| {
        analyze_vertical_cut(
            elements,
            split_x,
            h_gap.map(|(_, gap)| gap).unwrap_or(0.0),
            v_size,
        )
        .map(|analysis| (split_x, analysis))
    });

    if std::env::var("XYCUT_DEBUG").is_ok() {
        eprintln!(
            "[XYCUT] n={} h_gap={:?} v_gap={:?}",
            elements.len(),
            h_gap.map(|(y, g)| format!("y={:.1} gap={:.1}", y, g)),
            v_gap.map(|(x, g)| format!("x={:.1} gap={:.1}", x, g))
        );
        for e in elements.iter() {
            let b = e.bbox();
            eprintln!(
                "  el: l={:.1} r={:.1} top={:.1} bot={:.1}",
                b.left_x, b.right_x, b.top_y, b.bottom_y
            );
        }
    }

    // Prefer a vertical split only when the page really behaves like concurrent
    // columns.  This avoids the old "always horizontal first" bias that
    // interleaved figure-heavy two-column pages row-by-row, while still keeping
    // full-width spanning elements (titles, section headings, footers) outside
    // the column subtrees.
    let prefer_vertical = vertical_analysis.is_some();

    if prefer_vertical {
        // Try vertical cut first (column split)
        if let Some((split_x, analysis)) = vertical_analysis.as_ref() {
            if reorder_vertical_bands(elements, *split_x, analysis, region) {
                return;
            }
        }
        if let Some((split_x, _)) = v_gap {
            let (left, right) = partition_by_x(elements, split_x);
            if !left.is_empty() && !right.is_empty() {
                xycut_recursive(left, region);
                xycut_recursive(right, region);
                return;
            }
        }
        // Fallback to horizontal cut
        if let Some((split_y, _)) = h_gap {
            let (top, bottom) = partition_by_y(elements, split_y);
            if !top.is_empty() && !bottom.is_empty() {
                xycut_recursive(top, region);
                xycut_recursive(bottom, region);
                return;
            }
        }
    } else {
        // Try horizontal cut first (row split)
        if let Some((split_y, _)) = h_gap {
            let (top, bottom) = partition_by_y(elements, split_y);
            if !top.is_empty() && !bottom.is_empty() {
                xycut_recursive(top, region);
                xycut_recursive(bottom, region);
                return;
            }
        }
        // Fallback to vertical cut
        if let Some((split_x, _)) = v_gap {
            let (left, right) = partition_by_x(elements, split_x);
            if !left.is_empty() && !right.is_empty() {
                xycut_recursive(left, region);
                xycut_recursive(right, region);
                return;
            }
        }
    }

    // No split found — sort by quantized Y (top-to-bottom) then by left_x (column order).
    //
    // Rationale: in 2-column documents where the column gap is 0 pt (columns are
    // perfectly adjacent with no whitespace), the vertical-gap detector cannot find
    // a split, and the fallback sort takes over.  The left and right column elements
    // at the same visual row often differ by only a fraction of a point in top_y due
    // to PDF coordinate rounding.  If the right column element's top_y is even 0.5 pt
    // higher, the old comparison by raw top_y would place the right column first,
    // reversing the reading order.
    //
    // Quantizing top_y to 4 pt buckets groups elements that are visually on the same
    // text row (sub-line-height Y difference) into the same bucket, then sorts within
    // the bucket by left_x (left-column-first).  Typical body text lines are 10–14 pt
    // apart, so a 4 pt bucket never merges elements from adjacent text lines.
    const BUCKET_PT: f64 = 4.0;
    elements.sort_by_key(|e| {
        let y_bucket = -((e.bbox().top_y / BUCKET_PT).round() as i64); // descending
        let x_key = (e.bbox().left_x * 10.0).round() as i64; // ascending
        (y_bucket, x_key)
    });
}

#[derive(Debug, Clone, Copy)]
struct VerticalCutAnalysis {
    shared_top: f64,
    shared_bottom: f64,
}

fn analyze_vertical_cut(
    elements: &[ContentElement],
    split_x: f64,
    horizontal_gap: f64,
    vertical_gap: f64,
) -> Option<VerticalCutAnalysis> {
    if elements.len() < 4 {
        return None;
    }

    let min_x = elements
        .iter()
        .map(|e| e.bbox().left_x)
        .fold(f64::INFINITY, f64::min);
    let max_x = elements
        .iter()
        .map(|e| e.bbox().right_x)
        .fold(f64::NEG_INFINITY, f64::max);
    let content_width = (max_x - min_x).max(1.0);
    let spanning_width = content_width * 0.55;
    let split_margin = 6.0;
    let band_tolerance = 8.0;

    let mut left_count = 0usize;
    let mut right_count = 0usize;
    let mut left_top = f64::NEG_INFINITY;
    let mut left_bottom = f64::INFINITY;
    let mut right_top = f64::NEG_INFINITY;
    let mut right_bottom = f64::INFINITY;
    let mut crossing_bands: Vec<(f64, f64)> = Vec::new();

    for elem in elements {
        let bbox = elem.bbox();
        let width = bbox.right_x - bbox.left_x;
        let crosses_split =
            bbox.left_x < split_x - split_margin && bbox.right_x > split_x + split_margin;

        if crosses_split && width >= spanning_width {
            crossing_bands.push((bbox.top_y, bbox.bottom_y));
            continue;
        }

        if bbox.center_x() < split_x {
            left_count += 1;
            left_top = left_top.max(bbox.top_y);
            left_bottom = left_bottom.min(bbox.bottom_y);
        } else {
            right_count += 1;
            right_top = right_top.max(bbox.top_y);
            right_bottom = right_bottom.min(bbox.bottom_y);
        }
    }

    if left_count < 2 || right_count < 2 {
        return None;
    }

    let left_height = (left_top - left_bottom).max(0.0);
    let right_height = (right_top - right_bottom).max(0.0);
    if left_height <= 0.0 || right_height <= 0.0 {
        return None;
    }

    let overlap = (left_top.min(right_top) - left_bottom.max(right_bottom)).max(0.0);
    let overlap_ratio = overlap / left_height.min(right_height);
    if overlap_ratio < 0.35 || vertical_gap < horizontal_gap * 0.5 {
        return None;
    }

    let shared_top = left_top.min(right_top);
    let shared_bottom = left_bottom.max(right_bottom);
    if shared_top <= shared_bottom {
        return None;
    }

    let ambiguous_crossing = crossing_bands
        .iter()
        .filter(|(top_y, bottom_y)| {
            *bottom_y < shared_top - band_tolerance && *top_y > shared_bottom + band_tolerance
        })
        .count();
    if ambiguous_crossing > 0 {
        return None;
    }

    Some(VerticalCutAnalysis {
        shared_top,
        shared_bottom,
    })
}

fn reorder_vertical_bands(
    elements: &mut [ContentElement],
    split_x: f64,
    analysis: &VerticalCutAnalysis,
    region: &BoundingBox,
) -> bool {
    const SPLIT_MARGIN: f64 = 6.0;
    const BAND_TOLERANCE: f64 = 8.0;

    let mut top_spanning = Vec::new();
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut bottom_spanning = Vec::new();

    for element in elements.iter() {
        let bbox = element.bbox();
        let crosses_split =
            bbox.left_x < split_x - SPLIT_MARGIN && bbox.right_x > split_x + SPLIT_MARGIN;

        if crosses_split {
            if bbox.bottom_y >= analysis.shared_top - BAND_TOLERANCE {
                top_spanning.push(element.clone());
                continue;
            }
            if bbox.top_y <= analysis.shared_bottom + BAND_TOLERANCE {
                bottom_spanning.push(element.clone());
                continue;
            }
            return false;
        }

        if bbox.center_x() < split_x {
            left.push(element.clone());
        } else {
            right.push(element.clone());
        }
    }

    if left.is_empty() || right.is_empty() {
        return false;
    }

    if top_spanning.len() > 1 {
        xycut_recursive(top_spanning.as_mut_slice(), region);
    }
    xycut_recursive(left.as_mut_slice(), region);
    xycut_recursive(right.as_mut_slice(), region);
    if bottom_spanning.len() > 1 {
        xycut_recursive(bottom_spanning.as_mut_slice(), region);
    }

    let mut ordered = Vec::with_capacity(elements.len());
    ordered.extend(top_spanning);
    ordered.extend(left);
    ordered.extend(right);
    ordered.extend(bottom_spanning);

    if ordered.len() != elements.len() {
        return false;
    }

    for (dst, src) in elements.iter_mut().zip(ordered.into_iter()) {
        *dst = src;
    }

    true
}

/// Minimum gap size (points) required to consider a cut.
const MIN_GAP_THRESHOLD: f64 = 5.0;

/// Find the largest horizontal gap (between rows of elements)
/// using projection-profile gap detection.
/// Sorts elements by top_y descending (top of page first) and tracks
/// the accumulated minimum bottom edge to find true gaps.
/// Returns (split_y, gap_size).
fn find_horizontal_gap_size(elements: &[ContentElement]) -> Option<(f64, f64)> {
    if elements.len() < 2 {
        return None;
    }

    // Sort by top_y descending (scan from top of page downward)
    let mut sorted: Vec<(f64, f64)> = elements
        .iter()
        .map(|e| (e.bbox().bottom_y, e.bbox().top_y))
        .collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut best_gap = 0.0f64;
    let mut best_y = None;
    let mut prev_bottom = sorted[0].0; // track lowest bottom seen so far

    for &(bottom, top) in &sorted[1..] {
        if prev_bottom > top {
            // Gap exists between previous accumulated bottom and this element's top
            let gap = prev_bottom - top;
            if gap > best_gap && gap > MIN_GAP_THRESHOLD {
                best_gap = gap;
                best_y = Some((prev_bottom + top) / 2.0);
            }
        }
        prev_bottom = prev_bottom.min(bottom); // accumulate lowest bottom
    }

    best_y.map(|y| (y, best_gap))
}

/// Find the largest vertical gap (between columns of elements)
/// using interval-merge gap detection.
///
/// Merges overlapping X-ranges into consolidated intervals, then finds the
/// largest gap between consecutive intervals.  Elements significantly wider
/// than the median width are excluded from gap computation — they are likely
/// full-width spanning elements (titles, footers) that would otherwise
/// swallow the column gap.  This pre-masking mirrors the approach used in
/// column_detector.rs and the reference implementation XYCutPlusPlusSorter's cross-layout logic.
///
/// Returns (split_x, gap_size).
fn find_vertical_gap_size(elements: &[ContentElement]) -> Option<(f64, f64)> {
    if elements.len() < 2 {
        return None;
    }

    // Compute content width extents to identify spanning elements
    let min_x = elements
        .iter()
        .map(|e| e.bbox().left_x)
        .fold(f64::INFINITY, f64::min);
    let max_x = elements
        .iter()
        .map(|e| e.bbox().right_x)
        .fold(f64::NEG_INFINITY, f64::max);
    let content_width = (max_x - min_x).max(1.0);

    // Exclude elements spanning > 70% of content width from gap computation
    let span_threshold = content_width * 0.70;

    // Erode each element's bbox by BBOX_SHRINK before building intervals.
    // PDF TextChunk right_x is often inflated by advance-width calculations
    // (overshooting the actual visible glyph by 3–7 pt), causing the interval
    // to bridge a real column gutter and masking the gap.  Matching the 3 pt
    // erosion used in column_detector.rs makes the gap appear ~6 pt wider here.
    const BBOX_SHRINK: f64 = 3.0;

    // Build sorted list of (left_x, right_x) intervals, excluding spanning elements
    let mut intervals: Vec<(f64, f64)> = elements
        .iter()
        .filter_map(|e| {
            let b = e.bbox();
            let w = b.right_x - b.left_x;
            if w > span_threshold {
                None
            } else {
                let eff_left = b.left_x + BBOX_SHRINK;
                let eff_right = b.right_x - BBOX_SHRINK;
                if eff_right > eff_left {
                    Some((eff_left, eff_right))
                } else {
                    None // element too narrow after erosion
                }
            }
        })
        .collect();

    // If all elements are spanning (or too few remain), fall back to using all
    // with erosion applied (raw bboxes are not needed for the gap calculation).
    if intervals.len() < 2 {
        intervals = elements
            .iter()
            .filter_map(|e| {
                let b = e.bbox();
                let eff_left = b.left_x + BBOX_SHRINK;
                let eff_right = b.right_x - BBOX_SHRINK;
                if eff_right > eff_left {
                    Some((eff_left, eff_right))
                } else {
                    None
                }
            })
            .collect();
    }

    intervals.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Merge overlapping intervals (with a small tolerance for near-touching
    // elements whose bounding boxes slightly overlap due to PDF coordinate
    // imprecision).  Kept at 0.5 pt — large enough for true glyph jitter
    // but small enough not to swallow genuine column gaps that touch at 0 pt.
    let mut merged: Vec<(f64, f64)> = Vec::new();
    let merge_tolerance = 0.5; // points — tight to preserve genuinely adjacent columns
    for (left, right) in intervals {
        if let Some(last) = merged.last_mut() {
            if left <= last.1 + merge_tolerance {
                // Overlapping or within tolerance — extend
                last.1 = last.1.max(right);
                continue;
            }
        }
        merged.push((left, right));
    }

    // Find the largest gap between consecutive merged intervals
    let mut best_gap = 0.0f64;
    let mut best_x = None;
    for w in merged.windows(2) {
        let gap = w[1].0 - w[0].1;
        if gap > best_gap && gap > MIN_GAP_THRESHOLD {
            best_gap = gap;
            best_x = Some((w[0].1 + w[1].0) / 2.0);
        }
    }

    // Fallback: center_x clustering.
    //
    // When bbox inflation causes left-column right_x to overshoot the right-
    // column left_x (making the above interval method fail), we look for a
    // large gap between consecutive element center_x values.  This is immune
    // to bbox inflation because center_x ≈ visual center of the content.
    //
    // Two guards keep this fallback precise:
    //  1. MIN_COL_ELEMENT_WIDTH: only elements at least 50pt wide are used to
    //     compute center_x candidates.  Tiny chart bars, bullet dots, or legend
    //     markers (typically < 30pt wide) would otherwise fill in the space
    //     between two text columns, hiding the column gap from the clustering.
    //  2. Right-partition left_x guard: after finding a candidate split_x,
    //     verify that the right-partition elements actually start on the right
    //     side of the page (min left_x of right-partition elements >= split_x *
    //     0.85).  In genuine two-column layouts the right column's left margin
    //     is close to or above split_x.  In single-column documents with
    //     width-varying elements (headings, indented text, spanning paragraphs),
    //     spanning elements are forced into the right partition by their center_x
    //     but their left_x is still at the left margin — this guard rejects them.
    if best_x.is_none() {
        const MIN_CENTER_GAP: f64 = 20.0;
        // Minimum element width to be used for center_x computation.
        // Filters out tiny chart bars, bullet markers, and legend dots.
        const MIN_COL_ELEMENT_WIDTH: f64 = 50.0;
        let mut centers: Vec<f64> = elements
            .iter()
            .filter_map(|e| {
                let b = e.bbox();
                let w = b.right_x - b.left_x;
                if w > span_threshold || w < MIN_COL_ELEMENT_WIDTH {
                    None // skip spanning elements and tiny elements
                } else {
                    Some((b.left_x + b.right_x) * 0.5)
                }
            })
            .collect();
        if centers.len() >= 2 {
            centers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mut candidate_x: Option<f64> = None;
            let mut candidate_gap = 0.0f64;
            for pair in centers.windows(2) {
                let gap = pair[1] - pair[0];
                if gap > candidate_gap && gap >= MIN_CENTER_GAP {
                    candidate_gap = gap;
                    candidate_x = Some((pair[0] + pair[1]) * 0.5);
                }
            }
            // Right-partition left_x guard: reject if right-partition elements
            // don't genuinely start on the right side of the split.
            if let Some(sx) = candidate_x {
                let right_min_left_x = elements
                    .iter()
                    .filter(|e| {
                        let b = e.bbox();
                        (b.left_x + b.right_x) * 0.5 >= sx
                    })
                    .map(|e| e.bbox().left_x)
                    .fold(f64::INFINITY, f64::min);
                if std::env::var("XYCUT_DEBUG").is_ok() {
                    eprintln!("[XYCUT] center_x candidate_x={:.1} candidate_gap={:.1} right_min_left_x={:.1} threshold={:.1} pass={}",
                        sx, candidate_gap, right_min_left_x, sx * 0.85, right_min_left_x >= sx * 0.85);
                }
                if right_min_left_x >= sx * 0.85 {
                    best_x = candidate_x;
                    best_gap = candidate_gap;
                }
            }
        }
    }

    best_x.map(|x| (x, best_gap))
}

/// Partition elements by Y coordinate (above/below split).
fn partition_by_y(
    elements: &mut [ContentElement],
    split_y: f64,
) -> (&mut [ContentElement], &mut [ContentElement]) {
    let pivot = itertools_partition(elements, |e| e.bbox().center_y() >= split_y);
    elements.split_at_mut(pivot)
}

/// Partition elements by X coordinate (left/right of split).
fn partition_by_x(
    elements: &mut [ContentElement],
    split_x: f64,
) -> (&mut [ContentElement], &mut [ContentElement]) {
    let pivot = itertools_partition(elements, |e| e.bbox().center_x() < split_x);
    elements.split_at_mut(pivot)
}

/// Partition a slice in-place: elements satisfying predicate come first.
/// Returns the index where false elements start.
fn itertools_partition<T, F>(data: &mut [T], pred: F) -> usize
where
    F: Fn(&T) -> bool,
{
    let mut pivot = 0;
    for i in 0..data.len() {
        if pred(&data[i]) {
            data.swap(i, pivot);
            pivot += 1;
        }
    }
    pivot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::ImageChunk;

    fn make_element(left: f64, bottom: f64, right: f64, top: f64) -> ContentElement {
        ContentElement::Image(ImageChunk {
            bbox: BoundingBox::new(Some(1), left, bottom, right, top),
            index: None,
            level: None,
        })
    }

    #[test]
    fn test_xycut_sort_single() {
        let mut elements = vec![make_element(0.0, 0.0, 100.0, 50.0)];
        xycut_sort(
            &mut elements,
            &BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0),
        );
        assert_eq!(elements.len(), 1);
    }

    #[test]
    fn test_xycut_sort_two_rows() {
        let mut elements = vec![
            make_element(50.0, 100.0, 200.0, 150.0), // lower row
            make_element(50.0, 500.0, 200.0, 550.0), // upper row
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);
        // Upper row should come first (higher Y = top of page in PDF)
        assert!(elements[0].bbox().top_y > elements[1].bbox().top_y);
    }

    #[test]
    fn test_xycut_sort_two_columns_left_first() {
        // Two-column layout with clear X gap (> 5pt)
        // Left column: x=[50, 250], Right column: x=[300, 500]
        // Column gap = 50pt, much larger than any row gap
        let mut elements = vec![
            make_element(300.0, 500.0, 500.0, 550.0), // right col, top
            make_element(50.0, 500.0, 250.0, 550.0),  // left col, top
            make_element(300.0, 400.0, 500.0, 450.0), // right col, mid
            make_element(50.0, 400.0, 250.0, 450.0),  // left col, mid
            make_element(300.0, 300.0, 500.0, 350.0), // right col, bot
            make_element(50.0, 300.0, 250.0, 350.0),  // left col, bot
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);

        // Left column (x<=250) should come before right column (x>=300)
        // Left col: top, mid, bot, then Right col: top, mid, bot
        assert!(
            elements[0].bbox().left_x < 260.0,
            "First element should be left column"
        );
        assert!(
            elements[1].bbox().left_x < 260.0,
            "Second element should be left column"
        );
        assert!(
            elements[2].bbox().left_x < 260.0,
            "Third element should be left column"
        );
        assert!(
            elements[3].bbox().left_x > 260.0,
            "Fourth element should be right column"
        );
        assert!(
            elements[4].bbox().left_x > 260.0,
            "Fifth element should be right column"
        );
        assert!(
            elements[5].bbox().left_x > 260.0,
            "Sixth element should be right column"
        );

        // Within left column: top-to-bottom
        assert!(elements[0].bbox().top_y > elements[1].bbox().top_y);
        assert!(elements[1].bbox().top_y > elements[2].bbox().top_y);
        // Within right column: top-to-bottom
        assert!(elements[3].bbox().top_y > elements[4].bbox().top_y);
        assert!(elements[4].bbox().top_y > elements[5].bbox().top_y);
    }

    #[test]
    fn test_xycut_sort_header_then_two_columns() {
        // Header spanning full width, then two columns
        let mut elements = vec![
            make_element(50.0, 700.0, 500.0, 750.0), // header (full width)
            make_element(300.0, 500.0, 500.0, 550.0), // right col, top
            make_element(50.0, 500.0, 250.0, 550.0), // left col, top
            make_element(300.0, 400.0, 500.0, 450.0), // right col, bot
            make_element(50.0, 400.0, 250.0, 450.0), // left col, bot
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);

        // Header first (y=750 is highest)
        assert!(
            elements[0].bbox().top_y >= 750.0,
            "Header should come first"
        );
        // Then left column, then right column
        assert!(
            elements[1].bbox().left_x < 260.0,
            "Left col should come after header"
        );
        assert!(elements[2].bbox().left_x < 260.0);
        assert!(
            elements[3].bbox().left_x > 260.0,
            "Right col should come last"
        );
        assert!(elements[4].bbox().left_x > 260.0);
    }

    #[test]
    fn test_projection_gap_with_overlapping_elements() {
        // Elements overlap in X but there's a real column gap
        // Left col elements: x=[50,250] and x=[60, 240]
        // Right col elements: x=[310, 500]
        // Naive adjacent-pair comparison would miss the gap
        let mut elements = vec![
            make_element(50.0, 500.0, 250.0, 550.0),  // left col, wide
            make_element(60.0, 400.0, 240.0, 450.0),  // left col, narrower
            make_element(310.0, 500.0, 500.0, 550.0), // right col
            make_element(310.0, 400.0, 500.0, 450.0), // right col
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);

        // Left column should come before right column
        assert!(elements[0].bbox().left_x < 260.0);
        assert!(elements[1].bbox().left_x < 260.0);
        assert!(elements[2].bbox().left_x > 260.0);
        assert!(elements[3].bbox().left_x > 260.0);
    }

    #[test]
    fn test_xycut_prefers_columns_for_staggered_two_column_layout() {
        let mut elements = vec![
            make_element(50.0, 680.0, 250.0, 740.0), // left col, top figure
            make_element(50.0, 420.0, 250.0, 660.0), // left col, body
            make_element(320.0, 600.0, 520.0, 760.0), // right col, top figure
            make_element(320.0, 360.0, 520.0, 580.0), // right col, body
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);

        assert!(elements[0].bbox().left_x < 260.0);
        assert!(elements[1].bbox().left_x < 260.0);
        assert!(elements[2].bbox().left_x > 260.0);
        assert!(elements[3].bbox().left_x > 260.0);
    }

    #[test]
    fn test_xycut_keeps_spanning_header_and_footer_outside_columns() {
        let mut elements = vec![
            make_element(40.0, 760.0, 540.0, 810.0),  // spanning header
            make_element(50.0, 640.0, 250.0, 700.0),  // left col, top
            make_element(50.0, 520.0, 250.0, 620.0),  // left col, bottom
            make_element(320.0, 640.0, 520.0, 700.0), // right col, top
            make_element(320.0, 520.0, 520.0, 620.0), // right col, bottom
            make_element(40.0, 430.0, 540.0, 480.0),  // spanning footer/source
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);

        assert!(
            elements[0].bbox().top_y >= 800.0,
            "header should stay first"
        );
        assert!(elements[1].bbox().left_x < 260.0);
        assert!(elements[2].bbox().left_x < 260.0);
        assert!(elements[3].bbox().left_x > 260.0);
        assert!(elements[4].bbox().left_x > 260.0);
        assert!(elements[5].bbox().top_y <= 480.0, "footer should stay last");
    }

    #[test]
    fn test_xycut_rejects_vertical_cut_when_spanning_band_sits_between_columns() {
        let mut elements = vec![
            make_element(50.0, 700.0, 250.0, 760.0),  // left col, top
            make_element(320.0, 700.0, 520.0, 760.0), // right col, top
            make_element(40.0, 610.0, 540.0, 680.0),  // spanning mid-band graphic
            make_element(50.0, 500.0, 250.0, 580.0),  // left col, bottom
            make_element(320.0, 500.0, 520.0, 580.0), // right col, bottom
        ];
        let page = BoundingBox::new(Some(1), 0.0, 0.0, 595.0, 842.0);
        xycut_sort(&mut elements, &page);

        assert!(elements[0].bbox().top_y >= 760.0);
        assert!(elements[1].bbox().top_y >= 760.0);
        assert!(elements[2].bbox().top_y >= 680.0 && elements[2].bbox().bottom_y <= 610.0);
        assert!(elements[3].bbox().top_y <= 580.0);
        assert!(elements[4].bbox().top_y <= 580.0);
    }
}
