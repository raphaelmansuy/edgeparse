//! JSON output writer.

use crate::models::document::PdfDocument;
use crate::EdgePdfError;

/// Write a PdfDocument as JSON to the given writer.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` if serialization fails.
pub fn write_json<W: std::io::Write>(
    doc: &PdfDocument,
    writer: &mut W,
) -> Result<(), EdgePdfError> {
    serde_json::to_writer_pretty(writer, doc)
        .map_err(|e| EdgePdfError::OutputError(format!("JSON serialization failed: {}", e)))
}

/// Serialize a PdfDocument to a JSON string.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` if serialization fails.
pub fn to_json_string(doc: &PdfDocument) -> Result<String, EdgePdfError> {
    serde_json::to_string_pretty(doc)
        .map_err(|e| EdgePdfError::OutputError(format!("JSON serialization failed: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_output() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let json = to_json_string(&doc).unwrap();
        assert!(json.contains("test.pdf"));
        assert!(json.contains("number_of_pages"));
    }
}
