//! Content sanitization — PII masking, Unicode normalization.

use regex::Regex;
use std::sync::LazyLock;

/// Sanitization rule: a regex pattern and its replacement.
#[derive(Debug, Clone)]
pub struct SanitizationRule {
    /// Human-readable name of the rule
    pub name: &'static str,
    /// Compiled regex pattern
    pattern: &'static LazyLock<Regex>,
    /// Replacement placeholder
    pub replacement: &'static str,
}

static EMAIL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap());

static PHONE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\+?\d{1,3}[-.\s]?)?\(?\d{2,4}\)?[-.\s]?\d{3,4}[-.\s]?\d{3,4}").unwrap()
});

static IP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap());

static CREDIT_CARD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b").unwrap());

static URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?://[^\s<>"']+"#).unwrap());

/// Default sanitization rules for PII masking.
pub fn default_rules() -> Vec<SanitizationRule> {
    vec![
        SanitizationRule {
            name: "email",
            pattern: &EMAIL_RE,
            replacement: "[EMAIL]",
        },
        SanitizationRule {
            name: "credit_card",
            pattern: &CREDIT_CARD_RE,
            replacement: "[CREDIT_CARD]",
        },
        SanitizationRule {
            name: "ip_address",
            pattern: &IP_RE,
            replacement: "[IP]",
        },
        SanitizationRule {
            name: "url",
            pattern: &URL_RE,
            replacement: "[URL]",
        },
        SanitizationRule {
            name: "phone",
            pattern: &PHONE_RE,
            replacement: "[PHONE]",
        },
    ]
}

/// Apply all sanitization rules to a text string.
pub fn sanitize_text(text: &str, rules: &[SanitizationRule]) -> String {
    let mut result = text.to_string();
    for rule in rules {
        result = rule
            .pattern
            .replace_all(&result, rule.replacement)
            .to_string();
    }
    result
}

/// Normalize Unicode text (NFC normalization).
pub fn normalize_unicode(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    text.nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_email() {
        let rules = default_rules();
        let result = sanitize_text("Contact john@example.com for info", &rules);
        assert!(result.contains("[EMAIL]"));
        assert!(!result.contains("john@example.com"));
    }

    #[test]
    fn test_sanitize_url() {
        let rules = default_rules();
        let result = sanitize_text("Visit https://example.com/page for details", &rules);
        assert!(result.contains("[URL]"));
        assert!(!result.contains("https://example.com"));
    }

    #[test]
    fn test_sanitize_credit_card() {
        let rules = default_rules();
        let result = sanitize_text("Card: 4111-1111-1111-1111", &rules);
        assert!(result.contains("[CREDIT_CARD]"));
    }

    #[test]
    fn test_normalize_unicode() {
        // Combining accent vs precomposed
        let decomposed = "e\u{0301}"; // e + combining acute
        let normalized = normalize_unicode(decomposed);
        assert_eq!(normalized, "é");
    }
}
