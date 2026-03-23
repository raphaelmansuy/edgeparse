//! Text grouping types — TextLine, TextBlock, TextColumn.

use serde::{Deserialize, Serialize};

use super::bbox::BoundingBox;
use super::chunks::{LineArtChunk, TextChunk};
use super::enums::TextAlignment;

/// A horizontal group of TextChunks sharing a baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLine {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Dominant font size
    pub font_size: f64,
    /// Baseline Y coordinate
    pub base_line: f64,
    /// Slant degree
    pub slant_degree: f64,
    /// Whether all text is hidden
    pub is_hidden_text: bool,
    /// Component text chunks
    pub text_chunks: Vec<TextChunk>,
    /// Whether this line starts a new paragraph
    pub is_line_start: bool,
    /// Whether this line ends a paragraph
    pub is_line_end: bool,
    /// Whether this line is part of a list
    pub is_list_line: bool,
    /// Connected line art (bullet marker)
    pub connected_line_art_label: Option<LineArtChunk>,
}

impl TextLine {
    /// Concatenated text value of all chunks, inserting spaces between
    /// chunks when a horizontal gap indicates a word boundary.
    ///
    /// Whitespace-only chunks are skipped (matching the reference processTextLines
    /// which skips `isWhiteSpaceChunk()` chunks); word spaces are re-detected
    /// from bounding-box gaps via `needs_space()`.
    ///
    /// For letter-spaced text (≥70% of chunks are single-character), an adaptive
    /// gap threshold based on the median inter-chunk gap is used instead of the
    /// fixed `fontSize * 0.17` rule. This correctly collapses text like
    /// `"H O W  C A N"` into `"HOW CAN"`.
    pub fn value(&self) -> String {
        // Filter to non-whitespace, non-empty chunks (reference behaviour).
        let real_chunks: Vec<&TextChunk> = self
            .text_chunks
            .iter()
            .filter(|c| !c.value.is_empty() && !c.is_white_space_chunk())
            .collect();

        Self::concatenate_chunk_refs(&real_chunks)
    }

