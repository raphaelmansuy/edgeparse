# Tutorial 05 — Output Formats Deep-Dive

**Goal:** Understand every edge of EdgeParse's four output formats — JSON schema, Markdown variants, HTML structure, and plain text conventions.

→ **Previous:** [Rust library](04-rust-library.md) · [Back to index](README.md)

---

## Table of Contents

1. [JSON](#1-json)
   - [Document envelope](#document-envelope)
   - [Element schema by type](#element-schema-by-type)
   - [Bounding box coordinates](#bounding-box-coordinates)
   - [Working with JSON in Python](#working-with-json-in-python)
   - [Working with JSON in Node.js](#working-with-json-in-nodejs)
2. [Markdown](#2-markdown)
   - [Standard Markdown](#standard-markdown)
   - [Markdown with HTML tables](#markdown-with-html-tables)
   - [Markdown with images](#markdown-with-images)
3. [HTML](#3-html)
4. [Plain Text](#4-plain-text)
5. [Choosing the right format](#5-choosing-the-right-format)
6. [Format comparison table](#6-format-comparison-table)

---

## 1. JSON

JSON output is produced by:

```bash
edgeparse document.pdf -f json -o output/
```

### Document Envelope

```json
{
  "file name": "document.pdf",
  "number of pages": 15,
  "author": "Canjie Luo",
  "title": "MORAN: A Multi-Object Rectified Attention Network",
  "creation date": "D:20190111014532Z",
  "modification date": "D:20190111014532Z",
  "kids": [ /* array of elements */ ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `file name` | string | Base filename of the input PDF |
| `number of pages` | integer | Total pages in the PDF |
| `author` | string \| null | PDF `/Author` metadata |
| `title` | string \| null | PDF `/Title` metadata |
| `creation date` | string \| null | PDF `/CreationDate` string |
| `modification date` | string \| null | PDF `/ModDate` string |
| `kids` | array | All extracted elements in reading order |

### Element Schema by Type

All elements share these base fields:

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Element type (see below) |
| `id` | integer | Unique element ID within the document |
| `page number` | integer | 1-based page number |
| `bounding box` | `[x0, y0, x1, y1]` | Coordinates in PDF points |

#### `paragraph`

```json
{
  "type": "paragraph",
  "id": 1,
  "page number": 1,
  "bounding box": [130.15, 654.14, 302.54, 684.97],
  "font": "NimbusRomNo9L-Medi",
  "font size": 14.35,
  "text color": "[0.0]",
  "content": "Lorem ipsum dolor sit amet"
}
```

Additional fields: `font`, `font size`, `text color`, `content`.

#### `heading`

```json
{
  "type": "heading",
  "id": 5,
  "level": "Title",
  "page number": 1,
  "bounding box": [145.99, 530.21, 190.48, 540.95],
  "heading level": 1,
  "content": "Abstract"
}
```

Additional fields: `level` (semantic label: `"Title"`, `"H1"` … `"H6"`), `heading level` (integer 1–6), `content`.

#### `table`

```json
{
  "type": "table",
  "id": 12,
  "page number": 3,
  "bounding box": [72.0, 400.0, 540.0, 600.0],
  "number of rows": 2,
  "rows": [
    {
      "type": "table row",
      "row number": 1,
      "cells": [
        { "type": "table cell", "row number": 1, "column number": 1, "row span": 1, "column span": 1, "kids": [] },
        { "type": "table cell", "row number": 1, "column number": 2, "row span": 1, "column span": 1, "kids": [] }
      ]
    }
  ]
}
```

Additional field: `rows` — array of row objects. Each row has `row number` and `cells` (array of cell objects with `row number`, `column number`, `row span`, `column span`, `kids`).

#### `image`

```json
{
  "type": "image",
  "id": 8,
  "page number": 2,
  "bounding box": [72.0, 300.0, 300.0, 500.0],
  "source": "output/document_images/imageFile1.png"
}
```

`source` is always present and contains the generated image path.

#### `list`

```json
{
  "type": "list",
  "id": 20,
  "page number": 4,
  "bounding box": [72.0, 200.0, 400.0, 280.0],
  "list items": [
    { "type": "list item", "content": "First item", "kids": [] },
    { "type": "list item", "content": "Second item", "kids": [] }
  ]
}
```

#### `caption`

```json
{
  "type": "caption",
  "id": 15,
  "page number": 3,
  "bounding box": [72.0, 390.0, 540.0, 400.0],
  "content": "Figure 1: Architecture overview"
}
```

### Complete type list

| `type` | Description |
|--------|-------------|
| `paragraph` | Body text block |
| `heading` | Section heading (H1–H6 or Title) |
| `table` | Table with `rows` array |
| `image` | Extracted image with coordinates |
| `list` | Bulleted or numbered list |
| `caption` | Figure/table caption |
| `formula` | Mathematical formula |
| `figure` | Figure bounding box |

### Bounding Box Coordinates

```
"bounding box" = [x0, y0, x1, y1]
```

- **Origin**: bottom-left corner of the page
- **Y axis**: increases upward
- **Units**: PDF points (72 points = 1 inch)
- **x0, y0**: lower-left corner of the element
- **x1, y1**: upper-right corner of the element

To convert to top-left origin (screen coordinates), given page height `H`:

```python
x0, y0_pdf, x1, y1_pdf = element["bounding box"]
y0_screen = H - y1_pdf   # top of element from top of page
y1_screen = H - y0_pdf   # bottom of element from top of page
```

A standard US Letter page is 612 × 792 points. An A4 page is 595 × 842 points.

### Working with JSON in Python

```python
import edgeparse, json
from pathlib import Path

raw = edgeparse.convert("examples/pdf/1901.03003.pdf", format="json")
doc = json.loads(raw)

# --- Extract all headings -----------------------------------------------
headings = [e for e in doc["kids"] if e["type"] == "heading"]
for h in headings[:5]:
    print(f'H{h.get("heading level", "?")} [{h["level"]}] {h["content"]}')

# --- Extract all table data ---------------------------------------------
for e in doc["kids"]:
    if e["type"] == "table":
        print(f'\nTable on page {e["page number"]}:')
        for row in e["rows"]:
            n_cells = len(row.get("cells", []))
            print(f'  Row {row["row number"]}: {n_cells} cell(s)')

# --- Get bounding boxes for all paragraphs on page 1 --------------------
page1_paras = [
    e for e in doc["kids"]
    if e["type"] == "paragraph" and e["page number"] == 1
]
for p in page1_paras:
    x0, y0, x1, y1 = p["bounding box"]
    print(f'  ({x0:.0f},{y0:.0f})-({x1:.0f},{y1:.0f}): {p["content"][:40]}')

# --- Chunk text for RAG pipeline ----------------------------------------
chunks = []
for e in doc["kids"]:
    if e["type"] in ("paragraph", "heading", "caption"):
        chunks.append({
            "text": e["content"],
            "page": e["page number"],
            "type": e["type"],
        })
print(f"\n{len(chunks)} text chunks ready for embedding")
```

### Working with JSON in Node.js

```js
const { convert } = require('edgeparse');

const raw = convert('examples/pdf/1901.03003.pdf', { format: 'json' });
const doc = JSON.parse(raw);

// --- Extract headings ---------------------------------------------------
const headings = doc.kids.filter(e => e.type === 'heading');
headings.slice(0, 5).forEach(h => {
  console.log(`H${h['heading level']} [${h.level}] ${h.content}`);
});

// --- Extract table rows -------------------------------------------------
const tables = doc.kids.filter(e => e.type === 'table');
tables.forEach(t => {
  console.log(`\nTable on page ${t['page number']}:`);
  t.rows.forEach(row => {
    const nCells = row.cells?.length ?? 0;
    console.log(`  Row ${row['row number']}: ${nCells} cell(s)`);
  });
});

// --- Prepare RAG chunks -------------------------------------------------
const chunks = doc.kids
  .filter(e => ['paragraph', 'heading', 'caption'].includes(e.type))
  .map(e => ({
    text: e.content,
    page: e['page number'],
    type: e.type,
  }));
console.log(`\n${chunks.length} chunks ready for embedding`);
```

---

## 2. Markdown

### Standard Markdown

```bash
edgeparse document.pdf -f markdown -o output/
```

Output follows GitHub-Flavored Markdown (GFM):

- **Headings**: `#` through `######` based on detected heading level
- **Paragraphs**: plain text with a blank line separator
- **Tables**: GFM pipe tables

  ```markdown
  | Method | Accuracy | Speed |
  |--------|----------|-------|
  | EdgeParse | 0.881 | 0.023 s |
  ```

- **Lists**: `-` for unordered, `1.` for ordered
- **Images**: `![alt](path/to/image.png)` links (when `--image-output external` is set)

Multi-page documents produce a blank line between pages by default. Use `--markdown-page-separator` to customise:

```bash
edgeparse document.pdf -f markdown --markdown-page-separator $'\n\n---\n\n' -o output/
```

### Markdown with HTML Tables

```bash
edgeparse document.pdf -f markdown-with-html -o output/
```

Complex tables that contain merged cells or nested content are rendered as HTML `<table>` blocks inside the Markdown. Standard tables remain as GFM pipes. Use this format when your Markdown renderer supports inline HTML.

### Markdown with Images

```bash
edgeparse document.pdf -f markdown-with-images --image-output external --image-dir output/images/ -o output/
```

Images are extracted and referenced as:

```markdown
![Figure 1](images/page2_img1.png)
```

With `--image-output embedded`:

```markdown
![Figure 1](data:image/png;base64,iVBORw0KGgo...)
```

Embedded images produce a self-contained Markdown file with no external dependencies.

---

## 3. HTML

```bash
edgeparse document.pdf -f html -o output/
```

Produces a complete, valid HTML5 document:

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>document.pdf</title>
</head>
<body>
<h1>Section Title</h1>
<p>Body text paragraph...</p>
<table>
  <tr><th>Column A</th><th>Column B</th></tr>
  <tr><td>Value 1</td><td>Value 2</td></tr>
</table>
<ul>
  <li>List item 1</li>
  <li>List item 2</li>
</ul>
</body>
</html>
```

Semantic elements used:
- `<h1>`–`<h6>` for headings
- `<p>` for paragraphs
- `<table>`, `<tr>`, `<th>`, `<td>` for tables
- `<ul>`/`<ol>` + `<li>` for lists
- `<img>` for images (path or data URI depending on `--image-output`)
- `<figure>`, `<figcaption>` for captioned figures

Multi-page documents can use a custom separator:

```bash
edgeparse document.pdf -f html \
  --html-page-separator '<hr class="page-break">' \
  -o output/
```

---

## 4. Plain Text

```bash
edgeparse document.pdf -f text -o output/
```

- UTF-8 text only — no markup, no coordinates
- Reading order preserved (XY-Cut++ by default)
- Paragraphs separated by a blank line
- Table cells joined with spaces or tabs based on column width

Use `--text-page-separator` for custom page breaks:

```bash
edgeparse document.pdf -f text \
  --text-page-separator $'\n\f\n' \  # form feed between pages
  -o output/
```

For line-break preservation (useful for poetry or fixed-format documents):

```bash
edgeparse document.pdf -f text --keep-line-breaks -o output/
```

---

## 5. Choosing the Right Format

| Use case | Recommended format | Reason |
|----------|-------------------|--------|
| RAG / LLM ingestion | `json` | Bounding boxes + page numbers for precise chunking |
| Markdown preview / blog | `markdown` | Clean, widely supported |
| Word/Doc conversion | `markdown-with-html` | Better table fidelity |
| Web display | `html` | Semantic structure, CSS-styleable |
| Search indexing / NLP | `text` | No markup interference |
| Document archival | `json` + `markdown` | Rich structure + human-readable |
| Self-contained report | `markdown-with-images` (embedded) | Single file, no dependencies |

---

## 6. Format Comparison Table

| | JSON | Markdown | HTML | Text |
|---|------|----------|------|------|
| Bounding boxes | ✅ | ❌ | ❌ | ❌ |
| Font information | ✅ | ❌ | ❌ | ❌ |
| Page numbers per element | ✅ | ❌ | ❌ | ❌ |
| Heading hierarchy | ✅ | ✅ | ✅ | ❌ |
| Table structure | ✅ rows array | ✅ GFM pipes | ✅ `<table>` | ≈ flattened |
| Image embeddings | optional | optional | optional | ❌ |
| Human readable | moderate | ✅ | moderate | ✅ |
| Parseable without schema | ✅ (JSON) | varies | varies | N/A |
| LLM context length cost | high | medium | high | low |

---

→ [Back to Tutorial Index](README.md)
