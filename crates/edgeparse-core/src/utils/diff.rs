//! Document diff — compares two PdfDocument instances to detect structural changes.
//!
//! Useful for regression testing and verifying parsing consistency.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;

/// Type of change detected between two documents.
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeKind {
    /// An element was added in the new document.
    Added,
    /// An element was removed from the old document.
    Removed,
    /// An element was modified (same position, different content).
    Modified,
}

/// A single detected change between two documents.
#[derive(Debug, Clone)]
pub struct Change {
    /// Type of change.
    pub kind: ChangeKind,
    /// Element index in the respective document's kids list.
    pub index: usize,
    /// Description of the change.
    pub description: String,
}

/// Result of comparing two documents.
#[derive(Debug, Clone)]
pub struct DiffResult {
    /// List of detected changes.
    pub changes: Vec<Change>,
    /// Whether metadata fields changed.
    pub metadata_changed: bool,
    /// Whether page count changed.
    pub page_count_changed: bool,
}

impl DiffResult {
    /// Whether the two documents are structurally identical.
    pub fn is_identical(&self) -> bool {
        self.changes.is_empty() && !self.metadata_changed && !self.page_count_changed
    }

    /// Number of changes detected.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Summary of the diff.
    pub fn summary(&self) -> String {
        if self.is_identical() {
            return "Documents are identical.".to_string();
        }
        let added = self
            .changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Added)
            .count();
        let removed = self
            .changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Removed)
            .count();
        let modified = self
            .changes
            .iter()
            .filter(|c| c.kind == ChangeKind::Modified)
            .count();
        format!(
            "{} change(s): {} added, {} removed, {} modified{}{}",
            self.changes.len(),
            added,
            removed,
            modified,
            if self.metadata_changed {
                ", metadata changed"
            } else {
                ""
            },
            if self.page_count_changed {
                ", page count changed"
            } else {
                ""
            },
        )
    }
}

/// Compare two PdfDocuments and produce a diff.
pub fn diff_documents(old: &PdfDocument, new: &PdfDocument) -> DiffResult {
    let mut changes = Vec::new();

    let metadata_changed = old.author != new.author
        || old.title != new.title
        || old.creation_date != new.creation_date
        || old.producer != new.producer
        || old.creator != new.creator
        || old.subject != new.subject
        || old.keywords != new.keywords;

    let page_count_changed = old.number_of_pages != new.number_of_pages;

    // Compare elements using a simple sequential diff.
    let max_len = old.kids.len().max(new.kids.len());
    for i in 0..max_len {
        match (old.kids.get(i), new.kids.get(i)) {
            (Some(old_elem), Some(new_elem)) => {
                if !elements_equal(old_elem, new_elem) {
                    changes.push(Change {
                        kind: ChangeKind::Modified,
                        index: i,
                        description: format!(
                            "Element {} changed: {:?} -> {:?}",
                            i,
                            element_tag(old_elem),
                            element_tag(new_elem)
                        ),
                    });
                }
            }
            (Some(_), None) => {
                changes.push(Change {
                    kind: ChangeKind::Removed,
                    index: i,
                    description: format!("Element {} removed", i),
                });
            }
            (None, Some(_)) => {
                changes.push(Change {
                    kind: ChangeKind::Added,
                    index: i,
                    description: format!("Element {} added", i),
                });
            }
            (None, None) => unreachable!(),
        }
    }

    DiffResult {
        changes,
        metadata_changed,
        page_count_changed,
    }
}

/// Simple equality check for two content elements using their debug representation.
fn elements_equal(a: &ContentElement, b: &ContentElement) -> bool {
    // Compare using variant tag and bounding box as a proxy for equality.
    // Full deep comparison would require PartialEq on all subtypes.
    element_tag(a) == element_tag(b) && a.bbox() == b.bbox()
}

/// Get a string tag for a content element variant.
fn element_tag(elem: &ContentElement) -> &'static str {
    match elem {
        ContentElement::TextChunk(_) => "TextChunk",
        ContentElement::TextLine(_) => "TextLine",
        ContentElement::TextBlock(_) => "TextBlock",
        ContentElement::Paragraph(_) => "Paragraph",
        ContentElement::Heading(_) => "Heading",
        ContentElement::NumberHeading(_) => "NumberHeading",
        ContentElement::Table(_) => "Table",
        ContentElement::Figure(_) => "Figure",
        ContentElement::Formula(_) => "Formula",
        ContentElement::Picture(_) => "Picture",
        ContentElement::Caption(_) => "Caption",
        ContentElement::HeaderFooter(_) => "HeaderFooter",
        ContentElement::Image(_) => "Image",
        ContentElement::Line(_) => "Line",
        ContentElement::LineArt(_) => "LineArt",
        ContentElement::List(_) => "List",
        ContentElement::TableBorder(_) => "TableBorder",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::document::PdfDocument;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text_chunk(text: &str) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(None, 0.0, 0.0, 100.0, 10.0),
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
        })
    }

    #[test]
    fn test_identical_documents() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let result = diff_documents(&doc, &doc);
        assert!(result.is_identical());
        assert_eq!(result.change_count(), 0);
    }

    #[test]
    fn test_metadata_changed() {
        let mut old = PdfDocument::new("test.pdf".to_string());
        let mut new = PdfDocument::new("test.pdf".to_string());
        old.author = Some("Alice".to_string());
        new.author = Some("Bob".to_string());
        let result = diff_documents(&old, &new);
        assert!(result.metadata_changed);
        assert!(!result.page_count_changed);
    }

    #[test]
    fn test_element_added() {
        let old = PdfDocument::new("test.pdf".to_string());
        let mut new = PdfDocument::new("test.pdf".to_string());
        new.kids.push(make_text_chunk("hello"));
        let result = diff_documents(&old, &new);
        assert_eq!(result.change_count(), 1);
        assert_eq!(result.changes[0].kind, ChangeKind::Added);
    }

    #[test]
    fn test_element_removed() {
        let mut old = PdfDocument::new("test.pdf".to_string());
        old.kids.push(make_text_chunk("hello"));
        let new = PdfDocument::new("test.pdf".to_string());
        let result = diff_documents(&old, &new);
        assert_eq!(result.change_count(), 1);
        assert_eq!(result.changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn test_summary() {
        let old = PdfDocument::new("test.pdf".to_string());
        let result = diff_documents(&old, &old);
        assert_eq!(result.summary(), "Documents are identical.");
    }
}
