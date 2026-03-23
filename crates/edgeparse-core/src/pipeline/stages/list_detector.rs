//! Stage 9: List Detection (Pass 1 — TextLine level)
//!
//! Detects ordered and unordered lists by analysing the label pattern at
//! the start of each TextLine or TextBlock. Consecutive elements whose
//! labels form a valid sequence (bullets, numbers, letters) are grouped
//! into `PDFList` containers.

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::list::{ListBody, ListItem, ListLabel, PDFList};
use crate::models::table::TableTokenRow;

/// Maximum X-position gap (as a fraction of font size) for a line to be
/// considered part of the same list item body.
const LIST_BODY_X_RATIO: f64 = 0.3;

/// Minimum number of items to form a list.
const MIN_LIST_ITEMS: usize = 2;

/// Bullet characters recognised as unordered list labels.
const BULLET_CHARS: &[char] = &[
    '•', '◦', '▪', '▸', '▹', '►', '▻', '●', '○', '■', '□', '◆', '◇', '→', '➤', '✓', '✔', '★', '☆',
    '➜', '➢', '⁃', '‣', '∙', '⦿', '⦾',
];

/// Detect lists in a page of content elements.
///
/// Scans the elements for label patterns and groups consecutive labelled
/// elements into `PDFList` objects. Unlabelled elements between list items
/// are treated as continuation body if they are indented.
pub fn detect_lists(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    if elements.is_empty() {
        return elements;
    }

    // Build label info for each element
    let labels: Vec<Option<DetectedLabel>> = elements.iter().map(detect_label).collect();

    // Find runs of consecutive labels that form a list
    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());
    let mut i = 0;
    let n = elements.len();

    while i < n {
        if let Some(ref first_label) = labels[i] {
            // Try to extend a list starting at i
            let mut list_end = i + 1;
            let mut items_info: Vec<(usize, DetectedLabel)> = vec![(i, first_label.clone())];

            while list_end < n {
                if let Some(ref next_label) = labels[list_end] {
                    if labels_compatible(first_label, next_label)
                        && is_sequential_label(
                            items_info.last().map(|(_, l)| l).unwrap(),
                            next_label,
                        )
                    {
                        items_info.push((list_end, next_label.clone()));
                        list_end += 1;
                        continue;
                    }
                }
                // Check if this is a body continuation line (indented, no label)
                if labels[list_end].is_none()
                    && is_body_continuation(&elements, &items_info, list_end)
                {
                    list_end += 1;
                    continue;
                }
                break;
            }

            if items_info.len() >= MIN_LIST_ITEMS {
                // Build PDFList from elements[i..list_end]
                let list = build_list(&elements, &items_info, i, list_end);
                result.push(ContentElement::List(list));
                i = list_end;
                continue;
            }
        }
        // Not part of a list — pass through
        result.push(elements[i].clone());
        i += 1;
    }

    result
}

/// A detected label at the start of an element.
#[derive(Debug, Clone)]
struct DetectedLabel {
    /// The label text (e.g., "1.", "•", "a)")
    label_text: String,
    /// The category of label
    category: LabelCategory,
    /// Sequence value (for ordered labels)
    sequence_value: i64,
}

/// Category of list label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelCategory {
    Bullet,
    ArabicNumber,
    /// Bracket-number notation `[N]` — used for bibliography references.
    BracketNumber,
    LowercaseLetter,
    UppercaseLetter,
    RomanLower,
    RomanUpper,
}

