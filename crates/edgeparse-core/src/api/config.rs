//! Processing configuration for EdgeParse.

use serde::{Deserialize, Serialize};

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    /// JSON structured output
    Json,
    /// Plain text
    Text,
    /// HTML5
    Html,
    /// Annotated PDF with bounding boxes
    Pdf,
    /// Markdown
    Markdown,
    /// Markdown with HTML tables
    MarkdownWithHtml,
    /// Markdown with embedded images
    MarkdownWithImages,
}

/// Table detection method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableMethod {
    /// Border-based detection only
    Default,
    /// Border + cluster detection
    Cluster,
}

/// Reading order algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadingOrder {
    /// No reading order sorting
    Off,
    /// XY-Cut++ algorithm
    XyCut,
}

/// Image output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageOutput {
    /// No image extraction
    Off,
    /// Base64 data URIs embedded in output
    Embedded,
    /// External file references
    External,
}

/// Image format for extracted images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    /// PNG format
    Png,
    /// JPEG format
    Jpeg,
}

/// Hybrid backend selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HybridBackend {
    /// No hybrid processing
    Off,
    /// Docling Fast server
    DoclingFast,
}

/// Hybrid triage mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HybridMode {
    /// Dynamic triage — route pages based on complexity
    Auto,
    /// Skip triage — send all pages to backend
    Full,
}

/// Main processing configuration (24 options from CLI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingConfig {
    /// Output directory (default: input file directory)
    pub output_dir: Option<String>,
    /// Password for encrypted PDFs
    pub password: Option<String>,
    /// Output formats
    pub formats: Vec<OutputFormat>,
    /// Suppress console logging
    pub quiet: bool,
    /// Content safety filter configuration
    pub filter_config: super::filter::FilterConfig,
    /// Enable PII sanitization
    pub sanitize: bool,
    /// Preserve original line breaks
    pub keep_line_breaks: bool,
    /// Replacement character for invalid characters
    pub replace_invalid_chars: String,
    /// Use PDF structure tree for tagged PDFs
    pub use_struct_tree: bool,
    /// Table detection method
    pub table_method: TableMethod,
    /// Reading order algorithm
    pub reading_order: ReadingOrder,
    /// Page separator for Markdown output
    pub markdown_page_separator: Option<String>,
    /// Page separator for text output
    pub text_page_separator: Option<String>,
    /// Page separator for HTML output
    pub html_page_separator: Option<String>,
    /// Image output mode
    pub image_output: ImageOutput,
    /// Image format
    pub image_format: ImageFormat,
    /// Directory for extracted images
    pub image_dir: Option<String>,
    /// Enable raster table OCR recovery on image-based tables
    pub raster_table_ocr: bool,
    /// Pages to extract (e.g., "1,3,5-7")
    pub pages: Option<String>,
    /// Include headers/footers in output
    pub include_header_footer: bool,
    /// Hybrid backend
    pub hybrid: HybridBackend,
    /// Hybrid triage mode
    pub hybrid_mode: HybridMode,
    /// Hybrid backend URL
    pub hybrid_url: Option<String>,
    /// Hybrid timeout in milliseconds
    pub hybrid_timeout: u64,
    /// Enable local fallback on hybrid error
    pub hybrid_fallback: bool,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            password: None,
            formats: vec![OutputFormat::Json],
            quiet: false,
            filter_config: super::filter::FilterConfig::default(),
            sanitize: false,
            keep_line_breaks: false,
            replace_invalid_chars: " ".to_string(),
            use_struct_tree: false,
            table_method: TableMethod::Default,
            reading_order: ReadingOrder::XyCut,
            markdown_page_separator: None,
            text_page_separator: None,
            html_page_separator: None,
            image_output: ImageOutput::External,
            image_format: ImageFormat::Png,
            image_dir: None,
            raster_table_ocr: true,
            pages: None,
            include_header_footer: false,
            hybrid: HybridBackend::Off,
            hybrid_mode: HybridMode::Auto,
            hybrid_url: None,
            hybrid_timeout: 30000,
            hybrid_fallback: false,
        }
    }
}

impl ProcessingConfig {
    /// Returns true when a hybrid backend is configured.
    pub fn hybrid_enabled(&self) -> bool {
        !matches!(self.hybrid, HybridBackend::Off)
    }

    /// Returns true when local OCR recovery should run.
    ///
    /// OCR is only active in hybrid mode and can still be disabled explicitly
    /// via `raster_table_ocr`.
    pub fn raster_table_ocr_enabled(&self) -> bool {
        self.raster_table_ocr && self.hybrid_enabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ProcessingConfig::default();
        assert_eq!(config.formats, vec![OutputFormat::Json]);
        assert!(!config.quiet);
        assert!(!config.sanitize);
        assert_eq!(config.reading_order, ReadingOrder::XyCut);
        assert_eq!(config.table_method, TableMethod::Default);
        assert_eq!(config.image_output, ImageOutput::External);
        assert_eq!(config.image_format, ImageFormat::Png);
        assert!(config.raster_table_ocr);
        assert_eq!(config.hybrid, HybridBackend::Off);
        assert_eq!(config.hybrid_timeout, 30000);
    }

    #[test]
    fn test_raster_table_ocr_requires_hybrid_mode() {
        let mut config = ProcessingConfig::default();
        assert!(!config.hybrid_enabled());
        assert!(!config.raster_table_ocr_enabled());

        config.hybrid = HybridBackend::DoclingFast;
        assert!(config.hybrid_enabled());
        assert!(config.raster_table_ocr_enabled());

        config.raster_table_ocr = false;
        assert!(!config.raster_table_ocr_enabled());
    }
}
