//! Stage 3: Table Border Detection
//!
//! Detects tables by finding intersecting horizontal and vertical line segments,
//! grouping them into table borders, and constructing a cell grid.

use crate::models::bbox::BoundingBox;
use crate::models::chunks::LineChunk;
use crate::models::content::ContentElement;
use crate::models::table::{TableBorder, TableBorderCell, TableBorderRow};
use crate::pipeline::stages::text_line_grouper;

/// Maximum line width to consider for table borders.
const MAX_LINE_WIDTH: f64 = 5.0;

/// Intersection tolerance for line segments.
const LINE_EPSILON: f64 = 0.5;

/// Minimum vertexes required for a valid table.
const MIN_VERTEXES: usize = 3;

/// Maximum fraction of the page area that a single bordered table may cover.
/// Tables covering more than this are likely layout artifacts (sidebars,
/// decorative borders) rather than data tables and should be rejected.
const MAX_TABLE_PAGE_COVERAGE: f64 = 0.90;

/// Standard A4 page area (595 × 842 pt). Used as fallback when page dimensions
/// are not available.
const DEFAULT_PAGE_AREA: f64 = 595.0 * 842.0;

/// Minimum column width (in points) to keep a physical column alive.
/// Columns narrower than this are "gutter" separators and should be merged
/// with the adjacent column to the right.
const MIN_COLUMN_WIDTH: f64 = 8.0;

/// Detect table borders from line segments on a single page.
pub fn detect_table_borders(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    // Separate lines from other content
    let mut h_lines: Vec<LineChunk> = Vec::new();
    let mut v_lines: Vec<LineChunk> = Vec::new();
    let mut other: Vec<ContentElement> = Vec::new();

    for elem in elements {
        match elem {
            ContentElement::Line(ref line) => {
                if line_width(line) <= MAX_LINE_WIDTH {
                    if is_horizontal(line) {
                        h_lines.push(line.clone());
                    } else if is_vertical(line) {
                        v_lines.push(line.clone());
                    } else {
                        other.push(elem);
                    }
                } else {
                    other.push(elem);
                }
            }
            _ => other.push(elem),
        }
    }

    if h_lines.is_empty() || v_lines.is_empty() {
        // No possible tables — return everything
        other.extend(h_lines.into_iter().map(ContentElement::Line));
        other.extend(v_lines.into_iter().map(ContentElement::Line));
        return other;
    }

    // Find table borders by grouping intersecting lines
    let borders = build_table_borders(&h_lines, &v_lines);

    // Collect lines consumed by tables
    let mut consumed_h: Vec<bool> = vec![false; h_lines.len()];
    let mut consumed_v: Vec<bool> = vec![false; v_lines.len()];

    let mut result = other;

    for border in borders {
        // Mark consumed lines
        for (i, hl) in h_lines.iter().enumerate() {
            if line_in_bbox(hl, &border.bbox) {
                consumed_h[i] = true;
            }
        }
        for (i, vl) in v_lines.iter().enumerate() {
            if line_in_bbox(vl, &border.bbox) {
                consumed_v[i] = true;
            }
        }
        result.push(ContentElement::TableBorder(border));
    }

    // Put back unconsumed lines
    for (i, hl) in h_lines.into_iter().enumerate() {
        if !consumed_h[i] {
            result.push(ContentElement::Line(hl));
        }
    }
    for (i, vl) in v_lines.into_iter().enumerate() {
        if !consumed_v[i] {
            result.push(ContentElement::Line(vl));
        }
    }

    result
}

/// Build table borders from intersecting horizontal and vertical lines.
fn build_table_borders(h_lines: &[LineChunk], v_lines: &[LineChunk]) -> Vec<TableBorder> {
    // Find all intersection vertices
    let mut groups: Vec<TableGroup> = Vec::new();

    for hl in h_lines {
        for vl in v_lines {
            if lines_intersect(hl, vl) {
                // Find existing group that contains either line
                let mut found = None;
                for (gi, group) in groups.iter_mut().enumerate() {
                    if group.contains_h(hl) || group.contains_v(vl) {
                        group.add_h(hl.clone());
                        group.add_v(vl.clone());
                        found = Some(gi);
                        break;
                    }
                }
                if found.is_none() {
                    let mut group = TableGroup::new();
                    group.add_h(hl.clone());
                    group.add_v(vl.clone());
                    groups.push(group);
                }
            }
        }
    }

    // Merge groups that share lines
    merge_groups(&mut groups);

    // Convert valid groups to TableBorders, then merge gutter columns and
    // reject full-page layout artifacts.
    groups
        .into_iter()
        .filter(|g| {
            g.vertex_count() >= MIN_VERTEXES && !g.h_lines.is_empty() && !g.v_lines.is_empty()
        })
        .filter_map(|g| g.to_table_border())
        .map(merge_gutter_columns)
        .filter(|tb| {
            // Reject tables that cover the full page — these are almost always
            // decorative borders / sidebar layouts, not data tables.
            let area = tb.bbox.width().max(0.0) * tb.bbox.height().max(0.0);
            area / DEFAULT_PAGE_AREA <= MAX_TABLE_PAGE_COVERAGE
        })
        .collect()
}

/// A group of lines being assembled into a table border.
struct TableGroup {
    h_lines: Vec<LineChunk>,
    v_lines: Vec<LineChunk>,
}

impl TableGroup {
    fn new() -> Self {
        Self {
            h_lines: Vec::new(),
            v_lines: Vec::new(),
        }
    }

    fn contains_h(&self, line: &LineChunk) -> bool {
        self.h_lines.iter().any(|l| lines_same(l, line))
    }

    fn contains_v(&self, line: &LineChunk) -> bool {
        self.v_lines.iter().any(|l| lines_same(l, line))
    }

    fn add_h(&mut self, line: LineChunk) {
        if !self.contains_h(&line) {
            self.h_lines.push(line);
        }
    }

    fn add_v(&mut self, line: LineChunk) {
        if !self.contains_v(&line) {
            self.v_lines.push(line);
        }
    }

    fn vertex_count(&self) -> usize {
        let mut count = 0;
        for hl in &self.h_lines {
            for vl in &self.v_lines {
                if lines_intersect(hl, vl) {
                    count += 1;
                }
            }
        }
        count
    }

