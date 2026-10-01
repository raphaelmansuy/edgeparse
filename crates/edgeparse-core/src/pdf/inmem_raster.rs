//! In-memory raster table recovery (WASM-safe).
//!
//! First principles: PDF Image XObjects are the pixel source (ISO 32000).
//! Ruled tables are recoverable from projection of dark ink — no Poppler /
//! Tesseract *process* for page render. Cell text uses [`crate::pdf::ocr`].

use lopdf::Document;

use crate::models::bbox::BoundingBox;
use crate::models::chunks::{ImageChunk, TextChunk};
use crate::models::enums::{PdfLayer, TextFormat, TextType};
use crate::models::table::{
    TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType,
};
use crate::pdf::image_extractor::extract_image_data;

#[cfg(feature = "image")]
use image::GrayImage;

const MAX_NATIVE_TEXT_CHARS_IN_IMAGE: usize = 250;
const MIN_LINE_DARK_RATIO: f64 = 0.28;
const MIN_TRUE_GRID_LINE_CONTINUITY: f64 = 0.60;
const MIN_BORDERED_VERTICAL_LINES: usize = 3;
const MIN_BORDERED_HORIZONTAL_LINES: usize = 3;
const MIN_BORDERED_INKED_CELL_RATIO: f64 = 0.18;
const MIN_BORDERED_ROWS_WITH_INK: usize = 2;
const RASTER_DARK_THRESHOLD: u8 = 180;

/// Recover bordered tables from embedded Image XObjects on a page.
pub fn recover_embedded_raster_tables(
    doc: &Document,
    page_id: lopdf::ObjectId,
    image_chunks: &[ImageChunk],
    text_chunks: &[TextChunk],
) -> Vec<TableBorder> {
    #[cfg(not(feature = "image"))]
    {
        let _ = (doc, page_id, image_chunks, text_chunks);
        Vec::new()
    }
    #[cfg(feature = "image")]
    {
        recover_embedded_raster_tables_image(doc, page_id, image_chunks, text_chunks)
    }
}

#[cfg(feature = "image")]
fn recover_embedded_raster_tables_image(
    doc: &Document,
    page_id: lopdf::ObjectId,
    image_chunks: &[ImageChunk],
    text_chunks: &[TextChunk],
) -> Vec<TableBorder> {
    let mut out = Vec::new();
    for image in image_chunks {
        if !is_table_image_candidate(image, text_chunks) {
            continue;
        }
        let Some(raw_index) = image.index else {
            continue;
        };
        // chunk_parser / extract_image_data use 1-based XObject order.
        let index = raw_index.max(1);
        let Ok(Some(extracted)) = extract_image_data(doc, page_id, index) else {
            continue;
        };
        if let Some(table) = table_from_extracted(
            &extracted.data,
            &extracted.filter,
            extracted.width,
            extracted.height,
            image,
            text_chunks,
        ) {
            if bordered_table_is_plausible(&table) {
                out.push(table);
            }
        }
    }
    out
}

#[cfg(feature = "image")]
fn is_table_image_candidate(image: &ImageChunk, text_chunks: &[TextChunk]) -> bool {
    let w = image.bbox.width();
    let h = image.bbox.height();
    if w < 80.0 || h < 40.0 {
        return false;
    }
    let native_chars: usize = text_chunks
        .iter()
        .filter(|t| image.bbox.intersection_percent(&t.bbox) >= 0.7)
        .map(|t| t.value.chars().filter(|c| !c.is_whitespace()).count())
        .sum();
    native_chars <= MAX_NATIVE_TEXT_CHARS_IN_IMAGE
}

/// Prepared OCR candidate for the two-phase (plan → OCR → finish) path.
///
/// Emitted only when OCR can change the output (table-like Image XObject that
/// is not an obvious bar chart). Missing OCR words at finish time degrades to
/// PDF-text cells when a bordered grid is already known.
#[cfg(feature = "image")]
#[derive(Debug, Clone)]
pub struct RasterCandidate {
    /// Stable id within a session (0-based).
    pub id: u32,
    /// 1-based PDF page number.
    pub page: u32,
    /// Grayscale pixels (row-major).
    pub gray: Vec<u8>,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// FNV-1a hash of gray pixels (hex), for OCR result caching.
    pub hash: String,
    /// Image placement in PDF user space.
    pub image_bbox: BoundingBox,
    /// Overlapping native text (PDF fallback when OCR is absent).
    pub text_chunks: Vec<TextChunk>,
    /// Precomputed ruled grid when ink lines are strong enough.
    bordered_grid: Option<Grid>,
}

