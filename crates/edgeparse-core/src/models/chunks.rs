//! Chunk types — atomic units of extracted content.

use serde::{Deserialize, Serialize};

use super::bbox::{BoundingBox, Vertex};
use super::enums::{PdfLayer, TextFormat, TextType};

/// Atomic text fragment — one font run in the PDF content stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    /// Decoded Unicode text content
    pub value: String,
    /// Bounding box in page coordinates
    pub bbox: BoundingBox,
    /// Font name (base font name like "Helvetica")
    pub font_name: String,
    /// Font size in points (effective, after matrix transforms)
    pub font_size: f64,
    /// Font weight (100.0 - 900.0)
    pub font_weight: f64,
    /// Italic angle from font descriptor
    pub italic_angle: f64,
    /// Text color as hex string (e.g. "#000000")
    pub font_color: String,
    /// Contrast ratio against background (1.0-21.0)
    pub contrast_ratio: f64,
    /// X-coordinate of each glyph end position
    pub symbol_ends: Vec<f64>,
    /// Text baseline format (normal, superscript, subscript)
    pub text_format: TextFormat,
    /// Text type classification
    pub text_type: TextType,
    /// Processing layer that produced this chunk
    pub pdf_layer: PdfLayer,
    /// Whether the OCG (Optional Content Group) is visible
    pub ocg_visible: bool,
    /// Global index in extraction order
    pub index: Option<usize>,
    /// Page number (1-based)
    pub page_number: Option<u32>,
    /// Nesting level (from structure tree)
    pub level: Option<String>,
    /// Marked content identifier (from BDC/BMC operators in the content stream).
    /// Links this chunk to a structure tree node for semantic tagging.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcid: Option<i64>,
}

impl TextChunk {
    /// Whether the entire text value is whitespace.
    pub fn is_white_space_chunk(&self) -> bool {
        self.value.chars().all(|c| c.is_whitespace())
    }

    /// Collapse consecutive spaces into single space.
    pub fn compress_spaces(&mut self) {
        let mut result = String::with_capacity(self.value.len());
        let mut last_was_space = false;
        for ch in self.value.chars() {
            if ch == ' ' {
                if !last_was_space {
                    result.push(' ');
                }
                last_was_space = true;
            } else {
                result.push(ch);
                last_was_space = false;
            }
        }
        self.value = result;
    }

    /// Number of characters in the text.
    pub fn text_length(&self) -> usize {
        self.value.chars().count()
    }

    /// Average width per symbol.
    pub fn average_symbol_width(&self) -> f64 {
        let len = self.text_length();
        if len == 0 {
            return 0.0;
        }
        self.bbox.width() / len as f64
    }

    /// Get the X coordinate where the symbol at `idx` starts.
    pub fn symbol_start_coordinate(&self, idx: usize) -> f64 {
        if idx == 0 {
            self.bbox.left_x
        } else if idx <= self.symbol_ends.len() {
            self.symbol_ends[idx - 1]
        } else {
            self.bbox.right_x
        }
    }

    /// Get the X coordinate where the symbol at `idx` ends.
    pub fn symbol_end_coordinate(&self, idx: usize) -> f64 {
        if idx < self.symbol_ends.len() {
            self.symbol_ends[idx]
        } else {
            self.bbox.right_x
        }
    }
}

/// Image bounding box — actual pixel data extracted at output time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageChunk {
    /// Bounding box in page coordinates
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
}

/// Line segment — used for table border detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineChunk {
    /// Bounding box in page coordinates
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Start vertex
    pub start: Vertex,
    /// End vertex
    pub end: Vertex,
    /// Line width in points
    pub width: f64,
    /// Whether this is a horizontal line
    pub is_horizontal_line: bool,
    /// Whether this is a vertical line
    pub is_vertical_line: bool,
    /// Whether this is a square-like shape
    pub is_square: bool,
}

/// Vector graphic — collection of line segments forming bullets, decorations, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineArtChunk {
    /// Bounding box encompassing the line art
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Component line segments
    pub line_chunks: Vec<LineChunk>,
}

/// Size comparison tolerance for line art classification.
pub const LINE_ART_SIZE_EPSILON: f64 = 1.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_chunk(value: &str) -> TextChunk {
        TextChunk {
            value: value.to_string(),
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
        }
    }

    #[test]
    fn test_is_white_space_chunk() {
        assert!(make_text_chunk("   ").is_white_space_chunk());
        assert!(!make_text_chunk("hello").is_white_space_chunk());
        assert!(make_text_chunk("").is_white_space_chunk());
    }

    #[test]
    fn test_compress_spaces() {
        let mut chunk = make_text_chunk("hello   world   test");
        chunk.compress_spaces();
        assert_eq!(chunk.value, "hello world test");
    }

    #[test]
    fn test_text_length() {
        assert_eq!(make_text_chunk("hello").text_length(), 5);
        assert_eq!(make_text_chunk("").text_length(), 0);
    }

    #[test]
    fn test_average_symbol_width() {
        let chunk = make_text_chunk("hello");
        assert!((chunk.average_symbol_width() - 20.0).abs() < 0.01);
    }
}
