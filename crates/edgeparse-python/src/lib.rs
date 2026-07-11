use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use edgeparse_core::api::config::{
    HybridBackend, HybridMode, ImageFormat, ImageOutput, OutputFormat, ProcessingConfig,
    ReadingOrder, TableMethod,
};
use edgeparse_core::output;

use std::path::Path;

/// Convert a single PDF file and return the extracted content as a string.
///
/// Arguments:
///   input_path: Path to the PDF file.
///   format: Output format — "markdown", "json", "html", or "text". Default: "markdown".
///   pages: Optional page range string, e.g. "1,3,5-7".
///   password: Optional password for encrypted PDFs.
///   reading_order: Reading order algorithm — "xycut" or "off". Default: "xycut".
///   table_method: Table detection method — "default" or "cluster". Default: "default".
///   image_output: Image output mode — "off", "embedded", or "external". Default: "off".
///   hybrid: Hybrid OCR backend — "off" or "docling-fast". Default: "off".
///           "docling-fast" enables OCR recovery of image-based text and tables
///           (requires pdftoppm, tesseract, and optionally rapidocr on the host).
///   hybrid_mode: Hybrid triage — "auto" (route complex pages) or "full" (all
///           pages). Only applies when hybrid is enabled. Default: "auto".
///
/// Returns:
///   The extracted content as a string in the requested format.
#[pyfunction]
#[pyo3(signature = (
    input_path,
    *,
    format = "markdown",
    pages = None,
    password = None,
    reading_order = "xycut",
    table_method = "default",
    image_output = "off",
    hybrid = "off",
    hybrid_mode = "auto",
))]
#[allow(clippy::too_many_arguments)]
fn convert(
    input_path: &str,
    format: &str,
    pages: Option<&str>,
    password: Option<&str>,
    reading_order: &str,
    table_method: &str,
    image_output: &str,
    hybrid: &str,
    hybrid_mode: &str,
) -> PyResult<String> {
    let pdf_path = Path::new(input_path);
    if !pdf_path.exists() {
        return Err(PyRuntimeError::new_err(format!(
            "File not found: {input_path}"
        )));
    }

    let output_format = match format {
        "json" => OutputFormat::Json,
        "html" => OutputFormat::Html,
        "text" => OutputFormat::Text,
        "markdown" | "md" => OutputFormat::Markdown,
        other => {
            return Err(PyRuntimeError::new_err(format!(
                "Unknown format: {other}. Valid: markdown, json, html, text"
            )));
        }
    };

    let config = ProcessingConfig {
        formats: vec![output_format],
        pages: pages.map(|s| s.to_string()),
        password: password.map(|s| s.to_string()),
        reading_order: match reading_order {
            "off" => ReadingOrder::Off,
            _ => ReadingOrder::XyCut,
        },
        table_method: match table_method {
            "cluster" => TableMethod::Cluster,
            _ => TableMethod::Default,
        },
        image_output: match image_output {
            "embedded" => ImageOutput::Embedded,
            "external" => ImageOutput::External,
            _ => ImageOutput::Off,
        },
        image_format: ImageFormat::Png,
        hybrid: match hybrid {
            "docling-fast" => HybridBackend::DoclingFast,
            _ => HybridBackend::Off,
        },
        hybrid_mode: match hybrid_mode {
            "full" => HybridMode::Full,
            _ => HybridMode::Auto,
        },
        ..ProcessingConfig::default()
    };

    let doc = edgeparse_core::convert(pdf_path, &config)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

    let stem = pdf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let content = match output_format {
        OutputFormat::Json => output::legacy_json::to_legacy_json_string(&doc, stem)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
        OutputFormat::Html => {
            output::html::to_html(&doc).map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        }
        OutputFormat::Text => {
            output::text::to_text(&doc).map_err(|e| PyRuntimeError::new_err(e.to_string()))?
        }
        OutputFormat::Markdown
        | OutputFormat::MarkdownWithHtml
        | OutputFormat::MarkdownWithImages => output::markdown::to_markdown(&doc)
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
        OutputFormat::Pdf => {
            return Err(PyRuntimeError::new_err("PDF output not yet implemented"));
        }
    };

    Ok(content)
}

/// Convert a PDF file and write the output to a file.
///
/// Arguments:
///   input_path: Path to the PDF file.
///   output_dir: Directory to write the output file.
///   format: Output format — "markdown", "json", "html", or "text". Default: "markdown".
///   pages: Optional page range string, e.g. "1,3,5-7".
///   password: Optional password for encrypted PDFs.
///   hybrid: Hybrid OCR backend — "off" or "docling-fast". Default: "off".
///   hybrid_mode: Hybrid triage — "auto" or "full". Default: "auto".
///
/// Returns:
///   The path to the created output file.
#[pyfunction]
#[pyo3(signature = (
    input_path,
    output_dir,
    *,
    format = "markdown",
    pages = None,
    password = None,
    hybrid = "off",
    hybrid_mode = "auto",
))]
fn convert_file(
    input_path: &str,
    output_dir: &str,
    format: &str,
    pages: Option<&str>,
    password: Option<&str>,
    hybrid: &str,
    hybrid_mode: &str,
) -> PyResult<String> {
    let content = convert(
        input_path, format, pages, password, "xycut", "default", "off", hybrid, hybrid_mode,
    )?;

    let out_dir = Path::new(output_dir);
    std::fs::create_dir_all(out_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Cannot create output dir: {e}")))?;

    let stem = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let ext = match format {
        "json" => "json",
        "html" => "html",
        "text" => "txt",
        _ => "md",
    };

    let out_path = out_dir.join(format!("{stem}.{ext}"));
    std::fs::write(&out_path, content)
        .map_err(|e| PyRuntimeError::new_err(format!("Cannot write output: {e}")))?;

    Ok(out_path.to_string_lossy().to_string())
}

/// Return the edgeparse version string.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// EdgeParse — High-performance PDF extraction (Rust engine).
#[pymodule]
fn _edgeparse(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(convert, m)?)?;
    m.add_function(wrap_pyfunction!(convert_file, m)?)?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
