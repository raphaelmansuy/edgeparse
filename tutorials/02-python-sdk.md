# Tutorial 02 — Python SDK

**Goal:** Install `edgeparse`, convert PDFs to every format, work with the returned data, and handle errors correctly.

→ **Previous:** [CLI](01-cli.md) · **Next:** [Node.js SDK](03-nodejs-sdk.md) · [Rust library](04-rust-library.md)

---

## Table of Contents

1. [Installation](#1-installation)
2. [Quick start](#2-quick-start)
3. [Convert to every format](#3-convert-to-every-format)
4. [Write output to a file](#4-write-output-to-a-file)
5. [Page ranges](#5-page-ranges)
6. [Table detection methods](#6-table-detection-methods)
7. [Image extraction](#7-image-extraction)
8. [Encrypted PDFs](#8-encrypted-pdfs)
9. [Batch processing](#9-batch-processing)
10. [Parse the JSON output](#10-parse-the-json-output)
11. [Error handling](#11-error-handling)
12. [Using the CLI entry point](#12-using-the-cli-entry-point)
13. [Build from source](#13-build-from-source)
14. [API reference](#14-api-reference)

---

## 1. Installation

```bash
pip install edgeparse
```

Requires Python 3.9+. Pre-built wheels are available for:
- macOS arm64 (Apple Silicon) and x64 (Intel)
- Linux x64 (glibc 2.31+) and arm64
- Windows x64

Verify:

```python
import edgeparse
print(edgeparse.version())   # "0.1.0"
print(edgeparse.__version__) # "0.1.0"
```

---

## 2. Quick Start

```python
import edgeparse

# Convert to Markdown — returns a string
markdown = edgeparse.convert("examples/pdf/lorem.pdf", format="markdown")
print(markdown[:200])
```

Output:

```
# Lorem

# Ipsum

Lorem ipsum dolor sit amet, incididunt ut labore et dolore exercitation ullamco
laboris dolor in reprehenderit ...
```

`convert()` is pure — it returns the extracted content as a Python string without touching the filesystem.

---

## 3. Convert to Every Format

### Markdown

```python
md = edgeparse.convert("report.pdf", format="markdown")
```

Returns standard Markdown with GFM tables, headings detected by font size, and lists.

### JSON with bounding boxes

```python
import json

raw = edgeparse.convert("report.pdf", format="json")
doc = json.loads(raw)

print(doc["file name"])         # "report.pdf"
print(doc["number of pages"])   # integer
for element in doc["kids"]:
    print(element["type"], element["content"][:40])
```

Each element has: `type`, `id`, `page number`, `bounding box` (4-float list: x0, y0, x1, y1), `font`, `font size`, `text color`, `content`.

See [Tutorial 05 — Output Formats](05-output-formats.md#json) for the full schema.

### HTML

```python
html = edgeparse.convert("report.pdf", format="html")
# Returns a complete <!DOCTYPE html> document
```

### Plain text

```python
text = edgeparse.convert("report.pdf", format="text")
# UTF-8 text preserving reading order, no markup
```

---

## 4. Write Output to a File

`convert_file()` writes the output directly to a directory and returns the output file path:

```python
import edgeparse

# Writes output/report.md, returns "output/report.md"
out_path = edgeparse.convert_file(
    "report.pdf",
    output_dir="output/",
    format="markdown",
)
print(f"Written to: {out_path}")
```

The output directory is created automatically if it does not exist.

```python
from pathlib import Path

# Works with Path objects too
out_path = edgeparse.convert_file(
    Path("examples/pdf/lorem.pdf"),
    output_dir=Path("output"),
    format="json",
)
print(Path(out_path).read_text()[:100])
```

---

## 5. Page Ranges

Extract a subset of pages with the `pages` parameter:

```python
# Pages 1 and 2
md = edgeparse.convert("paper.pdf", format="markdown", pages="1-2")

# Pages 1, 3, and 5 through 7
md = edgeparse.convert("paper.pdf", format="markdown", pages="1,3,5-7")

# Just the first page
md = edgeparse.convert("paper.pdf", format="markdown", pages="1")
```

Pages are 1-indexed. Out-of-range page numbers are silently ignored.

---

## 6. Table Detection Methods

```python
# Default: ruling-line detection (best for PDFs with visible table borders)
md = edgeparse.convert("report.pdf", format="markdown", table_method="default")

# Cluster: geometric detection (best for borderless/whitespace tables)
md = edgeparse.convert("report.pdf", format="markdown", table_method="cluster")
```

Use `"cluster"` for spreadsheet exports, Word-to-PDF conversions, or any document where tables lack visible lines.

---

## 7. Image Extraction

```python
# Off (default) — no image data in output
md = edgeparse.convert("doc.pdf", format="markdown", image_output="off")

# Embedded — base64 data URIs inline in the output string
md = edgeparse.convert("doc.pdf", format="markdown", image_output="embedded")

# External — images saved to files; use convert_file() for a path-based workflow
out_path = edgeparse.convert_file(
    "doc.pdf",
    output_dir="output/",
    format="markdown",
    # image_output is not yet a parameter of convert_file; use convert() for that
)
```

> Note: `image_output` is only available on `edgeparse.convert()`. Use `convert_file()` when the output format is enough; it calls `convert()` internally with defaults.

---

## 8. Encrypted PDFs

```python
md = edgeparse.convert(
    "secure.pdf",
    format="markdown",
    password="my-secret-password",
)
```

---

## 9. Batch Processing

### Sequential

```python
import edgeparse
from pathlib import Path

pdf_dir = Path("pdfs/")
out_dir = Path("output/")

for pdf in sorted(pdf_dir.glob("*.pdf")):
    out_path = edgeparse.convert_file(pdf, out_dir, format="markdown")
    print(f"✓ {pdf.name} → {out_path}")
```

### Parallel with `concurrent.futures`

```python
import edgeparse
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

pdf_files = list(Path("pdfs/").glob("*.pdf"))
out_dir = Path("output/")

def process(pdf: Path) -> tuple[Path, str]:
    out = edgeparse.convert_file(pdf, out_dir, format="markdown")
    return pdf, out

with ThreadPoolExecutor(max_workers=8) as pool:
    futures = {pool.submit(process, p): p for p in pdf_files}
    for fut in as_completed(futures):
        pdf, out = fut.result()
        print(f"✓ {pdf.name} → {out}")
```

> EdgeParse's Rust engine uses Rayon internally for per-page parallelism; the Python GIL is released during native calls, so thread-based parallelism works well here.

### Aggregate all results into a list

```python
import edgeparse, json
from pathlib import Path

results = []
for pdf in Path("pdfs/").glob("*.pdf"):
    raw = edgeparse.convert(pdf, format="json")
    doc = json.loads(raw)
    results.append({
        "file": pdf.name,
        "pages": doc["number of pages"],
        "elements": len(doc["kids"]),
    })

for r in results:
    print(f'{r["file"]}: {r["pages"]} pages, {r["elements"]} elements')
```

---

## 10. Parse the JSON Output

The JSON format returns rich structured data you can analyse, embed in a vector store, or pass to an LLM:

```python
import edgeparse, json

raw = edgeparse.convert("examples/pdf/1901.03003.pdf", format="json")
doc = json.loads(raw)

# --- Document metadata ---
print("Title:", doc.get("title") or "(none)")
print("Author:", doc.get("author"))
print("Pages:", doc["number of pages"])

# --- Extract all headings ---
headings = [e for e in doc["kids"] if e["type"] == "heading"]
for h in headings[:5]:
    print(f'  H{h.get("level", "?")} — {h["content"]}')

# --- Extract all table cells ---
tables = [e for e in doc["kids"] if e["type"] == "table"]
print(f"\n{len(tables)} table(s) found")

# --- Filter by page ---
page1 = [e for e in doc["kids"] if e["page number"] == 1]
print(f"\n{len(page1)} elements on page 1")

# --- Sort by bounding box (left-to-right, top-to-bottom) ---
page1_sorted = sorted(
    page1,
    key=lambda e: (round(e["bounding box"][1], 1), e["bounding box"][0]),
    reverse=True,  # PDF coordinates start at bottom-left
)
for e in page1_sorted[:10]:
    print(f'  [{e["bounding box"][1]:.0f}] {e["type"]}: {e["content"][:50]}')
```

---

## 11. Error Handling

```python
import edgeparse

# File not found
try:
    edgeparse.convert("/nonexistent.pdf")
except RuntimeError as e:
    print(f"File error: {e}")  # "File not found: /nonexistent.pdf"

# Invalid format
try:
    edgeparse.convert("doc.pdf", format="pdf2")
except RuntimeError as e:
    print(f"Format error: {e}")  # "Unknown format: pdf2..."

# Encrypted PDF, wrong password
try:
    edgeparse.convert("secure.pdf", password="wrong")
except RuntimeError as e:
    print(f"Password error: {e}")
```

All errors are raised as `RuntimeError` with a descriptive message.

---

## 12. Using the CLI Entry Point

The Python package installs an `edgeparse` CLI command:

```bash
# Convert to Markdown
edgeparse examples/pdf/lorem.pdf -f markdown -o output/

# Convert multiple files
edgeparse examples/pdf/*.pdf --format json --output-dir out/

# Page range
edgeparse paper.pdf -f markdown --pages "1-3" -o out/
```

The Python CLI accepts the same core flags as the Rust binary. See [Tutorial 01 — CLI](01-cli.md) for the full flag list.

> **Performance tip:** For large batches, prefer `cargo install edgeparse-cli` over the Python entry point. The Rust binary processes files directly without the Python import overhead.

---

## 13. Build from Source

If a pre-built wheel is not available for your platform:

```bash
git clone https://github.com/raphaelmansuy/edgeparse.git
cd edgeparse/sdks/python

# Create and activate a virtual environment
python3 -m venv .venv
source .venv/bin/activate            # Windows: .venv\Scripts\activate

# Install maturin and build
pip install maturin
maturin develop --release
```

Test the build:

```python
import edgeparse
print(edgeparse.version())   # "0.1.0"
```

---

## 14. API Reference

### `edgeparse.convert()`

```python
def convert(
    input_path: str | Path,
    *,
    format: str = "markdown",       # "markdown" | "json" | "html" | "text"
    pages: str | None = None,       # e.g. "1,3,5-7"
    password: str | None = None,
    reading_order: str = "xycut",   # "xycut" | "off"
    table_method: str = "default",  # "default" | "cluster"
    image_output: str = "off",      # "off" | "embedded" | "external"
) -> str: ...
```

Returns the extracted content as a string.

### `edgeparse.convert_file()`

```python
def convert_file(
    input_path: str | Path,
    output_dir: str | Path = "output",
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
) -> str: ...
```

Writes the output to `output_dir/{stem}.{ext}` and returns the absolute output path.

### `edgeparse.version()`

```python
def version() -> str: ...
```

Returns the version string, e.g. `"0.1.0"`.

---

→ **Continue:** [Node.js SDK Tutorial](03-nodejs-sdk.md)
