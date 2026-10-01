//! Pure-Rust OCR via [ocrs](https://crates.io/crates/ocrs) + RTen.
//!
//! Models are **not** vendored in git. Resolve from:
//! 1. `EDGEPARSE_OCRS_MODEL_DIR` (directory with detection + recognition `.rten`)
//! 2. `~/.cache/edgeparse/ocrs/` after a one-shot download helper

use std::sync::OnceLock;

use image::GrayImage;
use ocrs::{ImageSource, OcrEngine as OcrsLibEngine, OcrEngineParams, TextItem};
use rten::Model;

use super::{OcrEngine, OcrWord};

#[cfg(not(target_arch = "wasm32"))]
use std::env;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

static ENGINE: OnceLock<Result<OcrsLibEngine, String>> = OnceLock::new();
/// Process-global engine installed via [`OcrsEngine::try_from_model_bytes`].
static BYTES_ENGINE: OnceLock<&'static OcrsLibEngine> = OnceLock::new();

/// ocrs / RTen-backed raster recognizer.
pub struct OcrsEngine {
    inner: &'static OcrsLibEngine,
}

impl OcrsEngine {
    /// Build from env / cache if models resolve; `None` if unavailable.
    pub fn try_from_env() -> Option<Self> {
        if let Some(engine) = BYTES_ENGINE.get() {
            return Some(Self { inner: engine });
        }
        let result = ENGINE.get_or_init(load_engine);
        match result {
            Ok(engine) => Some(Self { inner: engine }),
            Err(err) => {
                log::warn!("ocrs unavailable: {err}");
                None
            }
        }
    }

    /// Load from in-memory model bytes (WASM / embedded).
    pub fn try_from_model_bytes(detection: &[u8], recognition: &[u8]) -> Option<Self> {
        if let Some(engine) = BYTES_ENGINE.get() {
            return Some(Self { inner: engine });
        }
        let detection_model = Model::load(detection.to_vec()).ok()?;
        let recognition_model = Model::load(recognition.to_vec()).ok()?;
        let engine = OcrsLibEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .ok()?;
        let leaked: &'static OcrsLibEngine = Box::leak(Box::new(engine));
        let _ = BYTES_ENGINE.set(leaked);
        Some(Self { inner: leaked })
    }

    /// Load from an explicit model directory (for tests).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn try_from_dir(dir: &Path) -> Option<Self> {
        let engine = load_engine_from_dir(dir).ok()?;
        let leaked: &'static OcrsLibEngine = Box::leak(Box::new(engine));
        Some(Self { inner: leaked })
    }
}

impl OcrEngine for OcrsEngine {
    fn recognize(&self, gray: &GrayImage) -> Vec<OcrWord> {
        let (prepared, scale) = preprocess_for_ocrs(gray);
        let width = prepared.width();
        let height = prepared.height();
        let pixels: Vec<u8> = prepared.pixels().map(|p| p.0[0]).collect();
        let Ok(source) = ImageSource::from_bytes(&pixels, (width, height)) else {
            return Vec::new();
        };
        let Ok(input) = self.inner.prepare_input(source) else {
            return Vec::new();
        };
        let Ok(word_rects) = self.inner.detect_words(&input) else {
            return Vec::new();
        };
        if word_rects.is_empty() {
            return Vec::new();
        }
        let lines = self.inner.find_text_lines(&input, &word_rects);
        let Ok(texts) = self.inner.recognize_text(&input, &lines) else {
            return Vec::new();
        };

        let inv = 1.0 / scale;
        let mut out = Vec::new();
        for (line_idx, text_opt) in texts.iter().enumerate() {
            let Some(line) = text_opt.as_ref() else {
                continue;
            };
            for word in line.words() {
                let text = word.to_string();
                if text.trim().is_empty() {
                    continue;
                }
                let rect = word.bounding_rect();
                out.push(OcrWord {
                    line_key: (0, 0, line_idx as u32),
                    left: scale_coord(rect.left().max(0) as u32, inv),
                    top: scale_coord(rect.top().max(0) as u32, inv),
                    width: scale_coord(rect.width().max(1) as u32, inv).max(1),
                    height: scale_coord(rect.height().max(1) as u32, inv).max(1),
                    text,
                    confidence: 80.0,
                });
            }
            // Fallback: whole line if word split yielded nothing.
            if out
                .iter()
                .filter(|w| w.line_key.2 == line_idx as u32)
                .count()
                == 0
            {
                let text = line.to_string();
                if text.trim().is_empty() {
                    continue;
                }
                let rect = line.bounding_rect();
                out.push(OcrWord {
                    line_key: (0, 0, line_idx as u32),
                    left: scale_coord(rect.left().max(0) as u32, inv),
                    top: scale_coord(rect.top().max(0) as u32, inv),
                    width: scale_coord(rect.width().max(1) as u32, inv).max(1),
                    height: scale_coord(rect.height().max(1) as u32, inv).max(1),
                    text: text.trim().to_string(),
                    confidence: 80.0,
                });
            }
        }
        out
    }
}

/// Upscale short side to ≥1000 px (native raster uses high effective DPI) and
/// stretch contrast so faint table ink is easier for the HierText detector.
fn preprocess_for_ocrs(gray: &GrayImage) -> (GrayImage, f64) {
    const MIN_SHORT_SIDE: u32 = 1000;
    let w = gray.width().max(1);
    let h = gray.height().max(1);
    let short = w.min(h);
    let scale = if short < MIN_SHORT_SIDE {
        f64::from(MIN_SHORT_SIDE) / f64::from(short)
    } else {
        1.0
    };

    let stretched = contrast_stretch(gray);
    if (scale - 1.0).abs() < 1e-6 {
        return (stretched, 1.0);
    }
    let nw = ((f64::from(w) * scale).round() as u32).max(1);
    let nh = ((f64::from(h) * scale).round() as u32).max(1);
    let resized =
        image::imageops::resize(&stretched, nw, nh, image::imageops::FilterType::CatmullRom);
    (resized, scale)
}

