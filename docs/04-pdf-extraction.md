# EdgeParse — PDF Extraction Layer

> Explains how raw PDF bytes become `PageChunks` before the 20-stage pipeline begins.

---

## 01 · Extraction Architecture

```
PDF File (bytes)
    │
    ▼ loader::load_pdf()                   [pdf/loader.rs]
    │
    ├── lopdf::Document::load()
    │     └── Parses cross-reference table (xref)
    │         Parses trailer dictionary
    │         Handles linearised PDFs
    │         Handles encrypted PDFs (RC4/AES)
    │
    ├── RawPdfDocument {
    │     document: lopdf::Document   ← in-memory PDF object graph
    │     num_pages: u32
    │     metadata: PdfMetadata { author, title, creation_date, modification_date }
    │   }
    │
    ▼ page_info::extract_page_info()       [pdf/page_info.rs]
    │
    │   For each page:
    │   ├── Read /MediaBox → BoundingBox
    │   ├── Read /CropBox  → BoundingBox (or clone MediaBox)
    │   └── Read /Rotate   → i64
    │
    ▼ chunk_parser::extract_page_chunks()  [pdf/chunk_parser.rs]
    │   (one call per page, parallelisable at call site)
    │
    │   1. resolve_page_fonts()  → FontCache
    │   2. get_page_content()    → Vec<u8> (merged decompressed streams)
    │   3. Content::decode()     → Vec<Operation>
    │   4. ChunkParserState::process_operations() → PageChunks
    │
    ▼ PageChunks {
         text_chunks:      Vec<TextChunk>
         image_chunks:     Vec<ImageChunk>
         line_chunks:      Vec<LineChunk>
         line_art_chunks:  Vec<LineArtChunk>
       }
```

---

## 02 · PDF Loading

**Source:** [`pdf/loader.rs`](../crates/edgeparse-core/src/pdf/loader.rs)

```rust
pub fn load_pdf(path: &Path, password: Option<&str>) -> Result<RawPdfDocument, EdgePdfError>
```

Uses `lopdf::Document::load()` (via `pdf-cos` fork). Internally:

1. Opens file
2. Parses xref table (supports PDF 1.0–1.7, PDF 2.0 cross-reference streams)
3. Decrypts if password provided (see `pdf/encryption.rs`)
4. Builds in-memory object graph (lazy: streams not decompressed yet)

After loading, `extract_metadata()` reads the `/Info` dictionary:
```
/Info dictionary keys read:
  Author, Title, CreationDate, ModDate
```

---

## 03 · Content Stream Parsing

**Source:** [`pdf/chunk_parser.rs`](../crates/edgeparse-core/src/pdf/chunk_parser.rs)

One call to `extract_page_chunks()` does a single-pass walk of all content stream operators on a page.

### Content Stream Acquisition

```rust
// get_page_content() — pdf/text_extractor.rs (reused by chunk_parser)
match page_dict.get("Contents") {
    Object::Reference(id) → dereference → Object::Stream → decompressed bytes
    Object::Array(refs)   → collect + concatenate all stream bytes
}
```

Multiple content streams per page are concatenated with a space separator to maintain operator boundary safety.

### Content Stream Operators Handled

```
TEXT OPERATORS
  BT / ET          ── begin/end text object
  Tf name size     ── set font + size
  Td dx dy         ── move text position (offset)
  TD dx dy         ── move + set leading
  Tm a b c d e f  ── set text matrix
  T*               ── move to next line
  Tj string        ── show string
  TJ array         ── show string array (with kerning adjustments)
  '  string        ── move to next line, show string
  "  aw ac string  ── set word+char spacing, show string

GRAPHICS STATE
  q / Q            ── push / pop graphics state stack
  cm a b c d e f  ── concatenate CTM (current transform matrix)
  gs name          ── apply extended graphics state from /ExtGState
  w                ── set line width
  J / j            ── set line cap / join style

PATH OPERATORS
  m x y            ── move to
  l x y            ── line to
  c ...            ── cubic bezier
  v / y ...        ── alternate bezier
  re x y w h      ── rectangle
  S / s            ── stroke path
  f / F            ── fill path
  B / b            ── fill + stroke
  n                ── end path (no paint)
  h                ── close subpath

IMAGE OPERATORS
  Do name          ── invoke XObject (image or form)
  BI/ID/EI         ── inline image

COLOR OPERATORS
  g / G            ── gray fill / stroke
  rg / RG          ── RGB fill / stroke
  k / K            ── CMYK fill / stroke
  cs / CS          ── set colorspace
  sc / SC / scn    ── set color components

MARKED CONTENT
  BMC / BDC        ── begin marked content
  EMC              ── end marked content
```

