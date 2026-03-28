//! Stage 12: Heading Detection
//!
//! Detects paragraphs that are likely headings based on font size and weight
//! rarity relative to the document body text. Heading levels are assigned
//! globally by grouping headings with the same TextStyle.
//!
//! ```text
//!   Scoring pipeline per paragraph:
//!
//!     font_size ──► size_score ──┐
//!     font_weight ─► weight_score ├──► base = size × weight × neighbor × lines
//!     neighbors ──► neighbor_score┘        │
//!     line_count ─► lines_mult ────────────┘
//!                                           │
//!     rarity(size)  ──► boost_size  ──┐     │
//!     rarity(weight) ─► boost_weight ─┤     │
//!                                     ▼     ▼
//!                              final = base + boosts
//!                                     │
//!                        final > 0.75 ? → Heading ✓
//!                                       → Paragraph (unchanged)
//! ```

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::semantic::{SemanticHeading, SemanticParagraph};
use crate::tagged::struct_tree::McidMap;
use std::collections::{BTreeMap, HashSet};

/// Probability threshold — a paragraph must score above this to become a heading.
const HEADING_PROBABILITY: f64 = 0.75;

/// Maximum boost from font size rarity.
const FONT_SIZE_RARITY_BOOST: f64 = 0.5;

/// Maximum boost from font weight rarity.
const FONT_WEIGHT_RARITY_BOOST: f64 = 0.3;

/// Body text font size mode search range.
/// Lowered min to 8.0 to handle academic documents where body text is
/// often 8.5–10pt (e.g., journal articles, textbooks).  The original
/// reference value was 10.0–13.0, which incorrectly promoted the
/// heading font as the "body mode" when body text was < 10pt, causing
/// the rarity boost to always return 0 and headings to fail detection.
const FONT_SIZE_DOMINANT_MIN: f64 = 8.0;
const FONT_SIZE_DOMINANT_MAX: f64 = 13.0;

/// Heading candidate font size range (reference: 10.0–32.0).
const FONT_SIZE_HEADING_MIN: f64 = 10.0;
const FONT_SIZE_HEADING_MAX: f64 = 32.0;

/// Body text font weight mode search range.
const FONT_WEIGHT_DOMINANT_MIN: f64 = 395.0;
const FONT_WEIGHT_DOMINANT_MAX: f64 = 405.0;

/// Heading candidate font weight range.
const FONT_WEIGHT_HEADING_MIN: f64 = 400.0;
const FONT_WEIGHT_HEADING_MAX: f64 = 900.0;

/// Maximum lines to still consider as a heading candidate.
const MAX_HEADING_LINES: usize = 4;

/// Minimum text length (chars after trimming) to consider as a heading candidate.
/// The reference implementation has no explicit minimum length — single-character headings like "4" are
/// valid chapter/section numbers.  Using 1 matches the reference permissive behaviour.
const MIN_HEADING_TEXT_LENGTH: usize = 1;

/// Maximum text length (chars) for heading candidates.  Real section headings
/// are short — "4.3.1 Instruction Tuning" ≈ 25 chars, "References" ≈ 10 chars.
/// Run-in bold paragraphs ("**Ablation.** We present results on...") are much
/// longer.  150 chars ≈ 2 full lines at typical heading font sizes.
const MAX_HEADING_TEXT_LENGTH: usize = 150;

// ---------------------------------------------------------------------------
// Neighbor comparison parameters — ported from the reference implementation NodeUtils.headingProbability
// ---------------------------------------------------------------------------

/// Epsilon for font weight comparison (HEADING_EPSILONS[0]).
const WEIGHT_EPSILON: f64 = 0.05;

/// Epsilon for font size comparison — same font (HEADING_EPSILONS[0]).
const SAME_FONT_SIZE_EPSILON: f64 = 0.05;

/// Epsilon for font size comparison — different font (HEADING_EPSILONS[1]).
const DIFF_FONT_SIZE_EPSILON: f64 = 0.08;

/// Boost from text color difference (HEADING_PROBABILITY_PARAMS[4]).
const TEXT_COLOR_BOOST: f64 = 0.1;

/// Boost when candidate is uppercase and neighbor is not (HEADING_PROBABILITY_PARAMS[5]).
const UPPERCASE_BOOST: f64 = 0.25;

/// Penalty when neighbor is uppercase and candidate is not (HEADING_PROBABILITY_PARAMS[6]).
const UPPERCASE_PENALTY: f64 = 0.2;

/// Boost when candidate is far from neighbor — gap > height/2 (DataLoader mode).
const FAR_NEIGHBOR_BOOST: f64 = 0.2;

/// Lines-number penalty factor: final_prob *= max(0, 1 - FACTOR * (lines-1)^2).
const LINES_PENALTY_FACTOR: f64 = 0.05;

/// Minimum one-sided neighbor support required before a numbered section heading
/// can survive an asymmetric reading-order comparison (for example, when the
/// next flat neighbor is actually in the next column and geometrically above the
/// candidate). This keeps the rescue path tied to a real local structural signal.
const NUMBERED_SECTION_ONE_SIDED_SUPPORT: f64 = 0.45;

// Scoring when fonts are the same (HEADING_PROBABILITY_PARAMS_SAME_FONT).
const SAME_FONT_HEAVIER_BOOST: f64 = 0.55; // [0]
const SAME_FONT_LIGHTER_PENALTY: f64 = 0.15; // [1]
const SAME_FONT_LARGER_SIZE_BOOST: f64 = 0.55; // [2]
const SAME_FONT_SMALLER_MAX_SIZE_PENALTY: f64 = 0.4; // [3]
const SAME_FONT_LARGER_PLAIN_BOOST: f64 = 0.5; // [4]
const SAME_FONT_SMALLER_PLAIN_PENALTY: f64 = 0.15; // [5]
const SAME_FONT_LARGE_RATIO_BOOST: f64 = 0.1; // [6]

// Scoring when fonts differ (HEADING_PROBABILITY_PARAMS_DIFF_FONT).
const DIFF_FONT_HEAVIER_BOOST: f64 = 0.46; // [0]
const DIFF_FONT_LIGHTER_PENALTY: f64 = 0.1; // [1]
const DIFF_FONT_LARGER_SIZE_BOOST: f64 = 0.4; // [2]
const DIFF_FONT_SMALLER_MAX_SIZE_PENALTY: f64 = 0.23; // [3]
const DIFF_FONT_LARGER_PLAIN_BOOST: f64 = 0.35; // [4]
const DIFF_FONT_SMALLER_PLAIN_PENALTY: f64 = 0.1; // [5]
const DIFF_FONT_LARGE_RATIO_BOOST: f64 = 0.1; // [6]

/// A sortable text style key for grouping headings by visual appearance.
#[derive(Debug, Clone, PartialEq)]
struct TextStyle {
    font_size: f64,
    font_weight: f64,
}

impl Eq for TextStyle {}

impl PartialOrd for TextStyle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TextStyle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by font size descending, then weight descending
        // (largest/boldest first = level 1)
        other
            .font_size
            .partial_cmp(&self.font_size)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                other
                    .font_weight
                    .partial_cmp(&self.font_weight)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

