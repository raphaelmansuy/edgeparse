//! List structures — PDFList, ListItem.

use serde::{Deserialize, Serialize};

use super::bbox::BoundingBox;
use super::content::ContentElement;
use super::enums::SemanticType;
use super::table::TableTokenRow;

/// An ordered or unordered list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PDFList {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// List items
    pub list_items: Vec<ListItem>,
    /// Detected numbering style (e.g., "1.", "a)", "•")
    pub numbering_style: Option<String>,
    /// Common prefix across items
    pub common_prefix: Option<String>,
    /// Previous list ID for cross-page linking
    pub previous_list_id: Option<u64>,
    /// Next list ID for cross-page linking
    pub next_list_id: Option<u64>,
}

/// An entry in a PDFList.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Global index
    pub index: Option<u32>,
    /// Nesting level
    pub level: Option<String>,
    /// Label (bullet/number)
    pub label: ListLabel,
    /// Body content
    pub body: ListBody,
    /// Character length of the label
    pub label_length: usize,
    /// Processed content elements
    pub contents: Vec<ContentElement>,
    /// Optional semantic type
    pub semantic_type: Option<SemanticType>,
}

/// Label part of a list item (bullet, number, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListLabel {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Content rows
    pub content: Vec<TableTokenRow>,
    /// Optional semantic type
    pub semantic_type: Option<SemanticType>,
}

/// Body part of a list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListBody {
    /// Bounding box
    pub bbox: BoundingBox,
    /// Content rows
    pub content: Vec<TableTokenRow>,
    /// Optional semantic type
    pub semantic_type: Option<SemanticType>,
}

/// Information about a sequence of list items during detection.
#[derive(Debug, Clone)]
pub struct ListInterval {
    /// Indices of paragraphs that form list items
    pub list_indexes: Vec<usize>,
    /// Extracted info for each list item
    pub list_item_infos: Vec<ListItemInfo>,
    /// Detected numbering style
    pub numbering_style: Option<String>,
    /// Number of columns in multi-column lists
    pub number_of_columns: Option<usize>,
}

/// Information about a single list item during detection.
#[derive(Debug, Clone)]
pub struct ListItemInfo {
    /// Label text (e.g., "1.", "a)", "•")
    pub label_text: String,
    /// Numeric sequence value for ordering
    pub sequence_value: i64,
}
