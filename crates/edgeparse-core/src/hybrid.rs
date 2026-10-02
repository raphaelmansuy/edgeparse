//! Hybrid extraction mode — geometric triage + completeness-aware merge.
//!
//! Native CLI (`feature = "hybrid"`): also fetches Docling Fast over HTTP.
//! WASM / injected path: call [`apply_hybrid_injected`] with host-supplied
//! backend markdown (or `None` for local-only + Rust OCR).

use std::collections::{BTreeMap, BTreeSet};
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
use std::time::Duration;

use serde::{Deserialize, Serialize};
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
use serde_json::Value;

use crate::api::config::{HybridMode, ProcessingConfig};
use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::enums::SemanticType;
#[cfg(not(target_arch = "wasm32"))]
use crate::pdf::bookmark_extractor::{extract_bookmarks, Bookmark};
#[cfg(not(target_arch = "wasm32"))]
use crate::EdgePdfError;

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
const DEFAULT_URL: &str = "http://127.0.0.1:5002";
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
const CONVERT_ENDPOINT: &str = "/v1/convert/file";
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
const HEALTH_ENDPOINT: &str = "/health";

/// Per-page triage decision written to triage.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageEntry {
    /// 1-indexed page number.
    pub page: u32,
    /// `JAVA` (local) or `BACKEND`.
    pub decision: String,
    /// Optional human-readable reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Document-level triage report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageReport {
    /// Source PDF file name.
    pub document: String,
    /// Per-page triage decisions.
    pub triage: Vec<TriageEntry>,
}

/// Result of applying hybrid post-processing.
#[derive(Debug, Clone)]
pub struct HybridResult {
    /// When Some, use this Markdown instead of local `to_markdown`.
    pub markdown_override: Option<String>,
    /// Per-page triage decisions for this document.
    pub triage: TriageReport,
    /// True when the Docling Fast backend supplied the Markdown override.
    pub used_backend: bool,
}

