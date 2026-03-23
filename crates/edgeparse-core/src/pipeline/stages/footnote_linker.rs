//! Footnote linker — pairs in-text footnote markers with detected footnote bodies.
//!
//! After the footnote detector marks footnote paragraphs (`SemanticType::Note`),
//! this module finds corresponding markers in body text and creates explicit links.

use regex::Regex;
use std::sync::LazyLock;

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;

/// Regex for superscript footnote markers in body text (e.g., "¹", "²³").
static SUPERSCRIPT_MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[¹²³⁴⁵⁶⁷⁸⁹⁰]+").unwrap());

/// Regex for extracting the marker number from a footnote body.
static FOOTNOTE_BODY_MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\s]*(\d{1,3}|[¹²³⁴⁵⁶⁷⁸⁹⁰]+)[\.\)\s]").unwrap());

/// A linked footnote — marker in text paired with footnote body.
#[derive(Debug, Clone)]
pub struct LinkedFootnote {
    /// The marker string (e.g., "1", "¹").
    pub marker: String,
    /// Normalized numeric value of the marker.
    pub number: u32,
    /// Page number where the marker appears (1-based).
    pub marker_page: u32,
    /// Page number of the footnote body.
    pub body_page: u32,
    /// Footnote body text.
    pub body_text: String,
}

/// Result of footnote linking for a document.
#[derive(Debug, Clone)]
pub struct FootnoteLinkResult {
    /// Successfully linked footnotes.
    pub linked: Vec<LinkedFootnote>,
    /// Markers found in text that have no matching footnote body.
    pub unmatched_markers: Vec<(String, u32)>, // (marker, page)
    /// Footnote bodies that have no matching marker in text.
    pub unmatched_bodies: Vec<(String, u32)>, // (first_line, page)
}

impl FootnoteLinkResult {
    /// Whether all markers and bodies were successfully paired.
    pub fn is_fully_linked(&self) -> bool {
        self.unmatched_markers.is_empty() && self.unmatched_bodies.is_empty()
    }

    /// Total number of footnotes found.
    pub fn total_footnotes(&self) -> usize {
        self.linked.len()
    }
}

/// Link footnote markers in body text to footnote bodies across pages.
pub fn link_footnotes(pages: &[Vec<ContentElement>]) -> FootnoteLinkResult {
    let mut markers: Vec<(String, u32, u32)> = Vec::new(); // (marker, number, page)
    let mut bodies: Vec<(u32, u32, String)> = Vec::new(); // (number, page, text)

    for (page_idx, page) in pages.iter().enumerate() {
        let page_num = (page_idx + 1) as u32;

        for elem in page {
            if let ContentElement::Paragraph(p) = elem {
                if p.base.semantic_type == SemanticType::Note {
                    // This is a footnote body
                    let text = p.base.value();
                    if let Some(num) = extract_footnote_number(&text) {
                        bodies.push((num, page_num, text));
                    }
                } else {
                    // Check for superscript markers in body text
                    let text = p.base.value();
                    for m in SUPERSCRIPT_MARKER_RE.find_iter(&text) {
                        let marker_str = m.as_str();
                        if let Some(num) = superscript_to_number(marker_str) {
                            markers.push((marker_str.to_string(), num, page_num));
                        }
                    }
                }
            }
        }
    }

    // Match markers to bodies by number
    let mut linked = Vec::new();
    let mut matched_body_indices = std::collections::HashSet::new();
    let mut matched_marker_indices = std::collections::HashSet::new();

    for (mi, (marker, number, marker_page)) in markers.iter().enumerate() {
        for (bi, (body_num, body_page, body_text)) in bodies.iter().enumerate() {
            if number == body_num && !matched_body_indices.contains(&bi) {
                linked.push(LinkedFootnote {
                    marker: marker.clone(),
                    number: *number,
                    marker_page: *marker_page,
                    body_page: *body_page,
                    body_text: body_text.clone(),
                });
                matched_body_indices.insert(bi);
                matched_marker_indices.insert(mi);
                break;
            }
        }
    }

    let unmatched_markers: Vec<(String, u32)> = markers
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_marker_indices.contains(i))
        .map(|(_, (m, _, p))| (m.clone(), *p))
        .collect();

    let unmatched_bodies: Vec<(String, u32)> = bodies
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched_body_indices.contains(i))
        .map(|(_, (_, p, t))| (t.clone(), *p))
        .collect();

    FootnoteLinkResult {
        linked,
        unmatched_markers,
        unmatched_bodies,
    }
}

/// Extract the footnote number from the beginning of a footnote body.
fn extract_footnote_number(text: &str) -> Option<u32> {
    let m = FOOTNOTE_BODY_MARKER_RE.find(text)?;
    let matched = m.as_str().trim();
    // Try parsing first as digits
    let digits: String = matched.chars().filter(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        return digits.parse().ok();
    }
    // Try superscript
    let superscripts: String = matched
        .chars()
        .filter(|c| "¹²³⁴⁵⁶⁷⁸⁹⁰".contains(*c))
        .collect();
    superscript_to_number(&superscripts)
}

/// Convert superscript digits to a regular number.
fn superscript_to_number(s: &str) -> Option<u32> {
    let converted: String = s
        .chars()
        .filter_map(|c| match c {
            '¹' => Some('1'),
            '²' => Some('2'),
            '³' => Some('3'),
            '⁴' => Some('4'),
            '⁵' => Some('5'),
            '⁶' => Some('6'),
            '⁷' => Some('7'),
            '⁸' => Some('8'),
            '⁹' => Some('9'),
            '⁰' => Some('0'),
            _ => None,
        })
        .collect();
    if converted.is_empty() {
        None
    } else {
        converted.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superscript_to_number() {
        assert_eq!(superscript_to_number("¹"), Some(1));
        assert_eq!(superscript_to_number("²³"), Some(23));
        assert_eq!(superscript_to_number("¹⁰"), Some(10));
        assert_eq!(superscript_to_number(""), None);
        assert_eq!(superscript_to_number("abc"), None);
    }

    #[test]
    fn test_extract_footnote_number() {
        assert_eq!(extract_footnote_number("1. This is a footnote."), Some(1));
        assert_eq!(extract_footnote_number("23) Another note"), Some(23));
        assert_eq!(extract_footnote_number("¹ First note"), Some(1));
        assert_eq!(extract_footnote_number("No number"), None);
    }

    #[test]
    fn test_link_empty() {
        let pages: Vec<Vec<ContentElement>> = vec![vec![]];
        let result = link_footnotes(&pages);
        assert!(result.is_fully_linked());
        assert_eq!(result.total_footnotes(), 0);
    }

    #[test]
    fn test_superscript_regex() {
        assert!(SUPERSCRIPT_MARKER_RE.is_match("text with ¹ marker"));
        assert!(SUPERSCRIPT_MARKER_RE.is_match("see²³ here"));
        assert!(!SUPERSCRIPT_MARKER_RE.is_match("no markers here"));
    }
}
