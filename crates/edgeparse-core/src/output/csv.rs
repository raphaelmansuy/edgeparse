//! CSV output generator.
//!
//! Extracts table data from a PdfDocument and renders each table
//! as comma-separated values. Non-table content is output as single-column rows.

use crate::models::content::ContentElement;
use crate::models::document::PdfDocument;
use crate::models::table::TableBorderCell;
use crate::EdgePdfError;

/// Generate CSV representation of a PdfDocument.
///
/// Each table becomes a block of CSV rows. Non-table text appears
/// as single-column entries separated by blank lines between tables.
///
/// # Errors
/// Returns `EdgePdfError::OutputError` on failures.
pub fn to_csv(doc: &PdfDocument) -> Result<String, EdgePdfError> {
    let mut output = String::new();

    if doc.kids.is_empty() {
        return Ok(output);
    }

    let mut first = true;
    for element in &doc.kids {
        match element {
            ContentElement::TableBorder(table) => {
                if !first {
                    output.push('\n');
                }
                for row in &table.rows {
                    let cells: Vec<String> = row
                        .cells
                        .iter()
                        .map(|cell| csv_escape(&cell_text(cell)))
                        .collect();
                    output.push_str(&cells.join(","));
                    output.push('\n');
                }
                first = false;
            }
            ContentElement::Paragraph(p) => {
                let text = p.base.value();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    output.push_str(&csv_escape(trimmed));
                    output.push('\n');
                }
            }
            ContentElement::Heading(h) => {
                let text = h.base.base.value();
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    output.push_str(&csv_escape(trimmed));
                    output.push('\n');
                }
            }
            _ => {}
        }
    }

    Ok(output)
}

/// Extract plain text from a table cell's content tokens.
fn cell_text(cell: &TableBorderCell) -> String {
    cell.content
        .iter()
        .map(|token| token.base.value.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Escape a value for CSV: quote it if it contains commas, quotes, or newlines.
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_escape_plain() {
        assert_eq!(csv_escape("hello"), "hello");
    }

    #[test]
    fn test_csv_escape_comma() {
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
    }

    #[test]
    fn test_csv_escape_quotes() {
        assert_eq!(csv_escape("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn test_csv_escape_newline() {
        assert_eq!(csv_escape("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn test_empty_doc() {
        let doc = PdfDocument::new("test.pdf".to_string());
        let csv = to_csv(&doc).unwrap();
        assert!(csv.is_empty());
    }
}