/// Try to detect a list label at the start of an element's text.
fn detect_label(elem: &ContentElement) -> Option<DetectedLabel> {
    let text = element_text(elem)?;
    let text = text.trim_start();
    if text.is_empty() {
        return None;
    }

    // Check bullet characters
    let first_char = text.chars().next()?;
    if BULLET_CHARS.contains(&first_char) {
        return Some(DetectedLabel {
            label_text: first_char.to_string(),
            category: LabelCategory::Bullet,
            sequence_value: 0,
        });
    }

    // Check dash bullet (must be followed by space)
    if (first_char == '-' || first_char == '\u{2013}' || first_char == '\u{2014}')
        && text.len() > 1
        && text.chars().nth(1).is_some_and(|c| c == ' ' || c == '\t')
    {
        // Dash followed by primarily-numeric content is likely table data (dash = N/A)
        // e.g. "- 91.2", "- 88.3 95.0", "- - - 81.2 81.9"
        let after_space = text.get(2..).unwrap_or("").trim();
        if !after_space.is_empty() && is_primarily_numeric(after_space) {
            return None;
        }
        return Some(DetectedLabel {
            label_text: first_char.to_string(),
            category: LabelCategory::Bullet,
            sequence_value: 0,
        });
    }

    // Bracket notation "[N]" for bibliography references (e.g. "[1]", "[12]")
    // is NOT detected here at the TextLine/TextBlock level, because in
    // two-column layouts the text line grouper interleaves columns by Y-position,
    // making sequential matching impossible.  Instead, bracket-number lists
    // are detected in list_pass2 (Stage 11) after reading-order sort has
    // separated columns.  This matches the reference implementation approach where `[N]` is in
    // ARABIC_NUMBER_REGEXES (used in processListsFromTextNodes) but NOT in
    // BULLET_REGEXES/isLabeledLine (used in processLists at TextLine level).
    // The text block grouper's [N]+uppercase heuristic still prevents
    // bibliography entries from being merged into one block.

    // Extract the label prefix before the first space
    let label_end = text.find([' ', '\t']).unwrap_or(text.len());
    let candidate = &text[..label_end];

    // Try patterns: "N.", "N)", "(N)", where N is number
    // Bold numbered text is likely a section heading, not a list item.
    if let Some(label) = try_arabic_number(candidate) {
        if element_font_weight(elem) >= BOLD_WEIGHT_THRESHOLD {
            return None;
        }
        return Some(label);
    }

    // Try letters: "a.", "a)", "(a)", "A.", etc.
    if let Some(label) = try_letter(candidate) {
        return Some(label);
    }

    // Try Roman numerals: "i.", "i)", "I.", "I)", "(iv)", etc.
    if let Some(label) = try_roman(candidate) {
        return Some(label);
    }

    None
}

/// Try to parse an Arabic number label like "1.", "2)", "(3)"
fn try_arabic_number(s: &str) -> Option<DetectedLabel> {
    // Pattern: N. or N) or (N)
    let (num_str, suffix) = if let Some(stripped) = s.strip_prefix('(') {
        let stripped = stripped.strip_suffix(')')?;
        (stripped, format!("({stripped})"))
    } else if let Some(stripped) = s.strip_suffix('.') {
        (stripped, format!("{stripped}."))
    } else if let Some(stripped) = s.strip_suffix(')') {
        (stripped, format!("{stripped})"))
    } else {
        return None;
    };

    let num: i64 = num_str.parse().ok()?;
    Some(DetectedLabel {
        label_text: suffix,
        category: LabelCategory::ArabicNumber,
        sequence_value: num,
    })
}

/// Try to parse a letter label like "a.", "b)", "(c)", "A.", "B)"
fn try_letter(s: &str) -> Option<DetectedLabel> {
    let (letter_str, suffix) = if let Some(stripped) = s.strip_prefix('(') {
        let stripped = stripped.strip_suffix(')')?;
        (stripped, format!("({stripped})"))
    } else if let Some(stripped) = s.strip_suffix('.') {
        (stripped, format!("{stripped}."))
    } else if let Some(stripped) = s.strip_suffix(')') {
        (stripped, format!("{stripped})"))
    } else {
        return None;
    };

    if letter_str.len() != 1 {
        return None;
    }
    let ch = letter_str.chars().next()?;

    if ch.is_ascii_lowercase() {
        Some(DetectedLabel {
            label_text: suffix,
            category: LabelCategory::LowercaseLetter,
            sequence_value: (ch as i64) - ('a' as i64) + 1,
        })
    } else if ch.is_ascii_uppercase() {
        Some(DetectedLabel {
            label_text: suffix,
            category: LabelCategory::UppercaseLetter,
            sequence_value: (ch as i64) - ('A' as i64) + 1,
        })
    } else {
        None
    }
}

