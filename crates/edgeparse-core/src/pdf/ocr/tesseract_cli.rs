//! Tesseract / RapidOCR CLI-backed [`super::OcrEngine`].

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use image::GrayImage;
use serde::Deserialize;

use super::{OcrEngine, OcrWord};

const TESSERACT_EFFECTIVE_DPI: u32 = 300;
const MIN_OCR_WORD_CONFIDENCE: f64 = 6.0;
const MAX_OCR_WORD_CONFIDENCE: f64 = 101.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliOcrKind {
    Tesseract,
    RapidOcr,
}

#[derive(Debug, Deserialize)]
struct RapidOcrLine {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    text: String,
    confidence: f64,
}

static CLI_KIND: OnceLock<CliOcrKind> = OnceLock::new();
static RAPIDOCR_PYTHON: OnceLock<Option<String>> = OnceLock::new();

const RAPIDOCR_RUNNER: &str = r#"
import json, sys
from rapidocr import RapidOCR

engine = RapidOCR()
result = engine(sys.argv[1], use_det=True, use_cls=True, use_rec=True)

if result is None:
    print('[]')
    raise SystemExit(0)

boxes = getattr(result, 'boxes', []) or []
txts = getattr(result, 'txts', []) or []
scores = getattr(result, 'scores', []) or []
out = []
for box, text, score in zip(boxes, txts, scores):
    if not text or not str(text).strip():
        continue
    xs = [pt[0] for pt in box]
    ys = [pt[1] for pt in box]
    out.append({
        'left': int(min(xs)),
        'top': int(min(ys)),
        'width': max(1, int(max(xs) - min(xs))),
        'height': max(1, int(max(ys) - min(ys))),
        'text': str(text),
        'confidence': float(score),
    })
print(json.dumps(out, ensure_ascii=False))
"#;

/// Shells out to Tesseract (or RapidOCR via Python when `EDGEPARSE_OCR_ENGINE=rapidocr`).
#[derive(Debug, Clone)]
pub struct TesseractCliEngine {
    /// Tesseract page-segmentation mode (ignored for RapidOCR).
    pub psm: String,
    /// Tesseract OCR engine mode.
    pub oem: String,
}

impl Default for TesseractCliEngine {
    fn default() -> Self {
        Self {
            psm: "6".into(),
            oem: "3".into(),
        }
    }
}

impl TesseractCliEngine {
    /// Engine with an explicit PSM mode.
    pub fn with_psm(psm: impl Into<String>) -> Self {
        Self {
            psm: psm.into(),
            oem: "3".into(),
        }
    }
}

impl OcrEngine for TesseractCliEngine {
    fn recognize(&self, gray: &GrayImage) -> Vec<OcrWord> {
        self.recognize_psm(gray, &self.psm)
    }

    fn recognize_psm(&self, gray: &GrayImage, psm: &str) -> Vec<OcrWord> {
        match selected_cli_kind() {
            CliOcrKind::RapidOcr => run_rapidocr_words(gray).unwrap_or_default(),
            CliOcrKind::Tesseract => run_tesseract_tsv(gray, psm, &self.oem).unwrap_or_default(),
        }
    }
}

fn selected_cli_kind() -> CliOcrKind {
    *CLI_KIND.get_or_init(|| match env::var("EDGEPARSE_OCR_ENGINE") {
        Ok(value) => match value.to_ascii_lowercase().as_str() {
            "rapidocr" if rapidocr_python_command().is_some() => CliOcrKind::RapidOcr,
            _ => CliOcrKind::Tesseract,
        },
        Err(_) => CliOcrKind::Tesseract,
    })
}

fn rapidocr_python_command() -> Option<&'static str> {
    RAPIDOCR_PYTHON
        .get_or_init(|| {
            let preferred = env::var("EDGEPARSE_OCR_PYTHON").ok();
            let mut candidates = Vec::new();
            if let Some(cmd) = preferred {
                candidates.push(cmd);
            }
            candidates.push("python3".to_string());
            candidates.push("python".to_string());

            for candidate in candidates {
                let ok = Command::new(&candidate)
                    .arg("-c")
                    .arg("import rapidocr")
                    .output()
                    .ok()
                    .is_some_and(|out| out.status.success());
                if ok {
                    return Some(candidate);
                }
            }
            None
        })
        .as_deref()
}