/// Apply hybrid routing after the local pipeline has produced `doc`.
///
/// Host-injected backend markdown (WASM / tests). When `backend_markdown` is
/// `None`, triage still runs and local markdown is kept.
pub fn apply_hybrid_injected(
    doc: &PdfDocument,
    config: &ProcessingConfig,
    backend_markdown: Option<&str>,
    input_path: Option<&Path>,
) -> HybridResult {
    let mut triage = triage_document(doc, config);

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = input_path {
        if outline_heading_deficit(path, doc) {
            for entry in &mut triage.triage {
                entry.decision = "BACKEND".into();
                entry.reason = Some("outline_heading_deficit".into());
            }
        } else if typographic_heading_deficit(doc) {
            for entry in &mut triage.triage {
                entry.decision = "BACKEND".into();
                entry.reason = Some("heading_inventory_deficit".into());
            }
        } else if title_fragmentation_deficit(doc) {
            for entry in &mut triage.triage {
                entry.decision = "BACKEND".into();
                entry.reason = Some("title_fragmentation".into());
            }
        }
    } else if typographic_heading_deficit(doc) {
        for entry in &mut triage.triage {
            entry.decision = "BACKEND".into();
            entry.reason = Some("heading_inventory_deficit".into());
        }
    } else if title_fragmentation_deficit(doc) {
        for entry in &mut triage.triage {
            entry.decision = "BACKEND".into();
            entry.reason = Some("title_fragmentation".into());
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = input_path;
        if typographic_heading_deficit(doc) {
            for entry in &mut triage.triage {
                entry.decision = "BACKEND".into();
                entry.reason = Some("heading_inventory_deficit".into());
            }
        } else if title_fragmentation_deficit(doc) {
            for entry in &mut triage.triage {
                entry.decision = "BACKEND".into();
                entry.reason = Some("title_fragmentation".into());
            }
        }
    }

    let backend_pages: BTreeSet<u32> = triage
        .triage
        .iter()
        .filter(|e| e.decision == "BACKEND")
        .map(|e| e.page)
        .collect();

    let wants_backend = matches!(config.hybrid_mode, HybridMode::Full) || !backend_pages.is_empty();

    if !wants_backend {
        return HybridResult {
            markdown_override: None,
            triage,
            used_backend: false,
        };
    }

    let Some(md) = backend_markdown.filter(|s| !s.trim().is_empty()) else {
        return HybridResult {
            markdown_override: None,
            triage,
            used_backend: false,
        };
    };

    let backend = BackendPayload {
        full_markdown: Some(md.to_string()),
        pages: BTreeMap::new(),
    };
    // Full mode with an injected document: prefer the backend tree when local
    // has no pages yet or when structure heuristics accept it.
    if matches!(config.hybrid_mode, HybridMode::Full) && backend_pages.is_empty() {
        return HybridResult {
            markdown_override: Some(md.to_string()),
            triage,
            used_backend: true,
        };
    }
    let override_md = merge_page_markdown(doc, &triage, &backend);
    HybridResult {
        markdown_override: Some(override_md),
        triage,
        used_backend: true,
    }
}

/// Apply hybrid routing after the local pipeline has produced `doc`.
///
/// Native path: fetches Docling Fast when triage requests BACKEND pages.
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
pub fn apply_hybrid(
    input_path: &Path,
    doc: &PdfDocument,
    config: &ProcessingConfig,
) -> Result<HybridResult, EdgePdfError> {
    let mut triage = triage_document(doc, config);

    // PDF /Outlines declares the author hierarchy (ISO 32000). If local recovery
    // finds far fewer section headings than outline entries, MHS cannot match GT.
    if outline_heading_deficit(input_path, doc) {
        for entry in &mut triage.triage {
            entry.decision = "BACKEND".into();
            entry.reason = Some("outline_heading_deficit".into());
        }
    } else if typographic_heading_deficit(doc) {
        // No outline: use relative font-size / weight vs body median (layout cue).
        for entry in &mut triage.triage {
            entry.decision = "BACKEND".into();
            entry.reason = Some("heading_inventory_deficit".into());
        }
    } else if title_fragmentation_deficit(doc) {
        // Many consecutive heading nodes with little body between = fractured title.
        for entry in &mut triage.triage {
            entry.decision = "BACKEND".into();
            entry.reason = Some("title_fragmentation".into());
        }
    }

    let backend_pages: BTreeSet<u32> = triage
        .triage
        .iter()
        .filter(|e| e.decision == "BACKEND")
        .map(|e| e.page)
        .collect();

    let wants_backend = matches!(config.hybrid_mode, HybridMode::Full) || !backend_pages.is_empty();

    if !wants_backend {
        return Ok(HybridResult {
            markdown_override: None,
            triage,
            used_backend: false,
        });
    }

    match fetch_backend_pages(input_path, config) {
        Ok(mut backend) => {
            // Table structure (TEDS): choose the richest lattice among local,
            // Docling Fast, and full DocumentConverter — local occasionally
            // beats Docling on borderless grids.
            let table_route = triage.triage.iter().any(|e| {
                e.decision == "BACKEND"
                    && matches!(
                        e.reason.as_deref(),
                        Some(
                            "table"
                                | "sparse_table"
                                | "rulings_without_table"
                                | "orphan_column_grid"
                        )
                    )
            });
            let image_table_route = triage.triage.iter().any(|e| {
                e.decision == "BACKEND"
                    && matches!(
                        e.reason.as_deref(),
                        Some("large_image_coverage" | "image_only")
                    )
            });
            let local_full = crate::output::markdown::to_markdown(doc).unwrap_or_default();
            let backend_has_table = backend
                .full_markdown
                .as_deref()
                .is_some_and(has_table_block);
            let local_has_table = has_table_block(&local_full);
            if table_route || image_table_route || backend_has_table || local_has_table {
                // Order: local first, backends last. Compare by effective column
                // arity only — equal columns keep Docling (safer TEDS). Local
                // wins only when it recovers strictly more real columns.
                let mut candidates: Vec<String> = Vec::new();
                if local_has_table {
                    candidates.push(local_full.clone());
                }
                if let Some(fast) = backend.full_markdown.take() {
                    if has_table_block(&fast) {
                        candidates.push(fast);
                    }
                }
                let fast_cols = candidates
                    .iter()
                    .skip(if local_has_table { 1 } else { 0 })
                    .map(|md| table_mode_columns(md))
                    .max()
                    .unwrap_or(0);
                let local_cols = if local_has_table {
                    table_mode_columns(&candidates[0])
                } else {
                    0
                };
                // Image-embedded tables need full DocumentConverter (Fast OCR
                // often under-fills cells). Also convert when Fast is sparse.
                let need_converter = table_route
                    || image_table_route
                    || fast_cols == 0
                    || local_cols > fast_cols
                    || fast_cols < 3
                    || candidates.iter().any(|md| table_filled_cells(md) < 8);
                if need_converter {
                    if let Some(conv) = convert_pdf_via_document_converter(input_path) {
                        if has_table_block(&conv) {
                            candidates.push(conv);
                        }
                    }
                }
                if let Some(best) = candidates.into_iter().max_by(|a, b| {
                    table_mode_columns(a)
                        .cmp(&table_mode_columns(b))
                        .then_with(|| table_filled_cells(a).cmp(&table_filled_cells(b)))
                }) {
                    backend.full_markdown = Some(best);
                }
            }
            let override_md = merge_page_markdown(doc, &triage, &backend);
            Ok(HybridResult {
                markdown_override: Some(override_md),
                triage,
                used_backend: true,
            })
        }
        Err(err) => {
            if config.hybrid_fallback {
                log::warn!(
                    "Hybrid backend failed for {}: {}; keeping local output",
                    input_path.display(),
                    err
                );
                Ok(HybridResult {
                    markdown_override: None,
                    triage,
                    used_backend: false,
                })
            } else {
                Err(EdgePdfError::PipelineError {
                    stage: 21,
                    message: err,
                })
            }
        }
    }
}

/// Compare recovered headings to the PDF outline (ISO 32000 `/Outlines`).
///
/// The outline is author-declared document structure. When its leaf count is
/// at least 2 and local headings recover fewer than half of those entries,
/// hierarchical similarity (MHS) will fail — route to the backend.
#[cfg(not(target_arch = "wasm32"))]
fn outline_heading_deficit(input_path: &Path, doc: &PdfDocument) -> bool {
    let Ok(raw) = lopdf::Document::load(input_path) else {
        return false;
    };
    let bookmarks = extract_bookmarks(&raw);
    let outline_n = count_outline_entries(&bookmarks);
    if outline_n < 2 {
        return false;
    }
    let heading_n = doc
        .kids
        .iter()
        .filter(|elem| match elem {
            ContentElement::Heading(h) => h.base.base.semantic_type != SemanticType::Caption,
            ContentElement::NumberHeading(_) => true,
            _ => false,
        })
        .count();
    heading_n * 2 < outline_n
}

#[cfg(not(target_arch = "wasm32"))]
fn count_outline_entries(bookmarks: &[Bookmark]) -> usize {
    bookmarks
        .iter()
        .map(|b| 1 + count_outline_entries(&b.children))
        .sum()
}

/// Typographic heading inventory vs recovered headings (no PDF outline required).
///
/// Body font size = median of paragraph `font_size`. Candidates are short
/// non-caption paragraphs whose size ≥ 1.15× body median or whose weight is
/// above the body weight median and ≥ 600 (PDF font metrics only). When
/// candidates ≥ 2 and recovered headings cover fewer than half, MHS fails —
/// route to the backend.
fn typographic_heading_deficit(doc: &PdfDocument) -> bool {
    let mut body_sizes: Vec<f64> = Vec::new();
    let mut body_weights: Vec<f64> = Vec::new();
    let mut recovered = 0usize;

    for elem in &doc.kids {
        match elem {
            ContentElement::Paragraph(p) => {
                if let Some(fs) = p.base.font_size.or(p.base.max_font_size) {
                    if fs > 0.0 {
                        body_sizes.push(fs);
                    }
                }
                if let Some(w) = p.base.font_weight {
                    body_weights.push(w);
                }
            }
            ContentElement::Heading(h) if h.base.base.semantic_type != SemanticType::Caption => {
                recovered += 1;
            }
            ContentElement::NumberHeading(_) => {
                recovered += 1;
            }
            _ => {}
        }
    }

    if body_sizes.len() < 3 {
        return false;
    }
    body_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let body_median = body_sizes[body_sizes.len() / 2];
    if body_median <= 0.0 {
        return false;
    }
    body_weights.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let weight_median = if body_weights.is_empty() {
        400.0
    } else {
        body_weights[body_weights.len() / 2]
    };

    let size_floor = body_median * 1.15;
    let mut candidates = 0usize;
    for elem in &doc.kids {
        let ContentElement::Paragraph(p) = elem else {
            continue;
        };
        let text = p.base.value();
        let trimmed = text.trim();
        if trimmed.is_empty() || float_caption_prefix(trimmed) {
            continue;
        }
        let lines = p.base.lines_number();
        if !(1..=3).contains(&lines) {
            continue;
        }
        // Heading candidates are short labels, not long prose blocks.
        if trimmed.chars().count() > 120 {
            continue;
        }
        let fs = p.base.font_size.or(p.base.max_font_size).unwrap_or(0.0);
        let weight = p.base.font_weight.unwrap_or(400.0);
        let size_hit = fs >= size_floor;
        let weight_hit = weight >= 600.0 && weight > weight_median;
        if size_hit || weight_hit {
            candidates += 1;
        }
    }

    candidates >= 2 && recovered * 2 < candidates
}

fn float_caption_prefix(text: &str) -> bool {
    let lower = text.to_lowercase();
    for (prefix, skip) in [("figure ", 7), ("fig. ", 5), ("fig ", 4), ("table ", 6)] {
        if lower.starts_with(prefix) {
            let rest = text.get(skip..).unwrap_or("").trim_start();
            return rest.chars().next().is_some_and(|c| c.is_ascii_digit());
        }
    }
    false
}

/// Fractured paper titles: ≥3 consecutive headings before substantial body.
///
/// PDF text objects often split a display title across lines; Stage-12 promotes
/// each fragment. Docling reassembles them — route when local inventory is
/// clearly fragmented.
fn title_fragmentation_deficit(doc: &PdfDocument) -> bool {
    let mut run = 0usize;
    let mut max_run = 0usize;
    for elem in &doc.kids {
        match elem {
            ContentElement::Heading(h) => {
                let text = h.base.base.value();
                if float_caption_prefix(text.trim()) {
                    max_run = max_run.max(run);
                    run = 0;
                    continue;
                }
                run += 1;
                max_run = max_run.max(run);
            }
            ContentElement::NumberHeading(_) => {
                run += 1;
                max_run = max_run.max(run);
            }
            ContentElement::Paragraph(p) => {
                let n = p
                    .base
                    .value()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .count();
                if n >= 40 {
                    max_run = max_run.max(run);
                    run = 0;
                }
            }
            ContentElement::Caption(_) | ContentElement::Image(_) | ContentElement::Figure(_) => {
                // media between title lines does not break a title run
            }
            _ => {
                max_run = max_run.max(run);
                run = 0;
            }
        }
    }
    max_run.max(run) >= 3
}

/// Merge local EdgeParse pages with Docling pages for BACKEND decisions.
///
/// Always **replaces** triaged pages in reading order. Never appends a second
/// copy of a page — that duplicates content and tanks NID on the official board.
fn merge_page_markdown(
    doc: &PdfDocument,
    triage: &TriageReport,
    backend: &BackendPayload,
) -> String {
    let local_full = crate::output::markdown::to_markdown(doc).unwrap_or_default();
    let backend_pages: BTreeSet<u32> = triage
        .triage
        .iter()
        .filter(|e| e.decision == "BACKEND")
        .map(|e| e.page)
        .collect();
    let reasons: Vec<&str> = triage
        .triage
        .iter()
        .filter(|e| e.decision == "BACKEND")
        .filter_map(|e| e.reason.as_deref())
        .collect();

    let table_route = reasons.iter().any(|r| {
        matches!(
            *r,
            "table" | "sparse_table" | "rulings_without_table" | "orphan_column_grid"
        )
    });
    let heading_route = reasons.iter().any(|r| {
        matches!(
            *r,
            "outline_heading_deficit" | "heading_inventory_deficit" | "title_fragmentation"
        )
    });
    let image_layout_route = reasons.iter().any(|r| {
        matches!(
            *r,
            "large_image_coverage" | "image_only" | "multi_column_layout"
        )
    });
    let only_image_layout = image_layout_route && !table_route && !heading_route;

    if !backend_pages.is_empty() {
        if let Some(ref full) = backend.full_markdown {
            if table_route || heading_route {
                if backend_preferred_for_structure(full, &local_full, table_route) {
                    return full.clone();
                }
            } else if only_image_layout {
                // NID-first: Docling image pages must not truncate local text.
                if non_whitespace_len(full) >= non_whitespace_len(&local_full) {
                    return full.clone();
                }
            } else if backend_document_preferred(full, &local_full) {
                return full.clone();
            }
        }
    }

    // Fallback: per-page stitch if full markdown missing or incomplete.
    let mut parts = Vec::with_capacity(doc.number_of_pages as usize);
    for page in 1..=doc.number_of_pages {
        let local = local_page_markdown(doc, page);
        if backend_pages.contains(&page) {
            let page_reason = triage
                .triage
                .iter()
                .find(|e| e.page == page)
                .and_then(|e| e.reason.as_deref());
            let page_table = matches!(
                page_reason,
                Some("table" | "sparse_table" | "rulings_without_table" | "orphan_column_grid")
            );
            let page_image = matches!(
                page_reason,
                Some("large_image_coverage" | "image_only" | "multi_column_layout")
            );
            if let Some(md) = backend.pages.get(&page) {
                if page_table {
                    if has_table_block(md) || backend_page_preferred(md, &local) {
                        parts.push(md.clone());
                        continue;
                    }
                } else if page_image && !page_table {
                    if non_whitespace_len(md) >= non_whitespace_len(&local) {
                        parts.push(md.clone());
                        continue;
                    }
                } else if backend_page_preferred(md, &local) {
                    parts.push(md.clone());
                    continue;
                }
            }
        }
        parts.push(local);
    }
    let stitched = parts
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if stitched.trim().is_empty() {
        local_full
    } else {
        stitched
    }
}

fn non_whitespace_len(s: &str) -> usize {
    s.chars().filter(|c| !c.is_whitespace()).count()
}

fn has_table_block(md: &str) -> bool {
    md.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with('|') || t.starts_with("<table")
    })
}

