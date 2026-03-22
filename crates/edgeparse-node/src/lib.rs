#![deny(clippy::all)]

use napi_derive::napi;

use edgeparse_core::api::config::{
    ImageOutput, OutputFormat, ProcessingConfig, ReadingOrder, TableMethod,
};
use edgeparse_core::output;

use std::path::Path;

#[napi(object)]
pub struct ConvertOptions {
    pub format: Option<String>,
    pub pages: Option<String>,
    pub password: Option<String>,
    pub reading_order: Option<String>,
    pub table_method: Option<String>,
    pub image_output: Option<String>,
}

#[napi]
pub fn convert(input_path: String, options: Option<ConvertOptions>) -> napi::Result<String> {
    let opts = options.unwrap_or(ConvertOptions {
        format: None,
        pages: None,
        password: None,
        reading_order: None,
        table_method: None,
        image_output: None,
    });

    let format_str = opts.format.as_deref().unwrap_or("markdown");
    let pdf_path = Path::new(&input_path);

    if !pdf_path.exists() {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            format!("File not found: {input_path}"),
        ));
    }

    let output_format = match format_str {
        "json" => OutputFormat::Json,
        "html" => OutputFormat::Html,
        "text" => OutputFormat::Text,
        "markdown" | "md" => OutputFormat::Markdown,
        other => {
            return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("Unknown format: {other}"),
            ));
        }
    };

    let config = ProcessingConfig {
        formats: vec![output_format],
        pages: opts.pages,
        password: opts.password,
        reading_order: match opts.reading_order.as_deref() {
            Some("off") => ReadingOrder::Off,
            _ => ReadingOrder::XyCut,
        },
        table_method: match opts.table_method.as_deref() {
            Some("cluster") => TableMethod::Cluster,
            _ => TableMethod::Default,
        },
        image_output: match opts.image_output.as_deref() {
            Some("embedded") => ImageOutput::Embedded,
            Some("external") => ImageOutput::External,
            _ => ImageOutput::Off,
        },
        ..ProcessingConfig::default()
    };

    let doc = edgeparse_core::convert(pdf_path, &config)
        .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?;

    let stem = pdf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let content = match output_format {
        OutputFormat::Json => output::legacy_json::to_legacy_json_string(&doc, stem)
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?,
        OutputFormat::Html => output::html::to_html(&doc)
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?,
        OutputFormat::Text => output::text::to_text(&doc)
            .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?,
        OutputFormat::Markdown | OutputFormat::MarkdownWithHtml | OutputFormat::MarkdownWithImages => {
            output::markdown::to_markdown(&doc)
                .map_err(|e| napi::Error::new(napi::Status::GenericFailure, e.to_string()))?
        }
        OutputFormat::Pdf => {
            return Err(napi::Error::new(
                napi::Status::GenericFailure,
                "PDF output not yet implemented",
            ));
        }
    };

    Ok(content)
}

#[napi]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
