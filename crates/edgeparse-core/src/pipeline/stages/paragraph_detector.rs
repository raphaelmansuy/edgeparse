//! Stage 10: Paragraph Detection
//!
//! Converts remaining TextBlock elements into SemanticParagraph nodes.
//! Adjacent TextBlocks with compatible alignment and font size are merged
//! before wrapping.

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
use crate::models::text::{TextBlock, TextColumn, TextLine};

/// Merge probability threshold — blocks need this overlap to be merged.
const MERGE_PROBABILITY: f64 = 0.75;

/// Font size tolerance for merging (as fraction of font size).
const FONT_SIZE_TOLERANCE: f64 = 0.15;

/// Maximum vertical gap (as multiple of font size) to merge blocks.
/// OODA-1: Reduced from 2.5 → 2.0 to avoid merging blocks with large gaps.
const MAX_GAP_FACTOR: f64 = 2.0;

/// Maximum width ratio of the first line to subsequent lines to consider it
/// a potential heading line for font-based splitting.
const HEADING_WIDTH_RATIO: f64 = 0.6;

/// Tolerance for alignment edge comparison (in points).
const ALIGNMENT_TOLERANCE: f64 = 3.0;

/// Detect paragraphs by merging compatible TextBlocks and wrapping them.
pub fn detect_paragraphs(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    if elements.is_empty() {
        return elements;
    }

    // Pre-pass: split TextBlocks that start with a short heading-font line.
    // This matches the reference first LEFT-alignment pass in ParagraphProcessor which
    // uses `areTextChunksHaveSameStyle()` (font name equality) to prevent merging
    // heading lines with body text.
    let elements = split_heading_first_lines(elements);

    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());
    let mut pending_block: Option<TextBlock> = None;

    for elem in elements {
        match elem {
            ContentElement::TextBlock(block) => {
                if let Some(prev) = pending_block.take() {
                    if should_merge(&prev, &block) {
                        // Merge blocks
                        pending_block = Some(merge_blocks(prev, block));
                    } else {
                        // Emit previous as paragraph, start new pending
                        result.push(wrap_paragraph(prev));
                        pending_block = Some(block);
                    }
                } else {
                    pending_block = Some(block);
                }
            }
            other => {
                // Non-TextBlock element — flush pending and pass through
                if let Some(prev) = pending_block.take() {
                    result.push(wrap_paragraph(prev));
                }
                result.push(other);
            }
        }
    }

    // Flush final pending block
    if let Some(prev) = pending_block.take() {
        result.push(wrap_paragraph(prev));
    }

    result
}

