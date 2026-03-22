//! Page range parsing and filtering.
//!
//! Parses page range strings like "1-3,5,7-10" into a set of 1-based page
//! numbers, then filters pipeline pages to keep only selected ones.

use std::collections::HashSet;

/// Parse a page range string into a set of 1-based page numbers.
///
/// Supports formats:
/// - `"3"` — single page
/// - `"1-5"` — range (inclusive)
/// - `"1,3,5"` — comma-separated
/// - `"1-3,7,10-12"` — mixed
///
/// Returns `None` if the string is empty or invalid.
pub fn parse_page_range(range_str: &str, total_pages: usize) -> Option<HashSet<usize>> {
    let trimmed = range_str.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut pages = HashSet::new();
    for part in trimmed.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((start_str, end_str)) = part.split_once('-') {
            let start: usize = start_str.trim().parse().ok()?;
            let end: usize = end_str.trim().parse().ok()?;
            if start == 0 || end == 0 || start > end {
                return None;
            }
            for p in start..=end.min(total_pages) {
                pages.insert(p);
            }
        } else {
            let p: usize = part.parse().ok()?;
            if p == 0 {
                return None;
            }
            if p <= total_pages {
                pages.insert(p);
            }
        }
    }

    if pages.is_empty() {
        None
    } else {
        Some(pages)
    }
}

/// Filter pages to keep only those in the selected set.
/// `selected` contains 1-based page numbers.
/// Returns a new Vec with only the selected pages (preserving order).
pub fn filter_pages<T>(pages: Vec<Vec<T>>, selected: &HashSet<usize>) -> Vec<Vec<T>> {
    pages
        .into_iter()
        .enumerate()
        .filter_map(|(i, page)| {
            let page_num = i + 1; // 1-based
            if selected.contains(&page_num) {
                Some(page)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_page() {
        let result = parse_page_range("3", 10).unwrap();
        assert_eq!(result, HashSet::from([3]));
    }

    #[test]
    fn test_parse_range() {
        let result = parse_page_range("2-5", 10).unwrap();
        assert_eq!(result, HashSet::from([2, 3, 4, 5]));
    }

    #[test]
    fn test_parse_mixed() {
        let result = parse_page_range("1-3,7,10-12", 15).unwrap();
        assert_eq!(result, HashSet::from([1, 2, 3, 7, 10, 11, 12]));
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_page_range("", 10).is_none());
    }

    #[test]
    fn test_parse_beyond_total() {
        let result = parse_page_range("1-5", 3).unwrap();
        assert_eq!(result, HashSet::from([1, 2, 3]));
    }

    #[test]
    fn test_filter_pages() {
        let pages = vec![vec![1], vec![2], vec![3], vec![4], vec![5]];
        let selected = HashSet::from([1, 3, 5]);
        let filtered = filter_pages(pages, &selected);
        assert_eq!(filtered, vec![vec![1], vec![3], vec![5]]);
    }
}
