//! PDF line segment extraction — extract stroked/filled paths as LineChunks.
//!
//! Parses content stream path operators (m, l, c, re, S, s, f, h) and
//! classifies resulting paths as horizontal lines, vertical lines, or shapes.

use lopdf::{Document, Object};

use crate::models::bbox::{BoundingBox, Vertex};
use crate::models::chunks::{LineArtChunk, LineChunk};
use crate::pdf::graphics_state::GraphicsStateStack;
use crate::EdgePdfError;

/// Minimum line width to consider a path as a line segment (in points).
const MIN_LINE_WIDTH: f64 = 0.1;

/// Aspect ratio threshold: if width/height > this, it's horizontal.
const LINE_ASPECT_RATIO: f64 = 3.0;

/// Maximum thickness for a line to be classified as a line (vs a rectangle).
const MAX_LINE_THICKNESS: f64 = 10.0;

/// Extract line segments and line art from a PDF page's content stream.
pub fn extract_line_chunks(
    doc: &Document,
    page_number: u32,
    page_id: lopdf::ObjectId,
) -> Result<(Vec<LineChunk>, Vec<LineArtChunk>), EdgePdfError> {
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
        })?;

    let content_bytes = crate::pdf::text_extractor::get_page_content(doc, page_dict)?;
    if content_bytes.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let content = lopdf::content::Content::decode(&content_bytes).map_err(|e| {
        EdgePdfError::PipelineError {
            stage: 1,
            message: format!(
                "Failed to decode content stream for page {}: {}",
                page_number, e
            ),
        }
    })?;

    let mut gs_stack = GraphicsStateStack::default();
    let mut line_width: f64 = 1.0;

    // Current path state
    let mut current_path: Vec<PathSegment> = Vec::new();
    let mut subpath_start: Option<(f64, f64)> = None;
    let mut current_point: Option<(f64, f64)> = None;

    let mut line_chunks: Vec<LineChunk> = Vec::new();
    let mut line_art_chunks: Vec<LineArtChunk> = Vec::new();
    let mut line_index = 0u32;

    for op in &content.operations {
        match op.operator.as_str() {
            // Graphics state
            "q" => gs_stack.save(),
            "Q" => gs_stack.restore(),
            "cm" => {
                if op.operands.len() >= 6 {
                    let vals: Vec<f64> = op.operands.iter().filter_map(get_number).collect();
                    if vals.len() >= 6 {
                        gs_stack.concat_ctm(vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]);
                    }
                }
            }
            // Line width
            "w" => {
                if let Some(w) = op.operands.first().and_then(get_number) {
                    line_width = w;
                }
            }
            // Path construction
            "m" => {
                // moveto
                if op.operands.len() >= 2 {
                    if let (Some(x), Some(y)) = (
                        op.operands.first().and_then(get_number),
                        op.operands.get(1).and_then(get_number),
                    ) {
                        let (tx, ty) = transform_point(&gs_stack, x, y);
                        subpath_start = Some((tx, ty));
                        current_point = Some((tx, ty));
                    }
                }
            }
            "l" => {
                // lineto
                if op.operands.len() >= 2 {
                    if let (Some(x), Some(y)) = (
                        op.operands.first().and_then(get_number),
                        op.operands.get(1).and_then(get_number),
                    ) {
                        let (tx, ty) = transform_point(&gs_stack, x, y);
                        if let Some((cx, cy)) = current_point {
                            current_path.push(PathSegment::Line {
                                x1: cx,
                                y1: cy,
                                x2: tx,
                                y2: ty,
                            });
                        }
                        current_point = Some((tx, ty));
                    }
                }
            }
            "c" => {
                // curveto (cubic Bézier)
                if op.operands.len() >= 6 {
                    let vals: Vec<f64> = op.operands.iter().filter_map(get_number).collect();
                    if vals.len() >= 6 {
                        let (tx, ty) = transform_point(&gs_stack, vals[4], vals[5]);
                        if let Some((cx, cy)) = current_point {
                            let (cp1x, cp1y) = transform_point(&gs_stack, vals[0], vals[1]);
                            let (cp2x, cp2y) = transform_point(&gs_stack, vals[2], vals[3]);
                            current_path.push(PathSegment::Curve {
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
                        current_point = Some((tx, ty));
                    }
                }
            }
            "v" => {
                // curveto (initial point replicated)
                if op.operands.len() >= 4 {
                    let vals: Vec<f64> = op.operands.iter().filter_map(get_number).collect();
                    if vals.len() >= 4 {
                        let (tx, ty) = transform_point(&gs_stack, vals[2], vals[3]);
                        if let Some((cx, cy)) = current_point {
                            let (cp2x, cp2y) = transform_point(&gs_stack, vals[0], vals[1]);
                            current_path.push(PathSegment::Curve {
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
                        current_point = Some((tx, ty));
                    }
                }
            }
            "y" => {
                // curveto (final point replicated)
                if op.operands.len() >= 4 {
                    let vals: Vec<f64> = op.operands.iter().filter_map(get_number).collect();
                    if vals.len() >= 4 {
                        let (tx, ty) = transform_point(&gs_stack, vals[2], vals[3]);
                        if let Some((cx, cy)) = current_point {
                            let (cp1x, cp1y) = transform_point(&gs_stack, vals[0], vals[1]);
                            current_path.push(PathSegment::Curve {
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
                        current_point = Some((tx, ty));
                    }
                }
            }
            "h" => {
                // closepath
                if let (Some((sx, sy)), Some((cx, cy))) = (subpath_start, current_point) {
                    if (sx - cx).abs() > 0.01 || (sy - cy).abs() > 0.01 {
                        current_path.push(PathSegment::Line {
                            x1: cx,
                            y1: cy,
                            x2: sx,
                            y2: sy,
                        });
                    }
                    current_point = subpath_start;
                }
            }
            "re" => {
                // rectangle
                if op.operands.len() >= 4 {
                    let vals: Vec<f64> = op.operands.iter().filter_map(get_number).collect();
                    if vals.len() >= 4 {
                        let (x, y, w, h) = (vals[0], vals[1], vals[2], vals[3]);
                        let (x1, y1) = transform_point(&gs_stack, x, y);
                        let (x2, y2) = transform_point(&gs_stack, x + w, y);
                        let (x3, y3) = transform_point(&gs_stack, x + w, y + h);
                        let (x4, y4) = transform_point(&gs_stack, x, y + h);
                        current_path.push(PathSegment::Line { x1, y1, x2, y2 });
                        current_path.push(PathSegment::Line {
                            x1: x2,
                            y1: y2,
                            x2: x3,
                            y2: y3,
                        });
                        current_path.push(PathSegment::Line {
                            x1: x3,
                            y1: y3,
                            x2: x4,
                            y2: y4,
                        });
                        current_path.push(PathSegment::Line {
                            x1: x4,
                            y1: y4,
                            x2: x1,
                            y2: y1,
                        });
                        subpath_start = Some((x1, y1));
                        current_point = Some((x1, y1));
                    }
                }
            }
            // Path painting — stroke
            "S" | "s" => {
                if op.operator == "s" {
                    // close and stroke — close first
                    if let (Some((sx, sy)), Some((cx, cy))) = (subpath_start, current_point) {
                        if (sx - cx).abs() > 0.01 || (sy - cy).abs() > 0.01 {
                            current_path.push(PathSegment::Line {
                                x1: cx,
                                y1: cy,
                                x2: sx,
                                y2: sy,
                            });
                        }
                    }
                }
                classify_path(
                    &current_path,
                    line_width,
                    page_number,
                    &mut line_chunks,
                    &mut line_art_chunks,
                    &mut line_index,
                );
                current_path.clear();
                subpath_start = None;
                current_point = None;
            }
            // Fill (also acts as implicit close)
            "f" | "F" | "f*" => {
                classify_path(
                    &current_path,
                    line_width,
                    page_number,
                    &mut line_chunks,
                    &mut line_art_chunks,
                    &mut line_index,
                );
                current_path.clear();
                subpath_start = None;
                current_point = None;
            }
            // Fill and stroke
            "B" | "B*" | "b" | "b*" => {
                classify_path(
                    &current_path,
                    line_width,
                    page_number,
                    &mut line_chunks,
                    &mut line_art_chunks,
                    &mut line_index,
                );
                current_path.clear();
                subpath_start = None;
                current_point = None;
            }
            // End path without painting
            "n" => {
                current_path.clear();
                subpath_start = None;
                current_point = None;
            }
            _ => {}
        }
    }

    Ok((line_chunks, line_art_chunks))
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

/// Transform a point through the current CTM.
fn transform_point(gs_stack: &GraphicsStateStack, x: f64, y: f64) -> (f64, f64) {
    let ctm = &gs_stack.current.ctm;
    ctm.transform_point(x, y)
}

/// Classify a completed path as line chunks or line art.
fn classify_path(
    segments: &[PathSegment],
    line_width: f64,
    page_number: u32,
    line_chunks: &mut Vec<LineChunk>,
    line_art_chunks: &mut Vec<LineArtChunk>,
    index: &mut u32,
) {
    if segments.is_empty() {
        return;
    }

    if line_width < MIN_LINE_WIDTH {
        return;
    }

    let has_curves = segments
        .iter()
        .any(|s| matches!(s, PathSegment::Curve { .. }));

    if !has_curves && segments.len() <= 4 {
        // Try to classify individual segments as line chunks
        let mut classified_lines = Vec::new();
        for seg in segments {
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
                    *index += 1;
                    let min_x = x1.min(*x2);
                    let max_x = x1.max(*x2);
                    let min_y = y1.min(*y2);
                    let max_y = y1.max(*y2);

                    // Expand bbox by half the line width
                    let half_w = line_width / 2.0;
                    let bbox = BoundingBox::new(
                        Some(page_number),
                        min_x - if is_vertical { half_w } else { 0.0 },
                        min_y - if is_horizontal { half_w } else { 0.0 },
                        max_x + if is_vertical { half_w } else { 0.0 },
                        max_y + if is_horizontal { half_w } else { 0.0 },
                    );

                    classified_lines.push(LineChunk {
                        bbox,
                        index: Some(*index),
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
                        width: line_width,
                        is_horizontal_line: is_horizontal,
                        is_vertical_line: is_vertical,
                        is_square: false,
                    });
                }
            }
        }

        if !classified_lines.is_empty() {
            line_chunks.extend(classified_lines);
            return;
        }
    }

    // Check for rectangle (4 lines forming a box)
    if !has_curves && segments.len() == 4 {
        if let Some(rect) = try_classify_rectangle(segments, line_width, page_number, index) {
            line_chunks.push(rect);
            return;
        }
    }

    // Complex path → LineArtChunk
    if segments.len() >= 2 {
        let mut art_lines = Vec::new();
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;

        for seg in segments {
            let (sx, sy, ex, ey) = match seg {
                PathSegment::Line { x1, y1, x2, y2 } => (*x1, *y1, *x2, *y2),
                PathSegment::Curve { x1, y1, x2, y2, .. } => (*x1, *y1, *x2, *y2),
            };
            min_x = min_x.min(sx).min(ex);
            min_y = min_y.min(sy).min(ey);
            max_x = max_x.max(sx).max(ex);
            max_y = max_y.max(sy).max(ey);

            *index += 1;
            let lbbox = BoundingBox::new(
                Some(page_number),
                sx.min(ex),
                sy.min(ey),
                sx.max(ex),
                sy.max(ey),
            );
            art_lines.push(LineChunk {
                bbox: lbbox,
                index: Some(*index),
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
                width: line_width,
                is_horizontal_line: false,
                is_vertical_line: false,
                is_square: false,
            });
        }

        *index += 1;
        let art_bbox = BoundingBox::new(Some(page_number), min_x, min_y, max_x, max_y);
        line_art_chunks.push(LineArtChunk {
            bbox: art_bbox,
            index: Some(*index),
            level: None,
            line_chunks: art_lines,
        });
    }
}

/// Try to classify 4 line segments as a rectangle.
fn try_classify_rectangle(
    segments: &[PathSegment],
    _line_width: f64,
    page_number: u32,
    index: &mut u32,
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

    // Determine if it's a thin rectangle (line) or a square-like shape
    let is_square = (w - h).abs() / w.max(h) < 0.3;

    *index += 1;
    Some(LineChunk {
        bbox: BoundingBox::new(Some(page_number), min_x, min_y, max_x, max_y),
        index: Some(*index),
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

fn get_number(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(f) => Some(*f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{content::Content, content::Operation, dictionary, Stream};

    fn create_doc_with_content(operations: Vec<Operation>) -> (Document, u32, lopdf::ObjectId) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();

        let content = Content { operations };
        let encoded = content.encode().unwrap();
        let content_id = doc.add_object(Stream::new(dictionary! {}, encoded));

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

        let pages_map = doc.get_pages();
        let (&page_num, &pid) = pages_map.iter().next().unwrap();
        (doc, page_num, pid)
    }

    #[test]
    fn test_empty_page_no_lines() {
        let (doc, page_num, pid) = create_doc_with_content(vec![]);
        let (lines, arts) = extract_line_chunks(&doc, page_num, pid).unwrap();
        assert!(lines.is_empty());
        assert!(arts.is_empty());
    }

    #[test]
    fn test_horizontal_line() {
        let ops = vec![
            Operation::new("w", vec![Object::Real(1.0)]),
            Operation::new("m", vec![72.into(), 400.into()]),
            Operation::new("l", vec![500.into(), 400.into()]),
            Operation::new("S", vec![]),
        ];
        let (doc, page_num, pid) = create_doc_with_content(ops);
        let (lines, arts) = extract_line_chunks(&doc, page_num, pid).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_horizontal_line);
        assert!(!lines[0].is_vertical_line);
        assert!(arts.is_empty());
    }

    #[test]
    fn test_vertical_line() {
        let ops = vec![
            Operation::new("w", vec![Object::Real(1.0)]),
            Operation::new("m", vec![200.into(), 100.into()]),
            Operation::new("l", vec![200.into(), 700.into()]),
            Operation::new("S", vec![]),
        ];
        let (doc, page_num, pid) = create_doc_with_content(ops);
        let (lines, arts) = extract_line_chunks(&doc, page_num, pid).unwrap();
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].is_horizontal_line);
        assert!(lines[0].is_vertical_line);
    }

    #[test]
    fn test_rectangle() {
        let ops = vec![
            Operation::new("w", vec![Object::Real(1.0)]),
            Operation::new(
                "re",
                vec![
                    Object::Real(100.0),
                    Object::Real(200.0),
                    Object::Real(300.0),
                    Object::Real(400.0),
                ],
            ),
            Operation::new("S", vec![]),
        ];
        let (doc, page_num, pid) = create_doc_with_content(ops);
        let (lines, _arts) = extract_line_chunks(&doc, page_num, pid).unwrap();
        // Rectangle should be classified as a square-like shape
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_close_and_stroke() {
        let ops = vec![
            Operation::new("w", vec![Object::Real(1.0)]),
            Operation::new("m", vec![72.into(), 400.into()]),
            Operation::new("l", vec![500.into(), 400.into()]),
            Operation::new("l", vec![500.into(), 410.into()]),
            Operation::new("s", vec![]), // close-and-stroke
        ];
        let (doc, page_num, pid) = create_doc_with_content(ops);
        let (lines, arts) = extract_line_chunks(&doc, page_num, pid).unwrap();
        // Should have classified the triangle/shape
        assert!(!lines.is_empty() || !arts.is_empty());
    }
}