/// Check whether two TextBlocks should be merged into a single paragraph.
fn should_merge(a: &TextBlock, b: &TextBlock) -> bool {
    // Must be on the same page
    if a.bbox.page_number != b.bbox.page_number {
        return false;
    }
    if let (Some(a_col), Some(b_col)) = (a.level.as_deref(), b.level.as_deref()) {
        if a_col != b_col {
            return false;
        }
    }

    // Font size must be similar
    let max_size = a.font_size.max(b.font_size);
    if max_size > 0.0 && (a.font_size - b.font_size).abs() / max_size > FONT_SIZE_TOLERANCE {
        return false;
    }

    // Check vertical gap (a should be above b in PDF coords: a.bottom_y > b.top_y)
    let gap = a.bbox.bottom_y - b.bbox.top_y;
    if gap > a.font_size * MAX_GAP_FACTOR || gap < -a.font_size * 0.5 {
        return false;
    }

    if should_merge_parenthetical_heading_stack(a, b) {
        return true;
    }

    if should_merge_lowercase_continuation(a, b) {
        return true;
    }

    if should_keep_list_item_separate(a, b) {
        return false;
    }

    // Check horizontal alignment overlap
    let overlap = horizontal_overlap(&a.bbox, &b.bbox);
    if overlap < MERGE_PROBABILITY {
        return false;
    }

    // Same alignment preferred
    if a.text_alignment.is_some()
        && b.text_alignment.is_some()
        && a.text_alignment != b.text_alignment
    {
        return false;
    }

    // Hidden text mismatch
    if a.is_hidden_text != b.is_hidden_text {
        return false;
    }

    // Font weight compatibility — bold blocks normally don't merge with regular blocks,
    // EXCEPT when they are at essentially the same font size (bold run-in label pattern).
    // The reference tag-based paragraph formation keeps "TODO Initialization We initialize..."
    // as ONE paragraph element; Rust must replicate this for same-size blocks.
    // For different-size blocks (e.g., 11pt heading + 10pt body) the weight check
    // prevents merging, preserving the heading as a standalone paragraph.
    if !are_compatible_block_weights(a, b) {
        return false;
    }

    // Short-last-line guard: if block A's last line is significantly short of
    // the block's right margin in justified text, it ends a paragraph.
    // This prevents the paragraph_detector from re-merging blocks that the
    // text_block_grouper correctly split at paragraph boundaries.
    // OODA-2: Lowered line count guard from >=3 to >=2 to extend to shorter blocks.
    // OODA-3: Reduced threshold multiplier from 2.0 to 1.5 to catch more paragraph endings.
    if a.text_lines.len() >= 2 {
        let a_right = a
            .text_lines
            .iter()
            .map(|l| l.bbox.right_x)
            .fold(f64::NEG_INFINITY, f64::max);
        let tol = ALIGNMENT_TOLERANCE * 2.0;
        let near_margin = a
            .text_lines
            .iter()
            .filter(|l| (a_right - l.bbox.right_x) < tol)
            .count();
        // At least 60% of lines reach the right margin → justified text
        if near_margin * 5 >= a.text_lines.len() * 3 {
            if let Some(last_line) = a.text_lines.last() {
                let short_gap = a_right - last_line.bbox.right_x;
                let last_text = last_line.value();
                let trimmed = last_text.trim_end();
                let ends_hyphen = trimmed.ends_with('-')
                    || trimmed.ends_with('\u{00AD}')
                    || trimmed.ends_with('\u{2010}');
                let last_chars = trimmed.chars().count();
                let ends_sentence = trimmed.ends_with('.')
                    || trimmed.ends_with(')')
                    || trimmed.ends_with('!')
                    || trimmed.ends_with('?')
                    || trimmed.ends_with('"')
                    || trimmed.ends_with('\u{201D}');
                let is_real_sentence_end = last_chars >= 20 || ends_sentence;
                if short_gap > a.font_size.max(1.0) * 1.5
                    && !ends_hyphen
                    && is_real_sentence_end
                    && !looks_like_lowercase_block_continuation(b)
                {
                    return false;
                }
            }
        }
    }

    // OODA-4: Geometric first-line indentation detection.
    // In LaTeX/Word documents, new paragraphs often start with an indented first
    // line (\parindent). If block B's first line is significantly more indented
    // (larger left_x) than B's body text left margin, B is starting a new paragraph.
    // This signal is otherwise invisible to the Jaccard overlap check because
    // indented lines still share most of the horizontal extent with body lines.
    if block_first_line_is_indented(b) {
        let last_text = a.text_lines.last().map(|l| l.value()).unwrap_or_default();
        let trimmed = last_text.trim_end();
        let last_ends_hyphen = trimmed.ends_with('-')
            || trimmed.ends_with('\u{00AD}')
            || trimmed.ends_with('\u{2010}');
        if !last_ends_hyphen {
            return false;
        }
    }

    true
}

