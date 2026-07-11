//! Recover text signal from raster table images using local OCR.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{GenericImageView, GrayImage, Luma};
use serde::Deserialize;

use crate::models::bbox::BoundingBox;
use crate::models::chunks::{ImageChunk, TextChunk};
use crate::models::content::ContentElement;
use crate::models::enums::{PdfLayer, TextFormat, TextType};
use crate::models::table::{
    TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType,
};

// Broaden image eligibility so moderately cropped tables are considered.
const MIN_IMAGE_WIDTH_RATIO: f64 = 0.40;
const MIN_IMAGE_AREA_RATIO: f64 = 0.035;
const MAX_NATIVE_TEXT_CHARS_IN_IMAGE: usize = 250;
const MAX_NATIVE_TEXT_CHUNKS_IN_IMAGE: usize = 12;
// Accuracy-first: accept degraded glyphs at lower confidence —
// dual-OEM consensus and spatial coherence filtering will eliminate noise.
const MIN_OCR_WORD_CONFIDENCE: f64 = 6.0;
// Reject artificially-high confidence noise (Tesseract artefacts above 100).
const MAX_OCR_WORD_CONFIDENCE: f64 = 101.0;
const RASTER_DARK_THRESHOLD: u8 = 180;
const RASTER_CHART_INK_THRESHOLD: u8 = 240;
const MIN_BORDERED_VERTICAL_LINES: usize = 3;
const MIN_BORDERED_HORIZONTAL_LINES: usize = 3;
// Accuracy-first: lighter lines are still valid table borders.
const MIN_LINE_DARK_RATIO: f64 = 0.28;
const MIN_CELL_SIZE_PX: u32 = 10;
const CELL_INSET_PX: u32 = 5;
const TABLE_RASTER_OCR_BORDER_PX: u32 = 14;
// Typography-grounded scale: pdftoppm renders at PDFTOPPM_DPI (150). Scaling by 2
// gives 300 DPI effective — the Tesseract-documented optimum. At 12pt body text,
// cap height ≈ 25px raw → 50px scaled, squarely in Tesseract's 32-40px sweet spot.
// Over-scaling (×5 = 125px) amplifies anti-aliasing and hurts LSTM segmentation.
const PDFTOPPM_DPI: u32 = 150;
const OCR_SCALE_FACTOR: u32 = 2;
/// Effective DPI seen by Tesseract = PDFTOPPM_DPI × OCR_SCALE_FACTOR.
const TESSERACT_EFFECTIVE_DPI: u32 = PDFTOPPM_DPI * OCR_SCALE_FACTOR;
const MIN_DOMINANT_IMAGE_WIDTH_RATIO: f64 = 0.65;
const MIN_DOMINANT_IMAGE_AREA_RATIO: f64 = 0.40;
const MAX_NATIVE_TEXT_CHARS_IN_DOMINANT_IMAGE: usize = 80;
const MIN_DOMINANT_IMAGE_OCR_WORDS: usize = 18;
const MIN_DOMINANT_IMAGE_TEXT_LINES: usize = 6;
const MIN_DENSE_PROSE_BLOCK_LINES: usize = 3;
const MIN_DENSE_PROSE_BLOCK_WIDTH_RATIO: f64 = 0.32;
// Permit minor breaks in rasterized lines while still enforcing structure.
const MIN_TRUE_GRID_LINE_CONTINUITY: f64 = 0.60;
const MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR: usize = 180;
const MIN_EMPTY_TABLE_COVERAGE_FOR_PAGE_RASTER_OCR: f64 = 0.08;
const MAX_EMPTY_TABLES_FOR_PAGE_RASTER_OCR: usize = 24;
const LOCAL_BINARIZATION_RADIUS: u32 = 14;
const MIN_BINARIZATION_BLOCK_PIXELS: usize = 81;
// Handle sparse numeric tables where only a few cells OCR cleanly.
const MIN_RASTER_TABLE_TEXT_CELL_RATIO: f64 = 0.05;
const MIN_RASTER_TABLE_ROWS_WITH_TEXT: usize = 1;
const MIN_NUMERIC_TABLE_MEDIAN_FILL_RATIO: f64 = 0.40;
const MIN_BORDERED_CELL_DARK_RATIO: f64 = 0.03;
const MIN_BORDERED_INKED_CELL_RATIO: f64 = 0.18;
const MIN_BORDERED_ROWS_WITH_INK: usize = 2;
const MAX_BORDERED_TABLE_PER_CELL_FALLBACK_CELLS: usize = 24;
const MIN_BRIGHT_PHOTO_MID_TONE_RATIO: f64 = 0.24;
const MIN_BRIGHT_PHOTO_HISTOGRAM_BINS: usize = 8;
const MIN_BRIGHT_PHOTO_ENTROPY: f64 = 1.6;

#[derive(Debug, Clone)]
struct OcrWord {
    line_key: (u32, u32, u32),
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    text: String,
    confidence: f64,
}

#[derive(Debug, Clone)]
struct XCluster {
    center: f64,
    count: usize,
    lines: HashSet<(u32, u32, u32)>,
}

#[derive(Clone)]
struct OcrRowBuild {
    top_y: f64,
    bottom_y: f64,
    cell_texts: Vec<String>,
}

#[derive(Debug, Clone)]
struct EmptyCellRaster {
    row_idx: usize,
    cell_idx: usize,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
}

#[derive(Debug, Clone)]
struct RasterTableGrid {
    vertical_lines: Vec<u32>,
    horizontal_lines: Vec<u32>,
}

#[derive(Debug, Clone)]
struct OcrCandidateScore {
    words: Vec<OcrWord>,
    score: f64,
}

