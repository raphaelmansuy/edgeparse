//! Batch processing API — process multiple PDF files with progress tracking.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::api::config::ProcessingConfig;
use crate::EdgePdfError;

/// Result of processing a single file in a batch.
#[derive(Debug, Clone)]
pub struct BatchFileResult {
    /// Input file path.
    pub input_path: PathBuf,
    /// Whether processing succeeded.
    pub success: bool,
    /// Error message, if failed.
    pub error: Option<String>,
    /// Processing duration.
    pub duration: Duration,
    /// Number of pages processed.
    pub page_count: Option<u32>,
}

/// Aggregate results of a batch processing run.
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Individual file results.
    pub files: Vec<BatchFileResult>,
    /// Total duration of the batch.
    pub total_duration: Duration,
}

impl BatchResult {
    /// Number of successfully processed files.
    pub fn success_count(&self) -> usize {
        self.files.iter().filter(|f| f.success).count()
    }

    /// Number of failed files.
    pub fn failure_count(&self) -> usize {
        self.files.iter().filter(|f| !f.success).count()
    }

    /// Total number of files.
    pub fn total_count(&self) -> usize {
        self.files.len()
    }

    /// Average processing time per file.
    pub fn avg_duration(&self) -> Duration {
        if self.files.is_empty() {
            return Duration::ZERO;
        }
        self.total_duration / self.files.len() as u32
    }

    /// Summary string.
    pub fn summary(&self) -> String {
        format!(
            "Batch complete: {}/{} succeeded, {} failed, {:.1}s total",
            self.success_count(),
            self.total_count(),
            self.failure_count(),
            self.total_duration.as_secs_f64(),
        )
    }
}

/// A batch processing request.
#[derive(Debug, Clone)]
pub struct BatchRequest {
    /// Input file paths.
    pub files: Vec<PathBuf>,
    /// Processing configuration.
    pub config: ProcessingConfig,
    /// Output directory (if any).
    pub output_dir: Option<PathBuf>,
}

impl BatchRequest {
    /// Create a new batch request from a list of file paths.
    pub fn new(files: Vec<PathBuf>, config: ProcessingConfig) -> Self {
        Self {
            files,
            config,
            output_dir: None,
        }
    }

    /// Set the output directory.
    pub fn with_output_dir(mut self, dir: PathBuf) -> Self {
        self.output_dir = Some(dir);
        self
    }
}

/// Collect PDF files from a directory (non-recursive).
pub fn collect_pdf_files(dir: &Path) -> Result<Vec<PathBuf>, EdgePdfError> {
    let mut files = Vec::new();

    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("pdf") {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}

/// Collect PDF files recursively from a directory.
pub fn collect_pdf_files_recursive(dir: &Path) -> Result<Vec<PathBuf>, EdgePdfError> {
    let mut files = Vec::new();
    collect_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), EdgePdfError> {
    let entries = std::fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_recursive(&path, files)?;
        } else if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext.eq_ignore_ascii_case("pdf") {
                    files.push(path);
                }
            }
        }
    }

    Ok(())
}

/// Process a batch of files sequentially (placeholder — actual processing
/// would call the full pipeline for each file).
pub fn process_batch<F>(request: &BatchRequest, mut process_fn: F) -> BatchResult
where
    F: FnMut(&Path, &ProcessingConfig) -> Result<u32, String>,
{
    let batch_start = Instant::now();
    let mut results = Vec::with_capacity(request.files.len());

    for file_path in &request.files {
        let file_start = Instant::now();
        match process_fn(file_path, &request.config) {
            Ok(page_count) => {
                results.push(BatchFileResult {
                    input_path: file_path.clone(),
                    success: true,
                    error: None,
                    duration: file_start.elapsed(),
                    page_count: Some(page_count),
                });
            }
            Err(e) => {
                results.push(BatchFileResult {
                    input_path: file_path.clone(),
                    success: false,
                    error: Some(e),
                    duration: file_start.elapsed(),
                    page_count: None,
                });
            }
        }
    }

    BatchResult {
        files: results,
        total_duration: batch_start.elapsed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_batch_result_counts() {
        let result = BatchResult {
            files: vec![
                BatchFileResult {
                    input_path: PathBuf::from("a.pdf"),
                    success: true,
                    error: None,
                    duration: Duration::from_millis(100),
                    page_count: Some(5),
                },
                BatchFileResult {
                    input_path: PathBuf::from("b.pdf"),
                    success: false,
                    error: Some("bad".to_string()),
                    duration: Duration::from_millis(10),
                    page_count: None,
                },
                BatchFileResult {
                    input_path: PathBuf::from("c.pdf"),
                    success: true,
                    error: None,
                    duration: Duration::from_millis(200),
                    page_count: Some(10),
                },
            ],
            total_duration: Duration::from_millis(310),
        };
        assert_eq!(result.success_count(), 2);
        assert_eq!(result.failure_count(), 1);
        assert_eq!(result.total_count(), 3);
    }

    #[test]
    fn test_process_batch() {
        let request = BatchRequest::new(
            vec![PathBuf::from("test1.pdf"), PathBuf::from("test2.pdf")],
            ProcessingConfig::default(),
        );
        let result = process_batch(&request, |path, _config| {
            if path.to_str().unwrap().contains("test1") {
                Ok(5)
            } else {
                Err("not found".to_string())
            }
        });
        assert_eq!(result.success_count(), 1);
        assert_eq!(result.failure_count(), 1);
    }

    #[test]
    fn test_batch_request_with_output() {
        let req = BatchRequest::new(vec![], ProcessingConfig::default())
            .with_output_dir(PathBuf::from("/tmp/output"));
        assert_eq!(req.output_dir.unwrap(), PathBuf::from("/tmp/output"));
    }

    #[test]
    fn test_empty_batch() {
        let request = BatchRequest::new(vec![], ProcessingConfig::default());
        let result = process_batch(&request, |_, _| Ok(0));
        assert_eq!(result.total_count(), 0);
        assert_eq!(result.success_count(), 0);
    }

    #[test]
    fn test_summary() {
        let result = BatchResult {
            files: vec![],
            total_duration: Duration::from_secs(5),
        };
        let summary = result.summary();
        assert!(summary.contains("0/0"));
    }
}
