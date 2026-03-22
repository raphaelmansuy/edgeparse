# 08 — Output Formats

> **Cross-references**: [05-data-models](05-data-models.md) | [06-cli-interface](06-cli-interface.md) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md)

---

## 1. JSON Output

### 1.1 Document Envelope

```json
{
  "file name": "document.pdf",
  "number of pages": 12,
  "author": "John Doe",
  "title": "Annual Report",
  "creation date": "2024-01-15T10:30:00Z",
  "modification date": "2024-06-20T14:00:00Z",
  "kids": [
    { ... content elements ... }
  ]
}
```

All pages' elements are flattened into a single `"kids"` array. `LineArtChunk` objects are always excluded.

### 1.2 JSON Field Names

All field names use **space-separated lowercase**:

| Internal Name | JSON Field |
|---------------|------------|
| pageNumber | `"page number"` (1-indexed) |
| boundingBox | `"bounding box"` → `[leftX, bottomY, rightX, topY]` |
| fontSize | `"font size"` |
| fontName | `"font"` |
| fontWeight | — (not serialized) |
| fontColor | `"text color"` → string `"[r, g, b]"` |
| textFormat | — (not serialized) |
| value | `"content"` |
| isHiddenText | `"hidden text"` (only if true) |
| headingLevel | `"heading level"` |
| linkedContentId | `"linked content id"` |
| numRows | `"number of rows"` |
| numColumns | `"number of columns"` |
| rowNumber | `"row number"` (1-indexed) |
| colNumber | `"column number"` (1-indexed) |
| rowSpan | `"row span"` |
| colSpan | `"column span"` |
| numberingStyle | `"numbering style"` |
| numListItems | `"number of list items"` |
| previousTableId | `"previous table id"` |
| nextTableId | `"next table id"` |
| previousListId | `"previous list id"` |
| nextListId | `"next list id"` |
| recognizedStructureId | `"id"` (only if non-null, non-zero) |
| level | `"level"` (only if non-null) |

### 1.3 Element Type Strings

| Internal Type | `"type"` Value |
|---------------|----------------|
| SemanticParagraph | `"paragraph"` |
| SemanticHeading | `"heading"` |
| SemanticCaption | `"caption"` |
| SemanticHeaderOrFooter | `"header"` or `"footer"` |
| TableBorder | `"table"` or `"text block"` (if isTextBlock) |
| TableBorderRow | `"table row"` |
| TableBorderCell | `"table cell"` |
| PDFList | `"list"` |
| ListItem | `"list item"` |
| ImageChunk | `"image"` |
| SemanticPicture | `"image"` + `"description"` |
| SemanticFormula | `"formula"` |
| TextChunk | `"text chunk"` |
| TextLine | `"text chunk"` |
| LineChunk | `"line"` |

### 1.4 Common Element Structure

Every element includes:
```json
{
  "type": "paragraph",
  "id": 42,
  "level": "1",
  "page number": 3,
  "bounding box": [72.0, 680.5, 540.0, 720.3]
}
```

- `"id"`: only present if non-null and non-zero
- `"level"`: only present if non-null

### 1.5 Text Element Structure

```json
{
  "type": "paragraph",
  "page number": 1,
  "bounding box": [72.0, 700.0, 540.0, 720.0],
  "font": "Times-Roman",
  "font size": 12.0,
  "text color": "[0.0, 0.0, 0.0]",
  "content": "This is paragraph text.",
  "hidden text": false
}
```

### 1.6 Heading Structure

```json
{
  "type": "heading",
  "page number": 1,
  "bounding box": [72.0, 750.0, 400.0, 770.0],
  "font": "Helvetica-Bold",
  "font size": 18.0,
  "text color": "[0.0, 0.0, 0.0]",
  "content": "Chapter 1: Introduction",
  "heading level": 1
}
```

### 1.7 Table Structure

```json
{
  "type": "table",
  "page number": 2,
  "bounding box": [72.0, 400.0, 540.0, 600.0],
  "number of rows": 3,
  "number of columns": 4,
  "previous table id": null,
  "next table id": 55,
  "rows": [
    {
      "type": "table row",
      "row number": 1,
      "cells": [
        {
          "type": "table cell",
          "page number": 2,
          "bounding box": [72.0, 560.0, 200.0, 600.0],
          "row number": 1,
          "column number": 1,
          "row span": 1,
          "column span": 2,
          "kids": [
            { "type": "paragraph", "content": "Header Cell", ... }
          ]
        }
      ]
    }
  ]
}
```

When `isTextBlock()` is true (table degenerates to a text container):
```json
{
  "type": "text block",
  "page number": 2,
  "bounding box": [...],
  "kids": [ ... ]
}
```

### 1.8 List Structure

