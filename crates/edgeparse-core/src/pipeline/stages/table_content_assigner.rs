//! Stage 4b: Table Content Assignment
//!
//! Assigns text and image content elements to the table cells they overlap with.
//! Runs after table border detection (Stage 3-4) and before text line grouping.

use crate::models::chunks::TextChunk;
use crate::models::content::ContentElement;
use crate::models::table::{TableBorder, TableToken, TableTokenType};

/// Minimum intersection fraction to assign an element to a table.
const MIN_TABLE_INTERSECTION: f64 = 0.6;

/// Minimum intersection fraction to assign content to a specific cell.
const MIN_CELL_INTERSECTION: f64 = 0.01;

/// Assign content elements to table cells based on bounding box overlap.
///
/// Elements that overlap a table by ≥60% are consumed and assigned to the
/// best-matching cell. The table border itself replaces the consumed elements.
pub fn assign_content_to_tables(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    // Separate tables from other content
    let mut tables: Vec<TableBorder> = Vec::new();
    let mut others: Vec<ContentElement> = Vec::new();

    for elem in elements {
        match elem {
            ContentElement::TableBorder(t) => tables.push(t),
            _ => others.push(elem),
        }
    }

    if tables.is_empty() {
        return others;
    }

    // Try to assign each non-table element to a table cell
    let mut remaining: Vec<ContentElement> = Vec::new();

    for elem in others {
        let elem_bbox = elem.bbox();

        // Find the first table that overlaps enough
        // We want: intersection / elem_area >= threshold
        // intersection_percent(other) = intersection / other.area
        // So table.bbox.intersection_percent(elem_bbox) = intersection / elem_area
        let mut assigned = false;
        for table in &mut tables {
            let overlap = table.bbox.intersection_percent(elem_bbox);
            if overlap >= MIN_TABLE_INTERSECTION && assign_to_cell(table, &elem) {
                assigned = true;
                break;
            }
        }

        if !assigned {
            remaining.push(elem);
        }
    }

    // Re-emit tables (now with content) and remaining elements
    let mut result = remaining;
    for table in tables {
        result.push(ContentElement::TableBorder(table));
    }
    result
}

