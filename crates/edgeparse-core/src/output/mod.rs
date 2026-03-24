//! Output generators — TOC, JSON, Markdown, HTML, Text, CSV, Annotated PDF.

pub mod csv;
#[cfg(not(target_arch = "wasm32"))]
pub mod docx;
pub mod html;
pub mod json;
pub mod legacy_json;
pub mod markdown;
pub mod text;
pub mod toc_builder;