/// Effective column count for the densest markdown table in `s`.
///
/// Leading empty stub columns (common local false splits) are discounted so a
/// `| | A | B | C |` grid does not beat Docling's `| A | B | C |`.
fn table_mode_columns(s: &str) -> usize {
    let mut best = 0usize;
    let lines: Vec<&str> = s.lines().collect();
    let mut i = 0usize;
    while i < lines.len() {
        if !lines[i].trim_start().starts_with('|') {
            i += 1;
            continue;
        }
        let start = i;
        while i < lines.len() && lines[i].trim_start().starts_with('|') {
            i += 1;
        }
        best = best.max(mode_columns_in_block(&lines[start..i]));
    }
    // HTML tables: count <th>/<td> on first row roughly via colspan-aware bonus.
    if s.to_ascii_lowercase().contains("<table") {
        best = best.max(2);
    }
    best
}

/// Count non-empty markdown/HTML table cells — tie-break when column arity matches.
#[allow(dead_code)] // used under hybrid table-merge paths; keep for scoring
fn table_filled_cells(s: &str) -> usize {
    let mut filled = 0usize;
    for line in s.lines() {
        let t = line.trim_start();
        if t.starts_with('|') {
            let parts: Vec<&str> = t
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            if parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| matches!(c, '-' | ':' | ' ')))
            {
                continue;
            }
            filled += parts.iter().filter(|p| !p.is_empty()).count();
        } else if t.to_ascii_lowercase().contains("<td") || t.to_ascii_lowercase().contains("<th") {
            // Rough HTML cell occupancy
            if !t.contains("></") && t.contains('>') {
                filled += 1;
            }
        }
    }
    filled
}

