//! Table structures — TableBorder, TableBorderRow, TableBorderCell.

use serde::{Deserialize, Serialize};

use super::bbox::BoundingBox;
use super::chunks::TextChunk;
use super::content::ContentElement;
use super::enums::SemanticType;

/// Epsilon for table border coordinate comparisons.
pub const TABLE_BORDER_EPSILON: f64 = 0.5;

/// Minimum intersection for assigning content to cells.
pub const MIN_CELL_CONTENT_INTERSECTION_PERCENT: f64 = 0.01;

/// Grid-based table structure defined by row/column coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBorder {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// X-coordinates of column boundaries (N+1 for N columns)
    pub x_coordinates: Vec<f64>,
    /// Widths of column boundary lines
    pub x_widths: Vec<f64>,
    /// Y-coordinates of row boundaries (M+1 for M rows)
    pub y_coordinates: Vec<f64>,
    /// Widths of row boundary lines
    pub y_widths: Vec<f64>,
    /// Table rows
    pub rows: Vec<TableBorderRow>,
    /// Number of rows
    pub num_rows: usize,
    /// Number of columns
    pub num_columns: usize,
    /// Whether this table has structural problems
    pub is_bad_table: bool,
    /// Whether this came from a transformer model
    pub is_table_transformer: bool,
    /// Previous table in cross-page chain
    pub previous_table: Option<Box<TableBorder>>,
    /// Next table in cross-page chain
    pub next_table: Option<Box<TableBorder>>,
}

/// A row in a TableBorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBorderRow {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Row number (0-based)
    pub row_number: usize,
    /// Cells in this row
    pub cells: Vec<TableBorderCell>,
    /// Optional semantic type (header, body, footer)
    pub semantic_type: Option<SemanticType>,
}

/// A cell in a TableBorderRow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBorderCell {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Row number (0-based)
    pub row_number: usize,
    /// Column number (0-based)
    pub col_number: usize,
    /// Number of rows this cell spans
    pub row_span: usize,
    /// Number of columns this cell spans
    pub col_span: usize,
    /// Raw text content (table tokens)
    pub content: Vec<TableToken>,
    /// Processed content elements (after sub-pipeline)
    pub contents: Vec<ContentElement>,
    /// Optional semantic type
    pub semantic_type: Option<SemanticType>,
}

/// A text chunk assigned to a table cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableToken {
    /// Base text chunk
    pub base: TextChunk,
    /// Token type
    pub token_type: TableTokenType,
}

/// Type of content in a table cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableTokenType {
    /// Text content
    Text,
    /// Image content
    Image,
    /// Nested table
    Table,
}

/// Row of tokens in a table cell.
pub type TableTokenRow = Vec<TableToken>;

/// Collection of detected table borders, indexed by page.
#[derive(Debug, Clone, Default)]
pub struct TableBordersCollection {
    /// Per-page table borders
    pub table_borders: Vec<Vec<TableBorder>>,
}

impl TableBordersCollection {
    /// Create a new collection for the given number of pages.
    pub fn new(num_pages: usize) -> Self {
        Self {
            table_borders: vec![Vec::new(); num_pages],
        }
    }

    /// Add a table border to a page.
    pub fn add(&mut self, page: usize, border: TableBorder) {
        if page < self.table_borders.len() {
            self.table_borders[page].push(border);
        }
    }

    /// Get table borders for a page.
    pub fn get_page(&self, page: usize) -> &[TableBorder] {
        if page < self.table_borders.len() {
            &self.table_borders[page]
        } else {
            &[]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_borders_collection() {
        let mut collection = TableBordersCollection::new(5);
        let border = TableBorder {
            bbox: BoundingBox::new(Some(1), 10.0, 10.0, 200.0, 300.0),
            index: None,
            level: None,
            x_coordinates: vec![10.0, 100.0, 200.0],
            x_widths: vec![1.0, 1.0, 1.0],
            y_coordinates: vec![10.0, 150.0, 300.0],
            y_widths: vec![1.0, 1.0, 1.0],
            rows: vec![],
            num_rows: 2,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };
        collection.add(0, border);
        assert_eq!(collection.get_page(0).len(), 1);
        assert_eq!(collection.get_page(1).len(), 0);
        assert_eq!(collection.get_page(10).len(), 0);
    }
}
