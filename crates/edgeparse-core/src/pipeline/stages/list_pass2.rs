//! Stage 11: List Detection Pass 2 (Paragraph Level)
//!
//! After paragraphs are formed, this second pass detects sequences of
//! paragraphs that start with list labels and converts them into PDFList
//! structures, complementing Pass 1 which works at the TextLine level.

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::list::{ListBody, ListItem, ListLabel, PDFList};

/// Minimum labeled items to form a list (not counting body continuations).
const MIN_LIST_ITEMS: usize = 2;

/// Maximum consecutive non-labeled paragraphs absorbed as list item body.
/// Prevents runaway absorption of regular body text after a list ends.
const MAX_BODY_CONTINUATIONS: usize = 5;

/// For BracketNumber (bibliography) lists, allow more body continuations because
/// reference entries are frequently fragmented across columns with accent marks,
/// author initials parsed as letter labels, and cross-column fragments.
const MAX_BODY_CONTINUATIONS_BRACKET: usize = 12;

/// Detect lists from paragraph sequences and convert to PDFList.
pub fn detect_paragraph_lists(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    if elements.is_empty() {
        return elements;
    }

    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());
    let mut i = 0;
    let n = elements.len();

    while i < n {
        // Check if current element starts a list run
        if let Some(label_type) = paragraph_label_type(&elements[i]) {
            let run_start = i;
            let mut run_end = i + 1;
            let mut last_labeled_idx = i; // index of the most recent labeled item
            let mut consecutive_body = 0usize; // non-labeled items since last label
            let is_bracket = label_type == LabelType::BracketNumber;
            let is_letter = label_type == LabelType::Letter;
            let needs_sequence = is_bracket || is_letter;
            // For BracketNumber lists (bibliography), allow more body continuations
            // since references are often fragmented across columns and pages.
            let max_body = if is_bracket {
                MAX_BODY_CONTINUATIONS_BRACKET
            } else {
                MAX_BODY_CONTINUATIONS
            };

            // Track last seen sequence value for types that need sequential checking:
            // - BracketNumber: monotonically increasing (matches the reference ListLabelsUtils)
            // - Letter: strictly sequential (a→b→c) to avoid false positives from
            //   repeated subfigure references like "(b)...(b)..."
            let mut last_seq_value = if is_bracket {
                paragraph_bracket_number(&elements[i]).unwrap_or(0)
            } else if is_letter {
                paragraph_letter_value(&elements[i]).unwrap_or(0)
            } else {
                0
            };

            // Extend the run while consecutive paragraphs have compatible labels,
            // also allowing non-labeled continuation body paragraphs between items.
            while run_end < n {
                if let Some(lt) = paragraph_label_type(&elements[run_end]) {
                    if lt == label_type {
                        // Sequential checking for types that require it:
                        // - BracketNumber: monotonically increasing
                        // - Letter: strictly sequential (a→b→c, no repeats)
                        if needs_sequence {
                            let val = if is_bracket {
                                paragraph_bracket_number(&elements[run_end])
                            } else {
                                paragraph_letter_value(&elements[run_end])
                            };
                            if let Some(val) = val {
                                if is_bracket && val <= last_seq_value {
                                    break;
                                }
                                if is_letter && val != last_seq_value + 1 {
                                    break;
                                }
                                last_seq_value = val;
                            } else {
                                break;
                            }
                        }
                        // Same label type (and valid sequence) → extend the list
                        last_labeled_idx = run_end;
                        consecutive_body = 0;
                        run_end += 1;
                        continue;
                    }
                    // Different label type: might be a false positive (e.g., "A. Gordo"
                    // parsed as letter label when it's actually a reference body).
                    // Fall through to body-continuation check.
                }

                // Check if element is an existing List (from Stage 6.5/9) whose
                // first item has a compatible label. This handles the case where
                // list_detector already grouped (b)+ items into a list while (a)
                // remains as a paragraph.  Absorbing the existing list allows
                // proper grouping of (a) + (b) into one list.
                if let ContentElement::List(ref existing_list) = elements[run_end] {
                    if let Some(lt) = list_first_item_label_type(existing_list) {
                        if lt == label_type {
                            // Validate sequence for absorbed lists too
                            if needs_sequence {
                                if let Some(val) = list_first_item_letter_value(existing_list) {
                                    if is_letter && val != last_seq_value + 1 {
                                        break;
                                    }
                                    if is_bracket && val <= last_seq_value {
                                        break;
                                    }
                                    last_seq_value = val;
                                } else if needs_sequence {
                                    break;
                                }
                            }
                            last_labeled_idx = run_end;
                            consecutive_body = 0;
                            run_end += 1;
                            continue;
                        }
                    }
                }

                // No label (or incompatible label): check if it's a body continuation.
                if consecutive_body < max_body
                    && is_list_body_continuation(&elements, run_end, last_labeled_idx, is_bracket)
                {
                    consecutive_body += 1;
                    run_end += 1;
                    continue;
                }
                break;
            }

            // Count labeled entries in the run, including items from absorbed Lists.
            let labeled_count: usize = (run_start..run_end)
                .map(|j| {
                    if paragraph_label_type(&elements[j]) == Some(label_type) {
                        return 1;
                    }
                    if let ContentElement::List(ref list) = elements[j] {
                        return list.list_items.len();
                    }
                    0
                })
                .sum();

            if labeled_count >= MIN_LIST_ITEMS {
                // Build a PDFList from the run (labeled items + body continuations)
                let list = build_list_from_paragraphs(&elements[run_start..run_end]);
                // For BracketNumber (bibliography) lists, split at column
                // boundaries within the same page.  The reference TextLine-level
                // backward search naturally breaks at column alignment changes;
                // at paragraph level we must detect column jumps explicitly.
                if is_bracket {
                    for sub_list in split_bracket_list_at_columns(list) {
                        result.push(ContentElement::List(sub_list));
                    }
                } else {
                    result.push(ContentElement::List(list));
                }
                i = run_end;
                continue;
            }
        }

        result.push(elements[i].clone());
        i += 1;
    }

    // The common-prefix caption detection (Figure X, Table X sequences) is
    // handled at the document level by detect_common_prefix_lists_document()
    // because caption sequences span multiple pages.
    result
}

