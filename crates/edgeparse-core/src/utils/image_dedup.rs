//! Image deduplication — identifies duplicate images across pages
//! using content hashing to reduce output size and improve processing.

use std::collections::HashMap;

/// An image fingerprint for deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageFingerprint {
    /// Hash of the raw image data.
    pub content_hash: u64,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Information about a deduplicated image.
#[derive(Debug, Clone)]
pub struct DeduplicatedImage {
    /// Unique fingerprint for this image.
    pub fingerprint: ImageFingerprint,
    /// Page numbers where this image appears (1-based).
    pub page_numbers: Vec<u32>,
    /// Number of occurrences.
    pub occurrence_count: usize,
}

/// An image reference before deduplication.
#[derive(Debug, Clone)]
pub struct ImageRef {
    /// Raw image data (or a portion for fingerprinting).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Page number (1-based).
    pub page_number: u32,
    /// Object ID in the PDF.
    pub object_id: Option<(u32, u16)>,
}

/// Compute a fingerprint for image data using FNV-1a hash.
pub fn fingerprint(image: &ImageRef) -> ImageFingerprint {
    ImageFingerprint {
        content_hash: fnv1a_hash(&image.data),
        width: image.width,
        height: image.height,
    }
}

/// Deduplicate a list of image references, grouping identical images.
pub fn deduplicate(images: &[ImageRef]) -> Vec<DeduplicatedImage> {
    let mut groups: HashMap<ImageFingerprint, Vec<u32>> = HashMap::new();

    for img in images {
        let fp = fingerprint(img);
        groups.entry(fp).or_default().push(img.page_number);
    }

    let mut result: Vec<DeduplicatedImage> = groups
        .into_iter()
        .map(|(fp, pages)| DeduplicatedImage {
            fingerprint: fp,
            occurrence_count: pages.len(),
            page_numbers: pages,
        })
        .collect();

    result.sort_by(|a, b| b.occurrence_count.cmp(&a.occurrence_count));
    result
}

/// Count how many images are duplicates (total - unique).
pub fn duplicate_count(images: &[ImageRef]) -> usize {
    let deduped = deduplicate(images);
    let unique = deduped.len();
    images.len().saturating_sub(unique)
}

/// Compute deduplication savings ratio (0.0 = no savings, 1.0 = all duplicates).
pub fn savings_ratio(images: &[ImageRef]) -> f64 {
    if images.is_empty() {
        return 0.0;
    }
    duplicate_count(images) as f64 / images.len() as f64
}

/// FNV-1a 64-bit hash for byte slices.
fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_image(data: &[u8], page: u32, w: u32, h: u32) -> ImageRef {
        ImageRef {
            data: data.to_vec(),
            width: w,
            height: h,
            page_number: page,
            object_id: None,
        }
    }

    #[test]
    fn test_fingerprint_deterministic() {
        let img = make_image(b"hello image data", 1, 100, 50);
        let fp1 = fingerprint(&img);
        let fp2 = fingerprint(&img);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_fingerprint_different_data() {
        let img1 = make_image(b"image A", 1, 100, 50);
        let img2 = make_image(b"image B", 1, 100, 50);
        assert_ne!(fingerprint(&img1), fingerprint(&img2));
    }

    #[test]
    fn test_deduplicate() {
        let images = vec![
            make_image(b"logo", 1, 50, 50),
            make_image(b"logo", 2, 50, 50),    // duplicate
            make_image(b"logo", 3, 50, 50),    // duplicate
            make_image(b"photo", 1, 200, 150), // unique
        ];
        let deduped = deduplicate(&images);
        assert_eq!(deduped.len(), 2); // 2 unique images
                                      // "logo" has 3 occurrences, should be first (sorted by count desc)
        assert_eq!(deduped[0].occurrence_count, 3);
        assert_eq!(deduped[0].page_numbers.len(), 3);
        assert_eq!(deduped[1].occurrence_count, 1);
    }

    #[test]
    fn test_duplicate_count() {
        let images = vec![
            make_image(b"same", 1, 10, 10),
            make_image(b"same", 2, 10, 10),
            make_image(b"diff", 3, 10, 10),
        ];
        assert_eq!(duplicate_count(&images), 1); // 3 images - 2 unique = 1 dup
    }

    #[test]
    fn test_savings_ratio() {
        let images = vec![
            make_image(b"x", 1, 10, 10),
            make_image(b"x", 2, 10, 10),
            make_image(b"x", 3, 10, 10),
            make_image(b"x", 4, 10, 10),
        ];
        let ratio = savings_ratio(&images);
        // 4 images, 1 unique = 3 duplicates. 3/4 = 0.75
        assert!((ratio - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_empty_images() {
        let images: Vec<ImageRef> = vec![];
        assert_eq!(duplicate_count(&images), 0);
        assert_eq!(savings_ratio(&images), 0.0);
        assert!(deduplicate(&images).is_empty());
    }
}
