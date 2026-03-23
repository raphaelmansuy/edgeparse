//! Text extraction from PDF content streams.
//!
//! Walks content stream operations and produces TextChunks with position,
//! font, and Unicode text information.

use lopdf::{content::Content, Document, Object};

use crate::models::bbox::BoundingBox;
use crate::models::chunks::TextChunk;
use crate::EdgePdfError;

use super::font::{resolve_page_fonts, FontCache, PdfFont};
use super::graphics_state::GraphicsStateStack;

/// Extract text chunks from a single page.
///
/// Returns a vector of positioned text chunks with font and bounding box info.
pub fn extract_text_chunks(
    doc: &Document,
    page_number: u32,
    page_id: lopdf::ObjectId,
) -> Result<Vec<TextChunk>, EdgePdfError> {
    let font_cache = resolve_page_fonts(doc, page_id);
    let page_dict = doc
        .get_object(page_id)
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Failed to get page {}: {}", page_number, e),
        })?
        .as_dict()
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Page {} is not a dictionary: {}", page_number, e),
        })?
        .clone();

    // Get MediaBox for page dimensions
    let media_box = get_media_box(doc, &page_dict);

    // Get content stream(s)
    let content_data = get_page_content(doc, &page_dict)?;
    if content_data.is_empty() {
        return Ok(Vec::new());
    }

    // Parse content stream operations
    let content = Content::decode(&content_data).map_err(|e| EdgePdfError::PipelineError {
        stage: 1,
        message: format!(
            "Failed to decode content stream for page {}: {}",
            page_number, e
        ),
    })?;

    // Process operations
    let chunks = process_operations(&content.operations, &font_cache, page_number, &media_box);

    Ok(chunks)
}

/// Get page content stream data.
pub(crate) fn get_page_content(
    doc: &Document,
    page_dict: &lopdf::Dictionary,
) -> Result<Vec<u8>, EdgePdfError> {
    let contents = match page_dict.get(b"Contents") {
        Ok(c) => c.clone(),
        Err(_) => return Ok(Vec::new()),
    };

    // Helper: collect stream bytes from an array of items (each item may be a
    // Reference-to-Stream or a direct Stream).
    fn collect_array(doc: &Document, arr: &[Object]) -> Result<Vec<u8>, EdgePdfError> {
        let mut data = Vec::new();
        for item in arr {
            let obj = match item {
                Object::Reference(id) => {
                    doc.get_object(*id)
                        .map_err(|e| EdgePdfError::PipelineError {
                            stage: 1,
                            message: format!("Failed to resolve content array item: {}", e),
                        })?
                }
                other => other,
            };
            if let Object::Stream(ref stream) = obj {
                if let Ok(content) = get_stream_data(stream) {
                    data.extend_from_slice(&content);
                    data.push(b' '); // Separate stream contents
                }
            }
        }
        Ok(data)
    }

    match contents {
        Object::Reference(id) => {
            let obj = doc
                .get_object(id)
                .map_err(|e| EdgePdfError::PipelineError {
                    stage: 1,
                    message: format!("Failed to get content object: {}", e),
                })?;
            match obj {
                // Most common: Contents → single stream
                Object::Stream(ref stream) => get_stream_data(stream),
                // Also valid: Contents → Reference → Array of stream references
                Object::Array(ref arr) => collect_array(doc, arr),
                _ => Ok(Vec::new()),
            }
        }
        Object::Array(ref arr) => collect_array(doc, arr),
        _ => Ok(Vec::new()),
    }
}

/// Get data from a stream, handling both compressed and uncompressed cases.
fn get_stream_data(stream: &lopdf::Stream) -> Result<Vec<u8>, EdgePdfError> {
    // If no Filter key, the content is already uncompressed
    if stream.dict.get(b"Filter").is_err() {
        return Ok(stream.content.clone());
    }
    stream
        .decompressed_content()
        .map_err(|e| EdgePdfError::PipelineError {
            stage: 1,
            message: format!("Failed to decompress content stream: {}", e),
        })
}

/// Get page MediaBox.
fn get_media_box(doc: &Document, page_dict: &lopdf::Dictionary) -> BoundingBox {
    if let Ok(mb) = page_dict.get(b"MediaBox") {
        if let Ok(arr) = resolve_obj(doc, mb).as_array() {
            if arr.len() == 4 {
                let vals: Vec<f64> = arr
                    .iter()
                    .filter_map(|o| obj_to_f64(resolve_obj(doc, o)))
                    .collect();
                if vals.len() == 4 {
                    return BoundingBox::new(None, vals[0], vals[1], vals[2], vals[3]);
                }
            }
        }
    }
    // Default A4 page
    BoundingBox::new(None, 0.0, 0.0, 595.0, 842.0)
}