/// Detect headings across all pages and assign levels.
///
/// This matches the reference `HeadingProcessor.processHeadings()` algorithm:
/// paragraphs are evaluated as-is using neighbor comparison and rarity boosts.
/// The reference implementation never splits paragraphs during heading detection — paragraph boundaries
/// are established by the paragraph formation stage.
///
/// When an MCID map from the structure tree is available (tagged PDFs),
/// tagged headings (H/H1-H6) are promoted directly with their tagged level.
/// Geometry-based detection only runs on paragraphs that aren't tagged as headings.
pub fn detect_headings(pages: &mut [Vec<ContentElement>], mcid_map: Option<&McidMap>) {
    // Phase 0: Tagged heading detection (for tagged PDFs with structure tree)
    let tagged_set = if let Some(map) = mcid_map {
        promote_tagged_headings(pages, map)
    } else {
        HashSet::new()
    };

    // Phase 1: Collect font size/weight statistics from all paragraphs
    let stats = collect_statistics(pages);

    // Phase 1b: Build a flat list of paragraph references for neighbor comparison
    let para_index = build_paragraph_index(pages);

    // Phase 2: Score each paragraph with neighbor comparison and promote headings
    let mut heading_styles: BTreeMap<TextStyle, Vec<(usize, usize)>> = BTreeMap::new();
    let mut promoted: HashSet<(usize, usize)> = HashSet::new();

    for (flat_idx, &(page_idx, elem_idx)) in para_index.iter().enumerate() {
        let elem = &pages[page_idx][elem_idx];
        let p = match elem {
            ContentElement::Paragraph(p) => p,
            _ => continue,
        };

        // Skip paragraphs already promoted as tagged headings
        if tagged_set.contains(&(page_idx, elem_idx)) {
            continue;
        }

        let font_size = p.base.font_size.unwrap_or(0.0);
        let font_weight = p.base.font_weight.unwrap_or(400.0);
        let lines = p.base.lines_number();

        // Skip space-only, very long nodes, or very short text
        if p.base.is_space_node() || lines > MAX_HEADING_LINES {
            continue;
        }

        // Skip very short text (single chars, math symbols) — not headings
        let text_len = p.base.value().trim().len();
        if text_len < MIN_HEADING_TEXT_LENGTH {
            continue;
        }

        // Skip text with fewer than 2 alphabetic characters — isolated math
        // symbols ("O", "M =") or formula fragments are not section headings.
        // Pure-digit text (chapter numbers like "4") is exempt: it has 0 alpha
        // chars but is handled by the existing numeric filter below.
        let trimmed_text = p.base.value().trim().to_string();
        let alpha_count = trimmed_text.chars().filter(|c| c.is_alphabetic()).count();
        if alpha_count == 1 {
            continue;
        }

        // Skip overly long paragraphs — genuine headings are short (section
        // titles).  Run-in bold text ("**Ablation.** We present results...")
        // and body paragraphs with distinctive font properties are always long.
        // 150 chars ≈ 2 full lines at typical heading font sizes.
        if text_len > MAX_HEADING_TEXT_LENGTH {
            continue;
        }

        // Skip text ending with a comma — commas indicate continuation
        // (author lists, enumerated items) and never occur at the end of
        // section headings.  This is a structural text property, not a
        // content heuristic.
        if trimmed_text.ends_with(',') {
            continue;
        }

        // Skip text ending with a hyphen — indicates a word broken at a
        // column/page boundary (e.g. "When ap-"). Section headings are
        // complete phrases and never end with a hyphenated word break.
        if trimmed_text.ends_with('-')
            && trimmed_text.len() > 3
            && trimmed_text.as_bytes()[trimmed_text.len() - 2].is_ascii_alphabetic()
        {
            continue;
        }

        // Skip body text containing internal sentence breaks.
        // A period followed by a space and uppercase letter (". [A-Z]")
        // indicates a sentence boundary within the text.  Section headings
        // are single phrases/titles and never contain sentence breaks.
        // Guards: skip periods preceded by digits (section numbers like "4.3"),
        // single uppercase letters (abbreviations like "U.S."), and periods
        // too close to the start (short prefixes like "Dr. Smith").
        if contains_internal_sentence_break(&trimmed_text) {
            continue;
        }

        // Skip standalone dates — "June 2023", "Jan 2024" etc. are
        // publication metadata, never section headings.
        if is_standalone_date(&trimmed_text) {
            continue;
        }

        // Determine early whether this paragraph is at heading-level font size.
        // Needed to gate the numeric filter: standalone chapter numbers like "4"
        // at 24pt should not be rejected, while table data like "76.1" at body
        // size should still be filtered.
        let is_above_body =
            !stats.has_larger_font_sizes() || stats.is_in_higher_font_sizes(font_size);

        // Skip primarily-numeric text — table data, not headings
        // e.g. "76.1 76.1", "94.3", "0.4 0.6 0.8"
        // Exception: allow above-body-size numeric text — these are typically
        // standalone chapter/section numbers ("4", "II") displayed at heading FS.
        if !is_above_body && is_primarily_numeric(&p.base.value()) {
            continue;
        }

        // Skip standalone page numbers — purely numeric text positioned at the
        // extreme top or bottom margin of the page (top/bottom 8% of page height).
        // Even at heading font size, isolated page numbers like "6" at the page
        // bottom are not headings.  Real chapter numbers ("4") appear within the
        // body area, not at page margins.
        if is_standalone_page_number(p, &stats) {
            continue;
        }

        // Skip continuation text that starts with punctuation (comma, semicolon).
        // These are part of author-name lists or other non-heading content; no
        // valid section heading starts with these characters.
        if p.base.value().trim_start().starts_with([',', ';']) {
            continue;
        }

        // Skip fully parenthesized text — "(Niederle and Vesterlund 2007)" is a
        // citation or parenthetical note, not a section heading. No real heading
        // is enclosed in parentheses.
        if trimmed_text.starts_with('(') && trimmed_text.ends_with(')') {
            continue;
        }

        // Skip text that starts with a lowercase letter — real headings start
        // capitalized ("References"), with a number ("4. Entropy"), or with an
        // uppercase prefix ("B.6 Data Contamination").  Body text fragments that
        // were split from their parent paragraph by column interleaving often
        // start lowercase ("and development in the field of LLMs.").
        let first_alpha = p.base.value().trim().chars().find(|c| c.is_alphabetic());
        if let Some(c) = first_alpha {
            if c.is_lowercase() {
                continue;
            }
        }

        // Skip figure/table captions — "Figure N.", "Table N.", "Fig. N" are
        // captions, not section headings.  They often have distinct font
        // properties (bold, italic) that the scoring would otherwise promote.
        if is_caption_prefix(p.base.value().trim()) {
            continue;
        }

        // Skip text containing email addresses — "EMAIL FOO@BAR.COM" or
        // "Contact: user@domain.org" are contact info, never section headings.
        if contains_email_address(&trimmed_text) {
            continue;
        }

        // Skip text starting with arrow/bullet symbols — ⮚, ▶, ►, ➤, ☛, →
        // These are list-item or callout markers from infographics, not headings.
        if starts_with_bullet_or_arrow(&trimmed_text) && !is_above_body {
            continue;
        }

        if overlaps_detected_table_region(&pages[page_idx], elem_idx, p) {
            continue;
        }

        // Skip already-typed elements (lists should NOT be promoted to headings),
        // UNLESS they are at above-body font size — in some documents headings are
        // mis-classified as list items by the list detector but their font size
        // clearly marks them as section headings.
        if p.base.semantic_type == SemanticType::List && !is_above_body {
            continue;
        }

        // --- Neighbor-based structural scoring (reference-style) ---
        let prev_info = find_neighbor_info(pages, &para_index, flat_idx, false, &promoted);
        let next_info = find_neighbor_info(pages, &para_index, flat_idx, true, &promoted);

        // `is_above_body` was computed earlier (before the numeric filter).
        let neighbor_score = compute_neighbor_score(p, &prev_info, &next_info);

        // Formula-element filter: skip body-size paragraphs whose neighbors on
        // BOTH sides are more than 25% smaller than the candidate.  This pattern
        // (a larger element sandwiched between subscript/superscript-sized
        // fragments) indicates a math formula identifier rather than a section
        // heading.  Genuine headings at above-body FS are exempt: they may have
        // large-font differentials with normal body text, but those are handled
        // correctly by the rarity-boost path.
        if !is_above_body {
            let sub_threshold = font_size * 0.75;
            let prev_subscript = prev_info
                .as_ref()
                .is_some_and(|n| n.font_size < sub_threshold);
            let next_subscript = next_info
                .as_ref()
                .is_some_and(|n| n.font_size < sub_threshold);
            if prev_subscript && next_subscript {
                continue;
            }
        }

        // Lines-number multiplier: penalizes multi-line headings (the reference getLinesNumberHeadingProbability)
        let lines_mult = (1.0 - LINES_PENALTY_FACTOR * ((lines as f64 - 1.0).powi(2))).max(0.0);
        let base_prob = (neighbor_score * lines_mult).clamp(0.0, 1.0);

        // Rarity boosts are added OUTSIDE the multiplier/clamp (matches the reference implementation HeadingProcessor).
        //
        // Rarity boosts should only promote paragraphs that are genuinely above
        // body text size. In 1901-style documents the body and (sub-)heading fonts
        // differ by as little as 0.05 pt, and a continuous 0.2 pt tolerance would
        // erroneously classify body-size bold paragraphs (run-in labels) as
        // "heading-sized".
        //
        // Strategy:
        //  • `is_above_body` uses the same 0.1 pt quantization as `find_higher_values`:
        //    a paragraph is "above body" only if its quantized key strictly exceeds
        //    the mode's quantized key (e.g., 10.909 → key 109 = mode → FALSE;
        //    10.959 → key 110 > 109 → TRUE).
        //  • In multi-size docs, size rarity only applies when above body.
        //  • Weight rarity is always applied (matches the reference implementation HeadingProcessor behaviour).
        //    Phase 2b body-bold filter catches excessive false positives.

        let size_rarity = if is_above_body {
            stats.font_size_rarity_boost(font_size)
        } else {
            0.0 // body-size paragraph in multi-size doc: no false-positive size boost
        };

        // Weight rarity boost is unconditional — the reference implementation applies it to all paragraphs
        // whose font weight is above the document's mode weight.
        let weight_rarity = stats.font_weight_rarity_boost(font_weight);

        let probability = base_prob + size_rarity + weight_rarity;

        if probability >= HEADING_PROBABILITY {
            promoted.insert((page_idx, elem_idx));
            let style = TextStyle {
                font_size,
                font_weight,
            };
            heading_styles
                .entry(style)
                .or_default()
                .push((page_idx, elem_idx));
        }
    }

    // Phase 2b: Filter excessive body-mode-bold headings.
    // In the reference implementation, inline bold text is merged into body paragraphs by alignment-based
    // paragraph formation, so it never becomes a heading candidate. Our weight-based
    // text block grouper separates inline bold into individual paragraphs, creating
    // false positive heading candidates. When body-mode-bold headings outnumber
    // properly-sized headings, it indicates inline bold usage, not section structure.
    if !stats.higher_sizes.is_empty() {
        let body_bold_count: usize = heading_styles
            .iter()
            .filter(|(s, _)| {
                stats.is_body_font_size(s.font_size) && s.font_weight >= BOLD_WEIGHT_THRESHOLD
            })
            .map(|(_, positions)| positions.len())
            .sum();
        let non_body_count: usize = heading_styles
            .iter()
            .filter(|(s, _)| !stats.is_body_font_size(s.font_size))
            .map(|(_, positions)| positions.len())
            .sum();

        if body_bold_count > non_body_count * 2 {
            let styles_to_remove: Vec<TextStyle> = heading_styles
                .keys()
                .filter(|s| {
                    stats.is_body_font_size(s.font_size) && s.font_weight >= BOLD_WEIGHT_THRESHOLD
                })
                .cloned()
                .collect();
            for style in styles_to_remove {
                if let Some(positions) = heading_styles.remove(&style) {
                    for pos in &positions {
                        promoted.remove(pos);
                    }
                }
            }
        }
    }

    // Phase 3: Assign levels (BTreeMap iterates in Ord order: largest/boldest first)
    let mut level = 1u32;
    for positions in heading_styles.values() {
        for &(page_idx, elem_idx) in positions {
            if let ContentElement::Paragraph(p) = &pages[page_idx][elem_idx] {
                let heading = SemanticHeading {
                    base: p.clone(),
                    heading_level: Some(level.min(6)),
                };
                pages[page_idx][elem_idx] = ContentElement::Heading(heading);
            }
        }
        level += 1;
    }
}

