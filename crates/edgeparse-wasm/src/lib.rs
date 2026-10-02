//! EdgeParse WebAssembly entry points.
//!
//! Provides `wasm-bindgen` bindings for browser-based PDF parsing.

use wasm_bindgen::prelude::*;

use edgeparse_core::api::config::{
    HybridBackend, HybridMode, ImageOutput, ProcessingConfig, ReadingOrder, TableMethod,
};
use edgeparse_core::hybrid::apply_hybrid_injected;
use edgeparse_core::output;
use edgeparse_core::pdf::ocr::OcrWord;
use edgeparse_core::{assemble, extract_session, ExtractSession, OcrAssembleMode};

/// Initialize panic hook for better error messages in browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Warn).ok();
}

/// Two-phase parse session: plan (candidates) → host OCR → finish.
///
/// Allows async OCR (ONNX Runtime Web) without blocking the WASM thread on
/// `Atomics.wait` / COOP+COEP. Missing OCR for a candidate degrades to PDF
/// text cells.
#[wasm_bindgen]
pub struct ParseSession {
    inner: Option<ExtractSession>,
}

#[wasm_bindgen]
impl ParseSession {
    /// Open a PDF and extract pages + OCR candidates.
    ///
    /// `opts` may include `{pages, readingOrder, tableMethod, fileName, rasterTableOcr}`.
    /// `rasterTableOcr` (bool) or `ocr: false|"off"` disables raster OCR candidate collection.
    /// `on_progress(phase, done, total)` is called synchronously between pages.
    #[wasm_bindgen]
    pub fn open(
        bytes: &[u8],
        opts: JsValue,
        on_progress: Option<js_sys::Function>,
    ) -> Result<ParseSession, JsError> {
        let config = config_from_opts(&opts);
        let file_name = opts_string(&opts, "fileName").unwrap_or_else(|| "uploaded.pdf".into());

        let mut progress_cb;
        let progress_ref: Option<&mut edgeparse_core::ProgressFn<'_>> = if let Some(f) = on_progress
        {
            progress_cb = move |phase: &str, done: u32, total: u32| {
                let _ = f.call3(
                    &JsValue::NULL,
                    &JsValue::from_str(phase),
                    &JsValue::from(done),
                    &JsValue::from(total),
                );
            };
            Some(&mut progress_cb)
        } else {
            None
        };

        let session = extract_session(bytes, &file_name, &config, progress_ref)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(ParseSession {
            inner: Some(session),
        })
    }

