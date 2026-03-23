//! PDF metadata writer — update document metadata (title, author, etc.)
//! in a lopdf Document before saving.

use lopdf::{Document, Object};

/// Metadata fields that can be written to a PDF.
#[derive(Debug, Clone, Default)]
pub struct PdfMetadata {
    /// Document title.
    pub title: Option<String>,
    /// Document author.
    pub author: Option<String>,
    /// Document subject.
    pub subject: Option<String>,
    /// Keywords associated with the document.
    pub keywords: Option<String>,
    /// Application that created the document.
    pub creator: Option<String>,
    /// Application that produced the PDF.
    pub producer: Option<String>,
}

impl PdfMetadata {
    /// Create metadata with just a title.
    pub fn with_title(title: &str) -> Self {
        Self {
            title: Some(title.to_string()),
            ..Default::default()
        }
    }

    /// Whether any field is set.
    pub fn has_any(&self) -> bool {
        self.title.is_some()
            || self.author.is_some()
            || self.subject.is_some()
            || self.keywords.is_some()
            || self.creator.is_some()
            || self.producer.is_some()
    }
}

/// Write metadata into a PDF document's /Info dictionary.
pub fn write_metadata(doc: &mut Document, metadata: &PdfMetadata) {
    if !metadata.has_any() {
        return;
    }

    // Get or create /Info dictionary reference from trailer
    let info_id = get_or_create_info_dict(doc);

    if let Ok(Object::Dictionary(ref mut dict)) = doc.get_object_mut(info_id) {
        if let Some(ref title) = metadata.title {
            dict.set("Title", Object::string_literal(title.as_bytes()));
        }
        if let Some(ref author) = metadata.author {
            dict.set("Author", Object::string_literal(author.as_bytes()));
        }
        if let Some(ref subject) = metadata.subject {
            dict.set("Subject", Object::string_literal(subject.as_bytes()));
        }
        if let Some(ref keywords) = metadata.keywords {
            dict.set("Keywords", Object::string_literal(keywords.as_bytes()));
        }
        if let Some(ref creator) = metadata.creator {
            dict.set("Creator", Object::string_literal(creator.as_bytes()));
        }
        if let Some(ref producer) = metadata.producer {
            dict.set("Producer", Object::string_literal(producer.as_bytes()));
        }
    }
}

/// Read metadata from a PDF document's /Info dictionary.
pub fn read_metadata(doc: &Document) -> PdfMetadata {
    let info_ref = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(r)) => *r,
        _ => return PdfMetadata::default(),
    };

    let dict = match doc.get_object(info_ref).and_then(|o| o.as_dict()) {
        Ok(d) => d,
        Err(_) => return PdfMetadata::default(),
    };

    PdfMetadata {
        title: get_string(dict, b"Title"),
        author: get_string(dict, b"Author"),
        subject: get_string(dict, b"Subject"),
        keywords: get_string(dict, b"Keywords"),
        creator: get_string(dict, b"Creator"),
        producer: get_string(dict, b"Producer"),
    }
}

fn get_string(dict: &lopdf::Dictionary, key: &[u8]) -> Option<String> {
    dict.get(key).ok().and_then(|o| match o {
        Object::String(s, _) => Some(String::from_utf8_lossy(s).to_string()),
        _ => None,
    })
}

fn get_or_create_info_dict(doc: &mut Document) -> lopdf::ObjectId {
    // Check if /Info already exists in trailer
    if let Ok(Object::Reference(r)) = doc.trailer.get(b"Info") {
        return *r;
    }

    // Create a new /Info dictionary
    let info_dict = lopdf::Dictionary::new();
    let info_id = doc.add_object(Object::Dictionary(info_dict));
    doc.trailer.set("Info", Object::Reference(info_id));
    info_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    fn make_empty_pdf() -> Document {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![],
            "Count" => 0,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
        let catalog = dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages_id),
        };
        let catalog_id = doc.add_object(Object::Dictionary(catalog));
        doc.trailer.set("Root", Object::Reference(catalog_id));
        doc
    }

    #[test]
    fn test_write_and_read_metadata() {
        let mut doc = make_empty_pdf();
        let meta = PdfMetadata {
            title: Some("Test Title".to_string()),
            author: Some("Author".to_string()),
            subject: None,
            keywords: Some("pdf, test".to_string()),
            creator: None,
            producer: Some("EdgeParse".to_string()),
        };
        write_metadata(&mut doc, &meta);
        let read = read_metadata(&doc);
        assert_eq!(read.title.as_deref(), Some("Test Title"));
        assert_eq!(read.author.as_deref(), Some("Author"));
        assert_eq!(read.keywords.as_deref(), Some("pdf, test"));
        assert_eq!(read.producer.as_deref(), Some("EdgeParse"));
        assert!(read.subject.is_none());
    }

    #[test]
    fn test_empty_metadata_noop() {
        let mut doc = make_empty_pdf();
        let meta = PdfMetadata::default();
        assert!(!meta.has_any());
        write_metadata(&mut doc, &meta);
        // No /Info should be created
        assert!(doc.trailer.get(b"Info").is_err());
    }

    #[test]
    fn test_with_title() {
        let meta = PdfMetadata::with_title("Hello");
        assert!(meta.has_any());
        assert_eq!(meta.title.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_read_nonexistent_info() {
        let doc = make_empty_pdf();
        let meta = read_metadata(&doc);
        assert!(!meta.has_any());
    }
}
