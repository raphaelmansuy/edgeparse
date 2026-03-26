//! Stage 7: Text Block Grouper — group adjacent TextLines into TextBlocks.
//!
//! ```text
//!   TextLines (top to bottom):
//!     L1: "Introduction"       (font=14, bold)     ─┐
//!     L2: "This paper shows"   (font=12)            │ gap=14pt → merge?
//!     L3: "that parsing is"    (font=12)            │ gap=12pt → merge ✓
//!     L4: "important."         (font=12)           ─┘ → Block A (body para)
//!                                                      gap=30pt → split ✗
//!     L5: "Related Work"       (font=14, bold)     ─┐  → Block B (heading)
//! ```
//!
//! Groups TextLines that belong to the same paragraph/block based on:
//! - Vertical proximity (line spacing via mergeLeadingProbability formula)
//! - Font size consistency
//! - Text alignment (left, center, right, justify)

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::models::enums::TextAlignment;
use crate::models::text::{TextBlock, TextLine};
use std::collections::HashMap;

/// Maximum line spacing ratio relative to font size used for garbage-collecting
/// distant active blocks. The actual merge decision uses mergeLeadingProbability.
const MAX_LINE_SPACING_RATIO: f64 = 2.5;

/// DATA_LOADER_DEFAULT_FONT_LEADING_INTERVAL: [lo, hi] for normalized
/// baseline gap.  Uses the data-loader variant (0.7, 2.2) from
/// ChunksMergeUtils.mergeLeadingProbability — the wider range allows lines
/// separated by up to 2.2× font size to merge. This branch is always active
/// in edgeparse's pipeline.
const LEADING_LO: f64 = 0.7;
const LEADING_HI: f64 = 2.2;
const LEADING_STEP: f64 = 1.0;

/// DIFFERENT_LINES_PROBABILITY threshold — the minimum merge-probability
/// for two lines to be grouped into the same block.
const LEADING_THRESHOLD: f64 = 0.75;

/// Tolerance multiplier for adaptive leading when the block already
/// has multiple lines.  `mergeLeadingProbability(TextBlock, TextLine, false)`
/// compares the inter-block gap against `max_internal_leading * 1.3`.
const ADAPTIVE_LEADING_FACTOR: f64 = 1.3;

/// Tolerance for font size comparison (relative).
const FONT_SIZE_TOLERANCE: f64 = 0.15;

/// Tolerance for alignment edge comparison (in points).
const ALIGNMENT_TOLERANCE: f64 = 3.0;

/// Group TextLines into TextBlocks based on vertical proximity, font size, and horizontal overlap.
///
/// Uses a multi-active-block approach to correctly handle multi-column layouts.
/// Each active block tracks lines in a single column; new TextLines are matched
/// to the best-overlapping active block. Non-TextLine elements flush all blocks.
pub fn group_text_blocks(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    let mut result: Vec<ContentElement> = Vec::new();
    let mut active_blocks: Vec<Vec<TextLine>> = Vec::new();
    let mut page_tops: HashMap<u32, f64> = HashMap::new();

    for element in &elements {
        if let ContentElement::TextLine(line) = element {
            if let Some(page) = line.bbox.page_number {
                page_tops
                    .entry(page)
                    .and_modify(|top| *top = top.max(line.bbox.top_y))
                    .or_insert(line.bbox.top_y);
            }
        }
    }

    for element in elements {
        match element {
            ContentElement::TextLine(line) => {
                // Find the active block that best matches this line
                let page_top = line
                    .bbox
                    .page_number
                    .and_then(|page| page_tops.get(&page).copied());
                let best_idx = find_matching_block(&active_blocks, &line, page_top);
                match best_idx {
                    Some(idx) => {
                        active_blocks[idx].push(line);
                    }
                    None => {
                        // Close blocks that are too far away vertically
                        close_distant_blocks(&line, &mut active_blocks, &mut result);
                        active_blocks.push(vec![line]);
                    }
                }
            }
            other => {
                // Flush all active blocks before non-text element
                flush_all_blocks(&mut active_blocks, &mut result);
                result.push(other);
            }
        }
    }

    // Flush remaining blocks
    flush_all_blocks(&mut active_blocks, &mut result);

    result
}

/// Bullet characters recognised as list labels.
const LIST_BULLET_CHARS: &[char] = &[
    '•', '◦', '▪', '▸', '▹', '►', '▻', '●', '○', '■', '□', '◆', '◇', '→', '➤', '✓', '✔', '★', '☆',
    '➜', '➢', '⁃', '‣', '∙', '⦿', '⦾',
];

