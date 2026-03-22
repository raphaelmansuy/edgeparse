# 02 — Functional Specification

> **Cross-references**: [01-overview](01-overview.md) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) | [06-cli-interface](06-cli-interface.md) | [08-output-formats](08-output-formats.md)

---

## 1. Core Use Cases

### UC-1: Convert PDF to Structured Data

**Actor**: Developer / Data Engineer  
**Input**: One or more PDF files, or a directory of PDFs  
**Output**: JSON, Markdown, HTML, text, and/or annotated PDF files  
**Preconditions**: Rust binary installed  

**Flow**:
1. User invokes CLI: `opendataloader-pdf input.pdf --format json,markdown`
2. System validates all input paths exist
3. For each PDF file (recursive directory traversal):
   a. Load PDF document
   b. Extract text chunks, line segments, images from each page
   c. Apply content safety filters (hidden text, off-page, tiny text)
   d. Detect tables (border-based and/or cluster-based)
   e. Group text chunks into text lines
   f. Detect headers and footers (cross-page comparison)
   g. Detect lists (label pattern matching)
   h. Form paragraphs (alignment-based grouping)
   i. Detect headings (font statistics heuristics)
   j. Link captions to tables/images
   k. Assign heading levels and nesting levels
   l. Sort content by reading order (XY-Cut++)
   m. Apply PII sanitization (if enabled)
   n. Generate requested output formats
4. Exit with code 0 (success) or 1 (any file failed)

### UC-2: Convert with Hybrid AI Backend

**Actor**: Developer needing high accuracy on complex PDFs  
**Preconditions**: Hybrid backend server running (Python FastAPI)  

**Flow**:
1. User starts backend: `opendataloader-pdf-hybrid --port 5002`
2. User invokes: `opendataloader-pdf --hybrid docling-fast input.pdf`
3. System checks backend health (`GET /health`)
4. For each page, triage classifier decides:
   - **JAVA path**: Simple text page → process locally (Steps 3c–3n from UC-1)
   - **BACKEND path**: Complex page → send PDF subset via HTTP POST
5. Backend returns structured JSON (Docling/Hancom schema)
6. Schema transformer converts backend response to internal model
7. Results merged preserving page order
8. Post-processing (headers/footers, headings, levels) applied
9. Output generated

### UC-3: Extract from Tagged PDF

**Actor**: Developer with well-tagged accessible PDFs  
**Flow**:
1. User invokes: `opendataloader-pdf --use-struct-tree input.pdf`
2. System reads PDF structure tree
3. Walks tree nodes, maps tag types to semantic elements
4. Heading levels extracted from H1-H6 tags
5. Table structure read from Table/TR/TD/TH tags
6. List structure read from L/LI tags
7. Reading order follows structure tree order (no XY-Cut needed)
8. Output generated

### UC-4: Batch Processing

**Actor**: Data pipeline processing document corpus  
**Flow**:
1. User invokes: `opendataloader-pdf dir1/ dir2/ file1.pdf file2.pdf`
2. System recursively traverses each argument
3. Non-PDF files silently skipped
4. Each PDF processed independently
5. Failure in one file does not stop others
6. Exit code 1 if any file failed, 0 otherwise

---

## 2. Input Specifications

### 2.1 Supported Input

| Input Type | Behavior |
|-----------|----------|
| Digital PDF (text-based) | Full extraction with local engine |
| Scanned PDF (image-based) | Requires hybrid mode with `--force-ocr` on server |
| Tagged PDF (with structure tree) | Use `--use-struct-tree` for semantic extraction |
| Encrypted PDF | Supply password via `--password` |
| Multiple files | Positional arguments, processed sequentially |
| Directories | Recursive traversal, `.pdf` extension filter (case-insensitive) |

### 2.2 Page Selection

**Option**: `--pages "1,3,5-7"`

**Parsing rules**:
- Comma-separated page numbers and ranges
- Ranges use dash: `5-7` → pages 5, 6, 7
- 1-indexed
- Invalid pages (> total) silently ignored
- Default: all pages

---

## 3. Output Specifications

### 3.1 Output Formats

