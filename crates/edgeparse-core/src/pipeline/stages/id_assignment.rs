//! Stage 13: ID Assignment
//!
//! Assigns sequential global indices to all content elements across
//! all pages. The index reflects reading order (Stage 18 must run first).

use crate::models::content::ContentElement;

/// Assign sequential IDs to all content elements across all pages.
pub fn assign_ids(pages: &mut [Vec<ContentElement>]) {
    let mut next_id: u32 = 0;
    for page in pages.iter_mut() {
        for elem in page.iter_mut() {
            elem.set_index(next_id);
            next_id += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::ImageChunk;

    fn make_element(page: u32) -> ContentElement {
        ContentElement::Image(ImageChunk {
            bbox: BoundingBox::new(Some(page), 0.0, 0.0, 100.0, 50.0),
            index: None,
            level: None,
        })
    }

    #[test]
    fn test_sequential_ids() {
        let mut pages = vec![
            vec![make_element(1), make_element(1)],
            vec![make_element(2), make_element(2), make_element(2)],
        ];
        assign_ids(&mut pages);
        assert_eq!(pages[0][0].index(), Some(0));
        assert_eq!(pages[0][1].index(), Some(1));
        assert_eq!(pages[1][0].index(), Some(2));
        assert_eq!(pages[1][1].index(), Some(3));
        assert_eq!(pages[1][2].index(), Some(4));
    }

    #[test]
    fn test_empty_pages() {
        let mut pages: Vec<Vec<ContentElement>> = vec![vec![], vec![]];
        assign_ids(&mut pages);
    }
}