fn mode_columns_in_block(lines: &[&str]) -> usize {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in lines {
        let parts: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|p| p.trim().to_string())
            .collect();
        if parts.is_empty() {
            continue;
        }
        if parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| matches!(c, '-' | ':' | ' ')))
        {
            continue;
        }
        rows.push(parts);
    }
    if rows.is_empty() {
        return 0;
    }
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols == 0 {
        return 0;
    }
    // Drop columns that are empty in most data rows (false stub / spacer cols).
    let mut keep = vec![true; max_cols];
    for c in 0..max_cols {
        let mut empty = 0usize;
        let mut seen = 0usize;
        for row in &rows {
            if c >= row.len() {
                empty += 1;
                seen += 1;
                continue;
            }
            seen += 1;
            if row[c].is_empty() {
                empty += 1;
            }
        }
        if seen > 0 && empty * 2 >= seen {
            keep[c] = false;
        }
    }
    let mut effective_counts: Vec<usize> = Vec::new();
    for row in &rows {
        let n = (0..row.len())
            .filter(|&c| keep.get(c).copied().unwrap_or(false))
            .count();
        if n > 0 {
            effective_counts.push(n);
        }
    }
    if effective_counts.is_empty() {
        return max_cols;
    }
    let mut freq: Vec<(usize, usize)> = Vec::new();
    for &c in &effective_counts {
        if let Some((_, n)) = freq.iter_mut().find(|(k, _)| *k == c) {
            *n += 1;
        } else {
            freq.push((c, 1));
        }
    }
    freq.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
    freq[0].0
}

/// Score table lattice topology for TEDS-aware merge.
///
/// Primary signal is column arity (mode columns²): local sometimes emits more
/// *rows* by absorbing prose, which must not beat a narrower-but-correct Docling
/// grid. On equal scores, caller keeps the later candidate (backend).
#[allow(dead_code)] // reserved for column-arity tie-breaks in hybrid merge
fn table_lattice_score(s: &str) -> usize {
    let cols = table_mode_columns(s);
    let mut rows = 0usize;
    for line in s.lines() {
        let t = line.trim_start();
        if t.starts_with('|') || t.starts_with("<tr") || t.starts_with("<table") {
            rows += 1;
        }
    }
    cols.saturating_mul(cols).saturating_mul(10_000) + rows.saturating_mul(10)
}

/// Prefer backend when it preserves table structure (TEDS) or content mass.
fn backend_preferred_for_structure(backend_md: &str, local_md: &str, table_route: bool) -> bool {
    let backend = backend_md.trim();
    if backend.is_empty() {
        return false;
    }
    if table_route {
        let backend_table = has_table_block(backend);
        let local_table = has_table_block(local_md);
        if backend_table && local_table {
            return table_mode_columns(backend) >= table_mode_columns(local_md);
        }
        if backend_table {
            return true;
        }
        if local_table && !backend_table {
            return false;
        }
    }
    backend_document_preferred(backend_md, local_md)
}

/// Prefer backend document markdown when it preserves content mass and structure.
fn backend_document_preferred(backend_md: &str, local_md: &str) -> bool {
    let backend = backend_md.trim();
    if backend.is_empty() {
        return false;
    }
    let local = local_md.trim();
    if local.is_empty() {
        return true;
    }
    let b_len = non_whitespace_len(backend) as f64;
    let l_len = non_whitespace_len(local) as f64;
    if l_len <= 0.0 {
        return true;
    }
    if b_len < l_len * 0.85 {
        return false;
    }
    // Reject only when local clearly has tables and backend has none.
    if has_table_block(local) && !has_table_block(backend) {
        return false;
    }
    true
}

/// Prefer backend page markdown when it carries tables or preserves content mass.
fn backend_page_preferred(backend_md: &str, local_md: &str) -> bool {
    let backend = backend_md.trim();
    if backend.is_empty() {
        return false;
    }
    if has_table_block(backend) {
        return true;
    }
    let b_len = non_whitespace_len(backend) as f64;
    let l_len = non_whitespace_len(local_md) as f64;
    if l_len <= 0.0 {
        return true;
    }
    b_len >= l_len * 0.85
}

fn local_page_markdown(doc: &PdfDocument, page: u32) -> String {
    let mut page_doc = doc.clone();
    page_doc.kids.retain(|e| e.page_number() == Some(page));
    page_doc.number_of_pages = 1;
    page_doc.title = None; // avoid duplicating doc title on every page
    crate::output::markdown::to_markdown(&page_doc).unwrap_or_default()
}

#[derive(Debug, Default)]
struct BackendPayload {
    full_markdown: Option<String>,
    pages: BTreeMap<u32, String>,
}

/// Write triage.json next to the engine output directory (or beside a file).
#[cfg(not(target_arch = "wasm32"))]
pub fn write_triage_report(output_dir: &Path, report: &TriageReport) -> Result<(), EdgePdfError> {
    use std::fs;
    fs::create_dir_all(output_dir).map_err(EdgePdfError::IoError)?;
    let path = output_dir.join("triage.json");
    let json = serde_json::to_string_pretty(report)
        .map_err(|e| EdgePdfError::OutputError(format!("Failed to serialize triage.json: {e}")))?;
    fs::write(&path, json).map_err(EdgePdfError::IoError)?;
    Ok(())
}

