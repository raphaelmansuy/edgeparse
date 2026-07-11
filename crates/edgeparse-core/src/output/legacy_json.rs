//! Legacy-compatible JSON serializer.
//!
//! Produces JSON output with the legacy schema, including:
//!   - Space-separated key names ("file name", "page number", "bounding box", ...)
//!   - Array-style color format ("[0.0, 0.0, 0.0]" for black, "[r, g, b]" for RGB)
//!   - Globally sequential integer IDs
//!   - Element types: heading, paragraph, list, table, image, caption, header, footer
//!   - BoundingBox as [left_x, bottom_y, right_x, top_y] float array

use std::cell::Cell;

use serde_json::{json, Map, Value};

use crate::models::bbox::BoundingBox;
use crate::models::chunks::ImageChunk;
use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::enums::SemanticType;
use crate::models::list::{ListItem, PDFList};
use crate::models::semantic::{
    SemanticCaption, SemanticFigure, SemanticHeaderOrFooter, SemanticHeading,
    SemanticNumberHeading, SemanticParagraph, SemanticTable, SemanticTextNode,
};
use crate::models::table::{TableBorder, TableBorderCell, TableBorderRow, TableToken};
use crate::EdgePdfError;

// ---------------------------------------------------------------------------
// Thread-local ID counter
// ---------------------------------------------------------------------------