/// Geometric check: detect if a TextBlock starts with a first-line indentation
/// pattern — i.e., the first line is significantly more indented (larger left_x)
/// than the median left_x of the body text (lines 1..n).
///
/// This is the principal paragraph-boundary signal in LaTeX documents with
/// \parindent > 0. The threshold is 0.8× font_size to avoid triggering on
/// typical PDF coordinate noise (≤ 2pt).
fn block_first_line_is_indented(block: &TextBlock) -> bool {
    if block.text_lines.len() < 2 {
        return false;
    }
    let first_left = block.text_lines[0].bbox.left_x;
    let mut body_lefts: Vec<f64> = block.text_lines[1..]
        .iter()
        .map(|l| l.bbox.left_x)
        .collect();
    body_lefts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let body_left = body_lefts[body_lefts.len() / 2]; // median
    let font_size = block.font_size.max(1.0);
    // The indentation must exceed 0.8× font_size (typically 8–12pt) to
    // distinguish true paragraph indents from PDF rendering noise.
    first_left > body_left + font_size * 0.8
}

fn should_merge_parenthetical_heading_stack(a: &TextBlock, b: &TextBlock) -> bool {
    if a.text_lines.len() > 2 || b.text_lines.len() > 2 {
        return false;
    }

    let a_text = block_text(a);
    let b_text = block_text(b);
    if a_text.is_empty() || b_text.is_empty() {
        return false;
    }

    let b_parenthetical = b_text.starts_with('(') && b_text.contains(')');
    if !b_parenthetical {
        return false;
    }

    if a_text.chars().count() > 80 || b_text.chars().count() > 32 {
        return false;
    }
    if !contains_alpha(&a_text) || !contains_alpha(&b_text) {
        return false;
    }

    let center_delta = (a.bbox.center_x() - b.bbox.center_x()).abs();
    let center_tolerance = a.font_size.max(b.font_size) * 3.0;
    if center_delta > center_tolerance {
        return false;
    }

    match (block_dominant_font_name(a), block_dominant_font_name(b)) {
        (Some(name_a), Some(name_b))
            if normalize_font_family(&name_a) != normalize_font_family(&name_b) =>
        {
            return false;
        }
        _ => {}
    }

    true
}

