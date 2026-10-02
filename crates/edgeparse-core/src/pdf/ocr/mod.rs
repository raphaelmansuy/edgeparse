//! Pluggable raster OCR engines for table recovery.
//!
//! Dependency inversion: lattice / cell bucketing consume [`OcrWord`] lists;
//! only the recognizer implementation changes.
//!
//! - [`TesseractCliEngine`] — default on native (Tesseract / RapidOCR CLI).
//! - [`OcrsEngine`] — pure-Rust RTen models behind feature `ocr-ocrs`.
//! - [`HostOcrEngine`] — wasm32 host callback (PP-OCRv6 etc.).

#[cfg(all(target_arch = "wasm32", feature = "image"))]
mod host_engine;
#[cfg(feature = "ocr-ocrs")]
mod ocrs_engine;
#[cfg(not(target_arch = "wasm32"))]
mod tesseract_cli;

#[cfg(all(target_arch = "wasm32", feature = "image"))]
pub use host_engine::{host_ocr_registered, register_host_ocr_backend, HostOcrEngine};
#[cfg(feature = "ocr-ocrs")]
pub use ocrs_engine::OcrsEngine;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use tesseract_cli::parse_tesseract_tsv;
#[cfg(not(target_arch = "wasm32"))]
pub use tesseract_cli::TesseractCliEngine;

use image::GrayImage;
use std::sync::Arc;

/// One recognized word with axis-aligned box in image pixel space.
#[derive(Debug, Clone)]
pub struct OcrWord {
    /// Layout grouping key (block, paragraph, line) — Tesseract TSV semantics.
    pub line_key: (u32, u32, u32),
    /// Left edge in pixels.
    pub left: u32,
    /// Top edge in pixels.
    pub top: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Recognized text.
    pub text: String,
    /// Engine confidence (0–100 scale preferred).
    pub confidence: f64,
}

/// Raster OCR provider (SOLID: depend on this abstraction, not a CLI spawn).
pub trait OcrEngine: Send + Sync {
    /// Recognize words in a grayscale image (engine-default page segmentation).
    fn recognize(&self, gray: &GrayImage) -> Vec<OcrWord>;

    /// Optional PSM / layout hint for CLI engines; ignored by neural engines.
    fn recognize_psm(&self, gray: &GrayImage, _psm: &str) -> Vec<OcrWord> {
        self.recognize(gray)
    }
}

/// Default engine for the current build / env.
///
/// Preference: host callback (wasm) → ocrs → Tesseract CLI (native) → null.
/// When a host is registered but returns no words, ocrs is tried next.
pub fn default_engine() -> Arc<dyn OcrEngine> {
    #[cfg(all(target_arch = "wasm32", feature = "image"))]
    {
        if HostOcrEngine::try_new().is_some() {
            #[cfg(feature = "ocr-ocrs")]
            let secondary: Option<Arc<dyn OcrEngine>> =
                OcrsEngine::try_from_env().map(|e| Arc::new(e) as Arc<dyn OcrEngine>);
            #[cfg(not(feature = "ocr-ocrs"))]
            let secondary: Option<Arc<dyn OcrEngine>> = None;
            return Arc::new(FallbackOcrEngine {
                primary: Arc::new(HostOcrEngine),
                secondary,
            });
        }
    }
    #[cfg(feature = "ocr-ocrs")]
    {
        if let Some(engine) = OcrsEngine::try_from_env() {
            return Arc::new(engine);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Arc::new(TesseractCliEngine::default())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Arc::new(NullOcrEngine)
    }
}

/// Try `primary`, then `secondary` if the first yields no words.
#[cfg(all(target_arch = "wasm32", feature = "image"))]
struct FallbackOcrEngine {
    primary: Arc<dyn OcrEngine>,
    secondary: Option<Arc<dyn OcrEngine>>,
}

#[cfg(all(target_arch = "wasm32", feature = "image"))]
impl OcrEngine for FallbackOcrEngine {
    fn recognize(&self, gray: &GrayImage) -> Vec<OcrWord> {
        let words = self.primary.recognize(gray);
        if !words.is_empty() {
            return words;
        }
        if let Some(sec) = &self.secondary {
            return sec.recognize(gray);
        }
        Vec::new()
    }
}

#[cfg(target_arch = "wasm32")]
struct NullOcrEngine;

#[cfg(target_arch = "wasm32")]
impl OcrEngine for NullOcrEngine {
    fn recognize(&self, _gray: &GrayImage) -> Vec<OcrWord> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic engine for wiring tests (no CLI / models).
    struct FakeOcrEngine;

    impl OcrEngine for FakeOcrEngine {
        fn recognize(&self, _gray: &GrayImage) -> Vec<OcrWord> {
            vec![
                OcrWord {
                    line_key: (1, 1, 1),
                    left: 10,
                    top: 10,
                    width: 80,
                    height: 14,
                    text: "Tube".into(),
                    confidence: 90.0,
                },
                OcrWord {
                    line_key: (1, 1, 1),
                    left: 145,
                    top: 10,
                    width: 110,
                    height: 14,
                    text: "Enzyme".into(),
                    confidence: 90.0,
                },
                OcrWord {
                    line_key: (1, 1, 2),
                    left: 10,
                    top: 42,
                    width: 80,
                    height: 14,
                    text: "1".into(),
                    confidence: 90.0,
                },
                OcrWord {
                    line_key: (1, 1, 2),
                    left: 145,
                    top: 42,
                    width: 110,
                    height: 14,
                    text: "BamHI".into(),
                    confidence: 90.0,
                },
            ]
        }
    }

    #[test]
    fn fake_engine_returns_columnar_words() {
        let img = GrayImage::new(400, 100);
        let words = FakeOcrEngine.recognize(&img);
        assert_eq!(words.len(), 4);
        assert_eq!(words[0].text, "Tube");
        assert_eq!(words[1].text, "Enzyme");
        let lefts: Vec<_> = words.iter().map(|w| w.left).collect();
        assert!(lefts.contains(&10));
        assert!(lefts.contains(&145));
    }
}
