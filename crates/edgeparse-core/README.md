# edgeparse-core

High-performance PDF-to-structured-data extraction engine.

`edgeparse-core` implements a 20-stage processing pipeline that extracts text,
tables, images, and semantic structure from PDF documents and produces
structured output in Markdown, JSON, HTML, or plain text.

## Usage

```rust
use edgeparse_core::{convert, api::config::ProcessingConfig};
use std::path::Path;

let config = ProcessingConfig::default();
let doc = convert(Path::new("report.pdf"), &config)?;

println!("Pages: {}", doc.number_of_pages);
for element in &doc.kids {
    // process extracted content elements
}
```

## Output Formats

Generate output in multiple formats using the output modules:

```rust
use edgeparse_core::output;

let markdown = output::markdown::to_markdown(&doc)?;
let json = output::legacy_json::to_legacy_json_string(&doc, "report")?;
let html = output::html::to_html(&doc)?;
let text = output::text::to_text(&doc)?;
```

## Features

- **Tagged PDF support** — uses PDF structure tree for semantic extraction
- **Table detection** — border-based and cluster detection methods
- **Reading order** — XY-Cut++ algorithm for correct reading order
- **Image extraction** — embedded or external image output
- **Content safety** — filters hidden text, off-page content, tiny text
- **PII sanitization** — optional personal data redaction
- **Multi-column layout** — automatic column detection and ordering

## Feature Flags

| Flag | Description |
|------|-------------|
| `hybrid` | Enable Docling backend integration (requires `tokio` + `reqwest`) |

## License

Apache-2.0
