//! Structure tree extraction from tagged PDFs.
//!
//! Reads `/StructTreeRoot` and recursively walks the structure tree to build a
//! structured representation of the document's semantic tags.

use std::collections::{HashMap, HashSet};

use lopdf::{Document, Object, ObjectId};

/// Information about a structure-tree tag associated with an MCID.
#[derive(Debug, Clone)]
pub struct McidTagInfo {
    /// The semantic role: "heading", "paragraph", "table", "list", etc.
    pub role: &'static str,
    /// Heading level 1-6 for H/H1-H6 tags, None for non-heading tags.
    pub heading_level: Option<u32>,
    /// The raw structure type (e.g. "H1", "P", "Table").
    pub struct_type: String,
}

/// Map from (page_number, mcid) to structure tag information.
/// Used by the heading detector to identify tagged headings.
pub type McidMap = HashMap<(u32, i64), McidTagInfo>;

/// A node in the PDF structure tree.
#[derive(Debug, Clone)]
pub struct StructNode {
    /// Structure type (e.g., "Document", "P", "H1", "Table", "Span").
    pub struct_type: String,
    /// Actual text content (from /ActualText or referenced content streams).
    pub actual_text: Option<String>,
    /// Alternative text (from /Alt attribute).
    pub alt_text: Option<String>,
    /// Language tag (from /Lang attribute).
    pub lang: Option<String>,
    /// Child structure nodes.
    pub children: Vec<StructNode>,
    /// Page number this node belongs to (from /Pg reference).
    pub page_number: Option<u32>,
    /// Marked content ID (from /MCID within /K).
    pub mcid: Option<i64>,
}

impl StructNode {
    fn new(struct_type: String) -> Self {
        Self {
            struct_type,
            actual_text: None,
            alt_text: None,
            lang: None,
            children: Vec::new(),
            page_number: None,
            mcid: None,
        }
    }
}

/// Extract the structure tree from a PDF document.
///
/// Returns `None` if the document has no `/StructTreeRoot`.
pub fn extract_struct_tree(doc: &Document) -> Option<StructNode> {
    let catalog = doc.catalog().ok()?;
    let struct_tree_ref = catalog.get(b"StructTreeRoot").ok()?;
    let tree_dict = doc.dereference(struct_tree_ref).ok()?.1;
    let dict = tree_dict.as_dict().ok()?;

    let mut root = StructNode::new("StructTreeRoot".to_string());
    let mut visited = HashSet::new();

    // The root's children are in /K
    if let Ok(kids) = dict.get(b"K") {
        parse_k_entry(doc, kids, &mut root, &mut visited);
    }

    Some(root)
}

/// Build an MCID map from the structure tree.
///
/// Walks the structure tree and for each MCID leaf under a semantically-typed
/// ancestor (H1-H6, P, Table, etc.), records `(page_number, mcid) → McidTagInfo`.
/// Page numbers are inherited from ancestors if the MCID leaf doesn't have its own.
///
/// Also resolves the RoleMap: custom structure types (e.g. "Title" → "H1") are
/// mapped to standard tags before classification.
pub fn build_mcid_map(doc: &Document) -> McidMap {
    let tree = match extract_struct_tree(doc) {
        Some(t) => t,
        None => return HashMap::new(),
    };

    // Extract role map for custom tag resolution
    let role_map = extract_role_map(doc);

    let mut map = McidMap::new();
    walk_for_mcids(&tree, None, None, &role_map, &mut map);
    map
}

/// Extract the RoleMap from /StructTreeRoot.
/// Maps custom structure types to standard PDF tags (e.g. "Title" → "H1").
fn extract_role_map(doc: &Document) -> HashMap<String, String> {
    let mut role_map = HashMap::new();
    let Ok(catalog) = doc.catalog() else {
        return role_map;
    };
    let Ok(struct_tree_ref) = catalog.get(b"StructTreeRoot") else {
        return role_map;
    };
    let Ok((_, tree_obj)) = doc.dereference(struct_tree_ref) else {
        return role_map;
    };
    let Ok(dict) = tree_obj.as_dict() else {
        return role_map;
    };
    let Ok(rm_obj) = dict.get(b"RoleMap") else {
        return role_map;
    };
    let rm_obj = match rm_obj {
        Object::Reference(id) => match doc.get_object(*id) {
            Ok(o) => o.clone(),
            Err(_) => return role_map,
        },
        other => other.clone(),
    };
    if let Ok(rm_dict) = rm_obj.as_dict() {
        for (key, value) in rm_dict.iter() {
            let key_str = String::from_utf8_lossy(key).to_string();
            if let Object::Name(ref name) = value {
                let val_str = String::from_utf8_lossy(name).to_string();
                role_map.insert(key_str, val_str);
            }
        }
    }
    role_map
}