/// Try to parse a Roman numeral label like "i.", "iv)", "(IX)", "III."
fn try_roman(s: &str) -> Option<DetectedLabel> {
    let (roman_str, suffix) = if let Some(stripped) = s.strip_prefix('(') {
        let stripped = stripped.strip_suffix(')')?;
        (stripped, format!("({stripped})"))
    } else if let Some(stripped) = s.strip_suffix('.') {
        (stripped, format!("{stripped}."))
    } else if let Some(stripped) = s.strip_suffix(')') {
        (stripped, format!("{stripped})"))
    } else {
        return None;
    };

    if roman_str.is_empty() {
        return None;
    }

    let is_lower = roman_str.chars().all(|c| "ivxlcdm".contains(c));
    let is_upper = roman_str.chars().all(|c| "IVXLCDM".contains(c));

    if !is_lower && !is_upper {
        return None;
    }

    // Avoid single-letter ambiguity with regular letter labels (a-z)
    // Only accept Roman if it's clearly a multi-char sequence
    // or is one of: i, v, x, l, c, d, m (case-insensitive)
    let upper = roman_str.to_uppercase();
    let val = parse_roman_value(&upper)?;

    Some(DetectedLabel {
        label_text: suffix,
        category: if is_lower {
            LabelCategory::RomanLower
        } else {
            LabelCategory::RomanUpper
        },
        sequence_value: val,
    })
}

/// Parse Roman numeral string to integer value.
fn parse_roman_value(s: &str) -> Option<i64> {
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
    if total <= 0 {
        return None;
    }
    Some(total)
}

/// Check if two labels are from the same category.
fn labels_compatible(a: &DetectedLabel, b: &DetectedLabel) -> bool {
    a.category == b.category
}

/// Check if label `b` follows label `a` in sequence.
fn is_sequential_label(a: &DetectedLabel, b: &DetectedLabel) -> bool {
    match a.category {
        LabelCategory::Bullet => true, // Bullets are always compatible
        _ => b.sequence_value == a.sequence_value + 1,
    }
}

/// Check if element at `idx` could be a body continuation of the last list item.
///
/// For BracketNumber (bibliography) lists, the check is relaxed: any element
/// on the same page and vertically close counts as body continuation, because
/// bibliography entries frequently have no hanging indent and continuation
/// lines start at the same X position as the bracket label.
fn is_body_continuation(
    elements: &[ContentElement],
    items: &[(usize, DetectedLabel)],
    idx: usize,
) -> bool {
    if items.is_empty() || idx >= elements.len() {
        return false;
    }

    let last_item_idx = items.last().unwrap().0;
    let last_label = &items.last().unwrap().1;
    let last_elem = &elements[last_item_idx];
    let candidate = &elements[idx];

    let last_bbox = last_elem.bbox();
    let cand_bbox = candidate.bbox();

    // Must be on the same page
    if last_bbox.page_number != cand_bbox.page_number {
        return false;
    }

    let font_size = element_font_size(last_elem).unwrap_or(12.0);

    // For BracketNumber lists (bibliography), use relaxed check:
    // same page, vertically close, no strict indentation required.
    if last_label.category == LabelCategory::BracketNumber {
        // Use the previous element (not necessarily the label) for vertical gap
        let prev_elem = if idx > 0 {
            &elements[idx - 1]
        } else {
            last_elem
        };
        let prev_bbox = prev_elem.bbox();
        if prev_bbox.page_number != cand_bbox.page_number {
            return false;
        }
        let v_gap = prev_bbox.bottom_y - cand_bbox.top_y;
        // Allow overlap and gap up to 2.5× font size
        return v_gap >= -font_size * 0.5 && v_gap <= font_size * 2.5;
    }

    // Must be immediately below (within reasonable gap)
    let vertical_gap = last_bbox.bottom_y - cand_bbox.top_y;
    if vertical_gap < -font_size * 0.5 || vertical_gap > font_size * 2.0 {
        return false;
    }

    // Must be indented (X position to the right of label start)
    let x_diff = cand_bbox.left_x - last_bbox.left_x;
    x_diff > font_size * LIST_BODY_X_RATIO
}

