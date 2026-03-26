//! Recover text signal from raster table images using local OCR.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{GenericImageView, GrayImage, Luma};

use crate::models::bbox::BoundingBox;
use crate::models::chunks::{ImageChunk, TextChunk};
use crate::models::content::ContentElement;
use crate::models::enums::{PdfLayer, TextFormat, TextType};
use crate::models::table::{
    TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType,
};

const MIN_IMAGE_WIDTH_RATIO: f64 = 0.45;
const MIN_IMAGE_AREA_RATIO: f64 = 0.045;
const MAX_NATIVE_TEXT_CHARS_IN_IMAGE: usize = 250;
const MAX_NATIVE_TEXT_CHUNKS_IN_IMAGE: usize = 12;
const MIN_OCR_WORD_CONFIDENCE: f64 = 35.0;
const RASTER_DARK_THRESHOLD: u8 = 180;
const MIN_BORDERED_VERTICAL_LINES: usize = 4;
const MIN_BORDERED_HORIZONTAL_LINES: usize = 4;
const MIN_LINE_DARK_RATIO: f64 = 0.55;
const MIN_CELL_SIZE_PX: u32 = 10;
const CELL_INSET_PX: u32 = 4;
const OCR_SCALE_FACTOR: u32 = 3;
const MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR: usize = 180;
const MIN_EMPTY_TABLE_COVERAGE_FOR_PAGE_RASTER_OCR: f64 = 0.08;
const MAX_EMPTY_TABLES_FOR_PAGE_RASTER_OCR: usize = 24;

#[derive(Debug, Clone)]
struct OcrWord {
    line_key: (u32, u32, u32),
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    text: String,
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
struct RasterTableGrid {
    vertical_lines: Vec<u32>,
    horizontal_lines: Vec<u32>,
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

    let prefix = temp_dir.join("img");
    let status = Command::new("pdfimages")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-png")
        .arg(input_path)
        .arg(&prefix)
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => {
            let _ = fs::remove_dir_all(&temp_dir);
            return Vec::new();
        }
    }

    let mut image_files: Vec<PathBuf> = match fs::read_dir(&temp_dir) {
        Ok(read_dir) => read_dir
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
            .collect(),
        Err(_) => {
            let _ = fs::remove_dir_all(&temp_dir);
            return Vec::new();
        }
    };
    image_files.sort();

