# edgeparse

High-performance PDF-to-structured-data extraction CLI.

Convert PDF documents to Markdown, JSON, HTML, or plain text with a single
command. Built on top of [`edgeparse-core`](https://crates.io/crates/edgeparse-core).

## Installation

```bash
cargo install edgeparse
```

## Usage

```bash
# Convert to Markdown (default format is JSON)
edgeparse report.pdf -f markdown -o output/

# Convert to multiple formats
edgeparse report.pdf -f json,markdown,html -o output/

# Extract specific pages
edgeparse report.pdf --pages 1,3,5-7 -f markdown -o output/

# Use XY-Cut reading order (enabled by default)
edgeparse report.pdf --reading-order xycut -f markdown

# Extract with cluster-based table detection
edgeparse report.pdf --table-method cluster -f json

# Extract images externally
edgeparse report.pdf --image-output external --image-format png -f markdown
```

## Features

- **Multiple output formats** — JSON, Markdown, HTML, plain text, DOCX, CSV
- **Table detection** — border-based and cluster detection methods
- **Reading order** — XY-Cut++ algorithm for correct multi-column reading order
- **Image extraction** — embedded base64 or external file output (PNG/JPEG)
- **Content safety** — filters hidden text, off-page content, watermarks
- **Encrypted PDFs** — password-based decryption support
- **Tagged PDFs** — uses PDF structure tree when available
- **PII sanitization** — optional personal data redaction

## License

Apache-2.0
