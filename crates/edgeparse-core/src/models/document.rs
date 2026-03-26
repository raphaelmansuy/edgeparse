//! PdfDocument — top-level extracted document.

use serde::{Deserialize, Serialize};

use super::content::ContentElement;

/// The top-level extracted PDF document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfDocument {
    /// Original file name
    pub file_name: String,
    /// Original source path when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// Number of pages
    pub number_of_pages: u32,
    /// Document author
    pub author: Option<String>,
    /// Document title
    pub title: Option<String>,
    /// Creation date
    pub creation_date: Option<String>,
    /// Modification date
    pub modification_date: Option<String>,
    /// PDF producer application
    pub producer: Option<String>,
    /// Creator application
    pub creator: Option<String>,
    /// Document subject
    pub subject: Option<String>,
    /// Comma-separated keywords
    pub keywords: Option<String>,
    /// Top-level content elements (reading order)
    pub kids: Vec<ContentElement>,
}

impl PdfDocument {
    /// Create a new empty PdfDocument.
    pub fn new(file_name: String) -> Self {
        Self {
            file_name,
            source_path: None,
            number_of_pages: 0,
            author: None,
            title: None,
            creation_date: None,
            modification_date: None,
            producer: None,
            creator: None,
            subject: None,
            keywords: None,
            kids: Vec::new(),
        }
    }

    /// Return a list of (key, value) pairs for non-empty metadata fields.
    pub fn metadata_pairs(&self) -> Vec<(&str, &str)> {
        let mut pairs = Vec::new();
        pairs.push(("File", self.file_name.as_str()));
        if let Some(ref v) = self.title {
            pairs.push(("Title", v.as_str()));
        }
        if let Some(ref v) = self.author {
            pairs.push(("Author", v.as_str()));
        }
        if let Some(ref v) = self.subject {
            pairs.push(("Subject", v.as_str()));
        }
        if let Some(ref v) = self.keywords {
            pairs.push(("Keywords", v.as_str()));
        }
        if let Some(ref v) = self.creator {
            pairs.push(("Creator", v.as_str()));
        }
        if let Some(ref v) = self.producer {
            pairs.push(("Producer", v.as_str()));
        }
        if let Some(ref v) = self.creation_date {
            pairs.push(("Created", v.as_str()));
        }
        if let Some(ref v) = self.modification_date {
            pairs.push(("Modified", v.as_str()));
        }
        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_document() {
        let doc = PdfDocument::new("test.pdf".to_string());
        assert_eq!(doc.file_name, "test.pdf");
        assert_eq!(doc.source_path, None);
        assert_eq!(doc.number_of_pages, 0);
        assert!(doc.kids.is_empty());
    }

    #[test]
    fn test_metadata_pairs() {
        let mut doc = PdfDocument::new("report.pdf".to_string());
        doc.title = Some("Annual Report".to_string());
        doc.author = Some("Alice".to_string());
        doc.keywords = Some("finance, report".to_string());

        let pairs = doc.metadata_pairs();
        assert_eq!(pairs[0], ("File", "report.pdf"));
        assert_eq!(pairs[1], ("Title", "Annual Report"));
        assert_eq!(pairs[2], ("Author", "Alice"));
        assert_eq!(pairs[3], ("Keywords", "finance, report"));
        assert_eq!(pairs.len(), 4);
    }

    #[test]
    fn test_metadata_pairs_empty() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let pairs = doc.metadata_pairs();
        // Only "File" present
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "File");
    }
}
