//! Stage 4c: Boxed Heading Promoter
//!
//! After table content assignment, some decorative bordered boxes wrap section
//! headings (e.g., "2. General Profile of MSMEs" inside a rectangle). These
//! are mis-classified as single-cell tables. This stage detects such cases and
//! releases the cell content back as free TextChunk elements so they can be
//! properly classified as headings by the heading detector.
//!
//! Criteria for a "boxed heading":
//! - TableBorder with exactly 1 row and 1 cell
//! - Cell content is short text (< 150 chars, < 3 logical lines of text tokens)
//! - The table bbox height is not extreme (< 200 pts — not a full-page frame)
//! - No image content in the cell

use crate::models::content::ContentElement;

/// Maximum text length (chars) for a single-cell table to be treated as a boxed heading.
const MAX_TEXT_LEN: usize = 150;

/// Maximum number of text tokens in a single-cell table for it to be treated as a heading.
const MAX_TOKEN_COUNT: usize = 8;

/// Maximum height (pts) of a single-cell table to be considered a decorative box.
const MAX_BOX_HEIGHT: f64 = 200.0;

/// Promote single-cell tables with short text back to free text chunks.
///
/// This allows the heading detector to see the content rather than treating it
/// as table data.
pub fn promote_boxed_headings(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());

    for elem in elements {
        match elem {
            ContentElement::TableBorder(ref table) => {
                // Case 1: Single-row, single-cell tables (decorative bordered boxes)
                if table.rows.len() == 1 {
                    let row = &table.rows[0];
                    if row.cells.len() == 1 {
                        let cell = &row.cells[0];

                        // Check box height
                        let box_height = table.bbox.top_y - table.bbox.bottom_y;
                        if box_height > MAX_BOX_HEIGHT {
                            result.push(elem);
                            continue;
                        }

                        // Count tokens and total text length
                        let token_count = cell.content.len();
                        let total_text: String = cell
                            .content
                            .iter()
                            .map(|t| t.base.value.as_str())
                            .collect::<Vec<_>>()
                            .join(" ");
                        let text_len = total_text.trim().len();

                        // Check for image content (skip promotion if there's an image)
                        let has_image = cell.content.iter().any(|t| t.base.value == "[image]");

                        if !has_image
                            && token_count <= MAX_TOKEN_COUNT
                            && text_len > 0
                            && text_len <= MAX_TEXT_LEN
                        {
                            // Release as free text chunks
                            for token in &cell.content {
                                result.push(ContentElement::TextChunk(token.base.clone()));
                            }
                            // Also promote sub-contents if any (e.g., headings inside)
                            for sub in &cell.contents {
                                result.push(sub.clone());
                            }
                            continue;
                        }
                    }
                }

                result.push(elem);
            }
            _ => result.push(elem),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::table::{
        TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType,
    };

    fn make_token(text: &str) -> TableToken {
        TableToken {
            base: TextChunk {
                value: text.to_string(),
                bbox: BoundingBox::new(Some(1), 100.0, 700.0, 400.0, 720.0),
                index: None,
                level: None,
                mcid: None,
                font_size: 14.0,
                font_weight: 700.0,
                font_name: "Arial-Bold".into(),
                italic_angle: 0.0,
                font_color: "#000000".into(),
                contrast_ratio: 21.0,
                symbol_ends: vec![],
                text_format: TextFormat::Normal,
                text_type: TextType::Regular,
                pdf_layer: PdfLayer::Main,
                ocg_visible: true,
                page_number: Some(1),
            },
            token_type: TableTokenType::Text,
        }
    }

    fn make_single_cell_table(text: &str) -> ContentElement {
        let token = make_token(text);
        let cell = TableBorderCell {
            bbox: BoundingBox::new(Some(1), 50.0, 690.0, 500.0, 730.0),
            index: None,
            level: None,
            row_number: 0,
            col_number: 0,
            row_span: 1,
            col_span: 1,
            content: vec![token],
            contents: vec![],
            semantic_type: None,
        };
        let row = TableBorderRow {
            bbox: BoundingBox::new(Some(1), 50.0, 690.0, 500.0, 730.0),
            cells: vec![cell],
            index: None,
            level: None,
            row_number: 0,
            semantic_type: None,
        };
        ContentElement::TableBorder(TableBorder {
            bbox: BoundingBox::new(Some(1), 50.0, 690.0, 500.0, 730.0),
            rows: vec![row],
            num_columns: 1,
            num_rows: 1,
            index: None,
            level: None,
            x_coordinates: vec![50.0, 500.0],
            x_widths: vec![1.0, 1.0],
            y_coordinates: vec![690.0, 730.0],
            y_widths: vec![1.0, 1.0],
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        })
    }

    #[test]
    fn test_single_cell_short_text_promoted() {
        let elem = make_single_cell_table("2. General Profile of MSMEs");
        let result = promote_boxed_headings(vec![elem]);
        // Should produce a TextChunk, not a TableBorder
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::TextChunk(_)));
    }

    #[test]
    fn test_multi_row_table_preserved() {
        let elem = make_single_cell_table("data");
        // Create multi-row table instead
        if let ContentElement::TableBorder(mut table) = elem {
            // Add a second row
            table.rows.push(table.rows[0].clone());
            let multi = ContentElement::TableBorder(table);
            let result = promote_boxed_headings(vec![multi]);
            assert_eq!(result.len(), 1);
            assert!(matches!(result[0], ContentElement::TableBorder(_)));
        }
    }

    #[test]
    fn test_long_text_preserved_as_table() {
        let long_text = "x".repeat(200);
        let elem = make_single_cell_table(&long_text);
        let result = promote_boxed_headings(vec![elem]);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::TableBorder(_)));
    }
}