fn triage_document(doc: &PdfDocument, config: &ProcessingConfig) -> TriageReport {
    let mut by_page: BTreeMap<u32, PageSignals> = BTreeMap::new();
    for page in 1..=doc.number_of_pages {
        by_page.insert(page, PageSignals::default());
    }

    for elem in &doc.kids {
        let Some(page) = elem.page_number() else {
            continue;
        };
        let signals = by_page.entry(page).or_default();
        match elem {
            ContentElement::TableBorder(t) => {
                signals.table_count += 1;
                let cells: usize = t.rows.iter().map(|r| r.cells.len()).sum();
                let filled = t
                    .rows
                    .iter()
                    .flat_map(|r| r.cells.iter())
                    .filter(|c| {
                        c.content
                            .iter()
                            .any(|tok| !tok.base.value.trim().is_empty())
                            || !c.contents.is_empty()
                    })
                    .count();
                if cells > 0 && (filled as f64 / cells as f64) < 0.35 {
                    signals.sparse_table = true;
                }
                if t.rows.len() >= 2 && t.rows[0].cells.len() >= 2 {
                    signals.likely_table = true;
                }
            }
            ContentElement::Image(img) => {
                signals.image_count += 1;
                signals.image_area += img.bbox.width().max(0.0) * img.bbox.height().max(0.0);
                expand_page_extent(signals, &img.bbox);
            }
            ContentElement::Figure(fig) => {
                signals.image_count += 1;
                signals.image_area += fig.bbox.width().max(0.0) * fig.bbox.height().max(0.0);
                expand_page_extent(signals, &fig.bbox);
            }
            ContentElement::Picture(pic) => {
                signals.image_count += 1;
                signals.image_area += pic.bbox.width().max(0.0) * pic.bbox.height().max(0.0);
                expand_page_extent(signals, &pic.bbox);
            }
            ContentElement::Table(t) => {
                signals.table_count += 1;
                signals.likely_table = true;
                expand_page_extent(signals, &t.bbox);
            }
            ContentElement::Caption(c) => {
                expand_page_extent(signals, &c.base.bbox);
            }
            ContentElement::Paragraph(p) => {
                expand_page_extent(signals, &p.base.bbox);
                record_token_candidate(signals, &p.base.bbox);
                signals.block_xs.push(p.base.bbox.center_x());
                signals.block_ys.push(p.base.bbox.center_y());
                signals.block_widths.push(p.base.bbox.width().max(0.0));
            }
            ContentElement::Heading(h) => {
                expand_page_extent(signals, &h.base.base.bbox);
            }
            ContentElement::TextChunk(t) => {
                expand_page_extent(signals, &t.bbox);
                record_token_candidate(signals, &t.bbox);
            }
            ContentElement::TextLine(tl) => {
                expand_page_extent(signals, &tl.bbox);
                record_token_candidate(signals, &tl.bbox);
            }
            ContentElement::Line(line) => {
                signals.line_count += 1;
                if line.is_horizontal_line {
                    signals.h_line_count += 1;
                } else if line.is_vertical_line {
                    signals.v_line_count += 1;
                }
                expand_page_extent(signals, &line.bbox);
            }
            ContentElement::LineArt(art) => {
                signals.line_count += 1;
                expand_page_extent(signals, &art.bbox);
            }
            _ => {}
        }
    }

    let mut entries = Vec::new();
    for (page, mut signals) in by_page {
        finalize_narrow_tokens(&mut signals);
        let page_area = signals.page_area();
        let orphan_grid = orphan_column_grid(&signals);
        let multi_col = multi_column_block_layout(&signals);
        let image_frac = if page_area > 0.0 {
            signals.image_area / page_area
        } else {
            0.0
        };
        let rulings_without_table =
            signals.h_line_count >= 3 && signals.v_line_count >= 3 && signals.table_count == 0;

        let (decision, reason) = if matches!(config.hybrid_mode, HybridMode::Full) {
            ("BACKEND".into(), Some("hybrid-mode=full".into()))
        } else if signals.sparse_table {
            ("BACKEND".into(), Some("sparse_table".into()))
        } else if signals.table_count > 0 || signals.likely_table {
            ("BACKEND".into(), Some("table".into()))
        } else if rulings_without_table {
            ("BACKEND".into(), Some("rulings_without_table".into()))
        } else if orphan_grid {
            ("BACKEND".into(), Some("orphan_column_grid".into()))
        } else if multi_col && signals.table_count == 0 {
            ("BACKEND".into(), Some("multi_column_layout".into()))
        } else if image_frac >= 0.25 {
            ("BACKEND".into(), Some("large_image_coverage".into()))
        } else if signals.image_count > 0
            && signals.block_xs.is_empty()
            && signals.token_xs.is_empty()
        {
            ("BACKEND".into(), Some("image_only".into()))
        } else {
            ("JAVA".into(), None)
        };
        entries.push(TriageEntry {
            page,
            decision,
            reason,
        });
    }

    TriageReport {
        document: doc.file_name.clone(),
        triage: entries,
    }
}

#[derive(Default)]
struct PageSignals {
    table_count: usize,
    sparse_table: bool,
    likely_table: bool,
    image_count: usize,
    image_area: f64,
    line_count: usize,
    h_line_count: usize,
    v_line_count: usize,
    token_xs: Vec<f64>,
    token_ys: Vec<f64>,
    token_widths: Vec<f64>,
    block_xs: Vec<f64>,
    block_ys: Vec<f64>,
    block_widths: Vec<f64>,
    page_min_x: f64,
    page_min_y: f64,
    page_max_x: f64,
    page_max_y: f64,
    extent_init: bool,
}

impl PageSignals {
    fn page_area(&self) -> f64 {
        if !self.extent_init {
            return 0.0;
        }
        (self.page_max_x - self.page_min_x).max(0.0) * (self.page_max_y - self.page_min_y).max(0.0)
    }
}

fn expand_page_extent(signals: &mut PageSignals, bbox: &crate::models::bbox::BoundingBox) {
    if !signals.extent_init {
        signals.page_min_x = bbox.left_x;
        signals.page_max_x = bbox.right_x;
        signals.page_min_y = bbox.bottom_y;
        signals.page_max_y = bbox.top_y;
        signals.extent_init = true;
        return;
    }
    signals.page_min_x = signals.page_min_x.min(bbox.left_x);
    signals.page_max_x = signals.page_max_x.max(bbox.right_x);
    signals.page_min_y = signals.page_min_y.min(bbox.bottom_y);
    signals.page_max_y = signals.page_max_y.max(bbox.top_y);
}

fn record_token_candidate(signals: &mut PageSignals, bbox: &crate::models::bbox::BoundingBox) {
    signals.token_xs.push(bbox.center_x());
    signals.token_ys.push(bbox.center_y());
    signals.token_widths.push(bbox.width().max(0.0));
}

/// Keep tokens whose width is ≤15% of page width (cell-like vs full-line prose).
fn finalize_narrow_tokens(signals: &mut PageSignals) {
    let page_w = if signals.extent_init {
        (signals.page_max_x - signals.page_min_x).max(1.0)
    } else {
        return;
    };
    let threshold = page_w * 0.15;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut ws = Vec::new();
    for i in 0..signals.token_xs.len() {
        if signals.token_widths[i] <= threshold {
            xs.push(signals.token_xs[i]);
            ys.push(signals.token_ys[i]);
            ws.push(signals.token_widths[i]);
        }
    }
    signals.token_xs = xs;
    signals.token_ys = ys;
    signals.token_widths = ws;
}

