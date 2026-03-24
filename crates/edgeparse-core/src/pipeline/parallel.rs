//! Parallel page-processing utilities.
//!
//! On native targets: uses rayon for data parallelism.
//! On wasm32: falls back to sequential iteration.
//!
//! Provides helpers that apply a per-page transformation in parallel across all
//! pages of a document. Designed as a drop-in replacement for the sequential
//! `for page in &mut pages { ... }` loops in the orchestrator.

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

use crate::models::content::ContentElement;

/// Per-page content alias (mirrors `orchestrator::PageContent`).
type PageContent = Vec<ContentElement>;

/// Apply `op` to each page in parallel, replacing each page's content.
///
/// This is the parallel equivalent of:
/// ```ignore
/// for page in &mut pages {
///     let elements = std::mem::take(page);
///     *page = op(elements);
/// }
/// ```
pub fn par_map_pages<F>(pages: &mut Vec<PageContent>, op: F)
where
    F: Fn(Vec<ContentElement>) -> Vec<ContentElement> + Sync + Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        let results: Vec<PageContent> = std::mem::take(pages).into_par_iter().map(&op).collect();
        *pages = results;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let results: Vec<PageContent> = std::mem::take(pages).into_iter().map(op).collect();
        *pages = results;
    }
}

/// Apply `op` to each page in parallel where the closure also receives a
/// zero-based page index.
pub fn par_map_pages_indexed<F>(pages: &mut Vec<PageContent>, op: F)
where
    F: Fn(usize, Vec<ContentElement>) -> Vec<ContentElement> + Sync + Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        let results: Vec<PageContent> = std::mem::take(pages)
            .into_par_iter()
            .enumerate()
            .map(|(i, page)| op(i, page))
            .collect();
        *pages = results;
    }
    #[cfg(target_arch = "wasm32")]
    {
        let results: Vec<PageContent> = std::mem::take(pages)
            .into_iter()
            .enumerate()
            .map(|(i, page)| op(i, page))
            .collect();
        *pages = results;
    }
}

/// Parallel fold — map each page to a value of type `T` and collect results.
///
/// Useful for gathering per-page statistics or metadata without modifying pages.
pub fn par_extract<T, F>(pages: &[PageContent], op: F) -> Vec<T>
where
    T: Send,
    F: Fn(&[ContentElement]) -> T + Sync + Send,
{
    #[cfg(not(target_arch = "wasm32"))]
    {
        pages.par_iter().map(|page| op(page)).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        pages.iter().map(|page| op(page)).collect()
    }
}

/// Configure the global rayon thread pool with the given number of threads.
///
/// On WASM, this is a no-op.
#[cfg(not(target_arch = "wasm32"))]
pub fn configure_thread_pool(num_threads: usize) -> Result<(), rayon::ThreadPoolBuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
}

/// Configure the global rayon thread pool with the given number of threads.
///
/// On WASM, this is a no-op.
#[cfg(target_arch = "wasm32")]
pub fn configure_thread_pool(_num_threads: usize) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::content::ContentElement;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn text_chunk(val: &str) -> ContentElement {
        ContentElement::TextChunk(TextChunk {
            value: val.to_string(),
            bbox: BoundingBox::new(None, 0.0, 0.0, 100.0, 10.0),
            font_name: String::new(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: String::new(),
            contrast_ratio: 21.0,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: None,
            level: None,
            mcid: None,
        })
    }

    #[test]
    fn test_par_map_pages_identity() {
        let mut pages = vec![
            vec![text_chunk("a"), text_chunk("b")],
            vec![text_chunk("c")],
        ];
        par_map_pages(&mut pages, |elems| elems);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].len(), 2);
        assert_eq!(pages[1].len(), 1);
    }

    #[test]
    fn test_par_map_pages_transform() {
        let mut pages = vec![
            vec![text_chunk("a"), text_chunk("b"), text_chunk("c")],
            vec![text_chunk("x")],
        ];
        // Keep only first element per page
        par_map_pages(&mut pages, |mut elems| {
            elems.truncate(1);
            elems
        });
        assert_eq!(pages[0].len(), 1);
        assert_eq!(pages[1].len(), 1);
    }

    #[test]
    fn test_par_map_pages_indexed() {
        let mut pages = vec![
            vec![text_chunk("a")],
            vec![text_chunk("b")],
            vec![text_chunk("c")],
        ];
        let indices_seen = std::sync::Mutex::new(vec![]);
        par_map_pages_indexed(&mut pages, |i, elems| {
            indices_seen.lock().unwrap().push(i);
            elems
        });
        let mut seen = indices_seen.into_inner().unwrap();
        seen.sort();
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[test]
    fn test_par_extract() {
        let pages = vec![
            vec![text_chunk("a"), text_chunk("b")],
            vec![text_chunk("c")],
            vec![],
        ];
        let counts: Vec<usize> = par_extract(&pages, |elems| elems.len());
        assert_eq!(counts, vec![2, 1, 0]);
    }

    #[test]
    fn test_empty_pages() {
        let mut pages: Vec<PageContent> = vec![];
        par_map_pages(&mut pages, |e| e);
        assert!(pages.is_empty());

        let counts: Vec<usize> = par_extract(&pages, |e| e.len());
        assert!(counts.is_empty());
    }
}
