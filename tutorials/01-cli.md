# Tutorial 01 — CLI Quick-Start

**Goal:** Install the `edgeparse` CLI, convert PDFs to every supported format, and master every flag.

→ **Next:** [Python SDK](02-python-sdk.md) · [Node.js SDK](03-nodejs-sdk.md) · [Rust library](04-rust-library.md)

---

## Table of Contents

1. [Installation](#1-installation)
2. [First conversion](#2-first-conversion)
3. [Output formats](#3-output-formats)
4. [Page ranges](#4-page-ranges)
5. [Multiple files and batch output](#5-multiple-files-and-batch-output)
6. [Table detection methods](#6-table-detection-methods)
7. [Image extraction](#7-image-extraction)
8. [Reading order control](#8-reading-order-control)
9. [Encrypted PDFs](#9-encrypted-pdfs)
10. [Content safety filters](#10-content-safety-filters)
11. [Layout options](#11-layout-options)
12. [Page separators](#12-page-separators)
13. [Complete flag reference](#13-complete-flag-reference)

---

## 1. Installation

### Option A — Install from crates.io (recommended)

```bash
cargo install edgeparse-cli
```

This puts `edgeparse` on your `$PATH`. Requires [Rust 1.85+](https://rustup.rs/).

Verify:

```bash
edgeparse --version
# edgeparse 0.1.0
```

### Option B — Build from source

```bash
git clone https://github.com/raphaelmansuy/edgeparse.git
cd edgeparse
cargo build --release
```

The binary is at `target/release/edgeparse`. Either add it to your `$PATH` or run it as `./target/release/edgeparse`.

```bash
./target/release/edgeparse --version
# edgeparse 0.1.0
```

### Option C — Python package brings a CLI too

If you already have the Python SDK installed, it registers the same `edgeparse` command:

```bash
pip install edgeparse
edgeparse --version
```

> The Python-installed CLI is a Python wrapper; the Rust binary from `cargo install edgeparse-cli` is typically 5–10× faster for large batches.

---

## 2. First Conversion

Convert a PDF to Markdown and write the result to `output/`:

```bash
edgeparse examples/pdf/lorem.pdf --format markdown --output-dir output/
```

Expected output file: `output/lorem.md`

Content:

```markdown
# Lorem

# Ipsum

Lorem ipsum dolor sit amet, incididunt ut labore et dolore exercitation ullamco
laboris dolor in reprehenderit ...
```

To print to **stdout** instead of writing a file, omit `--output-dir`:

```bash
edgeparse examples/pdf/lorem.pdf --format markdown
```

---

## 3. Output Formats

EdgeParse supports five output formats. Use `-f` / `--format`:

### Markdown (default)

```bash
edgeparse examples/pdf/lorem.pdf -f markdown -o output/
```

Output: `output/lorem.md` — headings, paragraphs, and GFM tables.

### JSON with bounding boxes

```bash
edgeparse examples/pdf/lorem.pdf -f json -o output/
```

Output: `output/lorem.json` — full structured data including element type, bounding box coordinates, font information, and page number. Example:

```json
{
  "file name": "lorem.pdf",
  "number of pages": 1,
  "kids": [
    {
      "type": "paragraph",
      "id": 1,
      "page number": 1,
      "bounding box": [200.89, 706.94, 299.69, 745.09],
      "font": "Pretendard-Regular",
      "font size": 32.0,
      "content": "Lorem"
    }
  ]
}
```

See [Tutorial 05 — Output Formats](05-output-formats.md#json) for the full JSON schema.

### HTML

```bash
edgeparse examples/pdf/lorem.pdf -f html -o output/
```

Output: `output/lorem.html` — HTML5 document with semantic `<p>`, `<h1>`–`<h6>`, and `<table>` elements.

### Plain text

```bash
edgeparse examples/pdf/lorem.pdf -f text -o output/
```

Output: `output/lorem.txt` — UTF-8 text preserving reading order.

### Multiple formats at once

Combine formats with a comma. One output file per format:

```bash
edgeparse examples/pdf/lorem.pdf -f markdown,json,html,text -o output/
# Writes: output/lorem.md  output/lorem.json  output/lorem.html  output/lorem.txt
```

### Markdown variants

| Value | Description |
|-------|-------------|
| `markdown` | Standard Markdown with GFM tables |
| `markdown-with-html` | Markdown with HTML `<table>` fallback for complex tables |
| `markdown-with-images` | Markdown with images linked or embedded |

```bash
edgeparse examples/pdf/1901.03003.pdf -f markdown-with-html -o output/
```

---

## 4. Page Ranges

Extract a subset of pages with `--pages`:

```bash
# Only pages 1 and 2
edgeparse examples/pdf/1901.03003.pdf -f markdown --pages "1-2" -o output/

# Pages 1, 3, and 5 through 7
edgeparse examples/pdf/1901.03003.pdf -f markdown --pages "1,3,5-7" -o output/

# Just page 1
edgeparse examples/pdf/1901.03003.pdf -f markdown --pages "1" -o output/
```

Pages are 1-indexed. Out-of-range pages are silently skipped.

---

## 5. Multiple Files and Batch Output

Pass multiple files — EdgeParse processes them in parallel:

```bash
edgeparse examples/pdf/*.pdf -f markdown -o output/
```

Each PDF produces its own output file in `output/`.

> Tip: EdgeParse uses Rayon for per-file and per-page parallelism. On an M-series Mac, processing 200 PDFs averages 0.023 s/doc.

Suppress the per-file log messages with `--quiet` / `-q`:

```bash
edgeparse examples/pdf/*.pdf -f markdown -o output/ -q
```

---

## 6. Table Detection Methods

EdgeParse has two table detection modes:

| Flag | Description | Best for |
|------|-------------|---------|
| `--table-method default` | Ruling-line detection | PDFs with visible table borders |
| `--table-method cluster` | Geometric clustering | Borderless/whitespace tables |

```bash
# Default (ruling lines)
edgeparse document.pdf -f markdown --table-method default -o output/

# Cluster (borderless tables)
edgeparse document.pdf -f markdown --table-method cluster -o output/
```

Most academic papers and reports use ruled tables — the default is correct for those. Use `cluster` for spreadsheet exports or looser layouts.

---

## 7. Image Extraction

Control how images are handled with `--image-output`:

| Value | Description | Output |
|-------|-------------|--------|
| `off` | Skip images entirely | No image data |
| `external` | Extract to files (default) | PNG/JPEG in `--image-dir` |
| `embedded` | Base64 data URIs in text | Inline in Markdown/HTML |

### External images (default)

```bash
edgeparse examples/pdf/lorem.pdf -f markdown \
  --image-output external \
  --image-dir output/images/ \
  -o output/
```

Images are saved as `output/images/page1_img1.png`, etc. The Markdown file references them with relative paths.

### Embedded images (data URIs)

```bash
edgeparse examples/pdf/lorem.pdf -f markdown \
  --image-output embedded \
  -o output/
```

Images are base64-encoded inline in the Markdown — useful for self-contained documents.

### Image format

```bash
# PNG (default, lossless)
edgeparse document.pdf --image-output external --image-format png -o output/

# JPEG (smaller, lossy)
edgeparse document.pdf --image-output external --image-format jpeg -o output/
```

---

## 8. Reading Order Control

XY-Cut++ is the default. The `off` mode outputs text in the order it appears in the PDF content stream — useful for debugging or PDFs with non-standard layout:

```bash
# Default: XY-Cut++ reading order (correct for most PDFs)
edgeparse document.pdf -f markdown --reading-order xycut -o output/

# Raw content-stream order
edgeparse document.pdf -f markdown --reading-order off -o output/
```

---

## 9. Encrypted PDFs

Pass the password with `-p` / `--password`:

```bash
edgeparse secure.pdf -f markdown -p "mypassword" -o output/
```

---

## 10. Content Safety Filters

EdgeParse removes hidden, off-page, and invisible text by default to prevent prompt-injection attacks in RAG pipelines. Disable specific filters with `--content-safety-off`:

| Flag value | Filter disabled |
|-----------|----------------|
| `hidden-text` | Rendering-mode hidden text (`Tr=3`) |
| `off-page` | Text outside the visible page box |
| `tiny` | Text smaller than 1 pt |
| `hidden-ocg` | Text in hidden optional content groups |
| `all` | All safety filters |

```bash
# Disable all filters (e.g., for debugging)
edgeparse document.pdf -f markdown --content-safety-off all -o output/

# Disable only off-page filter
edgeparse document.pdf -f markdown --content-safety-off off-page -o output/
```

> **Security note:** Only disable safety filters if you control and trust the PDF source.

---

## 11. Layout Options

### Preserve line breaks

By default, EdgeParse joins soft line-breaks within paragraphs. Use `--keep-line-breaks` to preserve them:

```bash
edgeparse document.pdf -f markdown --keep-line-breaks -o output/
```

### Include headers and footers

Headers and footers are filtered by default. Include them with:

```bash
edgeparse document.pdf -f markdown --include-header-footer -o output/
```

### Tagged PDF structure tree

For tagged PDFs (accessibility-compliant), use the structure tree to improve heading and list detection:

```bash
edgeparse document.pdf -f markdown --use-struct-tree -o output/
```

### PII sanitization

Redact common PII patterns (emails, phone numbers, SSNs, credit card numbers):

```bash
edgeparse document.pdf -f markdown --sanitize -o output/
```

### Invalid character replacement

Replace characters that cannot be decoded with a custom string (default: space):

```bash
edgeparse document.pdf -f text --replace-invalid-chars "?" -o output/
```

---

## 12. Page Separators

Insert a custom string between pages in multi-page documents:

```bash
# Markdown separator
edgeparse document.pdf -f markdown \
  --markdown-page-separator $'\n---\n' \
  -o output/

# Text separator
edgeparse document.pdf -f text \
  --text-page-separator $'\n=====\n' \
  -o output/

# HTML separator
edgeparse document.pdf -f html \
  --html-page-separator '<hr class="page-break">' \
  -o output/
```

---

## 13. Complete Flag Reference

```
edgeparse [OPTIONS] <INPUT>...

Arguments:
  <INPUT>...   One or more PDF file paths (required)

Core options:
  -o, --output-dir <DIR>     Write output files here (default: input file directory)
  -f, --format <FMT>         Output formats, comma-separated:
                               json | text | html | markdown | markdown-with-html
                               | markdown-with-images  (default: json)
  -p, --password <PW>        Password for encrypted PDFs
      --pages <RANGE>        Page range, e.g. "1,3,5-7"
  -q, --quiet                Suppress log output

Layout options:
      --reading-order <ALG>  xycut (default) | off
      --table-method <M>     default (ruling lines) | cluster (borderless)
      --keep-line-breaks     Preserve original line breaks
      --use-struct-tree      Use tagged PDF structure tree
      --include-header-footer  Include headers/footers
      --sanitize             Enable PII sanitisation
      --replace-invalid-chars <CH>  Replace invalid chars (default: space)

Image options:
      --image-output <MODE>  off | external (default) | embedded
      --image-format <FMT>   png (default) | jpeg
      --image-dir <DIR>      Directory for extracted image files

Page separator options:
      --markdown-page-separator <STR>
      --text-page-separator <STR>
      --html-page-separator <STR>

Safety options:
      --content-safety-off <FLAGS>   all | hidden-text | off-page | tiny | hidden-ocg

Hybrid backend options (advanced):
      --hybrid <BACKEND>     off (default) | docling-fast
      --hybrid-mode <MODE>   auto (default) | full
      --hybrid-url <URL>     Hybrid backend service URL
      --hybrid-timeout <MS>  Timeout in milliseconds (default: 30000)
      --hybrid-fallback      Fall back to local extraction on error

Standard:
  -V, --version              Print version
  -h, --help                 Print help
```

---

## Quick Cheat Sheet

```bash
# Markdown to stdout
edgeparse doc.pdf -f markdown

# JSON with bounding boxes
edgeparse doc.pdf -f json -o out/

# Multiple formats, pages 1-5
edgeparse doc.pdf -f markdown,json --pages "1-5" -o out/

# Borderless-table PDF
edgeparse doc.pdf -f markdown --table-method cluster -o out/

# Extract images to files
edgeparse doc.pdf -f markdown --image-output external --image-dir out/images/ -o out/

# Encrypted PDF
edgeparse secure.pdf -f markdown -p "password" -o out/

# Batch all PDFs in a folder
edgeparse pdfs/*.pdf -f markdown -o out/ -q
```

---

→ **Continue:** [Python SDK Tutorial](02-python-sdk.md)
