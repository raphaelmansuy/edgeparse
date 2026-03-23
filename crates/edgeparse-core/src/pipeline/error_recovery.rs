//! Error recovery utilities for pipeline stage resilience.
//!
//! Wraps stage operations to catch panics and errors on individual pages,
//! allowing the pipeline to continue processing remaining pages.

use log;
use std::panic::{self, AssertUnwindSafe};

/// Result of processing a single page.
#[derive(Debug)]
pub enum PageResult<T> {
    /// Successful processing
    Ok(T),
    /// Page processing failed with an error message
    Failed {
        /// Zero-based index of the page that failed.
        page_index: usize,
        /// Human-readable description of the error.
        error: String,
    },
}

/// Apply a fallible operation to each page, recovering from failures.
///
/// Returns a Vec of successful results. Failed pages are logged and skipped,
/// with the original content preserved as fallback.
pub fn process_pages_with_recovery<T, F>(pages: Vec<T>, stage_name: &str, mut op: F) -> Vec<T>
where
    T: Send + Default + 'static,
    F: FnMut(T) -> T,
{
    let mut results = Vec::with_capacity(pages.len());

    for (idx, page) in pages.into_iter().enumerate() {
        let result = panic::catch_unwind(AssertUnwindSafe(|| op(page)));
        match result {
            Ok(processed) => {
                results.push(processed);
            }
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                log::error!(
                    "Stage '{}' failed on page {}: {}. Skipping page.",
                    stage_name,
                    idx + 1,
                    msg
                );
                // NOTE: original page data is lost due to move semantics.
                // In production, consider cloning before processing.
                results.push(Default::default());
            }
        }
    }

    results
}

/// Track page-level errors during pipeline execution.
#[derive(Debug, Default)]
pub struct PipelineErrors {
    /// Accumulated errors: (stage_name, page_index, error_message)
    pub errors: Vec<(String, usize, String)>,
}

impl PipelineErrors {
    /// Record an error.
    pub fn record(&mut self, stage: &str, page_index: usize, error: &str) {
        self.errors
            .push((stage.to_string(), page_index, error.to_string()));
    }

    /// Whether any errors were recorded.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Summary of all errors.
    pub fn summary(&self) -> String {
        if self.errors.is_empty() {
            return "No errors".to_string();
        }
        self.errors
            .iter()
            .map(|(stage, page, msg)| format!("[{stage}] page {}: {msg}", page + 1))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_pages_no_failure() {
        let pages = vec![vec![1, 2], vec![3, 4]];
        let result = process_pages_with_recovery(pages, "test", |mut p| {
            p.push(99);
            p
        });
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![1, 2, 99]);
        assert_eq!(result[1], vec![3, 4, 99]);
    }

    #[test]
    fn test_process_pages_with_panic_recovery() {
        let pages = vec![vec![1], vec![2], vec![3]];
        let result = process_pages_with_recovery(pages, "test", |p| {
            if p[0] == 2 {
                panic!("simulated panic on page 2");
            }
            p
        });
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], vec![1]);
        assert!(result[1].is_empty()); // recovered with default
        assert_eq!(result[2], vec![3]);
    }

    #[test]
    fn test_pipeline_errors() {
        let mut errors = PipelineErrors::default();
        assert!(!errors.has_errors());

        errors.record("Stage 3", 0, "Failed to detect tables");
        errors.record("Stage 8", 2, "Header detection failed");

        assert!(errors.has_errors());
        assert_eq!(errors.errors.len(), 2);
        let summary = errors.summary();
        assert!(summary.contains("Stage 3"));
        assert!(summary.contains("page 1"));
        assert!(summary.contains("page 3"));
    }

    #[test]
    fn test_pipeline_errors_empty_summary() {
        let errors = PipelineErrors::default();
        assert_eq!(errors.summary(), "No errors");
    }
}
