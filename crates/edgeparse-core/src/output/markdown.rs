//! Markdown output generator.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::enums::SemanticType;
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

    let mut i = 0usize;
    while i < doc.kids.len() {
        match &doc.kids[i] {
            ContentElement::Heading(h) => {
                let text = h.base.base.value();
                let trimmed = text.trim();
                if trimmed.is_empty() || should_skip_heading_text(trimmed) {
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

                let level = h.heading_level.unwrap_or(1).min(6);

                // Merge consecutive heading fragments at the same level.
                // When the PDF splits a title across multiple text elements,
                // each becomes a separate heading; merge them into one.
                let mut merged_heading = trimmed.to_string();
                while let Some(ContentElement::Heading(next_h)) = doc.kids.get(i + 1) {
                    let next_level = next_h.heading_level.unwrap_or(1).min(6);
                    if next_level != level {
                        break;
                    }
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

                let hashes = "#".repeat(level as usize);
                output.push_str(&format!("{} {}\n\n", hashes, merged_heading.trim()));
            }
            ContentElement::NumberHeading(nh) => {
                let text = nh.base.base.base.value();
                let trimmed = text.trim();
                if trimmed.is_empty() || should_skip_heading_text(trimmed) {
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

                let level = nh.base.heading_level.unwrap_or(1).min(6);
                let hashes = "#".repeat(level as usize);
                output.push_str(&format!("{} {}\n\n", hashes, trimmed));
            }
            ContentElement::Paragraph(_) | ContentElement::TextBlock(_) | ContentElement::TextLine(_) => {
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
                    output.push_str(&format!("# {}\n\n", trimmed));
                    i += 1;
                    continue;
                }

                if matches!(element, ContentElement::Paragraph(p) if p.base.semantic_type == SemanticType::TableOfContent) {
                    output.push_str(&escape_md_line_start(trimmed));
                    output.push('\n');
                    i += 1;
                    continue;
                }

                if is_short_caption_label(trimmed) {
                    if let Some(next_text) = next_mergeable_paragraph_text(doc.kids.get(i + 1)) {
                        if let Some((caption_tail, body)) = split_following_caption_tail_and_body(&next_text) {
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

                            if let Some(year_text) = next_mergeable_paragraph_text(doc.kids.get(i + 2)) {
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
    let has_explicit_heading = early
        .clone()
        .any(|element| matches!(element, ContentElement::Heading(_) | ContentElement::NumberHeading(_)));
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
    let lines = collect_plain_lines(doc);
    let mut out = String::new();

    let mut iter = lines.into_iter();
    if let Some(first) = iter.next() {
        out.push_str("# ");
        out.push_str(first.trim());
        out.push_str("\n\n");
    }
    for line in iter {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out.push('\n');
    out
}

fn looks_like_compact_toc_document(doc: &PdfDocument) -> bool {
    let lines = collect_plain_lines(doc);
    if lines.len() < 8 {
        return false;
    }

    let page_like = lines.iter().filter(|line| ends_with_page_marker(line)).count();
    let support_like = lines
        .iter()
        .filter(|line| looks_like_toc_support_heading(line))
        .count();

    page_like >= 3
        && support_like >= 2
        && (page_like + support_like) * 10 >= lines.len() * 8
}

fn render_compact_toc_document(doc: &PdfDocument) -> String {
    let mut out = String::new();
    for line in collect_plain_lines(doc) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out.push('\n');
    out
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
                        list_item_text_from_contents(&item.contents).trim().to_string()
                    };
                    if !combined.trim().is_empty() {
                        lines.push(combined);
                    }
                }
            }
            ContentElement::Table(table) => {
                extend_contents_lines_from_rows(
                    &mut lines,
                    collect_rendered_table_rows(&table.table_border.rows, table.table_border.num_columns),
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
        for row in rows {
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
    trimmed.split_whitespace().count() <= 3
        && trimmed.len() <= 24
        && !trimmed.ends_with(['.', ':'])
}

fn split_following_caption_tail_and_body(text: &str) -> Option<(&str, &str)> {
    let trimmed = text.trim();
    if trimmed.is_empty() || starts_with_caption_prefix(trimmed) || !starts_with_uppercase_word(trimmed) {
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
    repair_fragmented_words(&rows.iter()
        .flat_map(|row| row.iter())
        .map(|token| token.base.value.as_str())
        .collect::<Vec<_>>()
        .join(" "))
}

fn render_element(out: &mut String, element: &ContentElement) {
    match element {
        ContentElement::Heading(h) => {
            let text = h.base.base.value();
            let trimmed = text.trim();
            if should_skip_heading_text(trimmed) {
                return;
            }
            let level = h.heading_level.unwrap_or(1).min(6);
            let hashes = "#".repeat(level as usize);
            out.push_str(&format!("{} {}\n\n", hashes, trimmed));
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
            while i < list.list_items.len() {
                let item = &list.list_items[i];
                let label = token_rows_text(&item.label.content);
                let body = token_rows_text(&item.body.content);
                let label_trimmed = label.trim();
                let body_trimmed = body.trim();
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
                    out.push_str(&format!("## {}\n\n", combined.trim_end_matches(':').trim()));
                    i += 1;
                    continue;
                }

                if !label_trimmed.is_empty() || !body_trimmed.is_empty() {
                    if !label_trimmed.is_empty() && !body_trimmed.is_empty() {
                        out.push_str(&format!("- {} {}\n", label_trimmed, body_trimmed));
                    } else if !body_trimmed.is_empty() {
                        out.push_str(&format!("- {}\n", body_trimmed));
                    } else {
                        out.push_str(&format!("- {}\n", label_trimmed));
                    }
                } else if !item.contents.is_empty() {
                    // Fallback: extract text from contents (used by list_pass2)
                    let text = list_item_text_from_contents(&item.contents);
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        out.push_str(&format!("- {}\n", trimmed));
                    }
                }
                i += 1;
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
            let trimmed = text.trim();
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
            let level = nh.base.heading_level.unwrap_or(1).min(6);
            let hashes = "#".repeat(level as usize);
            out.push_str(&format!("{} {}\n\n", hashes, trimmed));
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
            let trimmed = text.trim();
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
        "figure ", "fig. ", "table ", "tab. ", "chart ", "graph ", "image ", "illustration ",
        "diagram ", "plate ", "map ", "exhibit ",
        "photo by ", "photo credit", "image by ", "image credit",
        "image courtesy", "photo courtesy", "credit: ", "source: ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
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
    result
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
                        && should_rescue_as_heading(doc, idx, text)
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
    doc.kids.iter().any(|e| matches!(e, ContentElement::Heading(_) | ContentElement::NumberHeading(_)))
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
        .filter(|e| matches!(e, ContentElement::Heading(_) | ContentElement::NumberHeading(_)))
        .count();
    heading_count as f64 / total as f64
}

/// Rescue headings: identify short standalone paragraphs that likely serve
/// as section headings.  Only runs when the pipeline produced zero headings.
fn should_rescue_as_heading(
    doc: &PdfDocument,
    idx: usize,
    text: &str,
) -> bool {

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }

    let word_count = trimmed.split_whitespace().count();
    let has_alpha = trimmed.chars().any(char::is_alphabetic);

    // Must have alphabetic chars and not end with sentence/continuation punctuation
    if !has_alpha || trimmed.ends_with(['.', '!', '?', ';', ',']) {
        return false;
    }

    // Must not be fully parenthesized (citations)
    if trimmed.starts_with('(') && trimmed.ends_with(')') {
        return false;
    }

    // Must not look like a caption or chart label
    if starts_with_caption_prefix(trimmed) || looks_like_chart_label_heading(&doc.kids[idx], trimmed) {
        return false;
    }

    // Must be short: ≤ 6 words, ≤ 60 chars
    if word_count > 6 || trimmed.len() > 60 {
        return false;
    }

    // Must not be a purely numeric string
    if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ' ') {
        return false;
    }

    // First alphabetic character should be uppercase
    if let Some(first_alpha) = trimmed.chars().find(|c| c.is_alphabetic()) {
        if first_alpha.is_lowercase() {
            return false;
        }
    }

    // Look ahead for substantive content — require at least 3x longer or > 15 words
    let mut found_substantive = false;
    for offset in 1..=4 {
        let lookahead_idx = idx + offset;
        if lookahead_idx >= doc.kids.len() {
            break;
        }
        let look_elem = &doc.kids[lookahead_idx];
        match look_elem {
            ContentElement::Paragraph(p) => {
                let next_text = p.base.value();
                let nw = next_text.trim().split_whitespace().count();
                if nw >= word_count * 3 || nw > 15 {
                    found_substantive = true;
                    break;
                }
            }
            ContentElement::TextBlock(tb) => {
                let next_text = tb.value();
                let nw = next_text.trim().split_whitespace().count();
                if nw >= word_count * 3 || nw > 15 {
                    found_substantive = true;
                    break;
                }
            }
            ContentElement::TextLine(tl) => {
                let next_text = tl.value();
                let nw = next_text.trim().split_whitespace().count();
                if nw >= word_count * 3 || nw > 15 {
                    found_substantive = true;
                    break;
                }
            }
            ContentElement::List(_) | ContentElement::Table(_) | ContentElement::TableBorder(_)
            | ContentElement::Image(_) | ContentElement::Figure(_) => {
                found_substantive = true;
                break;
            }
            _ => continue,
        }
    }

    found_substantive
}

/// Rescue numbered section headings like "01 - Find Open Educational Resources"
/// or "4.2 Main Results" when heading density is low.
fn should_rescue_numbered_heading(
    doc: &PdfDocument,
    idx: usize,
    text: &str,
) -> bool {
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
    if trimmed.ends_with(['!', '?', ';']) {
        return false;
    }
    if trimmed.ends_with('.') && !looks_like_keyword_numbered_section(trimmed) {
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
                let nw = p.base.value().trim().split_whitespace().count();
                if nw > 10 {
                    return true;
                }
            }
            ContentElement::TextBlock(tb) => {
                let nw = tb.value().trim().split_whitespace().count();
                if nw > 10 {
                    return true;
                }
            }
            ContentElement::TextLine(tl) => {
                let nw = tl.value().trim().split_whitespace().count();
                if nw > 10 {
                    return true;
                }
            }
            ContentElement::List(_) | ContentElement::Table(_) | ContentElement::TableBorder(_)
            | ContentElement::Image(_) | ContentElement::Figure(_) => {
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
    "activity", "appendix", "case", "chapter", "exercise", "experiment",
    "lab", "lesson", "module", "part", "phase", "problem", "question",
    "section", "stage", "step", "task", "topic", "unit",
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
    if !SECTION_KEYWORDS.iter().any(|k| keyword.eq_ignore_ascii_case(k)) {
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
fn should_rescue_allcaps_heading(
    doc: &PdfDocument,
    idx: usize,
    text: &str,
) -> bool {
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
    if trimmed.ends_with(['.', ';']) {
        return false;
    }

    // Must not look like a caption
    if starts_with_caption_prefix(trimmed) {
        return false;
    }

    // Must not be purely numeric or a page number
    if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ' ') {
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
                let nw = p.base.value().trim().split_whitespace().count();
                if nw > 6 {
                    return true;
                }
            }
            ContentElement::TextBlock(tb) => {
                let nw = tb.value().trim().split_whitespace().count();
                if nw > 6 {
                    return true;
                }
            }
            ContentElement::TextLine(tl) => {
                let nw = tl.value().trim().split_whitespace().count();
                if nw > 6 {
                    return true;
                }
            }
            ContentElement::List(_) | ContentElement::Table(_) | ContentElement::TableBorder(_)
            | ContentElement::Image(_) | ContentElement::Figure(_) => {
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

    title_like
        && matches!(next, Some(ContentElement::List(_)))
        && !looks_like_chart_label_heading(element, trimmed)
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
        word.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase().as_str(),
        "a"
            | "an"
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
        && trimmed.chars().any(char::is_alphabetic)
        && !trimmed.chars().next().is_some_and(|c| c.is_ascii_digit())
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
        && target.chars().rev().nth(1).is_some_and(|c| c.is_alphabetic())
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
    !trimmed.is_empty()
        && trimmed.len() <= 4
        && trimmed.chars().all(|c| c.is_ascii_digit())
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
        "a", "an", "and", "are", "as", "at", "be", "by", "can", "for", "from", "if", "in",
        "into", "is", "it", "may", "must", "not", "of", "on", "or", "per", "that", "the",
        "to", "with",
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
    if rows[0].first().map_or(true, |c| c.trim().is_empty()) {
        return;
    }

    let mut merge_count = 0usize;
    for i in 1..rows.len() {
        let first_empty = rows[i].first().map_or(true, |c| c.trim().is_empty());
        if !first_empty {
            break; // hit a data row
        }
        // All non-empty cells must be short (header-like fragments).
        let all_short = rows[i]
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
        let ncols = rows[0].len().min(rows[i].len());
        for j in 0..ncols {
            let fragment = rows[i][j].trim().to_string();
            if !fragment.is_empty() {
                let target = rows[0][j].trim().to_string();
                rows[0][j] = if target.is_empty() {
                    fragment
                } else {
                    format!("{} {}", target, fragment)
                };
            }
        }
    }

    // Remove the merged rows.
    rows.drain(1..=merge_count);
}

/// Render a SemanticTable as a markdown table.
fn render_table(out: &mut String, table: &crate::models::semantic::SemanticTable) {
    let rows = &table.table_border.rows;
    if rows.is_empty() {
        return;
    }

    let num_cols = table.table_border.num_columns.max(1);

    // Collect non-empty rows (skip rows where all cells have no content).
    let mut rendered_rows: Vec<Vec<String>> = Vec::new();
    for row in rows.iter() {
        let cell_texts: Vec<String> = (0..num_cols)
            .map(|col| {
                row.cells.iter()
                    .find(|c| c.col_number == col)
                    .map(|c| cell_text_content(c))
                    .unwrap_or_default()
            })
            .collect();
        if !cell_texts.iter().all(|t| t.trim().is_empty()) {
            rendered_rows.push(cell_texts);
        }
    }

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

    for (row_idx, cell_texts) in rendered_rows.iter().enumerate() {
        out.push('|');
        for cell_text in cell_texts {
            out.push_str(&format!(" {} |", cell_text.trim()));
        }
        out.push('\n');

        // Add separator after first row (header)
        if row_idx == 0 {
            out.push('|');
            for _ in 0..num_cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

/// Render a TableBorder directly as a markdown table.
fn render_table_border(out: &mut String, table: &crate::models::table::TableBorder) {
    let rows = &table.rows;
    if rows.is_empty() {
        return;
    }

    let num_cols = table.num_columns.max(1);

    // Collect row texts, skipping entirely empty rows (artifact of line-art grid detection).
    // Empty rows arise when thin horizontal grid lines are detected as row separators,
    // producing rows with no corresponding text content from the content assigner.
    let mut rendered_rows: Vec<Vec<String>> = Vec::new();
    for row in rows.iter() {
        let cell_texts: Vec<String> = (0..num_cols)
            .map(|col| {
                row.cells.iter()
                    .find(|c| c.col_number == col)
                    .map(|c| cell_text_content(c))
                    .unwrap_or_default()
            })
            .collect();
        // Skip row if all cells are empty (whitespace only).
        let is_empty = cell_texts.iter().all(|t| t.trim().is_empty());
        if !is_empty {
            rendered_rows.push(cell_texts);
        }
    }

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

    for (row_idx, cell_texts) in rendered_rows.iter().enumerate() {
        out.push('|');
        for cell_text in cell_texts {
            out.push_str(&format!(" {} |", cell_text.trim()));
        }
        out.push('\n');

        // Add separator after first row (header)
        if row_idx == 0 {
            out.push('|');
            for _ in 0..num_cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
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
        return crate::models::text::TextLine::concatenate_chunks(&chunks);
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
    repair_fragmented_words(&text)
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
            blocks.push(Block { start: i, sep, end, cols });
            i = end + 1;
        } else {
            i += 1;
        }
    }

    if blocks.len() < 2 {
        return markdown.to_string();
    }

    // Group adjacent blocks that can be merged (only blanks between, same cols).
    // merge_leader[i] = the first block index this block merges into, or None.
    let mut merge_leader: Vec<Option<usize>> = vec![None; blocks.len()];
    for bi in 1..blocks.len() {
        let prev = &blocks[bi - 1];
        let curr = &blocks[bi];
        let gap_all_blank = (prev.end + 1..curr.start)
            .all(|li| lines[li].trim().is_empty());
        if gap_all_blank && prev.cols == curr.cols && prev.cols > 0 {
            let leader = merge_leader[bi - 1].unwrap_or(bi - 1);
            merge_leader[bi] = Some(leader);
        }
    }

    // Build the set of line ranges to skip (gap blanks + merged header/sep).
    let mut skip = vec![false; lines.len()];
    for (bi, leader) in merge_leader.iter().enumerate() {
        if leader.is_none() {
            continue;
        }
        let prev_bi = bi - 1;
        let prev_end = blocks[prev_bi].end;
        let curr = &blocks[bi];
        // Skip blank lines in the gap between prev and curr.
        for li in (prev_end + 1)..curr.start {
            skip[li] = true;
        }
        // Skip the separator line of the merged block.
        skip[curr.sep] = true;
    }

    let mut result = String::new();
    for (li, line) in lines.iter().enumerate() {
        if skip[li] {
            continue;
        }
        result.push_str(line);
        result.push('\n');
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
    use crate::models::semantic::{SemanticHeading, SemanticParagraph, SemanticTextNode};
    use crate::models::table::{TableBorder, TableBorderCell, TableBorderRow, TableToken, TableTokenType};
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
        assert!(!md.contains("# "), "Empty/whitespace title should not produce a heading");
    }

    #[test]
    fn test_repair_fragmented_words() {
        assert_eq!(
            repair_fragmented_words("Jurisdic tion Fore ign Req uire me nts"),
            "Jurisdiction Foreign Requirements"
        );
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
        let bbox = BoundingBox::new(Some(1), 72.0, bottom, 300.0, top);
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
        assert!(md.contains("Experiment #1: Hydrostatic Pressure 3"));
        assert!(md.contains("Experiment #2: Bernoulli's Theorem Demonstration 13"));
        assert!(md.contains("Experiment #7: Osborne Reynolds' Demonstration 59"));
        assert!(md.contains("References 101"));
    }

    #[test]
    fn test_toc_semantic_paragraphs_render_without_blank_lines() {
        let mut doc = PdfDocument::new("toc-semantic.pdf".to_string());
        let mut first = make_paragraph("Part V. Chapter Five - Comparing Associations Between Multiple Variables", 700.0, 712.0);
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
        doc.kids.push(make_paragraph("Part V. Chapter Five - Comparing Associations Between Multiple Variables", 700.0, 712.0));
        doc.kids.push(make_paragraph("Section 5.1: The Linear Model 35", 684.0, 696.0));
        doc.kids.push(make_paragraph("Part VI. Chapter Six - Comparing Three or More Group Means", 668.0, 680.0));
        doc.kids.push(make_paragraph("Section 6.1: Between Versus Within Group Analyses 49", 652.0, 664.0));
        doc.kids.push(make_paragraph("Part VII. Chapter Seven - Moderation and Mediation Analyses", 636.0, 648.0));
        doc.kids.push(make_paragraph("Section 7.1: Mediation and Moderation Models 64", 620.0, 632.0));
        doc.kids.push(make_paragraph("References 101", 604.0, 616.0));
        doc.kids.push(make_paragraph("Section 8.1: Factor Analysis Definitions 75", 588.0, 600.0));

        let md = to_markdown(&doc).unwrap();
        assert!(!md.contains("\n\nSection 5.1: The Linear Model 35"));
        assert!(md.contains("Part V. Chapter Five - Comparing Associations Between Multiple Variables\nSection 5.1: The Linear Model 35"));
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
        assert!(md.contains(
            "Figure 4\nKomnas HAM's YouTube channel as of 1 December\n2021"
        ));
        assert!(!md.contains("\n\n2021"));
    }

    #[test]
    fn test_mid_page_numeric_labels_are_not_dropped_as_page_numbers() {
        let mut doc = PdfDocument::new("chart.pdf".to_string());
        doc.kids.push(make_paragraph("Figure 1", 760.0, 772.0));
        doc.kids.push(make_paragraph("100", 520.0, 528.0));
        doc.kids.push(make_paragraph("Body text continues here.", 400.0, 412.0));
        doc.kids.push(make_paragraph("36", 20.0, 28.0));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("100"));
        assert!(!md.lines().any(|line| line.trim() == "36"));
    }

    #[test]
    fn test_semantic_paragraphs_are_not_remerged_in_markdown() {
        let mut doc = PdfDocument::new("paragraphs.pdf".to_string());
        doc.kids.push(make_paragraph("First semantic paragraph ends here.", 520.0, 532.0));
        doc.kids.push(make_paragraph("Second semantic paragraph starts here.", 500.0, 512.0));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("First semantic paragraph ends here.\n\nSecond semantic paragraph starts here."));
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

    fn make_two_column_table(rows: &[(&str, &str)]) -> ContentElement {
        let mut table_rows = Vec::new();
        for (row_number, (left, right)) in rows.iter().enumerate() {
            let top = 656.0 - row_number as f64 * 18.0;
            let bottom = top - 16.0;
            let mut cells = Vec::new();
            for (col_number, (text, left_x, right_x)) in [
                (*left, 72.0, 220.0),
                (*right, 220.0, 420.0),
            ]
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
            bbox: BoundingBox::new(Some(1), 72.0, 656.0 - rows.len() as f64 * 18.0 - 16.0, 420.0, 656.0),
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
            ("Added cation", "Relative Size & Settling Rates of Floccules"),
            ("K+", ""),
            ("Na+", ""),
            ("Ca2+", ""),
        ]));

        let md = to_markdown(&doc).unwrap();
        assert!(md.contains("| Added cation | Relative Size & Settling Rates of Floccules |"));
        assert!(md.contains("| K+ |  |"));
    }
}
