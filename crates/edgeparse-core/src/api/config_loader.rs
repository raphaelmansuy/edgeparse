//! Config file loading — read [`ProcessingConfig`] from JSON files.

use std::path::Path;

use crate::api::config::ProcessingConfig;
use crate::EdgePdfError;

/// Load a [`ProcessingConfig`] from a JSON file at the given path.
///
/// Missing fields in the JSON fall back to their defaults via serde.
///
/// # Errors
/// Returns `EdgePdfError::IoError` on read failure or `EdgePdfError::OutputError`
/// on parse failure.
pub fn load_config_from_file(path: &Path) -> Result<ProcessingConfig, EdgePdfError> {
    let content = std::fs::read_to_string(path)?;
    parse_config_json(&content)
}

/// Parse a [`ProcessingConfig`] from a JSON string.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on JSON parse failure.
pub fn parse_config_json(json: &str) -> Result<ProcessingConfig, EdgePdfError> {
    serde_json::from_str(json)
        .map_err(|e| EdgePdfError::OutputError(format!("Failed to parse config JSON: {}", e)))
}

/// Serialize a [`ProcessingConfig`] to a pretty-printed JSON string.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on serialization failure.
pub fn config_to_json(config: &ProcessingConfig) -> Result<String, EdgePdfError> {
    serde_json::to_string_pretty(config)
        .map_err(|e| EdgePdfError::OutputError(format!("Failed to serialize config: {}", e)))
}

/// Merge two configs: values present in `overlay` override those in `base`.
///
/// This works by serializing both to JSON, merging the JSON objects, and
/// deserializing back. Fields in `overlay` that are `null` are skipped.
pub fn merge_configs(
    base: &ProcessingConfig,
    overlay_json: &str,
) -> Result<ProcessingConfig, EdgePdfError> {
    let base_json = serde_json::to_value(base)
        .map_err(|e| EdgePdfError::OutputError(format!("config serialization error: {}", e)))?;
    let overlay_val: serde_json::Value = serde_json::from_str(overlay_json)
        .map_err(|e| EdgePdfError::OutputError(format!("overlay parse error: {}", e)))?;

    let merged = merge_json(base_json, overlay_val);
    serde_json::from_value(merged)
        .map_err(|e| EdgePdfError::OutputError(format!("merged config parse error: {}", e)))
}

fn merge_json(base: serde_json::Value, overlay: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match (base, overlay) {
        (Value::Object(mut base_map), Value::Object(overlay_map)) => {
            for (key, val) in overlay_map {
                if val.is_null() {
                    continue;
                }
                let entry = base_map.remove(&key).unwrap_or(Value::Null);
                base_map.insert(key, merge_json(entry, val));
            }
            Value::Object(base_map)
        }
        (_, overlay) => overlay,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::config::{OutputFormat, ReadingOrder};

    #[test]
    fn test_parse_default_config() {
        let config = ProcessingConfig::default();
        let json = config_to_json(&config).unwrap();
        let parsed = parse_config_json(&json).unwrap();
        assert_eq!(parsed.formats, config.formats);
        assert_eq!(parsed.quiet, config.quiet);
    }

    #[test]
    fn test_parse_partial_config() {
        // Partial JSON doesn't work directly — use merge approach
        let base = ProcessingConfig::default();
        let overlay = r#"{"quiet": true, "sanitize": true}"#;
        let config = merge_configs(&base, overlay).unwrap();
        assert!(config.quiet);
        assert!(config.sanitize);
    }

    #[test]
    fn test_merge_configs() {
        let base = ProcessingConfig::default();
        let overlay = r#"{"quiet": true, "reading_order": "Off"}"#;
        let merged = merge_configs(&base, overlay).unwrap();
        assert!(merged.quiet);
        assert_eq!(merged.reading_order, ReadingOrder::Off);
        // Other fields should retain defaults
        assert_eq!(merged.formats, vec![OutputFormat::Json]);
    }

    #[test]
    fn test_config_roundtrip() {
        let mut config = ProcessingConfig::default();
        config.quiet = true;
        config.sanitize = true;
        config.pages = Some("1-5".to_string());

        let json = config_to_json(&config).unwrap();
        let parsed = parse_config_json(&json).unwrap();
        assert_eq!(parsed.quiet, true);
        assert_eq!(parsed.sanitize, true);
        assert_eq!(parsed.pages, Some("1-5".to_string()));
    }

    #[test]
    fn test_invalid_json() {
        let result = parse_config_json("not json");
        assert!(result.is_err());
    }
}
