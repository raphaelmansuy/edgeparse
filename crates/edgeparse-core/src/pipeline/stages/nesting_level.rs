//! Stage 17: Nesting Level Assignment
//!
//! Assigns nesting depth levels to content elements based on their type
//! and horizontal position (indentation). Uses a stack to track visual
//! nesting context.

use crate::models::content::ContentElement;

/// Horizontal tolerance factor (relative to typical font size) for same-level matching.
const X_TOLERANCE_FACTOR: f64 = 0.3;

/// Default font size if none available.
const DEFAULT_FONT_SIZE: f64 = 12.0;

/// Assign nesting levels to all elements across pages.
pub fn assign_nesting_levels(pages: &mut [Vec<ContentElement>]) {
    let mut stack: Vec<LevelEntry> = Vec::new();

    for page in pages.iter_mut() {
        for elem in page.iter_mut() {
            let entry = LevelEntry::from_element(elem);
            let level = find_or_push_level(&mut stack, &entry);
            let level_str = format!("{level}");
            set_level(elem, &level_str);
        }
    }
}

/// A nesting context entry on the stack.
#[derive(Debug, Clone)]
struct LevelEntry {
    kind: EntryKind,
    left_x: f64,
    font_size: f64,
}

#[derive(Debug, Clone, PartialEq)]
enum EntryKind {
    List,
    Table,
    Heading,
    Paragraph,
    Other,
}

impl LevelEntry {
    fn from_element(elem: &ContentElement) -> Self {
        let bbox = elem.bbox();
        let left_x = bbox.left_x;
        let (kind, font_size) = match elem {
            ContentElement::List(_) => (EntryKind::List, DEFAULT_FONT_SIZE),
            ContentElement::TableBorder(_) | ContentElement::Table(_) => {
                (EntryKind::Table, DEFAULT_FONT_SIZE)
            }
            ContentElement::Heading(h) => (
                EntryKind::Heading,
                h.base.base.font_size.unwrap_or(DEFAULT_FONT_SIZE),
            ),
            ContentElement::Paragraph(p) => (
                EntryKind::Paragraph,
                p.base.font_size.unwrap_or(DEFAULT_FONT_SIZE),
            ),
            _ => (EntryKind::Other, DEFAULT_FONT_SIZE),
        };
        Self {
            kind,
            left_x,
            font_size,
        }
    }

    fn matches(&self, other: &LevelEntry) -> bool {
        if self.kind != other.kind {
            return false;
        }
        // Tables never match — each gets unique level
        if self.kind == EntryKind::Table {
            return false;
        }
        let tolerance = self.font_size * X_TOLERANCE_FACTOR;
        (self.left_x - other.left_x).abs() <= tolerance
    }
}

/// Find matching level in the stack or create a new one.
fn find_or_push_level(stack: &mut Vec<LevelEntry>, entry: &LevelEntry) -> usize {
    // Search stack from top (most recent) to bottom
    for i in (0..stack.len()).rev() {
        if stack[i].matches(entry) {
            // Pop back to this level
            stack.truncate(i + 1);
            return i + 1;
        }
    }
    // No match — new deeper level
    stack.push(entry.clone());
    stack.len()
}

/// Set the level field on a content element.
fn set_level(elem: &mut ContentElement, level: &str) {
    match elem {
        ContentElement::TextChunk(c) => c.level = Some(level.to_string()),
        ContentElement::TextLine(l) => l.level = Some(level.to_string()),
        ContentElement::TextBlock(b) => b.level = Some(level.to_string()),
        ContentElement::Image(i) => i.level = Some(level.to_string()),
        ContentElement::Line(l) => l.level = Some(level.to_string()),
        ContentElement::LineArt(a) => a.level = Some(level.to_string()),
        ContentElement::TableBorder(t) => t.level = Some(level.to_string()),
        ContentElement::List(l) => l.level = Some(level.to_string()),
        ContentElement::Paragraph(p) => p.base.level = Some(level.to_string()),
        ContentElement::Heading(h) => h.base.base.level = Some(level.to_string()),
        ContentElement::HeaderFooter(h) => h.level = Some(level.to_string()),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::enums::SemanticType;
    use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
    use crate::models::text::{TextBlock, TextColumn};

    fn para_at_x(x: f64) -> ContentElement {
        let block = TextBlock {
            bbox: BoundingBox::new(Some(1), x, 700.0, x + 200.0, 720.0),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 702.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: Vec::new(),
            has_start_line: false,
            has_end_line: false,
            text_alignment: None,
        };
        let col = TextColumn {
            bbox: BoundingBox::new(Some(1), x, 700.0, x + 200.0, 720.0),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 702.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        let node = SemanticTextNode {
            bbox: BoundingBox::new(Some(1), x, 700.0, x + 200.0, 720.0),
            index: None,
            level: None,
            semantic_type: SemanticType::Paragraph,
            correct_semantic_score: None,
            columns: vec![col],
            font_weight: None,
            font_size: Some(12.0),
            text_color: None,
            italic_angle: None,
            font_name: None,
            text_format: None,
            max_font_size: None,
            background_color: None,
            is_hidden_text: false,
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: node,
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    #[test]
    fn test_same_indent_same_level() {
        let mut pages = vec![vec![para_at_x(72.0), para_at_x(72.0)]];
        assign_nesting_levels(&mut pages);
        // Both at same x → level 1
        assert_elem_level(&pages[0][0], "1");
        assert_elem_level(&pages[0][1], "1");
    }

    #[test]
    fn test_deeper_indent_higher_level() {
        let mut pages = vec![vec![para_at_x(72.0), para_at_x(108.0), para_at_x(72.0)]];
        assign_nesting_levels(&mut pages);
        assert_elem_level(&pages[0][0], "1");
        assert_elem_level(&pages[0][1], "2"); // indented
        assert_elem_level(&pages[0][2], "1"); // back to original
    }

    #[test]
    fn test_tables_always_unique() {
        use crate::models::table::TableBorder;
        let t1 = ContentElement::TableBorder(TableBorder {
            bbox: BoundingBox::new(Some(1), 72.0, 600.0, 500.0, 700.0),
            index: None,
            level: None,
            x_coordinates: Vec::new(),
            x_widths: Vec::new(),
            y_coordinates: Vec::new(),
            y_widths: Vec::new(),
            rows: Vec::new(),
            num_rows: 0,
            num_columns: 0,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        });
        let t2 = t1.clone();
        let mut pages = vec![vec![t1, t2]];
        assign_nesting_levels(&mut pages);
        // Two tables at same x → should still get different levels
        assert_elem_level(&pages[0][0], "1");
        assert_elem_level(&pages[0][1], "2");
    }

    fn assert_elem_level(elem: &ContentElement, expected: &str) {
        match elem {
            ContentElement::Paragraph(p) => assert_eq!(p.base.level.as_deref(), Some(expected)),
            ContentElement::TableBorder(t) => assert_eq!(t.level.as_deref(), Some(expected)),
            _ => panic!("unexpected element type"),
        }
    }
}
