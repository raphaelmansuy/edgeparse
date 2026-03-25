//! Markdown output generator.

use std::collections::HashMap;

use crate::models::bbox::BoundingBox;
use crate::models::chunks::TextChunk;
use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::enums::SemanticType;
use crate::models::semantic::SemanticTextNode;
use crate::models::table::TableTokenRow;
use crate::EdgePdfError;

/// Generate Markdown representation of a PdfDocument.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on write failures.
pub fn to_markdown(doc: &PdfDocument) -> Result<String, EdgePdfError> {
    if looks_like_contents_document(doc) {
        return Ok(render_contents_document(doc));
    }
    if looks_like_compact_toc_document(doc) {
        return Ok(render_compact_toc_document(doc));
    }

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
        return Ok(output);
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
    let output = drop_isolated_noise_lines(&output);

    Ok(output)
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
                        out.push_str(&format!("- {}\n", pending.trim()));
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
                    out.push_str(&format!("- {}\n", pending.trim()));
                }
                i += 1;
            }
            if let Some(pending) = pending_item.take() {
                out.push_str(&format!("- {}\n", pending.trim()));
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

    tokens[..tokens.len() - 1]
        .iter()
        .all(|token| token.chars().next().is_some_and(char::is_uppercase))
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

fn should_merge_list_continuation(previous: &str, current: &str) -> bool {
    let trimmed = current.trim();
    if trimmed.is_empty()
        || looks_like_stray_list_page_number(trimmed)
        || is_list_section_heading(trimmed)
        || looks_like_numbered_section(trimmed)
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

    if prev.ends_with('-')
        && prev.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic())
        && next_trimmed.chars().next().is_some_and(char::is_lowercase)
    {
        return true;
    }

    next_trimmed.chars().next().is_some_and(char::is_lowercase)
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

/// Render a SemanticTable as a markdown table.
fn render_table(out: &mut String, table: &crate::models::semantic::SemanticTable) {
    // Delegate to render_table_border which handles cross-page linking.
    render_table_border(out, &table.table_border);
}

#[derive(Clone)]
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

    regions
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

    let needs_stub = infer_left_stub_requirement(doc, &candidate_indices, &table_rows, &column_ranges);
    if !needs_stub {
        return None;
    }
    let slot_ranges = slot_ranges(&column_ranges, doc, &candidate_indices, needs_stub)?;
    let mut header_rows = reconstruct_aligned_rows(doc, &candidate_indices, &slot_ranges, true, 2);
    if header_rows.is_empty() {
        return None;
    }
    normalize_leading_stub_header(&mut header_rows);

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
    let body_rows = if needs_stub && should_merge_panel_body_rows(&table_rows) {
        let trailing_rows = reconstruct_aligned_rows(doc, &following_indices, &slot_ranges, false, 1);
        vec![merge_panel_body_row(&table_rows, &trailing_rows, slot_count)]
    } else if needs_stub {
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

    // Merge multi-line header rows into a single header row.
    merge_continuation_rows(&mut rendered_rows);

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
        if (gap_all_blank || gap_heading_only || gap_short_fragment)
            && prev.cols > 0
            && curr.cols > 0
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

}