// ---------------------------------------------------------------------------
//  Document-level common-prefix list detection
// ---------------------------------------------------------------------------

/// Candidate with page + element index for document-level processing.
struct DocCaptionCandidate {
    page_idx: usize,
    elem_idx: usize,
    number: i64,
    left_x: f64,
}

/// Maximum distance (in points) from the minimum left-X for a candidate to be
/// considered "left-margin-aligned".  The reference TextLine-level backward search
/// effectively restricts matches to items in the same column; this threshold
/// approximates that behaviour at paragraph level.
const LEFT_MARGIN_TOLERANCE: f64 = 30.0;

/// Maximum gap (in caption numbers) that can be bridged between two left-margin
/// candidates when all intermediate numbers exist somewhere in the document
/// (at any X position).  The reference implementation detects these intermediates at TextLine level;
/// at paragraph level they may be embedded in larger paragraphs and invisible
/// to `extract_caption_number`, but their presence confirms the sequence is
/// real.  A gap of 2 means e.g. Figure 5 → Figure 7 with Figure 6 existing
/// in a different column.
const MAX_BRIDGE_GAP: i64 = 4;

/// Detect sequences of paragraphs/captions across all pages that share a common
/// prefix followed by a sequential number (e.g., "Figure 2", "Figure 3") and
/// convert each into a single-item list.  This mirrors the reference
/// `processListsFromTextNodes()` which operates on the full document.
///
/// Only left-margin-aligned candidates are considered, matching the reference
/// TextLine-level backward search which naturally restricts to same-column
/// items due to alignment-based break conditions.
pub fn detect_common_prefix_lists_document(pages: &mut [Vec<ContentElement>]) {
    let mut convert_set: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();

    for prefix in CAPTION_PREFIXES {
        let mut candidates: Vec<DocCaptionCandidate> = Vec::new();
        for (page_idx, page) in pages.iter().enumerate() {
            for (elem_idx, elem) in page.iter().enumerate() {
                if let Some(num) = extract_caption_number(elem, prefix) {
                    let left_x = elem.bbox().left_x;
                    candidates.push(DocCaptionCandidate {
                        page_idx,
                        elem_idx,
                        number: num,
                        left_x,
                    });
                }
            }
        }
        if candidates.len() < 2 {
            continue;
        }

        // Collect all caption numbers (from ALL candidates, before filtering)
        // so we can check whether gaps between left-margin candidates are
        // bridged by items that exist in other columns / embedded paragraphs.
        let all_numbers: std::collections::HashSet<i64> =
            candidates.iter().map(|c| c.number).collect();

        // Filter to left-margin-aligned candidates only.
        // The reference backward search naturally limits matches to same-column items.
        // We approximate this by keeping only candidates whose leftX is close
        // to the minimum leftX across all candidates for this prefix.
        let min_left_x = candidates.iter().map(|c| c.left_x).fold(f64::MAX, f64::min);
        candidates.retain(|c| (c.left_x - min_left_x).abs() < LEFT_MARGIN_TOLERANCE);
        if candidates.len() < 2 {
            continue;
        }

        // Sort by number for sequential chain detection (candidates may not
        // be in numeric order if pages contain non-sequential numbering).
        candidates.sort_by_key(|c| c.number);

        // Find maximal consecutive-number subsequences, allowing gaps up to
        // MAX_BRIDGE_GAP when every intermediate number exists somewhere in
        // the document (the full, unfiltered candidate set).  This bridges
        // items that the reference implementation detects at TextLine level but which are invisible
        // at paragraph level because they sit in different columns or are
        // embedded in larger paragraphs.
        let mut seq_start = 0;
        while seq_start < candidates.len() {
            let mut seq_end = seq_start + 1;
            while seq_end < candidates.len() {
                let prev_num = candidates[seq_end - 1].number;
                let curr_num = candidates[seq_end].number;
                let gap = curr_num - prev_num;
                if gap == 1 {
                    // Strictly sequential — always extend.
                    seq_end += 1;
                } else if (2..=MAX_BRIDGE_GAP).contains(&gap)
                    && (prev_num + 1..curr_num).all(|n| all_numbers.contains(&n))
                {
                    // Gap bridgeable: all intermediate numbers exist in the
                    // document, confirming the sequence is real even though
                    // the intermediates aren't at the left margin.
                    seq_end += 1;
                } else {
                    break;
                }
            }
            if seq_end - seq_start >= 2 {
                for c in &candidates[seq_start..seq_end] {
                    convert_set.insert((c.page_idx, c.elem_idx));
                }
            }
            seq_start = seq_end;
        }
    }

    if convert_set.is_empty() {
        return;
    }

    // Convert marked elements in-place to single-item lists.
    for (page_idx, page) in pages.iter_mut().enumerate() {
        let indices: Vec<usize> = (0..page.len())
            .filter(|ei| convert_set.contains(&(page_idx, *ei)))
            .collect();
        if indices.is_empty() {
            continue;
        }
        let old_page = std::mem::take(page);
        let mut new_page = Vec::with_capacity(old_page.len());
        for (ei, elem) in old_page.into_iter().enumerate() {
            if convert_set.contains(&(page_idx, ei)) {
                new_page.push(ContentElement::List(single_item_list(elem)));
            } else {
                new_page.push(elem);
            }
        }
        *page = new_page;
    }
}

