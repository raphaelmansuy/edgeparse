//! Unified PDF content stream parser — matches the reference ChunkParser architecture.
//!
//! Single-pass content stream walker that produces text, image, and line chunks
//! with shared graphics state. Handles:
//! - Text operators (BT/ET/Tf/Td/Tm/Tj/TJ/etc.)
//! - Image extraction via `Do` operator (XObject images with CTM-based bbox)
//! - Form XObject recursive processing via `Do` operator
//! - Inline images (BI/ID/EI)
//! - Path/line operators (m/l/c/re/S/f/B/etc.)
//! - Graphics state (q/Q/cm/gs)
//! - Color operators (g/rg/k/cs/sc/etc.)
//! - Marked content (BMC/BDC/EMC)

use lopdf::{content::Content, Dictionary, Document, Object, ObjectId};

use crate::models::bbox::{BoundingBox, Vertex};
use crate::models::chunks::{ImageChunk, LineArtChunk, LineChunk, TextChunk};
use crate::EdgePdfError;

use super::font::{resolve_page_fonts, FontCache, PdfFont};
use super::graphics_state::{GraphicsStateStack, Matrix};

/// Maximum recursion depth for Form XObject processing (prevents infinite loops).
const MAX_FORM_RECURSION_DEPTH: u32 = 10;

/// Minimum line width to consider a path segment (in points).
const MIN_LINE_WIDTH: f64 = 0.1;

/// Aspect ratio threshold: width/height > this means horizontal line.
const LINE_ASPECT_RATIO: f64 = 3.0;

/// Maximum thickness for a line (vs rectangle classification).
const MAX_LINE_THICKNESS: f64 = 10.0;

/// All chunks extracted from a single page.
#[derive(Debug, Default)]
pub struct PageChunks {
    /// Text chunks with position and font info
    pub text_chunks: Vec<TextChunk>,
    /// Image chunks with CTM-based bounding boxes
    pub image_chunks: Vec<ImageChunk>,
    /// Line segments (horizontal/vertical lines, rectangles)
    pub line_chunks: Vec<LineChunk>,
    /// Vector graphics (complex paths)
    pub line_art_chunks: Vec<LineArtChunk>,
}

/// Extract all chunks from a single page in one content stream pass.
pub fn extract_page_chunks(
    doc: &Document,
    page_number: u32,
    page_id: ObjectId,
) -> Result<PageChunks, EdgePdfError> {
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

    // Get content stream(s)
    let content_data = super::text_extractor::get_page_content(doc, &page_dict)?;
    if content_data.is_empty() {
        return Ok(PageChunks::default());
    }

    // Parse content stream operations
    let content = Content::decode(&content_data).map_err(|e| EdgePdfError::PipelineError {
        stage: 1,
        message: format!(
            "Failed to decode content stream for page {}: {}",
            page_number, e
        ),
    })?;

    // Resolve the Resources dictionary for this page (needed for Do/gs operators)
    let resources = resolve_page_resources(doc, &page_dict);

    let mut parser = ChunkParserState::new(page_number, font_cache);
    parser.process_operations(doc, &content.operations, &resources, 0);

    Ok(parser.into_page_chunks())
}

/// Resolve the /Resources dictionary for a page, following references.
fn resolve_page_resources(doc: &Document, page_dict: &Dictionary) -> Dictionary {
    match page_dict.get(b"Resources") {
        Ok(obj) => {
            let resolved = resolve_obj(doc, obj);
            resolved.as_dict().cloned().unwrap_or_default()
        }
        Err(_) => Dictionary::new(),
    }
}

/// Internal state for the unified chunk parser — equivalent to the reference ChunkParser.
struct ChunkParserState {
    page_number: u32,
    font_cache: FontCache,
    gs_stack: GraphicsStateStack,

    // Chunk accumulators
    text_chunks: Vec<TextChunk>,
    image_chunks: Vec<ImageChunk>,
    line_chunks: Vec<LineChunk>,
    line_art_chunks: Vec<LineArtChunk>,

    // Indices
    text_index: usize,
    image_index: u32,
    line_index: u32,

    // Marked content tracking
    mcid_stack: Vec<Option<i64>>,

    // Path construction state
    current_path: Vec<PathSegment>,
    subpath_start: Option<(f64, f64)>,
    current_point: Option<(f64, f64)>,
    line_width: f64,
}

impl ChunkParserState {
    fn new(page_number: u32, font_cache: FontCache) -> Self {
        Self {
            page_number,
            font_cache,
            gs_stack: GraphicsStateStack::default(),

            text_chunks: Vec::new(),
            image_chunks: Vec::new(),
            line_chunks: Vec::new(),
            line_art_chunks: Vec::new(),

            text_index: 0,
            image_index: 0,
            line_index: 0,

            mcid_stack: Vec::new(),

            current_path: Vec::new(),
            subpath_start: None,
            current_point: None,
            line_width: 1.0,
        }
    }

    fn into_page_chunks(self) -> PageChunks {
        PageChunks {
            text_chunks: self.text_chunks,
            image_chunks: self.image_chunks,
            line_chunks: self.line_chunks,
            line_art_chunks: self.line_art_chunks,
        }
    }

