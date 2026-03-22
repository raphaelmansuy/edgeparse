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
    pub fn value(&self) -> String {
        // Filter to non-whitespace, non-empty chunks (reference behaviour).
        let real_chunks: Vec<&TextChunk> = self
            .text_chunks
            .iter()
            .filter(|c| !c.value.is_empty() && !c.is_white_space_chunk())
            .collect();

        if real_chunks.is_empty() {
            return String::new();
        }
        if real_chunks.len() == 1 {
            return real_chunks[0].value.clone();
        }

        let mut result = String::with_capacity(
            real_chunks.iter().map(|c| c.value.len()).sum::<usize>()
                + real_chunks.len(),
        );
        result.push_str(&real_chunks[0].value);

        for i in 1..real_chunks.len() {
            let prev = real_chunks[i - 1];
            let curr = real_chunks[i];

            if Self::needs_space(prev, curr) {
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
                    if before_hyphen.map_or(false, |c| c.is_alphabetic()) {
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
