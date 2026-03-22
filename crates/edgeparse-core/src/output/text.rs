//! Plain text output generator.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::table::TableTokenRow;
use crate::EdgePdfError;

/// Generate plain text representation of a PdfDocument.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on write failures.
pub fn to_text(doc: &PdfDocument) -> Result<String, EdgePdfError> {
    let mut output = String::new();

    if doc.kids.is_empty() {
        output.push_str("[No content extracted]\n");
        return Ok(output);
    }

    for element in &doc.kids {
        render_element(&mut output, element);
    }

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
            let text = h.base.base.value();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                out.push_str("\n\n");
            }
        }
        ContentElement::Paragraph(p) => {
            let text = p.base.value();
            let trimmed = clean_text(&text);
            if !trimmed.is_empty() {
                out.push_str(&trimmed);
                out.push_str("\n\n");
            }
        }
        ContentElement::List(list) => {
            for item in &list.list_items {
                let label = token_rows_text(&item.label.content);
                let body = token_rows_text(&item.body.content);
                let label_trimmed = label.trim();
                let body_trimmed = body.trim();
                if !label_trimmed.is_empty() || !body_trimmed.is_empty() {
                    if !label_trimmed.is_empty() && !body_trimmed.is_empty() {
                        out.push_str(&format!("  {} {}\n", label_trimmed, body_trimmed));
                    } else if !body_trimmed.is_empty() {
                        out.push_str(&format!("  {}\n", body_trimmed));
                    } else {
                        out.push_str(&format!("  {}\n", label_trimmed));
                    }
                }
            }
            out.push('\n');
        }
        ContentElement::Image(_) => {
            out.push_str("[Image]\n\n");
        }
        ContentElement::HeaderFooter(_) => {
            // Skip headers/footers in text by default
        }
        ContentElement::TextBlock(tb) => {
            let text = tb.value();
            let trimmed = clean_text(&text);
            if !trimmed.is_empty() {
                out.push_str(&trimmed);
                out.push_str("\n\n");
            }
        }
        ContentElement::TextLine(tl) => {
            let text = tl.value();
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                out.push('\n');
            }
        }
        ContentElement::TextChunk(tc) => {
            out.push_str(&tc.value);
        }
        _ => {}
    }
}

/// Clean paragraph text: trim whitespace, collapse multiple spaces.
fn clean_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_doc() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let text = to_text(&doc).unwrap();
        assert!(text.contains("No content extracted"));
    }
}