    /// Process all content stream operations — the core parser loop.
    fn process_operations(
        &mut self,
        doc: &Document,
        operations: &[lopdf::content::Operation],
        resources: &Dictionary,
        recursion_depth: u32,
    ) {
        for op in operations {
            match op.operator.as_str() {
                // ── Marked content operators ──
                "BMC" => {
                    self.mcid_stack.push(None);
                }
                "BDC" => {
                    let mcid = extract_mcid_from_bdc(&op.operands);
                    self.mcid_stack.push(mcid);
                }
                "EMC" => {
                    self.mcid_stack.pop();
                }

                // ── Graphics state ──
                "q" => self.gs_stack.save(),
                "Q" => self.gs_stack.restore(),
                "cm" => {
                    if op.operands.len() == 6 {
                        let vals: Vec<f64> = op
                            .operands
                            .iter()
                            .filter_map(|o| obj_to_f64(o.clone()))
                            .collect();
                        if vals.len() == 6 {
                            self.gs_stack
                                .concat_ctm(vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]);
                        }
                    }
                }
                "gs" => {
                    // Extended Graphics State — look up in /ExtGState resources
                    if let Some(name) = op.operands.first().and_then(obj_name_bytes) {
                        self.apply_ext_gstate(doc, resources, &name);
                    }
                }

                // ── Text state operators ──
                "BT" => self.gs_stack.current.begin_text(),
                "ET" => {}

                "Tf" => {
                    if op.operands.len() == 2 {
                        if let Object::Name(ref name) = op.operands[0] {
                            self.gs_stack.current.text_state.font_name =
                                String::from_utf8_lossy(name).to_string();
                        }
                        if let Some(size) = obj_to_f64(op.operands[1].clone()) {
                            self.gs_stack.current.text_state.font_size = size;
                        }
                    }
                }
                "Tc" => {
                    if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.text_state.char_spacing = v;
                    }
                }
                "Tw" => {
                    if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.text_state.word_spacing = v;
                    }
                }
                "Tz" => {
                    if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.text_state.horizontal_scaling = v;
                    }
                }
                "TL" => {
                    if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.text_state.leading = v;
                    }
                }
                "Ts" => {
                    if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.text_state.rise = v;
                    }
                }
                "Tr" => {
                    if let Some(v) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.text_state.render_mode = v as i32;
                    }
                }

                // ── Text positioning ──
                "Td" => {
                    if op.operands.len() == 2 {
                        let tx = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                        let ty = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                        self.gs_stack.current.translate_text(tx, ty);
                    }
                }
                "TD" => {
                    if op.operands.len() == 2 {
                        let tx = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                        let ty = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                        self.gs_stack.current.text_state.leading = -ty;
                        self.gs_stack.current.translate_text(tx, ty);
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
                            self.gs_stack.current.set_text_matrix(
                                vals[0], vals[1], vals[2], vals[3], vals[4], vals[5],
                            );
                        }
                    }
                }
                "T*" => {
                    self.gs_stack.current.next_line();
                }

                // ── Text showing operators → TextChunk ──
                "Tj" => {
                    if let Some(text_bytes) = op.operands.first().and_then(extract_string_bytes) {
                        self.emit_text_chunk(&text_bytes);
                    }
                }
                "TJ" => {
                    if let Some(Object::Array(ref arr)) = op.operands.first() {
                        self.process_tj_array(arr);
                    }
                }
                "'" => {
                    self.gs_stack.current.next_line();
                    if let Some(text_bytes) = op.operands.first().and_then(extract_string_bytes) {
                        self.emit_text_chunk(&text_bytes);
                    }
                }
                "\"" => {
                    if op.operands.len() == 3 {
                        if let Some(aw) = obj_to_f64(op.operands[0].clone()) {
                            self.gs_stack.current.text_state.word_spacing = aw;
                        }
                        if let Some(ac) = obj_to_f64(op.operands[1].clone()) {
                            self.gs_stack.current.text_state.char_spacing = ac;
                        }
                        self.gs_stack.current.next_line();
                        if let Some(text_bytes) = extract_string_bytes(&op.operands[2]) {
                            self.emit_text_chunk(&text_bytes);
                        }
                    }
                }

                // ── Color operators ──
                "g" => {
                    if let Some(gray) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.fill_color = vec![gray];
                        self.gs_stack.current.fill_color_space_components = 1;
                    }
                }
                "G" => {
                    if let Some(gray) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.gs_stack.current.stroke_color = vec![gray];
                        self.gs_stack.current.stroke_color_space_components = 1;
                    }
                }
                "rg" => {
                    if op.operands.len() == 3 {
                        let r = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                        let g = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                        let b = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                        self.gs_stack.current.fill_color = vec![r, g, b];
                        self.gs_stack.current.fill_color_space_components = 3;
                    }
                }
                "RG" => {
                    if op.operands.len() == 3 {
                        let r = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                        let g = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                        let b = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                        self.gs_stack.current.stroke_color = vec![r, g, b];
                        self.gs_stack.current.stroke_color_space_components = 3;
                    }
                }
                "k" => {
                    if op.operands.len() == 4 {
                        let c = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                        let m = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                        let y = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                        let kk = obj_to_f64(op.operands[3].clone()).unwrap_or(0.0);
                        self.gs_stack.current.fill_color = vec![c, m, y, kk];
                        self.gs_stack.current.fill_color_space_components = 4;
                    }
                }
                "K" => {
                    if op.operands.len() == 4 {
                        let c = obj_to_f64(op.operands[0].clone()).unwrap_or(0.0);
                        let m = obj_to_f64(op.operands[1].clone()).unwrap_or(0.0);
                        let y = obj_to_f64(op.operands[2].clone()).unwrap_or(0.0);
                        let kk = obj_to_f64(op.operands[3].clone()).unwrap_or(0.0);
                        self.gs_stack.current.stroke_color = vec![c, m, y, kk];
                        self.gs_stack.current.stroke_color_space_components = 4;
                    }
                }
                "cs" => {
                    if let Some(name) = op.operands.first() {
                        let cs_name = obj_to_name(name);
                        let comps = color_space_components(&cs_name);
                        self.gs_stack.current.fill_color_space_components = comps;
                        // PDF spec 8.6.5.3: reset color to default for new space
                        self.gs_stack.current.fill_color = default_color_for_space(comps);
                    }
                }
                "CS" => {
                    if let Some(name) = op.operands.first() {
                        let cs_name = obj_to_name(name);
                        let comps = color_space_components(&cs_name);
                        self.gs_stack.current.stroke_color_space_components = comps;
                        // PDF spec 8.6.5.3: reset color to default for new space
                        self.gs_stack.current.stroke_color = default_color_for_space(comps);
                    }
                }
                "sc" | "scn" => {
                    let components: Vec<f64> = op
                        .operands
                        .iter()
                        .filter_map(|o| obj_to_f64(o.clone()))
                        .collect();
                    if !components.is_empty() {
                        self.gs_stack.current.fill_color = components;
                    }
                }
                "SC" | "SCN" => {
                    let components: Vec<f64> = op
                        .operands
                        .iter()
                        .filter_map(|o| obj_to_f64(o.clone()))
                        .collect();
                    if !components.is_empty() {
                        self.gs_stack.current.stroke_color = components;
                    }
                }

                // ── Line width ──
                "w" => {
                    if let Some(w) = op.operands.first().and_then(|o| obj_to_f64(o.clone())) {
                        self.line_width = w;
                    }
                }

                // ── Path construction ──
                "m" => {
                    if op.operands.len() >= 2 {
                        if let (Some(x), Some(y)) = (
                            op.operands.first().and_then(|o| obj_to_f64(o.clone())),
                            op.operands.get(1).and_then(|o| obj_to_f64(o.clone())),
                        ) {
                            let (tx, ty) = self.transform_point(x, y);
                            self.subpath_start = Some((tx, ty));
                            self.current_point = Some((tx, ty));
                        }
                    }
                }
                "l" => {
                    if op.operands.len() >= 2 {
                        if let (Some(x), Some(y)) = (
                            op.operands.first().and_then(|o| obj_to_f64(o.clone())),
                            op.operands.get(1).and_then(|o| obj_to_f64(o.clone())),
                        ) {
                            let (tx, ty) = self.transform_point(x, y);
                            if let Some((cx, cy)) = self.current_point {
                                self.current_path.push(PathSegment::Line {
                                    x1: cx,
                                    y1: cy,
                                    x2: tx,
                                    y2: ty,
                                });
                            }
                            self.current_point = Some((tx, ty));
                        }
                    }
                }
                "c" => {
                    if op.operands.len() >= 6 {
                        let vals: Vec<f64> = op
                            .operands
                            .iter()
                            .filter_map(|o| obj_to_f64(o.clone()))
                            .collect();
                        if vals.len() >= 6 {
                            let (tx, ty) = self.transform_point(vals[4], vals[5]);
                            if let Some((cx, cy)) = self.current_point {
                                let (cp1x, cp1y) = self.transform_point(vals[0], vals[1]);
                                let (cp2x, cp2y) = self.transform_point(vals[2], vals[3]);
                                self.current_path.push(PathSegment::Curve {
                                    x1: cx,
                                    y1: cy,
                                    cp1x,
                                    cp1y,
                                    cp2x,
                                    cp2y,
                                    x2: tx,
                                    y2: ty,
                                });
                            }
                            self.current_point = Some((tx, ty));
                        }
                    }
                }
                "v" => {
                    if op.operands.len() >= 4 {
                        let vals: Vec<f64> = op
                            .operands
                            .iter()
                            .filter_map(|o| obj_to_f64(o.clone()))
                            .collect();
                        if vals.len() >= 4 {
                            let (tx, ty) = self.transform_point(vals[2], vals[3]);
                            if let Some((cx, cy)) = self.current_point {
                                let (cp2x, cp2y) = self.transform_point(vals[0], vals[1]);
                                self.current_path.push(PathSegment::Curve {
                                    x1: cx,
                                    y1: cy,
                                    cp1x: cx,
                                    cp1y: cy,
                                    cp2x,
                                    cp2y,
                                    x2: tx,
                                    y2: ty,
                                });
                            }
                            self.current_point = Some((tx, ty));
                        }
                    }
                }
                "y" => {
                    if op.operands.len() >= 4 {
                        let vals: Vec<f64> = op
                            .operands
                            .iter()
                            .filter_map(|o| obj_to_f64(o.clone()))
                            .collect();
                        if vals.len() >= 4 {
                            let (tx, ty) = self.transform_point(vals[2], vals[3]);
                            if let Some((cx, cy)) = self.current_point {
                                let (cp1x, cp1y) = self.transform_point(vals[0], vals[1]);
                                self.current_path.push(PathSegment::Curve {
                                    x1: cx,
                                    y1: cy,
                                    cp1x,
                                    cp1y,
                                    cp2x: tx,
                                    cp2y: ty,
                                    x2: tx,
                                    y2: ty,
                                });
                            }
                            self.current_point = Some((tx, ty));
                        }
                    }
                }
                "h" => {
                    if let (Some((sx, sy)), Some((cx, cy))) =
                        (self.subpath_start, self.current_point)
                    {
                        if (sx - cx).abs() > 0.01 || (sy - cy).abs() > 0.01 {
                            self.current_path.push(PathSegment::Line {
                                x1: cx,
                                y1: cy,
                                x2: sx,
                                y2: sy,
                            });
                        }
                        self.current_point = self.subpath_start;
                    }
                }
                "re" => {
                    if op.operands.len() >= 4 {
                        let vals: Vec<f64> = op
                            .operands
                            .iter()
                            .filter_map(|o| obj_to_f64(o.clone()))
                            .collect();
                        if vals.len() >= 4 {
                            let (x, y, w, h) = (vals[0], vals[1], vals[2], vals[3]);
                            let (x1, y1) = self.transform_point(x, y);
                            let (x2, y2) = self.transform_point(x + w, y);
                            let (x3, y3) = self.transform_point(x + w, y + h);
                            let (x4, y4) = self.transform_point(x, y + h);
                            self.current_path.push(PathSegment::Line { x1, y1, x2, y2 });
                            self.current_path.push(PathSegment::Line {
                                x1: x2,
                                y1: y2,
                                x2: x3,
                                y2: y3,
                            });
                            self.current_path.push(PathSegment::Line {
                                x1: x3,
                                y1: y3,
                                x2: x4,
                                y2: y4,
                            });
                            self.current_path.push(PathSegment::Line {
                                x1: x4,
                                y1: y4,
                                x2: x1,
                                y2: y1,
                            });
                            self.subpath_start = Some((x1, y1));
                            self.current_point = Some((x1, y1));
                        }
                    }
                }

                // ── Path painting ──
                "S" => {
                    self.classify_and_emit_path();
                }
                "s" => {
                    // close and stroke
                    self.close_subpath();
                    self.classify_and_emit_path();
                }
                "f" | "F" | "f*" => {
                    self.classify_and_emit_path();
                }
                "B" | "B*" | "b" | "b*" => {
                    if op.operator.starts_with('b') {
                        self.close_subpath();
                    }
                    self.classify_and_emit_path();
                }
                "n" => {
                    // End path without painting
                    self.current_path.clear();
                    self.subpath_start = None;
                    self.current_point = None;
                }

                // ── XObject (Do) — Image and Form XObject handling ──
                "Do" => {
                    if let Some(name_bytes) = op.operands.first().and_then(obj_name_bytes) {
                        self.handle_do_operator(doc, resources, &name_bytes, recursion_depth);
                    }
                }

                // ── Inline image (BI/ID/EI) ──
                // lopdf parses BI inline images as a special operation;
                // the operator is "BI" with the image dict + data as operands.
                // We create an ImageChunk using the current CTM.
                "BI" => {
                    self.emit_inline_image();
                }

                _ => {
                    // Ignore unknown/unhandled operators
                }
            }
        }
    }

    // ── Text chunk creation ──

    fn emit_text_chunk(&mut self, text_bytes: &[u8]) {
        if text_bytes.is_empty() {
            return;
        }

        let font = self
            .font_cache
            .get(&self.gs_stack.current.text_state.font_name)
            .cloned()
            .unwrap_or_else(|| PdfFont::default_font(&self.gs_stack.current.text_state.font_name));
        let active_mcid = self.active_mcid();

        if let Some(chunk) = create_text_chunk(
            text_bytes,
            &font,
            &mut self.gs_stack,
            self.page_number,
            &mut self.text_index,
            active_mcid,
        ) {
            self.text_chunks.push(chunk);
        }
    }

    fn process_tj_array(&mut self, arr: &[Object]) {
        let font = self
            .font_cache
            .get(&self.gs_stack.current.text_state.font_name)
            .cloned()
            .unwrap_or_else(|| PdfFont::default_font(&self.gs_stack.current.text_state.font_name));
        let active_mcid = self.active_mcid();

        for item in arr {
            match item {
                Object::String(bytes, _) => {
                    if let Some(chunk) = create_text_chunk(
                        bytes,
                        &font,
                        &mut self.gs_stack,
                        self.page_number,
                        &mut self.text_index,
                        active_mcid,
                    ) {
                        self.text_chunks.push(chunk);
                    }
                }
                _ => {
                    if let Some(adj) = obj_to_f64(item.clone()) {
                        let displacement =
                            -adj / 1000.0 * self.gs_stack.current.text_state.font_size;
                        self.gs_stack.current.advance_text(displacement);
                    }
                }
            }
        }
    }

    // ── Image handling ──

    /// Handle `Do` operator — dispatches to image or form XObject processing.
    fn handle_do_operator(
        &mut self,
        doc: &Document,
        resources: &Dictionary,
        name_bytes: &[u8],
        recursion_depth: u32,
    ) {
        // Look up the XObject in /Resources/XObject
        let xobject_dict = match resources.get(b"XObject") {
            Ok(obj) => {
                let resolved = resolve_obj(doc, obj);
                match resolved.as_dict() {
                    Ok(d) => d.clone(),
                    Err(_) => return,
                }
            }
            Err(_) => return,
        };

        let xobj_ref = match xobject_dict.get(name_bytes) {
            Ok(obj) => resolve_obj(doc, obj),
            Err(_) => return,
        };

        let stream = match xobj_ref.as_stream() {
            Ok(s) => s.clone(),
            Err(_) => return,
        };

        let subtype = stream
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| match resolve_obj(doc, o) {
                Object::Name(n) => Some(String::from_utf8_lossy(&n).to_string()),
                _ => None,
            });

        match subtype.as_deref() {
            Some("Image") => {
                // Image XObject → create ImageChunk with CTM-based bbox
                self.emit_image_from_ctm();
            }
            Some("Form") => {
                // Form XObject → recursive content stream processing
                if recursion_depth < MAX_FORM_RECURSION_DEPTH {
                    self.process_form_xobject(doc, &stream, resources, recursion_depth);
                }
            }
            _ => {}
        }
    }

    /// Create an ImageChunk using the current CTM to compute position.
    /// Image occupies [0,0] to [1,1] in user space before CTM transform.
    fn emit_image_from_ctm(&mut self) {
        let ctm = &self.gs_stack.current.ctm;

        // Transform the image unit square corners through CTM
        let (x0, y0) = ctm.transform_point(0.0, 0.0);
        let (x1, y1) = ctm.transform_point(1.0, 0.0);
        let (x2, y2) = ctm.transform_point(1.0, 1.0);
        let (x3, y3) = ctm.transform_point(0.0, 1.0);

        let min_x = x0.min(x1).min(x2).min(x3);
        let max_x = x0.max(x1).max(x2).max(x3);
        let min_y = y0.min(y1).min(y2).min(y3);
        let max_y = y0.max(y1).max(y2).max(y3);

        // Skip degenerate images
        if (max_x - min_x).abs() < 0.1 || (max_y - min_y).abs() < 0.1 {
            return;
        }

        self.image_index += 1;
        self.image_chunks.push(ImageChunk {
            bbox: BoundingBox::new(Some(self.page_number), min_x, min_y, max_x, max_y),
            index: Some(self.image_index),
            level: None,
        });
    }

    /// Create an ImageChunk for an inline image (BI/ID/EI).
    fn emit_inline_image(&mut self) {
        // Inline images also use the current CTM for positioning
        self.emit_image_from_ctm();
    }

    /// Process a Form XObject — recursively parse its content stream.
    fn process_form_xobject(
        &mut self,
        doc: &Document,
        stream: &lopdf::Stream,
        parent_resources: &Dictionary,
        recursion_depth: u32,
    ) {
        // Get the form's /Matrix (default identity)
        let form_matrix = get_form_matrix(doc, &stream.dict);

        // Concatenate form matrix with current CTM (like the reference implementation: xFormGraphicsState.getCTM().concatenate(matrix))
        self.gs_stack.save();
        let m = form_matrix;
        self.gs_stack.concat_ctm(m.a, m.b, m.c, m.d, m.e, m.f);

        // Resolve form's own resources, falling back to parent
        let form_resources = match stream.dict.get(b"Resources") {
            Ok(obj) => {
                let resolved = resolve_obj(doc, obj);
                resolved
                    .as_dict()
                    .cloned()
                    .unwrap_or_else(|_| parent_resources.clone())
            }
            Err(_) => parent_resources.clone(),
        };

        // Decompress the form's content stream
        let form_content = if stream.dict.get(b"Filter").is_ok() {
            match stream.decompressed_content() {
                Ok(data) => data,
                Err(_) => {
                    self.gs_stack.restore();
                    return;
                }
            }
        } else {
            stream.content.clone()
        };

        if form_content.is_empty() {
            self.gs_stack.restore();
            return;
        }

        // Parse the form's content stream
        if let Ok(content) = Content::decode(&form_content) {
            // Resolve fonts from form's resources and merge with page fonts
            let form_font_cache = resolve_form_fonts(doc, &form_resources);
            let mut merged_cache = FontCache::default();
            // Copy page fonts first
            for (name, font) in self.font_cache.iter() {
                merged_cache.insert(name.clone(), font.clone());
            }
            // Override with form fonts
            for (name, font) in form_font_cache.iter() {
                merged_cache.insert(name.clone(), font.clone());
            }

            let saved_fc = std::mem::replace(&mut self.font_cache, merged_cache);
            self.process_operations(
                doc,
                &content.operations,
                &form_resources,
                recursion_depth + 1,
            );
            self.font_cache = saved_fc;
        }

        self.gs_stack.restore();
    }

    // ── Extended Graphics State ──

    fn apply_ext_gstate(&mut self, doc: &Document, resources: &Dictionary, name: &[u8]) {
        let ext_gstate_dict = match resources.get(b"ExtGState") {
            Ok(obj) => {
                let resolved = resolve_obj(doc, obj);
                match resolved.as_dict() {
                    Ok(d) => d.clone(),
                    Err(_) => return,
                }
            }
            Err(_) => return,
        };

        let gs_obj = match ext_gstate_dict.get(name) {
            Ok(obj) => resolve_obj(doc, obj),
            Err(_) => return,
        };

        let gs_dict = match gs_obj.as_dict() {
            Ok(d) => d,
            Err(_) => return,
        };

        // Apply relevant properties from ExtGState
        // /Font — set font and size
        if let Ok(font_arr) = gs_dict.get(b"Font") {
            if let Ok(arr) = resolve_obj(doc, font_arr).as_array() {
                if arr.len() >= 2 {
                    if let Object::Name(ref name) = arr[0] {
                        self.gs_stack.current.text_state.font_name =
                            String::from_utf8_lossy(name).to_string();
                    }
                    if let Some(size) = obj_to_f64(arr[1].clone()) {
                        self.gs_stack.current.text_state.font_size = size;
                    }
                }
            }
        }

        // /LW — line width
        if let Ok(lw) = gs_dict.get(b"LW") {
            if let Some(w) = obj_to_f64(resolve_obj(doc, lw)) {
                self.line_width = w;
            }
        }
    }

    // ── Path classification ──

    fn close_subpath(&mut self) {
        if let (Some((sx, sy)), Some((cx, cy))) = (self.subpath_start, self.current_point) {
            if (sx - cx).abs() > 0.01 || (sy - cy).abs() > 0.01 {
                self.current_path.push(PathSegment::Line {
                    x1: cx,
                    y1: cy,
                    x2: sx,
                    y2: sy,
                });
            }
            self.current_point = self.subpath_start;
        }
    }

    fn classify_and_emit_path(&mut self) {
        let path = std::mem::take(&mut self.current_path);
        self.subpath_start = None;
        self.current_point = None;

        if path.is_empty() || self.line_width < MIN_LINE_WIDTH {
            return;
        }

        let has_curves = path.iter().any(|s| matches!(s, PathSegment::Curve { .. }));

        // Try to classify individual segments as line chunks
        if !has_curves && path.len() <= 4 {
            let mut classified_lines = Vec::new();
            for seg in &path {
                if let PathSegment::Line { x1, y1, x2, y2 } = seg {
                    let dx = (x2 - x1).abs();
                    let dy = (y2 - y1).abs();
                    let length = (dx * dx + dy * dy).sqrt();

                    if length < MIN_LINE_WIDTH {
                        continue;
                    }

                    let is_horizontal = dy < MAX_LINE_THICKNESS && dx > dy * LINE_ASPECT_RATIO;
                    let is_vertical = dx < MAX_LINE_THICKNESS && dy > dx * LINE_ASPECT_RATIO;

                    if is_horizontal || is_vertical {
                        self.line_index += 1;
                        let min_x = x1.min(*x2);
                        let max_x = x1.max(*x2);
                        let min_y = y1.min(*y2);
                        let max_y = y1.max(*y2);
                        let half_w = self.line_width / 2.0;

                        classified_lines.push(LineChunk {
                            bbox: BoundingBox::new(
                                Some(self.page_number),
                                min_x - if is_vertical { half_w } else { 0.0 },
                                min_y - if is_horizontal { half_w } else { 0.0 },
                                max_x + if is_vertical { half_w } else { 0.0 },
                                max_y + if is_horizontal { half_w } else { 0.0 },
                            ),
                            index: Some(self.line_index),
                            level: None,
                            start: Vertex {
                                x: *x1,
                                y: *y1,
                                radius: 0.0,
                            },
                            end: Vertex {
                                x: *x2,
                                y: *y2,
                                radius: 0.0,
                            },
                            width: self.line_width,
                            is_horizontal_line: is_horizontal,
                            is_vertical_line: is_vertical,
                            is_square: false,
                        });
                    }
                }
            }
            if !classified_lines.is_empty() {
                self.line_chunks.extend(classified_lines);
                return;
            }
        }

        // Rectangle classification (4 line segments forming a box)
        if !has_curves && path.len() == 4 {
            if let Some(rect) = try_classify_rectangle(&path, self.line_width, self.page_number) {
                self.line_index += 1;
                let mut rect = rect;
                rect.index = Some(self.line_index);
                self.line_chunks.push(rect);
                return;
            }
        }

        // Complex path → LineArtChunk
        if path.len() >= 2 {
            let mut art_lines = Vec::new();
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;

            for seg in &path {
                let (sx, sy, ex, ey) = match seg {
                    PathSegment::Line { x1, y1, x2, y2 } => (*x1, *y1, *x2, *y2),
                    PathSegment::Curve { x1, y1, x2, y2, .. } => (*x1, *y1, *x2, *y2),
                };
                min_x = min_x.min(sx).min(ex);
                min_y = min_y.min(sy).min(ey);
                max_x = max_x.max(sx).max(ex);
                max_y = max_y.max(sy).max(ey);

                self.line_index += 1;
                art_lines.push(LineChunk {
                    bbox: BoundingBox::new(
                        Some(self.page_number),
                        sx.min(ex),
                        sy.min(ey),
                        sx.max(ex),
                        sy.max(ey),
                    ),
                    index: Some(self.line_index),
                    level: None,
                    start: Vertex {
                        x: sx,
                        y: sy,
                        radius: 0.0,
                    },
                    end: Vertex {
                        x: ex,
                        y: ey,
                        radius: 0.0,
                    },
                    width: self.line_width,
                    is_horizontal_line: false,
                    is_vertical_line: false,
                    is_square: false,
                });
            }

            self.line_index += 1;
            self.line_art_chunks.push(LineArtChunk {
                bbox: BoundingBox::new(Some(self.page_number), min_x, min_y, max_x, max_y),
                index: Some(self.line_index),
                level: None,
                line_chunks: art_lines,
            });
        }
    }

    // ── Helpers ──

    fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        self.gs_stack.current.ctm.transform_point(x, y)
    }

    fn active_mcid(&self) -> Option<i64> {
        self.mcid_stack.iter().rev().find_map(|&mcid| mcid)
    }
}

