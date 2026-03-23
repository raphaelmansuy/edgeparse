//! Stage 14: Caption Linking
//!
//! Links caption-like paragraphs to nearby images and tables.
//! A caption is a short paragraph starting with "Figure", "Table", "Fig.", etc.
//! that is vertically adjacent to an image or table.

use regex::Regex;
use std::sync::LazyLock;

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
use crate::models::text::{TextBlock, TextColumn, TextLine};

/// Maximum vertical gap (in points) between a caption and its target.
const MAX_CAPTION_GAP: f64 = 30.0;

/// Maximum number of lines for a paragraph to be considered a caption.
const MAX_CAPTION_LINES: usize = 6;

static CAPTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(figure|fig\.|table|tab\.|chart|graph|image|illustration|diagram|plate|map|exhibit)\s*[\d.:ixvIXV]").unwrap()
});

/// Link captions to their target images/tables.
///
/// This sets caption semantic type on qualifying paragraphs but does not
/// remove or restructure them. A future iteration could add explicit
/// caption-target links.
pub fn link_captions(pages: &mut [Vec<ContentElement>]) {
    for page in pages.iter_mut() {
        let mut i = 0usize;
        while i < page.len() {
            if let Some((caption, body)) = split_mixed_caption_paragraph(page, i) {
                page[i] = caption;
                page.insert(i + 1, body);
                i += 2;
                continue;
            }
            i += 1;
        }

        let len = page.len();
        if len < 2 {
            continue;
        }
        // Collect indices of potential captions and their targets
        let mut caption_indices: Vec<usize> = Vec::new();

        for i in 0..len {
            if !is_caption_candidate(&page[i]) {
                continue;
            }

            // Check if adjacent element above or below is an image or table
            let has_target_above = i > 0
                && is_captionable(&page[i - 1])
                && vertical_gap(&page[i - 1], &page[i]) < MAX_CAPTION_GAP;
            let has_target_below = i + 1 < len
                && is_captionable(&page[i + 1])
                && vertical_gap(&page[i], &page[i + 1]) < MAX_CAPTION_GAP;

            if has_target_above || has_target_below {
                caption_indices.push(i);
            }
        }

        // Mark captions
        for idx in caption_indices {
            mark_as_caption(&mut page[idx]);
        }
    }
}

fn split_mixed_caption_paragraph(
    page: &[ContentElement],
    idx: usize,
) -> Option<(ContentElement, ContentElement)> {
    let elem = page.get(idx)?;
    let paragraph = match elem {
        ContentElement::Paragraph(p) => p,
        _ => return None,
    };
    if !starts_with_caption_prefix(&paragraph.base.value()) {
        return None;
    }
    let has_target_above = idx > 0
        && is_captionable(&page[idx - 1])
        && vertical_gap(&page[idx - 1], elem) < MAX_CAPTION_GAP;
    let has_target_below = idx + 1 < page.len()
        && is_captionable(&page[idx + 1])
        && vertical_gap(elem, &page[idx + 1]) < MAX_CAPTION_GAP;
    if !has_target_above && !has_target_below {
        return None;
    }

    let block = paragraph.base.columns.first()?.text_blocks.first()?;
    let split_at = detect_caption_body_split(block)?;
    let (caption_block, body_block) = split_block_at(block, split_at)?;

    let mut caption = wrap_block_as_paragraph(caption_block, SemanticType::Caption);
    if let ContentElement::Paragraph(cap) = &mut caption {
        cap.enclosed_top = paragraph.enclosed_top;
        cap.indentation = paragraph.indentation;
    }
    let mut body = wrap_block_as_paragraph(body_block, SemanticType::Paragraph);
    if let ContentElement::Paragraph(rest) = &mut body {
        rest.enclosed_bottom = paragraph.enclosed_bottom;
        rest.indentation = paragraph.indentation;
    }
    Some((caption, body))
}

/// Check if an element could be a caption.
fn is_caption_candidate(elem: &ContentElement) -> bool {
    match elem {
        ContentElement::Paragraph(p) => {
            if p.base.lines_number() > MAX_CAPTION_LINES {
                return false;
            }
            let text = p.base.value();
            let trimmed = text.trim();
            CAPTION_RE.is_match(trimmed)
        }
        ContentElement::Heading(h) => {
            if h.base.base.lines_number() > MAX_CAPTION_LINES {
                return false;
            }
            let text = h.base.base.value();
            CAPTION_RE.is_match(text.trim())
        }
        _ => false,
    }
}