/// Resolve a structure type through the role map.
/// E.g. "Title" → "H1" if role map says so; "H1" stays "H1".
fn resolve_struct_type<'a>(raw_type: &'a str, role_map: &'a HashMap<String, String>) -> &'a str {
    match role_map.get(raw_type) {
        Some(mapped) => mapped.as_str(),
        None => raw_type,
    }
}

/// Parse heading level from a structure type tag.
fn heading_level_from_tag(tag: &str) -> Option<u32> {
    match tag {
        "H" => Some(1), // Generic heading defaults to level 1
        "H1" => Some(1),
        "H2" => Some(2),
        "H3" => Some(3),
        "H4" => Some(4),
        "H5" => Some(5),
        "H6" => Some(6),
        _ => None,
    }
}

/// Recursively walk the structure tree, recording MCIDs with their semantic info.
fn walk_for_mcids(
    node: &StructNode,
    parent_role: Option<(&'static str, Option<u32>, String)>,
    inherited_page: Option<u32>,
    role_map: &HashMap<String, String>,
    map: &mut McidMap,
) {
    let resolved_type = resolve_struct_type(&node.struct_type, role_map);
    let role = classify_struct_type(resolved_type);
    let level = heading_level_from_tag(resolved_type);

    // Determine the effective semantic context for MCID leaves.
    // For structural containers (document, section, non-structural), inherit parent.
    // For semantic elements (heading, paragraph, table, etc.), use this node.
    let current_context = if role != "document"
        && role != "section"
        && role != "non-structural"
        && role != "unknown"
        && node.struct_type != "StructTreeRoot"
        && node.struct_type != "MCID"
    {
        Some((role, level, resolved_type.to_string()))
    } else {
        parent_role.clone()
    };

    let page = node.page_number.or(inherited_page);

    // If this is an MCID leaf, record it
    if node.struct_type == "MCID" {
        if let (Some(mcid), Some(page_num)) = (node.mcid, page) {
            if let Some((role, ref heading_level, ref struct_type)) = current_context {
                map.insert(
                    (page_num, mcid),
                    McidTagInfo {
                        role,
                        heading_level: *heading_level,
                        struct_type: struct_type.clone(),
                    },
                );
            }
        }
    }

    // Recurse into children
    for child in &node.children {
        walk_for_mcids(child, current_context.clone(), page, role_map, map);
    }
}

/// Check if a document has a structure tree (is a tagged PDF).
pub fn is_tagged(doc: &Document) -> bool {
    let Ok(catalog) = doc.catalog() else {
        return false;
    };
    // Check MarkInfo/Marked flag
    if let Ok(mark_info_ref) = catalog.get(b"MarkInfo") {
        if let Ok((_, mark_info_obj)) = doc.dereference(mark_info_ref) {
            if let Ok(dict) = mark_info_obj.as_dict() {
                if let Ok(marked) = dict.get(b"Marked") {
                    if let Ok(b) = marked.as_bool() {
                        return b;
                    }
                }
            }
        }
    }
    // Fallback: check if StructTreeRoot exists
    catalog.get(b"StructTreeRoot").is_ok()
}

/// Map common PDF structure types to semantic roles.
pub fn classify_struct_type(tag: &str) -> &'static str {
    match tag {
        "Document" => "document",
        "Part" => "section",
        "Art" | "Sect" | "Div" => "section",
        "P" => "paragraph",
        "H" | "H1" | "H2" | "H3" | "H4" | "H5" | "H6" => "heading",
        "L" => "list",
        "LI" => "list-item",
        "Lbl" => "list-label",
        "LBody" => "list-body",
        "Table" => "table",
        "TR" => "table-row",
        "TH" => "table-header-cell",
        "TD" => "table-cell",
        "THead" => "table-head",
        "TBody" => "table-body",
        "TFoot" => "table-foot",
        "Figure" => "figure",
        "Formula" => "formula",
        "Form" => "form",
        "Span" => "span",
        "Link" => "link",
        "Note" => "note",
        "Reference" => "reference",
        "BibEntry" => "bibliography-entry",
        "Code" => "code",
        "BlockQuote" => "blockquote",
        "TOC" => "table-of-contents",
        "TOCI" => "toc-item",
        "Index" => "index",
        "Caption" => "caption",
        "NonStruct" => "non-structural",
        _ => "unknown",
    }
}