/// Info about a neighbor paragraph for heading comparison.
struct NeighborInfo {
    font_size: f64,
    max_font_size: f64,
    font_weight: f64,
    font_name: Option<String>,
    text_color: Option<Vec<f64>>,
    is_heading: bool,
    is_uppercase: bool,
    top_y: f64,
    bottom_y: f64,
    /// Page number of this neighbor — used to skip the direction check for
    /// cross-page comparisons (Y coordinates reset across pages).
    page_number: Option<u32>,
}

/// Build a flat index of all paragraph positions across all pages.
fn build_paragraph_index(pages: &[Vec<ContentElement>]) -> Vec<(usize, usize)> {
    let mut index = Vec::new();
    for (page_idx, page) in pages.iter().enumerate() {
        for (elem_idx, elem) in page.iter().enumerate() {
            match elem {
                ContentElement::Paragraph(_) | ContentElement::Heading(_) => {
                    index.push((page_idx, elem_idx));
                }
                _ => {}
            }
        }
    }
    index
}

fn overlaps_detected_table_region(
    page: &[ContentElement],
    elem_idx: usize,
    para: &SemanticParagraph,
) -> bool {
    let bbox = &para.base.bbox;
    let height = bbox.height().max(1.0);
    let center_x = bbox.center_x();

    page.iter().enumerate().any(|(idx, elem)| {
        if idx == elem_idx {
            return false;
        }
        let table = match elem {
            ContentElement::TableBorder(table) => table,
            _ => return false,
        };

        let tb = &table.bbox;
        if bbox.page_number != tb.page_number {
            return false;
        }
        if center_x < tb.left_x - 6.0 || center_x > tb.right_x + 6.0 {
            return false;
        }

        let overlap = (bbox.top_y.min(tb.top_y) - bbox.bottom_y.max(tb.bottom_y)).max(0.0);
        overlap / height >= 0.6
    })
}

/// Find the prev or next non-space paragraph neighbor.
///
/// When no paragraph/heading neighbor exists in the `para_index` (e.g., all
/// body text was absorbed into cluster tables before heading detection), falls
/// back to the nearest `ContentElement::TableBorder` in the search direction
/// on the same page. This replicates the reference implementation behaviour where `ClusterTableProcessor`
/// runs AFTER heading detection — TOC entries are still free paragraphs when
/// headings are scored in the reference implementation, but in Rust they have already been consumed into
/// cluster tables by Stage 7b.
fn find_neighbor_info(
    pages: &[Vec<ContentElement>],
    para_index: &[(usize, usize)],
    current_flat_idx: usize,
    forward: bool,
    promoted: &HashSet<(usize, usize)>,
) -> Option<NeighborInfo> {
    let mut idx = current_flat_idx as isize;
    loop {
        if forward {
            idx += 1;
        } else {
            idx -= 1;
        }
        if idx < 0 || idx >= para_index.len() as isize {
            break; // no paragraph neighbor found — fall through to table fallback
        }
        let (pi, ei) = para_index[idx as usize];
        let elem = &pages[pi][ei];
        match elem {
            ContentElement::Paragraph(p) => {
                if p.base.is_space_node() {
                    continue;
                }
                return Some(NeighborInfo {
                    font_size: p.base.font_size.unwrap_or(0.0),
                    max_font_size: p
                        .base
                        .max_font_size
                        .unwrap_or(p.base.font_size.unwrap_or(0.0)),
                    font_weight: p.base.font_weight.unwrap_or(400.0),
                    font_name: p.base.font_name.clone(),
                    text_color: p.base.text_color.clone(),
                    is_heading: promoted.contains(&(pi, ei)),
                    is_uppercase: is_uppercase_text(&p.base.value()),
                    top_y: p.base.bbox.top_y,
                    bottom_y: p.base.bbox.bottom_y,
                    page_number: p.base.bbox.page_number,
                });
            }
            ContentElement::Heading(h) => {
                let p = &h.base;
                return Some(NeighborInfo {
                    font_size: p.base.font_size.unwrap_or(0.0),
                    max_font_size: p
                        .base
                        .max_font_size
                        .unwrap_or(p.base.font_size.unwrap_or(0.0)),
                    font_weight: p.base.font_weight.unwrap_or(400.0),
                    font_name: p.base.font_name.clone(),
                    text_color: p.base.text_color.clone(),
                    is_heading: true,
                    is_uppercase: is_uppercase_text(&p.base.value()),
                    top_y: p.base.bbox.top_y,
                    bottom_y: p.base.bbox.bottom_y,
                    page_number: p.base.bbox.page_number,
                });
            }
            _ => continue,
        }
    }

    // Fallback: no paragraph neighbor found in the index. Search the current page's
    // element list for the nearest TableBorder in the search direction and use its
    // cell token font statistics as a proxy body-text neighbor.
    let (page_idx, elem_idx) = para_index[current_flat_idx];
    find_table_border_neighbor(pages, page_idx, elem_idx, forward)
}

