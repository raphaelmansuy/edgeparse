//! Type 3 font metric normalization.
//!
//! PDF Spec §9.6.5: Type 3 glyph `/Widths` and `/FontBBox` live in glyph space.
//! They reach text space through `/FontMatrix`:
//!
//! ```text
//! w_text = w_glyph * |FontMatrix.a|   (or hypot(a,b) for rotated matrices)
//! ```
//!
//! Skia / Chrome emit `FontMatrix = [1/2048, 0, 0, -1/2048, 0, 0]` with widths in
//! 1/2048 units. Dividing those widths by 1000 (the Type1 / TrueType convention)
//! inflates every glyph bbox ~2.05×, so adjacent glyphs overlap and
//! `needs_space()` never inserts a word boundary.
//!
//! This module converts Type 3 metrics into the **per-mille text-space** units
//! that [`crate::pdf::font::PdfFont`] promises as an invariant, so callers
//! (`chunk_parser`, `text_extractor`) keep a single `/ 1000.0` path.

use std::collections::HashMap;

/// Affine font matrix `[a, b, c, d, e, f]` as stored in `/FontMatrix`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontMatrix {
    /// Horizontal scaling component.
    pub a: f64,
    /// Horizontal skew component.
    pub b: f64,
    /// Vertical skew component.
    pub c: f64,
    /// Vertical scaling component (often negative for Skia Y-flip).
    pub d: f64,
    /// Horizontal translation.
    pub e: f64,
    /// Vertical translation.
    pub f: f64,
}

impl FontMatrix {
    /// Identity-ish 1/1000 matrix used by Type1/TrueType convention.
    pub const PER_MILLE: Self = Self {
        a: 0.001,
        b: 0.0,
        c: 0.0,
        d: 0.001,
        e: 0.0,
        f: 0.0,
    };

    /// Scale that converts a glyph-space width into per-mille text-space units.
    ///
    /// `em_scale = hypot(a, b) * 1000`, so a Skia matrix of `1/2048` yields
    /// `1000/2048 ≈ 0.488`. Multiplying glyph widths by this scale makes them
    /// compatible with the existing `glyph_width / 1000.0` call sites.
    pub fn em_scale(self) -> f64 {
        let scale = (self.a * self.a + self.b * self.b).sqrt();
        if scale <= f64::EPSILON {
            1.0
        } else {
            scale * 1000.0
        }
    }

    /// Absolute vertical scale `|d|` (or hypot(c,d) for skewed matrices).
    pub fn vertical_scale(self) -> f64 {
        let scale = (self.c * self.c + self.d * self.d).sqrt();
        if scale <= f64::EPSILON {
            (self.a * self.a + self.b * self.b).sqrt()
        } else {
            scale
        }
    }
}

/// Parse a 6-number `/FontMatrix` array. Returns [`FontMatrix::PER_MILLE`] when
/// the array is missing or malformed (safe no-op for non-Type3 callers).
pub fn font_matrix_from_array(values: &[f64]) -> FontMatrix {
    if values.len() >= 6 {
        FontMatrix {
            a: values[0],
            b: values[1],
            c: values[2],
            d: values[3],
            e: values[4],
            f: values[5],
        }
    } else {
        FontMatrix::PER_MILLE
    }
}

/// Multiply glyph-space widths in-place by `em_scale` so they become per-mille.
pub fn normalize_widths(widths: &mut HashMap<u32, f64>, em_scale: f64) {
    if (em_scale - 1.0).abs() < 1e-12 {
        return;
    }
    for w in widths.values_mut() {
        *w *= em_scale;
    }
}