// ---------------------------------------------------------------------------
//  Common-prefix list detection (mirrors the reference implementation processListsFromTextNodes)
// ---------------------------------------------------------------------------

/// Known prefixes for caption-style lists.  After these prefixes the text is
/// expected to start with a number (e.g., "Figure 2.", "Table 9:").
/// Only ASCII-case-insensitive matching is used so "FIGURE 1" is also caught.
const CAPTION_PREFIXES: &[&str] = &["Figure ", "Table ", "Algorithm "];

/// Try to extract a caption number from an element that starts with `prefix`.
/// Returns `None` if the element is not a paragraph/caption or doesn't match.
fn extract_caption_number(elem: &ContentElement, prefix: &str) -> Option<i64> {
    let text = match elem {
        ContentElement::Paragraph(p) => p.base.value(),
        ContentElement::Caption(c) => c.base.value(),
        _ => return None,
    };
    let trimmed = text.trim_start();
    // Case-insensitive prefix match using char-safe lowercasing.
    // All CAPTION_PREFIXES are ASCII, so converting to lowercase is fine.
    let lower = trimmed.to_ascii_lowercase();
    let lower_prefix = prefix.to_ascii_lowercase();
    if !lower.starts_with(&lower_prefix) {
        return None;
    }
    let after_prefix = &trimmed[prefix.len()..];
    // Extract leading digits
    let digit_end = after_prefix
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_prefix.len());
    if digit_end == 0 {
        return None;
    }
    let num_str = &after_prefix[..digit_end];
    num_str.parse::<i64>().ok()
}

