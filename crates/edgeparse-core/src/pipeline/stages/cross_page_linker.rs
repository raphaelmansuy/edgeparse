//! Stage 15: Cross-Page Table Linking
//!
//! Links tables that span across page boundaries by matching column structure.
//! A table at the bottom of one page is linked to a matching table at the top
//! of the next page.

use crate::models::content::ContentElement;
use crate::models::table::TableBorder;

/// Width tolerance for matching tables across pages.
const WIDTH_TOLERANCE: f64 = 5.0;

/// Column count must match exactly for cross-page linking.
/// Width difference must be within tolerance.
/// Link tables across sequential pages.
pub fn link_cross_page_tables(pages: &mut [Vec<ContentElement>]) {
    if pages.len() < 2 {
        return;
    }

    for i in 0..pages.len() - 1 {
        // Find last table on current page
        let last_table_idx = pages[i]
            .iter()
            .rposition(|e| matches!(e, ContentElement::TableBorder(_)));

        // Find first table on next page
        let first_table_idx = pages[i + 1]
            .iter()
            .position(|e| matches!(e, ContentElement::TableBorder(_)));

        if let (Some(li), Some(fi)) = (last_table_idx, first_table_idx) {
            if tables_match(&pages[i][li], &pages[i + 1][fi]) {
                // Clone and link them
                let next_clone = extract_table(&pages[i + 1][fi]);
                let prev_clone = extract_table(&pages[i][li]);

                if let ContentElement::TableBorder(ref mut t) = pages[i][li] {
                    t.next_table = Some(Box::new(next_clone));
                }
                if let ContentElement::TableBorder(ref mut t) = pages[i + 1][fi] {
                    t.previous_table = Some(Box::new(prev_clone));
                }
            }
        }
    }
}

/// Check if two elements are matching tables (same column count, similar width).
fn tables_match(a: &ContentElement, b: &ContentElement) -> bool {
    if let (ContentElement::TableBorder(ta), ContentElement::TableBorder(tb)) = (a, b) {
        ta.num_columns == tb.num_columns
            && ta.num_columns > 0
            && (ta.bbox.width() - tb.bbox.width()).abs() < WIDTH_TOLERANCE
    } else {
        false
    }
}

/// Extract a TableBorder from a ContentElement (without linked tables to avoid infinite nesting).
fn extract_table(elem: &ContentElement) -> TableBorder {
    if let ContentElement::TableBorder(t) = elem {
        TableBorder {
            previous_table: None,
            next_table: None,
            ..t.clone()
        }
    } else {
        unreachable!("expected TableBorder")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::table::{TableBorder, TableBorderCell, TableBorderRow};

    fn make_table(page: u32, num_cols: usize, width: f64, y_top: f64) -> ContentElement {
        let mut rows = vec![TableBorderRow {
            bbox: BoundingBox::new(Some(page), 72.0, y_top - 20.0, 72.0 + width, y_top),
            index: None,
            level: None,
            row_number: 0,
            cells: (0..num_cols)
                .map(|c| TableBorderCell {
                    bbox: BoundingBox::new(
                        Some(page),
                        72.0 + c as f64 * (width / num_cols as f64),
                        y_top - 20.0,
                        72.0 + (c + 1) as f64 * (width / num_cols as f64),
                        y_top,
                    ),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: c,
                    row_span: 1,
                    col_span: 1,
                    content: Vec::new(),
                    contents: Vec::new(),
                    semantic_type: None,
                })
                .collect(),
            semantic_type: None,
        }];
        ContentElement::TableBorder(TableBorder {
            bbox: BoundingBox::new(Some(page), 72.0, y_top - 20.0, 72.0 + width, y_top),
            index: None,
            level: None,
            x_coordinates: (0..=num_cols)
                .map(|c| 72.0 + c as f64 * (width / num_cols as f64))
                .collect(),
            x_widths: vec![0.5; num_cols + 1],
            y_coordinates: vec![y_top, y_top - 20.0],
            y_widths: vec![0.5; 2],
            rows,
            num_rows: 1,
            num_columns: num_cols,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        })
    }

    #[test]
    fn test_link_matching_tables() {
        let t1 = make_table(1, 3, 400.0, 100.0); // bottom of page 1
        let t2 = make_table(2, 3, 400.0, 700.0); // top of page 2
        let mut pages = vec![vec![t1], vec![t2]];

        link_cross_page_tables(&mut pages);

        if let ContentElement::TableBorder(t) = &pages[0][0] {
            assert!(t.next_table.is_some());
        }
        if let ContentElement::TableBorder(t) = &pages[1][0] {
            assert!(t.previous_table.is_some());
        }
    }

    #[test]
    fn test_no_link_different_column_count() {
        let t1 = make_table(1, 3, 400.0, 100.0);
        let t2 = make_table(2, 5, 400.0, 700.0);
        let mut pages = vec![vec![t1], vec![t2]];

        link_cross_page_tables(&mut pages);

        if let ContentElement::TableBorder(t) = &pages[0][0] {
            assert!(t.next_table.is_none());
        }
    }

    #[test]
    fn test_no_tables_no_crash() {
        let mut pages: Vec<Vec<ContentElement>> = vec![vec![], vec![]];
        link_cross_page_tables(&mut pages);
    }
}
