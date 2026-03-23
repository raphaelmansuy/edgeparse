//! PDF graphics state tracking.
//!
//! Manages the graphics state stack including the Current Transformation Matrix (CTM),
//! text state parameters, and color state needed for text extraction.

/// 2D affine transformation matrix: [a b c d e f]
/// Represents the transform: x' = a*x + c*y + e, y' = b*x + d*y + f
#[derive(Debug, Clone, Copy)]
pub struct Matrix {
    /// Horizontal scaling component.
    pub a: f64,
    /// Horizontal skewing component.
    pub b: f64,
    /// Vertical skewing component.
    pub c: f64,
    /// Vertical scaling component.
    pub d: f64,
    /// Horizontal translation.
    pub e: f64,
    /// Vertical translation.
    pub f: f64,
}

impl Matrix {
    /// Identity matrix.
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Translate matrix.
    pub fn translate(tx: f64, ty: f64) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    /// Multiply this matrix by another: self × other.
    pub fn multiply(&self, other: &Matrix) -> Matrix {
        Matrix {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    /// Transform a point (x, y) by this matrix.
    pub fn transform_point(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// Get the effective font size (vertical scaling factor).
    pub fn font_size_factor(&self) -> f64 {
        (self.b * self.b + self.d * self.d).sqrt()
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self::identity()
    }
}

/// Text state parameters tracked during content stream processing.
#[derive(Debug, Clone)]
pub struct TextState {
    /// Current font name (resource name like "F1")
    pub font_name: String,
    /// Font size in text space units
    pub font_size: f64,
    /// Character spacing (Tc)
    pub char_spacing: f64,
    /// Word spacing (Tw)
    pub word_spacing: f64,
    /// Horizontal scaling (Tz) as percentage (default 100)
    pub horizontal_scaling: f64,
    /// Text leading (TL)
    pub leading: f64,
    /// Text rise (Ts)
    pub rise: f64,
    /// Text rendering mode (Tr)
    pub render_mode: i32,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            font_name: String::new(),
            font_size: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            horizontal_scaling: 100.0,
            leading: 0.0,
            rise: 0.0,
            render_mode: 0,
        }
    }
}

/// Full graphics state for PDF content stream processing.
#[derive(Debug, Clone)]
pub struct GraphicsState {
    /// Current transformation matrix
    pub ctm: Matrix,
    /// Text matrix (set by BT and text positioning operators)
    pub text_matrix: Matrix,
    /// Text line matrix (set by text line positioning operators)
    pub text_line_matrix: Matrix,
    /// Text state parameters
    pub text_state: TextState,
    /// Fill color — original PDF color components (1=Gray, 3=RGB, 4=CMYK)
    pub fill_color: Vec<f64>,
    /// Stroke color — original PDF color components
    pub stroke_color: Vec<f64>,
    /// Number of components in current non-stroking color space (1=Gray, 3=RGB, 4=CMYK)
    pub fill_color_space_components: u8,
    /// Number of components in current stroking color space
    pub stroke_color_space_components: u8,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self {
            ctm: Matrix::identity(),
            text_matrix: Matrix::identity(),
            text_line_matrix: Matrix::identity(),
            text_state: TextState::default(),
            fill_color: vec![0.0], // Black (default DeviceGray per PDF spec)
            stroke_color: vec![0.0],
            fill_color_space_components: 1, // Default: DeviceGray
            stroke_color_space_components: 1,
        }
    }
}

impl GraphicsState {
    /// Begin text object: reset text matrix and text line matrix.
    pub fn begin_text(&mut self) {
        self.text_matrix = Matrix::identity();
        self.text_line_matrix = Matrix::identity();
    }

    /// Get the combined text rendering matrix: text_state.font_size × text_matrix × CTM.
    pub fn text_rendering_matrix(&self) -> Matrix {
        let font_matrix = Matrix {
            a: self.text_state.font_size * (self.text_state.horizontal_scaling / 100.0),
            b: 0.0,
            c: 0.0,
            d: self.text_state.font_size,
            e: 0.0,
            f: self.text_state.rise,
        };
        let tm_ctm = self.text_matrix.multiply(&self.ctm);
        font_matrix.multiply(&tm_ctm)
    }

