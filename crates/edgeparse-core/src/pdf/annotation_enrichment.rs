//! Annotation enrichment — post-processing for extracted annotations.
//!
//! Groups, filters, and classifies annotations for output rendering
//! and content-aware processing.

use crate::pdf::annotation_extractor::{AnnotationType, PdfAnnotation};

/// An annotation group — annotations of the same type on a page.
#[derive(Debug, Clone)]
pub struct AnnotationGroup {
    /// Annotation type for this group.
    pub annotation_type: AnnotationType,
    /// Page number (1-based).
    pub page_number: u32,
    /// Annotations in this group.
    pub annotations: Vec<PdfAnnotation>,
}

/// Summary statistics for annotations in a document.
#[derive(Debug, Clone, Default)]
pub struct AnnotationStats {
    /// Total number of annotations.
    pub total: usize,
    /// Number of highlight annotations.
    pub highlights: usize,
    /// Number of comment/note annotations.
    pub comments: usize,
    /// Number of link annotations.
    pub links: usize,
    /// Number of stamp annotations.
    pub stamps: usize,
    /// Number of other annotation types.
    pub other: usize,
    /// Number of pages that contain at least one annotation.
    pub pages_with_annotations: usize,
}

/// Group annotations by type and page.
pub fn group_by_type_and_page(annotations: &[PdfAnnotation]) -> Vec<AnnotationGroup> {
    use std::collections::BTreeMap;

    // Group by (page_number, type_key)
    let mut groups: BTreeMap<(u32, String), Vec<PdfAnnotation>> = BTreeMap::new();

    for ann in annotations {
        let type_key = type_to_key(&ann.annotation_type);
        groups
            .entry((ann.page_number, type_key))
            .or_default()
            .push(ann.clone());
    }

    groups
        .into_iter()
        .map(|((page_number, _), anns)| {
            let annotation_type = anns[0].annotation_type.clone();
            AnnotationGroup {
                annotation_type,
                page_number,
                annotations: anns,
            }
        })
        .collect()
}

/// Filter annotations to only include user-facing ones (exclude popups, links).
pub fn filter_user_annotations(annotations: &[PdfAnnotation]) -> Vec<PdfAnnotation> {
    annotations
        .iter()
        .filter(|a| {
            !matches!(
                a.annotation_type,
                AnnotationType::Popup | AnnotationType::Link
            )
        })
        .cloned()
        .collect()
}

/// Filter annotations to only those with non-empty text content.
pub fn filter_with_content(annotations: &[PdfAnnotation]) -> Vec<PdfAnnotation> {
    annotations
        .iter()
        .filter(|a| a.contents.as_ref().is_some_and(|c| !c.trim().is_empty()))
        .cloned()
        .collect()
}

/// Compute annotation statistics.
pub fn compute_stats(annotations: &[PdfAnnotation]) -> AnnotationStats {
    let mut stats = AnnotationStats {
        total: annotations.len(),
        ..Default::default()
    };

    let mut pages = std::collections::HashSet::new();

    for ann in annotations {
        pages.insert(ann.page_number);
        match &ann.annotation_type {
            AnnotationType::Highlight | AnnotationType::Underline | AnnotationType::StrikeOut => {
                stats.highlights += 1;
            }
            AnnotationType::Text | AnnotationType::FreeText => {
                stats.comments += 1;
            }
            AnnotationType::Link => {
                stats.links += 1;
            }
            AnnotationType::Stamp => {
                stats.stamps += 1;
            }
            _ => {
                stats.other += 1;
            }
        }
    }

    stats.pages_with_annotations = pages.len();
    stats
}

/// Render annotations as a markdown summary.
pub fn annotations_to_markdown(annotations: &[PdfAnnotation]) -> String {
    if annotations.is_empty() {
        return String::new();
    }

    let mut out = String::from("## Annotations\n\n");
    let mut current_page = 0u32;

    for ann in annotations {
        if ann.page_number != current_page {
            current_page = ann.page_number;
            out.push_str(&format!("### Page {}\n\n", current_page));
        }
        let type_label = type_to_key(&ann.annotation_type);
        let content = ann.contents.as_deref().unwrap_or("(no content)");
        let author = ann
            .author
            .as_deref()
            .map(|a| format!(" — {}", a))
            .unwrap_or_default();
        out.push_str(&format!("- **[{}]** {}{}\n", type_label, content, author));
    }

    out
}

fn type_to_key(t: &AnnotationType) -> String {
    match t {
        AnnotationType::Text => "Text".to_string(),
        AnnotationType::Highlight => "Highlight".to_string(),
        AnnotationType::Underline => "Underline".to_string(),
        AnnotationType::StrikeOut => "StrikeOut".to_string(),
        AnnotationType::FreeText => "FreeText".to_string(),
        AnnotationType::Link => "Link".to_string(),
        AnnotationType::Stamp => "Stamp".to_string(),
        AnnotationType::Ink => "Ink".to_string(),
        AnnotationType::FileAttachment => "FileAttachment".to_string(),
        AnnotationType::Popup => "Popup".to_string(),
        AnnotationType::Other(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ann(ann_type: AnnotationType, page: u32, content: Option<&str>) -> PdfAnnotation {
        PdfAnnotation {
            annotation_type: ann_type,
            contents: content.map(|s| s.to_string()),
            author: None,
            page_number: page,
            rect: None,
            subject: None,
            creation_date: None,
            modification_date: None,
        }
    }

    #[test]
    fn test_group_by_type_and_page() {
        let annotations = vec![
            make_ann(AnnotationType::Highlight, 1, Some("important")),
            make_ann(AnnotationType::Highlight, 1, Some("also important")),
            make_ann(AnnotationType::Text, 1, Some("note")),
            make_ann(AnnotationType::Highlight, 2, Some("another")),
        ];
        let groups = group_by_type_and_page(&annotations);
        assert_eq!(groups.len(), 3); // H-p1, T-p1, H-p2
        assert_eq!(groups[0].annotations.len(), 2); // two highlights on p1
    }

    #[test]
    fn test_filter_user_annotations() {
        let annotations = vec![
            make_ann(AnnotationType::Highlight, 1, Some("yes")),
            make_ann(AnnotationType::Popup, 1, None),
            make_ann(AnnotationType::Link, 1, None),
            make_ann(AnnotationType::Text, 2, Some("note")),
        ];
        let filtered = filter_user_annotations(&annotations);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_with_content() {
        let annotations = vec![
            make_ann(AnnotationType::Highlight, 1, Some("has content")),
            make_ann(AnnotationType::Highlight, 1, None),
            make_ann(AnnotationType::Highlight, 1, Some("  ")),
        ];
        let filtered = filter_with_content(&annotations);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_compute_stats() {
        let annotations = vec![
            make_ann(AnnotationType::Highlight, 1, Some("a")),
            make_ann(AnnotationType::Text, 1, Some("b")),
            make_ann(AnnotationType::Link, 2, None),
            make_ann(AnnotationType::Stamp, 3, None),
        ];
        let stats = compute_stats(&annotations);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.highlights, 1);
        assert_eq!(stats.comments, 1);
        assert_eq!(stats.links, 1);
        assert_eq!(stats.stamps, 1);
        assert_eq!(stats.pages_with_annotations, 3);
    }

    #[test]
    fn test_annotations_to_markdown() {
        let annotations = vec![make_ann(AnnotationType::Highlight, 1, Some("key point"))];
        let md = annotations_to_markdown(&annotations);
        assert!(md.contains("## Annotations"));
        assert!(md.contains("### Page 1"));
        assert!(md.contains("[Highlight]"));
        assert!(md.contains("key point"));
    }
}
