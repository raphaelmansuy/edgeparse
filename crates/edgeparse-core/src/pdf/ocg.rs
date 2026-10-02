//! Optional Content Groups (OCG) — default visibility from the catalog.
//!
//! ISO 32000-1 §8.11: `/OCProperties` / `/D` (default configuration) lists
//! which OCGs are ON/OFF. Extraction should omit text whose OCG is off.

use std::collections::HashSet;

use lopdf::{Document, Object};

/// Default OCG visibility derived from `/Catalog` → `/OCProperties` → `/D`.
#[derive(Debug, Clone, Default)]
pub struct OcgVisibility {
    /// Object IDs of OCGs that are ON by default.
    pub on: HashSet<(u32, u16)>,
    /// Object IDs of OCGs that are OFF by default.
    pub off: HashSet<(u32, u16)>,
    /// When true, any OCG not listed in `on`/`off` is treated as visible
    /// (BaseState = ON, the PDF default).
    pub default_on: bool,
}

impl OcgVisibility {
    /// Whether an OCG object id is visible under the catalog defaults.
    pub fn is_visible(&self, id: (u32, u16)) -> bool {
        if self.off.contains(&id) {
            return false;
        }
        if self.on.contains(&id) {
            return true;
        }
        self.default_on
    }

    /// True when the document declares no OCG config (everything visible).
    pub fn is_empty(&self) -> bool {
        self.on.is_empty() && self.off.is_empty()
    }
}

/// Load OCG default visibility from the document catalog.
pub fn load_ocg_visibility(doc: &Document) -> OcgVisibility {
    let mut vis = OcgVisibility {
        default_on: true,
        ..OcgVisibility::default()
    };

    let Ok(catalog) = doc.catalog() else {
        return vis;
    };
    let Ok(oc_props) = catalog.get(b"OCProperties") else {
        return vis;
    };
    let oc_dict = match resolve(doc, oc_props) {
        Object::Dictionary(d) => d,
        _ => return vis,
    };

    // Collect all OCGs from /OCGs array for BaseState handling.
    let mut all_ocgs: HashSet<(u32, u16)> = HashSet::new();
    if let Ok(ocgs) = oc_dict.get(b"OCGs") {
        collect_refs(doc, ocgs, &mut all_ocgs);
    }

    let Ok(d_obj) = oc_dict.get(b"D") else {
        // No default config — all ON.
        vis.on = all_ocgs;
        return vis;
    };
    let d_dict = match resolve(doc, d_obj) {
        Object::Dictionary(d) => d,
        _ => {
            vis.on = all_ocgs;
            return vis;
        }
    };

    // BaseState: ON (default) | OFF | Unchanged
    if let Ok(Object::Name(name)) = d_dict.get(b"BaseState") {
        match name.as_slice() {
            b"OFF" => vis.default_on = false,
            _ => vis.default_on = true,
        }
    }

    if let Ok(on_arr) = d_dict.get(b"ON") {
        collect_refs(doc, on_arr, &mut vis.on);
    }
    if let Ok(off_arr) = d_dict.get(b"OFF") {
        collect_refs(doc, off_arr, &mut vis.off);
    }

    // When BaseState is ON and ON array empty, all OCGs are on unless in OFF.
    if vis.default_on && vis.on.is_empty() {
        for id in &all_ocgs {
            if !vis.off.contains(id) {
                vis.on.insert(*id);
            }
        }
    }

    vis
}

fn resolve(doc: &Document, obj: &Object) -> Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).cloned().unwrap_or(Object::Null),
        other => other.clone(),
    }
}

fn collect_refs(doc: &Document, obj: &Object, out: &mut HashSet<(u32, u16)>) {
    match obj {
        Object::Array(arr) => {
            for item in arr {
                collect_refs(doc, item, out);
            }
        }
        Object::Reference(id) => {
            out.insert(*id);
        }
        other => {
            // Dereference only for nested arrays; inline OCG dicts have no stable id.
            match resolve(doc, other) {
                Object::Array(arr) => {
                    for item in arr {
                        collect_refs(doc, &item, out);
                    }
                }
                Object::Reference(id) => {
                    out.insert(id);
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn empty_doc_all_visible() {
        let doc = Document::new();
        let vis = load_ocg_visibility(&doc);
        assert!(vis.is_visible((1, 0)));
        assert!(vis.default_on);
    }

    #[test]
    fn off_list_hides_ocg() {
        let mut doc = Document::with_version("1.5");
        let ocg_off = doc.add_object(dictionary! {
            "Type" => "OCG",
            "Name" => Object::string_literal("Hidden"),
        });
        let ocg_on = doc.add_object(dictionary! {
            "Type" => "OCG",
            "Name" => Object::string_literal("Shown"),
        });
        let pages_id = doc.new_object_id();
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => Vec::<Object>::new(),
            "Count" => 0,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "OCProperties" => dictionary! {
                "OCGs" => vec![ocg_off.into(), ocg_on.into()],
                "D" => dictionary! {
                    "BaseState" => "ON",
                    "OFF" => vec![ocg_off.into()],
                    "ON" => vec![ocg_on.into()],
                },
            },
        });
        doc.trailer.set("Root", catalog_id);

        let vis = load_ocg_visibility(&doc);
        assert!(!vis.is_visible(ocg_off));
        assert!(vis.is_visible(ocg_on));
    }
}
