//! Host-provided OCR for wasm32 (PP-OCRv6 / tesseract-wasm / etc.).
//!
//! The JS host registers a synchronous callback via
//! [`register_host_ocr_backend`]. Prefer this over in-process ocrs when set.
//!
//! Callback storage is `thread_local` because `js_sys::Function` is not `Send`
//! (wasm32 is single-threaded for this crate).

use std::cell::RefCell;

use image::GrayImage;
use js_sys::{Array, Function, Reflect, Uint8Array};
use wasm_bindgen::JsValue;

use super::{OcrEngine, OcrWord};

thread_local! {
    static HOST_BACKEND: RefCell<Option<Function>> = const { RefCell::new(None) };
}

/// Install (or clear with `None`) the host OCR callback.
///
/// Callback signature (sync):
/// `(gray: Uint8Array, width: number, height: number) => Array<{text,left,top,width,height,confidence?}>`
pub fn register_host_ocr_backend(callback: Option<Function>) {
    HOST_BACKEND.with(|slot| {
        *slot.borrow_mut() = callback;
    });
}

/// True when a host callback is installed.
pub fn host_ocr_registered() -> bool {
    HOST_BACKEND.with(|slot| slot.borrow().is_some())
}

/// OCR engine that forwards gray bitmaps to the registered JS callback.
pub struct HostOcrEngine;

impl HostOcrEngine {
    /// Returns `Some` when a host OCR callback is registered.
    pub fn try_new() -> Option<Self> {
        if host_ocr_registered() {
            Some(Self)
        } else {
            None
        }
    }
}

impl OcrEngine for HostOcrEngine {
    fn recognize(&self, gray: &GrayImage) -> Vec<OcrWord> {
        HOST_BACKEND.with(|slot| {
            let borrowed = slot.borrow();
            let Some(cb) = borrowed.as_ref() else {
                return Vec::new();
            };

            let width = gray.width();
            let height = gray.height();
            let pixels: Vec<u8> = gray.pixels().map(|p| p.0[0]).collect();
            let ua = Uint8Array::new_with_length(pixels.len() as u32);
            ua.copy_from(&pixels);

            let Ok(raw) = cb.call3(
                &JsValue::NULL,
                &ua.into(),
                &JsValue::from_f64(f64::from(width)),
                &JsValue::from_f64(f64::from(height)),
            ) else {
                return Vec::new();
            };
            if raw.is_undefined() || raw.is_null() {
                return Vec::new();
            }
            let Ok(arr) = Array::try_from(raw) else {
                return Vec::new();
            };

            let mut out = Vec::with_capacity(arr.length() as usize);
            for i in 0..arr.length() {
                let item = arr.get(i);
                if item.is_undefined() || item.is_null() {
                    continue;
                }
                let text = Reflect::get(&item, &JsValue::from_str("text"))
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();
                if text.trim().is_empty() {
                    continue;
                }
                let left = js_u32(&item, "left");
                let top = js_u32(&item, "top");
                let w = js_u32(&item, "width").max(1);
                let h = js_u32(&item, "height").max(1);
                let confidence = Reflect::get(&item, &JsValue::from_str("confidence"))
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(80.0);
                out.push(OcrWord {
                    line_key: (0, 0, i),
                    left,
                    top,
                    width: w,
                    height: h,
                    text,
                    confidence,
                });
            }
            out
        })
    }
}

fn js_u32(obj: &JsValue, key: &str) -> u32 {
    Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
        .max(0.0) as u32
}