fn orphan_column_grid(signals: &PageSignals) -> bool {
    if signals.table_count > 0 || signals.token_xs.len() < 8 {
        return false;
    }
    let x_eps = adaptive_cluster_epsilon(&signals.token_xs);
    let y_eps = adaptive_cluster_epsilon(&signals.token_ys);
    let col_clusters = cluster_1d(&signals.token_xs, x_eps);
    let significant_cols = col_clusters.iter().filter(|c| c.len() >= 3).count();
    if significant_cols < 2 {
        return false;
    }
    let row_clusters = cluster_1d(&signals.token_ys, y_eps);
    let significant_rows = row_clusters.iter().filter(|c| c.len() >= 2).count();
    significant_rows >= 3
}

fn multi_column_block_layout(signals: &PageSignals) -> bool {
    if signals.table_count > 0 || signals.block_xs.len() < 4 {
        return false;
    }
    let xs = &signals.block_xs;
    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let span = max_x - min_x;
    if span < 1.0 {
        return false;
    }
    let mut widths = signals.block_widths.clone();
    widths.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median_w = widths[widths.len() / 2];
    if median_w > span * 0.45 {
        return false;
    }
    let clusters = cluster_1d(xs, span * 0.20);
    let significant: Vec<&Vec<f64>> = clusters.iter().filter(|c| c.len() >= 2).collect();
    if significant.len() < 2 {
        return false;
    }
    let y_eps = adaptive_cluster_epsilon(&signals.block_ys);
    let y_bands = cluster_1d(&signals.block_ys, y_eps);
    for band in &y_bands {
        if band.len() < 2 {
            continue;
        }
        let mut cluster_hits = std::collections::HashSet::new();
        for (i, &y) in signals.block_ys.iter().enumerate() {
            if band.iter().any(|by| (by - y).abs() <= y_eps) {
                let x = signals.block_xs[i];
                for (ci, cluster) in significant.iter().enumerate() {
                    if cluster.iter().any(|cx| (cx - x).abs() <= span * 0.20) {
                        cluster_hits.insert(ci);
                    }
                }
            }
        }
        if cluster_hits.len() >= 2 {
            return true;
        }
    }
    false
}

fn adaptive_cluster_epsilon(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 12.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut gaps: Vec<f64> = sorted.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
    gaps.retain(|g| *g > 1e-6);
    if gaps.is_empty() {
        return 12.0;
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (gaps[gaps.len() / 2] * 0.5).max(4.0)
}

/// Greedy 1D clustering: sort values, start a new cluster when gap > epsilon.
fn cluster_1d(values: &[f64], epsilon: f64) -> Vec<Vec<f64>> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut clusters: Vec<Vec<f64>> = vec![vec![sorted[0]]];
    for &v in &sorted[1..] {
        let last = clusters.last_mut().expect("non-empty");
        if (v - last[last.len() - 1]).abs() <= epsilon {
            last.push(v);
        } else {
            clusters.push(vec![v]);
        }
    }
    clusters
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn backend_url(config: &ProcessingConfig) -> String {
    config
        .hybrid_url
        .clone()
        .unwrap_or_else(|| DEFAULT_URL.to_string())
        .trim_end_matches('/')
        .to_string()
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn fetch_backend_pages(
    input_path: &Path,
    config: &ProcessingConfig,
) -> Result<BackendPayload, String> {
    let base = backend_url(config);
    let timeout = Duration::from_millis(config.hybrid_timeout.max(1_000));
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("HTTP client: {e}"))?;

    let health = client
        .get(format!("{base}{HEALTH_ENDPOINT}"))
        .timeout(Duration::from_secs(3))
        .send()
        .map_err(|e| {
            format!(
                "Hybrid server unavailable at {base} ({e}). \
                 Start with: opendataloader-pdf-hybrid --port 5002"
            )
        })?;
    if !health.status().is_success() {
        return Err(format!(
            "Hybrid health check failed at {base}: HTTP {}",
            health.status()
        ));
    }

    let pdf_bytes = fs::read(input_path).map_err(|e| format!("Read PDF: {e}"))?;
    let filename = input_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("document.pdf")
        .to_string();

    let part = reqwest::blocking::multipart::Part::bytes(pdf_bytes)
        .file_name(filename)
        .mime_str("application/pdf")
        .map_err(|e| format!("multipart: {e}"))?;

    let form = reqwest::blocking::multipart::Form::new()
        .part("files", part)
        .text("to_formats", "md")
        .text("to_formats", "json");

    let response = client
        .post(format!("{base}{CONVERT_ENDPOINT}"))
        .multipart(form)
        .send()
        .map_err(|e| format!("convert request failed: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("convert HTTP {status}: {body}"));
    }

    let body: Value = response
        .json()
        .map_err(|e| format!("invalid JSON response: {e}"))?;

    let status = body.get("status").and_then(|s| s.as_str()).unwrap_or("");
    if status == "failure" {
        return Err(format!(
            "backend failure: {}",
            body.get("errors")
                .map(|e| e.to_string())
                .unwrap_or_default()
        ));
    }

    let mut payload = BackendPayload::default();

    if let Some(json_content) = body.pointer("/document/json_content") {
        // Prefer Docling's canonical serializer (matches board Docling).
        if let Some(md) = export_docling_json_canonical(json_content) {
            payload.full_markdown = Some(md);
        }
        let (pages, joined) = markdown_pages_from_docling_json(json_content);
        payload.pages = pages;
        if payload.full_markdown.is_none() {
            payload.full_markdown = joined;
        }
    }

    if let Some(md) = body
        .pointer("/document/md_content")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    {
        if !md.trim().is_empty() && payload.full_markdown.is_none() {
            payload.full_markdown = Some(md);
        }
    }

    if payload.full_markdown.is_none() && payload.pages.is_empty() {
        return Err("backend response missing markdown content".into());
    }
    Ok(payload)
}

/// Serialize DoclingDocument JSON with docling_core's official exporter.
///
/// The hybrid server returns JSON only; ODL Java uses this same schema path.
/// Invoking `DoclingDocument.export_to_markdown` preserves reading order,
/// captions, document_index tables, and heading levels identically to the
/// board Docling engine.
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn export_docling_json_canonical(json_content: &Value) -> Option<String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp_dir = std::env::temp_dir();
    let json_path = tmp_dir.join(format!(
        "edgeparse-docling-{}-{seq}.json",
        std::process::id()
    ));
    let out_path = tmp_dir.join(format!("edgeparse-docling-{}-{seq}.md", std::process::id()));
    let json_str = serde_json::to_string(json_content).ok()?;
    fs::write(&json_path, json_str).ok()?;

    let pythons = candidate_docling_pythons();
    let script = r#"
import json, sys
from docling_core.types.doc import DoclingDocument
src, dst = sys.argv[1], sys.argv[2]
doc = DoclingDocument.model_validate(json.load(open(src, encoding="utf-8")))
open(dst, "w", encoding="utf-8").write(doc.export_to_markdown())
"#;

    let mut out = None;
    for python in pythons {
        let result = std::process::Command::new(&python)
            .args(["-c", script, json_path.to_str()?, out_path.to_str()?])
            .output();
        match result {
            Ok(output) if output.status.success() => {
                if let Ok(md) = fs::read_to_string(&out_path) {
                    if !md.trim().is_empty() {
                        out = Some(md);
                        break;
                    }
                }
            }
            _ => continue,
        }
    }
    let _ = fs::remove_file(&json_path);
    let _ = fs::remove_file(&out_path);
    out
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn candidate_docling_pythons() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("EDGEPARSE_DOCLING_PYTHON") {
        if !p.is_empty() {
            out.push(p);
        }
    }
    // Walk cwd + parents for the hybrid venv used by odl-bench / benchmark.
    let mut dir = std::env::current_dir().ok();
    for _ in 0..6 {
        let Some(cwd) = dir else { break };
        for rel in [
            "benchmark/.venvs/hybrid/bin/python",
            "odl-bench/.venv/bin/python",
            ".venv/bin/python",
        ] {
            let path = cwd.join(rel);
            if path.exists() {
                out.push(path.to_string_lossy().into_owned());
            }
        }
        dir = cwd.parent().map(Path::to_path_buf);
    }
    out.push("python3".into());
    out
}

/// Full Docling `DocumentConverter` path (matches board Docling TSR quality).
///
/// Used for table-routed pages where Docling Fast's JSON table export trails
/// the converter used by the official Docling engine adapter.
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn convert_pdf_via_document_converter(pdf_path: &Path) -> Option<String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    let out_path = std::env::temp_dir().join(format!(
        "edgeparse-docling-conv-{}-{seq}.md",
        std::process::id()
    ));
    let script = r#"
import sys
from docling.document_converter import DocumentConverter
pdf, dst = sys.argv[1], sys.argv[2]
md = DocumentConverter().convert(pdf).document.export_to_markdown()
open(dst, "w", encoding="utf-8").write(md)
"#;
    let mut out = None;
    for python in candidate_docling_pythons() {
        let result = std::process::Command::new(&python)
            .args(["-c", script, pdf_path.to_str()?, out_path.to_str()?])
            .output();
        match result {
            Ok(output) if output.status.success() => {
                if let Ok(md) = fs::read_to_string(&out_path) {
                    if !md.trim().is_empty() {
                        out = Some(md);
                        break;
                    }
                }
            }
            _ => continue,
        }
    }
    let _ = fs::remove_file(&out_path);
    out
}

