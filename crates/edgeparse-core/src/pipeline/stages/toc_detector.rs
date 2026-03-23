//! TOC (Table of Contents) Detection
//!
//! Identifies paragraphs that form a Table of Contents by looking for
//! text lines ending with page numbers, optionally preceded by dot leaders.

use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::semantic::SemanticHeading;

/// Minimum number of consecutive TOC-like entries to classify a group as TOC.
const MIN_TOC_ENTRIES: usize = 3;

/// Detect and mark TOC entries across all pages.
pub fn detect_toc(pages: &mut [Vec<ContentElement>]) {
    for page in pages.iter_mut() {
        if mark_mixed_toc_page(page) {
            promote_explicit_toc_titles(page);
            continue;
        }

        // Identify which indices are TOC-like
        let toc_flags: Vec<bool> = page.iter().map(is_toc_entry).collect();

        // Find runs of consecutive TOC-like entries
        let mut run_start: Option<usize> = None;
        for (i, &flag) in toc_flags.iter().enumerate() {
            match (flag, run_start) {
                (true, None) => run_start = Some(i),
                (true, Some(_)) => {}
                (false, Some(start)) => {
                    mark_toc_run(page, start, i);
                    run_start = None;
                }
                (false, None) => {}
            }
        }
        if let Some(start) = run_start {
            mark_toc_run(page, start, toc_flags.len());
        }

        promote_explicit_toc_titles(page);
    }
}

fn mark_mixed_toc_page(page: &mut [ContentElement]) -> bool {
    let mut paragraph_indices = Vec::new();
    let mut toc_entry_count = 0usize;
    let mut supported_count = 0usize;

    for (idx, elem) in page.iter().enumerate() {
        let Some(text) = paragraph_text(elem) else {
            continue;
        };
        paragraph_indices.push(idx);
        if looks_like_toc_line(&text) {
            toc_entry_count += 1;
            supported_count += 1;
        } else if looks_like_toc_support_heading(&text) {
            supported_count += 1;
        }
    }

    if toc_entry_count < MIN_TOC_ENTRIES || paragraph_indices.len() < MIN_TOC_ENTRIES + 2 {
        return false;
    }
    if supported_count * 10 < paragraph_indices.len() * 8 {
        return false;
    }

    for idx in paragraph_indices {
        if let ContentElement::Paragraph(p) = &mut page[idx] {
            let text = p.base.value();
            if looks_like_toc_line(&text) || looks_like_toc_support_heading(&text) {
                p.base.semantic_type = SemanticType::TableOfContent;
            }
        }
    }

    true
}

/// Check if a content element looks like a TOC entry.
fn is_toc_entry(elem: &ContentElement) -> bool {
    if let ContentElement::Paragraph(p) = elem {
        if matches!(
            p.base.semantic_type,
            SemanticType::Header | SemanticType::Footer | SemanticType::Note
        ) {
            return false;
        }
        looks_like_toc_line(&p.base.value())
    } else {
        false
    }
}

fn looks_like_toc_line(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.ends_with(['.', ';', ':']) {
        return false;
    }

    let mut parts = trimmed.rsplitn(2, char::is_whitespace);
    let page = match parts.next() {
        Some(token) => token,
        None => return false,
    };
    let title = match parts.next() {
        Some(prefix) => prefix.trim_end(),
        None => return false,
    };

    if !(1..=4).contains(&page.len()) || !page.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    if title.is_empty() {
        return false;
    }

    let word_count = title.split_whitespace().count();
    if word_count == 0 || word_count > 14 || trimmed.len() > 90 {
        return false;
    }
    if !title.chars().any(char::is_alphabetic) {
        return false;
    }

    let has_leader = title.contains("...") || title.contains('…') || title.contains('·');
    if has_leader {
        return true;
    }

    if title.ends_with(['.', ';', ':']) {
        return false;
    }

    // Accept plain-space TOC entries such as "Experiment #10: Pumps 84" while
    // still rejecting long prose that merely ends in a number.
    word_count >= 2
}

fn paragraph_text(elem: &ContentElement) -> Option<String> {
    match elem {
        ContentElement::Paragraph(p) => Some(p.base.value()),
        _ => None,
    }
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

fn ends_with_page_marker(text: &str) -> bool {
    text.split_whitespace().last().is_some_and(|token| {
        let stripped = token.trim_matches(|c: char| matches!(c, '.' | ',' | ')' | '('));
        (1..=4).contains(&stripped.len()) && stripped.chars().all(|c| c.is_ascii_digit())
    })
}

/// Mark a run of elements as TOC if it's long enough.
fn mark_toc_run(page: &mut [ContentElement], start: usize, end: usize) {
    if end - start < MIN_TOC_ENTRIES {
        return;
    }

    promote_preceding_toc_title(page, start);

    for elem in &mut page[start..end] {
        if let ContentElement::Paragraph(p) = elem {
            p.base.semantic_type = SemanticType::TableOfContent;
        }
    }
}

fn promote_preceding_toc_title(page: &mut [ContentElement], start: usize) {
    if start == 0 {
        return;
    }

    for idx in (0..start).rev() {
        match &page[idx] {
            ContentElement::Paragraph(p) if is_toc_title(&p.base.value()) => {
                let para = p.clone();
                page[idx] = ContentElement::Heading(SemanticHeading {
                    base: para,
                    heading_level: Some(1),
                });
                break;
            }
            ContentElement::Paragraph(_) => break,
            ContentElement::Heading(_) => break,
            _ => {}
        }
    }
}

fn is_toc_title(text: &str) -> bool {
    matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "contents" | "table of contents"
    )
}

