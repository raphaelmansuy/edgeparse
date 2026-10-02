//! Decode PDF string objects (ISO 32000-1 §7.9.2).
//!
//! Handles:
//! - UTF-16BE with BOM (`FE FF`) — common for `/ActualText` / `/Alt`
//! - UTF-8 when valid
//! - PDFDocEncoding fallback for Latin text

/// Decode a PDF literal/hex string into Unicode.
pub fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16be(&bytes[2..]);
    }
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        // Prefer UTF-8 when the bytes are valid and don't look like UTF-16BE
        // without BOM (rare for ActualText).
        if !s.chars().any(|c| c == '\u{0}') {
            return s.to_string();
        }
    }
    decode_pdfdoc_encoding(bytes)
}

fn decode_utf16be(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let code = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
        i += 2;
        if let Some(c) = char::from_u32(u32::from(code)) {
            // Skip lone surrogates; handle pairs
            if (0xD800..=0xDBFF).contains(&code) {
                if i + 1 < bytes.len() {
                    let low = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
                    i += 2;
                    if (0xDC00..=0xDFFF).contains(&low) {
                        let cp = 0x10000
                            + (((code as u32) - 0xD800) << 10)
                            + ((low as u32) - 0xDC00);
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        }
                    }
                }
            } else if !(0xDC00..=0xDFFF).contains(&code) {
                out.push(c);
            }
        }
    }
    out
}

/// Minimal PDFDocEncoding (ISO 32000-1 Annex D) — identity for 0x20–0x7E,
/// plus common Latin-1 / Windows extensions used in ActualText.
fn decode_pdfdoc_encoding(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        let c = match b {
            0x20..=0x7E => b as char,
            // Common PDFDocEncoding extras (subset)
            0x80 => '\u{2022}', // bullet
            0x8A => '\u{0160}',
            0x8C => '\u{0152}',
            0x8E => '\u{017D}',
            0x91 => '\u{2018}',
            0x92 => '\u{2019}',
            0x93 => '\u{201C}',
            0x94 => '\u{201D}',
            0x95 => '\u{2013}', // en-dash (also bullet in WinAnsi — ActualText usually ASCII)
            0x96 => '\u{2014}',
            0x97 => '\u{2022}',
            0xA0 => '\u{00A0}',
            0xA1..=0xFF => char::from_u32(u32::from(b)).unwrap_or('\u{FFFD}'),
            _ => '\u{FFFD}',
        };
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16be_with_bom() {
        // BOM + "2"
        let bytes = [0xFE, 0xFF, 0x00, b'2'];
        assert_eq!(decode_pdf_string(&bytes), "2");
    }

    #[test]
    fn ascii_literal() {
        assert_eq!(decode_pdf_string(b"Hello"), "Hello");
    }

    #[test]
    fn utf16be_word() {
        let mut bytes = vec![0xFE, 0xFF];
        for c in "Hi".encode_utf16() {
            bytes.extend_from_slice(&c.to_be_bytes());
        }
        assert_eq!(decode_pdf_string(&bytes), "Hi");
    }
}