// ═══════════════════════════════════════════════════════════════════
// Helper functions (shared with text_extractor for backward compat)
// ═══════════════════════════════════════════════════════════════════

/// Create a TextChunk from raw text bytes — same logic as text_extractor.
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

    let trm = state.current.text_rendering_matrix();
    let start_x = trm.e;
    let font_size = trm.font_size_factor();

    if font_size < 0.1 {
        return None;
    }

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

        let glyph_w = font.glyph_width(char_code) / 1000.0;
        total_width += glyph_w;
        symbol_ends.push(start_x + total_width * font_size);

        total_width += state.current.text_state.char_spacing / state.current.text_state.font_size;

        if decoded == " " {
            total_width +=
                state.current.text_state.word_spacing / state.current.text_state.font_size;
        }
    }

    let displacement = total_width * state.current.text_state.font_size;
    state.current.advance_text(displacement);

    if text.is_empty() {
        return None;
    }

    // Compute TRM_after (text rendering matrix after text advancement)
    let trm_after = state.current.text_rendering_matrix();

    // Use font ascent/descent from font descriptor (glyph-space units, per-mille).
    let ascent = font.ascent;
    let descent = font.descent;

    // TRM matrix: a=scaleX, b=shearY, c=shearX, d=scaleY, e=translateX, f=translateY
    let trm_before = &trm;

    // The reference bbox formula with 4 branches based on text direction/orientation.
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

    let text_format = if state.current.text_state.rise > font_size * 0.1 {
        crate::models::enums::TextFormat::Superscript
    } else if state.current.text_state.rise < -font_size * 0.1 {
        crate::models::enums::TextFormat::Subscript
    } else {
        crate::models::enums::TextFormat::Normal
    };

    *chunk_index += 1;

    let fc = &state.current.fill_color;
    let font_color = format!(
        "[{}]",
        fc.iter()
            .map(|v| {
                // The reference veraPDF stores colors as the reference implementation float (f32), then serializes
                // via double's toString(), giving full f64 representation of the f32 value.
                // We replicate: parse as f64 (from lopdf) → round to f32 → back to f64.
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
        contrast_ratio: 21.0,
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

/// A segment in a path being constructed.
#[derive(Debug, Clone)]
enum PathSegment {
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    #[allow(dead_code)]
    Curve {
        x1: f64,
        y1: f64,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
        x2: f64,
        y2: f64,
    },
}

/// Try to classify 4 line segments as a rectangle.
fn try_classify_rectangle(
    segments: &[PathSegment],
    _line_width: f64,
    page_number: u32,
) -> Option<LineChunk> {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for seg in segments {
        if let PathSegment::Line { x1, y1, x2, y2 } = seg {
            min_x = min_x.min(*x1).min(*x2);
            min_y = min_y.min(*y1).min(*y2);
            max_x = max_x.max(*x1).max(*x2);
            max_y = max_y.max(*y1).max(*y2);
        } else {
            return None;
        }
    }

    let w = max_x - min_x;
    let h = max_y - min_y;

    if w < MIN_LINE_WIDTH || h < MIN_LINE_WIDTH {
        return None;
    }

    let is_square = (w - h).abs() / w.max(h) < 0.3;

    Some(LineChunk {
        bbox: BoundingBox::new(Some(page_number), min_x, min_y, max_x, max_y),
        index: None,
        level: None,
        start: Vertex {
            x: min_x,
            y: min_y,
            radius: 0.0,
        },
        end: Vertex {
            x: max_x,
            y: max_y,
            radius: 0.0,
        },
        width: w.min(h),
        is_horizontal_line: w > h * LINE_ASPECT_RATIO,
        is_vertical_line: h > w * LINE_ASPECT_RATIO,
        is_square,
    })
}

/// Get the /Matrix from a Form XObject dictionary (defaults to identity).
fn get_form_matrix(doc: &Document, dict: &Dictionary) -> Matrix {
    match dict.get(b"Matrix") {
        Ok(obj) => {
            let resolved = resolve_obj(doc, obj);
            if let Ok(arr) = resolved.as_array() {
                let vals: Vec<f64> = arr.iter().filter_map(|o| obj_to_f64(o.clone())).collect();
                if vals.len() == 6 {
                    return Matrix {
                        a: vals[0],
                        b: vals[1],
                        c: vals[2],
                        d: vals[3],
                        e: vals[4],
                        f: vals[5],
                    };
                }
            }
            Matrix::identity()
        }
        Err(_) => Matrix::identity(),
    }
}

/// Resolve fonts from a resources dictionary (for Form XObjects).
fn resolve_form_fonts(doc: &Document, resources: &Dictionary) -> FontCache {
    let font_dict = match resources.get(b"Font") {
        Ok(obj) => {
            let resolved = resolve_obj(doc, obj);
            match resolved.as_dict() {
                Ok(d) => d.clone(),
                Err(_) => return FontCache::default(),
            }
        }
        Err(_) => return FontCache::default(),
    };

    let mut cache = FontCache::default();
    for (name_bytes, font_ref) in font_dict.iter() {
        let font_name = String::from_utf8_lossy(name_bytes).to_string();
        let font_obj = resolve_obj(doc, font_ref);
        if let Ok(font_dict) = font_obj.as_dict() {
            let font = super::font::resolve_font_dict(doc, &font_name, font_dict);
            cache.insert(font_name, font);
        }
    }
    cache
}

// ── Utility functions ──

fn extract_string_bytes(obj: &Object) -> Option<Vec<u8>> {
    match obj {
        Object::String(bytes, _) => Some(bytes.clone()),
        _ => None,
    }
}

fn extract_mcid_from_bdc(operands: &[Object]) -> Option<i64> {
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

fn obj_to_f64(obj: Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(i as f64),
        Object::Real(f) => Some(f),
        _ => None,
    }
}

fn obj_to_name(obj: &Object) -> String {
    match obj {
        Object::Name(bytes) => String::from_utf8_lossy(bytes).to_string(),
        _ => String::new(),
    }
}

/// Extract raw name bytes from a PDF Name object.
fn obj_name_bytes(obj: &Object) -> Option<Vec<u8>> {
    match obj {
        Object::Name(bytes) => Some(bytes.clone()),
        _ => None,
    }
}

fn color_space_components(name: &str) -> u8 {
    match name {
        "DeviceGray" | "CalGray" | "G" => 1,
        "DeviceRGB" | "CalRGB" | "RGB" => 3,
        "DeviceCMYK" | "CMYK" => 4,
        _ => 3,
    }
}

/// Default color for a given color space component count (PDF spec 8.6.5.3).
fn default_color_for_space(components: u8) -> Vec<f64> {
    match components {
        4 => vec![0.0, 0.0, 0.0, 1.0], // CMYK: default black
        3 => vec![0.0, 0.0, 0.0],      // RGB: default black
        _ => vec![0.0],                // Gray: default black
    }
}

fn resolve_obj(doc: &Document, obj: &Object) -> Object {
    match obj {
        Object::Reference(id) => doc.get_object(*id).cloned().unwrap_or(Object::Null),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::Operation;
    use lopdf::{dictionary, Stream};

    fn create_test_pdf_with_text() -> Document {
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

        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

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
    fn test_unified_text_extraction() {
        let doc = create_test_pdf_with_text();
        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();

        let chunks = extract_page_chunks(&doc, page_num, page_id).unwrap();
        assert!(!chunks.text_chunks.is_empty(), "Expected text chunks");
        assert!(
            chunks.text_chunks[0].value.contains("Hello"),
            "Expected 'Hello' in text"
        );
        assert!(chunks.image_chunks.is_empty(), "No images expected");
    }

    #[test]
    fn test_image_from_do_operator() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        // Create image XObject
        let img_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => 200,
                "Height" => 100,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            vec![0u8; 100],
        );
        let img_id = doc.add_object(img_stream);

        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! {
                "Im1" => img_id,
            },
        });

        // Content stream: scale image and place at (72, 500)
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        Object::Real(200.0), // a: width
                        0.into(),
                        0.into(),
                        Object::Real(100.0), // d: height
                        Object::Real(72.0),  // e: x position
                        Object::Real(500.0), // f: y position
                    ],
                ),
                Operation::new("Do", vec!["Im1".into()]),
                Operation::new("Q", vec![]),
            ],
        };

        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

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

        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();

        let chunks = extract_page_chunks(&doc, page_num, page_id).unwrap();
        assert_eq!(chunks.image_chunks.len(), 1, "Expected 1 image chunk");

        let img = &chunks.image_chunks[0];
        // Image should be at (72, 500) with size (200, 100)
        assert!(
            (img.bbox.left_x - 72.0).abs() < 1.0,
            "Expected left_x ~72, got {}",
            img.bbox.left_x
        );
        assert!(
            (img.bbox.bottom_y - 500.0).abs() < 1.0,
            "Expected bottom_y ~500, got {}",
            img.bbox.bottom_y
        );
        assert!(
            (img.bbox.right_x - 272.0).abs() < 1.0,
            "Expected right_x ~272, got {}",
            img.bbox.right_x
        );
        assert!(
            (img.bbox.top_y - 600.0).abs() < 1.0,
            "Expected top_y ~600, got {}",
            img.bbox.top_y
        );
    }

    #[test]
    fn test_form_xobject_recursive() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        // Create a font
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        // Create a Form XObject with text inside
        let form_content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 10.into()]),
                Operation::new("Td", vec![0.into(), 0.into()]),
                Operation::new("Tj", vec![Object::string_literal("Form Text")]),
                Operation::new("ET", vec![]),
            ],
        };

        let form_stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Form",
                "BBox" => vec![0.into(), 0.into(), 200.into(), 50.into()],
                "Resources" => dictionary! {
                    "Font" => dictionary! {
                        "F1" => font_id,
                    },
                },
            },
            form_content.encode().unwrap(),
        );
        let form_id = doc.add_object(form_stream);

        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
            "XObject" => dictionary! {
                "Fm1" => form_id,
            },
        });

        // Page content stream: invoke the form XObject
        let page_content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new(
                    "cm",
                    vec![
                        1.into(),
                        0.into(),
                        0.into(),
                        1.into(),
                        Object::Real(50.0),
                        Object::Real(400.0),
                    ],
                ),
                Operation::new("Do", vec!["Fm1".into()]),
                Operation::new("Q", vec![]),
            ],
        };

        let content_id =
            doc.add_object(Stream::new(dictionary! {}, page_content.encode().unwrap()));

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

        let pages = doc.get_pages();
        let (&page_num, &page_id) = pages.iter().next().unwrap();

        let chunks = extract_page_chunks(&doc, page_num, page_id).unwrap();
        assert!(
            !chunks.text_chunks.is_empty(),
            "Expected text from Form XObject"
        );
        assert!(
            chunks.text_chunks[0].value.contains("Form"),
            "Expected 'Form' text, got: '{}'",
            chunks.text_chunks[0].value
        );
    }

    #[test]
    fn test_line_extraction_unified() {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let content = Content {
            operations: vec![
                Operation::new("w", vec![Object::Real(1.0)]),
                Operation::new("m", vec![72.into(), 400.into()]),
                Operation::new("l", vec![500.into(), 400.into()]),
                Operation::new("S", vec![]),
            ],
        };

        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));

        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
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

        let chunks = extract_page_chunks(&doc, page_num, page_id).unwrap();
        assert_eq!(chunks.line_chunks.len(), 1, "Expected 1 horizontal line");
        assert!(chunks.line_chunks[0].is_horizontal_line);
    }
}