fn promote_explicit_toc_titles(page: &mut [ContentElement]) {
    for idx in 0..page.len().saturating_sub(1) {
        let is_title =
            matches!(&page[idx], ContentElement::Paragraph(p) if is_toc_title(&p.base.value()));
        if !is_title || !next_element_looks_like_toc(page, idx + 1) {
            continue;
        }

        if let ContentElement::Paragraph(p) = &page[idx] {
            let para = p.clone();
            page[idx] = ContentElement::Heading(SemanticHeading {
                base: para,
                heading_level: Some(1),
            });
        }
    }
}

fn next_element_looks_like_toc(page: &[ContentElement], mut idx: usize) -> bool {
    while idx < page.len() {
        match &page[idx] {
            ContentElement::List(lst) => return lst.list_items.len() >= MIN_TOC_ENTRIES,
            ContentElement::Paragraph(p) => {
                return p.base.semantic_type == SemanticType::TableOfContent
                    || is_toc_entry(&page[idx]);
            }
            ContentElement::Heading(_) => return false,
            ContentElement::HeaderFooter(_) => idx += 1,
            _ => return false,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    fn make_para(text: &str, y: f64) -> ContentElement {
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(Some(1), 72.0, y, 500.0, y + 12.0),
            font_name: "Arial".to_string(),
            font_size: 10.0,
            font_weight: 400.0,
            italic_angle: 0.0,
            font_color: "000000".to_string(),
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
            bbox: chunk.bbox.clone(),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: true,
            is_line_end: true,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: line.bbox.clone(),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let col = TextColumn {
            bbox: block.bbox.clone(),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: y + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: SemanticTextNode {
                bbox: col.bbox.clone(),
                index: None,
                level: None,
                semantic_type: SemanticType::Paragraph,
                correct_semantic_score: None,
                columns: vec![col],
                font_weight: Some(400.0),
                font_size: Some(10.0),
                text_color: None,
                italic_angle: None,
                font_name: Some("Arial".to_string()),
                text_format: None,
                max_font_size: Some(10.0),
                background_color: None,
                is_hidden_text: false,
            },
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    #[test]
    fn test_toc_detected() {
        let e1 = make_para("Chapter 1 .............. 1", 700.0);
        let e2 = make_para("Chapter 2 .............. 15", 688.0);
        let e3 = make_para("Chapter 3 .............. 30", 676.0);
        let mut pages = vec![vec![e1, e2, e3]];

        detect_toc(&mut pages);

        for elem in &pages[0] {
            if let ContentElement::Paragraph(p) = elem {
                assert_eq!(p.base.semantic_type, SemanticType::TableOfContent);
            }
        }
    }

    #[test]
    fn test_too_few_entries_not_toc() {
        let e1 = make_para("Chapter 1 .............. 1", 700.0);
        let e2 = make_para("Chapter 2 .............. 15", 688.0);
        let mut pages = vec![vec![e1, e2]];

        detect_toc(&mut pages);

        for elem in &pages[0] {
            if let ContentElement::Paragraph(p) = elem {
                assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
            }
        }
    }

    #[test]
    fn test_normal_text_not_toc() {
        let e1 = make_para("This is a normal paragraph without page numbers.", 700.0);
        let e2 = make_para("Another regular paragraph.", 688.0);
        let e3 = make_para("Third paragraph here.", 676.0);
        let mut pages = vec![vec![e1, e2, e3]];

        detect_toc(&mut pages);

        for elem in &pages[0] {
            if let ContentElement::Paragraph(p) = elem {
                assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
            }
        }
    }

    #[test]
    fn test_toc_title_promoted_to_heading() {
        let title = make_para("Contents", 712.0);
        let e1 = make_para("Chapter 1 .............. 1", 700.0);
        let e2 = make_para("Chapter 2 .............. 15", 688.0);
        let e3 = make_para("Chapter 3 .............. 30", 676.0);
        let mut pages = vec![vec![title, e1, e2, e3]];

        detect_toc(&mut pages);

        assert!(matches!(pages[0][0], ContentElement::Heading(_)));
    }

    #[test]
    fn test_plain_space_toc_entries_detected() {
        let e1 = make_para("Experiment #1: Hydrostatic Pressure 3", 700.0);
        let e2 = make_para("Experiment #2: Bernoulli's Theorem Demonstration 13", 688.0);
        let e3 = make_para("Experiment #3: Energy Loss in Pipe Fittings 24", 676.0);
        let mut pages = vec![vec![e1, e2, e3]];

        detect_toc(&mut pages);

        for elem in &pages[0] {
            if let ContentElement::Paragraph(p) = elem {
                assert_eq!(p.base.semantic_type, SemanticType::TableOfContent);
            }
        }
    }

    #[test]
    fn test_mixed_toc_page_with_part_headings_detected() {
        let mut pages = vec![vec![
            make_para(
                "Part V. Chapter Five - Comparing Associations Between Multiple Variables",
                712.0,
            ),
            make_para("Section 5.1: The Linear Model 35", 700.0),
            make_para(
                "Part VI. Chapter Six - Comparing Three or More Group Means",
                688.0,
            ),
            make_para(
                "Section 6.1: Between Versus Within Group Analyses 49",
                676.0,
            ),
            make_para(
                "Part VII. Chapter Seven - Moderation and Mediation Analyses",
                664.0,
            ),
            make_para("Section 7.1: Mediation and Moderation Models 64", 652.0),
        ]];

        detect_toc(&mut pages);

        for elem in &pages[0] {
            if let ContentElement::Paragraph(p) = elem {
                assert_eq!(p.base.semantic_type, SemanticType::TableOfContent);
            }
        }
    }

    #[test]
    fn test_toc_title_promoted_before_list() {
        use crate::models::list::{ListBody, ListItem, ListLabel, PDFList};

        let title = make_para("Contents", 712.0);
        let mk_row = |value: &str| {
            vec![vec![crate::models::table::TableToken {
                base: TextChunk {
                    value: value.to_string(),
                    bbox: BoundingBox::new(Some(1), 72.0, 700.0, 500.0, 712.0),
                    font_name: "Arial".to_string(),
                    font_size: 10.0,
                    font_weight: 400.0,
                    italic_angle: 0.0,
                    font_color: "000000".to_string(),
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
                token_type: crate::models::table::TableTokenType::Text,
            }]]
        };

        let list = ContentElement::List(PDFList {
            bbox: BoundingBox::new(Some(1), 72.0, 640.0, 500.0, 700.0),
            index: None,
            level: None,
            list_items: vec![
                ListItem {
                    bbox: BoundingBox::new(Some(1), 72.0, 688.0, 500.0, 700.0),
                    index: None,
                    level: None,
                    label: ListLabel {
                        bbox: BoundingBox::new(Some(1), 72.0, 688.0, 90.0, 700.0),
                        content: mk_row("1."),
                        semantic_type: None,
                    },
                    body: ListBody {
                        bbox: BoundingBox::new(Some(1), 90.0, 688.0, 500.0, 700.0),
                        content: mk_row("Overview"),
                        semantic_type: None,
                    },
                    label_length: 2,
                    contents: Vec::new(),
                    semantic_type: None,
                },
                ListItem {
                    bbox: BoundingBox::new(Some(1), 72.0, 676.0, 500.0, 688.0),
                    index: None,
                    level: None,
                    label: ListLabel {
                        bbox: BoundingBox::new(Some(1), 72.0, 676.0, 90.0, 688.0),
                        content: mk_row("2."),
                        semantic_type: None,
                    },
                    body: ListBody {
                        bbox: BoundingBox::new(Some(1), 90.0, 676.0, 500.0, 688.0),
                        content: mk_row("Details"),
                        semantic_type: None,
                    },
                    label_length: 2,
                    contents: Vec::new(),
                    semantic_type: None,
                },
                ListItem {
                    bbox: BoundingBox::new(Some(1), 72.0, 664.0, 500.0, 676.0),
                    index: None,
                    level: None,
                    label: ListLabel {
                        bbox: BoundingBox::new(Some(1), 72.0, 664.0, 90.0, 676.0),
                        content: mk_row("3."),
                        semantic_type: None,
                    },
                    body: ListBody {
                        bbox: BoundingBox::new(Some(1), 90.0, 664.0, 500.0, 676.0),
                        content: mk_row("FAQ"),
                        semantic_type: None,
                    },
                    label_length: 2,
                    contents: Vec::new(),
                    semantic_type: None,
                },
            ],
            numbering_style: Some("1.".to_string()),
            common_prefix: None,
            previous_list_id: None,
            next_list_id: None,
        });

        let mut pages = vec![vec![title, list]];
        detect_toc(&mut pages);
        assert!(matches!(pages[0][0], ContentElement::Heading(_)));
    }
}
