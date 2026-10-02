//! Image-region classification for OCR routing (SRP / Strategy).
//!
//! Decides whether a raster should go through table OCR, prose OCR, or be
//! skipped (charts / photos). Keeps the gate in one place so native and
//! in-memory recovery share the same policy.

use image::GrayImage;

/// What to do with a page image region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRegionKind {
    /// Ruled or numeric grid — run table OCR.
    Table,
    /// Dense prose screenshot — run text OCR.
    Prose,
    /// Chart, photo, or UI chrome — skip OCR.
    Skip,
}

/// Strategy for classifying a grayscale page region.
pub trait ImageRegionClassifier: Send + Sync {
    /// Return the OCR routing decision for `gray`.
    fn classify(&self, gray: &GrayImage) -> ImageRegionKind;
}

/// Optional ML / table-structure model plugged into the `Table` branch.
///
/// Implementations (TableFormer, SLANet, …) refine cell geometry after the
/// heuristic classifier says [`ImageRegionKind::Table`]. The default is a
/// no-op that leaves lattice recovery to classical line detection.
pub trait TableStructureModel: Send + Sync {
    /// Refine a table region. Return `true` when the model produced usable
    /// structure that downstream OCR should prefer over pure heuristics.
    fn refine_table(&self, gray: &GrayImage) -> bool {
        let _ = gray;
        false
    }
}

/// No-op table-structure model (classical bordered-grid path only).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullTableStructureModel;

impl TableStructureModel for NullTableStructureModel {}

/// Classifier that runs heuristics, then optionally consults a table model.
#[derive(Debug, Clone)]
pub struct ClassifyingRegionRouter<C, M> {
    /// Region kind classifier.
    pub classifier: C,
    /// Table-structure model for the `Table` branch.
    pub table_model: M,
}

impl<C: ImageRegionClassifier, M: TableStructureModel> ClassifyingRegionRouter<C, M> {
    /// Create a router.
    pub fn new(classifier: C, table_model: M) -> Self {
        Self {
            classifier,
            table_model,
        }
    }

    /// Classify then, for tables, invoke the structure model.
    pub fn route(&self, gray: &GrayImage) -> (ImageRegionKind, bool) {
        let kind = self.classifier.classify(gray);
        let model_hit =
            matches!(kind, ImageRegionKind::Table) && self.table_model.refine_table(gray);
        (kind, model_hit)
    }
}

/// Cheap heuristic classifier (no ML). Wraps the existing raster heuristics in
/// [`crate::pdf::raster_table_ocr`] so policy lives behind one trait.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicImageRegionClassifier;

impl ImageRegionClassifier for HeuristicImageRegionClassifier {
    fn classify(&self, gray: &GrayImage) -> ImageRegionKind {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use crate::pdf::raster_table_ocr::classify_raster_region;
            classify_raster_region(gray)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = gray;
            ImageRegionKind::Table
        }
    }
}

/// Shared deadline tracker for OCR work across a document.
#[derive(Debug, Clone)]
pub struct OcrBudget {
    deadline: Option<std::time::Instant>,
}

impl OcrBudget {
    /// `None` = unlimited. `Some(0)` = already exhausted.
    pub fn from_millis(ms: Option<u64>) -> Self {
        let deadline = ms.map(|m| std::time::Instant::now() + std::time::Duration::from_millis(m));
        Self { deadline }
    }

    /// No deadline.
    pub fn unlimited() -> Self {
        Self { deadline: None }
    }

    /// True when the wall-clock budget has been spent.
    pub fn exhausted(&self) -> bool {
        self.deadline
            .is_some_and(|d| std::time::Instant::now() >= d)
    }

    /// True when more OCR work is still allowed.
    pub fn allow(&self) -> bool {
        !self.exhausted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysTable;
    impl TableStructureModel for AlwaysTable {
        fn refine_table(&self, _: &GrayImage) -> bool {
            true
        }
    }

    #[test]
    fn router_invokes_table_model_on_table_kind() {
        let router = ClassifyingRegionRouter::new(HeuristicImageRegionClassifier, AlwaysTable);
        let gray = GrayImage::new(8, 8);
        let (kind, hit) = router.route(&gray);
        // Heuristic on tiny blank image may Skip or Table depending on platform.
        if kind == ImageRegionKind::Table {
            assert!(hit);
        } else {
            assert!(!hit);
        }
    }
}