/// Search the given page for the nearest non-paragraph content element
/// (`TableBorder` or `List`) in the `forward` direction from `start_elem_idx`
/// and return a proxy `NeighborInfo` built from the median font size of its
/// body tokens. Returns `None` if no element with usable font data is found.
///
/// This handles documents (like TOC pages) where a section heading ("Contents",
/// "Table of Contents") is surrounded by cluster-table or list elements rather
/// than free paragraphs — replicating the reference behaviour where ClusterTableProcessor
/// and list formation run after heading detection.
fn find_table_border_neighbor(
    pages: &[Vec<ContentElement>],
    page_idx: usize,
    start_elem_idx: usize,
    forward: bool,
) -> Option<NeighborInfo> {
    let page = &pages[page_idx];
    let indices: Vec<usize> = if forward {
        ((start_elem_idx + 1)..page.len()).collect()
    } else {
        (0..start_elem_idx).rev().collect()
    };
    for ei in indices {
        match &page[ei] {
            ContentElement::TableBorder(tb) => {
                // Collect all cell-token font sizes (font_size is accurate; font_weight
                // is hardcoded to 400.0 in make_token — acceptable proxy for body text).
                let mut sizes: Vec<f64> = tb
                    .rows
                    .iter()
                    .flat_map(|r| r.cells.iter())
                    .flat_map(|c| c.content.iter())
                    .map(|tok| tok.base.font_size)
                    .filter(|&fs| fs > 0.0)
                    .collect();
                if sizes.is_empty() {
                    continue; // empty table, try next element
                }
                // Use median font size as the representative body-text size.
                sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let font_size = sizes[sizes.len() / 2];

                return Some(NeighborInfo {
                    font_size,
                    max_font_size: font_size,
                    font_weight: 400.0, // cluster table tokens are always regular weight
                    font_name: None,    // unknown (empty string in make_token)
                    text_color: None,
                    is_heading: false,
                    is_uppercase: false,
                    top_y: tb.bbox.top_y,
                    bottom_y: tb.bbox.bottom_y,
                    page_number: tb.bbox.page_number,
                });
            }
            ContentElement::List(lst) => {
                // Collect font sizes from list item body tokens.
                // ListBody.content is Vec<TableTokenRow> = Vec<Vec<TableToken>>.
                let mut sizes: Vec<f64> = lst
                    .list_items
                    .iter()
                    .flat_map(|item| item.body.content.iter())
                    .flat_map(|row| row.iter())
                    .map(|tok| tok.base.font_size)
                    .filter(|&fs| fs > 0.0)
                    .collect();
                if sizes.is_empty() {
                    continue; // empty list, try next element
                }
                sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let font_size = sizes[sizes.len() / 2];

                return Some(NeighborInfo {
                    font_size,
                    max_font_size: font_size,
                    font_weight: 400.0, // list items use regular weight as proxy
                    font_name: None,
                    text_color: None,
                    is_heading: false,
                    is_uppercase: false,
                    top_y: lst.bbox.top_y,
                    bottom_y: lst.bbox.bottom_y,
                    page_number: lst.bbox.page_number,
                });
            }
            _ => continue,
        }
    }
    None
}

/// Compare candidate against one neighbor; returns a score component.
/// Implements the reference NodeUtils.headingProbability(node, neighbor, isNext) in DataLoader mode.
fn score_against_neighbor(
    candidate: &SemanticParagraph,
    neighbor: &NeighborInfo,
    is_next: bool,
) -> f64 {
    let c_size = candidate.base.font_size.unwrap_or(0.0);
    let c_max_size = candidate.base.max_font_size.unwrap_or(c_size);
    let c_weight = candidate.base.font_weight.unwrap_or(400.0);
    let c_font = candidate.base.font_name.as_deref();

    let n_size = neighbor.font_size;
    let n_max_size = neighbor.max_font_size;
    let n_weight = neighbor.font_weight;
    let n_font = neighbor.font_name.as_deref();

    // DataLoader direction check: if next neighbor is actually above us (column
    // boundary / reading-order jump), the comparison is geometrically invalid.
    // The reference implementation returns 0.0 for this case, which then participates in min() and
    // effectively blocks the heading.  We match this by returning 0.0.
    //
    // IMPORTANT: this check only applies to SAME-PAGE neighbors. When the next
    // neighbor is on a different page, Y coordinates reset and the direction is
    // meaningless — skip the check so headings at page boundaries (e.g. a
    // standalone "CONTENTS" heading before a TOC cluster table) can still score
    // correctly against body-text on the following page.
    let same_page = candidate.base.bbox.page_number == neighbor.page_number;
    if is_next && same_page && candidate.base.bbox.top_y < neighbor.top_y {
        return 0.0;
    }

    let same_font = c_font == n_font;
    let size_eps = if same_font {
        SAME_FONT_SIZE_EPSILON
    } else {
        DIFF_FONT_SIZE_EPSILON
    };

    // 5-arg headingProbability: font weight + font size comparison
    let mut score = 0.0;

    // Font weight comparison (weight epsilon is always 0.05)
    if same_font {
        if c_weight > n_weight + WEIGHT_EPSILON {
            score += SAME_FONT_HEAVIER_BOOST;
        } else if n_weight > c_weight + WEIGHT_EPSILON {
            score -= SAME_FONT_LIGHTER_PENALTY;
        }
    } else if c_weight > n_weight + WEIGHT_EPSILON {
        score += DIFF_FONT_HEAVIER_BOOST;
    } else if n_weight > c_weight + WEIGHT_EPSILON {
        score -= DIFF_FONT_LIGHTER_PENALTY;
    }

    // Font size comparison (size epsilon varies by font match)
    if same_font {
        if c_size > n_max_size + size_eps {
            score += SAME_FONT_LARGER_SIZE_BOOST;
        } else if n_size > c_max_size + size_eps {
            score -= SAME_FONT_SMALLER_MAX_SIZE_PENALTY;
        } else if c_size > n_size + size_eps {
            score += SAME_FONT_LARGER_PLAIN_BOOST;
        } else if n_size > c_size + size_eps {
            score -= SAME_FONT_SMALLER_PLAIN_PENALTY;
        }
    } else if c_size > n_max_size + size_eps {
        score += DIFF_FONT_LARGER_SIZE_BOOST;
    } else if n_size > c_max_size + size_eps {
        score -= DIFF_FONT_SMALLER_MAX_SIZE_PENALTY;
    } else if c_size > n_size + size_eps {
        score += DIFF_FONT_LARGER_PLAIN_BOOST;
    } else if n_size > c_size + size_eps {
        score -= DIFF_FONT_SMALLER_PLAIN_PENALTY;
    }

    // DataLoader mode: extra boost if candidate is ≥ 1.5× neighbor size
    if same_font {
        if c_size > 1.5 * n_size + size_eps {
            score += SAME_FONT_LARGE_RATIO_BOOST;
        }
    } else if c_size > 1.5 * n_size + size_eps {
        score += DIFF_FONT_LARGE_RATIO_BOOST;
    }

    // Text color difference (HEADING_PROBABILITY_PARAMS[4])
    if let (Some(c_color), Some(n_color)) = (&candidate.base.text_color, &neighbor.text_color) {
        let diff: f64 = if c_color.len() != n_color.len() {
            1.0 // Different color spaces = different colors
        } else {
            c_color
                .iter()
                .zip(n_color.iter())
                .map(|(a, b)| (a - b).abs())
                .sum()
        };
        if diff > 0.01 {
            score += TEXT_COLOR_BOOST;
        }
    }

    // Uppercase comparison (HEADING_PROBABILITY_PARAMS[5]/[6])
    let c_upper = is_uppercase_text(&candidate.base.value());
    if c_upper && !neighbor.is_uppercase {
        score += UPPERCASE_BOOST;
    } else if !c_upper && neighbor.is_uppercase {
        score -= UPPERCASE_PENALTY;
    }

    // DataLoader far-from-neighbor boost: gap > height/2
    let c_height = candidate.base.bbox.top_y - candidate.base.bbox.bottom_y;
    let gap = if candidate.base.bbox.bottom_y > neighbor.top_y {
        candidate.base.bbox.bottom_y - neighbor.top_y
    } else if neighbor.bottom_y > candidate.base.bbox.top_y {
        neighbor.bottom_y - candidate.base.bbox.top_y
    } else {
        0.0
    };
    if gap >= c_height / 2.0 {
        score += FAR_NEIGHBOR_BOOST;
    }

    score
}

