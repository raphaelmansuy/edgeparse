//! Font and text statistics.

use std::collections::HashMap;

/// Mode-weighted font statistics for heading/paragraph classification.
#[derive(Debug, Clone, Default)]
pub struct ModeWeightStatistics {
    /// font_size → total weight (character count × frequency)
    weights: HashMap<ordered_float::OrderedFloat<f64>, f64>,
    /// Total character count
    total_count: usize,
}

impl ModeWeightStatistics {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a font size observation with the given character count.
    pub fn add(&mut self, font_size: f64, char_count: usize) {
        let key = ordered_float::OrderedFloat(font_size);
        *self.weights.entry(key).or_insert(0.0) += char_count as f64;
        self.total_count += char_count;
    }

    /// Get the mode (most frequent) font size.
    pub fn mode_font_size(&self) -> Option<f64> {
        self.weights
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, _)| k.into_inner())
    }

    /// Total character count.
    pub fn total_count(&self) -> usize {
        self.total_count
    }

    /// Whether a font size is larger than the mode (potential heading).
    pub fn is_larger_than_mode(&self, font_size: f64) -> bool {
        match self.mode_font_size() {
            Some(mode) => font_size > mode + 0.5,
            None => false,
        }
    }

    /// Whether a font size matches the mode (body text).
    pub fn is_mode_size(&self, font_size: f64) -> bool {
        match self.mode_font_size() {
            Some(mode) => (font_size - mode).abs() < 0.5,
            None => false,
        }
    }
}

/// Text style descriptor for line/paragraph comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Font name
    pub font_name: String,
    /// Font size
    pub font_size: f64,
    /// Font weight
    pub font_weight: f64,
    /// Is bold
    pub is_bold: bool,
    /// Is italic
    pub is_italic: bool,
}

impl TextStyle {
    /// Whether two styles are compatible (same font properties).
    pub fn is_compatible(&self, other: &TextStyle) -> bool {
        self.font_name == other.font_name
            && (self.font_size - other.font_size).abs() < 0.5
            && self.is_bold == other.is_bold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_weight_statistics() {
        let mut stats = ModeWeightStatistics::new();
        stats.add(12.0, 100);
        stats.add(14.0, 20);
        stats.add(12.0, 200);
        assert!((stats.mode_font_size().unwrap() - 12.0).abs() < 0.01);
        assert!(stats.is_mode_size(12.0));
        assert!(stats.is_larger_than_mode(14.0));
        assert!(!stats.is_larger_than_mode(12.0));
    }
}
