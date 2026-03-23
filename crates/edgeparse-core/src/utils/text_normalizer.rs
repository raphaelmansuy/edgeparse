//! Text normalization for PDF-extracted text.
//!
//! Handles ligature decomposition, soft-hyphen removal, zero-width
//! character stripping, and whitespace normalization.

use unicode_normalization::UnicodeNormalization;

/// Normalize extracted PDF text: decompose ligatures, strip zero-width
/// characters, remove soft hyphens, collapse whitespace, and apply NFC.
pub fn normalize_pdf_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        if let Some(replacement) = decompose_ligature(ch) {
            result.push_str(replacement);
        } else if is_ignorable(ch) {
            // Skip zero-width and soft-hyphen characters
        } else {
            result.push(ch);
        }
    }
    // NFC normalize after ligature decomposition
    result.nfc().collect()
}

/// Decompose common typographic ligatures to their component letters.
fn decompose_ligature(ch: char) -> Option<&'static str> {
    match ch {
        '\u{FB00}' => Some("ff"),
        '\u{FB01}' => Some("fi"),
        '\u{FB02}' => Some("fl"),
        '\u{FB03}' => Some("ffi"),
        '\u{FB04}' => Some("ffl"),
        '\u{FB05}' => Some("st"), // long s + t
        '\u{FB06}' => Some("st"), // st ligature
        '\u{0132}' => Some("IJ"), // Dutch IJ
        '\u{0133}' => Some("ij"), // Dutch ij
        '\u{0152}' => Some("OE"), // OE ligature
        '\u{0153}' => Some("oe"), // oe ligature
        '\u{00C6}' => Some("AE"), // Æ
        '\u{00E6}' => Some("ae"), // æ
        _ => None,
    }
}

/// Characters that should be stripped from extracted text.
fn is_ignorable(ch: char) -> bool {
    matches!(
        ch,
        '\u{00AD}'   // soft hyphen
        | '\u{200B}' // zero-width space
        | '\u{200C}' // zero-width non-joiner
        | '\u{200D}' // zero-width joiner
        | '\u{FEFF}' // byte-order mark / zero-width no-break space
        | '\u{2060}' // word joiner
        | '\u{FFFC}' // object replacement character
    )
}

/// Collapse runs of whitespace into single spaces and trim.
pub fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_space = true; // true to trim leading
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    // Trim trailing space
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

/// Remove all diacritical marks from text (NFC → NFD → strip combining marks → NFC).
/// Useful for accent-insensitive searching.
pub fn strip_diacritics(text: &str) -> String {
    text.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .nfc()
        .collect()
}

/// Normalize smart/curly quotes and dashes to ASCII equivalents.
pub fn normalize_typography(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => result.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => result.push('"'),
            '\u{2013}' => result.push('-'),       // en dash
            '\u{2014}' => result.push_str("--"),  // em dash
            '\u{2026}' => result.push_str("..."), // ellipsis
            '\u{00A0}' => result.push(' '),       // non-breaking space
            '\u{2002}' | '\u{2003}' | '\u{2009}' => result.push(' '), // en/em/thin space
            _ => result.push(ch),
        }
    }
    result
}

/// Full normalization pipeline: ligatures + ignorables + typography + whitespace + NFC.
pub fn full_normalize(text: &str) -> String {
    let step1 = normalize_pdf_text(text);
    let step2 = normalize_typography(&step1);
    collapse_whitespace(&step2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ligature_decomposition() {
        assert_eq!(normalize_pdf_text("e\u{FB03}cient"), "efficient");
        assert_eq!(normalize_pdf_text("\u{FB01}le"), "file");
        assert_eq!(normalize_pdf_text("\u{FB02}oor"), "floor");
        assert_eq!(normalize_pdf_text("\u{FB00}ect"), "ffect");
        assert_eq!(normalize_pdf_text("\u{FB04}e"), "ffle");
    }

    #[test]
    fn test_soft_hyphen_removal() {
        assert_eq!(normalize_pdf_text("con\u{00AD}tin\u{00AD}ue"), "continue");
    }

    #[test]
    fn test_zero_width_removal() {
        assert_eq!(
            normalize_pdf_text("he\u{200B}llo\u{FEFF} world"),
            "hello world"
        );
    }

    #[test]
    fn test_nfc_after_decomposition() {
        // Combining accent should be composed after normalization
        assert_eq!(normalize_pdf_text("e\u{0301}"), "é");
    }

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(collapse_whitespace("  hello   world  "), "hello world");
        assert_eq!(collapse_whitespace("a\n\n  b\tc"), "a b c");
        assert_eq!(collapse_whitespace(""), "");
        assert_eq!(collapse_whitespace("   "), "");
    }

    #[test]
    fn test_plain_text_unchanged() {
        assert_eq!(normalize_pdf_text("Hello World"), "Hello World");
    }

    #[test]
    fn test_strip_diacritics() {
        assert_eq!(strip_diacritics("café"), "cafe");
        assert_eq!(strip_diacritics("über"), "uber");
        assert_eq!(strip_diacritics("naïve"), "naive");
        assert_eq!(strip_diacritics("résumé"), "resume");
    }

    #[test]
    fn test_normalize_typography() {
        assert_eq!(normalize_typography("\u{201C}hello\u{201D}"), "\"hello\"");
        assert_eq!(normalize_typography("it\u{2019}s"), "it's");
        assert_eq!(normalize_typography("a\u{2013}b"), "a-b");
        assert_eq!(normalize_typography("wait\u{2026}"), "wait...");
    }

    #[test]
    fn test_full_normalize() {
        let input = "\u{FB01}le\u{00AD}name\u{200B} \u{201C}test\u{201D}  \u{2026}";
        let result = full_normalize(input);
        assert_eq!(result, "filename \"test\" ...");
    }
}