    fn bbox(&self) -> BoundingBox {
        let mut bbox = BoundingBox::new(None, f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for l in self.h_lines.iter().chain(self.v_lines.iter()) {
            bbox = bbox.union(&l.bbox);
        }
        bbox
    }

    fn to_table_border(&self) -> Option<TableBorder> {
        // Collect all X coordinates from vertical lines
        let mut x_coords: Vec<f64> = Vec::new();
        for vl in &self.v_lines {
            let x = vl.bbox.center_x();
            if !x_coords.iter().any(|&c| (c - x).abs() < LINE_EPSILON) {
                x_coords.push(x);
            }
        }
        x_coords.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Collect all Y coordinates from horizontal lines
        let mut y_coords: Vec<f64> = Vec::new();
        for hl in &self.h_lines {
            let y = hl.bbox.center_y();
            if !y_coords.iter().any(|&c| (c - y).abs() < LINE_EPSILON) {
                y_coords.push(y);
            }
        }
        // Sort Y descending (top-to-bottom in PDF)
        y_coords.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        let num_cols = if x_coords.len() > 1 {
            x_coords.len() - 1
        } else {
            return None;
        };
        let num_rows = if y_coords.len() > 1 {
            y_coords.len() - 1
        } else {
            return None;
        };

        // Build cell grid
        let mut rows = Vec::with_capacity(num_rows);
        for r in 0..num_rows {
            let mut cells = Vec::with_capacity(num_cols);
            for c in 0..num_cols {
                let cell_bbox = BoundingBox::new(
                    self.bbox().page_number,
                    x_coords[c],
                    y_coords[r + 1], // bottom (lower Y)
                    x_coords[c + 1],
                    y_coords[r], // top (higher Y)
                );
                cells.push(TableBorderCell {
                    bbox: cell_bbox,
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
            let row_bbox = BoundingBox::new(
                self.bbox().page_number,
                x_coords[0],
                y_coords[r + 1],
                *x_coords.last().unwrap_or(&0.0),
                y_coords[r],
            );
            rows.push(TableBorderRow {
                bbox: row_bbox,
                index: None,
                level: None,
                row_number: r,
                cells,
                semantic_type: None,
            });
        }

        let bbox = self.bbox();
        Some(TableBorder {
            bbox,
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
        })
    }
}

/// Merge narrow "gutter" columns into their right neighbor.
///
/// PDF tables often have decorative separator lines between logical columns,
/// creating very narrow physical columns (1-5pt wide) that fragment cell text.
/// This function removes those gutter columns by merging adjacent cells,
/// producing a table with the correct logical column count.
fn merge_gutter_columns(mut table: TableBorder) -> TableBorder {
    if table.x_coordinates.len() < 3 {
        return table; // need at least 2 columns to merge
    }

    // Identify which physical columns are narrow gutters
    let num_cols = table.num_columns;
    let mut keep_col = vec![true; num_cols];
    for (c, kept) in keep_col.iter_mut().enumerate() {
        let width = table.x_coordinates[c + 1] - table.x_coordinates[c];
        if width < MIN_COLUMN_WIDTH {
            *kept = false;
        }
    }

    // If no gutters found, return unchanged
    if keep_col.iter().all(|&k| k) {
        return table;
    }

    // Build mapping: old_col → new_col (gutters share the previous kept column)
    let mut new_col_idx = Vec::with_capacity(num_cols);
    let mut new_idx: usize = 0;
    for &kept in keep_col.iter() {
        if kept {
            new_col_idx.push(new_idx);
            new_idx += 1;
        } else {
            // Merge this gutter column with the previous kept column if possible,
            // otherwise with the next kept column (for leading gutters).
            let target = if new_idx > 0 { new_idx - 1 } else { 0 };
            new_col_idx.push(target);
        }
    }
    let new_num_cols = new_idx;
    if new_num_cols == 0 || new_num_cols == num_cols {
        return table; // nothing changed
    }

    // Build new x_coordinates: keep only the boundaries of non-gutter columns
    let mut new_x: Vec<f64> = Vec::with_capacity(new_num_cols + 1);
    new_x.push(table.x_coordinates[0]); // leftmost boundary
    for (c, &kept) in keep_col.iter().enumerate() {
        if kept {
            new_x.push(table.x_coordinates[c + 1]);
        }
    }
    // Ensure we have the rightmost boundary
    if new_x.len() <= new_num_cols {
        new_x.push(*table.x_coordinates.last().unwrap_or(&0.0));
    }

    // Rebuild rows with merged cells
    for row in &mut table.rows {
        let mut merged_cells: Vec<TableBorderCell> = Vec::with_capacity(new_num_cols);
        for (&target, cell_c) in new_col_idx.iter().zip(row.cells.iter()) {
            if target >= merged_cells.len() {
                // Start a new merged cell
                let mut cell = cell_c.clone();
                cell.col_number = target;
                // Expand bbox to cover gutter if this is a real column
                if target < new_x.len().saturating_sub(1) {
                    cell.bbox = BoundingBox::new(
                        cell.bbox.page_number,
                        new_x[target],
                        cell.bbox.bottom_y,
                        new_x[target + 1],
                        cell.bbox.top_y,
                    );
                }
                merged_cells.push(cell);
            } else {
                // Merge this gutter cell's content into the existing cell
                let existing = &mut merged_cells[target];
                existing.content.extend(cell_c.content.clone());
                existing.contents.extend(cell_c.contents.clone());
                // Expand bbox
                existing.bbox = existing.bbox.union(&cell_c.bbox);
            }
        }
        // Update col_span for merged cells
        for cell in &mut merged_cells {
            cell.col_span = 1;
        }
        row.cells = merged_cells;
        row.bbox = BoundingBox::new(
            row.bbox.page_number,
            new_x[0],
            row.bbox.bottom_y,
            *new_x.last().unwrap_or(&0.0),
            row.bbox.top_y,
        );
    }

    table.x_coordinates = new_x;
    table.x_widths = vec![0.5; new_num_cols + 1];
    table.num_columns = new_num_cols;

    table
}

/// Merge groups that share intersecting lines.
fn merge_groups(groups: &mut Vec<TableGroup>) {
    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..groups.len() {
            for j in (i + 1)..groups.len() {
                if groups_connected(&groups[i], &groups[j]) {
                    let other = groups.remove(j);
                    for hl in other.h_lines {
                        groups[i].add_h(hl);
                    }
                    for vl in other.v_lines {
                        groups[i].add_v(vl);
                    }
                    changed = true;
                    break;
                }
            }
            if changed {
                break;
            }
        }
    }
}

/// Check if two groups share any intersecting lines.
fn groups_connected(a: &TableGroup, b: &TableGroup) -> bool {
    for hl in &a.h_lines {
        for vl in &b.v_lines {
            if lines_intersect(hl, vl) {
                return true;
            }
        }
    }
    for hl in &b.h_lines {
        for vl in &a.v_lines {
            if lines_intersect(hl, vl) {
                return true;
            }
        }
    }
    false
}

/// Check if a horizontal and vertical line intersect.
fn lines_intersect(h: &LineChunk, v: &LineChunk) -> bool {
    if v.bbox.right_x + LINE_EPSILON < h.bbox.left_x
        || v.bbox.left_x - LINE_EPSILON > h.bbox.right_x
    {
        return false;
    }
    if h.bbox.top_y + LINE_EPSILON < v.bbox.bottom_y
        || h.bbox.bottom_y - LINE_EPSILON > v.bbox.top_y
    {
        return false;
    }
    true
}

/// Check if a line is horizontal.
fn is_horizontal(line: &LineChunk) -> bool {
    let dx = (line.bbox.right_x - line.bbox.left_x).abs();
    let dy = (line.bbox.top_y - line.bbox.bottom_y).abs();
    dx > dy && dy <= MAX_LINE_WIDTH
}

/// Check if a line is vertical.
fn is_vertical(line: &LineChunk) -> bool {
    let dx = (line.bbox.right_x - line.bbox.left_x).abs();
    let dy = (line.bbox.top_y - line.bbox.bottom_y).abs();
    dy > dx && dx <= MAX_LINE_WIDTH
}

/// Get the width/thickness of a line.
fn line_width(line: &LineChunk) -> f64 {
    let dx = (line.bbox.right_x - line.bbox.left_x).abs();
    let dy = (line.bbox.top_y - line.bbox.bottom_y).abs();
    dx.min(dy)
}

/// Check if two line chunks represent the same line.
fn lines_same(a: &LineChunk, b: &LineChunk) -> bool {
    (a.bbox.left_x - b.bbox.left_x).abs() < LINE_EPSILON
        && (a.bbox.bottom_y - b.bbox.bottom_y).abs() < LINE_EPSILON
        && (a.bbox.right_x - b.bbox.right_x).abs() < LINE_EPSILON
        && (a.bbox.top_y - b.bbox.top_y).abs() < LINE_EPSILON
}

/// Check if a line falls within a bounding box.
fn line_in_bbox(line: &LineChunk, bbox: &BoundingBox) -> bool {
    line.bbox.left_x >= bbox.left_x - LINE_EPSILON
        && line.bbox.right_x <= bbox.right_x + LINE_EPSILON
        && line.bbox.bottom_y >= bbox.bottom_y - LINE_EPSILON
        && line.bbox.top_y <= bbox.top_y + LINE_EPSILON
}

// ── Post-content-assignment table filter ─────────────────────────────────────

/// Maximum fraction of cells that may be empty before a bordered table is
/// rejected.  Chart grid lines produce tables where 70-95% of cells are
/// empty — real data tables have most cells populated.
const MAX_EMPTY_CELL_FRACTION: f64 = 0.80;

/// Minimum number of cells (across all rows) to apply the empty-cell filter.
/// Very small tables (e.g. 1×1, 2×1) are already handled by the boxed-heading
/// promoter and should not be filtered here.
const MIN_CELLS_FOR_EMPTY_FILTER: usize = 4;
/// Minimum columns required in the recovered token lattice before a page-wide
/// `1x1` pseudo-table is released for cluster-table recovery.
const PRE_CLUSTER_MIN_COLUMNS: usize = 3;
/// Minimum number of repeated multi-column rows required before releasing a
/// page-wide `1x1` pseudo-table.  One aligned infographic row is not enough.
const PRE_CLUSTER_MIN_ROWS: usize = 3;
/// Stronger column layouts can be trusted with fewer repeated rows.
const PRE_CLUSTER_STRONG_COLUMNS: usize = 4;
/// Minimum fraction of candidate rows that must align to the recovered column
/// anchors before the table is released.
const PRE_CLUSTER_MIN_STABLE_ROW_FRACTION: f64 = 0.6;
/// Gap threshold, in font-size units, for splitting trapped tokens into
/// separate column segments during pre-cluster recovery checks.
const PRE_CLUSTER_COLUMN_GAP_FACTOR: f64 = 1.0;
/// Baseline tolerance, in font-size units, when grouping trapped tokens into
/// rows during pre-cluster recovery checks.
const PRE_CLUSTER_BASELINE_TOLERANCE: f64 = 0.9;
/// Alignment tolerance, in font-size units, for matching recovered segments to
/// anchor columns during pre-cluster recovery checks.
const PRE_CLUSTER_ALIGN_TOLERANCE: f64 = 2.0;

/// Filter out bordered tables that are mostly empty (chart grid artifacts).
///
/// After content has been assigned to table cells, tables that originated
/// from chart axis grid lines typically have few or no populated cells.
/// This filter rejects such tables, releasing their (few) text chunks back
/// as free elements.
pub fn filter_empty_tables(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());

    for elem in elements {
        match elem {
            ContentElement::TableBorder(ref table) => {
                if is_tiny_empty_table(table) {
                    continue;
                }

                if should_preserve_sparse_recovered_table(table) {
                    result.push(elem);
                    continue;
                }

                let total_cells: usize = table.rows.iter().map(|r| r.cells.len()).sum();

                if total_cells >= MIN_CELLS_FOR_EMPTY_FILTER {
                    let empty_cells: usize = table
                        .rows
                        .iter()
                        .flat_map(|r| &r.cells)
                        .filter(|cell| !cell_has_textual_content(cell))
                        .count();

                    let empty_fraction = empty_cells as f64 / total_cells as f64;

                    if empty_fraction > MAX_EMPTY_CELL_FRACTION {
                        // Release any non-empty cell content as free TextChunks
                        for row in &table.rows {
                            for cell in &row.cells {
                                for token in &cell.content {
                                    let text = token.base.value.trim();
                                    if text.is_empty() || text == "[image]" {
                                        continue;
                                    }
                                    result.push(ContentElement::TextChunk(token.base.clone()));
                                }
                                for sub in &cell.contents {
                                    if element_has_textual_content(sub) {
                                        result.push(sub.clone());
                                    }
                                }
                            }
                        }
                        continue;
                    }
                }

                result.push(elem);
            }
            _ => result.push(elem),
        }
    }

    result
}

fn should_preserve_sparse_recovered_table(table: &TableBorder) -> bool {
    if !table.is_table_transformer || table.num_rows < 2 || table.num_columns < 2 {
        return false;
    }

    let mut populated_rows = 0usize;
    let mut populated_cols = std::collections::HashSet::new();
    let mut populated_cells = 0usize;

    for row in &table.rows {
        let mut row_has_text = false;
        for cell in &row.cells {
            if cell_has_textual_content(cell) {
                row_has_text = true;
                populated_cols.insert(cell.col_number);
                populated_cells += 1;
            }
        }
        if row_has_text {
            populated_rows += 1;
        }
    }

    populated_cells >= 2 && populated_rows >= 2 && populated_cols.len() >= 2
}

fn is_tiny_empty_table(table: &TableBorder) -> bool {
    let total_cells: usize = table.rows.iter().map(|r| r.cells.len()).sum();
    if total_cells == 0 || total_cells > 2 {
        return false;
    }

    let has_any_content = table
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .any(cell_has_textual_content);
    if has_any_content {
        return false;
    }

    table.num_rows <= 1 || (table.bbox.width() <= 220.0 && table.bbox.height() <= 60.0)
}

fn cell_has_textual_content(cell: &TableBorderCell) -> bool {
    cell.content.iter().any(|token| {
        let text = token.base.value.trim();
        !text.is_empty() && text != "[image]"
    }) || cell.contents.iter().any(element_has_textual_content)
}

fn element_has_textual_content(element: &ContentElement) -> bool {
    match element {
        ContentElement::TextChunk(chunk) => !chunk.value.trim().is_empty(),
        ContentElement::TextLine(line) => !line.value().trim().is_empty(),
        ContentElement::TextBlock(block) => !block.value().trim().is_empty(),
        ContentElement::Paragraph(paragraph) => !paragraph.base.value().trim().is_empty(),
        ContentElement::Heading(heading) => !heading.base.base.value().trim().is_empty(),
        ContentElement::NumberHeading(heading) => !heading.base.base.base.value().trim().is_empty(),
        ContentElement::List(list) => list
            .list_items
            .iter()
            .flat_map(|item| item.contents.iter())
            .any(element_has_textual_content),
        ContentElement::Table(_) | ContentElement::TableBorder(_) => true,
        _ => false,
    }
}

/// Filter out structurally suspicious tables and release their content back as
/// free-flowing text.
///
/// This catches two common false positives:
/// 1. Page-wide single-column "tables" produced from decorative borders or
///    scanned-page artifacts.
/// 2. Shallow brochure/card layouts that the cluster table detector mistakes
///    for a real 3+ column table.
pub fn filter_suspicious_tables(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());

    for elem in elements {
        match elem {
            ContentElement::TableBorder(ref table) if is_suspicious_table(table) => {
                release_table_content(table, &mut result);
            }
            _ => result.push(elem),
        }
    }

    result
}

/// Release obviously bogus page-wide single-cell tables early enough for the
/// cluster detector to recover the underlying multi-column text layout.
pub fn release_pre_cluster_tables(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());