    /// Candidate metadata: `[{id, page, width, height, hash}]`.
    #[wasm_bindgen]
    pub fn candidates(&self) -> Result<JsValue, JsError> {
        let session = self
            .inner
            .as_ref()
            .ok_or_else(|| JsError::new("session already finished"))?;
        let metas = session.candidate_metas();
        serde_wasm_bindgen::to_value(&metas).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Grayscale pixels for candidate `id` (row-major, length = width*height).
    #[wasm_bindgen]
    pub fn candidate_gray(&self, id: u32) -> Result<Vec<u8>, JsError> {
        let session = self
            .inner
            .as_ref()
            .ok_or_else(|| JsError::new("session already finished"))?;
        session
            .candidate_gray(id)
            .map(|g| g.to_vec())
            .ok_or_else(|| JsError::new(&format!("unknown candidate id {id}")))
    }

    /// Provide OCR words for a candidate.
    ///
    /// `words` is an array of `{text,left,top,width,height,confidence?}`.
    #[wasm_bindgen]
    pub fn provide_ocr(&mut self, id: u32, words: JsValue) -> Result<(), JsError> {
        let session = self
            .inner
            .as_mut()
            .ok_or_else(|| JsError::new("session already finished"))?;
        let parsed: Vec<JsOcrWord> = serde_wasm_bindgen::from_value(words)
            .map_err(|e| JsError::new(&format!("invalid OCR words: {e}")))?;
        let ocr: Vec<OcrWord> = parsed
            .into_iter()
            .map(|w| OcrWord {
                line_key: (0, 0, 0),
                left: w.left,
                top: w.top,
                width: w.width,
                height: w.height,
                text: w.text,
                confidence: w.confidence.unwrap_or(0.0),
            })
            .collect();
        session.provide_ocr(id, ocr);
        Ok(())
    }

    /// Assemble and return all common formats as a JS object.
    #[wasm_bindgen(js_name = finishAll)]
    pub fn finish_all(&mut self) -> Result<JsValue, JsError> {
        let session = self
            .inner
            .take()
            .ok_or_else(|| JsError::new("session already finished"))?;
        let doc = assemble(session, OcrAssembleMode::Provided)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let json = {
            let stem = doc
                .file_name
                .trim_end_matches(".pdf")
                .trim_end_matches(".PDF");
            output::legacy_json::to_legacy_json_string(&doc, stem)
                .map_err(|e| JsError::new(&e.to_string()))?
        };
        let markdown =
            output::markdown::to_markdown(&doc).map_err(|e| JsError::new(&e.to_string()))?;
        let html = output::html::to_html(&doc).map_err(|e| JsError::new(&e.to_string()))?;
        let text = output::text::to_text(&doc).map_err(|e| JsError::new(&e.to_string()))?;
        let payload = serde_json::json!({
            "json": json,
            "markdown": markdown,
            "html": html,
            "text": text,
        });
        serde_wasm_bindgen::to_value(&payload).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Assemble the document and return formatted output.
    #[wasm_bindgen]
    pub fn finish(&mut self, format: &str) -> Result<String, JsError> {
        let session = self
            .inner
            .take()
            .ok_or_else(|| JsError::new("session already finished"))?;
        let doc = assemble(session, OcrAssembleMode::Provided)
            .map_err(|e| JsError::new(&e.to_string()))?;
        format_doc(&doc, format)
    }

    /// Assemble with optional host-injected hybrid backend markdown.
    #[wasm_bindgen]
    pub fn finish_hybrid(
        &mut self,
        backend_md: Option<String>,
        format: &str,
    ) -> Result<String, JsError> {
        let mut session = self
            .inner
            .take()
            .ok_or_else(|| JsError::new("session already finished"))?;
        session.config.hybrid = HybridBackend::DoclingFast;
        session.config.hybrid_mode = HybridMode::Auto;
        session.config.hybrid_fallback = true;
        let config = session.config.clone();
        let doc = assemble(session, OcrAssembleMode::Provided)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let hybrid = apply_hybrid_injected(&doc, &config, backend_md.as_deref(), None);
        if let Some(md) = hybrid.markdown_override {
            if matches!(format, "markdown" | "md") {
                return Ok(md);
            }
        }
        format_doc(&doc, format)
    }
}

#[derive(serde::Deserialize)]
struct JsOcrWord {
    text: String,
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    #[serde(default)]
    confidence: Option<f64>,
}

fn format_doc(
    doc: &edgeparse_core::models::document::PdfDocument,
    format: &str,
) -> Result<String, JsError> {
    let result = match format {
        "markdown" | "md" => output::markdown::to_markdown(doc),
        "html" => output::html::to_html(doc),
        "text" | "txt" => output::text::to_text(doc),
        // Match the CLI: compact legacy JSON, not the internal pretty model.
        "json" => {
            let stem = doc
                .file_name
                .trim_end_matches(".pdf")
                .trim_end_matches(".PDF");
            output::legacy_json::to_legacy_json_string(doc, stem)
        }
        _ => {
            let stem = doc
                .file_name
                .trim_end_matches(".pdf")
                .trim_end_matches(".PDF");
            output::legacy_json::to_legacy_json_string(doc, stem)
        }
    };
    result.map_err(|e| JsError::new(&e.to_string()))
}

fn opts_string(opts: &JsValue, key: &str) -> Option<String> {
    if opts.is_null() || opts.is_undefined() {
        return None;
    }
    let obj = js_sys::Object::try_from(opts)?;
    let v = js_sys::Reflect::get(obj, &JsValue::from_str(key)).ok()?;
    v.as_string()
}

/// Read a boolean option. Also treats string `"off"` / `"false"` / `"0"` as false
/// and `"on"` / `"true"` / `"1"` as true.
fn opts_bool(opts: &JsValue, key: &str) -> Option<bool> {
    if opts.is_null() || opts.is_undefined() {
        return None;
    }
    let obj = js_sys::Object::try_from(opts)?;
    let v = js_sys::Reflect::get(obj, &JsValue::from_str(key)).ok()?;
    if v.is_undefined() || v.is_null() {
        return None;
    }
    if let Some(b) = v.as_bool() {
        return Some(b);
    }
    if let Some(s) = v.as_string() {
        return match s.to_ascii_lowercase().as_str() {
            "off" | "false" | "0" | "no" => Some(false),
            "on" | "true" | "1" | "yes" => Some(true),
            _ => None,
        };
    }
    None
}

/// Effective raster OCR enable from opts (`rasterTableOcr`, or alias `ocr`).
fn opts_raster_table_ocr(opts: &JsValue) -> Option<bool> {
    if let Some(b) = opts_bool(opts, "rasterTableOcr") {
        return Some(b);
    }
    // Alias: `ocr: false` / `"off"` disables; `ocr: true` enables.
    // String model tiers like "small" are ignored here (handled by the JS SDK).
    if let Some(b) = opts_bool(opts, "ocr") {
        return Some(b);
    }
    None
}

fn config_from_opts(opts: &JsValue) -> ProcessingConfig {
    let pages = opts_string(opts, "pages");
    let reading_order = opts_string(opts, "readingOrder");
    let table_method = opts_string(opts, "tableMethod");
    let mut config = build_config(
        None,
        pages.as_deref(),
        reading_order.as_deref(),
        table_method.as_deref(),
        None,
    );
    if let Some(enabled) = opts_raster_table_ocr(opts) {
        config.raster_table_ocr = enabled;
    }
    config
}

/// Convert PDF bytes to a structured document object (returned as JS value).
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
        None,
    );

