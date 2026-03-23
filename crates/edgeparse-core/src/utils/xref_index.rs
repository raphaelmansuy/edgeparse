//! Cross-reference builder — creates an index mapping element IDs to
//! their locations and relationships across the document.

use std::collections::HashMap;

use crate::models::content::ContentElement;

/// A cross-reference entry for a document element.
#[derive(Debug, Clone)]
pub struct XRefEntry {
    /// Element type tag.
    pub element_type: String,
    /// Element index in its parent container.
    pub element_index: usize,
    /// Page number (1-based, if known).
    pub page_number: Option<u32>,
    /// Heading text (if this is a heading element).
    pub heading_text: Option<String>,
    /// Heading level (if this is a heading element).
    pub heading_level: Option<u32>,
}

/// A cross-reference index for the entire document.
#[derive(Debug, Clone)]
pub struct CrossReferenceIndex {
    /// Map from element ID to its cross-reference entry.
    entries: HashMap<usize, XRefEntry>,
    /// Map from heading text (lowercase) to element ID.
    heading_index: HashMap<String, Vec<usize>>,
    /// Map from page number to element IDs on that page.
    page_index: HashMap<u32, Vec<usize>>,
}

impl CrossReferenceIndex {
    /// Build a cross-reference index from document pages.
    pub fn from_pages(pages: &[Vec<ContentElement>]) -> Self {
        let mut entries = HashMap::new();
        let mut heading_index: HashMap<String, Vec<usize>> = HashMap::new();
        let mut page_index: HashMap<u32, Vec<usize>> = HashMap::new();
        let mut global_idx = 0usize;

        for (page_idx, page) in pages.iter().enumerate() {
            let page_num = (page_idx + 1) as u32;

            for elem in page {
                let entry = build_entry(elem, global_idx, page_num);

                if let Some(ref heading) = entry.heading_text {
                    heading_index
                        .entry(heading.to_lowercase())
                        .or_default()
                        .push(global_idx);
                }

                page_index.entry(page_num).or_default().push(global_idx);
                entries.insert(global_idx, entry);
                global_idx += 1;
            }
        }

        Self {
            entries,
            heading_index,
            page_index,
        }
    }

    /// Look up an element by its global index.
    pub fn get(&self, id: usize) -> Option<&XRefEntry> {
        self.entries.get(&id)
    }

    /// Find elements by heading text (case-insensitive substring match).
    pub fn find_by_heading(&self, query: &str) -> Vec<(usize, &XRefEntry)> {
        let query_lower = query.to_lowercase();
        self.heading_index
            .iter()
            .filter(|(key, _)| key.contains(&query_lower))
            .flat_map(|(_, ids)| {
                ids.iter()
                    .filter_map(|id| self.entries.get(id).map(|e| (*id, e)))
            })
            .collect()
    }

    /// Get all elements on a given page.
    pub fn elements_on_page(&self, page_number: u32) -> Vec<(usize, &XRefEntry)> {
        self.page_index
            .get(&page_number)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.entries.get(id).map(|e| (*id, e)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Total number of indexed elements.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of headings indexed.
    pub fn heading_count(&self) -> usize {
        self.heading_index.values().map(|v| v.len()).sum()
    }

    /// Number of pages with indexed elements.
    pub fn page_count(&self) -> usize {
        self.page_index.len()
    }
}

fn build_entry(elem: &ContentElement, index: usize, page_number: u32) -> XRefEntry {
    let element_type = element_type_name(elem).to_string();
    let (heading_text, heading_level) = extract_heading_info(elem);

    XRefEntry {
        element_type,
        element_index: index,
        page_number: Some(page_number),
        heading_text,
        heading_level,
    }
}

fn extract_heading_info(elem: &ContentElement) -> (Option<String>, Option<u32>) {
    match elem {
        ContentElement::Heading(h) => {
            let text = h.base.base.value().trim().to_string();
            let level = h.heading_level;
            (if text.is_empty() { None } else { Some(text) }, level)
        }
        ContentElement::NumberHeading(nh) => {
            let text = nh.base.base.base.value().trim().to_string();
            let level = nh.base.heading_level;
            (if text.is_empty() { None } else { Some(text) }, level)
        }
        _ => (None, None),
    }
}

fn element_type_name(elem: &ContentElement) -> &'static str {
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
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn make_text(text: &str) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(None, 0.0, 0.0, 100.0, 10.0),
            font_name: "F".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000".to_string(),
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
    fn test_build_index() {
        let pages = vec![
            vec![make_text("Hello"), make_text("World")],
            vec![make_text("Page 2")],
        ];
        let index = CrossReferenceIndex::from_pages(&pages);
        assert_eq!(index.len(), 3);
        assert_eq!(index.page_count(), 2);
    }

    #[test]
    fn test_get_element() {
        let pages = vec![vec![make_text("Test")]];
        let index = CrossReferenceIndex::from_pages(&pages);
        let entry = index.get(0).unwrap();
        assert_eq!(entry.element_type, "TextChunk");
        assert_eq!(entry.page_number, Some(1));
    }

    #[test]
    fn test_elements_on_page() {
        let pages = vec![vec![make_text("A"), make_text("B")], vec![make_text("C")]];
        let index = CrossReferenceIndex::from_pages(&pages);
        assert_eq!(index.elements_on_page(1).len(), 2);
        assert_eq!(index.elements_on_page(2).len(), 1);
        assert_eq!(index.elements_on_page(3).len(), 0);
    }

    #[test]
    fn test_empty_index() {
        let pages: Vec<Vec<ContentElement>> = vec![];
        let index = CrossReferenceIndex::from_pages(&pages);
        assert!(index.is_empty());
        assert_eq!(index.heading_count(), 0);
    }
}