#[cfg(feature = "image")]
impl RasterCandidate {
    /// Metadata for JS without copying gray pixels.
    pub fn meta(&self) -> RasterCandidateMeta {
        RasterCandidateMeta {
            id: self.id,
            page: self.page,
            width: self.width,
            height: self.height,
            hash: self.hash.clone(),
        }
    }
}

/// Serializable candidate metadata (no pixel buffer).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RasterCandidateMeta {
    /// Session-local id.
    pub id: u32,
    /// 1-based page.
    pub page: u32,
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// Content hash for cache keys.
    pub hash: String,
}

/// Collect OCR candidates from Image XObjects (no OCR yet).
#[cfg(feature = "image")]
pub fn collect_raster_candidates(
    doc: &Document,
    page_id: lopdf::ObjectId,
    image_chunks: &[ImageChunk],
    text_chunks: &[TextChunk],
    next_id: &mut u32,
) -> Vec<RasterCandidate> {
    let mut out = Vec::new();
    for image in image_chunks {
        if !is_table_image_candidate(image, text_chunks) {
            continue;
        }
        let Some(raw_index) = image.index else {
            continue;
        };
        let index = raw_index.max(1);
        let Ok(Some(extracted)) = extract_image_data(doc, page_id, index) else {
            continue;
        };
        if let Some(cand) = prepare_candidate(
            *next_id,
            &extracted.data,
            &extracted.filter,
            extracted.width,
            extracted.height,
            image,
            text_chunks,
        ) {
            *next_id = next_id.saturating_add(1);
            out.push(cand);
        }
    }
    out
}

#[cfg(not(feature = "image"))]
/// Stub when the `image` feature is off.
pub fn collect_raster_candidates(
    _doc: &Document,
    _page_id: lopdf::ObjectId,
    _image_chunks: &[ImageChunk],
    _text_chunks: &[TextChunk],
    _next_id: &mut u32,
) -> Vec<()> {
    Vec::new()
}

/// Decode + pre-filter a single Image XObject into an OCR candidate.
#[cfg(feature = "image")]
pub fn prepare_candidate(
    id: u32,
    data: &[u8],
    filter: &str,
    width: u32,
    height: u32,
    image: &ImageChunk,
    text_chunks: &[TextChunk],
) -> Option<RasterCandidate> {
    let gray = decode_to_gray(data, filter, width, height)?;
    if is_obvious_bar_chart(&gray) {
        return None;
    }
    let bordered = detect_bordered_grid(&gray).filter(|g| grid_has_cell_ink(&gray, g));
    // Emit even without a ruled grid: host OCR word centers can seed the lattice
    // (docs like 122 / BamHI). Cheap size gate already applied upstream.
    let hash = fnv1a_hex(gray.as_raw());
    let overlapping: Vec<TextChunk> = text_chunks
        .iter()
        .filter(|t| image.bbox.intersection_percent(&t.bbox) >= 0.7)
        .cloned()
        .collect();
    let (gw, gh) = (gray.width(), gray.height());
    Some(RasterCandidate {
        id,
        page: image.bbox.page_number?,
        gray: gray.into_raw(),
        width: gw,
        height: gh,
        hash,
        image_bbox: image.bbox.clone(),
        text_chunks: overlapping,
        bordered_grid: bordered,
    })
}

/// Build a [`TableBorder`] from a prepared candidate + optional OCR words.
#[cfg(feature = "image")]
pub fn build_table_from_candidate(
    candidate: &RasterCandidate,
    words: &[crate::pdf::ocr::OcrWord],
) -> Option<TableBorder> {
    let gray = GrayImage::from_raw(candidate.width, candidate.height, candidate.gray.clone())?;
    let mut words = words.to_vec();
    let mut grid = candidate
        .bordered_grid
        .clone()
        .or_else(|| detect_ocr_word_grid(&words, candidate.width, candidate.height));
    if grid.is_none() {
        if let Some(ocrs_words) = recognize_words_ocrs_only(&gray) {
            if let Some(g) = candidate
                .bordered_grid
                .clone()
                .or_else(|| detect_ocr_word_grid(&ocrs_words, candidate.width, candidate.height))
            {
                words = ocrs_words;
                grid = Some(g);
            }
        }
    }
    let grid = grid?;
    build_table_cells(
        &grid,
        &gray,
        &candidate.image_bbox,
        &candidate.text_chunks,
        &words,
    )
}

