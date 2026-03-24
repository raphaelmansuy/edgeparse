//! Stage 18: Reading Order Sorting
//!
//! Sorts content elements on each page into reading order using
//! the XY-Cut++ recursive segmentation algorithm.
//!
//! Uses actual page dimensions from the PDF MediaBox if available,
//! falling back to A4 (595×842 pt) when page info is absent.

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::pdf::page_info::PageInfo;
use crate::utils::xycut;

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

/// Sort content elements on each page into reading order.
///
/// `page_info` is indexed 0..N matching `pages`. Pass an empty slice to use
/// the A4 fallback for all pages.
pub fn sort_reading_order(pages: &mut [Vec<ContentElement>], page_info: &[PageInfo]) {
    let sort_page = |(i, page): (usize, &mut Vec<ContentElement>)| {
        let page_bbox = page_info
            .get(i)
            .map(|info| {
                BoundingBox::new(
                    None,
                    info.media_box.left_x,
                    info.media_box.bottom_y,
                    info.media_box.right_x,
                    info.media_box.top_y,
                )
            })
            .unwrap_or_else(|| BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0));
        xycut::xycut_sort(page, &page_bbox);
    };

    #[cfg(not(target_arch = "wasm32"))]
    {
        pages.par_iter_mut().enumerate().for_each(sort_page);
    }
    #[cfg(target_arch = "wasm32")]
    {
        pages.iter_mut().enumerate().for_each(sort_page);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::chunks::ImageChunk;

    fn a4_page_info(page_idx: usize) -> PageInfo {
        PageInfo {
            index: page_idx,
            page_number: (page_idx + 1) as u32,
            media_box: BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0),
            crop_box: BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0),
            rotation: 0,
            width: 595.0,
            height: 842.0,
        }
    }

    fn make_element(page: u32, left: f64, bottom: f64, right: f64, top: f64) -> ContentElement {
        ContentElement::Image(ImageChunk {
            bbox: BoundingBox::new(Some(page), left, bottom, right, top),
            index: None,
            level: None,
        })
    }

    #[test]
    fn test_empty_pages() {
        let mut pages: Vec<Vec<ContentElement>> = vec![vec![], vec![]];
        let page_info = vec![a4_page_info(0), a4_page_info(1)];
        sort_reading_order(&mut pages, &page_info);
        assert!(pages[0].is_empty());
    }

    #[test]
    fn test_single_element() {
        let mut pages = vec![vec![make_element(1, 72.0, 700.0, 300.0, 712.0)]];
        let page_info = vec![a4_page_info(0)];
        sort_reading_order(&mut pages, &page_info);
        assert_eq!(pages[0].len(), 1);
    }

    #[test]
    fn test_reading_order_top_to_bottom() {
        let mut pages = vec![vec![
            make_element(1, 72.0, 100.0, 300.0, 112.0), // bottom
            make_element(1, 72.0, 700.0, 300.0, 712.0), // top
            make_element(1, 72.0, 400.0, 300.0, 412.0), // middle
        ]];
        let page_info = vec![a4_page_info(0)];
        sort_reading_order(&mut pages, &page_info);
        // Should be sorted top-to-bottom
        assert!(pages[0][0].bbox().top_y > pages[0][1].bbox().top_y);
        assert!(pages[0][1].bbox().top_y > pages[0][2].bbox().top_y);
    }

    #[test]
    fn test_multi_page_sorting() {
        let mut pages = vec![
            vec![
                make_element(1, 72.0, 100.0, 300.0, 112.0),
                make_element(1, 72.0, 700.0, 300.0, 712.0),
            ],
            vec![
                make_element(2, 72.0, 200.0, 300.0, 212.0),
                make_element(2, 72.0, 600.0, 300.0, 612.0),
            ],
        ];
        let page_info = vec![a4_page_info(0), a4_page_info(1)];
        sort_reading_order(&mut pages, &page_info);
        assert!(pages[0][0].bbox().top_y > pages[0][1].bbox().top_y);
        assert!(pages[1][0].bbox().top_y > pages[1][1].bbox().top_y);
    }
}