fn block_text(block: &TextBlock) -> String {
    block
        .text_lines
        .iter()
        .map(TextLine::value)
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn contains_alpha(text: &str) -> bool {
    text.chars().any(|c| c.is_alphabetic())
}

fn should_merge_lowercase_continuation(a: &TextBlock, b: &TextBlock) -> bool {
    if !looks_like_lowercase_block_continuation(b) {
        return false;
    }

    let b_text = block_text(b);
    if b_text.split_whitespace().count() > 4 || b_text.chars().count() > 32 {
        return false;
    }

    let left_delta = (a.bbox.left_x - b.bbox.left_x).abs();
    if left_delta > a.font_size.max(b.font_size).max(1.0) * 1.5 {
        return false;
    }

    let gap = a.bbox.bottom_y - b.bbox.top_y;
    gap >= -a.font_size.max(1.0) * 0.5 && gap <= a.font_size.max(1.0) * MAX_GAP_FACTOR
}

fn looks_like_lowercase_block_continuation(block: &TextBlock) -> bool {
    let Some(first_line) = block.text_lines.first() else {
        return false;
    };
    let trimmed = first_line.value();
    for ch in trimmed.trim_start().chars() {
        if ch.is_alphabetic() {
            return ch.is_lowercase();
        }
        if !matches!(ch, '"' | '\'' | '(' | '[') {
            break;
        }
    }
    false
}

fn should_keep_list_item_separate(a: &TextBlock, b: &TextBlock) -> bool {
    let a_text = block_text(a);
    let b_text = block_text(b);
    if !starts_with_list_marker(&a_text) {
        return false;
    }
    if starts_with_list_marker(&b_text) || looks_like_lowercase_block_continuation(b) {
        return false;
    }

    let body_words = b_text.split_whitespace().count();
    body_words >= 6 && starts_with_uppercase_word(&b_text)
}

fn starts_with_list_marker(text: &str) -> bool {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.chars().next().unwrap();
    if matches!(
        first,
        '•' | '◦'
            | '▪'
            | '▸'
            | '▹'
            | '►'
            | '▻'
            | '●'
            | '○'
            | '■'
            | '□'
            | '◆'
            | '◇'
            | '→'
            | '➤'
            | '✓'
            | '✔'
            | '★'
            | '☆'
            | '➜'
            | '➢'
            | '⁃'
            | '‣'
            | '∙'
            | '⦿'
            | '⦾'
    ) {
        return true;
    }
    if (first == '-' || first == '\u{2013}' || first == '\u{2014}')
        && trimmed.chars().nth(1).is_some_and(|c| c.is_whitespace())
    {
        return true;
    }

    let digit_prefix_len = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    digit_prefix_len > 0
        && trimmed
            .chars()
            .nth(digit_prefix_len)
            .is_some_and(|c| c == '.' || c == ')')
}

fn starts_with_uppercase_word(text: &str) -> bool {
    for ch in text.trim_start().chars() {
        if ch.is_alphabetic() {
            return ch.is_uppercase();
        }
        if !matches!(ch, '"' | '\'' | '(' | '[') {
            break;
        }
    }
    false
}

/// Font weight (raw) threshold for bold classification.
const BOLD_WEIGHT_THRESHOLD: f64 = 550.0;

/// Check if two TextBlocks have compatible font weights for merging.
///
/// Same-boldness blocks are always compatible. Different-boldness blocks
/// are never merged — matching the reference `areCloseStyle()` in `ParagraphProcessor`,
/// which rejects merges when font weights differ by more than 0.1 (regular=400
/// vs bold=700 → diff=300 → always rejected).
///
/// This ensures bold headings like "4.1. Datasets" (NimbusRomNo9L-Medi, 700)
/// remain separate from regular body text (NimbusRomNo9L-Regu, 400) even when
/// their font sizes are nearly identical (10.934 vs 10.909).
fn are_compatible_block_weights(a: &TextBlock, b: &TextBlock) -> bool {
    let a_bold = block_is_bold(a);
    let b_bold = block_is_bold(b);
    a_bold == b_bold
}

/// Check if two TextBlocks have compatible dominant font names for merging.
///
/// Blocks with different font families should stay separate — this prevents
/// italic headings or headings in a different typeface from being merged with
/// body paragraphs. Uses the dominant (by character count) font name from the
/// first text line of each block.
#[allow(dead_code)]
fn are_compatible_block_font_names(a: &TextBlock, b: &TextBlock) -> bool {
    let name_a = block_dominant_font_name(a);
    let name_b = block_dominant_font_name(b);
    match (name_a, name_b) {
        (Some(na), Some(nb)) => {
            // Short blocks (≤ 2 chunks total) may be symbols or subscripts
            let a_chars: usize = a
                .text_lines
                .iter()
                .flat_map(|l| l.text_chunks.iter())
                .map(|c| c.value.chars().count())
                .sum();
            let b_chars: usize = b
                .text_lines
                .iter()
                .flat_map(|l| l.text_chunks.iter())
                .map(|c| c.value.chars().count())
                .sum();
            if a_chars <= 2 || b_chars <= 2 {
                return true;
            }
            normalize_font_family(&na) == normalize_font_family(&nb)
        }
        _ => true, // Missing font name → allow merge
    }
}

/// Get the dominant font name from a block's first text line (most characters).
fn block_dominant_font_name(block: &TextBlock) -> Option<String> {
    use std::collections::HashMap;
    let first_line = block.text_lines.first()?;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for chunk in &first_line.text_chunks {
        if !chunk.font_name.is_empty() {
            *counts.entry(&chunk.font_name).or_insert(0) += chunk.value.len().max(1);
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(name, _)| name.to_string())
}

/// Normalize a font name to its base family by stripping common style suffixes.
fn normalize_font_family(name: &str) -> String {
    let lower = name.to_lowercase();
    let stripped = lower
        .trim_end_matches("-bold")
        .trim_end_matches("-italic")
        .trim_end_matches("-bolditalic")
        .trim_end_matches("-regular")
        .trim_end_matches(",bold")
        .trim_end_matches(",italic")
        .trim_end_matches(",bolditalic")
        .trim_end_matches("-roman");
    stripped.to_string()
}

/// Returns true if the block's dominant font weight is bold.
fn block_is_bold(block: &TextBlock) -> bool {
    block_dominant_weight(block) >= BOLD_WEIGHT_THRESHOLD
}

/// Compute the dominant (by character count) font weight from the first text line.
fn block_dominant_weight(block: &TextBlock) -> f64 {
    let first_line = match block.text_lines.first() {
        Some(l) => l,
        None => return 400.0,
    };
    let total: usize = first_line
        .text_chunks
        .iter()
        .map(|c| c.value.len().max(1))
        .sum();
    if total == 0 {
        return 400.0;
    }
    let weighted: f64 = first_line
        .text_chunks
        .iter()
        .map(|c| c.font_weight * c.value.len().max(1) as f64)
        .sum();
    weighted / total as f64
}

/// Calculate horizontal overlap ratio between two bounding boxes using the
/// Jaccard index (intersection / union).
///
/// Jaccard correctly handles the case where one block is full-page-width and
/// the other is a narrow column block:
///   - Full-width abstract (72→522, w=450) vs left-column body (72→280, w=208):
///     intersection=208, union=450, Jaccard=0.46 → below MERGE_PROBABILITY → no merge ✓
///   - Same-column blocks (both 72→280, w=208):
///     intersection=208, union=208, Jaccard=1.0 → merge ✓
///
/// The old `overlap / min_width` metric caused full-width blocks to unconditionally
/// merge with any narrower block that starts at the same left edge.
fn horizontal_overlap(
    a: &crate::models::bbox::BoundingBox,
    b: &crate::models::bbox::BoundingBox,
) -> f64 {
    let left = a.left_x.max(b.left_x);
    let right = a.right_x.min(b.right_x);
    let overlap_width = (right - left).max(0.0);

    if overlap_width == 0.0 {
        return 0.0;
    }

    let a_width = a.width();
    let b_width = b.width();
    // Jaccard = intersection / union
    let union_width = a_width + b_width - overlap_width;

    if union_width <= 0.0 {
        return 0.0;
    }

    overlap_width / union_width
}

/// Merge two TextBlocks into one.
fn merge_blocks(mut a: TextBlock, b: TextBlock) -> TextBlock {
    a.bbox = a.bbox.union(&b.bbox);
    a.text_lines.extend(b.text_lines);
    a.has_end_line = b.has_end_line;
    if a.level != b.level {
        a.level = None;
    }
    // Keep alignment from first block if available
    if a.text_alignment.is_none() {
        a.text_alignment = b.text_alignment;
    }
    a
}

/// Wrap a TextBlock into a SemanticParagraph ContentElement.
fn wrap_paragraph(block: TextBlock) -> ContentElement {
    let bbox = block.bbox.clone();
    let font_size = block.font_size;
    let is_hidden = block.is_hidden_text;
    let level = block.level.clone();

    // Compute dominant font weight from underlying chunks
    let font_weight = dominant_font_weight(&block);

    let column = TextColumn {
        bbox: bbox.clone(),
        index: None,
        level: None,
        font_size,
        base_line: block.base_line,
        slant_degree: block.slant_degree,
        is_hidden_text: is_hidden,
        text_blocks: vec![block],
    };

    ContentElement::Paragraph(SemanticParagraph {
        base: SemanticTextNode {
            bbox,
            index: None,
            level,
            semantic_type: SemanticType::Paragraph,
            correct_semantic_score: None,
            columns: vec![column],
            font_weight: Some(font_weight),
            font_size: Some(font_size),
            text_color: None,
            italic_angle: None,
            font_name: None,
            text_format: None,
            max_font_size: Some(font_size),
            background_color: None,
            is_hidden_text: is_hidden,
        },
        enclosed_top: false,
        enclosed_bottom: false,
        indentation: 0,
    })
}

/// Compute the dominant (most frequent) font weight from all TextChunks in a block.
fn dominant_font_weight(block: &TextBlock) -> f64 {
    use std::collections::HashMap;
    let mut weight_counts: HashMap<i32, usize> = HashMap::new();
    for line in &block.text_lines {
        for chunk in &line.text_chunks {
            let key = chunk.font_weight.round() as i32;
            *weight_counts.entry(key).or_insert(0) += 1;
        }
    }
    weight_counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(key, _)| key as f64)
        .unwrap_or(400.0)
}

/// Compute the dominant (most characters) font name from all TextChunks in a block.
#[allow(dead_code)]
fn dominant_font_name(block: &TextBlock) -> Option<String> {
    use std::collections::HashMap;
    let mut name_counts: HashMap<&str, usize> = HashMap::new();
    for line in &block.text_lines {
        for chunk in &line.text_chunks {
            if !chunk.font_name.is_empty() {
                *name_counts.entry(&chunk.font_name).or_insert(0) += chunk.value.len().max(1);
            }
        }
    }
    name_counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(name, _)| name.to_string())
}

