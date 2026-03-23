//! Stage 8: Header/Footer Detection
//!
//! Cross-page analysis to identify repeated content at the top and bottom
//! of pages. Elements that appear at the same position across multiple pages
//! with similar text or sequential numbering are classified as headers/footers.

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::semantic::SemanticHeaderOrFooter;

/// Minimum number of pages that must share repeated content for it to be
/// classified as a header or footer (including the original page).
const MIN_PAGES_FOR_HEADER_FOOTER: usize = 2;

/// Tolerance for bounding-box horizontal position matching (in points).
const BBOX_X_TOLERANCE: f64 = 5.0;

/// Tolerance for bounding-box width matching (as fraction of width).
const BBOX_WIDTH_TOLERANCE: f64 = 0.15;

/// Tolerance for font size matching (as fraction of font size).
const FONT_SIZE_TOLERANCE: f64 = 0.15;

/// Fraction of page height that defines the header zone (top portion).
const HEADER_ZONE_FRACTION: f64 = 1.0 / 3.0;

/// Fraction of page height that defines the footer zone (bottom portion).
const FOOTER_ZONE_FRACTION: f64 = 1.0 / 3.0;

/// Detect and extract headers and footers across all pages.
///
/// Returns the modified page contents with header/footer elements wrapped
/// in `SemanticHeaderOrFooter` containers.
pub fn detect_headers_footers(pages: &mut [Vec<ContentElement>], page_height: f64) {
    detect_single_page_margin_headers(pages, page_height);

    if pages.len() < MIN_PAGES_FOR_HEADER_FOOTER {
        return;
    }

    // Detect headers (from top of page)
    let header_counts = count_repeated_from_edge(pages, page_height, true);
    // Detect footers (from bottom of page)
    let footer_counts = count_repeated_from_edge(pages, page_height, false);

    // Extract headers and footers, processing from last page to first
    // to avoid index invalidation
    for page_idx in (0..pages.len()).rev() {
        let h_count = header_counts.get(page_idx).copied().unwrap_or(0);
        let f_count = footer_counts.get(page_idx).copied().unwrap_or(0);

        // Sort page elements by vertical position (top-to-bottom in PDF coords)
        pages[page_idx].sort_by(|a, b| {
            b.bbox()
                .top_y
                .partial_cmp(&a.bbox().top_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let page_len = pages[page_idx].len();

        // Extract footer elements (from bottom)
        if f_count > 0 && f_count <= page_len {
            let footer_start = page_len - f_count;
            let footer_elements: Vec<ContentElement> =
                pages[page_idx].drain(footer_start..).collect();
            let footer_bbox = compute_union_bbox(&footer_elements);
            pages[page_idx].push(ContentElement::HeaderFooter(SemanticHeaderOrFooter {
                bbox: footer_bbox,
                index: None,
                level: None,
                semantic_type: SemanticType::Footer,
                contents: footer_elements,
            }));
        }

        // Extract header elements (from top)
        if h_count > 0 && h_count <= pages[page_idx].len() {
            let header_elements: Vec<ContentElement> = pages[page_idx].drain(..h_count).collect();
            let header_bbox = compute_union_bbox(&header_elements);
            pages[page_idx].insert(
                0,
                ContentElement::HeaderFooter(SemanticHeaderOrFooter {
                    bbox: header_bbox,
                    index: None,
                    level: None,
                    semantic_type: SemanticType::Header,
                    contents: header_elements,
                }),
            );
        }
    }
}

/// Promote cautious single-page running-header candidates before the repeated
/// cross-page matcher runs. Benchmark pages are often single-page crops, so
/// repeated-header logic cannot fire even when the page number / running title
/// is visually obvious.
fn detect_single_page_margin_headers(pages: &mut [Vec<ContentElement>], page_height: f64) {
    for page in pages {
        promote_single_page_margin_headers(page, page_height);
    }
}

fn promote_single_page_margin_headers(page: &mut Vec<ContentElement>, page_height: f64) {
    if page.is_empty() {
        return;
    }

    page.sort_by(|a, b| {
        b.bbox()
            .top_y
            .partial_cmp(&a.bbox().top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let body_width = page
        .iter()
        .filter(|elem| elem.bbox().width() >= 120.0 && elem.bbox().height() >= 24.0)
        .map(|elem| elem.bbox().width())
        .fold(0.0_f64, f64::max);
    if body_width <= 0.0 {
        return;
    }

    let candidate_indices: Vec<usize> = page
        .iter()
        .enumerate()
        .take_while(|(_, elem)| elem.bbox().top_y >= page_height * 0.88)
        .filter_map(|(idx, elem)| {
            if is_single_page_margin_header_candidate(elem, body_width, page_height) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    if candidate_indices.is_empty() {
        return;
    }

    let mut header_elements = Vec::with_capacity(candidate_indices.len());
    for idx in candidate_indices.into_iter().rev() {
        header_elements.push(page.remove(idx));
    }
    header_elements.reverse();

    let header_bbox = compute_union_bbox(&header_elements);
    page.insert(
        0,
        ContentElement::HeaderFooter(SemanticHeaderOrFooter {
            bbox: header_bbox,
            index: None,
            level: None,
            semantic_type: SemanticType::Header,
            contents: header_elements,
        }),
    );
}

fn is_single_page_margin_header_candidate(
    elem: &ContentElement,
    body_width: f64,
    page_height: f64,
) -> bool {
    let bbox = elem.bbox();
    if bbox.top_y < page_height * 0.88 || bbox.height() > 24.0 {
        return false;
    }

    let text = match elem {
        ContentElement::TextChunk(tc) => tc.value.trim().to_string(),
        ContentElement::TextLine(tl) => tl.value().trim().to_string(),
        ContentElement::TextBlock(tb) => {
            if tb.text_lines.len() != 1 {
                return false;
            }
            tb.value().trim().to_string()
        }
        _ => return false,
    };

    if text.is_empty() || text.split_whitespace().count() > 4 || text.chars().count() > 24 {
        return false;
    }

    let alpha_count = text.chars().filter(|ch| ch.is_alphabetic()).count();
    let digit_count = text.chars().filter(|ch| ch.is_ascii_digit()).count();
    let near_left_edge = bbox.left_x <= body_width * 0.08 + 24.0;
    let near_right_edge = bbox.right_x >= body_width * 0.92;
    let all_caps = alpha_count > 0
        && text
            .chars()
            .filter(|ch| ch.is_alphabetic())
            .all(|ch| ch.is_uppercase());
    let text_density =
        text.chars().filter(|ch| !ch.is_whitespace()).count() as f64 / bbox.width().max(1.0);
    let compact_or_sparse = bbox.width() <= body_width * 0.72 || text_density <= 0.06;

    compact_or_sparse && (digit_count > 0 || (all_caps && (near_left_edge || near_right_edge)))
}

/// Count how many elements from the top (is_header=true) or bottom
/// (is_header=false) of each page are repeated across adjacent pages.
fn count_repeated_from_edge(
    pages: &[Vec<ContentElement>],
    page_height: f64,
    is_header: bool,
) -> Vec<usize> {
    let num_pages = pages.len();
    let mut counts = vec![0usize; num_pages];

    // Pre-sort each page by top_y descending (top-to-bottom)
    let sorted_pages: Vec<Vec<&ContentElement>> = pages
        .iter()
        .map(|page| {
            let mut sorted: Vec<&ContentElement> = page.iter().collect();
            sorted.sort_by(|a, b| {
                b.bbox()
                    .top_y
                    .partial_cmp(&a.bbox().top_y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            sorted
        })
        .collect();

    // Iteratively check position_index from edge
    let mut position_index = 0;
    loop {
        let mut any_match = false;

        for page_idx in 0..num_pages.saturating_sub(1) {
            let elem_a = get_element_at_edge(&sorted_pages[page_idx], position_index, is_header);
            let elem_b =
                get_element_at_edge(&sorted_pages[page_idx + 1], position_index, is_header);

            let matches_adjacent = match (elem_a, elem_b) {
                (Some(a), Some(b)) => {
                    is_in_zone(a, page_height, is_header)
                        && is_in_zone(b, page_height, is_header)
                        && elements_match(a, b)
                }
                _ => false,
            };

            // Also check stride-2 (alternating pages like odd/even)
            let elem_c = if page_idx + 2 < num_pages {
                get_element_at_edge(&sorted_pages[page_idx + 2], position_index, is_header)
            } else {
                None
            };

            let matches_alternating = match (elem_a, elem_c) {
                (Some(a), Some(c)) => {
                    is_in_zone(a, page_height, is_header)
                        && is_in_zone(c, page_height, is_header)
                        && elements_match(a, c)
                }
                _ => false,
            };

            if matches_adjacent {
                counts[page_idx] = position_index + 1;
                counts[page_idx + 1] = position_index + 1;
                any_match = true;
            }
            if matches_alternating {
                if let Some(count) = counts.get_mut(page_idx) {
                    *count = (*count).max(position_index + 1);
                }
                if let Some(count) = counts.get_mut(page_idx + 2) {
                    *count = (*count).max(position_index + 1);
                }
                any_match = true;
            }
        }

        if !any_match {
            break;
        }
        position_index += 1;
    }

    counts
}

/// Get element at the given position from the edge (top or bottom).
fn get_element_at_edge<'a>(
    sorted: &'a [&'a ContentElement],
    position: usize,
    is_header: bool,
) -> Option<&'a ContentElement> {
    if is_header {
        // From top (sorted is already top-to-bottom)
        sorted.get(position).copied()
    } else {
        // From bottom
        if position < sorted.len() {
            sorted.get(sorted.len() - 1 - position).copied()
        } else {
            None
        }
    }
}

/// Check whether an element is in the header or footer zone.
fn is_in_zone(elem: &ContentElement, page_height: f64, is_header: bool) -> bool {
    let bbox = elem.bbox();
    if is_header {
        // Header zone: top portion of page (top_y >= page_height * (1 - fraction))
        bbox.bottom_y >= page_height * (1.0 - HEADER_ZONE_FRACTION)
    } else {
        // Footer zone: bottom portion of page (top_y <= page_height * fraction)
        bbox.top_y <= page_height * FOOTER_ZONE_FRACTION
    }
}

/// Check whether two elements match as potential repeated header/footer content.
fn elements_match(a: &ContentElement, b: &ContentElement) -> bool {
    // Check bounding box similarity (ignoring page number)
    if !bbox_similar(a.bbox(), b.bbox()) {
        return false;
    }

    // Check content-type specific matching
    match (a, b) {
        (ContentElement::TextChunk(ta), ContentElement::TextChunk(tb)) => {
            font_size_similar(ta.font_size, tb.font_size)
                && (ta.value == tb.value || is_sequential_text(&ta.value, &tb.value))
        }
        (ContentElement::TextLine(la), ContentElement::TextLine(lb)) => {
            font_size_similar(la.font_size, lb.font_size)
                && (la.value() == lb.value() || is_sequential_text(&la.value(), &lb.value()))
        }
        (ContentElement::TextBlock(ba), ContentElement::TextBlock(bb)) => {
            font_size_similar(ba.font_size, bb.font_size)
                && (ba.value() == bb.value() || is_sequential_text(&ba.value(), &bb.value()))
        }
        (ContentElement::Paragraph(pa), ContentElement::Paragraph(pb)) => {
            let fs_a = pa.base.font_size.unwrap_or(0.0);
            let fs_b = pb.base.font_size.unwrap_or(0.0);
            font_size_similar(fs_a, fs_b)
                && (pa.base.value() == pb.base.value()
                    || is_sequential_text(&pa.base.value(), &pb.base.value()))
        }
        // For non-text elements, bbox similarity is sufficient
        _ => true,
    }
}

/// Check if two bounding boxes are at a similar position (ignoring page number).
///
/// Uses overlap-based matching (like the reference areOverlapsBoundingBoxesExcludingPages)
/// which is more permissive than absolute tolerance. Two boxes "match" if they
/// overlap or are very close. Additionally checks width similarity to avoid
/// matching elements of very different sizes.
fn bbox_similar(a: &BoundingBox, b: &BoundingBox) -> bool {
    // Check horizontal overlap or proximity (within tolerance)
    let x_overlap =
        a.left_x <= b.right_x + BBOX_X_TOLERANCE && b.left_x <= a.right_x + BBOX_X_TOLERANCE;

    if !x_overlap {
        return false;
    }

    // Check width similarity
    let a_width = a.width();
    let b_width = b.width();
    let max_width = a_width.max(b_width);
    if max_width > 0.0 {
        (a_width - b_width).abs() / max_width < BBOX_WIDTH_TOLERANCE
    } else {
        true
    }
}

/// Check if two font sizes are similar.
fn font_size_similar(a: f64, b: f64) -> bool {
    let max_size = a.max(b);
    if max_size <= 0.0 {
        return true;
    }
    (a - b).abs() / max_size <= FONT_SIZE_TOLERANCE
}

/// Check if two text values represent sequential numbering (page numbers).
///
/// Handles:
/// - Simple numbers: "3" vs "4"
/// - Roman numerals: "IV" vs "V"
/// - Composite patterns: "Page 3 of 10" vs "Page 4 of 10"
/// - Composite patterns: "3 / 10" vs "4 / 10"
fn is_sequential_text(a: &str, b: &str) -> bool {
    let a = a.trim();
    let b = b.trim();

    // Try Arabic numbers
    if let (Ok(na), Ok(nb)) = (a.parse::<i64>(), b.parse::<i64>()) {
        return (na - nb).abs() <= 2; // Allow stride-2 for odd/even pages
    }

    // Try Roman numerals (simplified: just check if both look like roman and differ by ≤2)
    if let (Some(ra), Some(rb)) = (parse_roman(a), parse_roman(b)) {
        return (ra - rb).abs() <= 2;
    }

    // Try extracting the first number from composite patterns like "Page 3 of 10"
    if let (Some(na), Some(nb)) = (extract_first_number(a), extract_first_number(b)) {
        return (na - nb).abs() <= 2;
    }

    false
}

/// Extract the first integer from a string (for composite page numbers).
/// "Page 3 of 10" → Some(3), "3 / 10" → Some(3), "hello" → None
fn extract_first_number(s: &str) -> Option<i64> {
    let num_str: String = s
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if num_str.is_empty() {
        None
    } else {
        num_str.parse().ok()
    }
}

/// Parse a Roman numeral string to its integer value (case-insensitive).
fn parse_roman(s: &str) -> Option<i64> {
    let s = s.to_uppercase();
    if s.is_empty() || !s.chars().all(|c| "IVXLCDM".contains(c)) {
        return None;
    }

    let mut total: i64 = 0;
    let mut prev = 0i64;
    for ch in s.chars().rev() {
        let val = match ch {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };
        if val < prev {
            total -= val;
        } else {
            total += val;
        }
        prev = val;
    }
    Some(total)
}

/// Compute the union bounding box of a set of elements.
fn compute_union_bbox(elements: &[ContentElement]) -> BoundingBox {
    let mut result = BoundingBox::new(None, f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for elem in elements {
        let bb = elem.bbox();
        if bb.left_x < result.left_x {
            result.left_x = bb.left_x;
        }
        if bb.bottom_y < result.bottom_y {
            result.bottom_y = bb.bottom_y;
        }
        if bb.right_x > result.right_x {
            result.right_x = bb.right_x;
        }
        if bb.top_y > result.top_y {
            result.top_y = bb.top_y;
        }
        if result.page_number.is_none() {
            result.page_number = bb.page_number;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text_chunk(
        text: &str,
        page: u32,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
    ) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(Some(page), left_x, bottom_y, right_x, top_y),
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
            page_number: Some(page),
            level: None,
            mcid: None,
        })
    }

    #[test]
    fn test_no_pages() {
        let mut pages: Vec<Vec<ContentElement>> = vec![];
        detect_headers_footers(&mut pages, 842.0);
        assert!(pages.is_empty());
    }

    #[test]
    fn test_single_page_no_detection() {
        let mut pages = vec![vec![
            make_text_chunk("Header Text", 1, 72.0, 800.0, 300.0, 812.0),
            make_text_chunk("Body", 1, 72.0, 400.0, 300.0, 412.0),
        ]];
        detect_headers_footers(&mut pages, 842.0);
        // Single page — no header/footer detection
        assert_eq!(pages[0].len(), 2);
        assert!(matches!(pages[0][0], ContentElement::TextChunk(_)));
    }

    #[test]
    fn test_single_page_page_number_and_running_title_promoted_to_header() {
        let mut pages = vec![vec![
            make_text_chunk("314", 1, 62.0, 622.0, 84.0, 634.0),
            make_text_chunk("YARROW", 1, 343.0, 622.0, 391.0, 634.0),
            make_text_chunk(
                "1999 such iterations to form parameter distributions and continue the first body paragraph",
                1,
                62.0,
                480.0,
                392.0,
                620.0,
            ),
        ]];

        detect_headers_footers(&mut pages, 666.0);

        assert_eq!(pages[0].len(), 2);
        match &pages[0][0] {
            ContentElement::HeaderFooter(hf) => {
                assert_eq!(hf.semantic_type, SemanticType::Header);
                assert_eq!(hf.contents.len(), 2);
            }
            other => panic!("Expected HeaderFooter, got {other:?}"),
        }
        assert!(matches!(pages[0][1], ContentElement::TextChunk(_)));
    }

    #[test]
    fn test_identical_header_detected() {
        let mut pages = vec![
            vec![
                make_text_chunk("Chapter 1", 1, 72.0, 800.0, 300.0, 812.0),
                make_text_chunk("Body page 1", 1, 72.0, 400.0, 300.0, 412.0),
            ],
            vec![
                make_text_chunk("Chapter 1", 2, 72.0, 800.0, 300.0, 812.0),
                make_text_chunk("Body page 2", 2, 72.0, 400.0, 300.0, 412.0),
            ],
            vec![
                make_text_chunk("Chapter 1", 3, 72.0, 800.0, 300.0, 812.0),
                make_text_chunk("Body page 3", 3, 72.0, 400.0, 300.0, 412.0),
            ],
        ];
        detect_headers_footers(&mut pages, 842.0);

        // Each page should have 2 elements: HeaderFooter + body text
        for (i, page) in pages.iter().enumerate() {
            assert_eq!(page.len(), 2, "page {i} should have 2 elements");
            assert!(
                matches!(&page[0], ContentElement::HeaderFooter(hf) if hf.semantic_type == SemanticType::Header),
                "page {i} first element should be a Header"
            );
            assert!(
                matches!(&page[1], ContentElement::TextChunk(_)),
                "page {i} second element should be body TextChunk"
            );
        }
    }

    #[test]
    fn test_sequential_page_numbers_as_footer() {
        let mut pages = vec![
            vec![
                make_text_chunk("Body page 1", 1, 72.0, 400.0, 300.0, 412.0),
                make_text_chunk("1", 1, 280.0, 20.0, 300.0, 32.0),
            ],
            vec![
                make_text_chunk("Body page 2", 2, 72.0, 400.0, 300.0, 412.0),
                make_text_chunk("2", 2, 280.0, 20.0, 300.0, 32.0),
            ],
            vec![
                make_text_chunk("Body page 3", 3, 72.0, 400.0, 300.0, 412.0),
                make_text_chunk("3", 3, 280.0, 20.0, 300.0, 32.0),
            ],
        ];
        detect_headers_footers(&mut pages, 842.0);

        for (i, page) in pages.iter().enumerate() {
            assert_eq!(page.len(), 2, "page {i} should have 2 elements");
            // Footer should be appended at the end
            let has_footer = page
                .iter()
                .any(|e| matches!(e, ContentElement::HeaderFooter(hf) if hf.semantic_type == SemanticType::Footer));
            assert!(has_footer, "page {i} should have a Footer");
        }
    }

    #[test]
    fn test_roman_numeral_page_numbers() {
        assert_eq!(parse_roman("I"), Some(1));
        assert_eq!(parse_roman("IV"), Some(4));
        assert_eq!(parse_roman("IX"), Some(9));
        assert_eq!(parse_roman("XIV"), Some(14));
        assert_eq!(parse_roman("XLII"), Some(42));
        assert_eq!(parse_roman(""), None);
        assert_eq!(parse_roman("ABC"), None);
    }

    #[test]
    fn test_is_sequential_text() {
        assert!(is_sequential_text("1", "2"));
        assert!(is_sequential_text("42", "43"));
        assert!(is_sequential_text("I", "II"));
        assert!(is_sequential_text("III", "IV"));
        assert!(!is_sequential_text("hello", "world"));
        assert!(!is_sequential_text("1", "100"));
    }

    #[test]
    fn test_bbox_similar() {
        let a = BoundingBox::new(Some(1), 72.0, 20.0, 300.0, 32.0);
        let b = BoundingBox::new(Some(2), 72.0, 20.0, 300.0, 32.0);
        assert!(bbox_similar(&a, &b));

        let c = BoundingBox::new(Some(2), 100.0, 20.0, 400.0, 32.0);
        assert!(!bbox_similar(&a, &c));
    }

    #[test]
    fn test_middle_content_not_detected() {
        // Same text at middle of page should NOT be detected as header/footer
        let mut pages = vec![
            vec![make_text_chunk("Same text", 1, 72.0, 400.0, 300.0, 412.0)],
            vec![make_text_chunk("Same text", 2, 72.0, 400.0, 300.0, 412.0)],
        ];
        detect_headers_footers(&mut pages, 842.0);
        // No headers or footers should be detected (content is in middle zone)
        for page in &pages {
            for elem in page {
                assert!(
                    !matches!(elem, ContentElement::HeaderFooter(_)),
                    "Middle content should not become header/footer"
                );
            }
        }
    }

    #[test]
    fn test_header_and_footer_on_same_page() {
        let mut pages = vec![
            vec![
                make_text_chunk("Report Title", 1, 72.0, 800.0, 300.0, 812.0),
                make_text_chunk("Body", 1, 72.0, 400.0, 300.0, 412.0),
                make_text_chunk("1", 1, 280.0, 20.0, 300.0, 32.0),
            ],
            vec![
                make_text_chunk("Report Title", 2, 72.0, 800.0, 300.0, 812.0),
                make_text_chunk("Body", 2, 72.0, 400.0, 300.0, 412.0),
                make_text_chunk("2", 2, 280.0, 20.0, 300.0, 32.0),
            ],
        ];
        detect_headers_footers(&mut pages, 842.0);

        for (i, page) in pages.iter().enumerate() {
            let headers: Vec<_> = page
                .iter()
                .filter(|e| matches!(e, ContentElement::HeaderFooter(hf) if hf.semantic_type == SemanticType::Header))
                .collect();
            let footers: Vec<_> = page
                .iter()
                .filter(|e| matches!(e, ContentElement::HeaderFooter(hf) if hf.semantic_type == SemanticType::Footer))
                .collect();
            assert_eq!(headers.len(), 1, "page {i} should have 1 header");
            assert_eq!(footers.len(), 1, "page {i} should have 1 footer");
        }
    }
}
