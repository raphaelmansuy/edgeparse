//! Figure Detection
//!
//! Groups standalone images and adjacent line art chunks into
//! SemanticFigure elements for structured output.

use crate::models::bbox::BoundingBox;
use crate::models::content::ContentElement;
use crate::models::enums::SemanticType;
use crate::models::semantic::SemanticFigure;

/// Maximum gap between image and adjacent line art to group them.
const MAX_FIGURE_GAP: f64 = 10.0;

/// Detect and group images into figure elements.
pub fn detect_figures(elements: Vec<ContentElement>) -> Vec<ContentElement> {
    if elements.is_empty() {
        return elements;
    }

    let mut result: Vec<ContentElement> = Vec::with_capacity(elements.len());
    let mut i = 0;

    while i < elements.len() {
        match &elements[i] {
            ContentElement::Image(img) => {
                // Start a figure from this image
                let mut images = vec![img.clone()];
                let mut line_arts = Vec::new();
                let mut bbox = img.bbox.clone();
                i += 1;

                // Absorb adjacent images and line art
                while i < elements.len() {
                    match &elements[i] {
                        ContentElement::Image(next_img) => {
                            if is_adjacent(&bbox, &next_img.bbox) {
                                bbox = bbox.union(&next_img.bbox);
                                images.push(next_img.clone());
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        ContentElement::LineArt(la) => {
                            if is_adjacent(&bbox, &la.bbox) {
                                bbox = bbox.union(&la.bbox);
                                line_arts.push(la.clone());
                                i += 1;
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }

                result.push(ContentElement::Figure(SemanticFigure {
                    bbox,
                    index: None,
                    level: None,
                    semantic_type: SemanticType::Figure,
                    images,
                    line_arts,
                }));
            }
            _ => {
                result.push(elements[i].clone());
                i += 1;
            }
        }
    }

    result
}

/// Check if two bounding boxes are adjacent (within MAX_FIGURE_GAP).
fn is_adjacent(a: &BoundingBox, b: &BoundingBox) -> bool {
    // Vertical gap
    let v_gap = if a.bottom_y > b.top_y {
        a.bottom_y - b.top_y
    } else if b.bottom_y > a.top_y {
        b.bottom_y - a.top_y
    } else {
        0.0
    };

    // Horizontal gap
    let h_gap = if a.left_x > b.right_x {
        a.left_x - b.right_x
    } else if b.left_x > a.right_x {
        b.left_x - a.right_x
    } else {
        0.0
    };

    v_gap <= MAX_FIGURE_GAP && h_gap <= MAX_FIGURE_GAP
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::bbox::BoundingBox;
    use crate::models::chunks::{ImageChunk, LineArtChunk};

    fn make_image(left: f64, bottom: f64, right: f64, top: f64) -> ContentElement {
        ContentElement::Image(ImageChunk {
            bbox: BoundingBox::new(Some(1), left, bottom, right, top),
            index: None,
            level: None,
        })
    }

    fn make_line_art(left: f64, bottom: f64, right: f64, top: f64) -> ContentElement {
        ContentElement::LineArt(LineArtChunk {
            bbox: BoundingBox::new(Some(1), left, bottom, right, top),
            index: None,
            level: None,
            line_chunks: vec![],
        })
    }

    #[test]
    fn test_single_image_becomes_figure() {
        let elements = vec![make_image(100.0, 300.0, 400.0, 500.0)];
        let result = detect_figures(elements);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0], ContentElement::Figure(_)));
    }

    #[test]
    fn test_adjacent_images_grouped() {
        let elements = vec![
            make_image(100.0, 300.0, 400.0, 500.0),
            make_image(100.0, 200.0, 400.0, 298.0), // 2pt gap
        ];
        let result = detect_figures(elements);
        assert_eq!(result.len(), 1);
        if let ContentElement::Figure(f) = &result[0] {
            assert_eq!(f.images.len(), 2);
        }
    }

    #[test]
    fn test_image_with_line_art() {
        let elements = vec![
            make_image(100.0, 300.0, 400.0, 500.0),
            make_line_art(100.0, 290.0, 400.0, 300.0),
        ];
        let result = detect_figures(elements);
        assert_eq!(result.len(), 1);
        if let ContentElement::Figure(f) = &result[0] {
            assert_eq!(f.images.len(), 1);
            assert_eq!(f.line_arts.len(), 1);
        }
    }

    #[test]
    fn test_distant_images_separate() {
        let elements = vec![
            make_image(100.0, 600.0, 400.0, 700.0),
            make_image(100.0, 100.0, 400.0, 200.0), // 400pt gap
        ];
        let result = detect_figures(elements);
        assert_eq!(result.len(), 2);
    }
}