/// Check whether a TextLine's text starts with a recognizable list label pattern.
///
/// Matches the patterns used by the reference `BulletedParagraphUtils.isLabeledLine()`:
/// - Bullet characters (•, ●, ■, ▪, …) — from POSSIBLE_LABELS
/// - Dash/en-dash/em-dash followed by space ("- ", "– ", "— ")
/// - `N.` or `N)` followed by whitespace — from BULLET_REGEXES `^\d+[\.\)]\s+.*`
/// - `(N)` parenthesized numbers — from BULLET_REGEXES `^\(\d+\).*`
///
/// `[N]` bracket notation is NOT checked here (same as the reference isLabeledLine)
/// because it produces false positives with inline citations.  `[N]` lists are
/// caught by the list detector running at TextLine level before block grouping.
fn starts_with_list_label(line: &TextLine) -> bool {
    let text = line.value();
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    let first = trimmed.chars().next().unwrap();

    // Bullet characters
    if LIST_BULLET_CHARS.contains(&first) {
        return true;
    }

    // Dash/en-dash/em-dash followed by space
    if (first == '-' || first == '\u{2013}' || first == '\u{2014}')
        && trimmed.len() > 1
        && trimmed
            .chars()
            .nth(1)
            .is_some_and(|c| c == ' ' || c == '\t')
    {
        // Dash followed by primarily-numeric content is table data, not a list
        let after_space = trimmed.get(2..).unwrap_or("").trim();
        if !after_space.is_empty() && is_primarily_numeric(after_space) {
            return false;
        }
        return true;
    }

    // ── Numbered patterns matching the reference BULLET_REGEXES in isLabeledLine ──

    // Pattern: (N) — ^\(\d+\).*
    if first == '(' {
        if let Some(rest) = trimmed.strip_prefix('(') {
            if let Some(end_paren) = rest.find(')') {
                let between = &rest[..end_paren];
                if !between.is_empty() && between.chars().all(|c| c.is_ascii_digit()) {
                    return true;
                }
            }
        }
    }

    // Pattern: N. or N) followed by whitespace — ^\d+[\.\)]\s+.*
    if first.is_ascii_digit() {
        // Find the end of the digit run
        let digit_end = trimmed
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(trimmed.len());
        if digit_end > 0 && digit_end < trimmed.len() {
            let after_digits = &trimmed[digit_end..];
            // Check for `.` or `)` followed by whitespace
            if (after_digits.starts_with('.') || after_digits.starts_with(')'))
                && after_digits.len() > 1
                && after_digits.as_bytes()[1].is_ascii_whitespace()
            {
                // Avoid bold text (likely section headings, not list items)
                if line.text_chunks.first().map_or(400.0, |tc| tc.font_weight) < 600.0 {
                    return true;
                }
            }
        }
    }

    // Pattern: [N] bracket notation — matches the reference ARABIC_NUMBER_REGEXES: ^\[\d+\].*
    // Used for bibliography references.  Not in the reference isLabeledLine(), but
    // essential for preventing block grouper from merging bibliography entries.
    // Stage 11 (after reading-order sort) then detects these as list sequences.
    //
    // Only break if [N] is followed by whitespace + uppercase letter, which is
    // the signature of a bibliography entry ("[1] J. Smith…").  Inline citations
    // like "[13], the…" or "[18] proposed methods…" have lowercase/punctuation
    // after the label and are NOT block-break triggers.
    if first == '[' {
        let after_bracket = &trimmed[1..];
        if let Some(end) = after_bracket.find(']') {
            let between = &after_bracket[..end];
            if !between.is_empty() && between.chars().all(|c| c.is_ascii_digit()) {
                let after_label = &after_bracket[end + 1..];
                let after_ws = after_label.trim_start();
                if after_ws
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Check if text is primarily numeric (table data, not list content).
fn is_primarily_numeric(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let alpha_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
    let total = trimmed.chars().count();
    total > 0 && alpha_count * 100 / total < 30
}

/// Find the active block that best matches this line by horizontal overlap,
/// vertical proximity, font size, and page number.
fn find_matching_block(
    active_blocks: &[Vec<TextLine>],
    line: &TextLine,
    page_top: Option<f64>,
) -> Option<usize> {
    // If the incoming line starts with a list label pattern, force a new block.
    // This prevents the grouper from merging individual list items into a single
    // large block, which would prevent downstream list detection (Stages 9/11).
    if starts_with_list_label(line) {
        return None;
    }

    let mut best_idx = None;
    let mut best_overlap = 0.0f64;

    for (idx, block) in active_blocks.iter().enumerate() {
        let last = block.last().unwrap();

        // Must be on the same page
        if last.bbox.page_number != line.bbox.page_number {
            continue;
        }
        if let (Some(block_col), Some(line_col)) =
            (common_block_column_level(block), line.level.as_deref())
        {
            if block_col != line_col {
                continue;
            }
        }
        // Hidden text mismatch
        if last.is_hidden_text != line.is_hidden_text {
            continue;
        }
        // ── Font size check ─────────────────────────────────────────────────
        // The reference ParagraphProcessor.areTextBlocksHaveSameTextSize() accepts ANY
        // pair of text sizes between two blocks (a block stores ALL chunk sizes,
        // including already-embedded subscripts).  We approximate this as:
        //   (a) Compare against the block's MAX font size so that after a subscript
        //       line was appended, the next main-sized line still matches.
        //   (b) Also allow "subscript-sized" lines (55–90% of block max) that are
        //       geometrically positioned as math sub/superscripts.
        let block_max_font = block
            .iter()
            .map(|l| l.font_size)
            .fold(0.0f64, f64::max)
            .max(1.0);
        let font_size_ok = are_close_font_sizes(block_max_font, line.font_size);
        let is_subsup = !font_size_ok && is_subscript_or_superscript(block, line);
        if !font_size_ok && !is_subsup {
            continue;
        }
        // ── Vertical proximity ───────────────────────────────────────────────
        let reference_size = last.font_size.max(line.font_size).max(1.0);
        let v_gap = vertical_distance(last, line);
        if v_gap > reference_size * MAX_LINE_SPACING_RATIO {
            continue;
        }
        // ── Font weight ──────────────────────────────────────────────────────
        // Prevent bold/regular merging.  Math subscripts share the same weight
        // as the main text, so this naturally passes for them.
        if !are_compatible_font_weights(last, line) {
            continue;
        }
        // ── Font name / style check ──────────────────────────────────────────
        // Prevent merging lines with a different dominant font name/style.
        // In many PDFs, italic headings (e.g., "Functional Abstraction" in CMTI10)
        // differ from body text (CMR10) only by font name — not weight or size.
        // Without this check, such headings get absorbed into body paragraphs.
        // Skip for subscripts, which commonly use a different math font.
        if !is_subsup && !are_compatible_font_names(last, line) {
            continue;
        }
        if is_probable_running_header_to_body_transition(block, line, page_top) {
            continue;
        }
        // ── Short-line paragraph break (justified text) ──────────────────────
        // In justified text, the last line of a paragraph typically doesn't
        // reach the right margin.  When a block's last line is significantly
        // shorter than the block's "right margin" (the farthest right edge of
        // most lines), treat it as a paragraph-ending line and prevent further
        // merging.  This mirrors the reference Justify pass which naturally stops
        // merging at paragraph boundaries.
        //
        // Guards:
        //   - block.len() >= 3: need enough lines to establish a margin pattern
        //     (OODA-5: lowered from 4 to 3 to include shorter blocks)
        //   - near_margin >= 60%: confirms justified/flush text
        //   - gap > 1.8× font_size: short enough to catch most paragraph breaks
        //     (OODA-6: lowered from 2.0 to 1.8 for earlier detection)
        //   - not hyphenated: lines ending with '-' are word-wrap, not paragraphs
        if block.len() >= 3 && !is_subsup {
            let block_right = block
                .iter()
                .map(|l| l.bbox.right_x)
                .fold(f64::NEG_INFINITY, f64::max);
            // Count lines reaching the right margin
            let near_margin = block
                .iter()
                .filter(|l| (block_right - l.bbox.right_x) < ALIGNMENT_TOLERANCE * 2.0)
                .count();
            let last_text = last.value();
            let last_trimmed = last_text.trim_end();
            let last_ends_hyphen = last_trimmed.ends_with('-')
                || last_trimmed.ends_with('\u{00AD}')
                || last_trimmed.ends_with('\u{2010}');
            let short_gap = block_right - last.bbox.right_x;
            // If at least 60% of lines reach the right margin (justified/flush)
            // and the last line falls significantly short and doesn't end in a
            // hyphen (word-wrapping) → paragraph break.
            // Extra guard: the short last line must look like a real sentence
            // ending — either it has substantial text (>= 20 chars) or it ends
            // with sentence-terminal punctuation.  Short math symbols like "Δt"
            // or formula fragments must NOT trigger paragraph breaks.
            let last_chars = last_trimmed.chars().count();
            let ends_sentence = last_trimmed.ends_with('.')
                || last_trimmed.ends_with(')')
                || last_trimmed.ends_with('!')
                || last_trimmed.ends_with('?')
                || last_trimmed.ends_with('"')
                || last_trimmed.ends_with('\u{201D}');
            let is_real_sentence_end = last_chars >= 20 || ends_sentence;
            if near_margin * 5 >= block.len() * 3
                && short_gap > reference_size * 1.8
                && !last_ends_hyphen
                && is_real_sentence_end
                && !looks_like_lowercase_continuation(&line.value())
            {
                continue;
            }
        }
        // ── Geometric first-line indentation detection (OODA-7) ──────────────
        // In LaTeX/Word documents with \parindent > 0, new paragraphs start with
        // a first-line indent. Detect this using the block's median left_x:
        //   - Compute the median left_x of all lines in the current block.
        //   - If the incoming line's left_x is significantly higher (more indented)
        //     than this median, AND the block's last line is not hyphenated,
        //     treat the incoming line as a paragraph first line → new block.
        // This is geometrically principled: in justified text the left margin
        // is bimodal — body lines cluster at the column margin, first lines at
        // margin + parindent. The median robustly represents the body margin.
        if block.len() >= 2 && !is_subsup {
            let mut lefts: Vec<f64> = block.iter().map(|l| l.bbox.left_x).collect();
            lefts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let block_median_left = lefts[lefts.len() / 2];
            let indent_threshold = reference_size * 0.8;
            if line.bbox.left_x > block_median_left + indent_threshold {
                // The incoming line is more indented than the block's body margin.
                // Only treat as paragraph break if block ends non-hyphenated.
                let last_text2 = last.value();
                let trimmed2 = last_text2.trim_end();
                let ends_hyphen2 = trimmed2.ends_with('-')
                    || trimmed2.ends_with('\u{00AD}')
                    || trimmed2.ends_with('\u{2010}');
                if !ends_hyphen2 {
                    continue; // Force new block: incoming line starts new paragraph
                }
            }
        }
        // ── Leading probability ──────────────────────────────────────────────
        // The reference implementation uses two completely different strategies depending on block size:
        //
        // (A) Both blocks are 1-line: use DATA_LOADER_DEFAULT_FONT_LEADING_INTERVAL
        //     = {0.7, 2.2} with getUniformProbability.
        //
        // (B) Block has >= 2 lines: use ADAPTIVE leading based on the block's
        //     internal max baseline spacing.  The new line's gap must be within
        //     max_internal_leading * 1.3 (reference: mergeLeadingProbability(TextBlock,
        //     TextLine, areShouldBeCloseLines=false) uses factor 1.3).
        //
        // Inline math subscripts sit only 2–5 pt below the main baseline.
        // Skip the leading check for confirmed sub/superscripts.
        if !is_subsup {
            if block.len() >= 2 {
                // Adaptive leading: use the block's internal max baseline gap
                let max_internal = max_internal_leading(block);
                if max_internal > 0.0 {
                    let inter_gap = (block.last().unwrap().base_line - line.base_line).abs();
                    if inter_gap > max_internal * ADAPTIVE_LEADING_FACTOR {
                        continue;
                    }
                } else if merge_leading_probability(last, line) < LEADING_THRESHOLD {
                    continue;
                }
            } else if merge_leading_probability(last, line) < LEADING_THRESHOLD {
                continue;
            }
        }
        // Horizontal overlap (column guard).
        // For subscripts, `is_subscript_or_superscript` already verified that
        // the line either overlaps or is adjacent to the block's full x-span.
        // Using `last` alone is too strict for subscripts: the subscript may
        // start just to the right of the parent character (0 pt overlap with
        // `last` but valid adjacency).  So for is_subsup we compute the score
        // against the block's full x-span; for normal lines we use last.
        let overlap = if is_subsup {
            // Recompute block span for the overlap score.
            let bs_left = block
                .iter()
                .map(|l| l.bbox.left_x)
                .fold(f64::INFINITY, f64::min);
            let bs_right = block
                .iter()
                .map(|l| l.bbox.right_x)
                .fold(f64::NEG_INFINITY, f64::max);
            let o = line.bbox.right_x.min(bs_right) - line.bbox.left_x.max(bs_left);
            if o > 0.0 {
                let line_width = (line.bbox.right_x - line.bbox.left_x).max(1.0);
                (o / line_width).min(1.0)
            } else {
                // Adjacent subscript (is_subscript_or_superscript verified adjacency).
                // Assign a small positive score so this block can still win.
                0.3
            }
        } else {
            // Use the ratio-to-min-width metric with a 0.2 threshold so that:
            //   - Lines in the same column (same left/right ≈ same width) merge ✓
            //   - Lines in opposite columns (no intersection at all) don't merge ✓
            //   - Tiny characters (diacritics, superscripts w < 6×font_size) merge ✓
            horizontal_overlap_ratio(last, line)
        };
        // Subscript blocks checked above already guarantee overlap > 0.
        // Normal blocks need > 0.2 to guard against cross-column merging.
        if is_subsup && overlap <= 0.0 {
            continue;
        }
        if !is_subsup && overlap <= 0.2 {
            continue;
        }
        // Pick block with highest overlap
        if overlap > best_overlap {
            best_overlap = overlap;
            best_idx = Some(idx);
        }
    }

    best_idx
}

fn is_probable_running_header_to_body_transition(
    block: &[TextLine],
    line: &TextLine,
    page_top: Option<f64>,
) -> bool {
    if block.len() != 1 {
        return false;
    }
    let Some(page_top) = page_top else {
        return false;
    };
    let header = &block[0];
    if header.bbox.top_y < page_top - header.font_size.max(1.0) * 3.0 {
        return false;
    }

    let header_text = header.value();
    let body_text = line.value();
    let header_chars = header_text.chars().filter(|ch| !ch.is_whitespace()).count();
    let body_chars = body_text.chars().filter(|ch| !ch.is_whitespace()).count();
    if header_chars == 0 || body_chars < 16 || header_chars > 24 {
        return false;
    }
    if !header_text.chars().any(|ch| ch.is_ascii_digit()) {
        return false;
    }

    let header_density = text_density(header);
    let body_density = text_density(line);
    let left_aligned =
        (header.bbox.left_x - line.bbox.left_x).abs() <= header.font_size.max(1.0) * 1.5;
    let body_is_substantially_wider =
        line.bbox.width() >= header.bbox.width() * 1.15 || body_chars >= header_chars * 2;

    left_aligned && body_is_substantially_wider && header_density * 2.0 < body_density
}

fn text_density(line: &TextLine) -> f64 {
    let chars = line
        .value()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .count() as f64;
    chars / line.bbox.width().max(1.0)
}

fn looks_like_lowercase_continuation(text: &str) -> bool {
    let trimmed = text.trim_start();
    for ch in trimmed.chars() {
        if ch.is_alphabetic() {
            return ch.is_lowercase();
        }
        if !matches!(ch, '"' | '\'' | '(' | '[') {
            break;
        }
    }
    false
}

/// Close active blocks that are too far away from the current line vertically
/// or on a different page.
fn close_distant_blocks(
    current: &TextLine,
    active_blocks: &mut Vec<Vec<TextLine>>,
    result: &mut Vec<ContentElement>,
) {
    let mut i = 0;
    while i < active_blocks.len() {
        let last = active_blocks[i].last().unwrap();
        let v_gap = vertical_distance(last, current);
        let ref_size = last.font_size.max(1.0);
        if v_gap > ref_size * MAX_LINE_SPACING_RATIO * 3.0
            || last.bbox.page_number != current.bbox.page_number
        {
            let block = active_blocks.remove(i);
            result.push(ContentElement::TextBlock(build_text_block(block)));
        } else {
            i += 1;
        }
    }
}

/// Flush all active blocks into the result.
fn flush_all_blocks(active_blocks: &mut Vec<Vec<TextLine>>, result: &mut Vec<ContentElement>) {
    for block in active_blocks.drain(..) {
        result.push(ContentElement::TextBlock(build_text_block(block)));
    }
}

/// Compute the maximum absolute baseline gap between consecutive TextLines
/// in a block.  This is the reference approach in `mergeLeadingProbability(TextBlock,
/// TextLine, boolean)`: iterate all consecutive line pairs and take the max.
fn max_internal_leading(block: &[TextLine]) -> f64 {
    if block.len() < 2 {
        return 0.0;
    }
    let mut max_gap = 0.0f64;
    for i in 0..block.len() - 1 {
        let gap = (block[i].base_line - block[i + 1].base_line).abs();
        if gap > max_gap {
            max_gap = gap;
        }
    }
    max_gap
}

/// Compute leading probability between two consecutive TextLines.
///
/// Ports the reference `ChunksMergeUtils.mergeLeadingProbability(TextLine, TextLine)`:
///   - DEFAULT_FONT_LEADING_INTERVAL = [0.7, 1.51], step = 1.0
///   - normalizedGap = (baseLine_a − baseLine_b) / fontSize_b
///   - Returns getUniformProbability([0.7, 1.51], normalizedGap, 1.0)
///
/// Returns 1.0 for normal paragraph spacing (gap ≈ 1× font_size),
/// returns < 0.75 when the gap exceeds ~1.76× font_size (section break),
/// returns 0.0 when gap > 2.51× font_size.
///
/// Special case: when normalizedGap ≤ 0, `b` is at the same level as or above
/// `a` in PDF y-up coordinates.  This happens for inline superscripts / footnote
/// markers that are on a visually higher position than the host line.  We allow
/// these merges (return 1.0) because the horizontal-overlap filter is the correct
/// guard there — the leading check is only meaningful for downward-consecutive lines.
fn merge_leading_probability(a: &TextLine, b: &TextLine) -> f64 {
    let font_size = b.font_size.max(a.font_size).max(1.0);
    // base_line = bbox.bottom_y (set by text_line_grouper), approximates PDF baseline.
    let normalized_gap = (a.base_line - b.base_line) / font_size;
    // Non-positive gap: b is at same level or above a (inline superscript / same line).
    // Don't apply the "next line" penalty; horizontal overlap is the right guard.
    if normalized_gap <= 0.0 {
        return 1.0;
    }
    get_uniform_probability(LEADING_LO, LEADING_HI, normalized_gap, LEADING_STEP)
}

/// Port of the reference `ChunksMergeUtils.getUniformProbability(double[], double, double)`.
///
/// Returns 1.0 within (lo, hi), 0.0 outside [lo − step, hi + step],
/// linear interpolation in the buffer zones.
fn get_uniform_probability(lo: f64, hi: f64, value: f64, step: f64) -> f64 {
    const EPS: f64 = 1e-7;
    // Within core interval → probability 1.0
    if value + EPS > lo && value < hi + EPS {
        return 1.0;
    }
    // Outside interval + full step → probability 0.0
    if value <= lo - step - EPS || value >= hi + step + EPS {
        return 0.0;
    }
    // Linear interpolation in buffer zone [lo-step, lo] or [hi, hi+step]
    let d = if value < lo + EPS {
        lo - value
    } else {
        value - hi
    };
    (step - d) / step
}

/// Compute horizontal overlap ratio between two TextLines using the Jaccard
/// index (intersection / union).
///
/// This prevents a full-width line from unconditionally absorbing a narrower
/// same-left-margin column line. On two-column figure pages, wide caption or
/// spillover lines can overlap a whole left-column line while still spanning
/// into the right column; ratio-to-min-width incorrectly scores that as 1.0.
/// Jaccard keeps true same-column lines near 1.0 while reducing wide-vs-narrow
/// containment cases below the merge threshold.
fn horizontal_overlap_ratio(a: &TextLine, b: &TextLine) -> f64 {
    let overlap = a.bbox.right_x.min(b.bbox.right_x) - a.bbox.left_x.max(b.bbox.left_x);
    if overlap <= 0.0 {
        return 0.0;
    }
    let a_width = a.bbox.right_x - a.bbox.left_x;
    let b_width = b.bbox.right_x - b.bbox.left_x;
    let union_width = a_width + b_width - overlap;
    if union_width <= 0.0 {
        return 0.0;
    }
    (overlap / union_width).min(1.0)
}

/// Compute vertical distance between two lines.
/// Uses the bottom of the first line and top of the second line.
fn vertical_distance(a: &TextLine, b: &TextLine) -> f64 {
    // In PDF coordinates, y increases upward.
    // a.bbox.bottom_y is the bottom of line a, b.bbox.top_y is the top of line b.
    // After top-to-bottom sorting, a is above b, so a.bottom_y > b.top_y.
    (a.bbox.bottom_y - b.bbox.top_y).abs()
}

/// Check whether `line` could be a subscript or superscript of `block`.
///
/// This mirrors how the reference TextLineProcessor embeds subscripts into their parent
/// TextLine via ChunksMergeUtils.countOneLineProbability() before any block
/// grouping happens.  In Rust we cannot replicate that chunk-level detection
/// (chunks are sorted by y-bucket before sequential grouping, so subscript
/// chunks land in a different group from their parent formula text).  Instead
/// we recover the relationship at the block level.
///
/// Conditions (ported from the reference implementation constants):
///   SUBSCRIPT_FONTSIZE_THRESHOLD = 0.1  → font must differ > 10% (ratio < 0.9)
///   SUBSCRIPT_BASELINE_THRESHOLD = 0.08 → vertical offset exists
///
/// Additional guards to avoid false positives (e.g. heading → body-text):
///   - Font ratio [0.52, 0.90): catches sub-subscripts at ~55% while rejecting
///     heading→body at 0.50 (12 pt body / 24 pt heading).
///   - Char count ≤ 12: subscripts are short labels; body text starting under a
///     heading would be longer.
///   - v_gap ≤ 1.7 × max_block_font: inline subscripts sit ~0–5 pt off-baseline
///     but sigma-type limits (∑_{i=1}) can be up to ~1.4 × font_size away.
///   - Horizontal: either overlaps block's x-span OR is adjacent (inline α_{t,k}).
///   - Width guard: subscripts are narrow relative to the block width.
fn is_subscript_or_superscript(block: &[TextLine], line: &TextLine) -> bool {
    let max_block_font = block
        .iter()
        .map(|l| l.font_size)
        .fold(0.0f64, f64::max)
        .max(1.0);

    // Sub/superscripts are typically 52–90% of main text font.
    // Lower bound 0.52 catches sub-subscripts (e.g., 5.98 pt / 10.91 pt ≈ 0.548)
    // while excluding heading→body transitions (e.g., 12 pt / 24 pt = 0.50 < 0.52).
    let ratio = line.font_size / max_block_font;
    if !(0.52..0.90).contains(&ratio) {
        return false;
    }

    // Char-count guard: subscripts are short labels like "i", "t,k", "i=1 t=1".
    // Body text starting below a slightly-smaller heading would have more chars.
    if line.value().chars().count() > 12 {
        return false;
    }

    // Same page.
    let last = block.last().unwrap();
    if last.bbox.page_number != line.bbox.page_number {
        return false;
    }

    // Vertically close: inline subscripts sit only 0–5 pt off the main baseline,
    // but sigma-type limits (∑_{i=1}^{N}) can be up to ~1.4 × font_size below.
    // Allow up to 1.7 × max_block_font so that all formula-subscript patterns fit.
    let v_gap = vertical_distance(last, line);
    if v_gap > max_block_font * 1.7 {
        return false;
    }

    // Horizontal span checks:
    let block_left = block
        .iter()
        .map(|l| l.bbox.left_x)
        .fold(f64::INFINITY, f64::min);
    let block_right = block
        .iter()
        .map(|l| l.bbox.right_x)
        .fold(f64::NEG_INFINITY, f64::max);

    // (a) The line overlaps the block's x-span (sigma-type limits).
    let horiz_overlap = line.bbox.right_x.min(block_right) - line.bbox.left_x.max(block_left);

    // (b) The line is adjacent to the block's right edge (inline subscript
    //     like α_{t,k}: subscript starts right after α with ≤ 0 pt overlap).
    //     Accept if the line's left edge is within 2 pt of block_right to the
    //     left, and within 1.5 × font_size to the right.
    let is_adjacent = horiz_overlap <= 0.0
        && line.bbox.left_x >= block_right - 2.0
        && line.bbox.left_x <= block_right + line.font_size * 1.5;

    if horiz_overlap <= 0.0 && !is_adjacent {
        return false;
    }

    // (c) Width guard: subscripts are narrow and fit inside the block's span,
    //     or they are adjacent (in which case width is irrelevant — they extend
    //     slightly beyond block_right by design).
    //     Body text below a smaller heading would be as wide as the column.
    let line_width = (line.bbox.right_x - line.bbox.left_x).max(1.0);
    let block_width = (block_right - block_left).max(1.0);
    if !is_adjacent && line_width > block_width * 1.1 {
        return false;
    }

    true
}

/// Check if two font sizes are similar within tolerance.
fn are_close_font_sizes(a: f64, b: f64) -> bool {
    let max = a.max(b);
    if max < 0.001 {
        return true;
    }
    (a - b).abs() / max <= FONT_SIZE_TOLERANCE
}

/// Font weight threshold for bold detection.
const BOLD_WEIGHT_THRESHOLD: f64 = 550.0;

/// Check if two TextLines have compatible font weights.
/// Prevents merging bold heading lines with regular body lines.
fn are_compatible_font_weights(a: &TextLine, b: &TextLine) -> bool {
    let w_a = dominant_line_font_weight(a);
    let w_b = dominant_line_font_weight(b);
    // If one is bold and the other is regular, don't merge
    let a_bold = w_a >= BOLD_WEIGHT_THRESHOLD;
    let b_bold = w_b >= BOLD_WEIGHT_THRESHOLD;
    a_bold == b_bold
}

/// Check if two TextLines have compatible dominant font names.
///
/// Returns false when the main font family of the two lines differs, which
/// indicates a style change (e.g., italic heading vs. roman body text).
/// Uses the font name with the most character coverage in each line.
///
/// Families are compared after stripping common suffixes like "-Bold",
/// "-Italic", "-Regular", etc., so "Helvetica-Bold" and "Helvetica" are
/// treated as the same family.  Simple italic variants ("CMTI10" vs "CMR10")
/// are naturally different after stripping, which is the desired behaviour.
///
/// Single-chunk lines with ≤ 2 characters (subscripts, superscripts, symbols)
/// are always compatible to avoid fragmenting inline mathematical notation.
fn are_compatible_font_names(a: &TextLine, b: &TextLine) -> bool {
    let name_a = dominant_line_font_name(a);
    let name_b = dominant_line_font_name(b);
    match (name_a, name_b) {
        (Some(na), Some(nb)) => {
            // Treat ≤ 2-char lines as compatible (symbols, subscripts)
            let a_chars: usize = a.text_chunks.iter().map(|c| c.value.chars().count()).sum();
            let b_chars: usize = b.text_chunks.iter().map(|c| c.value.chars().count()).sum();
            if a_chars <= 2 || b_chars <= 2 {
                return true;
            }
            normalize_font_family(&na) == normalize_font_family(&nb)
        }
        _ => true, // If either line has no font name, allow merge
    }
}

/// Get the dominant font name of a TextLine (most characters).
fn dominant_line_font_name(line: &TextLine) -> Option<String> {
    use std::collections::HashMap;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for chunk in &line.text_chunks {
        *counts.entry(&chunk.font_name).or_insert(0) += chunk.value.len().max(1);
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(name, _)| name.to_string())
}

/// Normalize a font name to its base family by stripping common style suffixes.
fn normalize_font_family(name: &str) -> String {
    let lower = name.to_lowercase();
    // Strip common suffixes
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
/// Compute dominant font weight of a TextLine (weighted by character count).
fn dominant_line_font_weight(line: &TextLine) -> f64 {
    let total: usize = line.text_chunks.iter().map(|c| c.value.len().max(1)).sum();
    if total == 0 {
        return 400.0;
    }
    let weighted: f64 = line
        .text_chunks
        .iter()
        .map(|c| c.font_weight * c.value.len().max(1) as f64)
        .sum();
    weighted / total as f64
}

/// Build a TextBlock from a group of TextLines.
fn build_text_block(lines: Vec<TextLine>) -> TextBlock {
    assert!(!lines.is_empty());

    let bbox = lines
        .iter()
        .map(|l| l.bbox.clone())
        .reduce(|a, b| a.union(&b))
        .unwrap();

    let font_size = lines.iter().map(|l| l.font_size).sum::<f64>() / lines.len() as f64;

    let base_line = lines.last().map(|l| l.base_line).unwrap_or(0.0);
    let is_hidden = lines.iter().all(|l| l.is_hidden_text);
    let alignment = detect_alignment(&lines, &bbox);
    let level = common_block_column_level(&lines).map(str::to_string);

    TextBlock {
        bbox,
        index: None,
        level,
        font_size,
        base_line,
        slant_degree: 0.0,
        is_hidden_text: is_hidden,
        text_lines: lines,
        has_start_line: false,
        has_end_line: false,
        text_alignment: Some(alignment),
    }
}

fn common_block_column_level(lines: &[TextLine]) -> Option<&str> {
    let mut iter = lines.iter().filter_map(|line| line.level.as_deref());
    let first = iter.next()?;
    if iter.all(|level| level == first) {
        Some(first)
    } else {
        None
    }
}

/// Detect the text alignment of a group of lines within a bounding box.
fn detect_alignment(lines: &[TextLine], block_bbox: &BoundingBox) -> TextAlignment {
    if lines.len() < 2 {
        return TextAlignment::Left;
    }

    let block_left = block_bbox.left_x;
    let block_right = block_bbox.right_x;
    let block_center = block_bbox.center_x();

    let mut left_aligned = 0usize;
    let mut right_aligned = 0usize;
    let mut center_aligned = 0usize;
    let mut justify_count = 0usize;

    for line in lines {
        let line_left = line.bbox.left_x;
        let line_right = line.bbox.right_x;
        let line_center = line.bbox.center_x();

        if (line_left - block_left).abs() < ALIGNMENT_TOLERANCE {
            left_aligned += 1;
        }
        if (line_right - block_right).abs() < ALIGNMENT_TOLERANCE {
            right_aligned += 1;
        }
        if (line_center - block_center).abs() < ALIGNMENT_TOLERANCE {
            center_aligned += 1;
        }
        // Justify = both edges aligned
        if (line_left - block_left).abs() < ALIGNMENT_TOLERANCE
            && (line_right - block_right).abs() < ALIGNMENT_TOLERANCE
        {
            justify_count += 1;
        }
    }

    let total = lines.len();
    let threshold = total * 3 / 4; // 75%

    if justify_count >= threshold {
        TextAlignment::Justify
    } else if center_aligned >= threshold {
        TextAlignment::Center
    } else if right_aligned >= threshold && left_aligned < threshold {
        TextAlignment::Right
    } else {
        // Default to left (most common)
        TextAlignment::Left
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_line(text: &str, left_x: f64, top_y: f64, width: f64, font_size: f64) -> TextLine {
        let right_x = left_x + width;
        let bottom_y = top_y - font_size;
        TextLine {
            bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + font_size * 0.2,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![TextChunk {
                value: text.to_string(),
                bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                font_name: "Helvetica".to_string(),
                font_size,
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
            }],
            is_line_start: false,
            is_line_end: false,
            is_list_line: false,
            connected_line_art_label: None,
        }
    }

    #[test]
    fn test_empty_input() {
        let result = group_text_blocks(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_line_becomes_block() {
        let line = make_line("Hello", 72.0, 700.0, 200.0, 12.0);
        let input = vec![ContentElement::TextLine(line)];
        let result = group_text_blocks(input);

        assert_eq!(result.len(), 1);
        if let ContentElement::TextBlock(ref block) = result[0] {
            assert_eq!(block.text_lines.len(), 1);
            assert_eq!(block.value(), "Hello");
        } else {
            panic!("Expected TextBlock");
        }
    }

    #[test]
    fn test_close_lines_grouped_into_one_block() {
        // Two lines with typical single-spacing (14pt leading for 12pt font)
        // In PDF coords: line1 is higher on page (top_y=700), line2 below (top_y=686)
        let line1 = make_line("First line", 72.0, 700.0, 200.0, 12.0);
        let line2 = make_line("Second line", 72.0, 686.0, 200.0, 12.0);

        let input = vec![
            ContentElement::TextLine(line1),
            ContentElement::TextLine(line2),
        ];
        let result = group_text_blocks(input);

        assert_eq!(result.len(), 1);
        if let ContentElement::TextBlock(ref block) = result[0] {
            assert_eq!(block.text_lines.len(), 2);
        } else {
            panic!("Expected TextBlock");
        }
    }

    #[test]
    fn test_large_gap_creates_separate_blocks() {
        // Two lines with a large gap (~50pt for 12pt font = 4x ratio)
        // line1 top at 700, bottom at 688; line2 top at 640, bottom at 628
        // gap = 688 - 640 = 48pt → 48/12 = 4x → exceeds MAX_LINE_SPACING_RATIO
        let line1 = make_line("Para one", 72.0, 700.0, 200.0, 12.0);
        let line2 = make_line("Para two", 72.0, 640.0, 200.0, 12.0);

        let input = vec![
            ContentElement::TextLine(line1),
            ContentElement::TextLine(line2),
        ];
        let result = group_text_blocks(input);

        assert_eq!(result.len(), 2);
        for elem in &result {
            assert!(matches!(elem, ContentElement::TextBlock(_)));
        }
    }

    #[test]
    fn test_different_font_sizes_separate_blocks() {
        let line1 = make_line("Title", 72.0, 700.0, 200.0, 24.0); // Large heading
        let line2 = make_line("Body text", 72.0, 670.0, 200.0, 12.0); // Normal body below

        let input = vec![
            ContentElement::TextLine(line1),
            ContentElement::TextLine(line2),
        ];
        let result = group_text_blocks(input);

        // ratio = 12/24 = 0.50 < 0.52 lower bound → not a subscript → separate blocks
        assert_eq!(result.len(), 2);
    }

    /// Inline subscript: "and α" at 10.91 pt followed immediately to the right
    /// by a subscript "t,k" at 7.97 pt (ratio ≈ 0.73, adjacent x-range).
    #[test]
    fn test_inline_subscript_merges_into_parent_block() {
        // Simulate "and α" text (10.91 pt, x=[72, 99.7], y bottom=423.7 top=434.6)
        // and its subscript "t,k" (7.97 pt, x=[98, 107.8], y bottom=422.5 top=430.5)
        // Both on same page.  The subscript's left_x (98) overlaps block_right (99.7)
        // by only 1.7 pt — would fail the old 0.2 ratio check, but passes the new
        // block-span overlap check (1.7/9.8 ≈ 0.17 < 0.2 old threshold).
        let main_line = {
            let left_x = 72.0;
            let right_x = 99.7;
            let bottom_y = 423.7;
            let top_y = 434.6;
            let font_size = 10.91;
            TextLine {
                bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                index: None,
                level: None,
                font_size,
                base_line: bottom_y,
                slant_degree: 0.0,
                is_hidden_text: false,
                text_chunks: vec![TextChunk {
                    value: "and α".to_string(),
                    bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                    font_name: "Helvetica".to_string(),
                    font_size,
                    font_weight: 400.0,
                    italic_angle: 0.0,
                    font_color: "#000000".to_string(),
                    contrast_ratio: 21.0,
                    symbol_ends: vec![],
                    text_format: crate::models::enums::TextFormat::Normal,
                    text_type: crate::models::enums::TextType::Regular,
                    pdf_layer: crate::models::enums::PdfLayer::Main,
                    ocg_visible: true,
                    index: None,
                    page_number: Some(1),
                    level: None,
                    mcid: None,
                }],
                is_line_start: false,
                is_line_end: false,
                is_list_line: false,
                connected_line_art_label: None,
            }
        };
        let subscript_line = {
            let left_x = 98.0;
            let right_x = 107.8;
            let bottom_y = 422.5;
            let top_y = 430.5;
            let font_size = 7.97;
            TextLine {
                bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                index: None,
                level: None,
                font_size,
                base_line: bottom_y,
                slant_degree: 0.0,
                is_hidden_text: false,
                text_chunks: vec![TextChunk {
                    value: "t,k".to_string(),
                    bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                    font_name: "Helvetica".to_string(),
                    font_size,
                    font_weight: 400.0,
                    italic_angle: 0.0,
                    font_color: "#000000".to_string(),
                    contrast_ratio: 21.0,
                    symbol_ends: vec![],
                    text_format: crate::models::enums::TextFormat::Normal,
                    text_type: crate::models::enums::TextType::Regular,
                    pdf_layer: crate::models::enums::PdfLayer::Main,
                    ocg_visible: true,
                    index: None,
                    page_number: Some(1),
                    level: None,
                    mcid: None,
                }],
                is_line_start: false,
                is_line_end: false,
                is_list_line: false,
                connected_line_art_label: None,
            }
        };
        let input = vec![
            ContentElement::TextLine(main_line),
            ContentElement::TextLine(subscript_line),
        ];
        let result = group_text_blocks(input);
        // Subscript should merge into parent's block → 1 block with 2 lines
        assert_eq!(
            result.len(),
            1,
            "inline subscript must merge into parent block"
        );
        if let ContentElement::TextBlock(ref block) = result[0] {
            assert_eq!(
                block.text_lines.len(),
                2,
                "should contain both main and subscript lines"
            );
        } else {
            panic!("Expected TextBlock");
        }
    }

    /// Sigma lower limit: "XL" at 10.91 pt (top limit above sigma), followed by
    /// "i=1" at 7.97 pt (lower limit, 14.9 pt below XL — beyond old 1× limit).
    #[test]
    fn test_sigma_limit_subscript_merges_into_upper_limit_block() {
        let upper_limit = {
            let font_size = 10.91;
            let left_x = 100.0;
            let right_x = 115.8;
            let bottom_y = 401.9;
            let top_y = 413.7;
            TextLine {
                bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                index: None,
                level: None,
                font_size,
                base_line: bottom_y,
                slant_degree: 0.0,
                is_hidden_text: false,
                text_chunks: vec![TextChunk {
                    value: "XL".to_string(),
                    bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                    font_name: "Helvetica".to_string(),
                    font_size,
                    font_weight: 400.0,
                    italic_angle: 0.0,
                    font_color: "#000000".to_string(),
                    contrast_ratio: 21.0,
                    symbol_ends: vec![],
                    text_format: crate::models::enums::TextFormat::Normal,
                    text_type: crate::models::enums::TextType::Regular,
                    pdf_layer: crate::models::enums::PdfLayer::Main,
                    ocg_visible: true,
                    index: None,
                    page_number: Some(1),
                    level: None,
                    mcid: None,
                }],
                is_line_start: false,
                is_line_end: false,
                is_list_line: false,
                connected_line_art_label: None,
            }
        };
        let lower_limit = {
            let font_size = 7.97;
            let left_x = 100.0;
            let right_x = 113.7;
            let bottom_y = 379.0;
            let top_y = 387.0;
            TextLine {
                bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                index: None,
                level: None,
                font_size,
                base_line: bottom_y,
                slant_degree: 0.0,
                is_hidden_text: false,
                text_chunks: vec![TextChunk {
                    value: "i=1".to_string(),
                    bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                    font_name: "Helvetica".to_string(),
                    font_size,
                    font_weight: 400.0,
                    italic_angle: 0.0,
                    font_color: "#000000".to_string(),
                    contrast_ratio: 21.0,
                    symbol_ends: vec![],
                    text_format: crate::models::enums::TextFormat::Normal,
                    text_type: crate::models::enums::TextType::Regular,
                    pdf_layer: crate::models::enums::PdfLayer::Main,
                    ocg_visible: true,
                    index: None,
                    page_number: Some(1),
                    level: None,
                    mcid: None,
                }],
                is_line_start: false,
                is_line_end: false,
                is_list_line: false,
                connected_line_art_label: None,
            }
        };
        // v_gap = |401.9 - 387.0| = 14.9 pt, max_block_font = 10.91 pt
        // 14.9 / 10.91 ≈ 1.37 < 1.7 limit → should merge
        let input = vec![
            ContentElement::TextLine(upper_limit),
            ContentElement::TextLine(lower_limit),
        ];
        let result = group_text_blocks(input);
        assert_eq!(
            result.len(),
            1,
            "sigma lower limit must merge into upper-limit block"
        );
        if let ContentElement::TextBlock(ref block) = result[0] {
            assert_eq!(block.text_lines.len(), 2);
        } else {
            panic!("Expected TextBlock");
        }
    }

    #[test]
    fn test_non_text_elements_preserved() {
        use crate::models::chunks::ImageChunk;

        let line1 = make_line("Before image", 72.0, 700.0, 200.0, 12.0);
        let img = ImageChunk {
            bbox: BoundingBox::new(Some(1), 72.0, 500.0, 372.0, 680.0),
            index: Some(1),
            level: None,
        };
        let line2 = make_line("After image", 72.0, 480.0, 200.0, 12.0);

        let input = vec![
            ContentElement::TextLine(line1),
            ContentElement::Image(img),
            ContentElement::TextLine(line2),
        ];
        let result = group_text_blocks(input);

        // Should be: TextBlock, Image, TextBlock
        assert_eq!(result.len(), 3);
        assert!(matches!(result[0], ContentElement::TextBlock(_)));
        assert!(matches!(result[1], ContentElement::Image(_)));
        assert!(matches!(result[2], ContentElement::TextBlock(_)));
    }

    #[test]
    fn test_alignment_detection_left() {
        // All lines start at same X, different widths → left aligned
        let lines = vec![
            make_line("Line one is here", 72.0, 700.0, 200.0, 12.0),
            make_line("Line two shorter", 72.0, 686.0, 150.0, 12.0),
            make_line("Line three", 72.0, 672.0, 120.0, 12.0),
        ];
        let bbox = lines
            .iter()
            .map(|l| l.bbox.clone())
            .reduce(|a, b| a.union(&b))
            .unwrap();
        assert_eq!(detect_alignment(&lines, &bbox), TextAlignment::Left);
    }

    #[test]
    fn test_alignment_detection_center() {
        // Lines centered around same midpoint (center_x = 200)
        let lines = vec![
            make_line("Line one", 100.0, 700.0, 200.0, 12.0), // center=200
            make_line("Short", 125.0, 686.0, 150.0, 12.0),    // center=200
            make_line("Tiny", 140.0, 672.0, 120.0, 12.0),     // center=200
        ];
        let bbox = lines
            .iter()
            .map(|l| l.bbox.clone())
            .reduce(|a, b| a.union(&b))
            .unwrap();
        assert_eq!(detect_alignment(&lines, &bbox), TextAlignment::Center);
    }

    #[test]
    fn test_two_column_lines_create_separate_blocks() {
        // Simulate two-column layout: left column [72-297], right column [315-540]
        // Lines arrive interleaved (Y-desc, X-asc order from text_line_grouper)
        let l1 = make_line("Left para line 1", 72.0, 700.0, 225.0, 12.0);
        let r1 = make_line("Right para line 1", 315.0, 700.0, 225.0, 12.0);
        let l2 = make_line("Left para line 2", 72.0, 686.0, 225.0, 12.0);
        let r2 = make_line("Right para line 2", 315.0, 686.0, 225.0, 12.0);
        let l3 = make_line("Left para line 3", 72.0, 672.0, 225.0, 12.0);
        let r3 = make_line("Right para line 3", 315.0, 672.0, 225.0, 12.0);

        let input = vec![
            ContentElement::TextLine(l1),
            ContentElement::TextLine(r1),
            ContentElement::TextLine(l2),
            ContentElement::TextLine(r2),
            ContentElement::TextLine(l3),
            ContentElement::TextLine(r3),
        ];
        let result = group_text_blocks(input);

        // Should produce 2 blocks: one for left column, one for right column
        assert_eq!(
            result.len(),
            2,
            "Expected 2 blocks for two-column layout, got {}",
            result.len()
        );

        let blocks: Vec<&TextBlock> = result
            .iter()
            .filter_map(|e| {
                if let ContentElement::TextBlock(ref b) = e {
                    Some(b)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(blocks.len(), 2);
        // Each block should have 3 lines
        assert_eq!(
            blocks[0].text_lines.len(),
            3,
            "Left block should have 3 lines"
        );
        assert_eq!(
            blocks[1].text_lines.len(),
            3,
            "Right block should have 3 lines"
        );

        // Verify column separation: blocks should not overlap horizontally
        let block0_right = blocks[0].bbox.right_x;
        let block1_left = blocks[1].bbox.left_x;
        assert!(
            block0_right < block1_left || blocks[1].bbox.right_x < blocks[0].bbox.left_x,
            "Blocks should not overlap horizontally: block0 right={} block1 left={}",
            block0_right,
            block1_left
        );
    }

    #[test]
    fn test_full_width_line_does_not_absorb_column_block() {
        let mut wide = make_line("Wide spillover line", 72.0, 700.0, 448.0, 12.0);
        let mut left1 = make_line("Left column line 1", 72.0, 686.0, 208.0, 12.0);
        let mut left2 = make_line("Left column line 2", 72.0, 672.0, 208.0, 12.0);

        wide.level = Some("col:1".to_string());
        left1.level = Some("col:0".to_string());
        left2.level = Some("col:0".to_string());

        let input = vec![
            ContentElement::TextLine(wide),
            ContentElement::TextLine(left1),
            ContentElement::TextLine(left2),
        ];
        let result = group_text_blocks(input);

        assert_eq!(
            result.len(),
            2,
            "wide and column lines must form separate blocks"
        );

        let blocks: Vec<&TextBlock> = result
            .iter()
            .filter_map(|e| match e {
                ContentElement::TextBlock(b) => Some(b),
                _ => None,
            })
            .collect();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].text_lines.len(), 1);
        assert_eq!(blocks[1].text_lines.len(), 2);
        assert!(blocks[0].bbox.width() > blocks[1].bbox.width());
    }

    #[test]
    fn test_top_margin_running_header_stays_separate_from_body_block() {
        let header = make_line("3 4 Yarrow", 72.0, 800.0, 320.0, 11.0);
        let body = make_line(
            "1999 such iterations to form parameter distributions and continue the first body paragraph",
            72.0,
            786.0,
            326.0,
            11.0,
        );

        let input = vec![
            ContentElement::TextLine(header),
            ContentElement::TextLine(body),
        ];
        let result = group_text_blocks(input);

        assert_eq!(result.len(), 2);
        match &result[0] {
            ContentElement::TextBlock(block) => assert_eq!(block.value(), "3 4 Yarrow"),
            other => panic!("Expected TextBlock, got {other:?}"),
        }
    }

    #[test]
    fn test_short_last_line_with_lowercase_follow_on_stays_in_same_block() {
        let input = vec![
            ContentElement::TextLine(make_line(
                "What you are forming is a null distribution of the expected difference between",
                72.0,
                700.0,
                326.0,
                11.0,
            )),
            ContentElement::TextLine(make_line(
                "model parameters that would occur just by chance. You can then compare the",
                72.0,
                686.0,
                326.0,
                11.0,
            )),
            ContentElement::TextLine(make_line(
                "difference you actually obtained against this null distribution to generate",
                72.0,
                672.0,
                326.0,
                11.0,
            )),
            ContentElement::TextLine(make_line(
                "a p value for your difference",
                72.0,
                658.0,
                180.0,
                11.0,
            )),
            ContentElement::TextLine(make_line("of interest.", 72.0, 644.0, 60.0, 11.0)),
        ];

        let result = group_text_blocks(input);
        let blocks: Vec<_> = result
            .iter()
            .filter_map(|e| match e {
                ContentElement::TextBlock(block) => Some(block),
                _ => None,
            })
            .collect();

        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].value().contains("of interest."));
    }
}
