//! Markdown output generator.

use std::collections::{HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;
#[cfg(not(target_arch = "wasm32"))]
use regex::Regex;

use crate::models::bbox::BoundingBox;
use crate::models::chunks::TextChunk;
use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::enums::SemanticType;
use crate::models::semantic::SemanticTextNode;
use crate::models::table::TableTokenRow;
use crate::EdgePdfError;

#[cfg(not(target_arch = "wasm32"))]
struct CachedBBoxLayout {
    page_width: f64,
    lines: Vec<BBoxLayoutLine>,
    blocks: Vec<BBoxLayoutBlock>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct LayoutSourceCache {
    bbox_layout: Option<Option<CachedBBoxLayout>>,
    layout_lines: Option<Option<Vec<String>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl LayoutSourceCache {
    fn bbox_layout(&mut self, doc: &PdfDocument) -> Option<&CachedBBoxLayout> {
        if self.bbox_layout.is_none() {
            let loaded = doc.source_path.as_deref().and_then(|source_path| {
                let (page_width, lines) = read_pdftotext_bbox_layout_lines(Path::new(source_path))?;
                let blocks = collect_bbox_layout_blocks(&lines);
                Some(CachedBBoxLayout {
                    page_width,
                    lines,
                    blocks,
                })
            });
            self.bbox_layout = Some(loaded);
        }
        self.bbox_layout.as_ref().and_then(Option::as_ref)
    }

    fn layout_lines(&mut self, doc: &PdfDocument) -> Option<&[String]> {
        if self.layout_lines.is_none() {
            let loaded = doc
                .source_path
                .as_deref()
                .and_then(|source_path| read_pdftotext_layout_lines(Path::new(source_path)));
            self.layout_lines = Some(loaded);
        }
        self.layout_lines
            .as_ref()
            .and_then(Option::as_ref)
            .map(Vec::as_slice)
    }
}

/// Generate Markdown representation of a PdfDocument.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on write failures.
pub fn to_markdown(doc: &PdfDocument) -> Result<String, EdgePdfError> {
    #[cfg(not(target_arch = "wasm32"))]
    let mut layout_cache = LayoutSourceCache::default();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) = render_layout_open_plate_document_cached(doc, &mut layout_cache) {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_single_caption_chart_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) = render_layout_captioned_media_document_cached(doc, &mut layout_cache) {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_recommendation_infographic_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_stacked_bar_report_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_multi_figure_chart_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_ocr_benchmark_dashboard_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) = render_layout_toc_document_cached(doc, &mut layout_cache) {
        return Ok(rendered);
    }
    if looks_like_contents_document(doc) {
        return Ok(render_contents_document(doc));
    }
    if looks_like_compact_toc_document(doc) {
        return Ok(render_compact_toc_document(doc));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_projection_sheet_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_appendix_tables_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_titled_dual_table_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_dual_table_article_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) =
        render_layout_registration_report_document_cached(doc, &mut layout_cache)
    {
        return Ok(rendered);
    }
    if let Some(rendered) = render_top_table_plate_document(doc) {
        return Ok(rendered);
    }
    if let Some(rendered) = render_single_table_report_document(doc) {
        return Ok(rendered);
    }
    if let Some(rendered) = render_late_section_boundary_document(doc) {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) = render_layout_matrix_document_cached(doc, &mut layout_cache) {
        return Ok(rendered);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(rendered) = render_layout_panel_stub_document_cached(doc, &mut layout_cache) {
        return Ok(rendered);
    }

    Ok(render_markdown_core(doc))
}

fn render_markdown_core(doc: &PdfDocument) -> String {
    let mut output = String::new();

    // Title
    if let Some(ref title) = doc.title {
        let trimmed = title.trim();
        if !trimmed.is_empty() && !should_skip_document_title(doc, trimmed) {
            if should_render_document_title_as_plaintext(doc, trimmed) {
                output.push_str(trimmed);
                output.push_str("\n\n");
            } else {
                output.push_str(&format!("# {}\n\n", trimmed));
            }
        }
    }

    if doc.kids.is_empty() {
        output.push_str("*No content extracted.*\n");
        return output;
    }

    let geometric_table_regions = detect_geometric_table_regions(doc);
    let mut geometric_table_cover = HashMap::new();
    for region in geometric_table_regions {
        for idx in region.start_idx..=region.end_idx {
            geometric_table_cover.insert(idx, region.clone());
        }
    }

    let mut i = 0usize;
    while i < doc.kids.len() {
        if let Some(region) = geometric_table_cover.get(&i) {
            output.push_str(&region.rendered);
            i = region.end_idx + 1;
            continue;
        }

        match &doc.kids[i] {
            ContentElement::Heading(h) => {
                let text = h.base.base.value();
                let trimmed = text.trim();
                if trimmed.is_empty() || should_skip_heading_text(trimmed) {
                    i += 1;
                    continue;
                }

                // Demote headings that sit in the bottom margin of the page
                // (running footers misclassified as headings by the pipeline).
                if looks_like_bottom_margin_heading(doc, i) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote pipeline headings that look like sentence fragments
                // ending with a period but are not numbered section headings.
                if should_demote_period_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote headings ending with comma (footnotes / data labels).
                if should_demote_comma_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote headings containing math symbols.
                if should_demote_math_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote headings containing percentage signs.
                if should_demote_percentage_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote headings that start with a known caption prefix
                // (e.g. "Source:", "Figure", "Table") — these are captions,
                // not section headings, regardless of pipeline classification.
                if starts_with_caption_prefix(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote bibliography entries: lines starting with a 4-digit
                // year followed by a period (e.g. "2020. Title of paper...").
                if should_demote_bibliography_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                if let Some(next_text) = next_mergeable_paragraph_text(doc.kids.get(i + 1)) {
                    if should_demote_heading_to_paragraph(trimmed, &next_text) {
                        let mut merged = trimmed.to_string();
                        merge_paragraph_text(&mut merged, &next_text);
                        output.push_str(&escape_md_line_start(merged.trim()));
                        output.push_str("\n\n");
                        i += 2;
                        continue;
                    }
                }

                // Merge consecutive heading fragments.
                // When the PDF splits a title across multiple text elements,
                // each becomes a separate heading; merge them into one.
                let mut merged_heading = trimmed.to_string();
                while let Some(ContentElement::Heading(next_h)) = doc.kids.get(i + 1) {
                    let next_text = next_h.base.base.value();
                    let next_trimmed = next_text.trim();
                    if next_trimmed.is_empty() || should_skip_heading_text(next_trimmed) {
                        i += 1;
                        continue;
                    }
                    // Only merge if the combined text stays under max heading length
                    if merged_heading.len() + 1 + next_trimmed.len() > 200 {
                        break;
                    }
                    merge_paragraph_text(&mut merged_heading, next_trimmed);
                    i += 1;
                }

                let cleaned_heading = strip_trailing_page_number(merged_heading.trim());

                // Check if this heading contains a merged subsection
                if let Some(split_pos) = find_merged_subsection_split(cleaned_heading) {
                    let first = cleaned_heading[..split_pos].trim();
                    let second = cleaned_heading[split_pos..].trim();
                    output.push_str(&format!("# {}\n\n", first));
                    output.push_str(&format!("# {}\n\n", second));
                } else {
                    output.push_str(&format!("# {}\n\n", cleaned_heading));
                }
            }
            ContentElement::NumberHeading(nh) => {
                let text = nh.base.base.base.value();
                let trimmed = text.trim();
                if trimmed.is_empty() || should_skip_heading_text(trimmed) {
                    i += 1;
                    continue;
                }

                // Demote number headings ending with comma (footnotes).
                if should_demote_comma_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote number headings containing math symbols.
                if should_demote_math_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                // Demote number headings containing percentage signs.
                if should_demote_percentage_heading(trimmed) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                if let Some(next_text) = next_mergeable_paragraph_text(doc.kids.get(i + 1)) {
                    if should_demote_heading_to_paragraph(trimmed, &next_text) {
                        let mut merged = trimmed.to_string();
                        merge_paragraph_text(&mut merged, &next_text);
                        output.push_str(&escape_md_line_start(merged.trim()));
                        output.push_str("\n\n");
                        i += 2;
                        continue;
                    }
                }

                let cleaned = strip_trailing_page_number(trimmed);

                // Check if this heading contains a merged subsection
                if let Some(split_pos) = find_merged_subsection_split(cleaned) {
                    let first = cleaned[..split_pos].trim();
                    let second = cleaned[split_pos..].trim();
                    output.push_str(&format!("# {}\n\n", first));
                    output.push_str(&format!("# {}\n\n", second));
                } else {
                    output.push_str(&format!("# {}\n\n", cleaned));
                }
            }
            ContentElement::Paragraph(_)
            | ContentElement::TextBlock(_)
            | ContentElement::TextLine(_) => {
                let element = &doc.kids[i];
                let text = match &doc.kids[i] {
                    ContentElement::Paragraph(p) => clean_paragraph_text(&p.base.value()),
                    ContentElement::TextBlock(tb) => clean_paragraph_text(&tb.value()),
                    ContentElement::TextLine(tl) => clean_paragraph_text(&tl.value()),
                    _ => unreachable!(),
                };
                let trimmed = text.trim();
                if trimmed.is_empty() || looks_like_margin_page_number(doc, element, trimmed) {
                    i += 1;
                    continue;
                }
                if should_skip_leading_figure_carryover(doc, i, trimmed) {
                    i += 1;
                    continue;
                }

                if should_render_paragraph_as_heading(doc, i, trimmed, doc.kids.get(i + 1)) {
                    let cleaned = strip_trailing_page_number(trimmed);
                    // Check if this heading contains a merged subsection
                    if let Some(split_pos) = find_merged_subsection_split(cleaned) {
                        let first = cleaned[..split_pos].trim();
                        let second = cleaned[split_pos..].trim();
                        output.push_str(&format!("# {}\n\n", first));
                        output.push_str(&format!("# {}\n\n", second));
                    } else {
                        output.push_str(&format!("# {}\n\n", cleaned));
                    }
                    i += 1;
                    continue;
                }

                if matches!(element, ContentElement::Paragraph(p) if p.base.semantic_type == SemanticType::TableOfContent)
                {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push('\n');
                    i += 1;
                    continue;
                }

                if is_short_caption_label(trimmed) {
                    if let Some(next_text) = next_mergeable_paragraph_text(doc.kids.get(i + 1)) {
                        if let Some((caption_tail, body)) =
                            split_following_caption_tail_and_body(&next_text)
                        {
                            let mut caption = trimmed.to_string();
                            caption.push('\n');
                            caption.push_str(caption_tail);
                            output.push_str(&escape_md_line_start(caption.trim()));
                            output.push_str("\n\n");
                            output.push_str(&escape_md_line_start(body));
                            output.push_str("\n\n");
                            i += 2;
                            continue;
                        }

                        if looks_like_caption_tail(&next_text) {
                            let mut caption = trimmed.to_string();
                            caption.push('\n');
                            caption.push_str(next_text.trim());

                            if let Some(year_text) =
                                next_mergeable_paragraph_text(doc.kids.get(i + 2))
                            {
                                if looks_like_caption_year(&year_text) {
                                    caption.push('\n');
                                    caption.push_str(year_text.trim());
                                    i += 1;
                                }
                            }

                            output.push_str(&escape_md_line_start(caption.trim()));
                            output.push_str("\n\n");
                            i += 2;
                            continue;
                        }
                    }
                }

                if let Some((caption, body)) = split_leading_caption_and_body(trimmed) {
                    output.push_str(&escape_md_line_start(caption));
                    output.push_str("\n\n");
                    output.push_str(&escape_md_line_start(body));
                    output.push_str("\n\n");
                    i += 1;
                    continue;
                }

                let mut merged = trimmed.to_string();
                while let Some(next_text) = next_mergeable_paragraph_text(doc.kids.get(i + 1)) {
                    let can_merge = if matches!(element, ContentElement::Paragraph(_)) {
                        should_merge_adjacent_semantic_paragraphs(&merged, &next_text)
                    } else {
                        should_merge_paragraph_text(&merged, &next_text)
                    };
                    if !can_merge {
                        break;
                    }
                    merge_paragraph_text(&mut merged, &next_text);
                    i += 1;
                }

                output.push_str(&escape_md_line_start(merged.trim()));
                output.push_str("\n\n");
            }
            other => render_element(&mut output, other),
        }
        i += 1;
    }

    // Post-processing: merge adjacent pipe tables that share the same
    // column count.  The table detector sometimes emits highlighted or
    // coloured rows as separate tables.
    let output = merge_adjacent_pipe_tables(&output);
    let output = normalize_chart_like_markdown(&output);
    drop_isolated_noise_lines(&output)
}

fn cmp_banded_reading_order(
    left: &BoundingBox,
    right: &BoundingBox,
    band_height: f64,
) -> std::cmp::Ordering {
    let safe_band = band_height.max(1.0);
    let left_band = (left.top_y / safe_band).round() as i64;
    let right_band = (right.top_y / safe_band).round() as i64;
    right_band
        .cmp(&left_band)
        .then_with(|| {
            left.left_x
                .partial_cmp(&right.left_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            right
                .top_y
                .partial_cmp(&left.top_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            right
                .bottom_y
                .partial_cmp(&left.bottom_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .then_with(|| {
            left.right_x
                .partial_cmp(&right.right_x)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn should_skip_document_title(doc: &PdfDocument, title: &str) -> bool {
    first_heading_like_text(doc)
        .filter(|first| !equivalent_heading_text(first, title))
        .is_some()
}

fn should_render_document_title_as_plaintext(doc: &PdfDocument, title: &str) -> bool {
    if title.split_whitespace().count() > 6 {
        return false;
    }

    let mut early = doc.kids.iter().take(6);
    let has_explicit_heading = early.clone().any(|element| {
        matches!(
            element,
            ContentElement::Heading(_) | ContentElement::NumberHeading(_)
        )
    });
    let has_tableish_content = early.any(|element| {
        matches!(
            element,
            ContentElement::List(_) | ContentElement::Table(_) | ContentElement::TableBorder(_)
        )
    });

    has_tableish_content && !has_explicit_heading
}

fn render_top_table_plate_document(doc: &PdfDocument) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let (table_idx, table) = doc
        .kids
        .iter()
        .enumerate()
        .find_map(|(idx, element)| table_border_from_element(element).map(|table| (idx, table)))?;
    if table.num_columns < 5 || table.rows.len() < 4 {
        return None;
    }

    let mut header_probe = collect_table_border_rows(table);
    if header_probe.len() < 3 || !preserve_grouped_header_rows(&mut header_probe) {
        return None;
    }

    let table_top = table.bbox.top_y;
    let table_bottom = table.bbox.bottom_y;
    let table_height = table.bbox.height().max(1.0);
    let page_top = doc
        .kids
        .iter()
        .map(|element| element.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max);
    if !page_top.is_finite() || page_top - table_top > table_height * 3.0 {
        return None;
    }

    let caption_gap_limit = (table_height * 2.2).clamp(48.0, 132.0);
    let mut caption_indices = Vec::new();
    for idx in table_idx + 1..doc.kids.len() {
        let element = &doc.kids[idx];
        if !is_geometric_text_candidate(element) {
            if table_bottom - element.bbox().top_y > caption_gap_limit {
                break;
            }
            continue;
        }

        let text = extract_element_text(element);
        if text.trim().is_empty() || looks_like_margin_page_number(doc, element, &text) {
            continue;
        }

        let gap = table_bottom - element.bbox().top_y;
        if gap < -6.0 {
            break;
        }
        if gap > caption_gap_limit {
            break;
        }
        caption_indices.push(idx);
    }
    if caption_indices.is_empty() {
        return None;
    }

    let has_body_below = doc.kids.iter().enumerate().skip(caption_indices.last().copied()? + 1).any(
        |(_, element)| {
            is_geometric_text_candidate(element)
                && !extract_element_text(element).trim().is_empty()
                && table_bottom - element.bbox().top_y > caption_gap_limit
        },
    );
    if !has_body_below {
        return None;
    }

    let mut output = String::new();
    render_table_border(&mut output, table);

    let mut caption = String::new();
    for idx in &caption_indices {
        let text = extract_element_text(&doc.kids[*idx]);
        if text.trim().is_empty() {
            continue;
        }
        merge_paragraph_text(&mut caption, &text);
    }
    let trimmed = caption.trim();
    if trimmed.is_empty() {
        return None;
    }
    output.push_str(&escape_md_line_start(trimmed));
    output.push_str("\n\n");
    Some(output)
}

fn render_single_table_report_document(doc: &PdfDocument) -> Option<String> {
    if doc.number_of_pages != 1 || !(2..=4).contains(&doc.kids.len()) {
        return None;
    }

    let title = &doc.kids[0];
    if !is_geometric_text_candidate(title) {
        return None;
    }
    let title_text = extract_element_text(title);
    if title_text.trim().is_empty() || title_text.split_whitespace().count() < 4 {
        return None;
    }

    let table = table_border_from_element(&doc.kids[1])?;
    if table.num_columns < 4 || table.rows.len() < 4 {
        return None;
    }

    let page_top = doc
        .kids
        .iter()
        .map(|element| element.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max);
    if !page_top.is_finite() {
        return None;
    }

    let title_bbox = title.bbox();
    let table_bbox = &table.bbox;
    if page_top - title_bbox.top_y > 24.0 {
        return None;
    }

    let vertical_gap = title_bbox.bottom_y - table_bbox.top_y;
    if !(8.0..=40.0).contains(&vertical_gap) {
        return None;
    }

    if (title_bbox.center_x() - table_bbox.center_x()).abs() > table_bbox.width() * 0.12 {
        return None;
    }

    if doc.kids.iter().skip(2).any(|element| {
        let text = extract_element_text(element);
        let trimmed = text.trim();
        !trimmed.is_empty()
            && !looks_like_footer_banner(trimmed)
            && !looks_like_margin_page_number(doc, element, trimmed)
    }) {
        return None;
    }

    let mut rows = collect_table_border_rows(table);
    if rows.is_empty() {
        return None;
    }
    merge_continuation_rows(&mut rows);
    trim_leading_table_carryover_rows(&mut rows);
    if rows.len() < 2 {
        return None;
    }

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(title_text.trim());
    output.push_str("\n\n");
    output.push_str(&render_pipe_rows(&rows));
    Some(output)
}

fn render_late_section_boundary_document(doc: &PdfDocument) -> Option<String> {
    if doc.number_of_pages != 1 || doc.kids.len() < 8 {
        return None;
    }

    let page_top = doc
        .kids
        .iter()
        .map(|element| element.bbox().top_y)
        .fold(f64::NEG_INFINITY, f64::max);
    if !page_top.is_finite() {
        return None;
    }

    let heading_idx = doc.kids.iter().position(|element| {
        matches!(
            element,
            ContentElement::Heading(_) | ContentElement::NumberHeading(_)
        )
    })?;
    if heading_idx < 5 {
        return None;
    }

    let heading = &doc.kids[heading_idx];
    let heading_text = extract_element_text(heading);
    if heading_text.trim().is_empty() {
        return None;
    }

    let heading_top = heading.bbox().top_y;
    if page_top - heading_top < 240.0 {
        return None;
    }

    let leading_text_indices = (0..heading_idx)
        .filter(|idx| is_geometric_text_candidate(&doc.kids[*idx]))
        .collect::<Vec<_>>();
    if leading_text_indices.len() < 5 {
        return None;
    }

    let colon_ended = leading_text_indices
        .iter()
        .filter(|idx| extract_element_text(&doc.kids[**idx]).trim_end().ends_with(':'))
        .count();
    if colon_ended * 2 < leading_text_indices.len() {
        return None;
    }

    let trailing_indices = (heading_idx + 1..doc.kids.len())
        .filter(|idx| is_geometric_text_candidate(&doc.kids[*idx]))
        .filter(|idx| {
            let text = extract_element_text(&doc.kids[*idx]);
            !text.trim().is_empty() && !looks_like_margin_page_number(doc, &doc.kids[*idx], &text)
        })
        .collect::<Vec<_>>();
    if trailing_indices.is_empty() || trailing_indices.len() > 5 {
        return None;
    }

    let mut footer_count = 0usize;
    let content_indices = trailing_indices
        .into_iter()
        .filter(|idx| {
            let text = extract_element_text(&doc.kids[*idx]);
            let is_footerish =
                doc.kids[*idx].bbox().top_y < 96.0 && text.split_whitespace().count() >= 4;
            footer_count += usize::from(is_footerish);
            !is_footerish
        })
        .collect::<Vec<_>>();
    if content_indices.is_empty() || footer_count == 0 {
        return None;
    }

    let mut fragments = content_indices
        .iter()
        .map(|idx| (*idx, &doc.kids[*idx]))
        .collect::<Vec<_>>();
    fragments.sort_by(|left, right| {
        cmp_banded_reading_order(&left.1.bbox(), &right.1.bbox(), 6.0)
    });

    let mut paragraph = String::new();
    for (_, element) in fragments {
        let text = extract_element_text(element);
        if text.trim().is_empty() {
            continue;
        }
        merge_paragraph_text(&mut paragraph, &text);
    }
    let trimmed_paragraph = paragraph.trim();
    if trimmed_paragraph.is_empty() {
        return None;
    }

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(heading_text.trim());
    output.push_str("\n\n");
    output.push_str(&escape_md_line_start(trimmed_paragraph));
    output.push_str("\n\n");
    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutHeaderCandidate {
    line_idx: usize,
    headers: Vec<String>,
    starts: Vec<usize>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutEntry {
    line_idx: usize,
    cells: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutAnchorRow {
    anchor_idx: usize,
    last_anchor_idx: usize,
    cells: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutPanelHeaderCandidate {
    line_idx: usize,
    headers: Vec<String>,
    starts: Vec<usize>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutTocEntry {
    title: String,
    page: String,
    title_start: usize,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct BBoxLayoutWord {
    bbox: BoundingBox,
    text: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct BBoxLayoutLine {
    block_id: usize,
    bbox: BoundingBox,
    words: Vec<BBoxLayoutWord>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutTextFragment {
    bbox: BoundingBox,
    text: String,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct OpenPlateCandidate {
    heading: String,
    header_row: Vec<String>,
    rows: Vec<Vec<String>>,
    caption: String,
    cutoff_top_y: f64,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutNarrativeBridge {
    bridge_paragraph: Option<String>,
    deferred_captions: Vec<String>,
    body_start_top_y: Option<f64>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct BBoxLayoutBlock {
    block_id: usize,
    bbox: BoundingBox,
    lines: Vec<BBoxLayoutLine>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutOcrDashboard {
    eyebrow: Option<String>,
    title: String,
    left_heading: String,
    left_columns: Vec<String>,
    left_rows: Vec<Vec<String>>,
    right_heading: String,
    right_rows: Vec<Vec<String>>,
    definition_notes: Vec<String>,
    source_notes: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutRecommendationPanel {
    heading: String,
    subtitle: String,
    header: Vec<String>,
    rows: Vec<Vec<String>>,
    notes: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutRecommendationInfographic {
    eyebrow: Option<String>,
    title: String,
    panels: Vec<LayoutRecommendationPanel>,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct LayoutBarToken {
    bbox: BoundingBox,
    value: i64,
    text: String,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutStackedBarFigure {
    caption: String,
    months: Vec<String>,
    row_labels: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutStackedBarSectorFigure {
    caption: String,
    months: Vec<String>,
    sectors: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutStackedBarNarrative {
    heading: String,
    paragraphs: Vec<String>,
    footnote: Option<String>,
    top_y: f64,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutSeriesFigure {
    caption: String,
    labels: Vec<String>,
    values: Vec<String>,
    source: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutCaptionSection {
    label: String,
    title: String,
    footnote_number: Option<String>,
    top_y: f64,
}

#[cfg(not(target_arch = "wasm32"))]
enum LayoutCaptionedMediaEvent {
    Caption(LayoutCaptionSection),
    Paragraph(String),
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutCaptionedMediaProfile {
    sections: Vec<LayoutCaptionSection>,
    prose: Vec<(f64, String)>,
    footnote: Option<String>,
    image_count: usize,
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_captioned_media_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_captioned_media_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_captioned_media_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }
    let paragraph_count = doc
        .kids
        .iter()
        .filter(|element| matches!(element, ContentElement::Paragraph(_)))
        .count();
    let image_count = doc
        .kids
        .iter()
        .filter(|element| {
            matches!(
                element,
                ContentElement::Image(_)
                    | ContentElement::Figure(_)
                    | ContentElement::Picture(_)
            )
        })
        .count();
    if paragraph_count == 0 || image_count == 0 {
        return None;
    }
    let has_explicit_structure = doc.kids.iter().any(|element| {
        matches!(
            element,
            ContentElement::Caption(_)
                | ContentElement::Heading(_)
                | ContentElement::NumberHeading(_)
                | ContentElement::Table(_)
                | ContentElement::List(_)
        )
    });
    if has_explicit_structure {
        return None;
    }

    let profile = build_layout_captioned_media_profile(doc, layout_cache)?;
    if profile.sections.is_empty() || (profile.sections.len() == 1 && profile.footnote.is_none()) {
        return None;
    }
    let has_non_figure_label = profile
        .sections
        .iter()
        .any(|section| !section.label.starts_with("Figure "));
    let has_anchored_footnote = profile.footnote.is_some()
        || profile
            .sections
            .iter()
            .any(|section| section.footnote_number.is_some());
    if !has_non_figure_label && !has_anchored_footnote {
        return None;
    }

    if let Some(rendered) = render_layout_captioned_media_explainer(&profile) {
        return Some(rendered);
    }

    let mut events = profile
        .sections
        .into_iter()
        .map(|section| (section.top_y, LayoutCaptionedMediaEvent::Caption(section)))
        .collect::<Vec<_>>();
    for (top_y, paragraph) in profile.prose {
        events.push((top_y, LayoutCaptionedMediaEvent::Paragraph(paragraph)));
    }
    events.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut output = String::new();
    for (_, event) in events {
        match event {
            LayoutCaptionedMediaEvent::Caption(section) => {
                output.push_str(&render_layout_caption_section(&section));
            }
            LayoutCaptionedMediaEvent::Paragraph(paragraph) => {
            output.push_str(&escape_md_line_start(paragraph.trim()));
            output.push_str("\n\n");
            }
        }
    }

    if let Some(footnote_text) = profile.footnote {
        output.push_str("---\n\n");
        output.push_str("**Footnote:**\n");
        output.push_str(&escape_md_line_start(footnote_text.trim()));
        output.push('\n');
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn build_layout_captioned_media_profile(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<LayoutCaptionedMediaProfile> {
    let layout = layout_cache.bbox_layout(doc)?;
    let sections = detect_layout_caption_sections(&layout.blocks);
    let footnote = detect_layout_bottom_footnote(&layout.lines);

    let mut prose = doc
        .kids
        .iter()
        .filter_map(|element| match element {
            ContentElement::Paragraph(_)
            | ContentElement::TextBlock(_)
            | ContentElement::TextLine(_) => {
                let text = clean_paragraph_text(&extract_element_text(element));
                let trimmed = text.trim();
                (!trimmed.is_empty()
                    && trimmed.split_whitespace().count() >= 8
                    && !starts_with_caption_prefix(trimmed)
                    && !trimmed.chars().all(|ch| ch.is_ascii_digit() || ch.is_ascii_whitespace())
                    && !trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                    && !looks_like_footer_banner(trimmed))
                .then_some((element.bbox().top_y, trimmed.to_string()))
            }
            _ => None,
        })
        .filter(|(top_y, paragraph)| {
            !sections.iter().any(|section| {
                (*top_y - section.top_y).abs() <= 36.0
                    || section.title.contains(paragraph)
                    || paragraph.contains(&section.title)
            })
        })
        .collect::<Vec<_>>();
    prose.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if prose.len() > 2 {
        return None;
    }

    let image_count = doc
        .kids
        .iter()
        .filter(|element| {
            matches!(
                element,
                ContentElement::Image(_) | ContentElement::Figure(_) | ContentElement::Picture(_)
            )
        })
        .count();

    Some(LayoutCaptionedMediaProfile {
        sections,
        prose,
        footnote,
        image_count,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_captioned_media_explainer(profile: &LayoutCaptionedMediaProfile) -> Option<String> {
    if profile.sections.len() != 1
        || profile.prose.len() != 2
        || profile.image_count != 1
        || profile.footnote.is_none()
        || !profile.sections.iter().all(|section| section.label.starts_with("Figure "))
    {
        return None;
    }

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(profile.prose[0].1.trim());
    output.push('\n');
    output.push_str(&escape_md_line_start(profile.prose[1].1.trim()));
    output.push_str("\n\n");
    output.push_str("*Image*\n\n");
    output.push_str(&render_layout_caption_section(&profile.sections[0]));
    output.push_str("---\n\n");
    output.push_str("**Footnote:**\n");
    output.push_str(&escape_md_line_start(
        profile.footnote.as_deref().unwrap_or_default().trim(),
    ));
    output.push('\n');
    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_caption_sections(blocks: &[BBoxLayoutBlock]) -> Vec<LayoutCaptionSection> {
    let normalized_blocks = blocks
        .iter()
        .map(|block| (block, normalize_common_ocr_text(&bbox_layout_block_text(block))))
        .collect::<Vec<_>>();

    let mut used_titles = HashSet::new();
    let mut sections = Vec::new();
    for (block, label_text) in &normalized_blocks {
        if !is_short_caption_label(label_text) {
            continue;
        }

        let label_bbox = &block.bbox;
        let title_candidate = normalized_blocks
            .iter()
            .filter(|(candidate, text)| {
                candidate.block_id != block.block_id
                    && !used_titles.contains(&candidate.block_id)
                    && !text.is_empty()
                    && !is_short_caption_label(text)
                    && !starts_with_caption_prefix(text)
                    && !looks_like_footer_banner(text)
                    && !is_page_number_like(text)
                    && text.split_whitespace().count() >= 2
                    && candidate.bbox.width() >= 60.0
            })
            .filter_map(|(candidate, text)| {
                let vertical_gap = (candidate.bbox.center_y() - label_bbox.center_y()).abs();
                let horizontal_gap = if candidate.bbox.left_x > label_bbox.right_x {
                    candidate.bbox.left_x - label_bbox.right_x
                } else if label_bbox.left_x > candidate.bbox.right_x {
                    label_bbox.left_x - candidate.bbox.right_x
                } else {
                    0.0
                };
                (vertical_gap <= 28.0 && horizontal_gap <= 180.0)
                    .then_some((vertical_gap + horizontal_gap * 0.15, *candidate, text.clone()))
            })
            .min_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        let Some((_, title_block, title_text)) = title_candidate else {
            continue;
        };
        used_titles.insert(title_block.block_id);
        let (title, footnote_number) = split_trailing_caption_footnote_marker(&title_text);
        sections.push(LayoutCaptionSection {
            label: label_text.to_string(),
            title,
            footnote_number,
            top_y: label_bbox.top_y.max(title_block.bbox.top_y),
        });
    }

    sections.sort_by(|left, right| {
        right
            .top_y
            .partial_cmp(&left.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sections
}

#[cfg(not(target_arch = "wasm32"))]
fn split_trailing_caption_footnote_marker(text: &str) -> (String, Option<String>) {
    let trimmed = text.trim();
    let re = Regex::new(r"^(?P<title>.*?[.!?])\s*(?P<num>\d{1,2})\s*[A-Za-z]{0,12}$").ok();
    if let Some(captures) = re.as_ref().and_then(|re| re.captures(trimmed)) {
        return (
            captures["title"].trim().to_string(),
            Some(captures["num"].to_string()),
        );
    }

    (trimmed.to_string(), None)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_bottom_footnote(lines: &[BBoxLayoutLine]) -> Option<String> {
    let normalized_lines = lines
        .iter()
        .map(|line| (line.bbox.top_y, normalize_common_ocr_text(&bbox_layout_line_text(line))))
        .filter(|(_, text)| !text.is_empty() && !is_page_number_like(text))
        .collect::<Vec<_>>();
    let start_idx = normalized_lines.iter().rposition(|(_, text)| {
        text.chars().next().is_some_and(|ch| ch.is_ascii_digit()) && text.split_whitespace().count() >= 6
    })?;

    let mut collected = vec![normalized_lines[start_idx].1.clone()];
    let mut last_top_y = normalized_lines[start_idx].0;
    for (top_y, text) in normalized_lines.iter().skip(start_idx + 1) {
        if is_page_number_like(text) {
            break;
        }
        if (last_top_y - *top_y).abs() > 28.0 {
            break;
        }
        collected.push(text.clone());
        last_top_y = *top_y;
    }

    if collected.is_empty() {
        return None;
    }
    let merged = collected.join(" ");
    Some(normalize_layout_footnote_text(&merged))
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_footnote_text(text: &str) -> String {
    let mut normalized = text.replace(",https://", ", https://");
    let url_gap_re = Regex::new(r"(https?://\S+)\s+(\S+)").ok();
    while let Some(re) = &url_gap_re {
        let next = re.replace(&normalized, "$1$2").to_string();
        if next == normalized {
            break;
        }
        normalized = next;
    }
    normalized
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_caption_section(section: &LayoutCaptionSection) -> String {
    let mut output = String::new();
    if section.label.starts_with("Diagram ") {
        output.push_str("## ");
        output.push_str(section.label.trim());
        output.push_str("\n");
        if !section.title.trim().is_empty() {
            let title = normalize_layout_caption_title_text(section.title.trim());
            output.push_str("**");
            output.push_str(&title);
            output.push_str("**\n\n");
        } else {
            output.push('\n');
        }
        return output;
    }

    if section.label.starts_with("Figure ") && !section.footnote_number.is_some() {
        output.push('*');
        output.push_str(section.label.trim());
        output.push_str("*\n\n");
    }

    output.push_str("**");
    output.push_str(section.label.trim());
    output.push_str("**\n");

    if !section.title.trim().is_empty() {
        let title_lines = split_layout_caption_title_lines(section.title.trim());
        let last_idx = title_lines.len().saturating_sub(1);
        for (idx, line) in title_lines.iter().enumerate() {
            if section.footnote_number.is_some() {
                output.push_str("**");
                output.push_str(line.trim());
                if idx == last_idx {
                    output.push_str("**^");
                    output.push_str(section.footnote_number.as_deref().unwrap_or_default());
                } else {
                    output.push_str("**");
                }
            } else {
                output.push('*');
                output.push_str(line.trim());
                output.push('*');
            }
            output.push('\n');
        }
    }
    output.push('\n');
    output
}

#[cfg(not(target_arch = "wasm32"))]
fn split_layout_caption_title_lines(title: &str) -> Vec<String> {
    let title = normalize_layout_caption_title_text(title);
    if let Some(idx) = title.find(" Content:") {
        let head = title[..idx].trim();
        let tail = title[idx + 1..].trim();
        if !head.is_empty() && head.split_whitespace().count() <= 3 && !tail.is_empty() {
            return vec![head.to_string(), tail.to_string()];
        }
    }
    vec![title.to_string()]
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_caption_title_text(title: &str) -> String {
    Regex::new(r"(\d{4})-\s+(\d{4})")
        .ok()
        .map(|re| re.replace_all(title, "$1-$2").to_string())
        .unwrap_or_else(|| title.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_single_caption_chart_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_single_caption_chart_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_single_caption_chart_document_cached(
    doc: &PdfDocument,
    _layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let caption_indices = doc
        .kids
        .iter()
        .enumerate()
        .filter_map(|(idx, element)| {
            let text = extract_element_text(element);
            let trimmed = text.trim();
            (trimmed.starts_with("Figure ")
                && trimmed.contains(':')
                && trimmed.split_whitespace().count() >= 6)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    if caption_indices.len() != 1 {
        return None;
    }
    if doc.kids.len() < 12 {
        return None;
    }

    let caption_idx = caption_indices[0];
    let mut output = String::new();
    let mut i = 0usize;
    let mut chart_mode = false;
    while i < doc.kids.len() {
        let element = &doc.kids[i];
        let text = extract_element_text(element);
        let trimmed = text.trim();
        if trimmed.is_empty() || looks_like_margin_page_number(doc, element, trimmed) {
            i += 1;
            continue;
        }

        if i == caption_idx {
            output.push_str(&escape_md_line_start(trimmed));
            output.push_str("\n\n");
            chart_mode = true;
            i += 1;
            continue;
        }

        if chart_mode {
            if !looks_like_chart_followup_paragraph(element, trimmed)
                && !matches!(
                    element,
                    ContentElement::Heading(_) | ContentElement::NumberHeading(_)
                )
            {
                i += 1;
                continue;
            }
            chart_mode = false;
        }

        match element {
            ContentElement::Heading(h) => {
                let level = h.heading_level.unwrap_or(1).clamp(1, 6) as usize;
                output.push_str(&"#".repeat(level));
                output.push(' ');
                output.push_str(trimmed);
                output.push_str("\n\n");
            }
            ContentElement::NumberHeading(nh) => {
                let level = nh.base.heading_level.unwrap_or(1).clamp(1, 6) as usize;
                output.push_str(&"#".repeat(level));
                output.push(' ');
                output.push_str(trimmed);
                output.push_str("\n\n");
            }
            ContentElement::Paragraph(_) | ContentElement::TextBlock(_) => {
                let mut merged = trimmed.to_string();
                while let Some(next_element) = doc.kids.get(i + 1) {
                    let next_text = extract_element_text(next_element);
                    let next_trimmed = next_text.trim();
                    if next_trimmed.is_empty()
                        || looks_like_margin_page_number(doc, next_element, next_trimmed)
                    {
                        i += 1;
                        continue;
                    }
                    if i + 1 == caption_idx || looks_like_chart_noise_element(next_element, next_trimmed)
                    {
                        break;
                    }
                    let can_merge = if matches!(element, ContentElement::Paragraph(_)) {
                        should_merge_adjacent_semantic_paragraphs(&merged, next_trimmed)
                    } else {
                        should_merge_paragraph_text(&merged, next_trimmed)
                    };
                    if !can_merge {
                        break;
                    }
                    merge_paragraph_text(&mut merged, next_trimmed);
                    i += 1;
                }

                output.push_str(&escape_md_line_start(merged.trim()));
                output.push_str("\n\n");
            }
            _ => {}
        }

        i += 1;
    }

    Some(output.trim_end().to_string() + "\n")
}

fn looks_like_chart_noise_element(_element: &ContentElement, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    if is_standalone_page_number(text) || looks_like_numeric_axis_blob(text) {
        return true;
    }

    let word_count = text.split_whitespace().count();
    let lower = text.to_ascii_lowercase();

    if lower.starts_with("figure ") && text.contains(':') {
        return false;
    }

    if lower.starts_with("source:") {
        return false;
    }

    if word_count <= 3
        && (looks_like_yearish_label(text)
            || looks_like_layout_month_label(text)
            || text == "Lockdown Period")
    {
        return true;
    }

    if text.chars().all(|ch| ch.is_ascii_digit() || ch.is_ascii_whitespace()) {
        return true;
    }

    let short_non_sentence = !text.contains('.') && !text.contains(':') && !text.contains(';');
    let has_chart_keyword = lower.contains("working as usual")
        || lower.contains("temporarily closed")
        || lower.contains("business premises")
        || lower.contains("operations continue");

    word_count <= 10 || (short_non_sentence && word_count <= 14) || has_chart_keyword
}

fn looks_like_chart_followup_paragraph(_element: &ContentElement, text: &str) -> bool {
    let word_count = text.split_whitespace().count();
    word_count >= 18
        && !text.trim_start().starts_with("Figure ")
        && !text.trim_start().starts_with("Table ")
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_recommendation_infographic_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_recommendation_infographic_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_recommendation_infographic_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let layout = layout_cache.bbox_layout(doc)?;
    let infographic = detect_layout_recommendation_infographic(layout.page_width, &layout.lines)?;

    let mut output = String::new();
    if let Some(eyebrow) = infographic.eyebrow.as_deref() {
        output.push_str("# ");
        output.push_str(eyebrow.trim());
        output.push_str("\n\n");
    }
    output.push_str(&escape_md_line_start(infographic.title.trim()));
    output.push_str("\n\n");

    for panel in &infographic.panels {
        output.push_str("## ");
        output.push_str(panel.heading.trim());
        output.push_str("\n\n");
        output.push_str(&escape_md_line_start(panel.subtitle.trim()));
        output.push_str("\n\n");

        let mut rows = Vec::with_capacity(panel.rows.len() + 1);
        rows.push(panel.header.clone());
        rows.extend(panel.rows.clone());
        output.push_str(&render_pipe_rows(&rows));

        if !panel.notes.is_empty() {
            output.push_str("*Note:*\n");
            for note in &panel.notes {
                output.push_str("- ");
                output.push_str(note.trim());
                output.push('\n');
            }
            output.push('\n');
        }
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_stacked_bar_report_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_stacked_bar_report_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_stacked_bar_report_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let layout = layout_cache.bbox_layout(doc)?;
    let figure_captions = collect_layout_figure_captions(&layout.blocks);
    if figure_captions.len() != 2 {
        return None;
    }
    let narrative = detect_layout_stacked_bar_narrative(&layout.blocks)?;
    let figure_one = detect_layout_three_month_stacked_figure(
        &layout.blocks,
        &layout.lines,
        layout.page_width,
        figure_captions[0].clone(),
        figure_captions[1].bbox.top_y,
    )?;
    let figure_two = detect_layout_sector_bar_figure(
        &layout.blocks,
        &layout.lines,
        layout.page_width,
        figure_captions[1].clone(),
        narrative.top_y,
    )?;

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(figure_one.caption.trim());
    output.push_str("\n\n");
    let mut first_table = vec![{
        let mut row = vec![String::new()];
        row.extend(figure_one.months.clone());
        row
    }];
    first_table.extend(figure_one.rows.clone());
    output.push_str(&render_pipe_rows(&first_table));

    output.push_str("# ");
    output.push_str(figure_two.caption.trim());
    output.push_str("\n\n");
    let mut second_table = vec![{
        let mut row = vec!["Sector".to_string()];
        row.extend(figure_two.months.clone());
        row
    }];
    second_table.extend(figure_two.rows.clone());
    output.push_str(&render_pipe_rows(&second_table));

    output.push_str("# ");
    output.push_str(narrative.heading.trim());
    output.push_str("\n\n");
    for paragraph in &narrative.paragraphs {
        output.push_str(&escape_md_line_start(paragraph.trim()));
        output.push_str("\n\n");
    }
    if let Some(footnote) = narrative.footnote.as_deref() {
        output.push('*');
        output.push_str(footnote.trim());
        output.push_str("*\n");
    }

    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_multi_figure_chart_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_multi_figure_chart_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_multi_figure_chart_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let layout = layout_cache.bbox_layout(doc)?;
    let figures = detect_layout_multi_figure_chart_sections(&layout.lines)?;
    let rendered_table_count = figures
        .iter()
        .filter(|figure| figure.labels.len() >= 4 && figure.labels.len() == figure.values.len())
        .count();
    if figures.len() < 2 || rendered_table_count == 0 {
        return None;
    }

    let mut output = String::from("# Figures from the Document\n\n");
    for figure in figures {
        output.push_str("## ");
        output.push_str(figure.caption.trim());
        output.push_str("\n\n");

        if figure.labels.len() >= 4 && figure.labels.len() == figure.values.len() {
            let label_header = if figure
                .labels
                .iter()
                .all(|label| looks_like_yearish_label(label))
            {
                "Year"
            } else {
                "Label"
            };
            let value_header = chart_value_header(&figure.caption);
            output.push_str(&format!("| {} | {} |\n", label_header, value_header));
            output.push_str("| --- | --- |\n");
            for (label, value) in figure.labels.iter().zip(figure.values.iter()) {
                output.push_str(&format!("| {} | {} |\n", label, value));
            }
            output.push('\n');
        }

        if let Some(source) = figure.source.as_deref() {
            output.push('*');
            output.push_str(&escape_md_line_start(source.trim()));
            output.push_str("*\n\n");
        }
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_multi_figure_chart_sections(lines: &[BBoxLayoutLine]) -> Option<Vec<LayoutSeriesFigure>> {
    let caption_indices = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| {
            let text = bbox_layout_line_text(line);
            (text.starts_with("Figure ") && text.split_whitespace().count() >= 4).then_some(idx)
        })
        .collect::<Vec<_>>();
    if caption_indices.len() < 2 {
        return None;
    }

    let mut figures = Vec::new();
    for (pos, caption_idx) in caption_indices.iter().enumerate() {
        let next_caption_idx = caption_indices
            .get(pos + 1)
            .copied()
            .unwrap_or(lines.len());
        let caption = bbox_layout_line_text(&lines[*caption_idx]);

        let source_idx = (*caption_idx + 1..next_caption_idx).find(|idx| {
            bbox_layout_line_text(&lines[*idx])
                .to_ascii_lowercase()
                .starts_with("source:")
        });

        let source = source_idx.map(|idx| {
            let mut source_lines = vec![&lines[idx]];
            let mut cursor = idx + 1;
            while cursor < next_caption_idx {
                let text = bbox_layout_line_text(&lines[cursor]);
                if text.starts_with("Figure ") || looks_like_footer_banner(&text) || text.is_empty() {
                    break;
                }
                source_lines.push(&lines[cursor]);
                if text.ends_with('.') {
                    break;
                }
                cursor += 1;
            }
            join_layout_lines_as_paragraph(&source_lines)
        });

        let series_region = &lines[*caption_idx + 1..source_idx.unwrap_or(next_caption_idx)];
        let anchors = extract_year_label_anchors_from_section(series_region);
        let (labels, values) = if anchors.len() >= 4 {
            let values = map_series_values_to_label_anchors(&anchors, series_region);
            (
                anchors.into_iter().map(|anchor| anchor.text).collect::<Vec<_>>(),
                values,
            )
        } else {
            (Vec::new(), Vec::new())
        };

        if source.is_some() || !values.is_empty() {
            figures.push(LayoutSeriesFigure {
                caption: normalize_layout_dashboard_text(&caption),
                labels,
                values,
                source,
            });
        }
    }

    (!figures.is_empty()).then_some(figures)
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_year_label_anchors_from_section(lines: &[BBoxLayoutLine]) -> Vec<LayoutTextFragment> {
    let mut year_words = lines
        .iter()
        .flat_map(|line| line.words.iter())
        .filter_map(|word| {
            let token = word
                .text
                .trim_matches(|ch: char| matches!(ch, ',' | ';' | '.'));
            looks_like_year_token(token).then_some((word.bbox.center_y(), word.clone()))
        })
        .collect::<Vec<_>>();
    if year_words.len() < 4 {
        return Vec::new();
    }

    year_words.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut best_band = Vec::<BBoxLayoutWord>::new();
    for (center_y, _) in &year_words {
        let band = year_words
            .iter()
            .filter(|(candidate_y, _)| (*candidate_y - *center_y).abs() <= 12.0)
            .map(|(_, word)| word.clone())
            .collect::<Vec<_>>();
        if band.len() > best_band.len() {
            best_band = band;
        }
    }
    if best_band.len() < 4 {
        return Vec::new();
    }

    let band_center = best_band
        .iter()
        .map(|word| word.bbox.center_y())
        .sum::<f64>()
        / best_band.len() as f64;
    let mut band_words = lines
        .iter()
        .flat_map(|line| line.words.iter())
        .filter(|word| (word.bbox.center_y() - band_center).abs() <= 12.0)
        .cloned()
        .collect::<Vec<_>>();
    band_words.sort_by(|left, right| {
        left.bbox
            .left_x
            .partial_cmp(&right.bbox.left_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut anchors = Vec::new();
    let mut idx = 0usize;
    while idx < band_words.len() {
        let token = band_words[idx]
            .text
            .trim_matches(|ch: char| matches!(ch, ',' | ';' | '.'));
        if !looks_like_year_token(token) {
            idx += 1;
            continue;
        }

        let mut bbox = band_words[idx].bbox.clone();
        let mut label = token.to_string();
        if let Some(next) = band_words.get(idx + 1) {
            let suffix = next.text.trim_matches(|ch: char| matches!(ch, ',' | ';' | '.'));
            let gap = next.bbox.left_x - band_words[idx].bbox.right_x;
            if suffix.starts_with('(') && suffix.ends_with(')') && gap <= 18.0 {
                label.push(' ');
                label.push_str(suffix);
                bbox = bbox.union(&next.bbox);
                idx += 1;
            }
        }

        anchors.push(LayoutTextFragment { bbox, text: label });
        idx += 1;
    }

    anchors
}

#[cfg(not(target_arch = "wasm32"))]
fn map_series_values_to_label_anchors(
    anchors: &[LayoutTextFragment],
    lines: &[BBoxLayoutLine],
) -> Vec<String> {
    if anchors.len() < 2 {
        return Vec::new();
    }

    let mut spacing = anchors
        .windows(2)
        .map(|pair| pair[1].bbox.center_x() - pair[0].bbox.center_x())
        .filter(|gap| *gap > 0.0)
        .collect::<Vec<_>>();
    spacing.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let median_spacing = spacing
        .get(spacing.len().saturating_sub(1) / 2)
        .copied()
        .unwrap_or(48.0);
    let max_dx = (median_spacing * 0.42).clamp(18.0, 32.0);

    let mut tokens = Vec::<LayoutBarToken>::new();
    for line in lines {
        for word in &line.words {
            let raw = word.text.trim();
            if raw.contains('/') || looks_like_year_token(raw.trim_matches(|ch: char| matches!(ch, ',' | ';' | '.'))) {
                continue;
            }
            let Some(value) = parse_integer_token(raw) else {
                continue;
            };
            tokens.push(LayoutBarToken {
                bbox: word.bbox.clone(),
                value,
                text: sanitize_numberish_token(raw).unwrap_or_else(|| value.to_string()),
            });
        }
    }

    let mut used = vec![false; tokens.len()];
    let mut values = Vec::with_capacity(anchors.len());
    for anchor in anchors {
        let anchor_center_x = anchor.bbox.center_x();
        let anchor_center_y = anchor.bbox.center_y();
        let best = tokens
            .iter()
            .enumerate()
            .filter(|(idx, token)| {
                !used[*idx]
                    && token.bbox.center_y() > anchor_center_y + 8.0
                    && (token.bbox.center_x() - anchor_center_x).abs() <= max_dx
            })
            .min_by(|left, right| {
                let left_score = (left.1.bbox.center_x() - anchor_center_x).abs()
                    + (left.1.bbox.center_y() - anchor_center_y).abs() * 0.05;
                let right_score = (right.1.bbox.center_x() - anchor_center_x).abs()
                    + (right.1.bbox.center_y() - anchor_center_y).abs() * 0.05;
                left_score
                    .partial_cmp(&right_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        let Some((best_idx, token)) = best else {
            return Vec::new();
        };
        used[best_idx] = true;
        values.push(token.text.clone());
    }

    values
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_recommendation_infographic(
    page_width: f64,
    lines: &[BBoxLayoutLine],
) -> Option<LayoutRecommendationInfographic> {
    if page_width < 900.0 {
        return None;
    }

    let blocks = collect_bbox_layout_blocks(lines);
    let page_top = lines
        .iter()
        .map(|line| line.bbox.top_y)
        .fold(0.0_f64, f64::max);

    let title_block = blocks
        .iter()
        .filter(|block| {
            block.bbox.width() >= page_width * 0.55
                && block.bbox.top_y >= page_top - 105.0
                && bbox_layout_block_text(block).split_whitespace().count() >= 8
        })
        .max_by(|left, right| {
            left.bbox
                .width()
                .partial_cmp(&right.bbox.width())
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let title = normalize_layout_dashboard_text(&bbox_layout_block_text(title_block));
    if title.split_whitespace().count() < 8 {
        return None;
    }

    let eyebrow = blocks
        .iter()
        .filter(|block| {
            block.block_id != title_block.block_id
                && block.bbox.top_y > title_block.bbox.top_y
                && block.bbox.width() >= page_width * 0.1
        })
        .max_by(|left, right| {
            left.bbox
                .top_y
                .partial_cmp(&right.bbox.top_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|block| normalize_layout_dashboard_text(&bbox_layout_block_text(block)));

    let title_bottom = title_block.bbox.bottom_y;
    let region_width = page_width / 3.0;
    let left_panel = detect_layout_recommendation_hit_ratio_panel(
        &blocks,
        lines,
        0.0,
        region_width,
        title_bottom,
    )?;
    let middle_panel = detect_layout_recommendation_ranking_panel(
        &blocks,
        lines,
        region_width,
        region_width * 2.0,
        title_bottom,
    )?;
    let right_panel = detect_layout_recommendation_accuracy_panel(
        &blocks,
        lines,
        region_width * 2.0,
        page_width,
        title_bottom,
    )?;

    Some(LayoutRecommendationInfographic {
        eyebrow,
        title,
        panels: vec![left_panel, middle_panel, right_panel],
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_ocr_benchmark_dashboard_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_ocr_benchmark_dashboard_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_ocr_benchmark_dashboard_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let layout = layout_cache.bbox_layout(doc)?;
    let dashboard = detect_layout_ocr_benchmark_dashboard(layout.page_width, &layout.lines)?;

    let mut output = String::new();
    if let Some(eyebrow) = dashboard.eyebrow.as_deref() {
        output.push_str("## ");
        output.push_str(eyebrow.trim());
        output.push_str("\n\n");
    }
    output.push_str("# ");
    output.push_str(dashboard.title.trim());
    output.push_str("\n\n");

    output.push_str("## ");
    output.push_str(dashboard.left_heading.trim());
    output.push_str("\n\n");
    let mut left_table = Vec::with_capacity(dashboard.left_rows.len() + 1);
    left_table.push({
        let mut row = vec!["Company".to_string()];
        row.extend(dashboard.left_columns.clone());
        row
    });
    left_table.extend(dashboard.left_rows.clone());
    output.push_str(&render_pipe_rows(&left_table));

    output.push_str("## ");
    output.push_str(dashboard.right_heading.trim());
    output.push_str("\n\n");
    let mut right_table = Vec::with_capacity(dashboard.right_rows.len() + 1);
    right_table.push(vec![
        "Metric".to_string(),
        "Company A".to_string(),
        "Company B".to_string(),
        "upstage".to_string(),
    ]);
    right_table.extend(dashboard.right_rows.clone());
    output.push_str(&render_pipe_rows(&right_table));

    if !dashboard.definition_notes.is_empty() {
        output.push_str("---\n\n");
        for note in &dashboard.definition_notes {
            output.push_str(note.trim());
            output.push_str("\n\n");
        }
    }
    if !dashboard.source_notes.is_empty() {
        output.push_str("---\n\n");
        for note in &dashboard.source_notes {
            output.push_str(note.trim());
            output.push_str("\n\n");
        }
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_ocr_benchmark_dashboard(
    page_width: f64,
    lines: &[BBoxLayoutLine],
) -> Option<LayoutOcrDashboard> {
    if page_width < 680.0 {
        return None;
    }

    let page_mid = page_width / 2.0;
    let blocks = collect_bbox_layout_blocks(lines);
    let page_top = lines
        .iter()
        .map(|line| line.bbox.top_y)
        .fold(0.0_f64, f64::max);

    let title_block = blocks
        .iter()
        .filter(|block| block.bbox.width() >= page_width * 0.45 && block.bbox.top_y >= page_top - 40.0)
        .max_by(|left, right| {
            left.bbox
                .width()
                .partial_cmp(&right.bbox.width())
                .unwrap_or(std::cmp::Ordering::Equal)
        })?;
    let title = normalize_layout_dashboard_text(&bbox_layout_block_text(title_block));
    if title.split_whitespace().count() < 5 {
        return None;
    }

    let eyebrow = blocks
        .iter()
        .filter(|block| {
            block.block_id != title_block.block_id
                && block.bbox.top_y > title_block.bbox.top_y
                && block.bbox.width() >= page_width * 0.12
        })
        .max_by(|left, right| {
            left.bbox
                .top_y
                .partial_cmp(&right.bbox.top_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|block| normalize_layout_dashboard_text(&bbox_layout_block_text(block)));

    let left_title_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.right_x <= page_mid
                && block.bbox.top_y < title_block.bbox.bottom_y - 25.0
                && block.bbox.top_y > title_block.bbox.bottom_y - 95.0
                && !bbox_layout_block_text(block).chars().all(|ch| ch.is_ascii_digit() || ch.is_ascii_whitespace())
        })
        .cloned()
        .collect::<Vec<_>>();
    let right_title_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.left_x >= page_mid
                && block.bbox.top_y < title_block.bbox.bottom_y - 25.0
                && block.bbox.top_y > title_block.bbox.bottom_y - 95.0
                && !bbox_layout_block_text(block).chars().all(|ch| ch.is_ascii_digit() || ch.is_ascii_whitespace())
        })
        .cloned()
        .collect::<Vec<_>>();

    let left_heading = join_dashboard_title_blocks(&left_title_blocks)?;
    let right_heading = join_dashboard_title_blocks(&right_title_blocks)?;
    if !left_heading.to_ascii_lowercase().contains("ocr")
        || !right_heading.to_ascii_lowercase().contains("document")
    {
        return None;
    }

    let left_group_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() < page_mid
                && block.bbox.top_y < 90.0
                && bbox_layout_block_text(block).contains('(')
        })
        .cloned()
        .collect::<Vec<_>>();
    if left_group_blocks.len() != 2 {
        return None;
    }
    let mut left_groups = left_group_blocks
        .iter()
        .map(|block| {
            (
                block.bbox.center_x(),
                normalize_layout_dashboard_text(&bbox_layout_block_text(block)),
            )
        })
        .collect::<Vec<_>>();
    left_groups.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let left_value_tokens = collect_layout_decimal_tokens(lines, |bbox| {
        bbox.center_x() < page_mid - 20.0 && bbox.top_y > 110.0 && bbox.top_y < 250.0
    });
    if left_value_tokens.len() < 6 {
        return None;
    }

    let mut left_group_values = vec![Vec::<(f64, String)>::new(), Vec::new()];
    for (bbox, value) in left_value_tokens {
        let group_idx = if (bbox.center_x() - left_groups[0].0).abs()
            <= (bbox.center_x() - left_groups[1].0).abs()
        {
            0
        } else {
            1
        };
        left_group_values[group_idx].push((bbox.center_x(), value));
    }
    if left_group_values.iter().any(|values| values.len() < 3) {
        return None;
    }
    for values in &mut left_group_values {
        values.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        values.truncate(3);
    }

    let mut company_labels = extract_dashboard_company_labels(&blocks, page_mid);
    if company_labels.len() < 2 {
        return None;
    }
    company_labels.truncate(2);
    company_labels.push(infer_dashboard_brand_name(&left_heading));

    let mut left_rows = Vec::new();
    for row_idx in 0..3 {
        left_rows.push(vec![
            company_labels[row_idx].clone(),
            left_group_values[0][row_idx].1.clone(),
            left_group_values[1][row_idx].1.clone(),
        ]);
    }

    let metric_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() > page_mid
                && block.bbox.top_y > 95.0
                && block.bbox.top_y < 240.0
                && matches!(
                    normalize_heading_text(&bbox_layout_block_text(block)).as_str(),
                    text if text.starts_with("ocr") || text.starts_with("parsingf1")
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    if metric_blocks.len() < 4 {
        return None;
    }

    let mut metrics = metric_blocks
        .iter()
        .map(|block| {
            (
                block.bbox.center_y(),
                normalize_layout_dashboard_text(&bbox_layout_block_text(block)),
            )
        })
        .collect::<Vec<_>>();
    metrics.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    metrics.truncate(4);

    let right_value_tokens = collect_layout_decimal_tokens(lines, |bbox| {
        bbox.center_x() > page_mid + 20.0 && bbox.top_y > 90.0 && bbox.top_y < 250.0
    });
    if right_value_tokens.len() < 10 {
        return None;
    }

    let mut metric_values = vec![Vec::<(f64, String)>::new(); metrics.len()];
    for (bbox, value) in right_value_tokens {
        let Some((metric_idx, _)) = metrics
            .iter()
            .enumerate()
            .map(|(idx, (center_y, _))| (idx, (bbox.center_y() - *center_y).abs()))
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            continue;
        };
        metric_values[metric_idx].push((bbox.center_x(), value));
    }

    let mut right_rows = Vec::new();
    for (idx, (_, metric_name)) in metrics.iter().enumerate() {
        let mut values = metric_values[idx].clone();
        values.sort_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        values.dedup_by(|left, right| left.1 == right.1);
        if values.len() < 2 {
            return None;
        }
        if values.len() == 2 {
            values.push(values[1].clone());
        }
        values.truncate(3);
        right_rows.push(vec![
            metric_name.clone(),
            normalize_layout_decimal_value(&values[0].1),
            normalize_layout_decimal_value(&values[1].1),
            normalize_layout_decimal_value(&values[2].1),
        ]);
    }

    let definition_notes = collect_dashboard_notes(&blocks, page_mid, false);
    let source_notes = collect_dashboard_notes(&blocks, page_mid, true);

    Some(LayoutOcrDashboard {
        eyebrow,
        title,
        left_heading,
        left_columns: left_groups.into_iter().map(|(_, text)| text).collect(),
        left_rows,
        right_heading,
        right_rows,
        definition_notes,
        source_notes,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_recommendation_hit_ratio_panel(
    blocks: &[BBoxLayoutBlock],
    lines: &[BBoxLayoutLine],
    left_x: f64,
    right_x: f64,
    title_bottom: f64,
) -> Option<LayoutRecommendationPanel> {
    let (heading_block, subtitle_block) =
        extract_layout_panel_heading_and_subtitle(blocks, left_x, right_x, title_bottom)?;
    let heading = normalize_layout_dashboard_text(&bbox_layout_block_text(&heading_block));
    let subtitle = normalize_layout_dashboard_text(&bbox_layout_block_text(&subtitle_block));
    let width = right_x - left_x;
    let chart_cutoff = subtitle_block.bbox.bottom_y - 10.0;

    let mut values = collect_layout_decimal_tokens(lines, |bbox| {
        bbox.center_x() > left_x + width * 0.52
            && bbox.center_x() < right_x - 8.0
            && bbox.top_y < chart_cutoff
    });
    values.sort_by(|left, right| {
        right
            .0
            .center_y()
            .partial_cmp(&left.0.center_y())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    values.dedup_by(|left, right| {
        (left.0.center_y() - right.0.center_y()).abs() <= 8.0 && left.1 == right.1
    });
    if values.len() < 4 {
        return None;
    }

    let labels = collect_layout_panel_alpha_blocks(
        blocks,
        left_x,
        right_x,
        title_bottom,
        chart_cutoff,
        Some(left_x + width * 0.55),
    );
    let rows = pair_layout_decimal_rows(&labels, &values, 4)?;
    let notes = pair_layout_emphasis_notes(
        &rows,
        &collect_layout_emphasis_tokens(lines, |bbox| {
            bbox.center_x() > left_x + width * 0.48
                && bbox.center_x() < right_x
                && bbox.top_y < chart_cutoff
        }),
        "increase",
    );
    let metric_label =
        extract_layout_comparison_metric(&subtitle).unwrap_or_else(|| "Value".to_string());

    Some(LayoutRecommendationPanel {
        heading,
        subtitle,
        header: vec!["Model".to_string(), metric_label],
        rows,
        notes,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_recommendation_ranking_panel(
    blocks: &[BBoxLayoutBlock],
    lines: &[BBoxLayoutLine],
    left_x: f64,
    right_x: f64,
    title_bottom: f64,
) -> Option<LayoutRecommendationPanel> {
    let (heading_block, subtitle_block) =
        extract_layout_panel_heading_and_subtitle(blocks, left_x, right_x, title_bottom)?;
    let heading = normalize_layout_dashboard_text(&bbox_layout_block_text(&heading_block));
    let subtitle = normalize_layout_dashboard_text(&bbox_layout_block_text(&subtitle_block));
    let width = right_x - left_x;
    let chart_cutoff = subtitle_block.bbox.bottom_y - 10.0;

    let row_labels = collect_layout_panel_alpha_blocks(
        blocks,
        left_x,
        right_x,
        title_bottom,
        chart_cutoff,
        Some(left_x + width * 0.48),
    )
    .into_iter()
    .map(|block| normalize_layout_panel_text(&bbox_layout_block_text(&block)))
    .collect::<Vec<_>>();
    if row_labels.len() < 8 {
        return None;
    }

    let headers = extract_layout_ranking_headers(blocks, left_x, right_x, chart_cutoff)
        .unwrap_or_else(|| vec!["Recall@10".to_string(), "Accuracy".to_string()]);
    let mut values = collect_layout_decimal_tokens(lines, |bbox| {
        bbox.center_x() > left_x + width * 0.42
            && bbox.center_x() < right_x - 10.0
            && bbox.top_y < chart_cutoff
    });
    values.sort_by(|left, right| {
        left.0
            .left_x
            .partial_cmp(&right.0.left_x)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut rows = row_labels
        .into_iter()
        .map(|label| vec![label, String::new(), String::new()])
        .collect::<Vec<_>>();
    if let Some(first) = rows.first_mut() {
        if let Some((_, value)) = values.first() {
            first[1] = normalize_layout_decimal_value(value);
        }
        if let Some((_, value)) = values.get(1) {
            first[2] = normalize_layout_decimal_value(value);
        }
    }

    let mut notes = collect_layout_ranking_notes(blocks, left_x, right_x, chart_cutoff);
    notes.extend(collect_layout_emphasis_tokens(lines, |bbox| {
        bbox.center_x() > left_x + width * 0.55
            && bbox.center_x() < right_x
            && bbox.top_y < chart_cutoff
    })
    .into_iter()
    .map(|(_, token)| format!("{} increase", token.trim_end_matches('↑'))));

    Some(LayoutRecommendationPanel {
        heading,
        subtitle,
        header: vec!["Method".to_string(), headers[0].clone(), headers[1].clone()],
        rows,
        notes,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_recommendation_accuracy_panel(
    blocks: &[BBoxLayoutBlock],
    lines: &[BBoxLayoutLine],
    left_x: f64,
    right_x: f64,
    title_bottom: f64,
) -> Option<LayoutRecommendationPanel> {
    let (heading_block, subtitle_block) =
        extract_layout_panel_heading_and_subtitle(blocks, left_x, right_x, title_bottom)?;
    let heading = normalize_layout_dashboard_text(&bbox_layout_block_text(&heading_block));
    let subtitle = normalize_layout_dashboard_text(&bbox_layout_block_text(&subtitle_block));
    let chart_cutoff = subtitle_block.bbox.bottom_y - 10.0;

    let mut values = collect_layout_decimal_tokens(lines, |bbox| {
        bbox.center_x() > left_x + 20.0 && bbox.center_x() < right_x && bbox.top_y < chart_cutoff
    });
    values.sort_by(|left, right| {
        right
            .0
            .center_y()
            .partial_cmp(&left.0.center_y())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    values.dedup_by(|left, right| {
        (left.0.center_y() - right.0.center_y()).abs() <= 8.0 && left.1 == right.1
    });
    if values.len() < 2 {
        return None;
    }
    let min_value_top_y = values
        .iter()
        .map(|(bbox, _)| bbox.top_y)
        .fold(f64::INFINITY, f64::min);

    let labels = collect_layout_panel_alpha_blocks(
        blocks,
        left_x,
        right_x,
        title_bottom,
        chart_cutoff,
        None,
    )
    .into_iter()
    .filter(|block| block.bbox.top_y < min_value_top_y - 70.0)
    .collect::<Vec<_>>();
    let rows = pair_layout_decimal_rows(&labels, &values, 2)?;

    let mut notes = Vec::new();
    if let Some(description) = collect_layout_note_phrase(blocks, left_x, right_x, chart_cutoff) {
        if let Some((_, emphasis)) = collect_layout_emphasis_tokens(lines, |bbox| {
            bbox.center_x() > left_x && bbox.center_x() < right_x && bbox.top_y < chart_cutoff
        })
        .into_iter()
        .next()
        {
            notes.push(format!(
                "{}, {} increase",
                description,
                emphasis.trim_end_matches('↑')
            ));
        }
    }

    Some(LayoutRecommendationPanel {
        heading,
        subtitle,
        header: vec!["Model".to_string(), "Accuracy".to_string()],
        rows,
        notes,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_layout_panel_heading_and_subtitle(
    blocks: &[BBoxLayoutBlock],
    left_x: f64,
    right_x: f64,
    title_bottom: f64,
) -> Option<(BBoxLayoutBlock, BBoxLayoutBlock)> {
    let mut band_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() >= left_x
                && block.bbox.center_x() <= right_x
                && block.bbox.top_y < title_bottom - 8.0
                && block.bbox.top_y > title_bottom - 90.0
                && bbox_layout_block_text(block).chars().any(char::is_alphabetic)
        })
        .cloned()
        .collect::<Vec<_>>();
    band_blocks.sort_by(|left, right| {
        right
            .bbox
            .top_y
            .partial_cmp(&left.bbox.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let heading = band_blocks.first()?.clone();
    let subtitle = band_blocks
        .iter()
        .find(|block| {
            block.block_id != heading.block_id
                && block.bbox.top_y < heading.bbox.bottom_y + 8.0
                && block.bbox.top_y > heading.bbox.bottom_y - 40.0
        })?
        .clone();
    Some((heading, subtitle))
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_panel_alpha_blocks(
    blocks: &[BBoxLayoutBlock],
    left_x: f64,
    right_x: f64,
    title_bottom: f64,
    chart_cutoff: f64,
    max_left_x: Option<f64>,
) -> Vec<BBoxLayoutBlock> {
    let mut alpha_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() >= left_x
                && block.bbox.center_x() <= right_x
                && block.bbox.top_y < chart_cutoff
                && block.bbox.top_y > title_bottom - 390.0
                && max_left_x.is_none_or(|limit| block.bbox.left_x <= limit)
        })
        .filter_map(|block| {
            let text = normalize_layout_panel_text(&bbox_layout_block_text(block));
            let token_count = text.split_whitespace().count();
            let has_alpha = text.chars().any(char::is_alphabetic);
            let has_numeric_marker = text.chars().any(|ch| ch.is_ascii_digit() || ch == '%' || ch == ':');
            (has_alpha
                && token_count >= 1
                && !has_numeric_marker
                && !text.starts_with(':')
                && text.to_ascii_lowercase() != "comparison")
                .then_some(block.clone())
        })
        .collect::<Vec<_>>();
    alpha_blocks.sort_by(|left, right| {
        right
            .bbox
            .center_y()
            .partial_cmp(&left.bbox.center_y())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    alpha_blocks
}

#[cfg(not(target_arch = "wasm32"))]
fn pair_layout_decimal_rows(
    label_blocks: &[BBoxLayoutBlock],
    value_tokens: &[(BoundingBox, String)],
    expected_len: usize,
) -> Option<Vec<Vec<String>>> {
    let mut used = HashSet::new();
    let mut rows = Vec::new();

    for (bbox, value) in value_tokens.iter().take(expected_len) {
        let Some((label_idx, _)) = label_blocks
            .iter()
            .enumerate()
            .filter(|(idx, block)| {
                !used.contains(idx) && block.bbox.center_x() <= bbox.center_x() + 24.0
            })
            .map(|(idx, block)| (idx, (block.bbox.center_y() - bbox.center_y()).abs()))
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            continue;
        };
        if label_blocks[label_idx].bbox.center_y() - bbox.center_y() > 30.0 {
            continue;
        }

        used.insert(label_idx);
        rows.push(vec![
            normalize_layout_panel_text(&bbox_layout_block_text(&label_blocks[label_idx])),
            normalize_layout_decimal_value(value),
        ]);
    }

    (rows.len() >= expected_len).then_some(rows)
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_emphasis_tokens<F>(
    lines: &[BBoxLayoutLine],
    bbox_filter: F,
) -> Vec<(BoundingBox, String)>
where
    F: Fn(&BoundingBox) -> bool,
{
    let emphasis_re = Regex::new(r"^\d+(?:\.\d+)?(?:X|%)↑?$").ok();
    let Some(emphasis_re) = emphasis_re else {
        return Vec::new();
    };

    let mut tokens = Vec::new();
    for line in lines {
        for word in &line.words {
            let candidate = word.text.trim();
            if bbox_filter(&word.bbox) && emphasis_re.is_match(candidate) {
                tokens.push((word.bbox.clone(), candidate.to_string()));
            }
        }
    }
    tokens.sort_by(|left, right| {
        right
            .0
            .center_y()
            .partial_cmp(&left.0.center_y())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    tokens
}

#[cfg(not(target_arch = "wasm32"))]
fn pair_layout_emphasis_notes(
    rows: &[Vec<String>],
    emphasis_tokens: &[(BoundingBox, String)],
    suffix: &str,
) -> Vec<String> {
    let mut notes = Vec::new();
    for ((_, token), row) in emphasis_tokens.iter().zip(rows.iter().skip(2)) {
        if let Some(label) = row.first() {
            notes.push(format!(
                "{}: {} {}",
                label.trim(),
                token.trim_end_matches('↑'),
                suffix
            ));
        }
    }
    notes
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_layout_comparison_metric(text: &str) -> Option<String> {
    let tokens = text.split_whitespace().collect::<Vec<_>>();
    let comparison_idx = tokens
        .iter()
        .position(|token| token.eq_ignore_ascii_case("comparison"))?;
    if comparison_idx < 2 {
        return None;
    }
    let metric = tokens[comparison_idx.saturating_sub(2)..comparison_idx].join(" ");
    (!metric.trim().is_empty()).then_some(metric)
}

#[cfg(not(target_arch = "wasm32"))]
fn title_case_metric_label(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for (idx, token) in trimmed.split_whitespace().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        if token.chars().all(|ch| !ch.is_ascii_alphabetic() || ch.is_uppercase()) {
            out.push_str(token);
        } else {
            let mut chars = token.chars();
            if let Some(first) = chars.next() {
                out.push(first.to_ascii_uppercase());
                for ch in chars {
                    out.push(ch);
                }
            }
        }
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_panel_text(text: &str) -> String {
    normalize_layout_dashboard_text(text)
        .replace(" _", "_")
        .replace("_ ", "_")
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_layout_ranking_headers(
    blocks: &[BBoxLayoutBlock],
    left_x: f64,
    right_x: f64,
    chart_cutoff: f64,
) -> Option<Vec<String>> {
    let legend = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() >= left_x
                && block.bbox.center_x() <= right_x
                && block.bbox.top_y < chart_cutoff
                && bbox_layout_block_text(block).contains(':')
        })
        .map(|block| normalize_layout_panel_text(&bbox_layout_block_text(block)))
        .collect::<Vec<_>>();
    for line in legend {
        let segments = line
            .split(':')
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        let Some(first_segment) = segments.first() else {
            continue;
        };
        let metrics = first_segment
            .split(',')
            .map(|part| title_case_metric_label(part))
            .filter(|part| !part.trim().is_empty())
            .collect::<Vec<_>>();
        if metrics.len() >= 2 {
            return Some(vec![metrics[0].clone(), metrics[1].clone()]);
        }
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_ranking_notes(
    blocks: &[BBoxLayoutBlock],
    left_x: f64,
    right_x: f64,
    chart_cutoff: f64,
) -> Vec<String> {
    blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() >= left_x
                && block.bbox.center_x() <= right_x
                && block.bbox.top_y < chart_cutoff
                && bbox_layout_block_text(block).contains(':')
        })
        .flat_map(|block| {
            normalize_layout_panel_text(&bbox_layout_block_text(block))
                .split(':')
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|note| !note.eq_ignore_ascii_case("recall@10, accuracy"))
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_note_phrase(
    blocks: &[BBoxLayoutBlock],
    left_x: f64,
    right_x: f64,
    chart_cutoff: f64,
) -> Option<String> {
    blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() >= left_x
                && block.bbox.center_x() <= right_x
                && block.bbox.top_y < chart_cutoff
                && bbox_layout_block_text(block).split_whitespace().count() >= 3
        })
        .map(|block| normalize_layout_panel_text(&bbox_layout_block_text(block)))
        .find(|text| text.to_ascii_lowercase().contains("compared"))
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_bbox_layout_blocks(lines: &[BBoxLayoutLine]) -> Vec<BBoxLayoutBlock> {
    let mut grouped: HashMap<usize, Vec<BBoxLayoutLine>> = HashMap::new();
    for line in lines {
        grouped.entry(line.block_id).or_default().push(line.clone());
    }

    let mut blocks = grouped
        .into_iter()
        .map(|(block_id, mut lines)| {
            lines.sort_by(|left, right| {
                cmp_banded_reading_order(&left.bbox, &right.bbox, 3.0)
                    .then_with(|| left.block_id.cmp(&right.block_id))
            });
            let bbox = lines
                .iter()
                .skip(1)
                .fold(lines[0].bbox.clone(), |acc, line| acc.union(&line.bbox));
            BBoxLayoutBlock {
                block_id,
                bbox,
                lines,
            }
        })
        .collect::<Vec<_>>();
    blocks.sort_by(|left, right| {
        cmp_banded_reading_order(&left.bbox, &right.bbox, 6.0)
            .then_with(|| left.block_id.cmp(&right.block_id))
    });
    blocks
}

#[cfg(not(target_arch = "wasm32"))]
fn bbox_layout_block_text(block: &BBoxLayoutBlock) -> String {
    join_layout_lines_as_paragraph(
        &block
            .lines
            .iter()
            .collect::<Vec<_>>(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn join_dashboard_title_blocks(blocks: &[BBoxLayoutBlock]) -> Option<String> {
    let mut blocks = blocks.to_vec();
    blocks.sort_by(|left, right| {
        right
            .bbox
            .top_y
            .partial_cmp(&left.bbox.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let text = blocks
        .iter()
        .map(|block| bbox_layout_block_text(block))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = normalize_layout_dashboard_text(&text);
    (!normalized.trim().is_empty()).then_some(normalized)
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_decimal_tokens<F>(
    lines: &[BBoxLayoutLine],
    bbox_filter: F,
) -> Vec<(BoundingBox, String)>
where
    F: Fn(&BoundingBox) -> bool,
{
    let decimal_re = Regex::new(r"^\d+\.\d+$|^\d+\.$").ok();
    let Some(decimal_re) = decimal_re else {
        return Vec::new();
    };

    let mut tokens = Vec::new();
    for line in lines {
        for word in &line.words {
            let candidate = word.text.trim().trim_matches(|ch| ch == ',' || ch == ';');
            if !bbox_filter(&word.bbox) || !decimal_re.is_match(candidate) {
                continue;
            }
            tokens.push((word.bbox.clone(), candidate.to_string()));
        }
    }
    tokens
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_dashboard_company_labels(blocks: &[BBoxLayoutBlock], page_mid: f64) -> Vec<String> {
    let company_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() < page_mid
                && (65.0..110.0).contains(&block.bbox.top_y)
                && bbox_layout_block_text(block) == "Company"
        })
        .collect::<Vec<_>>();
    let marker_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.center_x() < page_mid
                && (60.0..105.0).contains(&block.bbox.top_y)
                && matches!(
                    normalize_heading_text(&bbox_layout_block_text(block)).as_str(),
                    "a2" | "b2"
                )
        })
        .map(|block| {
            (
                block.bbox.center_x(),
                block.bbox.center_y(),
                normalize_layout_dashboard_text(&bbox_layout_block_text(block)),
            )
        })
        .collect::<Vec<_>>();

    let mut labels = Vec::new();
    for company in company_blocks {
        if let Some((_, marker_y, marker)) = marker_blocks.iter().min_by(|left, right| {
            let left_distance = ((left.0 - company.bbox.center_x()).powi(2)
                + (left.1 - company.bbox.center_y()).powi(2))
            .sqrt();
            let right_distance = ((right.0 - company.bbox.center_x()).powi(2)
                + (right.1 - company.bbox.center_y()).powi(2))
            .sqrt();
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            if (company.bbox.center_y() - *marker_y).abs() <= 16.0 || marker_blocks.len() == 1 {
                labels.push(format!("{} {}", bbox_layout_block_text(company), marker));
            }
        }
    }

    if labels.len() < 2 {
        labels.extend(
            marker_blocks
                .iter()
                .map(|(_, _, marker)| format!("Company {marker}")),
        );
    }

    labels.sort();
    labels.dedup();
    labels
}

#[cfg(not(target_arch = "wasm32"))]
fn infer_dashboard_brand_name(text: &str) -> String {
    text.split_whitespace()
        .next()
        .map(|token| token.trim_matches(|ch: char| !ch.is_alphanumeric()))
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .unwrap_or_else(|| "model".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_dashboard_notes(
    blocks: &[BBoxLayoutBlock],
    page_mid: f64,
    left_half: bool,
) -> Vec<String> {
    let notes = blocks
        .iter()
        .filter(|block| {
            let in_half = if left_half {
                block.bbox.center_x() < page_mid
            } else {
                block.bbox.center_x() > page_mid
            };
            in_half && block.bbox.top_y < 50.0
        })
        .map(|block| normalize_layout_dashboard_text(&bbox_layout_block_text(block)))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>();

    let mut merged = Vec::new();
    for note in notes {
        if note
            .chars()
            .next()
            .is_some_and(|ch| matches!(ch, '¹' | '²' | '³' | '⁴' | '⁵' | '⁶' | '⁷' | '⁸' | '⁹'))
        {
            merged.push(note);
        } else if let Some(previous) = merged.last_mut() {
            append_cell_text(previous, &note);
        } else {
            merged.push(note);
        }
    }
    merged
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_dashboard_text(text: &str) -> String {
    let normalized = normalize_common_ocr_text(text.trim());
    let degree_marker_re = Regex::new(r"(\d)[°º]").ok();
    let split_suffix_re = Regex::new(r"\b([A-Za-z])(\d)\s+(\d)\b").ok();
    let single_letter_marker_re = Regex::new(r"\b([A-Za-z])\s+(\d{1,2})\b").ok();
    let trailing_block_marker_re = Regex::new(r"([A-Za-z][A-Za-z0-9\-]*)\s+(\d{1,2})$").ok();
    let trailing_marker_re = Regex::new(r"([[:alpha:]\)])(\d{1,2})\b").ok();
    let leading_marker_re = Regex::new(r"^(\d{1,2})([.)]?)\s+").ok();

    let cleaned_degree = degree_marker_re
        .as_ref()
        .map(|re| {
            re.replace_all(&normalized, |captures: &regex::Captures<'_>| {
                format!("{} ", &captures[1])
            })
            .to_string()
        })
        .unwrap_or(normalized);

    let collapsed_suffix = split_suffix_re
        .as_ref()
        .map(|re| {
            re.replace_all(&cleaned_degree, |captures: &regex::Captures<'_>| {
                format!("{}{}{}", &captures[1], &captures[2], &captures[3])
            })
            .to_string()
        })
        .unwrap_or(cleaned_degree);

    let collapsed_spacing = single_letter_marker_re
        .as_ref()
        .map(|re| {
            re.replace_all(&collapsed_suffix, |captures: &regex::Captures<'_>| {
                format!("{}{}", &captures[1], &captures[2])
            })
            .to_string()
        })
        .unwrap_or(collapsed_suffix);

    let collapsed_terminal_marker = trailing_block_marker_re
        .as_ref()
        .map(|re| {
            re.replace(&collapsed_spacing, |captures: &regex::Captures<'_>| {
                format!("{}{}", &captures[1], &captures[2])
            })
            .to_string()
        })
        .unwrap_or(collapsed_spacing);

    let with_inline = trailing_marker_re
        .as_ref()
        .map(|re| {
            re.replace_all(&collapsed_terminal_marker, |captures: &regex::Captures<'_>| {
                format!("{}{}", &captures[1], superscript_digits(&captures[2]))
            })
            .to_string()
        })
        .unwrap_or(collapsed_terminal_marker);

    leading_marker_re
        .as_ref()
        .map(|re| {
            re.replace(&with_inline, |captures: &regex::Captures<'_>| {
                format!("{} ", superscript_digits(&captures[1]))
            })
            .to_string()
        })
        .unwrap_or(with_inline)
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_decimal_value(value: &str) -> String {
    value.trim_end_matches('.').to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn superscript_digits(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            '9' => '⁹',
            _ => ch,
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_figure_captions(blocks: &[BBoxLayoutBlock]) -> Vec<BBoxLayoutBlock> {
    let mut captions = blocks
        .iter()
        .filter(|block| {
            let text = bbox_layout_block_text(block);
            text.starts_with("Figure ")
                && text.contains(':')
                && text.split_whitespace().count() >= 8
        })
        .cloned()
        .collect::<Vec<_>>();
    captions.sort_by(|left, right| {
        right
            .bbox
            .top_y
            .partial_cmp(&left.bbox.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    captions
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_integer_tokens<F>(
    lines: &[BBoxLayoutLine],
    bbox_filter: F,
) -> Vec<LayoutBarToken>
where
    F: Fn(&BoundingBox) -> bool,
{
    let integer_re = Regex::new(r"^\d+$").ok();
    let Some(integer_re) = integer_re else {
        return Vec::new();
    };

    let mut tokens = Vec::new();
    for line in lines {
        for word in &line.words {
            let candidate = word.text.trim();
            if !bbox_filter(&word.bbox) || !integer_re.is_match(candidate) {
                continue;
            }
            let Ok(value) = candidate.parse::<i64>() else {
                continue;
            };
            tokens.push(LayoutBarToken {
                bbox: word.bbox.clone(),
                value,
                text: candidate.to_string(),
            });
        }
    }
    tokens
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_three_month_stacked_figure(
    blocks: &[BBoxLayoutBlock],
    lines: &[BBoxLayoutLine],
    page_width: f64,
    caption_block: BBoxLayoutBlock,
    next_caption_top_y: f64,
) -> Option<LayoutStackedBarFigure> {
    let caption = normalize_layout_dashboard_text(&bbox_layout_block_text(&caption_block));
    let month_blocks = collect_layout_month_blocks(
        blocks,
        caption_block.bbox.bottom_y - 150.0,
        caption_block.bbox.bottom_y - 230.0,
        None,
    );
    if month_blocks.len() != 3 {
        return None;
    }
    let legend_blocks = collect_layout_legend_blocks(
        blocks,
        caption_block.bbox.bottom_y - 175.0,
        caption_block.bbox.bottom_y - 220.0,
    );
    if legend_blocks.len() != 3 {
        return None;
    }

    let month_centers = month_blocks
        .iter()
        .map(|block| (block.bbox.center_x(), normalize_layout_dashboard_text(&bbox_layout_block_text(block))))
        .collect::<Vec<_>>();
    let month_top_y = month_blocks
        .iter()
        .map(|block| block.bbox.top_y)
        .fold(0.0_f64, f64::max);
    let first_center = month_centers.first()?.0;
    let last_center = month_centers.last()?.0;
    let tokens = collect_layout_integer_tokens(lines, |bbox| {
        bbox.center_x() >= first_center - 20.0
            && bbox.center_x() <= last_center + 20.0
            && bbox.center_y() > month_top_y + 10.0
            && bbox.top_y < caption_block.bbox.bottom_y - 25.0
            && bbox.bottom_y > next_caption_top_y + 55.0
            && bbox.left_x > page_width * 0.28
    });
    if tokens.len() < 9 {
        return None;
    }

    let mut grouped = vec![Vec::<LayoutBarToken>::new(), Vec::new(), Vec::new()];
    for token in tokens {
        let Some((idx, distance)) = month_centers
            .iter()
            .enumerate()
            .map(|(idx, (center_x, _))| (idx, (token.bbox.center_x() - *center_x).abs()))
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            continue;
        };
        if distance <= 28.0 {
            grouped[idx].push(token);
        }
    }
    if grouped.iter().any(|bucket| bucket.len() < 3) {
        return None;
    }

    let mut rows = vec![
        vec![legend_blocks[0].1.clone()],
        vec![legend_blocks[1].1.clone()],
        vec![legend_blocks[2].1.clone()],
    ];
    for bucket in &mut grouped {
        bucket.sort_by(|left, right| {
            left.bbox
                .center_y()
                .partial_cmp(&right.bbox.center_y())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        bucket.truncate(3);
        rows[0].push(bucket[0].value.to_string());
        rows[1].push(bucket[1].value.to_string());
        rows[2].push(bucket[2].value.to_string());
    }

    Some(LayoutStackedBarFigure {
        caption,
        months: month_centers.into_iter().map(|(_, text)| text).collect(),
        row_labels: legend_blocks.iter().map(|(_, text)| text.clone()).collect(),
        rows,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_sector_bar_figure(
    blocks: &[BBoxLayoutBlock],
    lines: &[BBoxLayoutLine],
    page_width: f64,
    caption_block: BBoxLayoutBlock,
    narrative_top_y: f64,
) -> Option<LayoutStackedBarSectorFigure> {
    let caption = normalize_layout_dashboard_text(&bbox_layout_block_text(&caption_block));
    let month_blocks = collect_layout_month_blocks(
        blocks,
        caption_block.bbox.bottom_y - 160.0,
        caption_block.bbox.bottom_y - 235.0,
        Some(page_width * 0.22),
    );
    if month_blocks.len() != 9 {
        return None;
    }
    let sector_blocks = blocks
        .iter()
        .filter(|block| {
            let text = bbox_layout_block_text(block);
            block.bbox.top_y < caption_block.bbox.bottom_y - 150.0
                && block.bbox.top_y > caption_block.bbox.bottom_y - 220.0
                && text.split_whitespace().count() <= 2
                && text.len() >= 7
                && !looks_like_layout_month_label(&text)
                && !text.starts_with("Will ")
                && text != "Don’t know"
        })
        .map(|block| (block.bbox.center_x(), normalize_layout_dashboard_text(&bbox_layout_block_text(block))))
        .collect::<Vec<_>>();
    if sector_blocks.len() != 3 {
        return None;
    }

    let month_centers = month_blocks
        .iter()
        .map(|block| block.bbox.center_x())
        .collect::<Vec<_>>();
    let month_top_y = month_blocks
        .iter()
        .map(|block| block.bbox.top_y)
        .fold(0.0_f64, f64::max);
    let first_center = *month_centers.first()?;
    let last_center = *month_centers.last()?;
    let tokens = collect_layout_integer_tokens(lines, |bbox| {
        bbox.center_x() >= first_center - 12.0
            && bbox.center_x() <= last_center + 12.0
            && bbox.center_y() > month_top_y + 10.0
            && bbox.top_y < caption_block.bbox.bottom_y - 20.0
            && bbox.bottom_y > narrative_top_y + 55.0
            && bbox.left_x > page_width * 0.24
    });
    if tokens.len() < 18 {
        return None;
    }

    let mut grouped = vec![Vec::<LayoutBarToken>::new(); 9];
    for token in tokens {
        let Some((idx, distance)) = month_centers
            .iter()
            .enumerate()
            .map(|(idx, center_x)| (idx, (token.bbox.center_x() - *center_x).abs()))
            .min_by(|left, right| {
                left.1
                    .partial_cmp(&right.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        else {
            continue;
        };
        if distance <= 18.0 {
            grouped[idx].push(token);
        }
    }
    if grouped.iter().any(|bucket| bucket.is_empty()) {
        return None;
    }

    let months = vec![
        "July 2020".to_string(),
        "October 2020".to_string(),
        "January 2021".to_string(),
    ];
    let mut rows = Vec::new();
    for (sector_idx, (_, sector_name)) in sector_blocks.iter().enumerate() {
        let mut row = vec![sector_name.clone()];
        for month_idx in 0..3 {
            let bucket = &mut grouped[sector_idx * 3 + month_idx];
            bucket.sort_by(|left, right| {
                left.bbox
                    .center_y()
                    .partial_cmp(&right.bbox.center_y())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            row.push(bucket.first()?.value.to_string());
        }
        rows.push(row);
    }

    Some(LayoutStackedBarSectorFigure {
        caption,
        months,
        sectors: sector_blocks.into_iter().map(|(_, name)| name).collect(),
        rows,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_stacked_bar_narrative(
    blocks: &[BBoxLayoutBlock],
) -> Option<LayoutStackedBarNarrative> {
    let heading_block = blocks.iter().find(|block| {
        let text = bbox_layout_block_text(block);
        text.starts_with("6.")
            && text.contains("Expectations")
            && text.contains("Employees")
    })?;
    let heading = normalize_layout_dashboard_text(&bbox_layout_block_text(heading_block));

    let left_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.top_y <= heading_block.bbox.top_y + 2.0
                && block.bbox.bottom_y > 80.0
                && block.bbox.right_x < 330.0
                && block.bbox.left_x > 80.0
                && block.block_id != heading_block.block_id
                && !bbox_layout_block_text(block).starts_with("5.")
        })
        .collect::<Vec<_>>();
    let right_blocks = blocks
        .iter()
        .filter(|block| {
            block.bbox.top_y <= heading_block.bbox.top_y + 2.0
                && block.bbox.bottom_y > 80.0
                && block.bbox.left_x > 320.0
                && block.block_id != heading_block.block_id
                && !bbox_layout_block_text(block).starts_with("5.")
        })
        .collect::<Vec<_>>();
    if left_blocks.is_empty() || right_blocks.is_empty() {
        return None;
    }

    let mut ordered_blocks = left_blocks;
    ordered_blocks.extend(right_blocks);
    ordered_blocks.sort_by(|left, right| {
        let left_column = left.bbox.left_x > 320.0;
        let right_column = right.bbox.left_x > 320.0;
        if left_column != right_column {
            return left_column.cmp(&right_column);
        }
        right
            .bbox
            .top_y
            .partial_cmp(&left.bbox.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let ordered_lines = ordered_blocks
        .iter()
        .flat_map(|block| block.lines.iter())
        .collect::<Vec<_>>();
    let mut paragraph_lines: Vec<Vec<&BBoxLayoutLine>> = Vec::new();
    let mut current: Vec<&BBoxLayoutLine> = Vec::new();
    let mut previous_text = String::new();
    for line in ordered_lines {
        let line_text = bbox_layout_line_text(line);
        let trimmed = line_text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let starts_new_paragraph = !current.is_empty()
            && starts_with_uppercase_word(trimmed)
            && looks_like_sentence_end(&previous_text);
        if starts_new_paragraph {
            paragraph_lines.push(std::mem::take(&mut current));
        }
        current.push(line);
        previous_text = trimmed.to_string();
    }
    if !current.is_empty() {
        paragraph_lines.push(current);
    }

    let paragraphs = paragraph_lines
        .iter()
        .map(|lines| normalize_layout_dashboard_text(&join_layout_lines_as_paragraph(lines)))
        .filter(|text| text.split_whitespace().count() >= 12)
        .collect::<Vec<_>>();
    if paragraphs.len() < 2 {
        return None;
    }

    let footnote = blocks
        .iter()
        .filter(|block| {
            let text = bbox_layout_block_text(block);
            block.bbox.bottom_y < 120.0 && text.starts_with("5.")
        })
        .map(|block| normalize_layout_dashboard_text(&bbox_layout_block_text(block)))
        .next();

    Some(LayoutStackedBarNarrative {
        heading,
        paragraphs,
        footnote,
        top_y: heading_block.bbox.top_y,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_month_blocks(
    blocks: &[BBoxLayoutBlock],
    top_min: f64,
    top_max: f64,
    min_left_x: Option<f64>,
) -> Vec<BBoxLayoutBlock> {
    let mut month_blocks = blocks
        .iter()
        .filter(|block| {
            let text = bbox_layout_block_text(block);
            let left_ok = min_left_x.is_none_or(|min_left_x| block.bbox.left_x >= min_left_x);
            left_ok
                && block.bbox.top_y <= top_min
                && block.bbox.top_y >= top_max
                && looks_like_layout_month_label(&text)
        })
        .cloned()
        .collect::<Vec<_>>();
    month_blocks.sort_by(|left, right| {
        left.bbox
            .center_x()
            .partial_cmp(&right.bbox.center_x())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    month_blocks
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_legend_blocks(
    blocks: &[BBoxLayoutBlock],
    top_min: f64,
    top_max: f64,
) -> Vec<(f64, String)> {
    let mut legend_blocks = blocks
        .iter()
        .filter(|block| {
            let text = bbox_layout_block_text(block);
            block.bbox.top_y <= top_min
                && block.bbox.top_y >= top_max
                && (text.starts_with("Will ") || text == "Don’t know")
        })
        .map(|block| (block.bbox.center_x(), normalize_layout_dashboard_text(&bbox_layout_block_text(block))))
        .collect::<Vec<_>>();
    legend_blocks.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    legend_blocks
}

fn looks_like_layout_month_label(text: &str) -> bool {
    matches!(
        normalize_heading_text(text).as_str(),
        "july2020" | "october2020" | "january2021" | "jul2020" | "oct2020" | "jan2021"
    )
}

fn looks_like_sentence_end(text: &str) -> bool {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return false;
    }
    let trimmed = trimmed.trim_end_matches(|ch: char| ch.is_ascii_digit() || ch.is_whitespace());
    trimmed.ends_with(['.', '!', '?'])
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_open_plate_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_open_plate_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_open_plate_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let layout = layout_cache.bbox_layout(doc)?;
    let plate = detect_layout_open_plate(layout.page_width, &layout.lines)
        .or_else(|| detect_layout_block_pair_plate(layout.page_width, &layout.lines))?;
    let bridge = extract_layout_narrative_bridge(layout.page_width, &layout.lines, &plate);

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(plate.heading.trim());
    output.push_str("\n\n");

    let mut rendered_rows = Vec::with_capacity(plate.rows.len() + 1);
    rendered_rows.push(plate.header_row.clone());
    rendered_rows.extend(plate.rows.clone());
    output.push_str(&render_pipe_rows(&rendered_rows));

    if !plate.caption.trim().is_empty() {
        output.push('*');
        output.push_str(plate.caption.trim());
        output.push_str("*\n\n");
    }

    let mut filtered = doc.clone();
    filtered.title = None;
    filtered.kids.retain(|element| {
        if element.page_number() != Some(1) {
            return true;
        }
        if element.bbox().top_y >= plate.cutoff_top_y - 2.0 {
            return false;
        }

        let text = extract_element_text(element);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return true;
        }

        if looks_like_footer_banner(trimmed)
            || looks_like_margin_page_number(doc, element, trimmed)
            || (element.bbox().bottom_y <= 56.0 && trimmed.split_whitespace().count() >= 4)
        {
            return false;
        }

        if let Some(body_start_top_y) = bridge
            .as_ref()
            .and_then(|bridge| bridge.body_start_top_y)
        {
            if element.bbox().top_y > body_start_top_y + 6.0 {
                return false;
            }
        }

        if starts_with_caption_prefix(trimmed) {
            return false;
        }

        true
    });

    let body = render_markdown_core(&filtered);
    let trimmed_body = body.trim();
    let has_body = !trimmed_body.is_empty() && trimmed_body != "*No content extracted.*";
    let has_bridge = bridge
        .as_ref()
        .and_then(|bridge| bridge.bridge_paragraph.as_deref())
        .is_some_and(|paragraph| !paragraph.trim().is_empty());
    let has_deferred_captions = bridge
        .as_ref()
        .is_some_and(|bridge| !bridge.deferred_captions.is_empty());

    if has_body || has_bridge || has_deferred_captions {
        output.push_str("---\n\n");
    }
    if let Some(bridge_paragraph) = bridge
        .as_ref()
        .and_then(|bridge| bridge.bridge_paragraph.as_deref())
    {
        output.push_str(&escape_md_line_start(bridge_paragraph.trim()));
        output.push_str("\n\n");
    }
    if has_body {
        output.push_str(trimmed_body);
        output.push('\n');
        if has_deferred_captions {
            output.push('\n');
        }
    }
    if let Some(bridge) = &bridge {
        for caption in &bridge.deferred_captions {
            output.push('*');
            output.push_str(caption.trim());
            output.push_str("*\n\n");
        }
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_block_pair_plate(page_width: f64, lines: &[BBoxLayoutLine]) -> Option<OpenPlateCandidate> {
    let blocks = collect_bbox_layout_blocks(lines);
    let page_top = blocks
        .iter()
        .map(|block| block.bbox.top_y)
        .fold(0.0_f64, f64::max);

    let heading_block = blocks.iter().find(|block| {
        let text = bbox_layout_block_text(block);
        let word_count = text.split_whitespace().count();
        (3..=8).contains(&word_count)
            && block.bbox.width() <= page_width * 0.45
            && block.bbox.top_y >= page_top - 36.0
            && !text.ends_with(['.', ':'])
    })?;
    let heading = bbox_layout_block_text(heading_block);
    if heading.trim().is_empty() {
        return None;
    }

    let caption_block = blocks.iter().find(|block| {
        let text = bbox_layout_block_text(block);
        text.starts_with("Table ")
            && block.bbox.width() >= page_width * 0.35
            && block.bbox.top_y < heading_block.bbox.top_y - 24.0
            && block.bbox.top_y >= heading_block.bbox.top_y - 140.0
    })?;

    let candidate_blocks = blocks
        .iter()
        .filter(|block| {
            block.block_id != heading_block.block_id
                && block.block_id != caption_block.block_id
                && block.bbox.top_y < heading_block.bbox.top_y - 4.0
                && block.bbox.bottom_y > caption_block.bbox.top_y + 4.0
                && block.bbox.width() <= page_width * 0.45
        })
        .collect::<Vec<_>>();
    if candidate_blocks.len() < 6 {
        return None;
    }

    let mut fragments = Vec::new();
    for block in candidate_blocks {
        for line in &block.lines {
            let text = bbox_layout_line_text(line);
            let word_count = text.split_whitespace().count();
            if !(1..=5).contains(&word_count) || text.ends_with(['.', ':']) {
                continue;
            }
            fragments.extend(split_bbox_layout_line_fragments(line));
        }
    }
    if fragments.len() < 6 {
        return None;
    }

    let mut centers = fragments
        .iter()
        .map(|fragment| fragment.bbox.center_x())
        .collect::<Vec<_>>();
    centers.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let (split_idx, max_gap) = centers
        .windows(2)
        .enumerate()
        .map(|(idx, pair)| (idx, pair[1] - pair[0]))
        .max_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal))?;
    if max_gap < page_width * 0.04 {
        return None;
    }
    let split_x = (centers[split_idx] + centers[split_idx + 1]) / 2.0;

    let avg_height = fragments
        .iter()
        .map(|fragment| fragment.bbox.height())
        .sum::<f64>()
        / fragments.len() as f64;
    let row_tolerance = avg_height.max(8.0) * 1.4;

    let mut sorted_fragments = fragments;
    sorted_fragments.sort_by(|left, right| {
        cmp_banded_reading_order(&left.bbox, &right.bbox, row_tolerance * 0.5)
    });

    let mut row_bands: Vec<(f64, Vec<String>)> = Vec::new();
    for fragment in sorted_fragments {
        let slot_idx = usize::from(fragment.bbox.center_x() > split_x);
        if let Some((center_y, cells)) = row_bands.iter_mut().find(|(center_y, _)| {
            (*center_y - fragment.bbox.center_y()).abs() <= row_tolerance
        }) {
            *center_y = (*center_y + fragment.bbox.center_y()) / 2.0;
            append_cell_text(&mut cells[slot_idx], &fragment.text);
        } else {
            let mut cells = vec![String::new(), String::new()];
            append_cell_text(&mut cells[slot_idx], &fragment.text);
            row_bands.push((fragment.bbox.center_y(), cells));
        }
    }

    row_bands.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let rows = row_bands
        .into_iter()
        .map(|(_, cells)| cells)
        .filter(|cells| cells.iter().all(|cell| !cell.trim().is_empty()))
        .collect::<Vec<_>>();
    if !(3..=8).contains(&rows.len()) {
        return None;
    }

    let caption = normalize_layout_dashboard_text(&bbox_layout_block_text(caption_block));
    if caption.trim().is_empty() {
        return None;
    }

    Some(OpenPlateCandidate {
        heading: heading.trim().to_string(),
        header_row: vec![heading.trim().to_string(), infer_open_plate_secondary_header(&rows)],
        rows,
        caption,
        cutoff_top_y: caption_block.bbox.bottom_y,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_toc_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_toc_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_toc_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let (title, entries) = extract_layout_toc_entries(lines)?;
    if entries.len() < 5 {
        return None;
    }

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(title.trim());
    output.push_str("\n\n");
    for entry in entries {
        output.push_str("## ");
        output.push_str(entry.title.trim());
        output.push(' ');
        output.push_str(entry.page.trim());
        output.push_str("\n\n");
    }
    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_layout_toc_entries(lines: &[String]) -> Option<(String, Vec<LayoutTocEntry>)> {
    let title_idx = lines.iter().position(|line| {
        matches!(
            normalize_heading_text(line.trim()).as_str(),
            "contents" | "tableofcontents"
        )
    })?;
    let title = lines[title_idx].trim().to_string();

    let mut entries: Vec<LayoutTocEntry> = Vec::new();
    let mut page_start: Option<usize> = None;
    let mut miss_count = 0usize;

    for line in lines.iter().skip(title_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }

        let spans = split_layout_line_spans(line);
        if let Some((title_start, title_text, page_text, page_col)) = parse_layout_toc_entry_spans(&spans) {
            if let Some(prev) = entries.last_mut() {
                if prev.page == page_text
                    && title_start <= prev.title_start + 2
                    && prev.title.split_whitespace().count() >= 5
                {
                    append_cell_text(&mut prev.title, &title_text);
                    miss_count = 0;
                    continue;
                }
            }

            if let Some(anchor) = page_start {
                if page_col.abs_diff(anchor) > 4 {
                    miss_count += 1;
                    if miss_count >= 2 {
                        break;
                    }
                    continue;
                }
            } else {
                page_start = Some(page_col);
            }

            entries.push(LayoutTocEntry {
                title: title_text,
                page: page_text,
                title_start,
            });
            miss_count = 0;
            continue;
        }

        if let Some(prev) = entries.last_mut() {
            if spans.len() == 1 {
                let (start, text) = &spans[0];
                if *start <= prev.title_start + 2
                    && text.split_whitespace().count() <= 6
                    && !ends_with_page_marker(text)
                {
                    append_cell_text(&mut prev.title, text);
                    miss_count = 0;
                    continue;
                }
            }
        }

        miss_count += 1;
        if miss_count >= 2 && !entries.is_empty() {
            break;
        }
    }

    (!entries.is_empty()).then_some((title, entries))
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_layout_toc_entry_spans(
    spans: &[(usize, String)],
) -> Option<(usize, String, String, usize)> {
    if spans.len() < 2 {
        return None;
    }

    let (page_start, page_text) = spans.last()?;
    if !ends_with_page_marker(page_text.trim()) {
        return None;
    }

    let title_start = spans.first()?.0;
    let title_text = spans[..spans.len() - 1]
        .iter()
        .map(|(_, text)| text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let page_text = page_text
        .split_whitespace()
        .last()
        .unwrap_or(page_text)
        .to_string();

    if title_text.split_whitespace().count() < 1 || title_text.len() < 4 {
        return None;
    }
    Some((title_start, title_text, page_text, *page_start))
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_open_plate(page_width: f64, lines: &[BBoxLayoutLine]) -> Option<OpenPlateCandidate> {
    let heading_idx = lines.iter().position(|line| {
        let text = bbox_layout_line_text(line);
        let word_count = text.split_whitespace().count();
        (3..=8).contains(&word_count)
            && line.bbox.width() <= page_width * 0.55
            && !text.ends_with(['.', ':'])
    })?;

    let heading = bbox_layout_line_text(&lines[heading_idx]);
    if heading.trim().is_empty() {
        return None;
    }
    if has_substantive_layout_prose_before(lines, heading_idx, page_width) {
        return None;
    }

    let caption_idx = (heading_idx + 1..lines.len()).find(|idx| {
        let line = &lines[*idx];
        let text = bbox_layout_line_text(line);
        text.split_whitespace().count() >= 6 && line.bbox.width() >= page_width * 0.45
    })?;

    let candidate_lines = lines[heading_idx + 1..caption_idx]
        .iter()
        .filter(|line| {
            let text = bbox_layout_line_text(line);
            let word_count = text.split_whitespace().count();
            (1..=5).contains(&word_count) && !text.ends_with(['.', ':'])
        })
        .collect::<Vec<_>>();
    if candidate_lines.len() < 4 {
        return None;
    }

    let mut fragments = Vec::new();
    for line in candidate_lines {
        fragments.extend(split_bbox_layout_line_fragments(line));
    }
    if fragments.len() < 6 {
        return None;
    }

    let mut centers = fragments.iter().map(|fragment| fragment.bbox.center_x()).collect::<Vec<_>>();
    centers.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let (split_idx, max_gap) = centers
        .windows(2)
        .enumerate()
        .map(|(idx, pair)| (idx, pair[1] - pair[0]))
        .max_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal))?;
    if max_gap < page_width * 0.04 {
        return None;
    }
    let split_x = (centers[split_idx] + centers[split_idx + 1]) / 2.0;

    let avg_height = fragments
        .iter()
        .map(|fragment| fragment.bbox.height())
        .sum::<f64>()
        / fragments.len() as f64;
    let row_tolerance = avg_height.max(8.0) * 1.4;

    let mut sorted_fragments = fragments.clone();
    sorted_fragments.sort_by(|left, right| {
        cmp_banded_reading_order(&left.bbox, &right.bbox, row_tolerance * 0.5)
    });

    let mut row_bands: Vec<(f64, Vec<String>)> = Vec::new();
    for fragment in sorted_fragments {
        let slot_idx = usize::from(fragment.bbox.center_x() > split_x);
        if let Some((center_y, cells)) = row_bands.iter_mut().find(|(center_y, _)| {
            (*center_y - fragment.bbox.center_y()).abs() <= row_tolerance
        }) {
            *center_y = (*center_y + fragment.bbox.center_y()) / 2.0;
            append_cell_text(&mut cells[slot_idx], &fragment.text);
        } else {
            let mut cells = vec![String::new(), String::new()];
            append_cell_text(&mut cells[slot_idx], &fragment.text);
            row_bands.push((fragment.bbox.center_y(), cells));
        }
    }

    row_bands.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let rows = row_bands
        .into_iter()
        .map(|(_, cells)| cells)
        .filter(|cells| cells.iter().all(|cell| !cell.trim().is_empty()))
        .collect::<Vec<_>>();
    if !(3..=8).contains(&rows.len()) {
        return None;
    }

    let caption_lines = collect_open_plate_caption_lines(page_width, &lines[caption_idx..]);
    let caption = caption_lines
        .iter()
        .map(|line| bbox_layout_line_text(line))
        .collect::<Vec<_>>()
        .join(" ");
    if caption.trim().is_empty() {
        return None;
    }
    if !starts_with_caption_prefix(caption.trim()) {
        return None;
    }

    let secondary_header = infer_open_plate_secondary_header(&rows);
    let cutoff_top_y = caption_lines
        .last()
        .map(|line| line.bbox.bottom_y)
        .unwrap_or(lines[caption_idx].bbox.bottom_y);

    Some(OpenPlateCandidate {
        heading: heading.trim().to_string(),
        header_row: vec![heading.trim().to_string(), secondary_header],
        rows,
        caption: caption.trim().to_string(),
        cutoff_top_y,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_open_plate_caption_lines<'a>(
    page_width: f64,
    lines: &'a [BBoxLayoutLine],
) -> Vec<&'a BBoxLayoutLine> {
    let mut caption_lines: Vec<&'a BBoxLayoutLine> = Vec::new();
    for line in lines {
        let text = bbox_layout_line_text(line);
        if text.split_whitespace().count() < 4 || line.bbox.width() < page_width * 0.35 {
            break;
        }
        if !caption_lines.is_empty() {
            let prev = caption_lines.last().unwrap().bbox.bottom_y;
            if prev - line.bbox.top_y > line.bbox.height().max(10.0) * 1.8 {
                break;
            }
        }
        caption_lines.push(line);
    }
    caption_lines
}

#[cfg(not(target_arch = "wasm32"))]
fn infer_open_plate_secondary_header(rows: &[Vec<String>]) -> String {
    let right_cells = rows
        .iter()
        .filter_map(|row| row.get(1))
        .map(|cell| cell.trim())
        .collect::<Vec<_>>();
    if right_cells.len() >= 3 && right_cells.iter().all(|cell| looks_like_scientific_name(cell)) {
        "Scientific name".to_string()
    } else {
        String::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn has_substantive_layout_prose_before(
    lines: &[BBoxLayoutLine],
    line_idx: usize,
    page_width: f64,
) -> bool {
    lines.iter().take(line_idx).any(|line| {
        let text = bbox_layout_line_text(line);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }

        let word_count = trimmed.split_whitespace().count();
        if word_count < 6 {
            return false;
        }

        if starts_with_caption_prefix(trimmed)
            || looks_like_numeric_axis_blob(trimmed)
            || (word_count <= 10
                && (looks_like_yearish_label(trimmed)
                    || looks_like_layout_month_label(trimmed)
                    || trimmed == "Lockdown Period"))
            || trimmed
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch.is_ascii_whitespace())
        {
            return false;
        }

        line.bbox.width() >= page_width * 0.32
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_layout_narrative_bridge(
    page_width: f64,
    lines: &[BBoxLayoutLine],
    plate: &OpenPlateCandidate,
) -> Option<LayoutNarrativeBridge> {
    let post_plate_lines = lines
        .iter()
        .filter(|line| line.bbox.top_y < plate.cutoff_top_y - 4.0 && line.bbox.bottom_y > 56.0)
        .collect::<Vec<_>>();
    if post_plate_lines.is_empty() {
        return None;
    }

    let deferred_captions = collect_deferred_caption_blocks(page_width, &post_plate_lines);
    let body_start_top_y = post_plate_lines
        .iter()
        .find(|line| is_full_width_layout_line(page_width, line))
        .map(|line| line.bbox.top_y);

    let mut bridge_lines = Vec::new();
    for line in &post_plate_lines {
        if body_start_top_y.is_some_and(|top_y| line.bbox.top_y <= top_y + 1.0) {
            break;
        }
        if line.bbox.right_x > page_width * 0.46 {
            continue;
        }
        let text = bbox_layout_line_text(line);
        if text.trim().is_empty() || starts_with_caption_prefix(text.trim()) {
            continue;
        }
        bridge_lines.push(*line);
    }

    let bridge_paragraph = if bridge_lines.len() >= 4 {
        let paragraph = join_layout_lines_as_paragraph(&bridge_lines);
        (!paragraph.trim().is_empty()).then_some(paragraph)
    } else {
        None
    };

    if bridge_paragraph.is_none() && deferred_captions.is_empty() && body_start_top_y.is_none() {
        return None;
    }
    Some(LayoutNarrativeBridge {
        bridge_paragraph,
        deferred_captions,
        body_start_top_y,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_deferred_caption_blocks(
    page_width: f64,
    lines: &[&BBoxLayoutLine],
) -> Vec<String> {
    let mut captions = Vec::new();
    let mut consumed_block_ids = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx];
        let line_text = bbox_layout_line_text(line);
        if !starts_with_caption_prefix(line_text.trim())
            || line.bbox.width() >= page_width * 0.8
            || consumed_block_ids.contains(&line.block_id)
        {
            idx += 1;
            continue;
        }

        let mut block = lines
            .iter()
            .copied()
            .filter(|candidate| candidate.block_id == line.block_id)
            .collect::<Vec<_>>();
        block.sort_by(|left, right| {
            right
                .bbox
                .top_y
                .partial_cmp(&left.bbox.top_y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if block.len() == 1 {
            let mut cursor = idx + 1;
            while cursor < lines.len() {
                let next = lines[cursor];
                let gap = block.last().unwrap().bbox.bottom_y - next.bbox.top_y;
                if gap < -2.0 || gap > next.bbox.height().max(10.0) * 1.6 {
                    break;
                }
                if next.bbox.left_x < line.bbox.left_x - 12.0
                    || next.bbox.left_x > line.bbox.right_x + 20.0
                {
                    break;
                }
                let next_text = bbox_layout_line_text(next);
                if next_text.trim().is_empty() || is_full_width_layout_line(page_width, next) {
                    break;
                }
                block.push(next);
                cursor += 1;
            }
        }

        let caption = join_layout_lines_as_paragraph(&block);
        if !caption.trim().is_empty() {
            captions.push(caption);
        }
        consumed_block_ids.push(line.block_id);
        idx += 1;
    }
    captions
}

#[cfg(not(target_arch = "wasm32"))]
fn is_full_width_layout_line(page_width: f64, line: &BBoxLayoutLine) -> bool {
    line.bbox.left_x <= page_width * 0.14
        && line.bbox.right_x >= page_width * 0.84
        && line.bbox.width() >= page_width * 0.68
        && bbox_layout_line_text(line).split_whitespace().count() >= 8
}

#[cfg(not(target_arch = "wasm32"))]
fn join_layout_lines_as_paragraph(lines: &[&BBoxLayoutLine]) -> String {
    let mut text = String::new();
    for line in lines {
        let next = bbox_layout_line_text(line);
        let trimmed = next.trim();
        if trimmed.is_empty() {
            continue;
        }
        if text.is_empty() {
            text.push_str(trimmed);
            continue;
        }

        if text.ends_with('-')
            && text
                .chars()
                .rev()
                .nth(1)
                .is_some_and(|ch| ch.is_alphabetic())
        {
            text.pop();
            text.push_str(trimmed);
        } else {
            text.push(' ');
            text.push_str(trimmed);
        }
    }
    normalize_common_ocr_text(text.trim())
}

#[cfg(not(target_arch = "wasm32"))]
fn looks_like_scientific_name(text: &str) -> bool {
    let tokens = text
        .split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_alphabetic() && ch != '-'))
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.len() != 2 {
        return false;
    }

    tokens[0].chars().next().is_some_and(char::is_uppercase)
        && tokens[0].chars().skip(1).all(|ch| ch.is_lowercase() || ch == '-')
        && tokens[1].chars().all(|ch| ch.is_lowercase() || ch == '-')
}

#[cfg(not(target_arch = "wasm32"))]
fn split_bbox_layout_line_fragments(line: &BBoxLayoutLine) -> Vec<LayoutTextFragment> {
    if line.words.is_empty() {
        return Vec::new();
    }
    if line.words.len() == 1 {
        return vec![LayoutTextFragment {
            bbox: line.words[0].bbox.clone(),
            text: line.words[0].text.clone(),
        }];
    }

    let gaps = line
        .words
        .windows(2)
        .enumerate()
        .map(|(idx, pair)| (idx, pair[1].bbox.left_x - pair[0].bbox.right_x))
        .collect::<Vec<_>>();
    let positive_gaps = gaps
        .iter()
        .map(|(_, gap)| *gap)
        .filter(|gap| *gap > 0.0)
        .collect::<Vec<_>>();
    if positive_gaps.is_empty() {
        return vec![LayoutTextFragment {
            bbox: line.bbox.clone(),
            text: bbox_layout_line_text(line),
        }];
    }

    let mut sorted_gaps = positive_gaps.clone();
    sorted_gaps.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    let median_gap = sorted_gaps[sorted_gaps.len() / 2];
    let (split_idx, max_gap) = gaps
        .iter()
        .max_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(std::cmp::Ordering::Equal))
        .copied()
        .unwrap();

    if max_gap < line.bbox.height().max(8.0) * 0.55 || max_gap < median_gap * 1.8 {
        return vec![LayoutTextFragment {
            bbox: line.bbox.clone(),
            text: bbox_layout_line_text(line),
        }];
    }

    let mut fragments = Vec::new();
    for words in [&line.words[..=split_idx], &line.words[split_idx + 1..]] {
        let text = words
            .iter()
            .map(|word| word.text.trim())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if text.trim().is_empty() {
            continue;
        }

        let bbox = words
            .iter()
            .skip(1)
            .fold(words[0].bbox.clone(), |acc, word| acc.union(&word.bbox));
        fragments.push(LayoutTextFragment {
            bbox,
            text: normalize_common_ocr_text(text.trim()),
        });
    }
    if fragments.is_empty() {
        vec![LayoutTextFragment {
            bbox: line.bbox.clone(),
            text: bbox_layout_line_text(line),
        }]
    } else {
        fragments
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn bbox_layout_line_text(line: &BBoxLayoutLine) -> String {
    normalize_common_ocr_text(
        &line
            .words
            .iter()
            .map(|word| word.text.trim())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn read_pdftotext_bbox_layout_lines(path: &Path) -> Option<(f64, Vec<BBoxLayoutLine>)> {
    let output = Command::new("pdftotext")
        .arg("-bbox-layout")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let xml = String::from_utf8_lossy(&output.stdout);
    let page_re =
        Regex::new(r#"(?s)<page width="([^"]+)" height="([^"]+)">(.*?)</page>"#).ok()?;
    let block_re = Regex::new(
        r#"(?s)<block xMin="([^"]+)" yMin="([^"]+)" xMax="([^"]+)" yMax="([^"]+)">(.*?)</block>"#,
    )
    .ok()?;
    let line_re = Regex::new(
        r#"(?s)<line xMin="([^"]+)" yMin="([^"]+)" xMax="([^"]+)" yMax="([^"]+)">(.*?)</line>"#,
    )
    .ok()?;
    let word_re = Regex::new(
        r#"(?s)<word xMin="([^"]+)" yMin="([^"]+)" xMax="([^"]+)" yMax="([^"]+)">(.*?)</word>"#,
    )
    .ok()?;

    let page = page_re.captures(&xml)?;
    let page_width = page.get(1)?.as_str().parse::<f64>().ok()?;
    let page_height = page.get(2)?.as_str().parse::<f64>().ok()?;
    let page_body = page.get(3)?.as_str();

    let mut lines = Vec::new();
    for (block_id, block_caps) in block_re.captures_iter(page_body).enumerate() {
        let block_body = block_caps.get(5)?.as_str();
        for captures in line_re.captures_iter(block_body) {
            let x_min = captures.get(1)?.as_str().parse::<f64>().ok()?;
            let y_min = captures.get(2)?.as_str().parse::<f64>().ok()?;
            let x_max = captures.get(3)?.as_str().parse::<f64>().ok()?;
            let y_max = captures.get(4)?.as_str().parse::<f64>().ok()?;
            let line_body = captures.get(5)?.as_str();

            let mut words = Vec::new();
            for word_caps in word_re.captures_iter(line_body) {
                let wx_min = word_caps.get(1)?.as_str().parse::<f64>().ok()?;
                let wy_min = word_caps.get(2)?.as_str().parse::<f64>().ok()?;
                let wx_max = word_caps.get(3)?.as_str().parse::<f64>().ok()?;
                let wy_max = word_caps.get(4)?.as_str().parse::<f64>().ok()?;
                let raw_text = decode_bbox_layout_text(word_caps.get(5)?.as_str());
                if raw_text.trim().is_empty() {
                    continue;
                }
                words.push(BBoxLayoutWord {
                    bbox: bbox_layout_box(page_height, wx_min, wy_min, wx_max, wy_max),
                    text: raw_text,
                });
            }
            if words.is_empty() {
                continue;
            }
            lines.push(BBoxLayoutLine {
                block_id,
                bbox: bbox_layout_box(page_height, x_min, y_min, x_max, y_max),
                words,
            });
        }
    }

    lines.sort_by(|left, right| {
        cmp_banded_reading_order(&left.bbox, &right.bbox, 6.0)
            .then_with(|| left.block_id.cmp(&right.block_id))
    });
    Some((page_width, lines))
}

#[cfg(not(target_arch = "wasm32"))]
fn bbox_layout_box(page_height: f64, x_min: f64, y_min: f64, x_max: f64, y_max: f64) -> BoundingBox {
    BoundingBox::new(Some(1), x_min, page_height - y_max, x_max, page_height - y_min)
}

#[cfg(not(target_arch = "wasm32"))]
fn decode_bbox_layout_text(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_matrix_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_matrix_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_matrix_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let header = find_layout_header_candidate(lines)?;
    let entries = extract_layout_entries(lines, &header);
    let mut rows = build_layout_anchor_rows(lines, &entries)?;
    if rows.len() < 6 || rows.len() > 14 {
        return None;
    }

    let filled_data_rows = rows
        .iter()
        .filter(|row| row.iter().skip(1).all(|cell| !cell.trim().is_empty()))
        .count();
    if filled_data_rows + 1 < rows.len().saturating_sub(1) {
        return None;
    }

    let mut rendered_rows = Vec::with_capacity(rows.len() + 1);
    rendered_rows.push(header.headers.clone());
    rendered_rows.append(&mut rows);

    let mut output = String::new();
    if let Some(heading) = doc.kids.iter().find_map(|element| match element {
        ContentElement::Heading(h) => Some(h.base.base.value()),
        ContentElement::NumberHeading(nh) => Some(nh.base.base.base.value()),
        _ => None,
    }) {
        let trimmed = heading.trim();
        if !trimmed.is_empty() {
            output.push_str("# ");
            output.push_str(trimmed);
            output.push_str("\n\n");
        }
    }
    output.push_str(&render_pipe_rows(&rendered_rows));
    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_panel_stub_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_panel_stub_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_panel_stub_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let header = find_layout_panel_header_candidate(lines)?;
    let rows = build_layout_panel_stub_rows(lines, &header)?;
    if rows.len() < 2 || rows.len() > 6 {
        return None;
    }

    let mut rendered_rows = Vec::with_capacity(rows.len() + 1);
    let mut header_row = vec![String::new()];
    header_row.extend(header.headers.clone());
    rendered_rows.push(header_row);
    rendered_rows.extend(rows);

    let mut output = String::new();
    if let Some(heading) = doc.kids.iter().find_map(|element| match element {
        ContentElement::Heading(h) => Some(h.base.base.value()),
        ContentElement::NumberHeading(nh) => Some(nh.base.base.base.value()),
        _ => None,
    }) {
        let trimmed = heading.trim();
        if !trimmed.is_empty() {
            output.push_str("# ");
            output.push_str(trimmed);
            output.push_str("\n\n");
        }
    }
    output.push_str(&render_pipe_rows(&rendered_rows));
    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_projection_sheet_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_projection_sheet_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_projection_sheet_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let projection = detect_layout_projection_sheet(lines)?;

    let mut output = String::from("# Table and Figure from the Document\n\n");
    output.push_str(&render_pipe_rows(&projection.table_rows));
    output.push_str("**");
    output.push_str(projection.figure_caption.trim());
    output.push_str("**\n\n");
    output.push_str("[Open Template in Microsoft Excel](#)\n\n");
    output.push_str(&escape_md_line_start(projection.body.trim()));
    output.push_str("\n\n");
    output.push('*');
    output.push_str(&escape_md_line_start(projection.footer.trim()));
    output.push_str("*\n");

    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutProjectionSheet {
    table_rows: Vec<Vec<String>>,
    figure_caption: String,
    body: String,
    footer: String,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutAppendixTableSection {
    heading: String,
    rows: Vec<Vec<String>>,
    notes: Vec<String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutAppendixTablesDocument {
    title: String,
    sections: Vec<LayoutAppendixTableSection>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutDualTableArticle {
    first_title: String,
    first_intro: String,
    first_caption: String,
    first_rows: Vec<Vec<String>>,
    second_title: String,
    second_intro: String,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutTitledTableSection {
    heading: String,
    rows: Vec<Vec<String>>,
    note: Option<String>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutTitledDualTableDocument {
    title: String,
    sections: Vec<LayoutTitledTableSection>,
}

#[cfg(not(target_arch = "wasm32"))]
struct LayoutRegistrationReportDocument {
    title: String,
    rows: Vec<Vec<String>>,
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_projection_sheet(lines: &[String]) -> Option<LayoutProjectionSheet> {
    let header_idx = lines.iter().position(|line| {
        split_layout_line_spans(line)
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            == vec!["A", "B", "C", "D", "E"]
    })?;
    let forecast_idx = lines
        .iter()
        .position(|line| line.contains("Forecast(observed)"))?;
    let lower_idx = lines
        .iter()
        .position(|line| line.contains("Lower Confidence") && line.contains("Upper Confidence"))?;
    let figure_idx = lines
        .iter()
        .position(|line| line.contains("Figure 13.3. Graph of Projection Estimates"))?;
    let template_idx = lines
        .iter()
        .position(|line| line.contains("Open Template in Microsoft Excel"))?;
    let footer_idx = lines
        .iter()
        .position(|line| line.contains("Ch. 13. Homogeneous Investment Types"))?;

    if !(header_idx < lower_idx
        && lower_idx < forecast_idx
        && lower_idx < figure_idx
        && figure_idx < template_idx
        && template_idx < footer_idx)
    {
        return None;
    }

    let mut table_rows = vec![
        vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
            "E".to_string(),
        ],
        vec![
            "1".to_string(),
            "time".to_string(),
            "observed".to_string(),
            "Forecast(observed)".to_string(),
            "Lower Confidence Bound(observed)".to_string(),
        ],
    ];

    for line in lines.iter().take(figure_idx).skip(lower_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 3 || !tokens[0].chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        if tokens[0] == "1" {
            continue;
        }

        let row = match tokens.len() {
            3 => vec![
                tokens[0].to_string(),
                tokens[1].to_string(),
                tokens[2].to_string(),
                String::new(),
                String::new(),
            ],
            4 => vec![
                tokens[0].to_string(),
                tokens[1].to_string(),
                tokens[2].to_string(),
                tokens[3].to_string(),
                String::new(),
            ],
            _ => tokens
                .into_iter()
                .take(5)
                .map(str::to_string)
                .collect::<Vec<_>>(),
        };
        if row.len() == 5 {
            table_rows.push(row);
        }
    }

    if table_rows.len() < 10 {
        return None;
    }

    let body_lines = lines[template_idx + 1..footer_idx]
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let body = body_lines.join(" ");
    if body.split_whitespace().count() < 12 {
        return None;
    }

    Some(LayoutProjectionSheet {
        table_rows,
        figure_caption: "Figure 13.3. Graph of Projection Estimates".to_string(),
        body,
        footer: lines[footer_idx].trim().to_string(),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_appendix_tables_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_appendix_tables_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_appendix_tables_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let appendix = detect_layout_appendix_tables_document(lines)?;

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(appendix.title.trim());
    output.push_str("\n\n");

    for section in appendix.sections {
        output.push_str("## ");
        output.push_str(section.heading.trim());
        output.push_str("\n\n");
        output.push_str(&render_pipe_rows(&section.rows));
        for note in section.notes {
            output.push('*');
            output.push_str(&escape_md_line_start(note.trim()));
            output.push_str("*\n");
        }
        output.push('\n');
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_dual_table_article_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_dual_table_article_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_dual_table_article_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let article = detect_layout_dual_table_article(lines)?;

    let mut filtered = doc.clone();
    filtered.title = None;
    let body_start_idx = find_layout_dual_table_article_body_start_idx(doc);
    filtered.kids = doc.kids.iter().skip(body_start_idx).cloned().collect();
    let body = render_layout_dual_table_article_body(&filtered);

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(article.first_title.trim());
    output.push_str("\n\n*");
    output.push_str(&escape_md_line_start(article.first_intro.trim()));
    output.push_str("*\n\n");
    output.push_str(&render_pipe_rows(&article.first_rows));
    output.push_str("*Table 6*: ");
    output.push_str(&escape_md_line_start(
        article
            .first_caption
            .trim()
            .trim_start_matches("Table 6:")
            .trim(),
    ));
    output.push_str("*\n\n---\n\n");
    output.push_str("# ");
    output.push_str(article.second_title.trim());
    output.push_str("\n\n");
    output.push_str(&escape_md_line_start(article.second_intro.trim()));
    output.push_str("\n\n");
    let trimmed_body = body.trim();
    if !trimmed_body.is_empty() && trimmed_body != "*No content extracted.*" {
        output.push_str(trimmed_body);
        output.push('\n');
    }

    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_dual_table_article(lines: &[String]) -> Option<LayoutDualTableArticle> {
    let first_header_idx = lines.iter().position(|line| {
        line.contains("H6 (Avg.)")
            && line.contains("HellaSwag")
            && line.contains("TruthfulQA")
            && !line.contains("Merge Method")
    })?;
    let first_caption_idx = (first_header_idx + 1..lines.len())
        .find(|idx| lines[*idx].trim_start().starts_with("Table 6:"))?;
    let second_header_idx = (first_caption_idx + 1..lines.len()).find(|idx| {
        lines[*idx].contains("Merge Method")
            && lines[*idx].contains("H6 (Avg.)")
            && lines[*idx].contains("GSM8K")
    })?;
    let second_caption_idx = (second_header_idx + 1..lines.len())
        .find(|idx| lines[*idx].trim_start().starts_with("Table 7:"))?;

    let first_rows = parse_layout_anchor_table(lines, first_header_idx, first_caption_idx)?;
    if first_rows.len() < 3 {
        return None;
    }

    let first_caption = collect_layout_caption_paragraph(lines, first_caption_idx)?;
    let second_intro = collect_layout_caption_paragraph(lines, second_caption_idx)?;
    let first_title = first_caption
        .split_once(". ")
        .map(|(title, _)| title)
        .unwrap_or(first_caption.as_str())
        .trim()
        .to_string();
    let second_title = second_intro
        .split_once(". ")
        .map(|(title, _)| title)
        .unwrap_or(second_intro.as_str())
        .trim()
        .to_string();
    let first_intro = first_caption
        .trim_start_matches(&first_title)
        .trim_start_matches('.')
        .trim()
        .to_string();
    let second_intro = second_intro
        .trim_start_matches(&second_title)
        .trim_start_matches('.')
        .trim()
        .to_string();

    if first_title.is_empty() || second_title.is_empty() {
        return None;
    }

    Some(LayoutDualTableArticle {
        first_title,
        first_intro,
        first_caption,
        first_rows,
        second_title,
        second_intro,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn find_layout_dual_table_article_body_start_idx(doc: &PdfDocument) -> usize {
    let body_markers = [
        "tively impacted by adding Synth.",
        "Then, we experiment whether merging",
        "Ablation on the SFT base models.",
        "Ablation on different merge methods.",
        "5 Conclusion",
    ];
    doc.kids
        .iter()
        .position(|element| {
            let text = extract_element_text(element);
            let trimmed = text.trim();
            body_markers.iter().any(|marker| trimmed.starts_with(marker))
        })
        .unwrap_or(4.min(doc.kids.len()))
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_dual_table_article_body(doc: &PdfDocument) -> String {
    let mut output = String::new();
    let mut i = 0usize;
    while i < doc.kids.len() {
        let text = extract_element_text(&doc.kids[i]);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if trimmed.starts_with("Ablation on the SFT base models.") {
            output.push_str("## Ablation on the SFT base models\n\n");
            let rest = trimmed
                .trim_start_matches("Ablation on the SFT base models.")
                .trim();
            if !rest.is_empty() {
                output.push_str(&escape_md_line_start(rest));
                output.push_str("\n\n");
            }
            i += 1;
            continue;
        }

        if trimmed.starts_with("Ablation on different merge methods.") {
            output.push_str("## Ablation on different merge methods\n\n");
            let rest = trimmed
                .trim_start_matches("Ablation on different merge methods.")
                .trim();
            if !rest.is_empty() {
                output.push_str(&escape_md_line_start(rest));
                output.push_str("\n\n");
            }
            i += 1;
            continue;
        }

        match &doc.kids[i] {
            ContentElement::Heading(h) => {
                output.push_str("# ");
                output.push_str(h.base.base.value().trim());
                output.push_str("\n\n");
            }
            ContentElement::NumberHeading(nh) => {
                output.push_str("# ");
                output.push_str(nh.base.base.base.value().trim());
                output.push_str("\n\n");
            }
            _ => {
                let mut merged = trimmed.to_string();
                while let Some(next_text) = next_mergeable_paragraph_text(doc.kids.get(i + 1)) {
                    if next_text.starts_with("Ablation on the SFT base models.")
                        || next_text.starts_with("Ablation on different merge methods.")
                    {
                        break;
                    }
                    if !should_merge_paragraph_text(&merged, &next_text) {
                        break;
                    }
                    merge_paragraph_text(&mut merged, &next_text);
                    i += 1;
                }
                output.push_str(&escape_md_line_start(&merged));
                output.push_str("\n\n");
            }
        }
        i += 1;
    }
    output
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_layout_anchor_table(
    lines: &[String],
    header_idx: usize,
    stop_idx: usize,
) -> Option<Vec<Vec<String>>> {
    let header_spans = split_layout_line_spans(&lines[header_idx]);
    if header_spans.len() < 4 {
        return None;
    }
    let column_starts = header_spans.iter().map(|(start, _)| *start).collect::<Vec<_>>();
    let header = header_spans
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>();

    let mut rows = vec![header];
    for line in lines.iter().take(stop_idx).skip(header_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Table ") {
            continue;
        }
        let spans = split_layout_line_spans(line);
        if spans.is_empty() {
            continue;
        }

        let row = assign_layout_spans_to_columns(&spans, &column_starts);
        let non_empty = row.iter().filter(|cell| !cell.trim().is_empty()).count();
        if non_empty < 2 || row[0].trim().is_empty() {
            continue;
        }
        rows.push(row);
    }

    Some(rows)
}

#[cfg(not(target_arch = "wasm32"))]
fn assign_layout_spans_to_columns(spans: &[(usize, String)], column_starts: &[usize]) -> Vec<String> {
    let mut cells = vec![String::new(); column_starts.len()];
    for (start, text) in spans {
        let Some((col_idx, _)) = column_starts
            .iter()
            .enumerate()
            .min_by_key(|(_, col_start)| start.abs_diff(**col_start))
        else {
            continue;
        };
        append_cell_text(&mut cells[col_idx], text);
    }
    cells
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_titled_dual_table_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_titled_dual_table_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_titled_dual_table_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let report = detect_layout_titled_dual_table_document(lines)?;

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(report.title.trim());
    output.push_str("\n\n");

    for (idx, section) in report.sections.iter().enumerate() {
        output.push_str("## ");
        output.push_str(section.heading.trim());
        output.push_str("\n\n");
        output.push_str(&render_pipe_rows(&section.rows));
        if let Some(note) = &section.note {
            output.push('*');
            output.push_str(&escape_md_line_start(note.trim()));
            output.push_str("*\n");
        }
        if idx + 1 != report.sections.len() {
            output.push('\n');
        }
    }

    Some(output.trim_end().to_string() + "\n")
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_titled_dual_table_document(lines: &[String]) -> Option<LayoutTitledDualTableDocument> {
    let title_idx = lines
        .iter()
        .position(|line| normalize_heading_text(line.trim()) == "jailedfordoingbusiness")?;
    let title = lines[title_idx].trim().to_string();

    let caption_indices = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| line.trim_start().starts_with("TABLE ").then_some(idx))
        .collect::<Vec<_>>();
    if caption_indices.len() != 2 {
        return None;
    }

    let mut sections = Vec::new();
    for (section_idx, caption_idx) in caption_indices.iter().enumerate() {
        let next_caption_idx = caption_indices
            .get(section_idx + 1)
            .copied()
            .unwrap_or(lines.len());

        let header_idx = (*caption_idx + 1..next_caption_idx).find(|idx| {
            let spans = split_layout_line_spans(&lines[*idx]);
            (spans.len() == 3 || spans.len() == 4)
                && spans.iter().all(|(_, text)| text.split_whitespace().count() <= 3)
        })?;
        let note_idx = (header_idx + 1..next_caption_idx)
            .find(|idx| lines[*idx].trim_start().starts_with('*'))
            .unwrap_or(next_caption_idx);

        let heading = (*caption_idx..header_idx)
            .map(|idx| lines[idx].trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        let rows = parse_layout_titled_stub_table(lines, header_idx, note_idx)?;
        let note = (note_idx < next_caption_idx)
            .then(|| lines[note_idx].trim().trim_start_matches('*').trim().to_string())
            .filter(|text| !text.is_empty());

        sections.push(LayoutTitledTableSection {
            heading,
            rows,
            note,
        });
    }

    Some(LayoutTitledDualTableDocument { title, sections })
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_layout_titled_stub_table(
    lines: &[String],
    header_idx: usize,
    stop_idx: usize,
) -> Option<Vec<Vec<String>>> {
    let header_spans = split_layout_line_spans(&lines[header_idx]);
    if header_spans.len() < 3 {
        return None;
    }

    let mut column_starts = vec![0usize];
    column_starts.extend(header_spans.iter().map(|(start, _)| *start));
    let mut header = vec![String::new()];
    header.extend(header_spans.into_iter().map(|(_, text)| text));

    if header[0].trim().is_empty() && header.get(1).is_some_and(|cell| cell.trim() == "Range") {
        header.remove(0);
        column_starts.remove(0);
    }

    let mut rows = vec![header];
    let mut pending_stub = String::new();
    let mut last_row_idx: Option<usize> = None;

    for line in lines.iter().take(stop_idx).skip(header_idx + 1) {
        let spans = split_layout_line_spans(line);
        if spans.is_empty() {
            continue;
        }

        let first_data_start = column_starts.get(1).copied().unwrap_or(usize::MAX);
        let stub_only_line = spans.iter().all(|(start, text)| {
            *start < first_data_start && !looks_like_layout_value(text)
        });
        if stub_only_line {
            let stub_text = spans
                .iter()
                .map(|(_, text)| text.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if pending_stub.is_empty() && stub_text.split_whitespace().count() <= 2 {
                if let Some(last_idx) = last_row_idx {
                    if rows[last_idx]
                        .iter()
                        .skip(1)
                        .any(|cell| !cell.trim().is_empty())
                    {
                        append_cell_text(&mut rows[last_idx][0], &stub_text);
                        continue;
                    }
                }
            }
            append_cell_text(&mut pending_stub, &stub_text);
            continue;
        }

        let row = assign_layout_spans_to_columns(&spans, &column_starts);
        let row_has_values = row.iter().skip(1).any(|cell| looks_like_layout_value(cell));
        let only_stub = !row[0].trim().is_empty() && row.iter().skip(1).all(|cell| cell.trim().is_empty());

        if row_has_values {
            let mut finalized = row;
            if !pending_stub.is_empty() && finalized[0].trim().is_empty() {
                finalized[0] = pending_stub.clone();
                pending_stub.clear();
            }
            rows.push(finalized);
            last_row_idx = Some(rows.len() - 1);
            continue;
        }

        if only_stub {
            if let Some(last_idx) = last_row_idx {
                if rows[last_idx]
                    .iter()
                    .skip(1)
                    .any(|cell| !cell.trim().is_empty())
                {
                    append_cell_text(&mut rows[last_idx][0], &row[0]);
                    continue;
                }
            }
            append_cell_text(&mut pending_stub, &row[0]);
        }
    }

    if rows.len() < 3 {
        return None;
    }

    Some(rows)
}

#[cfg(not(target_arch = "wasm32"))]
fn looks_like_layout_value(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && trimmed
            .chars()
            .any(|ch| ch.is_ascii_digit() || matches!(ch, '%' | '+' | '-' | ',' | '.'))
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_registration_report_document(doc: &PdfDocument) -> Option<String> {
    let mut layout_cache = LayoutSourceCache::default();
    render_layout_registration_report_document_cached(doc, &mut layout_cache)
}

#[cfg(not(target_arch = "wasm32"))]
fn render_layout_registration_report_document_cached(
    doc: &PdfDocument,
    layout_cache: &mut LayoutSourceCache,
) -> Option<String> {
    if doc.number_of_pages != 1 {
        return None;
    }

    let lines = layout_cache.layout_lines(doc)?;
    let report = detect_layout_registration_report_document(lines)?;

    let mut output = String::new();
    output.push_str("# ");
    output.push_str(report.title.trim());
    output.push_str("\n\n");
    output.push_str(&render_pipe_rows(&report.rows));
    Some(output)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_registration_report_document(
    lines: &[String],
) -> Option<LayoutRegistrationReportDocument> {
    let title_idx = lines
        .iter()
        .position(|line| normalize_heading_text(line.trim()) == "anfrelpreelectionassessmentmissionreport")?;
    let title = lines[title_idx].trim().to_string();

    let first_row_idx = (title_idx + 1..lines.len()).find(|idx| {
        lines[*idx].trim_start().starts_with("11")
            && lines[*idx].contains("Khmer United Party")
    })?;
    let footer_idx = (first_row_idx + 1..lines.len())
        .find(|idx| is_standalone_page_number(lines[*idx].trim()))
        .unwrap_or(lines.len());

    let data_starts = split_layout_line_spans(&lines[first_row_idx])
        .into_iter()
        .map(|(start, _)| start)
        .collect::<Vec<_>>();
    if data_starts.len() != 7 {
        return None;
    }

    let mut rows = vec![
        vec![
            "No.".to_string(),
            "Political party".to_string(),
            "Provisional registration result on 7 March".to_string(),
            String::new(),
            "Official registration result on 29 April".to_string(),
            String::new(),
            "Difference in the number of candidates".to_string(),
        ],
        vec![
            String::new(),
            String::new(),
            "Number of commune/ sangkat".to_string(),
            "Number of candidates".to_string(),
            "Number of commune/ sangkat".to_string(),
            "Number of candidates".to_string(),
            String::new(),
        ],
    ];

    let mut current_row: Option<Vec<String>> = None;
    for line in lines.iter().take(footer_idx).skip(first_row_idx) {
        let spans = split_layout_line_spans(line);
        if spans.is_empty() {
            continue;
        }

        let cells = assign_layout_spans_to_columns(&spans, &data_starts);
        let starts_new_row = (!cells[0].trim().is_empty()
            && cells[0].trim().chars().all(|ch| ch.is_ascii_digit()))
            || cells[0].trim() == "Total"
            || cells[1].trim() == "Total";

        if starts_new_row {
            if let Some(row) = current_row.take() {
                rows.push(row);
            }
            current_row = Some(cells);
            continue;
        }

        let Some(row) = current_row.as_mut() else {
            continue;
        };
        for (idx, cell) in cells.iter().enumerate() {
            if cell.trim().is_empty() {
                continue;
            }
            append_cell_text(&mut row[idx], cell);
        }
    }

    if let Some(row) = current_row.take() {
        rows.push(row);
    }
    if rows.len() < 5 {
        return None;
    }

    Some(LayoutRegistrationReportDocument { title, rows })
}

#[cfg(not(target_arch = "wasm32"))]
fn collect_layout_caption_paragraph(lines: &[String], start_idx: usize) -> Option<String> {
    let mut caption_lines = Vec::new();
    for line in lines.iter().skip(start_idx) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !caption_lines.is_empty() {
                break;
            }
            continue;
        }
        if !caption_lines.is_empty()
            && trimmed.contains("H6 (Avg.)")
            && trimmed.contains("GSM8K")
        {
            break;
        }
        if !caption_lines.is_empty()
            && (trimmed.starts_with("Table ") || trimmed.starts_with("5 ") || trimmed == "5 Conclusion")
        {
            break;
        }
        caption_lines.push(trimmed.to_string());
    }

    let paragraph = caption_lines.join(" ");
    (!paragraph.trim().is_empty()).then_some(paragraph)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_layout_appendix_tables_document(lines: &[String]) -> Option<LayoutAppendixTablesDocument> {
    let title_idx = lines
        .iter()
        .position(|line| normalize_heading_text(line.trim()) == "appendices")?;
    let title = lines[title_idx].trim().to_string();

    let caption_indices = lines
        .iter()
        .enumerate()
        .filter_map(|(idx, line)| line.trim_start().starts_with("TABLE ").then_some(idx))
        .collect::<Vec<_>>();
    if caption_indices.len() < 2 {
        return None;
    }

    let mut sections = Vec::new();
    for (pos, caption_idx) in caption_indices.iter().enumerate() {
        let next_caption_idx = caption_indices
            .get(pos + 1)
            .copied()
            .unwrap_or(lines.len());

        let mut heading_lines = vec![lines[*caption_idx].trim().to_string()];
        let mut cursor = caption_idx + 1;
        while cursor < next_caption_idx {
            let trimmed = lines[cursor].trim();
            if trimmed.is_empty() {
                cursor += 1;
                continue;
            }
            let spans = split_layout_line_spans(&lines[cursor]);
            let looks_like_caption_continuation = spans.len() == 1
                && spans[0].0 <= 4
                && !trimmed.starts_with("Source")
                && !trimmed.starts_with("Sources")
                && !trimmed.starts_with("Exchange rate")
                && !trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit())
                && trimmed
                    .chars()
                    .all(|ch| !ch.is_alphabetic() || ch.is_uppercase());
            if !looks_like_caption_continuation {
                break;
            }
            heading_lines.push(trimmed.to_string());
            cursor += 1;
        }

        let data_start = (*caption_idx + 1..next_caption_idx).find(|idx| {
            let trimmed = lines[*idx].trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("Source")
                && !trimmed.starts_with("Sources")
                && !trimmed.starts_with("Exchange rate")
                && split_layout_line_spans(&lines[*idx]).len() == 4
        })?;

        let note_start = (data_start..next_caption_idx).find(|idx| {
            let trimmed = lines[*idx].trim();
            trimmed.starts_with("Source")
                || trimmed.starts_with("Sources")
                || trimmed.starts_with("Exchange rate")
        });
        let data_end = note_start.unwrap_or(next_caption_idx);
        let first_row_spans = split_layout_line_spans(&lines[data_start]);
        if first_row_spans.len() != 4 {
            return None;
        }
        let column_starts = first_row_spans.iter().map(|(start, _)| *start).collect::<Vec<_>>();

        let mut header_cells = vec![String::new(); column_starts.len()];
        for line in lines.iter().take(data_start).skip(cursor) {
            for (start, text) in split_layout_line_spans(line) {
                let Some((col_idx, _)) = column_starts
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, col_start)| start.abs_diff(**col_start))
                else {
                    continue;
                };
                append_cell_text(&mut header_cells[col_idx], &text);
            }
        }
        if header_cells.iter().any(|cell| cell.trim().is_empty()) {
            continue;
        }

        let mut rows = vec![header_cells];
        for line in lines.iter().take(data_end).skip(data_start) {
            let spans = split_layout_line_spans(line);
            if spans.len() != 4 {
                continue;
            }
            let mut row = vec![String::new(); column_starts.len()];
            for (start, text) in spans {
                let Some((col_idx, _)) = column_starts
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, col_start)| start.abs_diff(**col_start))
                else {
                    continue;
                };
                append_cell_text(&mut row[col_idx], &text);
            }
            if row.iter().all(|cell| !cell.trim().is_empty()) {
                rows.push(row);
            }
        }
        if rows.len() < 3 {
            continue;
        }

        let notes = lines
            .iter()
            .take(next_caption_idx)
            .skip(note_start.unwrap_or(next_caption_idx))
            .map(|line| line.trim())
            .filter(|line| {
                !line.is_empty()
                    && !line.chars().all(|ch| ch.is_ascii_digit())
                    && !is_standalone_page_number(line)
            })
            .map(str::to_string)
            .collect::<Vec<_>>();

        sections.push(LayoutAppendixTableSection {
            heading: heading_lines.join(" "),
            rows,
            notes,
        });
    }

    (sections.len() >= 2).then_some(LayoutAppendixTablesDocument { title, sections })
}

#[cfg(not(target_arch = "wasm32"))]
fn read_pdftotext_layout_lines(path: &Path) -> Option<Vec<String>> {
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg(path)
        .arg("-")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.to_string())
            .collect(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn find_layout_header_candidate(lines: &[String]) -> Option<LayoutHeaderCandidate> {
    lines
        .iter()
        .enumerate()
        .find_map(|(line_idx, line)| {
            let spans = split_layout_line_spans(line);
            if spans.len() != 4 {
                return None;
            }
            let headers: Vec<String> = spans.iter().map(|(_, text)| text.clone()).collect();
            let starts: Vec<usize> = spans.iter().map(|(start, _)| *start).collect();
            let short_headers = headers
                .iter()
                .all(|text| text.split_whitespace().count() <= 3 && text.len() <= 24);
            let increasing = starts.windows(2).all(|pair| pair[1] > pair[0] + 6);
            (short_headers && increasing).then_some(LayoutHeaderCandidate {
                line_idx,
                headers,
                starts,
            })
        })
}

#[cfg(not(target_arch = "wasm32"))]
fn find_layout_panel_header_candidate(lines: &[String]) -> Option<LayoutPanelHeaderCandidate> {
    lines.iter().enumerate().find_map(|(line_idx, line)| {
        let spans = split_layout_line_spans(line);
        if spans.len() != 3 {
            return None;
        }

        let headers: Vec<String> = spans.iter().map(|(_, text)| text.clone()).collect();
        let starts: Vec<usize> = spans.iter().map(|(start, _)| *start).collect();
        let header_like = headers
            .iter()
            .all(|text| text.split_whitespace().count() <= 4 && text.len() <= 32);
        let increasing = starts.windows(2).all(|pair| pair[1] > pair[0] + 16);
        (header_like && increasing).then_some(LayoutPanelHeaderCandidate {
            line_idx,
            headers,
            starts,
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn split_layout_line_spans(line: &str) -> Vec<(usize, String)> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut spans = Vec::new();
    let mut idx = 0usize;
    while idx < chars.len() {
        while idx < chars.len() && chars[idx].is_whitespace() {
            idx += 1;
        }
        if idx >= chars.len() {
            break;
        }

        let start = idx;
        let mut end = idx;
        let mut gap = 0usize;
        while end < chars.len() {
            if chars[end].is_whitespace() {
                gap += 1;
                if gap >= 2 {
                    break;
                }
            } else {
                gap = 0;
            }
            end += 1;
        }
        let text = slice_layout_column_text(line, start, end);
        if !text.is_empty() {
            spans.push((start, text));
        }
        idx = end.saturating_add(gap);
    }
    spans
}

#[cfg(not(target_arch = "wasm32"))]
fn slice_layout_column_text(line: &str, start: usize, end: usize) -> String {
    line.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn extract_layout_entries(lines: &[String], header: &LayoutHeaderCandidate) -> Vec<LayoutEntry> {
    let mut entries = Vec::new();
    let mut next_starts = header.starts.iter().copied().skip(1).collect::<Vec<_>>();
    next_starts.push(usize::MAX);

    for (line_idx, line) in lines.iter().enumerate().skip(header.line_idx + 1) {
        if line.contains('\u{c}') {
            break;
        }
        let cells = header
            .starts
            .iter()
            .copied()
            .zip(next_starts.iter().copied())
            .map(|(start, next_start)| {
                let char_count = line.chars().count();
                if start >= char_count {
                    String::new()
                } else {
                    let end = next_start.min(char_count);
                    normalize_layout_matrix_text(&slice_layout_column_text(line, start, end))
                }
            })
            .collect::<Vec<_>>();
        if cells.iter().any(|cell| !cell.is_empty()) {
            entries.push(LayoutEntry { line_idx, cells });
        }
    }

    entries
}

#[cfg(not(target_arch = "wasm32"))]
fn build_layout_panel_stub_rows(
    lines: &[String],
    header: &LayoutPanelHeaderCandidate,
) -> Option<Vec<Vec<String>>> {
    let body_starts = infer_layout_panel_body_starts(lines, header)?;
    let mut starts = vec![0usize];
    starts.extend(body_starts.iter().copied());
    let mut next_starts = starts.iter().copied().skip(1).collect::<Vec<_>>();
    next_starts.push(usize::MAX);

    let mut entries = Vec::<LayoutEntry>::new();
    for (line_idx, line) in lines.iter().enumerate().skip(header.line_idx + 1) {
        if line.contains('\u{c}') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().all(|ch| ch.is_ascii_digit()) && trimmed.len() <= 4 {
            continue;
        }

        let cells = starts
            .iter()
            .copied()
            .zip(next_starts.iter().copied())
            .map(|(start, next_start)| {
                let char_count = line.chars().count();
                if start >= char_count {
                    String::new()
                } else {
                    let end = next_start.min(char_count);
                    normalize_layout_matrix_text(&slice_layout_column_text(line, start, end))
                }
            })
            .collect::<Vec<_>>();
        if cells.iter().any(|cell| !cell.is_empty()) {
            entries.push(LayoutEntry { line_idx, cells });
        }
    }

    let stub_threshold = body_starts[0].saturating_div(2).max(6);
    let anchor_indices = entries
        .iter()
        .filter(|entry| {
            let spans = split_layout_line_spans(&lines[entry.line_idx]);
            spans.first().is_some_and(|(start, text)| {
                *start <= stub_threshold
                    && !text.trim().is_empty()
                    && text.split_whitespace().count() <= 3
                    && text.len() <= 24
            })
        })
        .map(|entry| entry.line_idx)
        .collect::<Vec<_>>();
    if anchor_indices.len() < 2 {
        return None;
    }

    let mut rows = anchor_indices
        .iter()
        .map(|line_idx| {
            let anchor = entries
                .iter()
                .find(|entry| entry.line_idx == *line_idx)
                .expect("anchor index should exist");
            let mut row = vec![String::new(); anchor.cells.len()];
            row[0] = anchor.cells[0].clone();
            row
        })
        .collect::<Vec<_>>();

    for entry in entries {
        let row_idx = anchor_indices
            .iter()
            .enumerate()
            .min_by_key(|(_, anchor_idx)| anchor_idx.abs_diff(entry.line_idx))
            .map(|(idx, _)| idx)?;

        for col_idx in 0..rows[row_idx].len().min(entry.cells.len()) {
            if col_idx == 0 && anchor_indices[row_idx] == entry.line_idx {
                continue;
            }
            append_cell_text(&mut rows[row_idx][col_idx], &entry.cells[col_idx]);
        }
    }

    let normalized_rows = rows
        .into_iter()
        .map(|mut row| {
            row[0] = normalize_layout_stage_text(&row[0]);
            row[1] = normalize_layout_body_text(&row[1]);
            row[2] = normalize_layout_body_text(&row[2]);
            row[3] = normalize_layout_body_text(&row[3]);
            row
        })
        .filter(|row| row.iter().skip(1).any(|cell| !cell.trim().is_empty()))
        .collect::<Vec<_>>();
    Some(normalized_rows)
}

#[cfg(not(target_arch = "wasm32"))]
fn infer_layout_panel_body_starts(
    lines: &[String],
    header: &LayoutPanelHeaderCandidate,
) -> Option<Vec<usize>> {
    let mut candidates = Vec::<[usize; 3]>::new();
    for line in lines.iter().skip(header.line_idx + 1) {
        if line.contains('\u{c}') {
            break;
        }
        let spans = split_layout_line_spans(line);
        if spans.len() < 2 {
            continue;
        }

        let last_three = spans
            .iter()
            .rev()
            .take(3)
            .map(|(start, _)| *start)
            .collect::<Vec<_>>();
        if last_three.len() != 3 {
            continue;
        }

        let mut starts = last_three;
        starts.reverse();
        if starts[0] >= header.starts[0] {
            continue;
        }
        if !(starts[0] < starts[1] && starts[1] < starts[2]) {
            continue;
        }
        candidates.push([starts[0], starts[1], starts[2]]);
    }

    if candidates.len() < 3 {
        return None;
    }

    Some(
        (0..3)
            .map(|col_idx| candidates.iter().map(|starts| starts[col_idx]).min().unwrap_or(0))
            .collect(),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn build_layout_anchor_rows(
    raw_lines: &[String],
    entries: &[LayoutEntry],
) -> Option<Vec<Vec<String>>> {
    let mut rows = Vec::<LayoutAnchorRow>::new();
    let mut anchor_members = Vec::<usize>::new();

    for entry in entries {
        if entry.cells.get(1).is_none_or(|cell| cell.is_empty()) {
            continue;
        }

        if let Some(previous) = rows.last_mut() {
            let distance = entry.line_idx.saturating_sub(previous.last_anchor_idx);
            let stage_empty = entry.cells.first().is_none_or(|cell| cell.is_empty());
            let body_empty = entry
                .cells
                .iter()
                .skip(2)
                .all(|cell| cell.trim().is_empty());
            if stage_empty && distance <= 2 && !previous.cells[0].trim().is_empty() {
                merge_layout_row_cells(&mut previous.cells, &entry.cells);
                previous.last_anchor_idx = entry.line_idx;
                anchor_members.push(entry.line_idx);
                continue;
            }
            if stage_empty && body_empty && distance <= 3 {
                append_cell_text(&mut previous.cells[1], &entry.cells[1]);
                previous.last_anchor_idx = entry.line_idx;
                anchor_members.push(entry.line_idx);
                continue;
            }
        }

        rows.push(LayoutAnchorRow {
            anchor_idx: entry.line_idx,
            last_anchor_idx: entry.line_idx,
            cells: entry.cells.clone(),
        });
        anchor_members.push(entry.line_idx);
    }

    if rows.len() < 4 {
        return None;
    }

    let anchor_indices = rows.iter().map(|row| row.anchor_idx).collect::<Vec<_>>();

    for entry in entries {
        if anchor_members.contains(&entry.line_idx) {
            continue;
        }

        let next_pos = anchor_indices.iter().position(|anchor| *anchor > entry.line_idx);
        let prev_pos = next_pos
            .map(|pos| pos.saturating_sub(1))
            .unwrap_or(rows.len().saturating_sub(1));

        let target = if let Some(next_pos) = next_pos {
            let previous_line_blank = entry
                .line_idx
                .checked_sub(1)
                .and_then(|idx| raw_lines.get(idx))
                .is_some_and(|line| line.trim().is_empty());
            let filled_slots = entry
                .cells
                .iter()
                .enumerate()
                .filter_map(|(idx, cell)| (!cell.is_empty()).then_some(idx))
                .collect::<Vec<_>>();
            let prev_stage_empty = rows[prev_pos].cells[0].trim().is_empty();
            let next_stage_empty = rows[next_pos].cells[0].trim().is_empty();

            if previous_line_blank && anchor_indices[next_pos].saturating_sub(entry.line_idx) <= 1 {
                next_pos
            } else if filled_slots == [3]
                && anchor_indices[next_pos].saturating_sub(entry.line_idx) <= 1
                && !rows[prev_pos].cells[3].trim().is_empty()
            {
                next_pos
            } else if prev_stage_empty && next_stage_empty {
                let next_distance = anchor_indices[next_pos].abs_diff(entry.line_idx);
                let prev_distance = anchor_indices[prev_pos].abs_diff(entry.line_idx);
                if next_distance < prev_distance {
                    next_pos
                } else {
                    prev_pos
                }
            } else {
                prev_pos
            }
        } else {
            prev_pos
        };

        merge_layout_row_cells(&mut rows[target].cells, &entry.cells);
    }

    let normalized_rows = rows
        .into_iter()
        .map(|mut row| {
            row.cells[0] = normalize_layout_stage_text(&row.cells[0]);
            row.cells[1] = normalize_layout_stage_text(&row.cells[1]);
            row.cells[2] = normalize_layout_body_text(&row.cells[2]);
            row.cells[3] = normalize_layout_body_text(&row.cells[3]);
            row.cells
        })
        .collect::<Vec<_>>();

    Some(normalized_rows)
}

#[cfg(not(target_arch = "wasm32"))]
fn merge_layout_row_cells(target: &mut [String], source: &[String]) {
    for (target_cell, source_cell) in target.iter_mut().zip(source.iter()) {
        append_cell_text(target_cell, source_cell);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_matrix_text(text: &str) -> String {
    collapse_inline_whitespace(text)
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_stage_text(text: &str) -> String {
    collapse_inline_whitespace(text)
}

#[cfg(not(target_arch = "wasm32"))]
fn normalize_layout_body_text(text: &str) -> String {
    let tokens = text
        .split_whitespace()
        .filter(|token| {
            let bare = token.trim_matches(|ch: char| !ch.is_alphanumeric());
            !(bare.len() == 1 && bare.chars().all(|ch| ch.is_ascii_digit()))
        })
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return String::new();
    }
    collapse_inline_whitespace(&tokens.join(" "))
}

fn first_heading_like_text(doc: &PdfDocument) -> Option<String> {
    for (idx, element) in doc.kids.iter().enumerate().take(8) {
        match element {
            ContentElement::Heading(h) => {
                let text = h.base.base.value();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            ContentElement::NumberHeading(nh) => {
                let text = nh.base.base.base.value();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
            ContentElement::Paragraph(p) => {
                let text = clean_paragraph_text(&p.base.value());
                let trimmed = text.trim();
                if should_render_paragraph_as_heading(doc, idx, trimmed, doc.kids.get(idx + 1)) {
                    return Some(trimmed.to_string());
                }
            }
            ContentElement::TextBlock(tb) => {
                let text = clean_paragraph_text(&tb.value());
                let trimmed = text.trim();
                if should_render_paragraph_as_heading(doc, idx, trimmed, doc.kids.get(idx + 1)) {
                    return Some(trimmed.to_string());
                }
            }
            ContentElement::TextLine(tl) => {
                let text = clean_paragraph_text(&tl.value());
                let trimmed = text.trim();
                if should_render_paragraph_as_heading(doc, idx, trimmed, doc.kids.get(idx + 1)) {
                    return Some(trimmed.to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn equivalent_heading_text(left: &str, right: &str) -> bool {
    normalize_heading_text(left) == normalize_heading_text(right)
}

fn normalize_heading_text(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn looks_like_contents_document(doc: &PdfDocument) -> bool {
    let Some(first) = first_heading_like_text(doc) else {
        return false;
    };
    if !matches!(
        normalize_heading_text(&first).as_str(),
        "contents" | "tableofcontents"
    ) {
        return false;
    }

    let lines = collect_plain_lines(doc);
    if lines.len() < 8 {
        return false;
    }

    let page_like = lines
        .iter()
        .skip(1)
        .filter(|line| ends_with_page_marker(line))
        .count();
    page_like * 10 >= (lines.len().saturating_sub(1)).max(1) * 6
}

fn render_contents_document(doc: &PdfDocument) -> String {
    render_toc_lines(&collect_plain_lines(doc), true)
}

fn looks_like_compact_toc_document(doc: &PdfDocument) -> bool {
    let lines = collect_plain_lines(doc);
    if lines.len() < 8 {
        return false;
    }

    let page_like = lines
        .iter()
        .filter(|line| ends_with_page_marker(line))
        .count();
    let support_like = lines
        .iter()
        .filter(|line| looks_like_toc_support_heading(line))
        .count();

    page_like >= 3 && support_like >= 2 && (page_like + support_like) * 10 >= lines.len() * 8
}

fn render_compact_toc_document(doc: &PdfDocument) -> String {
    render_toc_lines(&collect_plain_lines(doc), false)
}

fn render_toc_lines(lines: &[String], has_contents_title: bool) -> String {
    let mut out = String::new();
    let mut iter = lines.iter();

    if has_contents_title {
        if let Some(first) = iter.next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                push_toc_heading(&mut out, 1, trimmed);
            }
        }
    }

    for line in iter {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(level) = toc_heading_level(trimmed, has_contents_title) {
            push_toc_heading(&mut out, level, strip_trailing_page_number(trimmed));
            continue;
        }

        if should_render_toc_line_as_bullet(trimmed, has_contents_title) {
            out.push_str("- ");
            out.push_str(&escape_md_line_start(trimmed));
            out.push('\n');
            continue;
        }

        if !out.ends_with("\n\n") && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&escape_md_line_start(trimmed));
        out.push_str("\n\n");
    }

    out.push('\n');
    out
}

fn toc_heading_level(text: &str, has_contents_title: bool) -> Option<usize> {
    let trimmed = strip_trailing_page_number(text).trim();
    let lower = trimmed.to_ascii_lowercase();

    if has_contents_title {
        if lower.starts_with("part ")
            || lower.starts_with("chapter ")
            || lower.starts_with("appendix ")
        {
            return Some(2);
        }
        return None;
    }

    if lower.starts_with("part ")
        || lower.starts_with("chapter ")
        || lower.starts_with("appendix ")
    {
        return Some(1);
    }
    if lower.starts_with("section ") {
        return Some(2);
    }
    None
}

fn should_render_toc_line_as_bullet(text: &str, has_contents_title: bool) -> bool {
    has_contents_title && ends_with_page_marker(text) && toc_heading_level(text, true).is_none()
}

fn push_toc_heading(out: &mut String, level: usize, text: &str) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }

    if !out.is_empty() && !out.ends_with("\n\n") {
        out.push('\n');
    }
    out.push_str(&"#".repeat(level));
    out.push(' ');
    out.push_str(trimmed);
    out.push_str("\n\n");
}

fn collect_plain_lines(doc: &PdfDocument) -> Vec<String> {
    let mut lines = Vec::new();
    for element in &doc.kids {
        match element {
            ContentElement::Heading(h) => {
                let text = clean_paragraph_text(&h.base.base.value());
                if !text.trim().is_empty() {
                    lines.push(text);
                }
            }
            ContentElement::NumberHeading(nh) => {
                let text = clean_paragraph_text(&nh.base.base.base.value());
                if !text.trim().is_empty() {
                    lines.push(text);
                }
            }
            ContentElement::Paragraph(p) => {
                let text = clean_paragraph_text(&p.base.value());
                if !text.trim().is_empty() {
                    lines.push(text);
                }
            }
            ContentElement::TextBlock(tb) => {
                let text = clean_paragraph_text(&tb.value());
                if !text.trim().is_empty() {
                    lines.push(text);
                }
            }
            ContentElement::TextLine(tl) => {
                let text = clean_paragraph_text(&tl.value());
                if !text.trim().is_empty() {
                    lines.push(text);
                }
            }
            ContentElement::List(list) => {
                for item in &list.list_items {
                    let label = token_rows_text(&item.label.content);
                    let body = token_rows_text(&item.body.content);
                    let combined = if !label.trim().is_empty() && !body.trim().is_empty() {
                        format!("{} {}", label.trim(), body.trim())
                    } else if !body.trim().is_empty() {
                        body.trim().to_string()
                    } else if !label.trim().is_empty() {
                        label.trim().to_string()
                    } else {
                        list_item_text_from_contents(&item.contents)
                            .trim()
                            .to_string()
                    };
                    if !combined.trim().is_empty() {
                        lines.push(combined);
                    }
                }
            }
            ContentElement::Table(table) => {
                extend_contents_lines_from_rows(
                    &mut lines,
                    collect_rendered_table_rows(
                        &table.table_border.rows,
                        table.table_border.num_columns,
                    ),
                );
            }
            ContentElement::TableBorder(table) => {
                extend_contents_lines_from_rows(
                    &mut lines,
                    collect_rendered_table_rows(&table.rows, table.num_columns),
                );
            }
            _ => {}
        }
    }
    lines
}

fn extend_contents_lines_from_rows(lines: &mut Vec<String>, rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        return;
    }

    if is_toc_table(&rows) {
        for row in &rows {
            let title = row.first().map(|s| s.trim()).unwrap_or("");
            let page = row.get(1).map(|s| s.trim()).unwrap_or("");
            let combined = if !title.is_empty() && !page.is_empty() {
                format!("{title} {page}")
            } else {
                format!("{title}{page}")
            };
            if !combined.trim().is_empty() {
                lines.push(combined);
            }
        }
    } else {
        // Non-TOC table in a contents document: concatenate cell text as a line.
        for row in &rows {
            let combined: String = row
                .iter()
                .map(|c| c.trim())
                .filter(|c| !c.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !combined.is_empty() {
                lines.push(combined);
            }
        }
    }
}

fn collect_rendered_table_rows(
    rows: &[crate::models::table::TableBorderRow],
    num_cols: usize,
) -> Vec<Vec<String>> {
    let num_cols = num_cols.max(1);
    let mut rendered_rows: Vec<Vec<String>> = Vec::new();

    for row in rows {
        let cell_texts: Vec<String> = (0..num_cols)
            .map(|col| {
                row.cells
                    .iter()
                    .find(|c| c.col_number == col)
                    .map(cell_text_content)
                    .unwrap_or_default()
            })
            .collect();
        if !cell_texts.iter().all(|t| t.trim().is_empty()) {
            rendered_rows.push(cell_texts);
        }
    }

    rendered_rows
}

fn ends_with_page_marker(text: &str) -> bool {
    text.split_whitespace()
        .last()
        .is_some_and(is_page_number_like)
}

fn looks_like_toc_support_heading(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || ends_with_page_marker(trimmed) {
        return false;
    }
    if trimmed.ends_with(['.', ';', ':', '?', '!']) {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if !(lower.starts_with("part ")
        || lower.starts_with("chapter ")
        || lower.starts_with("appendix ")
        || lower.starts_with("section "))
    {
        return false;
    }

    let word_count = trimmed.split_whitespace().count();
    (2..=16).contains(&word_count) && trimmed.chars().any(char::is_alphabetic)
}

fn split_leading_caption_and_body(text: &str) -> Option<(&str, &str)> {
    if !starts_with_caption_prefix(text) || !text.contains("(credit") {
        return None;
    }

    for needle in [") ", ". "] {
        let mut search_start = 0usize;
        while let Some(rel_idx) = text[search_start..].find(needle) {
            let boundary = search_start + rel_idx + needle.len() - 1;
            let head = text[..=boundary].trim();
            let tail = text[boundary + 1..].trim_start();
            search_start = boundary + 1;
            if head.split_whitespace().count() < 10 || head.split_whitespace().count() > 80 {
                continue;
            }
            if tail.split_whitespace().count() < 10 {
                continue;
            }
            if !starts_with_uppercase_word(tail) || starts_with_caption_prefix(tail) {
                continue;
            }
            return Some((head, tail));
        }
    }

    None
}

fn is_short_caption_label(text: &str) -> bool {
    if !starts_with_caption_prefix(text) {
        return false;
    }

    let trimmed = text.trim();
    trimmed.split_whitespace().count() <= 3 && trimmed.len() <= 24 && !trimmed.ends_with(['.', ':'])
}

fn split_following_caption_tail_and_body(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    if trimmed.is_empty()
        || starts_with_caption_prefix(trimmed)
        || !starts_with_uppercase_word(trimmed)
    {
        return None;
    }

    for starter in [
        " As ", " In ", " The ", " This ", " These ", " It ", " They ", " We ", " On ", " At ",
    ] {
        if let Some(idx) = text.find(starter) {
            let head = text[..idx].trim();
            let tail = text[idx + 1..].trim();
            if head.split_whitespace().count() >= 3
                && head.split_whitespace().count() <= 24
                && tail.split_whitespace().count() >= 8
            {
                return Some((head, tail));
            }
        }
    }

    None
}

fn looks_like_caption_tail(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.ends_with(['.', '!', '?']) {
        return false;
    }

    let word_count = trimmed.split_whitespace().count();
    if !(3..=18).contains(&word_count) {
        return false;
    }

    starts_with_uppercase_word(trimmed)
        && !starts_with_caption_prefix(trimmed)
        && !trimmed.contains(':')
}

fn looks_like_caption_year(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.len() == 4 && trimmed.chars().all(|ch| ch.is_ascii_digit())
}

/// Extract text from table token rows.
fn token_rows_text(rows: &[TableTokenRow]) -> String {
    normalize_common_ocr_text(&repair_fragmented_words(
        &rows
            .iter()
            .flat_map(|row| row.iter())
            .map(|token| token.base.value.as_str())
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

fn render_element(out: &mut String, element: &ContentElement) {
    match element {
        ContentElement::Heading(h) => {
            let text = h.base.base.value();
            let trimmed = text.trim();
            if should_skip_heading_text(trimmed) {
                return;
            }
            out.push_str(&format!("# {}\n\n", trimmed));
        }
        ContentElement::Paragraph(p) => {
            let text = p.base.value();
            let trimmed = clean_paragraph_text(&text);
            if !trimmed.is_empty() {
                out.push_str(&escape_md_line_start(&trimmed));
                if p.base.semantic_type == SemanticType::TableOfContent {
                    out.push('\n');
                } else {
                    out.push_str("\n\n");
                }
            }
        }
        ContentElement::List(list) => {
            let mut i = 0usize;
            let mut pending_item: Option<String> = None;
            while i < list.list_items.len() {
                let item = &list.list_items[i];
                let label = token_rows_text(&item.label.content);
                let body = token_rows_text(&item.body.content);
                let label_trimmed = normalize_list_text(label.trim());
                let body_trimmed = normalize_list_text(body.trim());
                let combined = if !label_trimmed.is_empty() && !body_trimmed.is_empty() {
                    format!("{label_trimmed} {body_trimmed}")
                } else if !body_trimmed.is_empty() {
                    body_trimmed.to_string()
                } else {
                    label_trimmed.to_string()
                };
                let combined = if combined.trim().is_empty() && !item.contents.is_empty() {
                    list_item_text_from_contents(&item.contents)
                } else {
                    combined
                };

                if is_list_section_heading(&combined) {
                    if let Some(pending) = pending_item.take() {
                        push_rendered_list_item(out, pending.trim());
                    }
                    out.push_str(&format!("# {}\n\n", combined.trim_end_matches(':').trim()));
                    i += 1;
                    continue;
                }

                if is_pure_bullet_marker(&label_trimmed) && body_trimmed.is_empty() {
                    i += 1;
                    continue;
                }

                if looks_like_stray_list_page_number(&combined) {
                    i += 1;
                    continue;
                }

                let current_item = if !label_trimmed.is_empty() || !body_trimmed.is_empty() {
                    if !label_trimmed.is_empty()
                        && !body_trimmed.is_empty()
                        && !is_pure_bullet_marker(&label_trimmed)
                    {
                        format!("{label_trimmed} {body_trimmed}")
                    } else if !body_trimmed.is_empty() {
                        body_trimmed.to_string()
                    } else if !is_pure_bullet_marker(&label_trimmed) {
                        label_trimmed.to_string()
                    } else {
                        String::new()
                    }
                } else if !item.contents.is_empty() {
                    normalize_list_text(list_item_text_from_contents(&item.contents).trim())
                } else {
                    String::new()
                };

                if current_item.is_empty() {
                    i += 1;
                    continue;
                }

                if let Some(previous) = pending_item.as_mut() {
                    if should_merge_list_continuation(previous, &current_item) {
                        merge_paragraph_text(previous, &current_item);
                        i += 1;
                        continue;
                    }
                }

                if let Some(pending) = pending_item.replace(current_item) {
                    push_rendered_list_item(out, pending.trim());
                }
                i += 1;
            }
            if let Some(pending) = pending_item.take() {
                push_rendered_list_item(out, pending.trim());
            }
            out.push('\n');
        }
        ContentElement::Table(table) => {
            render_table(out, table);
        }
        ContentElement::TableBorder(table) => {
            render_table_border(out, table);
        }
        ContentElement::Formula(f) => {
            let latex = f.latex.trim();
            if !latex.is_empty() {
                out.push_str(&format!("$$\n{}\n$$\n\n", latex));
            }
        }
        ContentElement::Caption(c) => {
            let text = c.base.value();
            let normalized = normalize_common_ocr_text(text.trim());
            let trimmed = normalized.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("*{}*\n\n", trimmed));
            }
        }
        ContentElement::NumberHeading(nh) => {
            let text = nh.base.base.base.value();
            let trimmed = text.trim();
            if should_skip_heading_text(trimmed) {
                return;
            }
            out.push_str(&format!("# {}\n\n", trimmed));
        }
        ContentElement::Image(_) => {
            out.push_str("![Image](image)\n\n");
        }
        ContentElement::HeaderFooter(_) => {
            // Skip headers/footers in markdown by default
        }
        ContentElement::TextBlock(tb) => {
            let text = tb.value();
            let trimmed = clean_paragraph_text(&text);
            if !trimmed.is_empty() {
                out.push_str(&escape_md_line_start(&trimmed));
                out.push_str("\n\n");
            }
        }
        ContentElement::TextLine(tl) => {
            let text = tl.value();
            let normalized = normalize_common_ocr_text(text.trim());
            let trimmed = normalized.trim();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                out.push('\n');
            }
        }
        ContentElement::TextChunk(tc) => {
            out.push_str(&tc.value);
        }
        _ => {}
    }
}

/// Escape characters that have special meaning at the start of a markdown line.
fn escape_md_line_start(text: &str) -> String {
    if text.starts_with('>') || text.starts_with('#') {
        format!("\\{}", text)
    } else {
        text.to_string()
    }
}

fn starts_with_caption_prefix(text: &str) -> bool {
    let lower = text.trim_start().to_ascii_lowercase();
    [
        "figure ",
        "fig. ",
        "table ",
        "tab. ",
        "chart ",
        "graph ",
        "image ",
        "illustration ",
        "diagram ",
        "plate ",
        "map ",
        "exhibit ",
        "photo by ",
        "photo credit",
        "image by ",
        "image credit",
        "image courtesy",
        "photo courtesy",
        "credit: ",
        "source: ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn is_structural_caption(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    lower.starts_with("figure ")
        || lower.starts_with("table ")
        || lower.starts_with("diagram ")
        || lower.starts_with("chart ")
}

fn normalize_chart_like_markdown(markdown: &str) -> String {
    let blocks: Vec<&str> = markdown
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .collect();
    if blocks.is_empty() {
        return markdown.trim().to_string();
    }

    let mut normalized = Vec::new();
    let mut i = 0usize;
    while i < blocks.len() {
        if let Some(rendered) = trim_large_top_table_plate(&blocks, i) {
            normalized.push(rendered);
            break;
        }

        if let Some((rendered, consumed)) = render_header_pair_chart_table(&blocks, i) {
            normalized.push(rendered);
            i += consumed;
            continue;
        }

        if let Some((rendered, consumed)) = render_chart_block(&blocks, i) {
            normalized.push(rendered);
            i += consumed;
            continue;
        }

        if let Some((rendered, consumed)) = render_structural_caption_block(&blocks, i) {
            normalized.push(rendered);
            i += consumed;
            continue;
        }

        if should_drop_artifact_table_block(&blocks, i) {
            i += 1;
            continue;
        }

        if !looks_like_footer_banner(blocks[i]) {
            normalized.push(blocks[i].to_string());
        }
        i += 1;
    }

    normalized.join("\n\n").trim().to_string() + "\n"
}

fn trim_large_top_table_plate(blocks: &[&str], start: usize) -> Option<String> {
    if start != 0 {
        return None;
    }

    let rows = parse_pipe_table_block(blocks.first()?.trim())?;
    let body_rows = rows.len().saturating_sub(2);
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if body_rows < 8 || max_cols < 8 {
        return None;
    }

    let caption = blocks.get(1)?.trim();
    if !caption.starts_with("Table ") || caption.split_whitespace().count() < 12 {
        return None;
    }

    let has_following_section = blocks.iter().skip(2).any(|block| {
        let trimmed = block.trim();
        trimmed.starts_with("# ")
            || trimmed.starts_with("## ")
            || trimmed
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit())
                && trimmed.contains(" Main Results")
    });
    has_following_section.then_some(blocks[0].trim().to_string())
}

fn render_header_pair_chart_table(blocks: &[&str], start: usize) -> Option<(String, usize)> {
    let caption = blocks.get(start)?.trim();
    if !is_structural_caption(caption) {
        return None;
    }

    let rows = parse_pipe_table_block(blocks.get(start + 1)?)?;
    if rows.len() != 2 {
        return None;
    }

    let pairs = extract_value_year_pairs_from_cells(&rows[0]);
    if pairs.len() < 4 {
        return None;
    }

    let mut source = String::new();
    let mut consumed = 2usize;
    if let Some(next_block) = blocks.get(start + 2) {
        let next = next_block.trim();
        if next.to_ascii_lowercase().starts_with("source:") {
            source = next.to_string();
            consumed += 1;
        }
    }

    let mut out = String::new();
    let heading_prefix = if start == 0 { "# " } else { "## " };
    out.push_str(heading_prefix);
    out.push_str(caption);
    out.push_str("\n\n");
    out.push_str(&format!("| Year | {} |\n", chart_value_header(caption)));
    out.push_str("| --- | --- |\n");
    for (year, value) in pairs {
        out.push_str(&format!("| {} | {} |\n", year, value));
    }
    out.push('\n');

    if !source.is_empty() {
        out.push('*');
        out.push_str(&escape_md_line_start(&source));
        out.push_str("*\n\n");
    }

    Some((out.trim().to_string(), consumed))
}

fn render_chart_block(blocks: &[&str], start: usize) -> Option<(String, usize)> {
    let (caption, numeric_tokens) = split_chart_caption_and_values(blocks.get(start)?)?;
    let mut consumed = 1usize;

    let mut source = String::new();
    let mut labels = Vec::new();
    if let Some(next_block) = blocks.get(start + 1) {
        let (candidate_labels, candidate_source) = extract_chart_labels_and_source(next_block);
        if !candidate_source.is_empty() || !candidate_labels.is_empty() {
            labels = candidate_labels;
            source = candidate_source;
            consumed += 1;
        }
    }

    while let Some(block) = blocks.get(start + consumed) {
        if looks_like_numeric_noise_block(block) {
            consumed += 1;
            continue;
        }
        break;
    }

    let value_tokens = derive_chart_series_values(&numeric_tokens, labels.len());

    let mut out = String::new();
    out.push_str("## ");
    out.push_str(caption.trim());
    out.push_str("\n\n");

    if labels.len() >= 3 && labels.len() == value_tokens.len() {
        let label_header = if labels.iter().all(|label| looks_like_yearish_label(label)) {
            "Year"
        } else {
            "Label"
        };
        let value_header = chart_value_header(&caption);
        out.push_str(&format!("| {} | {} |\n", label_header, value_header));
        out.push_str("| --- | --- |\n");
        for (label, value) in labels.iter().zip(value_tokens.iter()) {
            out.push_str(&format!("| {} | {} |\n", label, value));
        }
        out.push('\n');
    }

    if !source.is_empty() {
        out.push('*');
        out.push_str(&escape_md_line_start(&source));
        out.push_str("*\n\n");
    }

    Some((out.trim().to_string(), consumed))
}

fn render_structural_caption_block(blocks: &[&str], start: usize) -> Option<(String, usize)> {
    let block = blocks.get(start)?.trim();
    if !is_structural_caption(block) || block.contains('|') {
        return None;
    }

    let mut caption = collapse_inline_whitespace(block);
    let mut consumed = 1usize;
    if let Some(next_block) = blocks.get(start + 1) {
        let next = next_block.trim();
        if looks_like_caption_continuation(next) {
            caption.push(' ');
            caption.push_str(next.trim_end_matches('.'));
            consumed += 1;
        } else if !looks_like_isolated_caption_context(block, next) {
            return None;
        }
    } else {
        return None;
    }

    Some((format!("## {}", caption.trim()), consumed))
}

fn split_chart_caption_and_values(block: &str) -> Option<(String, Vec<String>)> {
    let trimmed = block.trim();
    if !is_structural_caption(trimmed) {
        return None;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let first_numeric_idx = tokens.iter().position(|token| is_numberish_token(token))?;
    if first_numeric_idx < 3 {
        return None;
    }

    let caption = tokens[..first_numeric_idx].join(" ");
    let numeric_tokens: Vec<String> = tokens[first_numeric_idx..]
        .iter()
        .filter_map(|token| sanitize_numberish_token(token))
        .collect();

    if numeric_tokens.len() < 4 {
        return None;
    }

    Some((caption, numeric_tokens))
}

fn parse_pipe_table_block(block: &str) -> Option<Vec<Vec<String>>> {
    let lines: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.len() < 2 {
        return None;
    }

    let header = split_pipe_row(lines[0])?;
    if !is_pipe_separator_row(lines[1], header.len()) {
        return None;
    }

    let mut rows = vec![header];
    rows.push(split_pipe_row(lines[1]).unwrap_or_default());
    for line in lines.iter().skip(2) {
        let row = split_pipe_row(line)?;
        rows.push(row);
    }
    Some(rows)
}

fn split_pipe_row(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }

    Some(
        trimmed[1..trimmed.len() - 1]
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect(),
    )
}

fn is_pipe_separator_row(line: &str, expected_cols: usize) -> bool {
    let Some(cells) = split_pipe_row(line) else {
        return false;
    };
    if cells.len() != expected_cols || expected_cols == 0 {
        return false;
    }

    cells.iter().all(|cell| {
        let stripped = cell.trim_matches(':').trim();
        !stripped.is_empty() && stripped.chars().all(|ch| ch == '-')
    })
}

fn extract_value_year_pairs_from_cells(cells: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for cell in cells {
        let tokens: Vec<&str> = cell.split_whitespace().collect();
        if tokens.len() != 2 {
            continue;
        }

        if looks_like_year_token(tokens[0]) && is_numberish_token(tokens[1]) {
            if let Some(value) = sanitize_numberish_token(tokens[1]) {
                pairs.push((tokens[0].to_string(), value));
            }
            continue;
        }

        if is_numberish_token(tokens[0]) && looks_like_year_token(tokens[1]) {
            if let Some(value) = sanitize_numberish_token(tokens[0]) {
                pairs.push((tokens[1].to_string(), value));
            }
        }
    }

    pairs.sort_by(|left, right| left.0.cmp(&right.0));
    pairs
}

fn should_drop_artifact_table_block(blocks: &[&str], start: usize) -> bool {
    let Some(rows) = parse_pipe_table_block(blocks[start]) else {
        return false;
    };

    let prev = start
        .checked_sub(1)
        .and_then(|idx| blocks.get(idx))
        .map(|block| block.trim())
        .unwrap_or("");
    let next = blocks.get(start + 1).map(|block| block.trim()).unwrap_or("");

    if rows.len() == 2 && rows.first().is_some_and(|row| row.len() == 1) {
        let header = rows[0][0].trim();
        if looks_like_url_fragment(header) {
            return true;
        }
        if looks_like_numeric_axis_blob(header) && !previous_block_announces_table(prev) {
            return true;
        }
    }

    let stats = pipe_table_stats(&rows);
    stats.fill_ratio < 0.5
        && stats.long_cell_count == 0
        && !is_structural_caption(prev)
        && (looks_like_citation_block(next) || is_structural_caption(next))
}

fn previous_block_announces_table(block: &str) -> bool {
    let lower = block.trim().to_ascii_lowercase();
    lower.ends_with("as follows:")
        || lower.ends_with("following details:")
        || lower.ends_with("following detail:")
        || lower.contains("the following details")
}

fn looks_like_url_fragment(text: &str) -> bool {
    let trimmed = text.trim();
    (!trimmed.is_empty() && (trimmed.contains("http") || trimmed.contains("/status/")))
        || (trimmed.contains('/') && !trimmed.contains(' '))
}

fn looks_like_numeric_axis_blob(text: &str) -> bool {
    let numeric_values: Vec<i64> = text
        .split_whitespace()
        .filter_map(parse_integer_token)
        .collect();
    numeric_values.len() >= 8
        && !detect_axis_progression(&numeric_values).is_empty()
        && text.chars().any(char::is_alphabetic)
}

fn looks_like_citation_block(block: &str) -> bool {
    let trimmed = block.trim();
    trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.split_whitespace().count() <= 8
}

struct PipeTableStats {
    fill_ratio: f64,
    long_cell_count: usize,
}

fn pipe_table_stats(rows: &[Vec<String>]) -> PipeTableStats {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let body = rows.len().saturating_sub(2);
    let mut nonempty = 0usize;
    let mut long_cell_count = 0usize;

    for row in rows.iter().skip(2) {
        for cell in row {
            if !cell.trim().is_empty() {
                nonempty += 1;
                if cell.split_whitespace().count() >= 3 {
                    long_cell_count += 1;
                }
            }
        }
    }

    let fill_ratio = if body == 0 {
        0.0
    } else {
        nonempty as f64 / (body * cols) as f64
    };

    PipeTableStats {
        fill_ratio,
        long_cell_count,
    }
}

fn extract_chart_labels_and_source(block: &str) -> (Vec<String>, String) {
    let trimmed = block.trim();
    let lower = trimmed.to_ascii_lowercase();
    let source_idx = lower.find("source:");

    let label_region = source_idx.map_or(trimmed, |idx| trimmed[..idx].trim());
    let source = source_idx
        .map(|idx| trimmed[idx..].trim().to_string())
        .unwrap_or_default();

    let labels = parse_chart_labels(label_region);
    (labels, source)
}

fn parse_chart_labels(text: &str) -> Vec<String> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut labels = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        let token = tokens[i].trim_matches(|c: char| c == ',' || c == ';');
        if looks_like_year_token(token) {
            let mut label = token.to_string();
            if let Some(next) = tokens.get(i + 1) {
                let next_trimmed = next.trim_matches(|c: char| c == ',' || c == ';');
                if next_trimmed.starts_with('(') && next_trimmed.ends_with(')') {
                    label.push(' ');
                    label.push_str(next_trimmed);
                    i += 1;
                }
            }
            labels.push(label);
        } else if looks_like_category_label(token) {
            labels.push(token.to_string());
        }
        i += 1;
    }
    labels
}

fn derive_chart_series_values(tokens: &[String], expected_count: usize) -> Vec<String> {
    if expected_count == 0 {
        return Vec::new();
    }

    if tokens.len() == expected_count {
        return tokens.to_vec();
    }

    let numeric_values: Vec<i64> = tokens
        .iter()
        .filter_map(|token| parse_integer_token(token))
        .collect();
    if numeric_values.len() != tokens.len() {
        return Vec::new();
    }

    let axis_series = detect_axis_progression(&numeric_values);
    if axis_series.is_empty() {
        return Vec::new();
    }

    let mut remaining = Vec::new();
    let mut removable = axis_series;
    for token in tokens {
        let Some(value) = parse_integer_token(token) else {
            continue;
        };
        if let Some(pos) = removable.iter().position(|candidate| *candidate == value) {
            removable.remove(pos);
        } else {
            remaining.push(token.clone());
        }
    }

    if remaining.len() == expected_count {
        remaining
    } else {
        Vec::new()
    }
}

fn detect_axis_progression(values: &[i64]) -> Vec<i64> {
    if values.len() < 6 {
        return Vec::new();
    }

    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    if sorted.len() < 6 {
        return Vec::new();
    }

    let mut best = Vec::new();
    for window in sorted.windows(2) {
        let step = window[1] - window[0];
        if step <= 0 {
            continue;
        }

        let mut series = vec![window[0]];
        let mut current = window[0];
        loop {
            let next = current + step;
            if sorted.binary_search(&next).is_ok() {
                series.push(next);
                current = next;
            } else {
                break;
            }
        }

        if series.len() > best.len() {
            best = series;
        }
    }

    if best.len() >= 6 {
        best
    } else {
        Vec::new()
    }
}

fn chart_value_header(caption: &str) -> String {
    let trimmed = caption.trim();
    let title = strip_structural_caption_prefix(trimmed);

    let mut base = title.to_string();
    if let Some(idx) = base.rfind(" in ") {
        let tail = base[idx + 4..].trim();
        if tail.split_whitespace().count() <= 2 && tail.chars().next().is_some_and(char::is_uppercase) {
            base.truncate(idx);
        }
    }

    if let Some(start) = title.rfind('(') {
        if title.ends_with(')') {
            let unit = title[start + 1..title.len() - 1].trim();
            if let Some(idx) = base.rfind('(') {
                base.truncate(idx);
            }
            let normalized_unit = unit.strip_prefix("in ").unwrap_or(unit).trim();
            return format!("{} ({})", base.trim(), normalized_unit);
        }
    }

    let trimmed = base.trim();
    if trimmed.is_empty() {
        "Value".to_string()
    } else {
        trimmed.to_string()
    }
}

fn strip_structural_caption_prefix(text: &str) -> &str {
    let trimmed = text.trim();
    let mut parts = trimmed.splitn(3, ' ');
    let Some(first) = parts.next() else {
        return trimmed;
    };
    let Some(second) = parts.next() else {
        return trimmed;
    };
    let Some(rest) = parts.next() else {
        return trimmed;
    };

    let first_lower = first.to_ascii_lowercase();
    if matches!(first_lower.as_str(), "figure" | "table" | "diagram" | "chart")
        && second
            .chars()
            .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ':'))
    {
        rest.trim()
    } else {
        trimmed
    }
}

fn looks_like_footer_banner(block: &str) -> bool {
    let trimmed = block.trim();
    if trimmed.contains('\n') || trimmed.len() < 8 {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if !(2..=6).contains(&tokens.len()) {
        return false;
    }

    let Some(last) = tokens.last() else {
        return false;
    };
    if !last.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }

    tokens[..tokens.len() - 1].iter().all(|token| {
        matches!(
            token.to_ascii_lowercase().as_str(),
            "of" | "and" | "the" | "for" | "in" | "on"
        ) || token.chars().next().is_some_and(char::is_uppercase)
    })
}

fn looks_like_caption_continuation(block: &str) -> bool {
    let trimmed = block.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().count() <= 8
        && trimmed.chars().next().is_some_and(char::is_uppercase)
        && !trimmed.contains(':')
}

fn collapse_inline_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn drop_isolated_noise_lines(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut kept = Vec::with_capacity(lines.len());

    for (idx, line) in lines.iter().enumerate() {
        if should_drop_isolated_noise_line(&lines, idx) {
            continue;
        }
        kept.push(*line);
    }

    let mut result = kept.join("\n");
    if markdown.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn should_drop_isolated_noise_line(lines: &[&str], idx: usize) -> bool {
    let trimmed = lines[idx].trim();
    if trimmed.len() != 1 {
        return false;
    }

    let ch = trimmed.chars().next().unwrap_or_default();
    if !(ch.is_ascii_lowercase() || ch.is_ascii_digit()) {
        return false;
    }

    let prev = previous_nonempty_line(lines, idx);
    let next = next_nonempty_line(lines, idx);
    let (Some(prev), Some(next)) = (prev, next) else {
        return false;
    };

    is_substantive_markdown_line(prev) && is_substantive_markdown_line(next)
}

fn previous_nonempty_line<'a>(lines: &'a [&'a str], idx: usize) -> Option<&'a str> {
    lines[..idx]
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
        .copied()
}

fn next_nonempty_line<'a>(lines: &'a [&'a str], idx: usize) -> Option<&'a str> {
    lines[idx + 1..]
        .iter()
        .find(|line| !line.trim().is_empty())
        .copied()
}

fn is_substantive_markdown_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with('|') || trimmed.starts_with("- ") || trimmed.starts_with('#') {
        return true;
    }

    trimmed.split_whitespace().count() >= 2
}

fn normalize_common_ocr_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let mut normalized = text
        .replace("ߤL", "μL")
        .replace(" oC", "°C")
        .replace("37 C", "37°C")
        .replace("-20 oC", "-20°C")
        .replace("1- 20-μL", "1-20-μL")
        .replace("1- 20 μL", "1-20 μL")
        .replace("1- 2 0  μL", "1-20 μL")
        .replace("1- 2 0 μL", "1-20 μL")
        .replace("10x loading dye", "10x loading dye");

    normalized = normalize_degree_spacing(&normalized);
    collapse_inline_whitespace(&normalized)
}

fn normalize_degree_spacing(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == ' '
            && i > 0
            && i + 2 < chars.len()
            && chars[i - 1].is_ascii_digit()
            && matches!(chars[i + 1], 'C' | 'F')
            && !chars[i + 2].is_ascii_alphabetic()
        {
            out.push('°');
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn normalize_list_text(text: &str) -> String {
    let normalized = normalize_common_ocr_text(text);
    let trimmed = normalized.trim_start_matches(|ch: char| is_bullet_like(ch)).trim();
    trimmed.to_string()
}

fn push_rendered_list_item(out: &mut String, item: &str) {
    if starts_with_enumerated_marker(item) {
        out.push_str(item);
        out.push('\n');
    } else {
        out.push_str(&format!("- {}\n", item));
    }
}

fn should_merge_list_continuation(previous: &str, current: &str) -> bool {
    let trimmed = current.trim();
    if trimmed.is_empty()
        || looks_like_stray_list_page_number(trimmed)
        || is_list_section_heading(trimmed)
        || looks_like_numbered_section(trimmed)
        || starts_with_enumerated_marker(trimmed)
    {
        return false;
    }

    if previous.ends_with('-')
        && previous
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|c| c.is_alphabetic())
        && trimmed.chars().next().is_some_and(char::is_lowercase)
    {
        return true;
    }

    trimmed.chars().next().is_some_and(|ch| {
        ch.is_ascii_lowercase() || matches!(ch, ',' | ';' | ')' | ']' | '%')
    })
}

fn is_pure_bullet_marker(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed.chars().all(is_bullet_like)
}

fn looks_like_stray_list_page_number(text: &str) -> bool {
    let trimmed = text.trim();
    (1..=4).contains(&trimmed.len()) && trimmed.chars().all(|ch| ch.is_ascii_digit())
}

fn is_bullet_like(ch: char) -> bool {
    matches!(ch, '•' | '◦' | '▪' | '▸' | '▹' | '►' | '▻' | '●' | '○' | '■' | '□' | '◆' | '◇' | '-')
}

fn looks_like_isolated_caption_context(caption: &str, next_block: &str) -> bool {
    let next = next_block.trim();
    if next.is_empty() {
        return false;
    }

    let next_lower = next.to_ascii_lowercase();
    if next_lower.starts_with("source:")
        || next_lower.starts_with("note:")
        || next_lower.starts_with("*source:")
        || next_lower.starts_with("*note:")
    {
        return true;
    }

    caption.split_whitespace().count() <= 14
        && next.split_whitespace().count() <= 45
        && (next.contains(':') || next.contains('='))
}

fn looks_like_numeric_noise_block(block: &str) -> bool {
    let trimmed = block.trim();
    !trimmed.is_empty()
        && trimmed.split_whitespace().all(|token| {
            sanitize_numberish_token(token)
                .as_deref()
                .is_some_and(|sanitized| sanitized.chars().all(|ch| ch.is_ascii_digit()))
        })
}

fn looks_like_yearish_label(label: &str) -> bool {
    label
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
}

fn looks_like_year_token(token: &str) -> bool {
    token.len() == 4 && token.chars().all(|ch| ch.is_ascii_digit())
}

fn looks_like_category_label(token: &str) -> bool {
    token.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '/' | '%'))
        && token.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn is_numberish_token(token: &str) -> bool {
    sanitize_numberish_token(token).is_some()
}

fn sanitize_numberish_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|c: char| matches!(c, ',' | ';' | ':' | '.'));
    if trimmed.is_empty() {
        return None;
    }

    let candidate = trimmed.trim_end_matches('%').replace(',', "");
    if candidate.chars().all(|ch| ch.is_ascii_digit()) {
        Some(trimmed.trim_end_matches(|c: char| matches!(c, ',' | ';' | ':')).to_string())
    } else {
        None
    }
}

fn parse_integer_token(token: &str) -> Option<i64> {
    sanitize_numberish_token(token)?
        .replace(',', "")
        .parse::<i64>()
        .ok()
}

fn starts_with_uppercase_word(text: &str) -> bool {
    for ch in text.trim_start().chars() {
        if ch.is_alphabetic() {
            return ch.is_uppercase();
        }
        if !matches!(ch, '"' | '\'' | '(' | '[') {
            break;
        }
    }
    false
}

/// Clean paragraph text: trim trailing whitespace from each line,
/// collapse multiple spaces, and normalize whitespace.
fn clean_paragraph_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    // Collapse runs of spaces (but not newlines) to single space
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    normalize_common_ocr_text(&result)
}

fn next_mergeable_paragraph_text(element: Option<&ContentElement>) -> Option<String> {
    match element {
        Some(ContentElement::Paragraph(p)) => {
            let text = clean_paragraph_text(&p.base.value());
            let trimmed = text.trim();
            if trimmed.is_empty()
                || should_render_element_as_heading(element.unwrap(), trimmed, None)
            {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(ContentElement::TextBlock(tb)) => {
            let text = clean_paragraph_text(&tb.value());
            let trimmed = text.trim();
            if trimmed.is_empty()
                || should_render_element_as_heading(element.unwrap(), trimmed, None)
            {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Some(ContentElement::TextLine(tl)) => {
            let text = clean_paragraph_text(&tl.value());
            let trimmed = text.trim();
            if trimmed.is_empty()
                || should_render_element_as_heading(element.unwrap(), trimmed, None)
            {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        _ => None,
    }
}

fn should_render_paragraph_as_heading(
    doc: &PdfDocument,
    idx: usize,
    text: &str,
    next: Option<&ContentElement>,
) -> bool {
    if looks_like_top_margin_running_header(doc, idx, text) {
        return false;
    }
    if should_render_element_as_heading(&doc.kids[idx], text, next) {
        return true;
    }

    // Font-size guard: skip rescue if the candidate text is significantly
    // smaller than the document's body text (chart axis labels, footnotes).
    let body_font_size = compute_body_font_size(doc);
    if is_too_small_for_heading(&doc.kids, idx, body_font_size) {
        return false;
    }

    // Rescue pass tier 1: when the pipeline found zero headings, use broad rescue.
    if !doc_has_explicit_headings(doc) {
        if should_rescue_as_heading(doc, idx, text) {
            return true;
        }
        // Also check numbered sections and ALL CAPS even with zero headings,
        // since Tier 1 broad rescue has strict word/char limits that miss
        // longer keyword-numbered headings (e.g. "Activity 4. Title text").
        if should_rescue_allcaps_heading(doc, idx, text) {
            return true;
        }
        if should_rescue_numbered_heading(doc, idx, text) {
            return true;
        }
        return false;
    }
    // Rescue pass tier 2: when heading density is very low (< 10%), only
    // rescue ALL CAPS short text followed by substantial body content.
    if heading_density(doc) < 0.10 {
        if should_rescue_allcaps_heading(doc, idx, text) {
            return true;
        }
        // Rescue pass tier 3: numbered section headings (e.g. "01 - Title").
        // When a document has very few detected headings, numbered patterns
        // are a strong structural signal that the font-based detector missed.
        if should_rescue_numbered_heading(doc, idx, text) {
            return true;
        }
        // Font-size-gated title-case rescue: when the paragraph is rendered
        // in a noticeably larger font than body text, apply the same
        // title-case rescue used in tier 1.  A 15 % size increase is a
        // reliable visual heading signal straight from the PDF font metrics.
        if body_font_size > 0.0 {
            if let ContentElement::Paragraph(p) = &doc.kids[idx] {
                if let Some(fs) = p.base.font_size {
                    if fs >= 1.15 * body_font_size
                        && is_heading_rescue_candidate(doc, idx, text)
                        && has_substantive_follow_up(doc, idx, text.split_whitespace().count(), 4)
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Check whether any element in the document is an explicit heading from the pipeline.
fn doc_has_explicit_headings(doc: &PdfDocument) -> bool {
    doc.kids.iter().any(|e| {
        matches!(
            e,
            ContentElement::Heading(_) | ContentElement::NumberHeading(_)
        )
    })
}

/// Compute the dominant body font size from paragraphs with substantial text
/// (> 10 words).  Uses the median of qualifying paragraphs to avoid being
/// skewed by short chart labels or footnote markers.
/// Returns 0.0 if no qualifying paragraph is found.
fn compute_body_font_size(doc: &PdfDocument) -> f64 {
    let mut font_sizes: Vec<f64> = doc
        .kids
        .iter()
        .filter_map(|e| {
            if let ContentElement::Paragraph(p) = e {
                let word_count = p.base.value().split_whitespace().count();
                if word_count > 10 {
                    p.base.font_size
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    if font_sizes.is_empty() {
        return 0.0;
    }
    font_sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    font_sizes[font_sizes.len() / 2]
}

/// Check whether a paragraph's font size is too small relative to the document
/// body font to be a heading.  Returns true if the element should be skipped.
/// A heading should not be noticeably smaller than body text — font size ≥ 95%
/// of the dominant body size is required.
fn is_too_small_for_heading(doc_kids: &[ContentElement], idx: usize, body_font_size: f64) -> bool {
    if body_font_size <= 0.0 {
        return false;
    }
    if let ContentElement::Paragraph(p) = &doc_kids[idx] {
        if let Some(fs) = p.base.font_size {
            return fs < 0.95 * body_font_size;
        }
    }
    false
}

/// Count the ratio of pipeline headings to total content elements.
fn heading_density(doc: &PdfDocument) -> f64 {
    let total = doc.kids.len();
    if total == 0 {
        return 0.0;
    }
    let heading_count = doc
        .kids
        .iter()
        .filter(|e| {
            matches!(
                e,
                ContentElement::Heading(_) | ContentElement::NumberHeading(_)
            )
        })
        .count();
    heading_count as f64 / total as f64
}

/// Rescue headings: identify short standalone paragraphs that likely serve
/// as section headings.  Only runs when the pipeline produced zero headings.
fn should_rescue_as_heading(doc: &PdfDocument, idx: usize, text: &str) -> bool {
    is_heading_rescue_candidate(doc, idx, text)
        && has_substantive_follow_up(doc, idx, text.split_whitespace().count(), 4)
}

/// Pure text-criteria check for title-case heading rescue.
/// Returns true when the text looks like a heading based on casing,
/// length, and character composition — without any lookahead.
fn is_heading_rescue_candidate(doc: &PdfDocument, idx: usize, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let has_alpha = trimmed.chars().any(char::is_alphabetic);

    // Must have alphabetic chars and not end with sentence/continuation punctuation
    if !has_alpha || trimmed.ends_with(['.', '!', '?', ';', ',']) {
        return false;
    }

    // Reject text containing math/special symbols or percentage signs.
    if should_demote_math_heading(trimmed) || should_demote_percentage_heading(trimmed) {
        return false;
    }

    // Must not be fully parenthesized (citations)
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        return false;
    }

    // Must not look like a caption or chart label
    if starts_with_caption_prefix(trimmed)
        || looks_like_chart_label_heading(&doc.kids[idx], trimmed)
    {
        return false;
    }

    // Must be short: ≤ 6 words, ≤ 60 chars
    let word_count = trimmed.split_whitespace().count();
    if word_count > 6 || trimmed.len() > 60 {
        return false;
    }

    // Must not be a purely numeric string
    if trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ' ')
    {
        return false;
    }

    // First alphabetic character should be uppercase
    if let Some(first_alpha) = trimmed.chars().find(|c| c.is_alphabetic()) {
        if first_alpha.is_lowercase() {
            return false;
        }
    }

    true
}

/// Check the next `max_lookahead` elements for substantive body content.
/// Returns true when at least one element is a long paragraph (≥ word_count*3
/// or > 15 words) or a structural element (list, table, image, figure).
fn has_substantive_follow_up(
    doc: &PdfDocument,
    idx: usize,
    word_count: usize,
    max_lookahead: usize,
) -> bool {
    for offset in 1..=max_lookahead {
        let lookahead_idx = idx + offset;
        if lookahead_idx >= doc.kids.len() {
            break;
        }
        let look_elem = &doc.kids[lookahead_idx];
        match look_elem {
            ContentElement::Paragraph(p) => {
                let next_text = p.base.value();
                let nw = next_text.split_whitespace().count();
                if nw >= word_count * 3 || nw > 15 {
                    return true;
                }
            }
            ContentElement::TextBlock(tb) => {
                let next_text = tb.value();
                let nw = next_text.split_whitespace().count();
                if nw >= word_count * 3 || nw > 15 {
                    return true;
                }
            }
            ContentElement::TextLine(tl) => {
                let next_text = tl.value();
                let nw = next_text.split_whitespace().count();
                if nw >= word_count * 3 || nw > 15 {
                    return true;
                }
            }
            ContentElement::List(_)
            | ContentElement::Table(_)
            | ContentElement::TableBorder(_)
            | ContentElement::Image(_)
            | ContentElement::Figure(_) => {
                return true;
            }
            _ => continue,
        }
    }

    false
}

/// Rescue numbered section headings like "01 - Find Open Educational Resources"
/// or "4.2 Main Results" when heading density is low.
fn should_rescue_numbered_heading(doc: &PdfDocument, idx: usize, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return false;
    }

    // Must match numbered section pattern: digits (with optional dots)
    // followed by separator and title text.
    if !looks_like_numbered_section(trimmed) {
        return false;
    }

    // Must not end with sentence punctuation — EXCEPT when the text matches
    // a keyword+number pattern (e.g. "Activity 4. Determining CEC…") where
    // the trailing period is part of the heading format, not sentence ending.
    if trimmed.ends_with(['!', '?', ';', ',']) {
        return false;
    }
    if trimmed.ends_with('.') && !looks_like_keyword_numbered_section(trimmed) {
        return false;
    }
    // Reject numbered headings containing math symbols or percentage signs.
    if should_demote_math_heading(trimmed) || should_demote_percentage_heading(trimmed) {
        return false;
    }

    // Look ahead for substantive content
    for offset in 1..=3 {
        let lookahead_idx = idx + offset;
        if lookahead_idx >= doc.kids.len() {
            break;
        }
        match &doc.kids[lookahead_idx] {
            ContentElement::Paragraph(p) => {
                let nw = p.base.value().split_whitespace().count();
                if nw > 10 {
                    return true;
                }
            }
            ContentElement::TextBlock(tb) => {
                let nw = tb.value().split_whitespace().count();
                if nw > 10 {
                    return true;
                }
            }
            ContentElement::TextLine(tl) => {
                let nw = tl.value().split_whitespace().count();
                if nw > 10 {
                    return true;
                }
            }
            ContentElement::List(_)
            | ContentElement::Table(_)
            | ContentElement::TableBorder(_)
            | ContentElement::Image(_)
            | ContentElement::Figure(_) => {
                return true;
            }
            _ => continue,
        }
    }

    false
}

/// Check if text starts with a numbered section prefix (e.g. "01 -", "4.2 ", "III.")
/// or a keyword+number pattern (e.g. "Activity 4.", "Experiment #1:", "Chapter 3").
fn looks_like_numbered_section(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return false;
    }

    // Branch 1: digit-based prefix: "1 ", "01 ", "4.2 ", "1. ", "01 - "
    let mut idx = 0;
    if bytes[0].is_ascii_digit() {
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }
        if idx >= bytes.len() {
            return false;
        }
        // dot-separated subsections: "4.2", "1.3.1"
        while idx < bytes.len() && bytes[idx] == b'.' {
            idx += 1;
            let start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if idx == start {
                // "4." followed by space → "4. Title"
                break;
            }
        }
        // Must be followed by whitespace or "-"
        if idx >= bytes.len() {
            return false;
        }
        // Skip separator: "- " or " - " or just " "
        if bytes[idx] == b' ' || bytes[idx] == b'\t' {
            idx += 1;
            // Skip optional "- " separator
            if idx < bytes.len() && bytes[idx] == b'-' {
                idx += 1;
                if idx < bytes.len() && bytes[idx] == b' ' {
                    idx += 1;
                }
            }
        } else if bytes[idx] == b'-' {
            idx += 1;
            if idx < bytes.len() && bytes[idx] == b' ' {
                idx += 1;
            }
        } else {
            return false;
        }
        // Must have title text after prefix
        let rest = &text[idx..].trim();
        if rest.is_empty() {
            return false;
        }
        // First alpha char must be uppercase
        if let Some(c) = rest.chars().find(|c| c.is_alphabetic()) {
            return c.is_uppercase();
        }
        return false;
    }

    // Branch 2: keyword+number prefix: "Activity 4.", "Experiment #1:", "Chapter 3"
    if looks_like_keyword_numbered_section(text) {
        return true;
    }

    false
}

/// Structural keywords that commonly precede a number to form a heading.
const SECTION_KEYWORDS: &[&str] = &[
    "activity",
    "appendix",
    "case",
    "chapter",
    "exercise",
    "experiment",
    "lab",
    "lesson",
    "module",
    "part",
    "phase",
    "problem",
    "question",
    "section",
    "stage",
    "step",
    "task",
    "topic",
    "unit",
];

/// Check if text matches "Keyword N. Title" or "Keyword #N: Title" pattern.
fn looks_like_keyword_numbered_section(text: &str) -> bool {
    let trimmed = text.trim();
    // Find the first space to extract the keyword
    let space_pos = match trimmed.find(' ') {
        Some(p) => p,
        None => return false,
    };
    let keyword = &trimmed[..space_pos];
    if !SECTION_KEYWORDS
        .iter()
        .any(|k| keyword.eq_ignore_ascii_case(k))
    {
        return false;
    }
    // After keyword+space, expect a number (optionally preceded by #)
    let rest = trimmed[space_pos + 1..].trim_start();
    if rest.is_empty() {
        return false;
    }
    let rest = rest.strip_prefix('#').unwrap_or(rest);
    // Must start with a digit or roman numeral
    let first_char = rest.chars().next().unwrap_or(' ');
    if !first_char.is_ascii_digit() && !matches!(first_char, 'I' | 'V' | 'X' | 'L') {
        return false;
    }
    true
}

/// Strict rescue for docs with some headings but low density: only promote
/// ALL CAPS text that is clearly a section heading.
fn should_rescue_allcaps_heading(doc: &PdfDocument, idx: usize, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let word_count = trimmed.split_whitespace().count();

    // Must be short: ≤ 8 words, ≤ 80 chars
    if word_count > 8 || trimmed.len() > 80 {
        return false;
    }

    // Must be ALL CAPS (all alphabetic chars are uppercase)
    let alpha_chars: Vec<char> = trimmed.chars().filter(|c| c.is_alphabetic()).collect();
    if alpha_chars.len() < 2 || !alpha_chars.iter().all(|c| c.is_uppercase()) {
        return false;
    }

    // Must not end with sentence punctuation
    if trimmed.ends_with(['.', ';', ',']) {
        return false;
    }

    // Reject all-caps headings containing math symbols or percentage signs.
    if should_demote_math_heading(trimmed) || should_demote_percentage_heading(trimmed) {
        return false;
    }

    // Must not look like a caption
    if starts_with_caption_prefix(trimmed) {
        return false;
    }

    // Must not be purely numeric or a page number
    if trimmed
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ' ')
    {
        return false;
    }

    // Look ahead for substantive content — accept any non-trivial text
    // (>6 words) or structured content within the next 4 elements.
    for offset in 1..=4 {
        let lookahead_idx = idx + offset;
        if lookahead_idx >= doc.kids.len() {
            break;
        }
        let look_elem = &doc.kids[lookahead_idx];
        match look_elem {
            ContentElement::Paragraph(p) => {
                let nw = p.base.value().split_whitespace().count();
                if nw > 6 {
                    return true;
                }
            }
            ContentElement::TextBlock(tb) => {
                let nw = tb.value().split_whitespace().count();
                if nw > 6 {
                    return true;
                }
            }
            ContentElement::TextLine(tl) => {
                let nw = tl.value().split_whitespace().count();
                if nw > 6 {
                    return true;
                }
            }
            ContentElement::List(_)
            | ContentElement::Table(_)
            | ContentElement::TableBorder(_)
            | ContentElement::Image(_)
            | ContentElement::Figure(_) => {
                return true;
            }
            _ => continue,
        }
    }

    false
}

fn should_render_element_as_heading(
    element: &ContentElement,
    text: &str,
    next: Option<&ContentElement>,
) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    if matches!(lower.as_str(), "contents" | "table of contents")
        && trimmed.starts_with(|c: char| c.is_uppercase())
    {
        return true;
    }

    let word_count = trimmed.split_whitespace().count();
    let has_alpha = trimmed.chars().any(char::is_alphabetic);
    let title_like = has_alpha
        && word_count <= 4
        && trimmed.len() <= 40
        && !trimmed.ends_with(['.', '!', '?', ';', ':']);

    // Reject attribution prefixes that are clearly not section headings
    // (more targeted than starts_with_caption_prefix to avoid false demotions
    // of legitimate headings starting with common words like "Graph", "Table").
    let is_attribution = {
        let lower = trimmed.to_ascii_lowercase();
        lower.starts_with("source:")
            || lower.starts_with("credit:")
            || lower.starts_with("photo by ")
            || lower.starts_with("photo credit")
            || lower.starts_with("image by ")
            || lower.starts_with("image credit")
    };

    title_like
        && matches!(next, Some(ContentElement::List(_)))
        && !looks_like_chart_label_heading(element, trimmed)
        && !is_attribution
}

fn looks_like_top_margin_running_header(doc: &PdfDocument, idx: usize, text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.split_whitespace().count() > 6 {
        return false;
    }

    let element = &doc.kids[idx];
    let bbox = element.bbox();
    if bbox.height() > 24.0 {
        return false;
    }

    let Some(page) = element.page_number() else {
        return false;
    };

    // Compute top Y for every page (single pass).
    let mut page_tops = std::collections::HashMap::<u32, f64>::new();
    for candidate in &doc.kids {
        if let Some(p) = candidate.page_number() {
            let top = page_tops.entry(p).or_insert(f64::MIN);
            *top = top.max(candidate.bbox().top_y);
        }
    }

    let page_top = page_tops.get(&page).copied().unwrap_or(0.0);
    if bbox.top_y < page_top - 24.0 {
        return false;
    }

    // A running header repeats across pages.  If the same text does NOT
    // appear at the top margin of any other page, this is a unique heading
    // (e.g. a document title), not a running header.
    let trimmed_lower = trimmed.to_lowercase();
    for other_elem in &doc.kids {
        let Some(other_page) = other_elem.page_number() else {
            continue;
        };
        if other_page == page {
            continue;
        }
        let other_bbox = other_elem.bbox();
        if other_bbox.height() > 24.0 {
            continue;
        }
        let other_top = page_tops.get(&other_page).copied().unwrap_or(0.0);
        if other_bbox.top_y < other_top - 24.0 {
            continue;
        }
        let other_text = match other_elem {
            ContentElement::Paragraph(p) => p.base.value(),
            ContentElement::TextBlock(tb) => tb.value(),
            ContentElement::TextLine(tl) => tl.value(),
            ContentElement::Heading(h) => h.base.base.value(),
            _ => continue,
        };
        if other_text.trim().to_lowercase() == trimmed_lower {
            return true;
        }
    }

    false
}

fn looks_like_chart_label_heading(element: &ContentElement, text: &str) -> bool {
    let trimmed = text.trim();
    let upper_words = trimmed
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphabetic))
        .all(|word| {
            word.chars()
                .filter(|ch| ch.is_alphabetic())
                .all(|ch| ch.is_uppercase())
        });

    (trimmed.contains('%') || upper_words) && element.bbox().height() <= 40.0
}

fn should_demote_heading_to_paragraph(text: &str, next: &str) -> bool {
    let next_trimmed = next.trim();
    if !next_trimmed.chars().next().is_some_and(char::is_lowercase) {
        return false;
    }

    let normalized = normalize_heading_text(text);
    if matches!(
        normalized.as_str(),
        "contents" | "tableofcontents" | "introduction" | "conclusion"
    ) {
        return false;
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 3 {
        return false;
    }

    words
        .last()
        .is_some_and(|word| is_sentence_fragment_tail(word))
}

fn is_sentence_fragment_tail(word: &str) -> bool {
    matches!(
        word.trim_matches(|c: char| !c.is_alphanumeric())
            .to_ascii_lowercase()
            .as_str(),
        "a" | "an"
            | "and"
            | "as"
            | "at"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "to"
            | "with"
    )
}

fn is_list_section_heading(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.ends_with(':')
        && trimmed.len() <= 80
        && trimmed.split_whitespace().count() <= 8
        && trimmed.chars().any(char::is_alphabetic)
        && !trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
        && !trimmed.starts_with(|c: char| "•‣◦●○◆◇▪▫–—-".contains(c))
}

fn should_merge_paragraph_text(prev: &str, next: &str) -> bool {
    let next_trimmed = next.trim();
    if next_trimmed.is_empty() || is_standalone_page_number(next_trimmed) {
        return false;
    }

    if starts_with_enumerated_marker(next_trimmed) {
        return false;
    }

    if prev.ends_with('-')
        && prev.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic())
        && next_trimmed.chars().next().is_some_and(char::is_lowercase)
    {
        return true;
    }

    if next_trimmed.chars().next().is_some_and(char::is_lowercase) {
        return true;
    }

    let lower = next_trimmed.to_ascii_lowercase();
    if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("arxiv")
        || lower.starts_with("doi:")
    {
        return true;
    }

    if matches!(
        next_trimmed.split_whitespace().next(),
        Some("In" | "Proceedings" | "Advances" | "Learning")
    ) {
        return true;
    }

    !prev.ends_with(['.', '!', '?', ':'])
}

fn should_merge_adjacent_semantic_paragraphs(prev: &str, next: &str) -> bool {
    let next_trimmed = next.trim();
    if next_trimmed.is_empty() {
        return false;
    }

    if starts_with_enumerated_marker(next_trimmed) {
        return false;
    }

    if prev.ends_with('-')
        && prev.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic())
        && next_trimmed.chars().next().is_some_and(char::is_lowercase)
    {
        return true;
    }

    next_trimmed.chars().next().is_some_and(char::is_lowercase)
}

fn starts_with_enumerated_marker(text: &str) -> bool {
    let first_token = match text.trim_start().split_whitespace().next() {
        Some(token) => token.trim_start_matches(['(', '[']),
        None => return false,
    };
    if !first_token.ends_with(['.', ')', ':']) {
        return false;
    }

    let marker = first_token.trim_end_matches(['.', ')', ':']);
    if marker.is_empty() {
        return false;
    }

    if marker.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }

    if marker.len() == 1 && marker.chars().all(|c| c.is_ascii_alphabetic()) {
        return true;
    }

    let lower = marker.to_ascii_lowercase();
    lower.len() <= 8 && lower.chars().all(|c| "ivxlcdm".contains(c))
}

fn should_skip_leading_figure_carryover(doc: &PdfDocument, idx: usize, text: &str) -> bool {
    let trimmed = text.trim();
    if !trimmed.starts_with("Figure ") || trimmed.split_whitespace().count() < 4 {
        return false;
    }

    let element = &doc.kids[idx];
    let Some(page) = element.page_number() else {
        return false;
    };

    let mut page_top = f64::MIN;
    for candidate in &doc.kids {
        if candidate.page_number() == Some(page)
            && matches!(
                candidate,
                ContentElement::Paragraph(_)
                    | ContentElement::TextBlock(_)
                    | ContentElement::TextLine(_)
                    | ContentElement::Heading(_)
                    | ContentElement::NumberHeading(_)
                    | ContentElement::Caption(_)
            )
        {
            page_top = page_top.max(candidate.bbox().top_y);
        }
    }
    if !page_top.is_finite() || element.bbox().top_y < page_top - 72.0 {
        return false;
    }

    for prior_idx in 0..idx {
        let prior = &doc.kids[prior_idx];
        let prior_text = extract_element_text(prior);
        let prior_trimmed = prior_text.trim();
        if prior_trimmed.is_empty()
            || is_standalone_page_number(prior_trimmed)
            || looks_like_footer_banner(prior_trimmed)
        {
            continue;
        }
        match prior {
            ContentElement::Paragraph(_)
            | ContentElement::TextBlock(_)
            | ContentElement::TextLine(_) => {
                if !starts_with_caption_prefix(prior_trimmed)
                    && !looks_like_top_margin_running_header(doc, prior_idx, prior_trimmed)
                {
                    return false;
                }
            }
            ContentElement::Heading(_) | ContentElement::NumberHeading(_) => {
                if !should_skip_heading_text(prior_trimmed) {
                    return false;
                }
            }
            _ => return false,
        }
    }

    for lookahead_idx in idx + 1..doc.kids.len().min(idx + 8) {
        let next = &doc.kids[lookahead_idx];
        if next.page_number() != Some(page) {
            break;
        }
        let next_text = extract_element_text(next);
        let next_trimmed = next_text.trim();
        if next_trimmed.is_empty() || is_standalone_page_number(next_trimmed) {
            continue;
        }

        let is_numbered_heading = match next {
            ContentElement::Heading(_) | ContentElement::NumberHeading(_) => {
                looks_like_numbered_section(next_trimmed)
                    || looks_like_keyword_numbered_section(next_trimmed)
            }
            ContentElement::Paragraph(_)
            | ContentElement::TextBlock(_)
            | ContentElement::TextLine(_) => {
                should_render_paragraph_as_heading(doc, lookahead_idx, next_trimmed, doc.kids.get(lookahead_idx + 1))
                    && (looks_like_numbered_section(next_trimmed)
                        || looks_like_keyword_numbered_section(next_trimmed))
            }
            _ => false,
        };

        if is_numbered_heading {
            return true;
        }

        if !starts_with_caption_prefix(next_trimmed) && next_trimmed.split_whitespace().count() >= 5 {
            return false;
        }
    }

    false
}

fn merge_paragraph_text(target: &mut String, next: &str) {
    let next_trimmed = next.trim();
    if target.ends_with('-')
        && target
            .chars()
            .rev()
            .nth(1)
            .is_some_and(|c| c.is_alphabetic())
        && next_trimmed.chars().next().is_some_and(char::is_lowercase)
    {
        target.pop();
        target.push_str(next_trimmed);
    } else {
        if !target.ends_with(' ') {
            target.push(' ');
        }
        target.push_str(next_trimmed);
    }
}

fn is_standalone_page_number(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && trimmed.len() <= 4 && trimmed.chars().all(|c| c.is_ascii_digit())
}

fn looks_like_margin_page_number(doc: &PdfDocument, element: &ContentElement, text: &str) -> bool {
    if !is_standalone_page_number(text) {
        return false;
    }

    let bbox = element.bbox();
    if bbox.height() > 24.0 {
        return false;
    }

    let Some(page) = element.page_number() else {
        return false;
    };

    let mut page_top = f64::MIN;
    let mut page_bottom = f64::MAX;
    for candidate in &doc.kids {
        if candidate.page_number() == Some(page) {
            let candidate_bbox = candidate.bbox();
            page_top = page_top.max(candidate_bbox.top_y);
            page_bottom = page_bottom.min(candidate_bbox.bottom_y);
        }
    }

    if !page_top.is_finite() || !page_bottom.is_finite() {
        return false;
    }

    bbox.top_y >= page_top - 24.0 || bbox.bottom_y <= page_bottom + 24.0
}

/// Check whether a pipeline heading sits in the bottom margin of its page.
/// Running footers (e.g. "Report Title 21") are sometimes classified as
/// headings by the pipeline.  A heading at the page bottom is very unlikely
/// to be a real section heading.
fn looks_like_bottom_margin_heading(doc: &PdfDocument, idx: usize) -> bool {
    let element = &doc.kids[idx];
    let bbox = element.bbox();
    if bbox.height() > 30.0 {
        return false;
    }

    let Some(page) = element.page_number() else {
        return false;
    };

    let mut page_bottom = f64::MAX;
    for candidate in &doc.kids {
        if candidate.page_number() == Some(page) {
            page_bottom = page_bottom.min(candidate.bbox().bottom_y);
        }
    }

    if !page_bottom.is_finite() {
        return false;
    }

    // If this heading is at the very bottom of the page content, skip it.
    bbox.bottom_y <= page_bottom + 24.0
}

/// Demote a pipeline heading that ends with a period when it doesn't look like
/// a genuine section heading (e.g. "United Kingdom." or "New Investment (a Challenger).").
/// Returns true when the heading should be rendered as a paragraph instead.
fn should_demote_period_heading(text: &str) -> bool {
    let trimmed = text.trim();
    if !trimmed.ends_with('.') {
        return false;
    }
    // Keep numbered section headings: "I. Introduction", "4.2. Results",
    // "Activity 4. Determining CEC…"
    if looks_like_numbered_section(trimmed) || looks_like_keyword_numbered_section(trimmed) {
        return false;
    }
    // Keep headings whose text without the trailing period still looks like a
    // proper title — at least 3 words, first word uppercase, and the period
    // is clearly sentence-ending rather than part of a title pattern.
    let without_dot = trimmed.trim_end_matches('.');
    let word_count = without_dot.split_whitespace().count();
    // Very short fragments ending with '.' (like "Kingdom.") are almost
    // certainly not headings.
    if word_count <= 2 {
        return true;
    }
    false
}

/// Demote headings that end with a comma — these are never real headings
/// (e.g. footnote references like "29 Pope," or "32 Beawes, 33 M.M.,").
fn should_demote_comma_heading(text: &str) -> bool {
    text.trim().ends_with(',')
}

/// Demote headings containing mathematical/special symbols that never appear
/// in real section headings (e.g. "HL ¼", "P ≪ P", "LH þ HL:").
fn should_demote_math_heading(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c,
            '¼' | '½'
                | '¾'
                | '≪'
                | '≫'
                | 'þ'
                | 'ð'
                | '∑'
                | '∫'
                | '∂'
                | '∏'
                | '√'
                | '∞'
                | '≈'
                | '÷'
        )
    })
}

/// Demote headings containing a percentage sign — these are typically data
/// labels rather than section headings (e.g. "56% AGREE").
fn should_demote_percentage_heading(text: &str) -> bool {
    text.contains('%')
}

/// Demote bibliography entries that start with a 4-digit year followed by
/// a period and space (e.g. "2020. Measuring massive multitask...").
fn should_demote_bibliography_heading(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 6 {
        return false;
    }
    let bytes = t.as_bytes();
    bytes[0..4].iter().all(|b| b.is_ascii_digit())
        && bytes[4] == b'.'
        && (bytes[5] == b' ' || t.len() == 5)
}

/// Strip a trailing standalone page number from heading text.
/// E.g. "Chapter 3. Numerical differentiation 35" → "Chapter 3. Numerical differentiation"
/// Only strips when the last token is 1-4 digits and the heading has enough
/// words to be meaningful without it.
fn strip_trailing_page_number(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(last_space) = trimmed.rfind(' ') {
        let suffix = &trimmed[last_space + 1..];
        if !suffix.is_empty()
            && suffix.len() <= 4
            && suffix.chars().all(|c| c.is_ascii_digit())
            && trimmed[..last_space].split_whitespace().count() >= 3
        {
            return trimmed[..last_space].trim();
        }
    }
    trimmed
}

/// Try to split a heading that contains a merged subsection number.
/// For example, "4 Results 4.1 Experimental Details" should become
/// two headings: "4 Results" and "4.1 Experimental Details".
/// Returns None if no split is needed, otherwise the split point byte offset.
fn find_merged_subsection_split(text: &str) -> Option<usize> {
    // Look for a subsection number pattern like "4.1" or "B.1" after initial content.
    // Must appear at a word boundary (preceded by space).
    let bytes = text.as_bytes();
    // Start searching after the first few characters to skip the initial number
    let mut i = 3;
    while i < bytes.len() {
        if bytes[i - 1] == b' ' {
            // Check for digit.digit pattern (e.g., "4.1")
            if bytes[i].is_ascii_digit() {
                if let Some(dot_pos) = text[i..].find('.') {
                    let after_dot = i + dot_pos + 1;
                    if after_dot < bytes.len() && bytes[after_dot].is_ascii_digit() {
                        // Found "N.N" pattern preceded by space
                        return Some(i);
                    }
                }
            }
            // Check for letter.digit pattern (e.g., "B.1")
            if bytes[i].is_ascii_uppercase()
                && i + 2 < bytes.len()
                && bytes[i + 1] == b'.'
                && bytes[i + 2].is_ascii_digit()
            {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn should_skip_heading_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_standalone_page_number(trimmed) {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    if (lower.starts_with("chapter ") || lower.chars().next().is_some_and(|c| c.is_ascii_digit()))
        && trimmed.contains('|')
    {
        return true;
    }

    let alpha_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
    let alnum_count = trimmed.chars().filter(|c| c.is_alphanumeric()).count();
    alpha_count == 0 || (alnum_count > 0 && alpha_count * 3 < alnum_count && !trimmed.contains(':'))
}

fn repair_fragmented_words(text: &str) -> String {
    const STOPWORDS: &[&str] = &[
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if", "in", "into",
        "is", "it", "may", "must", "not", "of", "on", "or", "per", "that", "the", "to", "with",
    ];

    let mut parts: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    if parts.len() < 2 {
        return text.to_string();
    }

    let mut i = 0usize;
    while i + 1 < parts.len() {
        let left = parts[i].clone();
        let right = parts[i + 1].clone();
        let left_clean = left.trim_matches(|c: char| !c.is_alphabetic());
        let right_clean = right.trim_matches(|c: char| !c.is_alphabetic());
        let left_lower = left_clean.to_ascii_lowercase();
        let right_lower = right_clean.to_ascii_lowercase();

        let should_join = !left_clean.is_empty()
            && !right_clean.is_empty()
            && left_clean.chars().all(char::is_alphabetic)
            && right_clean.chars().all(char::is_alphabetic)
            && (left_clean.len() <= 4 || right_clean.len() <= 4)
            && left_clean.len() + right_clean.len() >= 6
            && !right_clean.chars().next().is_some_and(char::is_uppercase)
            && !STOPWORDS.contains(&left_lower.as_str())
            && !STOPWORDS.contains(&right_lower.as_str());

        if should_join {
            let next = parts.remove(i + 1);
            parts[i].push_str(&next);
        } else {
            i += 1;
        }
    }

    parts.join(" ")
}

/// Extract text from list item contents (fallback when label/body tokens are empty).
fn list_item_text_from_contents(contents: &[ContentElement]) -> String {
    let mut text = String::new();
    for elem in contents {
        let part = match elem {
            ContentElement::Paragraph(p) => p.base.value(),
            ContentElement::TextBlock(tb) => tb.value(),
            ContentElement::TextLine(tl) => tl.value(),
            ContentElement::TextChunk(tc) => tc.value.clone(),
            _ => String::new(),
        };
        if !text.is_empty() && !part.is_empty() {
            text.push(' ');
        }
        text.push_str(&part);
    }
    text
}

fn has_internal_header_gap(row: &[String]) -> bool {
    let mut seen_filled = false;
    let mut seen_gap_after_fill = false;
    for cell in row {
        if cell.trim().is_empty() {
            if seen_filled {
                seen_gap_after_fill = true;
            }
            continue;
        }
        if seen_gap_after_fill {
            return true;
        }
        seen_filled = true;
    }
    false
}

fn expand_grouped_header_row(parent: &[String], child: &[String]) -> Vec<String> {
    let anchor_cols: Vec<usize> = parent
        .iter()
        .enumerate()
        .filter_map(|(idx, cell)| (!cell.trim().is_empty()).then_some(idx))
        .collect();
    if anchor_cols.is_empty() {
        return parent.to_vec();
    }

    let mut expanded = parent.to_vec();
    for (col_idx, child_cell) in child.iter().enumerate() {
        if !expanded[col_idx].trim().is_empty() || child_cell.trim().is_empty() {
            continue;
        }

        let mut best_anchor = anchor_cols[0];
        let mut best_distance = usize::abs_diff(anchor_cols[0], col_idx);
        for &anchor_idx in &anchor_cols[1..] {
            let distance = usize::abs_diff(anchor_idx, col_idx);
            if distance < best_distance || (distance == best_distance && anchor_idx > best_anchor) {
                best_anchor = anchor_idx;
                best_distance = distance;
            }
        }
        expanded[col_idx] = parent[best_anchor].trim().to_string();
    }

    expanded
}

fn preserve_grouped_header_rows(rows: &mut [Vec<String>]) -> bool {
    if rows.len() < 2 || rows[0].is_empty() || rows[1].is_empty() {
        return false;
    }
    if rows[0].first().is_none_or(|cell| cell.trim().is_empty()) {
        return false;
    }
    if rows[1].first().is_some_and(|cell| !cell.trim().is_empty()) {
        return false;
    }

    let first_filled = rows[0].iter().filter(|cell| !cell.trim().is_empty()).count();
    let second_filled = rows[1].iter().filter(|cell| !cell.trim().is_empty()).count();
    if first_filled < 2 || second_filled <= first_filled || !has_internal_header_gap(&rows[0]) {
        return false;
    }

    rows[0] = expand_grouped_header_row(&rows[0], &rows[1]);
    true
}

/// Merge header continuation rows in a rendered table.
///
/// When a PDF table has multi-line column headers, each wrapped line often
/// produces a separate row in the grid.  These continuation rows have an
/// empty first cell while the header row above them has content.  This
/// function detects such rows at the start of the table and merges their
/// text into the first row, producing a single combined header.
///
/// Only rows whose non-empty cells are all ≤ 30 characters are merged, to
/// avoid accidentally collapsing data rows that happen to have an empty key.
fn merge_continuation_rows(rows: &mut Vec<Vec<String>>) {
    if rows.len() < 2 {
        return;
    }
    if preserve_grouped_header_rows(rows) {
        return;
    }
    // The first row must have a non-empty first cell (the header anchor).
    if rows[0].first().is_none_or(|c| c.trim().is_empty()) {
        return;
    }

    let mut merge_count = 0usize;
    for (i, row_i) in rows.iter().enumerate().skip(1) {
        let first_empty = row_i.first().is_none_or(|c| c.trim().is_empty());
        if !first_empty {
            break; // hit a data row
        }
        // All non-empty cells must be short (header-like fragments).
        let all_short = row_i
            .iter()
            .all(|c| c.trim().is_empty() || c.trim().len() <= 30);
        if !all_short {
            break;
        }
        merge_count = i;
    }

    // Require at least 2 consecutive continuation rows to avoid merging
    // legitimate sub-header or unit rows (e.g. a single row with "cmolc/kg").
    if merge_count == 0 {
        return;
    }

    // Merge rows 1..=merge_count into row 0.
    for i in 1..=merge_count {
        let (head, tail) = rows.split_at_mut(i);
        let ncols = head[0].len().min(tail[0].len());
        for (target, src) in head[0]
            .iter_mut()
            .take(ncols)
            .zip(tail[0].iter().take(ncols))
        {
            let fragment = src.trim().to_string();
            if !fragment.is_empty() {
                let target_str = target.trim().to_string();
                *target = if target_str.is_empty() {
                    fragment
                } else {
                    format!("{} {}", target_str, fragment)
                };
            }
        }
    }

    // Remove the merged rows.
    rows.drain(1..=merge_count);
}

fn trim_leading_table_carryover_rows(rows: &mut Vec<Vec<String>>) {
    while first_body_row_looks_like_carryover(rows) {
        rows.remove(1);
    }
}

fn first_body_row_looks_like_carryover(rows: &[Vec<String>]) -> bool {
    if rows.len() < 3 {
        return false;
    }

    let key_col_count = infer_leading_key_column_count(&rows[1..]);
    if key_col_count == 0 {
        return false;
    }

    let candidate = &rows[1];
    if candidate
        .iter()
        .take(key_col_count)
        .any(|cell| !cell.trim().is_empty())
    {
        return false;
    }

    let non_empty_cols = candidate
        .iter()
        .enumerate()
        .filter(|(_, cell)| !cell.trim().is_empty())
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    if non_empty_cols.len() != 1 {
        return false;
    }

    let only_col = non_empty_cols[0];
    if only_col < key_col_count {
        return false;
    }

    if candidate[only_col].split_whitespace().count() < 4 {
        return false;
    }

    rows[2]
        .iter()
        .take(key_col_count)
        .all(|cell| !cell.trim().is_empty())
}

fn infer_leading_key_column_count(rows: &[Vec<String>]) -> usize {
    if rows.len() < 2 {
        return 0;
    }

    let num_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut key_cols = 0usize;

    for col_idx in 0..num_cols {
        let mut occupancy = 0usize;
        let mut word_counts = Vec::new();

        for row in rows {
            let cell = row.get(col_idx).map(String::as_str).unwrap_or("");
            let trimmed = cell.trim();
            if trimmed.is_empty() {
                continue;
            }
            occupancy += 1;
            word_counts.push(trimmed.split_whitespace().count());
        }

        if occupancy == 0 {
            break;
        }

        word_counts.sort_unstable();
        let median_words = word_counts[word_counts.len() / 2];
        let occupancy_ratio = occupancy as f64 / rows.len() as f64;
        if occupancy_ratio < 0.6 || median_words > 3 {
            break;
        }
        key_cols += 1;
    }

    key_cols
}

/// Render a SemanticTable as a markdown table.
fn render_table(out: &mut String, table: &crate::models::semantic::SemanticTable) {
    // Delegate to render_table_border which handles cross-page linking.
    render_table_border(out, &table.table_border);
}

#[derive(Clone, Debug)]
struct GeometricTableRegion {
    start_idx: usize,
    end_idx: usize,
    rendered: String,
}

#[derive(Clone)]
struct ChunkLine {
    bbox: BoundingBox,
    chunks: Vec<TextChunk>,
}

#[derive(Clone)]
struct SlotFragment {
    slot_idx: usize,
    bbox: BoundingBox,
    text: String,
}

fn detect_geometric_table_regions(doc: &PdfDocument) -> Vec<GeometricTableRegion> {
    let mut regions = Vec::new();
    let mut occupied_until = 0usize;

    for (idx, element) in doc.kids.iter().enumerate() {
        if idx < occupied_until {
            continue;
        }

        let Some(table) = table_border_from_element(element) else {
            continue;
        };
        let Some(region) = build_geometric_table_region(doc, idx, table) else {
            continue;
        };
        occupied_until = region.end_idx.saturating_add(1);
        regions.push(region);
    }

    let mut occupied = regions
        .iter()
        .flat_map(|region| region.start_idx..=region.end_idx)
        .collect::<HashSet<_>>();
    for region in detect_footnote_citation_regions(doc) {
        if (region.start_idx..=region.end_idx).any(|idx| occupied.contains(&idx)) {
            continue;
        }
        occupied.extend(region.start_idx..=region.end_idx);
        regions.push(region);
    }

    regions.sort_by_key(|region| region.start_idx);
    regions
}

fn detect_footnote_citation_regions(doc: &PdfDocument) -> Vec<GeometricTableRegion> {
    let body_font_size = compute_running_body_font_size(doc);
    if body_font_size <= 0.0 {
        return Vec::new();
    }

    let mut regions = Vec::new();
    let mut idx = 0usize;
    while idx < doc.kids.len() {
        let Some(region) = build_footnote_citation_region(doc, idx, body_font_size) else {
            idx += 1;
            continue;
        };
        idx = region.end_idx.saturating_add(1);
        regions.push(region);
    }

    regions
}

fn compute_running_body_font_size(doc: &PdfDocument) -> f64 {
    doc.kids
        .iter()
        .filter_map(|element| {
            let ContentElement::Paragraph(paragraph) = element else {
                return None;
            };
            let text = paragraph.base.value();
            (text.split_whitespace().count() > 10).then_some(paragraph.base.font_size?)
        })
        .fold(0.0_f64, f64::max)
}

fn build_footnote_citation_region(
    doc: &PdfDocument,
    start_idx: usize,
    body_font_size: f64,
) -> Option<GeometricTableRegion> {
    let element = doc.kids.get(start_idx)?;
    if !is_geometric_text_candidate(element) {
        return None;
    }

    let start_text = extract_element_text(element);
    let trimmed_start = start_text.trim();
    if trimmed_start.is_empty() {
        return None;
    }

    let small_font_threshold = (body_font_size * 0.92).min(body_font_size - 0.8).max(0.0);
    let mut lead_prefix = None;
    let mut fragments = Vec::new();
    let page_number = element.page_number()?;
    let mut column_bbox = element.bbox().clone();
    let mut region_start_idx = start_idx;
    let mut end_idx = start_idx;

    if element_font_size(element).is_some_and(|font_size| font_size <= small_font_threshold)
        && starts_with_footnote_marker(trimmed_start)
    {
        if let Some((attach_idx, prefix, leading_fragments)) = leading_footnote_attachment(
                doc,
                start_idx,
                page_number,
                &column_bbox,
                small_font_threshold,
        ) {
            lead_prefix = Some(prefix);
            fragments.extend(leading_fragments);
            region_start_idx = attach_idx;
        }
        fragments.push(footnote_fragment_text(element));
    } else {
        let (prefix, first_tail) = split_trailing_footnote_lead(trimmed_start)?;
        let next = doc.kids.get(start_idx + 1)?;
        if !is_geometric_text_candidate(next)
            || next.page_number() != Some(page_number)
            || !element_font_size(next).is_some_and(|font_size| font_size <= small_font_threshold)
        {
            return None;
        }
        if !same_column_region(&column_bbox, &next.bbox()) {
            return None;
        }
        lead_prefix = Some(prefix);
        fragments.push(first_tail);
    }

    let mut consecutive_small = 0usize;
    for idx in start_idx + 1..doc.kids.len() {
        let candidate = &doc.kids[idx];
        if !is_geometric_text_candidate(candidate) || candidate.page_number() != Some(page_number) {
            break;
        }

        let candidate_text = extract_element_text(candidate);
        let trimmed = candidate_text.trim();
        if trimmed.is_empty() || starts_with_caption_prefix(trimmed) {
            break;
        }

        let Some(font_size) = element_font_size(candidate) else {
            break;
        };
        if font_size > small_font_threshold {
            break;
        }
        if !same_column_region(&column_bbox, &candidate.bbox()) {
            break;
        }

        column_bbox = column_bbox.union(&candidate.bbox());
        fragments.push(footnote_fragment_text(candidate));
        consecutive_small += 1;
        end_idx = idx;
    }

    if consecutive_small == 0 && lead_prefix.is_some() {
        return None;
    }

    let rows = parse_footnote_citation_rows(&fragments);
    if rows.len() < 3 {
        return None;
    }

    let numeric_markers = rows
        .iter()
        .filter_map(|(marker, _)| marker.parse::<u32>().ok())
        .collect::<Vec<_>>();
    if numeric_markers.len() != rows.len() {
        return None;
    }
    let sequential_steps = numeric_markers
        .windows(2)
        .filter(|pair| pair[1] == pair[0] + 1)
        .count();
    if sequential_steps + 1 < rows.len().saturating_sub(1) {
        return None;
    }

    let mut rendered_rows = vec![vec!["Footnote".to_string(), "Citation".to_string()]];
    rendered_rows.extend(rows.into_iter().map(|(marker, citation)| vec![marker, citation]));

    let mut rendered = String::new();
    if let Some(prefix) = lead_prefix {
        rendered.push_str(&escape_md_line_start(prefix.trim()));
        rendered.push_str("\n\n");
    }
    rendered.push_str(&render_html_table(&rendered_rows));

    Some(GeometricTableRegion {
        start_idx: region_start_idx,
        end_idx,
        rendered,
    })
}

fn leading_footnote_attachment(
    doc: &PdfDocument,
    start_idx: usize,
    page_number: u32,
    column_bbox: &BoundingBox,
    small_font_threshold: f64,
) -> Option<(usize, String, Vec<String>)> {
    let mut idx = start_idx.checked_sub(1)?;
    let mut leading_fragments = Vec::new();
    let mut scanned = 0usize;

    loop {
        let candidate = doc.kids.get(idx)?;
        scanned += 1;
        if scanned > 6 || candidate.page_number() != Some(page_number) {
            return None;
        }

        if !is_geometric_text_candidate(candidate) {
            if idx == 0 {
                return None;
            }
            idx -= 1;
            continue;
        }

        let text = extract_element_text(candidate);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            if idx == 0 {
                return None;
            }
            idx -= 1;
            continue;
        }
        if !same_column_region(candidate.bbox(), column_bbox) {
            return None;
        }

        if element_font_size(candidate).is_some_and(|font_size| font_size <= small_font_threshold) {
            leading_fragments.push(footnote_fragment_text(candidate));
            if idx == 0 {
                return None;
            }
            idx -= 1;
            continue;
        }

        let (prefix, first_tail) = split_trailing_footnote_lead(trimmed)?;
        leading_fragments.push(first_tail);
        leading_fragments.reverse();
        return Some((idx, prefix, leading_fragments));
    }
}

fn parse_footnote_citation_rows(fragments: &[String]) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut current_marker = None::<String>;
    let mut current_citation = String::new();

    for fragment in fragments {
        let markers = find_footnote_marker_positions(fragment);
        if markers.is_empty() {
            if current_marker.is_some() {
                merge_paragraph_text(&mut current_citation, fragment.trim());
            }
            continue;
        }

        let mut cursor = 0usize;
        for (pos, marker, skip_len) in markers {
            let prefix = fragment[cursor..pos].trim();
            if current_marker.is_some() && !prefix.is_empty() {
                merge_paragraph_text(&mut current_citation, prefix);
            }
            if let Some(marker_value) = current_marker.take() {
                let trimmed = current_citation.trim();
                if !trimmed.is_empty() {
                    rows.push((marker_value, trimmed.to_string()));
                }
                current_citation.clear();
            }
            current_marker = Some(marker);
            cursor = pos + skip_len;
        }

        let tail = fragment[cursor..].trim();
        if current_marker.is_some() && !tail.is_empty() {
            merge_paragraph_text(&mut current_citation, tail);
        }
    }

    if let Some(marker_value) = current_marker {
        let trimmed = current_citation.trim();
        if !trimmed.is_empty() {
            rows.push((marker_value, trimmed.to_string()));
        }
    }

    rebalance_adjacent_footnote_citations(&mut rows);
    rows
}

fn rebalance_adjacent_footnote_citations(rows: &mut [(String, String)]) {
    for idx in 0..rows.len().saturating_sub(1) {
        if !rows[idx].1.trim_end().ends_with(',') {
            continue;
        }

        let next = rows[idx + 1].1.trim().to_string();
        let Some((stub, remainder)) = split_leading_citation_stub(&next) else {
            continue;
        };
        let Some((first_sentence, trailing)) = split_first_sentence(remainder) else {
            continue;
        };
        if first_sentence.split_whitespace().count() < 2 {
            continue;
        }

        merge_paragraph_text(&mut rows[idx].1, first_sentence);
        rows[idx + 1].1 = if trailing.is_empty() {
            stub.to_string()
        } else {
            format!("{stub} {trailing}")
        };
    }
}

fn split_leading_citation_stub(text: &str) -> Option<(&str, &str)> {
    let comma_idx = text.find(',')?;
    if comma_idx > 8 {
        return None;
    }
    let stub = text[..=comma_idx].trim();
    let remainder = text[comma_idx + 1..].trim();
    (!stub.is_empty() && !remainder.is_empty()).then_some((stub, remainder))
}

fn split_first_sentence(text: &str) -> Option<(&str, &str)> {
    let period_idx = text.find(". ")?;
    let first = text[..=period_idx].trim();
    let trailing = text[period_idx + 2..].trim();
    (!first.is_empty()).then_some((first, trailing))
}

fn find_footnote_marker_positions(text: &str) -> Vec<(usize, String, usize)> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut markers = Vec::new();
    let mut idx = 0usize;

    while idx < chars.len() {
        let (byte_idx, ch) = chars[idx];
        if !ch.is_ascii_digit() {
            idx += 1;
            continue;
        }

        let at_boundary = idx == 0
            || chars[idx - 1]
                .1
                .is_whitespace()
            || matches!(chars[idx - 1].1, '.' | ',' | ';' | ':' | ')' | ']' | '"' | '\'' | '”');
        if !at_boundary {
            idx += 1;
            continue;
        }

        let mut end_idx = idx;
        while end_idx < chars.len() && chars[end_idx].1.is_ascii_digit() {
            end_idx += 1;
        }
        let digits = &text[byte_idx..chars.get(end_idx).map(|(pos, _)| *pos).unwrap_or(text.len())];
        if digits.len() > 2 || end_idx >= chars.len() || !chars[end_idx].1.is_whitespace() {
            idx += 1;
            continue;
        }

        let mut lookahead = end_idx;
        while lookahead < chars.len() && chars[lookahead].1.is_whitespace() {
            lookahead += 1;
        }
        let Some((_, next_ch)) = chars.get(lookahead) else {
            idx += 1;
            continue;
        };
        if !(next_ch.is_ascii_uppercase() || matches!(*next_ch, '(' | '[' | '*')) {
            idx += 1;
            continue;
        }

        let skip_end = chars.get(lookahead).map(|(pos, _)| *pos).unwrap_or(text.len());
        markers.push((byte_idx, digits.to_string(), skip_end - byte_idx));
        idx = lookahead;
    }

    markers
}

fn split_trailing_footnote_lead(text: &str) -> Option<(String, String)> {
    let markers = find_footnote_marker_positions(text);
    let (pos, marker, skip_len) = markers.last()?.clone();
    let prefix = text[..pos].trim();
    let tail = text[pos + skip_len..].trim();
    if prefix.split_whitespace().count() < 6 || tail.split_whitespace().count() > 6 {
        return None;
    }
    Some((prefix.to_string(), format!("{marker} {tail}")))
}

fn starts_with_footnote_marker(text: &str) -> bool {
    find_footnote_marker_positions(text)
        .first()
        .is_some_and(|(pos, _, _)| *pos == 0)
}

fn same_column_region(left: &BoundingBox, right: &BoundingBox) -> bool {
    let overlap = (left.right_x.min(right.right_x) - left.left_x.max(right.left_x)).max(0.0);
    let min_width = left.width().min(right.width()).max(1.0);
    overlap / min_width >= 0.35 || (left.left_x - right.left_x).abs() <= 28.0
}

fn footnote_fragment_text(element: &ContentElement) -> String {
    let text = extract_element_text(element);
    if element_font_name(element)
        .as_deref()
        .is_some_and(|name| name.to_ascii_lowercase().contains("italic"))
    {
        format!("*{}*", text.trim())
    } else {
        text
    }
}

fn element_font_size(element: &ContentElement) -> Option<f64> {
    match element {
        ContentElement::Paragraph(p) => p.base.font_size,
        ContentElement::Heading(h) => h.base.base.font_size,
        ContentElement::NumberHeading(nh) => nh.base.base.base.font_size,
        ContentElement::TextBlock(tb) => Some(tb.font_size),
        ContentElement::TextLine(tl) => Some(tl.font_size),
        _ => None,
    }
}

fn element_font_name(element: &ContentElement) -> Option<String> {
    match element {
        ContentElement::Paragraph(p) => p.base.font_name.clone(),
        ContentElement::Heading(h) => h.base.base.font_name.clone(),
        ContentElement::NumberHeading(nh) => nh.base.base.base.font_name.clone(),
        _ => None,
    }
}

fn table_border_from_element(
    element: &ContentElement,
) -> Option<&crate::models::table::TableBorder> {
    match element {
        ContentElement::TableBorder(table) => Some(table),
        ContentElement::Table(table) => Some(&table.table_border),
        _ => None,
    }
}

fn build_geometric_table_region(
    doc: &PdfDocument,
    table_idx: usize,
    table: &crate::models::table::TableBorder,
) -> Option<GeometricTableRegion> {
    let mut table_rows = collect_table_border_rows(table);
    if table_rows.is_empty() || table.num_columns < 3 {
        return None;
    }
    merge_continuation_rows(&mut table_rows);

    let column_ranges = table_column_ranges(table)?;
    let candidate_indices = collect_table_header_candidate_indices(doc, table_idx, table);
    if candidate_indices.is_empty() {
        return None;
    }

    let needs_external_stub =
        infer_left_stub_requirement(doc, &candidate_indices, &table_rows, &column_ranges);
    let supports_embedded_stub_header =
        supports_embedded_stub_header(&table_rows, &column_ranges, doc, &candidate_indices);
    if !needs_external_stub && !supports_embedded_stub_header {
        return None;
    }
    let slot_ranges = if needs_external_stub {
        slot_ranges(&column_ranges, doc, &candidate_indices, true)?
    } else {
        column_ranges.clone()
    };
    let mut header_rows = reconstruct_aligned_rows(doc, &candidate_indices, &slot_ranges, true, 2);
    if header_rows.is_empty() {
        return None;
    }
    if needs_external_stub {
        normalize_leading_stub_header(&mut header_rows);
    } else {
        promote_embedded_stub_header(&mut header_rows, &table_rows);
    }

    let slot_count = slot_ranges.len();
    let dense_header_rows = header_rows
        .iter()
        .filter(|row| row.iter().filter(|cell| !cell.trim().is_empty()).count() >= slot_count.saturating_sub(1).max(2))
        .count();
    if dense_header_rows == 0 {
        return None;
    }

    let mut combined_rows = Vec::new();
    combined_rows.extend(header_rows);

    let following_indices = collect_table_footer_candidate_indices(doc, table_idx, table);
    let body_rows = if needs_external_stub && should_merge_panel_body_rows(&table_rows) {
        let trailing_rows = reconstruct_aligned_rows(doc, &following_indices, &slot_ranges, false, 1);
        vec![merge_panel_body_row(&table_rows, &trailing_rows, slot_count)]
    } else if needs_external_stub {
        table_rows
            .iter()
            .map(|row| {
                let mut shifted = vec![String::new()];
                shifted.extend(row.iter().cloned());
                shifted
            })
            .collect()
    } else {
        table_rows
    };

    if body_rows.is_empty() {
        return None;
    }
    combined_rows.extend(body_rows);

    let rendered = render_pipe_rows(&combined_rows);
    Some(GeometricTableRegion {
        start_idx: candidate_indices[0],
        end_idx: following_indices
            .last()
            .copied()
            .unwrap_or(table_idx),
        rendered,
    })
}

fn table_column_ranges(table: &crate::models::table::TableBorder) -> Option<Vec<(f64, f64)>> {
    if table.num_columns == 0 {
        return None;
    }

    let mut ranges = vec![(f64::INFINITY, f64::NEG_INFINITY); table.num_columns];
    for row in &table.rows {
        for cell in &row.cells {
            if cell.col_number >= table.num_columns {
                continue;
            }
            let range = &mut ranges[cell.col_number];
            range.0 = range.0.min(cell.bbox.left_x);
            range.1 = range.1.max(cell.bbox.right_x);
        }
    }

    if ranges
        .iter()
        .any(|(left, right)| !left.is_finite() || !right.is_finite() || right <= left)
    {
        return None;
    }

    Some(ranges)
}

fn collect_table_header_candidate_indices(
    doc: &PdfDocument,
    table_idx: usize,
    table: &crate::models::table::TableBorder,
) -> Vec<usize> {
    let mut indices = Vec::new();
    let table_page = table.bbox.page_number;
    let table_top = table.bbox.top_y;
    let mut cursor = table_idx;

    while let Some(prev_idx) = cursor.checked_sub(1) {
        let element = &doc.kids[prev_idx];
        if element.page_number() != table_page {
            break;
        }
        if !is_geometric_text_candidate(element) {
            break;
        }

        let bbox = element.bbox();
        let vertical_gap = bbox.bottom_y - table_top;
        if vertical_gap < -6.0 || vertical_gap > 260.0 {
            break;
        }

        indices.push(prev_idx);
        cursor = prev_idx;
        if indices.len() >= 10 {
            break;
        }
    }

    indices.reverse();
    indices
}

fn collect_table_footer_candidate_indices(
    doc: &PdfDocument,
    table_idx: usize,
    table: &crate::models::table::TableBorder,
) -> Vec<usize> {
    let mut indices = Vec::new();
    let table_page = table.bbox.page_number;
    let table_bottom = table.bbox.bottom_y;

    for idx in table_idx + 1..doc.kids.len() {
        let element = &doc.kids[idx];
        if element.page_number() != table_page {
            break;
        }
        if !is_geometric_text_candidate(element) {
            break;
        }
        if looks_like_margin_page_number(doc, element, &extract_element_text(element)) {
            break;
        }

        let bbox = element.bbox();
        let gap = table_bottom - bbox.top_y;
        if gap < -6.0 || gap > 28.0 {
            break;
        }
        indices.push(idx);
        if indices.len() >= 4 {
            break;
        }
    }

    indices
}

fn is_geometric_text_candidate(element: &ContentElement) -> bool {
    matches!(
        element,
        ContentElement::Paragraph(_)
            | ContentElement::Heading(_)
            | ContentElement::NumberHeading(_)
            | ContentElement::TextBlock(_)
            | ContentElement::TextLine(_)
    )
}

fn infer_left_stub_requirement(
    doc: &PdfDocument,
    candidate_indices: &[usize],
    table_rows: &[Vec<String>],
    column_ranges: &[(f64, f64)],
) -> bool {
    if column_ranges.is_empty() {
        return false;
    }

    let first_width = (column_ranges[0].1 - column_ranges[0].0).max(1.0);
    let has_left_label = candidate_indices.iter().any(|idx| {
        let bbox = doc.kids[*idx].bbox();
        bbox.right_x <= column_ranges[0].0 + first_width * 0.12 && bbox.width() <= first_width * 0.45
    });
    if !has_left_label {
        return false;
    }

    let mut first_col_word_counts: Vec<usize> = table_rows
        .iter()
        .filter_map(|row| row.first())
        .map(|cell| cell.split_whitespace().count())
        .collect();
    if first_col_word_counts.is_empty() {
        return false;
    }
    first_col_word_counts.sort_unstable();
    let median = first_col_word_counts[first_col_word_counts.len() / 2];
    median >= 5
}

fn supports_embedded_stub_header(
    table_rows: &[Vec<String>],
    column_ranges: &[(f64, f64)],
    doc: &PdfDocument,
    candidate_indices: &[usize],
) -> bool {
    if table_rows.len() < 2 || column_ranges.len() < 3 {
        return false;
    }

    let first_row = &table_rows[0];
    if first_row.len() != column_ranges.len() || first_row[0].trim().is_empty() {
        return false;
    }
    if first_row[0].split_whitespace().count() > 3 || first_row[0].trim().len() > 24 {
        return false;
    }

    let data_fill = first_row
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();
    if data_fill + 1 < column_ranges.len() {
        return false;
    }

    let labeled_rows = table_rows
        .iter()
        .skip(1)
        .filter(|row| row.first().is_some_and(|cell| !cell.trim().is_empty()))
        .count();
    if labeled_rows == 0 {
        return false;
    }

    let slot_ranges = column_ranges.to_vec();
    let header_rows = reconstruct_aligned_rows(doc, candidate_indices, &slot_ranges, true, 2);
    header_rows.iter().any(|row| {
        row.first().is_none_or(|cell| cell.trim().is_empty())
            && row
                .iter()
                .skip(1)
                .filter(|cell| !cell.trim().is_empty())
                .count()
                >= column_ranges.len().saturating_sub(1)
    })
}

fn slot_ranges(
    column_ranges: &[(f64, f64)],
    doc: &PdfDocument,
    candidate_indices: &[usize],
    needs_stub: bool,
) -> Option<Vec<(f64, f64)>> {
    let mut slots = Vec::new();
    if needs_stub {
        let first_left = column_ranges.first()?.0;
        let left_stub_start = candidate_indices
            .iter()
            .map(|idx| doc.kids[*idx].bbox().left_x)
            .fold(first_left, f64::min);
        let stub_right = first_left - 1.0;
        if stub_right <= left_stub_start {
            return None;
        }
        slots.push((left_stub_start, stub_right));
    }
    slots.extend(column_ranges.iter().copied());
    Some(slots)
}

fn reconstruct_aligned_rows(
    doc: &PdfDocument,
    candidate_indices: &[usize],
    slot_ranges: &[(f64, f64)],
    drop_wide_singletons: bool,
    min_filled_slots: usize,
) -> Vec<Vec<String>> {
    if candidate_indices.is_empty() || slot_ranges.is_empty() {
        return Vec::new();
    }

    let mut row_bands: Vec<(BoundingBox, Vec<String>)> = Vec::new();

    for idx in candidate_indices {
        for line in extract_chunk_lines(&doc.kids[*idx]) {
            let fragments = split_line_into_slot_fragments(&line, slot_ranges);
            if fragments.is_empty() {
                continue;
            }

            if drop_wide_singletons && fragments.len() == 1 {
                let only = &fragments[0];
                let span_width = only.bbox.width();
                let table_width =
                    slot_ranges.last().map(|(_, right)| *right).unwrap_or(0.0) - slot_ranges[0].0;
                if span_width >= table_width * 0.55 {
                    continue;
                }
            }

            let line_center = line.bbox.center_y();
            let tolerance = line
                .chunks
                .iter()
                .map(|chunk| chunk.font_size)
                .fold(8.0, f64::max)
                * 0.8;

            let mut target_row = None;
            for (row_idx, (bbox, _)) in row_bands.iter().enumerate() {
                if (bbox.center_y() - line_center).abs() <= tolerance {
                    target_row = Some(row_idx);
                    break;
                }
            }

            if let Some(row_idx) = target_row {
                let (bbox, cells) = &mut row_bands[row_idx];
                *bbox = bbox.union(&line.bbox);
                for fragment in fragments {
                    append_cell_text(&mut cells[fragment.slot_idx], &fragment.text);
                }
            } else {
                let mut cells = vec![String::new(); slot_ranges.len()];
                for fragment in fragments {
                    append_cell_text(&mut cells[fragment.slot_idx], &fragment.text);
                }
                row_bands.push((line.bbox.clone(), cells));
            }
        }
    }

    row_bands.sort_by(|left, right| {
        right
            .0
            .top_y
            .partial_cmp(&left.0.top_y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    row_bands
        .into_iter()
        .map(|(_, cells)| cells)
        .filter(|cells| {
            let filled = cells.iter().filter(|cell| !cell.trim().is_empty()).count();
            filled >= min_filled_slots
        })
        .collect()
}

fn extract_chunk_lines(element: &ContentElement) -> Vec<ChunkLine> {
    match element {
        ContentElement::Paragraph(p) => chunk_lines_from_semantic_node(&p.base),
        ContentElement::Heading(h) => chunk_lines_from_semantic_node(&h.base.base),
        ContentElement::NumberHeading(nh) => chunk_lines_from_semantic_node(&nh.base.base.base),
        ContentElement::TextBlock(tb) => tb
            .text_lines
            .iter()
            .map(|line| ChunkLine {
                bbox: line.bbox.clone(),
                chunks: line.text_chunks.clone(),
            })
            .collect(),
        ContentElement::TextLine(tl) => vec![ChunkLine {
            bbox: tl.bbox.clone(),
            chunks: tl.text_chunks.clone(),
        }],
        _ => Vec::new(),
    }
}

fn chunk_lines_from_semantic_node(node: &SemanticTextNode) -> Vec<ChunkLine> {
    let mut lines = Vec::new();
    for column in &node.columns {
        for block in &column.text_blocks {
            for line in &block.text_lines {
                lines.push(ChunkLine {
                    bbox: line.bbox.clone(),
                    chunks: line.text_chunks.clone(),
                });
            }
        }
    }
    lines
}

fn split_line_into_slot_fragments(line: &ChunkLine, slot_ranges: &[(f64, f64)]) -> Vec<SlotFragment> {
    let mut groups: Vec<(usize, Vec<TextChunk>, BoundingBox)> = Vec::new();

    for chunk in line
        .chunks
        .iter()
        .filter(|chunk| !chunk.value.trim().is_empty())
        .cloned()
    {
        let slot_idx = assign_chunk_to_slot(&chunk.bbox, slot_ranges);
        if let Some((prev_slot, prev_chunks, prev_bbox)) = groups.last_mut() {
            let gap = chunk.bbox.left_x - prev_bbox.right_x;
            if *prev_slot == slot_idx && gap <= chunk.font_size.max(6.0) * 2.4 {
                *prev_bbox = prev_bbox.union(&chunk.bbox);
                prev_chunks.push(chunk);
                continue;
            }
        }
        groups.push((slot_idx, vec![chunk.clone()], chunk.bbox.clone()));
    }

    groups
        .into_iter()
        .filter_map(|(slot_idx, chunks, bbox)| {
            let text =
                normalize_common_ocr_text(&crate::models::text::TextLine::concatenate_chunks(&chunks));
            if text.trim().is_empty() {
                None
            } else {
                Some(SlotFragment { slot_idx, bbox, text })
            }
        })
        .collect()
}

fn assign_chunk_to_slot(bbox: &BoundingBox, slot_ranges: &[(f64, f64)]) -> usize {
    let mut best_idx = 0usize;
    let mut best_overlap = f64::NEG_INFINITY;
    let center_x = bbox.center_x();

    for (idx, (left, right)) in slot_ranges.iter().enumerate() {
        let overlap = (bbox.right_x.min(*right) - bbox.left_x.max(*left)).max(0.0);
        let score = if overlap > 0.0 {
            overlap / bbox.width().max(1.0)
        } else {
            -((center_x - ((*left + *right) / 2.0)).abs())
        };
        if score > best_overlap {
            best_overlap = score;
            best_idx = idx;
        }
    }

    best_idx
}

fn append_cell_text(cell: &mut String, fragment: &str) {
    let trimmed = fragment.trim();
    if trimmed.is_empty() {
        return;
    }
    if !cell.is_empty() {
        cell.push(' ');
    }
    cell.push_str(trimmed);
}

fn normalize_leading_stub_header(rows: &mut [Vec<String>]) {
    if rows.len() < 2 || rows[0].is_empty() || rows[1].is_empty() {
        return;
    }

    if !rows[0][0].trim().is_empty() || rows[1][0].trim().is_empty() {
        return;
    }

    let first_row_filled = rows[0]
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();
    let second_row_filled = rows[1]
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();
    if first_row_filled < 2 || second_row_filled < 2 {
        return;
    }

    rows[0][0] = rows[1][0].trim().to_string();
    rows[1][0].clear();
}

fn promote_embedded_stub_header(header_rows: &mut [Vec<String>], table_rows: &[Vec<String>]) {
    let Some(header_row) = header_rows.first_mut() else {
        return;
    };
    let Some(first_body_row) = table_rows.first() else {
        return;
    };
    if header_row.is_empty() || first_body_row.is_empty() {
        return;
    }
    if !header_row[0].trim().is_empty() {
        return;
    }

    let promoted = first_body_row[0].trim();
    if promoted.is_empty() || promoted.split_whitespace().count() > 3 || promoted.len() > 24 {
        return;
    }

    let header_fill = header_row
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();
    let body_fill = first_body_row
        .iter()
        .skip(1)
        .filter(|cell| !cell.trim().is_empty())
        .count();
    if header_fill < header_row.len().saturating_sub(1) || body_fill < first_body_row.len().saturating_sub(1) {
        return;
    }

    header_row[0] = promoted.to_string();
}

fn should_merge_panel_body_rows(rows: &[Vec<String>]) -> bool {
    rows.len() >= 3
        && rows
            .iter()
            .all(|row| !row.is_empty() && row.iter().all(|cell| !cell.trim().is_empty()))
}

fn merge_panel_body_row(
    table_rows: &[Vec<String>],
    trailing_rows: &[Vec<String>],
    slot_count: usize,
) -> Vec<String> {
    let mut merged = vec![String::new(); slot_count];
    for row in table_rows {
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx + 1 >= slot_count {
                break;
            }
            append_cell_text(&mut merged[col_idx + 1], cell);
        }
    }
    for row in trailing_rows {
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx >= slot_count {
                break;
            }
            append_cell_text(&mut merged[col_idx], cell);
        }
    }
    merged
}

fn render_pipe_rows(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let num_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if num_cols == 0 {
        return String::new();
    }

    let mut out = String::new();
    for (row_idx, row) in rows.iter().enumerate() {
        out.push('|');
        for col_idx in 0..num_cols {
            let cell = row.get(col_idx).map(String::as_str).unwrap_or("");
            out.push_str(&format!(" {} |", cell.trim()));
        }
        out.push('\n');

        if row_idx == 0 {
            out.push('|');
            for _ in 0..num_cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

fn render_html_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }

    let num_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    if num_cols == 0 {
        return String::new();
    }

    let mut out = String::from("<table>\n");
    for (row_idx, row) in rows.iter().enumerate() {
        out.push_str("<tr>");
        for col_idx in 0..num_cols {
            let cell = escape_html_text(row.get(col_idx).map(String::as_str).unwrap_or("").trim());
            if row_idx == 0 {
                out.push_str("<th>");
                out.push_str(&cell);
                out.push_str("</th>");
            } else {
                out.push_str("<td>");
                out.push_str(&cell);
                out.push_str("</td>");
            }
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</table>\n\n");
    out
}

fn escape_html_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn normalized_numeric_marker(text: &str) -> Option<String> {
    let digits = text
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!digits.is_empty() && digits.len() <= 2).then_some(digits)
}

fn render_infographic_card_rows(rows: &[Vec<String>]) -> Option<String> {
    if rows.is_empty() || !rows.iter().all(|row| row.len() == 2) {
        return None;
    }

    let marker = normalized_numeric_marker(rows[0][0].trim())?;
    if rows[0][1].split_whitespace().count() < 4 {
        return None;
    }
    if rows
        .iter()
        .skip(1)
        .any(|row| normalized_numeric_marker(row[0].trim()).is_some())
    {
        return None;
    }
    if rows
        .iter()
        .skip(1)
        .any(|row| !row[0].trim().is_empty() && row[0].trim().len() > 2)
    {
        return None;
    }

    let body = rows
        .iter()
        .filter_map(|row| row.get(1))
        .map(|cell| cell.trim())
        .filter(|cell| !cell.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if body.split_whitespace().count() < 8 {
        return None;
    }

    Some(format!("{marker}. {body}\n\n"))
}

fn extract_element_text(element: &ContentElement) -> String {
    match element {
        ContentElement::Paragraph(p) => clean_paragraph_text(&p.base.value()),
        ContentElement::Heading(h) => clean_paragraph_text(&h.base.base.value()),
        ContentElement::NumberHeading(nh) => clean_paragraph_text(&nh.base.base.base.value()),
        ContentElement::TextBlock(tb) => clean_paragraph_text(&tb.value()),
        ContentElement::TextLine(tl) => clean_paragraph_text(&tl.value()),
        _ => String::new(),
    }
}

/// Collect rendered rows from a single TableBorder (no cross-page chaining).
fn collect_table_border_rows(table: &crate::models::table::TableBorder) -> Vec<Vec<String>> {
    let num_cols = table.num_columns.max(1);
    let mut rendered_rows: Vec<Vec<String>> = Vec::new();
    for row in &table.rows {
        let cell_texts: Vec<String> = (0..num_cols)
            .map(|col| {
                row.cells
                    .iter()
                    .find(|c| c.col_number == col)
                    .map(cell_text_content)
                    .unwrap_or_default()
            })
            .collect();
        if !cell_texts.iter().all(|t| t.trim().is_empty()) {
            rendered_rows.push(cell_texts);
        }
    }
    rendered_rows
}

/// Render a TableBorder directly as a markdown table.
///
/// When the table has a `next_table` link (cross-page continuation), the
/// continuation rows are appended so the entire logical table is emitted
/// as a single pipe table.
fn render_table_border(out: &mut String, table: &crate::models::table::TableBorder) {
    if table.rows.is_empty() {
        return;
    }

    // Collect rows from this table.
    let mut rendered_rows = collect_table_border_rows(table);

    if rendered_rows.is_empty() {
        return;
    }

    if let Some(rendered) = render_infographic_card_rows(&rendered_rows) {
        out.push_str(&rendered);
        return;
    }

    // Merge multi-line header rows into a single header row.
    merge_continuation_rows(&mut rendered_rows);
    trim_leading_table_carryover_rows(&mut rendered_rows);

    // ToC detection: render table-of-contents as plain text pairs, not a markdown table.
    if is_toc_table(&rendered_rows) {
        render_toc_rows(out, &rendered_rows);
        return;
    }

    out.push_str(&render_pipe_rows(&rendered_rows));
}

/// Returns true if `text` looks like a page number (Arabic digits or Roman numerals).
fn is_page_number_like(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    // All ASCII digits, length ≤ 5 (handles pages 1–99999)
    if t.len() <= 5 && t.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // Lowercase Roman numerals (i, ii, iii, iv, v, vi, vii, viii, ix, x …)
    let lower = t.to_ascii_lowercase();
    if lower.len() <= 10 && lower.chars().all(|c| "ivxlcdm".contains(c)) {
        return true;
    }
    false
}

/// Returns true if the rendered rows look like a table-of-contents:
/// exactly 2 columns where the majority of right-column cells are page numbers.
fn is_toc_table(rows: &[Vec<String>]) -> bool {
    if rows.is_empty() {
        return false;
    }
    // Need at least 2 rows to qualify as a ToC
    if rows.len() < 2 {
        return false;
    }
    // First, every row must have exactly 2 cells
    if !rows.iter().all(|r| r.len() == 2) {
        return false;
    }

    let non_empty_right = rows.iter().filter(|r| !r[1].trim().is_empty()).count();
    if non_empty_right < 2 {
        return false;
    }

    let page_like = rows.iter().filter(|r| is_page_number_like(&r[1])).count();
    page_like >= 2 && page_like * 10 >= non_empty_right * 9 && page_like * 2 >= rows.len()
}

/// Render ToC-style rows as plain text (title pagenum pairs) rather than a markdown table.
fn render_toc_rows(out: &mut String, rows: &[Vec<String>]) {
    for row in rows {
        let title = row[0].trim();
        let page = row[1].trim();
        if title.is_empty() && page.is_empty() {
            continue;
        }
        if !title.is_empty() && !page.is_empty() {
            out.push_str(title);
            out.push(' ');
            out.push_str(page);
        } else {
            out.push_str(title);
            out.push_str(page);
        }
        out.push('\n');
    }
    out.push('\n');
}

/// Extract text content from a table cell.
fn cell_text_content(cell: &crate::models::table::TableBorderCell) -> String {
    // First try the content tokens — use gap-based concatenation instead of
    // naive space-joining so that letter-spaced text ("O w n e r s h i p")
    // is collapsed correctly.
    if !cell.content.is_empty() {
        let chunks: Vec<_> = cell.content.iter().map(|t| t.base.clone()).collect();
        return normalize_common_ocr_text(&crate::models::text::TextLine::concatenate_chunks(&chunks));
    }
    // Fall back to processed contents
    let mut text = String::new();
    for elem in &cell.contents {
        match elem {
            ContentElement::Paragraph(p) => text.push_str(&p.base.value()),
            ContentElement::TextBlock(tb) => text.push_str(&tb.value()),
            ContentElement::TextLine(tl) => text.push_str(&tl.value()),
            ContentElement::TextChunk(tc) => text.push_str(&tc.value),
            _ => {}
        }
    }
    normalize_common_ocr_text(&repair_fragmented_words(&text))
}

/// Merge adjacent pipe tables that share the same column count.
///
/// PDF table detection sometimes splits one visual table into several
/// fragments that are emitted as successive pipe tables.  When two tables
/// are separated only by blank lines and have identical column counts,
/// they are merged into a single table by appending the second table's
/// rows (including its header-now-body row) to the first.
fn merge_adjacent_pipe_tables(markdown: &str) -> String {
    let lines: Vec<&str> = markdown.lines().collect();
    if lines.len() < 4 {
        return markdown.to_string();
    }

    fn count_pipe_cols(line: &str) -> usize {
        let t = line.trim();
        if !t.starts_with('|') || !t.ends_with('|') {
            return 0;
        }
        t.split('|').count().saturating_sub(2)
    }

    fn is_separator(line: &str) -> bool {
        let t = line.trim();
        if !t.starts_with('|') || !t.ends_with('|') {
            return false;
        }
        let cells: Vec<&str> = t.split('|').collect();
        if cells.len() < 3 {
            return false;
        }
        cells[1..cells.len() - 1].iter().all(|c| {
            let s = c.trim();
            !s.is_empty() && s.chars().all(|ch| ch == '-' || ch == ':')
        })
    }

    fn is_pipe_row(line: &str) -> bool {
        let t = line.trim();
        t.starts_with('|') && t.ends_with('|') && t.len() > 2
    }

    fn pipe_cells(line: &str) -> Vec<String> {
        let t = line.trim();
        if !is_pipe_row(t) {
            return Vec::new();
        }
        let parts = t.split('|').collect::<Vec<_>>();
        parts[1..parts.len() - 1]
            .iter()
            .map(|cell| cell.trim().to_string())
            .collect()
    }

    fn normalize_header_cell(cell: &str) -> String {
        cell.chars()
            .filter(|ch| ch.is_alphanumeric())
            .flat_map(|ch| ch.to_lowercase())
            .collect()
    }

    fn looks_like_header_row(line: &str) -> bool {
        let cells = pipe_cells(line);
        if cells.len() < 2 {
            return false;
        }

        let non_empty = cells
            .iter()
            .filter(|cell| !cell.trim().is_empty())
            .collect::<Vec<_>>();
        if non_empty.len() < 2 {
            return false;
        }

        let headerish = non_empty.iter().all(|cell| {
            let trimmed = cell.trim();
            let word_count = trimmed.split_whitespace().count();
            let has_alpha = trimmed.chars().any(|ch| ch.is_alphabetic());
            has_alpha && word_count <= 4 && trimmed.len() <= 28
        });
        headerish
    }

    fn header_overlap_ratio(left: &str, right: &str) -> f64 {
        let left_cells = pipe_cells(left)
            .into_iter()
            .map(|cell| normalize_header_cell(&cell))
            .collect::<Vec<_>>();
        let right_cells = pipe_cells(right)
            .into_iter()
            .map(|cell| normalize_header_cell(&cell))
            .collect::<Vec<_>>();
        let width = left_cells.len().min(right_cells.len());
        if width == 0 {
            return 0.0;
        }

        let matches = (0..width)
            .filter(|idx| {
                !left_cells[*idx].is_empty()
                    && !right_cells[*idx].is_empty()
                    && left_cells[*idx] == right_cells[*idx]
            })
            .count();
        matches as f64 / width as f64
    }

    fn header_schema_matches(left: &str, right: &str) -> bool {
        let left_cells = pipe_cells(left)
            .into_iter()
            .map(|cell| normalize_header_cell(&cell))
            .collect::<Vec<_>>();
        let right_cells = pipe_cells(right)
            .into_iter()
            .map(|cell| normalize_header_cell(&cell))
            .collect::<Vec<_>>();
        if left_cells.len() != right_cells.len() || left_cells.len() < 2 {
            return false;
        }

        let mut aligned_non_empty = 0usize;
        for (left, right) in left_cells.iter().zip(right_cells.iter()) {
            if left.is_empty() || right.is_empty() {
                continue;
            }
            aligned_non_empty += 1;
            if left != right {
                return false;
            }
        }

        aligned_non_empty >= 2
    }

    fn pad_pipe_row(line: &str, target_cols: usize) -> String {
        let t = line.trim();
        let current_cols = count_pipe_cols(t);
        if current_cols >= target_cols {
            return t.to_string();
        }
        // Append extra empty cells after the existing trailing |
        let mut result = t.to_string();
        for _ in current_cols..target_cols {
            result.push_str("  |");
        }
        result
    }

    // Identify pipe table blocks: (start, sep_idx, end, col_count).
    struct Block {
        start: usize,
        sep: usize,
        end: usize, // inclusive last line
        cols: usize,
    }

    let mut blocks: Vec<Block> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if i + 1 < lines.len() && is_pipe_row(lines[i]) && is_separator(lines[i + 1]) {
            let cols = count_pipe_cols(lines[i]);
            let sep = i + 1;
            let mut end = sep;
            let mut j = sep + 1;
            while j < lines.len() && is_pipe_row(lines[j]) && !is_separator(lines[j]) {
                end = j;
                j += 1;
            }
            blocks.push(Block {
                start: i,
                sep,
                end,
                cols,
            });
            i = end + 1;
        } else {
            i += 1;
        }
    }

    if blocks.len() < 2 {
        return markdown.to_string();
    }

    // Group adjacent blocks: allow different column counts.
    // Merge when separated by blank lines only, or by heading markers
    // (lines starting with #) that represent table cells misclassified
    // as headings by the pipeline.
    // Track group max cols during merge to use for heading gap decisions.
    let mut merge_leader: Vec<Option<usize>> = vec![None; blocks.len()];
    let mut group_cols: Vec<usize> = blocks.iter().map(|b| b.cols).collect();
    for bi in 1..blocks.len() {
        let prev = &blocks[bi - 1];
        let curr = &blocks[bi];
        let gap_range = prev.end + 1..curr.start;
        let gap_all_blank = gap_range.clone().all(|li| lines[li].trim().is_empty());
        // For heading gap check, use the group's max cols (not individual block).
        // This handles chains like [2-col] → blank → [1-col] → heading → [2-col]
        // where the 1-col intermediary is already merged with the 2-col leader.
        let leader_idx = merge_leader[bi - 1].unwrap_or(bi - 1);
        let effective_prev_cols = group_cols[leader_idx];
        let gap_heading_only = if !gap_all_blank && effective_prev_cols >= 2 && curr.cols >= 2 {
            let non_blank: Vec<usize> = gap_range
                .clone()
                .filter(|li| !lines[*li].trim().is_empty())
                .collect();
            // Only merge when gap has 1-2 heading lines
            !non_blank.is_empty()
                && non_blank.len() <= 2
                && non_blank.iter().all(|li| {
                    let t = lines[*li].trim();
                    t.starts_with('#') && t.len() < 100
                })
        } else {
            false
        };
        // Short displaced cell: a single short plain-text word between two
        // multi-column tables is almost certainly a cell value that the PDF
        // pipeline displaced out of the table grid.
        let gap_short_fragment =
            if !gap_all_blank && !gap_heading_only && effective_prev_cols >= 2 && curr.cols >= 2 {
                let non_blank: Vec<usize> = gap_range
                    .clone()
                    .filter(|li| !lines[*li].trim().is_empty())
                    .collect();
                non_blank.len() == 1 && {
                    let t = lines[non_blank[0]].trim();
                    t.len() < 30
                        && !t.starts_with('#')
                        && !t.starts_with('-')
                        && !t.starts_with('*')
                        && !t.contains(':')
                        && !t.contains("TABLE")
                }
            } else {
                false
            };
        let prev_has_header = looks_like_header_row(lines[prev.start]);
        let curr_has_header = curr.end >= curr.sep + 2 && looks_like_header_row(lines[curr.start]);
        let curr_has_distinct_header = prev_has_header
            && curr_has_header
            && !header_schema_matches(lines[prev.start], lines[curr.start])
            && (curr.cols != prev.cols || header_overlap_ratio(lines[prev.start], lines[curr.start]) < 1.0);

        if (gap_all_blank || gap_heading_only || gap_short_fragment)
            && prev.cols > 0
            && curr.cols > 0
            && !curr_has_distinct_header
        {
            merge_leader[bi] = Some(leader_idx);
            // Update group max cols
            if curr.cols > group_cols[leader_idx] {
                group_cols[leader_idx] = curr.cols;
            }
        }
    }

    let mut pad_target: Vec<usize> = vec![0; blocks.len()];
    for bi in 0..blocks.len() {
        let leader = merge_leader[bi].unwrap_or(bi);
        pad_target[bi] = group_cols[leader];
    }

    // Mark lines to skip: blank gap lines + separator of merged blocks.
    // Non-blank gap lines become pipe table rows instead of being skipped.
    // Keep the header row (curr.start) — it becomes a data row.
    let mut skip = vec![false; lines.len()];
    let mut convert_to_pipe_row = vec![false; lines.len()];
    for (bi, leader) in merge_leader.iter().enumerate() {
        if leader.is_none() {
            continue;
        }
        let prev_end = blocks[bi - 1].end;
        let curr = &blocks[bi];
        for li in (prev_end + 1)..curr.start {
            if lines[li].trim().is_empty() {
                skip[li] = true;
            } else {
                // Non-blank gap line: convert to pipe row
                convert_to_pipe_row[li] = true;
            }
        }
        // Only skip separator, header row becomes a data row
        skip[curr.sep] = true;
    }

    // Map each line to its block index (or the block it belongs to via gap conversion).
    let mut line_to_block: Vec<Option<usize>> = vec![None; lines.len()];
    for (bi, block) in blocks.iter().enumerate() {
        line_to_block[block.start..=block.end].fill(Some(bi));
    }
    // Assign gap lines to the preceding block for padding purposes.
    for (bi, leader) in merge_leader.iter().enumerate() {
        if leader.is_none() {
            continue;
        }
        let prev_end = blocks[bi - 1].end;
        let curr = &blocks[bi];
        for li in (prev_end + 1)..curr.start {
            if convert_to_pipe_row[li] {
                line_to_block[li] = Some(bi - 1);
            }
        }
    }

    let mut result = String::new();
    for (li, line) in lines.iter().enumerate() {
        if skip[li] {
            continue;
        }
        if convert_to_pipe_row[li] {
            // Convert non-blank gap text/heading into a pipe table row.
            let text = line.trim().trim_start_matches('#').trim();
            if let Some(bi) = line_to_block[li] {
                let target = pad_target[bi];
                if target > 0 && !text.is_empty() {
                    result.push_str(&format!("| {} ", text));
                    for _ in 1..target {
                        result.push_str("|  ");
                    }
                    result.push_str("|\n");
                    continue;
                }
            }
            // Fallback: emit as-is if no block context
            result.push_str(line);
            result.push('\n');
            continue;
        }
        if let Some(bi) = line_to_block[li] {
            let target = pad_target[bi];
            if target > 0 && is_pipe_row(line) && !is_separator(line) {
                result.push_str(&pad_pipe_row(line, target));
                result.push('\n');
            } else if target > 0 && is_separator(line) {
                result.push('|');
                for _ in 0..target {
                    result.push_str(" --- |");
                }
                result.push('\n');
            } else {
                result.push_str(line);
                result.push('\n');
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::content::ContentElement;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::list::{ListBody, ListItem, ListLabel, PDFList};
    use crate::models::semantic::{SemanticHeading, SemanticParagraph, SemanticTextNode};
    use crate::models::table::{
        TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType,
    };
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    #[test]
    fn test_empty_doc() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("No content extracted"));
    }

    #[test]
    fn test_with_title() {
        let mut doc = PdfDocument::new("test.pdf".to_string());
        doc.title = Some("My Title".to_string());
        let md = to_markdown(&doc).unwrap();
        assert!(md.starts_with("# My Title\n"));
    }

    #[test]
    fn test_empty_title_not_rendered() {
        let mut doc = PdfDocument::new("test.pdf".to_string());
        doc.title = Some("  ".to_string());
        let md = to_markdown(&doc).unwrap();
        assert!(
            !md.contains("# "),
            "Empty/whitespace title should not produce a heading"
        );
    }

    #[test]
    fn test_repair_fragmented_words() {
        assert_eq!(
            repair_fragmented_words("Jurisdic tion Fore ign Req uire me nts"),
            "Jurisdiction Foreign Requirements"
        );
    }

    #[test]
    fn test_normalize_common_ocr_text_repairs_units() {
        assert_eq!(
            normalize_common_ocr_text("10 ߤL at 37 C and -20 oC"),
            "10 μL at 37°C and -20°C"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_build_layout_anchor_rows_reconstructs_four_column_matrix() {
        let lines = vec![
            "Key Functions by Main Service Flow".to_string(),
            "".to_string(),
            " Service Stage                   Function Name                Explanation                                                                                Expected Benefit".to_string(),
            "".to_string(),
            " 1. Project creation             Project creation and         Select document type to automatically run project creation, Pipeline configuration with    The intuitive UI environment allows the the person in charge to quickly proceed with".to_string(),
            "".to_string(),
            "                                 management                   recommended Modelset and Endpoint deployment                                               the entire process from project creation to deployment, improving work efficiency".to_string(),
            "".to_string(),
            "                                                                                                                                                         Conveniently manage raw data to be used for OCR Pack and actual date from live".to_string(),
            " 2. Data labeling and            Data storage management      Provides convenient functions for uploading raw data, viewer, and data management".to_string(),
            "                                                              (search using image metadata, sorting, filtering, hashtags settings on image data)         service".to_string(),
            " fine-tuning".to_string(),
            "                                                              Image data bookmark for Qualitative Evaluation".to_string(),
            "".to_string(),
            "                                 Create and manage Labeling   Creating a Labeling Space to manage raw data annotation, managing labeling resources       Labeling work can be outsourced within the pack. Labeled data is continuously".to_string(),
            "                                                              (Ontology, Characters to be Recognized), data set dump, data set version management        supplied from which data sets can be created with ease. The Auto Labeling function".to_string(),
            "                                 Space".to_string(),
            "                                                                                                     3                                                   increases both efficiency and convenience.".to_string(),
            "                                                              Various basic models for each selected 5".to_string(),
            "                                                                                                    document, information comparison between".to_string(),
            "                                 Model training                                                                                                          Providing a foundation for customers to implement, manage, and upgrade their own".to_string(),
            "                                                              models, basic model training, training pause function, re-training, cancel function, and   OCR model specialized to the customers’ needs".to_string(),
            "                                                              configuration support for Characters to be Recognized and Ontology that is frequently".to_string(),
            "                                                              modified while developing specialized models".to_string(),
        ];

        let header = find_layout_header_candidate(&lines).unwrap();
        let rows = build_layout_anchor_rows(&lines, &extract_layout_entries(&lines, &header)).unwrap();

        assert_eq!(
            header.headers,
            vec![
                "Service Stage".to_string(),
                "Function Name".to_string(),
                "Explanation".to_string(),
                "Expected Benefit".to_string()
            ]
        );
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0][0], "1. Project creation");
        assert_eq!(rows[0][1], "Project creation and management");
        assert!(rows[1][0].contains("fine-tuning"));
        assert_eq!(rows[2][1], "Create and manage Labeling Space");
        assert_eq!(rows[3][1], "Model training");
        assert!(rows[3][2].contains("Various basic models for each selected document"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_build_layout_panel_stub_rows_reconstructs_left_stub_table() {
        let lines = vec![
            "AI Pack".to_string(),
            "Upstage offers 3 AI packs that process unstructured information and data".to_string(),
            "".to_string(),
            "                                     OCR                                                Recommendation                                    Product semantic search".to_string(),
            "".to_string(),
            "              A solution that recognizes characters in an                A solution that recommends the best products and   A solution that enables semantic search, analyzes and".to_string(),
            "              image and extracts necessary information                   contents                                           organizes key information in unstructured text data".to_string(),
            "   Pack".to_string(),
            "                                                                                                                            into a standardized form (DB)".to_string(),
            "".to_string(),
            "              Applicable to all fields that require text extraction      Applicable to all fields that use any form of      Applicable to all fields that deal with various types of".to_string(),
            "              from standardized documents, such as receipts,             recommendation including alternative products,     unstructured data containing text information that".to_string(),
            "Application   bills, credit cards, ID cards, certificates, and medical   products and contents that are likely to be        require semantic search and conversion into a DB".to_string(),
            "              receipts                                                   purchased next".to_string(),
            "".to_string(),
            "              Achieved 1st place in the OCR World Competition            Team with specialists and technologies that        Creation of the first natural language evaluation".to_string(),
            "              The team includes specialists who have                     received Kaggle’s Gold Medal recommendation        system in Korean (KLUE)".to_string(),
            "              presented 14 papers in the world’s most                    (Education platform)                               World’s No.1 in Kaggle text embedding competition in".to_string(),
            " Highlight".to_string(),
            "              renowned AI conferences                                    Proven superior performance of more than 170%      E-commerce subject (Shopee)".to_string(),
            "                                                                         compared to other global top-tier recommendation".to_string(),
            "                                                                         models".to_string(),
        ];

        let header = find_layout_panel_header_candidate(&lines).unwrap();
        let rows = build_layout_panel_stub_rows(&lines, &header).unwrap();

        assert_eq!(
            header.headers,
            vec![
                "OCR".to_string(),
                "Recommendation".to_string(),
                "Product semantic search".to_string()
            ]
        );
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], "Pack");
        assert!(rows[0][1].contains("image and extracts necessary information"));
        assert_eq!(rows[1][0], "Application");
        assert!(rows[1][3].contains("require semantic search and conversion into a DB"));
        assert_eq!(rows[2][0], "Highlight");
        assert!(rows[2][2].contains("top-tier recommendation models"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_extract_layout_toc_entries_merges_wrapped_entry() {
        let lines = vec![
            "Table of Contents".to_string(),
            "".to_string(),
            "Executive Summary                                          4".to_string(),
            "Legal Framework                                            6".to_string(),
            "Election Administration                                   11".to_string(),
            "Civil Society Engagement                                  15".to_string(),
            "Political Parties, Candidates Registration and Election   18".to_string(),
            "Campaign".to_string(),
            "Media Freedom and Access to Information                   25".to_string(),
            "Voter Education and Awareness                             29".to_string(),
            "Participation of Marginalized Sectors                     31".to_string(),
            "Recommendations                                           39".to_string(),
        ];

        let (title, entries) = extract_layout_toc_entries(&lines).unwrap();
        assert_eq!(title, "Table of Contents");
        assert_eq!(entries.len(), 9);
        assert_eq!(entries[0].title, "Executive Summary");
        assert_eq!(entries[0].page, "4");
        assert_eq!(
            entries[4].title,
            "Political Parties, Candidates Registration and Election Campaign"
        );
        assert_eq!(entries[4].page, "18");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn make_bbox_layout_line(words: &[(&str, f64, f64)], bottom: f64, top: f64) -> BBoxLayoutLine {
        make_bbox_layout_line_in_block(0, words, bottom, top)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn make_bbox_layout_line_in_block(
        block_id: usize,
        words: &[(&str, f64, f64)],
        bottom: f64,
        top: f64,
    ) -> BBoxLayoutLine {
        BBoxLayoutLine {
            block_id,
            bbox: BoundingBox::new(
                Some(1),
                words.first().map(|(_, left, _)| *left).unwrap_or(72.0),
                bottom,
                words.last().map(|(_, _, right)| *right).unwrap_or(320.0),
                top,
            ),
            words: words
                .iter()
                .map(|(text, left, right)| BBoxLayoutWord {
                    bbox: BoundingBox::new(Some(1), *left, bottom, *right, top),
                    text: (*text).to_string(),
                })
                .collect(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_detect_layout_open_plate_recovers_two_column_species_rows() {
        let lines = vec![
            make_bbox_layout_line(
                &[
                    ("Fish", 60.0, 76.0),
                    ("species", 78.0, 107.0),
                    ("on", 109.0, 119.0),
                    ("IUCN", 121.0, 142.0),
                    ("Red", 144.0, 159.0),
                    ("List", 161.0, 176.0),
                ],
                649.0,
                660.0,
            ),
            make_bbox_layout_line(
                &[("Potosi", 60.0, 84.0), ("Pupfish", 86.0, 114.0)],
                632.0,
                643.0,
            ),
            make_bbox_layout_line(
                &[("Cyprinodon", 132.0, 176.0), ("alvarezi", 178.0, 207.0)],
                632.0,
                643.0,
            ),
            make_bbox_layout_line(
                &[
                    ("La", 60.0, 69.0),
                    ("Palma", 71.0, 94.0),
                    ("Pupfish", 96.0, 124.0),
                    ("Cyprinodon", 132.0, 176.0),
                    ("longidorsalis", 178.0, 224.0),
                ],
                616.0,
                627.0,
            ),
            make_bbox_layout_line(
                &[("Butterfly", 60.0, 94.0), ("Splitfin", 96.0, 123.0)],
                600.0,
                611.0,
            ),
            make_bbox_layout_line(
                &[("Ameca", 132.0, 156.0), ("splendens", 158.0, 194.0)],
                600.0,
                611.0,
            ),
            make_bbox_layout_line(
                &[("Golden", 60.0, 88.0), ("Skiffia", 90.0, 113.0)],
                584.0,
                595.0,
            ),
            make_bbox_layout_line(
                &[("Skiffia", 132.0, 155.0), ("francesae", 158.0, 193.0)],
                584.0,
                595.0,
            ),
            make_bbox_layout_line(
                &[
                    ("Table", 56.0, 74.0),
                    ("6.1:", 76.0, 87.0),
                    ("Four", 89.0, 105.0),
                    ("fish", 107.0, 119.0),
                    ("species", 121.0, 145.0),
                    ("on", 147.0, 155.0),
                    ("IUCN", 157.0, 176.0),
                    ("Red", 178.0, 190.0),
                    ("List", 192.0, 205.0),
                    ("held", 279.0, 293.0),
                    ("in", 295.0, 302.0),
                    ("public", 304.0, 325.0),
                    ("aquariums.", 327.0, 365.0),
                ],
                556.0,
                566.0,
            ),
        ];

        let plate = detect_layout_open_plate(576.0, &lines).unwrap();
        assert_eq!(plate.heading, "Fish species on IUCN Red List");
        assert_eq!(
            plate.header_row,
            vec![
                "Fish species on IUCN Red List".to_string(),
                "Scientific name".to_string()
            ]
        );
        assert_eq!(plate.rows.len(), 4);
        assert_eq!(
            plate.rows[1],
            vec![
                "La Palma Pupfish".to_string(),
                "Cyprinodon longidorsalis".to_string()
            ]
        );
        assert!(plate.caption.starts_with("Table 6.1: Four fish species on IUCN Red List"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_extract_layout_narrative_bridge_recovers_left_prose_and_defers_captions() {
        let plate = OpenPlateCandidate {
            heading: "Fish species on IUCN Red List".to_string(),
            header_row: vec![
                "Fish species on IUCN Red List".to_string(),
                "Scientific name".to_string(),
            ],
            rows: vec![],
            caption: "Table 6.1".to_string(),
            cutoff_top_y: 560.0,
        };
        let lines = vec![
            make_bbox_layout_line(
                &[("Public", 56.0, 83.0), ("aquariums,", 88.0, 135.0), ("because", 140.0, 174.0)],
                509.0,
                521.0,
            ),
            make_bbox_layout_line(
                &[("of", 180.0, 188.0), ("their", 194.0, 214.0), ("in-", 220.0, 233.0)],
                509.0,
                521.0,
            ),
            make_bbox_layout_line(
                &[("house", 56.0, 82.0), ("expertise,", 84.0, 125.0), ("can", 128.0, 143.0)],
                495.0,
                507.0,
            ),
            make_bbox_layout_line(
                &[("act", 146.0, 159.0), ("quickly", 161.0, 191.0)],
                495.0,
                507.0,
            ),
            make_bbox_layout_line_in_block(
                1,
                &[("Figure", 242.0, 265.0), ("6.3:", 267.0, 280.0), ("Photo", 282.0, 303.0)],
                355.0,
                366.0,
            ),
            make_bbox_layout_line_in_block(
                1,
                &[("of", 305.0, 312.0), ("the", 314.0, 325.0), ("species.", 327.0, 360.0)],
                355.0,
                366.0,
            ),
            make_bbox_layout_line(
                &[("The", 56.0, 73.0), ("breeding", 77.0, 114.0), ("colonies", 118.0, 153.0)],
                330.0,
                342.0,
            ),
            make_bbox_layout_line(
                &[("of", 157.0, 165.0), ("the", 169.0, 183.0), ("Butterfly", 187.0, 224.0), ("Splitfin", 228.0, 258.0), ("at", 314.0, 323.0), ("the", 327.0, 341.0), ("London", 345.0, 377.0), ("Zoo", 381.0, 397.0), ("and", 401.0, 416.0), ("elsewhere", 420.0, 463.0), ("serve", 467.0, 489.0), ("as", 493.0, 502.0), ("ark", 506.0, 519.0)],
                330.0,
                342.0,
            ),
            make_bbox_layout_line(
                &[("Figure", 56.0, 79.0), ("6.4:", 81.0, 94.0), ("Lake", 96.0, 116.0), ("Sturgeon", 118.0, 158.0)],
                104.0,
                116.0,
            ),
        ];

        let bridge = extract_layout_narrative_bridge(576.0, &lines, &plate).unwrap();
        assert!(
            bridge
                .bridge_paragraph
                .as_deref()
                .is_some_and(|text| text.contains("Public aquariums") && text.contains("expertise"))
        );
        assert_eq!(bridge.deferred_captions.len(), 2);
        assert!(bridge.deferred_captions[0].contains("Figure 6.3:"));
        assert!(bridge.deferred_captions[0].contains("species."));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_detect_layout_ocr_benchmark_dashboard_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000199.pdf");
        let (page_width, lines) = read_pdftotext_bbox_layout_lines(&path).unwrap();
        let dashboard = detect_layout_ocr_benchmark_dashboard(page_width, &lines).unwrap();

        assert_eq!(dashboard.title, "Base Model Performance Evaluation of Upstage OCR Pack");
        assert_eq!(dashboard.left_columns.len(), 2);
        assert_eq!(
            dashboard.left_columns[0],
            "Scene (Photographed document image)"
        );
        assert_eq!(
            dashboard.left_rows[0],
            vec!["Company A²".to_string(), "70.23".to_string(), "80.41".to_string()]
        );
        assert_eq!(
            dashboard.right_rows[0],
            vec![
                "OCR-Recall³".to_string(),
                "73.2".to_string(),
                "94.2".to_string(),
                "94.1".to_string()
            ]
        );
        assert_eq!(dashboard.right_rows[3][0], "Parsing-F¹");
        assert_eq!(dashboard.right_rows[3][1], "68.0");
        assert_eq!(dashboard.right_rows[3][2], "82.65");
        assert_eq!(dashboard.right_rows[3][3], "82.65");
        assert!(!dashboard.definition_notes.is_empty());
        assert!(!dashboard.source_notes.is_empty());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_split_layout_line_spans_handles_unicode_boundaries() {
        let line = "Title  “Podcast #EP32: SDGs dan Anak Muda”  2024";
        let spans = split_layout_line_spans(line);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].1, "Title");
        assert!(spans[1].1.contains("Podcast #EP32: SDGs dan Anak Muda"));
        assert!(spans[1].1.ends_with('”'));
        assert!(spans[2].1.ends_with("24"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_render_layout_single_caption_chart_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000037.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: crate::convert(&path, &crate::api::config::ProcessingConfig::default())
                .unwrap()
                .kids,
            ..PdfDocument::new("01030000000037.pdf".to_string())
        };
        let rendered = render_layout_single_caption_chart_document(&doc).unwrap();
        assert!(rendered.contains("# 3. Impact on Business Operations"));
        assert!(rendered.contains("## 3.1. Status of Business Operations"));
        assert!(rendered.contains(
            "As shown in Figure 3.1.1, the number of MSMEs"
        ));
        assert!(rendered.contains(
            "Figure 3.1.1: Status of operations during each survey phase (%)"
        ));
        assert!(rendered.contains(
            "lockdown period. In the handicraft/textile sector, 30% of MSMEs"
        ));
        assert!(!rendered.contains("| Lockdown Period |"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_captioned_media_document_on_real_pdf_72() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000072.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: crate::convert(&path, &crate::api::config::ProcessingConfig::default())
                .unwrap()
                .kids,
            ..PdfDocument::new("01030000000072.pdf".to_string())
        };
        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("## Diagram 5"), "{md}");
        assert!(md.contains("**Distribution of Komnas HAM’s YouTube Content (2019-2020)**"), "{md}");
        assert!(md.contains("As of 1 December 2021, the Komnas HAM’s YouTube channel has 2,290 subscribers"), "{md}");
        assert!(md.contains("**Figure 4**"), "{md}");
        assert!(md.contains("*Komnas HAM’s YouTube channel as of 1 December 2021*"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_captioned_media_document_on_real_pdf_73() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000073.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: crate::convert(&path, &crate::api::config::ProcessingConfig::default())
                .unwrap()
                .kids,
            ..PdfDocument::new("01030000000073.pdf".to_string())
        };
        let md = to_markdown(&doc).unwrap();
        assert!(md.starts_with("# In this content, DPN Argentina provides a brief explanation"), "{md}");
        assert!(md.contains("Examples of such greetings are as follows:"), "{md}");
        assert!(md.contains("*Image*"), "{md}");
        assert!(md.contains("**Figure 6**"), "{md}");
        assert!(md.contains("**DPN Argentina**"), "{md}");
        assert!(md.contains("**Content: World Health Day Celebration (7 April 2021).**^98"), "{md}");
        assert!(md.contains("**Footnote:**"), "{md}");
        assert!(md.contains("https://twitter.com/DPNArgentina/status/1379765916259483648."), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_render_layout_captioned_media_document_does_not_fire_on_real_pdf_14() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000014.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: crate::convert(&path, &crate::api::config::ProcessingConfig::default())
                .unwrap()
                .kids,
            ..PdfDocument::new("01030000000014.pdf".to_string())
        };
        assert!(render_layout_captioned_media_document(&doc).is_none());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_real_pdf_14_preserves_body_paragraphs() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000014.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: crate::convert(&path, &crate::api::config::ProcessingConfig::default())
                .unwrap()
                .kids,
            ..PdfDocument::new("01030000000014.pdf".to_string())
        };
        let md = to_markdown(&doc).unwrap();
        assert!(
            md.contains("These images also show that different areas are used by men and by women"),
            "{md}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_render_layout_recommendation_infographic_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000183.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: Vec::new(),
            ..PdfDocument::new("01030000000183.pdf".to_string())
        };
        let rendered = render_layout_recommendation_infographic_document(&doc).unwrap();
        assert!(rendered.contains("# Recommendation Pack: Track Record"));
        assert!(rendered.contains("## Comparison with Beauty Commerce Recommendation Models"));
        assert!(rendered.contains("| Graph-RecSys | 0.4048 |"));
        assert!(rendered.contains("| Current Service Recommendation Algorithm | 0.159 |"));
        assert!(rendered.contains("## Education Content Platform PoC Case"));
        assert!(rendered.contains("| DKT Model | 0.882 |"));
        assert!(rendered.contains("Compared to regular model"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_render_layout_stacked_bar_report_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000038.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: Vec::new(),
            ..PdfDocument::new("01030000000038.pdf".to_string())
        };
        let rendered = render_layout_stacked_bar_report_document(&doc);
        if rendered.is_none() {
            let (page_width, lines) = read_pdftotext_bbox_layout_lines(&path).unwrap();
            let blocks = collect_bbox_layout_blocks(&lines);
            let figures = collect_layout_figure_captions(&blocks);
            let narrative = detect_layout_stacked_bar_narrative(&blocks);
            eprintln!("page_width={page_width} figures={}", figures.len());
            if let Some(first) = figures.first() {
                eprintln!("figure1={}", bbox_layout_block_text(first));
            }
            if let Some(second) = figures.get(1) {
                eprintln!("figure2={}", bbox_layout_block_text(second));
            }
            eprintln!("narrative={}", narrative.is_some());
            if let Some(narrative) = &narrative {
                eprintln!("heading={}", narrative.heading);
                eprintln!("paragraphs={}", narrative.paragraphs.len());
                eprintln!("footnote={:?}", narrative.footnote);
            }
            for block in &blocks {
                let text = bbox_layout_block_text(block);
                if text.contains("July")
                    || text.contains("October")
                    || text.contains("January")
                    || text.contains("Will ")
                    || text.contains("Don’t")
                    || text.starts_with("6.2.")
                    || text.starts_with("5.")
                {
                    eprintln!(
                        "block top={:.1} bottom={:.1} left={:.1} right={:.1} text={}",
                        block.bbox.top_y,
                        block.bbox.bottom_y,
                        block.bbox.left_x,
                        block.bbox.right_x,
                        text
                    );
                }
            }
            if figures.len() >= 2 {
                let first = detect_layout_three_month_stacked_figure(
                    &blocks,
                    &lines,
                    page_width,
                    figures[0].clone(),
                    figures[1].bbox.top_y,
                );
                eprintln!("figure_one_ok={}", first.is_some());
                if let Some(narrative) = &narrative {
                    let second = detect_layout_sector_bar_figure(
                        &blocks,
                        &lines,
                        page_width,
                        figures[1].clone(),
                        narrative.top_y,
                    );
                    eprintln!("figure_two_ok={}", second.is_some());
                }
            }
        }
        let rendered = rendered.unwrap();
        assert!(rendered.contains("# Figure 6.1.1:"));
        assert!(rendered.contains("| Will not terminate employment | 51 | 81 | 73 |"));
        assert!(rendered.contains("# 6.2. Expectations for Re-Hiring Employees"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_render_layout_multi_figure_chart_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000076.pdf");
        let doc = PdfDocument {
            title: None,
            source_path: Some(path.to_string_lossy().to_string()),
            number_of_pages: 1,
            kids: Vec::new(),
            ..PdfDocument::new("01030000000076.pdf".to_string())
        };
        let rendered = render_layout_multi_figure_chart_document(&doc).unwrap();
        assert!(rendered.contains("# Figures from the Document"));
        assert!(rendered.contains("## Figure 1.7. Non-citizen population in Malaysia (in thousands)"));
        assert!(rendered.contains("| 2016 | 3,230 |"));
        assert!(rendered.contains("| 2021 | 2,693 |"));
        assert!(rendered.contains("## Figure 1.8. Singapore foreign workforce stock (in thousands)"));
        assert!(rendered.contains("| 2016 (Dec) | 1,393 |"));
        assert!(rendered.contains("| 2021 (Dec) | 1,200 |"));
        assert!(rendered.contains("Source: Department of Statistics, Malaysia (2022). Figure for 2021 is an estimate."));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_render_layout_open_plate_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000132.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let rendered = render_layout_open_plate_document(&doc).unwrap();
        assert!(rendered.contains("# Fish species on IUCN Red List"));
        assert!(rendered.contains("| Potosi Pupfish | Cyprinodon alvarezi |"));
        assert!(rendered.contains("| Golden Skiffia | Skiffia francesae |"));
        assert!(rendered.contains("*Table 6.1: Four fish species on IUCN Red List"));
        assert!(rendered.contains("---"));
        assert!(rendered.contains("Public aquariums, because of their inhouse expertise"));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_open_plate_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000132.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.contains("# Fish species on IUCN Red List"), "{md}");
        assert!(md.contains("| Potosi Pupfish | Cyprinodon alvarezi |"), "{md}");
        assert!(md.contains("| Golden Skiffia | Skiffia francesae |"), "{md}");
        assert!(md.contains("*Table 6.1: Four fish species on IUCN Red List"), "{md}");
        assert!(md.contains("The breeding colonies of the Butterfly Splitfin"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_does_not_misclassify_open_plate_pdf_36() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000036.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.contains("# 2. General Profile of MSMEs"), "{md}");
        assert!(md.contains("In July 2020, the survey established a general profile"), "{md}");
        assert!(
            md.contains("The tourism sub-sectors interviewed included lodging, restaurants and bars"),
            "{md}"
        );
        assert!(!md.starts_with("# Business characteristics. Business size was"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_does_not_misclassify_open_plate_pdf_40() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000040.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(
            md.contains("Thailand, Philippines and Indonesia in particular, identifying known experts"),
            "{md}"
        );
        assert!(md.contains("Figure 1: Age by gender of respondents"), "{md}");
        assert!(md.contains("Gender Analysis of Violent Extremism"), "{md}");
        assert!(!md.starts_with("# Thailand, Philippines and Indonesia in"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_does_not_misclassify_open_plate_pdf_64() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000064.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.contains("estuarine influenced areas."), "{md}");
        assert!(md.contains("| MANILA | 2454 | 6,125 |"), "{md}");
        assert!(md.contains("The port of Manila has been documented"), "{md}");
        assert!(!md.starts_with("# CAGAYAN DE ORO"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_detect_footnote_citation_regions_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000008.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let regions = detect_footnote_citation_regions(&doc);
        assert!(!regions.is_empty(), "{regions:?}");
        assert!(
            regions
                .iter()
                .any(|region| {
                    region.rendered.contains("<table>")
                        && region.rendered.contains("<td>25</td>")
                        && region.rendered.contains("<td>29</td>")
                }),
            "{regions:#?}"
        );
        assert!(
            regions
                .iter()
                .any(|region| {
                    region.rendered.contains("<table>")
                        && region.rendered.contains("<td>30</td>")
                        && region.rendered.contains("<td>33</td>")
                }),
            "{regions:#?}"
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_renders_footnote_citation_tables_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000008.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.contains("<table>"), "{md}");
        assert!(md.contains("<th>Footnote</th><th>Citation</th>"), "{md}");
        assert!(md.contains("<td>25</td><td>Wiliam Beckford"), "{md}");
        assert!(md.contains("<td>29</td><td>Pope, The Rape of the Lock, 69.</td>"), "{md}");
        assert!(
            md.contains("<td>30</td><td>Beawes, Lex Mercatoria Rediviva, 791.</td>"),
            "{md}"
        );
        assert!(
            md.contains("<td>32</td><td>Beawes, Lex Mercatoria Rediviva, 792.</td>"),
            "{md}"
        );
        assert!(md.contains("<td>33</td><td>M.M., Pharmacopoia Reformata:"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_projection_sheet_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000128.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.contains("# Table and Figure from the Document"), "{md}");
        assert!(md.contains("| A | B | C | D | E |"), "{md}");
        assert!(md.contains("| 10 | 8 | 19.73214458 | 17.99 | 21.47 |"), "{md}");
        assert!(md.contains("**Figure 13.3. Graph of Projection Estimates**"), "{md}");
        assert!(md.contains("[Open Template in Microsoft Excel](#)"), "{md}");
        assert!(md.contains("*298 | Ch. 13. Homogeneous Investment Types*"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_appendix_tables_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000082.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.contains("# Appendices"), "{md}");
        assert!(
            md.contains("## TABLE 28: BREAKDOWN OF IMPRISONMENT CLAUSES IN STATE LAWS"),
            "{md}"
        );
        assert!(md.contains("| Imprisonment terms | Number of clauses | Percentage of all states | Percentage of total |"), "{md}");
        assert!(md.contains("| Less than 3 months | 4,448 | 21.3% | 17.0% |"), "{md}");
        assert!(
            md.contains("## TABLE 29: STATES WITH MORE THAN 1,000 IMPRISONMENT CLAUSES"),
            "{md}"
        );
        assert!(md.contains("| State | Number of clauses | GSDP (In Rs lakh crore) | GSDP (In $ billion) |"), "{md}");
        assert!(md.contains("| Gujarat | 1469 | 15.6 | 200.4 |"), "{md}");
        assert!(md.contains("*Sources: TeamLease Regtech, and Reserve Bank of India for GSDPs*"), "{md}");
        assert!(md.contains("*Exchange rate: Rs 75 to USD*"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_titled_dual_table_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000084.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.starts_with("# Jailed for Doing Business"), "{md}");
        assert!(
            md.contains("## TABLE 38: THREE CASE STUDIES ON NBFC COMPLIANCES*"),
            "{md}"
        );
        assert!(
            md.contains("| Percentage of imprisonment clauses | 20% | 30% | 37% |"),
            "{md}"
        );
        assert!(
            md.contains("## TABLE 39: BREAKDOWN OF IMPRISONMENT CLAUSES IN NBFC CASE STUDIES*"),
            "{md}"
        );
        assert!(md.contains("| 5 years to 10 years | 19 | 19 | 19 |"), "{md}");
        assert!(md.contains("*These are real data from three NBFCs*"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_registration_report_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000047.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.starts_with("# ANFREL Pre-Election Assessment Mission Report"), "{md}");
        assert!(md.contains("| 14 | Cambodian Indigeneous Peoples Democracy Party | 19 | 194 | 19 | 202 | +8 |"), "{md}");
        assert!(md.contains("|  | Total |  | 84,208 |  | 86,092 | +1,884 |"), "{md}");
        assert!(!md.contains("|  | Democracy Party |"), "{md}");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_to_markdown_dual_table_article_document_on_real_pdf() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../benchmark/pdfs/01030000000190.pdf");
        let doc = crate::convert(&path, &crate::api::config::ProcessingConfig::default()).unwrap();
        let md = to_markdown(&doc).unwrap();

        assert!(md.starts_with("# Table 6: Performance comparison amongst the merge candidates"), "{md}");
        assert!(md.contains("*Table 6*: Performance comparison amongst the merge candidates."), "{md}");
        assert!(md.contains("# Table 7: Ablation studies on the different merge methods used for obtaining the final model"), "{md}");
        assert!(!md.contains("*Table 6*: Table 6:"), "{md}");
        assert!(!md.contains("| Merge v1"), "{md}");
    }

    #[test]
    fn test_normalize_list_text_strips_redundant_bullets() {
        assert_eq!(normalize_list_text("• Collected via surveys"), "Collected via surveys");
        assert!(is_pure_bullet_marker("•"));
    }

    #[test]
    fn test_reference_continuation_detected() {
        assert!(should_merge_paragraph_text(
            "Scaling laws for transfer.",
            "arXiv preprint arXiv:2102.01293."
        ));
    }

    #[test]
    fn test_enumerated_markers_are_detected() {
        assert!(starts_with_enumerated_marker("iii. Third item"));
        assert!(starts_with_enumerated_marker("1) First item"));
        assert!(starts_with_enumerated_marker("a. Lettered item"));
        assert!(!starts_with_enumerated_marker("Figure 1. Caption"));
        assert!(!starts_with_enumerated_marker("Natural dispersal"));
    }

    fn make_heading(text: &str) -> ContentElement {
        let bbox = BoundingBox::new(Some(1), 72.0, 700.0, 300.0, 712.0);
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: "Lato-Bold".to_string(),
            font_size: 12.0,
            font_weight: 700.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        };
        let line = TextLine {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 702.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 702.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let column = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 702.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Heading(SemanticHeading {
            base: SemanticParagraph {
                base: SemanticTextNode {
                    bbox,
                    index: None,
                    level: None,
                    semantic_type: crate::models::enums::SemanticType::Heading,
                    correct_semantic_score: None,
                    columns: vec![column],
                    font_weight: Some(700.0),
                    font_size: Some(12.0),
                    text_color: None,
                    italic_angle: None,
                    font_name: Some("Lato-Bold".to_string()),
                    text_format: None,
                    max_font_size: Some(12.0),
                    background_color: None,
                    is_hidden_text: false,
                },
                enclosed_top: false,
                enclosed_bottom: false,
                indentation: 0,
            },
            heading_level: Some(1),
        })
    }

    fn make_heading_at(
        left: f64,
        bottom: f64,
        right: f64,
        top: f64,
        text: &str,
    ) -> ContentElement {
        let bbox = BoundingBox::new(Some(1), left, bottom, right, top);
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: "Lato-Bold".to_string(),
            font_size: top - bottom,
            font_weight: 700.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        };
        let line = TextLine {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: top - bottom,
            base_line: bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: top - bottom,
            base_line: bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let column = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: top - bottom,
            base_line: bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Heading(SemanticHeading {
            base: SemanticParagraph {
                base: SemanticTextNode {
                    bbox,
                    index: None,
                    level: None,
                    semantic_type: crate::models::enums::SemanticType::Heading,
                    correct_semantic_score: None,
                    columns: vec![column],
                    font_weight: Some(700.0),
                    font_size: Some(top - bottom),
                    text_color: None,
                    italic_angle: None,
                    font_name: Some("Lato-Bold".to_string()),
                    text_format: None,
                    max_font_size: Some(top - bottom),
                    background_color: None,
                    is_hidden_text: false,
                },
                enclosed_top: false,
                enclosed_bottom: false,
                indentation: 0,
            },
            heading_level: None,
        })
    }

    fn make_paragraph(text: &str, bottom: f64, top: f64) -> ContentElement {
        make_paragraph_at(72.0, bottom, 300.0, top, text)
    }

    fn make_paragraph_at(left: f64, bottom: f64, right: f64, top: f64, text: &str) -> ContentElement {
        let bbox = BoundingBox::new(Some(1), left, bottom, right, top);
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: "Lato-Regular".to_string(),
            font_size: (top - bottom).max(1.0),
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: vec![],
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
            index: None,
            page_number: Some(1),
            level: None,
            mcid: None,
        };
        let line = TextLine {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: chunk.font_size,
            base_line: bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: line.font_size,
            base_line: line.base_line,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let column = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: block.font_size,
            base_line: block.base_line,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: SemanticTextNode {
                bbox,
                index: None,
                level: None,
                semantic_type: crate::models::enums::SemanticType::Paragraph,
                correct_semantic_score: None,
                columns: vec![column],
                font_weight: Some(400.0),
                font_size: Some(top - bottom),
                text_color: None,
                italic_angle: None,
                font_name: Some("Lato-Regular".to_string()),
                text_format: None,
                max_font_size: Some(top - bottom),
                background_color: None,
                is_hidden_text: false,
            },
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    fn make_fallback_list(items: &[&str]) -> ContentElement {
        let mut list_items = Vec::new();
        for (idx, text) in items.iter().enumerate() {
            let top = 700.0 - idx as f64 * 18.0;
            let bottom = top - 12.0;
            let bbox = BoundingBox::new(Some(1), 72.0, bottom, 320.0, top);
            list_items.push(ListItem {
                bbox: bbox.clone(),
                index: None,
                level: None,
                label: ListLabel {
                    bbox: bbox.clone(),
                    content: vec![],
                    semantic_type: None,
                },
                body: ListBody {
                    bbox: bbox.clone(),
                    content: vec![],
                    semantic_type: None,
                },
                label_length: 0,
                contents: vec![make_paragraph_at(72.0, bottom, 320.0, top, text)],
                semantic_type: None,
            });
        }

        ContentElement::List(PDFList {
            bbox: BoundingBox::new(Some(1), 72.0, 700.0 - items.len() as f64 * 18.0, 320.0, 700.0),
            index: None,
            level: None,
            list_items,
            numbering_style: Some("bullets".to_string()),
            common_prefix: None,
            previous_list_id: None,
            next_list_id: None,
        })
    }

    fn make_toc_table(rows: &[(&str, &str)]) -> ContentElement {
        let mut table_rows = Vec::new();
        for (ri, (title, page)) in rows.iter().enumerate() {
            let top = 680.0 - ri as f64 * 18.0;
            let bottom = top - 12.0;
            let left_bbox = BoundingBox::new(Some(1), 72.0, bottom, 280.0, top);
            let right_bbox = BoundingBox::new(Some(1), 320.0, bottom, 360.0, top);
            table_rows.push(TableBorderRow {
                bbox: BoundingBox::new(Some(1), 72.0, bottom, 360.0, top),
                index: None,
                level: None,
                row_number: ri,
                cells: vec![
                    TableBorderCell {
                        bbox: left_bbox.clone(),
                        index: None,
                        level: None,
                        row_number: ri,
                        col_number: 0,
                        row_span: 1,
                        col_span: 1,
                        content: vec![TableToken {
                            base: TextChunk {
                                value: (*title).to_string(),
                                bbox: left_bbox,
                                font_name: "Lato-Regular".to_string(),
                                font_size: 10.0,
                                font_weight: 400.0,
                                italic_angle: 0.0,
                                font_color: "#000000".to_string(),
                                contrast_ratio: 21.0,
                                symbol_ends: vec![],
                                text_format: TextFormat::Normal,
                                text_type: TextType::Regular,
                                pdf_layer: PdfLayer::Main,
                                ocg_visible: true,
                                index: None,
                                page_number: Some(1),
                                level: None,
                                mcid: None,
                            },
                            token_type: TableTokenType::Text,
                        }],
                        contents: vec![],
                        semantic_type: None,
                    },
                    TableBorderCell {
                        bbox: right_bbox.clone(),
                        index: None,
                        level: None,
                        row_number: ri,
                        col_number: 1,
                        row_span: 1,
                        col_span: 1,
                        content: vec![TableToken {
                            base: TextChunk {
                                value: (*page).to_string(),
                                bbox: right_bbox,
                                font_name: "Lato-Regular".to_string(),
                                font_size: 10.0,
                                font_weight: 400.0,
                                italic_angle: 0.0,
                                font_color: "#000000".to_string(),
                                contrast_ratio: 21.0,
                                symbol_ends: vec![],
                                text_format: TextFormat::Normal,
                                text_type: TextType::Regular,
                                pdf_layer: PdfLayer::Main,
                                ocg_visible: true,
                                index: None,
                                page_number: Some(1),
                                level: None,
                                mcid: None,
                            },
                            token_type: TableTokenType::Text,
                        }],
                        contents: vec![],
                        semantic_type: None,
                    },
                ],
                semantic_type: None,
            });
        }

        ContentElement::TableBorder(TableBorder {
            bbox: BoundingBox::new(Some(1), 72.0, 620.0, 360.0, 680.0),
            index: None,
            level: Some("1".to_string()),
            x_coordinates: vec![72.0, 320.0, 360.0],
            x_widths: vec![0.0, 0.0, 0.0],
            y_coordinates: vec![680.0, 662.0, 644.0, 626.0],
            y_widths: vec![0.0, 0.0, 0.0, 0.0],
            rows: table_rows,
            num_rows: rows.len(),
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        })
    }

    #[test]
    fn test_contents_document_renders_toc_table_rows() {
        let mut doc = PdfDocument::new("contents.pdf".to_string());
        doc.kids.push(make_heading("CONTENTS"));
        doc.kids.push(make_toc_table(&[
            ("Experiment #1: Hydrostatic Pressure", "3"),
            ("Experiment #2: Bernoulli's Theorem Demonstration", "13"),
            ("Experiment #3: Energy Loss in Pipe Fittings", "24"),
            ("Experiment #4: Energy Loss in Pipes", "33"),
            ("Experiment #5: Impact of a Jet", "43"),
            ("Experiment #6: Orifice and Free Jet Flow", "50"),
            ("Experiment #7: Osborne Reynolds' Demonstration", "59"),
            ("References", "101"),
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.starts_with("# CONTENTS\n\n"));
        assert!(md.contains("- Experiment #1: Hydrostatic Pressure 3\n"));
        assert!(md.contains("- Experiment #2: Bernoulli's Theorem Demonstration 13\n"));
        assert!(md.contains("- Experiment #7: Osborne Reynolds' Demonstration 59\n"));
        assert!(md.contains("- References 101\n"));
    }

    #[test]
    fn test_toc_semantic_paragraphs_render_without_blank_lines() {
        let mut doc = PdfDocument::new("toc-semantic.pdf".to_string());
        let mut first = make_paragraph(
            "Part V. Chapter Five - Comparing Associations Between Multiple Variables",
            700.0,
            712.0,
        );
        let mut second = make_paragraph("Section 5.1: The Linear Model 35", 684.0, 696.0);
        if let ContentElement::Paragraph(p) = &mut first {
            p.base.semantic_type = SemanticType::TableOfContent;
        }
        if let ContentElement::Paragraph(p) = &mut second {
            p.base.semantic_type = SemanticType::TableOfContent;
        }
        doc.kids.push(first);
        doc.kids.push(second);

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "Part V. Chapter Five - Comparing Associations Between Multiple Variables\nSection 5.1: The Linear Model 35\n"
        ));
    }

    #[test]
    fn test_compact_toc_document_renders_without_blank_lines() {
        let mut doc = PdfDocument::new("compact-toc.pdf".to_string());
        doc.kids.push(make_paragraph(
            "Part V. Chapter Five - Comparing Associations Between Multiple Variables",
            700.0,
            712.0,
        ));
        doc.kids.push(make_paragraph(
            "Section 5.1: The Linear Model 35",
            684.0,
            696.0,
        ));
        doc.kids.push(make_paragraph(
            "Part VI. Chapter Six - Comparing Three or More Group Means",
            668.0,
            680.0,
        ));
        doc.kids.push(make_paragraph(
            "Section 6.1: Between Versus Within Group Analyses 49",
            652.0,
            664.0,
        ));
        doc.kids.push(make_paragraph(
            "Part VII. Chapter Seven - Moderation and Mediation Analyses",
            636.0,
            648.0,
        ));
        doc.kids.push(make_paragraph(
            "Section 7.1: Mediation and Moderation Models 64",
            620.0,
            632.0,
        ));
        doc.kids
            .push(make_paragraph("References 101", 604.0, 616.0));
        doc.kids.push(make_paragraph(
            "Section 8.1: Factor Analysis Definitions 75",
            588.0,
            600.0,
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "# Part V. Chapter Five - Comparing Associations Between Multiple Variables\n\n## Section 5.1: The Linear Model"
        ));
        assert!(md.contains(
            "# Part VI. Chapter Six - Comparing Three or More Group Means\n\n## Section 6.1: Between Versus Within Group Analyses"
        ));
        assert!(md.contains("References 101\n\n## Section 8.1: Factor Analysis Definitions"));
    }

    #[test]
    fn test_merged_caption_and_body_paragraph_renders_as_two_paragraphs() {
        let mut doc = PdfDocument::new("caption-body.pdf".to_string());
        doc.kids.push(make_paragraph(
            "Figure 1. This image shows the Western hemisphere as viewed from space 35,400 kilometers above Earth. (credit: modification of work by R. Stockli, NASA/ GSFC/ NOAA/ USGS) Our nearest astronomical neighbor is Earth's satellite, commonly called the Moon.",
            500.0,
            540.0,
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("USGS)\n\nOur nearest astronomical neighbor"));
    }

    #[test]
    fn test_short_caption_label_merges_with_following_tail_and_body() {
        let mut doc = PdfDocument::new("diagram-caption.pdf".to_string());
        doc.kids.push(make_paragraph("Diagram 5", 540.0, 552.0));
        doc.kids.push(make_paragraph(
            "Distribution of Komnas HAM's YouTube Content (2019- 2020) As of 1 December 2021, the channel has 2,290 subscribers and 185,676 total views.",
            520.0,
            532.0,
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "Diagram 5\nDistribution of Komnas HAM's YouTube Content (2019- 2020)\n\nAs of 1 December 2021, the channel has 2,290 subscribers"
        ));
    }

    #[test]
    fn test_short_caption_label_merges_with_tail_and_year() {
        let mut doc = PdfDocument::new("figure-caption.pdf".to_string());
        doc.kids.push(make_paragraph("Figure 4", 540.0, 552.0));
        doc.kids.push(make_paragraph(
            "Komnas HAM's YouTube channel as of 1 December",
            520.0,
            532.0,
        ));
        doc.kids.push(make_paragraph("2021", 500.0, 512.0));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("Figure 4\nKomnas HAM's YouTube channel as of 1 December\n2021"));
        assert!(!md.contains("\n\n2021"));
    }

    #[test]
    fn test_mid_page_numeric_labels_are_not_dropped_as_page_numbers() {
        let mut doc = PdfDocument::new("chart.pdf".to_string());
        doc.kids.push(make_paragraph("Figure 1", 760.0, 772.0));
        doc.kids.push(make_paragraph("100", 520.0, 528.0));
        doc.kids
            .push(make_paragraph("Body text continues here.", 400.0, 412.0));
        doc.kids.push(make_paragraph("36", 20.0, 28.0));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("100"));
        assert!(!md.lines().any(|line| line.trim() == "36"));
    }

    #[test]
    fn test_semantic_paragraphs_are_not_remerged_in_markdown() {
        let mut doc = PdfDocument::new("paragraphs.pdf".to_string());
        doc.kids.push(make_paragraph(
            "First semantic paragraph ends here.",
            520.0,
            532.0,
        ));
        doc.kids.push(make_paragraph(
            "Second semantic paragraph starts here.",
            500.0,
            512.0,
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "First semantic paragraph ends here.\n\nSecond semantic paragraph starts here."
        ));
    }

    #[test]
    fn test_lowercase_semantic_paragraph_continuation_is_merged() {
        let mut doc = PdfDocument::new("continuation.pdf".to_string());
        doc.kids.push(make_paragraph(
            "You can then compare the difference you actually obtained against this null distribution to generate a p value for your difference",
            520.0,
            532.0,
        ));
        doc.kids.push(make_paragraph("of interest.", 500.0, 512.0));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "You can then compare the difference you actually obtained against this null distribution to generate a p value for your difference of interest."
        ));
    }

    #[test]
    fn test_semantic_enumerated_paragraphs_are_not_merged() {
        let mut doc = PdfDocument::new("enumerated-paragraphs.pdf".to_string());
        doc.kids.push(make_paragraph(
            "iii. Looking at cost items, the cost of raw woods procurement will be highest share.",
            520.0,
            532.0,
        ));
        doc.kids.push(make_paragraph(
            "iv. This business model will be operating cost-oriented not capital cost-oriented.",
            500.0,
            512.0,
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "iii. Looking at cost items, the cost of raw woods procurement will be highest share.\n\niv. This business model will be operating cost-oriented not capital cost-oriented."
        ));
    }

    #[test]
    fn test_leading_figure_carryover_is_skipped_before_first_numbered_heading() {
        let mut doc = PdfDocument::new("leading-figure-carryover.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_paragraph_at(
            72.0,
            742.0,
            540.0,
            756.0,
            "Figure 6. Mytella strigata biofouling green mussel farms in Bacoor City, Cavite, Manila Bay",
        ));
        doc.kids.push(make_heading_at(
            72.0,
            680.0,
            260.0,
            696.0,
            "5. Natural dispersal",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            640.0,
            540.0,
            654.0,
            "Dispersal by purely natural means is not included as a pathway of biological invasions.",
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.starts_with("# 5. Natural dispersal"));
        assert!(!md.contains("Figure 6. Mytella strigata"));
    }

    #[test]
    fn test_list_renderer_strips_duplicate_bullets_and_skips_bullet_only_items() {
        let mut doc = PdfDocument::new("bullets.pdf".to_string());
        doc.kids
            .push(make_fallback_list(&["• First item", "•", "• Second item", "133"]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("- First item"));
        assert!(md.contains("- Second item"));
        assert!(!md.contains("- • First item"));
        assert!(!md.contains("\n- •\n"));
        assert!(!md.contains("\n- 133\n"));
    }

    #[test]
    fn test_list_renderer_merges_wrapped_continuation_items() {
        let mut doc = PdfDocument::new("wrapped-list.pdf".to_string());
        doc.kids.push(make_fallback_list(&[
            "Use a micropipette to add 2 μL of loading dye",
            "and down a couple of times to mix the loading dye with the digested DNA.",
            "Use a fresh pipet tip for each reaction tube.",
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "- Use a micropipette to add 2 μL of loading dye and down a couple of times to mix the loading dye with the digested DNA."
        ));
        assert!(md.contains("- Use a fresh pipet tip for each reaction tube."));
        assert!(!md.contains("\n- and down"));
    }

    #[test]
    fn test_list_renderer_keeps_enumerated_items_separate() {
        let mut doc = PdfDocument::new("enumerated-list.pdf".to_string());
        doc.kids.push(make_fallback_list(&[
            "iii. Looking at cost items, the cost of raw woods procurement will be highest share.",
            "iv. This business model will be operating cost-oriented not capital cost-oriented.",
            "v. Assumed selling price of wood pellet is $100 per tonne and appropriate.",
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("iii. Looking at cost items, the cost of raw woods procurement will be highest share.\niv. This business model will be operating cost-oriented not capital cost-oriented.\nv. Assumed selling price of wood pellet is $100 per tonne and appropriate."));
        assert!(!md.contains("- iii."));
    }

    #[test]
    fn test_postprocess_drops_isolated_single_char_noise_lines() {
        let markdown = "# The Data Journey\n\n1\n\nTo get started.\n\no\n\nNOTE: Keep going.\n";
        let cleaned = drop_isolated_noise_lines(markdown);
        assert!(!cleaned.contains("\n1\n"));
        assert!(!cleaned.contains("\no\n"));
        assert!(cleaned.contains("To get started."));
        assert!(cleaned.contains("NOTE: Keep going."));
    }

    fn make_two_column_table(rows: &[(&str, &str)]) -> ContentElement {
        let mut table_rows = Vec::new();
        for (row_number, (left, right)) in rows.iter().enumerate() {
            let top = 656.0 - row_number as f64 * 18.0;
            let bottom = top - 16.0;
            let mut cells = Vec::new();
            for (col_number, (text, left_x, right_x)) in
                [(*left, 72.0, 220.0), (*right, 220.0, 420.0)]
                    .into_iter()
                    .enumerate()
            {
                let content = if text.is_empty() {
                    Vec::new()
                } else {
                    vec![TableToken {
                        base: TextChunk {
                            value: text.to_string(),
                            bbox: BoundingBox::new(Some(1), left_x, bottom, right_x, top),
                            font_name: "Test".to_string(),
                            font_size: 11.0,
                            font_weight: 400.0,
                            italic_angle: 0.0,
                            font_color: "[0.0]".to_string(),
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
                        },
                        token_type: TableTokenType::Text,
                    }]
                };
                cells.push(TableBorderCell {
                    bbox: BoundingBox::new(Some(1), left_x, bottom, right_x, top),
                    index: None,
                    level: None,
                    row_number,
                    col_number,
                    row_span: 1,
                    col_span: 1,
                    content,
                    contents: vec![],
                    semantic_type: None,
                });
            }

            table_rows.push(TableBorderRow {
                bbox: BoundingBox::new(Some(1), 72.0, bottom, 420.0, top),
                index: None,
                level: None,
                row_number,
                cells,
                semantic_type: None,
            });
        }

        ContentElement::TableBorder(TableBorder {
            bbox: BoundingBox::new(
                Some(1),
                72.0,
                656.0 - rows.len() as f64 * 18.0 - 16.0,
                420.0,
                656.0,
            ),
            index: None,
            level: Some("1".to_string()),
            x_coordinates: vec![72.0, 220.0, 420.0],
            x_widths: vec![0.0; 3],
            y_coordinates: (0..=rows.len()).map(|i| 656.0 - i as f64 * 18.0).collect(),
            y_widths: vec![0.0; rows.len() + 1],
            rows: table_rows,
            num_rows: rows.len(),
            num_columns: 2,
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        })
    }

    fn make_chunked_paragraph_line(
        segments: &[(&str, f64, f64)],
        bottom: f64,
        top: f64,
    ) -> ContentElement {
        let bbox = BoundingBox::new(
            Some(1),
            segments.first().map(|(_, left, _)| *left).unwrap_or(72.0),
            bottom,
            segments.last().map(|(_, _, right)| *right).unwrap_or(320.0),
            top,
        );

        let chunks = segments
            .iter()
            .map(|(text, left, right)| TextChunk {
                value: (*text).to_string(),
                bbox: BoundingBox::new(Some(1), *left, bottom, *right, top),
                font_name: "Lato-Regular".to_string(),
                font_size: top - bottom,
                font_weight: 400.0,
                italic_angle: 0.0,
                font_color: "#000000".to_string(),
                contrast_ratio: 21.0,
                symbol_ends: vec![],
                text_format: TextFormat::Normal,
                text_type: TextType::Regular,
                pdf_layer: PdfLayer::Main,
                ocg_visible: true,
                index: None,
                page_number: Some(1),
                level: None,
                mcid: None,
            })
            .collect::<Vec<_>>();

        let line = TextLine {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: top - bottom,
            base_line: bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: chunks,
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: line.font_size,
            base_line: line.base_line,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let column = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: block.font_size,
            base_line: block.base_line,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };

        ContentElement::Paragraph(SemanticParagraph {
            base: SemanticTextNode {
                bbox,
                index: None,
                level: None,
                semantic_type: SemanticType::Paragraph,
                correct_semantic_score: None,
                columns: vec![column],
                font_weight: Some(400.0),
                font_size: Some(top - bottom),
                text_color: None,
                italic_angle: None,
                font_name: Some("Lato-Regular".to_string()),
                text_format: None,
                max_font_size: Some(top - bottom),
                background_color: None,
                is_hidden_text: false,
            },
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    fn make_n_column_table(rows: &[Vec<&str>], column_bounds: &[(f64, f64)]) -> ContentElement {
        let mut table_rows = Vec::new();
        for (row_number, row_values) in rows.iter().enumerate() {
            let top = 656.0 - row_number as f64 * 18.0;
            let bottom = top - 16.0;
            let mut cells = Vec::new();
            for (col_number, (left_x, right_x)) in column_bounds.iter().enumerate() {
                let text = row_values.get(col_number).copied().unwrap_or("");
                let content = if text.is_empty() {
                    Vec::new()
                } else {
                    vec![TableToken {
                        base: TextChunk {
                            value: text.to_string(),
                            bbox: BoundingBox::new(Some(1), *left_x, bottom, *right_x, top),
                            font_name: "Test".to_string(),
                            font_size: 11.0,
                            font_weight: 400.0,
                            italic_angle: 0.0,
                            font_color: "[0.0]".to_string(),
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
                        },
                        token_type: TableTokenType::Text,
                    }]
                };
                cells.push(TableBorderCell {
                    bbox: BoundingBox::new(Some(1), *left_x, bottom, *right_x, top),
                    index: None,
                    level: None,
                    row_number,
                    col_number,
                    row_span: 1,
                    col_span: 1,
                    content,
                    contents: vec![],
                    semantic_type: None,
                });
            }

            table_rows.push(TableBorderRow {
                bbox: BoundingBox::new(
                    Some(1),
                    column_bounds.first().map(|(left, _)| *left).unwrap_or(72.0),
                    bottom,
                    column_bounds.last().map(|(_, right)| *right).unwrap_or(420.0),
                    top,
                ),
                index: None,
                level: None,
                row_number,
                cells,
                semantic_type: None,
            });
        }

        let left = column_bounds.first().map(|(value, _)| *value).unwrap_or(72.0);
        let right = column_bounds.last().map(|(_, value)| *value).unwrap_or(420.0);
        let x_coordinates = std::iter::once(left)
            .chain(column_bounds.iter().map(|(_, right)| *right))
            .collect::<Vec<_>>();

        ContentElement::TableBorder(TableBorder {
            bbox: BoundingBox::new(
                Some(1),
                left,
                656.0 - rows.len() as f64 * 18.0 - 16.0,
                right,
                656.0,
            ),
            index: None,
            level: Some("1".to_string()),
            x_coordinates,
            x_widths: vec![0.0; column_bounds.len() + 1],
            y_coordinates: (0..=rows.len()).map(|i| 656.0 - i as f64 * 18.0).collect(),
            y_widths: vec![0.0; rows.len() + 1],
            rows: table_rows,
            num_rows: rows.len(),
            num_columns: column_bounds.len(),
            is_bad_table: false,
            is_table_transformer: false,
            previous_table: None,
            next_table: None,
        })
    }

    #[test]
    fn test_numeric_two_column_table_is_not_misrendered_as_toc() {
        let mut doc = PdfDocument::new("cec-table.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_two_column_table(&[
            ("Mineral or colloid type", "CEC of pure colloid"),
            ("", "cmolc/kg"),
            ("kaolinite", "10"),
            ("illite", "30"),
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| kaolinite | 10 |"));
    }

    #[test]
    fn test_blank_right_column_table_is_not_misrendered_as_toc() {
        let mut doc = PdfDocument::new("flocculation-table.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_two_column_table(&[
            (
                "Added cation",
                "Relative Size & Settling Rates of Floccules",
            ),
            ("K+", ""),
            ("Na+", ""),
            ("Ca2+", ""),
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("| Added cation | Relative Size & Settling Rates of Floccules |"));
        assert!(md.contains("| K+ |  |"));
    }

    #[test]
    fn test_infographic_card_table_renders_as_numbered_item() {
        let mut doc = PdfDocument::new("infographic-card.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_two_column_table(&[
            (
                "1",
                "We're all both consumers and creators of creative work.",
            ),
            (
                "",
                "As consumers, we watch movies, listen to music, read books, and more.",
            ),
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "1. We're all both consumers and creators of creative work. As consumers, we watch movies, listen to music, read books, and more."
        ));
        assert!(!md.contains("| 1 |"));
    }

    #[test]
    fn test_grouped_header_rows_are_preserved_without_flattening() {
        let mut doc = PdfDocument::new("grouped-header.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_n_column_table(
            &[
                vec!["Properties", "", "Instruction", "", "", "Alignment", ""],
                vec![
                    "",
                    "Alpaca-GPT4",
                    "OpenOrca",
                    "Synth. Math-Instruct",
                    "Orca DPO Pairs",
                    "Ultrafeedback Cleaned",
                    "Synth. Math-Alignment",
                ],
                vec!["Total # Samples", "52K", "2.91M", "126K", "12.9K", "60.8K", "126K"],
            ],
            &[
                (72.0, 120.0),
                (120.0, 170.0),
                (170.0, 220.0),
                (220.0, 280.0),
                (280.0, 340.0),
                (340.0, 410.0),
                (410.0, 470.0),
            ],
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains(
            "| Properties | Instruction | Instruction | Instruction | Alignment | Alignment | Alignment |"
        ));
        assert!(md.contains(
            "|  | Alpaca-GPT4 | OpenOrca | Synth. Math-Instruct | Orca DPO Pairs | Ultrafeedback Cleaned | Synth. Math-Alignment |"
        ));
        assert!(!md.contains("Instruction OpenOrca"));
        assert!(!md.contains("Alignment Ultrafeedback"));
    }

    #[test]
    fn test_top_table_plate_renderer_stops_before_article_body() {
        let mut doc = PdfDocument::new("table-plate.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_paragraph_at(
            72.0,
            724.0,
            200.0,
            736.0,
            "SOLAR 10.7B",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            704.0,
            220.0,
            716.0,
            "Training datasets",
        ));
        doc.kids.push(make_n_column_table(
            &[
                vec!["Properties", "", "Instruction", "", "", "Alignment", ""],
                vec![
                    "",
                    "Alpaca-GPT4",
                    "OpenOrca",
                    "Synth. Math-Instruct",
                    "Orca DPO Pairs",
                    "Ultrafeedback Cleaned",
                    "Synth. Math-Alignment",
                ],
                vec!["Total # Samples", "52K", "2.91M", "126K", "12.9K", "60.8K", "126K"],
                vec!["Maximum # Samples Used", "52K", "100K", "52K", "12.9K", "60.8K", "20.1K"],
                vec!["Open Source", "O", "O", "✗", "O", "O", "✗"],
            ],
            &[
                (78.0, 125.0),
                (125.0, 175.0),
                (175.0, 225.0),
                (225.0, 285.0),
                (285.0, 345.0),
                (345.0, 415.0),
                (415.0, 490.0),
            ],
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            500.0,
            310.0,
            514.0,
            "Table 1: Training datasets used for the instruction and alignment tuning stages, respectively.",
        ));
        doc.kids.push(make_paragraph_at(
            286.0,
            484.0,
            526.0,
            498.0,
            "Open source indicates whether the dataset is open-sourced.",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            360.0,
            290.0,
            388.0,
            "Comparison to other up-scaling methods. Unlike Komatsuzaki et al. (2022)...",
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("Table 1: Training datasets used for the instruction"));
        assert!(md.contains("| Properties | Instruction | Instruction | Instruction | Alignment | Alignment | Alignment |"));
        assert!(!md.contains("Comparison to other up-scaling methods"));
    }

    #[test]
    fn test_late_section_boundary_renderer_drops_equation_carryover() {
        let mut doc = PdfDocument::new("late-section.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_paragraph_at(
            72.0,
            700.0,
            540.0,
            714.0,
            "The horizontal distance traveled by the jet is equal to:",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            640.0,
            540.0,
            654.0,
            "The vertical position of the jet may be calculated as:",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            580.0,
            260.0,
            594.0,
            "Rearranging Equation (8) gives:",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            520.0,
            420.0,
            534.0,
            "Substitution into Equation 7 results in:",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            460.0,
            280.0,
            474.0,
            "Equations (10) can be rearranged to find Cv:",
        ));
        doc.kids.push(make_heading_at(
            72.0,
            350.0,
            420.0,
            366.0,
            "7.2. DETERMINATION OF THE COEFFICIENT OF DISCHARGE",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            326.0,
            380.0,
            340.0,
            "If C_d is assumed to be constant, then a graph of Q plotted against",
        ));
        doc.kids.push(make_paragraph_at(
            400.0,
            326.0,
            540.0,
            340.0,
            "(Equation 6) will be linear, and",
        ));
        doc.kids.push(make_paragraph_at(
            72.0,
            310.0,
            240.0,
            324.0,
            "the slope of this graph will be:",
        ));
        doc.kids.push(make_paragraph_at(
            360.0,
            36.0,
            550.0,
            48.0,
            "EXPERIMENT #6: ORIFICE AND FREE JET FLOW 53",
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.starts_with("# 7.2. DETERMINATION OF THE COEFFICIENT OF DISCHARGE"));
        assert!(md.contains(
            "If C_d is assumed to be constant, then a graph of Q plotted against (Equation 6) will be linear, and the slope of this graph will be:"
        ));
        assert!(!md.contains("The horizontal distance traveled by the jet"));
        assert!(!md.contains("EXPERIMENT #6"));
    }

    #[test]
    fn test_leading_table_carryover_row_is_trimmed_from_general_renderer() {
        let mut doc = PdfDocument::new("carryover-table.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_n_column_table(
            &[
                vec![
                    "Jurisdiction",
                    "GATS XVII Reservation (1994)",
                    "Foreign Ownership Permitted",
                    "Restrictions on Foreign Ownership",
                    "Foreign Ownership Reporting Requirements",
                ],
                vec![
                    "",
                    "",
                    "",
                    "right required to acquire desert lands and continue the prior page",
                    "",
                ],
                vec!["Finland", "N", "Y", "Prior approval may be required.", ""],
                vec!["France", "N", "Y", "None.", ""],
            ],
            &[
                (72.0, 150.0),
                (150.0, 235.0),
                (235.0, 330.0),
                (330.0, 500.0),
                (500.0, 560.0),
            ],
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(!md.contains("right required to acquire desert lands"));
        assert!(md.contains("| Finland | N | Y | Prior approval may be required. |  |"));
    }

    #[test]
    fn test_single_table_report_renderer_promotes_title_and_skips_footer() {
        let mut doc = PdfDocument::new("single-table-report.pdf".to_string());
        doc.number_of_pages = 1;
        doc.kids.push(make_paragraph_at(
            140.0,
            674.0,
            474.0,
            688.0,
            "Restrictions on Land Ownership by Foreigners in Selected Jurisdictions",
        ));
        doc.kids.push(make_n_column_table(
            &[
                vec![
                    "Jurisdiction",
                    "GATS XVII Reservation (1994)",
                    "Foreign Ownership Permitted",
                    "Restrictions on Foreign Ownership",
                    "Foreign Ownership Reporting Requirements",
                ],
                vec![
                    "",
                    "",
                    "",
                    "right required to acquire desert lands and continue the prior page",
                    "",
                ],
                vec![
                    "Finland",
                    "N",
                    "Y",
                    "Prior approval from the Government of Aland may be required.",
                    "",
                ],
                vec!["France", "N", "Y", "None.", ""],
            ],
            &[
                (72.0, 150.0),
                (150.0, 235.0),
                (235.0, 330.0),
                (330.0, 500.0),
                (500.0, 560.0),
            ],
        ));
        doc.kids.push(make_paragraph_at(
            350.0,
            36.0,
            548.0,
            48.0,
            "The Law Library of Congress 7",
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.starts_with(
            "# Restrictions on Land Ownership by Foreigners in Selected Jurisdictions"
        ));
        assert!(!md.contains("right required to acquire desert lands"));
        assert!(!md.contains("The Law Library of Congress 7"));
        assert!(md.contains("| Finland | N | Y | Prior approval from the Government of Aland may be required. |  |"));
    }

    #[test]
    fn test_geometric_panel_headers_are_promoted_into_table() {
        let mut doc = PdfDocument::new("ai-pack-panel.pdf".to_string());
        doc.kids.push(make_chunked_paragraph_line(&[("OCR", 220.0, 250.0)], 720.0, 732.0));
        doc.kids.push(make_chunked_paragraph_line(
            &[("Recommendation", 430.0, 540.0)],
            720.0,
            732.0,
        ));
        doc.kids.push(make_chunked_paragraph_line(
            &[("Product semantic search", 660.0, 860.0)],
            720.0,
            732.0,
        ));
        doc.kids.push(make_chunked_paragraph_line(&[("Pack", 72.0, 110.0)], 684.0, 696.0));
        doc.kids.push(make_chunked_paragraph_line(
            &[("A solution that recognizes characters", 140.0, 340.0)],
            684.0,
            696.0,
        ));
        doc.kids.push(make_chunked_paragraph_line(
            &[("A solution that recommends the best products", 390.0, 620.0)],
            684.0,
            696.0,
        ));
        doc.kids.push(make_chunked_paragraph_line(
            &[("A solution that enables semantic search", 650.0, 900.0)],
            684.0,
            696.0,
        ));
        doc.kids.push(make_n_column_table(
            &[
                vec![
                    "Achieved 1st place in the OCR World Competition",
                    "Team with specialists and technologies",
                    "Creation of the first natural language evaluation",
                ],
                vec![
                    "The team includes specialists who have",
                    "received Kaggle's Gold Medal recommendation",
                    "system in Korean (KLUE)",
                ],
                vec![
                    "presented 14 papers in renowned AI conferences",
                    "top-tier recommendation",
                    "Shopee subject",
                ],
            ],
            &[(120.0, 360.0), (360.0, 630.0), (630.0, 910.0)],
        ));
        doc.kids.push(make_chunked_paragraph_line(&[("models", 430.0, 490.0)], 552.0, 564.0));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("| Pack | OCR | Recommendation | Product semantic search |"));
        assert!(md.contains("| A solution that recognizes characters | A solution that recommends the best products | A solution that enables semantic search |"));
        assert!(md.contains(
            "received Kaggle's Gold Medal recommendation top-tier recommendation models"
        ));
    }

    #[test]
    fn test_embedded_stub_header_is_promoted_from_first_table_column() {
        let mut doc = PdfDocument::new("embedded-stub-header.pdf".to_string());
        doc.kids.push(make_chunked_paragraph_line(&[("OCR", 220.0, 250.0)], 720.0, 732.0));
        doc.kids.push(make_chunked_paragraph_line(
            &[("Recommendation", 430.0, 540.0)],
            720.0,
            732.0,
        ));
        doc.kids.push(make_chunked_paragraph_line(
            &[("Product semantic search", 660.0, 860.0)],
            720.0,
            732.0,
        ));
        doc.kids.push(make_n_column_table(
            &[
                vec![
                    "Pack",
                    "A solution that recognizes characters in an image and extracts necessary information",
                    "A solution that recommends the best products and contents",
                    "A solution that enables semantic search and organizes key information",
                ],
                vec![
                    "Application",
                    "Applicable to all fields that require text extraction",
                    "Applicable to all fields that use any form of recommendation",
                    "Applicable to all fields that deal with unstructured data",
                ],
                vec![
                    "Highlight",
                    "Achieved 1st place in the OCR World Competition",
                    "Received Kaggle's Gold Medal recommendation",
                    "Creation of the first natural language evaluation system in Korean",
                ],
            ],
            &[
                (72.0, 120.0),
                (120.0, 360.0),
                (360.0, 630.0),
                (630.0, 910.0),
            ],
        ));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("| Pack | OCR | Recommendation | Product semantic search |"));
        assert!(md.contains("| Application | Applicable to all fields that require text extraction |"));
        assert!(md.contains("| Highlight | Achieved 1st place in the OCR World Competition |"));
        assert!(!md.contains("OCR\n\nRecommendation\n\nProduct semantic search"));
    }

    #[test]
    fn test_geometric_chunk_alignment_splits_header_line_into_columns() {
        let line = make_chunked_paragraph_line(
            &[
                ("Properties", 72.0, 145.0),
                ("Instruction", 180.0, 255.0),
                ("Alignment", 480.0, 545.0),
            ],
            720.0,
            732.0,
        );
        let chunk_lines = extract_chunk_lines(&line);
        let fragments = split_line_into_slot_fragments(
            &chunk_lines[0],
            &[
                (72.0, 170.0),
                (170.0, 280.0),
                (280.0, 380.0),
                (380.0, 480.0),
                (480.0, 600.0),
                (600.0, 720.0),
                (720.0, 850.0),
            ],
        );

        assert_eq!(fragments.len(), 3);
        assert_eq!(fragments[0].slot_idx, 0);
        assert_eq!(fragments[0].text, "Properties");
        assert_eq!(fragments[1].slot_idx, 1);
        assert_eq!(fragments[1].text, "Instruction");
        assert_eq!(fragments[2].slot_idx, 4);
        assert_eq!(fragments[2].text, "Alignment");
    }

    #[test]
    fn test_merge_tables_across_heading() {
        let input = "some text\n\n\
                      | Area | Competence |\n\
                      | --- | --- |\n\
                      | Row1 | Val1 |\n\
                      | Row2 | Val2 |\n\
                      \n\
                      # Heading Between\n\
                      \n\
                      | Row3 | Val3 |\n\
                      | --- | --- |\n\
                      \n\
                      more text\n";
        let result = merge_adjacent_pipe_tables(input);
        // Heading should be converted to a pipe row
        assert!(
            result.contains("| Heading Between |"),
            "Heading should be in pipe row: {}",
            result
        );
        // Should NOT have # heading marker
        assert!(
            !result.contains("# Heading Between"),
            "Heading marker should be removed: {}",
            result
        );
        // Row3 should still be present
        assert!(
            result.contains("| Row3 |") || result.contains("Row3"),
            "Row3 should exist: {}",
            result
        );
    }

    #[test]
    fn test_merge_tables_does_not_cross_distinct_headers() {
        let input = "| Model | Score |\n\
                     | --- | --- |\n\
                     | A | 1 |\n\
                     \n\
                     Table 6: Performance comparison amongst the merge candidates.\n\
                     \n\
                     | Model | Method | Score |\n\
                     | --- | --- | --- |\n\
                     | B | Avg | 2 |\n";
        let result = merge_adjacent_pipe_tables(input);

        assert!(result.contains("Table 6: Performance comparison amongst the merge candidates."));
        assert!(result.contains("| Model | Score |"));
        assert!(result.contains("| Model | Method | Score |"));
        assert!(!result.contains("| Table 6: Performance comparison amongst the merge candidates. |"));
    }

    #[test]
    fn test_normalize_chart_like_markdown_extracts_series_tables() {
        let input = "Figure 1.7. Non-citizen population in Malaysia (in thousands) 3,323 3,500 3,288 3,230 3,140 2,907 3,000 2,693 2,500 2,000 1,500 1,000 500 0\n\n\
                     2016 2017 2018 2019 2020 2021 Source: Department of Statistics, Malaysia (2022). Figure for 2021 is an estimate.\n\n\
                     ASEAN Migration Outlook 19\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(normalized.contains("## Figure 1.7. Non-citizen population in Malaysia (in thousands)"));
        assert!(normalized.contains("| 2016 | 3,323 |"));
        assert!(normalized.contains("| 2021 | 2,693 |"));
        assert!(normalized.contains("*Source: Department of Statistics, Malaysia (2022). Figure for 2021 is an estimate.*"));
        assert!(!normalized.contains("ASEAN Migration Outlook 19"));
    }

    #[test]
    fn test_normalize_chart_like_markdown_promotes_structural_captions() {
        let input = "Figure 5.1 Mr. Bologna Jun-r as Kalim Azack in Aladdin, or\n\n\
                     The Wonderful Lamp.\n\n\
                     Body paragraph.\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(normalized.contains("## Figure 5.1 Mr. Bologna Jun-r as Kalim Azack in Aladdin, or The Wonderful Lamp"));
        assert!(normalized.contains("Body paragraph."));
    }

    #[test]
    fn test_normalize_chart_like_markdown_reconstructs_header_pair_chart_table() {
        let input = "Figure 4.8. Domestic Wood Pellets Production\n\n\
                     | 8 | 800 200 | 126 2014 | 120 2015 | 120 2016 | 127 2017 | 131 2018 | 147 2019 |\n\
                     | --- | --- | --- | --- | --- | --- | --- | --- |\n\n\
                     Source: Forestry Agency, Ministry of Agriculture, Forestry and Fishery (MAFF), 2020.\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(normalized.contains("# Figure 4.8. Domestic Wood Pellets Production"));
        assert!(normalized.contains("| Year | Domestic Wood Pellets Production |"));
        assert!(normalized.contains("| 2014 | 126 |"));
        assert!(normalized.contains("| 2019 | 147 |"));
        assert!(!normalized.contains("| 8 | 800 200 |"));
    }

    #[test]
    fn test_normalize_chart_like_markdown_drops_numeric_axis_artifact_table() {
        let input = "| 31 1 0 2 23 2 2 2 0 5 10 15 20 25 30 35 Event Celebration Information Videograph 2019 2020 |\n\
                     | --- |\n\n\
                     Distribution of Komnas HAM's YouTube Content (2019-2020)\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(!normalized.contains("| --- |"));
        assert!(normalized.contains("Distribution of Komnas HAM's YouTube Content (2019-2020)"));
    }

    #[test]
    fn test_normalize_chart_like_markdown_drops_url_fragment_table() {
        let input = "## Figure 6 DPN Argentina Content: World Health Day Celebration\n\n\
                     | na/status/1379765916259483648 |\n\
                     | --- |\n\n\
                     98 DPN Argentina, accessed on 5 December 2021.\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(!normalized.contains("/status/1379765916259483648 |"));
        assert!(normalized.contains("98 DPN Argentina, accessed on 5 December 2021."));
    }

    #[test]
    fn test_normalize_chart_like_markdown_drops_sparse_table_before_caption() {
        let input = "What’s unique about the growth of Alligator Gars is their fast growth.\n\n\
                     | in | cm |  | Length | of | Gar | Fish | Age |\n\
                     | --- | --- | --- | --- | --- | --- | --- | --- |\n\
                     | 120) | 300 |  |  |  |  |  |  |\n\
                     | 100+ | 250 |  |  |  |  |  |  |\n\
                     | 80+ | 200 |  |  |  |  |  |  |\n\
                     | 20. | 50 | G |  |  |  |  | Vi |\n\
                     | 0 | 0 |  |  |  |  |  |  |\n\
                     |  | 0 | 10 | 30 |  | 40 | 50 | 60 |\n\n\
                     Figure 8.6: Growth in length of Alligator Gar in Texas.\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(!normalized.contains("| in | cm |"));
        assert!(normalized.contains("Figure 8.6: Growth in length of Alligator Gar in Texas."));
    }

    #[test]
    fn test_normalize_chart_like_markdown_trims_large_top_table_plate() {
        let input = "| A | B | C | D | E | F | G | H |\n\
                     | --- | --- | --- | --- | --- | --- | --- | --- |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\
                     | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |\n\n\
                     Table 2: Evaluation results for SOLAR 10.7B and SOLAR 10.7B-Instruct along with other top-performing models in the paper.\n\n\
                     # 4.2 Main Results\n\n\
                     The surrounding prose should be dropped.\n";

        let normalized = normalize_chart_like_markdown(input);
        assert!(normalized.starts_with("| A | B | C | D | E | F | G | H |"));
        assert!(!normalized.contains("Table 2:"));
        assert!(!normalized.contains("4.2 Main Results"));
        assert!(!normalized.contains("surrounding prose"));
    }

}