### ChunkParserState Internal State

```
ChunkParserState {
    page_number:    u32
    font_cache:     FontCache           // resolved fonts for this page
    graphics_stack: GraphicsStateStack  // q/Q push/pop stack
    current_font:   Option<(name, PdfFont)>
    current_color:  [f64; 4]           // fill color (RGB/CMYK/Gray)
    text_matrix:    Matrix             // Tm — text position matrix
    line_matrix:    Matrix             // Tlm — line position matrix
    mcid_stack:     Vec<Option<i64>>   // BDC/EMC marked content stack
    chunks:         Vec<TextChunk>
    images:         Vec<ImageChunk>
    lines:          Vec<(start, end, width)>  // pending path segments
    paths:          Vec<Vec<Vertex>>   // complex paths
}
```

---

## 04 · Graphics State Machine

**Source:** [`pdf/graphics_state.rs`](../crates/edgeparse-core/src/pdf/graphics_state.rs)

The graphics state machine tracks the CTM (Current Transformation Matrix) and color state across `q`/`Q` push/pop boundaries.

```
GraphicsStateStack {
    stack: Vec<GraphicsState>
}

GraphicsState {
    ctm:          Matrix   // current transformation matrix [a b c d e f]
    fill_color:   [f64; 4]
    stroke_color: [f64; 4]
    line_width:   f64
    text_state:   TextState {
        font_size, char_spacing, word_spacing, text_rise, text_leading
    }
}

Matrix [a b c d e f]:
  a  b  0
  c  d  0
  e  f  1

Transform: (x', y') = (a*x + c*y + e, b*x + d*y + f)
```

**CTM usage for text positioning:**
```
Tm (text matrix) × CTM → absolute page coordinates of glyph
```

**CTM usage for image positioning:**
```
/Do XObject: current CTM defines image bounding box as CTM × unit square
  → bbox = { left_x: e, bottom_y: f, right_x: e+a, top_y: f+d }
```

---

## 05 · Font Resolution

**Source:** [`pdf/font.rs`](../crates/edgeparse-core/src/pdf/font.rs)

```
resolve_page_fonts(doc, page_id) → FontCache

FontCache = HashMap<fontname: Vec<u8>, PdfFont>

PdfFont {
    base_font_name:  String
    font_size:       f64          // from Tf operator
    font_weight:     f64          // from font descriptor
    italic_angle:    f64          // from font descriptor
    encoding:        Encoding     // StandardEncoding | WinAnsiEncoding | custom
    cmap:            Option<ToUnicode>  // CMap for non-Latin scripts
    widths:          Vec<f64>     // glyph advance widths
    first_char:      u32          // first encoded character code
}
```

### Glyph → Unicode Mapping

```
Priority order:
  1. ToUnicode CMap (highest fidelity for complex scripts)
  2. Encoding array (StandardEncoding, WinAnsiEncoding, MacRomanEncoding)
  3. GlyphName → Unicode via AGL (Adobe Glyph List)
  4. Direct byte value (Latin-1 fallback)
  5. U+FFFD (replaced by Stage 2b)
```

### CMap Parsing

**Source:** [`pdf-cos/src/encodings/cmap.rs`](../crates/pdf-cos/src/encodings/cmap.rs)

Parses `beginbfchar` / `endbfchar` / `beginbfrange` / `endbfrange` sections to build code → Unicode mappings for:
- Type0 (CIDFont) composite fonts
- Type1/TrueType fonts with custom encoding

---

## 06 · Glyph Positioning

When the `Tj` or `TJ` operator fires:

```
For each glyph code c:
  1. Look up (c → Unicode) via font encoding/CMap
  2. Advance width = PdfFont.widths[c - first_char] (or fallback)
  3. Apply text matrix: glyph_x = Tm.e, glyph_y = Tm.f
  4. Apply CTM: page_x = CTM.transform(glyph_x, glyph_y)
  5. Accumulate glyph for current TextChunk
  6. Advance Tm by width * font_size / 1000

TJ array kerning:
  Negative number → advance left (tighter spacing)
  Positive number → advance right (looser spacing)
  Threshold: |kern| > font_size * 0.3 → split into new TextChunk
```