    for elem in elements {
        match elem {
            ContentElement::TableBorder(ref table) if should_release_pre_cluster(table) => {
                release_table_content(table, &mut result);
            }
            _ => result.push(elem),
        }
    }

    result
}

fn is_suspicious_table(table: &TableBorder) -> bool {
    if is_single_cell_artifact_table(table) {
        return true;
    }

    if is_captionish_two_column_table(table) {
        return true;
    }

    if is_toc_like_two_column_table(table) {
        return true;
    }

    if is_single_column_prose_table(table) {
        return true;
    }

    if is_brochure_card_table(table) {
        return true;
    }

    if is_two_row_marketing_grid(table) {
        return true;
    }

    if is_wrapped_prose_grid_table(table) {
        return true;
    }

    if table.num_columns > 1 {
        return false;
    }

    // Only reject obviously pathological single-column page-wide tables. Real
    // one-column tables do exist; the benchmark regressions came from full-page
    // pseudo-tables wrapping OCR/noise.
    let width = table.bbox.width();
    let has_image_token = table
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .flat_map(|c| c.content.iter())
        .any(|t| t.base.value == "[image]");

    width >= 500.0 && (has_image_token || table.num_rows <= 4)
}

fn should_release_pre_cluster(table: &TableBorder) -> bool {
    if table.num_columns != 1 || table.num_rows != 1 {
        return false;
    }

    let only_cell = match table.rows.first().and_then(|row| row.cells.first()) {
        Some(cell) => cell,
        None => return false,
    };

    if table.bbox.width() < 500.0 || table.bbox.height() < 250.0 {
        return false;
    }

    let text = cell_text(only_cell);
    let image_tokens = only_cell
        .content
        .iter()
        .filter(|tok| tok.base.value == "[image]")
        .count();
    let word_count = text.split_whitespace().count();

    (image_tokens >= 2 || text.chars().count() >= 500 || word_count >= 80)
        && has_recoverable_pre_cluster_signal(only_cell)
}

fn is_single_cell_artifact_table(table: &TableBorder) -> bool {
    if table.num_columns != 1 || table.num_rows != 1 {
        return false;
    }

    let Some(cell) = table.rows.first().and_then(|row| row.cells.first()) else {
        return false;
    };
    let text = normalized_cell_text(cell);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let normalized: String = trimmed
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    let word_count = trimmed.split_whitespace().count();
    let char_count = trimmed.chars().count();
    let digit_count = trimmed.chars().filter(|ch| ch.is_ascii_digit()).count();
    let percent_count = trimmed.matches('%').count();
    let has_sentence_punctuation = trimmed.contains(['.', '?', '!']);
    let alpha_words: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphabetic))
        .collect();
    let titlecase_words = alpha_words
        .iter()
        .filter(|word| {
            word.chars()
                .find(|ch| ch.is_alphabetic())
                .is_some_and(char::is_uppercase)
        })
        .count();

    if (normalized.starts_with("figure")
        || normalized.starts_with("fig")
        || normalized.starts_with("diagram"))
        && table.bbox.width() <= 360.0
        && table.bbox.height() <= 80.0
    {
        return true;
    }

    let alpha_tokens: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphabetic))
        .collect();
    let single_char_alpha_tokens = alpha_tokens
        .iter()
        .filter(|word| word.chars().filter(|ch| ch.is_alphabetic()).count() == 1)
        .count();

    if table.bbox.height() <= 240.0
        && table.bbox.width() >= 220.0
        && percent_count >= 3
        && digit_count >= 6
        && !has_sentence_punctuation
        && (single_char_alpha_tokens * 10 >= alpha_tokens.len().max(1) * 2 || word_count >= 12)
    {
        return true;
    }

    if table.bbox.width() <= 180.0
        && table.bbox.height() >= 180.0
        && word_count >= 35
        && char_count >= 220
        && has_sentence_punctuation
    {
        return true;
    }

    table.bbox.width() <= 320.0
        && table.bbox.height() <= 90.0
        && (4..=20).contains(&word_count)
        && char_count <= 160
        && trimmed.contains(':')
        && digit_count <= 6
        && titlecase_words * 10 >= alpha_words.len().max(1) * 6
}

#[derive(Clone)]
struct PreClusterTokenRef {
    left_x: f64,
    right_x: f64,
    baseline: f64,
    font_size: f64,
    text: String,
}

#[derive(Clone)]
struct PreClusterSegment {
    left_x: f64,
    right_x: f64,
    word_count: usize,
}

struct PreClusterRowSignal {
    font_size: f64,
    segments: Vec<PreClusterSegment>,
}

fn has_recoverable_pre_cluster_signal(cell: &TableBorderCell) -> bool {
    let tokens = collect_pre_cluster_tokens(cell);
    if tokens.len() < PRE_CLUSTER_MIN_COLUMNS * PRE_CLUSTER_MIN_ROWS {
        return false;
    }

    let rows = group_pre_cluster_rows(&tokens);
    if rows.len() < PRE_CLUSTER_MIN_ROWS {
        return false;
    }

    let anchor = match rows.iter().max_by(|a, b| {
        a.segments.len().cmp(&b.segments.len()).then_with(|| {
            a.segments
                .iter()
                .map(|seg| seg.word_count)
                .sum::<usize>()
                .cmp(&b.segments.iter().map(|seg| seg.word_count).sum::<usize>())
        })
    }) {
        Some(row) => row,
        None => return false,
    };

    let anchor_cols = anchor.segments.len();
    if anchor_cols < PRE_CLUSTER_MIN_COLUMNS {
        return false;
    }

    let stable_rows = rows
        .iter()
        .filter(|row| pre_cluster_row_matches_anchor(row, anchor))
        .count();
    let stable_fraction = stable_rows as f64 / rows.len() as f64;

    if stable_fraction < PRE_CLUSTER_MIN_STABLE_ROW_FRACTION {
        return false;
    }

    if anchor_cols >= PRE_CLUSTER_STRONG_COLUMNS {
        stable_rows >= PRE_CLUSTER_MIN_ROWS
    } else {
        stable_rows > PRE_CLUSTER_MIN_ROWS
    }
}

fn collect_pre_cluster_tokens(cell: &TableBorderCell) -> Vec<PreClusterTokenRef> {
    let mut tokens = Vec::new();

    if !cell.content.is_empty() {
        for token in &cell.content {
            if token.base.value == "[image]" {
                continue;
            }
            let text = token.base.value.trim();
            if text.is_empty() {
                continue;
            }
            tokens.push(PreClusterTokenRef {
                left_x: token.base.bbox.left_x,
                right_x: token.base.bbox.right_x,
                baseline: token.base.bbox.bottom_y,
                font_size: token.base.font_size.max(1.0),
                text: text.to_string(),
            });
        }
        return tokens;
    }

    for elem in &cell.contents {
        match elem {
            ContentElement::TextChunk(chunk) => {
                let text = chunk.value.trim();
                if text.is_empty() {
                    continue;
                }
                tokens.push(PreClusterTokenRef {
                    left_x: chunk.bbox.left_x,
                    right_x: chunk.bbox.right_x,
                    baseline: chunk.bbox.bottom_y,
                    font_size: chunk.font_size.max(1.0),
                    text: text.to_string(),
                });
            }
            ContentElement::TextLine(line) => {
                for chunk in &line.text_chunks {
                    let text = chunk.value.trim();
                    if text.is_empty() {
                        continue;
                    }
                    tokens.push(PreClusterTokenRef {
                        left_x: chunk.bbox.left_x,
                        right_x: chunk.bbox.right_x,
                        baseline: line.base_line,
                        font_size: chunk.font_size.max(line.font_size).max(1.0),
                        text: text.to_string(),
                    });
                }
            }
            _ => {}
        }
    }

    tokens
}

