//! Enumerations for EdgeParse data models.

use serde::{Deserialize, Serialize};

/// Semantic type classification for PDF elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticType {
    /// Document root
    Document,
    /// Generic division
    Div,
    /// Text paragraph
    Paragraph,
    /// Inline span
    Span,
    /// Table element
    Table,
    /// Table headers section
    TableHeaders,
    /// Table footer section
    TableFooter,
    /// Table body section
    TableBody,
    /// Table row
    TableRow,
    /// Table header cell
    TableHeader,
    /// Table data cell
    TableCell,
    /// Form element
    Form,
    /// Hyperlink
    Link,
    /// Annotation
    Annot,
    /// Caption for image or table
    Caption,
    /// List container
    List,
    /// List item label
    ListLabel,
    /// List item body
    ListBody,
    /// List item
    ListItem,
    /// Table of contents
    TableOfContent,
    /// Table of contents item
    TableOfContentItem,
    /// Figure/image
    Figure,
    /// Numbered heading
    NumberHeading,
    /// Heading
    Heading,
    /// Title
    Title,
    /// Block quote
    BlockQuote,
    /// Footnote/endnote
    Note,
    /// Page header
    Header,
    /// Page footer
    Footer,
    /// Code block
    Code,
    /// Part/section
    Part,
}

impl SemanticType {
    /// Whether this type should be ignored in normal processing.
    pub fn is_ignored_standard_type(&self) -> bool {
        matches!(
            self,
            SemanticType::Div
                | SemanticType::Span
                | SemanticType::Form
                | SemanticType::Link
                | SemanticType::Annot
        )
    }
}

/// Text alignment within a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignment {
    /// Left-aligned
    Left,
    /// Right-aligned
    Right,
    /// Center-aligned
    Center,
    /// Justified
    Justify,
}

/// Text format (baseline position).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextFormat {
    /// Normal baseline
    #[default]
    Normal,
    /// Superscript
    Superscript,
    /// Subscript
    Subscript,
}

/// Text type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TextType {
    /// Regular text
    #[default]
    Regular,
    /// Large text
    Large,
    /// Logo/title text
    Logo,
}

/// Processing layer that produced/modified an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PdfLayer {
    /// Main content layer (initial extraction)
    #[default]
    Main,
    /// Raw content extraction
    Content,
    /// Table cell assignment
    TableCells,
    /// List item detection
    ListItems,
    /// Table content processing
    TableContent,
    /// List content processing
    ListContent,
    /// Text block processing
    TextBlockContent,
    /// Header and footer processing
    HeaderAndFooterContent,
}

/// Triage decision for hybrid mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TriageDecision {
    /// Process locally (Rust pipeline)
    Local,
    /// Send to backend
    Backend,
    /// Use both and merge
    Both,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_type_ignored() {
        assert!(SemanticType::Div.is_ignored_standard_type());
        assert!(SemanticType::Span.is_ignored_standard_type());
        assert!(!SemanticType::Paragraph.is_ignored_standard_type());
        assert!(!SemanticType::Heading.is_ignored_standard_type());
        assert!(!SemanticType::Table.is_ignored_standard_type());
    }

    #[test]
    fn test_text_format_default() {
        assert_eq!(TextFormat::default(), TextFormat::Normal);
    }
}