/// Process content stream operations and extract text chunks.
fn process_operations(
    operations: &[lopdf::content::Operation],
    font_cache: &FontCache,
    page_number: u32,
    _media_box: &BoundingBox,
) -> Vec<TextChunk> {
    let mut chunks = Vec::new();
    let mut state = GraphicsStateStack::default();
    let mut chunk_index: usize = 0;
    // Track marked content sequence IDs (BDC/BMC/EMC)
    let mut mcid_stack: Vec<Option<i64>> = Vec::new();

    for op in operations {
        match op.operator.as_str() {
            // Marked content operators — track MCID for structure tree linkage
            "BMC" => {
                // Begin Marked Content (no properties) — push None
                mcid_stack.push(None);
            }
            "BDC" => {
                // Begin Marked Content with properties — extract MCID if present
                let mcid = extract_mcid_from_bdc(&op.operands);
                mcid_stack.push(mcid);
            }
            "EMC" => {
                // End Marked Content — pop the stack
                mcid_stack.pop();
            }

            // Graphics state operators
            "q" => state.save(),
            "Q" => state.restore(),
            "cm" => {
                if op.operands.len() == 6 {
                    let vals: Vec<f64> = op
                        .operands
                        .iter()
                        .filter_map(|o| obj_to_f64(o.clone()))
                        .collect();
                    if vals.len() == 6 {
                        state.concat_ctm(vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]);
                    }
                }
            }

            // Text state operators
            "BT" => state.current.begin_text(),
            "ET" => {} // End text object

            "Tf" => {
                // Set font and size
                if op.operands.len() == 2 {
                    if let Object::Name(ref name) = op.operands[0] {
                        state.current.text_state.font_name =
                            String::from_utf8_lossy(name).to_string();
                    }
                    if let Some(size) = obj_to_f64(op.operands[1].clone()) {
                        state.current.text_state.font_size = size;
                    }
                }
            }

            "Tc" => {
                if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.text_state.char_spacing = v;
                }
            }

            "Tw" => {
                if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.text_state.word_spacing = v;
                }
            }

            "Tz" => {
                if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.text_state.horizontal_scaling = v;
                }
            }

            "TL" => {
                if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.text_state.leading = v;
                }
            }

            "Ts" => {
                if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.text_state.rise = v;
                }
            }

            "Tr" => {
                if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.text_state.render_mode = v as i32;
                }
            }

            // Text positioning operators
            "Td" => {
                if op.operands.len() == 2 {
                    let tx = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                    let ty = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                    state.current.translate_text(tx, ty);
                }
            }

            "TD" => {
                // Same as Td but also sets leading
                if op.operands.len() == 2 {
                    let tx = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                    let ty = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                    state.current.text_state.leading = -ty;
                    state.current.translate_text(tx, ty);
                }
            }

            "Tm" => {
                if op.operands.len() == 6 {
                    let vals: Vec<f64> = op
                        .operands
                        .iter()
                        .filter_map(|o| obj_to_f64(o.clone()))
                        .collect();
                    if vals.len() == 6 {
                        state
                            .current
                            .set_text_matrix(vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]);
                    }
                }
            }

            "T*" => {
                state.current.next_line();
            }

            // Text showing operators
            "Tj" => {
                if let Some(text_bytes) = op.operands.first().and_then(extract_string_bytes) {
                    let font = font_cache
                        .get(&state.current.text_state.font_name)
                        .cloned()
                        .unwrap_or_else(|| {
                            PdfFont::default_font(&state.current.text_state.font_name)
                        });
                    let active_mcid = active_mcid(&mcid_stack);

                    if let Some(chunk) = create_text_chunk(
                        &text_bytes,
                        &font,
                        &mut state,
                        page_number,
                        &mut chunk_index,
                        active_mcid,
                    ) {
                        chunks.push(chunk);
                    }
                }
            }

            "TJ" => {
                // Array of strings and positioning adjustments
                if let Some(Object::Array(ref arr)) = op.operands.first() {
                    let font = font_cache
                        .get(&state.current.text_state.font_name)
                        .cloned()
                        .unwrap_or_else(|| {
                            PdfFont::default_font(&state.current.text_state.font_name)
                        });
                    let active_mcid = active_mcid(&mcid_stack);

                    for item in arr {
                        match item {
                            Object::String(bytes, _) => {
                                if let Some(chunk) = create_text_chunk(
                                    bytes,
                                    &font,
                                    &mut state,
                                    page_number,
                                    &mut chunk_index,
                                    active_mcid,
                                ) {
                                    chunks.push(chunk);
                                }
                            }
                            _ => {
                                // Numeric adjustment: displacement in thousandths of text space
                                if let Some(adj) = obj_to_f64(item.clone()) {
                                    let displacement =
                                        -adj / 1000.0 * state.current.text_state.font_size;
                                    state.current.advance_text(displacement);
                                }
                            }
                        }
                    }
                }
            }

            "'" => {
                // Move to next line, show text
                state.current.next_line();
                if let Some(text_bytes) = op.operands.first().and_then(extract_string_bytes) {
                    let font = font_cache
                        .get(&state.current.text_state.font_name)
                        .cloned()
                        .unwrap_or_else(|| {
                            PdfFont::default_font(&state.current.text_state.font_name)
                        });
                    let active_mcid = active_mcid(&mcid_stack);
                    if let Some(chunk) = create_text_chunk(
                        &text_bytes,
                        &font,
                        &mut state,
                        page_number,
                        &mut chunk_index,
                        active_mcid,
                    ) {
                        chunks.push(chunk);
                    }
                }
            }

            "\"" => {
                // Set spacing, move to next line, show text
                if op.operands.len() == 3 {
                    if let Some(aw) = obj_to_f64(op.operands[0].clone()) {
                        state.current.text_state.word_spacing = aw;
                    }
                    if let Some(ac) = obj_to_f64(op.operands[1].clone()) {
                        state.current.text_state.char_spacing = ac;
                    }
                    state.current.next_line();
                    if let Some(text_bytes) = extract_string_bytes(&op.operands[2]) {
                        let font = font_cache
                            .get(&state.current.text_state.font_name)
                            .cloned()
                            .unwrap_or_else(|| {
                                PdfFont::default_font(&state.current.text_state.font_name)
                            });
                        let active_mcid = active_mcid(&mcid_stack);
                        if let Some(chunk) = create_text_chunk(
                            &text_bytes,
                            &font,
                            &mut state,
                            page_number,
                            &mut chunk_index,
                            active_mcid,
                        ) {
                            chunks.push(chunk);
                        }
                    }
                }
            }

            // Color operators — preserve original color space components (reference approach)
            "g" => {
                if let Some(gray) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.fill_color = vec![gray];
                    state.current.fill_color_space_components = 1;
                }
            }
            "G" => {
                if let Some(gray) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                    state.current.stroke_color = vec![gray];
                    state.current.stroke_color_space_components = 1;
                }
            }
            "rg" => {
                if op.operands.len() == 3 {
                    let r = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                    let g = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                    let b = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                    state.current.fill_color = vec![r, g, b];
                    state.current.fill_color_space_components = 3;
                }
            }
            "RG" => {
                if op.operands.len() == 3 {
                    let r = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                    let g = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                    let b = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                    state.current.stroke_color = vec![r, g, b];
                    state.current.stroke_color_space_components = 3;
                }
            }
            "k" => {
                if op.operands.len() == 4 {
                    let c = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                    let m = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                    let y = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                    let k = obj_to_f64(op.operands[3].clone()).unwrap_or(0.0);
                    state.current.fill_color = vec![c, m, y, k];
                    state.current.fill_color_space_components = 4;
                }
            }
            "K" => {
                if op.operands.len() == 4 {
                    let c = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                    let m = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                    let y = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                    let k = obj_to_f64(op.operands[3].clone()).unwrap_or(0.0);
                    state.current.stroke_color = vec![c, m, y, k];
                    state.current.stroke_color_space_components = 4;
                }
            }
            // cs/CS — set color space; sc/SC/scn/SCN — set color in current space
            "cs" => {
                if let Some(name) = op.operands.first() {
                    let cs_name = obj_to_name(name);
                    state.current.fill_color_space_components = color_space_components(&cs_name);
                }
            }
            "CS" => {
                if let Some(name) = op.operands.first() {
                    let cs_name = obj_to_name(name);
                    state.current.stroke_color_space_components = color_space_components(&cs_name);
                }
            }
            "sc" | "scn" => {
                let components: Vec<f64> = op
                    .operands
                    .iter()
                    .filter_map(|o| obj_to_f64(o.clone()))
                    .collect();
                if !components.is_empty() {
                    state.current.fill_color = components;
                }
            }
            "SC" | "SCN" => {
                let components: Vec<f64> = op
                    .operands
                    .iter()
                    .filter_map(|o| obj_to_f64(o.clone()))
                    .collect();
                if !components.is_empty() {
                    state.current.stroke_color = components;
                }
            }

            _ => {
                // Ignore unknown operators
            }
        }
    }

    chunks
}

