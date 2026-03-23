//! Simple trigram-based language detection.
//!
//! Uses character trigram frequency profiles for the most common languages.
//! No external dependencies — profiles are embedded as static data.

use std::collections::HashMap;

/// Detected language result.
#[derive(Debug, Clone, PartialEq)]
pub struct LangDetection {
    /// ISO 639-1 code (e.g., "en", "fr", "de").
    pub code: String,
    /// Human-readable language name.
    pub name: String,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
}

/// Detect the most likely language of the given text.
///
/// Returns `None` if the text is too short (< 20 characters after cleanup).
pub fn detect_language(text: &str) -> Option<LangDetection> {
    let clean: String = text
        .chars()
        .filter(|c| c.is_alphabetic() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase();

    if clean.len() < 20 {
        return None;
    }

    let trigrams = extract_trigrams(&clean);
    if trigrams.is_empty() {
        return None;
    }

    let mut best = ("en", "English", 0.0_f64);

    for &(code, name, profile) in PROFILES {
        let score = cosine_similarity(&trigrams, profile);
        if score > best.2 {
            best = (code, name, score);
        }
    }

    Some(LangDetection {
        code: best.0.to_string(),
        name: best.1.to_string(),
        confidence: best.2,
    })
}

/// Extract top trigram frequencies from text.
fn extract_trigrams(text: &str) -> HashMap<&str, f64> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let bytes = text.as_bytes();
    if bytes.len() < 3 {
        return HashMap::new();
    }
    // Only works for ASCII-range text; for non-ASCII we fall back to byte slicing
    let len = text.len();
    for i in 0..len.saturating_sub(2) {
        if text.is_char_boundary(i) && text.is_char_boundary(i + 3) {
            let tri = &text[i..i + 3];
            *counts.entry(tri).or_insert(0) += 1;
        }
    }
    let total: f64 = counts.values().sum::<usize>() as f64;
    if total == 0.0 {
        return HashMap::new();
    }
    counts
        .into_iter()
        .map(|(k, v)| (k, v as f64 / total))
        .collect()
}