| Format Flag | File Extension | Spec Reference |
|-------------|---------------|----------------|
| `json` | `.json` | [08-output-formats §1](08-output-formats.md#1-json-output) |
| `markdown` | `.md` | [08-output-formats §2](08-output-formats.md#2-markdown-output) |
| `markdown-with-html` | `.md` | [08-output-formats §3](08-output-formats.md#3-markdown-with-html) |
| `markdown-with-images` | `.md` | [08-output-formats §4](08-output-formats.md#4-markdown-with-images) |
| `html` | `.html` | [08-output-formats §5](08-output-formats.md#5-html-output) |
| `text` | `.txt` | [08-output-formats §6](08-output-formats.md#6-text-output) |
| `pdf` | `_annotated.pdf` | [08-output-formats §7](08-output-formats.md#7-annotated-pdf) |

**Combining formats**: `--format json,markdown,html` generates all three.

**Default behavior**:
- When `--format` is NOT specified → JSON output only (default)
- When `--format` IS specified → ONLY the listed formats (JSON disabled unless listed)

### 3.2 Output Directory

| Scenario | Output Location |
|----------|----------------|
| `--output-dir ./out` specified | All output goes to `./out/` |
| No `--output-dir`, input is file | File's parent directory |
| No `--output-dir`, input is directory | The directory itself |

### 3.3 Image Output Modes

| Mode | `--image-output` | Behavior |
|------|------------------|----------|
| External (default) | `external` | Images saved as separate files |
| Embedded | `embedded` | Base64 data URIs in JSON/Markdown/HTML |
| Off | `off` | No image extraction |

Image format: `--image-format png` (default) or `jpeg`  
Image directory: `--image-dir ./images` (default: alongside output)

---

## 4. Document Element Types

The system recognizes these semantic element types in the output:

| Element Type | JSON `type` Value | Description |
|-------------|-------------------|-------------|
| Paragraph | `"paragraph"` | Body text block |
| Heading | `"heading"` | Section heading with level (1-6) |
| Table | `"table"` | Structured table with rows and cells |
| Table Row | `"table row"` | Row within a table |
| Table Cell | `"table cell"` | Cell with row/col span support |
| List | `"list"` | Ordered or unordered list |
| List Item | `"list item"` | Item within a list |
| Image | `"image"` | Extracted image with coordinates |
| Caption | `"caption"` | Caption linked to table/image via `linked content id` |
| Header | `"header"` | Page header (filtered by default) |
| Footer | `"footer"` | Page footer (filtered by default) |
| Text Block | `"text block"` | Generic container with child elements |
| Formula | `"formula"` | LaTeX formula (hybrid mode only) |
| Picture | `"picture"` | AI-described image (hybrid mode only) |

→ Full data model: [05-data-models](05-data-models.md)

---

## 5. Content Safety Features

### 5.1 Default Filters (Always Active)

| Filter | What It Catches | Disable Flag |
|--------|----------------|--------------|
| Hidden text | Transparent/zero-contrast text (ratio < 1.2) | `--content-safety-off hidden-text` |
| Off-page content | Content outside CropBox boundaries | `--content-safety-off off-page` |
| Tiny text | Text with height ≤ 1 point | `--content-safety-off tiny` |
| Hidden OCG layers | Invisible Optional Content Groups | `--content-safety-off hidden-ocg` |
| All of the above | — | `--content-safety-off all` |

### 5.2 Prompt Injection Protection

Hidden text filtering serves as **AI safety**: attackers embed invisible instructions in PDFs to manipulate LLMs. Default-on filtering prevents these attacks from reaching downstream systems.

### 5.3 PII Sanitization (Opt-in)

**Option**: `--sanitize`

Replaces sensitive data patterns with safe placeholders:

| Pattern | Regex | Replacement |
|---------|-------|-------------|
| Email | Standard email pattern | `email@example.com` |
| Phone | `+XX-XXXX-XXXX` formats | `+00-0000-0000` |
| ID Numbers | 1-2 uppercase + 6-9 digits | `AA0000000` |
| Credit Cards | 4×4 digit groups | `0000-0000-0000-0000` |
| Long Numbers | 10-18 consecutive digits | `0000000000000000` |
| IPv4 | Dotted quad | `0.0.0.0` |
| IPv6 | Colon-separated | `0.0.0.0::1` |
| MAC Address | 6 hex pairs | `00:00:00:00:00:00` |
| IMEI | 15 digits | `000000000000000` |
| URLs | `http(s)://...` | `https://example.com` |

**Application scope**: All text content including text inside table cells, list items, headers, and footers. Sanitization runs at the TextLine level — regex matches are found across concatenated chunk text and replacement chunks are created preserving positional metadata.

→ Algorithm details: [04-pdf-parsing-pipeline §16](04-pdf-parsing-pipeline.md#16-content-sanitization)

---

## 6. Table Detection

### 6.1 Detection Methods

| Method | `--table-method` | How It Works |
|--------|-----------------|--------------|
| Default | `default` | Border-based: detects visible line segments forming table grids |
| Cluster | `cluster` | Border + cluster: additionally uses text alignment clustering for borderless tables |

### 6.2 Table Features

| Feature | Supported |
|---------|-----------|
| Simple bordered tables | Yes (default mode) |
| Complex/borderless tables | Yes (cluster mode or hybrid) |
| Merged cells (row/col span) | Yes |
| Nested tables | Yes (depth limit: 10) |
| Cross-page tables | Yes (linked via `previous table id` / `next table id`) |
| Table-image detection | Yes (large images with table-like aspect ratio trigger hybrid) |
| Korean format tables | Yes (수신/경유/제목 pattern detection) |

### 6.3 Cross-Page Table Linking

Tables are linked across pages when:
1. Same number of columns
2. Same total width (within 20% tolerance)
3. Same column width distribution

---

## 7. Reading Order

### 7.1 XY-Cut++ Algorithm

**Option**: `--reading-order xycut` (default) or `off`

Four-phase algorithm based on arXiv:2504.10258:

```
Phase 1: Pre-mask cross-layout elements
    Elements wider than beta * maxWidth AND overlapping >= 2 others
    
Phase 2: Compute density ratio
    contentArea / regionArea > 0.9 => prefer horizontal-first cuts
    
Phase 3: Recursive segmentation
    Find best horizontal/vertical cut via projection profiles
    Split by center point, recurse on both groups
    Base case: <= 1 element or no valid cuts (gap < 5 pts)
    
Phase 4: Merge cross-layout elements back by Y-position
```

### 7.2 Reading Order Off

When `--reading-order off`, content appears in PDF content stream order (typically top-to-bottom, left-to-right within each page, but not guaranteed for multi-column layouts).

---

## 8. Heading Detection

### 8.1 Detection Heuristic

Headings are detected through a **multi-signal probability score**:

```
score = base_heading_probability(textNode, context)
      + font_size_rarity_boost(textNode)     (max +0.5)
      + font_weight_rarity_boost(textNode)   (max +0.3)
      + bulleted_paragraph_bonus             (+0.1)
```

If `score ≥ 0.75` AND the node is not a list item → classified as heading.

### 8.2 Level Assignment

1. Collect all detected headings across entire document
2. Group by visual style (font name + size + weight + color)
3. Sort style groups by visual prominence (larger/bolder first)
4. Assign levels: most prominent = H1, next = H2, etc.

---

## 9. Header/Footer Detection

### 9.1 Cross-Page Comparison

Headers and footers are detected by comparing content at corresponding positions across adjacent pages:

1. For each page, sort top elements (headers) and bottom elements (footers)
2. Compare element at position `i` on page `n` with position `i` on page `n+1`
3. Match criteria:
   - Same text content, OR
   - Sequential numbering (Arabic, Roman, Korean, alphabetic) with increment 1 or 2
   - Overlapping bounding boxes (excluding page-specific coordinates)
   - Similar font size
4. Position filter: headers must be in top 1/3, footers in bottom 1/3
5. Supports 2-page styles (comparing pages n and n+2 for alternating left/right layouts)

### 9.2 Output Behavior

| `--include-header-footer` | Behavior |
|---------------------------|----------|
| `false` (default) | Headers and footers detected but excluded from output |
| `true` | Headers and footers included as `"header"` / `"footer"` elements |

---

## 10. List Detection

### 10.1 Two-Pass Detection

**Pass 1** — TextLine level:
- Scan for list label patterns in TextLine text
- Supported patterns: Arabic numerals, Korean (가나다라..., 제N장/조/절), Roman numerals, circled numbers, bullet characters, `붙임` prefix
- Build list intervals, verify sequence consistency
- Group consecutive matching labels into PDFList structures

**Pass 2** — Paragraph level:
- After paragraphs are formed, detect lists formed by SemanticTextNode sequences
- Same label pattern matching
- Filter: reject sequences like `1.0, 2.5` (decimal numbers, not lists)

### 10.2 Cross-Page Lists

Adjacent lists across page boundaries are merged if they form a continuous numbering sequence.

### 10.3 List Item Body

Content between consecutive list labels is assigned as the list item body. Criteria:
- Same vertical alignment (within `fontSize * 0.3`)
- Not a labeled line itself
- Merge probability > 0.7
- Baseline difference ratio < 1.2

---

## 11. Caption Linking

Captions are linked to adjacent tables or images:
1. Linear scan through page content
2. For each table/image, compare caption probability of preceding and following text nodes
3. If best probability ≥ 0.75 → mark as caption with `linked content id` pointing to the table/image
4. Skip decorative images (aspect ratio < 0.01)

---

## 12. Tagged PDF Extraction

When `--use-struct-tree` is enabled:
1. Parse the PDF structure tree root
2. Walk structure tree nodes recursively
3. Map standard structure types to semantic elements:
   - `H1`-`H6` → SemanticHeading with level
   - `P` → SemanticParagraph
   - `Table` → TableBorder
   - `TR`, `TD`, `TH` → Row/Cell structures
   - `L`, `LI` → PDFList/ListItem
   - `Figure` → SemanticFigure
4. Reading order follows structure tree order
5. No heuristic detection needed (headings, lists, tables from tags)

---

## 13. Text Processing

### 13.1 Character Handling

| Feature | Option | Default |
|---------|--------|---------|
| Invalid character replacement | `--replace-invalid-chars " "` | Space |
| Line break preservation | `--keep-line-breaks` | `false` (merge lines) |

### 13.2 Text Chunk Merging

Adjacent text chunks with the same font, size, and baseline are merged. Whitespace handling:
1. Trim leading/trailing whitespace from each chunk
2. Compress consecutive spaces within chunks
3. Split chunks by internal whitespace boundaries
4. Insert space chunks at natural word boundaries (gap > `fontSize * spaceRatio`)

---

## 14. Performance Requirements

| Metric | Target (Local Mode) | Target (Hybrid Mode) |
|--------|--------------------|--------------------|
| Speed (s/page) | ≤ 0.10 | ≤ 1.0 |
| Memory per page | ≤ 50 MB | ≤ 100 MB |
| Startup time | ≤ 50 ms | ≤ 100 ms |
| NID (reading order) | ≥ 0.85 | ≥ 0.90 |
| TEDS (table accuracy) | ≥ 0.40 | ≥ 0.85 |
| MHS (heading accuracy) | ≥ 0.55 | ≥ 0.75 |
| Table detection F1 | ≥ 0.55 | ≥ 0.55 |

---

## 15. Error Handling

| Scenario | Behavior | Exit Code |
|----------|----------|-----------|
| Invalid CLI arguments | Print error + help text | 2 |
| File not found | Log warning, continue | 1 (at end) |
| Directory unreadable | Log warning, continue | 1 (at end) |
| PDF parsing error | Log error, continue | 1 (at end) |
| Encrypted PDF, no password | Log error, continue | 1 (at end) |
| Hybrid backend unreachable | Throw error (no fallback) | 1 |
| Hybrid partial failure | Fallback if `--hybrid-fallback` set | 0 or 1 |
| All files successful | — | 0 |
| Help/version displayed | — | 0 |
| `--export-options` | Print JSON to stdout | 0 |

---

## 16. Internationalization

### 16.1 Supported Text Content

All Unicode text is supported. No language-specific processing in text extraction.

### 16.2 Language-Specific Features

| Feature | Languages |
|---------|-----------|
| List label detection | Arabic numerals, Korean (가나다라, 제N장/조/절), Roman, circled numbers |
| Header/footer numbering | Arabic, Roman, Korean, alphabetic |
| Special table patterns | Korean government format (수신/경유/제목) |
| OCR languages (hybrid) | 80+ via EasyOCR: `en`, `ko`, `ja`, `ch_sim`, `ch_tra`, `de`, `fr`, `ar`, etc. |