/// Compute the neighbor-based heading score for a paragraph.
///
/// When the next neighbor is geometrically above the candidate (column-break
/// scenario), `score_against_neighbor` returns `0.0` (matching the reference implementation) to signal
/// that the comparison is not informative.
///
/// The reference implementation always uses `min(prev_score, next_score)` — the stricter of the two
/// neighbor comparisons.  We match this exactly.
///
/// The reference implementation returns 1.0 for null neighbors (no prev or no next), so single-paragraph
/// pages or edge positions can still score positively via rarity boosts.
fn compute_neighbor_score(
    para: &SemanticParagraph,
    prev: &Option<NeighborInfo>,
    next: &Option<NeighborInfo>,
) -> f64 {
    let prev_is_heading = prev.as_ref().is_some_and(|n| n.is_heading);
    let numbered_section = is_numbered_section_heading(&para.base.value());

    // The reference headingProbability returns 1.0 for null neighbors.
    // When BOTH are missing, we return 1.0 so rarity boosts alone can promote.
    let prev_score_raw = match prev {
        Some(n) => {
            let s = score_against_neighbor(para, n, false);
            if s.is_finite() {
                Some(s)
            } else {
                None
            }
        }
        None => None, // null neighbor
    };
    let next_score_raw = match next {
        Some(n) => {
            let s = score_against_neighbor(para, n, true);
            if s.is_finite() {
                Some(s)
            } else {
                None
            }
        }
        None => None, // null neighbor
    };

    match (prev_score_raw, next_score_raw) {
        (None, None) => {
            // Both neighbors missing (e.g., single paragraph on page with only tables/lists).
            // The reference implementation returns min(1.0, 1.0) = 1.0 for this case.
            1.0
        }
        (Some(ps), None) => {
            // Only prev exists:
            // The reference implementation returns min(prev_score, 1.0) for next=null.
            // If prev is heading, the reference implementation uses next-only path which returns 1.0 for null next.
            if prev_is_heading {
                1.0
            } else {
                ps.min(1.0)
            }
        }
        (None, Some(ns)) => {
            // Only next exists:
            // The reference implementation returns min(1.0, next_score) for prev=null.
            ns.min(1.0)
        }
        (Some(ps), Some(ns)) => {
            // Both neighbors exist.
            if prev_is_heading {
                // Previous is heading — compare with next only (matches the reference implementation).
                ns
            } else if numbered_section
                && ps.max(ns) >= NUMBERED_SECTION_ONE_SIDED_SUPPORT
                && ps.min(ns) <= 0.0
            {
                // Numbered section headings carry their own strong structural
                // signal. When one side shows clear heading-vs-body contrast
                // and the other side collapses to a non-positive comparison,
                // prefer the informative side instead of letting a cross-column
                // neighbor jump erase the heading entirely.
                ps.max(ns)
            } else {
                // The reference implementation always uses min(prev, next) — the stricter of the two
                // neighbor comparisons.  Using min() prevents one favourable
                // direction from masking an unfavourable one, reducing false
                // positives for body-bold paragraphs that happen to sit next
                // to a smaller-font element.
                ps.min(ns)
            }
        }
    }
}

/// Check if a string is entirely uppercase (only considering alphabetic characters).
fn is_uppercase_text(text: &str) -> bool {
    let mut has_alpha = false;
    for c in text.chars() {
        if c.is_alphabetic() {
            has_alpha = true;
            if !c.is_uppercase() {
                return false;
            }
        }
    }
    has_alpha
}

/// Check if text is primarily numeric (digits, dots, spaces, dashes, commas).
/// Text like "76.1 76.1", "94.3", "0.4 0.6 0.8" should not be headings.
fn is_primarily_numeric(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let alpha_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
    let total = trimmed.chars().count();
    // If alphabetic chars are less than 30% of total, treat as numeric
    alpha_count * 100 / total < 30
}

/// Check if text contains an internal sentence break — a period followed by
/// a space and an uppercase letter (e.g. "results. When") that is not part
/// of a number pattern or single-letter abbreviation.
fn contains_internal_sentence_break(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 4 {
        return false;
    }
    for i in 1..bytes.len().saturating_sub(2) {
        if bytes[i] != b'.' {
            continue;
        }
        if i + 2 >= bytes.len() || bytes[i + 1] != b' ' || !bytes[i + 2].is_ascii_uppercase() {
            continue;
        }
        // Skip periods preceded by a digit (section numbers: "4.3 Results")
        if bytes[i - 1].is_ascii_digit() {
            continue;
        }
        // Skip periods preceded by a single uppercase letter (abbreviations: "U.S.")
        if i >= 2
            && bytes[i - 1].is_ascii_uppercase()
            && (i < 3 || !bytes[i - 2].is_ascii_alphanumeric())
        {
            continue;
        }
        // Skip periods too close to the start (short prefixes: "Dr. Smith", "Mr. Jones")
        if i < 12 {
            continue;
        }
        return true;
    }
    false
}

/// Standalone date check — "June 2023", "Jan 2024", etc.
/// Publication metadata is never a section heading.
fn is_standalone_date(text: &str) -> bool {
    const MONTHS: &[&str] = &[
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
        "jan",
        "feb",
        "mar",
        "apr",
        "jun",
        "jul",
        "aug",
        "sep",
        "oct",
        "nov",
        "dec",
    ];
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() != 2 {
        return false;
    }
    let word0_lower = words[0].to_lowercase();
    let is_month = MONTHS.contains(&word0_lower.as_str());
    let is_year = words[1].len() == 4 && words[1].chars().all(|c| c.is_ascii_digit());
    is_month && is_year
}

/// Check if a paragraph is a standalone page number positioned at the extreme
/// top or bottom margin of its page.  Such elements look like headings (isolated,
/// sometimes bold) but are not section headings.
///
/// Detection criteria:
///  1. Text is purely numeric (no alphabetic chars).
///  2. Text is short (1-4 digits, e.g. "6", "123").
///  3. The paragraph's Y-position is in the top or bottom 10% of the typical
///     page height (estimated from document statistics).
fn is_standalone_page_number(para: &SemanticParagraph, stats: &DocFontStats) -> bool {
    let text = para.base.value();
    let trimmed = text.trim();

    // Must be purely numeric and short (page numbers are 1-4 digits).
    if trimmed.is_empty() || trimmed.len() > 4 {
        return false;
    }
    if !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // Must have no alphabetic characters at all.
    if trimmed.chars().any(|c| c.is_alphabetic()) {
        return false;
    }

    // Check vertical position: top or bottom 10% of the page.
    // Estimate page height from the body font mode size.
    // Use the paragraph's own Y-coordinates vs. standard page margins.
    // A4 = 842pt; US Letter = 792pt.  Use 800pt as an approximation.
    // Header/footer zone: within 10% of page top or bottom = 80pt from edge.
    let top_y = para.base.bbox.top_y;
    let bottom_y = para.base.bbox.bottom_y;

    // Page number at top (above ~720pt on a 800pt page = top 10%).
    let at_top = top_y > 720.0;
    // Page number at bottom (below ~80pt on a 800pt page = bottom 10%).
    let at_bottom = bottom_y < 80.0;

    // Also check with typical A4 (842pt) and Letter (792pt) ranges.
    let at_top_a4 = top_y > 758.0; // top 10% of 842pt page
    let at_bottom_a4 = bottom_y < 84.0; // bottom 10% of 842pt page

    // Use body font mode size as additional signal: if the page number font
    // is the same as body text, it is unlikely to be a heading even if at body position.
    let _ = stats; // stats available for future use

    at_top || at_bottom || at_top_a4 || at_bottom_a4
}

/// Check if text starts with a figure/table caption prefix.
/// "Figure 1.", "Table 2:", "Fig. 3", "FIGURE 4" are captions.
fn is_caption_prefix(text: &str) -> bool {
    let lower = text.to_lowercase();
    // Match "figure N", "fig. N", "fig N", "table N"
    if lower.starts_with("figure ")
        || lower.starts_with("fig. ")
        || lower.starts_with("fig ")
        || lower.starts_with("table ")
    {
        // Check if the next non-space character is a digit
        let rest = if lower.starts_with("figure ") {
            &text[7..]
        } else if lower.starts_with("fig. ") {
            &text[5..]
        } else if lower.starts_with("fig ") {
            &text[4..]
        } else {
            &text[6..]
        };
        let first_non_space = rest.trim_start().chars().next();
        return first_non_space.is_some_and(|c| c.is_ascii_digit());
    }
    false
}

/// Check if text contains an email address pattern (something@domain.tld).
fn contains_email_address(text: &str) -> bool {
    if let Some(at_pos) = text.find('@') {
        // Check for at least one char before @ and a dot after @
        let before = &text[..at_pos];
        let after = &text[at_pos + 1..];
        let has_prefix = before
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric() || c == '.' || c == '_');
        let has_domain = after.contains('.');
        return has_prefix && has_domain;
    }
    false
}

/// Check if text starts with an arrow or bullet symbol that indicates a list
/// item or callout marker rather than a section heading.
fn starts_with_bullet_or_arrow(text: &str) -> bool {
    let first = text.chars().next();
    matches!(
        first,
        Some(
            '⮚' | '▶'
                | '►'
                | '➤'
                | '☛'
                | '→'
                | '➜'
                | '➔'
                | '⯈'
                | '◆'
                | '◉'
                | '▸'
                | '‣'
        )
    )
}