/// Build a single-item PDFList from an element.
fn single_item_list(elem: ContentElement) -> PDFList {
    let bbox = elem.bbox().clone();
    let text = match &elem {
        ContentElement::Paragraph(p) => p.base.value(),
        ContentElement::Caption(c) => c.base.value(),
        _ => String::new(),
    };
    let label_len = detect_label_length(&text);

    let label = ListLabel {
        bbox: bbox.clone(),
        content: vec![],
        semantic_type: Some(SemanticType::ListLabel),
    };
    let body = ListBody {
        bbox: bbox.clone(),
        content: vec![],
        semantic_type: Some(SemanticType::ListBody),
    };
    let item = ListItem {
        bbox: bbox.clone(),
        index: None,
        level: None,
        label,
        body,
        label_length: label_len,
        contents: vec![elem],
        semantic_type: Some(SemanticType::ListItem),
    };

    PDFList {
        bbox,
        index: None,
        level: None,
        list_items: vec![item],
        numbering_style: None,
        common_prefix: None,
        previous_list_id: None,
        next_list_id: None,
    }
}

/// Check whether `elements[idx]` is a plausible body continuation of the
/// list item at `last_label_idx`.  Used to absorb fragmented reference entries
/// (e.g., accent-continuation lines) without breaking the list run.
///
/// When `is_bracket` is true (BracketNumber / bibliography lists), the check
/// is relaxed: any paragraph on the same page counts as a body continuation,
/// because bibliography entries are frequently fragmented across columns.
fn is_list_body_continuation(
    elements: &[ContentElement],
    idx: usize,
    last_label_idx: usize,
    is_bracket: bool,
) -> bool {
    let candidate = &elements[idx];

    // Only absorb plain paragraph elements.
    match candidate {
        ContentElement::Paragraph(p) => {
            // Headings, footers, captions etc. terminate the list.
            if matches!(
                p.base.semantic_type,
                SemanticType::Header
                    | SemanticType::Footer
                    | SemanticType::Heading
                    | SemanticType::Caption
                    | SemanticType::TableOfContent
                    | SemanticType::Note
            ) {
                return false;
            }
        }
        _ => return false, // Tables, images, lists → terminate
    }

    let label_bbox = elements[last_label_idx].bbox();
    let cand_bbox = candidate.bbox();

    // For BracketNumber (bibliography) lists, accept any paragraph on the same
    // page as the most recent labeled item.  Bibliography references in two-column
    // layouts produce fragments that overlap vertically with the parent paragraph
    // or sit in different columns, so strict vertical proximity checks fail.
    if is_bracket {
        return label_bbox.page_number == cand_bbox.page_number;
    }

    // Must be on the same page as the reference (labeled) item.
    if label_bbox.page_number != cand_bbox.page_number {
        return false;
    }

    // Must be close to the preceding element in the run.
    let prev = if idx > 0 {
        &elements[idx - 1]
    } else {
        return false;
    };
    let prev_bbox = prev.bbox();
    if prev_bbox.page_number != cand_bbox.page_number {
        return false;
    }

    // Use font size approximation from label bbox height, clamped to reasonable range.
    let font_approx = (label_bbox.top_y - label_bbox.bottom_y)
        .abs()
        .clamp(8.0, 20.0);

    // Compute vertical distance: how far below the previous element the candidate sits.
    // In PDF coords Y increases upward, so "below" means smaller Y.
    // Distance = bottom of prev - top of cand (positive means gap, negative means overlap).
    let vertical_dist =
        prev_bbox.bottom_y.min(prev_bbox.top_y) - cand_bbox.bottom_y.max(cand_bbox.top_y);
    // Allow overlap (negative distance) and gap up to 2.5× font size.
    if vertical_dist > font_approx * 2.5 {
        return false;
    }

    true
}