/// Assign a content element to the best-matching cell in a table.
fn assign_to_cell(table: &mut TableBorder, elem: &ContentElement) -> bool {
    let elem_bbox = elem.bbox();

    let mut best_overlap = 0.0_f64;
    let mut best_row = 0;
    let mut best_col = 0;

    for row in &table.rows {
        for cell in &row.cells {
            // intersection / elem_area — how much of the element is in this cell
            let overlap = cell.bbox.intersection_percent(elem_bbox);
            if overlap > best_overlap {
                best_overlap = overlap;
                best_row = cell.row_number;
                best_col = cell.col_number;
            }
        }
    }

    if best_overlap < MIN_CELL_INTERSECTION {
        return false;
    }

    if let Some(row) = table.rows.get_mut(best_row) {
        if let Some(cell) = row.cells.get_mut(best_col) {
            // Add as TableToken if it's a text chunk
            match elem {
                ContentElement::TextChunk(tc) => {
                    cell.content.push(TableToken {
                        base: tc.clone(),
                        token_type: TableTokenType::Text,
                    });
                }
                ContentElement::Image(_) => {
                    // Raster-recovered OCR tables already contain the decoded text.
                    // Re-attaching the full image as a table token pollutes markdown
                    // with "[image]" while adding no extra signal.
                    if table.is_table_transformer {
                        return true;
                    }
                    let dummy = TextChunk {
                        value: "[image]".to_string(),
                        bbox: elem_bbox.clone(),
                        index: None,
                        level: None,
                        mcid: None,
                        font_size: 0.0,
                        font_weight: 0.0,
                        font_name: String::new(),
                        italic_angle: 0.0,
                        font_color: String::new(),
                        contrast_ratio: 0.0,
                        symbol_ends: Vec::new(),
                        text_format: crate::models::enums::TextFormat::Normal,
                        text_type: crate::models::enums::TextType::Regular,
                        pdf_layer: crate::models::enums::PdfLayer::Main,
                        ocg_visible: true,
                        page_number: elem_bbox.page_number,
                    };
                    cell.content.push(TableToken {
                        base: dummy,
                        token_type: TableTokenType::Image,
                    });
                }
                _ => {
                    // Other element types are stored in contents
                    cell.contents.push(elem.clone());
                }
            }
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::TextType;
    use crate::models::table::{TableBorder, TableBorderCell, TableBorderRow};

    fn make_text_chunk(
        value: &str,
        left: f64,
        bottom: f64,
        right: f64,
        top: f64,
    ) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: value.to_string(),
            bbox: BoundingBox::new(Some(1), left, bottom, right, top),
            index: None,
            level: None,
            mcid: None,
            font_size: 12.0,
            font_weight: 400.0,
            font_name: "Arial".to_string(),
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: crate::models::enums::TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: crate::models::enums::PdfLayer::Main,
            ocg_visible: true,
            page_number: Some(1),
        })
    }

    fn make_table_2x2() -> TableBorder {
        // 2x2 table from (100,660) to (300,700)
        let x_coords = vec![100.0, 200.0, 300.0];
        let y_coords = vec![700.0, 680.0, 660.0]; // top to bottom

        let mut rows = Vec::new();
        for r in 0..2 {
            let mut cells = Vec::new();
            for c in 0..2 {
                cells.push(TableBorderCell {
                    bbox: BoundingBox::new(
                        Some(1),
                        x_coords[c],
                        y_coords[r + 1],
                        x_coords[c + 1],
                        y_coords[r],
                    ),
                    index: None,
                    level: None,
                    row_number: r,
                    col_number: c,
                    row_span: 1,
                    col_span: 1,
                    content: Vec::new(),
                    contents: Vec::new(),
                    semantic_type: None,
                });
            }
            rows.push(TableBorderRow {
                bbox: BoundingBox::new(Some(1), 100.0, y_coords[r + 1], 300.0, y_coords[r]),
                index: None,
                level: None,
                row_number: r,
                cells,
                semantic_type: None,
            });
        }

        TableBorder {
            bbox: BoundingBox::new(Some(1), 100.0, 660.0, 300.0, 700.0),
            index: None,
            level: None,
            x_coordinates: x_coords,
            x_widths: vec![0.5; 3],
            y_coordinates: y_coords,
            y_widths: vec![0.5; 3],
            rows,
            num_rows: 2,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        }
    }

    fn make_dense_table(num_rows: usize, num_cols: usize) -> TableBorder {
        let left = 100.0;
        let right = 300.0;
        let bottom = 660.0;
        let top = 700.0;
        let col_width = (right - left) / num_cols as f64;
        let row_height = (top - bottom) / num_rows as f64;

        let x_coords: Vec<f64> = (0..=num_cols)
            .map(|i| left + i as f64 * col_width)
            .collect();
        let y_coords: Vec<f64> = (0..=num_rows)
            .map(|i| top - i as f64 * row_height)
            .collect();

        let mut rows = Vec::new();
        for r in 0..num_rows {
            let mut cells = Vec::new();
            for c in 0..num_cols {
                cells.push(TableBorderCell {
                    bbox: BoundingBox::new(
                        Some(1),
                        x_coords[c],
                        y_coords[r + 1],
                        x_coords[c + 1],
                        y_coords[r],
                    ),
                    index: None,
                    level: None,
                    row_number: r,
                    col_number: c,
                    row_span: 1,
                    col_span: 1,
                    content: Vec::new(),
                    contents: Vec::new(),
                    semantic_type: None,
                });
            }
            rows.push(TableBorderRow {
                bbox: BoundingBox::new(Some(1), left, y_coords[r + 1], right, y_coords[r]),
                index: None,
                level: None,
                row_number: r,
                cells,
                semantic_type: None,
            });
        }

        TableBorder {
            bbox: BoundingBox::new(Some(1), left, bottom, right, top),
            index: None,
            level: None,
            x_coordinates: x_coords,
            x_widths: vec![0.5; num_cols + 1],
            y_coordinates: y_coords,
            y_widths: vec![0.5; num_rows + 1],
            rows,
            num_rows,
            num_columns: num_cols,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        }
    }

    #[test]
    fn test_assign_text_to_cell() {
        // Text chunk fully inside cell (0,0): x=[100,200], y=[680,700]
        let text = make_text_chunk("Hello", 110.0, 685.0, 190.0, 695.0);
        let table = ContentElement::TableBorder(make_table_2x2());

        let elements = vec![text, table];
        let result = assign_content_to_tables(elements);

        let tables: Vec<_> = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .collect();
        assert_eq!(tables.len(), 1);
        if let ContentElement::TableBorder(t) = &tables[0] {
            assert_eq!(t.rows[0].cells[0].content.len(), 1);
            assert_eq!(t.rows[0].cells[0].content[0].base.value, "Hello");
        }
    }

    #[test]
    fn test_text_outside_table_not_consumed() {
        // Text chunk far away from table
        let text = make_text_chunk("Outside", 500.0, 500.0, 600.0, 520.0);
        let table = ContentElement::TableBorder(make_table_2x2());

        let elements = vec![text, table];
        let result = assign_content_to_tables(elements);

        // Text should remain in output
        let texts: Vec<_> = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TextChunk(_)))
            .collect();
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn test_assign_to_correct_cell() {
        // Text in cell (1,1): x=[200,300], y=[660,680]
        let text = make_text_chunk("BottomRight", 210.0, 665.0, 290.0, 675.0);
        let table = ContentElement::TableBorder(make_table_2x2());

        let elements = vec![text, table];
        let result = assign_content_to_tables(elements);

        let tables: Vec<_> = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .collect();
        if let ContentElement::TableBorder(t) = &tables[0] {
            // Cell (0,0) should be empty
            assert!(t.rows[0].cells[0].content.is_empty());
            // Cell (1,1) should have the text
            assert_eq!(t.rows[1].cells[1].content.len(), 1);
            assert_eq!(t.rows[1].cells[1].content[0].base.value, "BottomRight");
        }
    }

    #[test]
    fn test_no_tables_passthrough() {
        let text = make_text_chunk("Hello", 100.0, 100.0, 200.0, 120.0);
        let result = assign_content_to_tables(vec![text]);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_unassignable_text_is_not_dropped() {
        let text = make_text_chunk("Across rows", 100.0, 660.0, 300.0, 700.0);
        let table = ContentElement::TableBorder(make_dense_table(11, 11));

        let result = assign_content_to_tables(vec![text, table]);

        let text_count = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TextChunk(_)))
            .count();
        assert_eq!(text_count, 1);
    }
}