/// Cosine similarity between extracted trigrams and a profile.
fn cosine_similarity(trigrams: &HashMap<&str, f64>, profile: &[(&str, f64)]) -> f64 {
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;

    let profile_map: HashMap<&str, f64> = profile.iter().copied().collect();

    for (&tri, &freq) in trigrams {
        norm_a += freq * freq;
        if let Some(&pf) = profile_map.get(tri) {
            dot += freq * pf;
        }
    }
    for &(_, pf) in profile {
        norm_b += pf * pf;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-10 {
        0.0
    } else {
        dot / denom
    }
}

/// Language profiles — top trigrams with approximate relative frequencies.
/// These are compressed profiles with the most distinctive trigrams per language.
type LangProfile = (&'static str, &'static str, &'static [(&'static str, f64)]);
static PROFILES: &[LangProfile] = &[
    (
        "en",
        "English",
        &[
            ("the", 0.035),
            ("he ", 0.025),
            ("and", 0.020),
            ("ing", 0.018),
            ("tion", 0.015),
            ("er ", 0.014),
            ("ion", 0.013),
            (" th", 0.025),
            ("ed ", 0.012),
            ("in ", 0.012),
            ("to ", 0.011),
            (" to", 0.011),
            ("of ", 0.020),
            (" of", 0.018),
            ("ent", 0.010),
            ("is ", 0.010),
            (" is", 0.009),
            ("hat", 0.009),
            (" an", 0.012),
            ("nd ", 0.010),
        ],
    ),
    (
        "fr",
        "French",
        &[
            ("es ", 0.025),
            ("de ", 0.022),
            (" de", 0.022),
            ("le ", 0.018),
            ("ent", 0.017),
            (" le", 0.016),
            ("ion", 0.015),
            ("les", 0.014),
            ("la ", 0.013),
            (" la", 0.013),
            ("re ", 0.012),
            ("tion", 0.011),
            ("que", 0.013),
            (" qu", 0.011),
            ("ue ", 0.010),
            ("et ", 0.010),
            (" et", 0.009),
            ("des", 0.012),
            (" de", 0.022),
            ("ont", 0.009),
        ],
    ),
    (
        "de",
        "German",
        &[
            ("en ", 0.030),
            ("er ", 0.025),
            ("der", 0.018),
            ("die", 0.017),
            ("ein", 0.015),
            ("sch", 0.014),
            (" de", 0.016),
            ("ich", 0.014),
            ("und", 0.013),
            (" un", 0.012),
            ("nd ", 0.011),
            ("den", 0.010),
            ("che", 0.012),
            (" di", 0.013),
            ("ie ", 0.012),
            ("ung", 0.010),
            ("gen", 0.009),
            ("ine", 0.009),
            (" ei", 0.010),
            ("das", 0.008),
        ],
    ),
    (
        "es",
        "Spanish",
        &[
            ("de ", 0.025),
            (" de", 0.023),
            ("os ", 0.018),
            ("la ", 0.016),
            (" la", 0.015),
            ("en ", 0.015),
            ("el ", 0.014),
            (" el", 0.013),
            ("ión", 0.012),
            ("es ", 0.020),
            (" en", 0.012),
            ("ent", 0.010),
            ("que", 0.012),
            (" qu", 0.010),
            ("ue ", 0.009),
            ("aci", 0.008),
            ("ado", 0.008),
            ("las", 0.010),
            (" lo", 0.009),
            ("los", 0.010),
        ],
    ),
    (
        "it",
        "Italian",
        &[
            ("la ", 0.020),
            (" la", 0.018),
            (" di", 0.017),
            ("di ", 0.016),
            ("che", 0.015),
            ("re ", 0.014),
            ("ell", 0.013),
            ("lla", 0.012),
            ("to ", 0.011),
            ("ne ", 0.011),
            (" de", 0.012),
            ("del", 0.011),
            ("ent", 0.010),
            ("ion", 0.010),
            ("con", 0.009),
            (" co", 0.009),
            ("per", 0.009),
            (" pe", 0.008),
            ("ato", 0.008),
            ("ment", 0.007),
        ],
    ),
    (
        "pt",
        "Portuguese",
        &[
            ("de ", 0.025),
            (" de", 0.023),
            ("os ", 0.016),
            (" qu", 0.012),
            ("que", 0.012),
            ("ão ", 0.014),
            ("ção", 0.012),
            (" do", 0.010),
            ("do ", 0.010),
            ("da ", 0.011),
            (" da", 0.011),
            ("ent", 0.010),
            ("es ", 0.015),
            (" co", 0.009),
            ("com", 0.009),
            ("nte", 0.008),
            ("ment", 0.007),
            ("para", 0.007),
            (" pa", 0.007),
            (" no", 0.008),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_english() {
        let text = "The quick brown fox jumps over the lazy dog and then runs away into the forest";
        let result = detect_language(text).unwrap();
        assert_eq!(result.code, "en");
        assert!(result.confidence > 0.0);
    }

    #[test]
    fn test_detect_french() {
        let text = "Le petit prince est un livre que tout le monde devrait lire au moins une fois dans sa vie";
        let result = detect_language(text).unwrap();
        assert_eq!(result.code, "fr");
    }

    #[test]
    fn test_detect_german() {
        let text = "Die Bundesrepublik Deutschland ist ein demokratischer und sozialer Bundesstaat";
        let result = detect_language(text).unwrap();
        assert_eq!(result.code, "de");
    }

    #[test]
    fn test_too_short() {
        let result = detect_language("hi");
        assert!(result.is_none());
    }

    #[test]
    fn test_empty() {
        let result = detect_language("");
        assert!(result.is_none());
    }
}