/// Split TextBlocks whose first line is a short, font-homogeneous heading
/// followed by significantly wider body lines.
///
/// Catches "run-in headings" like "4.1. Datasets | IIIT5K-Words…" where the
/// heading line (59pt) and body line (224pt) were merged into one TextBlock by
/// the text_block_grouper.  The reference ParagraphProcessor avoids this via multi-pass
/// alignment detection (short non-justified headings stay unassigned while body
/// lines are grouped by justify alignment first).
fn split_heading_first_lines(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    let mut result = Vec::with_capacity(elements.len() + 8);
    for elem in elements {
        match elem {
            ContentElement::TextBlock(block) if block.text_lines.len() >= 2 => {
                if should_split_heading(&block) {
                    let (head, rest) = split_block_at(block, 1);
                    result.push(ContentElement::TextBlock(head));
                    result.push(ContentElement::TextBlock(rest));
                } else {
                    result.push(ContentElement::TextBlock(block));
                }
            }
            other => result.push(other),
        }
    }
    result
}

/// Check if a TextBlock's first line looks like a run-in heading that should
/// be split off: significantly shorter than body lines AND homogeneous font
/// (all chunks in one font — typical for headings, section titles, captions).
///
/// This mirrors the reference multi-pass paragraph formation where short heading lines
/// (not justify-aligned) are never grouped with the body text below.  After the
/// split, the merge loop's Jaccard‐overlap check (0.75 threshold) prevents
/// re-merging because the narrow heading has very low horizontal overlap with
/// the wide body paragraph.
fn should_split_heading(block: &TextBlock) -> bool {
    let first = &block.text_lines[0];

    // First line must be significantly shorter than the widest body line
    let first_width = first.bbox.right_x - first.bbox.left_x;
    let max_body_width = block.text_lines[1..]
        .iter()
        .map(|l| l.bbox.right_x - l.bbox.left_x)
        .fold(0.0f64, f64::max);

    // Body lines must be substantial (typical column width)
    if max_body_width < 100.0 {
        return false;
    }

    if first_width / max_body_width >= HEADING_WIDTH_RATIO {
        return false;
    }

    // First line must be heading-like: short text, but not a word fragment
    let first_chars: usize = first.text_chunks.iter().map(|c| c.value.len()).sum();
    if !(6..=80).contains(&first_chars) {
        return false;
    }

    // Reject sentence fragments: heading text must NOT end with sentence-terminal
    // punctuation (period, comma, semicolon, hyphen, etc.).  Headings like
    // "4.1. Datasets" end with a word, not punctuation.
    let last_char = first
        .text_chunks
        .last()
        .and_then(|c| c.value.chars().last())
        .unwrap_or(' ');
    if matches!(
        last_char,
        '.' | ',' | ';' | ':' | '-' | '?' | '!' | ')' | ']'
    ) {
        return false;
    }

    // First line must be font-homogeneous (all chunks in the same font).
    // Headings are typically rendered in a single font face, while body text
    // fragments that happen to start a block usually have mixed inline styles.
    let first_font = match first.text_chunks.first() {
        Some(c) => c.font_name.as_str(),
        None => return false,
    };
    if !first.text_chunks.iter().all(|c| c.font_name == first_font) {
        return false;
    }

    // First line must be bold (heading-weight), and body must contain non-bold
    // text. This distinguishes section headings (bold, short) from sentence
    // fragments that happen to start a block (regular weight, short).
    let first_weight = first.text_chunks[0].font_weight;
    if first_weight < BOLD_WEIGHT_THRESHOLD {
        return false;
    }
    // Body must have at least one chunk in regular weight
    block.text_lines[1..]
        .iter()
        .flat_map(|l| l.text_chunks.iter())
        .any(|c| c.font_weight < BOLD_WEIGHT_THRESHOLD)
}

