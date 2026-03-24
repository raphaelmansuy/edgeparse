//! EdgeParse WebAssembly entry points.
//!
//! Provides `wasm-bindgen` bindings for browser-based PDF parsing.

use wasm_bindgen::prelude::*;

use edgeparse_core::api::config::{ImageOutput, ProcessingConfig, ReadingOrder, TableMethod};
use edgeparse_core::output;

/// Initialize panic hook for better error messages in browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Warn).ok();
}

/// Convert PDF bytes to a structured document object (returned as JS value).
///
/// # Arguments
/// * `pdf_bytes` — raw PDF file as `Uint8Array`
/// * `format` — output format hint: `"json"` (default) | `"markdown"` | `"html"` | `"text"`
/// * `pages` — page range: `"all"` (default) or `"1-5"` or `"1,3,7"`
/// * `reading_order` — `"auto"` (default) or `"off"`
/// * `table_method` — `"default"` (default) or `"cluster"`
#[wasm_bindgen]
pub fn convert(
    pdf_bytes: &[u8],
    format: Option<String>,
    pages: Option<String>,
    reading_order: Option<String>,
    table_method: Option<String>,
) -> Result<JsValue, JsError> {
    let config = build_config(
        format.as_deref(),
        pages.as_deref(),
        reading_order.as_deref(),
        table_method.as_deref(),
    );

    let doc = edgeparse_core::convert_bytes(pdf_bytes, "uploaded.pdf", &config)
        .map_err(|e| JsError::new(&e.to_string()))?;

    serde_wasm_bindgen::to_value(&doc).map_err(|e| JsError::new(&e.to_string()))
}

/// Convert PDF bytes to a formatted output string.
///
/// # Arguments
/// * `pdf_bytes` — raw PDF file as `Uint8Array`
/// * `format` — `"json"` (default) | `"markdown"` | `"html"` | `"text"`
/// * `pages` — page range
/// * `reading_order` — `"auto"` | `"off"`
/// * `table_method` — `"default"` | `"cluster"`
#[wasm_bindgen]
pub fn convert_to_string(
    pdf_bytes: &[u8],
    format: Option<String>,
    pages: Option<String>,
    reading_order: Option<String>,
    table_method: Option<String>,
) -> Result<String, JsError> {
    let config = build_config(
        format.as_deref(),
        pages.as_deref(),
        reading_order.as_deref(),
        table_method.as_deref(),
    );

    let doc = edgeparse_core::convert_bytes(pdf_bytes, "uploaded.pdf", &config)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let fmt = format.as_deref().unwrap_or("json");
    let result = match fmt {
        "markdown" | "md" => output::markdown::to_markdown(&doc),
        "html" => output::html::to_html(&doc),
        "text" | "txt" => output::text::to_text(&doc),
        _ => output::json::to_json_string(&doc),
    };

    result.map_err(|e| JsError::new(&e.to_string()))
}

/// Return the edgeparse version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn build_config(
    _format: Option<&str>,
    pages: Option<&str>,
    reading_order: Option<&str>,
    table_method: Option<&str>,
) -> ProcessingConfig {
    let mut config = ProcessingConfig::default();
    config.image_output = ImageOutput::Off;

    if let Some(p) = pages {
        if p != "all" {
            config.pages = Some(p.to_string());
        }
    }

    if let Some(ro) = reading_order {
        config.reading_order = match ro {
            "off" | "none" => ReadingOrder::Off,
            _ => ReadingOrder::XyCut,
        };
    }

    if let Some(tm) = table_method {
        config.table_method = match tm {
            "cluster" => TableMethod::Cluster,
            _ => TableMethod::Default,
        };
    }

    config
}