fn starts_with_caption_prefix(text: &str) -> bool {
    CAPTION_RE.is_match(text.trim())
}

/// Check if an element can have a caption (image, table, or figure).
fn is_captionable(elem: &ContentElement) -> bool {
    matches!(
        elem,
        ContentElement::Image(_)
            | ContentElement::TableBorder(_)
            | ContentElement::Table(_)
            | ContentElement::Figure(_)
    )
}

/// Compute vertical gap between two elements.
fn vertical_gap(above: &ContentElement, below: &ContentElement) -> f64 {
    let a = above.bbox();
    let b = below.bbox();
    (a.bottom_y - b.top_y).abs()
}

/// Mark an element as a caption by setting its semantic type.
fn mark_as_caption(elem: &mut ContentElement) {
    match elem {
        ContentElement::Paragraph(p) => {
            p.base.semantic_type = crate::models::enums::SemanticType::Caption;
        }
        ContentElement::Heading(h) => {
            h.base.base.semantic_type = crate::models::enums::SemanticType::Caption;
        }
        _ => {}
    }
}

fn detect_caption_body_split(block: &TextBlock) -> Option<usize> {
    if block.text_lines.len() < 4 {
        return None;
    }
    if !starts_with_caption_prefix(&block.value()) {
        return None;
    }

    let gaps: Vec<f64> = block
        .text_lines
        .windows(2)
        .map(|pair| (pair[0].base_line - pair[1].base_line).abs())
        .collect();
    let typical_gap = gaps
        .iter()
        .copied()
        .reduce(f64::min)
        .unwrap_or(0.0)
        .max(1.0);

    for split_at in 2..block.text_lines.len() {
        if split_at > MAX_CAPTION_LINES {
            break;
        }
        let gap = gaps.get(split_at - 1).copied().unwrap_or(0.0);
        if gap < typical_gap * 1.35 {
            continue;
        }

        let caption_text = block.text_lines[..split_at]
            .iter()
            .map(TextLine::value)
            .collect::<Vec<_>>()
            .join(" ");
        let body_text = block.text_lines[split_at..]
            .iter()
            .map(TextLine::value)
            .collect::<Vec<_>>()
            .join(" ");
        if !starts_with_caption_prefix(&caption_text) {
            continue;
        }
        if body_text.split_whitespace().count() < 8 {
            continue;
        }
        return Some(split_at);
    }

    None
}

fn split_block_at(block: &TextBlock, at: usize) -> Option<(TextBlock, TextBlock)> {
    if at == 0 || at >= block.text_lines.len() {
        return None;
    }

    let head_lines = block.text_lines[..at].to_vec();
    let rest_lines = block.text_lines[at..].to_vec();
    Some((
        rebuild_block(block, head_lines, false),
        rebuild_block(block, rest_lines, block.has_end_line),
    ))
}

fn rebuild_block(template: &TextBlock, text_lines: Vec<TextLine>, has_end_line: bool) -> TextBlock {
    let bbox = union_line_bboxes(&text_lines);
    let font_size =
        text_lines.iter().map(|line| line.font_size).sum::<f64>() / text_lines.len() as f64;
    let base_line = text_lines
        .last()
        .map(|line| line.base_line)
        .unwrap_or(template.base_line);
    let is_hidden_text = text_lines.iter().all(|line| line.is_hidden_text);

    TextBlock {
        bbox,
        index: None,
        level: template.level.clone(),
        font_size,
        base_line,
        slant_degree: template.slant_degree,
        is_hidden_text,
        text_lines,
        has_start_line: false,
        has_end_line,
        text_alignment: template.text_alignment,
    }
}

fn union_line_bboxes(lines: &[TextLine]) -> BoundingBox {
    lines
        .iter()
        .map(|line| line.bbox.clone())
        .reduce(|a, b| a.union(&b))
        .unwrap_or_else(|| BoundingBox::new(None, 0.0, 0.0, 0.0, 0.0))
}