/// Create a TextChunk from raw text bytes using the current graphics state.
fn create_text_chunk(
    text_bytes: &[u8],
    font: &PdfFont,
    state: &mut GraphicsStateStack,
    page_number: u32,
    chunk_index: &mut usize,
    mcid: Option<i64>,
) -> Option<TextChunk> {
    if text_bytes.is_empty() {
        return None;
    }

    // Get text position before rendering
    let trm = state.current.text_rendering_matrix();
    let start_x = trm.e;
    let font_size = trm.font_size_factor();

    if font_size < 0.1 {
        return None; // Skip invisible text
    }

    // Decode text to Unicode
    let mut text = String::new();
    let mut total_width = 0.0;
    let mut symbol_ends = Vec::new();

    let bpc = font.bytes_per_code as usize;
    let mut pos = 0;
    while pos + bpc <= text_bytes.len() {
        let char_code = if bpc == 2 {
            ((text_bytes[pos] as u32) << 8) | (text_bytes[pos + 1] as u32)
        } else {
            text_bytes[pos] as u32
        };
        pos += bpc;

        let decoded = font.decode_char(char_code);
        text.push_str(&decoded);

        // Calculate glyph width
        let glyph_w = font.glyph_width(char_code) / 1000.0;
        total_width += glyph_w;
        symbol_ends.push(start_x + total_width * font_size);

        // Add character spacing
        total_width += state.current.text_state.char_spacing / state.current.text_state.font_size;

        // Add word spacing for space character
        if decoded == " " {
            total_width +=
                state.current.text_state.word_spacing / state.current.text_state.font_size;
        }
    }

    // Advance text position
    let displacement = total_width * state.current.text_state.font_size;
    state.current.advance_text(displacement);

    if text.is_empty() {
        return None;
    }

    // Compute TRM_after (text rendering matrix after text advancement)
    let trm_after = state.current.text_rendering_matrix();

    // Use font ascent/descent from font descriptor (glyph-space units, per-mille).
    // The reference implementation: TextChunksHelper.calculateTextBoundingBox uses font.getAscent()/getDescent()
    // with fallback to font bounding box.
    let ascent = font.ascent;
    let descent = font.descent;

    // TRM matrix components:
    //   a = scaleX,  b = shearY
    //   c = shearX,  d = scaleY
    //   e = translateX, f = translateY
    let trm_before = &trm; // TRM at start of text

    // The reference bbox formula with 4 branches based on text direction/orientation.
    // scaleX = trm.a, shearX = trm.c, scaleY = trm.d, shearY = trm.b
    let (x1, x2) = if trm_before.a >= 0.0 && trm_before.c >= 0.0 {
        (
            trm_before.e + descent * trm_before.c / 1000.0,
            trm_after.e + ascent * trm_after.c / 1000.0,
        )
    } else if trm_before.a < 0.0 && trm_before.c < 0.0 {
        (
            trm_after.e + ascent * trm_after.c / 1000.0,
            trm_before.e + descent * trm_before.c / 1000.0,
        )
    } else if trm_before.a >= 0.0 {
        (
            trm_before.e + ascent * trm_before.c / 1000.0,
            trm_after.e + descent * trm_after.c / 1000.0,
        )
    } else {
        (
            trm_after.e + descent * trm_after.c / 1000.0,
            trm_before.e + ascent * trm_before.c / 1000.0,
        )
    };

    let (y1, y2) = if trm_before.d >= 0.0 && trm_before.b >= 0.0 {
        (
            trm_before.f + descent * trm_before.d / 1000.0,
            trm_after.f + ascent * trm_after.d / 1000.0,
        )
    } else if trm_before.d < 0.0 && trm_before.b < 0.0 {
        (
            trm_after.f + ascent * trm_after.d / 1000.0,
            trm_before.f + descent * trm_before.d / 1000.0,
        )
    } else if trm_before.d >= 0.0 {
        (
            trm_after.f + descent * trm_after.d / 1000.0,
            trm_before.f + ascent * trm_before.d / 1000.0,
        )
    } else {
        (
            trm_before.f + ascent * trm_before.d / 1000.0,
            trm_after.f + descent * trm_after.d / 1000.0,
        )
    };

    let bbox = BoundingBox::new(Some(page_number), x1, y1, x2, y2);

    // Determine text format from text rise (Ts operator).
    let text_format = if state.current.text_state.rise > font_size * 0.1 {
        crate::models::enums::TextFormat::Superscript
    } else if state.current.text_state.rise < -font_size * 0.1 {
        crate::models::enums::TextFormat::Subscript
    } else {
        crate::models::enums::TextFormat::Normal
    };

    *chunk_index += 1;

    // Format fill color as the reference Arrays.toString() — preserves original color space.
    // The reference veraPDF stores colors as the reference implementation float (f32), then serializes via double's toString(),
    // giving full f64 representation of the f32 value. We replicate:
    // parse as f64 (from lopdf) → round to f32 → back to f64 for full-precision display.
    let fc = &state.current.fill_color;
    let font_color = format!(
        "[{}]",
        fc.iter()
            .map(|v| {
                let f32_val = *v as f32;
                let f64_repr = f32_val as f64;
                if f32_val.fract() == 0.0 {
                    format!("{:.1}", f64_repr)
                } else {
                    format!("{}", f64_repr)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    );

    Some(TextChunk {
        value: text,
        bbox,
        font_name: font.base_font.clone(),
        font_size,
        font_weight: font.weight,
        italic_angle: font.italic_angle,
        font_color,
        contrast_ratio: 21.0, // Default: black on white = max contrast
        symbol_ends,
        text_format,
        text_type: crate::models::enums::TextType::Regular,
        pdf_layer: crate::models::enums::PdfLayer::Main,
        ocg_visible: true,
        index: Some(*chunk_index),
        page_number: Some(page_number),
        level: None,
        mcid,
    })
}

/// Extract string bytes from a PDF Object.
fn extract_string_bytes(obj: &Object) -> Option<Vec<u8>> {
    match obj {
        Object::String(bytes, _) => Some(bytes.clone()),
        _ => None,
    }
}

/// Get the currently active MCID from the marked content stack.
/// Returns the most recent non-None MCID (innermost BDC with MCID).
fn active_mcid(stack: &[Option<i64>]) -> Option<i64> {
    stack.iter().rev().find_map(|&mcid| mcid)
}

/// Extract the MCID from BDC operands.
/// BDC can appear as: `/Tag <</MCID 0>>` or `/Tag /PropertyListName`
/// We handle the inline dictionary case (most common in tagged PDFs).
fn extract_mcid_from_bdc(operands: &[Object]) -> Option<i64> {
    // BDC has 2 operands: tag name and properties
    if operands.len() < 2 {
        return None;
    }
    match &operands[1] {
        Object::Dictionary(dict) => {
            if let Ok(Object::Integer(n)) = dict.get(b"MCID") {
                return Some(*n);
            }
            None
        }
        _ => None,
    }
}

/// Convert PDF object to f64.
fn obj_to_f64(obj: Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(i as f64),
        Object::Real(f) => Some(f),
        _ => None,
    }
}

/// Extract a name string from a PDF object (Name type).
fn obj_to_name(obj: &Object) -> String {
    match obj {
        Object::Name(bytes) => String::from_utf8_lossy(bytes).to_string(),
        _ => String::new(),
    }
}

/// Map a PDF color space name to the number of components.
fn color_space_components(name: &str) -> u8 {
    match name {
        "DeviceGray" | "CalGray" | "G" => 1,
        "DeviceRGB" | "CalRGB" | "RGB" => 3,
        "DeviceCMYK" | "CMYK" => 4,
        _ => 3, // Default to RGB for unknown/ICCBased color spaces
    }
}

/// Resolve a PDF object via reference.
fn resolve_obj<'a>(doc: &'a Document, obj: &'a Object) -> lopdf::Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).cloned().unwrap_or(Object::Null),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Operation;
    use lopdf::dictionary;

    /// Create a minimal PDF document with text content for testing.
    fn create_test_pdf() -> Document {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        });

        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![100.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal("Hello World!")]),
                Operation::new("ET", vec![]),
            ],
        };

        let content_id = doc.add_object(lopdf::Stream::new(
            dictionary! {},
            content.encode().unwrap(),
        ));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        doc
    }

    #[test]
    fn test_extract_text_from_synthetic_pdf() {
        let doc = create_test_pdf();
        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();

        let chunks = extract_text_chunks(&doc, page_num, page_id).unwrap();

        // Should extract at least one text chunk
        assert!(!chunks.is_empty(), "Expected text chunks from test PDF");

        // The first chunk should contain "Hello World!"
        let first = &chunks[0];
        assert!(
            first.value.contains("Hello"),
            "Expected 'Hello' in chunk, got: '{}'",
            first.value
        );
    }

    #[test]
    fn test_extract_empty_page() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();

        let chunks = extract_text_chunks(&doc, page_num, page_id).unwrap();
        assert!(chunks.is_empty());
    }
}
