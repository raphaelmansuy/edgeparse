# EdgeParse API Reference

Complete Python and Node.js API for the EdgeParse skill.

---

## Python API

### Installation

```bash
pip install edgeparse
```

Requires Python 3.9+. Pre-built wheels for macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x86_64).

---

### `edgeparse.convert()`

Extract content from a PDF and return it as a string.

```python
def convert(
    input_path: str | Path,
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
    reading_order: str = "xycut",
    table_method: str = "default",
    image_output: str = "off",
    keep_line_breaks: bool = False,
    use_struct_tree: bool = False,
    include_header_footer: bool = False,
    sanitize: bool = False,
) -> str: ...
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `input_path` | `str \| Path` | required | Path to the PDF file |
| `format` | `str` | `"markdown"` | Output format: `"markdown"`, `"markdown-with-html"`, `"markdown-with-images"`, `"json"`, `"html"`, `"text"` |
| `pages` | `str \| None` | `None` | Page range, e.g. `"1-5"` or `"1,3,7-10"`. `None` = all pages |
| `password` | `str \| None` | `None` | Password for encrypted PDFs |
| `reading_order` | `str` | `"xycut"` | Reading order algorithm: `"xycut"` (spatial XY-Cut++) or `"off"` |
| `table_method` | `str` | `"default"` | Table detection: `"default"` (ruling-line) or `"cluster"` (borderless) |
| `image_output` | `str` | `"off"` | Image handling: `"off"`, `"embedded"` (base64 in output), `"external"` (write files) |
| `keep_line_breaks` | `bool` | `False` | Preserve original line breaks within paragraphs |
| `use_struct_tree` | `bool` | `False` | Use tagged PDF structure tree when available |
| `include_header_footer` | `bool` | `False` | Include page headers and footers |
| `sanitize` | `bool` | `False` | Enable PII sanitization |

**Returns:** `str` — extracted content in the requested format.

**Raises:**
- `FileNotFoundError` — PDF file not found at `input_path`
- `ValueError` — Invalid format, corrupt/unreadable PDF, wrong password, bad page range

---

### `edgeparse.convert_file()`

Extract content from a PDF and write the output to a file.

```python
def convert_file(
    input_path: str | Path,
    output_dir: str | Path = "output",
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
    reading_order: str = "xycut",
    table_method: str = "default",
    image_output: str = "off",
) -> str: ...
```

**Parameters:** same as `convert()` plus `output_dir`.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `output_dir` | `str \| Path` | `"output"` | Directory to write the output file |

**Returns:** `str` — path to the written output file.

---

### Output format reference

#### `format="markdown"` (recommended for LLMs)

| Element | Markdown rendering |
|---------|-------------------|
| H1–H6 headings | `# Heading`, `## Heading`, ... |
| Paragraph | plain text |
| Table with ruling lines | GFM table `\| col \| col \|` |
| Table borderless | GFM table (with `table_method="cluster"`) |
| Bullet list | `- item` |
| Numbered list | `1. item` |
| Figure/image | `![alt](path)` (requires `image_output` ≠ `"off"`) |
| Header/footer | omitted (unless `include_header_footer=True`) |

#### `format="json"` (for structured workflows)

Returns a JSON string. Parse with `json.loads()`.

```json
{
  "page_count": 10,
  "title": "Document Title",
  "elements": [
    {
      "type": "heading",
      "level": 1,
      "text": "Introduction",
      "page_number": 1,
      "reading_order": 0,
      "bounding_box": { "x0": 72.0, "y0": 144.0, "x1": 540.0, "y1": 180.0 }
    },
    {
      "type": "table",
      "text": "| Col A | Col B |\n|-------|-------|\n| val1  | val2  |",
      "page_number": 2,
      "reading_order": 5,
      "bounding_box": { "x0": 72.0, "y0": 200.0, "x1": 540.0, "y1": 350.0 },
      "rows": [
        { "cells": [{ "text": "Col A", "rowspan": 1, "colspan": 1 }, { "text": "Col B", "rowspan": 1, "colspan": 1 }] }
      ]
    },
    {
      "type": "paragraph",
      "text": "This is body text.",
      "page_number": 1,
      "reading_order": 2,
      "bounding_box": { "x0": 72.0, "y0": 190.0, "x1": 540.0, "y1": 220.0 }
    }
  ]
}
```

**Element types:** `heading`, `paragraph`, `table`, `list`, `list_item`, `figure`, `caption`, `header`, `footer`

**Bounding box** coordinates are in PDF points from the bottom-left of the page.

---

## Node.js API

### Installation

```bash
npm install edgeparse
```

Requires Node.js 18+.

---

### `convert()`

```ts
import { convert } from 'edgeparse';

function convert(inputPath: string, options?: ConvertOptions): string
```

**ConvertOptions:**

```ts
interface ConvertOptions {
  format?:        string;   // "markdown" | "json" | "html" | "text"  (default: "markdown")
  pages?:         string;   // e.g. "1-5" or "1,3,7-10"
  password?:      string;
  readingOrder?:  string;   // "xycut" | "off"                         (default: "xycut")
  tableMethod?:   string;   // "default" | "cluster"                   (default: "default")
  imageOutput?:   string;   // "off" | "embedded" | "external"         (default: "off")
}
```

**Returns:** `string` — extracted content.  
**Throws:** `Error` on file not found, corrupt PDF, or invalid options.

---

## CLI reference

```bash
edgeparse [OPTIONS] <PDF_FILE>...

# Core flags
-f, --format <FMT>        markdown | json | html | text   (default: json)
-o, --output-dir <DIR>    Write output files here
-p, --password <PW>       Password for encrypted PDFs
    --pages <RANGE>       e.g. "1-5" or "1,3,7-10"
-q, --quiet               Suppress log output

# Table and layout
    --table-method <M>    default | cluster
    --reading-order <A>   xycut | off
    --keep-line-breaks    Preserve original line breaks
    --use-struct-tree     Use tagged PDF structure tree
    --include-header-footer

# Image options
    --image-output <M>    off | embedded | external
    --image-format <F>    png | jpeg
    --image-dir <DIR>     Directory for extracted images
```

**Examples:**

```bash
# Markdown to stdout
edgeparse report.pdf --format markdown

# JSON to file
edgeparse report.pdf --format json --output-dir ./output

# Batch
edgeparse docs/*.pdf --format markdown --output-dir ./output

# Password + page range
edgeparse secure.pdf --format markdown --password mypass --pages 1-5
```