/// Split a TextBlock at a given line index, producing two blocks.
fn split_block_at(block: TextBlock, at: usize) -> (TextBlock, TextBlock) {
    let mut lines = block.text_lines;
    let rest_lines = lines.split_off(at);

    let head_bbox = lines
        .iter()
        .map(|l| l.bbox.clone())
        .reduce(|a, b| a.union(&b))
        .unwrap();
    let head_fs = lines.iter().map(|l| l.font_size).sum::<f64>() / lines.len() as f64;
    let head_bl = lines.last().map(|l| l.base_line).unwrap_or(0.0);
    let head_hidden = lines.iter().all(|l| l.is_hidden_text);

    let rest_bbox = rest_lines
        .iter()
        .map(|l| l.bbox.clone())
        .reduce(|a, b| a.union(&b))
        .unwrap();
    let rest_fs = rest_lines.iter().map(|l| l.font_size).sum::<f64>() / rest_lines.len() as f64;
    let rest_bl = rest_lines.last().map(|l| l.base_line).unwrap_or(0.0);
    let rest_hidden = rest_lines.iter().all(|l| l.is_hidden_text);

    let head = TextBlock {
        bbox: head_bbox,
        index: block.index,
        level: block.level.clone(),
        font_size: head_fs,
        base_line: head_bl,
        slant_degree: block.slant_degree,
        is_hidden_text: head_hidden,
        text_lines: lines,
        text_alignment: None,
        has_start_line: false,
        has_end_line: false,
    };

    let rest = TextBlock {
        bbox: rest_bbox,
        index: None,
        level: block.level,
        font_size: rest_fs,
        base_line: rest_bl,
        slant_degree: block.slant_degree,
        is_hidden_text: rest_hidden,
        text_lines: rest_lines,
        text_alignment: block.text_alignment,
        has_start_line: false,
        has_end_line: block.has_end_line,
    };

    (head, rest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::text::TextLine;

    fn make_text_block(
        text: &str,
        page: u32,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
    ) -> ContentElement {
        make_text_block_with_style(
            text,
            page,
            left_x,
            bottom_y,
            right_x,
            top_y,
            "Helvetica",
            12.0,
            400.0,
        )
    }

    fn make_text_block_with_style(
        text: &str,
        page: u32,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
        font_name: &str,
        font_size: f64,
        font_weight: f64,
    ) -> ContentElement {
        let bbox = BoundingBox::new(Some(page), left_x, bottom_y, right_x, top_y);
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: font_name.to_string(),
            font_size,
            font_weight,
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
        };
        let line = TextLine {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        ContentElement::TextBlock(TextBlock {
            bbox,
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        })
    }

    #[test]
    fn test_empty_input() {
        assert!(detect_paragraphs(vec![]).is_empty());
    }

    #[test]
    fn test_single_block_becomes_paragraph() {
        let elements = vec![make_text_block("Hello", 1, 72.0, 700.0, 300.0, 712.0)];
        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::Paragraph(_)));
    }

    #[test]
    fn test_two_close_blocks_merge() {
        let elements = vec![
            make_text_block("First line", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_block("Second line", 1, 72.0, 686.0, 300.0, 698.0),
        ];
        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 1);
        if let ContentElement::Paragraph(p) = &result[0] {
            assert_eq!(p.base.columns[0].text_blocks[0].text_lines.len(), 2);
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn test_short_lowercase_continuation_block_merges() {
        let elements = vec![
            make_text_block(
                "You can then compare the difference you actually obtained against this null distribution to generate a p value for your difference",
                1,
                72.0,
                220.0,
                392.0,
                236.0,
            ),
            make_text_block("of interest.", 1, 72.0, 206.0, 116.0, 222.0),
        ];

        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 1);
        match &result[0] {
            ContentElement::Paragraph(p) => {
                assert!(p.base.value().contains("of interest."));
            }
            other => panic!("Expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn test_list_item_block_does_not_absorb_following_paragraph() {
        let elements = vec![
            make_text_block(
                "• Republic Act No. 7877: Anti-Sexual Harassment Act of 1995 (February 14, 1995)",
                1,
                72.0,
                220.0,
                392.0,
                236.0,
            ),
            make_text_block(
                "During the first Aquino administration (1986-1992), three women sectoral representatives were appointed in Congress.",
                1,
                72.0,
                206.0,
                392.0,
                222.0,
            ),
        ];

        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_distant_blocks_separate() {
        let elements = vec![
            make_text_block("First paragraph", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_block("Second paragraph", 1, 72.0, 400.0, 300.0, 412.0),
        ];
        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0], ContentElement::Paragraph(_)));
        assert!(matches!(result[1], ContentElement::Paragraph(_)));
    }

    #[test]
    fn test_different_pages_separate() {
        let elements = vec![
            make_text_block("Page 1 text", 1, 72.0, 700.0, 300.0, 712.0),
            make_text_block("Page 2 text", 2, 72.0, 700.0, 300.0, 712.0),
        ];
        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_non_text_elements_pass_through() {
        use crate::models::chunks::ImageChunk;
        let elements = vec![
            make_text_block("Before", 1, 72.0, 700.0, 300.0, 712.0),
            ContentElement::Image(ImageChunk {
                bbox: BoundingBox::new(Some(1), 72.0, 500.0, 200.0, 600.0),
                index: None,
                level: None,
            }),
            make_text_block("After", 1, 72.0, 400.0, 300.0, 412.0),
        ];
        let result = detect_paragraphs(elements);
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], ContentElement::Paragraph(_)));
        assert!(matches!(result[1], ContentElement::Image(_)));
        assert!(matches!(result[2], ContentElement::Paragraph(_)));
    }

    #[test]
    fn test_paragraph_value() {
        let elements = vec![make_text_block("Hello world", 1, 72.0, 700.0, 300.0, 712.0)];
        let result = detect_paragraphs(elements);
        if let ContentElement::Paragraph(p) = &result[0] {
            assert_eq!(p.base.value(), "Hello world");
            assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn test_parenthetical_heading_continuation_merges() {
        let elements = vec![
            make_text_block_with_style(
                "Observations from the Spitzer Space Telescope",
                1,
                66.0,
                228.0,
                330.0,
                242.4,
                "CormorantGaramond-Regular",
                14.4,
                400.0,
            ),
            make_text_block_with_style(
                "(SST).",
                1,
                180.0,
                211.0,
                215.5,
                225.4,
                "CormorantGaramond-Regular",
                14.4,
                400.0,
            ),
        ];

        let result = detect_paragraphs(elements);

        assert_eq!(result.len(), 1);
        match &result[0] {
            ContentElement::Paragraph(p) => {
                assert_eq!(p.base.columns[0].text_blocks[0].text_lines.len(), 2);
                assert!(p.base.value().contains("(SST)."));
            }
            other => panic!("Expected Paragraph, got {other:?}"),
        }
    }
}
