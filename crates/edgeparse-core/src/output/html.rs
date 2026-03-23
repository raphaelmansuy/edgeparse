//! HTML5 output generator.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::table::TableTokenRow;
use crate::EdgePdfError;

/// Generate HTML5 representation of a PdfDocument.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on write failures.
pub fn to_html(doc: &PdfDocument) -> Result<String, EdgePdfError> {
    let mut output = String::new();
    output.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    output.push_str("<meta charset=\"UTF-8\">\n");

    if let Some(ref title) = doc.title {
        output.push_str(&format!("<title>{}</title>\n", html_escape(title)));
    } else {
        output.push_str(&format!("<title>{}</title>\n", html_escape(&doc.file_name)));
    }

    output.push_str("</head>\n<body>\n");

    if doc.kids.is_empty() {
        output.push_str("<p>No content extracted.</p>\n");
    } else {
        for element in &doc.kids {
            render_element(&mut output, element);
        }
    }

    output.push_str("</body>\n</html>\n");
    Ok(output)
}

/// Extract text from table token rows.
fn token_rows_text(rows: &[TableTokenRow]) -> String {
    rows.iter()
        .flat_map(|row| row.iter())
        .map(|token| token.base.value.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_element(out: &mut String, element: &ContentElement) {
    match element {
        ContentElement::Heading(h) => {
            let level = h.heading_level.unwrap_or(1).clamp(1, 6);
            let text = html_escape(&h.base.base.value());
            out.push_str(&format!("<h{level}>{text}</h{level}>\n"));
        }
        ContentElement::Paragraph(p) => {
            let text = html_escape(&p.base.value());
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("<p>{trimmed}</p>\n"));
            }
        }
        ContentElement::List(list) => {
            out.push_str("<ul>\n");
            for item in &list.list_items {
                let label = token_rows_text(&item.label.content);
                let body = token_rows_text(&item.body.content);
                out.push_str(&format!(
                    "<li>{}{}</li>\n",
                    html_escape(label.trim()),
                    html_escape(body.trim())
                ));
            }
            out.push_str("</ul>\n");
        }
        ContentElement::Image(_) => {
            out.push_str("<img src=\"image\" alt=\"Image\">\n");
        }
        ContentElement::HeaderFooter(_) => {
            // Skip headers/footers in HTML by default
        }
        ContentElement::TextBlock(tb) => {
            let text = html_escape(&tb.value());
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("<p>{trimmed}</p>\n"));
            }
        }
        ContentElement::TextLine(tl) => {
            let text = html_escape(&tl.value());
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(&format!("<span>{trimmed}</span>\n"));
            }
        }
        ContentElement::TextChunk(tc) => {
            out.push_str(&html_escape(&tc.value));
        }
        _ => {}
    }
}

/// Escape HTML special characters.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
    }

    #[test]
    fn test_html_structure() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let html = to_html(&doc).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html lang=\"en\">"));
        assert!(html.contains("</body>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn test_html_title() {
        let mut doc = PdfDocument::new("test.pdf".to_string());
        doc.title = Some("Test <Title>".to_string());
        let html = to_html(&doc).unwrap();
        assert!(html.contains("<title>Test &lt;Title&gt;</title>"));
    }
}