/// Label categories for matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LabelType {
    Bullet,
    Number,
    Letter,
    /// Bracket-number notation `[N]` — used for bibliography references.
    /// Unlike regular `Number`, allows gaps in numbering (e.g. [1] → [6] → [29])
    /// because references are often interleaved with body-continuation paragraphs
    /// that absorb intermediate entries.
    BracketNumber,
}

/// Font weight threshold above which text is considered bold.
const BOLD_WEIGHT_THRESHOLD: f64 = 600.0;

/// Detect if a paragraph starts with a list label.
fn paragraph_label_type(elem: &ContentElement) -> Option<LabelType> {
    let (text, font_weight) = match elem {
        ContentElement::Paragraph(p) => {
            if matches!(
                p.base.semantic_type,
                SemanticType::Header
                    | SemanticType::Footer
                    | SemanticType::Heading
                    | SemanticType::Note
                    | SemanticType::TableOfContent
            ) {
                return None;
            }
            (p.base.value(), p.base.font_weight.unwrap_or(400.0))
        }
        _ => return None,
    };

    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    let first_char = trimmed.chars().next()?;

    // Bullet characters
    if matches!(
        first_char,
        '•' | '◦'
            | '▪'
            | '▫'
            | '●'
            | '○'
            | '■'
            | '□'
            | '►'
            | '▸'
            | '‣'
            | '⁃'
            | '–'
            | '—'
    ) {
        return Some(LabelType::Bullet);
    }

    // Hyphen/dash followed by space
    if first_char == '-' && trimmed.len() > 1 && trimmed.chars().nth(1) == Some(' ') {
        // Dash followed by primarily-numeric content is likely table data (dash = N/A)
        // e.g. "- 91.2", "- - - 81.2 81.9" are table cells, not bullet items
        let after_space = &trimmed[2..];
        if !after_space.is_empty() && is_primarily_numeric(after_space) {
            return None;
        }
        return Some(LabelType::Bullet);
    }

    // Number followed by . or )
    if first_char.is_ascii_digit() {
        let rest = &trimmed[1..];
        // Consume more digits
        let after_digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        if let Some(after_dot) = after_digits.strip_prefix('.') {
            // Ensure the '.' is a label terminator (followed by space or end of text),
            // not a decimal point (followed by more digits).
            // "1. Item text" is a list label; "75.9 score" is a decimal number.
            if (after_dot.is_empty() || after_dot.starts_with(' ') || after_dot.starts_with('\t'))
                && font_weight < BOLD_WEIGHT_THRESHOLD
            {
                return Some(LabelType::Number);
            }
        } else if after_digits.starts_with(')') {
            // Bold numbered paragraphs are likely section headings, not list items.
            if font_weight < BOLD_WEIGHT_THRESHOLD {
                return Some(LabelType::Number);
            }
        }
    }

    // Parenthesized lowercase letter: (a), (b), (c) — common sub-list labels.
    // Note: we intentionally do NOT match parenthesized numbers (N) here at
    // paragraph level because academic papers use (1), (2), ... for equation
    // references which are not list items. list_detector handles (N) at the
    // TextLine level where sequential checking prevents false positives.
    if first_char == '(' {
        let after_paren = &trimmed[1..];
        if let Some(close) = after_paren.find(')') {
            let between = &after_paren[..close];
            if between.len() == 1 {
                let ch = between.chars().next().unwrap();
                if ch.is_ascii_lowercase() {
                    return Some(LabelType::Letter);
                }
            }
        }
    }

    // Letter followed by . or )
    // Only lowercase letters qualify (a., b., c.) — uppercase "F.", "A." etc.
    // are almost always name initials or abbreviations, not list labels.
    if first_char.is_ascii_lowercase() && trimmed.len() > 1 {
        let second = trimmed.chars().nth(1)?;
        if (second == '.' || second == ')')
            && (trimmed.len() == 2 || trimmed.chars().nth(2) == Some(' '))
        {
            return Some(LabelType::Letter);
        }
    }

    // Bracket notation: [N] for bibliography references — e.g. "[1]", "[12]", "[123]"
    if first_char == '[' {
        let after_bracket = &trimmed[1..];
        if let Some(end) = after_bracket.find(']') {
            let between = &after_bracket[..end];
            if !between.is_empty() && between.chars().all(|c| c.is_ascii_digit()) {
                return Some(LabelType::BracketNumber);
            }
        }
    }

    None
}