#[cfg(feature = "image")]
fn table_from_extracted(
    data: &[u8],
    filter: &str,
    width: u32,
    height: u32,
    image: &ImageChunk,
    text_chunks: &[TextChunk],
) -> Option<TableBorder> {
    let candidate = prepare_candidate(0, data, filter, width, height, image, text_chunks)?;
    let gray = GrayImage::from_raw(candidate.width, candidate.height, candidate.gray.clone())?;
    let mut words = recognize_words(&gray);
    let mut grid = candidate
        .bordered_grid
        .clone()
        .or_else(|| detect_ocr_word_grid(&words, gray.width(), gray.height()));
    // Host engines (PP-OCR) may return few line-level boxes that cannot seed
    // a column lattice; retry with in-process ocrs when available.
    if grid.is_none() {
        if let Some(ocrs_words) = recognize_words_ocrs_only(&gray) {
            if let Some(g) = candidate
                .bordered_grid
                .clone()
                .or_else(|| detect_ocr_word_grid(&ocrs_words, gray.width(), gray.height()))
            {
                words = ocrs_words;
                grid = Some(g);
            }
        }
    }
    let grid = grid?;
    build_table_cells(
        &grid,
        &gray,
        &candidate.image_bbox,
        &candidate.text_chunks,
        &words,
    )
}