fn wrap_block_as_paragraph(block: TextBlock, semantic_type: SemanticType) -> ContentElement {
    let bbox = block.bbox.clone();
    let font_size = block.font_size;
    let font_weight = dominant_font_weight(&block);
    let font_name = dominant_font_name(&block);
    let is_hidden_text = block.is_hidden_text;
    let level = block.level.clone();

    ContentElement::Paragraph(SemanticParagraph {
        base: SemanticTextNode {
            bbox: bbox.clone(),
            index: None,
            level,
            semantic_type,
            correct_semantic_score: None,
            columns: vec![TextColumn {
                bbox,
                index: None,
                level: None,
                font_size,
                base_line: block.base_line,
                slant_degree: block.slant_degree,
                is_hidden_text,
                text_blocks: vec![block],
            }],
            font_weight: Some(font_weight),
            font_size: Some(font_size),
            text_color: None,
            italic_angle: None,
            font_name,
            text_format: None,
            max_font_size: Some(font_size),
            background_color: None,
            is_hidden_text,
        },
        enclosed_top: false,
        enclosed_bottom: false,
        indentation: 0,
    })
}

fn dominant_font_weight(block: &TextBlock) -> f64 {
    use std::collections::HashMap;

    let mut counts: HashMap<i32, usize> = HashMap::new();
    for line in &block.text_lines {
        for chunk in &line.text_chunks {
            *counts.entry(chunk.font_weight.round() as i32).or_insert(0) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(weight, _)| weight as f64)
        .unwrap_or(400.0)
}

fn dominant_font_name(block: &TextBlock) -> Option<String> {
    use std::collections::HashMap;

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for line in &block.text_lines {
        for chunk in &line.text_chunks {
            if !chunk.font_name.is_empty() {
                *counts.entry(chunk.font_name.as_str()).or_insert(0) +=
                    chunk.value.chars().count().max(1);
            }
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, count)| count)
        .map(|(name, _)| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::ImageChunk;
    use crate::models::chunks::TextChunk;
    use crate::models::enums::SemanticType;
    use crate::models::enums::{PdfLayer, TextFormat, TextType};
    use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
    use crate::models::text::{TextBlock, TextColumn, TextLine};

    fn make_paragraph(text: &str, y_top: f64, y_bottom: f64) -> ContentElement {
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: BoundingBox::new(Some(1), 72.0, y_bottom, 300.0, y_top),
            index: None,
            level: None,
            mcid: None,
            page_number: Some(1),
            font_size: 10.0,
            font_weight: 400.0,
            font_name: "Arial".to_string(),
            italic_angle: 0.0,
            font_color: "#000000".to_string(),
            contrast_ratio: 21.0,
            symbol_ends: Vec::new(),
            text_format: TextFormat::Normal,
            text_type: TextType::Regular,
            pdf_layer: PdfLayer::Main,
            ocg_visible: true,
        };
        let line = TextLine {
            bbox: BoundingBox::new(Some(1), 72.0, y_bottom, 300.0, y_top),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: y_bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_chunks: vec![chunk],
            is_line_start: false,
            is_line_end: false,
            is_list_line: false,
            connected_line_art_label: None,
        };
        let block = TextBlock {
            bbox: BoundingBox::new(Some(1), 72.0, y_bottom, 300.0, y_top),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: y_bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: false,
            has_end_line: false,
            text_alignment: None,
        };
        let col = TextColumn {
            bbox: BoundingBox::new(Some(1), 72.0, y_bottom, 300.0, y_top),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: y_bottom + 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        let node = SemanticTextNode {
            bbox: BoundingBox::new(Some(1), 72.0, y_bottom, 300.0, y_top),
            index: None,
            level: None,
            semantic_type: SemanticType::Paragraph,
            correct_semantic_score: None,
            columns: vec![col],
            font_weight: None,
            font_size: Some(10.0),
            text_color: None,
            italic_angle: None,
            font_name: None,
            text_format: None,
            max_font_size: None,
            background_color: None,
            is_hidden_text: false,
        };
        ContentElement::Paragraph(SemanticParagraph {
            base: node,
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        })
    }

    fn make_multiline_paragraph_with_gaps(
        lines: &[&str],
        left: f64,
        right: f64,
        top: f64,
        line_gaps: &[f64],
    ) -> ContentElement {
        let mut text_lines = Vec::new();
        let mut line_top = top;
        for (idx, line_text) in lines.iter().enumerate() {
            let line_bottom = line_top - 10.0;
            let chunk = TextChunk {
                value: (*line_text).to_string(),
                bbox: BoundingBox::new(Some(1), left, line_bottom, right, line_top),
                index: None,
                level: None,
                mcid: None,
                page_number: Some(1),
                font_size: 10.0,
                font_weight: 400.0,
                font_name: "Arial".to_string(),
                italic_angle: 0.0,
                font_color: "#000000".to_string(),
                contrast_ratio: 21.0,
                symbol_ends: Vec::new(),
                text_format: TextFormat::Normal,
                text_type: TextType::Regular,
                pdf_layer: PdfLayer::Main,
                ocg_visible: true,
            };
            text_lines.push(TextLine {
                bbox: chunk.bbox.clone(),
                index: None,
                level: None,
                font_size: 10.0,
                base_line: line_bottom + 2.0,
                slant_degree: 0.0,
                is_hidden_text: false,
                text_chunks: vec![chunk],
                is_line_start: false,
                is_line_end: false,
                is_list_line: false,
                connected_line_art_label: None,
            });
            let gap = line_gaps.get(idx).copied().unwrap_or(4.0);
            line_top = line_bottom - gap;
        }

        let bbox = text_lines
            .iter()
            .map(|line| line.bbox.clone())
            .reduce(|a, b| a.union(&b))
            .unwrap();
        let block = TextBlock {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: 10.0,
            base_line: text_lines.last().map(|line| line.base_line).unwrap_or(0.0),
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines,
            has_start_line: false,
            has_end_line: true,
            text_alignment: None,
        };
        let col = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: 10.0,
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

    fn make_image(y_top: f64, y_bottom: f64) -> ContentElement {
        ContentElement::Image(ImageChunk {
            bbox: BoundingBox::new(Some(1), 72.0, y_bottom, 300.0, y_top),
            index: None,
            level: None,
        })
    }

    #[test]
    fn test_caption_below_image() {
        let img = make_image(500.0, 400.0);
        let cap = make_paragraph("Figure 1: Sample image", 395.0, 385.0);
        let mut pages = vec![vec![img, cap]];
        link_captions(&mut pages);
        if let ContentElement::Paragraph(p) = &pages[0][1] {
            assert_eq!(p.base.semantic_type, SemanticType::Caption);
        }
    }

    #[test]
    fn test_caption_above_image() {
        let cap = make_paragraph("Figure 2: Another image", 520.0, 510.0);
        let img = make_image(505.0, 400.0);
        let mut pages = vec![vec![cap, img]];
        link_captions(&mut pages);
        if let ContentElement::Paragraph(p) = &pages[0][0] {
            assert_eq!(p.base.semantic_type, SemanticType::Caption);
        }
    }

    #[test]
    fn test_non_caption_text_not_marked() {
        let img = make_image(500.0, 400.0);
        let text = make_paragraph("This is regular text", 395.0, 385.0);
        let mut pages = vec![vec![img, text]];
        link_captions(&mut pages);
        if let ContentElement::Paragraph(p) = &pages[0][1] {
            assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
        }
    }

    #[test]
    fn test_caption_too_far_not_linked() {
        let img = make_image(500.0, 400.0);
        let cap = make_paragraph("Figure 3: Distant caption", 350.0, 340.0);
        let mut pages = vec![vec![img, cap]];
        link_captions(&mut pages);
        if let ContentElement::Paragraph(p) = &pages[0][1] {
            assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
        }
    }

    #[test]
    fn test_merged_caption_and_body_paragraph_is_split() {
        let img = make_image(500.0, 400.0);
        let para = make_multiline_paragraph_with_gaps(
            &[
                "Figure 1. This image shows the Western hemisphere as viewed",
                "from space 35,400 kilometers above Earth.",
                "(credit: NASA/ GSFC/ NOAA/ USGS)",
                "Our nearest astronomical neighbor is Earth's satellite, commonly called the Moon.",
                "Figure 2 shows Earth and the Moon drawn to scale on the same diagram.",
            ],
            72.0,
            320.0,
            395.0,
            &[4.0, 4.0, 14.0, 4.0, 4.0],
        );
        let mut pages = vec![vec![img, para]];

        link_captions(&mut pages);

        assert_eq!(pages[0].len(), 3);
        match &pages[0][1] {
            ContentElement::Paragraph(p) => {
                assert_eq!(p.base.semantic_type, SemanticType::Caption);
                assert!(p.base.value().starts_with("Figure 1."));
            }
            other => panic!("Expected caption paragraph, got {other:?}"),
        }
        match &pages[0][2] {
            ContentElement::Paragraph(p) => {
                assert_eq!(p.base.semantic_type, SemanticType::Paragraph);
                assert!(p
                    .base
                    .value()
                    .starts_with("Our nearest astronomical neighbor"));
            }
            other => panic!("Expected body paragraph, got {other:?}"),
        }
    }
}