`symbol_ends` in `TextChunk` records the right X of each glyph.

---

## 07 · Image Extraction

**Source:** [`pdf/image_extractor.rs`](../crates/edgeparse-core/src/pdf/image_extractor.rs), handled in `chunk_parser.rs`

### XObject Images (`Do` operator)

```
Do /ImageName
  → look up XObject in /Resources/XObject dictionary
  → if /Subtype == /Image:
      bbox from CTM × unit_square
      emit ImageChunk{bbox}
  → if /Subtype == /Form:
      save state, apply /Matrix, recurse process_operations()
```

Recursion depth limited by `MAX_FORM_RECURSION_DEPTH = 10` to prevent infinite loops from self-referential Forms.

### Inline Images (`BI`/`ID`/`EI`)

Inline images appear directly in the content stream between `BI` (begin image) and `EI` (end image) markers. Parsed from the raw content bytes; bounding box computed from the current CTM.

---

## 08 · Line / Path Extraction

**Source:** [`pdf/line_extractor.rs`](../crates/edgeparse-core/src/pdf/line_extractor.rs), `chunk_parser.rs`

```
Path lifecycle:
  m x y    → start new subpath vertex
  l x y    → add line segment
  re x y w h → add rectangle (4 vertices)
  c,v,y    → Bezier (sampled to endpoints for bbox)
  h         → close subpath
  S/f/B/n   → paint/end path

Classification after path is complete:
  Segment count == 1 AND aspect > LINE_ASPECT_RATIO (3.0) AND thickness < 10pt
    → LineChunk { is_horizontal_line: true/false }

  Segment count == 4 (rectangle) AND small size
    → LineChunk { is_square: true }

  Otherwise (complex path, compound figure)
    → LineArtChunk { line_chunks: [...] }
```

Constants:
- `LINE_ASPECT_RATIO = 3.0` (width/height threshold for line classification)
- `MAX_LINE_THICKNESS = 10.0` pt
- `MIN_LINE_WIDTH = 0.1` pt (below this: invisible, skip)

---

## 09 · Raster Table OCR Recovery

**Source:** [`pdf/raster_table_ocr.rs`](../crates/edgeparse-core/src/pdf/raster_table_ocr.rs)

When a PDF page contains a raster image that looks like a table (dense horizontal/vertical lines within the image pixels), this module attempts to recover the table structure by:

1. Rendering the page region to a pixel buffer
2. Detecting horizontal/vertical dark line segments in the pixel data
3. Converting pixel coordinates to PDF coordinate space using the CropBox
4. Emitting `TableBorder` elements

This is a fallback path for scanned documents where table borders are embedded in raster images rather than as PDF path operators.

---

## 10 · Marked Content / Tagged PDFs

**Source:** [`tagged/struct_tree.rs`](../crates/edgeparse-core/src/tagged/struct_tree.rs)

Tagged PDFs embed accessibility structure via:
- Content stream: `BDC /tag <</MCID N>>` … `EMC` operators
- Catalogue: `/StructTreeRoot` → tree of structure elements

The chunk parser records `mcid: Option<i64>` on each `TextChunk` from the innermost BDC context.

`build_mcid_map(doc)` then creates:
```
McidMap: (page_number, mcid) → McidTagInfo { role, heading_level, struct_type }
```

Stage 12 (`heading_detector`) uses this map to promote chunks tagged as `H`, `H1`–`H6` directly to headings.

---

## 11 · Encryption

**Source:** [`pdf/encryption.rs`](../crates/edgeparse-core/src/pdf/encryption.rs)
**pdf-cos:** [`crates/pdf-cos/src/encryption/`](../crates/pdf-cos/src/encryption/)

Supports:
- RC4 40-bit (PDF 1.1–1.3)
- RC4 128-bit (PDF 1.4)
- AES 128-bit (PDF 1.5–1.6)
- AES 256-bit (PDF 1.7, ISO 32000-2)

Password is passed through `config.password` → `loader::load_pdf(path, password)`.

---

## Cross-Reference

| Topic | Document |
|-------|---------|
| Data types produced here | [03-data-model.md](03-data-model.md) |
| Pipeline that processes these chunks | [02-pipeline.md](02-pipeline.md) |
| Architecture overview | [01-architecture.md](01-architecture.md) |