```json
{
  "type": "list",
  "page number": 3,
  "bounding box": [72.0, 300.0, 540.0, 500.0],
  "numbering style": "arabic",
  "number of list items": 5,
  "previous list id": null,
  "next list id": 78,
  "list items": [
    {
      "type": "list item",
      "page number": 3,
      "bounding box": [90.0, 460.0, 540.0, 500.0],
      "font": "Times-Roman",
      "font size": 12.0,
      "text color": "[0.0, 0.0, 0.0]",
      "content": "1. First item with full text",
      "kids": [ ... ]
    }
  ]
}
```

### 1.9 Image Structure

**External images**:
```json
{
  "type": "image",
  "page number": 4,
  "bounding box": [100.0, 200.0, 400.0, 500.0],
  "source": "document_images/imageFile1.png"
}
```

**Embedded images** (base64):
```json
{
  "type": "image",
  "page number": 4,
  "bounding box": [100.0, 200.0, 400.0, 500.0],
  "data": "data:image/png;base64,iVBORw0KGgo...",
  "format": "png"
}
```

Max embedded image size: **10 MB**. Larger images silently excluded.

### 1.10 Formula Structure

```json
{
  "type": "formula",
  "page number": 5,
  "bounding box": [72.0, 100.0, 540.0, 130.0],
  "content": "E = mc^2"
}
```

### 1.11 Numeric Precision

All `double` values are serialized with **3 decimal places** via custom `DoubleSerializer`.

---

## 2. Markdown Output

### 2.1 Element Rendering Rules

| Element | Markdown |
|---------|----------|
| Heading (level N) | `#` repeated N times (max 6) + space + text |
| Paragraph | Plain text |
| Table (pipe) | Pipe-delimited rows with header separator |
| Table (HTML mode) | `<table>` with `<th>`/`<td>`, colspan/rowspan |
| List | `- ` prefix per item |
| Image | `![image N](path)` |
| Picture | `![image N](path)` + optional `\n\n*description*` |
| Formula | `$$\nlatex\n$$` |
| Header/Footer | Contents rendered (only if `include_header_footer`) |
| Caption | Plain text |

### 2.2 Table Rendering (Pipe Mode)

```markdown
|Header 1|Header 2|Header 3|
|---|---|---|
|Cell A|Cell B|Cell C|
|Cell D|Cell E|Cell F|
```

- No colspan/rowspan support in pipe mode
- Empty cells rendered as single space
- Cell line breaks: `<br>` if `keep_line_breaks`, else space

### 2.3 Table Rendering (HTML Mode)

```html
<table>
<tr><th colspan="2">Header</th><th>Col 3</th></tr>
<tr><td>A</td><td rowspan="2">B</td><td>C</td></tr>
<tr><td>D</td><td>E</td></tr>
</table>
```

### 2.4 Special Handling

- **Inside tables**: Headings lose `#` prefix (plain text), line breaks become `<br>` or space
- **Null characters**: `\u0000` → space
- **Content separator**: `\n\n` between top-level elements
- **Page separator**: Configurable, supports `{page_number}` placeholder (1-indexed)

### 2.5 Image with Description (SemanticPicture)

```markdown
![image 3](document_images/imageFile3.png)

*AI-generated description of the image content*

```

---

## 3. HTML Output

### 3.1 Document Wrapper

```html
<!DOCTYPE html>
<html lang="und">
<head>
<meta charset="utf-8">
<title>document.pdf</title>
</head>
<body>
...content...
</body>
</html>
```

### 3.2 Element Rendering Rules

| Element | HTML |
|---------|------|
| Heading (level N) | `<hN>text</hN>` (N capped 1–6) |
| Paragraph | `<p>text</p>` |
| Table | `<table border="1">`, `<tr>`, `<th>`/`<td>` with colspan/rowspan |
| List | `<ul>` + `<li>` items, item body in `<p>` |
| Image | `<img src="path" alt="figureN">` |
| Picture | `<figure><img ...><figcaption>desc</figcaption></figure>` |
| Formula | `<div class="math-display">\[latex\]</div>` (MathJax) |
| Header/Footer | Contents rendered recursively |
| Caption | `<figcaption>text</figcaption>` |

### 3.3 HTML Escaping

Attribute values: `&` → `&amp;`, `"` → `&quot;`, `<` → `&lt;`, `>` → `&gt;`, `\n` → space, `\r` → removed.

### 3.4 Table HTML

```html
<table border="1">
<tr><th>Header 1</th><th>Header 2</th></tr>
<tr><td colspan="2">Merged Cell</td></tr>
<tr><td rowspan="2">Spans 2 rows</td><td>Normal</td></tr>
<tr><td>Normal</td></tr>
</table>
```

First row always uses `<th>`. Subsequent rows use `<td>`.

---

## 4. Text Output

### 4.1 Element Rendering Rules

| Element | Text |
|---------|------|
| Heading | Plain text, indented at current level |
| Paragraph | Plain text, indented at current level |
| Table | Tab-separated cell text per row |
| List | Item text, indented, sub-content at level+1 |
| Image | Not rendered |
| Formula | Not rendered |
| Picture | Not rendered |