/// Build a PDFList from a range of elements.
fn build_list(
    elements: &[ContentElement],
    items_info: &[(usize, DetectedLabel)],
    start: usize,
    end: usize,
) -> PDFList {
    let mut list_items: Vec<ListItem> = Vec::new();
    let numbering_style = items_info.first().map(|(_, l)| l.label_text.clone());

    for (item_i, (item_idx, label)) in items_info.iter().enumerate() {
        // Find body elements: all elements after this label until next label
        let next_label_idx = if item_i + 1 < items_info.len() {
            items_info[item_i + 1].0
        } else {
            end
        };

        let item_element = &elements[*item_idx];
        let mut item_bbox = item_element.bbox().clone();

        // Collect body elements
        let mut body_elements: Vec<ContentElement> = Vec::new();
        for elem in elements.iter().take(next_label_idx).skip(*item_idx + 1) {
            body_elements.push(elem.clone());
            item_bbox = item_bbox.union(elem.bbox());
        }

        let label_bbox = item_element.bbox().clone();
        let body_bbox = if body_elements.is_empty() {
            label_bbox.clone()
        } else {
            let mut bb = body_elements[0].bbox().clone();
            for be in &body_elements[1..] {
                bb = bb.union(be.bbox());
            }
            bb
        };

        list_items.push(ListItem {
            bbox: item_bbox,
            index: None,
            level: None,
            label: ListLabel {
                bbox: label_bbox,
                content: Vec::<TableTokenRow>::new(),
                semantic_type: Some(SemanticType::ListLabel),
            },
            body: ListBody {
                bbox: body_bbox,
                content: Vec::<TableTokenRow>::new(),
                semantic_type: Some(SemanticType::ListBody),
            },
            label_length: label.label_text.len(),
            contents: std::iter::once(item_element.clone())
                .chain(body_elements)
                .collect(),
            semantic_type: Some(SemanticType::ListItem),
        });
    }

    // Compute list bounding box
    let list_bbox = if !list_items.is_empty() {
        let mut bb = list_items[0].bbox.clone();
        for item in &list_items[1..] {
            bb = bb.union(&item.bbox);
        }
        bb
    } else {
        elements[start].bbox().clone()
    };

    PDFList {
        bbox: list_bbox,
        index: None,
        level: None,
        list_items,
        numbering_style,
        common_prefix: None,
        previous_list_id: None,
        next_list_id: None,
    }
}

/// Check if text is primarily numeric (digits, dots, spaces, dashes, commas).
/// Returns true if alphabetic characters make up less than 30% of total characters.
fn is_primarily_numeric(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let alpha_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
    let total = trimmed.chars().count();
    alpha_count * 100 / total < 30
}

/// Extract the text content of a content element.
fn element_text(elem: &ContentElement) -> Option<String> {
    match elem {
        ContentElement::TextChunk(t) => Some(t.value.clone()),
        ContentElement::TextLine(l) => Some(l.value()),
        ContentElement::TextBlock(b) => Some(b.value()),
        _ => None,
    }
}

/// Extract font size from a content element.
fn element_font_size(elem: &ContentElement) -> Option<f64> {
    match elem {
        ContentElement::TextChunk(t) => Some(t.font_size),
        ContentElement::TextLine(l) => Some(l.font_size),
        ContentElement::TextBlock(b) => Some(b.font_size),
        _ => None,
    }
}

/// Font weight threshold above which text is considered bold.
const BOLD_WEIGHT_THRESHOLD: f64 = 600.0;