/// Detect numbered section headings such as "1 Introduction",
/// "4.2 Main Results", or "B.6 Data Contamination".
///
/// This is intentionally narrower than general list parsing:
/// - a section-number prefix must be followed by title-like text,
/// - the title must start with an uppercase letter,
/// - sentence-ending punctuation is rejected to avoid list items / body text.
fn is_numbered_section_heading(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_HEADING_TEXT_LENGTH {
        return false;
    }

    let title = if let Some(rest) = strip_numeric_section_prefix(trimmed) {
        rest
    } else if let Some(rest) = strip_appendix_section_prefix(trimmed) {
        rest
    } else {
        return false;
    };

    let title = title.trim();
    if title.is_empty() {
        return false;
    }

    if title.ends_with(['.', ':', ';', '?', '!']) {
        return false;
    }

    let first_alpha = match title.chars().find(|c| c.is_alphabetic()) {
        Some(c) => c,
        None => return false,
    };
    if !first_alpha.is_uppercase() {
        return false;
    }

    let alpha_count = title.chars().filter(|c| c.is_alphabetic()).count();
    let word_count = title.split_whitespace().count();
    alpha_count >= 3 && (1..=16).contains(&word_count)
}

fn strip_numeric_section_prefix(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut idx = 0;
    let mut segments = 0;

    loop {
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx == start || idx - start > 3 {
            return None;
        }
        segments += 1;
        if segments > 4 {
            return None;
        }

        if idx >= bytes.len() {
            return None;
        }

        match bytes[idx] {
            b'.' => {
                idx += 1;
                if idx < bytes.len() && bytes[idx].is_ascii_digit() {
                    continue;
                }
                if idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                        idx += 1;
                    }
                    return Some(&text[idx..]);
                }
                return None;
            }
            b' ' | b'\t' => {
                while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                    idx += 1;
                }
                return Some(&text[idx..]);
            }
            _ => return None,
        }
    }
}

fn strip_appendix_section_prefix(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_uppercase() || bytes[1] != b'.' {
        return None;
    }

    let rest = &text[2..];
    strip_numeric_section_prefix(rest)
}

/// Font statistics collected from all paragraphs in the document.
struct DocFontStats {
    /// Body text font size mode (most frequent in dominant range).
    mode_size: Option<f64>,
    /// Font sizes larger than the mode, sorted ascending.
    higher_sizes: Vec<f64>,
    /// Font weights larger than the mode, sorted ascending.
    higher_weights: Vec<f64>,
}

impl DocFontStats {
    fn font_size_rarity_boost(&self, size: f64) -> f64 {
        rarity_boost(size, &self.higher_sizes, FONT_SIZE_RARITY_BOOST)
    }

    fn font_weight_rarity_boost(&self, weight: f64) -> f64 {
        rarity_boost(weight, &self.higher_weights, FONT_WEIGHT_RARITY_BOOST)
    }

    /// Returns true when the document contains paragraphs at font sizes above the body
    /// mode — i.e., there are genuine larger-size heading candidates.
    fn has_larger_font_sizes(&self) -> bool {
        !self.higher_sizes.is_empty()
    }

    /// Returns true if `size` is strictly above the body text mode (using
    /// the same 0.1pt quantization used when building `higher_sizes`).
    /// This correctly separates body-mode paragraphs (e.g., 10.909pt) from
    /// slightly-larger heading paragraphs (e.g., 10.959pt → quantized 11.0pt)
    /// when both would otherwise fall within a continuous 0.2pt distance.
    fn is_in_higher_font_sizes(&self, size: f64) -> bool {
        let size_key = (size * 10.0).round() as i32;
        let mode_key = match self.mode_size {
            Some(m) => (m * 10.0).round() as i32,
            None => return false,
        };
        size_key > mode_key && !self.higher_sizes.is_empty()
    }

    /// Check if a font size matches the body text mode (quantized to 0.1pt).
    fn is_body_font_size(&self, size: f64) -> bool {
        self.mode_size
            .is_some_and(|m| (size * 10.0).round() as i32 == (m * 10.0).round() as i32)
    }
}

/// Calculate rarity boost for a value in a sorted ascending list.
fn rarity_boost(value: f64, higher_values: &[f64], max_boost: f64) -> f64 {
    if higher_values.is_empty() {
        return 0.0;
    }

    // Find position in sorted list (tolerance-based matching, 0.2pt resolution)
    let pos = higher_values.iter().position(|&v| (v - value).abs() < 0.2);

    match pos {
        Some(i) => {
            let rank = (i + 1) as f64 / higher_values.len() as f64;
            rank * max_boost
        }
        None => 0.0,
    }
}

/// Extract a representative font size from a ContentElement.
/// Returns None for elements without readable font size information.
/// Used by `collect_statistics` to gather body-text font size evidence from
/// List item contents (TextLine, TextChunk) and other non-Paragraph elements.
fn content_element_font_size(elem: &ContentElement) -> Option<f64> {
    match elem {
        ContentElement::TextLine(tl) => {
            if tl.font_size > 0.0 {
                Some(tl.font_size)
            } else {
                None
            }
        }
        ContentElement::TextBlock(tb) => {
            if tb.font_size > 0.0 {
                Some(tb.font_size)
            } else {
                None
            }
        }
        ContentElement::TextChunk(tc) => {
            if tc.font_size > 0.0 {
                Some(tc.font_size)
            } else {
                None
            }
        }
        ContentElement::Paragraph(p) => p.base.font_size.filter(|&s| s > 0.0),
        _ => None,
    }
}