fn group_pre_cluster_rows(tokens: &[PreClusterTokenRef]) -> Vec<PreClusterRowSignal> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<&PreClusterTokenRef> = tokens.iter().collect();
    sorted.sort_by(|a, b| {
        b.baseline
            .partial_cmp(&a.baseline)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.left_x
                    .partial_cmp(&b.left_x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut groups: Vec<Vec<&PreClusterTokenRef>> = Vec::new();
    let mut baselines: Vec<f64> = Vec::new();
    let mut font_sizes: Vec<f64> = Vec::new();

    for token in &sorted {
        let mut placed = false;
        for (idx, group) in groups.iter_mut().enumerate() {
            let tol = PRE_CLUSTER_BASELINE_TOLERANCE * token.font_size.min(font_sizes[idx]);
            if (baselines[idx] - token.baseline).abs() < tol {
                group.push(token);
                placed = true;
                break;
            }
        }
        if !placed {
            groups.push(vec![token]);
            baselines.push(token.baseline);
            font_sizes.push(token.font_size);
        }
    }

    let mut rows = Vec::new();
    for (idx, group) in groups.iter().enumerate() {
        let mut sorted_group = group.to_vec();
        sorted_group.sort_by(|a, b| {
            a.left_x
                .partial_cmp(&b.left_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let segments = split_pre_cluster_segments(&sorted_group, font_sizes[idx]);
        if is_viable_pre_cluster_row(&segments) {
            rows.push(PreClusterRowSignal {
                font_size: font_sizes[idx],
                segments,
            });
        }
    }

    rows
}

fn split_pre_cluster_segments(
    tokens: &[&PreClusterTokenRef],
    font_size: f64,
) -> Vec<PreClusterSegment> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut segments = Vec::new();
    let mut seg_left = tokens[0].left_x;
    let mut seg_right = tokens[0].right_x;
    let mut seg_font = tokens[0].font_size;
    let mut seg_words = tokens[0].text.split_whitespace().count();

    for token in &tokens[1..] {
        let gap = token.left_x - seg_right;
        let max_font = seg_font.max(token.font_size).max(font_size).max(1.0);
        if gap > PRE_CLUSTER_COLUMN_GAP_FACTOR * max_font {
            segments.push(PreClusterSegment {
                left_x: seg_left,
                right_x: seg_right,
                word_count: seg_words,
            });
            seg_left = token.left_x;
            seg_right = token.right_x;
            seg_font = token.font_size;
            seg_words = token.text.split_whitespace().count();
        } else {
            seg_right = seg_right.max(token.right_x);
            seg_words += token.text.split_whitespace().count();
        }
    }

    segments.push(PreClusterSegment {
        left_x: seg_left,
        right_x: seg_right,
        word_count: seg_words,
    });

    segments
}

fn is_viable_pre_cluster_row(segments: &[PreClusterSegment]) -> bool {
    if segments.len() < PRE_CLUSTER_MIN_COLUMNS {
        return false;
    }

    let max_words = segments.iter().map(|seg| seg.word_count).max().unwrap_or(0);
    let total_words: usize = segments.iter().map(|seg| seg.word_count).sum();

    if segments.len() >= PRE_CLUSTER_STRONG_COLUMNS {
        return max_words <= 28 && total_words <= 80;
    }

    max_words <= 12 && total_words <= 28
}

fn pre_cluster_row_matches_anchor(row: &PreClusterRowSignal, anchor: &PreClusterRowSignal) -> bool {
    let tol = PRE_CLUSTER_ALIGN_TOLERANCE * row.font_size.max(anchor.font_size).max(1.0);
    let matches = row
        .segments
        .iter()
        .filter(|seg| {
            anchor.segments.iter().any(|col| {
                let seg_center = (seg.left_x + seg.right_x) / 2.0;
                seg_center >= col.left_x - tol && seg_center <= col.right_x + tol
                    || (seg.left_x - col.left_x).abs() <= tol
            })
        })
        .count();

    matches >= PRE_CLUSTER_MIN_COLUMNS
        && (row.segments.len() >= anchor.segments.len().saturating_sub(1)
            || anchor.segments.len() >= PRE_CLUSTER_STRONG_COLUMNS)
}

fn is_captionish_two_column_table(table: &TableBorder) -> bool {
    if table.num_columns != 2 || table.num_rows > 2 {
        return false;
    }

    let first_row = match table.rows.first() {
        Some(row) if row.cells.len() >= 2 => row,
        _ => return false,
    };

    let left = normalize_table_keyword_text(&normalized_cell_text(&first_row.cells[0]));
    left.starts_with("figure") || left.starts_with("diagram") || left.starts_with("fig")
}

fn is_single_column_prose_table(table: &TableBorder) -> bool {
    if table.num_columns != 1 {
        return false;
    }

    let row_texts: Vec<String> = table
        .rows
        .iter()
        .filter_map(|row| row.cells.first())
        .map(normalized_cell_text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect();
    if row_texts.is_empty() {
        return false;
    }

    let has_image_token = table
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .flat_map(|c| c.content.iter())
        .any(|t| t.base.value == "[image]");
    if has_image_token && row_texts.len() <= 3 {
        return true;
    }

    if row_texts.len() != 1 || table.bbox.width() < 260.0 {
        return false;
    }

    let text = &row_texts[0];
    let word_count = text.split_whitespace().count();
    let hash_enumerations = text.match_indices("#").count();
    let has_sentence_punctuation = text.contains(['.', '?', '!']);
    let has_bullets = text.contains(['•', '●', '·']);

    word_count >= 25
        && text.chars().count() >= 180
        && has_sentence_punctuation
        && hash_enumerations < 2
        && (!has_bullets || (table.bbox.height() >= 260.0 && word_count >= 80))
}

fn is_wrapped_prose_grid_table(table: &TableBorder) -> bool {
    if table.num_columns < 3 || table.num_rows < 3 || table.num_rows > 4 {
        return false;
    }

    if table.bbox.width() < 600.0 || table.bbox.height() > 90.0 {
        return false;
    }

    let populated_cells: Vec<String> = table
        .rows
        .iter()
        .flat_map(|row| row.cells.iter().map(normalized_cell_text))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect();
    if populated_cells.len() < table.num_columns * 2 {
        return false;
    }

    let wordy_cells = populated_cells
        .iter()
        .filter(|text| {
            let words = text.split_whitespace().count();
            let chars = text.chars().count();
            words >= 5 && (20..=90).contains(&chars)
        })
        .count();
    let punctuated_cells = populated_cells
        .iter()
        .filter(|text| text.contains(['.', '?', '!']))
        .count();
    let numericish_cells = populated_cells
        .iter()
        .filter(|text| text.chars().any(|ch| ch.is_ascii_digit()))
        .count();

    if wordy_cells * 10 < populated_cells.len() * 8 || punctuated_cells > 0 || numericish_cells > 1
    {
        return false;
    }

    let first_row = match table.rows.first() {
        Some(row) => row,
        None => return false,
    };
    let first_row_avg_words = first_row
        .cells
        .iter()
        .map(normalized_cell_text)
        .map(|text| text.split_whitespace().count())
        .sum::<usize>() as f64
        / first_row.cells.len().max(1) as f64;

    first_row_avg_words >= 6.0
}

fn is_brochure_card_table(table: &TableBorder) -> bool {
    if table.num_columns < 3 || table.num_rows > 4 {
        return false;
    }

    if table.bbox.width() < 360.0 || table.bbox.height() > 340.0 {
        return false;
    }

    let populated_cells: Vec<&TableBorderCell> = table
        .rows
        .iter()
        .flat_map(|r| r.cells.iter())
        .filter(|cell| !cell_text(cell).is_empty())
        .collect();
    if populated_cells.len() < 3 {
        return false;
    }

    let short_cell_count = populated_cells
        .iter()
        .filter(|cell| cell_text(cell).chars().count() <= 90)
        .count();
    let headingish_count = populated_cells
        .iter()
        .filter(|cell| {
            let text = cell_text(cell);
            let first = text.chars().find(|c| c.is_alphanumeric());
            first.is_some_and(|c| c.is_uppercase()) && !text.contains(['.', '?', '!'])
        })
        .count();
    let numericish_count = populated_cells
        .iter()
        .filter(|cell| cell_text(cell).chars().any(|c| c.is_ascii_digit()))
        .count();
    let long_sentence_count = populated_cells
        .iter()
        .filter(|cell| {
            let text = cell_text(cell);
            text.chars().count() > 90 || text.matches('.').count() > 1
        })
        .count();

    short_cell_count * 10 >= populated_cells.len() * 8
        && headingish_count * 10 >= populated_cells.len() * 7
        && numericish_count * 5 <= populated_cells.len()
        && long_sentence_count * 3 <= populated_cells.len()
}

fn is_two_row_marketing_grid(table: &TableBorder) -> bool {
    if table.num_columns < 3 || table.num_rows > 2 {
        return false;
    }

    let populated_cells: Vec<String> = table
        .rows
        .iter()
        .flat_map(|row| row.cells.iter().map(cell_text))
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect();
    if populated_cells.len() < table.num_columns {
        return false;
    }

    let numeric_cells = populated_cells
        .iter()
        .filter(|text| text.chars().any(|ch| ch.is_ascii_digit()))
        .count();
    let long_sentence_cells = populated_cells
        .iter()
        .filter(|text| text.chars().count() > 120 || text.matches('.').count() > 1)
        .count();

    numeric_cells == 0 && long_sentence_cells == 0
}

fn is_toc_like_two_column_table(table: &TableBorder) -> bool {
    if table.num_columns != 2 || table.num_rows < 3 {
        return false;
    }

    let mut matched_rows = 0usize;
    let mut page_markers: Vec<u32> = Vec::new();
    for row in &table.rows {
        if row.cells.len() < 2 {
            continue;
        }
        let left = cell_text(&row.cells[0]);
        let right = cell_text(&row.cells[1]);
        if is_toc_title_text(&left) && parse_page_marker(&right).is_some() {
            matched_rows += 1;
            if let Some(page) = parse_page_marker(&right) {
                page_markers.push(page);
            }
        }
    }

    if matched_rows < 3 || matched_rows * 10 < table.num_rows * 6 {
        return false;
    }

    let ascending_pairs = page_markers
        .windows(2)
        .filter(|pair| pair[0] <= pair[1])
        .count();
    ascending_pairs + 1 >= page_markers.len().saturating_sub(1)
}

fn cell_text(cell: &TableBorderCell) -> String {
    let token_text = cell
        .content
        .iter()
        .map(|t| t.base.value.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if !token_text.is_empty() {
        return token_text;
    }

    cell.contents
        .iter()
        .filter_map(|elem| match elem {
            ContentElement::TextChunk(chunk) => Some(chunk.value.trim().to_string()),
            _ => None,
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalized_cell_text(cell: &TableBorderCell) -> String {
    repair_fragmented_table_text(&cell_text(cell))
}

fn normalize_table_keyword_text(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn repair_fragmented_table_text(text: &str) -> String {
    const STOPWORDS: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if", "in", "into",
        "is", "it", "may", "must", "not", "of", "on", "or", "per", "that", "the", "to", "with",
    ];

    let mut parts: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    if parts.len() < 2 {
        return text.to_string();
    }

    let mut i = 0usize;
    while i + 1 < parts.len() {
        let left = parts[i].clone();
        let right = parts[i + 1].clone();
        let left_clean = left.trim_matches(|c: char| !c.is_alphabetic());
        let right_clean = right.trim_matches(|c: char| !c.is_alphabetic());
        let left_lower = left_clean.to_ascii_lowercase();
        let right_lower = right_clean.to_ascii_lowercase();

        let should_join = !left_clean.is_empty()
            && !right_clean.is_empty()
            && left_clean.chars().all(char::is_alphabetic)
            && right_clean.chars().all(char::is_alphabetic)
            && (left_clean.len() <= 4 || right_clean.len() <= 4)
            && left_clean.len() + right_clean.len() >= 6
            && !right_clean.chars().next().is_some_and(char::is_uppercase)
            && !STOPWORDS.contains(&left_lower.as_str())
            && !STOPWORDS.contains(&right_lower.as_str());

        if should_join {
            let next = parts.remove(i + 1);
            parts[i].push_str(&next);
        } else {
            i += 1;
        }
    }

    parts.join(" ")
}

fn starts_with_ordered_index(text: &str) -> bool {
    let trimmed = text.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut saw_digit = false;

    while let Some(c) = chars.peek().copied() {
        if c.is_ascii_digit() {
            saw_digit = true;
            chars.next();
        } else {
            break;
        }
    }

    if !saw_digit {
        return false;
    }

    matches!(chars.next(), Some('.' | ')')) && chars.next().is_some_and(char::is_whitespace)
}

#[allow(dead_code)]
fn is_short_numeric_text(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed.len() <= 4 && trimmed.chars().all(|c| c.is_ascii_digit())
}

fn is_toc_title_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return false;
    }

    let word_count = trimmed.split_whitespace().count();
    if !(1..=14).contains(&word_count) {
        return false;
    }

    if !trimmed.chars().any(|c| c.is_alphabetic()) {
        return false;
    }

    if parse_page_marker(trimmed).is_some() {
        return false;
    }

    if trimmed.ends_with(['.', ';']) {
        return false;
    }

    starts_with_ordered_index(trimmed)
        || trimmed.starts_with("Experiment #")
        || trimmed.chars().next().is_some_and(|c| c.is_uppercase())
}

fn parse_page_marker(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.len() <= 5 && trimmed.chars().all(|c| c.is_ascii_digit()) {
        return trimmed.parse().ok();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.len() <= 10
        && lower
            .chars()
            .all(|c| matches!(c, 'i' | 'v' | 'x' | 'l' | 'c' | 'd' | 'm'))
    {
        return roman_to_int(&lower);
    }

    None
}

fn roman_to_int(text: &str) -> Option<u32> {
    let mut total = 0u32;
    let mut prev = 0u32;

    for ch in text.chars().rev() {
        let value = match ch {
            'i' => 1,
            'v' => 5,
            'x' => 10,
            'l' => 50,
            'c' => 100,
            'd' => 500,
            'm' => 1000,
            _ => return None,
        };
        if value < prev {
            total = total.checked_sub(value)?;
        } else {
            total = total.checked_add(value)?;
            prev = value;
        }
    }

    Some(total)
}

fn release_table_content(table: &TableBorder, out: &mut Vec<ContentElement>) {
    let toc_like = is_toc_like_two_column_table(table);

    for row in &table.rows {
        if toc_like && row.cells.len() >= 2 {
            if let Some(elem) = merge_toc_row_cells(&row.cells[0], &row.cells[1]) {
                out.extend(text_line_grouper::group_text_lines(vec![elem], None));
            }
            continue;
        }

        for cell in &row.cells {
            release_cell_content(cell, out);
        }
    }
}

fn release_cell_content(cell: &TableBorderCell, out: &mut Vec<ContentElement>) {
    let mut cell_elements = Vec::new();
    append_cell_content(cell, &mut cell_elements);

    if cell_elements.is_empty() {
        return;
    }

    out.extend(text_line_grouper::group_text_lines(cell_elements, None));
}

fn append_cell_content(cell: &TableBorderCell, out: &mut Vec<ContentElement>) {
    for token in &cell.content {
        if token.base.value == "[image]" {
            continue;
        }
        out.push(ContentElement::TextChunk(token.base.clone()));
    }
    for sub in &cell.contents {
        out.push(sub.clone());
    }
}

fn merge_toc_row_cells(left: &TableBorderCell, right: &TableBorderCell) -> Option<ContentElement> {
    let left_text = cell_text(left);
    let right_text = cell_text(right);
    let combined = match (left_text.trim(), right_text.trim()) {
        ("", "") => return None,
        ("", right) => right.to_string(),
        (left, "") => left.to_string(),
        (left, right) => format!("{left} {right}"),
    };

    let mut chunk = left
        .content
        .first()
        .or_else(|| right.content.first())
        .map(|token| token.base.clone())
        .or_else(|| {
            left.contents
                .iter()
                .chain(right.contents.iter())
                .find_map(|elem| match elem {
                    ContentElement::TextChunk(chunk) => Some(chunk.clone()),
                    _ => None,
                })
        })?;

    chunk.value = combined;
    chunk.bbox = left.bbox.union(&right.bbox);
    chunk.page_number = chunk.bbox.page_number;
    Some(ContentElement::TextChunk(chunk))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::models::bbox::Vertex;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::table::{TableToken, TableTokenType};

    fn h_line(x1: f64, y: f64, x2: f64) -> LineChunk {
        LineChunk {
            bbox: BoundingBox::new(Some(1), x1, y - 0.5, x2, y + 0.5),
            index: None,
            level: None,
            start: Vertex {
                x: x1,
                y,
                radius: 0.0,
            },
            end: Vertex {
                x: x2,
                y,
                radius: 0.0,
            },
            width: 1.0,
            is_horizontal_line: true,
            is_vertical_line: false,
            is_square: false,
        }
    }

    fn v_line(x: f64, y1: f64, y2: f64) -> LineChunk {
        LineChunk {
            bbox: BoundingBox::new(Some(1), x - 0.5, y1, x + 0.5, y2),
            index: None,
            level: None,
            start: Vertex {
                x,
                y: y1,
                radius: 0.0,
            },
            end: Vertex {
                x,
                y: y2,
                radius: 0.0,
            },
            width: 1.0,
            is_horizontal_line: false,
            is_vertical_line: true,
            is_square: false,
        }
    }

    fn make_positioned_token(
        value: &str,
        font_size: f64,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
    ) -> TableToken {
        TableToken {
            base: TextChunk {
                value: value.to_string(),
                bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
                index: None,
                level: None,
                mcid: None,
                font_size,
                font_weight: 400.0,
                font_name: "Arial".to_string(),
                italic_angle: 0.0,
                font_color: "#000".to_string(),
                contrast_ratio: 21.0,
                symbol_ends: Vec::new(),
                text_format: TextFormat::Normal,
                text_type: TextType::Regular,
                pdf_layer: PdfLayer::Main,
                ocg_visible: true,
                page_number: Some(1),
            },
            token_type: TableTokenType::Text,
        }
    }

    fn make_token(value: &str, font_size: f64) -> TableToken {
        make_positioned_token(value, font_size, 0.0, 0.0, 100.0, 20.0)
    }

    fn make_table_cell(
        row_number: usize,
        col_number: usize,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
        text: &str,
    ) -> TableBorderCell {
        TableBorderCell {
            bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
            index: None,
            level: None,
            row_number,
            col_number,
            row_span: 1,
            col_span: 1,
            content: vec![make_positioned_token(
                text, 12.0, left_x, bottom_y, right_x, top_y,
            )],
            contents: Vec::new(),
            semantic_type: None,
        }
    }

    fn make_empty_table_cell(
        row_number: usize,
        col_number: usize,
        left_x: f64,
        bottom_y: f64,
        right_x: f64,
        top_y: f64,
    ) -> TableBorderCell {
        TableBorderCell {
            bbox: BoundingBox::new(Some(1), left_x, bottom_y, right_x, top_y),
            index: None,
            level: None,
            row_number,
            col_number,
            row_span: 1,
            col_span: 1,
            content: Vec::new(),
            contents: Vec::new(),
            semantic_type: None,
        }
    }

    #[test]
    fn test_simple_2x2_table() {
        // A 2-column, 2-row grid
        let elements = vec![
            // Horizontal lines (3 lines for 2 rows)
            ContentElement::Line(h_line(100.0, 700.0, 300.0)),
            ContentElement::Line(h_line(100.0, 680.0, 300.0)),
            ContentElement::Line(h_line(100.0, 660.0, 300.0)),
            // Vertical lines (3 lines for 2 columns)
            ContentElement::Line(v_line(100.0, 660.0, 700.0)),
            ContentElement::Line(v_line(200.0, 660.0, 700.0)),
            ContentElement::Line(v_line(300.0, 660.0, 700.0)),
        ];

        let result = detect_table_borders(elements);

        let tables: Vec<_> = result
            .iter()
            .filter(|e| matches!(e, ContentElement::TableBorder(_)))
            .collect();
        assert_eq!(tables.len(), 1, "Expected 1 table, found {}", tables.len());

        if let ContentElement::TableBorder(t) = &tables[0] {
            assert_eq!(t.num_rows, 2);
            assert_eq!(t.num_columns, 2);
            assert_eq!(t.rows.len(), 2);
            assert_eq!(t.rows[0].cells.len(), 2);
        }
    }

    #[test]
    fn test_no_lines_no_table() {
        let elements = vec![ContentElement::Image(crate::models::chunks::ImageChunk {
            bbox: BoundingBox::new(Some(1), 100.0, 100.0, 200.0, 200.0),
            index: None,
            level: None,
        })];
        let result = detect_table_borders(elements);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_insufficient_lines() {
        // Only horizontals, no verticals
        let elements = vec![
            ContentElement::Line(h_line(100.0, 700.0, 300.0)),
            ContentElement::Line(h_line(100.0, 680.0, 300.0)),
        ];
        let result = detect_table_borders(elements);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_non_intersecting_lines_no_table() {
        let elements = vec![
            ContentElement::Line(h_line(100.0, 700.0, 200.0)),
            ContentElement::Line(v_line(300.0, 500.0, 600.0)), // far away
        ];
        let result = detect_table_borders(elements);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_is_horizontal() {
        assert!(is_horizontal(&h_line(0.0, 100.0, 200.0)));
        assert!(!is_horizontal(&v_line(100.0, 0.0, 200.0)));
    }

    #[test]
    fn test_is_vertical() {
        assert!(is_vertical(&v_line(100.0, 0.0, 200.0)));
        assert!(!is_vertical(&h_line(0.0, 100.0, 200.0)));
    }

    #[test]
    fn test_lines_intersect() {
        let h = h_line(100.0, 700.0, 300.0);
        let v = v_line(200.0, 660.0, 720.0);
        assert!(lines_intersect(&h, &v));
    }

    #[test]
    fn test_lines_do_not_intersect() {
        let h = h_line(100.0, 700.0, 150.0);
        let v = v_line(200.0, 660.0, 720.0);
        assert!(!lines_intersect(&h, &v));
    }

    #[test]
    fn test_filter_suspicious_pagewide_single_column_table_releases_text() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 720.0, 400.0),
            index: None,
            level: None,
            x_coordinates: vec![0.0, 720.0],
            x_widths: vec![0.0, 0.0],
            y_coordinates: vec![400.0, 300.0, 0.0],
            y_widths: vec![0.0, 0.0, 0.0],
            rows: vec![
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 0.0, 300.0, 720.0, 400.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    cells: vec![TableBorderCell {
                        bbox: BoundingBox::new(Some(1), 0.0, 300.0, 720.0, 400.0),
                        index: None,
                        level: None,
                        row_number: 0,
                        col_number: 0,
                        row_span: 1,
                        col_span: 1,
                        content: vec![make_token("[image]", 18.0), make_token("Contents", 18.0)],
                        contents: Vec::new(),
                        semantic_type: None,
                    }],
                    semantic_type: None,
                },
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 0.0, 0.0, 720.0, 300.0),
                    index: None,
                    level: None,
                    row_number: 1,
                    cells: vec![TableBorderCell {
                        bbox: BoundingBox::new(Some(1), 0.0, 0.0, 720.0, 300.0),
                        index: None,
                        level: None,
                        row_number: 1,
                        col_number: 0,
                        row_span: 1,
                        col_span: 1,
                        content: vec![make_token("1. Overview of OCR Pack", 12.0)],
                        contents: Vec::new(),
                        semantic_type: None,
                    }],
                    semantic_type: None,
                },
            ],
            num_rows: 2,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert_eq!(result.len(), 2);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
    }

    #[test]
    fn test_filter_empty_tiny_table_removes_empty_caption_artifact() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 338.33, 403.59, 523.92, 451.05),
            index: None,
            level: None,
            x_coordinates: vec![338.33, 402.67, 523.92],
            x_widths: vec![0.0; 3],
            y_coordinates: vec![451.05, 403.59],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 338.33, 403.59, 523.92, 451.05),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![
                    TableBorderCell {
                        bbox: BoundingBox::new(Some(1), 338.33, 403.59, 402.67, 451.05),
                        index: None,
                        level: None,
                        row_number: 0,
                        col_number: 0,
                        row_span: 1,
                        col_span: 1,
                        content: Vec::new(),
                        contents: Vec::new(),
                        semantic_type: None,
                    },
                    TableBorderCell {
                        bbox: BoundingBox::new(Some(1), 402.67, 403.59, 523.92, 451.05),
                        index: None,
                        level: None,
                        row_number: 0,
                        col_number: 1,
                        row_span: 1,
                        col_span: 1,
                        content: Vec::new(),
                        contents: Vec::new(),
                        semantic_type: None,
                    },
                ],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_empty_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_suspicious_tables_keeps_narrow_single_column_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 100.0, 100.0, 260.0, 260.0),
            index: None,
            level: None,
            x_coordinates: vec![100.0, 260.0],
            x_widths: vec![0.0, 0.0],
            y_coordinates: vec![260.0, 100.0],
            y_widths: vec![0.0, 0.0],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 100.0, 100.0, 260.0, 260.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 100.0, 100.0, 260.0, 260.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token("Header", 12.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::TableBorder(_)));
    }

    #[test]
    fn test_filter_empty_tables_drops_structural_only_cells() {
        let image = ContentElement::Image(crate::models::chunks::ImageChunk {
            bbox: BoundingBox::new(Some(1), 110.0, 110.0, 150.0, 150.0),
            index: None,
            level: None,
        });
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 100.0, 100.0, 300.0, 300.0),
            index: None,
            level: None,
            x_coordinates: vec![100.0, 200.0, 300.0],
            x_widths: vec![0.0; 3],
            y_coordinates: vec![300.0, 200.0, 100.0],
            y_widths: vec![0.0; 3],
            rows: vec![
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 100.0, 200.0, 300.0, 300.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    cells: vec![
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 100.0, 200.0, 200.0, 300.0),
                            index: None,
                            level: None,
                            row_number: 0,
                            col_number: 0,
                            row_span: 1,
                            col_span: 1,
                            content: vec![make_token("[image]", 12.0)],
                            contents: vec![image.clone()],
                            semantic_type: None,
                        },
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 200.0, 200.0, 300.0, 300.0),
                            index: None,
                            level: None,
                            row_number: 0,
                            col_number: 1,
                            row_span: 1,
                            col_span: 1,
                            content: Vec::new(),
                            contents: vec![image.clone()],
                            semantic_type: None,
                        },
                    ],
                    semantic_type: None,
                },
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 100.0, 100.0, 300.0, 200.0),
                    index: None,
                    level: None,
                    row_number: 1,
                    cells: vec![
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 100.0, 100.0, 200.0, 200.0),
                            index: None,
                            level: None,
                            row_number: 1,
                            col_number: 0,
                            row_span: 1,
                            col_span: 1,
                            content: vec![make_token("[image]", 12.0)],
                            contents: vec![image.clone()],
                            semantic_type: None,
                        },
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 200.0, 100.0, 300.0, 200.0),
                            index: None,
                            level: None,
                            row_number: 1,
                            col_number: 1,
                            row_span: 1,
                            col_span: 1,
                            content: Vec::new(),
                            contents: vec![image],
                            semantic_type: None,
                        },
                    ],
                    semantic_type: None,
                },
            ],
            num_rows: 2,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_empty_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_empty_tables_keeps_sparse_raster_recovered_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 100.0, 100.0, 520.0, 360.0),
            index: None,
            level: None,
            x_coordinates: vec![100.0, 200.0, 320.0, 420.0, 520.0],
            x_widths: vec![0.0; 5],
            y_coordinates: vec![360.0, 300.0, 240.0, 180.0, 100.0],
            y_widths: vec![0.0; 5],
            rows: vec![
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 100.0, 300.0, 520.0, 360.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    cells: vec![
                        make_table_cell(0, 0, 100.0, 300.0, 200.0, 360.0, "Biomass Type"),
                        make_empty_table_cell(0, 1, 200.0, 300.0, 320.0, 360.0),
                        make_table_cell(0, 2, 320.0, 300.0, 420.0, 360.0, "Domestic logs"),
                        make_empty_table_cell(0, 3, 420.0, 300.0, 520.0, 360.0),
                    ],
                    semantic_type: None,
                },
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 100.0, 240.0, 520.0, 300.0),
                    index: None,
                    level: None,
                    row_number: 1,
                    cells: vec![
                        make_table_cell(1, 0, 100.0, 240.0, 200.0, 300.0, "Biogas"),
                        make_table_cell(1, 1, 200.0, 240.0, 320.0, 300.0, "98%"),
                        make_empty_table_cell(1, 2, 320.0, 240.0, 420.0, 300.0),
                        make_empty_table_cell(1, 3, 420.0, 240.0, 520.0, 300.0),
                    ],
                    semantic_type: None,
                },
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 100.0, 180.0, 520.0, 240.0),
                    index: None,
                    level: None,
                    row_number: 2,
                    cells: vec![
                        make_table_cell(2, 0, 100.0, 180.0, 200.0, 240.0, "Unutilised wood"),
                        make_empty_table_cell(2, 1, 200.0, 180.0, 320.0, 240.0),
                        make_table_cell(2, 2, 320.0, 180.0, 420.0, 240.0, "2%"),
                        make_empty_table_cell(2, 3, 420.0, 180.0, 520.0, 240.0),
                    ],
                    semantic_type: None,
                },
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 100.0, 100.0, 520.0, 180.0),
                    index: None,
                    level: None,
                    row_number: 3,
                    cells: vec![
                        make_empty_table_cell(3, 0, 100.0, 100.0, 200.0, 180.0),
                        make_empty_table_cell(3, 1, 200.0, 100.0, 320.0, 180.0),
                        make_empty_table_cell(3, 2, 320.0, 100.0, 420.0, 180.0),
                        make_empty_table_cell(3, 3, 420.0, 100.0, 520.0, 180.0),
                    ],
                    semantic_type: None,
                },
            ],
            num_rows: 4,
            num_columns: 4,
            is_bad_table: false,
            is_table_transformer: true,
            previous_table: None,
            next_table: None,
        };

        let result = filter_empty_tables(vec![ContentElement::TableBorder(table)]);
        assert!(matches!(
            result.first(),
            Some(ContentElement::TableBorder(_))
        ));
    }

    #[test]
    fn test_release_pre_cluster_pagewide_single_cell_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 721.0, 406.0),
            index: None,
            level: None,
            x_coordinates: vec![0.0, 721.0],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![406.0, 0.0],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 0.0, 0.0, 721.0, 406.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 0.0, 0.0, 721.0, 406.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![
                        make_positioned_token("[image]", 11.0, 12.0, 352.0, 42.0, 384.0),
                        make_positioned_token("[image]", 11.0, 56.0, 352.0, 86.0, 384.0),
                        make_positioned_token("Service", 11.0, 38.0, 330.0, 88.0, 342.0),
                        make_positioned_token("Stage", 11.0, 145.0, 330.0, 188.0, 342.0),
                        make_positioned_token("Function Name", 11.0, 268.0, 330.0, 375.0, 342.0),
                        make_positioned_token("Explanation", 11.0, 425.0, 330.0, 515.0, 342.0),
                        make_positioned_token("Expected Benefit", 11.0, 560.0, 330.0, 690.0, 342.0),
                        make_positioned_token("1. Project creation", 11.0, 38.0, 300.0, 170.0, 312.0),
                        make_positioned_token("Project creation and management", 11.0, 145.0, 300.0, 360.0, 312.0),
                        make_positioned_token("Select document type", 11.0, 268.0, 300.0, 410.0, 312.0),
                        make_positioned_token("Automatically run", 11.0, 425.0, 300.0, 530.0, 312.0),
                        make_positioned_token("Create projects faster", 11.0, 560.0, 300.0, 700.0, 312.0),
                        make_positioned_token("2. Labeling", 11.0, 38.0, 272.0, 120.0, 284.0),
                        make_positioned_token("Manage annotation jobs", 11.0, 145.0, 272.0, 320.0, 284.0),
                        make_positioned_token("Assign reviewers", 11.0, 268.0, 272.0, 390.0, 284.0),
                        make_positioned_token("Track quality", 11.0, 425.0, 272.0, 505.0, 284.0),
                        make_positioned_token("Reduce rework", 11.0, 560.0, 272.0, 665.0, 284.0),
                        make_positioned_token("3. Deployment", 11.0, 38.0, 244.0, 135.0, 256.0),
                        make_positioned_token("Ship approved models", 11.0, 145.0, 244.0, 315.0, 256.0),
                        make_positioned_token("Monitor drift", 11.0, 268.0, 244.0, 350.0, 256.0),
                        make_positioned_token("Update alerts", 11.0, 425.0, 244.0, 510.0, 256.0),
                        make_positioned_token("Improve reliability", 11.0, 560.0, 244.0, 690.0, 256.0),
                        make_positioned_token(
                            "This supporting sentence makes the total text volume realistic without changing the lattice signal",
                            11.0,
                            40.0,
                            206.0,
                            705.0,
                            218.0,
                        ),
                    ],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = release_pre_cluster_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
        assert!(result
            .iter()
            .any(|e| matches!(e, ContentElement::TextLine(_))));
    }

    #[test]
    fn test_release_pre_cluster_keeps_pagewide_single_cell_prose_panel() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 721.0, 406.0),
            index: None,
            level: None,
            x_coordinates: vec![0.0, 721.0],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![406.0, 0.0],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 0.0, 0.0, 721.0, 406.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 0.0, 0.0, 721.0, 406.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![
                        make_positioned_token("[image]", 11.0, 12.0, 352.0, 42.0, 384.0),
                        make_positioned_token("[image]", 11.0, 56.0, 352.0, 86.0, 384.0),
                        make_positioned_token(
                            "This course resource explains the lesson goals and gives teachers a long narrative introduction",
                            11.0,
                            38.0,
                            314.0,
                            685.0,
                            326.0,
                        ),
                        make_positioned_token(
                            "Students should discuss the material in groups and reflect on how the examples connect to practice",
                            11.0,
                            38.0,
                            286.0,
                            690.0,
                            298.0,
                        ),
                        make_positioned_token(
                            "Additional explanation continues here with full-sentence prose rather than repeated aligned columns",
                            11.0,
                            38.0,
                            258.0,
                            676.0,
                            270.0,
                        ),
                        make_positioned_token(
                            "A final paragraph offers implementation notes and stretches across the full width of the bordered panel",
                            11.0,
                            38.0,
                            230.0,
                            700.0,
                            242.0,
                        ),
                    ],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = release_pre_cluster_tables(vec![ContentElement::TableBorder(table)]);
        assert!(matches!(
            result.first(),
            Some(ContentElement::TableBorder(_))
        ));
    }

    #[test]
    fn test_filter_suspicious_single_cell_figure_caption_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 278.8, 350.1, 547.0, 388.0),
            index: None,
            level: None,
            x_coordinates: vec![278.8, 547.0],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![388.0, 350.1],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 278.8, 350.1, 547.0, 388.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 278.8, 350.1, 547.0, 388.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(
                        "Fig u r e 2 . Fo u ler s f r om the S ou th Ha r b or of M a n il a Bay. P h oto b y S AI L S - P OR TE C M a n il a Bay",
                        11.0,
                    )],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_filter_suspicious_single_cell_chart_label_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 156.6, 162.4, 487.3, 334.0),
            index: None,
            level: None,
            x_coordinates: vec![156.6, 487.3],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![334.0, 162.4],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 156.6, 162.4, 487.3, 334.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 156.6, 162.4, 487.3, 334.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(
                        "32 % 44 % 8% 12 % B elo w 5% of the L GU budget 5% t o belo w 10% 10% t o belo w 20% 20% and o v er No A lloca tion I don ’ t k no w",
                        11.0,
                    )],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_filter_suspicious_single_cell_narrow_prose_sidebar() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 56.2, 154.0, 173.7, 508.9),
            index: None,
            level: None,
            x_coordinates: vec![56.2, 173.7],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![508.9, 154.0],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 56.2, 154.0, 173.7, 508.9),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 56.2, 154.0, 173.7, 508.9),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(
                        "I n this c on te xt, w e ar e talking about fac t-che cking tha t is done bef or e a sour c e is publishe d. Ov er the last t w o de c ades ther e has be en an incr e ase in fac t che cking as an ac tivi t y tha t tak es plac e af ter a sour c e has be en publishe d, a pr ac tic e discusse d in mor e de tail in the chapter , SIFTing I nf orma tion.",
                        11.0,
                    )],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_filter_suspicious_single_cell_prompt_heading_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 56.2, 100.8, 339.8, 168.9),
            index: None,
            level: None,
            x_coordinates: vec![56.2, 339.8],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![168.9, 100.8],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 56.2, 100.8, 339.8, 168.9),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 56.2, 100.8, 339.8, 168.9),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(
                        "Reflection & Discussion Question 1: Taking Stock of What You Already Know",
                        11.0,
                    )],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_release_pre_cluster_keeps_multirow_single_column_table() {
        let rows = (0..3)
            .map(|row_number| TableBorderRow {
                bbox: BoundingBox::new(
                    Some(1),
                    0.0,
                    300.0 - row_number as f64 * 40.0,
                    900.0,
                    340.0 - row_number as f64 * 40.0,
                ),
                index: None,
                level: None,
                row_number,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(
                        Some(1),
                        0.0,
                        300.0 - row_number as f64 * 40.0,
                        900.0,
                        340.0 - row_number as f64 * 40.0,
                    ),
                    index: None,
                    level: None,
                    row_number,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![
                        make_token("[image]", 11.0),
                        make_token("Chart axis label", 11.0),
                    ],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            })
            .collect();

        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 1023.0, 406.0),
            index: None,
            level: None,
            x_coordinates: vec![0.0, 1023.0],
            x_widths: vec![0.0; 2],
            y_coordinates: vec![406.0, 300.0, 260.0, 220.0],
            y_widths: vec![0.0; 4],
            rows,
            num_rows: 3,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = release_pre_cluster_tables(vec![ContentElement::TableBorder(table)]);
        assert!(matches!(
            result.first(),
            Some(ContentElement::TableBorder(_))
        ));
    }

    #[test]
    fn test_filter_suspicious_toc_like_two_column_table() {
        let row = |row_number: usize, left: &str, right: &str| TableBorderRow {
            bbox: BoundingBox::new(Some(1), 240.0, 0.0, 600.0, 20.0),
            index: None,
            level: None,
            row_number,
            cells: vec![
                TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 240.0, 0.0, 540.0, 20.0),
                    index: None,
                    level: None,
                    row_number,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(left, 12.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                },
                TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 540.0, 0.0, 600.0, 20.0),
                    index: None,
                    level: None,
                    row_number,
                    col_number: 1,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(right, 12.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                },
            ],
            semantic_type: None,
        };

        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 240.0, 140.0, 600.0, 300.0),
            index: None,
            level: None,
            x_coordinates: vec![240.0, 540.0, 600.0],
            x_widths: vec![0.0, 0.0, 0.0],
            y_coordinates: vec![300.0, 260.0, 220.0, 180.0, 140.0],
            y_widths: vec![0.0; 5],
            rows: vec![
                row(0, "1. Overview of OCR Pack", "1"),
                row(1, "2. Introduction of Product Services", "2"),
                row(2, "3. Product - Detail Specification", "6"),
                row(3, "4. Integration Policy", "7"),
            ],
            num_rows: 4,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
        let texts: Vec<String> = result
            .iter()
            .filter_map(|e| match e {
                ContentElement::TextLine(line) => Some(line.value()),
                _ => None,
            })
            .collect();
        assert!(texts
            .iter()
            .any(|t| t.contains("1. Overview of OCR Pack 1")));
    }

    #[test]
    fn test_filter_suspicious_captionish_two_column_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 338.0, 403.0, 524.0, 451.0),
            index: None,
            level: None,
            x_coordinates: vec![338.0, 403.0, 524.0],
            x_widths: vec![0.0; 3],
            y_coordinates: vec![451.0, 403.0],
            y_widths: vec![0.0; 2],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 338.0, 403.0, 524.0, 451.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![
                    TableBorderCell {
                        bbox: BoundingBox::new(Some(1), 338.0, 403.0, 403.0, 451.0),
                        index: None,
                        level: None,
                        row_number: 0,
                        col_number: 0,
                        row_span: 1,
                        col_span: 1,
                        content: vec![make_token("Figure 6", 12.0)],
                        contents: Vec::new(),
                        semantic_type: None,
                    },
                    TableBorderCell {
                        bbox: BoundingBox::new(Some(1), 403.0, 403.0, 524.0, 451.0),
                        index: None,
                        level: None,
                        row_number: 0,
                        col_number: 1,
                        row_span: 1,
                        col_span: 1,
                        content: vec![make_token("World Health Day Celebration", 12.0)],
                        contents: Vec::new(),
                        semantic_type: None,
                    },
                ],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
        assert!(result
            .iter()
            .all(|e| !matches!(e, ContentElement::TableBorder(_))));
    }

    #[test]
    fn test_filter_suspicious_toc_like_experiment_table() {
        let row = |row_number: usize, left: &str, right: &str| TableBorderRow {
            bbox: BoundingBox::new(Some(1), 56.0, 0.0, 557.0, 20.0),
            index: None,
            level: None,
            row_number,
            cells: vec![
                TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 56.0, 0.0, 349.0, 20.0),
                    index: None,
                    level: None,
                    row_number,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(left, 10.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                },
                TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 349.0, 0.0, 557.0, 20.0),
                    index: None,
                    level: None,
                    row_number,
                    col_number: 1,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(right, 10.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                },
            ],
            semantic_type: None,
        };

        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 56.0, 100.0, 557.0, 220.0),
            index: None,
            level: None,
            x_coordinates: vec![56.0, 349.0, 557.0],
            x_widths: vec![0.0, 0.0, 0.0],
            y_coordinates: vec![220.0, 190.0, 160.0, 130.0, 100.0],
            y_widths: vec![0.0; 5],
            rows: vec![
                row(0, "Experiment #1: Hydrostatic Pressure", "3"),
                row(1, "Experiment #2: Bernoulli's Theorem Demonstration", "13"),
                row(2, "Experiment #3: Energy Loss in Pipe Fittings", "24"),
                row(3, "References", "101"),
            ],
            num_rows: 4,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
        let texts: Vec<String> = result
            .iter()
            .filter_map(|e| match e {
                ContentElement::TextLine(line) => Some(line.value()),
                _ => None,
            })
            .collect();
        assert!(texts
            .iter()
            .any(|t| t.contains("Experiment #1: Hydrostatic Pressure 3")));
        assert!(texts.iter().any(|t| t.contains("References 101")));
    }

    #[test]
    fn test_filter_suspicious_brochure_card_table() {
        let mut rows = Vec::new();
        let labels = [
            ["Our Purpose", "Our Mission", "What We Do"],
            [
                "Making AI Beneficial",
                "Easy-to-apply AI, Everywhere",
                "Providing easy-to-use AI solutions",
            ],
        ];

        for (row_number, texts) in labels.into_iter().enumerate() {
            let top = 300.0 - row_number as f64 * 90.0;
            let bottom = top - 80.0;
            let mut cells = Vec::new();
            for (col_number, text) in texts.into_iter().enumerate() {
                let left = 40.0 + col_number as f64 * 180.0;
                let right = left + 160.0;
                cells.push(TableBorderCell {
                    bbox: BoundingBox::new(Some(1), left, bottom, right, top),
                    index: None,
                    level: None,
                    row_number,
                    col_number,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(text, 16.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                });
            }
            rows.push(TableBorderRow {
                bbox: BoundingBox::new(Some(1), 40.0, bottom, 560.0, top),
                index: None,
                level: None,
                row_number,
                cells,
                semantic_type: None,
            });
        }

        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 40.0, 120.0, 560.0, 300.0),
            index: None,
            level: None,
            x_coordinates: vec![40.0, 220.0, 400.0, 560.0],
            x_widths: vec![0.5; 4],
            y_coordinates: vec![300.0, 210.0, 120.0],
            y_widths: vec![0.5; 3],
            rows,
            num_rows: 2,
            num_columns: 3,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert_eq!(result.len(), 6);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
    }

    #[test]
    fn test_filter_suspicious_single_column_prose_table() {
        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 40.0, 120.0, 560.0, 240.0),
            index: None,
            level: None,
            x_coordinates: vec![40.0, 560.0],
            x_widths: vec![0.0, 0.0],
            y_coordinates: vec![240.0, 120.0],
            y_widths: vec![0.0, 0.0],
            rows: vec![TableBorderRow {
                bbox: BoundingBox::new(Some(1), 40.0, 120.0, 560.0, 240.0),
                index: None,
                level: None,
                row_number: 0,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 40.0, 120.0, 560.0, 240.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(
                        "In this context, we are talking about fact-checking that is done before a source is published.",
                        11.0,
                    )],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            }],
            num_rows: 1,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
    }

    #[test]
    fn test_filter_suspicious_single_column_list_table_kept() {
        let rows = (0..4)
            .map(|row_number| TableBorderRow {
                bbox: BoundingBox::new(
                    Some(1),
                    80.0,
                    300.0 - row_number as f64 * 40.0,
                    320.0,
                    330.0 - row_number as f64 * 40.0,
                ),
                index: None,
                level: None,
                row_number,
                cells: vec![TableBorderCell {
                    bbox: BoundingBox::new(
                        Some(1),
                        80.0,
                        300.0 - row_number as f64 * 40.0,
                        320.0,
                        330.0 - row_number as f64 * 40.0,
                    ),
                    index: None,
                    level: None,
                    row_number,
                    col_number: 0,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(
                        &format!("#{}: Circular economy", row_number + 1),
                        11.0,
                    )],
                    contents: Vec::new(),
                    semantic_type: None,
                }],
                semantic_type: None,
            })
            .collect();

        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 80.0, 140.0, 320.0, 330.0),
            index: None,
            level: None,
            x_coordinates: vec![80.0, 320.0],
            x_widths: vec![0.0, 0.0],
            y_coordinates: vec![330.0, 290.0, 250.0, 210.0, 170.0],
            y_widths: vec![0.0; 5],
            rows,
            num_rows: 4,
            num_columns: 1,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(matches!(
            result.first(),
            Some(ContentElement::TableBorder(_))
        ));
    }

    #[test]
    fn test_filter_suspicious_two_row_marketing_grid() {
        let mut rows = Vec::new();
        let labels = [
            ["Our Purpose", "Our Mission", "What We Do"],
            [
                "Making AI Beneficial",
                "Easy-to-apply AI Everywhere",
                "Providing the world's best easy-to-use AI solutions",
            ],
        ];

        for (row_number, texts) in labels.into_iter().enumerate() {
            let top = 320.0 - row_number as f64 * 90.0;
            let bottom = top - 80.0;
            let mut cells = Vec::new();
            for (col_number, text) in texts.into_iter().enumerate() {
                let left = 40.0 + col_number as f64 * 180.0;
                let right = left + 160.0;
                cells.push(TableBorderCell {
                    bbox: BoundingBox::new(Some(1), left, bottom, right, top),
                    index: None,
                    level: None,
                    row_number,
                    col_number,
                    row_span: 1,
                    col_span: 1,
                    content: vec![make_token(text, 15.0)],
                    contents: Vec::new(),
                    semantic_type: None,
                });
            }
            rows.push(TableBorderRow {
                bbox: BoundingBox::new(Some(1), 40.0, bottom, 560.0, top),
                index: None,
                level: None,
                row_number,
                cells,
                semantic_type: None,
            });
        }

        let table = TableBorder {
            bbox: BoundingBox::new(Some(1), 40.0, 140.0, 560.0, 320.0),
            index: None,
            level: None,
            x_coordinates: vec![40.0, 220.0, 400.0, 560.0],
            x_widths: vec![0.0; 4],
            y_coordinates: vec![320.0, 230.0, 140.0],
            y_widths: vec![0.0; 3],
            rows,
            num_rows: 2,
            num_columns: 3,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        };

        let result = filter_suspicious_tables(vec![ContentElement::TableBorder(table)]);
        assert!(result
            .iter()
            .all(|e| matches!(e, ContentElement::TextLine(_))));
    }
}
