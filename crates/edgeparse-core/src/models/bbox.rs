//! BoundingBox — Core geometry type for all positioned PDF elements.

use serde::{Deserialize, Serialize};

/// Epsilon for floating-point comparisons in bounding box operations.
const BBOX_EPSILON: f64 = 0.0001;

/// A rectangular bounding box in PDF coordinate space.
///
/// PDF coordinates: origin at bottom-left, Y increases upward.
/// 72 points = 1 inch.
///
/// ```text
/// (leftX, topY) ──────── (rightX, topY)
///       │                      │
///       │      Element         │
///       │                      │
/// (leftX, bottomY) ──── (rightX, bottomY)
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Page number (1-based)
    pub page_number: Option<u32>,
    /// Last page number (for cross-page elements)
    pub last_page_number: Option<u32>,
    /// Left X coordinate
    pub left_x: f64,
    /// Bottom Y coordinate
    pub bottom_y: f64,
    /// Right X coordinate
    pub right_x: f64,
    /// Top Y coordinate
    pub top_y: f64,
}

impl PartialEq for BoundingBox {
    fn eq(&self, other: &Self) -> bool {
        self.page_number == other.page_number
            && self.last_page_number == other.last_page_number
            && (self.left_x - other.left_x).abs() < BBOX_EPSILON
            && (self.bottom_y - other.bottom_y).abs() < BBOX_EPSILON
            && (self.right_x - other.right_x).abs() < BBOX_EPSILON
            && (self.top_y - other.top_y).abs() < BBOX_EPSILON
    }
}

impl BoundingBox {
    /// Create a new BoundingBox.
    pub fn new(page: Option<u32>, left_x: f64, bottom_y: f64, right_x: f64, top_y: f64) -> Self {
        Self {
            page_number: page,
            last_page_number: page,
            left_x,
            bottom_y,
            right_x,
            top_y,
        }
    }

    /// Create an empty bounding box (zero area at origin).
    pub fn empty() -> Self {
        Self {
            page_number: None,
            last_page_number: None,
            left_x: 0.0,
            bottom_y: 0.0,
            right_x: 0.0,
            top_y: 0.0,
        }
    }

    /// Width of the bounding box.
    pub fn width(&self) -> f64 {
        self.right_x - self.left_x
    }

    /// Height of the bounding box.
    pub fn height(&self) -> f64 {
        self.top_y - self.bottom_y
    }

    /// Area of the bounding box.
    pub fn area(&self) -> f64 {
        let w = self.width();
        let h = self.height();
        if w < 0.0 || h < 0.0 {
            0.0
        } else {
            w * h
        }
    }

    /// Center X coordinate.
    pub fn center_x(&self) -> f64 {
        (self.left_x + self.right_x) / 2.0
    }

    /// Center Y coordinate.
    pub fn center_y(&self) -> f64 {
        (self.bottom_y + self.top_y) / 2.0
    }

    /// Whether this bounding box has zero or negative area.
    pub fn is_empty(&self) -> bool {
        self.width() <= BBOX_EPSILON || self.height() <= BBOX_EPSILON
    }

    /// Whether this element is on a single page.
    pub fn is_one_page(&self) -> bool {
        match (self.page_number, self.last_page_number) {
            (Some(p), Some(lp)) => p == lp,
            (Some(_), None) | (None, None) => true,
            _ => false,
        }
    }

    /// Whether this element spans multiple pages.
    pub fn is_multi_page(&self) -> bool {
        !self.is_one_page()
    }

    /// Normalize: ensure left < right and bottom < top.
    pub fn normalize(&mut self) {
        if self.left_x > self.right_x {
            std::mem::swap(&mut self.left_x, &mut self.right_x);
        }
        if self.bottom_y > self.top_y {
            std::mem::swap(&mut self.bottom_y, &mut self.top_y);
        }
    }

