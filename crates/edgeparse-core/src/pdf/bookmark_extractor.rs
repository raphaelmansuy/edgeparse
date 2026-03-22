//! PDF bookmark/outline extraction.
//!
//! Reads the document outline (bookmarks) from the /Outlines dictionary,
//! producing a tree of `Bookmark` nodes with titles and nesting levels.

use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A single bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    /// Display title
    pub title: String,
    /// Nesting level (0 = top-level)
    pub level: u32,
    /// Destination page number (1-based, if resolvable)
    pub page_number: Option<u32>,
    /// Child bookmarks
    pub children: Vec<Bookmark>,
}

/// Extract the document outline as a flat list of bookmarks with levels.
pub fn extract_bookmarks(doc: &Document) -> Vec<Bookmark> {
    let catalog = match doc.catalog() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let outlines_obj = match catalog.get(b"Outlines") {
        Ok(obj) => resolve(doc, obj),
        Err(_) => return Vec::new(),
    };

    let outlines_dict = match outlines_obj.as_dict() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    // Get the first child
    let first_ref = match outlines_dict.get(b"First") {
        Ok(obj) => match obj {
            Object::Reference(id) => *id,
            _ => return Vec::new(),
        },
        Err(_) => return Vec::new(),
    };

    // Build page number lookup
    let page_ids = doc.get_pages();

    let mut visited = HashSet::new();
    read_outline_items(doc, first_ref, 0, &page_ids, &mut visited)
}

/// Recursively read outline items following /First, /Next, /Last links.
fn read_outline_items(
    doc: &Document,
    first_id: ObjectId,
    level: u32,
    page_ids: &std::collections::BTreeMap<u32, ObjectId>,
    visited: &mut HashSet<ObjectId>,
) -> Vec<Bookmark> {
    let mut bookmarks = Vec::new();
    let mut current_id = Some(first_id);

    while let Some(obj_id) = current_id {
        // Prevent infinite loops from malformed PDFs
        if !visited.insert(obj_id) {
            break;
        }

        let dict = match doc.get_object(obj_id).and_then(|o| o.as_dict()) {
            Ok(d) => d,
            Err(_) => break,
        };

        // Get title
        let title = match dict.get(b"Title") {
            Ok(Object::String(bytes, _)) => String::from_utf8_lossy(bytes).to_string(),
            _ => String::new(),
        };

        // Resolve destination page
        let page_number = resolve_bookmark_page(doc, dict, page_ids);

        // Recurse into children
        let children = match dict.get(b"First") {
            Ok(Object::Reference(child_id)) => {
                read_outline_items(doc, *child_id, level + 1, page_ids, visited)
            }
            _ => Vec::new(),
        };

        bookmarks.push(Bookmark {
            title,
            level,
            page_number,
            children,
        });

        // Move to next sibling
        current_id = match dict.get(b"Next") {
            Ok(Object::Reference(next_id)) => Some(*next_id),
            _ => None,
        };
    }

    bookmarks
}

/// Resolve the page number from a bookmark's /Dest or /A entry.
fn resolve_bookmark_page(
    doc: &Document,
    dict: &lopdf::Dictionary,
    page_ids: &std::collections::BTreeMap<u32, ObjectId>,
) -> Option<u32> {
    // Try /Dest first
    if let Ok(dest) = dict.get(b"Dest") {
        return page_from_dest(doc, dest, page_ids);
    }

    // Try /A (action) → /D (destination)
    if let Ok(action_obj) = dict.get(b"A") {
        let action = resolve(doc, action_obj);
        if let Ok(action_dict) = action.as_dict() {
            if let Ok(dest) = action_dict.get(b"D") {
                return page_from_dest(doc, dest, page_ids);
            }
        }
    }

    None
}

/// Extract page number from a destination object.
fn page_from_dest(
    doc: &Document,
    dest: &Object,
    page_ids: &std::collections::BTreeMap<u32, ObjectId>,
) -> Option<u32> {
    let dest = resolve(doc, dest);

    // Dest can be an array [page_ref, /type, ...] or a name ref
    let arr = match dest.as_array() {
        Ok(a) if !a.is_empty() => a,
        _ => return None,
    };

    // First element should be a page reference
    let page_ref = match &arr[0] {
        Object::Reference(id) => *id,
        _ => return None,
    };

    // Find matching page number
    page_ids
        .iter()
        .find(|(_, id)| **id == page_ref)
        .map(|(num, _)| *num)
}

/// Resolve an indirect reference.
fn resolve<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).unwrap_or(obj),
        _ => obj,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document_no_bookmarks() {
        let doc = Document::new();
        let bookmarks = extract_bookmarks(&doc);
        assert!(bookmarks.is_empty());
    }

    #[test]
    fn test_bookmark_struct() {
        let bm = Bookmark {
            title: "Chapter 1".to_string(),
            level: 0,
            page_number: Some(1),
            children: vec![Bookmark {
                title: "Section 1.1".to_string(),
                level: 1,
                page_number: Some(3),
                children: vec![],
            }],
        };
        assert_eq!(bm.title, "Chapter 1");
        assert_eq!(bm.children.len(), 1);
        assert_eq!(bm.children[0].level, 1);
    }

    #[test]
    fn test_visited_prevents_infinite_loop() {
        let mut visited = HashSet::new();
        let id = (1, 0);
        assert!(visited.insert(id));
        assert!(!visited.insert(id)); // second insert returns false
    }
}