/// Extract the numeric value from a `[N]` bracket label in a paragraph.
/// Returns `None` if the element is not a paragraph or doesn't start with `[N]`.
fn paragraph_bracket_number(elem: &ContentElement) -> Option<i64> {
    let text = match elem {
        ContentElement::Paragraph(p) => p.base.value(),
        _ => return None,
    };
    let trimmed = text.trim_start();
    if let Some(after) = trimmed.strip_prefix('[') {
        if let Some(end) = after.find(']') {
            let between = &after[..end];
            if !between.is_empty() && between.chars().all(|c| c.is_ascii_digit()) {
                return between.parse::<i64>().ok();
            }
        }
    }
    None
}

/// Extract the sequence value from a letter label in a paragraph.
/// Returns the 1-based position: (a)→1, (b)→2, a.→1, b.→2, etc.
fn paragraph_letter_value(elem: &ContentElement) -> Option<i64> {
    let text = match elem {
        ContentElement::Paragraph(p) => p.base.value(),
        _ => return None,
    };
    letter_value_from_text(&text)
}

/// Extract letter sequence value from raw text.
/// Handles patterns: (a), (b), a., b., a), b)
fn letter_value_from_text(text: &str) -> Option<i64> {
    let trimmed = text.trim_start();
    let first_char = trimmed.chars().next()?;
    if first_char == '(' {
        let after_paren = &trimmed[1..];
        if let Some(close) = after_paren.find(')') {
            let between = &after_paren[..close];
            if between.len() == 1 {
                let ch = between.chars().next().unwrap();
                if ch.is_ascii_lowercase() {
                    return Some((ch as i64) - ('a' as i64) + 1);
                }
            }
        }
    } else if first_char.is_ascii_lowercase() && trimmed.len() > 1 {
        let second = trimmed.chars().nth(1)?;
        if second == '.' || second == ')' {
            return Some((first_char as i64) - ('a' as i64) + 1);
        }
    }
    None
}

/// Check if an existing List's first item starts with a given label type.
/// Used to detect compatible Lists that can be merged into a paragraph-level run.
/// Handles list items from list_detector (TextLine/TextBlock contents) as well
/// as items from list_pass2 (Paragraph contents).
fn list_first_item_label_type(list: &PDFList) -> Option<LabelType> {
    let first_item = list.list_items.first()?;
    // Try to get text from ANY content element type (TextLine, TextBlock, Paragraph)
    let text = first_item.contents.iter().find_map(content_element_text)?;
    text_label_type(&text)
}

/// Extract the letter sequence value from an existing List's first item.
fn list_first_item_letter_value(list: &PDFList) -> Option<i64> {
    let first_item = list.list_items.first()?;
    let text = first_item.contents.iter().find_map(content_element_text)?;
    letter_value_from_text(&text)
}

/// Extract text from any ContentElement variant.
fn content_element_text(elem: &ContentElement) -> Option<String> {
    match elem {
        ContentElement::Paragraph(p) => Some(p.base.value()),
        ContentElement::TextBlock(b) => Some(b.value()),
        ContentElement::TextLine(l) => Some(l.value()),
        ContentElement::TextChunk(t) => Some(t.value.clone()),
        _ => None,
    }
}

/// Detect label type from raw text (not tied to a specific element type).
fn text_label_type(text: &str) -> Option<LabelType> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let first_char = trimmed.chars().next()?;

    // Bullet characters
    if matches!(
        first_char,
        '•' | '◦'
            | '▪'
            | '▫'
            | '●'
            | '○'
            | '■'
            | '□'
            | '►'
            | '▸'
            | '‣'
            | '⁃'
            | '–'
            | '—'
    ) {
        return Some(LabelType::Bullet);
    }

    // Parenthesized lowercase letter: (a), (b), (c)
    if first_char == '(' {
        let after_paren = &trimmed[1..];
        if let Some(close) = after_paren.find(')') {
            let between = &after_paren[..close];
            if between.len() == 1 {
                let ch = between.chars().next().unwrap();
                if ch.is_ascii_lowercase() {
                    return Some(LabelType::Letter);
                }
            }
        }
    }

    // Number followed by . or )
    if first_char.is_ascii_digit() {
        let rest = &trimmed[1..];
        let after_digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
        if after_digits.starts_with('.') || after_digits.starts_with(')') {
            return Some(LabelType::Number);
        }
    }

    // Lowercase letter followed by . or )
    if first_char.is_ascii_lowercase() && trimmed.len() > 1 {
        let second = trimmed.chars().nth(1)?;
        if (second == '.' || second == ')')
            && (trimmed.len() == 2 || trimmed.chars().nth(2) == Some(' '))
        {
            return Some(LabelType::Letter);
        }
    }

    // Bracket number: [N]
    if first_char == '[' {
        let after_bracket = &trimmed[1..];
        if let Some(end) = after_bracket.find(']') {
            let between = &after_bracket[..end];
            if !between.is_empty() && between.chars().all(|c| c.is_ascii_digit()) {
                return Some(LabelType::BracketNumber);
            }
        }
    }

    None
}

