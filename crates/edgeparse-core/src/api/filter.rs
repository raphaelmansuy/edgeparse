//! Content safety filter configuration.

use serde::{Deserialize, Serialize};

/// Content safety filter configuration.
///
/// Controls which types of potentially hidden or malicious content
/// are filtered from the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Filter hidden text (low contrast ratio)
    pub filter_hidden_text: bool,
    /// Filter off-page content (outside CropBox)
    pub filter_out_of_page: bool,
    /// Filter tiny text (below minimum height)
    pub filter_tiny_text: bool,
    /// Filter hidden OCG layers
    pub filter_hidden_ocg: bool,
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            filter_hidden_text: true,
            filter_out_of_page: true,
            filter_tiny_text: true,
            filter_hidden_ocg: true,
        }
    }
}

impl FilterConfig {
    /// Create a FilterConfig with all filters disabled.
    pub fn all_off() -> Self {
        Self {
            filter_hidden_text: false,
            filter_out_of_page: false,
            filter_tiny_text: false,
            filter_hidden_ocg: false,
        }
    }

    /// Apply content-safety-off flags.
    ///
    /// Accepts a comma-separated string of filter names to disable:
    /// "all", "hidden-text", "off-page", "tiny", "hidden-ocg"
    pub fn apply_safety_off(&mut self, flags: &str) {
        for flag in flags.split(',').map(|s| s.trim()) {
            match flag {
                "all" => {
                    self.filter_hidden_text = false;
                    self.filter_out_of_page = false;
                    self.filter_tiny_text = false;
                    self.filter_hidden_ocg = false;
                }
                "hidden-text" => self.filter_hidden_text = false,
                "off-page" => self.filter_out_of_page = false,
                "tiny" => self.filter_tiny_text = false,
                "hidden-ocg" => self.filter_hidden_ocg = false,
                _ => {} // Ignore unknown flags
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_filter_config() {
        let config = FilterConfig::default();
        assert!(config.filter_hidden_text);
        assert!(config.filter_out_of_page);
        assert!(config.filter_tiny_text);
        assert!(config.filter_hidden_ocg);
    }

    #[test]
    fn test_all_off() {
        let config = FilterConfig::all_off();
        assert!(!config.filter_hidden_text);
        assert!(!config.filter_out_of_page);
        assert!(!config.filter_tiny_text);
        assert!(!config.filter_hidden_ocg);
    }

    #[test]
    fn test_apply_safety_off_all() {
        let mut config = FilterConfig::default();
        config.apply_safety_off("all");
        assert!(!config.filter_hidden_text);
        assert!(!config.filter_out_of_page);
        assert!(!config.filter_tiny_text);
        assert!(!config.filter_hidden_ocg);
    }

    #[test]
    fn test_apply_safety_off_individual() {
        let mut config = FilterConfig::default();
        config.apply_safety_off("hidden-text, tiny");
        assert!(!config.filter_hidden_text);
        assert!(config.filter_out_of_page);
        assert!(!config.filter_tiny_text);
        assert!(config.filter_hidden_ocg);
    }
}
