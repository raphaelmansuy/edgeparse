# Tutorial 04 — Rust Library

**Goal:** Add `edgeparse-core` to your Rust project, convert PDFs programmatically, inspect the document model, and render to every output format.

→ **Previous:** [Node.js SDK](03-nodejs-sdk.md) · [CLI](01-cli.md) · [Python SDK](02-python-sdk.md)

---

## Table of Contents

1. [Add the dependency](#1-add-the-dependency)
2. [First conversion](#2-first-conversion)
3. [ProcessingConfig in full](#3-processingconfig-in-full)
4. [Inspect the document model](#4-inspect-the-document-model)
5. [Render to output formats](#5-render-to-output-formats)
6. [Page ranges](#6-page-ranges)
7. [Table detection methods](#7-table-detection-methods)
8. [Batch processing with Rayon](#8-batch-processing-with-rayon)
9. [Error handling](#9-error-handling)
10. [Building the CLI from this crate](#10-building-the-cli-from-this-crate)
11. [API reference](#11-api-reference)

---

## 1. Add the Dependency

`Cargo.toml`:

```toml
[dependencies]
edgeparse-core = "0.1"
```

Docs: [docs.rs/edgeparse-core](https://docs.rs/edgeparse-core)

If you also need the low-level PDF object model (lopdf fork):

```toml
[dependencies]
edgeparse-core = "0.1"
pdf-cos = "0.39"
```

---

## 2. First Conversion

```rust
use std::path::Path;
use edgeparse_core::{convert, EdgePdfError};
use edgeparse_core::api::config::{ProcessingConfig, OutputFormat};
use edgeparse_core::output;

fn main() -> Result<(), EdgePdfError> {
    let path = Path::new("examples/pdf/lorem.pdf");

    // Build a configuration — only set the fields you need
    let config = ProcessingConfig {
        formats: vec![OutputFormat::Markdown],
        quiet: true,
        ..ProcessingConfig::default()
    };

    // Convert — returns a PdfDocument
    let doc = convert(path, &config)?;

    // Render to Markdown
    let markdown = output::markdown::to_markdown(&doc)?;
    println!("{}", &markdown[..200.min(markdown.len())]);

    Ok(())
}
```

Run:

```bash
cargo run
```

---

## 3. ProcessingConfig in Full

`ProcessingConfig` has 24 fields. All have sensible defaults — only override what you need.

```rust
use edgeparse_core::api::config::{
    ProcessingConfig, OutputFormat, ReadingOrder, TableMethod,
    ImageOutput, ImageFormat, HybridBackend, HybridMode,
};
use edgeparse_core::api::filter::FilterConfig;

let config = ProcessingConfig {
    // ── Output ─────────────────────────────────────────────────────────────
    output_dir: Some("output/".to_string()),      // Write files here
    formats: vec![                                 // Produce multiple formats
        OutputFormat::Markdown,
        OutputFormat::Json,
    ],

    // ── Content ────────────────────────────────────────────────────────────
    pages: Some("1-5".to_string()),               // Only pages 1–5
    password: None,                                // Encrypted PDF password
    quiet: false,                                  // Print per-page log

    // ── Layout ─────────────────────────────────────────────────────────────
    reading_order: ReadingOrder::XyCut,            // XY-Cut++ (default)
    table_method: TableMethod::Default,            // Ruling-line detection
    keep_line_breaks: false,                       // Join soft breaks
    use_struct_tree: false,                        // Use tagged PDF tree
    include_header_footer: false,                  // Filter headers/footers

    // ── Safety & quality ───────────────────────────────────────────────────
    sanitize: false,                               // PII redaction
    filter_config: FilterConfig::default(),        // Content safety filters
    replace_invalid_chars: " ".to_string(),        // Replacement for bad chars

    // ── Images ─────────────────────────────────────────────────────────────
    image_output: ImageOutput::External,           // Save images to files
    image_format: ImageFormat::Png,
    image_dir: Some("output/images".to_string()),

    // ── Page separators ────────────────────────────────────────────────────
    markdown_page_separator: None,
    text_page_separator: None,
    html_page_separator: None,

    // ── Hybrid backend (advanced) ──────────────────────────────────────────
    hybrid: HybridBackend::Off,
    hybrid_mode: HybridMode::Auto,
    hybrid_url: None,
    hybrid_timeout: 30_000,
    hybrid_fallback: false,
};
```

---

## 4. Inspect the Document Model

`convert()` returns a `PdfDocument`. Traverse it to extract specific information:

```rust
use std::path::Path;
use edgeparse_core::{convert, EdgePdfError};
use edgeparse_core::api::config::{ProcessingConfig, OutputFormat};
use edgeparse_core::models::content::ContentElement;

fn main() -> Result<(), EdgePdfError> {
    let config = ProcessingConfig {
        formats: vec![OutputFormat::Markdown],
        quiet: true,
        ..ProcessingConfig::default()
    };

    let doc = convert(Path::new("examples/pdf/1901.03003.pdf"), &config)?;

    // ── Metadata ──────────────────────────────────────────────────────────
    println!("File:   {}", doc.file_name);
    println!("Pages:  {}", doc.number_of_pages);
    if let Some(ref title) = doc.title {
        println!("Title:  {title}");
    }
    if let Some(ref author) = doc.author {
        println!("Author: {author}");
    }

    // ── Iterate content elements ───────────────────────────────────────────
    for element in &doc.kids {
        match element {
            ContentElement::Heading(h) => {
                println!("[H{}] {}", h.level, h.base.base.content);
            }
            ContentElement::NumberHeading(nh) => {
                println!("[NH] {}", nh.base.base.base.content);
            }
            ContentElement::Paragraph(p) => {
                let text = &p.base.content;
                println!("[P]  {}", &text[..60.min(text.len())]);
            }
            ContentElement::Table(t) => {
                println!("[T]  table with {} rows", t.rows.len());
            }
            ContentElement::Image(img) => {
                println!("[I]  image on page {:?}", img.bbox.page_number);
            }
            _ => {}
        }
    }

    Ok(())
}
```

### BoundingBox coordinates

All elements have a `bbox()` method:

```rust
for element in &doc.kids {
    let bb = element.bbox();
    println!(
        "page={:?} x0={:.1} y0={:.1} x1={:.1} y1={:.1}",
        bb.page_number,
        bb.left_x, bb.bottom_y,
        bb.right_x, bb.top_y,
    );
}
```

PDF coordinate origin is at the bottom-left; Y increases upward. Units are PDF points (72 pt = 1 inch).

### Filter elements by page

```rust
let page1_elements: Vec<_> = doc.kids.iter()
    .filter(|e| e.bbox().page_number == Some(1))
    .collect();
println!("{} elements on page 1", page1_elements.len());
```

### Extract all text content

```rust
let all_text: Vec<String> = doc.kids.iter()
    .filter_map(|e| match e {
        ContentElement::Paragraph(p) => Some(p.base.content.clone()),
        ContentElement::Heading(h) => Some(h.base.base.content.clone()),
        ContentElement::NumberHeading(nh) => Some(nh.base.base.base.content.clone()),
        _ => None,
    })
    .collect();

let full_text = all_text.join("\n\n");
println!("Total chars: {}", full_text.len());
```

---

## 5. Render to Output Formats

```rust
use edgeparse_core::output;

// Markdown
let md = output::markdown::to_markdown(&doc)?;

// JSON (with bounding boxes — the "legacy" flat JSON format)
let json_str = output::legacy_json::to_legacy_json_string(&doc, "stem_name")?;

// HTML5
let html = output::html::to_html(&doc)?;

// Plain text
let text = output::text::to_text(&doc)?;
```

Write to files:

```rust
use std::fs;

fs::write("output/document.md", &md)?;
fs::write("output/document.json", &json_str)?;
fs::write("output/document.html", &html)?;
fs::write("output/document.txt", &text)?;
```

---

## 6. Page Ranges

```rust
let config = ProcessingConfig {
    formats: vec![OutputFormat::Markdown],
    pages: Some("1,3,5-7".to_string()),
    ..ProcessingConfig::default()
};
let doc = convert(Path::new("paper.pdf"), &config)?;
```

Pages are 1-indexed. Out-of-range pages are silently skipped.

---

## 7. Table Detection Methods

```rust
use edgeparse_core::api::config::TableMethod;

// Ruling-line detection (default — best for PDFs with visible table borders)
let ruling_config = ProcessingConfig {
    table_method: TableMethod::Default,
    ..ProcessingConfig::default()
};

// Cluster / geometric detection (best for borderless tables)
let cluster_config = ProcessingConfig {
    table_method: TableMethod::Cluster,
    ..ProcessingConfig::default()
};
```

---

## 8. Batch Processing with Rayon

The `convert()` function is thread-safe. Use Rayon for parallel processing:

```rust
use std::path::{Path, PathBuf};
use rayon::prelude::*;
use edgeparse_core::{convert, EdgePdfError};
use edgeparse_core::api::config::{ProcessingConfig, OutputFormat};
use edgeparse_core::output;

fn process_batch(pdfs: &[PathBuf]) -> Vec<Result<(PathBuf, String), EdgePdfError>> {
    let config = ProcessingConfig {
        formats: vec![OutputFormat::Markdown],
        quiet: true,
        ..ProcessingConfig::default()
    };

    pdfs.par_iter()
        .map(|path| {
            let doc = convert(path.as_path(), &config)?;
            let md = output::markdown::to_markdown(&doc)?;
            Ok((path.clone(), md))
        })
        .collect()
}

fn main() {
    let pdfs: Vec<PathBuf> = std::fs::read_dir("examples/pdf")
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |e| e == "pdf"))
        .collect();

    let results = process_batch(&pdfs);

    for result in results {
        match result {
            Ok((path, md)) => {
                println!("✓ {} ({} chars)", path.display(), md.len());
            }
            Err(e) => {
                eprintln!("✗ {e}");
            }
        }
    }
}
```

`Cargo.toml` for the above:

```toml
[dependencies]
edgeparse-core = "0.1"
rayon = "1"
```

---

## 9. Error Handling

`convert()` returns `Result<PdfDocument, EdgePdfError>`.

```rust
use edgeparse_core::{convert, EdgePdfError};

match convert(Path::new("document.pdf"), &config) {
    Ok(doc) => {
        println!("Converted {} pages", doc.number_of_pages);
    }
    Err(EdgePdfError::LoadError(msg)) => {
        eprintln!("Cannot open PDF: {msg}");
    }
    Err(EdgePdfError::LopdfError(msg)) => {
        eprintln!("Malformed PDF: {msg}");
    }
    Err(EdgePdfError::PipelineError { stage, message }) => {
        eprintln!("Pipeline failed at stage {stage}: {message}");
    }
    Err(EdgePdfError::OutputError(msg)) => {
        eprintln!("Render error: {msg}");
    }
    Err(EdgePdfError::IoError(e)) => {
        eprintln!("I/O error: {e}");
    }
    Err(EdgePdfError::ConfigError(msg)) => {
        eprintln!("Config error: {msg}");
    }
}
```

`EdgePdfError` implements `std::error::Error` and `Display`, so it composes with `?` and `anyhow`/`thiserror`:

```rust
use anyhow::Result;

fn convert_to_markdown(path: &Path) -> Result<String> {
    let config = ProcessingConfig {
        formats: vec![OutputFormat::Markdown],
        ..ProcessingConfig::default()
    };
    let doc = convert(path, &config)?;  // ? converts EdgePdfError → anyhow::Error
    Ok(output::markdown::to_markdown(&doc)?)
}
```

---

## 10. Building the CLI from This Crate

The `edgeparse-cli` crate is a thin clap wrapper around `edgeparse-core`. If you want your own CLI:

```toml
# Cargo.toml
[package]
name = "my-pdf-tool"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "my-pdf-tool"

[dependencies]
edgeparse-core = "0.1"
clap = { version = "4", features = ["derive"] }
```

```rust
// src/main.rs
use clap::Parser;
use std::path::PathBuf;
use edgeparse_core::{convert, EdgePdfError};
use edgeparse_core::api::config::{ProcessingConfig, OutputFormat};
use edgeparse_core::output;

#[derive(Parser)]
struct Cli {
    input: PathBuf,
    #[arg(short = 'o', default_value = "output")]
    output_dir: String,
    #[arg(short = 'f', default_value = "markdown")]
    format: String,
}

fn main() -> Result<(), EdgePdfError> {
    let cli = Cli::parse();

    let fmt = match cli.format.as_str() {
        "json" => OutputFormat::Json,
        "html" => OutputFormat::Html,
        "text" => OutputFormat::Text,
        _ => OutputFormat::Markdown,
    };

    let config = ProcessingConfig {
        formats: vec![fmt],
        output_dir: Some(cli.output_dir),
        quiet: false,
        ..ProcessingConfig::default()
    };

    let doc = convert(&cli.input, &config)?;
    println!("Converted {} ({} pages)", doc.file_name, doc.number_of_pages);

    Ok(())
}
```

---

## 11. API Reference

### `edgeparse_core::convert()`

```rust
pub fn convert(
    input_path: &Path,
    config: &ProcessingConfig,
) -> Result<PdfDocument, EdgePdfError>
```

Main entry point. Thread-safe. Internally uses Rayon for page-level parallelism.

### `ProcessingConfig`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `formats` | `Vec<OutputFormat>` | `[Json]` | Output formats to produce |
| `pages` | `Option<String>` | `None` | Page range, e.g. `"1,3,5-7"` |
| `password` | `Option<String>` | `None` | Encrypted PDF password |
| `output_dir` | `Option<String>` | `None` | Write files here |
| `quiet` | `bool` | `false` | Suppress log output |
| `reading_order` | `ReadingOrder` | `XyCut` | `XyCut` or `Off` |
| `table_method` | `TableMethod` | `Default` | `Default` or `Cluster` |
| `image_output` | `ImageOutput` | `External` | `Off`, `Embedded`, `External` |
| `image_format` | `ImageFormat` | `Png` | `Png` or `Jpeg` |
| `image_dir` | `Option<String>` | `None` | Directory for extracted images |
| `keep_line_breaks` | `bool` | `false` | Preserve soft line breaks |
| `use_struct_tree` | `bool` | `false` | Use tagged PDF structure tree |
| `include_header_footer` | `bool` | `false` | Include headers/footers |
| `sanitize` | `bool` | `false` | PII redaction |

### `PdfDocument`

```rust
pub struct PdfDocument {
    pub file_name: String,
    pub number_of_pages: u32,
    pub author: Option<String>,
    pub title: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub producer: Option<String>,
    pub creator: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub kids: Vec<ContentElement>,  // all extracted elements in reading order
}
```

### `ContentElement` variants

| Variant | Access text | Notes |
|---------|------------|-------|
| `Heading(h)` | `h.base.base.content` | Has `h.level: u32` |
| `NumberHeading(nh)` | `nh.base.base.base.content` | Numbered sections |
| `Paragraph(p)` | `p.base.content` | |
| `Table(t)` | `t.rows` | Nested `Vec<Vec<String>>` |
| `Image(img)` | — | Has `img.bbox` |
| `List(l)` | `l.items` | |
| `Caption(c)` | `c.base.content` | |
| `HeaderFooter(hf)` | `hf.content` | Filtered by default |

All variants expose `.bbox()` → `&BoundingBox`.

### `BoundingBox`

```rust
pub struct BoundingBox {
    pub page_number: Option<u32>,    // 1-based
    pub left_x: f64,
    pub bottom_y: f64,               // PDF origin: bottom-left
    pub right_x: f64,
    pub top_y: f64,
}
```

Units: PDF points (72 pt = 1 inch).

### `EdgePdfError`

```rust
pub enum EdgePdfError {
    LoadError(String),
    PipelineError { stage: u32, message: String },
    OutputError(String),
    IoError(std::io::Error),
    ConfigError(String),
    LopdfError(String),
}
```

---

→ **Continue:** [Output Formats Deep-Dive](05-output-formats.md)
