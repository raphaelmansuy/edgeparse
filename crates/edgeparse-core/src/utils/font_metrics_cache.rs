//! Font metrics cache — caches computed string widths and font measurements
//! to avoid redundant calculations during pipeline stages.

use std::collections::HashMap;

/// A computed text measurement result.
#[derive(Debug, Clone, PartialEq)]
pub struct TextMeasurement {
    /// Width of the text in PDF user space units.
    pub width: f64,
    /// Number of characters measured.
    pub char_count: usize,
    /// Average character width.
    pub avg_char_width: f64,
}

/// Cache key combining font identity and text content.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    font_name: String,
    font_size_millis: i64, // font_size * 1000 as integer for hashing
    text: String,
}

/// A font metrics cache that stores computed measurements to avoid
/// redundant width calculations during pipeline processing.
#[derive(Debug)]
pub struct FontMetricsCache {
    measurements: HashMap<CacheKey, TextMeasurement>,
    hits: u64,
    misses: u64,
    max_entries: usize,
}

impl FontMetricsCache {
    /// Create a new cache with a maximum entry limit.
    pub fn new(max_entries: usize) -> Self {
        Self {
            measurements: HashMap::with_capacity(max_entries.min(4096)),
            hits: 0,
            misses: 0,
            max_entries,
        }
    }

    /// Create a cache with default capacity (10,000 entries).
    pub fn default_capacity() -> Self {
        Self::new(10_000)
    }

    /// Look up a cached measurement. Returns None on cache miss.
    pub fn get(&mut self, font_name: &str, font_size: f64, text: &str) -> Option<&TextMeasurement> {
        let key = Self::make_key(font_name, font_size, text);
        if self.measurements.contains_key(&key) {
            self.hits += 1;
            self.measurements.get(&key)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Store a measurement in the cache.
    /// If the cache is full, it will be cleared (simple eviction strategy).
    pub fn put(
        &mut self,
        font_name: &str,
        font_size: f64,
        text: &str,
        measurement: TextMeasurement,
    ) {
        if self.measurements.len() >= self.max_entries {
            self.measurements.clear();
        }
        let key = Self::make_key(font_name, font_size, text);
        self.measurements.insert(key, measurement);
    }

    /// Get or compute a measurement, using the cache if available.
    pub fn get_or_compute<F>(
        &mut self,
        font_name: &str,
        font_size: f64,
        text: &str,
        compute: F,
    ) -> TextMeasurement
    where
        F: FnOnce() -> TextMeasurement,
    {
        let key = Self::make_key(font_name, font_size, text);
        if let Some(cached) = self.measurements.get(&key) {
            self.hits += 1;
            return cached.clone();
        }
        self.misses += 1;
        let result = compute();
        if self.measurements.len() >= self.max_entries {
            self.measurements.clear();
        }
        self.measurements.insert(key, result.clone());
        result
    }

    /// Estimate the width of a text string given per-character widths from a font.
    /// This is a convenience method using a simple character-width lookup.
    pub fn estimate_width(
        &mut self,
        font_name: &str,
        font_size: f64,
        text: &str,
        char_widths: &HashMap<char, f64>,
        default_width: f64,
    ) -> TextMeasurement {
        self.get_or_compute(font_name, font_size, text, || {
            let scale = font_size / 1000.0;
            let mut total_width = 0.0;
            let char_count = text.chars().count();
            for ch in text.chars() {
                let glyph_width = char_widths.get(&ch).copied().unwrap_or(default_width);
                total_width += glyph_width * scale;
            }
            let avg = if char_count > 0 {
                total_width / char_count as f64
            } else {
                0.0
            };
            TextMeasurement {
                width: total_width,
                char_count,
                avg_char_width: avg,
            }
        })
    }

    /// Number of cache hits.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Number of cache misses.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Cache hit rate as a fraction (0.0 to 1.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.measurements.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.measurements.is_empty()
    }

    /// Clear all cached entries and reset counters.
    pub fn clear(&mut self) {
        self.measurements.clear();
        self.hits = 0;
        self.misses = 0;
    }

    fn make_key(font_name: &str, font_size: f64, text: &str) -> CacheKey {
        CacheKey {
            font_name: font_name.to_string(),
            font_size_millis: (font_size * 1000.0) as i64,
            text: text.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit_and_miss() {
        let mut cache = FontMetricsCache::new(100);
        // Miss first time
        assert!(cache.get("Helvetica", 12.0, "hello").is_none());
        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 0);

        // Store and hit
        cache.put(
            "Helvetica",
            12.0,
            "hello",
            TextMeasurement {
                width: 25.0,
                char_count: 5,
                avg_char_width: 5.0,
            },
        );
        let m = cache.get("Helvetica", 12.0, "hello").unwrap();
        assert_eq!(m.width, 25.0);
        assert_eq!(cache.hits(), 1);
    }

    #[test]
    fn test_get_or_compute() {
        let mut cache = FontMetricsCache::new(100);
        let result = cache.get_or_compute("Arial", 10.0, "test", || TextMeasurement {
            width: 20.0,
            char_count: 4,
            avg_char_width: 5.0,
        });
        assert_eq!(result.width, 20.0);
        assert_eq!(cache.misses(), 1);

        // Second call should be cached
        let result2 = cache.get_or_compute("Arial", 10.0, "test", || TextMeasurement {
            width: 999.0, // should NOT be used
            char_count: 0,
            avg_char_width: 0.0,
        });
        assert_eq!(result2.width, 20.0); // Still 20, not 999
        assert_eq!(cache.hits(), 1);
    }

    #[test]
    fn test_estimate_width() {
        let mut cache = FontMetricsCache::default_capacity();
        let mut widths = HashMap::new();
        widths.insert('H', 700.0);
        widths.insert('i', 300.0);

        let m = cache.estimate_width("Helvetica", 12.0, "Hi", &widths, 500.0);
        assert_eq!(m.char_count, 2);
        // width = (700 * 12/1000) + (300 * 12/1000) = 8.4 + 3.6 = 12.0
        assert!((m.width - 12.0).abs() < 0.001);
        assert!((m.avg_char_width - 6.0).abs() < 0.001);
    }

    #[test]
    fn test_eviction_on_full() {
        let mut cache = FontMetricsCache::new(3);
        for i in 0..3 {
            cache.put(
                "F",
                10.0,
                &format!("text{}", i),
                TextMeasurement {
                    width: i as f64,
                    char_count: 1,
                    avg_char_width: i as f64,
                },
            );
        }
        assert_eq!(cache.len(), 3);

        // 4th entry triggers eviction (clear)
        cache.put(
            "F",
            10.0,
            "text3",
            TextMeasurement {
                width: 3.0,
                char_count: 1,
                avg_char_width: 3.0,
            },
        );
        assert_eq!(cache.len(), 1); // only the new entry remains
    }

    #[test]
    fn test_hit_rate() {
        let mut cache = FontMetricsCache::new(100);
        cache.put(
            "F",
            10.0,
            "a",
            TextMeasurement {
                width: 1.0,
                char_count: 1,
                avg_char_width: 1.0,
            },
        );
        let _ = cache.get("F", 10.0, "a"); // hit
        let _ = cache.get("F", 10.0, "a"); // hit
        let _ = cache.get("F", 10.0, "b"); // miss
        assert_eq!(cache.hits(), 2);
        assert_eq!(cache.misses(), 1);
        assert!((cache.hit_rate() - 2.0 / 3.0).abs() < 0.001);
    }
}
