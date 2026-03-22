# EdgeParse — Output Formats

> All renderers take a fully-classified `PdfDocument` (post-pipeline) and return `Result<String, EdgePdfError>`.

---

## 01 · Renderer Map

```
PdfDocument
    │
    ├── output::legacy_json::to_legacy_json_string()  → .json   [legacy_json.rs]
    ├── output::json::to_json()                        → .json   [json.rs]
    ├── output::markdown::to_markdown()               → .md    [markdown.rs]
    ├── output::html::to_html()                       → .html  [html.rs]
    ├── output::text::to_text()                       → .txt   [text.rs]
    └── output::csv::to_csv()                         → .csv   [csv.rs]
```

All live in [`crates/edgeparse-core/src/output/`](../crates/edgeparse-core/src/output/).

The CLI and SDKs select a renderer via `OutputFormat` enum after converting the document at [`edgeparse-cli/src/main.rs#L143`](../crates/edgeparse-cli/src/main.rs#L143).

---

## 02 · Legacy JSON (`legacy_json.rs`)

**Source:** [`output/legacy_json.rs`](../crates/edgeparse-core/src/output/legacy_json.rs)
**Function:** `to_legacy_json_string(doc: &PdfDocument, stem: &str) → Result<String>`

This is the **default output format** (used when `--format json` or no format specified).

### Key Schema Characteristics

| Feature | Detail |
|---------|--------|
| Key style | Space-separated: `"file name"`, `"page number"`, `"bounding box"` |
| BoundingBox | `[left_x, bottom_y, right_x, top_y]` float array |
| IDs | Globally sequential integers starting from 1 |
| Color | `"[r, g, b]"` or `"[k]"` string (preserves original color space) |
| Font names | Subset prefix stripped: `"ABCDEF+Helvetica"` → `"Helvetica"` |

### Document-Level Fields

```json
{
  "file name": "report.pdf",
  "number of pages": 10,
  "title": "Annual Report",
  "author": "Alice Smith",
  "creation date": "D:20240101",
  "modification date": "D:20240201",
  "elements": [ ... ]
}
```

### Element Schema

Every element has:
```json
{
  "id": 42,
  "type": "paragraph",
  "bounding box": [72.0, 680.0, 540.0, 700.0],
  "page number": 1
}
```

Type-specific fields:

**Heading / paragraph:**
```json
{
  "type": "heading",
  "level": "h1",
  "value": "Introduction",
  "font name": "Helvetica-Bold",
  "font size": "14.0",
  "font weight": "700.0",
  "text color": "[0.0, 0.0, 0.0]"
}
```

**Table:**
```json
{
  "type": "table",
  "rows": [
    {
      "type": "table row",
      "cells": [
        { "type": "table header cell", "value": "Name" },
        { "type": "table data cell",   "value": "Alice" }
      ]
    }
  ]
}
```

**List:**
```json
{
  "type": "list",
  "list items": [
    { "type": "list item", "label value": "•", "body value": "First item" }
  ]
}
```

### ID Counter

The legacy JSON serialiser uses a thread-local counter (`NEXT_ID`) reset at the start of each document:
```rust
thread_local! {
    static NEXT_ID: Cell<u64> = Cell::new(1);
}
```
**Source:** [`legacy_json.rs#L31`](../crates/edgeparse-core/src/output/legacy_json.rs#L31)

---

## 03 · Markdown (`markdown.rs`)

**Source:** [`output/markdown.rs`](../crates/edgeparse-core/src/output/markdown.rs)
**Function:** `to_markdown(doc: &PdfDocument) → Result<String>`

### Element Mapping

| ContentElement | Markdown output |
|---------------|-----------------|
| `Heading{level=1}` | `# Heading text` |
| `Heading{level=2}` | `## Heading text` |
| `NumberHeading{level=2}` | `## 1.2 Heading text` |
| `Paragraph` | `Paragraph text\n\n` |
| `List` | `- item 1\n- item 2\n` |
| `Table` (with borders) | GFM table: `\| col1 \| col2 \|` |
| `Figure` | `![Image]\n` |
| `Caption` | Appended after figure/table |
| `HeaderFooter` | Skipped (unless `include_header_footer=true`) |
| `TextBlock` | Treated as paragraph (fallback) |

### Special-Case Document Detection

The markdown renderer has two special-case document detectors that produce cleaner output for specific document types:

```rust
if looks_like_contents_document(doc) {
    return Ok(render_contents_document(doc));
}
if looks_like_compact_toc_document(doc) {
    return Ok(render_compact_toc_document(doc));
}
```

These detect documents that are primarily a Table of Contents and render them without heading noise.

### Heading Demotion

When a heading is very long or followed immediately by a paragraph that semantically "continues" it, the renderer promotes the heading to a plain paragraph to avoid spurious `#` markers:

```rust
if should_demote_heading_to_paragraph(trimmed, &next_text) {
    // merge heading + next paragraph as plain text
}
```

### Markdown-Start Escaping

Lines beginning with `-`, `*`, `#`, `>`, etc. that are not intended as Markdown syntax are escaped:
```rust
escape_md_line_start(text) → adds U+200B zero-width space prefix
```

---

## 04 · HTML5 (`html.rs`)

**Source:** [`output/html.rs`](../crates/edgeparse-core/src/output/html.rs)
**Function:** `to_html(doc: &PdfDocument) → Result<String>`

### Document Template

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{doc.title or doc.file_name}</title>
</head>
<body>
  {elements}
</body>
</html>
```

### Element Mapping

| ContentElement | HTML tag |
|---------------|----------|
| `Heading{level=N}` | `<hN>text</hN>` |
| `Paragraph` | `<p>text</p>` |
| `List` | `<ul><li>…</li></ul>` |
| `Image` | `<img src="image" alt="Image">` |
| `TextBlock` | `<p>text</p>` (fallback) |
| `TextLine` | `<span>text</span>` (fallback) |
| `TextChunk` | bare text node |
| `HeaderFooter` | skipped by default |

HTML special characters are escaped via `html_escape()`:
```
& → &amp;
< → &lt;
> → &gt;
```

### Table HTML

Tables use full semantic markup:
```html
<table>
  <thead><tr><th>Col A</th><th>Col B</th></tr></thead>
  <tbody>
    <tr><td>val1</td><td>val2</td></tr>
  </tbody>
</table>
```

---

## 05 · Plain Text (`text.rs`)

**Source:** [`output/text.rs`](../crates/edgeparse-core/src/output/text.rs)
**Function:** `to_text(doc: &PdfDocument) → Result<String>`

Simplest renderer. Text separated by `\n\n` between elements.

### Element Mapping

| ContentElement | Text output |
|---------------|------------|
| `Heading` | `text\n\n` |
| `Paragraph` | `text\n\n` |
| `List` / item | `  label body\n` |
| `Image` | `[Image]\n\n` |
| `TextBlock` | `text\n\n` |
| `HeaderFooter` | skipped |

### Page Separator

If `config.text_page_separator` is set, it is inserted between pages when flattening to text.

---

## 06 · CSV (`csv.rs`)

**Source:** [`output/csv.rs`](../crates/edgeparse-core/src/output/csv.rs)

Extracts all tables from the document and renders them as CSV. Each table becomes a separate section. Non-table elements are skipped.

---

## 07 · TOC Builder (`toc_builder.rs`)

**Source:** [`output/toc_builder.rs`](../crates/edgeparse-core/src/output/toc_builder.rs)

Helper used by markdown and HTML renderers to extract a table of contents from `Heading` elements:

```
toc_builder::build_toc(doc) → Vec<TocEntry>

TocEntry {
    level:   u32
    text:    String
    anchor:  String   // slug of heading text
}
```

---

## 08 · Output Format Selection Flow

```
CLI: --format json,markdown
           │
           ▼
    build_config() → ProcessingConfig.formats: Vec<OutputFormat>
           │
           ▼
    edgeparse_core::convert() → PdfDocument
           │
           ▼
    write_outputs():
      for fmt in config.formats:
        match fmt:
          OutputFormat::Json     → legacy_json::to_legacy_json_string()  → .json
          OutputFormat::Text     → text::to_text()                        → .txt
          OutputFormat::Html     → html::to_html()                        → .html
          OutputFormat::Markdown → markdown::to_markdown()                → .md
          OutputFormat::Pdf      → log::warn! (not yet implemented)
```

**Source:** [`edgeparse-cli/src/main.rs`](../crates/edgeparse-cli/src/main.rs#L143)

---

## 09 · Format Comparison Matrix

| Feature | JSON | Markdown | HTML | Text |
|---------|------|----------|------|------|
| Structured data | ✅ full schema | ✗ | ✗ | ✗ |
| Human readable | ⚠️ verbose | ✅ | ✅ | ✅ |
| Table support | ✅ full cells | ✅ GFM | ✅ semantic | ⚠️ flat |
| Heading levels | ✅ field | ✅ `#` prefix | ✅ `<h1>` | ✗ (flat) |
| Bounding boxes | ✅ | ✗ | ✗ | ✗ |
| Font info | ✅ | ✗ | ✗ | ✗ |
| Page numbers | ✅ | ✗ (optional sep) | ✗ | ✗ |
| Image data | ✅ (base64 if embedded) | ✅ `![]()` | ✅ `<img>` | `[Image]` |
| Cross-page tables | ✅ linked | ✅ rendered | ✅ rendered | ✅ flat |

---

## Cross-Reference

| Topic | Document |
|-------|---------|
| How doc.kids is populated | [02-pipeline.md](02-pipeline.md) |
| ContentElement types | [03-data-model.md](03-data-model.md) |
| CLI format flag | [06-sdk-integration.md](06-sdk-integration.md) |
