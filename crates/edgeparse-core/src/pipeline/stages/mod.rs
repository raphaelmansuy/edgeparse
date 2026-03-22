//! Pipeline stages — individual processing steps.

pub mod boxed_heading_promoter;
pub mod caption_linker;
pub mod cluster_table_detector;
pub mod column_detector;
pub mod content_filter;
pub mod content_sanitizer;
pub mod cross_page_linker;
pub mod figure_detector;
pub mod footnote_detector;
pub mod footnote_linker;
pub mod header_footer;
pub mod heading_detector;
pub mod id_assignment;
pub mod list_detector;
pub mod list_pass2;
pub mod nesting_level;
pub mod output_builder;
pub mod paragraph_detector;
pub mod reading_order;
pub mod table_content_assigner;
pub mod table_detector;
pub mod text_block_grouper;
pub mod text_line_grouper;
pub mod toc_detector;
pub mod watermark_detector;
