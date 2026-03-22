//! Unified ContentElement enum — all page content.

use serde::{Deserialize, Serialize};

use super::bbox::BoundingBox;
use super::chunks::{ImageChunk, LineArtChunk, LineChunk, TextChunk};
use super::list::PDFList;
use super::semantic::{
    SemanticCaption, SemanticFigure, SemanticFormula, SemanticHeaderOrFooter, SemanticHeading,
    SemanticNumberHeading, SemanticParagraph, SemanticPicture, SemanticTable,
};
use super::table::TableBorder;
use super::text::{TextBlock, TextLine};

/// Unified enum for all content elements on a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentElement {
    /// Raw text chunk
    TextChunk(TextChunk),
    /// Grouped text line
    TextLine(TextLine),
    /// Grouped text block
    TextBlock(TextBlock),
    /// Image bounding box
    Image(ImageChunk),
    /// Line segment
    Line(LineChunk),
    /// Vector graphic
    LineArt(LineArtChunk),
    /// Table border structure
    TableBorder(TableBorder),
    /// List
    List(PDFList),
    /// Paragraph
    Paragraph(SemanticParagraph),
    /// Heading
    Heading(SemanticHeading),
    /// Numbered heading
    NumberHeading(SemanticNumberHeading),
    /// Caption
    Caption(SemanticCaption),
    /// Header or footer
    HeaderFooter(SemanticHeaderOrFooter),
    /// Figure
    Figure(SemanticFigure),
    /// Formula
    Formula(SemanticFormula),
    /// Picture with description
    Picture(SemanticPicture),
    /// Table (semantic wrapper)
    Table(SemanticTable),
}

impl ContentElement {
    /// Get the bounding box of this element.
    pub fn bbox(&self) -> &BoundingBox {
        match self {
            Self::TextChunk(e) => &e.bbox,
            Self::TextLine(e) => &e.bbox,
            Self::TextBlock(e) => &e.bbox,
            Self::Image(e) => &e.bbox,
            Self::Line(e) => &e.bbox,
            Self::LineArt(e) => &e.bbox,
            Self::TableBorder(e) => &e.bbox,
            Self::List(e) => &e.bbox,
            Self::Paragraph(e) => &e.base.bbox,
            Self::Heading(e) => &e.base.base.bbox,
            Self::NumberHeading(e) => &e.base.base.base.bbox,
            Self::Caption(e) => &e.base.bbox,
            Self::HeaderFooter(e) => &e.bbox,
            Self::Figure(e) => &e.bbox,
            Self::Formula(e) => &e.bbox,
            Self::Picture(e) => &e.bbox,
            Self::Table(e) => &e.bbox,
        }
    }

    /// Get the global index.
    pub fn index(&self) -> Option<u32> {
        match self {
            Self::TextChunk(e) => e.index.map(|i| i as u32),
            Self::TextLine(e) => e.index,
            Self::TextBlock(e) => e.index,
            Self::Image(e) => e.index,
            Self::Line(e) => e.index,
            Self::LineArt(e) => e.index,
            Self::TableBorder(e) => e.index,
            Self::List(e) => e.index,
            Self::Paragraph(e) => e.base.index,
            Self::Heading(e) => e.base.base.index,
            Self::NumberHeading(e) => e.base.base.base.index,
            Self::Caption(e) => e.base.index,
            Self::HeaderFooter(e) => e.index,
            Self::Figure(e) => e.index,
            Self::Formula(e) => e.index,
            Self::Picture(e) => e.index,
            Self::Table(e) => e.index,
        }
    }

    /// Get the page number.
    pub fn page_number(&self) -> Option<u32> {
        self.bbox().page_number
    }

    /// Set the global index.
    pub fn set_index(&mut self, idx: u32) {
        match self {
            Self::TextChunk(e) => e.index = Some(idx as usize),
            Self::TextLine(e) => e.index = Some(idx),
            Self::TextBlock(e) => e.index = Some(idx),
            Self::Image(e) => e.index = Some(idx),
            Self::Line(e) => e.index = Some(idx),
            Self::LineArt(e) => e.index = Some(idx),
            Self::TableBorder(e) => e.index = Some(idx),
            Self::List(e) => e.index = Some(idx),
            Self::Paragraph(e) => e.base.index = Some(idx),
            Self::Heading(e) => e.base.base.index = Some(idx),
            Self::NumberHeading(e) => e.base.base.base.index = Some(idx),
            Self::Caption(e) => e.base.index = Some(idx),
            Self::HeaderFooter(e) => e.index = Some(idx),
            Self::Figure(e) => e.index = Some(idx),
            Self::Formula(e) => e.index = Some(idx),
            Self::Picture(e) => e.index = Some(idx),
            Self::Table(e) => e.index = Some(idx),
        }
    }
}
