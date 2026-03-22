//! Semantic node types — SemanticParagraph, SemanticHeading, etc.

use serde::{Deserialize, Serialize};

use super::bbox::BoundingBox;
use super::chunks::ImageChunk;
use super::chunks::LineArtChunk;
use super::content::ContentElement;
use super::enums::{SemanticType, TextFormat};
use super::table::TableBorder;
use super::text::TextColumn;

/// Base for all text-bearing semantic elements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTextNode {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Semantic classification
    pub semantic_type: SemanticType,
    /// Confidence score for semantic classification
    pub correct_semantic_score: Option<f64>,
    /// Text columns
    pub columns: Vec<TextColumn>,
    /// Dominant font weight
    pub font_weight: Option<f64>,
    /// Dominant font size
    pub font_size: Option<f64>,
    /// Dominant text color — original PDF color components (1=Gray, 3=RGB, 4=CMYK)
    pub text_color: Option<Vec<f64>>,
    /// Italic angle
    pub italic_angle: Option<f64>,
    /// Font name
    pub font_name: Option<String>,
    /// Text format
    pub text_format: Option<TextFormat>,
    /// Maximum font size in this node
    pub max_font_size: Option<f64>,
    /// Background color — original PDF color components (1=Gray, 3=RGB, 4=CMYK)
    pub background_color: Option<Vec<f64>>,
    /// Whether all text is hidden
    pub is_hidden_text: bool,
}

impl SemanticTextNode {
    /// Concatenated text value of all columns.
    pub fn value(&self) -> String {
        self.columns
            .iter()
            .map(|c| c.value())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Number of text lines across all columns.
    pub fn lines_number(&self) -> usize {
        self.columns
            .iter()
            .flat_map(|c| &c.text_blocks)
            .map(|b| b.text_lines.len())
            .sum()
    }

    /// Number of columns.
    pub fn columns_number(&self) -> usize {
        self.columns.len()
    }

    /// Whether this node contains no text.
    pub fn is_empty(&self) -> bool {
        self.value().trim().is_empty()
    }

    /// Whether this node contains only whitespace.
    pub fn is_space_node(&self) -> bool {
        self.value().chars().all(|c| c.is_whitespace())
    }

    /// Whether the text starts with an Arabic (decimal) number.
    pub fn starts_with_arabic_number(&self) -> bool {
        let text = self.value();
        let trimmed = text.trim_start();
        trimmed.starts_with(|c: char| c.is_ascii_digit())
    }
}

/// A semantic paragraph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticParagraph {
    /// Base text node
    pub base: SemanticTextNode,
    /// Whether enclosed at top
    pub enclosed_top: bool,
    /// Whether enclosed at bottom
    pub enclosed_bottom: bool,
    /// Indentation level
    pub indentation: i32,
}

/// A semantic heading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHeading {
    /// Base paragraph
    pub base: SemanticParagraph,
    /// Heading level (1-6, None if not yet assigned)
    pub heading_level: Option<u32>,
}

/// A numbered heading (e.g., "1.2.3 Budget Overview").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNumberHeading {
    /// Base heading
    pub base: SemanticHeading,
}

/// A caption linked to an image or table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticCaption {
    /// Base text node
    pub base: SemanticTextNode,
    /// ID of the linked content (image or table)
    pub linked_content_id: Option<u64>,
}

/// Page header or footer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticHeaderOrFooter {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Header or Footer
    pub semantic_type: SemanticType,
    /// Nested content elements
    pub contents: Vec<ContentElement>,
}

/// A figure containing images and/or line art.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFigure {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Semantic type
    pub semantic_type: SemanticType,
    /// Image chunks
    pub images: Vec<ImageChunk>,
    /// Line art chunks
    pub line_arts: Vec<LineArtChunk>,
}

/// A semantic table wrapping a TableBorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTable {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Semantic type
    pub semantic_type: SemanticType,
    /// Table border structure
    pub table_border: TableBorder,
}

/// A LaTeX formula (from enrichment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFormula {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// LaTeX representation
    pub latex: String,
}

/// A described image (from enrichment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticPicture {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Image index
    pub image_index: u32,
    /// Human-readable description
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_empty_text_node() -> SemanticTextNode {
        SemanticTextNode {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 12.0),
            index: None,
            level: None,
            semantic_type: SemanticType::Paragraph,
            correct_semantic_score: None,
            columns: vec![],
            font_weight: None,
            font_size: None,
            text_color: None,
            italic_angle: None,
            font_name: None,
            text_format: None,
            max_font_size: None,
            background_color: None,
            is_hidden_text: false,
        }
    }

    #[test]
    fn test_empty_text_node() {
        let node = make_empty_text_node();
        assert!(node.is_empty());
        assert!(node.is_space_node());
        assert_eq!(node.lines_number(), 0);
        assert_eq!(node.columns_number(), 0);
    }
}
