//! Shared font name / weight helpers (DRY across Type1 / Type3 / TrueType).

/// Prefer `/BaseFont`, then `FontDescriptor /FontName`, else `"Unknown"`.
pub fn resolve_base_font_name(
    base_font: Option<&str>,
    descriptor_font_name: Option<&str>,
) -> String {
    if let Some(name) = base_font {
        let trimmed = name.trim();
        if !trimmed.is_empty() && trimmed != "Unknown" {
            return trimmed.to_string();
        }
    }
    if let Some(name) = descriptor_font_name {
        let trimmed = name.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    "Unknown".to_string()
}

/// Parse a CSS / OpenType weight from a PostScript or variable-font name.
///
/// Handles:
/// - Classic names: `Bold`, `Black`, `Heavy`, `Light`, `Thin`, `Regular`
/// - Variable-font tokens: `wght600` (CSS) or `wght45875200` (16.16 fixed-point)
///
/// Returns `None` when the name carries no weight signal. Ambiguous Skia
/// packing such as `wght2580000` (not CSS, not 16.16) is ignored — callers
/// should prefer `/FontWeight` from the font descriptor.
pub fn weight_from_font_name(name: &str) -> Option<f64> {
    let lower = name.to_lowercase();

    if let Some(idx) = lower.find("wght") {
        let rest = &lower[idx + 4..];
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(raw) = digits.parse::<u64>() {
            if (1..=1000).contains(&raw) {
                return Some(raw as f64);
            }
            // 16.16 fixed-point (CSS_weight << 16)
            if raw > 65_536 {
                let css = (raw as f64) / 65536.0;
                if (1.0..=1000.0).contains(&css) {
                    return Some(css.round());
                }
            }
        }
    }

    if lower.contains("black") || lower.contains("heavy") {
        return Some(900.0);
    }
    if lower.contains("bold") || lower.contains("semibold") || lower.contains("demibold") {
        return Some(700.0);
    }
    if lower.contains("medium") {
        return Some(500.0);
    }
    if lower.contains("light") {
        return Some(300.0);
    }
    if lower.contains("thin") || lower.contains("hairline") {
        return Some(100.0);
    }
    if lower.contains("regular") || lower.contains("book") || lower.contains("normal") {
        return Some(400.0);
    }
    None
}

/// Resolve final numeric weight: `/FontWeight` → name token → StemV.
pub fn resolve_font_weight(
    font_name: &str,
    descriptor_font_weight: Option<f64>,
    stem_v: f64,
) -> f64 {
    // Explicit descriptor weight is authoritative (Skia publishes FontWeight=600
    // while StemV=262 would otherwise look "bold").
    if let Some(w) = descriptor_font_weight {
        if (1.0..=1000.0).contains(&w) {
            return w;
        }
    }
    if let Some(w) = weight_from_font_name(font_name) {
        return w;
    }
    // StemV heuristic (legacy)
    if stem_v >= 140.0 {
        700.0
    } else if stem_v >= 100.0 {
        500.0
    } else {
        400.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_font_preferred() {
        assert_eq!(
            resolve_base_font_name(Some("Helvetica-Bold"), Some("Other")),
            "Helvetica-Bold"
        );
    }

    #[test]
    fn falls_back_to_descriptor_name() {
        assert_eq!(
            resolve_base_font_name(None, Some("AAAAAA+.SFNS-Regular")),
            "AAAAAA+.SFNS-Regular"
        );
        assert_eq!(
            resolve_base_font_name(Some("Unknown"), Some("DescName")),
            "DescName"
        );
    }

    #[test]
    fn weight_from_classic_names() {
        assert_eq!(weight_from_font_name("Helvetica-Bold"), Some(700.0));
        assert_eq!(weight_from_font_name("Arial-Black"), Some(900.0));
        assert_eq!(weight_from_font_name("Courier"), None);
    }

    #[test]
    fn weight_from_wght_token_css() {
        assert_eq!(weight_from_font_name("Family_wght600_opsz"), Some(600.0));
    }

    #[test]
    fn weight_from_wght_16_16() {
        // 700 << 16 = 45_875_200
        assert_eq!(weight_from_font_name("Font_wght45875200"), Some(700.0));
    }

    #[test]
    fn resolve_prefers_fontweight_over_stemv() {
        // Skia: FontWeight=600, StemV=262 would otherwise look bold
        let w = resolve_font_weight("AAAA+.SFNS-Regular_wght2580000", Some(600.0), 262.0);
        assert!((w - 600.0).abs() < 1e-6, "w={w}");
    }

    #[test]
    fn resolve_uses_name_when_no_fontweight() {
        let w = resolve_font_weight("Helvetica-Bold", None, 80.0);
        assert!((w - 700.0).abs() < 1e-6);
    }

    #[test]
    fn resolve_uses_fontweight_when_no_name_signal() {
        let w = resolve_font_weight("CustomEmbed", Some(600.0), 262.0);
        assert!((w - 600.0).abs() < 1e-6);
    }
}