/// Extract dominant font weight from a content element.
fn element_font_weight(elem: &ContentElement) -> f64 {
    match elem {
        ContentElement::TextChunk(t) => t.font_weight,
        ContentElement::TextLine(l) => l.text_chunks.first().map_or(400.0, |tc| tc.font_weight),
        ContentElement::TextBlock(b) => b
            .text_lines
            .first()
            .and_then(|tl| tl.text_chunks.first())
            .map_or(400.0, |tc| tc.font_weight),
        _ => 400.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text_line(
        text: &str,
        page: u32,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
    ) -> ContentElement {
        use crate::models::text::TextLine;
        ContentElement::TextLine(TextLine {
            bbox: BoundingBox::new(Some(page), left_x, bottom_y, right_x, top_y),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![TextChunk {
                value: text.to_string(),
                bbox: BoundingBox::new(Some(page), left_x, bottom_y, right_x, top_y),
                font_name: "Helvetica".to_string(),
                font_size: 12.0,
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
            }],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        })
    }

    #[test]
    fn test_empty_input() {
        assert!(detect_lists(vec![]).is_empty());
    }

    #[test]
    fn test_no_lists() {
        let elements = vec![
            make_text_line("Hello world", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("Another line", 1, 72.0, 686.0, 300.0, 698.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], ContentElement::TextLine(_)));
    }

    #[test]
    fn test_bullet_list() {
        let elements = vec![
            make_text_line("• First item", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("• Second item", 1, 72.0, 686.0, 300.0, 698.0),
            make_text_line("• Third item", 1, 72.0, 672.0, 300.0, 684.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], ContentElement::List(list) if list.list_items.len() == 3));
    }

    #[test]
    fn test_numbered_list() {
        let elements = vec![
            make_text_line("1. First item", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("2. Second item", 1, 72.0, 686.0, 300.0, 698.0),
            make_text_line("3. Third item", 1, 72.0, 672.0, 300.0, 684.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 1);
        if let ContentElement::List(list) = &result[0] {
            assert_eq!(list.list_items.len(), 3);
            assert_eq!(list.numbering_style.as_deref(), Some("1."));
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_letter_list() {
        let elements = vec![
            make_text_line("a) First", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("b) Second", 1, 72.0, 686.0, 300.0, 698.0),
            make_text_line("c) Third", 1, 72.0, 672.0, 300.0, 684.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 1);
        if let ContentElement::List(list) = &result[0] {
            assert_eq!(list.list_items.len(), 3);
        } else {
            panic!("Expected List");
        }
    }

    #[test]
    fn test_single_item_not_a_list() {
        let elements = vec![
            make_text_line("1. Only one item", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("Regular text", 1, 72.0, 686.0, 300.0, 698.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], ContentElement::TextLine(_)));
    }

    #[test]
    fn test_dash_bullet() {
        let elements = vec![
            make_text_line("- First item", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("- Second item", 1, 72.0, 686.0, 300.0, 698.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0], ContentElement::List(list) if list.list_items.len() == 2));
    }

    #[test]
    fn test_list_then_non_list() {
        let elements = vec![
            make_text_line("1. First", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_line("2. Second", 1, 72.0, 686.0, 300.0, 698.0),
            make_text_line("Regular paragraph", 1, 72.0, 650.0, 300.0, 662.0),
        ];
        let result = detect_lists(elements);
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0], ContentElement::List(_)));
        assert!(matches!(&result[1], ContentElement::TextLine(_)));
    }

    #[test]
    fn test_roman_numeral_labels() {
        assert!(try_roman("i.").is_some());
        assert!(try_roman("ii.").is_some());
        assert!(try_roman("III.").is_some());
        assert!(try_roman("(iv)").is_some());
        assert!(try_roman("xyz.").is_none());
    }

    #[test]
    fn test_detect_label_patterns() {
        let bullet = ContentElement::TextChunk(TextChunk {
            value: "• Hello".to_string(),
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 12.0),
            font_name: "H".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000".to_string(),
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
        let label = detect_label(&bullet).unwrap();
        assert_eq!(label.category, LabelCategory::Bullet);

        let num = ContentElement::TextChunk(TextChunk {
            value: "3. Something".to_string(),
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 12.0),
            font_name: "H".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000".to_string(),
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
        let label = detect_label(&num).unwrap();
        assert_eq!(label.category, LabelCategory::ArabicNumber);
        assert_eq!(label.sequence_value, 3);
    }
}