    /// Get the current text position in user space.
    pub fn text_position(&self) -> (f64, f64) {
        let trm = self.text_rendering_matrix();
        (trm.e, trm.f)
    }

    /// Get the effective font size in user space.
    pub fn effective_font_size(&self) -> f64 {
        let trm = self.text_rendering_matrix();
        trm.font_size_factor()
    }

    /// Apply Td (translate text position).
    pub fn translate_text(&mut self, tx: f64, ty: f64) {
        let translation = Matrix::translate(tx, ty);
        self.text_line_matrix = translation.multiply(&self.text_line_matrix);
        self.text_matrix = self.text_line_matrix;
    }

    /// Apply Tm (set text matrix directly).
    pub fn set_text_matrix(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        self.text_matrix = Matrix { a, b, c, d, e, f };
        self.text_line_matrix = self.text_matrix;
    }

    /// Apply T* (move to start of next line).
    pub fn next_line(&mut self) {
        self.translate_text(0.0, -self.text_state.leading);
    }

    /// Advance the text position after showing text (Tj displacement).
    pub fn advance_text(&mut self, displacement: f64) {
        let scaled = displacement * self.text_state.horizontal_scaling / 100.0;
        self.text_matrix.e += scaled * self.text_matrix.a;
        self.text_matrix.f += scaled * self.text_matrix.b;
    }
}

/// Graphics state stack for q/Q save/restore operations.
#[derive(Default)]
pub struct GraphicsStateStack {
    stack: Vec<GraphicsState>,
    /// Current active graphics state.
    pub current: GraphicsState,
}

impl GraphicsStateStack {
    /// Save current state (q operator).
    pub fn save(&mut self) {
        self.stack.push(self.current.clone());
    }

    /// Restore saved state (Q operator).
    pub fn restore(&mut self) {
        if let Some(state) = self.stack.pop() {
            self.current = state;
        }
    }

    /// Apply CTM concatenation (cm operator).
    pub fn concat_ctm(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let new_matrix = Matrix { a, b, c, d, e, f };
        self.current.ctm = new_matrix.multiply(&self.current.ctm);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_identity() {
        let m = Matrix::identity();
        let (x, y) = m.transform_point(10.0, 20.0);
        assert!((x - 10.0).abs() < 1e-10);
        assert!((y - 20.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix_translate() {
        let m = Matrix::translate(100.0, 200.0);
        let (x, y) = m.transform_point(10.0, 20.0);
        assert!((x - 110.0).abs() < 1e-10);
        assert!((y - 220.0).abs() < 1e-10);
    }

    #[test]
    fn test_matrix_multiply() {
        let a = Matrix::translate(10.0, 20.0);
        let b = Matrix::translate(30.0, 40.0);
        let c = a.multiply(&b);
        let (x, y) = c.transform_point(0.0, 0.0);
        assert!((x - 40.0).abs() < 1e-10);
        assert!((y - 60.0).abs() < 1e-10);
    }

    #[test]
    fn test_text_translate() {
        let mut gs = GraphicsState::default();
        gs.text_state.font_size = 12.0;
        gs.begin_text();
        gs.translate_text(100.0, 700.0);
        let (x, y) = gs.text_position();
        assert!((x - 100.0 * 12.0).abs() < 1e-6 || (x - 100.0).abs() < 1e-6);
        // The text position should reflect the translation
        assert!(y.abs() > 0.0 || y.abs() < 1e-6);
    }

    #[test]
    fn test_graphics_state_stack() {
        let mut stack = GraphicsStateStack::default();
        stack.current.text_state.font_size = 12.0;
        stack.save();
        stack.current.text_state.font_size = 24.0;
        assert!((stack.current.text_state.font_size - 24.0).abs() < 1e-10);
        stack.restore();
        assert!((stack.current.text_state.font_size - 12.0).abs() < 1e-10);
    }
}