/// Build a PDFList from a slice of elements that may include Paragraphs
/// (direct items/body) and existing Lists (whose items are absorbed).
fn build_list_from_paragraphs(elems: &[ContentElement]) -> PDFList {
    let mut bbox = elems[0].bbox().clone();
    let mut list_items = Vec::new();

    for elem in elems {
        bbox = bbox.union(elem.bbox());

        match elem {
            ContentElement::List(existing_list) => {
                // Absorb items from an existing List created by an earlier stage.
                for item in &existing_list.list_items {
                    list_items.push(item.clone());
                }
            }
            _ => {
                let text = match elem {
                    ContentElement::Paragraph(p) => p.base.value(),
                    _ => String::new(),
                };

                let label_len = detect_label_length(&text);

                let label = ListLabel {
                    bbox: elem.bbox().clone(),
                    content: vec![],
                    semantic_type: Some(SemanticType::ListLabel),
                };
                let body = ListBody {
                    bbox: elem.bbox().clone(),
                    content: vec![],
                    semantic_type: Some(SemanticType::ListBody),
                };
                list_items.push(ListItem {
                    bbox: elem.bbox().clone(),
                    index: None,
                    level: None,
                    label,
                    body,
                    label_length: label_len,
                    contents: vec![elem.clone()],
                    semantic_type: Some(SemanticType::ListItem),
                });
            }
        }
    }

    PDFList {
        bbox,
        index: None,
        level: None,
        list_items,
        numbering_style: None,
        common_prefix: None,
        previous_list_id: None,
        next_list_id: None,
    }
}

/// Minimum horizontal distance (in points) between consecutive labeled items
/// to consider a column boundary.  Two-column academic papers typically have
/// columns ~250pt apart, so 100pt is a safe threshold that avoids false
/// positives from indented continuations.
const COLUMN_JUMP_THRESHOLD: f64 = 100.0;

/// Split a BracketNumber (bibliography) list at column boundaries.
///
/// The reference TextLine-level backward search naturally breaks when column alignment
/// changes, producing separate lists for left and right columns on the same
/// page.  At paragraph level all items end up in one list because the reading
/// order places left-column items before right-column items.  This function
/// detects column transitions by examining the `left_x` of labeled items
/// (those starting with `[N]`) and splits the list at those points.
fn split_bracket_list_at_columns(list: PDFList) -> Vec<PDFList> {
    if list.list_items.len() < 4 {
        return vec![list];
    }

    // Identify indices of labeled items (content starts with '[').
    let labeled: Vec<usize> = list
        .list_items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let text = item
                .contents
                .iter()
                .find_map(content_element_text)
                .unwrap_or_default();
            if text.trim_start().starts_with('[') {
                Some(i)
            } else {
                None
            }
        })
        .collect();

    if labeled.len() < 3 {
        return vec![list];
    }

    // Find column-boundary split points: where the leftX of consecutive
    // labeled items jumps by more than COLUMN_JUMP_THRESHOLD.
    let mut split_at: Vec<usize> = Vec::new();
    for w in labeled.windows(2) {
        let prev_x = list.list_items[w[0]].bbox.left_x;
        let curr_x = list.list_items[w[1]].bbox.left_x;
        if (curr_x - prev_x).abs() > COLUMN_JUMP_THRESHOLD {
            split_at.push(w[1]);
        }
    }

    if split_at.is_empty() {
        return vec![list];
    }

    // Build sub-lists from the split points.
    let mut result: Vec<PDFList> = Vec::new();
    let mut start = 0;
    for &pos in &split_at {
        let items: Vec<ListItem> = list.list_items[start..pos].to_vec();
        if items.len() >= 2 {
            let bbox = items
                .iter()
                .skip(1)
                .fold(items[0].bbox.clone(), |acc, it| acc.union(&it.bbox));
            result.push(PDFList {
                bbox,
                index: None,
                level: None,
                list_items: items,
                numbering_style: None,
                common_prefix: None,
                previous_list_id: None,
                next_list_id: None,
            });
        }
        start = pos;
    }
    // Remaining items after the last split.
    let items: Vec<ListItem> = list.list_items[start..].to_vec();
    if items.len() >= 2 {
        let bbox = items
            .iter()
            .skip(1)
            .fold(items[0].bbox.clone(), |acc, it| acc.union(&it.bbox));
        result.push(PDFList {
            bbox,
            index: None,
            level: None,
            list_items: items,
            numbering_style: None,
            common_prefix: None,
            previous_list_id: None,
            next_list_id: None,
        });
    }

    // If splitting didn't produce at least 2 sub-lists, return original.
    if result.len() < 2 {
        return vec![list];
    }

    result
}