#[cfg(feature = "image")]
fn build_table_cells(
    grid: &Grid,
    gray: &GrayImage,
    image_bbox: &BoundingBox,
    text_chunks: &[TextChunk],
    words: &[crate::pdf::ocr::OcrWord],
) -> Option<TableBorder> {
    let num_cols = grid.v.len().checked_sub(1)?;
    let num_rows = grid.h.len().checked_sub(1)?;
    if num_cols < 2 || num_rows < 2 {
        return None;
    }

    let use_ocr = !words.is_empty();

    let x_coords = map_boundaries(&grid.v, image_bbox.left_x, image_bbox.right_x, gray.width())?;
    let y_coords = map_boundaries_desc(
        &grid.h,
        image_bbox.bottom_y,
        image_bbox.top_y,
        gray.height(),
    )?;

    let mut rows = Vec::with_capacity(num_rows);
    let mut non_empty = 0usize;
    for row_idx in 0..num_rows {
        let mut cells = Vec::with_capacity(num_cols);
        for col_idx in 0..num_cols {
            let x1 = grid.v[col_idx];
            let x2 = grid.v[col_idx + 1];
            let y1 = grid.h[row_idx];
            let y2 = grid.h[row_idx + 1];
            let cell_bbox = BoundingBox::new(
                image_bbox.page_number,
                x_coords[col_idx],
                y_coords[row_idx + 1],
                x_coords[col_idx + 1],
                y_coords[row_idx],
            );
            let (text, font_name) = if use_ocr {
                (words_in_cell(words, x1, y1, x2, y2), "OCR")
            } else {
                (text_chunks_in_cell(text_chunks, &cell_bbox), "PDF")
            };
            let mut content = Vec::new();
            if !text.is_empty() {
                non_empty += 1;
                content.push(TableToken {
                    base: TextChunk {
                        value: text,
                        bbox: cell_bbox.clone(),
                        font_name: font_name.into(),
                        font_size: (cell_bbox.height() * 0.55).max(6.0),
                        font_weight: if row_idx == 0 { 700.0 } else { 400.0 },
                        italic_angle: 0.0,
                        font_color: "#000000".into(),
                        contrast_ratio: 21.0,
                        symbol_ends: Vec::new(),
                        text_format: TextFormat::Normal,
                        text_type: TextType::Regular,
                        pdf_layer: PdfLayer::Content,
                        ocg_visible: true,
                        index: None,
                        page_number: image_bbox.page_number,
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
        let row_bbox = BoundingBox::new(
            image_bbox.page_number,
            image_bbox.left_x,
            y_coords[row_idx + 1],
            image_bbox.right_x,
            y_coords[row_idx],
        );
        rows.push(TableBorderRow {
            bbox: row_bbox,
            index: None,
            level: None,
            row_number: row_idx,
            cells,
            semantic_type: None,
        });
    }
    if non_empty == 0 {
        return None;
    }

    let table_bbox = BoundingBox::new(
        image_bbox.page_number,
        *x_coords.first()?,
        *y_coords.last()?,
        *x_coords.last()?,
        *y_coords.first()?,
    );
    Some(TableBorder {
        bbox: table_bbox,
        index: None,
        level: None,
        x_coordinates: x_coords.clone(),
        x_widths: vec![0.0; x_coords.len()],
        y_coordinates: y_coords.clone(),
        y_widths: vec![0.0; y_coords.len()],
        rows,
        num_rows,
        num_columns: num_cols,
        is_bad_table: false,
        is_table_transformer: true,
        previous_table: None,
        next_table: None,
    })
}

#[cfg(feature = "image")]
fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(feature = "image")]
fn recognize_words(gray: &GrayImage) -> Vec<crate::pdf::ocr::OcrWord> {
    #[cfg(any(not(target_arch = "wasm32"), feature = "ocr-ocrs"))]
    {
        use crate::pdf::ocr::{default_engine, OcrEngine};
        return OcrEngine::recognize(&*default_engine(), gray);
    }
    #[cfg(all(target_arch = "wasm32", not(feature = "ocr-ocrs")))]
    {
        let _ = gray;
        Vec::new()
    }
}

/// Force in-process ocrs (skip host), used when host boxes cannot form a lattice.
#[cfg(feature = "image")]
fn recognize_words_ocrs_only(gray: &GrayImage) -> Option<Vec<crate::pdf::ocr::OcrWord>> {
    #[cfg(feature = "ocr-ocrs")]
    {
        use crate::pdf::ocr::{OcrEngine, OcrsEngine};
        return OcrsEngine::try_from_env().map(|e| e.recognize(gray));
    }
    #[cfg(not(feature = "ocr-ocrs"))]
    {
        let _ = gray;
        None
    }
}

#[cfg(feature = "image")]
fn decode_to_gray(data: &[u8], filter: &str, width: u32, height: u32) -> Option<GrayImage> {
    if filter == "DCTDecode" || data.starts_with(&[0xFF, 0xD8]) {
        return image::load_from_memory(data).ok().map(|img| img.to_luma8());
    }
    if width == 0 || height == 0 {
        return None;
    }
    let n = (width as usize).checked_mul(height as usize)?;
    if data.len() == n {
        return GrayImage::from_raw(width, height, data.to_vec());
    }
    if data.len() == n * 3 {
        let mut luma = Vec::with_capacity(n);
        for px in data.chunks_exact(3) {
            let y = (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
                .round()
                .clamp(0.0, 255.0) as u8;
            luma.push(y);
        }
        return GrayImage::from_raw(width, height, luma);
    }
    if data.len() == n * 4 {
        let mut luma = Vec::with_capacity(n);
        for px in data.chunks_exact(4) {
            let y = (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
                .round()
                .clamp(0.0, 255.0) as u8;
            luma.push(y);
        }
        return GrayImage::from_raw(width, height, luma);
    }
    image::load_from_memory(data).ok().map(|img| img.to_luma8())
}

#[cfg(feature = "image")]
#[derive(Debug, Clone)]
struct Grid {
    v: Vec<u32>,
    h: Vec<u32>,
}

#[cfg(feature = "image")]
fn detect_bordered_grid(gray: &GrayImage) -> Option<Grid> {
    let width = gray.width();
    let height = gray.height();
    if width < 100 || height < 80 {
        return None;
    }
    let min_v = (f64::from(height) * MIN_LINE_DARK_RATIO).ceil() as u32;
    let min_h = (f64::from(width) * MIN_LINE_DARK_RATIO).ceil() as u32;
    let mut vertical: Vec<u32> =
        merge_runs((0..width).filter(|&x| count_dark_col(gray, x) >= min_v))
            .into_iter()
            .map(|(a, b)| (a + b) / 2)
            .collect();
    let mut horizontal: Vec<u32> =
        merge_runs((0..height).filter(|&y| count_dark_row(gray, y) >= min_h))
            .into_iter()
            .map(|(a, b)| (a + b) / 2)
            .collect();
    if vertical.len() < MIN_BORDERED_VERTICAL_LINES
        || horizontal.len() < MIN_BORDERED_HORIZONTAL_LINES
    {
        return None;
    }
    let (&min_x, &max_x) = vertical.first().zip(vertical.last())?;
    let (&min_y, &max_y) = horizontal.first().zip(horizontal.last())?;
    vertical.retain(|&x| dark_ratio_col(gray, x, min_y, max_y) >= MIN_TRUE_GRID_LINE_CONTINUITY);
    horizontal.retain(|&y| dark_ratio_row(gray, y, min_x, max_x) >= MIN_TRUE_GRID_LINE_CONTINUITY);
    if vertical.len() < MIN_BORDERED_VERTICAL_LINES
        || horizontal.len() < MIN_BORDERED_HORIZONTAL_LINES
    {
        return None;
    }
    Some(Grid {
        v: vertical,
        h: horizontal,
    })
}

/// Lattice from OCR word centers when ruling lines are faint / incomplete.
#[cfg(feature = "image")]
fn detect_ocr_word_grid(
    words: &[crate::pdf::ocr::OcrWord],
    width: u32,
    height: u32,
) -> Option<Grid> {
    if words.len() < 8 || width < 80 || height < 60 {
        return None;
    }
    let mut heights: Vec<u32> = words.iter().map(|w| w.height.max(1)).collect();
    heights.sort_unstable();
    let med_h = heights[heights.len() / 2].max(6);
    let y_gap = (med_h as f64 * 0.85).round() as u32;

    let mut xs: Vec<u32> = words
        .iter()
        // Full-width line boxes (common from PP-OCR) collapse all columns.
        .filter(|w| w.width < width.saturating_mul(45) / 100 || w.width < 80)
        .map(|w| w.left + w.width / 2)
        .collect();
    if xs.len() < 6 {
        xs = words.iter().map(|w| w.left + w.width / 2).collect();
    }
    let mut ys: Vec<u32> = words.iter().map(|w| w.top + w.height / 2).collect();
    xs.sort_unstable();
    ys.sort_unstable();
    // Header words are wide; gap from word *width* merges real columns.
    // Use median successive-center gap instead (within-col vs between-col).
    let x_gap = adaptive_col_gap(&xs, med_h);
    let col_centers = cluster_1d_centers(&xs, x_gap);
    let row_centers = cluster_1d_centers(&ys, y_gap);
    if col_centers.len() < 2 || row_centers.len() < 3 {
        return None;
    }
    // Reject sparse one-column prose wrapped as a "table".
    let occupied = count_occupied_cells(words, &col_centers, &row_centers, x_gap, y_gap);
    let cells = col_centers.len() * row_centers.len();
    if occupied < 6 || (occupied as f64) < 0.35 * cells as f64 {
        return None;
    }

    let v = centers_to_boundaries(&col_centers, 0, width);
    let h = centers_to_boundaries(&row_centers, 0, height);
    if v.len() < 3 || h.len() < 4 {
        return None;
    }
    Some(Grid { v, h })
}

/// Between-column gap: outlier vs typical within-column jitter of word centers.
#[cfg(feature = "image")]
fn adaptive_col_gap(xs_sorted: &[u32], med_h: u32) -> u32 {
    let mut u = xs_sorted.to_vec();
    u.dedup();
    if u.len() < 3 {
        return (med_h as f64 * 2.5).round().max(12.0) as u32;
    }
    let mut diffs: Vec<u32> = u
        .windows(2)
        .map(|w| w[1] - w[0])
        .filter(|&d| d > 1)
        .collect();
    if diffs.is_empty() {
        return (med_h as f64 * 2.5).round().max(12.0) as u32;
    }
    diffs.sort_unstable();
    let med = diffs[diffs.len() / 2].max(1);
    ((med as f64) * 1.35)
        .round()
        .max((med_h as f64) * 2.0)
        .max(16.0) as u32
}

#[cfg(feature = "image")]
fn cluster_1d_centers(sorted: &[u32], gap: u32) -> Vec<u32> {
    if sorted.is_empty() {
        return Vec::new();
    }
    let mut clusters: Vec<Vec<u32>> = vec![vec![sorted[0]]];
    for &x in &sorted[1..] {
        let last = clusters.last_mut().unwrap();
        if x.saturating_sub(*last.last().unwrap()) <= gap {
            last.push(x);
        } else {
            clusters.push(vec![x]);
        }
    }
    clusters
        .into_iter()
        .map(|c| c.iter().sum::<u32>() / c.len() as u32)
        .collect()
}

#[cfg(feature = "image")]
fn centers_to_boundaries(centers: &[u32], min_edge: u32, max_edge: u32) -> Vec<u32> {
    if centers.is_empty() {
        return vec![min_edge, max_edge];
    }
    let mut b = Vec::with_capacity(centers.len() + 1);
    b.push(min_edge);
    for i in 0..centers.len() - 1 {
        b.push((centers[i] + centers[i + 1]) / 2);
    }
    b.push(max_edge.max(min_edge + 1));
    // Ensure strictly increasing.
    for i in 1..b.len() {
        if b[i] <= b[i - 1] {
            b[i] = b[i - 1] + 1;
        }
    }
    b
}

#[cfg(feature = "image")]
fn count_occupied_cells(
    words: &[crate::pdf::ocr::OcrWord],
    cols: &[u32],
    rows: &[u32],
    x_gap: u32,
    y_gap: u32,
) -> usize {
    let mut seen = std::collections::HashSet::new();
    for w in words {
        let cx = w.left + w.width / 2;
        let cy = w.top + w.height / 2;
        let Some(ci) = cols
            .iter()
            .position(|&c| cx.abs_diff(c) <= x_gap.saturating_mul(2))
        else {
            continue;
        };
        let Some(ri) = rows
            .iter()
            .position(|&r| cy.abs_diff(r) <= y_gap.saturating_mul(2))
        else {
            continue;
        };
        seen.insert((ri, ci));
    }
    seen.len()
}

#[cfg(feature = "image")]
fn grid_has_cell_ink(gray: &GrayImage, grid: &Grid) -> bool {
    let cols = grid.v.len().saturating_sub(1);
    let rows = grid.h.len().saturating_sub(1);
    if cols == 0 || rows == 0 {
        return false;
    }
    let mut inked = 0usize;
    let mut rows_with = 0usize;
    let total = cols * rows;
    for r in 0..rows {
        let mut row_ink = false;
        for c in 0..cols {
            if cell_dark_ratio(gray, grid.v[c], grid.h[r], grid.v[c + 1], grid.h[r + 1]) >= 0.03 {
                inked += 1;
                row_ink = true;
            }
        }
        if row_ink {
            rows_with += 1;
        }
    }
    (inked as f64 / total as f64) >= MIN_BORDERED_INKED_CELL_RATIO
        && rows_with >= MIN_BORDERED_ROWS_WITH_INK
}

#[cfg(feature = "image")]
fn is_obvious_bar_chart(gray: &GrayImage) -> bool {
    if detect_bordered_grid(gray).is_some() {
        return false;
    }
    let w = gray.width();
    let h = gray.height();
    if w < 120 || h < 80 {
        return false;
    }
    let mut peaks = 0usize;
    let mut run = 0u32;
    let thresh = (h as f64 * 0.35) as u32;
    for x in 0..w {
        let dark = count_dark_col(gray, x);
        if dark >= thresh {
            run += 1;
        } else if run > 0 {
            if run >= 3 {
                peaks += 1;
            }
            run = 0;
        }
    }
    peaks >= 4
}

#[cfg(feature = "image")]
fn count_dark_col(gray: &GrayImage, x: u32) -> u32 {
    (0..gray.height())
        .filter(|&y| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count() as u32
}

#[cfg(feature = "image")]
fn count_dark_row(gray: &GrayImage, y: u32) -> u32 {
    (0..gray.width())
        .filter(|&x| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count() as u32
}

#[cfg(feature = "image")]
fn dark_ratio_col(gray: &GrayImage, x: u32, y1: u32, y2: u32) -> f64 {
    let h = y2.saturating_sub(y1).max(1);
    let dark = (y1..y2)
        .filter(|&y| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count() as f64;
    dark / f64::from(h)
}

#[cfg(feature = "image")]
fn dark_ratio_row(gray: &GrayImage, y: u32, x1: u32, x2: u32) -> f64 {
    let w = x2.saturating_sub(x1).max(1);
    let dark = (x1..x2)
        .filter(|&x| gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD)
        .count() as f64;
    dark / f64::from(w)
}

#[cfg(feature = "image")]
fn cell_dark_ratio(gray: &GrayImage, x1: u32, y1: u32, x2: u32, y2: u32) -> f64 {
    let mut dark = 0u32;
    let mut total = 0u32;
    for y in y1..y2 {
        for x in x1..x2 {
            total += 1;
            if gray.get_pixel(x, y).0[0] < RASTER_DARK_THRESHOLD {
                dark += 1;
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        f64::from(dark) / f64::from(total)
    }
}

#[cfg(feature = "image")]
fn merge_runs(xs: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut cur: Option<(u32, u32)> = None;
    for x in xs {
        cur = match cur {
            None => Some((x, x)),
            Some((a, b)) if x <= b + 2 => Some((a, x)),
            Some(r) => {
                out.push(r);
                Some((x, x))
            }
        };
    }
    if let Some(r) = cur {
        out.push(r);
    }
    out
}

#[cfg(feature = "image")]
fn map_boundaries(lines: &[u32], page_a: f64, page_b: f64, raster_span: u32) -> Option<Vec<f64>> {
    if raster_span == 0 || lines.is_empty() {
        return None;
    }
    let span = page_b - page_a;
    Some(
        lines
            .iter()
            .map(|&p| page_a + span * (f64::from(p) / f64::from(raster_span)))
            .collect(),
    )
}

#[cfg(feature = "image")]
fn map_boundaries_desc(
    lines: &[u32],
    page_bottom: f64,
    page_top: f64,
    raster_span: u32,
) -> Option<Vec<f64>> {
    if raster_span == 0 || lines.is_empty() {
        return None;
    }
    let span = page_top - page_bottom;
    Some(
        lines
            .iter()
            .map(|&p| page_top - span * (f64::from(p) / f64::from(raster_span)))
            .collect(),
    )
}

#[cfg(feature = "image")]
fn words_in_cell(words: &[crate::pdf::ocr::OcrWord], x1: u32, y1: u32, x2: u32, y2: u32) -> String {
    let mut hit: Vec<&crate::pdf::ocr::OcrWord> = words
        .iter()
        .filter(|w| {
            let cx = w.left + w.width / 2;
            let cy = w.top + w.height / 2;
            cx >= x1 && cx < x2 && cy >= y1 && cy < y2
        })
        .collect();
    hit.sort_by(|a, b| (a.top, a.left).cmp(&(b.top, b.left)));
    hit.iter()
        .map(|w| w.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Bucket PDF text whose center falls inside a cell (WASM path without OCR).
#[cfg(feature = "image")]
fn text_chunks_in_cell(text_chunks: &[TextChunk], cell: &BoundingBox) -> String {
    let mut hit: Vec<&TextChunk> = text_chunks
        .iter()
        .filter(|t| {
            if t.value.trim().is_empty() {
                return false;
            }
            let cx = (t.bbox.left_x + t.bbox.right_x) * 0.5;
            let cy = (t.bbox.bottom_y + t.bbox.top_y) * 0.5;
            cx >= cell.left_x && cx <= cell.right_x && cy >= cell.bottom_y && cy <= cell.top_y
        })
        .collect();
    hit.sort_by(|a, b| {
        b.bbox
            .top_y
            .partial_cmp(&a.bbox.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.bbox
                    .left_x
                    .partial_cmp(&b.bbox.left_x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    hit.iter()
        .map(|t| t.value.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(feature = "image")]
/// True when a recovered bordered table looks contentful enough to keep.
pub fn bordered_table_is_plausible(table: &TableBorder) -> bool {
    if table.num_rows < 2 || table.num_columns < 2 {
        return false;
    }
    let mut token_counts = Vec::new();
    for row in &table.rows {
        for cell in &row.cells {
            let n = cell
                .content
                .iter()
                .map(|t| t.base.value.split_whitespace().count())
                .sum::<usize>();
            if n > 0 {
                token_counts.push(n);
            }
        }
    }
    if token_counts.is_empty() {
        return false;
    }
    let mut sorted = token_counts.clone();
    sorted.sort_unstable();
    let median = sorted[sorted.len() / 2].max(1);
    let max = *token_counts.iter().max().unwrap_or(&0);
    !(max >= 8 && max >= median.saturating_mul(8))
}