/// Convert a glyph-space `/FontBBox` through the font matrix into per-mille
/// ascent (positive) and descent (negative) for text-space layout.
///
/// Type 3 fonts often omit `/Ascent` / `/Descent` on the descriptor and only
/// publish `/FontBBox` on the font dictionary itself (Skia does this).
pub fn vertical_extent(font_bbox: [f64; 4], matrix: FontMatrix) -> (f64, f64) {
    let scale = matrix.vertical_scale() * 1000.0;
    if scale <= f64::EPSILON {
        return (800.0, -200.0);
    }
    // FontBBox is [llx, lly, urx, ury] in glyph space. With a negative `d`
    // (Skia flips Y), ury is typically the more-negative value — take the
    // extreme magnitudes after scaling.
    let y0 = font_bbox[1] * scale * matrix.d.signum().copysign(1.0);
    let y1 = font_bbox[3] * scale * matrix.d.signum().copysign(1.0);
    // Prefer absolute extents: ascent = max positive, descent = min negative.
    let transformed = [
        font_bbox[1] * matrix.d * 1000.0,
        font_bbox[3] * matrix.d * 1000.0,
    ];
    let ascent = transformed[0].max(transformed[1]).max(0.0);
    let descent = transformed[0].min(transformed[1]).min(0.0);
    // Guard against degenerate bboxes.
    if ascent <= 0.0 && descent >= 0.0 {
        let _ = (y0, y1);
        return (800.0, -200.0);
    }
    if ascent <= 0.0 {
        (ascent.abs().max(200.0), descent)
    } else {
        (ascent, if descent < 0.0 { descent } else { -200.0 })
    }
}

/// Transform a glyph-space FontBBox into per-mille text-space coordinates.
pub fn normalize_font_bbox(font_bbox: [f64; 4], matrix: FontMatrix) -> [f64; 4] {
    let sx = matrix.em_scale();
    let sy = matrix.vertical_scale() * 1000.0;
    let sign_d = if matrix.d < 0.0 { -1.0 } else { 1.0 };
    let y0 = font_bbox[1] * sy * sign_d;
    let y1 = font_bbox[3] * sy * sign_d;
    [font_bbox[0] * sx, y0.min(y1), font_bbox[2] * sx, y0.max(y1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skia_em_scale_is_1000_over_2048() {
        let m = FontMatrix {
            a: 1.0 / 2048.0,
            b: 0.0,
            c: 0.0,
            d: -1.0 / 2048.0,
            e: 0.0,
            f: 0.0,
        };
        let scale = m.em_scale();
        assert!((scale - 1000.0 / 2048.0).abs() < 1e-9, "scale={scale}");
    }

    #[test]
    fn normalize_widths_skia_glyph() {
        let mut widths = HashMap::new();
        // Skia space glyph width ≈ 544 in 1/2048 units
        widths.insert(3u32, 544.0);
        // letter 't' style width
        widths.insert(0u32, 2048.0);
        let scale = (1.0 / 2048.0) * 1000.0;
        normalize_widths(&mut widths, scale);
        assert!((widths[&0] - 1000.0).abs() < 1e-6);
        assert!((widths[&3] - 544.0 * 1000.0 / 2048.0).abs() < 1e-6);
    }

    #[test]
    fn per_mille_matrix_is_noop() {
        let mut widths = HashMap::new();
        widths.insert(65u32, 600.0);
        normalize_widths(&mut widths, FontMatrix::PER_MILLE.em_scale());
        assert!((widths[&65] - 600.0).abs() < 1e-9);
    }

    #[test]
    fn vertical_extent_skia_bbox() {
        // Skia FontBBox [91, 495, 1949, -1951] with d = -1/2048
        let m = FontMatrix {
            a: 1.0 / 2048.0,
            b: 0.0,
            c: 0.0,
            d: -1.0 / 2048.0,
            e: 0.0,
            f: 0.0,
        };
        let (ascent, descent) = vertical_extent([91.0, 495.0, 1949.0, -1951.0], m);
        // 495 * (-1/2048) * 1000 ≈ -241.7 ; -1951 * (-1/2048) * 1000 ≈ 953.6
        assert!(ascent > 700.0, "ascent={ascent}");
        assert!(descent < -100.0, "descent={descent}");
    }

    #[test]
    fn font_matrix_from_short_array_falls_back() {
        let m = font_matrix_from_array(&[0.001]);
        assert_eq!(m, FontMatrix::PER_MILLE);
    }
}