### 4.2 Indentation

```
INDENT = "  "  (2 spaces)

Level 0: no indent
Level 1: "  "
Level 2: "    "
Level N: repeat INDENT N times
```

### 4.3 Table Text

```
Header1	Header2	Header3
Cell A	Cell B	Cell C
Cell D	Cell E	Cell F
```

- Tab character (`\t`) separates cells
- Empty rows/cells skipped
- Cell contents: whitespace compacted to single spaces

### 4.4 Separators

- Between elements within a page: system line separator
- Between pages: configurable `text_page_separator` with `{page_number}` substitution, followed by line separator

---

## 5. Annotated PDF Output

### 5.1 Overlay Mechanism

Creates a copy of the original PDF with semi-transparent colored rectangle annotations overlaid on detected elements.

### 5.2 Annotation Properties

- Type: `PDAnnotationSquare`
- Opacity: `0.4` (40% transparent)
- Tooltip: `"id = N, level = L, <description>"`
- Multi-page elements: annotations linked via `setInReplyTo()`

### 5.3 Color Scheme

```
Element Type        RGB Color       Visual
------------------  --------------  --------
Heading / Header    [0, 0, 1]       Blue
  / Footer
List                [0, 1, 0]       Green
Paragraph           [0, 1, 1]       Cyan
Figure              [1, 0, 0]       Red
Table               [1, 0, 1]       Magenta
Caption             [1, 1, 0]       Yellow
LineArt / Line      [0.9, 0.9, 0.9] Light Gray
```

### 5.4 PDF Layers (Optional Content Groups)

Togglable visibility layers:

| Layer | Name String |
|-------|-------------|
| Content | `"content"` |
| Table Cells | `"table cells"` |
| List Items | `"list items"` |
| Table Content | `"table content"` |
| List Content | `"list content"` |
| Text Block Content | `"text blocks content"` |
| Header/Footer Content | `"header and footer content"` |

### 5.5 Tooltip Content Examples

```
Table: 5 rows, 3 columns, next table id 42
List: number of items 10, previous list id 33
Table cell: row number 2, column number 1, row span 1, column span 2, text content "Amount"
List item: text content "First item"
Heading level 2
Caption, connected with object with id = 15
Image: height 200, width 300
```

---

## 6. Image Extraction

### 6.1 Extraction Flow

```
1. First image encountered:
   → Create directory: <output>/<pdfname>_images/
   → Initialize page renderer (for sub-image cropping)

2. For each ImageChunk/SemanticPicture:
   → Render page as image at print resolution
   → Crop bounding box region
   → Save as imageFile<N>.<format>

3. Recursive extraction:
   → Images inside table cells
   → Images inside list items
   → Images inside headers/footers
```

### 6.2 File Naming

```
<pdfname>_images/imageFile1.png
<pdfname>_images/imageFile2.png
<pdfname>_images/imageFile3.jpeg
```

Index is auto-incremented globally across the document.

### 6.3 Base64 Embedding

When `image_output = Embedded`:
- MIME type: `image/png` or `image/jpeg`
- Format: `data:<mime>;base64,<encoded>`
- **Max size: 10 MB** — larger images silently skipped
- Used in JSON `"data"` field, Markdown `![]()` src, HTML `<img src>`

### 6.4 Image Directory Override

- Default: `<output_dir>/<pdfname>_images/`
- Override: `--image-dir <path>` → all images go to `<path>/`

---

## 7. Output File Naming

```
Input: /path/to/document.pdf

Outputs:
  /output/document.json                  (JSON)
  /output/document.md                    (Markdown)
  /output/document.html                  (HTML)
  /output/document.txt                   (Text)
  /output/document_annotated.pdf         (Annotated PDF)
  /output/document_images/imageFile1.png (External images)
```

When processing directories, relative structure is preserved:
```
Input:  /input/subdir/report.pdf
Output: /output/subdir/report.json
        /output/subdir/report_images/imageFile1.png
```

---

## 8. JSON Schema Reference

The output JSON conforms to [schema.json](../schema.json) (JSON Schema Draft-07).

### 8.1 Type Discriminator

The `"kids"` array uses `oneOf` discrimination on the `"type"` field:

```
contentElement = oneOf:
  paragraph     (type = "paragraph")
  heading       (type = "heading")
  caption       (type = "caption")
  table         (type = "table")
  textBlock     (type = "text block")
  list          (type = "list")
  image         (type = "image")
  headerFooter  (type = "header" | "footer")
```

**Note**: The `formula` type is produced by the serializer but not currently defined in schema.json. The Rust rewrite should add it.

### 8.2 Cross-Page Linking Fields

| Field | On Type | Value |
|-------|---------|-------|
| `"previous table id"` | table | Integer ID of table on prior page |
| `"next table id"` | table | Integer ID of table on next page |
| `"previous list id"` | list | Integer ID of list on prior page |
| `"next list id"` | list | Integer ID of list on next page |