thread_local! {
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

fn next_id() -> u64 {
    NEXT_ID.with(|c| {
        let id = c.get();
        c.set(id + 1);
        id
    })
}

fn reset_ids() {
    NEXT_ID.with(|c| c.set(1));
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Strip the PDF font subset prefix (e.g., "ATNKJE+NimbusRomNo9L-Medi" → "NimbusRomNo9L-Medi").
fn strip_font_prefix(name: &str) -> &str {
    // PDF subset names: exactly 6 uppercase letters followed by '+'
    let bytes = name.as_bytes();
    if bytes.len() > 7 && bytes[6] == b'+' && bytes[..6].iter().all(|b| b.is_ascii_uppercase()) {
        &name[7..]
    } else {
        name
    }
}

/// Format a float with explicit decimal: always with one decimal place minimum.
/// e.g. 0.0 → "0.0", 14.0 → "14.0", 9.963 → "9.963"
fn legacy_float_str(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{:.1}", v)
    } else {
        // Trim trailing zeros but keep at least one decimal
        let s = format!("{}", v);
        s
    }
}

/// Format text color as array string.
/// Preserves original color space components (1=Gray, 3=RGB, 4=CMYK).
fn text_color_string(color: &Option<Vec<f64>>) -> String {
    match color {
        Some(components) if !components.is_empty() => {
            let parts: Vec<String> = components.iter().map(|v| legacy_float_str(*v)).collect();
            format!("[{}]", parts.join(", "))
        }
        _ => String::new(),
    }
}

/// Build a JSON array for [left_x, bottom_y, right_x, top_y].
fn bbox_array(bbox: &BoundingBox) -> Value {
    json!([bbox.left_x, bbox.bottom_y, bbox.right_x, bbox.top_y])
}

/// 1-based page number from a BoundingBox.
fn page_num(bbox: &BoundingBox) -> u32 {
    bbox.page_number.unwrap_or(1)
}

// ---------------------------------------------------------------------------
// Text extraction
// ---------------------------------------------------------------------------

/// Concatenated text from a SemanticTextNode (columns → blocks → lines → chunks).
fn node_text(node: &SemanticTextNode) -> String {
    node.value().trim().to_string()
}

/// Extract (font_name, font_size, text_color_str) from a SemanticTextNode.
/// Falls back to the first non-empty text chunk if the node's fields are None.
fn text_node_style(node: &SemanticTextNode) -> (String, f64, String) {
    // Try direct fields first
    let font_name = node.font_name.clone();
    let font_size = node.font_size;
    let text_color = &node.text_color;

    // If any is missing, extract from the first non-empty text chunk
    if font_name.is_none() || font_size.is_none() || text_color.is_none() {
        if let Some(first_chunk) = node
            .columns
            .iter()
            .flat_map(|c| c.text_blocks.iter())
            .flat_map(|b| b.text_lines.iter())
            .flat_map(|l| l.text_chunks.iter())
            .find(|c| !c.value.trim().is_empty())
        {
            let raw_font = font_name
                .as_deref()
                .unwrap_or(first_chunk.font_name.as_str());
            let resolved_font = strip_font_prefix(raw_font).to_string();
            let resolved_size = font_size.unwrap_or(first_chunk.font_size);
            let resolved_color = if text_color.is_some() {
                text_color_string(text_color)
            } else {
                format_font_color(&first_chunk.font_color)
            };
            return (resolved_font, resolved_size, resolved_color);
        }
    }

    (
        font_name
            .as_deref()
            .map(strip_font_prefix)
            .unwrap_or("")
            .to_string(),
        font_size.unwrap_or(0.0),
        text_color_string(text_color),
    )
}

/// Extract text from a ListItem.
/// First tries semantic contents (preferred), then falls back to raw body tokens.
fn list_item_text(item: &ListItem) -> String {
    if !item.contents.is_empty() {
        let parts: Vec<String> = item.contents.iter().filter_map(element_text_str).collect();
        if !parts.is_empty() {
            return parts.join(" ").trim().to_string();
        }
    }
    // Fallback: raw body content tokens
    table_tokens_text(&item.body.content)
}

/// Extract text from raw TableTokenRows.
fn table_tokens_text(rows: &[Vec<TableToken>]) -> String {
    rows.iter()
        .flat_map(|row| row.iter())
        .map(|t| t.base.value.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract text string from a ContentElement if it's a text-bearing type.
fn element_text_str(el: &ContentElement) -> Option<String> {
    match el {
        ContentElement::Paragraph(p) => Some(node_text(&p.base)),
        ContentElement::Heading(h) => Some(node_text(&h.base.base)),
        ContentElement::NumberHeading(nh) => Some(node_text(&nh.base.base.base)),
        ContentElement::Caption(c) => Some(node_text(&c.base)),
        // Raw text elements from list_detector and early pipeline stages
        ContentElement::TextLine(l) => {
            let s = l.value().trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        ContentElement::TextBlock(b) => {
            let s = b.value().trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        ContentElement::TextChunk(c) => {
            let s = c.value.trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => None,
    }
}

/// Extract (font_name, font_size, text_color) from the first text-bearing element.
fn list_item_style(item: &ListItem) -> (String, f64, String) {
    // Try semantic contents first
    for el in &item.contents {
        if let Some((font, size, color)) = element_style(el) {
            return (font, size, color);
        }
    }
    // Try body tokens first chunk
    let first_chunk = item.body.content.iter().flat_map(|row| row.iter()).next();
    if let Some(token) = first_chunk {
        let color_str = format_font_color(&token.base.font_color);
        return (
            strip_font_prefix(&token.base.font_name).to_string(),
            token.base.font_size,
            color_str,
        );
    }
    (String::new(), 0.0, "[0.0, 0.0, 0.0]".to_string())
}

/// Extract style from a ContentElement.
fn element_style(el: &ContentElement) -> Option<(String, f64, String)> {
    match el {
        ContentElement::Paragraph(p) => Some(text_node_style(&p.base)),
        ContentElement::Heading(h) => Some(text_node_style(&h.base.base)),
        ContentElement::NumberHeading(nh) => Some(text_node_style(&nh.base.base.base)),
        ContentElement::Caption(c) => Some(text_node_style(&c.base)),
        // Raw text elements from list_detector
        ContentElement::TextLine(l) => {
            if let Some(chunk) = l.text_chunks.iter().find(|c| !c.value.trim().is_empty()) {
                let font = strip_font_prefix(&chunk.font_name).to_string();
                let color = format_font_color(&chunk.font_color);
                Some((font, chunk.font_size, color))
            } else {
                None
            }
        }
        ContentElement::TextBlock(b) => {
            if let Some(chunk) = b
                .text_lines
                .iter()
                .flat_map(|l| l.text_chunks.iter())
                .find(|c| !c.value.trim().is_empty())
            {
                let font = strip_font_prefix(&chunk.font_name).to_string();
                let color = format_font_color(&chunk.font_color);
                Some((font, chunk.font_size, color))
            } else {
                None
            }
        }
        ContentElement::TextChunk(c) => {
            if !c.value.trim().is_empty() {
                let font = strip_font_prefix(&c.font_name).to_string();
                let color = format_font_color(&c.font_color);
                Some((font, c.font_size, color))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert a font color string to array notation.
/// Handles both new format "[0.0, 0.0, 0.0]" (pass-through) and legacy hex "#RRGGBB".
fn format_font_color(color: &str) -> String {
    // New format: already in array notation
    if color.starts_with('[') {
        return color.to_string();
    }
    // Legacy hex format
    let hex = color.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            let rf = r as f64 / 255.0;
            let gf = g as f64 / 255.0;
            let bf = b as f64 / 255.0;
            return text_color_string(&Some(vec![rf, gf, bf]));
        }
    }
    String::new()
}

// ---------------------------------------------------------------------------
// Heading level name
// ---------------------------------------------------------------------------

/// Map a heading level integer to a named level string.
fn heading_level_name(level: u32) -> &'static str {
    match level {
        1 => "Title",
        2 => "Subtitle",
        3 => "Heading1",
        4 => "Heading2",
        5 => "Heading3",
        _ => "Heading4",
    }
}

// ---------------------------------------------------------------------------
// Numbering style mapping
// ---------------------------------------------------------------------------

/// Map a raw numbering style token to a human-readable description.
fn numbering_style_label(raw: &str) -> &str {
    let r = raw.trim();
    if r.contains('•') || r.contains('-') || r.contains('*') || r == "bullet" || r == "–" {
        "bullets"
    } else if r.contains('a') || r.contains('A') || r == "letter" {
        "letters"
    } else if r.contains('i') || r.contains('I') || r == "roman" {
        "roman numerals"
    } else {
        "arabic numbers"
    }
}

// ---------------------------------------------------------------------------
// Element converters
// ---------------------------------------------------------------------------

fn paragraph_to_legacy(para: &SemanticParagraph) -> Value {
    let node = &para.base;
    let (font, font_size, color) = text_node_style(node);
    let mut obj = Map::new();
    obj.insert("type".into(), json!("paragraph"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&node.bbox)));
    obj.insert("bounding box".into(), bbox_array(&node.bbox));
    obj.insert("font".into(), json!(font));
    obj.insert("font size".into(), json!(font_size));
    obj.insert("text color".into(), json!(color));
    obj.insert("content".into(), json!(node_text(node)));
    Value::Object(obj)
}

fn heading_to_legacy(heading: &SemanticHeading) -> Value {
    let node = &heading.base.base;
    let (font, font_size, color) = text_node_style(node);
    let level_num = heading.heading_level.unwrap_or(3);
    let level_name = heading_level_name(level_num);
    let mut obj = Map::new();
    obj.insert("type".into(), json!("heading"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("level".into(), json!(level_name));
    obj.insert("page number".into(), json!(page_num(&node.bbox)));
    obj.insert("bounding box".into(), bbox_array(&node.bbox));
    obj.insert("heading level".into(), json!(level_num));
    obj.insert("font".into(), json!(font));
    obj.insert("font size".into(), json!(font_size));
    obj.insert("text color".into(), json!(color));
    obj.insert("content".into(), json!(node_text(node)));
    Value::Object(obj)
}

fn number_heading_to_legacy(nh: &SemanticNumberHeading) -> Value {
    heading_to_legacy(&nh.base)
}

fn caption_to_legacy(cap: &SemanticCaption) -> Value {
    let node = &cap.base;
    let (font, font_size, color) = text_node_style(node);
    let mut obj = Map::new();
    obj.insert("type".into(), json!("caption"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&node.bbox)));
    obj.insert("bounding box".into(), bbox_array(&node.bbox));
    if let Some(linked_id) = cap.linked_content_id {
        obj.insert("linked content id".into(), json!(linked_id));
    }
    obj.insert("font".into(), json!(font));
    obj.insert("font size".into(), json!(font_size));
    obj.insert("text color".into(), json!(color));
    obj.insert("content".into(), json!(node_text(node)));
    Value::Object(obj)
}

fn header_footer_to_legacy(hf: &SemanticHeaderOrFooter, stem: &str, img_idx: &mut u64) -> Value {
    let type_str = if hf.semantic_type == SemanticType::Header {
        "header"
    } else {
        "footer"
    };
    let kids: Vec<Value> = hf
        .contents
        .iter()
        .flat_map(|el| elements_to_legacy(el, stem, img_idx))
        .collect();
    let mut obj = Map::new();
    obj.insert("type".into(), json!(type_str));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&hf.bbox)));
    obj.insert("bounding box".into(), bbox_array(&hf.bbox));
    obj.insert("kids".into(), json!(kids));
    Value::Object(obj)
}

fn image_to_legacy(img: &ImageChunk, stem: &str, img_idx: &mut u64) -> Value {
    *img_idx += 1;
    let source = format!("{}_images/imageFile{}.png", stem, img_idx);
    let mut obj = Map::new();
    obj.insert("type".into(), json!("image"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&img.bbox)));
    obj.insert("bounding box".into(), bbox_array(&img.bbox));
    obj.insert("source".into(), json!(source));
    Value::Object(obj)
}

fn figure_to_legacy(fig: &SemanticFigure, stem: &str, img_idx: &mut u64) -> Vec<Value> {
    if fig.images.is_empty() {
        // No embedded images — emit a single image using the figure bbox
        *img_idx += 1;
        let source = format!("{}_images/imageFile{}.png", stem, img_idx);
        let mut obj = Map::new();
        obj.insert("type".into(), json!("image"));
        obj.insert("id".into(), json!(next_id()));
        obj.insert("page number".into(), json!(page_num(&fig.bbox)));
        obj.insert("bounding box".into(), bbox_array(&fig.bbox));
        obj.insert("source".into(), json!(source));
        vec![Value::Object(obj)]
    } else {
        // Each image chunk uses the figure's page bbox (since ImageChunk bbox is pixel-space)
        fig.images
            .iter()
            .map(|_img| {
                *img_idx += 1;
                let source = format!("{}_images/imageFile{}.png", stem, img_idx);
                let mut obj = Map::new();
                obj.insert("type".into(), json!("image"));
                obj.insert("id".into(), json!(next_id()));
                obj.insert("page number".into(), json!(page_num(&fig.bbox)));
                obj.insert("bounding box".into(), bbox_array(&fig.bbox));
                obj.insert("source".into(), json!(source));
                Value::Object(obj)
            })
            .collect()
    }
}

fn list_item_to_legacy(item: &ListItem, stem: &str, img_idx: &mut u64) -> Value {
    let (font, font_size, color) = list_item_style(item);
    let text = list_item_text(item);
    // Nested list items go in kids
    let kids: Vec<Value> = item
        .contents
        .iter()
        .filter(|e| matches!(e, ContentElement::List(_)))
        .flat_map(|el| elements_to_legacy(el, stem, img_idx))
        .collect();
    let mut obj = Map::new();
    obj.insert("type".into(), json!("list item"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&item.bbox)));
    obj.insert("bounding box".into(), bbox_array(&item.bbox));
    obj.insert("font".into(), json!(font));
    obj.insert("font size".into(), json!(font_size));
    obj.insert("text color".into(), json!(color));
    obj.insert("content".into(), json!(text));
    obj.insert("kids".into(), json!(kids));
    Value::Object(obj)
}

fn list_to_legacy(list: &PDFList, stem: &str, img_idx: &mut u64) -> Value {
    let numbering = list
        .numbering_style
        .as_deref()
        .map(numbering_style_label)
        .unwrap_or("arabic numbers");
    let num_items = list.list_items.len();
    let next_list_id_val = list.next_list_id.unwrap_or(0);
    let prev_list_id_val = list.previous_list_id.unwrap_or(0);
    // Use "1" as the default nesting level (convention for top-level lists)
    let level = "1".to_string();

    let list_items: Vec<Value> = list
        .list_items
        .iter()
        .map(|item| list_item_to_legacy(item, stem, img_idx))
        .collect();

    let mut obj = Map::new();
    obj.insert("type".into(), json!("list"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("level".into(), json!(level));
    obj.insert("page number".into(), json!(page_num(&list.bbox)));
    obj.insert("bounding box".into(), bbox_array(&list.bbox));
    obj.insert("numbering style".into(), json!(numbering));
    obj.insert("number of list items".into(), json!(num_items));
    obj.insert("next list id".into(), json!(next_list_id_val));
    obj.insert("previous list id".into(), json!(prev_list_id_val));
    obj.insert("list items".into(), json!(list_items));
    Value::Object(obj)
}

/// Build a paragraph node from a border-detected cell's raw text tokens.
///
/// Returns `None` when the cell has no textual tokens.
fn table_cell_tokens_to_paragraph(cell: &TableBorderCell) -> Option<Value> {
    let text = cell
        .content
        .iter()
        .map(|t| t.base.value.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        return None;
    }

    let mut para = Map::new();
    para.insert("type".into(), json!("paragraph"));
    para.insert("id".into(), json!(next_id()));
    para.insert("page number".into(), json!(page_num(&cell.bbox)));
    para.insert("bounding box".into(), bbox_array(&cell.bbox));
    if let Some(tok) = cell.content.iter().find(|t| !t.base.value.trim().is_empty()) {
        para.insert(
            "font".into(),
            json!(strip_font_prefix(&tok.base.font_name)),
        );
        para.insert("font size".into(), json!(tok.base.font_size));
        para.insert(
            "text color".into(),
            json!(format_font_color(&tok.base.font_color)),
        );
    }
    para.insert("content".into(), json!(text));
    Some(Value::Object(para))
}

fn table_cell_to_legacy(cell: &TableBorderCell, stem: &str, img_idx: &mut u64) -> Value {
    let mut kids: Vec<Value> = cell
        .contents
        .iter()
        .flat_map(|el| elements_to_legacy(el, stem, img_idx))
        .collect();

    // Fallback: cells populated by the table border-detection path store their
    // text in `content` (raw TableTokens), not `contents` (semantic elements),
    // and raw tokens are not emitted by `elements_to_legacy`. Without this, such
    // cells serialize with empty `kids` even though the text is present and
    // rendered correctly in the Markdown/HTML/text outputs. Synthesize a
    // paragraph kid from the token text so JSON matches the other formats.
    if kids.is_empty() {
        if let Some(kid) = table_cell_tokens_to_paragraph(cell) {
            kids.push(kid);
        }
    }

    let mut obj = Map::new();
    obj.insert("type".into(), json!("table cell"));
    obj.insert("page number".into(), json!(page_num(&cell.bbox)));
    obj.insert("bounding box".into(), bbox_array(&cell.bbox));
    obj.insert("row number".into(), json!(cell.row_number + 1));
    obj.insert("column number".into(), json!(cell.col_number + 1));
    obj.insert("row span".into(), json!(cell.row_span));
    obj.insert("column span".into(), json!(cell.col_span));
    obj.insert("kids".into(), json!(kids));
    Value::Object(obj)
}

fn table_row_to_legacy(row: &TableBorderRow, stem: &str, img_idx: &mut u64) -> Value {
    let cells: Vec<Value> = row
        .cells
        .iter()
        .map(|cell| table_cell_to_legacy(cell, stem, img_idx))
        .collect();
    let mut obj = Map::new();
    obj.insert("type".into(), json!("table row"));
    obj.insert("row number".into(), json!(row.row_number + 1));
    obj.insert("cells".into(), json!(cells));
    Value::Object(obj)
}

/// Check whether a detected TableBorder is likely a false positive.
///
/// The reference table detector uses tagged-PDF structure, so it
/// avoids outputting container rectangles, thin rule lines, and other
/// geometric noise that the geometric border detector picks up.  We apply
/// three conservative heuristic filters to match the reference behaviour:
///
/// 1. **Large single-cell table**: a 1×1 grid whose bounding box is much
///    bigger than a typical checkbox marker (≈15×8 pt) is a container rect.
/// 2. **Small multi-cell table**: a multi-cell grid whose height is < 30 pt
///    is almost certainly noise inside a figure or code block.
/// 3. **Thin horizontal strip**: width/height ratio > 8 with height < 25 pt
///    indicates a horizontal rule that accidentally forms a closed rectangle.
fn is_false_positive_table(tb: &TableBorder) -> bool {
    let width = tb.bbox.right_x - tb.bbox.left_x;
    let height = tb.bbox.top_y - tb.bbox.bottom_y;
    let num_cells = tb.num_rows * tb.num_columns;

    // Rule 1: Large single-cell container
    if num_cells == 1 && (width > 25.0 || height > 14.5) {
        return true;
    }
    // Rule 2: Small multi-cell table (noise from figures/code blocks)
    if num_cells > 1 && height < 30.0 {
        return true;
    }
    // Rule 3: Thin horizontal strip (horizontal rule detected as table)
    if height > 0.0 && width / height > 8.0 && height < 25.0 {
        return true;
    }
    false
}

fn table_border_to_legacy(tb: &TableBorder, stem: &str, img_idx: &mut u64) -> Option<Value> {
    if is_false_positive_table(tb) {
        return None;
    }

    let level = tb.level.clone().unwrap_or_else(|| "0".to_string());
    let next_table_id: u64 = 0; // Rust doesn't have next_table_id linkage in schema
    let rows: Vec<Value> = tb
        .rows
        .iter()
        .map(|row| table_row_to_legacy(row, stem, img_idx))
        .collect();

    let mut obj = Map::new();
    obj.insert("type".into(), json!("table"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("level".into(), json!(level));
    obj.insert("page number".into(), json!(page_num(&tb.bbox)));
    obj.insert("bounding box".into(), bbox_array(&tb.bbox));
    obj.insert("number of rows".into(), json!(tb.num_rows));
    obj.insert("number of columns".into(), json!(tb.num_columns));
    obj.insert("next table id".into(), json!(next_table_id));
    obj.insert("rows".into(), json!(rows));
    Some(Value::Object(obj))
}

fn semantic_table_to_legacy(st: &SemanticTable, stem: &str, img_idx: &mut u64) -> Option<Value> {
    table_border_to_legacy(&st.table_border, stem, img_idx)
}

/// Convert a Paragraph marked as Caption to legacy JSON caption format.
fn paragraph_as_caption_to_legacy(para: &SemanticParagraph) -> Value {
    let node = &para.base;
    let (font, font_size, color) = text_node_style(node);
    let mut obj = Map::new();
    obj.insert("type".into(), json!("caption"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&node.bbox)));
    obj.insert("bounding box".into(), bbox_array(&node.bbox));
    obj.insert("font".into(), json!(font));
    obj.insert("font size".into(), json!(font_size));
    obj.insert("text color".into(), json!(color));
    obj.insert("content".into(), json!(node_text(node)));
    Value::Object(obj)
}

/// Convert a Heading marked as Caption to legacy JSON caption format.
fn heading_as_caption_to_legacy(heading: &SemanticHeading) -> Value {
    let node = &heading.base.base;
    let (font, font_size, color) = text_node_style(node);
    let mut obj = Map::new();
    obj.insert("type".into(), json!("caption"));
    obj.insert("id".into(), json!(next_id()));
    obj.insert("page number".into(), json!(page_num(&node.bbox)));
    obj.insert("bounding box".into(), bbox_array(&node.bbox));
    obj.insert("font".into(), json!(font));
    obj.insert("font size".into(), json!(font_size));
    obj.insert("text color".into(), json!(color));
    obj.insert("content".into(), json!(node_text(node)));
    Value::Object(obj)
}

// ---------------------------------------------------------------------------
// Dispatcher: ContentElement → Vec<Value>
// ---------------------------------------------------------------------------

/// Convert a single ContentElement to zero or more legacy JSON values.
/// Returns a Vec because a Figure may expand to multiple images.
fn elements_to_legacy(el: &ContentElement, stem: &str, img_idx: &mut u64) -> Vec<Value> {
    match el {
        ContentElement::Paragraph(p) => {
            // Skip empty/whitespace-only paragraphs (never emitted in legacy output).
            if p.base.is_empty() {
                return vec![];
            }
            // Check if this paragraph has been marked as a Caption by caption_linker
            if p.base.semantic_type == SemanticType::Caption {
                vec![paragraph_as_caption_to_legacy(p)]
            } else {
                vec![paragraph_to_legacy(p)]
            }
        }
        ContentElement::Heading(h) => {
            // Skip empty/whitespace-only headings.
            if h.base.base.is_empty() {
                return vec![];
            }
            // Check if heading was marked as Caption
            if h.base.base.semantic_type == SemanticType::Caption {
                vec![heading_as_caption_to_legacy(h)]
            } else {
                vec![heading_to_legacy(h)]
            }
        }
        ContentElement::NumberHeading(nh) => vec![number_heading_to_legacy(nh)],
        ContentElement::Caption(c) => vec![caption_to_legacy(c)],
        ContentElement::HeaderFooter(hf) => vec![header_footer_to_legacy(hf, stem, img_idx)],
        ContentElement::Figure(fig) => figure_to_legacy(fig, stem, img_idx),
        ContentElement::Image(img) => vec![image_to_legacy(img, stem, img_idx)],
        ContentElement::List(l) => vec![list_to_legacy(l, stem, img_idx)],
        ContentElement::Table(st) => semantic_table_to_legacy(st, stem, img_idx)
            .into_iter()
            .collect(),
        ContentElement::TableBorder(tb) => table_border_to_legacy(tb, stem, img_idx)
            .into_iter()
            .collect(),
        // Skip raw low-level elements not exposed in legacy output
        ContentElement::TextChunk(_)
        | ContentElement::TextLine(_)
        | ContentElement::TextBlock(_)
        | ContentElement::Line(_)
        | ContentElement::LineArt(_)
        | ContentElement::Formula(_)
        | ContentElement::Picture(_) => vec![],
    }
}

// ---------------------------------------------------------------------------
// Top-level serializer
// ---------------------------------------------------------------------------

/// Convert a PdfDocument to a legacy-schema JSON `Value`.
///
/// `stem` is the PDF file stem (without extension), used for constructing
/// image source paths like `"{stem}_images/imageFile1.png"`.
pub fn to_legacy_json_value(doc: &PdfDocument, stem: &str) -> Value {
    reset_ids();
    let mut img_idx: u64 = 0;

    let kids: Vec<Value> = doc
        .kids
        .iter()
        .flat_map(|el| elements_to_legacy(el, stem, &mut img_idx))
        .collect();

    let mut obj = Map::new();
    obj.insert("file name".into(), json!(doc.file_name));
    obj.insert("number of pages".into(), json!(doc.number_of_pages));
    obj.insert(
        "author".into(),
        doc.author.as_deref().map_or(Value::Null, |s| json!(s)),
    );
    obj.insert(
        "title".into(),
        doc.title.as_deref().map_or(Value::Null, |s| json!(s)),
    );
    obj.insert(
        "creation date".into(),
        doc.creation_date
            .as_deref()
            .map_or(Value::Null, |s| json!(s)),
    );
    obj.insert(
        "modification date".into(),
        doc.modification_date
            .as_deref()
            .map_or(Value::Null, |s| json!(s)),
    );
    obj.insert("kids".into(), json!(kids));

    Value::Object(obj)
}

/// Serialize a PdfDocument to a legacy-compatible JSON string.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` if serialization fails.
pub fn to_legacy_json_string(doc: &PdfDocument, stem: &str) -> Result<String, EdgePdfError> {
    let value = to_legacy_json_value(doc, stem);
    serde_json::to_string_pretty(&value)
        .map_err(|e| EdgePdfError::OutputError(format!("Legacy JSON serialization failed: {}", e)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::enums::SemanticType;
    use crate::models::semantic::{SemanticParagraph, SemanticTextNode};
    use crate::models::text::TextColumn;

    fn make_bbox(page: u32, left: f64, bottom: f64, right: f64, top: f64) -> BoundingBox {
        BoundingBox::new(Some(page), left, bottom, right, top)
    }

    fn make_text_node(bbox: BoundingBox, text: &str) -> SemanticTextNode {
        use crate::models::chunks::TextChunk;
        use crate::models::enums::{PdfLayer, TextFormat, TextType};
        use crate::models::text::{TextBlock, TextLine};
        let chunk = TextChunk {
            value: text.to_string(),
            bbox: bbox.clone(),
            font_name: "TestFont".to_string(),
            font_size: 12.0,
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
            font_size: 12.0,
            base_line: 2.0,
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
            base_line: 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_lines: vec![line],
            has_start_line: true,
            has_end_line: true,
            text_alignment: None,
        };
        let col = TextColumn {
            bbox: bbox.clone(),
            index: None,
            level: None,
            font_size: 12.0,
            base_line: 2.0,
            slant_degree: 0.0,
            is_hidden_text: false,
            text_blocks: vec![block],
        };
        SemanticTextNode {
            bbox,
            index: None,
            level: None,
            semantic_type: SemanticType::Paragraph,
            correct_semantic_score: None,
            columns: vec![col],
            font_weight: Some(400.0),
            font_size: Some(12.0),
            text_color: Some(vec![0.0, 0.0, 0.0]),
            italic_angle: None,
            font_name: Some("TestFont".to_string()),
            text_format: None,
            max_font_size: None,
            background_color: None,
            is_hidden_text: false,
        }
    }

    #[test]
    fn test_empty_document() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let json = to_legacy_json_string(&doc, "test").unwrap();
        assert!(json.contains("\"file name\""));
        assert!(json.contains("\"number of pages\""));
        assert!(json.contains("\"kids\""));
        assert!(!json.contains("number_of_pages"));
    }

    #[test]
    fn test_paragraph_has_legacy_keys() {
        let bbox = make_bbox(1, 54.0, 100.0, 300.0, 120.0);
        let node = make_text_node(bbox, "Hello world");
        let para = SemanticParagraph {
            base: node,
            enclosed_top: false,
            enclosed_bottom: false,
            indentation: 0,
        };
        let val = paragraph_to_legacy(&para);
        let s = serde_json::to_string_pretty(&val).unwrap();
        assert!(s.contains("\"type\""));
        assert!(s.contains("\"page number\""));
        assert!(s.contains("\"bounding box\""));
        assert!(s.contains("\"font size\""));
        assert!(s.contains("\"text color\""));
        assert!(s.contains("\"content\""));
        assert!(s.contains("\"paragraph\""));
        assert!(s.contains("[0.0, 0.0, 0.0]"));
    }

    #[test]
    fn test_text_color_grayscale() {
        assert_eq!(
            text_color_string(&Some(vec![0.0, 0.0, 0.0])),
            "[0.0, 0.0, 0.0]"
        );
        assert_eq!(
            text_color_string(&Some(vec![1.0, 1.0, 1.0])),
            "[1.0, 1.0, 1.0]"
        );
        assert_eq!(text_color_string(&None), "");
    }

    #[test]
    fn test_text_color_rgb() {
        let result = text_color_string(&Some(vec![1.0, 0.0, 0.0]));
        assert!(result.contains("1.0") && result.contains("0.0"));
        assert!(result.starts_with('[') && result.ends_with(']'));
    }

    #[test]
    fn test_bbox_array_order() {
        let bbox = make_bbox(1, 10.0, 20.0, 300.0, 400.0);
        let arr = bbox_array(&bbox);
        if let Value::Array(v) = arr {
            // [left_x, bottom_y, right_x, top_y]
            assert_eq!(v[0].as_f64().unwrap(), 10.0);
            assert_eq!(v[1].as_f64().unwrap(), 20.0);
            assert_eq!(v[2].as_f64().unwrap(), 300.0);
            assert_eq!(v[3].as_f64().unwrap(), 400.0);
        } else {
            panic!("Expected array");
        }
    }
}