    /// Concatenate a slice of owned TextChunks using gap-based word boundary
    /// detection.  Handles letter-spaced text with adaptive threshold.
    ///
    /// For multi-line content (e.g. table cells), chunks on different visual
    /// lines are separated by spaces — detected via Y-position change.
    pub fn concatenate_chunks(chunks: &[TextChunk]) -> String {
        let filtered: Vec<&TextChunk> = chunks
            .iter()
            .filter(|c| !c.value.is_empty() && !c.is_white_space_chunk())
            .collect();

        if filtered.len() < 2 {
            return Self::concatenate_chunk_refs(&filtered);
        }

        // Split into same-line groups based on Y position, then concatenate
        // each group with gap-based logic and join groups with spaces.
        let mut groups: Vec<Vec<&TextChunk>> = Vec::new();
        let mut current_group: Vec<&TextChunk> = vec![filtered[0]];

        for i in 1..filtered.len() {
            let prev = filtered[i - 1];
            let curr = filtered[i];
            let y_diff = (curr.bbox.top_y - prev.bbox.top_y).abs();
            let font_size = prev.font_size.max(curr.font_size).max(1.0);
            // If Y changes by more than half the font size, it's a new visual line.
            if y_diff > font_size * 0.5 {
                groups.push(std::mem::take(&mut current_group));
                current_group = vec![curr];
            } else {
                current_group.push(curr);
            }
        }
        groups.push(current_group);

        if groups.len() == 1 {
            return Self::concatenate_chunk_refs(&groups[0]);
        }

        // Concatenate each group separately and join with spaces.
        groups
            .iter()
            .map(|g| Self::concatenate_chunk_refs(g))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Core gap-based concatenation logic for a pre-ordered slice of chunk refs.
    fn concatenate_chunk_refs(real_chunks: &[&TextChunk]) -> String {
        if real_chunks.is_empty() {
            return String::new();
        }
        if real_chunks.len() == 1 {
            return Self::collapse_letter_spaced(&real_chunks[0].value);
        }

        // Detect letter-spaced lines: ≥70% of chunks are single characters
        // and there are at least 5 chunks.
        let adaptive_threshold = if real_chunks.len() >= 5 {
            let single_char_count = real_chunks
                .iter()
                .filter(|c| c.value.chars().count() == 1)
                .count();
            if single_char_count * 10 >= real_chunks.len() * 7 {
                // Compute median positive gap to determine the typical letter-spacing.
                let mut gaps: Vec<f64> = Vec::new();
                for i in 1..real_chunks.len() {
                    let gap = real_chunks[i].bbox.left_x - real_chunks[i - 1].bbox.right_x;
                    if gap > 0.0 {
                        gaps.push(gap);
                    }
                }
                if gaps.len() >= 3 {
                    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    let median = gaps[gaps.len() / 2];
                    Some(median * 1.8)
                } else {
                    // Too few gaps to compute median; fall back to collapsing all
                    Some(f64::MAX)
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut result = String::with_capacity(
            real_chunks.iter().map(|c| c.value.len()).sum::<usize>() + real_chunks.len(),
        );
        result.push_str(&real_chunks[0].value);

        for i in 1..real_chunks.len() {
            let prev = real_chunks[i - 1];
            let curr = real_chunks[i];

            if let Some(threshold) = adaptive_threshold {
                // For letter-spaced lines, only insert a space when the gap
                // is significantly larger than the typical letter spacing.
                let gap = curr.bbox.left_x - prev.bbox.right_x;
                if gap > threshold {
                    result.push(' ');
                }
            } else if Self::needs_space(prev, curr) {
                result.push(' ');
            }
            result.push_str(&curr.value);
        }
        result
    }

    /// Determine if a space is needed between two adjacent chunks.
    /// Uses `fontSize * 0.17` threshold (TEXT_LINE_SPACE_RATIO constant).
    fn needs_space(prev: &super::chunks::TextChunk, curr: &super::chunks::TextChunk) -> bool {
        // If either already has boundary whitespace, skip
        if prev.value.ends_with(' ') || curr.value.starts_with(' ') {
            return false;
        }
        // If either is empty, no space needed
        if prev.value.is_empty() || curr.value.is_empty() {
            return false;
        }

        let gap = curr.bbox.left_x - prev.bbox.right_x;

        // If overlapping or touching, no space
        if gap <= 0.0 {
            return false;
        }

        // TEXT_LINE_SPACE_RATIO = 0.17.  After the pre-merge step
        // (merge_close_text_chunks), adjacent same-style fragments with small
        // gaps have been unified.  Remaining gaps represent actual word
        // boundaries or style changes, so 0.17 works correctly on bounding-box
        // coordinates.
        let font_size = prev.font_size.max(curr.font_size).max(1.0);
        let threshold = font_size * 0.17;

        gap > threshold
    }

    /// Collapse letter-spaced text within a single string.
    ///
    /// Detects strings where ≥60% of space-separated tokens are single
    /// alphabetic characters (min 4). Consecutive single-char tokens are
    /// joined; double spaces and multi-char tokens act as word boundaries.
    fn collapse_letter_spaced(text: &str) -> String {
        let tokens: Vec<&str> = text.split(' ').collect();
        if tokens.len() < 5 {
            return text.to_string();
        }

        let non_empty: Vec<&str> = tokens.iter().copied().filter(|t| !t.is_empty()).collect();
        if non_empty.len() < 4 {
            return text.to_string();
        }

        let single_alpha = non_empty
            .iter()
            .filter(|t| {
                let mut chars = t.chars();
                matches!(chars.next(), Some(c) if c.is_alphabetic()) && chars.next().is_none()
            })
            .count();

        if single_alpha < 4 || single_alpha * 10 < non_empty.len() * 6 {
            return text.to_string();
        }

        let mut result = String::new();
        for token in &tokens {
            if token.is_empty() {
                // Double space → word boundary.
                if !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                continue;
            }
            let is_single_alpha = {
                let mut chars = token.chars();
                matches!(chars.next(), Some(c) if c.is_alphabetic()) && chars.next().is_none()
            };
            if is_single_alpha {
                result.push_str(token);
            } else {
                if !result.is_empty() && !result.ends_with(' ') {
                    result.push(' ');
                }
                result.push_str(token);
            }
        }
        result.trim().to_string()
    }

    /// Number of text chunks in this line.
    pub fn chunk_count(&self) -> usize {
        self.text_chunks.len()
    }
}

/// A vertical group of TextLines forming a text block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Dominant font size
    pub font_size: f64,
    /// Baseline Y coordinate
    pub base_line: f64,
    /// Slant degree
    pub slant_degree: f64,
    /// Whether all text is hidden
    pub is_hidden_text: bool,
    /// Component text lines
    pub text_lines: Vec<TextLine>,
    /// Whether block starts with a new paragraph
    pub has_start_line: bool,
    /// Whether block ends a paragraph
    pub has_end_line: bool,
    /// Detected text alignment
    pub text_alignment: Option<TextAlignment>,
}

impl TextBlock {
    /// Concatenated text value of all lines.
    ///
    /// Joins lines with spaces, handling end-of-line hyphenation by removing
    /// the trailing hyphen and joining the word directly.
    pub fn value(&self) -> String {
        let line_values: Vec<String> = self.text_lines.iter().map(|l| l.value()).collect();
        if line_values.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        for (i, line) in line_values.iter().enumerate() {
            let trimmed = line.trim_end();
            if i > 0 {
                // If the previous line ended with a hyphen, remove it and join directly
                if result.ends_with('-') {
                    // Check it's a real hyphenation (lowercase letter before hyphen)
                    let before_hyphen = result[..result.len() - 1].chars().last();
                    if before_hyphen.is_some_and(|c| c.is_alphabetic()) {
                        result.pop(); // Remove the hyphen
                                      // Don't add a space — the word continues
                    } else {
                        result.push(' ');
                    }
                } else {
                    result.push(' ');
                }
            }
            result.push_str(trimmed);
        }
        result
    }

    /// Total number of lines.
    pub fn lines_count(&self) -> usize {
        self.text_lines.len()
    }
}

/// A vertical group of TextBlocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColumn {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Dominant font size
    pub font_size: f64,
    /// Baseline Y coordinate
    pub base_line: f64,
    /// Slant degree
    pub slant_degree: f64,
    /// Whether all text is hidden
    pub is_hidden_text: bool,
    /// Component text blocks
    pub text_blocks: Vec<TextBlock>,
}

impl TextColumn {
    /// Concatenated text value of all blocks.
    pub fn value(&self) -> String {
        self.text_blocks
            .iter()
            .map(|b| b.value())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text_line(text: &str) -> TextLine {
        TextLine {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 12.0),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![TextChunk {
                value: text.to_string(),
                bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 12.0),
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
    fn test_text_line_value() {
        let line = make_text_line("Hello World");
        assert_eq!(line.value(), "Hello World");
        assert_eq!(line.chunk_count(), 1);
    }
}