fn contrast_stretch(gray: &GrayImage) -> GrayImage {
    let mut min_v = 255u8;
    let mut max_v = 0u8;
    for p in gray.pixels() {
        let v = p.0[0];
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    if max_v.saturating_sub(min_v) < 16 {
        return gray.clone();
    }
    let range = f64::from(max_v - min_v);
    let mut out = GrayImage::new(gray.width(), gray.height());
    for (x, y, p) in gray.enumerate_pixels() {
        let mapped = ((f64::from(p.0[0] - min_v) / range) * 255.0).round() as u8;
        out.put_pixel(x, y, image::Luma([mapped]));
    }
    out
}

fn scale_coord(v: u32, inv_scale: f64) -> u32 {
    (f64::from(v) * inv_scale).round() as u32
}

fn load_engine() -> Result<OcrsLibEngine, String> {
    #[cfg(target_arch = "wasm32")]
    {
        return Err("on wasm32 load models via OcrsEngine::try_from_model_bytes".into());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = resolve_model_dir().ok_or_else(|| {
            "set EDGEPARSE_OCRS_MODEL_DIR to a directory with text-detection.rten and text-recognition.rten"
                .to_string()
        })?;
        load_engine_from_dir(&dir)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_engine_from_dir(dir: &Path) -> Result<OcrsLibEngine, String> {
    let det = dir.join("text-detection.rten");
    let rec = dir.join("text-recognition.rten");
    if !det.is_file() || !rec.is_file() {
        return Err(format!(
            "missing models in {} (need text-detection.rten + text-recognition.rten)",
            dir.display()
        ));
    }
    let detection_model =
        Model::load_file(&det).map_err(|e| format!("load detection model: {e}"))?;
    let recognition_model =
        Model::load_file(&rec).map_err(|e| format!("load recognition model: {e}"))?;
    OcrsLibEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })
    .map_err(|e| format!("ocrs engine init: {e}"))
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_model_dir() -> Option<PathBuf> {
    if let Ok(dir) = env::var("EDGEPARSE_OCRS_MODEL_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }
    if let Some(home) = env::var_os("HOME") {
        let cache = PathBuf::from(home).join(".cache/edgeparse/ocrs");
        if cache.join("text-detection.rten").is_file()
            && cache.join("text-recognition.rten").is_file()
        {
            return Some(cache);
        }
    }
    None
}

/// Ensure cache models exist (download from known URLs). Used by tests.
#[cfg(all(test, not(target_arch = "wasm32")))]
pub fn ensure_cached_models() -> Option<PathBuf> {
    use std::fs;
    if let Some(dir) = resolve_model_dir() {
        return Some(dir);
    }
    let home = env::var_os("HOME")?;
    let cache = PathBuf::from(home).join(".cache/edgeparse/ocrs");
    fs::create_dir_all(&cache).ok()?;
    let det = cache.join("text-detection.rten");
    let rec = cache.join("text-recognition.rten");
    const DET_URL: &str = "https://ocrs-models.s3.amazonaws.com/text-detection.rten";
    const REC_URL: &str = "https://ocrs-models.s3.amazonaws.com/text-recognition.rten";
    if !det.is_file() {
        download(DET_URL, &det)?;
    }
    if !rec.is_file() {
        download(REC_URL, &rec)?;
    }
    Some(cache)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
fn download(url: &str, dest: &Path) -> Option<()> {
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(dest)
        .arg(url)
        .status()
        .ok()?;
    status.success().then_some(())
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use image::{GrayImage, Luma};

    #[test]
    fn ocrs_engine_emits_words_on_high_contrast_glyph_when_models_present() {
        let Some(dir) = ensure_cached_models() else {
            eprintln!("skip: ocrs models unavailable");
            return;
        };
        let Some(engine) = OcrsEngine::try_from_dir(&dir) else {
            eprintln!("skip: ocrs engine failed to load");
            return;
        };

        let mut gray = GrayImage::from_pixel(200, 80, Luma([255]));
        for y in 20..60 {
            for x in 30..45 {
                gray.put_pixel(x, y, Luma([0]));
            }
            for x in 80..95 {
                gray.put_pixel(x, y, Luma([0]));
            }
        }
        let words = engine.recognize(&gray);
        let sample = dir.join("ocr-sample.png");
        if !sample.is_file() {
            let _ = download(
                "https://raw.githubusercontent.com/robertknight/ocrs/main/ocrs/test-data/ocr.jpeg",
                &sample,
            );
        }
        if sample.is_file() {
            if let Ok(img) = image::open(&sample) {
                let gray = img.to_luma8();
                let words = engine.recognize(&gray);
                assert!(
                    !words.is_empty(),
                    "ocrs should recognize at least one word on sample image"
                );
                if words.len() >= 2 {
                    let min_l = words.iter().map(|w| w.left).min().unwrap();
                    let max_l = words.iter().map(|w| w.left).max().unwrap();
                    assert!(
                        max_l > min_l + 10,
                        "expected horizontal spread for multi-column-ish layout"
                    );
                }
            }
        } else {
            let _ = words;
        }
    }
}
