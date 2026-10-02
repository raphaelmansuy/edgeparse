//! Type 3 CharProc → Unicode cache (appearance-only fonts).
//!
//! When a Type 3 font has no `/ToUnicode` and Differences/AGL failed, the only
//! remaining identity signal is the CharProc appearance. This module owns a
//! per-font code→Unicode cache filled by an optional OCR provider so the rest
//! of the pipeline stays free of raster details (SRP).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use image::GrayImage;

/// Provider that maps a rasterized CharProc glyph to a Unicode string.
pub trait GlyphOcrProvider: Send + Sync {
    /// OCR a single glyph bitmap. Return `None` when recognition fails.
    fn recognize_glyph(&self, gray: &GrayImage) -> Option<String>;
}

/// Shared code→Unicode cache for one Type 3 font resource name.
#[derive(Debug, Default, Clone)]
pub struct Type3GlyphCache {
    inner: Arc<Mutex<HashMap<u32, String>>>,
}

impl Type3GlyphCache {
    /// Empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a previously OCR'd / seeded mapping.
    pub fn get(&self, code: u32) -> Option<String> {
        self.inner.lock().ok()?.get(&code).cloned()
    }

    /// Insert or overwrite a mapping.
    pub fn insert(&self, code: u32, unicode: String) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(code, unicode);
        }
    }

    /// Fill vacant entries by OCR'ing provided bitmaps.
    ///
    /// `glyphs` is `(char_code, gray bitmap)`. Already-cached codes are skipped.
    pub fn fill_from_bitmaps<P: GlyphOcrProvider + ?Sized>(
        &self,
        glyphs: &[(u32, GrayImage)],
        ocr: &P,
    ) {
        for (code, gray) in glyphs {
            if self.get(*code).is_some() {
                continue;
            }
            if let Some(text) = ocr.recognize_glyph(gray) {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    self.insert(*code, trimmed);
                }
            }
        }
    }

    /// Merge cache entries into a ToUnicode map (vacant keys only).
    pub fn merge_into(&self, to_unicode: &mut HashMap<u32, String>) {
        let Ok(g) = self.inner.lock() else {
            return;
        };
        for (code, uni) in g.iter() {
            to_unicode.entry(*code).or_insert_with(|| uni.clone());
        }
    }
}

/// No-op OCR provider (used when OCR is disabled / WASM without engine).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullGlyphOcr;

impl GlyphOcrProvider for NullGlyphOcr {
    fn recognize_glyph(&self, _gray: &GrayImage) -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubOcr;
    impl GlyphOcrProvider for StubOcr {
        fn recognize_glyph(&self, gray: &GrayImage) -> Option<String> {
            // Any non-empty bitmap → "A"
            if gray.iter().any(|&p| p < 200) {
                Some("A".to_string())
            } else {
                None
            }
        }
    }

    #[test]
    fn cache_fill_and_merge() {
        let cache = Type3GlyphCache::new();
        let mut img = GrayImage::new(8, 8);
        img[(2, 2)] = image::Luma([0]);
        cache.fill_from_bitmaps(&[(65, img)], &StubOcr);
        assert_eq!(cache.get(65).as_deref(), Some("A"));

        let mut map = HashMap::new();
        map.insert(65u32, "Z".to_string()); // existing wins
        cache.merge_into(&mut map);
        assert_eq!(map.get(&65).map(String::as_str), Some("Z"));

        let mut empty = HashMap::new();
        cache.merge_into(&mut empty);
        assert_eq!(empty.get(&65).map(String::as_str), Some("A"));
    }
}