    /// Compute the union of two bounding boxes.
    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        BoundingBox {
            page_number: self.page_number.or(other.page_number),
            last_page_number: match (self.last_page_number, other.last_page_number) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            left_x: self.left_x.min(other.left_x),
            bottom_y: self.bottom_y.min(other.bottom_y),
            right_x: self.right_x.max(other.right_x),
            top_y: self.top_y.max(other.top_y),
        }
    }

    /// Whether this bounding box overlaps with another.
    pub fn overlaps(&self, other: &BoundingBox) -> bool {
        self.left_x < other.right_x
            && self.right_x > other.left_x
            && self.bottom_y < other.top_y
            && self.top_y > other.bottom_y
    }

    /// Whether this bounding box fully contains another.
    pub fn contains(&self, other: &BoundingBox) -> bool {
        self.left_x <= other.left_x + BBOX_EPSILON
            && self.right_x >= other.right_x - BBOX_EPSILON
            && self.bottom_y <= other.bottom_y + BBOX_EPSILON
            && self.top_y >= other.top_y - BBOX_EPSILON
    }

    /// Weakly contains: allows small margin of error.
    pub fn weakly_contains(&self, other: &BoundingBox) -> bool {
        let margin = 1.0; // 1 PDF point tolerance
        self.left_x <= other.left_x + margin
            && self.right_x >= other.right_x - margin
            && self.bottom_y <= other.bottom_y + margin
            && self.top_y >= other.top_y - margin
    }

    /// Compute intersection area percentage relative to `other`.
    pub fn intersection_percent(&self, other: &BoundingBox) -> f64 {
        let ix_left = self.left_x.max(other.left_x);
        let ix_right = self.right_x.min(other.right_x);
        let iy_bottom = self.bottom_y.max(other.bottom_y);
        let iy_top = self.top_y.min(other.top_y);

        if ix_left >= ix_right || iy_bottom >= iy_top {
            return 0.0;
        }

        let intersection_area = (ix_right - ix_left) * (iy_top - iy_bottom);
        let other_area = other.area();

        if other_area <= BBOX_EPSILON {
            return 0.0;
        }

        intersection_area / other_area
    }

    /// Vertical intersection percentage.
    pub fn vertical_intersection_percent(&self, other: &BoundingBox) -> f64 {
        let iy_bottom = self.bottom_y.max(other.bottom_y);
        let iy_top = self.top_y.min(other.top_y);

        if iy_bottom >= iy_top {
            return 0.0;
        }

        let intersection_height = iy_top - iy_bottom;
        let other_height = other.height();

        if other_height <= BBOX_EPSILON {
            return 0.0;
        }

        intersection_height / other_height
    }

    /// Vertical gap between two bounding boxes.
    pub fn vertical_gap(&self, other: &BoundingBox) -> f64 {
        if self.top_y < other.bottom_y {
            other.bottom_y - self.top_y
        } else if other.top_y < self.bottom_y {
            self.bottom_y - other.top_y
        } else {
            0.0 // overlapping
        }
    }

    /// Horizontal gap between two bounding boxes.
    pub fn horizontal_gap(&self, other: &BoundingBox) -> f64 {
        if self.right_x < other.left_x {
            other.left_x - self.right_x
        } else if other.right_x < self.left_x {
            self.left_x - other.right_x
        } else {
            0.0 // overlapping
        }
    }

    /// Whether two bounding boxes horizontally overlap.
    pub fn are_horizontal_overlapping(&self, other: &BoundingBox) -> bool {
        self.left_x < other.right_x && self.right_x > other.left_x
    }

    /// Whether two bounding boxes vertically overlap.
    pub fn are_vertical_overlapping(&self, other: &BoundingBox) -> bool {
        self.bottom_y < other.top_y && self.top_y > other.bottom_y
    }

    /// Scale the bounding box by a factor.
    pub fn scale(&mut self, factor: f64) {
        let cx = self.center_x();
        let cy = self.center_y();
        let half_w = self.width() * factor / 2.0;
        let half_h = self.height() * factor / 2.0;
        self.left_x = cx - half_w;
        self.right_x = cx + half_w;
        self.bottom_y = cy - half_h;
        self.top_y = cy + half_h;
    }

    /// Translate the bounding box by (dx, dy).
    pub fn translate(&mut self, dx: f64, dy: f64) {
        self.left_x += dx;
        self.right_x += dx;
        self.bottom_y += dy;
        self.top_y += dy;
    }

    /// Serialize to JSON array format: [leftX, bottomY, rightX, topY]
    pub fn to_json_array(&self) -> [f64; 4] {
        [self.left_x, self.bottom_y, self.right_x, self.top_y]
    }
}

/// A multi-bounding-box: outer boundary + inner fragments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiBoundingBox {
    /// Outer boundary encompassing all inner boxes
    pub outer: BoundingBox,
    /// Inner fragment bounding boxes
    pub inner: Vec<BoundingBox>,
}

/// A point with radius (used for line endpoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
    /// Radius (for round line caps)
    pub radius: f64,
}

