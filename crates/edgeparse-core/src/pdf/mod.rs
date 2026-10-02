//! PDF loading layer — document loading, text extraction, line extraction.

pub mod annotation_enrichment;
pub mod annotation_extractor;
pub mod bookmark_extractor;
pub mod chunk_parser;
pub mod encryption;
pub mod font;
pub mod font_style;
pub mod font_type3;
/// Type 3 CharProc → Unicode OCR cache (requires `image`).
#[cfg(feature = "image")]
pub mod font_type3_ocr;
pub mod form_extractor;
pub mod graphics_state;
pub mod hyperlink_extractor;
pub mod image_extractor;
/// Image XObject codecs (DCT / Flate / JBIG2 / JPX) for the OCR path.
#[cfg(feature = "image")]
pub mod image_codecs;
/// Cheap image-region classifier + OCR budget (native + WASM).
pub mod image_region;
/// In-memory Image-XObject → table lattice (WASM-safe; no pdfimages).
pub mod inmem_raster;
pub mod line_extractor;
pub mod loader;
pub mod metadata_writer;
/// Optional Content Group default visibility.
pub mod ocg;
/// Pluggable raster OCR engines (Tesseract CLI / ocrs).
pub mod ocr;
pub mod page_info;
pub mod pdf_string;
#[cfg(not(target_arch = "wasm32"))]
pub mod raster_table_ocr;
pub mod text_extractor;
