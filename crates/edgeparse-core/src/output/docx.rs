//! DOCX output generator (minimal stub).
//!
//! Produces a minimal Open XML (OOXML) .docx file as raw bytes.
//! The DOCX format is a Zip archive containing XML parts.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::table::TableTokenRow;
use crate::EdgePdfError;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Generate a DOCX file as bytes from a PdfDocument.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on write failures.
pub fn to_docx(doc: &PdfDocument) -> Result<Vec<u8>, EdgePdfError> {
    let buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buffer);
    let options = SimpleFileOptions::default();

    // Content Types
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;
    zip.write_all(CONTENT_TYPES_XML.as_bytes())
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;

    // Relationships
    zip.start_file("_rels/.rels", options)
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;
    zip.write_all(RELS_XML.as_bytes())
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;

    // Document relationships
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;
    zip.write_all(DOC_RELS_XML.as_bytes())
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;

    // Main document body
    let body_xml = build_document_xml(doc);
    zip.start_file("word/document.xml", options)
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;
    zip.write_all(body_xml.as_bytes())
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;

    let cursor = zip
        .finish()
        .map_err(|e| EdgePdfError::OutputError(e.to_string()))?;
    Ok(cursor.into_inner())
}

/// Build the word/document.xml content.
fn build_document_xml(doc: &PdfDocument) -> String {
    let mut body = String::new();

    if doc.kids.is_empty() {
        body.push_str(&make_paragraph("No content extracted."));
    } else {
        for element in &doc.kids {
            render_element(&mut body, element);
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<w:body>
{body}
</w:body>
</w:document>"#
    )
}

fn render_element(out: &mut String, element: &ContentElement) {
    match element {
        ContentElement::Heading(h) => {
            let level = h.heading_level.unwrap_or(1).clamp(1, 6);
            let text = xml_escape(&h.base.base.value());
            out.push_str(&make_heading(&text, level));
        }
        ContentElement::Paragraph(p) => {
            let text = xml_escape(&p.base.value());
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(&make_paragraph(trimmed));
            }
        }
        ContentElement::List(list) => {
            for item in &list.list_items {
                let label = token_rows_text(&item.label.content);
                let body = token_rows_text(&item.body.content);
                let text = format!("{} {}", label.trim(), body.trim());
                out.push_str(&make_paragraph(&xml_escape(&text)));
            }
        }
        ContentElement::TextBlock(tb) => {
            let text = xml_escape(&tb.value());
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(&make_paragraph(trimmed));
            }
        }
        _ => {}
    }
}

fn token_rows_text(rows: &[TableTokenRow]) -> String {
    rows.iter()
        .flat_map(|row| row.iter())
        .map(|token| token.base.value.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn make_paragraph(text: &str) -> String {
    format!("<w:p><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>\n")
}

fn make_heading(text: &str, level: u32) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"Heading{level}\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>\n"
    )
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

const RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const DOC_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_doc_produces_valid_docx() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let bytes = to_docx(&doc).unwrap();
        // DOCX is a ZIP — starts with PK signature
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_make_paragraph() {
        let p = make_paragraph("Hello");
        assert!(p.contains("<w:t xml:space=\"preserve\">Hello</w:t>"));
    }

    #[test]
    fn test_make_heading() {
        let h = make_heading("Title", 1);
        assert!(h.contains("Heading1"));
        assert!(h.contains("Title"));
    }
}