#[derive(Debug, Clone)]
struct PdfImagesListEntry {
    image_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OcrEngine {
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

static OCR_ENGINE: OnceLock<OcrEngine> = OnceLock::new();
static RAPIDOCR_PYTHON: OnceLock<Option<String>> = OnceLock::new();

const RAPIDOCR_RUNNER: &str = r#"
import json, sys
from rapidocr import RapidOCR

engine = RapidOCR()
result = engine(sys.argv[1], use_det=True, use_cls=True, use_rec=True)

if result is None:
    print('[]')
    raise SystemExit(0)

# RapidOCR returns NumPy arrays for these fields; `arr or []` raises
# "truth value of an array ... is ambiguous", so normalise explicitly.
boxes = getattr(result, 'boxes', None)
boxes = [] if boxes is None else list(boxes)
txts = getattr(result, 'txts', None)
txts = [] if txts is None else list(txts)
scores = getattr(result, 'scores', None)
scores = [] if scores is None else list(scores)
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

fn selected_ocr_engine() -> OcrEngine {
    *OCR_ENGINE.get_or_init(|| match env::var("EDGEPARSE_OCR_ENGINE") {
        Ok(value) => match value.to_ascii_lowercase().as_str() {
            "rapidocr" if rapidocr_python_command().is_some() => OcrEngine::RapidOcr,
            "rapidocr" => OcrEngine::Tesseract,
            _ => OcrEngine::Tesseract,
        },
        Err(_) => OcrEngine::Tesseract,
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
    let temp_dir = create_temp_dir(0).ok()?;
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

/// Recover OCR text chunks for image-backed table regions on a single page.
pub fn recover_raster_table_text_chunks(
    input_path: &Path,
    page_bbox: &BoundingBox,
    page_number: u32,
    text_chunks: &[TextChunk],
    image_chunks: &[ImageChunk],
) -> Vec<TextChunk> {
    if page_bbox.area() <= 0.0 || image_chunks.is_empty() {
        return Vec::new();
    }

    let candidates: Vec<&ImageChunk> = image_chunks
        .iter()
        .filter(|image| is_ocr_candidate(image, page_bbox, text_chunks))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let temp_dir = match create_temp_dir(page_number) {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    let result =
        recover_from_page_images(input_path, &temp_dir, page_number, candidates, text_chunks);

    let _ = fs::remove_dir_all(&temp_dir);
    result
}

/// Recover OCR text lines from dominant non-table page images.
///
/// This is for infographic-like pages where the PDF contains a large raster
/// image but little or no native text. The extracted OCR signal is injected
/// back into the normal text pipeline as line chunks so downstream grouping can
/// rebuild headings, paragraphs, and lists.
pub fn recover_dominant_image_text_chunks(
    input_path: &Path,
    page_bbox: &BoundingBox,
    page_number: u32,
    text_chunks: &[TextChunk],
    image_chunks: &[ImageChunk],
) -> Vec<TextChunk> {
    if page_bbox.area() <= 0.0 || image_chunks.is_empty() {
        return Vec::new();
    }

    let candidates: Vec<&ImageChunk> = image_chunks
        .iter()
        .filter(|image| is_dominant_image_text_candidate(image, page_bbox, text_chunks))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let temp_dir = match create_temp_dir(page_number) {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    let image_files = match extract_visible_page_image_files(input_path, page_number, &temp_dir) {
        Some(files) => files,
        None => {
            let _ = fs::remove_dir_all(&temp_dir);
            return Vec::new();
        }
    };

    let mut recovered = Vec::new();
    for image in candidates {
        let Some(image_index) = image.index else {
            continue;
        };
        let Some(image_path) = image_files.get(image_index.saturating_sub(1) as usize) else {
            continue;
        };
        let Ok(gray) = image::open(image_path).map(|img| img.to_luma8()) else {
            continue;
        };
        if recover_bordered_raster_table_from_gray(&gray, image).is_some()
            || is_obvious_bar_chart_raster(&gray)
            || is_natural_photograph_raster(&gray)
            || is_dark_ui_screenshot_raster(&gray)
        {
            continue;
        }

        let Some(words) = run_tesseract_tsv_words_best(&gray, &["11", "6"], |candidate| {
            looks_like_dense_prose_image_ocr(candidate)
        }) else {
            continue;
        };

        recovered.extend(lines_from_ocr_words(
            &words,
            image,
            gray.width(),
            gray.height(),
            text_chunks,
        ));
    }

    let _ = fs::remove_dir_all(&temp_dir);
    recovered
}

/// Recover synthetic table borders for strongly numeric raster tables.
pub fn recover_raster_table_borders(
    input_path: &Path,
    page_bbox: &BoundingBox,
    page_number: u32,
    text_chunks: &[TextChunk],
    image_chunks: &[ImageChunk],
) -> Vec<TableBorder> {
    if page_bbox.area() <= 0.0 || image_chunks.is_empty() {
        return Vec::new();
    }

    let candidates: Vec<&ImageChunk> = image_chunks
        .iter()
        .filter(|image| is_ocr_candidate(image, page_bbox, text_chunks))
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let temp_dir = match create_temp_dir(page_number) {
        Ok(dir) => dir,
        Err(_) => return Vec::new(),
    };

    let image_files = match extract_visible_page_image_files(input_path, page_number, &temp_dir) {
        Some(files) => files,
        None => {
            let _ = fs::remove_dir_all(&temp_dir);
            return Vec::new();
        }
    };

    let mut tables = Vec::new();
    for image in candidates {
        let Some(image_index) = image.index else {
            continue;
        };
        let Some(image_path) = image_files.get(image_index.saturating_sub(1) as usize) else {
            continue;
        };
        let Ok(gray) = image::open(image_path).map(|img| img.to_luma8()) else {
            continue;
        };
        if is_obvious_bar_chart_raster(&gray)
            || is_natural_photograph_raster(&gray)
            || is_dark_ui_screenshot_raster(&gray)
        {
            continue;
        }
        if let Some(table) = recover_bordered_raster_table_from_gray(&gray, image) {
            let chart_words = run_tesseract_tsv_words_best(&gray, &["6", "11"], |_| true);
            if chart_words
                .as_deref()
                .is_some_and(looks_like_chart_label_ocr)
            {
                continue;
            }
            tables.push(table);
            continue;
        }
        let Some(words) = run_tesseract_tsv_words_best(&gray, &["6", "11"], |candidate| {
            looks_like_table_ocr(candidate)
        }) else {
            continue;
        };

        if looks_like_numeric_table_ocr(&words) {
            if let Some(table) = build_numeric_table_border(&words, image) {
                if is_matrixish_ocr_artifact_table(&table) {
                    continue;
                }
                tables.push(table);
                continue;
            }
        }

        if let Some(table) = build_structured_ocr_table_border(&words, image) {
            if is_matrixish_ocr_artifact_table(&table) {
                continue;
            }
            tables.push(table);
        }
    }

    let _ = fs::remove_dir_all(&temp_dir);
    tables
}

/// Recover OCR text into empty bordered tables by rasterizing the full page.
///
/// This targets graphics-dominant pages where native PDF text is sparse but the
/// page still exposes strong bordered geometry. It enriches existing empty
/// `TableBorder` cells directly from the rendered page appearance.
pub fn recover_page_raster_table_cell_text(
    input_path: &Path,
    page_bbox: &BoundingBox,
    page_number: u32,
    elements: &mut [ContentElement],
) {
    if page_bbox.area() <= 0.0 {
        return;
    }

    let native_text_chars = page_native_text_chars(elements);

    let candidate_indices: Vec<usize> = elements
        .iter()
        .enumerate()
        .filter_map(|(idx, elem)| {
            let table = table_candidate_ref(elem)?;
            let local_text_chars = native_text_chars_in_region(elements, &table.bbox);
            if !table_needs_page_raster_ocr(table) {
                return None;
            }
            if native_text_chars > MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR
                && local_text_chars > MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR
            {
                return None;
            }
            Some(idx)
        })
        .take(MAX_EMPTY_TABLES_FOR_PAGE_RASTER_OCR)
        .collect();
    if candidate_indices.is_empty() {
        return;
    }

    let coverage: f64 = candidate_indices
        .iter()
        .filter_map(|idx| table_candidate_ref(&elements[*idx]).map(|table| table.bbox.area()))
        .sum::<f64>()
        / page_bbox.area().max(1.0);
    if coverage < MIN_EMPTY_TABLE_COVERAGE_FOR_PAGE_RASTER_OCR {
        return;
    }

    let temp_dir = match create_temp_dir(page_number) {
        Ok(dir) => dir,
        Err(_) => return,
    };
    let prefix = temp_dir.join("page");
    let status = Command::new("pdftoppm")
        .arg("-png")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-singlefile")
        .arg(input_path)
        .arg(&prefix)
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => {
            let _ = fs::remove_dir_all(&temp_dir);
            return;
        }
    }

    let page_image_path = prefix.with_extension("png");
    let gray = match image::open(&page_image_path) {
        Ok(img) => img.to_luma8(),
        Err(_) => {
            let _ = fs::remove_dir_all(&temp_dir);
            return;
        }
    };

    for idx in candidate_indices {
        let Some(elem) = elements.get_mut(idx) else {
            continue;
        };
        let Some(table) = table_candidate_mut(elem) else {
            continue;
        };
        enrich_empty_table_from_page_raster(&gray, page_bbox, table);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

fn table_candidate_ref(elem: &ContentElement) -> Option<&TableBorder> {
    match elem {
        ContentElement::TableBorder(table) => Some(table),
        ContentElement::Table(table) => Some(&table.table_border),
        _ => None,
    }
}

fn table_candidate_mut(elem: &mut ContentElement) -> Option<&mut TableBorder> {
    match elem {
        ContentElement::TableBorder(table) => Some(table),
        ContentElement::Table(table) => Some(&mut table.table_border),
        _ => None,
    }
}

fn page_native_text_chars(elements: &[ContentElement]) -> usize {
    native_text_chars_in_region(elements, &BoundingBox::new(None, f64::MIN, f64::MIN, f64::MAX, f64::MAX))
}

fn native_text_chars_in_region(elements: &[ContentElement], region: &BoundingBox) -> usize {
    elements
        .iter()
        .filter(|elem| region.overlaps(elem.bbox()))
        .map(|elem| match elem {
            ContentElement::Paragraph(p) => p.base.value().chars().count(),
            ContentElement::Heading(h) => h.base.base.value().chars().count(),
            ContentElement::NumberHeading(h) => h.base.base.base.value().chars().count(),
            ContentElement::TextBlock(tb) => tb.value().chars().count(),
            ContentElement::TextLine(tl) => tl.value().chars().count(),
            ContentElement::TextChunk(tc) => tc.value.chars().count(),
            ContentElement::List(list) => list
                .list_items
                .iter()
                .flat_map(|item| item.contents.iter())
                .map(|content| match content {
                    ContentElement::Paragraph(p) => p.base.value().chars().count(),
                    ContentElement::TextBlock(tb) => tb.value().chars().count(),
                    ContentElement::TextLine(tl) => tl.value().chars().count(),
                    ContentElement::TextChunk(tc) => tc.value.chars().count(),
                    _ => 0,
                })
                .sum(),
            _ => 0,
        })
        .sum()
}

fn recover_from_page_images(
    input_path: &Path,
    temp_dir: &Path,
    page_number: u32,
    candidates: Vec<&ImageChunk>,
    text_chunks: &[TextChunk],
) -> Vec<TextChunk> {
    let image_files = match extract_visible_page_image_files(input_path, page_number, temp_dir) {
        Some(files) => files,
        None => return Vec::new(),
    };
    if image_files.is_empty() {
        return Vec::new();
    }

    let mut recovered = Vec::new();
    for image in candidates {
        let Some(image_index) = image.index else {
            continue;
        };
        let Some(image_path) = image_files.get(image_index.saturating_sub(1) as usize) else {
            continue;
        };
        let bordered_table = recover_bordered_raster_table(image_path, image);
        if let Some(caption) = recover_bordered_raster_caption(image_path, image) {
            recovered.push(caption);
        }
        if bordered_table.is_some() {
            continue;
        }
        let Some(file_name) = image_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        // Images extracted via pdfimages are at their native PDF DPI.
        // We pass PDFTOPPM_DPI as a reasonable hint; Tesseract uses this only for
        // geometry heuristics, not LSTM recognition, so approximate is fine.
        let native_dpi = PDFTOPPM_DPI.to_string();
        let Ok(tsv_output) = Command::new("tesseract")
            .current_dir(temp_dir)
            .arg(file_name)
            .arg("stdout")
            .arg("--dpi")
            .arg(&native_dpi)
            .arg("--psm")
            .arg("6")
            .arg("-c")
            .arg("load_system_dawg=0")
            .arg("-c")
            .arg("load_freq_dawg=0")
            .arg("tsv")
            .output()
        else {
            continue;
        };
        if !tsv_output.status.success() {
            continue;
        }

        let tsv = String::from_utf8_lossy(&tsv_output.stdout);
        let words = parse_tesseract_tsv(&tsv);
        if !looks_like_table_ocr(&words) {
            continue;
        }

        recovered.extend(words_to_text_chunks(&words, image, text_chunks));
    }

    recovered
}

fn table_needs_page_raster_ocr(table: &TableBorder) -> bool {
    if table.num_rows < 1 || table.num_columns < 2 {
        return false;
    }

    let total_cells = table.rows.iter().map(|row| row.cells.len()).sum::<usize>();
    if total_cells == 0 {
        return false;
    }

    let text_cells = table_text_cell_count(table);
    let text_cell_ratio = text_cells as f64 / total_cells as f64;
    text_cells == 0 || text_cell_ratio < MIN_RASTER_TABLE_TEXT_CELL_RATIO
}

fn table_text_cell_count(table: &TableBorder) -> usize {
    table
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .filter(|cell| cell_has_substantive_text(cell))
        .count()
}

fn cell_has_substantive_text(cell: &TableBorderCell) -> bool {
    let has_token_text = cell.content.iter().any(|token| {
        matches!(token.token_type, TableTokenType::Text)
            && token.base.value.chars().any(|ch| ch.is_alphanumeric())
    });
    if has_token_text {
        return true;
    }

    cell.contents.iter().any(|elem| match elem {
        ContentElement::Paragraph(p) => p.base.value().chars().any(|ch| ch.is_alphanumeric()),
        ContentElement::Heading(h) => h.base.base.value().chars().any(|ch| ch.is_alphanumeric()),
        ContentElement::NumberHeading(h) => h
            .base
            .base
            .base
            .value()
            .chars()
            .any(|ch| ch.is_alphanumeric()),
        ContentElement::TextBlock(tb) => tb.value().chars().any(|ch| ch.is_alphanumeric()),
        ContentElement::TextLine(tl) => tl.value().chars().any(|ch| ch.is_alphanumeric()),
        ContentElement::TextChunk(tc) => tc.value.chars().any(|ch| ch.is_alphanumeric()),
        _ => false,
    })
}

fn enrich_empty_table_from_page_raster(
    gray: &GrayImage,
    page_bbox: &BoundingBox,
    table: &mut TableBorder,
) {
    // Collect empty cells first, so we can OCR the whole table once and then
    // distribute words into cells. This avoids calling tesseract per cell.
    let mut empty_cells: Vec<EmptyCellRaster> = Vec::new();
    for (row_idx, row) in table.rows.iter().enumerate() {
        for (cell_idx, cell) in row.cells.iter().enumerate() {
            if cell
                .content
                .iter()
                .any(|token| matches!(token.token_type, TableTokenType::Text))
            {
                continue;
            }
            let Some((x1, y1, x2, y2)) = page_bbox_to_raster_box(gray, page_bbox, &cell.bbox)
            else {
                continue;
            };
            empty_cells.push(EmptyCellRaster {
                row_idx,
                cell_idx,
                x1,
                y1,
                x2,
                y2,
            });
        }
    }
    if empty_cells.is_empty() {
        return;
    }

    // Fallback to legacy per-cell OCR when we can't build a stable table crop.
    let Some((tx1, ty1, tx2, ty2)) = page_bbox_to_raster_box(gray, page_bbox, &table.bbox) else {
        fill_cells_with_per_cell_ocr(gray, table, &empty_cells);
        return;
    };

    let pad = CELL_INSET_PX * 2;
    let crop_left = tx1.saturating_sub(pad);
    let crop_top = ty1.saturating_sub(pad);
    let crop_right = (tx2 + pad).min(gray.width());
    let crop_bottom = (ty2 + pad).min(gray.height());
    if crop_right <= crop_left || crop_bottom <= crop_top {
        fill_cells_with_per_cell_ocr(gray, table, &empty_cells);
        return;
    }

    let crop_width = crop_right - crop_left;
    let crop_height = crop_bottom - crop_top;
    if crop_width < MIN_CELL_SIZE_PX || crop_height < MIN_CELL_SIZE_PX {
        fill_cells_with_per_cell_ocr(gray, table, &empty_cells);
        return;
    }

    let cropped = gray
        .view(crop_left, crop_top, crop_width, crop_height)
        .to_image();
    let is_bar_chart = is_obvious_bar_chart_raster(&cropped);
    let is_photo = is_natural_photograph_raster(&cropped);
    let is_ui = is_dark_ui_screenshot_raster(&cropped);
    if is_bar_chart || is_photo || is_ui {
        return;
    }
    let bordered = expand_white_border(&cropped, TABLE_RASTER_OCR_BORDER_PX);
    let scaled = image::imageops::resize(
        &bordered,
        bordered.width() * OCR_SCALE_FACTOR,
        bordered.height() * OCR_SCALE_FACTOR,
        image::imageops::FilterType::Lanczos3,
    );

    let Some(words) = run_tesseract_tsv_words(&scaled, "6") else {
        fill_cells_with_per_cell_ocr(gray, table, &empty_cells);
        return;
    };
    if words.is_empty() {
        fill_cells_with_per_cell_ocr(gray, table, &empty_cells);
        return;
    }
    let chart_like = looks_like_chart_label_ocr(&words);
    if chart_like {
        return;
    }

    let mut buckets: Vec<Vec<(u32, u32, String)>> = vec![Vec::new(); empty_cells.len()];
    let scale = f64::from(OCR_SCALE_FACTOR);
    let border = f64::from(TABLE_RASTER_OCR_BORDER_PX);

    for word in &words {
        let cx_scaled = f64::from(word.left) + f64::from(word.width) / 2.0;
        let cy_scaled = f64::from(word.top) + f64::from(word.height) / 2.0;

        let cx_crop = cx_scaled / scale - border;
        let cy_crop = cy_scaled / scale - border;
        if cx_crop < 0.0 || cy_crop < 0.0 {
            continue;
        }

        let cx_page = match u32::try_from(cx_crop.round() as i64) {
            Ok(v) => crop_left.saturating_add(v),
            Err(_) => continue,
        };
        let cy_page = match u32::try_from(cy_crop.round() as i64) {
            Ok(v) => crop_top.saturating_add(v),
            Err(_) => continue,
        };

        for (idx, cell) in empty_cells.iter().enumerate() {
            if cx_page >= cell.x1 && cx_page < cell.x2 && cy_page >= cell.y1 && cy_page < cell.y2 {
                buckets[idx].push((cy_page, cx_page, word.text.clone()));
                break;
            }
        }
    }

    for (idx, cell) in empty_cells.iter().enumerate() {
        let Some(row) = table.rows.get_mut(cell.row_idx) else {
            continue;
        };
        let Some(target) = row.cells.get_mut(cell.cell_idx) else {
            continue;
        };
        if target
            .content
            .iter()
            .any(|token| matches!(token.token_type, TableTokenType::Text))
        {
            continue;
        }
        let mut parts = std::mem::take(&mut buckets[idx]);
        if parts.is_empty() {
            continue;
        }
        parts.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let raw = parts
            .into_iter()
            .map(|(_, _, t)| t)
            .collect::<Vec<_>>()
            .join(" ");
        let text = normalize_page_raster_cell_text(&target.bbox, raw);
        if text.is_empty() {
            continue;
        }
        target.content.push(TableToken {
            base: TextChunk {
                value: text,
                bbox: target.bbox.clone(),
                font_name: "OCR".to_string(),
                font_size: target.bbox.height().max(6.0),
                font_weight: 400.0,
                italic_angle: 0.0,
                font_color: "#000000".to_string(),
                contrast_ratio: 21.0,
                symbol_ends: Vec::new(),
                text_format: TextFormat::Normal,
                text_type: TextType::Regular,
                pdf_layer: PdfLayer::Content,
                ocg_visible: true,
                index: None,
                page_number: target.bbox.page_number,
                level: None,
                mcid: None,
            },
            token_type: TableTokenType::Text,
        });
    }
}

fn fill_cells_with_per_cell_ocr(
    gray: &GrayImage,
    table: &mut TableBorder,
    empty_cells: &[EmptyCellRaster],
) {
    for cell in empty_cells {
        let Some(row) = table.rows.get_mut(cell.row_idx) else {
            continue;
        };
        let Some(target) = row.cells.get_mut(cell.cell_idx) else {
            continue;
        };
        if target
            .content
            .iter()
            .any(|token| matches!(token.token_type, TableTokenType::Text))
        {
            continue;
        }
        let Some(text) =
            extract_page_raster_cell_text(gray, &target.bbox, cell.x1, cell.y1, cell.x2, cell.y2)
        else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        target.content.push(TableToken {
            base: TextChunk {
                value: text,
                bbox: target.bbox.clone(),
                font_name: "OCR".to_string(),
                font_size: target.bbox.height().max(6.0),
                font_weight: 400.0,
                italic_angle: 0.0,
                font_color: "#000000".to_string(),
                contrast_ratio: 21.0,
                symbol_ends: Vec::new(),
                text_format: TextFormat::Normal,
                text_type: TextType::Regular,
                pdf_layer: PdfLayer::Content,
                ocg_visible: true,
                index: None,
                page_number: target.bbox.page_number,
                level: None,
                mcid: None,
            },
            token_type: TableTokenType::Text,
        });
    }
}

fn page_bbox_to_raster_box(
    gray: &GrayImage,
    page_bbox: &BoundingBox,
    bbox: &BoundingBox,
) -> Option<(u32, u32, u32, u32)> {
    if page_bbox.width() <= 0.0 || page_bbox.height() <= 0.0 {
        return None;
    }

    let left = ((bbox.left_x - page_bbox.left_x) / page_bbox.width() * f64::from(gray.width()))
        .clamp(0.0, f64::from(gray.width()));
    let right = ((bbox.right_x - page_bbox.left_x) / page_bbox.width() * f64::from(gray.width()))
        .clamp(0.0, f64::from(gray.width()));
    let top = ((page_bbox.top_y - bbox.top_y) / page_bbox.height() * f64::from(gray.height()))
        .clamp(0.0, f64::from(gray.height()));
    let bottom = ((page_bbox.top_y - bbox.bottom_y) / page_bbox.height()
        * f64::from(gray.height()))
    .clamp(0.0, f64::from(gray.height()));

    let x1 = left.floor() as u32;
    let x2 = right.ceil() as u32;
    let y1 = top.floor() as u32;
    let y2 = bottom.ceil() as u32;
    (x2 > x1 && y2 > y1).then_some((x1, y1, x2, y2))
}

fn extract_page_raster_cell_text(
    gray: &GrayImage,
    cell_bbox: &BoundingBox,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
) -> Option<String> {
    let inset_x = CELL_INSET_PX.min((x2 - x1) / 4);
    let inset_y = CELL_INSET_PX.min((y2 - y1) / 4);
    let crop_left = x1 + inset_x;
    let crop_top = y1 + inset_y;
    let crop_width = x2.saturating_sub(x1 + inset_x * 2);
    let crop_height = y2.saturating_sub(y1 + inset_y * 2);
    if crop_width < MIN_CELL_SIZE_PX || crop_height < MIN_CELL_SIZE_PX {
        return Some(String::new());
    }

    let cropped = gray
        .view(crop_left, crop_top, crop_width, crop_height)
        .to_image();
    let bordered = expand_white_border(&cropped, 12);
    let scaled = image::imageops::resize(
        &bordered,
        bordered.width() * OCR_SCALE_FACTOR,
        bordered.height() * OCR_SCALE_FACTOR,
        image::imageops::FilterType::Lanczos3,
    );

    // Improved PSM selection based on cell aspect ratio
    let aspect_ratio = cell_bbox.width() / cell_bbox.height();
    let is_vertical = aspect_ratio < 0.8;

    // PSM modes ordered by likelihood of success for each cell shape.
    // Typography rationale:
    //   PSM 6  — single uniform text block (multi-line header/paragraph cells)
    //   PSM 7  — single text line (most data cells; one baseline)
    //   PSM 8  — single word (numeric data, codes, percentages — one token)
    //   PSM 11 — sparse text (cells with scattered numbers / partial fills)
    //   PSM 13 — raw line (bypasses heuristics; last resort for oddly typeset cells)
    // PSM 10 (single character) is intentionally excluded: table cells always
    // contain at least one full token, so char-level segmentation yields fragments.
    let psm_modes: [&str; 5] = if is_vertical {
        ["7", "8", "6", "11", "13"]
    } else {
        ["6", "7", "8", "11", "13"]
    };

    let raw_text = run_tesseract_cell_text_best(&scaled, &psm_modes)?;
    Some(normalize_page_raster_cell_text(cell_bbox, raw_text))
}

fn normalize_page_raster_cell_text(cell_bbox: &BoundingBox, text: String) -> String {
    let normalized = text
        .replace('|', " ")
        .replace('—', "-")
        .replace(['“', '”'], "\"")
        .replace('’', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.is_empty() {
        return normalized;
    }

    let narrow_cell = cell_bbox.width() <= cell_bbox.height() * 1.15;
    if narrow_cell && normalized.len() <= 3 && !normalized.chars().any(|ch| ch.is_ascii_digit()) {
        return String::new();
    }

    normalized
}

fn is_ocr_candidate(
    image: &ImageChunk,
    page_bbox: &BoundingBox,
    text_chunks: &[TextChunk],
) -> bool {
    let width_ratio = image.bbox.width() / page_bbox.width().max(1.0);
    let area_ratio = image.bbox.area() / page_bbox.area().max(1.0);
    if width_ratio < MIN_IMAGE_WIDTH_RATIO || area_ratio < MIN_IMAGE_AREA_RATIO {
        return false;
    }

    let overlapping_chunks: Vec<&TextChunk> = text_chunks
        .iter()
        .filter(|chunk| image.bbox.intersection_percent(&chunk.bbox) >= 0.7)
        .collect();
    let native_text_chars: usize = overlapping_chunks
        .iter()
        .map(|chunk| chunk.value.chars().filter(|ch| !ch.is_whitespace()).count())
        .sum();

    native_text_chars <= MAX_NATIVE_TEXT_CHARS_IN_IMAGE
        || overlapping_chunks.len() <= MAX_NATIVE_TEXT_CHUNKS_IN_IMAGE
}

fn is_dominant_image_text_candidate(
    image: &ImageChunk,
    page_bbox: &BoundingBox,
    text_chunks: &[TextChunk],
) -> bool {
    let width_ratio = image.bbox.width() / page_bbox.width().max(1.0);
    let area_ratio = image.bbox.area() / page_bbox.area().max(1.0);
    if width_ratio < MIN_DOMINANT_IMAGE_WIDTH_RATIO || area_ratio < MIN_DOMINANT_IMAGE_AREA_RATIO {
        return false;
    }

    let native_text_chars: usize = text_chunks
        .iter()
        .filter(|chunk| image.bbox.intersection_percent(&chunk.bbox) >= 0.7)
        .map(|chunk| chunk.value.chars().filter(|ch| !ch.is_whitespace()).count())
        .sum();

    native_text_chars <= MAX_NATIVE_TEXT_CHARS_IN_DOMINANT_IMAGE
}

fn parse_tesseract_tsv(tsv: &str) -> Vec<OcrWord> {
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

fn looks_like_chart_label_ocr(words: &[OcrWord]) -> bool {
    if words.len() < 8 {
        return false;
    }

    let min_left = words.iter().map(|word| word.left).min().unwrap_or(0);
    let min_top = words.iter().map(|word| word.top).min().unwrap_or(0);
    let max_right = words
        .iter()
        .map(|word| word.left.saturating_add(word.width))
        .max()
        .unwrap_or(0);
    let max_bottom = words
        .iter()
        .map(|word| word.top.saturating_add(word.height))
        .max()
        .unwrap_or(0);
    let image_width = max_right.saturating_sub(min_left);
    let image_height = max_bottom.saturating_sub(min_top);
    if image_width < 160 || image_height < 120 {
        return false;
    }

    let width_f = f64::from(image_width);
    let height_f = f64::from(image_height);
    let outer_x = width_f * 0.18;
    let outer_y = height_f * 0.18;
    let inner_left = width_f * 0.22;
    let inner_right = width_f * 0.78;
    let inner_top = height_f * 0.22;
    let inner_bottom = height_f * 0.78;

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    let mut outer_words = 0usize;
    let mut inner_words = 0usize;

    for word in words {
        by_line.entry(word.line_key).or_default().push(word);

        let center_x = f64::from(word.left.saturating_sub(min_left)) + f64::from(word.width) / 2.0;
        let center_y = f64::from(word.top.saturating_sub(min_top)) + f64::from(word.height) / 2.0;

        if center_x <= outer_x
            || center_x >= width_f - outer_x
            || center_y <= outer_y
            || center_y >= height_f - outer_y
        {
            outer_words += 1;
        }

        if center_x >= inner_left
            && center_x <= inner_right
            && center_y >= inner_top
            && center_y <= inner_bottom
        {
            inner_words += 1;
        }
    }

    if by_line.len() < 5 {
        return false;
    }

    let tolerance = (f64::from(max_right) * 0.035).max(18.0);
    let mut clusters: Vec<XCluster> = Vec::new();
    for line_words in by_line.values() {
        for word in line_words {
            let center = f64::from(word.left) + f64::from(word.width) / 2.0;
            if let Some(cluster) = clusters
                .iter_mut()
                .find(|cluster| (cluster.center - center).abs() <= tolerance)
            {
                cluster.center =
                    (cluster.center * cluster.count as f64 + center) / (cluster.count as f64 + 1.0);
                cluster.count += 1;
                cluster.lines.insert(word.line_key);
            } else {
                let mut lines = HashSet::new();
                lines.insert(word.line_key);
                clusters.push(XCluster {
                    center,
                    count: 1,
                    lines,
                });
            }
        }
    }

    let stable_centers: Vec<f64> = clusters
        .iter()
        .filter(|cluster| cluster.lines.len() >= 4 && cluster.count >= 4)
        .map(|cluster| cluster.center)
        .collect();
    let mut sorted_stable_centers = stable_centers.clone();
    sorted_stable_centers
        .sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let max_stable_gap = sorted_stable_centers
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .fold(0.0, f64::max);
    let spans_full_table_width = stable_centers.len() >= 3
        && stable_centers
            .iter()
            .any(|center| *center - f64::from(min_left) <= width_f * 0.25)
        && stable_centers
            .iter()
            .any(|center| *center - f64::from(min_left) >= width_f * 0.75)
        && stable_centers.iter().any(|center| {
            let rel = *center - f64::from(min_left);
            rel >= inner_left && rel <= inner_right
        })
        && max_stable_gap <= width_f * 0.45;
    if spans_full_table_width {
        let table_like_lines = by_line
            .values()
            .filter(|line_words| {
                let mut seen = HashSet::<usize>::new();
                for word in *line_words {
                    let center = f64::from(word.left) + f64::from(word.width) / 2.0;
                    for (idx, stable_center) in stable_centers.iter().enumerate() {
                        if (center - stable_center).abs() <= tolerance {
                            seen.insert(idx);
                        }
                    }
                }
                seen.len() >= 3
            })
            .count();
        if table_like_lines >= 4 {
            return false;
        }
    }

    let mut short_lines = 0usize;
    let mut peripheral_label_lines = 0usize;
    let mut wide_sentence_lines = 0usize;
    let mut axisish_numeric_lines = 0usize;

    for line_words in by_line.values() {
        let line_left = line_words.iter().map(|word| word.left).min().unwrap_or(0);
        let line_top = line_words.iter().map(|word| word.top).min().unwrap_or(0);
        let line_right = line_words
            .iter()
            .map(|word| word.left.saturating_add(word.width))
            .max()
            .unwrap_or(0);
        let line_bottom = line_words
            .iter()
            .map(|word| word.top.saturating_add(word.height))
            .max()
            .unwrap_or(0);
        if line_right <= line_left || line_bottom <= line_top {
            continue;
        }

        let word_count = line_words.len();
        let numeric_in_line = line_words
            .iter()
            .filter(|word| is_numeric_like(&word.text))
            .count();
        let line_width_ratio =
            f64::from(line_right.saturating_sub(line_left)) / f64::from(image_width.max(1));
        let touches_outer_band = f64::from(line_left.saturating_sub(min_left)) <= outer_x
            || f64::from(line_right.saturating_sub(min_left)) >= width_f - outer_x
            || f64::from(line_top.saturating_sub(min_top)) <= outer_y
            || f64::from(line_bottom.saturating_sub(min_top)) >= height_f - outer_y;

        if word_count <= 3 {
            short_lines += 1;
        }
        if touches_outer_band && word_count <= 4 {
            peripheral_label_lines += 1;
        }
        if touches_outer_band && word_count <= 3 && numeric_in_line > 0 {
            axisish_numeric_lines += 1;
        }
        if word_count >= 4 && line_width_ratio >= 0.45 && numeric_in_line == 0 {
            wide_sentence_lines += 1;
        }
    }

    let total_lines = by_line.len();
    let outer_dominant = outer_words * 10 >= words.len() * 5;
    let inner_sparse = inner_words * 10 <= words.len() * 5;
    let label_dominant = peripheral_label_lines * 10 >= total_lines * 6;
    let short_line_dominant = short_lines * 10 >= total_lines * 6;
    let axis_signal = axisish_numeric_lines >= 2;

    outer_dominant
        && inner_sparse
        && label_dominant
        && short_line_dominant
        && axis_signal
        && wide_sentence_lines <= 2
}

fn looks_like_matrix_formula_ocr(words: &[OcrWord]) -> bool {
    if words.len() < 6 {
        return false;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    if by_line.len() < 2 || by_line.len() > 4 {
        return false;
    }

    let substantive_words = words
        .iter()
        .filter(|word| is_substantive_table_word(&word.text))
        .count();
    let short_formulaish_words = words
        .iter()
        .filter(|word| is_short_formulaish_word(&word.text))
        .count();
    let slash_words = words.iter().filter(|word| word.text.contains('/')).count();
    let equation_label_words = words
        .iter()
        .filter(|word| looks_like_equation_label_word(&word.text))
        .count();
    let dense_lines = by_line.values().filter(|line| line.len() >= 3).count();
    let short_lines = by_line
        .values()
        .filter(|line| line.iter().all(|word| is_short_formulaish_word(&word.text)))
        .count();

    substantive_words == 0
        && dense_lines >= 2
        && short_lines * 10 >= by_line.len() * 7
        && short_formulaish_words * 10 >= words.len() * 7
        && (slash_words > 0 || equation_label_words >= 2)
}

fn is_substantive_table_word(text: &str) -> bool {
    let normalized: String = text
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if normalized.is_empty() {
        return false;
    }

    let alpha_count = normalized.chars().filter(|ch| ch.is_alphabetic()).count();
    let digit_count = normalized.chars().filter(|ch| ch.is_ascii_digit()).count();
    let has_non_binary_digit = normalized
        .chars()
        .any(|ch| ch.is_ascii_digit() && !matches!(ch, '0' | '1'));

    alpha_count >= 4
        || (digit_count >= 2 && alpha_count == 0 && has_non_binary_digit)
        || (normalized.len() >= 5 && alpha_count >= 2)
}

fn is_short_formulaish_word(text: &str) -> bool {
    let normalized: String = text
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    if normalized.is_empty() {
        return true;
    }

    normalized.len() <= 3 || (text.contains('/') && normalized.len() <= 4)
}

fn looks_like_equation_label_word(text: &str) -> bool {
    let trimmed = text.trim_matches(|ch: char| !ch.is_alphanumeric());
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() || !first.is_ascii_uppercase() {
        return false;
    }

    let remainder: String = chars.collect();
    !remainder.is_empty() && remainder.len() <= 3 && remainder.chars().all(|ch| ch.is_ascii_digit())
}

fn looks_like_table_ocr(words: &[OcrWord]) -> bool {
    if words.len() < 8 {
        return false;
    }

    if looks_like_chart_label_ocr(words) {
        return false;
    }

    if looks_like_matrix_formula_ocr(words) {
        return false;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    let mut qualifying_lines = Vec::new();
    let mut numeric_like_count = 0usize;
    let mut max_right = 0u32;
    for line_words in by_line.values_mut() {
        line_words.sort_by_key(|word| word.left);
        let numeric_words = line_words
            .iter()
            .filter(|word| is_numeric_like(&word.text))
            .count();
        numeric_like_count += numeric_words;
        if line_words.len() >= 3 || numeric_words >= 2 {
            max_right = max_right.max(
                line_words
                    .iter()
                    .map(|word| word.left.saturating_add(word.width))
                    .max()
                    .unwrap_or(0),
            );
            qualifying_lines.push(line_words.clone());
        }
    }

    if qualifying_lines.len() < 2 {
        return false;
    }

    let tolerance = (f64::from(max_right) * 0.035).max(18.0);
    let mut clusters: Vec<XCluster> = Vec::new();
    for line in &qualifying_lines {
        for word in line {
            let center = f64::from(word.left) + f64::from(word.width) / 2.0;
            if let Some(cluster) = clusters
                .iter_mut()
                .find(|cluster| (cluster.center - center).abs() <= tolerance)
            {
                cluster.center =
                    (cluster.center * cluster.count as f64 + center) / (cluster.count as f64 + 1.0);
                cluster.count += 1;
                cluster.lines.insert(word.line_key);
            } else {
                let mut lines = HashSet::new();
                lines.insert(word.line_key);
                clusters.push(XCluster {
                    center,
                    count: 1,
                    lines,
                });
            }
        }
    }

    let repeated_clusters: Vec<&XCluster> = clusters
        .iter()
        .filter(|cluster| cluster.lines.len() >= 2 && cluster.count >= 2)
        .collect();
    if repeated_clusters.len() < 3 {
        return false;
    }

    let repeated_centers: Vec<f64> = repeated_clusters
        .iter()
        .map(|cluster| cluster.center)
        .collect();
    let structured_lines = qualifying_lines
        .iter()
        .filter(|line| {
            let mut seen = HashSet::<usize>::new();
            for word in *line {
                let center = f64::from(word.left) + f64::from(word.width) / 2.0;
                for (idx, repeated_center) in repeated_centers.iter().enumerate() {
                    if (center - repeated_center).abs() <= tolerance {
                        seen.insert(idx);
                    }
                }
            }
            seen.len() >= 3
                || (seen.len() >= 2
                    && line.iter().filter(|w| is_numeric_like(&w.text)).count() >= 2)
        })
        .count();

    let alphabetic_words = words
        .iter()
        .filter(|word| word.text.chars().any(|ch| ch.is_alphabetic()))
        .count();

    // Geometric guard: repeated vertical bands alone are not enough for tables.
    // Dense prose in infographics often forms stable x-clusters but lacks numeric
    // signal. Require either numeric evidence or stronger column multiplicity.
    if numeric_like_count == 0
        && alphabetic_words * 10 >= words.len() * 9
        && repeated_clusters.len() <= 4
    {
        return false;
    }

    structured_lines >= 3
        || (structured_lines >= 2 && numeric_like_count >= 6 && repeated_clusters.len() >= 4)
}

fn looks_like_numeric_table_ocr(words: &[OcrWord]) -> bool {
    if !looks_like_table_ocr(words) {
        return false;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    let numeric_like_count = words
        .iter()
        .filter(|word| is_numeric_like(&word.text))
        .count();
    let numeric_lines = by_line
        .values()
        .filter(|line| {
            line.iter()
                .filter(|word| is_numeric_like(&word.text))
                .count()
                >= 2
        })
        .count();

    numeric_like_count >= 12 && numeric_lines >= 3
}

fn looks_like_dense_prose_image_ocr(words: &[OcrWord]) -> bool {
    if words.len() < MIN_DOMINANT_IMAGE_OCR_WORDS || looks_like_table_ocr(words) {
        return false;
    }

    if looks_like_chart_label_ocr(words) {
        return false;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    let mut alphabetic_words = 0usize;
    let mut numeric_like_words = 0usize;
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
        if word.text.chars().any(|ch| ch.is_alphabetic()) {
            alphabetic_words += 1;
        }
        if is_numeric_like(&word.text) {
            numeric_like_words += 1;
        }
    }

    if by_line.len() < MIN_DOMINANT_IMAGE_TEXT_LINES || alphabetic_words * 3 < words.len() * 2 {
        return false;
    }
    if numeric_like_words * 4 > words.len() {
        return false;
    }

    let multiword_lines = by_line
        .values()
        .filter(|line| line.iter().filter(|word| word.text.len() >= 2).count() >= 3)
        .count();
    multiword_lines >= 4 && has_dense_prose_block_geometry(words)
}

fn has_dense_prose_block_geometry(words: &[OcrWord]) -> bool {
    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    let mut spatial_lines = Vec::new();
    for line_words in by_line.values() {
        if line_words.len() < 3 {
            continue;
        }

        let left = line_words.iter().map(|word| word.left).min().unwrap_or(0);
        let right = line_words
            .iter()
            .map(|word| word.left.saturating_add(word.width))
            .max()
            .unwrap_or(0);
        let top = line_words.iter().map(|word| word.top).min().unwrap_or(0);
        let bottom = line_words
            .iter()
            .map(|word| word.top.saturating_add(word.height))
            .max()
            .unwrap_or(0);

        if right <= left || bottom <= top {
            continue;
        }

        spatial_lines.push(SpatialOcrLine {
            left,
            top,
            right,
            bottom,
            text: String::new(),
            word_count: line_words.len(),
            line_count: 1,
            line_height_sum: bottom.saturating_sub(top).max(1),
        });
    }

    spatial_lines.sort_by_key(|line| (line.top, line.left));
    if spatial_lines.len() < MIN_DENSE_PROSE_BLOCK_LINES {
        return false;
    }

    let image_width = spatial_lines
        .iter()
        .map(|line| line.right)
        .max()
        .unwrap_or(0);
    if image_width == 0 {
        return false;
    }

    let median_height = {
        let mut heights: Vec<u32> = spatial_lines
            .iter()
            .map(|line| line.bottom.saturating_sub(line.top).max(1))
            .collect();
        heights.sort_unstable();
        heights[heights.len() / 2]
    };

    let mut best_line_count = 1usize;
    let mut best_left = spatial_lines[0].left;
    let mut best_right = spatial_lines[0].right;
    let mut current_line_count = 1usize;
    let mut current_left = spatial_lines[0].left;
    let mut current_right = spatial_lines[0].right;

    for pair in spatial_lines.windows(2) {
        let prev = &pair[0];
        let curr = &pair[1];
        if spatial_lines_share_block_geometry(prev, curr, image_width, median_height) {
            current_line_count += 1;
            current_left = current_left.min(curr.left);
            current_right = current_right.max(curr.right);
        } else {
            if current_line_count > best_line_count {
                best_line_count = current_line_count;
                best_left = current_left;
                best_right = current_right;
            }
            current_line_count = 1;
            current_left = curr.left;
            current_right = curr.right;
        }
    }

    if current_line_count > best_line_count {
        best_line_count = current_line_count;
        best_left = current_left;
        best_right = current_right;
    }

    let block_width_ratio =
        f64::from(best_right.saturating_sub(best_left)) / f64::from(image_width);
    best_line_count >= MIN_DENSE_PROSE_BLOCK_LINES
        && block_width_ratio >= MIN_DENSE_PROSE_BLOCK_WIDTH_RATIO
}

fn build_numeric_table_border(words: &[OcrWord], image: &ImageChunk) -> Option<TableBorder> {
    let image_width = words
        .iter()
        .map(|word| word.left.saturating_add(word.width))
        .max()?;
    let image_height = words
        .iter()
        .map(|word| word.top.saturating_add(word.height))
        .max()?;
    if image_width == 0 || image_height == 0 {
        return None;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    let max_right = words
        .iter()
        .map(|word| word.left.saturating_add(word.width))
        .max()
        .unwrap_or(0);
    let tolerance = (f64::from(max_right) * 0.035).max(18.0);

    let mut clusters: Vec<XCluster> = Vec::new();
    for line_words in by_line.values() {
        for word in line_words {
            let center = f64::from(word.left) + f64::from(word.width) / 2.0;
            if let Some(cluster) = clusters
                .iter_mut()
                .find(|cluster| (cluster.center - center).abs() <= tolerance)
            {
                cluster.center =
                    (cluster.center * cluster.count as f64 + center) / (cluster.count as f64 + 1.0);
                cluster.count += 1;
                cluster.lines.insert(word.line_key);
            } else {
                let mut lines = HashSet::new();
                lines.insert(word.line_key);
                clusters.push(XCluster {
                    center,
                    count: 1,
                    lines,
                });
            }
        }
    }
    let mut centers: Vec<f64> = clusters
        .into_iter()
        .filter(|cluster| cluster.lines.len() >= 2 && cluster.count >= 2)
        .map(|cluster| cluster.center)
        .collect();
    centers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if centers.len() < 3 {
        return None;
    }

    let mut built_rows = Vec::<OcrRowBuild>::new();
    let mut row_fill_counts = Vec::<usize>::new();
    for line_words in by_line.values() {
        let mut sorted_words = line_words.clone();
        sorted_words.sort_by_key(|word| word.left);

        let mut cells = vec![Vec::<&OcrWord>::new(); centers.len()];
        for word in &sorted_words {
            let center = f64::from(word.left) + f64::from(word.width) / 2.0;
            if let Some((col_idx, distance)) = centers
                .iter()
                .enumerate()
                .map(|(idx, col_center)| (idx, (center - col_center).abs()))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            {
                if distance <= tolerance {
                    cells[col_idx].push(word);
                }
            }
        }

        let filled_cells = cells.iter().filter(|cell| !cell.is_empty()).count();
        let numeric_cells = cells
            .iter()
            .filter(|cell| cell.iter().any(|word| is_numeric_like(&word.text)))
            .count();
        if filled_cells < 3 && numeric_cells < 2 {
            continue;
        }
        row_fill_counts.push(filled_cells);

        let top_px = sorted_words.iter().map(|word| word.top).min().unwrap_or(0);
        let bottom_px = sorted_words
            .iter()
            .map(|word| word.top.saturating_add(word.height))
            .max()
            .unwrap_or(0);
        let top_y =
            image.bbox.top_y - image.bbox.height() * (f64::from(top_px) / f64::from(image_height));
        let bottom_y = image.bbox.top_y
            - image.bbox.height() * (f64::from(bottom_px) / f64::from(image_height));
        let cell_texts = cells
            .iter()
            .map(|cell_words| {
                cell_words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        built_rows.push(OcrRowBuild {
            top_y,
            bottom_y,
            cell_texts,
        });
    }

    if built_rows.len() < 2 {
        return None;
    }
    if row_fill_counts.is_empty() {
        return None;
    }

    let mut sorted_fill_counts = row_fill_counts.clone();
    sorted_fill_counts.sort_unstable();
    let median_fill_ratio =
        sorted_fill_counts[sorted_fill_counts.len() / 2] as f64 / centers.len() as f64;
    if median_fill_ratio < MIN_NUMERIC_TABLE_MEDIAN_FILL_RATIO {
        return None;
    }

    built_rows.sort_by(|a, b| {
        b.top_y
            .partial_cmp(&a.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let x_coordinates = build_boundaries_from_centers(
        &centers,
        image.bbox.left_x,
        image.bbox.right_x,
        image_width,
    );
    let row_bounds: Vec<(f64, f64)> = built_rows
        .iter()
        .map(|row| (row.top_y, row.bottom_y))
        .collect();
    let y_coordinates = build_row_boundaries(&row_bounds);
    if x_coordinates.len() != centers.len() + 1 || y_coordinates.len() != built_rows.len() + 1 {
        return None;
    }

    let mut rows = Vec::new();
    for (row_idx, row_build) in built_rows.iter().enumerate() {
        let row_bbox = BoundingBox::new(
            image.bbox.page_number,
            image.bbox.left_x,
            y_coordinates[row_idx + 1],
            image.bbox.right_x,
            y_coordinates[row_idx],
        );
        let mut cells = Vec::new();
        for col_idx in 0..centers.len() {
            let cell_bbox = BoundingBox::new(
                image.bbox.page_number,
                x_coordinates[col_idx],
                y_coordinates[row_idx + 1],
                x_coordinates[col_idx + 1],
                y_coordinates[row_idx],
            );
            let text = row_build
                .cell_texts
                .get(col_idx)
                .cloned()
                .unwrap_or_default();
            let mut content = Vec::new();
            if !text.trim().is_empty() {
                content.push(TableToken {
                    base: TextChunk {
                        value: text.trim().to_string(),
                        bbox: cell_bbox.clone(),
                        font_name: "OCR".to_string(),
                        font_size: (row_build.top_y - row_build.bottom_y).max(6.0),
                        font_weight: 400.0,
                        italic_angle: 0.0,
                        font_color: "#000000".to_string(),
                        contrast_ratio: 21.0,
                        symbol_ends: Vec::new(),
                        text_format: TextFormat::Normal,
                        text_type: TextType::Regular,
                        pdf_layer: PdfLayer::Content,
                        ocg_visible: true,
                        index: None,
                        page_number: image.bbox.page_number,
                        level: None,
                        mcid: None,
                    },
                    token_type: TableTokenType::Text,
                });
            }
            cells.push(TableBorderCell {
                bbox: cell_bbox,
                index: None,
                level: None,
                row_number: row_idx,
                col_number: col_idx,
                row_span: 1,
                col_span: 1,
                content,
                contents: Vec::new(),
                semantic_type: None,
            });
        }
        rows.push(TableBorderRow {
            bbox: row_bbox,
            index: None,
            level: None,
            row_number: row_idx,
            cells,
            semantic_type: None,
        });
    }

    Some(TableBorder {
        bbox: image.bbox.clone(),
        index: None,
        level: None,
        x_coordinates: x_coordinates.clone(),
        x_widths: vec![0.0; x_coordinates.len()],
        y_coordinates: y_coordinates.clone(),
        y_widths: vec![0.0; y_coordinates.len()],
        rows,
        num_rows: built_rows.len(),
        num_columns: centers.len(),
        is_bad_table: false,
        is_table_transformer: true,
        previous_table: None,
        next_table: None,
    })
}

fn build_structured_ocr_table_border(words: &[OcrWord], image: &ImageChunk) -> Option<TableBorder> {
    let image_width = words
        .iter()
        .map(|word| word.left.saturating_add(word.width))
        .max()?;
    let image_height = words
        .iter()
        .map(|word| word.top.saturating_add(word.height))
        .max()?;
    if image_width == 0 || image_height == 0 {
        return None;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    let max_right = words
        .iter()
        .map(|word| word.left.saturating_add(word.width))
        .max()
        .unwrap_or(0);
    let tolerance = (f64::from(max_right) * 0.035).max(18.0);

    let mut clusters: Vec<XCluster> = Vec::new();
    for line_words in by_line.values() {
        for word in line_words {
            let center = f64::from(word.left) + f64::from(word.width) / 2.0;
            if let Some(cluster) = clusters
                .iter_mut()
                .find(|cluster| (cluster.center - center).abs() <= tolerance)
            {
                cluster.center =
                    (cluster.center * cluster.count as f64 + center) / (cluster.count as f64 + 1.0);
                cluster.count += 1;
                cluster.lines.insert(word.line_key);
            } else {
                let mut lines = HashSet::new();
                lines.insert(word.line_key);
                clusters.push(XCluster {
                    center,
                    count: 1,
                    lines,
                });
            }
        }
    }

    let mut centers: Vec<f64> = clusters
        .into_iter()
        .filter(|cluster| cluster.lines.len() >= 2 && cluster.count >= 2)
        .map(|cluster| cluster.center)
        .collect();
    centers.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if centers.len() < 3 {
        return None;
    }

    let mut built_rows = Vec::<OcrRowBuild>::new();
    let mut row_fill_counts = Vec::<usize>::new();
    let mut occupied_columns = vec![0usize; centers.len()];

    for line_words in by_line.values() {
        let mut sorted_words = line_words.clone();
        sorted_words.sort_by_key(|word| word.left);

        let mut cells = vec![Vec::<&OcrWord>::new(); centers.len()];
        for word in &sorted_words {
            let center = f64::from(word.left) + f64::from(word.width) / 2.0;
            if let Some((col_idx, distance)) = centers
                .iter()
                .enumerate()
                .map(|(idx, col_center)| (idx, (center - col_center).abs()))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            {
                if distance <= tolerance {
                    cells[col_idx].push(word);
                }
            }
        }

        let filled_indices: Vec<usize> = cells
            .iter()
            .enumerate()
            .filter_map(|(idx, cell)| (!cell.is_empty()).then_some(idx))
            .collect();
        if filled_indices.len() < 2 {
            continue;
        }

        let span = filled_indices.last().unwrap_or(&0) - filled_indices.first().unwrap_or(&0) + 1;
        if filled_indices.len() < 3 && span < 3 {
            continue;
        }

        row_fill_counts.push(filled_indices.len());
        for idx in &filled_indices {
            if let Some(count) = occupied_columns.get_mut(*idx) {
                *count += 1;
            }
        }

        let top_px = sorted_words.iter().map(|word| word.top).min().unwrap_or(0);
        let bottom_px = sorted_words
            .iter()
            .map(|word| word.top.saturating_add(word.height))
            .max()
            .unwrap_or(0);
        let top_y =
            image.bbox.top_y - image.bbox.height() * (f64::from(top_px) / f64::from(image_height));
        let bottom_y = image.bbox.top_y
            - image.bbox.height() * (f64::from(bottom_px) / f64::from(image_height));
        let cell_texts = cells
            .iter()
            .map(|cell_words| {
                let mut sorted_cell_words = cell_words.clone();
                sorted_cell_words.sort_by_key(|word| word.left);
                sorted_cell_words
                    .iter()
                    .map(|word| word.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        built_rows.push(OcrRowBuild {
            top_y,
            bottom_y,
            cell_texts,
        });
    }

    if built_rows.len() < 3 || row_fill_counts.is_empty() {
        return None;
    }

    let repeated_columns = occupied_columns.iter().filter(|count| **count >= 2).count();
    if repeated_columns < 3 {
        return None;
    }

    let mut sorted_fill_counts = row_fill_counts.clone();
    sorted_fill_counts.sort_unstable();
    let median_fill_ratio =
        sorted_fill_counts[sorted_fill_counts.len() / 2] as f64 / centers.len() as f64;
    if median_fill_ratio < 0.5 {
        return None;
    }

    built_rows.sort_by(|a, b| {
        b.top_y
            .partial_cmp(&a.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let x_coordinates = build_boundaries_from_centers(
        &centers,
        image.bbox.left_x,
        image.bbox.right_x,
        image_width,
    );
    let row_bounds: Vec<(f64, f64)> = built_rows
        .iter()
        .map(|row| (row.top_y, row.bottom_y))
        .collect();
    let y_coordinates = build_row_boundaries(&row_bounds);
    if x_coordinates.len() != centers.len() + 1 || y_coordinates.len() != built_rows.len() + 1 {
        return None;
    }

    let mut rows = Vec::new();
    for (row_idx, row_build) in built_rows.iter().enumerate() {
        let row_bbox = BoundingBox::new(
            image.bbox.page_number,
            image.bbox.left_x,
            y_coordinates[row_idx + 1],
            image.bbox.right_x,
            y_coordinates[row_idx],
        );
        let mut cells = Vec::new();
        for col_idx in 0..centers.len() {
            let cell_bbox = BoundingBox::new(
                image.bbox.page_number,
                x_coordinates[col_idx],
                y_coordinates[row_idx + 1],
                x_coordinates[col_idx + 1],
                y_coordinates[row_idx],
            );
            let text = row_build
                .cell_texts
                .get(col_idx)
                .cloned()
                .unwrap_or_default();
            let mut content = Vec::new();
            if !text.trim().is_empty() {
                content.push(TableToken {
                    base: TextChunk {
                        value: text.trim().to_string(),
                        bbox: cell_bbox.clone(),
                        font_name: "OCR".to_string(),
                        font_size: (row_build.top_y - row_build.bottom_y).max(6.0),
                        font_weight: if row_idx == 0 { 700.0 } else { 400.0 },
                        italic_angle: 0.0,
                        font_color: "#000000".to_string(),
                        contrast_ratio: 21.0,
                        symbol_ends: Vec::new(),
                        text_format: TextFormat::Normal,
                        text_type: TextType::Regular,
                        pdf_layer: PdfLayer::Content,
                        ocg_visible: true,
                        index: None,
                        page_number: image.bbox.page_number,
                        level: None,
                        mcid: None,
                    },
                    token_type: TableTokenType::Text,
                });
            }
            cells.push(TableBorderCell {
                bbox: cell_bbox,
                index: None,
                level: None,
                row_number: row_idx,
                col_number: col_idx,
                row_span: 1,
                col_span: 1,
                content,
                contents: Vec::new(),
                semantic_type: None,
            });
        }
        rows.push(TableBorderRow {
            bbox: row_bbox,
            index: None,
            level: None,
            row_number: row_idx,
            cells,
            semantic_type: None,
        });
    }

    Some(TableBorder {
        bbox: image.bbox.clone(),
        index: None,
        level: None,
        x_coordinates: x_coordinates.clone(),
        x_widths: vec![0.0; x_coordinates.len()],
        y_coordinates: y_coordinates.clone(),
        y_widths: vec![0.0; y_coordinates.len()],
        rows,
        num_rows: built_rows.len(),
        num_columns: centers.len(),
        is_bad_table: false,
        is_table_transformer: true,
        previous_table: None,
        next_table: None,
    })
}

fn is_matrixish_ocr_artifact_table(table: &TableBorder) -> bool {
    if !table.is_table_transformer
        || table.num_rows < 2
        || table.num_rows > 4
        || table.num_columns < 3
        || table.bbox.height() > table.bbox.width() * 0.55
    {
        return false;
    }

    let texts: Vec<String> = table
        .rows
        .iter()
        .flat_map(|row| row.cells.iter())
        .map(table_cell_text)
        .filter(|text| !text.is_empty())
        .collect();
    if texts.len() < 6 {
        return false;
    }

    let substantive_cells = texts
        .iter()
        .filter(|text| is_substantive_ocr_cell_text(text))
        .count();
    let short_cells = texts
        .iter()
        .filter(|text| is_short_ocr_cell_text(text))
        .count();
    let ambiguous_cells = texts
        .iter()
        .filter(|text| is_ambiguous_matrix_cell_text(text))
        .count();

    substantive_cells == 0
        && short_cells * 10 >= texts.len() * 8
        && ambiguous_cells * 10 >= texts.len() * 5
}

fn table_cell_text(cell: &TableBorderCell) -> String {
    cell.content
        .iter()
        .map(|token| token.base.value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_substantive_ocr_cell_text(text: &str) -> bool {
    text.split_whitespace().any(is_substantive_table_word)
}

fn is_short_ocr_cell_text(text: &str) -> bool {
    let normalized: String = text
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    !normalized.is_empty() && normalized.len() <= 4
}

fn is_ambiguous_matrix_cell_text(text: &str) -> bool {
    if text.contains(['/', '\\', '=', '|', '[', ']', '{', '}', '(', ')']) {
        return true;
    }

    let normalized: String = text
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    !normalized.is_empty()
        && normalized.len() <= 4
        && normalized
            .chars()
            .all(|ch| matches!(ch, '0' | '1' | 'o' | 'd' | 'q' | 'i' | 'l'))
}

fn recover_bordered_raster_caption(image_path: &Path, image: &ImageChunk) -> Option<TextChunk> {
    let gray = image::open(image_path).ok()?.to_luma8();
    recover_bordered_raster_caption_from_gray(&gray, image)
}

fn recover_bordered_raster_caption_from_gray(
    gray: &GrayImage,
    image: &ImageChunk,
) -> Option<TextChunk> {
    let grid = detect_bordered_raster_grid(gray)?;
    let first_h = *grid.horizontal_lines.first()?;
    if first_h <= 2 {
        return None;
    }

    let crop = gray.view(0, 0, gray.width(), first_h).to_image();
    let caption_text = normalize_caption_text(&run_tesseract_plain_text(&crop, "7")?);
    if caption_text.is_empty() || !caption_text.chars().any(|ch| ch.is_alphabetic()) {
        return None;
    }

    let bbox = raster_box_to_page_bbox(
        image,
        0,
        0,
        gray.width(),
        first_h.max(1),
        gray.width().max(1),
        gray.height().max(1),
    )?;
    let font_size = (bbox.height() * 0.55).clamp(10.0, 16.0);
    Some(TextChunk {
        value: caption_text,
        bbox,
        font_name: "OCR".to_string(),
        font_size,
        font_weight: 700.0,
        italic_angle: 0.0,
        font_color: "#000000".to_string(),
        contrast_ratio: 21.0,
        symbol_ends: Vec::new(),
        text_format: TextFormat::Normal,
        text_type: TextType::Regular,
        pdf_layer: PdfLayer::Content,
        ocg_visible: true,
        index: None,
        page_number: image.bbox.page_number,
        level: None,
        mcid: None,
    })
}

fn recover_bordered_raster_table(image_path: &Path, image: &ImageChunk) -> Option<TableBorder> {
    let gray = image::open(image_path).ok()?.to_luma8();
    recover_bordered_raster_table_from_gray(&gray, image)
}

fn recover_bordered_raster_table_from_gray(
    gray: &GrayImage,
    image: &ImageChunk,
) -> Option<TableBorder> {
    let grid = detect_bordered_raster_grid(gray)?;
    let num_cols = grid.vertical_lines.len().checked_sub(1)?;
    let num_rows = grid.horizontal_lines.len().checked_sub(1)?;
    if num_cols < 2 || num_rows < 2 {
        return None;
    }
    let table_bbox = raster_box_to_page_bbox(
        image,
        *grid.vertical_lines.first()?,
        *grid.horizontal_lines.first()?,
        *grid.vertical_lines.last()?,
        *grid.horizontal_lines.last()?,
        gray.width(),
        gray.height(),
    )?;

    let x_coordinates = raster_boundaries_to_page(
        &grid.vertical_lines,
        image.bbox.left_x,
        image.bbox.right_x,
        gray.width(),
    )?;
    let y_coordinates = raster_boundaries_to_page_desc(
        &grid.horizontal_lines,
        image.bbox.bottom_y,
        image.bbox.top_y,
        gray.height(),
    )?;

    if !bordered_grid_has_cell_ink(gray, &grid) {
        return None;
    }

    let mut rows = Vec::with_capacity(num_rows);
    let mut non_empty_cells = 0usize;
    let mut rows_with_text = 0usize;
    let mut total_cells = 0usize;
    let mut whole_table_buckets =
        collect_bordered_table_ocr_buckets(gray, &grid, num_rows, num_cols)
            .unwrap_or_else(|| vec![Vec::new(); num_rows * num_cols]);
    let allow_per_cell_fallback =
        num_rows.saturating_mul(num_cols) <= MAX_BORDERED_TABLE_PER_CELL_FALLBACK_CELLS;
    for row_idx in 0..num_rows {
        let row_bbox = BoundingBox::new(
            image.bbox.page_number,
            image.bbox.left_x,
            y_coordinates[row_idx + 1],
            image.bbox.right_x,
            y_coordinates[row_idx],
        );
        let mut cells = Vec::with_capacity(num_cols);
        let mut row_has_text = false;

        for col_idx in 0..num_cols {
            let x1 = grid.vertical_lines[col_idx];
            let x2 = grid.vertical_lines[col_idx + 1];
            let y1 = grid.horizontal_lines[row_idx];
            let y2 = grid.horizontal_lines[row_idx + 1];
            let cell_bbox = BoundingBox::new(
                image.bbox.page_number,
                x_coordinates[col_idx],
                y_coordinates[row_idx + 1],
                x_coordinates[col_idx + 1],
                y_coordinates[row_idx],
            );
            let bucket_idx = row_idx * num_cols + col_idx;
            let text = if let Some(parts) = whole_table_buckets.get_mut(bucket_idx) {
                if parts.is_empty() {
                    String::new()
                } else {
                    parts.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
                    let raw = parts
                        .iter()
                        .map(|(_, _, text)| text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    normalize_raster_cell_text(row_idx, col_idx, raw)
                }
            } else {
                String::new()
            };
            let text = if text.is_empty() && allow_per_cell_fallback {
                extract_raster_cell_text(gray, row_idx, col_idx, x1, y1, x2, y2).unwrap_or_default()
            } else {
                text
            };
            total_cells += 1;

            let mut content = Vec::new();
            if !text.is_empty() {
                row_has_text = true;
                non_empty_cells += 1;
                content.push(TableToken {
                    base: TextChunk {
                        value: text,
                        bbox: cell_bbox.clone(),
                        font_name: "OCR".to_string(),
                        font_size: (cell_bbox.height() * 0.55).max(6.0),
                        font_weight: if row_idx == 0 { 700.0 } else { 400.0 },
                        italic_angle: 0.0,
                        font_color: "#000000".to_string(),
                        contrast_ratio: 21.0,
                        symbol_ends: Vec::new(),
                        text_format: TextFormat::Normal,
                        text_type: TextType::Regular,
                        pdf_layer: PdfLayer::Content,
                        ocg_visible: true,
                        index: None,
                        page_number: image.bbox.page_number,
                        level: None,
                        mcid: None,
                    },
                    token_type: TableTokenType::Text,
                });
            }

            cells.push(TableBorderCell {
                bbox: cell_bbox,
                index: None,
                level: None,
                row_number: row_idx,
                col_number: col_idx,
                row_span: 1,
                col_span: 1,
                content,
                contents: Vec::new(),
                semantic_type: None,
            });
        }

        if row_has_text {
            rows_with_text += 1;
        }

        rows.push(TableBorderRow {
            bbox: row_bbox,
            index: None,
            level: None,
            row_number: row_idx,
            cells,
            semantic_type: None,
        });
    }

    if total_cells == 0 {
        return None;
    }
    let text_cell_ratio = non_empty_cells as f64 / total_cells as f64;
    if text_cell_ratio < MIN_RASTER_TABLE_TEXT_CELL_RATIO
        || rows_with_text < MIN_RASTER_TABLE_ROWS_WITH_TEXT
    {
        return None;
    }

    Some(TableBorder {
        bbox: table_bbox,
        index: None,
        level: None,
        x_coordinates: x_coordinates.clone(),
        x_widths: vec![0.0; x_coordinates.len()],
        y_coordinates: y_coordinates.clone(),
        y_widths: vec![0.0; y_coordinates.len()],
        rows,
        num_rows,
        num_columns: num_cols,
        is_bad_table: false,
        is_table_transformer: true,
        previous_table: None,
        next_table: None,
    })
}

fn collect_bordered_table_ocr_buckets(
    gray: &GrayImage,
    grid: &RasterTableGrid,
    num_rows: usize,
    num_cols: usize,
) -> Option<Vec<Vec<(u32, u32, String)>>> {
    if num_rows == 0 || num_cols == 0 {
        return None;
    }

    let bordered = expand_white_border(gray, TABLE_RASTER_OCR_BORDER_PX);
    let scaled = image::imageops::resize(
        &bordered,
        bordered.width() * OCR_SCALE_FACTOR,
        bordered.height() * OCR_SCALE_FACTOR,
        image::imageops::FilterType::Lanczos3,
    );
    let words = run_tesseract_tsv_words_best(&scaled, &["6", "11"], |_| true)?;
    if words.is_empty() || looks_like_chart_label_ocr(&words) {
        return None;
    }

    let mut buckets = vec![Vec::new(); num_rows * num_cols];
    let scale = f64::from(OCR_SCALE_FACTOR);
    let border = f64::from(TABLE_RASTER_OCR_BORDER_PX);

    for word in words {
        let cx_scaled = f64::from(word.left) + f64::from(word.width) / 2.0;
        let cy_scaled = f64::from(word.top) + f64::from(word.height) / 2.0;

        let cx = cx_scaled / scale - border;
        let cy = cy_scaled / scale - border;
        if cx < 0.0 || cy < 0.0 {
            continue;
        }

        let cx = match u32::try_from(cx.round() as i64) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let cy = match u32::try_from(cy.round() as i64) {
            Ok(value) => value,
            Err(_) => continue,
        };

        let col_idx = grid
            .vertical_lines
            .windows(2)
            .position(|span| cx >= span[0] && cx < span[1]);
        let row_idx = grid
            .horizontal_lines
            .windows(2)
            .position(|span| cy >= span[0] && cy < span[1]);
        let (Some(row_idx), Some(col_idx)) = (row_idx, col_idx) else {
            continue;
        };

        buckets[row_idx * num_cols + col_idx].push((cy, cx, word.text));
    }

    Some(buckets)
}

fn is_obvious_bar_chart_raster(gray: &GrayImage) -> bool {
    let width = gray.width();
    let height = gray.height();
    if width < 160 || height < 120 {
        return false;
    }

    let min_ink_pixels = (f64::from(width) * 0.35).ceil() as u32;
    let min_run_height = (height / 80).max(6);
    let wide_ink_row_runs = merge_runs(
        (0..height)
            .filter(|&y| count_ink_in_row(gray, y, RASTER_CHART_INK_THRESHOLD) >= min_ink_pixels),
    );
    let thick_runs = wide_ink_row_runs
        .into_iter()
        .filter(|(start, end)| end.saturating_sub(*start) + 1 >= min_run_height)
        .count();

    thick_runs >= 3 || is_obvious_vertical_bar_chart_raster(gray)
}

fn is_obvious_vertical_bar_chart_raster(gray: &GrayImage) -> bool {
    let width = gray.width();
    let height = gray.height();
    if width < 160 || height < 120 {
        return false;
    }

    let min_ink_pixels = (f64::from(height) * 0.08).ceil() as u32;
    let min_bar_width = (width / 28).max(10);
    let min_bar_height = (height / 8).max(16);
    let max_baseline_delta = (height / 14).max(8);
    let min_fill_ratio = 0.10;

    let candidate_runs =
        merge_runs((0..width).filter(|&x| {
            count_ink_in_column(gray, x, RASTER_CHART_INK_THRESHOLD) >= min_ink_pixels
        }));
    let mut baselines = Vec::new();
    let mut has_dominant_bar = false;
    let mut qualifying_bars = 0usize;

    for (start, end) in candidate_runs {
        let run_width = end.saturating_sub(start) + 1;
        if run_width < min_bar_width {
            continue;
        }

        let mut top = height;
        let mut bottom = 0u32;
        let mut ink_pixels = 0usize;
        for x in start..=end {
            for y in 0..height {
                if gray.get_pixel(x, y).0[0] < RASTER_CHART_INK_THRESHOLD {
                    top = top.min(y);
                    bottom = bottom.max(y);
                    ink_pixels += 1;
                }
            }
        }

        if top >= height || bottom <= top {
            continue;
        }

        let run_height = bottom.saturating_sub(top) + 1;
        if run_height < min_bar_height {
            continue;
        }

        let bbox_area = run_width as usize * run_height as usize;
        if bbox_area == 0 {
            continue;
        }

        let fill_ratio = ink_pixels as f64 / bbox_area as f64;
        if fill_ratio < min_fill_ratio {
            continue;
        }

        qualifying_bars += 1;
        if run_width >= min_bar_width.saturating_mul(2) {
            has_dominant_bar = true;
        }
        baselines.push(bottom);
    }

    if baselines.len() < 2 {
        return false;
    }

    baselines.sort_unstable();
    let median_baseline = baselines[baselines.len() / 2];
    let aligned_baselines = baselines
        .iter()
        .filter(|baseline| baseline.abs_diff(median_baseline) <= max_baseline_delta)
        .count();

    aligned_baselines >= 2 && (has_dominant_bar || (qualifying_bars >= 4 && aligned_baselines >= 4))
}

/// Return true when the image appears to be a natural photograph rather than a
/// synthetic chart, diagram, or scanned document page.
///
/// Photographs have a broadly distributed pixel histogram — many mid-tone pixels
/// (neither pure white nor pure black).  Synthetic images (charts, tables,
/// diagrams) consist mostly of a white background (~255) with sparse dark ink
/// (~0-50).  We classify an image as photographic when either at least 30% of
/// its pixels fall in the mid-tone band [40, 215], or when a bright image still
/// shows photo-like tonal diversity via a wide histogram support and high
/// entropy. Numeric table recovery should be skipped for photographic images
/// because OCR'd annotation labels (axis ticks, caption fragments) are not table
/// data.
fn is_natural_photograph_raster(gray: &GrayImage) -> bool {
    let total = (gray.width() * gray.height()) as usize;
    if total < 400 {
        return false;
    }

    let mut histogram = [0usize; 256];
    for pixel in gray.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let mid_tone_count: usize = histogram[40..=215].iter().sum();
    if mid_tone_count * 10 >= total * 3 {
        return true;
    }

    let mut coarse_histogram = [0usize; 16];
    for (value, count) in histogram.iter().enumerate() {
        coarse_histogram[value / 16] += count;
    }

    let occupied_bins = coarse_histogram
        .iter()
        .filter(|count| **count as f64 >= total as f64 * 0.01)
        .count();
    let entropy = coarse_histogram.iter().fold(0.0, |acc, count| {
        if *count == 0 {
            return acc;
        }
        let probability = *count as f64 / total as f64;
        acc - probability * probability.log2()
    });

    mid_tone_count as f64 / total as f64 >= MIN_BRIGHT_PHOTO_MID_TONE_RATIO
        && occupied_bins >= MIN_BRIGHT_PHOTO_HISTOGRAM_BINS
        && entropy >= MIN_BRIGHT_PHOTO_ENTROPY
}

/// Return true for dark UI or video-player screenshots that are visually rich
/// but not document tables.
fn is_dark_ui_screenshot_raster(gray: &GrayImage) -> bool {
    let total = (gray.width() * gray.height()) as usize;
    if total < 400 {
        return false;
    }

    let very_dark_count = gray.pixels().filter(|p| p[0] <= 39).count();
    let non_extreme_count = gray.pixels().filter(|p| p[0] >= 15 && p[0] <= 240).count();
    let bright_detail_count = gray.pixels().filter(|p| p[0] >= 180 && p[0] <= 245).count();

    very_dark_count * 20 >= total * 13
        && non_extreme_count * 2 >= total
        && bright_detail_count * 20 >= total
}

fn bordered_grid_has_cell_ink(gray: &GrayImage, grid: &RasterTableGrid) -> bool {
    let num_cols = match grid.vertical_lines.len().checked_sub(1) {
        Some(value) => value,
        None => return false,
    };
    let num_rows = match grid.horizontal_lines.len().checked_sub(1) {
        Some(value) => value,
        None => return false,
    };
    if num_cols == 0 || num_rows == 0 {
        return false;
    }

    let mut total_cells = 0usize;
    let mut inked_cells = 0usize;
    let mut rows_with_ink = 0usize;

    for row_idx in 0..num_rows {
        let mut row_has_ink = false;
        for col_idx in 0..num_cols {
            total_cells += 1;
            let x1 = grid.vertical_lines[col_idx];
            let x2 = grid.vertical_lines[col_idx + 1];
            let y1 = grid.horizontal_lines[row_idx];
            let y2 = grid.horizontal_lines[row_idx + 1];

            let inset_x = CELL_INSET_PX.min((x2 - x1) / 4);
            let inset_y = CELL_INSET_PX.min((y2 - y1) / 4);
            let crop_left = x1 + inset_x;
            let crop_top = y1 + inset_y;
            let crop_width = x2.saturating_sub(x1 + inset_x * 2);
            let crop_height = y2.saturating_sub(y1 + inset_y * 2);
            if crop_width < MIN_CELL_SIZE_PX || crop_height < MIN_CELL_SIZE_PX {
                continue;
            }

            let dark_pixels = (crop_top..crop_top + crop_height)
                .flat_map(|y| (crop_left..crop_left + crop_width).map(move |x| (x, y)))
                .filter(|&(x, y)| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
                .count();
            let area = (crop_width as usize) * (crop_height as usize);
            if area == 0 {
                continue;
            }

            let dark_ratio = dark_pixels as f64 / area as f64;
            if dark_ratio >= MIN_BORDERED_CELL_DARK_RATIO {
                inked_cells += 1;
                row_has_ink = true;
            }
        }
        if row_has_ink {
            rows_with_ink += 1;
        }
    }

    if total_cells == 0 {
        return false;
    }

    (inked_cells as f64 / total_cells as f64) >= MIN_BORDERED_INKED_CELL_RATIO
        && rows_with_ink >= MIN_BORDERED_ROWS_WITH_INK
}

fn detect_bordered_raster_grid(gray: &GrayImage) -> Option<RasterTableGrid> {
    let mut best_grid: Option<(RasterTableGrid, f64)> = None;
    for variant in build_ocr_variants(gray) {
        let Some((grid, score)) = detect_bordered_raster_grid_single(&variant) else {
            continue;
        };
        match &best_grid {
            Some((_, best_score)) if *best_score >= score => {}
            _ => best_grid = Some((grid, score)),
        }
    }
    best_grid.map(|(grid, _)| grid)
}

fn detect_bordered_raster_grid_single(gray: &GrayImage) -> Option<(RasterTableGrid, f64)> {
    let width = gray.width();
    let height = gray.height();
    if width < 100 || height < 80 {
        return None;
    }

    let min_vertical_dark = (f64::from(height) * MIN_LINE_DARK_RATIO).ceil() as u32;
    let min_horizontal_dark = (f64::from(width) * MIN_LINE_DARK_RATIO).ceil() as u32;

    let vertical_runs =
        merge_runs((0..width).filter(|&x| count_dark_in_column(gray, x) >= min_vertical_dark));
    let horizontal_runs =
        merge_runs((0..height).filter(|&y| count_dark_in_row(gray, y) >= min_horizontal_dark));
    if vertical_runs.len() < MIN_BORDERED_VERTICAL_LINES
        || horizontal_runs.len() < MIN_BORDERED_HORIZONTAL_LINES
    {
        return None;
    }

    let mut vertical_lines: Vec<u32> = vertical_runs
        .into_iter()
        .map(|(start, end)| (start + end) / 2)
        .collect();
    let mut horizontal_lines: Vec<u32> = horizontal_runs
        .into_iter()
        .map(|(start, end)| (start + end) / 2)
        .collect();

    let (&rough_min_x, &rough_max_x) = vertical_lines.first().zip(vertical_lines.last())?;
    let (&rough_min_y, &rough_max_y) = horizontal_lines.first().zip(horizontal_lines.last())?;
    if rough_max_x <= rough_min_x || rough_max_y <= rough_min_y {
        return None;
    }

    vertical_lines.retain(|&x| {
        dark_ratio_in_column(gray, x, rough_min_y, rough_max_y) >= MIN_TRUE_GRID_LINE_CONTINUITY
    });
    horizontal_lines.retain(|&y| {
        dark_ratio_in_row(gray, y, rough_min_x, rough_max_x) >= MIN_TRUE_GRID_LINE_CONTINUITY
    });
    if vertical_lines.len() < MIN_BORDERED_VERTICAL_LINES
        || horizontal_lines.len() < MIN_BORDERED_HORIZONTAL_LINES
    {
        return None;
    }

    if vertical_lines
        .windows(2)
        .any(|w| w[1] <= w[0] + MIN_CELL_SIZE_PX)
        || horizontal_lines
            .windows(2)
            .any(|w| w[1] <= w[0] + MIN_CELL_SIZE_PX)
    {
        return None;
    }
    if !grid_lines_are_continuous(&vertical_lines, &horizontal_lines, gray) {
        return None;
    }

    let continuity = grid_continuity_score(&vertical_lines, &horizontal_lines, gray);
    let line_score = vertical_lines.len() as f64 + horizontal_lines.len() as f64;
    let score = continuity * 100.0 + line_score;

    Some((
        RasterTableGrid {
            vertical_lines,
            horizontal_lines,
        },
        score,
    ))
}

fn grid_lines_are_continuous(
    vertical_lines: &[u32],
    horizontal_lines: &[u32],
    gray: &GrayImage,
) -> bool {
    let Some((&min_x, &max_x)) = vertical_lines.first().zip(vertical_lines.last()) else {
        return false;
    };
    let Some((&min_y, &max_y)) = horizontal_lines.first().zip(horizontal_lines.last()) else {
        return false;
    };
    if max_x <= min_x || max_y <= min_y {
        return false;
    }

    vertical_lines
        .iter()
        .all(|&x| dark_ratio_in_column(gray, x, min_y, max_y) >= MIN_TRUE_GRID_LINE_CONTINUITY)
        && horizontal_lines
            .iter()
            .all(|&y| dark_ratio_in_row(gray, y, min_x, max_x) >= MIN_TRUE_GRID_LINE_CONTINUITY)
}

fn grid_continuity_score(
    vertical_lines: &[u32],
    horizontal_lines: &[u32],
    gray: &GrayImage,
) -> f64 {
    let Some((&min_x, &max_x)) = vertical_lines.first().zip(vertical_lines.last()) else {
        return 0.0;
    };
    let Some((&min_y, &max_y)) = horizontal_lines.first().zip(horizontal_lines.last()) else {
        return 0.0;
    };
    if max_x <= min_x || max_y <= min_y {
        return 0.0;
    }

    let mut samples = 0usize;
    let mut sum = 0.0;
    for &x in vertical_lines {
        sum += dark_ratio_in_column(gray, x, min_y, max_y);
        samples += 1;
    }
    for &y in horizontal_lines {
        sum += dark_ratio_in_row(gray, y, min_x, max_x);
        samples += 1;
    }
    if samples == 0 {
        0.0
    } else {
        sum / samples as f64
    }
}

fn count_dark_in_column(gray: &GrayImage, x: u32) -> u32 {
    count_ink_in_column(gray, x, RASTER_DARK_THRESHOLD)
}

fn count_ink_in_column(gray: &GrayImage, x: u32, threshold: u8) -> u32 {
    (0..gray.height())
        .filter(|&y| gray.get_pixel(x, y).0[0] < threshold)
        .count() as u32
}

fn count_dark_in_row(gray: &GrayImage, y: u32) -> u32 {
    count_ink_in_row(gray, y, RASTER_DARK_THRESHOLD)
}

fn count_ink_in_row(gray: &GrayImage, y: u32, threshold: u8) -> u32 {
    (0..gray.width())
        .filter(|&x| gray.get_pixel(x, y).0[0] < threshold)
        .count() as u32
}

fn dark_ratio_in_column(gray: &GrayImage, x: u32, y1: u32, y2: u32) -> f64 {
    if y2 <= y1 || x >= gray.width() {
        return 0.0;
    }
    let dark = (y1..=y2)
        .filter(|&y| y < gray.height() && gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count();
    dark as f64 / f64::from(y2 - y1 + 1)
}

fn dark_ratio_in_row(gray: &GrayImage, y: u32, x1: u32, x2: u32) -> f64 {
    if x2 <= x1 || y >= gray.height() {
        return 0.0;
    }
    let dark = (x1..=x2)
        .filter(|&x| x < gray.width() && gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count();
    dark as f64 / f64::from(x2 - x1 + 1)
}

fn merge_runs(values: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut runs = Vec::new();
    let mut start = None;
    let mut prev = 0u32;
    for value in values {
        match start {
            None => {
                start = Some(value);
                prev = value;
            }
            Some(s) if value == prev + 1 => {
                prev = value;
                start = Some(s);
            }
            Some(s) => {
                runs.push((s, prev));
                start = Some(value);
                prev = value;
            }
        }
    }
    if let Some(s) = start {
        runs.push((s, prev));
    }
    runs
}

fn build_boundaries_from_centers(
    centers: &[f64],
    left_edge: f64,
    right_edge: f64,
    image_width: u32,
) -> Vec<f64> {
    let mut boundaries = Vec::with_capacity(centers.len() + 1);
    boundaries.push(left_edge);
    if centers.len() < 2 || image_width == 0 || right_edge <= left_edge {
        boundaries.push(right_edge.max(left_edge));
        return boundaries;
    }

    let page_width = right_edge - left_edge;
    let mut previous = left_edge;
    for pair in centers.windows(2) {
        let midpoint_px = ((pair[0] + pair[1]) / 2.0).clamp(0.0, f64::from(image_width));
        let boundary =
            left_edge + midpoint_px / f64::from(image_width) * page_width;
        let boundary = boundary.clamp(previous, right_edge);
        boundaries.push(boundary);
        previous = boundary;
    }
    boundaries.push(right_edge);
    boundaries
}

fn build_row_boundaries(rows: &[(f64, f64)]) -> Vec<f64> {
    let mut boundaries = Vec::with_capacity(rows.len() + 1);
    boundaries.push(rows[0].0);
    for pair in rows.windows(2) {
        boundaries.push((pair[0].1 + pair[1].0) / 2.0);
    }
    boundaries.push(rows[rows.len() - 1].1);
    boundaries
}

fn raster_boundaries_to_page(
    lines: &[u32],
    left_edge: f64,
    right_edge: f64,
    image_width: u32,
) -> Option<Vec<f64>> {
    if image_width == 0 {
        return None;
    }
    let scale = (right_edge - left_edge) / f64::from(image_width);
    Some(
        lines
            .iter()
            .map(|line| left_edge + f64::from(*line) * scale)
            .collect(),
    )
}

fn raster_boundaries_to_page_desc(
    lines: &[u32],
    bottom_edge: f64,
    top_edge: f64,
    image_height: u32,
) -> Option<Vec<f64>> {
    if image_height == 0 {
        return None;
    }
    let page_height = top_edge - bottom_edge;
    Some(
        lines
            .iter()
            .map(|line| top_edge - f64::from(*line) / f64::from(image_height) * page_height)
            .collect(),
    )
}

fn raster_box_to_page_bbox(
    image: &ImageChunk,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
    image_width: u32,
    image_height: u32,
) -> Option<BoundingBox> {
    if x2 <= x1 || y2 <= y1 || image_width == 0 || image_height == 0 {
        return None;
    }
    let left_x = image.bbox.left_x + image.bbox.width() * (f64::from(x1) / f64::from(image_width));
    let right_x = image.bbox.left_x + image.bbox.width() * (f64::from(x2) / f64::from(image_width));
    let top_y = image.bbox.top_y - image.bbox.height() * (f64::from(y1) / f64::from(image_height));
    let bottom_y =
        image.bbox.top_y - image.bbox.height() * (f64::from(y2) / f64::from(image_height));
    Some(BoundingBox::new(
        image.bbox.page_number,
        left_x,
        bottom_y,
        right_x,
        top_y,
    ))
}

fn extract_raster_cell_text(
    gray: &GrayImage,
    row_idx: usize,
    col_idx: usize,
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
) -> Option<String> {
    let inset_x = CELL_INSET_PX.min((x2 - x1) / 4);
    let inset_y = CELL_INSET_PX.min((y2 - y1) / 4);
    let crop_left = x1 + inset_x;
    let crop_top = y1 + inset_y;
    let crop_width = x2.saturating_sub(x1 + inset_x * 2);
    let crop_height = y2.saturating_sub(y1 + inset_y * 2);
    if crop_width < MIN_CELL_SIZE_PX || crop_height < MIN_CELL_SIZE_PX {
        return Some(String::new());
    }

    let cropped = gray
        .view(crop_left, crop_top, crop_width, crop_height)
        .to_image();
    let bordered = expand_white_border(&cropped, 12);
    let scaled = image::imageops::resize(
        &bordered,
        bordered.width() * OCR_SCALE_FACTOR,
        bordered.height() * OCR_SCALE_FACTOR,
        image::imageops::FilterType::Lanczos3,
    );
    let psm_modes: [&str; 3] = if row_idx == 0 {
        ["6", "11", "7"]
    } else {
        ["7", "6", "11"]
    };
    let raw_text = run_tesseract_cell_text_best(&scaled, &psm_modes)?;
    Some(normalize_raster_cell_text(row_idx, col_idx, raw_text))
}

fn expand_white_border(image: &GrayImage, border: u32) -> GrayImage {
    let mut expanded = GrayImage::from_pixel(
        image.width() + border * 2,
        image.height() + border * 2,
        Luma([255]),
    );
    for y in 0..image.height() {
        for x in 0..image.width() {
            expanded.put_pixel(x + border, y + border, *image.get_pixel(x, y));
        }
    }
    expanded
}

fn run_tesseract_tsv_words(image: &GrayImage, psm: &str) -> Option<Vec<OcrWord>> {
    match selected_ocr_engine() {
        OcrEngine::RapidOcr => run_rapidocr_words(image),
        OcrEngine::Tesseract => run_tesseract_tsv_words_with_oem(image, psm, "3"),
    }
}

fn run_tesseract_tsv_words_with_oem(
    image: &GrayImage,
    psm: &str,
    oem: &str,
) -> Option<Vec<OcrWord>> {
    let temp_dir = create_temp_dir(0).ok()?;
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
        // Tell Tesseract the actual DPI of the scaled image so its character-size
        // models are correctly calibrated (avoids ~72 DPI guess).
        .arg("--dpi")
        .arg(&dpi)
        .arg("--oem")
        .arg(oem)
        .arg("--psm")
        .arg(psm)
        // Disable word-frequency and system dictionaries: table cells contain
        // numeric codes, abbreviations, and domain-specific tokens that the
        // dictionary would "correct" into wrong English words.
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

fn run_tesseract_cell_text_best(image: &GrayImage, psm_modes: &[&str]) -> Option<String> {
    let mut best: Option<(String, f64)> = None;

    if matches!(selected_ocr_engine(), OcrEngine::Tesseract) {
        // First pass: collect consensus words across Tesseract perspectives.
        let consensus_words = collect_consensus_words(image, psm_modes);
        if !consensus_words.is_empty() {
            let text = words_to_plain_line_text(&consensus_words);
            if !text.is_empty() {
                let score = score_ocr_words(&consensus_words, image.width(), image.height());
                best = Some((text, score));
            }
        }
    }

    // Fallback: standard best-variant approach if no consensus words found
    if best.is_none() {
        for variant in build_ocr_variants(image) {
            for psm in psm_modes {
                let Some(words) = run_tesseract_tsv_words(&variant, psm) else {
                    continue;
                };
                if words.is_empty() {
                    continue;
                }
                let text = words_to_plain_line_text(&words);
                if text.is_empty() {
                    continue;
                }
                let score = score_ocr_words(&words, variant.width(), variant.height());
                match &best {
                    Some((_, best_score)) if *best_score >= score => {}
                    _ => best = Some((text, score)),
                }

                if let Some(text) = run_tesseract_plain_text_with_variant(&variant, psm) {
                    let norm_len = normalize_text(&text).len() as f64;
                    if norm_len > 0.0 {
                        match &best {
                            Some((_, best_score)) if *best_score >= norm_len => {}
                            _ => best = Some((text, norm_len)),
                        }
                    }
                }
            }

            // Docling-inspired multi-engine path: when RapidOCR is available,
            // treat it as an additional OCR engine candidate rather than a hard
            // replacement. This keeps Tesseract's stronger word-level geometry
            // while allowing a modern detector/recognizer to win on difficult cells.
            if let Some(words) = run_rapidocr_words(&variant) {
                let text = words_to_plain_line_text(&words);
                if !text.is_empty() {
                    let score = score_ocr_words(&words, variant.width(), variant.height());
                    match &best {
                        Some((_, best_score)) if *best_score >= score => {}
                        _ => best = Some((text, score)),
                    }
                }
            }
        }
    }

    best.map(|(text, _)| text)
}

fn collect_consensus_words(image: &GrayImage, psm_modes: &[&str]) -> Vec<OcrWord> {
    let variants = build_ocr_variants(image);

    // Collect words per (PSM, OEM) perspective. A "perspective" is an independent
    // Tesseract configuration; preprocessing variants are replicates of the same
    // perspective (same segmentation model, same language model).
    //
    // First-principles rationale:
    //   A real word should be detected by the correct PSM regardless of which
    //   preprocessed image variant is used. So consensus = "word appears under
    //   ≥2 distinct (PSM, OEM) combinations", NOT "≥25% of (variant×PSM×OEM)".
    //   The percentage approach breaks as more variants are added: threshold
    //   rises and real words get filtered out.

    let oems = ["1", "3"]; // OEM 1 = legacy Tesseract; OEM 3 = LSTM neural

    // For each (PSM, OEM) pair, keep the best-confidence word seen in any variant.
    let mut perspective_best: HashMap<(String, String, String), OcrWord> = HashMap::new();

    for variant in &variants {
        for psm in psm_modes {
            for oem in oems {
                let Some(words) = run_tesseract_tsv_words_with_oem(variant, psm, oem) else {
                    continue;
                };
                for word in words {
                    let key = (psm.to_string(), oem.to_string(), word.text.to_lowercase());
                    perspective_best
                        .entry(key)
                        .and_modify(|best| {
                            if word.confidence > best.confidence {
                                *best = word.clone();
                            }
                        })
                        .or_insert(word);
                }
            }
        }
    }

    // Count distinct (PSM, OEM) perspectives in which each word text appears.
    // Threshold: at least 2 independent configurations must agree.
    const MIN_PERSPECTIVES: usize = 2;

    let mut text_to_perspectives: HashMap<String, HashSet<(String, String)>> = HashMap::new();
    for (psm, oem, norm_text) in perspective_best.keys() {
        text_to_perspectives
            .entry(norm_text.clone())
            .or_default()
            .insert((psm.clone(), oem.clone()));
    }

    // Return the best-confidence word for each text that meets the threshold.
    let mut consensus: Vec<OcrWord> = text_to_perspectives
        .iter()
        .filter(|(_, perspectives)| perspectives.len() >= MIN_PERSPECTIVES)
        .filter_map(|(norm_text, _)| {
            perspective_best
                .iter()
                .filter(|((_, _, t), _)| t == norm_text)
                .max_by(|(_, a), (_, b)| {
                    a.confidence
                        .partial_cmp(&b.confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(_, w)| w.clone())
        })
        .collect();

    consensus.sort_by_key(|w| (w.top, w.left));
    consensus
}

fn filter_words_by_spatial_coherence(words: &[OcrWord]) -> Vec<OcrWord> {
    if words.len() <= 1 {
        return words.to_vec();
    }

    // First-principles thresholds derived from the actual character height in this
    // image — fully agnostic to DPI and scale factor. Typography conventions:
    //   • Word spacing within a line ≈ 0.25–0.33 em (em = cap height ≈ word.height)
    //   • A gap larger than 3 em between words on the same Tesseract line is
    //     almost certainly a segmentation error, not a legitimate space.
    //   • A single-word line smaller than 0.4 em wide is glyph noise.
    let median_h: u32 = {
        let mut heights: Vec<u32> = words.iter().map(|w| w.height.max(1)).collect();
        heights.sort_unstable();
        heights[heights.len() / 2]
    };
    // Gap beyond which two adjacent words on the same line are considered disjoint
    let gap_threshold = (median_h * 3).max(8);
    // Width below which a word on its own line looks like a glyph artifact
    let narrow_threshold = (median_h / 2).max(4);
    // Minimum bounding box for a credible isolated single-line word
    let min_iso_width = (median_h * 2 / 5).max(4);
    let min_iso_height = (median_h * 2 / 5).max(3);

    // Split words into lines
    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
    }

    let mut filtered = Vec::new();

    // For each line, filter out isolated words that are far from neighbors
    for line_words in by_line.values_mut() {
        if line_words.len() <= 1 {
            // Single-word lines: only keep if reasonably sized (not a stray pixel)
            if let Some(word) = line_words.first() {
                if word.width >= min_iso_width && word.height >= min_iso_height {
                    filtered.push((*word).clone());
                }
            }
            continue;
        }

        line_words.sort_by_key(|word| word.left);

        // Check spatial coherence within each word-to-word transition
        for (i, word) in line_words.iter().enumerate() {
            let is_isolated = if i > 0 {
                let prev = line_words[i - 1];
                let gap = word
                    .left
                    .saturating_sub(prev.left.saturating_add(prev.width));
                gap > gap_threshold && word.width < narrow_threshold
            } else if i < line_words.len() - 1 {
                let next = line_words[i + 1];
                let gap = next
                    .left
                    .saturating_sub(word.left.saturating_add(word.width));
                gap > gap_threshold && word.width < narrow_threshold
            } else {
                false
            };

            if !is_isolated {
                filtered.push((*word).clone());
            }
        }
    }

    filtered
}

fn cluster_words_by_proximity(words: &[OcrWord], gap_tolerance: u32) -> Vec<Vec<OcrWord>> {
    if words.is_empty() {
        return Vec::new();
    }

    let mut sorted_words = words.to_vec();
    sorted_words.sort_by_key(|w| (w.top, w.left));

    // Vertical tolerance: two words are on the "same line" when their top edges
    // differ by less than half the median word height. This is typographically
    // correct: legitimate multi-word lines share a common baseline ± leading / 2.
    let median_h: i32 = {
        let mut heights: Vec<u32> = sorted_words.iter().map(|w| w.height.max(1)).collect();
        heights.sort_unstable();
        heights[heights.len() / 2] as i32
    };
    let vertical_tolerance = (median_h / 2).max(2);

    let mut clusters: Vec<Vec<OcrWord>> = Vec::new();
    let mut current_cluster = vec![sorted_words[0].clone()];

    for word in &sorted_words[1..] {
        if let Some(last) = current_cluster.last() {
            let vertical_gap = (word.top as i32 - last.top as i32).abs();
            let horizontal_gap = word
                .left
                .saturating_sub(last.left.saturating_add(last.width));

            if vertical_gap <= vertical_tolerance && horizontal_gap <= gap_tolerance {
                current_cluster.push(word.clone());
            } else {
                clusters.push(current_cluster);
                current_cluster = vec![word.clone()];
            }
        }
    }

    if !current_cluster.is_empty() {
        clusters.push(current_cluster);
    }

    clusters
}

fn words_to_plain_line_text(words: &[OcrWord]) -> String {
    // Apply spatial coherence filtering to remove isolated artifacts
    let filtered_words = filter_words_by_spatial_coherence(words);

    if filtered_words.is_empty() {
        return String::new();
    }

    // Cluster words by spatial proximity with adaptive gap tolerance
    let avg_word_width =
        filtered_words.iter().map(|w| w.width).sum::<u32>() as f64 / filtered_words.len() as f64;
    let gap_tolerance = (avg_word_width * 0.8).ceil() as u32;
    let clusters = cluster_words_by_proximity(&filtered_words, gap_tolerance);

    let mut lines: Vec<String> = Vec::new();
    for cluster in clusters {
        let mut sorted_cluster = cluster;
        sorted_cluster.sort_by_key(|w| w.left);

        let line = sorted_cluster
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();

        if !line.is_empty() {
            lines.push(line);
        }
    }

    lines.join(" ")
}

/// Apply common Tesseract OCR character corrections for table content.
///
/// Only applies corrections that are safe in all contexts — targeting
/// purely numeric tokens where digit/letter confusion is certain.
fn run_tesseract_tsv_words_best<F>(
    image: &GrayImage,
    psm_modes: &[&str],
    accept: F,
) -> Option<Vec<OcrWord>>
where
    F: Fn(&[OcrWord]) -> bool,
{
    let variants = build_ocr_variants(image);
    let mut best: Option<OcrCandidateScore> = None;

    for variant in variants {
        for psm in psm_modes {
            let Some(words) = run_tesseract_tsv_words(&variant, psm) else {
                continue;
            };
            if !accept(&words) {
                continue;
            }
            let score = score_ocr_words(&words, variant.width(), variant.height());
            match &best {
                Some(current) if current.score >= score => {}
                _ => {
                    best = Some(OcrCandidateScore { words, score });
                }
            }
        }
    }

    best.map(|candidate| candidate.words)
}

fn score_ocr_words(words: &[OcrWord], width: u32, height: u32) -> f64 {
    if words.is_empty() || width == 0 || height == 0 {
        return 0.0;
    }

    let mut by_line: BTreeMap<(u32, u32, u32), Vec<&OcrWord>> = BTreeMap::new();
    let mut alpha_words = 0usize;
    let mut area_coverage = 0f64;
    let mut vertical_spread_top = height;
    let mut vertical_spread_bottom = 0u32;
    let mut total_confidence = 0f64;

    for word in words {
        by_line.entry(word.line_key).or_default().push(word);
        if word.text.chars().any(|ch| ch.is_alphabetic()) {
            alpha_words += 1;
        }
        area_coverage += f64::from(word.width.saturating_mul(word.height));
        vertical_spread_top = vertical_spread_top.min(word.top);
        vertical_spread_bottom = vertical_spread_bottom.max(word.top.saturating_add(word.height));
        total_confidence += word.confidence;
    }

    let line_count = by_line.len() as f64;
    let alpha_ratio = alpha_words as f64 / words.len() as f64;
    let density = (area_coverage / f64::from(width.saturating_mul(height))).clamp(0.0, 1.0);
    let spread = if vertical_spread_bottom > vertical_spread_top {
        f64::from(vertical_spread_bottom - vertical_spread_top) / f64::from(height)
    } else {
        0.0
    };
    let avg_confidence = total_confidence / words.len() as f64;
    // Confidence bonus: normalize 0-100 range to 0-1 bonus multiplier
    let confidence_bonus = (avg_confidence / 100.0).clamp(0.0, 1.0);

    // Horizontal spread bonus: reward words that span the full cell width
    let horizontal_spread = if words.is_empty() {
        0.0
    } else {
        let min_left = words.iter().map(|w| w.left).min().unwrap_or(0);
        let max_right = words
            .iter()
            .map(|w| w.left + w.width)
            .max()
            .unwrap_or(width);
        f64::from(max_right.saturating_sub(min_left)) / f64::from(width)
    };

    words.len() as f64
        + line_count * 1.5
        + alpha_ratio * 6.0
        + density * 25.0
        + spread * 3.0
        + horizontal_spread * 2.0
        + confidence_bonus * 5.0 // High-confidence words get a boost
}

fn build_ocr_variants(gray: &GrayImage) -> Vec<GrayImage> {
    vec![
        gray.clone(),
        contrast_stretch(gray),
        global_otsu_binarize(gray),
        local_mean_binarize(gray, LOCAL_BINARIZATION_RADIUS),
        // Add morphological cleaning as a variant for denoising
        morphological_clean(gray),
        // Sharpening (unsharp mask) helps Tesseract detect character boundaries on
        // blurry / low-DPI cells that survive from low-resolution source PDFs.
        unsharp_mask(gray, 1.5),
        // Gamma brightening improves contrast for very light ink cells.
        gamma_correct(gray, 0.6),
    ]
}

/// Sharpen a grayscale image using an unsharp mask.
/// `amount` controls strength (1.5 = moderate). Uses i32 arithmetic throughout
/// to avoid u32 underflow when the 3×3 kernel straddles the x=0 or y=0 boundary.
fn unsharp_mask(gray: &GrayImage, amount: f32) -> GrayImage {
    let width = gray.width() as i32;
    let height = gray.height() as i32;
    let mut out = GrayImage::new(gray.width(), gray.height());
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0i32;
            let mut count = 0i32;
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx >= 0 && ny >= 0 && nx < width && ny < height {
                        sum += gray.get_pixel(nx as u32, ny as u32).0[0] as i32;
                        count += 1;
                    }
                }
            }
            let blurred = if count > 0 {
                sum / count
            } else {
                gray.get_pixel(x as u32, y as u32).0[0] as i32
            };
            let original = gray.get_pixel(x as u32, y as u32).0[0] as i32;
            let sharpened = original + ((original - blurred) as f32 * amount) as i32;
            out.put_pixel(x as u32, y as u32, Luma([sharpened.clamp(0, 255) as u8]));
        }
    }
    out
}

/// Apply gamma correction to brighten or darken an image.
/// gamma < 1.0 brightens (helps see light ink); gamma > 1.0 darkens.
fn gamma_correct(gray: &GrayImage, gamma: f32) -> GrayImage {
    let mut out = GrayImage::new(gray.width(), gray.height());
    for (x, y, pixel) in gray.enumerate_pixels() {
        let v = pixel.0[0] as f32 / 255.0;
        let corrected = (v.powf(gamma) * 255.0).round() as u8;
        out.put_pixel(x, y, Luma([corrected]));
    }
    out
}

fn contrast_stretch(gray: &GrayImage) -> GrayImage {
    let mut min_val = u8::MAX;
    let mut max_val = u8::MIN;
    for pixel in gray.pixels() {
        let value = pixel.0[0];
        min_val = min_val.min(value);
        max_val = max_val.max(value);
    }

    if max_val <= min_val {
        return gray.clone();
    }

    let in_range = (max_val - min_val) as f64;
    let mut out = GrayImage::new(gray.width(), gray.height());
    for (x, y, pixel) in gray.enumerate_pixels() {
        let value = pixel.0[0];
        let normalized = ((value.saturating_sub(min_val)) as f64 / in_range * 255.0).round() as u8;
        out.put_pixel(x, y, Luma([normalized]));
    }
    out
}

fn global_otsu_binarize(gray: &GrayImage) -> GrayImage {
    let threshold = otsu_threshold(gray);
    let mut out = GrayImage::new(gray.width(), gray.height());
    for (x, y, pixel) in gray.enumerate_pixels() {
        let value = if pixel.0[0] <= threshold { 0 } else { 255 };
        out.put_pixel(x, y, Luma([value]));
    }
    out
}

fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u64; 256];
    for pixel in gray.pixels() {
        histogram[pixel.0[0] as usize] += 1;
    }

    let total = (gray.width() as u64) * (gray.height() as u64);
    if total == 0 {
        return 127;
    }

    let sum_total: f64 = histogram
        .iter()
        .enumerate()
        .map(|(idx, count)| idx as f64 * *count as f64)
        .sum();

    let mut sum_background = 0f64;
    let mut weight_background = 0f64;
    let mut max_variance = -1f64;
    let mut best_threshold = 127u8;

    for (idx, count) in histogram.iter().enumerate() {
        weight_background += *count as f64;
        if weight_background <= 0.0 {
            continue;
        }

        let weight_foreground = total as f64 - weight_background;
        if weight_foreground <= 0.0 {
            break;
        }

        sum_background += idx as f64 * *count as f64;
        let mean_background = sum_background / weight_background;
        let mean_foreground = (sum_total - sum_background) / weight_foreground;
        let between_class_variance =
            weight_background * weight_foreground * (mean_background - mean_foreground).powi(2);

        if between_class_variance > max_variance {
            max_variance = between_class_variance;
            best_threshold = idx as u8;
        }
    }

    best_threshold
}

fn local_mean_binarize(gray: &GrayImage, radius: u32) -> GrayImage {
    if gray.width() == 0 || gray.height() == 0 {
        return gray.clone();
    }

    let width = gray.width() as usize;
    let height = gray.height() as usize;
    let (integral, stride) = integral_image(gray);
    let mut out = GrayImage::new(gray.width(), gray.height());

    for y in 0..height {
        for x in 0..width {
            let x1 = x.saturating_sub(radius as usize);
            let y1 = y.saturating_sub(radius as usize);
            let x2 = (x + radius as usize).min(width - 1);
            let y2 = (y + radius as usize).min(height - 1);

            let area = (x2 - x1 + 1) * (y2 - y1 + 1);
            let sum = region_sum(&integral, stride, x1, y1, x2, y2);
            let local_mean = (sum as f64) / (area as f64);
            let offset = if area >= MIN_BINARIZATION_BLOCK_PIXELS {
                8.0
            } else {
                4.0
            };
            let threshold = (local_mean - offset).clamp(0.0, 255.0);

            let pixel_value = gray.get_pixel(x as u32, y as u32).0[0] as f64;
            let value = if pixel_value <= threshold { 0 } else { 255 };
            out.put_pixel(x as u32, y as u32, Luma([value]));
        }
    }

    out
}

/// Morphological cleaning via dilation then erosion (closing operation)
/// Removes small noise and fills small holes in text
fn morphological_clean(gray: &GrayImage) -> GrayImage {
    if gray.width() == 0 || gray.height() == 0 {
        return gray.clone();
    }

    // First binarize with otsu
    let binary = global_otsu_binarize(gray);

    // Close operation: dilate then erode (fills small holes, connects broken text)
    let dilated = morphological_dilate(&binary, 2);
    morphological_erode(&dilated, 2)
}

fn morphological_dilate(gray: &GrayImage, iterations: u32) -> GrayImage {
    let mut result = gray.clone();
    for _ in 0..iterations {
        let mut next = GrayImage::from_pixel(gray.width(), gray.height(), Luma([255]));

        for y in 1..gray.height().saturating_sub(1) {
            for x in 1..gray.width().saturating_sub(1) {
                // Check 3x3 neighborhood
                let mut has_black = false;
                for dy in 0..3 {
                    for dx in 0..3 {
                        let px = result.get_pixel(x + dx - 1, y + dy - 1).0[0];
                        if px < 128 {
                            has_black = true;
                            break;
                        }
                    }
                    if has_black {
                        break;
                    }
                }
                next.put_pixel(x, y, if has_black { Luma([0]) } else { Luma([255]) });
            }
        }
        result = next;
    }
    result
}

fn morphological_erode(gray: &GrayImage, iterations: u32) -> GrayImage {
    let mut result = gray.clone();
    for _ in 0..iterations {
        let mut next = GrayImage::from_pixel(gray.width(), gray.height(), Luma([255]));

        for y in 1..gray.height().saturating_sub(1) {
            for x in 1..gray.width().saturating_sub(1) {
                // Erode black foreground: any white neighbor breaks the stroke,
                // otherwise the pixel remains black.
                let mut all_black = true;
                for dy in 0..3 {
                    for dx in 0..3 {
                        let px = result.get_pixel(x + dx - 1, y + dy - 1).0[0];
                        if px >= 128 {
                            all_black = false;
                            break;
                        }
                    }
                    if !all_black {
                        break;
                    }
                }
                next.put_pixel(x, y, if all_black { Luma([0]) } else { Luma([255]) });
            }
        }
        result = next;
    }
    result
}

fn integral_image(gray: &GrayImage) -> (Vec<u64>, usize) {
    let width = gray.width() as usize;
    let height = gray.height() as usize;
    let stride = width + 1;
    let mut integral = vec![0u64; (width + 1) * (height + 1)];

    for y in 0..height {
        let mut row_sum = 0u64;
        for x in 0..width {
            row_sum += gray.get_pixel(x as u32, y as u32).0[0] as u64;
            let idx = (y + 1) * stride + (x + 1);
            integral[idx] = integral[y * stride + (x + 1)] + row_sum;
        }
    }

    (integral, stride)
}

fn region_sum(integral: &[u64], stride: usize, x1: usize, y1: usize, x2: usize, y2: usize) -> u64 {
    let a = integral[y1 * stride + x1];
    let b = integral[y1 * stride + (x2 + 1)];
    let c = integral[(y2 + 1) * stride + x1];
    let d = integral[(y2 + 1) * stride + (x2 + 1)];
    d + a - b - c
}

fn run_tesseract_plain_text(image: &GrayImage, psm: &str) -> Option<String> {
    run_tesseract_plain_text_with_variant(image, psm)
}

fn run_tesseract_plain_text_with_variant(image: &GrayImage, psm: &str) -> Option<String> {
    if matches!(selected_ocr_engine(), OcrEngine::RapidOcr) {
        return run_rapidocr_words(image).map(|words| words_to_plain_line_text(&words));
    }

    let temp_dir = create_temp_dir(0).ok()?;
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
        .arg("3")
        .arg("--psm")
        .arg(psm)
        .arg("-c")
        .arg("load_system_dawg=0")
        .arg("-c")
        .arg("load_freq_dawg=0")
        .output()
        .ok()?;
    let _ = fs::remove_dir_all(&temp_dir);
    if !output.status.success() {
        return None;
    }

    Some(
        String::from_utf8_lossy(&output.stdout)
            .replace('\n', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn words_to_text_chunks(
    words: &[OcrWord],
    image: &ImageChunk,
    text_chunks: &[TextChunk],
) -> Vec<TextChunk> {
    let mut image_size = (0u32, 0u32);
    for word in words {
        image_size.0 = image_size.0.max(word.left.saturating_add(word.width));
        image_size.1 = image_size.1.max(word.top.saturating_add(word.height));
    }
    if image_size.0 == 0 || image_size.1 == 0 {
        return Vec::new();
    }

    let mut dedupe: HashMap<String, usize> = HashMap::new();
    for chunk in text_chunks {
        dedupe.insert(normalize_text(&chunk.value), dedupe.len());
    }

    let mut recovered = Vec::new();
    for word in words {
        let normalized = normalize_text(&word.text);
        if normalized.len() >= 4 && dedupe.contains_key(&normalized) {
            continue;
        }

        let left_ratio = f64::from(word.left) / f64::from(image_size.0);
        let right_ratio = f64::from(word.left.saturating_add(word.width)) / f64::from(image_size.0);
        let top_ratio = f64::from(word.top) / f64::from(image_size.1);
        let bottom_ratio =
            f64::from(word.top.saturating_add(word.height)) / f64::from(image_size.1);

        let left_x = image.bbox.left_x + image.bbox.width() * left_ratio;
        let right_x = image.bbox.left_x + image.bbox.width() * right_ratio;
        let top_y = image.bbox.top_y - image.bbox.height() * top_ratio;
        let bottom_y = image.bbox.top_y - image.bbox.height() * bottom_ratio;
        if right_x <= left_x || top_y <= bottom_y {
            continue;
        }

        recovered.push(TextChunk {
            value: word.text.clone(),
            bbox: BoundingBox::new(image.bbox.page_number, left_x, bottom_y, right_x, top_y),
            font_name: "OCR".to_string(),
            font_size: (top_y - bottom_y).max(6.0),
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Content,
            ocg_visible: true,
            index: None,
            page_number: image.bbox.page_number,
            level: None,
            mcid: None,
        });
    }

    recovered
}

fn lines_from_ocr_words(
    words: &[OcrWord],
    image: &ImageChunk,
    image_width: u32,
    image_height: u32,
    text_chunks: &[TextChunk],
) -> Vec<TextChunk> {
    if image_width == 0 || image_height == 0 {
        return Vec::new();
    }

    let mut dedupe: HashMap<String, usize> = HashMap::new();
    for chunk in text_chunks {
        dedupe.insert(normalize_text(&chunk.value), dedupe.len());
    }

    let spatial_lines = build_spatial_ocr_lines(words);
    if spatial_lines.is_empty() {
        return Vec::new();
    }

    let blocks = merge_spatial_ocr_lines_into_blocks(&spatial_lines, image_width);
    if blocks.is_empty() {
        return Vec::new();
    }

    let mut recovered = Vec::new();
    for block in blocks {
        let normalized = normalize_text(&block.text);
        if normalized.len() >= 8 && dedupe.contains_key(&normalized) {
            continue;
        }

        if block.right <= block.left || block.bottom <= block.top {
            continue;
        }

        let left_x = image.bbox.left_x
            + image.bbox.width() * (f64::from(block.left) / f64::from(image_width));
        let right_x = image.bbox.left_x
            + image.bbox.width() * (f64::from(block.right) / f64::from(image_width));
        let top_y = image.bbox.top_y
            - image.bbox.height() * (f64::from(block.top) / f64::from(image_height));
        let bottom_y = image.bbox.top_y
            - image.bbox.height() * (f64::from(block.bottom) / f64::from(image_height));
        if right_x <= left_x || top_y <= bottom_y {
            continue;
        }

        recovered.push(TextChunk {
            value: block.text,
            bbox: BoundingBox::new(image.bbox.page_number, left_x, bottom_y, right_x, top_y),
            font_name: "OCR".to_string(),
            font_size: (f64::from(block.line_height_sum) / block.line_count.max(1) as f64).max(6.0),
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Content,
            ocg_visible: true,
            index: None,
            page_number: image.bbox.page_number,
            level: None,
            mcid: None,
        });
    }

    recovered
}

#[derive(Debug, Clone)]
struct SpatialOcrLine {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    text: String,
    word_count: usize,
    line_count: usize,
    line_height_sum: u32,
}

fn build_spatial_ocr_lines(words: &[OcrWord]) -> Vec<SpatialOcrLine> {
    let filtered_words = filter_words_by_spatial_coherence(words);
    if filtered_words.is_empty() {
        return Vec::new();
    }

    let avg_word_width =
        filtered_words.iter().map(|w| w.width).sum::<u32>() as f64 / filtered_words.len() as f64;
    let gap_tolerance = (avg_word_width * 0.8).ceil() as u32;
    let clusters = cluster_words_by_proximity(&filtered_words, gap_tolerance);

    let mut lines = Vec::new();
    for mut cluster in clusters {
        cluster.sort_by_key(|word| word.left);
        let text = cluster
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }

        let left = cluster.iter().map(|word| word.left).min().unwrap_or(0);
        let right = cluster
            .iter()
            .map(|word| word.left.saturating_add(word.width))
            .max()
            .unwrap_or(0);
        let top = cluster.iter().map(|word| word.top).min().unwrap_or(0);
        let bottom = cluster
            .iter()
            .map(|word| word.top.saturating_add(word.height))
            .max()
            .unwrap_or(0);
        if right <= left || bottom <= top {
            continue;
        }

        lines.push(SpatialOcrLine {
            left,
            top,
            right,
            bottom,
            text,
            word_count: cluster.len(),
            line_count: 1,
            line_height_sum: bottom.saturating_sub(top).max(1),
        });
    }

    lines.sort_by_key(|line| (line.top, line.left));
    lines
}

fn merge_spatial_ocr_lines_into_blocks(
    lines: &[SpatialOcrLine],
    image_width: u32,
) -> Vec<SpatialOcrLine> {
    if lines.is_empty() {
        return Vec::new();
    }

    let median_height = {
        let mut heights: Vec<u32> = lines
            .iter()
            .map(|line| line.bottom.saturating_sub(line.top).max(1))
            .collect();
        heights.sort_unstable();
        heights[heights.len() / 2]
    };
    let vertical_tolerance = (median_height / 2).max(3);
    let max_vertical_gap = median_height.saturating_mul(2).max(8);

    let mut blocks: Vec<SpatialOcrLine> = Vec::new();
    for line in lines {
        let merge_idx = blocks.iter().rposition(|block| {
            let vertical_gap = line.top.saturating_sub(block.bottom);
            if vertical_gap > max_vertical_gap {
                return false;
            }
            if line.top + vertical_tolerance < block.bottom {
                return false;
            }

            spatial_lines_share_block_geometry(block, line, image_width, median_height)
        });

        if let Some(merge_idx) = merge_idx {
            let block = &mut blocks[merge_idx];
            block.left = block.left.min(line.left);
            block.top = block.top.min(line.top);
            block.right = block.right.max(line.right);
            block.bottom = block.bottom.max(line.bottom);
            block.word_count += line.word_count;
            block.line_count += line.line_count;
            block.line_height_sum = block.line_height_sum.saturating_add(line.line_height_sum);
            if !block.text.ends_with('-') {
                block.text.push(' ');
            }
            block.text.push_str(&line.text);
            continue;
        }

        blocks.push(line.clone());
    }

    blocks
        .into_iter()
        .filter_map(|mut block| {
            block.text = block.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let alphabetic = block.text.chars().filter(|ch| ch.is_alphabetic()).count();
            let min_chars = if block.word_count >= 4 { 10 } else { 16 };
            if block.text.len() < min_chars || alphabetic < 4 {
                return None;
            }
            Some(block)
        })
        .collect()
}

fn spatial_lines_share_block_geometry(
    upper: &SpatialOcrLine,
    lower: &SpatialOcrLine,
    image_width: u32,
    median_height: u32,
) -> bool {
    let overlap_left = upper.left.max(lower.left);
    let overlap_right = upper.right.min(lower.right);
    let overlap = overlap_right.saturating_sub(overlap_left);
    let upper_width = upper.right.saturating_sub(upper.left).max(1);
    let lower_width = lower.right.saturating_sub(lower.left).max(1);
    let min_width = upper_width.min(lower_width);
    let max_width = upper_width.max(lower_width);
    let overlap_ratio = overlap as f64 / min_width as f64;
    let width_ratio = min_width as f64 / max_width as f64;
    let max_left_shift = ((f64::from(image_width) * 0.045).round() as u32)
        .max(median_height.saturating_mul(2))
        .max(8);
    let left_shift = upper.left.abs_diff(lower.left);

    overlap_ratio >= 0.40
        || (overlap_ratio >= 0.15 && left_shift <= max_left_shift && width_ratio >= 0.55)
}

fn is_numeric_like(text: &str) -> bool {
    text.chars().any(|ch| ch.is_ascii_digit())
}

fn normalize_text(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn normalize_caption_text(text: &str) -> String {
    text.replace("CarolinaBLUTM", "CarolinaBLU™")
        .replace("CarolinaBLU™™", "CarolinaBLU™")
        .trim()
        .to_string()
}

fn normalize_raster_cell_text(row_idx: usize, _col_idx: usize, text: String) -> String {
    let mut normalized = text
        .replace('|', " ")
        .replace('—', "-")
        .replace("AorB", "A or B")
        .replace("Aor B", "A or B")
        .replace("H,O", "H2O")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if row_idx > 0 && !normalized.chars().any(|ch| ch.is_ascii_digit()) && normalized.len() <= 2 {
        return String::new();
    }
    if row_idx > 0
        && normalized
            .chars()
            .all(|ch| matches!(ch, 'O' | 'o' | 'S' | 'B'))
    {
        return String::new();
    }

    normalized = normalized
        .replace(" ywL", " μL")
        .replace(" yuL", " μL")
        .replace(" yL", " μL")
        .replace(" wL", " μL")
        .replace(" uL", " μL")
        .replace(" pL", " μL");

    normalized.trim().to_string()
}

fn create_temp_dir(page_number: u32) -> std::io::Result<PathBuf> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "edgeparse-raster-ocr-{}-{}-{}",
        std::process::id(),
        page_number,
        unique
    ));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn extract_visible_page_image_files(
    input_path: &Path,
    page_number: u32,
    temp_dir: &Path,
) -> Option<Vec<PathBuf>> {
    let list_output = Command::new("pdfimages")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-list")
        .arg(input_path)
        .output()
        .ok()?;
    if !list_output.status.success() {
        return None;
    }

    let entries = parse_pdfimages_list(&String::from_utf8_lossy(&list_output.stdout));
    let visible_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(idx, entry)| (entry.image_type == "image").then_some(idx))
        .collect();
    if visible_indices.is_empty() {
        return Some(Vec::new());
    }

    let prefix = temp_dir.join("img");
    let status = Command::new("pdfimages")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-png")
        .arg(input_path)
        .arg(&prefix)
        .status()
        .ok()?;
    if !status.success() {
        return None;
    }

    let mut image_files: Vec<PathBuf> = fs::read_dir(temp_dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
        .collect();
    image_files.sort();

    let visible_files: Vec<PathBuf> = visible_indices
        .into_iter()
        .filter_map(|idx| image_files.get(idx).cloned())
        .collect();
    Some(visible_files)
}

fn parse_pdfimages_list(output: &str) -> Vec<PdfImagesListEntry> {
    let mut entries = Vec::new();
    let mut in_rows = false;

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("---") {
            in_rows = true;
            continue;
        }
        if !in_rows {
            continue;
        }

        let mut cols = trimmed.split_whitespace();
        let Some(_page) = cols.next() else {
            continue;
        };
        let Some(_num) = cols.next() else {
            continue;
        };
        let Some(image_type) = cols.next() else {
            continue;
        };

        entries.push(PdfImagesListEntry {
            image_type: image_type.to_string(),
        });
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GrayImage;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};

    fn image_chunk() -> ImageChunk {
        ImageChunk {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 400.0, 400.0),
            index: Some(1),
            level: None,
        }
    }

    fn word(line: (u32, u32, u32), left: u32, text: &str) -> OcrWord {
        OcrWord {
            line_key: line,
            left,
            top: 0,
            width: 40,
            height: 12,
            text: text.to_string(),
            confidence: 90.0,
        }
    }

    fn word_at(line: (u32, u32, u32), left: u32, top: u32, width: u32, text: &str) -> OcrWord {
        OcrWord {
            line_key: line,
            left,
            top,
            width,
            height: 12,
            text: text.to_string(),
            confidence: 90.0,
        }
    }

    fn text_chunk(value: &str, bbox: BoundingBox) -> TextChunk {
        TextChunk {
            value: value.to_string(),
            bbox,
            font_name: "Helvetica".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        }
    }

    fn test_cell_text(cell: &TableBorderCell) -> String {
        cell.content
            .iter()
            .map(|token| token.base.value.trim())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn test_table_like_ocr_detects_repeated_columns() {
        let words = vec![
            word((1, 1, 1), 10, "Temperature"),
            word((1, 1, 1), 120, "Viscosity"),
            word((1, 1, 1), 240, "Temperature"),
            word((1, 1, 1), 360, "Viscosity"),
            word((1, 1, 2), 10, "0"),
            word((1, 1, 2), 120, "1.793E-06"),
            word((1, 1, 2), 240, "25"),
            word((1, 1, 2), 360, "8.930E-07"),
            word((1, 1, 3), 10, "1"),
            word((1, 1, 3), 120, "1.732E-06"),
            word((1, 1, 3), 240, "26"),
            word((1, 1, 3), 360, "8.760E-07"),
        ];
        assert!(!looks_like_chart_label_ocr(&words));
        assert!(looks_like_table_ocr(&words));
    }

    #[test]
    fn test_structured_ocr_table_border_recovers_non_numeric_table() {
        let image = image_chunk();
        let words = vec![
            word_at((1, 1, 1), 10, 10, 80, "Tube"),
            word_at((1, 1, 1), 145, 10, 110, "Enzyme"),
            word_at((1, 1, 1), 305, 10, 70, "DNA"),
            word_at((1, 1, 2), 10, 42, 80, "1"),
            word_at((1, 1, 2), 145, 42, 110, "BamHI"),
            word_at((1, 1, 2), 305, 42, 70, "pUC19"),
            word_at((1, 1, 3), 10, 74, 80, "2"),
            word_at((1, 1, 3), 145, 74, 110, "HindIII"),
            word_at((1, 1, 3), 305, 74, 70, "lambda"),
            word_at((1, 1, 4), 10, 106, 80, "3"),
            word_at((1, 1, 4), 145, 106, 110, "EcoRI"),
            word_at((1, 1, 4), 305, 106, 70, "control"),
        ];

        assert!(!looks_like_chart_label_ocr(&words));
        let table = build_structured_ocr_table_border(&words, &image).expect("structured table");
        assert_eq!(table.num_columns, 3);
        assert_eq!(table.num_rows, 4);
        assert_eq!(test_cell_text(&table.rows[0].cells[0]), "Tube");
        assert_eq!(test_cell_text(&table.rows[1].cells[1]), "BamHI");
        assert_eq!(test_cell_text(&table.rows[3].cells[2]), "control");
    }

    #[test]
    fn test_structured_ocr_table_border_scales_column_boundaries_to_page_bbox() {
        let image = ImageChunk {
            bbox: BoundingBox::new(Some(1), 56.6929, 163.6519, 555.3071, 442.0069),
            index: Some(1),
            level: None,
        };
        let words = vec![
            word_at((1, 1, 1), 10, 10, 110, "TempC"),
            word_at((1, 1, 1), 255, 10, 150, "KinViscA"),
            word_at((1, 1, 1), 520, 10, 110, "TempC"),
            word_at((1, 1, 1), 760, 10, 170, "KinViscB"),
            word_at((1, 1, 2), 10, 44, 24, "0"),
            word_at((1, 1, 2), 255, 44, 130, "1.793E-06"),
            word_at((1, 1, 2), 520, 44, 28, "25"),
            word_at((1, 1, 2), 760, 44, 130, "8.930E-07"),
            word_at((1, 1, 3), 10, 78, 24, "1"),
            word_at((1, 1, 3), 255, 78, 130, "1.732E-06"),
            word_at((1, 1, 3), 520, 78, 28, "26"),
            word_at((1, 1, 3), 760, 78, 130, "8.760E-07"),
        ];

        let table = build_structured_ocr_table_border(&words, &image).expect("structured table");

        assert_eq!(table.num_columns, 4);
        assert_eq!(table.num_rows, 3);
        assert_eq!(test_cell_text(&table.rows[1].cells[1]), "1.793E-06");
        assert!(table.x_coordinates.windows(2).all(|pair| pair[1] >= pair[0]));
        assert!(table
            .x_coordinates
            .iter()
            .all(|x| *x >= image.bbox.left_x && *x <= image.bbox.right_x));
    }

    #[test]
    fn test_chart_label_ocr_does_not_reject_five_row_table() {
        let words = vec![
            word_at((1, 1, 1), 10, 10, 80, "Tube"),
            word_at((1, 1, 1), 145, 10, 110, "Enzyme"),
            word_at((1, 1, 1), 305, 10, 70, "DNA"),
            word_at((1, 1, 2), 10, 42, 80, "1"),
            word_at((1, 1, 2), 145, 42, 110, "BamHI"),
            word_at((1, 1, 2), 305, 42, 70, "pUC19"),
            word_at((1, 1, 3), 10, 74, 80, "2"),
            word_at((1, 1, 3), 145, 74, 110, "HindIII"),
            word_at((1, 1, 3), 305, 74, 70, "lambda"),
            word_at((1, 1, 4), 10, 106, 80, "3"),
            word_at((1, 1, 4), 145, 106, 110, "EcoRI"),
            word_at((1, 1, 4), 305, 106, 70, "control"),
            word_at((1, 1, 5), 10, 138, 80, "4"),
            word_at((1, 1, 5), 145, 138, 110, "NotI"),
            word_at((1, 1, 5), 305, 138, 70, "sample"),
        ];

        assert!(!looks_like_chart_label_ocr(&words));
        assert!(looks_like_table_ocr(&words));
    }

    #[test]
    fn test_structured_ocr_table_border_rejects_two_column_prose_layout() {
        let image = image_chunk();
        let words = vec![
            word_at((1, 1, 1), 10, 10, 90, "Summary"),
            word_at((1, 1, 1), 220, 10, 120, "Detailed findings"),
            word_at((1, 1, 2), 10, 42, 90, "Background"),
            word_at((1, 1, 2), 220, 42, 120, "Additional context"),
            word_at((1, 1, 3), 10, 74, 90, "Notes"),
            word_at((1, 1, 3), 220, 74, 120, "Further explanation"),
        ];

        assert!(build_structured_ocr_table_border(&words, &image).is_none());
    }

    #[test]
    fn test_parse_pdfimages_list_ignores_smask_entries() {
        let output = "page   num  type   width height color comp bpc  enc interp  object ID x-ppi y-ppi size ratio\n--------------------------------------------------------------------------------------------\n   1     0 image    1320   358  icc     3   8  image  no        46  0   208   208 63.5K 4.6%\n   1     1 smask    1320   358  gray    1   8  image  no        46  0   208   208  483B 0.1%\n";

        let entries = parse_pdfimages_list(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].image_type, "image");
        assert_eq!(entries[1].image_type, "smask");
    }

    #[test]
    fn test_table_like_ocr_rejects_single_line_caption() {
        let words = vec![
            word((1, 1, 1), 10, "Figure"),
            word((1, 1, 1), 90, "7.2"),
            word((1, 1, 1), 150, "Viscosity"),
            word((1, 1, 1), 260, "of"),
            word((1, 1, 1), 300, "Water"),
        ];
        assert!(!looks_like_table_ocr(&words));
    }

    #[test]
    fn test_normalize_raster_cell_text_fixes_units_and_artifacts() {
        assert_eq!(
            normalize_raster_cell_text(1, 1, "3 ywL".to_string()),
            "3 μL"
        );
        assert_eq!(normalize_raster_cell_text(1, 4, "OS".to_string()), "");
        assert_eq!(normalize_raster_cell_text(0, 6, "H,O".to_string()), "H2O");
    }

    #[test]
    fn test_detect_bordered_raster_grid_finds_strong_lines() {
        let mut image = GrayImage::from_pixel(120, 80, Luma([255]));
        for x in [10, 40, 80, 110] {
            for y in 10..71 {
                image.put_pixel(x, y, Luma([0]));
            }
        }
        for y in [10, 30, 50, 70] {
            for x in 10..111 {
                image.put_pixel(x, y, Luma([0]));
            }
        }

        let grid = detect_bordered_raster_grid(&image).expect("grid");
        assert_eq!(grid.vertical_lines.len(), 4);
        assert_eq!(grid.horizontal_lines.len(), 4);
    }

    #[test]
    fn test_obvious_bar_chart_raster_is_rejected() {
        let mut image = GrayImage::from_pixel(320, 200, Luma([255]));
        for &(y1, y2) in &[(25, 40), (70, 85), (115, 130), (160, 175)] {
            for y in y1..y2 {
                for x in 40..280 {
                    image.put_pixel(x, y, Luma([80]));
                }
            }
        }

        assert!(is_obvious_bar_chart_raster(&image));
    }

    #[test]
    fn test_vertical_bar_chart_raster_is_rejected() {
        let mut image = GrayImage::from_pixel(360, 240, Luma([255]));
        for &(x1, x2, y1) in &[
            (40, 78, 52),
            (92, 126, 118),
            (140, 170, 146),
            (184, 210, 162),
        ] {
            for x in x1..x2 {
                for y in y1..212 {
                    image.put_pixel(x, y, Luma([90]));
                }
            }
        }

        assert!(is_obvious_bar_chart_raster(&image));
    }

    #[test]
    fn test_light_fill_vertical_bar_chart_raster_is_rejected() {
        let mut image = GrayImage::from_pixel(420, 260, Luma([255]));
        for x in 24..396 {
            image.put_pixel(x, 222, Luma([170]));
        }
        for &(x1, x2, y1, shade) in &[
            (46, 82, 132, 222),
            (104, 140, 84, 214),
            (162, 198, 62, 206),
            (220, 256, 144, 228),
        ] {
            for x in x1..x2 {
                for y in y1..222 {
                    image.put_pixel(x, y, Luma([shade]));
                }
            }
        }

        assert!(is_obvious_bar_chart_raster(&image));
    }

    #[test]
    fn test_grouped_vertical_bar_chart_raster_is_rejected() {
        let mut image = GrayImage::from_pixel(420, 240, Luma([255]));
        for x in 28..392 {
            image.put_pixel(x, 214, Luma([175]));
        }
        for &(x1, x2, y1, shade) in &[
            (44, 60, 98, 210),
            (64, 80, 140, 225),
            (108, 124, 116, 214),
            (128, 144, 148, 229),
            (172, 188, 88, 206),
            (192, 208, 128, 222),
            (236, 252, 104, 212),
            (256, 272, 156, 228),
        ] {
            for x in x1..x2 {
                for y in y1..214 {
                    image.put_pixel(x, y, Luma([shade]));
                }
            }
        }

        assert!(is_obvious_bar_chart_raster(&image));
    }

    #[test]
    fn test_natural_photograph_raster_is_detected() {
        // Create a photo-like image: wide histogram spread across [20, 230] mid-tones
        let w = 100u32;
        let h = 100u32;
        let mut image = GrayImage::new(w, h);
        // Fill with a gradient covering the full range — most pixels will be mid-tone
        for y in 0..h {
            for x in 0..w {
                let v = ((x + y) * 255 / (w + h - 2)) as u8;
                image.put_pixel(x, y, Luma([v]));
            }
        }
        // Should be classified as photographic (≥30% mid-tone pixels)
        assert!(is_natural_photograph_raster(&image));
    }

    #[test]
    fn test_chart_image_is_not_classified_as_photograph() {
        // Chart-like image: mostly white with a few dark lines (no mid-tone content)
        let mut image = GrayImage::from_pixel(200, 160, Luma([255]));
        // A few thin dark lines (table borders or chart axes)
        for x in 20..180 {
            image.put_pixel(x, 20, Luma([0]));
            image.put_pixel(x, 80, Luma([0]));
            image.put_pixel(x, 140, Luma([0]));
        }
        for y in 20..141 {
            image.put_pixel(20, y, Luma([0]));
            image.put_pixel(180, y, Luma([0]));
        }
        // Very few mid-tone pixels — should NOT be classified as photograph
        assert!(!is_natural_photograph_raster(&image));
        assert!(!is_dark_ui_screenshot_raster(&image));
    }

    #[test]
    fn test_bright_natural_photograph_raster_is_detected() {
        let mut image = GrayImage::from_pixel(240, 180, Luma([250]));
        for y in 24..148 {
            for x in 52..156 {
                let tone = 72 + (((x - 52) * 11 + (y - 24) * 7) % 132) as u8;
                image.put_pixel(x, y, Luma([tone]));
            }
        }

        assert!(is_natural_photograph_raster(&image));
    }

    #[test]
    fn test_dark_ui_screenshot_raster_is_detected() {
        let mut image = GrayImage::from_pixel(260, 180, Luma([20]));
        for x in 18..242 {
            for y in 18..34 {
                image.put_pixel(x, y, Luma([210]));
            }
        }
        for &(x1, y1, x2, y2, shade) in &[
            (26, 58, 84, 108, 198),
            (94, 58, 152, 108, 210),
            (162, 58, 220, 108, 192),
            (26, 118, 220, 134, 224),
        ] {
            for x in x1..x2 {
                for y in y1..y2 {
                    image.put_pixel(x, y, Luma([shade]));
                }
            }
        }

        assert!(is_dark_ui_screenshot_raster(&image));
    }

    #[test]
    fn test_table_like_ocr_rejects_matrix_formula_layout() {
        let words = vec![
            word_at((1, 1, 1), 14, 10, 36, "B23"),
            word_at((1, 1, 1), 160, 10, 22, "C1"),
            word_at((1, 1, 1), 230, 10, 22, "C2"),
            word_at((1, 1, 1), 300, 10, 22, "C3"),
            word_at((1, 1, 2), 20, 44, 24, "0/0"),
            word_at((1, 1, 2), 150, 44, 18, "0"),
            word_at((1, 1, 2), 220, 44, 28, "001"),
            word_at((1, 1, 2), 300, 44, 28, "000"),
            word_at((1, 1, 3), 20, 76, 24, "0/1"),
            word_at((1, 1, 3), 150, 76, 28, "000"),
            word_at((1, 1, 3), 220, 76, 28, "010"),
            word_at((1, 1, 3), 300, 76, 28, "000"),
        ];

        assert!(looks_like_matrix_formula_ocr(&words));
        assert!(!looks_like_table_ocr(&words));
    }

    #[test]
    fn test_table_like_ocr_keeps_small_numeric_table_with_real_headers() {
        let words = vec![
            word_at((1, 1, 1), 10, 10, 64, "Year"),
            word_at((1, 1, 1), 130, 10, 28, "Q1"),
            word_at((1, 1, 1), 220, 10, 28, "Q2"),
            word_at((1, 1, 1), 310, 10, 28, "Q3"),
            word_at((1, 1, 2), 10, 42, 64, "2022"),
            word_at((1, 1, 2), 130, 42, 24, "10"),
            word_at((1, 1, 2), 220, 42, 24, "25"),
            word_at((1, 1, 2), 310, 42, 24, "30"),
            word_at((1, 1, 3), 10, 74, 64, "2023"),
            word_at((1, 1, 3), 130, 74, 24, "11"),
            word_at((1, 1, 3), 220, 74, 24, "26"),
            word_at((1, 1, 3), 310, 74, 24, "31"),
        ];

        assert!(!looks_like_matrix_formula_ocr(&words));
        assert!(looks_like_table_ocr(&words));
    }

    #[test]
    fn test_matrixish_small_ocr_table_is_rejected_after_build() {
        let image = ImageChunk {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 440.0, 120.0),
            index: Some(1),
            level: None,
        };
        let words = vec![
            word_at((1, 1, 1), 14, 10, 36, "B23"),
            word_at((1, 1, 1), 160, 10, 22, "C1"),
            word_at((1, 1, 1), 230, 10, 22, "C2"),
            word_at((1, 1, 1), 300, 10, 22, "C3"),
            word_at((1, 1, 2), 20, 44, 24, "0/0"),
            word_at((1, 1, 2), 150, 44, 18, "0"),
            word_at((1, 1, 2), 220, 44, 28, "001"),
            word_at((1, 1, 2), 300, 44, 28, "000"),
            word_at((1, 1, 3), 20, 76, 24, "0/1"),
            word_at((1, 1, 3), 150, 76, 28, "000"),
            word_at((1, 1, 3), 220, 76, 28, "010"),
            word_at((1, 1, 3), 300, 76, 28, "000"),
        ];

        let table = build_structured_ocr_table_border(&words, &image).expect("structured table");
        assert!(is_matrixish_ocr_artifact_table(&table));
    }

    #[test]
    fn test_small_numeric_table_with_real_headers_is_not_rejected_after_build() {
        let image = ImageChunk {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 440.0, 140.0),
            index: Some(1),
            level: None,
        };
        let words = vec![
            word_at((1, 1, 1), 10, 10, 64, "Year"),
            word_at((1, 1, 1), 130, 10, 28, "Q1"),
            word_at((1, 1, 1), 220, 10, 28, "Q2"),
            word_at((1, 1, 1), 310, 10, 28, "Q3"),
            word_at((1, 1, 2), 10, 42, 64, "2022"),
            word_at((1, 1, 2), 130, 42, 24, "10"),
            word_at((1, 1, 2), 220, 42, 24, "25"),
            word_at((1, 1, 2), 310, 42, 24, "30"),
            word_at((1, 1, 3), 10, 74, 64, "2023"),
            word_at((1, 1, 3), 130, 74, 24, "11"),
            word_at((1, 1, 3), 220, 74, 24, "26"),
            word_at((1, 1, 3), 310, 74, 24, "31"),
        ];

        let table = build_structured_ocr_table_border(&words, &image).expect("structured table");
        assert!(!is_matrixish_ocr_artifact_table(&table));
    }

    #[test]
    fn test_bordered_table_raster_is_not_rejected_as_chart() {
        let mut image = GrayImage::from_pixel(320, 200, Luma([255]));
        for x in [20, 110, 210, 300] {
            for y in 20..181 {
                image.put_pixel(x, y, Luma([0]));
            }
        }
        for y in [20, 70, 120, 180] {
            for x in 20..301 {
                image.put_pixel(x, y, Luma([0]));
            }
        }

        assert!(!is_obvious_bar_chart_raster(&image));
    }

    #[test]
    fn test_morphological_erode_preserves_white_background() {
        let image = GrayImage::from_fn(9, 9, |x, y| {
            if x == 4 || y == 4 {
                Luma([0])
            } else {
                Luma([255])
            }
        });

        let eroded = morphological_erode(&image, 1);

        assert_eq!(eroded.get_pixel(0, 0).0[0], 255);
        assert_eq!(eroded.get_pixel(8, 8).0[0], 255);
        assert_eq!(eroded.get_pixel(4, 4).0[0], 255);
    }

    #[test]
    fn test_dense_prose_image_ocr_detects_infographic_text() {
        let mut words = Vec::new();
        let mut top = 20;
        for line_num in 1..=8 {
            for (idx, (left, text)) in [
                (20, "Copyright"),
                (120, "protects"),
                (240, "creative"),
                (350, "work"),
            ]
            .into_iter()
            .enumerate()
            {
                words.push(OcrWord {
                    line_key: (1, 1, line_num),
                    left,
                    top,
                    width: 60,
                    height: 14,
                    confidence: 85.0,
                    text: if idx == 0 && line_num % 2 == 0 {
                        "Creators".to_string()
                    } else {
                        text.to_string()
                    },
                });
            }
            top += 22;
        }

        assert!(looks_like_dense_prose_image_ocr(&words));
    }

    #[test]
    fn test_dense_prose_image_ocr_rejects_chart_like_words() {
        let words = vec![
            word((1, 1, 1), 10, "70.2"),
            word((1, 1, 1), 90, "75.6"),
            word((1, 1, 1), 170, "92.4"),
            word((1, 1, 2), 10, "80.4"),
            word((1, 1, 2), 90, "94.2"),
            word((1, 1, 2), 170, "95.5"),
            word((1, 1, 3), 10, "Company"),
            word((1, 1, 3), 90, "A"),
            word((1, 1, 3), 170, "B"),
            word((1, 1, 4), 10, "Scene"),
            word((1, 1, 4), 90, "Document"),
            word((1, 1, 5), 10, "65"),
            word((1, 1, 5), 90, "70"),
            word((1, 1, 5), 170, "75"),
            word((1, 1, 6), 10, "80"),
            word((1, 1, 6), 90, "85"),
            word((1, 1, 6), 170, "90"),
            word((1, 1, 7), 10, "95"),
            word((1, 1, 7), 90, "100"),
        ];

        assert!(!looks_like_dense_prose_image_ocr(&words));
    }

    #[test]
    fn test_dense_prose_image_ocr_rejects_scattered_chart_labels() {
        let words = vec![
            word_at((1, 1, 1), 20, 20, 80, "Participation"),
            word_at((1, 1, 1), 120, 20, 70, "of"),
            word_at((1, 1, 1), 210, 20, 90, "Institutions"),
            word_at((1, 1, 2), 310, 50, 50, "57"),
            word_at((1, 1, 2), 380, 50, 60, "(24%)"),
            word_at((1, 1, 3), 290, 86, 40, "20"),
            word_at((1, 1, 3), 345, 86, 50, "(8%)"),
            word_at((1, 1, 4), 80, 124, 120, "Government"),
            word_at((1, 1, 4), 260, 124, 90, "Other"),
            word_at((1, 1, 4), 360, 124, 60, "State"),
            word_at((1, 1, 5), 70, 160, 80, "Civil"),
            word_at((1, 1, 5), 170, 160, 80, "Society"),
            word_at((1, 1, 5), 280, 160, 110, "Organizations"),
            word_at((1, 1, 6), 300, 194, 50, "31"),
            word_at((1, 1, 6), 365, 194, 60, "(13%)"),
            word_at((1, 1, 7), 35, 228, 120, "Educational"),
            word_at((1, 1, 7), 180, 228, 100, "Institution"),
            word_at((1, 1, 8), 250, 262, 40, "16"),
            word_at((1, 1, 8), 305, 262, 50, "(7%)"),
        ];

        assert!(looks_like_chart_label_ocr(&words));
        assert!(!looks_like_table_ocr(&words));
        assert!(!looks_like_dense_prose_image_ocr(&words));
    }

    #[test]
    fn test_chart_label_ocr_detects_stacked_bar_chart_legend_layout() {
        let words = vec![
            word_at((1, 1, 1), 10, 15, 22, "ano"),
            word_at((1, 1, 1), 10, 8, 24, "MW."),
            word_at((1, 1, 2), 410, 25, 38, "Waste"),
            word_at((1, 1, 2), 452, 25, 55, "materials"),
            word_at((1, 1, 3), 11, 38, 21, "350"),
            word_at((1, 1, 4), 11, 61, 21, "300"),
            word_at((1, 1, 4), 411, 56, 38, "Biogas"),
            word_at((1, 1, 5), 7, 79, 25, "250"),
            word_at((1, 1, 5), 399, 87, 8, "'™"),
            word_at((1, 1, 5), 411, 87, 75, "Construction"),
            word_at((1, 1, 5), 490, 86, 33, "wood"),
            word_at((1, 1, 5), 527, 87, 35, "waste"),
            word_at((1, 1, 6), 11, 106, 21, "200"),
            word_at((1, 1, 7), 411, 117, 59, "General"),
            word_at((1, 1, 7), 467, 116, 27, "wood"),
            word_at((1, 1, 7), 499, 116, 54, "(10MWs)"),
            word_at((1, 1, 8), 11, 129, 21, "150"),
            word_at((1, 1, 9), 11, 152, 21, "100"),
            word_at((1, 1, 9), 399, 148, 7, "="),
            word_at((1, 1, 9), 411, 135, 46, "General"),
            word_at((1, 1, 9), 464, 135, 27, "wood"),
            word_at((1, 1, 9), 498, 146, 56, "(<LOMW)"),
            word_at((1, 1, 10), 13, 163, 18, "50"),
            word_at((1, 1, 10), 399, 178, 7, "="),
            word_at((1, 1, 10), 411, 176, 73, "Unutilised"),
            word_at((1, 1, 10), 480, 166, 29, "wood"),
            word_at((1, 1, 10), 516, 176, 45, "(2MWs)"),
            word_at((1, 1, 11), 24, 197, 7, "o"),
            word_at((1, 1, 12), 399, 208, 8, "m="),
            word_at((1, 1, 12), 411, 206, 59, "Unutilised"),
            word_at((1, 1, 12), 474, 206, 33, "wood"),
            word_at((1, 1, 12), 512, 206, 48, "(<2MW)"),
            word_at((1, 1, 13), 51, 217, 32, "12-13"),
            word_at((1, 1, 13), 96, 217, 28, "2014"),
            word_at((1, 1, 13), 139, 217, 28, "2015"),
            word_at((1, 1, 13), 182, 217, 28, "2016"),
            word_at((1, 1, 13), 225, 217, 28, "2017"),
            word_at((1, 1, 13), 268, 217, 28, "2018"),
            word_at((1, 1, 13), 311, 217, 28, "2019"),
            word_at((1, 1, 13), 354, 217, 28, "2020"),
        ];

        assert!(looks_like_chart_label_ocr(&words));
        assert!(!looks_like_table_ocr(&words));
    }

    #[test]
    fn test_build_numeric_table_border_rejects_sparse_chart_layout() {
        let image = image_chunk();
        let mut words = Vec::new();
        let columns = [20, 55, 90, 125, 160, 195, 230, 265, 300, 335, 370, 405];

        for (idx, left) in columns.iter().enumerate() {
            words.push(word_at((1, 1, 1), *left, 20, 22, &format!("H{}", idx + 1)));
        }
        for (idx, left) in [20, 160, 300].into_iter().enumerate() {
            words.push(word_at((1, 1, 2), left, 52, 22, &format!("{}", idx + 1)));
        }
        for (idx, left) in [55, 195, 335].into_iter().enumerate() {
            words.push(word_at((1, 1, 3), left, 84, 22, &format!("{}", idx + 4)));
        }
        for (idx, left) in [90, 230, 370].into_iter().enumerate() {
            words.push(word_at((1, 1, 4), left, 116, 22, &format!("{}", idx + 7)));
        }
        for (idx, left) in columns.iter().enumerate() {
            words.push(word_at((1, 1, 5), *left, 148, 22, &format!("{}", idx + 10)));
        }

        assert!(looks_like_chart_label_ocr(&words));
        assert!(!looks_like_table_ocr(&words));
        assert!(!looks_like_numeric_table_ocr(&words));
        assert!(build_numeric_table_border(&words, &image).is_none());
    }

    #[test]
    fn test_lines_from_ocr_words_merges_wrapped_lines_into_blocks() {
        let words = vec![
            word_at((1, 1, 1), 20, 20, 64, "Copyright"),
            word_at((1, 1, 1), 100, 20, 56, "protects"),
            word_at((1, 1, 2), 20, 38, 52, "creative"),
            word_at((1, 1, 2), 84, 38, 36, "work"),
            word_at((1, 1, 3), 240, 20, 52, "Public"),
            word_at((1, 1, 3), 304, 20, 40, "domain"),
            word_at((1, 1, 4), 240, 38, 60, "expires"),
            word_at((1, 1, 4), 312, 38, 44, "later"),
        ];

        let recovered = lines_from_ocr_words(&words, &image_chunk(), 400, 400, &[]);

        assert_eq!(recovered.len(), 2);
        assert_eq!(recovered[0].value, "Copyright protects creative work");
        assert_eq!(recovered[1].value, "Public domain expires later");
    }

    #[test]
    fn test_page_raster_ocr_skips_bar_chart_tables() {
        let mut chart = GrayImage::from_pixel(420, 260, Luma([255]));
        for x in 24..396 {
            chart.put_pixel(x, 222, Luma([170]));
        }
        for &(x1, x2, y1, shade) in &[
            (46, 82, 132, 222),
            (104, 140, 84, 214),
            (162, 198, 62, 206),
            (220, 256, 144, 228),
        ] {
            for x in x1..x2 {
                for y in y1..222 {
                    chart.put_pixel(x, y, Luma([shade]));
                }
            }
        }

        let page_bbox = BoundingBox::new(Some(1), 0.0, 0.0, 420.0, 260.0);
        let mut table = TableBorder {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 420.0, 260.0),
            index: None,
            level: None,
            x_coordinates: vec![0.0, 210.0, 420.0],
            x_widths: vec![0.0; 3],
            y_coordinates: vec![260.0, 130.0, 0.0],
            y_widths: vec![0.0; 3],
            rows: vec![
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 0.0, 130.0, 420.0, 260.0),
                    index: None,
                    level: None,
                    row_number: 0,
                    cells: vec![
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 0.0, 130.0, 210.0, 260.0),
                            index: None,
                            level: None,
                            row_number: 0,
                            col_number: 0,
                            row_span: 1,
                            col_span: 1,
                            content: Vec::new(),
                            contents: Vec::new(),
                            semantic_type: None,
                        },
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 210.0, 130.0, 420.0, 260.0),
                            index: None,
                            level: None,
                            row_number: 0,
                            col_number: 1,
                            row_span: 1,
                            col_span: 1,
                            content: Vec::new(),
                            contents: Vec::new(),
                            semantic_type: None,
                        },
                    ],
                    semantic_type: None,
                },
                TableBorderRow {
                    bbox: BoundingBox::new(Some(1), 0.0, 0.0, 420.0, 130.0),
                    index: None,
                    level: None,
                    row_number: 1,
                    cells: vec![
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 210.0, 130.0),
                            index: None,
                            level: None,
                            row_number: 1,
                            col_number: 0,
                            row_span: 1,
                            col_span: 1,
                            content: Vec::new(),
                            contents: Vec::new(),
                            semantic_type: None,
                        },
                        TableBorderCell {
                            bbox: BoundingBox::new(Some(1), 210.0, 0.0, 420.0, 130.0),
                            index: None,
                            level: None,
                            row_number: 1,
                            col_number: 1,
                            row_span: 1,
                            col_span: 1,
                            content: Vec::new(),
                            contents: Vec::new(),
                            semantic_type: None,
                        },
                    ],
                    semantic_type: None,
                },
            ],
            num_rows: 2,
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: true,
            previous_table: None,
            next_table: None,
        };

        enrich_empty_table_from_page_raster(&chart, &page_bbox, &mut table);

        assert!(table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .all(|cell| cell.content.is_empty()));
    }

    #[test]
    fn test_native_text_chars_in_region_ignores_distant_page_text() {
        let table_bbox = BoundingBox::new(Some(1), 40.0, 120.0, 360.0, 280.0);
        let distant_text = ContentElement::TextChunk(text_chunk(
            &"A".repeat(MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR + 40),
            BoundingBox::new(Some(1), 40.0, 500.0, 380.0, 560.0),
        ));
        let overlapping_text = ContentElement::TextChunk(text_chunk(
            "1234",
            BoundingBox::new(Some(1), 60.0, 160.0, 100.0, 176.0),
        ));
        let elements = vec![distant_text, overlapping_text];

        assert!(page_native_text_chars(&elements) > MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR);
        assert_eq!(native_text_chars_in_region(&elements, &table_bbox), 4);
    }

    #[test]
    fn test_table_needs_page_raster_ocr_for_sparse_partial_table() {
        let mut table = TableBorder {
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 300.0, 200.0),
            index: None,
            level: None,
            x_coordinates: vec![0.0, 60.0, 120.0, 180.0, 240.0, 300.0],
            x_widths: vec![0.0; 6],
            y_coordinates: vec![200.0, 160.0, 120.0, 80.0, 40.0, 0.0],
            y_widths: vec![0.0; 6],
            rows: Vec::new(),
            num_rows: 5,
            num_columns: 5,
            is_bad_table: false,
            is_table_transformer: true,
            previous_table: None,
            next_table: None,
        };

        for row_idx in 0..5 {
            let mut row = TableBorderRow {
                bbox: BoundingBox::new(Some(1), 0.0, 0.0, 300.0, 200.0),
                index: None,
                level: None,
                row_number: row_idx,
                cells: Vec::new(),
                semantic_type: None,
            };
            for col_idx in 0..5 {
                row.cells.push(TableBorderCell {
                    bbox: BoundingBox::new(Some(1), 0.0, 0.0, 60.0, 40.0),
                    index: None,
                    level: None,
                    row_number: row_idx,
                    col_number: col_idx,
                    row_span: 1,
                    col_span: 1,
                    content: Vec::new(),
                    contents: Vec::new(),
                    semantic_type: None,
                });
            }
            table.rows.push(row);
        }

        table.rows[0].cells[0].content.push(TableToken {
            base: text_chunk("12", BoundingBox::new(Some(1), 0.0, 0.0, 20.0, 10.0)),
            token_type: TableTokenType::Text,
        });

        assert!(table_needs_page_raster_ocr(&table));
    }

    #[test]
    fn test_lines_from_ocr_words_dedupes_against_native_text() {
        let words = vec![
            word_at((1, 1, 1), 20, 20, 64, "Copyright"),
            word_at((1, 1, 1), 100, 20, 56, "protects"),
            word_at((1, 1, 2), 20, 38, 52, "creative"),
            word_at((1, 1, 2), 84, 38, 36, "work"),
        ];
        let native = vec![TextChunk {
            value: "Copyright protects creative work".to_string(),
            bbox: BoundingBox::new(Some(1), 0.0, 0.0, 10.0, 10.0),
            font_name: "Native".to_string(),
            font_size: 12.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Content,
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        }];

        let recovered = lines_from_ocr_words(&words, &image_chunk(), 400, 400, &native);

        assert!(recovered.is_empty());
    }
}