fn create_temp_dir() -> std::io::Result<PathBuf> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "edgeparse-ocr-cli-{}-{}",
        std::process::id(),
        unique
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn run_tesseract_tsv(image: &GrayImage, psm: &str, oem: &str) -> Option<Vec<OcrWord>> {
    let temp_dir = create_temp_dir().ok()?;
    let image_path = temp_dir.join("ocr.png");
    if image.save(&image_path).is_err() {
        let _ = fs::remove_dir_all(&temp_dir);
        return None;
    }

    let dpi = TESSERACT_EFFECTIVE_DPI.to_string();
    let output = Command::new("tesseract")
        .current_dir(&temp_dir)
        .arg("ocr.png")
        .arg("stdout")
        .arg("--dpi")
        .arg(&dpi)
        .arg("--oem")
        .arg(oem)
        .arg("--psm")
        .arg(psm)
        .arg("-c")
        .arg("load_system_dawg=0")
        .arg("-c")
        .arg("load_freq_dawg=0")
        .arg("tsv")
        .output()
        .ok()?;
    let _ = fs::remove_dir_all(&temp_dir);
    if !output.status.success() {
        return None;
    }

    let tsv = String::from_utf8_lossy(&output.stdout);
    Some(parse_tesseract_tsv(&tsv))
}

pub(crate) fn parse_tesseract_tsv(tsv: &str) -> Vec<OcrWord> {
    let mut words = Vec::new();
    for line in tsv.lines().skip(1) {
        let mut cols = line.splitn(12, '\t');
        let level = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        if level != 5 {
            continue;
        }
        let _page_num = cols.next();
        let block_num = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let par_num = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let line_num = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let _word_num = cols.next();
        let left = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let top = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let width = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let height = cols.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let confidence = cols
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(-1.0);
        let text = cols.next().unwrap_or("").trim().to_string();
        if !(MIN_OCR_WORD_CONFIDENCE..=MAX_OCR_WORD_CONFIDENCE).contains(&confidence)
            || text.is_empty()
            || width == 0
            || height == 0
            || !text.chars().any(|ch| ch.is_alphanumeric())
        {
            continue;
        }
        words.push(OcrWord {
            line_key: (block_num, par_num, line_num),
            left,
            top,
            width,
            height,
            text,
            confidence,
        });
    }
    words
}

fn rapidocr_lines_to_words(lines: Vec<RapidOcrLine>) -> Vec<OcrWord> {
    let mut words = Vec::new();

    for (line_idx, line) in lines.into_iter().enumerate() {
        let tokens: Vec<&str> = line.text.split_whitespace().collect();
        if tokens.is_empty() {
            continue;
        }

        let total_chars: u32 = tokens
            .iter()
            .map(|token| token.chars().count() as u32)
            .sum();
        if total_chars == 0 {
            continue;
        }

        let mut cursor = line.left;
        let mut remaining_width = line.width.max(tokens.len() as u32);
        let mut remaining_chars = total_chars;

        for (token_idx, token) in tokens.iter().enumerate() {
            let token_chars = token.chars().count() as u32;
            let width = if token_idx == tokens.len() - 1 || remaining_chars <= token_chars {
                remaining_width.max(1)
            } else {
                let proportional = ((remaining_width as f64) * (token_chars as f64)
                    / (remaining_chars as f64))
                    .round() as u32;
                proportional.max(1).min(remaining_width)
            };

            words.push(OcrWord {
                line_key: (0, line_idx as u32, 0),
                left: cursor,
                top: line.top,
                width,
                height: line.height.max(1),
                text: (*token).to_string(),
                confidence: line.confidence,
            });

            cursor = cursor.saturating_add(width);
            remaining_width = remaining_width.saturating_sub(width);
            remaining_chars = remaining_chars.saturating_sub(token_chars);
        }
    }

    words
}

fn run_rapidocr_words(image: &GrayImage) -> Option<Vec<OcrWord>> {
    let python = rapidocr_python_command()?;
    let temp_dir = create_temp_dir().ok()?;
    let image_path = temp_dir.join("ocr.png");
    if image.save(&image_path).is_err() {
        let _ = fs::remove_dir_all(&temp_dir);
        return None;
    }

    let output = Command::new(python)
        .current_dir(&temp_dir)
        .arg("-c")
        .arg(RAPIDOCR_RUNNER)
        .arg("ocr.png")
        .output()
        .ok()?;
    let _ = fs::remove_dir_all(&temp_dir);
    if !output.status.success() {
        return None;
    }

    let json = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<RapidOcrLine> = serde_json::from_str(&json).ok()?;
    let words = rapidocr_lines_to_words(lines);
    (!words.is_empty()).then_some(words)
}
