//! PDF document loading via lopdf.

use std::path::Path;

use lopdf::Document;

use crate::EdgePdfError;

/// Raw loaded PDF document with page data.
pub struct RawPdfDocument {
    /// The lopdf Document handle
    pub document: Document,
    /// Number of pages
    pub num_pages: u32,
    /// Document metadata
    pub metadata: PdfMetadata,
}

impl std::fmt::Debug for RawPdfDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawPdfDocument")
            .field("num_pages", &self.num_pages)
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

/// Extracted PDF metadata.
#[derive(Debug, Clone, Default)]
pub struct PdfMetadata {
    /// Author
    pub author: Option<String>,
    /// Title
    pub title: Option<String>,
    /// Creation date
    pub creation_date: Option<String>,
    /// Modification date
    pub modification_date: Option<String>,
}

/// Load a PDF file and extract basic structure.
///
/// # Arguments
/// * `path` - Path to the PDF file
/// * `password` - Optional decryption password
///
/// # Errors
/// Returns `EdgePdfError::LoadError` if the file cannot be read or parsed.
pub fn load_pdf(path: &Path, _password: Option<&str>) -> Result<RawPdfDocument, EdgePdfError> {
    if !path.exists() {
        return Err(EdgePdfError::LoadError(format!(
            "File not found: {}",
            path.display()
        )));
    }

    let document = Document::load(path).map_err(|e| {
        EdgePdfError::LoadError(format!("Failed to load PDF {}: {}", path.display(), e))
    })?;

    let pages = document.get_pages();
    let num_pages = pages.len() as u32;

    let metadata = extract_metadata(&document);

    Ok(RawPdfDocument {
        document,
        num_pages,
        metadata,
    })
}

/// Extract metadata from the PDF document info dictionary.
fn extract_metadata(doc: &Document) -> PdfMetadata {
    let mut metadata = PdfMetadata::default();

    if let Ok(info_ref) = doc.trailer.get(b"Info") {
        if let Ok(info_ref) = info_ref.as_reference() {
            if let Ok(info) = doc.get_object(info_ref) {
                if let Ok(dict) = info.as_dict() {
                    metadata.author = extract_string_field(dict, b"Author");
                    metadata.title = extract_string_field(dict, b"Title");
                    metadata.creation_date = extract_string_field(dict, b"CreationDate");
                    metadata.modification_date = extract_string_field(dict, b"ModDate");
                }
            }
        }
    }

    metadata
}

/// Load a PDF from an in-memory byte slice.
///
/// Uses lopdf's `Document::load_mem()` which parses PDF from `&[u8]`.
/// This is the WASM-compatible loader — no filesystem access required.
///
/// # Arguments
/// * `data` — raw PDF bytes
/// * `_password` — optional decryption password (not yet implemented)
///
/// # Errors
/// Returns `EdgePdfError::LoadError` if the bytes cannot be parsed as PDF.
pub fn load_pdf_from_bytes(
    data: &[u8],
    _password: Option<&str>,
) -> Result<RawPdfDocument, EdgePdfError> {
    if data.is_empty() {
        return Err(EdgePdfError::LoadError("Empty PDF data".to_string()));
    }

    let document = Document::load_mem(data)
        .map_err(|e| EdgePdfError::LoadError(format!("Failed to parse PDF from bytes: {e}")))?;

    let pages = document.get_pages();
    let num_pages = pages.len() as u32;
    let metadata = extract_metadata(&document);

    Ok(RawPdfDocument {
        document,
        num_pages,
        metadata,
    })
}

/// Extract a string field from a PDF dictionary.
fn extract_string_field(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key).ok().and_then(|obj| match obj {
        lopdf::Object::String(bytes, _) => String::from_utf8(bytes.clone()).ok(),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_pdf(Path::new("/nonexistent/file.pdf"), None);
        assert!(result.is_err());
        match result.unwrap_err() {
            EdgePdfError::LoadError(msg) => assert!(msg.contains("File not found")),
            other => panic!("Unexpected error: {:?}", other),
        }
    }
}