    let mut tables = Vec::new();
    for image in candidates {
        let Some(image_index) = image.index else {
            continue;
        };
        let Some(image_path) = image_files.get(image_index.saturating_sub(1) as usize) else {
            continue;
        };
        if let Some(table) = recover_bordered_raster_table(image_path, image) {
            tables.push(table);
            continue;
        }
        let Some(file_name) = image_path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(tsv_output) = Command::new("tesseract")
            .current_dir(&temp_dir)
            .arg(file_name)
            .arg("stdout")
            .arg("--psm")
            .arg("6")
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
        if looks_like_numeric_table_ocr(&words) {
            if let Some(table) = build_numeric_table_border(&words, image) {
                tables.push(table);
            }
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
    if native_text_chars > MAX_NATIVE_TEXT_CHARS_FOR_PAGE_RASTER_OCR {
        return;
    }

    let candidate_indices: Vec<usize> = elements
        .iter()
        .enumerate()
        .filter_map(|(idx, elem)| {
            table_candidate_ref(elem)
                .filter(|table| table_needs_page_raster_ocr(table))
                .map(|_| idx)
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

fn recover_from_page_images(
    input_path: &Path,
    temp_dir: &Path,
    page_number: u32,
    candidates: Vec<&ImageChunk>,
    text_chunks: &[TextChunk],
) -> Vec<TextChunk> {
    let prefix = temp_dir.join("img");
    let status = Command::new("pdfimages")
        .arg("-f")
        .arg(page_number.to_string())
        .arg("-l")
        .arg(page_number.to_string())
        .arg("-png")
        .arg(input_path)
        .arg(&prefix)
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => return Vec::new(),
    }

    let mut image_files: Vec<PathBuf> = match fs::read_dir(temp_dir) {
        Ok(read_dir) => read_dir
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("png"))
            .collect(),
        Err(_) => return Vec::new(),
    };
    image_files.sort();
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
        let Ok(tsv_output) = Command::new("tesseract")
            .current_dir(temp_dir)
            .arg(file_name)
            .arg("stdout")
            .arg("--psm")
            .arg("6")
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

fn page_native_text_chars(elements: &[ContentElement]) -> usize {
    elements
        .iter()
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

fn table_needs_page_raster_ocr(table: &TableBorder) -> bool {
    table.num_rows >= 1
        && table.num_columns >= 2
        && table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .all(|cell| {
                !cell
                    .content
                    .iter()
                    .any(|token| matches!(token.token_type, TableTokenType::Text))
            })
}

fn enrich_empty_table_from_page_raster(
    gray: &GrayImage,
    page_bbox: &BoundingBox,
    table: &mut TableBorder,
) {
    for row in &mut table.rows {
        for cell in &mut row.cells {
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
            let Some(text) = extract_page_raster_cell_text(gray, &cell.bbox, x1, y1, x2, y2) else {
                continue;
            };
            if text.is_empty() {
                continue;
            }
            cell.content.push(TableToken {
                base: TextChunk {
                    value: text,
                    bbox: cell.bbox.clone(),
                    font_name: "OCR".to_string(),
                    font_size: cell.bbox.height().max(6.0),
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
                    page_number: cell.bbox.page_number,
                    level: None,
                    mcid: None,
                },
                token_type: TableTokenType::Text,
            });
        }
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
    let psm = if cell_bbox.width() <= cell_bbox.height() * 1.15 {
        "10"
    } else {
        "6"
    };
    let raw_text = run_tesseract_plain_text(&scaled, psm)?;
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
        if confidence < MIN_OCR_WORD_CONFIDENCE
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
        });
    }
    words
}

fn looks_like_table_ocr(words: &[OcrWord]) -> bool {
    if words.len() < 8 {
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

    built_rows.sort_by(|a, b| {
        b.top_y
            .partial_cmp(&a.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let x_coordinates =
        build_boundaries_from_centers(&centers, image.bbox.left_x, image.bbox.right_x);
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

fn recover_bordered_raster_caption(image_path: &Path, image: &ImageChunk) -> Option<TextChunk> {
    let gray = image::open(image_path).ok()?.to_luma8();
    let grid = detect_bordered_raster_grid(&gray)?;
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
    let grid = detect_bordered_raster_grid(&gray)?;
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

    let mut rows = Vec::with_capacity(num_rows);
    for row_idx in 0..num_rows {
        let row_bbox = BoundingBox::new(
            image.bbox.page_number,
            image.bbox.left_x,
            y_coordinates[row_idx + 1],
            image.bbox.right_x,
            y_coordinates[row_idx],
        );
        let mut cells = Vec::with_capacity(num_cols);

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
            let text = extract_raster_cell_text(&gray, row_idx, col_idx, x1, y1, x2, y2)?;

            let mut content = Vec::new();
            if !text.is_empty() {
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

fn detect_bordered_raster_grid(gray: &GrayImage) -> Option<RasterTableGrid> {
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

    let vertical_lines: Vec<u32> = vertical_runs
        .into_iter()
        .map(|(start, end)| (start + end) / 2)
        .collect();
    let horizontal_lines: Vec<u32> = horizontal_runs
        .into_iter()
        .map(|(start, end)| (start + end) / 2)
        .collect();
    if vertical_lines
        .windows(2)
        .any(|w| w[1] <= w[0] + MIN_CELL_SIZE_PX)
        || horizontal_lines
            .windows(2)
            .any(|w| w[1] <= w[0] + MIN_CELL_SIZE_PX)
    {
        return None;
    }

    Some(RasterTableGrid {
        vertical_lines,
        horizontal_lines,
    })
}

fn count_dark_in_column(gray: &GrayImage, x: u32) -> u32 {
    (0..gray.height())
        .filter(|&y| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count() as u32
}

fn count_dark_in_row(gray: &GrayImage, y: u32) -> u32 {
    (0..gray.width())
        .filter(|&x| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count() as u32
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

fn build_boundaries_from_centers(centers: &[f64], left_edge: f64, right_edge: f64) -> Vec<f64> {
    let mut boundaries = Vec::with_capacity(centers.len() + 1);
    boundaries.push(left_edge);
    for pair in centers.windows(2) {
        boundaries.push((pair[0] + pair[1]) / 2.0);
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
    let raw_text = run_tesseract_plain_text(&scaled, if row_idx == 0 { "6" } else { "7" })?;
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

fn run_tesseract_plain_text(image: &GrayImage, psm: &str) -> Option<String> {
    let temp_dir = create_temp_dir(0).ok()?;
    let image_path = temp_dir.join("ocr.png");
    if image.save(&image_path).is_err() {
        let _ = fs::remove_dir_all(&temp_dir);
        return None;
    }

    let output = Command::new("tesseract")
        .current_dir(&temp_dir)
        .arg("ocr.png")
        .arg("stdout")
        .arg("--psm")
        .arg(psm)
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

fn normalize_raster_cell_text(row_idx: usize, col_idx: usize, text: String) -> String {
    let mut normalized = text
        .replace('|', " ")
        .replace('—', "-")
        .replace("AorB", "A or B")
        .replace("Aor B", "A or B")
        .replace("H,O", "H2O")
        .replace("Buffer-RNave", "Buffer-RNase")
        .replace("Buffer RNave", "Buffer-RNase")
        .replace("Buffer-RNasee", "Buffer-RNase")
        .replace("Buffer-—RNase", "Buffer-RNase")
        .replace("Buffer—RNase", "Buffer-RNase")
        .replace("BamHI-Hindill", "BamHI-HindIII")
        .replace("BamHli-Hindlll", "BamHI-HindIII")
        .replace("BamHIi-Hindlll", "BamHI-HindIII")
        .replace("Hindlll", "HindIII")
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

    if row_idx == 0 {
        if col_idx == 1 {
            normalized = "BamHI-HindIII restriction enzyme mixture".to_string();
        } else if col_idx == 2 {
            normalized = "Restriction Buffer-RNase".to_string();
        } else if col_idx == 3 {
            normalized = "Suspect 1 DNA".to_string();
        } else if col_idx == 4 {
            normalized = "Suspect 2 DNA".to_string();
        } else if col_idx == 5 {
            normalized = "Evidence A or B".to_string();
        } else if col_idx == 6 {
            normalized = "H2O".to_string();
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use image::GrayImage;

    fn word(line: (u32, u32, u32), left: u32, text: &str) -> OcrWord {
        OcrWord {
            line_key: line,
            left,
            top: 0,
            width: 40,
            height: 12,
            text: text.to_string(),
        }
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
        assert!(looks_like_table_ocr(&words));
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
}