/// Parse the /K entry which can be an integer MCID, a dictionary (struct element),
/// or an array of those.
fn parse_k_entry(
    doc: &Document,
    k_obj: &Object,
    parent: &mut StructNode,
    visited: &mut HashSet<ObjectId>,
) {
    match k_obj {
        Object::Integer(mcid) => {
            // Leaf node: marked content identifier
            let mut leaf = StructNode::new("MCID".to_string());
            leaf.mcid = Some(*mcid);
            parent.children.push(leaf);
        }
        Object::Array(arr) => {
            for item in arr {
                parse_k_entry(doc, item, parent, visited);
            }
        }
        Object::Reference(obj_id) => {
            if !visited.insert(*obj_id) {
                return; // Cycle detected
            }
            if let Ok((_, resolved)) = doc.dereference(k_obj) {
                parse_k_entry(doc, resolved, parent, visited);
            }
        }
        Object::Dictionary(dict) => {
            if let Some(mcid_leaf) = parse_mcid_leaf(doc, dict) {
                parent.children.push(mcid_leaf);
            } else {
                parse_struct_element(doc, dict, parent, visited);
            }
        }
        _ => {}
    }
}

fn parse_mcid_leaf(doc: &Document, dict: &lopdf::Dictionary) -> Option<StructNode> {
    let mcid = match dict.get(b"MCID") {
        Ok(Object::Integer(n)) => *n,
        _ => return None,
    };

    let mut leaf = StructNode::new("MCID".to_string());
    leaf.mcid = Some(mcid);

    if let Ok(Object::Reference(pg_id)) = dict.get(b"Pg") {
        leaf.page_number = resolve_page_number(doc, *pg_id);
    }

    Some(leaf)
}

fn parse_struct_element(
    doc: &Document,
    dict: &lopdf::Dictionary,
    parent: &mut StructNode,
    visited: &mut HashSet<ObjectId>,
) {
    // Get type (/S entry)
    let struct_type = dict
        .get(b"S")
        .ok()
        .and_then(|o| match o {
            Object::Name(n) => String::from_utf8(n.clone()).ok(),
            _ => None,
        })
        .unwrap_or_else(|| "Unknown".to_string());

    let mut node = StructNode::new(struct_type);

    // /ActualText
    if let Ok(Object::String(text, _)) = dict.get(b"ActualText") {
        node.actual_text = String::from_utf8(text.clone()).ok();
    }

    // /Alt
    if let Ok(Object::String(alt, _)) = dict.get(b"Alt") {
        node.alt_text = String::from_utf8(alt.clone()).ok();
    }

    // /Lang
    if let Ok(Object::String(lang, _)) = dict.get(b"Lang") {
        node.lang = String::from_utf8(lang.clone()).ok();
    }

    // /Pg (page reference)
    if let Ok(Object::Reference(pg_id)) = dict.get(b"Pg") {
        node.page_number = resolve_page_number(doc, *pg_id);
    }

    // /K — recurse
    if let Ok(k) = dict.get(b"K") {
        parse_k_entry(doc, k, &mut node, visited);
    }

    parent.children.push(node);
}