/// Detect the character length of the label portion of a list item text.
/// Check if text is primarily numeric (table data, not list content).
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

fn detect_label_length(text: &str) -> usize {
    let trimmed = text.trim_start();
    let leading_spaces = text.len() - trimmed.len();
    if trimmed.is_empty() {
        return 0;
    }
    let first = trimmed.chars().next().unwrap();
    // Bullet: single char + space
    if matches!(
        first,
        '•' | '◦'
            | '▪'
            | '▫'
            | '●'
            | '○'
            | '■'
            | '□'
            | '►'
            | '▸'
            | '‣'
            | '⁃'
            | '–'
            | '—'
    ) {
        return leading_spaces + first.len_utf8() + 1;
    }
    // Dash bullet
    if first == '-' && trimmed.len() > 1 && trimmed.chars().nth(1) == Some(' ') {
        return leading_spaces + 2;
    }
    // Number + . or )
    if first.is_ascii_digit() {
        let digits_end = trimmed
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(trimmed.len());
        if digits_end < trimmed.len() {
            let next = trimmed.as_bytes()[digits_end];
            if next == b'.' || next == b')' {
                return leading_spaces + digits_end + 2; // digit(s) + punct + space
            }
        }
    }
    // Parenthesized: (N) or (letter)
    if first == '(' {
        if let Some(close) = trimmed.find(')') {
            // (N) or (a) + space
            return leading_spaces + close + 2;
        }
    }
    // Letter + . or )
    if first.is_ascii_alphabetic() && trimmed.len() > 1 {
        let second = trimmed.as_bytes()[1];
        if second == b'.' || second == b')' {
            return leading_spaces + 3;
        }
    }
    // Bracket number: [N]
    if first == '[' {
        if let Some(close) = trimmed.find(']') {
            return leading_spaces + close + 2;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    fn make_para(text: &str, y: f64) -> ContentElement {
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(Some(1), 72.0, y, 500.0, y + 12.0),
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
    fn test_numbered_paragraphs_become_list() {
        let elements = vec![
            make_para("1. First item", 700.0),
            make_para("2. Second item", 688.0),
            make_para("3. Third item", 676.0),
        ];
        let result = detect_paragraph_lists(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::List(_)));
        if let ContentElement::List(list) = &result[0] {
            assert_eq!(list.list_items.len(), 3);
        }
    }

    #[test]
    fn test_bullet_paragraphs_become_list() {
        let elements = vec![
            make_para("• First bullet", 700.0),
            make_para("• Second bullet", 688.0),
        ];
        let result = detect_paragraph_lists(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::List(_)));
    }

    #[test]
    fn test_single_item_stays_paragraph() {
        let elements = vec![make_para("1. Only one item", 700.0)];
        let result = detect_paragraph_lists(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::Paragraph(_)));
    }

    #[test]
    fn test_mixed_no_list() {
        let elements = vec![
            make_para("Regular paragraph text", 700.0),
            make_para("Another paragraph", 688.0),
        ];
        let result = detect_paragraph_lists(elements);
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], ContentElement::Paragraph(_)));
    }
}