    let doc = edgeparse_core::convert_bytes(pdf_bytes, "uploaded.pdf", &config)
        .map_err(|e| JsError::new(&e.to_string()))?;

    serde_wasm_bindgen::to_value(&doc).map_err(|e| JsError::new(&e.to_string()))
}

/// Convert PDF bytes to a formatted output string.
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
        None,
    );

    let doc = edgeparse_core::convert_bytes(pdf_bytes, "uploaded.pdf", &config)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let fmt = format.as_deref().unwrap_or("json");
    format_doc(&doc, fmt)
}

/// Hybrid convert: local WASM pipeline + optional host-injected backend markdown.
#[wasm_bindgen]
pub fn convert_hybrid(
    pdf_bytes: &[u8],
    backend_markdown: Option<String>,
    format: Option<String>,
    pages: Option<String>,
    reading_order: Option<String>,
    table_method: Option<String>,
) -> Result<String, JsError> {
    let mut config = build_config(
        format.as_deref(),
        pages.as_deref(),
        reading_order.as_deref(),
        table_method.as_deref(),
        None,
    );
    config.hybrid = HybridBackend::DoclingFast;
    config.hybrid_mode = HybridMode::Auto;
    config.hybrid_fallback = true;

    let doc = edgeparse_core::convert_bytes(pdf_bytes, "uploaded.pdf", &config)
        .map_err(|e| JsError::new(&e.to_string()))?;

    let hybrid = apply_hybrid_injected(&doc, &config, backend_markdown.as_deref(), None);

    let fmt = format.as_deref().unwrap_or("markdown");
    if let Some(md) = hybrid.markdown_override {
        if matches!(fmt, "markdown" | "md") {
            return Ok(md);
        }
    }

    format_doc(&doc, fmt)
}

/// Return the edgeparse version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Load ocrs detection + recognition model bytes (feature `ocr-ocrs`).
#[cfg(feature = "ocr-ocrs")]
#[wasm_bindgen]
pub fn init_ocrs_models(detection: &[u8], recognition: &[u8]) -> Result<(), JsError> {
    use edgeparse_core::pdf::ocr::OcrsEngine;
    OcrsEngine::try_from_model_bytes(detection, recognition)
        .ok_or_else(|| JsError::new("failed to load ocrs models"))?;
    Ok(())
}

/// Register a synchronous host OCR backend (preferred over in-process ocrs).
#[wasm_bindgen]
pub fn register_ocr_backend(callback: Option<js_sys::Function>) {
    edgeparse_core::pdf::ocr::register_host_ocr_backend(callback);
}

/// True when [`register_ocr_backend`] has an active callback.
#[wasm_bindgen]
pub fn host_ocr_ready() -> bool {
    edgeparse_core::pdf::ocr::host_ocr_registered()
}

/// OCR an RGBA bitmap (host-rendered page or image XObject).
#[wasm_bindgen]
pub fn recognize_rgba(rgba: &[u8], width: u32, height: u32) -> Result<JsValue, JsError> {
    use edgeparse_core::pdf::ocr::{default_engine, OcrEngine};

    if width == 0 || height == 0 {
        return Err(JsError::new("invalid dimensions"));
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| JsError::new("dimension overflow"))?;
    if rgba.len() != expected {
        return Err(JsError::new("rgba length does not match width*height*4"));
    }

    let mut luma = Vec::with_capacity((width * height) as usize);
    for px in rgba.chunks_exact(4) {
        let y = (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
            .round()
            .clamp(0.0, 255.0) as u8;
        luma.push(y);
    }
    let gray = image::GrayImage::from_raw(width, height, luma)
        .ok_or_else(|| JsError::new("invalid gray buffer"))?;

    let words = OcrEngine::recognize(&*default_engine(), &gray);
    if words.is_empty() && !edgeparse_core::pdf::ocr::host_ocr_registered() && {
        #[cfg(feature = "ocr-ocrs")]
        {
            edgeparse_core::pdf::ocr::OcrsEngine::try_from_env().is_none()
        }
        #[cfg(not(feature = "ocr-ocrs"))]
        {
            true
        }
    } {
        return Err(JsError::new(
            "no OCR backend: call register_ocr_backend or init_ocrs_models",
        ));
    }
    let payload: Vec<serde_json::Value> = words
        .into_iter()
        .map(|w| {
            serde_json::json!({
                "text": w.text,
                "left": w.left,
                "top": w.top,
                "width": w.width,
                "height": w.height,
                "confidence": w.confidence,
            })
        })
        .collect();
    serde_wasm_bindgen::to_value(&payload).map_err(|e| JsError::new(&e.to_string()))
}

fn build_config(
    _format: Option<&str>,
    pages: Option<&str>,
    reading_order: Option<&str>,
    table_method: Option<&str>,
    raster_table_ocr: Option<bool>,
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

    if let Some(enabled) = raster_table_ocr {
        config.raster_table_ocr = enabled;
    }

    config
}
