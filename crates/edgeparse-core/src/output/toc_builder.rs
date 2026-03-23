//! TOC (Table of Contents) builder — builds a structured table of contents
//! from headings detected in the document, suitable for output rendering.

use crate::models::content::ContentElement;

/// A single TOC entry with hierarchical nesting.
#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    /// Heading text.
    pub title: String,
    /// Heading level (1 = top-level, 2 = section, etc.).
    pub level: u32,
    /// Page number where the heading appears (1-based).
    pub page_number: u32,
    /// Child entries (sub-headings).
    pub children: Vec<TocEntry>,
}

/// A complete table of contents.
#[derive(Debug, Clone, Default)]
pub struct TableOfContents {
    /// Top-level entries.
    pub entries: Vec<TocEntry>,
}

impl TableOfContents {
    /// Build a TOC from document pages by extracting headings.
    pub fn from_pages(pages: &[Vec<ContentElement>]) -> Self {
        let mut flat: Vec<TocEntry> = Vec::new();

        for (page_idx, page) in pages.iter().enumerate() {
            let page_num = (page_idx + 1) as u32;
            for elem in page {
                if let Some(entry) = extract_heading(elem, page_num) {
                    flat.push(entry);
                }
            }
        }

        Self {
            entries: nest_entries(flat),
        }
    }

    /// Total number of entries (including nested).
    pub fn total_entries(&self) -> usize {
        count_entries(&self.entries)
    }

    /// Render the TOC as a markdown string.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        render_markdown(&self.entries, &mut out);
        out
    }

    /// Render the TOC as an HTML unordered list.
    pub fn to_html(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        render_html(&self.entries, &mut out, 0);
        out
    }

    /// Whether the TOC is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Extract heading info from a content element.
fn extract_heading(elem: &ContentElement, page_number: u32) -> Option<TocEntry> {
    match elem {
        ContentElement::Heading(h) => {
            let level = h.heading_level.unwrap_or(1);
            let title = h.base.base.value().trim().to_string();
            if title.is_empty() {
                return None;
            }
            Some(TocEntry {
                title,
                level,
                page_number,
                children: Vec::new(),
            })
        }
        _ => None,
    }
}

/// Nest flat entries into a tree based on levels.
/// Each entry's children are subsequent entries with a higher level number (lower priority).
fn nest_entries(flat: Vec<TocEntry>) -> Vec<TocEntry> {
    if flat.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<TocEntry> = Vec::new();
    let mut stack: Vec<TocEntry> = Vec::new();

    for entry in flat {
        // Pop entries from stack that are at the same or deeper level
        while let Some(top) = stack.last() {
            if top.level >= entry.level {
                let popped = stack.pop().unwrap();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(popped);
                } else {
                    result.push(popped);
                }
            } else {
                break;
            }
        }
        stack.push(entry);
    }

    // Flush remaining stack
    while let Some(popped) = stack.pop() {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(popped);
        } else {
            result.push(popped);
        }
    }

    result
}

fn count_entries(entries: &[TocEntry]) -> usize {
    entries.iter().map(|e| 1 + count_entries(&e.children)).sum()
}

fn render_markdown(entries: &[TocEntry], out: &mut String) {
    for entry in entries {
        let indent = "  ".repeat((entry.level - 1) as usize);
        out.push_str(&format!(
            "{}- {} (p. {})\n",
            indent, entry.title, entry.page_number
        ));
        render_markdown(&entry.children, out);
    }
}

fn render_html(entries: &[TocEntry], out: &mut String, depth: usize) {
    let indent = "  ".repeat(depth);
    out.push_str(&format!("{}<ul>\n", indent));
    for entry in entries {
        out.push_str(&format!(
            "{}  <li>{} (p. {})",
            indent, entry.title, entry.page_number
        ));
        if !entry.children.is_empty() {
            out.push('\n');
            render_html(&entry.children, out, depth + 2);
            out.push_str(&format!("{}  </li>\n", indent));
        } else {
            out.push_str("</li>\n");
        }
    }
    out.push_str(&format!("{}</ul>\n", indent));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(title: &str, level: u32, page: u32) -> TocEntry {
        TocEntry {
            title: title.to_string(),
            level,
            page_number: page,
            children: Vec::new(),
        }
    }

    #[test]
    fn test_nest_flat_entries() {
        let flat = vec![
            make_entry("Chapter 1", 1, 1),
            make_entry("Section 1.1", 2, 2),
            make_entry("Section 1.2", 2, 3),
            make_entry("Chapter 2", 1, 5),
        ];
        let nested = nest_entries(flat);
        assert_eq!(nested.len(), 2);
        assert_eq!(nested[0].title, "Chapter 1");
        assert_eq!(nested[0].children.len(), 2);
        assert_eq!(nested[0].children[0].title, "Section 1.1");
        assert_eq!(nested[1].title, "Chapter 2");
        assert!(nested[1].children.is_empty());
    }

    #[test]
    fn test_total_entries() {
        let toc = TableOfContents {
            entries: vec![TocEntry {
                title: "Ch1".to_string(),
                level: 1,
                page_number: 1,
                children: vec![make_entry("S1.1", 2, 2), make_entry("S1.2", 2, 3)],
            }],
        };
        assert_eq!(toc.total_entries(), 3);
    }

    #[test]
    fn test_to_markdown() {
        let toc = TableOfContents {
            entries: vec![TocEntry {
                title: "Intro".to_string(),
                level: 1,
                page_number: 1,
                children: vec![make_entry("Overview", 2, 2)],
            }],
        };
        let md = toc.to_markdown();
        assert!(md.contains("- Intro (p. 1)"));
        assert!(md.contains("  - Overview (p. 2)"));
    }

    #[test]
    fn test_to_html() {
        let toc = TableOfContents {
            entries: vec![make_entry("Title", 1, 1)],
        };
        let html = toc.to_html();
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>Title (p. 1)</li>"));
        assert!(html.contains("</ul>"));
    }

    #[test]
    fn test_empty_toc() {
        let toc = TableOfContents::default();
        assert!(toc.is_empty());
        assert_eq!(toc.total_entries(), 0);
        assert_eq!(toc.to_markdown(), "");
        assert_eq!(toc.to_html(), "");
    }
}