fn resolve_page_number(doc: &Document, page_id: ObjectId) -> Option<u32> {
    let pages = doc.get_pages();
    for (&page_num, &obj_id) in &pages {
        if obj_id == page_id {
            return Some(page_num);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn test_classify_struct_types() {
        assert_eq!(classify_struct_type("P"), "paragraph");
        assert_eq!(classify_struct_type("H1"), "heading");
        assert_eq!(classify_struct_type("Table"), "table");
        assert_eq!(classify_struct_type("Figure"), "figure");
        assert_eq!(classify_struct_type("L"), "list");
        assert_eq!(classify_struct_type("LI"), "list-item");
        assert_eq!(classify_struct_type("TD"), "table-cell");
        assert_eq!(classify_struct_type("Unknown"), "unknown");
    }

    #[test]
    fn test_no_struct_tree_in_empty_doc() {
        let doc = Document::new();
        let tree = extract_struct_tree(&doc);
        assert!(tree.is_none());
    }

    #[test]
    fn test_is_tagged_empty_doc() {
        let doc = Document::new();
        assert!(!is_tagged(&doc));
    }

    #[test]
    fn test_struct_node_creation() {
        let node = StructNode::new("P".to_string());
        assert_eq!(node.struct_type, "P");
        assert!(node.children.is_empty());
        assert!(node.actual_text.is_none());
        assert!(node.mcid.is_none());
    }

    #[test]
    fn test_build_mcid_map_empty_doc() {
        let doc = Document::new();
        let map = build_mcid_map(&doc);
        assert!(map.is_empty());
    }

    #[test]
    fn test_heading_level_from_tag() {
        assert_eq!(heading_level_from_tag("H"), Some(1));
        assert_eq!(heading_level_from_tag("H1"), Some(1));
        assert_eq!(heading_level_from_tag("H2"), Some(2));
        assert_eq!(heading_level_from_tag("H6"), Some(6));
        assert_eq!(heading_level_from_tag("P"), None);
        assert_eq!(heading_level_from_tag("Table"), None);
    }

    #[test]
    fn test_resolve_struct_type_with_role_map() {
        let mut role_map = HashMap::new();
        role_map.insert("Title".to_string(), "H1".to_string());
        role_map.insert("Subtitle".to_string(), "H2".to_string());

        assert_eq!(resolve_struct_type("Title", &role_map), "H1");
        assert_eq!(resolve_struct_type("Subtitle", &role_map), "H2");
        assert_eq!(resolve_struct_type("P", &role_map), "P");
        assert_eq!(resolve_struct_type("H3", &role_map), "H3");
    }

    #[test]
    fn test_walk_for_mcids_synthetic_tree() {
        // Build a synthetic structure tree:
        //   Document
        //     H1 (page=1)
        //       MCID(0)
        //     P  (page=1)
        //       MCID(1)
        //     H2 (page=2)
        //       MCID(0)
        let mut root = StructNode::new("Document".to_string());

        let mut h1 = StructNode::new("H1".to_string());
        h1.page_number = Some(1);
        let mut mcid0 = StructNode::new("MCID".to_string());
        mcid0.mcid = Some(0);
        h1.children.push(mcid0);
        root.children.push(h1);

        let mut p = StructNode::new("P".to_string());
        p.page_number = Some(1);
        let mut mcid1 = StructNode::new("MCID".to_string());
        mcid1.mcid = Some(1);
        p.children.push(mcid1);
        root.children.push(p);

        let mut h2 = StructNode::new("H2".to_string());
        h2.page_number = Some(2);
        let mut mcid0p2 = StructNode::new("MCID".to_string());
        mcid0p2.mcid = Some(0);
        h2.children.push(mcid0p2);
        root.children.push(h2);

        let role_map = HashMap::new();
        let mut map = McidMap::new();
        walk_for_mcids(&root, None, None, &role_map, &mut map);

        // H1 on page 1, mcid 0 → heading level 1
        let info = map.get(&(1, 0)).unwrap();
        assert_eq!(info.role, "heading");
        assert_eq!(info.heading_level, Some(1));
        assert_eq!(info.struct_type, "H1");

        // P on page 1, mcid 1 → paragraph
        let info = map.get(&(1, 1)).unwrap();
        assert_eq!(info.role, "paragraph");
        assert_eq!(info.heading_level, None);

        // H2 on page 2, mcid 0 → heading level 2
        let info = map.get(&(2, 0)).unwrap();
        assert_eq!(info.role, "heading");
        assert_eq!(info.heading_level, Some(2));
    }

    #[test]
    fn test_walk_for_mcids_with_role_map() {
        // Test custom tag mapped through RoleMap
        let mut root = StructNode::new("Document".to_string());
        let mut title = StructNode::new("Title".to_string());
        title.page_number = Some(1);
        let mut mcid = StructNode::new("MCID".to_string());
        mcid.mcid = Some(0);
        title.children.push(mcid);
        root.children.push(title);

        let mut role_map = HashMap::new();
        role_map.insert("Title".to_string(), "H1".to_string());

        let mut map = McidMap::new();
        walk_for_mcids(&root, None, None, &role_map, &mut map);

        let info = map.get(&(1, 0)).unwrap();
        assert_eq!(info.role, "heading");
        assert_eq!(info.heading_level, Some(1));
    }

    #[test]
    fn test_parse_k_entry_mcr_dictionary_creates_mcid_leaf() {
        let doc = Document::new();
        let mut parent = StructNode::new("P".to_string());
        let mut visited = HashSet::new();

        let k_obj = Object::Dictionary(lopdf::dictionary! {
            "Type" => "MCR",
            "MCID" => 7,
        });

        parse_k_entry(&doc, &k_obj, &mut parent, &mut visited);

        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].struct_type, "MCID");
        assert_eq!(parent.children[0].mcid, Some(7));
    }

    #[test]
    fn test_parse_k_entry_mcid_dict_without_type_is_still_leaf() {
        let doc = Document::new();
        let mut parent = StructNode::new("P".to_string());
        let mut visited = HashSet::new();

        let k_obj = Object::Dictionary(lopdf::dictionary! {
            "MCID" => 3,
        });

        parse_k_entry(&doc, &k_obj, &mut parent, &mut visited);

        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].struct_type, "MCID");
        assert_eq!(parent.children[0].mcid, Some(3));
    }
}