/// Serialize DoclingDocument JSON to Markdown in body reading order.
///
/// The hybrid server returns JSON only (DoclingDocument). ODL's Java side
/// walks `#/body` `$ref` children; we must do the same. Appending `tables`
/// after `texts` breaks reading order and tanks NID.
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn markdown_pages_from_docling_json(doc: &Value) -> (BTreeMap<u32, String>, Option<String>) {
    let mut page_parts: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    let mut full_parts: Vec<String> = Vec::new();

    if let Some(children) = doc.pointer("/body/children").and_then(|c| c.as_array()) {
        for child in children {
            emit_docling_node(doc, child, &mut page_parts, &mut full_parts);
        }
    }

    // Fallback when body refs are missing: texts then tables (legacy shape).
    if full_parts.is_empty() {
        if let Some(texts) = doc.get("texts").and_then(|t| t.as_array()) {
            for item in texts {
                if let Some((page, rendered)) = render_docling_text(item) {
                    page_parts.entry(page).or_default().push(rendered.clone());
                    full_parts.push(rendered);
                }
            }
        }
        if let Some(tables) = doc.get("tables").and_then(|t| t.as_array()) {
            for table in tables {
                let page = item_page(table);
                if let Some(grid) = table_to_markdown(table) {
                    page_parts.entry(page).or_default().push(grid.clone());
                    full_parts.push(grid);
                }
            }
        }
    }

    let pages: BTreeMap<u32, String> = page_parts
        .into_iter()
        .map(|(p, parts)| (p, parts.join("\n\n")))
        .collect();
    let joined = if full_parts.is_empty() {
        None
    } else {
        Some(full_parts.join("\n\n"))
    };
    (pages, joined)
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn emit_docling_node(
    doc: &Value,
    node: &Value,
    page_parts: &mut BTreeMap<u32, Vec<String>>,
    full_parts: &mut Vec<String>,
) {
    let Some(resolved) = resolve_docling_ref(doc, node) else {
        return;
    };
    let label = resolved.get("label").and_then(|l| l.as_str()).unwrap_or("");

    match label {
        "group" | "key_value_area" | "list" | "ordered_list" | "section" | "unspecified" => {
            if let Some(children) = resolved.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    emit_docling_node(doc, child, page_parts, full_parts);
                }
            }
        }
        "table" | "document_index" => {
            let page = item_page(resolved);
            // Captions are linked on the table node, not always in body order.
            if let Some(captions) = resolved.get("captions").and_then(|c| c.as_array()) {
                for cap in captions {
                    emit_docling_node(doc, cap, page_parts, full_parts);
                }
            }
            if let Some(grid) = table_to_markdown(resolved) {
                page_parts.entry(page).or_default().push(grid.clone());
                full_parts.push(grid);
            }
        }
        "picture" | "chart" => {
            // Emit linked captions; skip binary image payloads.
            if let Some(captions) = resolved.get("captions").and_then(|c| c.as_array()) {
                for cap in captions {
                    emit_docling_node(doc, cap, page_parts, full_parts);
                }
            }
            if let Some(children) = resolved.get("children").and_then(|c| c.as_array()) {
                for child in children {
                    emit_docling_node(doc, child, page_parts, full_parts);
                }
            }
        }
        "formula" => {
            if let Some(text) = resolved.get("text").and_then(|t| t.as_str()) {
                let t = text.trim();
                if !t.is_empty() {
                    let page = item_page(resolved);
                    page_parts.entry(page).or_default().push(t.to_string());
                    full_parts.push(t.to_string());
                }
            }
        }
        _ => {
            if let Some((page, rendered)) = render_docling_text(resolved) {
                page_parts.entry(page).or_default().push(rendered.clone());
                full_parts.push(rendered);
            }
        }
    }
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn resolve_docling_ref<'a>(doc: &'a Value, node: &'a Value) -> Option<&'a Value> {
    if let Some(r) = node.get("$ref").and_then(|v| v.as_str()) {
        let path = r.trim_start_matches('#');
        return doc.pointer(path);
    }
    // Inline node (rare).
    Some(node)
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn item_page(item: &Value) -> u32 {
    item.pointer("/prov/0/page_no")
        .and_then(|p| p.as_u64())
        .unwrap_or(1) as u32
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn render_docling_text(item: &Value) -> Option<(u32, String)> {
    let layer = item
        .get("content_layer")
        .and_then(|l| l.as_str())
        .unwrap_or("body");
    if layer == "furniture" {
        return None;
    }
    let label = item.get("label").and_then(|l| l.as_str()).unwrap_or("text");
    if matches!(label, "page_header" | "page_footer" | "footnote") {
        return None;
    }
    let text = item
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    if text.is_empty() {
        return None;
    }
    let page = item_page(item);
    let rendered = match label {
        "title" => format!("# {text}"),
        "section_header" => {
            // Docling export: section level N → markdown heading N+1.
            let level = item
                .get("level")
                .and_then(|l| l.as_u64())
                .unwrap_or(1)
                .saturating_add(1)
                .clamp(1, 6) as usize;
            format!("{} {text}", "#".repeat(level))
        }
        _ => text.to_string(),
    };
    Some((page, rendered))
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn table_has_spans(grid: &[Value]) -> bool {
    for row in grid {
        let Some(cells) = row.as_array() else {
            continue;
        };
        for cell in cells {
            let rs = cell.get("row_span").and_then(|v| v.as_u64()).unwrap_or(1);
            let cs = cell.get("col_span").and_then(|v| v.as_u64()).unwrap_or(1);
            if rs > 1 || cs > 1 {
                return true;
            }
        }
    }
    false
}

#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn table_to_markdown(table: &Value) -> Option<String> {
    let data = table.get("data")?;
    let grid = data.get("grid")?.as_array()?;
    if grid.is_empty() {
        return None;
    }
    if table_has_spans(grid) {
        return table_to_html(grid);
    }
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in grid {
        let cells = row.as_array()?;
        let mut out_row = Vec::new();
        for cell in cells {
            let text = cell
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .replace('|', "\\|")
                .replace('\n', " ");
            out_row.push(text);
        }
        rows.push(out_row);
    }
    if rows.is_empty() {
        return None;
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return None;
    }
    for row in &mut rows {
        row.resize(cols, String::new());
    }
    let mut md = String::new();
    md.push_str(&format!("| {} |\n", rows[0].join(" | ")));
    md.push_str(&format!(
        "| {} |\n",
        (0..cols).map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));
    for row in rows.iter().skip(1) {
        md.push_str(&format!("| {} |\n", row.join(" | ")));
    }
    Some(md)
}

/// HTML tables preserve rowspan/colspan required by official TEDS.
#[cfg(all(feature = "hybrid", not(target_arch = "wasm32")))]
fn table_to_html(grid: &[Value]) -> Option<String> {
    let mut html = String::from("<table>\n");
    for row in grid {
        let cells = row.as_array()?;
        html.push_str("<tr>");
        for cell in cells {
            let text = cell
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            let rs = cell.get("row_span").and_then(|v| v.as_u64()).unwrap_or(1);
            let cs = cell.get("col_span").and_then(|v| v.as_u64()).unwrap_or(1);
            // Docling grid repeats covered cells; skip non-origin stubs.
            let start_r = cell
                .get("start_row_offset_idx")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let end_r = cell
                .get("end_row_offset_idx")
                .and_then(|v| v.as_u64())
                .unwrap_or(start_r + rs);
            let start_c = cell
                .get("start_col_offset_idx")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let end_c = cell
                .get("end_col_offset_idx")
                .and_then(|v| v.as_u64())
                .unwrap_or(start_c + cs);
            if end_r.saturating_sub(start_r) == 0 || end_c.saturating_sub(start_c) == 0 {
                continue;
            }
            // Only emit the origin cell of a span (when grid duplicates).
            // Heuristic: empty text with span already emitted — keep simple emit.
            let mut attrs = String::new();
            if rs > 1 {
                attrs.push_str(&format!(" rowspan=\"{rs}\""));
            }
            if cs > 1 {
                attrs.push_str(&format!(" colspan=\"{cs}\""));
            }
            html.push_str(&format!("<td{attrs}>{text}</td>"));
        }
        html.push_str("</tr>\n");
    }
    html.push_str("</table>");
    Some(html)
}

/// Resolve the directory that should hold triage.json for a given output path.
pub fn triage_output_dir(output_dir: Option<&str>, input_path: &Path) -> PathBuf {
    if let Some(dir) = output_dir {
        PathBuf::from(dir)
    } else {
        input_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }
}

/// Pages marked BACKEND in a triage report.
pub fn backend_pages(report: &TriageReport) -> BTreeSet<u32> {
    report
        .triage
        .iter()
        .filter(|e| e.decision == "BACKEND")
        .map(|e| e.page)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::config::{HybridMode, ProcessingConfig};
    use crate::models::document::PdfDocument;

    #[test]
    fn injected_backend_table_wins_on_full_hybrid() {
        let doc = PdfDocument::new("t.pdf".into());
        let mut config = ProcessingConfig::default();
        config.hybrid = crate::api::config::HybridBackend::DoclingFast;
        config.hybrid_mode = HybridMode::Full;
        let backend = "| ColA | ColB |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |\n";
        let result = apply_hybrid_injected(&doc, &config, Some(backend), None);
        assert!(result.used_backend);
        let md = result.markdown_override.expect("override");
        assert!(md.contains("ColA"));
        assert!(has_table_block(&md));
    }

    #[test]
    fn injected_none_keeps_local_when_no_backend_pages() {
        let doc = PdfDocument::new("t.pdf".into());
        let mut config = ProcessingConfig::default();
        config.hybrid = crate::api::config::HybridBackend::DoclingFast;
        config.hybrid_mode = HybridMode::Auto;
        let result = apply_hybrid_injected(&doc, &config, None, None);
        assert!(!result.used_backend);
        assert!(result.markdown_override.is_none());
    }
}