impl Vertex {
    /// Create a new vertex.
    pub fn new(x: f64, y: f64, radius: f64) -> Self {
        Self { x, y, radius }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bbox_new() {
        let bbox = BoundingBox::new(Some(1), 10.0, 20.0, 100.0, 80.0);
        assert_eq!(bbox.page_number, Some(1));
        assert_eq!(bbox.left_x, 10.0);
        assert_eq!(bbox.bottom_y, 20.0);
        assert_eq!(bbox.right_x, 100.0);
        assert_eq!(bbox.top_y, 80.0);
    }

    #[test]
    fn test_bbox_dimensions() {
        let bbox = BoundingBox::new(Some(1), 10.0, 20.0, 110.0, 80.0);
        assert!((bbox.width() - 100.0).abs() < BBOX_EPSILON);
        assert!((bbox.height() - 60.0).abs() < BBOX_EPSILON);
        assert!((bbox.area() - 6000.0).abs() < BBOX_EPSILON);
        assert!((bbox.center_x() - 60.0).abs() < BBOX_EPSILON);
        assert!((bbox.center_y() - 50.0).abs() < BBOX_EPSILON);
    }

    #[test]
    fn test_bbox_empty() {
        let bbox = BoundingBox::empty();
        assert!(bbox.is_empty());
        assert_eq!(bbox.area(), 0.0);
    }

    #[test]
    fn test_bbox_normalize() {
        let mut bbox = BoundingBox::new(Some(1), 100.0, 80.0, 10.0, 20.0);
        bbox.normalize();
        assert_eq!(bbox.left_x, 10.0);
        assert_eq!(bbox.bottom_y, 20.0);
        assert_eq!(bbox.right_x, 100.0);
        assert_eq!(bbox.top_y, 80.0);
    }

    #[test]
    fn test_bbox_union() {
        let a = BoundingBox::new(Some(1), 10.0, 20.0, 50.0, 60.0);
        let b = BoundingBox::new(Some(1), 30.0, 10.0, 80.0, 50.0);
        let u = a.union(&b);
        assert_eq!(u.left_x, 10.0);
        assert_eq!(u.bottom_y, 10.0);
        assert_eq!(u.right_x, 80.0);
        assert_eq!(u.top_y, 60.0);
    }

    #[test]
    fn test_bbox_overlaps() {
        let a = BoundingBox::new(Some(1), 0.0, 0.0, 50.0, 50.0);
        let b = BoundingBox::new(Some(1), 25.0, 25.0, 75.0, 75.0);
        let c = BoundingBox::new(Some(1), 60.0, 60.0, 100.0, 100.0);
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn test_bbox_contains() {
        let outer = BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 100.0);
        let inner = BoundingBox::new(Some(1), 10.0, 10.0, 90.0, 90.0);
        let partial = BoundingBox::new(Some(1), 50.0, 50.0, 150.0, 150.0);
        assert!(outer.contains(&inner));
        assert!(!outer.contains(&partial));
    }

    #[test]
    fn test_bbox_intersection_percent() {
        let a = BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 100.0);
        let b = BoundingBox::new(Some(1), 50.0, 50.0, 150.0, 150.0);
        // Intersection = 50x50 = 2500, b area = 100x100 = 10000
        let pct = a.intersection_percent(&b);
        assert!((pct - 0.25).abs() < 0.01);
    }

    #[test]
    fn test_bbox_vertical_gap() {
        let a = BoundingBox::new(Some(1), 0.0, 50.0, 100.0, 100.0);
        let b = BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 30.0);
        assert!((a.vertical_gap(&b) - 20.0).abs() < BBOX_EPSILON);
    }

    #[test]
    fn test_bbox_horizontal_gap() {
        let a = BoundingBox::new(Some(1), 0.0, 0.0, 50.0, 100.0);
        let b = BoundingBox::new(Some(1), 80.0, 0.0, 150.0, 100.0);
        assert!((a.horizontal_gap(&b) - 30.0).abs() < BBOX_EPSILON);
    }

    #[test]
    fn test_bbox_is_one_page() {
        let mut bbox = BoundingBox::new(Some(1), 0.0, 0.0, 100.0, 100.0);
        assert!(bbox.is_one_page());
        bbox.last_page_number = Some(3);
        assert!(bbox.is_multi_page());
    }

    #[test]
    fn test_bbox_translate() {
        let mut bbox = BoundingBox::new(Some(1), 10.0, 20.0, 50.0, 60.0);
        bbox.translate(5.0, -10.0);
        assert!((bbox.left_x - 15.0).abs() < BBOX_EPSILON);
        assert!((bbox.bottom_y - 10.0).abs() < BBOX_EPSILON);
        assert!((bbox.right_x - 55.0).abs() < BBOX_EPSILON);
        assert!((bbox.top_y - 50.0).abs() < BBOX_EPSILON);
    }

    #[test]
    fn test_bbox_to_json_array() {
        let bbox = BoundingBox::new(Some(1), 10.0, 20.0, 100.0, 80.0);
        let arr = bbox.to_json_array();
        assert_eq!(arr, [10.0, 20.0, 100.0, 80.0]);
    }

    #[test]
    fn test_vertex() {
        let v = Vertex::new(10.0, 20.0, 1.5);
        assert_eq!(v.x, 10.0);
        assert_eq!(v.y, 20.0);
        assert_eq!(v.radius, 1.5);
    }
}
