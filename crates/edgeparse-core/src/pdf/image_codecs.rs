//! Image XObject codec decode for the OCR path.
//!
//! Routes PDF `/Filter` values to pixel buffers. DCT / Flate / raw samples are
//! always available when the `image` feature is on; JBIG2 uses `hayro-jbig2`
//! behind `codecs-jbig2`; JPX uses best-effort fallbacks (and `jpeg2k` when
//! `codecs-jpx` is enabled).

#![cfg(feature = "image")]

use image::GrayImage;

/// Result of decoding an Image XObject stream to grayscale for OCR.
#[derive(Debug, Clone)]
pub struct DecodedGray {
    /// Grayscale raster.
    pub gray: GrayImage,
    /// Filter that produced the pixels (for diagnostics).
    pub filter: String,
}

/// Decode image stream bytes into a grayscale raster suitable for OCR.
pub fn decode_image_to_gray(
    data: &[u8],
    filter: &str,
    width: u32,
    height: u32,
) -> Option<DecodedGray> {
    let primary = primary_filter(filter);
    let gray = match primary {
        "DCTDecode" => decode_dct(data),
        "JPXDecode" => decode_jpx(data, width, height),
        "JBIG2Decode" => decode_jbig2(data, width, height),
        "CCITTFaxDecode" => decode_ccitt_best_effort(data, width, height),
        _ => decode_raw_or_flate(data, width, height).or_else(|| decode_dct(data)),
    }?;
    Some(DecodedGray {
        gray,
        filter: primary.to_string(),
    })
}

fn primary_filter(filter: &str) -> &str {
    filter
        .split([' ', ',', '[', ']'])
        .map(str::trim)
        .rfind(|s| !s.is_empty())
        .unwrap_or(filter)
}

fn decode_dct(data: &[u8]) -> Option<GrayImage> {
    image::load_from_memory(data).ok().map(|img| img.to_luma8())
}

fn decode_raw_or_flate(data: &[u8], width: u32, height: u32) -> Option<GrayImage> {
    if width == 0 || height == 0 {
        return None;
    }
    let n = (width as usize).checked_mul(height as usize)?;
    if data.len() == n {
        return GrayImage::from_raw(width, height, data.to_vec());
    }
    if data.len() == n * 3 {
        let mut luma = Vec::with_capacity(n);
        for px in data.chunks_exact(3) {
            let y = (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
                .round()
                .clamp(0.0, 255.0) as u8;
            luma.push(y);
        }
        return GrayImage::from_raw(width, height, luma);
    }
    if data.len() == n * 4 {
        let mut luma = Vec::with_capacity(n);
        for px in data.chunks_exact(4) {
            let y = (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
                .round()
                .clamp(0.0, 255.0) as u8;
            luma.push(y);
        }
        return GrayImage::from_raw(width, height, luma);
    }
    None
}

fn decode_ccitt_best_effort(data: &[u8], width: u32, height: u32) -> Option<GrayImage> {
    if width == 0 || height == 0 {
        return None;
    }
    let row_bytes = (width as usize).div_ceil(8);
    let expected = row_bytes.checked_mul(height as usize)?;
    if data.len() < expected {
        return None;
    }
    let mut luma = Vec::with_capacity((width * height) as usize);
    for y in 0..height as usize {
        let row = &data[y * row_bytes..(y + 1) * row_bytes];
        for x in 0..width as usize {
            let byte = row[x / 8];
            let bit = 7 - (x % 8);
            let on = ((byte >> bit) & 1) != 0;
            luma.push(if on { 0 } else { 255 });
        }
    }
    GrayImage::from_raw(width, height, luma)
}

#[cfg(feature = "codecs-jpx")]
fn decode_jpx(data: &[u8], width: u32, height: u32) -> Option<GrayImage> {
    // jpeg2k crate: decode via from_bytes → get pixels.
    let img = jpeg2k::Image::from_bytes(data).ok()?;
    let w = img.width().max(1);
    let h = img.height().max(1);
    let _ = (width, height);
    let pixels = img.get_pixels(None).ok()?;
    let mut luma = Vec::with_capacity((w * h) as usize);
    // Assume RGBA or RGB interleaved.
    let bpp = if pixels.len() >= (w * h * 4) as usize {
        4
    } else if pixels.len() >= (w * h * 3) as usize {
        3
    } else if pixels.len() >= (w * h) as usize {
        1
    } else {
        return None;
    };
    if bpp == 1 {
        return GrayImage::from_raw(w, h, pixels[..(w * h) as usize].to_vec());
    }
    for px in pixels.chunks_exact(bpp) {
        let y = (0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]))
            .round()
            .clamp(0.0, 255.0) as u8;
        luma.push(y);
    }
    GrayImage::from_raw(w, h, luma)
}

#[cfg(not(feature = "codecs-jpx"))]
fn decode_jpx(data: &[u8], width: u32, height: u32) -> Option<GrayImage> {
    // Without OpenJPEG: try raw / JPEG-in-JPX fallbacks so hybrid pages still OCR.
    decode_raw_or_flate(data, width, height).or_else(|| decode_dct(data))
}

#[cfg(feature = "codecs-jbig2")]
fn decode_jbig2(data: &[u8], _width: u32, _height: u32) -> Option<GrayImage> {
    use hayro_jbig2::{Decoder, Image as Jbig2Image};

    let image = Jbig2Image::new_embedded(data, None)
        .or_else(|_| Jbig2Image::new(data))
        .ok()?;
    let w = image.width();
    let h = image.height();
    let mut buf = vec![255u8; (w as usize).saturating_mul(h as usize)];

    struct LumaBuf<'a> {
        buf: &'a mut [u8],
        pos: usize,
    }
    impl Decoder for LumaBuf<'_> {
        fn push_pixel(&mut self, black: bool) {
            if self.pos < self.buf.len() {
                self.buf[self.pos] = if black { 0 } else { 255 };
                self.pos += 1;
            }
        }
        fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
            let luma = if black { 0 } else { 255 };
            let count = (chunk_count as usize).saturating_mul(8);
            let end = (self.pos + count).min(self.buf.len());
            self.buf[self.pos..end].fill(luma);
            self.pos = end;
        }
        fn next_line(&mut self) {}
    }

    let mut decoder = LumaBuf {
        buf: &mut buf,
        pos: 0,
    };
    image.decode(&mut decoder).ok()?;
    GrayImage::from_raw(w, h, buf)
}

#[cfg(not(feature = "codecs-jbig2"))]
fn decode_jbig2(data: &[u8], width: u32, height: u32) -> Option<GrayImage> {
    decode_ccitt_best_effort(data, width, height)
        .or_else(|| decode_raw_or_flate(data, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_gray_roundtrip() {
        let data = vec![0u8, 128, 255, 64];
        let g = decode_image_to_gray(&data, "FlateDecode", 2, 2).unwrap();
        assert_eq!(g.gray.width(), 2);
        assert_eq!(g.gray.height(), 2);
    }

    #[test]
    fn primary_filter_from_array_like() {
        assert_eq!(primary_filter("FlateDecode"), "FlateDecode");
        assert_eq!(primary_filter("[ FlateDecode ]"), "FlateDecode");
    }

    #[test]
    fn jbig2_filter_does_not_panic_without_feature() {
        let _ = decode_image_to_gray(&[0u8; 16], "JBIG2Decode", 4, 4);
    }

    #[test]
    fn jpx_filter_does_not_panic_without_feature() {
        let _ = decode_image_to_gray(&[0u8; 16], "JPXDecode", 4, 4);
    }
}