/// Collect font size and weight statistics from all paragraphs.
fn collect_statistics(pages: &[Vec<ContentElement>]) -> DocFontStats {
    let mut size_counts: BTreeMap<i32, usize> = BTreeMap::new();
    let mut weight_counts: BTreeMap<i32, usize> = BTreeMap::new();

    for page in pages {
        for elem in page {
            match elem {
                ContentElement::Paragraph(p) => {
                    if p.base.is_space_node() {
                        continue;
                    }
                    let size = p.base.font_size.unwrap_or(0.0);
                    let weight = p.base.font_weight.unwrap_or(400.0);

                    // Quantize to avoid floating point fragmentation
                    let size_key = (size * 10.0).round() as i32; // 0.1pt resolution
                    let weight_key = weight.round() as i32;

                    *size_counts.entry(size_key).or_insert(0) += 1;
                    *weight_counts.entry(weight_key).or_insert(0) += 1;
                }
                ContentElement::List(lst) => {
                    // Include list item content font sizes in body-text statistics.
                    // In documents where body text is consumed into a List before heading
                    // detection (e.g., TOC pages), these elements represent the true body
                    // font distribution and let the heading detector correctly identify the
                    // mode/higher sizes for paragraphs like "Contents" or "Table of Contents".
                    // Note: item.body.content is always empty (populated later); the actual
                    // content is in item.contents: Vec<ContentElement>.
                    for item in &lst.list_items {
                        for content_elem in &item.contents {
                            if let Some(size) = content_element_font_size(content_elem) {
                                if size > 0.0 {
                                    let size_key = (size * 10.0).round() as i32;
                                    *size_counts.entry(size_key).or_insert(0) += 1;
                                    *weight_counts.entry(400).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
                ContentElement::TableBorder(tb) => {
                    // Include table cell token font sizes in body-text statistics.
                    // Similar rationale to List: on TOC pages, all body text may be
                    // inside cluster tables, leaving only the heading as a Paragraph.
                    for row in &tb.rows {
                        for cell in &row.cells {
                            for tok in &cell.content {
                                let size = tok.base.font_size;
                                if size > 0.0 {
                                    let size_key = (size * 10.0).round() as i32;
                                    *size_counts.entry(size_key).or_insert(0) += 1;
                                    *weight_counts.entry(400).or_insert(0) += 1;
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let size_mode = find_mode(
        &size_counts,
        FONT_SIZE_DOMINANT_MIN,
        FONT_SIZE_DOMINANT_MAX,
        10.0,
    )
    .or_else(|| {
        // Fallback: use the most frequent font size overall as body text
        size_counts
            .iter()
            .max_by_key(|&(_, &count)| count)
            .map(|(&key, _)| key as f64 / 10.0)
    });
    let higher_sizes = find_higher_values(
        &size_counts,
        size_mode,
        FONT_SIZE_HEADING_MIN,
        FONT_SIZE_HEADING_MAX,
        10.0,
    );

    let weight_mode = find_mode(
        &weight_counts,
        FONT_WEIGHT_DOMINANT_MIN,
        FONT_WEIGHT_DOMINANT_MAX,
        1.0,
    );
    let higher_weights = find_higher_values(
        &weight_counts,
        weight_mode,
        FONT_WEIGHT_HEADING_MIN,
        FONT_WEIGHT_HEADING_MAX,
        1.0,
    );

    DocFontStats {
        mode_size: size_mode,
        higher_sizes,
        higher_weights,
    }
}

/// Find the mode (most frequent value) within [min, max] range.
fn find_mode(counts: &BTreeMap<i32, usize>, min: f64, max: f64, scale: f64) -> Option<f64> {
    let min_key = (min * scale).round() as i32;
    let max_key = (max * scale).round() as i32;

    counts
        .range(min_key..=max_key)
        .max_by_key(|&(_, &count)| count)
        .map(|(&key, _)| key as f64 / scale)
}

/// Find all distinct values strictly larger than the mode and within [min, max], sorted ascending.
fn find_higher_values(
    counts: &BTreeMap<i32, usize>,
    mode: Option<f64>,
    min: f64,
    max: f64,
    scale: f64,
) -> Vec<f64> {
    let mode_val = match mode {
        Some(m) => m,
        None => return vec![],
    };

    let min_key = (min * scale).round() as i32;
    let max_key = (max * scale).round() as i32;
    let mode_key = (mode_val * scale).round() as i32;

    counts
        .range(min_key..=max_key)
        .filter(|(&key, _)| key > mode_key)
        .map(|(&key, _)| key as f64 / scale)
        .collect()
}

// ---------------------------------------------------------------------------
// Constants for body-bold heading filtering
// ---------------------------------------------------------------------------

/// Minimum font weight difference to consider a bold→regular transition.
const BOLD_WEIGHT_THRESHOLD: f64 = 600.0;

// ---------------------------------------------------------------------------
// Phase 0: Tagged heading promotion (structure tree → heading conversion)
// ---------------------------------------------------------------------------

/// Promote paragraphs whose MCID maps to a heading tag (H/H1-H6) in the
/// structure tree. Returns the set of (page_idx, elem_idx) positions that
/// were promoted so the geometry-based Phase 2 can skip them.
fn promote_tagged_headings(
    pages: &mut [Vec<ContentElement>],
    mcid_map: &McidMap,
) -> HashSet<(usize, usize)> {
    let mut tagged_set = HashSet::new();

    for (page_idx, page) in pages.iter_mut().enumerate() {
        // First pass: identify paragraphs to promote and build headings
        let to_promote: Vec<(usize, SemanticHeading)> = page
            .iter()
            .enumerate()
            .filter_map(|(elem_idx, elem)| {
                if let ContentElement::Paragraph(p) = elem {
                    let level = find_tagged_heading_level(p, mcid_map)?;
                    Some((
                        elem_idx,
                        SemanticHeading {
                            base: p.clone(),
                            heading_level: Some(level.min(6)),
                        },
                    ))
                } else {
                    None
                }
            })
            .collect();
        // Second pass: apply promotions
        for (elem_idx, heading) in to_promote {
            page[elem_idx] = ContentElement::Heading(heading);
            tagged_set.insert((page_idx, elem_idx));
        }
    }

    tagged_set
}

/// Check if any TextChunk in a paragraph has an MCID that maps to a heading
/// tag in the structure tree. Returns the heading level if found.
fn find_tagged_heading_level(para: &SemanticParagraph, mcid_map: &McidMap) -> Option<u32> {
    for column in &para.base.columns {
        for block in &column.text_blocks {
            for line in &block.text_lines {
                for chunk in &line.text_chunks {
                    if let (Some(page_num), Some(mcid)) = (chunk.page_number, chunk.mcid) {
                        if let Some(tag_info) = mcid_map.get(&(page_num, mcid)) {
                            if let Some(level) = tag_info.heading_level {
                                return Some(level);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, SemanticType, TextFormat, TextType};
    use crate::models::semantic::SemanticTextNode;
    use crate::models::table::TableBorder;
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    fn make_paragraph(
        text: &str,
        page: u32,
        font_size: f64,
        font_weight: f64,
        bottom_y: f64,
    ) -> ContentElement {
        let bbox = BoundingBox::new(Some(page), 72.0, bottom_y, 300.0, bottom_y + font_size);
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: "Helvetica".to_string(),
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
        let block = TextBlock {
            bbox: bbox.clone(),
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
        };
        let column = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size,
            base_line: bottom_y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: crate::models::semantic::SemanticTextNode {
                bbox,
                index: None,
                level: None,
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
                is_hidden_text: false,
            },
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    #[test]
    fn test_no_headings_same_size() {
        // All paragraphs have the same font size — no headings
        let mut pages = vec![vec![
            make_paragraph("Para 1", 1, 12.0, 400.0, 700.0),
            make_paragraph("Para 2", 1, 12.0, 400.0, 680.0),
            make_paragraph("Para 3", 1, 12.0, 400.0, 660.0),
        ]];
        detect_headings(&mut pages, None);
        for elem in &pages[0] {
            assert!(matches!(elem, ContentElement::Paragraph(_)));
        }
    }

    #[test]
    fn test_larger_font_becomes_heading() {
        // One paragraph with larger font → heading
        let mut pages = vec![vec![
            make_paragraph("Title", 1, 18.0, 700.0, 740.0),
            make_paragraph("Body text 1", 1, 12.0, 400.0, 700.0),
            make_paragraph("Body text 2", 1, 12.0, 400.0, 680.0),
            make_paragraph("Body text 3", 1, 12.0, 400.0, 660.0),
        ]];
        detect_headings(&mut pages, None);
        assert!(matches!(pages[0][0], ContentElement::Heading(_)));
        assert!(matches!(pages[0][1], ContentElement::Paragraph(_)));
    }

    #[test]
    fn test_heading_level_assignment() {
        // Two distinct heading sizes → level 1 and level 2
        let mut pages = vec![vec![
            make_paragraph("Chapter", 1, 20.0, 700.0, 760.0),
            make_paragraph("Section", 1, 16.0, 700.0, 740.0),
            make_paragraph("Body 1", 1, 12.0, 400.0, 700.0),
            make_paragraph("Body 2", 1, 12.0, 400.0, 680.0),
            make_paragraph("Body 3", 1, 12.0, 400.0, 660.0),
        ]];
        detect_headings(&mut pages, None);

        if let ContentElement::Heading(h) = &pages[0][0] {
            assert_eq!(h.heading_level, Some(1));
        } else {
            panic!("Expected Heading for 'Chapter'");
        }

        if let ContentElement::Heading(h) = &pages[0][1] {
            assert_eq!(h.heading_level, Some(2));
        } else {
            panic!("Expected Heading for 'Section'");
        }
    }

    #[test]
    fn test_bold_heading() {
        // Bold paragraph (weight 700) among normal (400) → heading
        let mut pages = vec![vec![
            make_paragraph("Bold Title", 1, 12.0, 700.0, 740.0),
            make_paragraph("Normal body 1", 1, 12.0, 400.0, 700.0),
            make_paragraph("Normal body 2", 1, 12.0, 400.0, 680.0),
            make_paragraph("Normal body 3", 1, 12.0, 400.0, 660.0),
        ]];
        detect_headings(&mut pages, None);
        assert!(matches!(pages[0][0], ContentElement::Heading(_)));
    }

    #[test]
    fn test_empty_pages_no_crash() {
        let mut pages: Vec<Vec<ContentElement>> = vec![vec![], vec![]];
        detect_headings(&mut pages, None);
        assert!(pages[0].is_empty());
    }

    #[test]
    fn test_cross_page_heading_detection() {
        // Heading on page 1, body on pages 1 and 2
        let mut pages = vec![
            vec![
                make_paragraph("Title", 1, 18.0, 700.0, 740.0),
                make_paragraph("Page 1 body", 1, 12.0, 400.0, 700.0),
            ],
            vec![
                make_paragraph("Page 2 body 1", 2, 12.0, 400.0, 700.0),
                make_paragraph("Page 2 body 2", 2, 12.0, 400.0, 680.0),
            ],
        ];
        detect_headings(&mut pages, None);
        assert!(matches!(pages[0][0], ContentElement::Heading(_)));
        assert!(matches!(pages[1][0], ContentElement::Paragraph(_)));
    }

    #[test]
    fn test_max_heading_level_clamped() {
        // 7 distinct heading sizes → levels should clamp at 6
        let mut pages = vec![vec![
            make_paragraph("H1", 1, 28.0, 700.0, 800.0),
            make_paragraph("H2", 1, 24.0, 700.0, 770.0),
            make_paragraph("H3", 1, 20.0, 700.0, 740.0),
            make_paragraph("H4", 1, 18.0, 700.0, 720.0),
            make_paragraph("H5", 1, 16.0, 700.0, 700.0),
            make_paragraph("H6", 1, 14.0, 700.0, 680.0),
            make_paragraph("H7", 1, 13.5, 600.0, 660.0),
            make_paragraph("Body 1", 1, 12.0, 400.0, 640.0),
            make_paragraph("Body 2", 1, 12.0, 400.0, 620.0),
            make_paragraph("Body 3", 1, 12.0, 400.0, 600.0),
        ]];
        detect_headings(&mut pages, None);

        // Last heading should be clamped at 6
        let mut max_level = 0;
        for elem in &pages[0] {
            if let ContentElement::Heading(h) = elem {
                if let Some(l) = h.heading_level {
                    max_level = max_level.max(l);
                }
            }
        }
        assert!(
            max_level <= 6,
            "Heading level should be clamped at 6, got {}",
            max_level
        );
    }

    #[test]
    fn test_rarity_boost_calculation() {
        let higher = vec![14.0, 16.0, 20.0];
        assert!((rarity_boost(14.0, &higher, 0.5) - 1.0 / 3.0 * 0.5).abs() < 0.01);
        assert!((rarity_boost(16.0, &higher, 0.5) - 2.0 / 3.0 * 0.5).abs() < 0.01);
        assert!((rarity_boost(20.0, &higher, 0.5) - 3.0 / 3.0 * 0.5).abs() < 0.01);
        assert_eq!(rarity_boost(12.0, &higher, 0.5), 0.0);
    }

    #[test]
    fn test_find_mode() {
        let mut counts = BTreeMap::new();
        counts.insert(100, 5); // 10.0pt, 5 occurrences
        counts.insert(120, 10); // 12.0pt, 10 occurrences (mode)
        counts.insert(140, 2); // 14.0pt, 2 occurrences
        assert_eq!(find_mode(&counts, 10.0, 13.0, 10.0), Some(12.0));
    }

    #[test]
    fn test_numbered_section_heading_matcher() {
        assert!(is_numbered_section_heading("1 Introduction"));
        assert!(is_numbered_section_heading("4.2 Main Results"));
        assert!(is_numbered_section_heading("4.3.1 Instruction Tuning"));
        assert!(is_numbered_section_heading("B.6 Data Contamination"));

        assert!(!is_numbered_section_heading("Indirect communications"));
        assert!(!is_numbered_section_heading("56% AGREE"));
        assert!(!is_numbered_section_heading("1. First item in a list."));
        assert!(!is_numbered_section_heading("1. first item"));
    }

    #[test]
    fn test_numbered_section_heading_survives_cross_column_neighbor_jump() {
        // Simulate left-column section heading followed in flat order by the
        // start of the right column, which sits geometrically above it.
        let mut pages = vec![vec![
            make_paragraph("Body text before the section.", 1, 10.9, 400.0, 220.0),
            make_paragraph("4.2 Main Results", 1, 10.9, 700.0, 180.0),
            make_paragraph("Right column body starts here.", 1, 10.9, 400.0, 680.0),
            make_paragraph("Continuation body text.", 1, 10.9, 400.0, 160.0),
            make_paragraph("More body text.", 1, 10.9, 400.0, 140.0),
        ]];

        detect_headings(&mut pages, None);

        assert!(
            matches!(pages[0][1], ContentElement::Heading(_)),
            "Expected numbered section heading to survive asymmetric neighbor comparison"
        );
    }

    /// Helper: create a multi-line paragraph with per-line font properties.
    fn make_multiline_paragraph(
        lines: &[(&str, f64, f64)], // (text, font_size, font_weight)
        page: u32,
    ) -> ContentElement {
        let mut text_lines = Vec::new();
        let mut y = 700.0;
        for &(text, fs, fw) in lines {
            let bbox = BoundingBox::new(Some(page), 72.0, y, 300.0, y + fs);
            let chunk = TextChunk {
                value: text.to_string(),
                bbox: bbox.clone(),
                font_name: "Helvetica".to_string(),
                font_size: fs,
                font_weight: fw,
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
            text_lines.push(TextLine {
                bbox: bbox.clone(),
                index: None,
                level: None,
                font_size: fs,
                base_line: y + 2.0,
                slant_degree: 0.0,
                is_hidden_text: false,
                text_chunks: vec![chunk],
                is_line_start: true,
                is_line_end: true,
                is_list_line: false,
                connected_line_art_label: None,
            });
            y -= fs + 2.0;
        }

        // Use first line's properties as dominant (will be overridden for body after split)
        let dominant_fs = lines.iter().map(|(_, fs, _)| *fs).fold(0.0_f64, |acc, x| {
            if acc == 0.0 {
                x
            } else {
                acc.min(x)
            }
        });
        let dominant_fw = 400.0; // Body text dominates in a merged paragraph

        let outer_bbox = text_lines
            .iter()
            .fold(text_lines[0].bbox.clone(), |acc, l| acc.union(&l.bbox));
        let block = TextBlock {
            bbox: outer_bbox.clone(),
            index: None,
            level: None,
            font_size: dominant_fs,
            base_line: text_lines.last().unwrap().base_line,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines,
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let column = TextColumn {
            bbox: outer_bbox.clone(),
            index: None,
            level: None,
            font_size: dominant_fs,
            base_line: block.base_line,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: SemanticTextNode {
                bbox: outer_bbox,
                index: None,
                level: None,
                semantic_type: SemanticType::Paragraph,
                correct_semantic_score: None,
                columns: vec![column],
                font_weight: Some(dominant_fw),
                font_size: Some(dominant_fs),
                text_color: None,
                italic_angle: None,
                font_name: None,
                text_format: None,
                max_font_size: Some(lines.iter().map(|(_, fs, _)| *fs).fold(0.0_f64, f64::max)),
                background_color: None,
                is_hidden_text: false,
            },
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    #[test]
    fn test_no_split_bold_merged_with_body() {
        // The reference HeadingProcessor never splits paragraphs.
        // A paragraph where "Abstract" (bold 12pt) is merged with body text (10.9pt)
        // should NOT be split during heading detection. Paragraph splitting is the
        // responsibility of the paragraph formation stage, not the heading detector.
        // This verifies that detect_headings matches the reference behaviour exactly:
        // paragraphs are evaluated as-is, never torn apart inside this stage.
        let mut pages = vec![vec![
            make_multiline_paragraph(
                &[
                    ("Abstract", 12.0, 700.0),
                    (
                        "This paper presents a novel approach to irradiance",
                        10.9,
                        400.0,
                    ),
                    (
                        "fields using neural networks and deep learning.",
                        10.9,
                        400.0,
                    ),
                    ("We demonstrate significant improvements.", 10.9, 400.0),
                ],
                1,
            ),
            make_paragraph("Body text continues", 1, 10.9, 400.0, 600.0),
        ]];

        detect_headings(&mut pages, None);

        // The number of elements must not change — no paragraph was split into two.
        assert_eq!(
            pages[0].len(),
            2,
            "detect_headings must not split paragraphs; expected 2 elements, got {}",
            pages[0].len()
        );
    }

    #[test]
    fn test_table_overlapping_label_is_not_promoted_to_heading() {
        let mut pages = vec![vec![
            make_paragraph("Reference frameworks:", 1, 13.0, 700.0, 730.0),
            make_paragraph(
                "2. Embracing complexity in sustainability",
                1,
                11.0,
                700.0,
                230.0,
            ),
            ContentElement::TableBorder(TableBorder {
                bbox: BoundingBox::new(Some(1), 70.0, 180.0, 540.0, 260.0),
                index: None,
                level: None,
                x_coordinates: vec![],
                x_widths: vec![],
                y_coordinates: vec![],
                y_widths: vec![],
                rows: vec![],
                num_rows: 0,
                num_columns: 0,
                is_bad_table: false,
                is_table_transformer: false,
                previous_table: None,
                next_table: None,
            }),
            make_paragraph("Body paragraph one", 1, 11.0, 400.0, 150.0),
            make_paragraph("Body paragraph two", 1, 11.0, 400.0, 130.0),
            make_paragraph("Body paragraph three", 1, 11.0, 400.0, 110.0),
        ]];

        detect_headings(&mut pages, None);

        assert!(matches!(pages[0][0], ContentElement::Heading(_)));
        assert!(matches!(pages[0][1], ContentElement::Paragraph(_)));
    }
}
