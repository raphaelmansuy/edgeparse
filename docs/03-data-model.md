# EdgeParse — Data Model

> All types listed here are defined in `crates/edgeparse-core/src/models/`.
> Every field shown maps to a real Rust struct field.

---

## 01 · Type Hierarchy

```
BoundingBox                          ← geometry primitive
    │
    ├── TextChunk                    ← atomic font run
    ├── ImageChunk                   ← raster/vector image position
    ├── LineChunk                    ← path segment (line/rect)
    └── LineArtChunk                 ← compound path (bullet, border)
             │
             │  Stage 6: text_line_grouper
             ▼
           TextLine                  ← baseline-aligned chunk group
             │
             │  Stage 7: text_block_grouper
             ▼
           TextBlock                 ← paragraph-like text region
             │
             │  Stage 10: paragraph_detector
             ▼
       SemanticTextNode              ← base for all semantic types
         ├── SemanticParagraph
         ├── SemanticHeading         ← heading_level: Option<u32>
         │     └── SemanticNumberHeading
         └── SemanticCaption
             │
             │ Lateral types (parallel to SemanticTextNode)
             ├── PDFList / PDFListItem
             ├── TableBorder / TableBorderRow / TableBorderCell
             ├── SemanticTable
             ├── SemanticFigure
             ├── SemanticHeaderOrFooter
             ├── SemanticFormula
             └── SemanticPicture
                         │
                         │  Wrapped by
                         ▼
                   ContentElement    ← unified page element enum
                         │
                         │  Collected into
                         ▼
                    PdfDocument.kids  ← final output
```

---

## 02 · BoundingBox

**Source:** [`models/bbox.rs`](../crates/edgeparse-core/src/models/bbox.rs)

```
BoundingBox {
    page_number:      Option<u32>   // 1-based; None = page-agnostic
    last_page_number: Option<u32>   // last page for cross-page elements
    left_x:           f64           // left edge in PDF user units (points)
    bottom_y:         f64           // bottom edge
    right_x:          f64           // right edge
    top_y:            f64           // top edge
}
```

**Coordinate system:**

```
   (0,0) ─────────────────── (width, 0)
     │   PDF user space           │
     │   72 pt = 1 inch           │
     │   origin: bottom-left      │
     │                            │
  (0, height) ──────── (width, height)   ← top-left in visual terms

  ┌─────────────────── right_x
  │  (left_x, top_y) ─────┐
  │  │                    │
  │  │   Element          │
  │  │                    │
  │  └── (left_x, bottom_y)
  ▼
  bottom_y
```

**Key methods:**

| Method | Formula |
|--------|---------|
| `width()` | `right_x - left_x` |
| `height()` | `top_y - bottom_y` |
| `area()` | `width() × height()` |
| `center_x()` | `(left_x + right_x) / 2` |
| `center_y()` | `(bottom_y + top_y) / 2` |
| `union(other)` | min left/bottom, max right/top |
| `intersects(other)` | overlap test with epsilon |
| `is_empty()` | width ≤ ε OR height ≤ ε |

---

## 03 · Chunk Types

**Source:** [`models/chunks.rs`](../crates/edgeparse-core/src/models/chunks.rs)

### TextChunk — Atomic font run

```
TextChunk {
    value:          String          // decoded Unicode text
    bbox:           BoundingBox
    font_name:      String          // base font name, e.g. "Helvetica"
    font_size:      f64             // effective size in points (post-CTM)
    font_weight:    f64             // 100.0–900.0
    italic_angle:   f64             // from font descriptor
    font_color:     String          // "#RRGGBB"
    contrast_ratio: f64             // 1.0–21.0 (WCAG contrast)
    symbol_ends:    Vec<f64>        // X position of each glyph end
    text_format:    TextFormat      // Normal | Superscript | Subscript
    text_type:      TextType        // Regular | Math | Code | ...
    pdf_layer:      PdfLayer        // Main | Form | Annotation
    ocg_visible:    bool            // OCG (Optional Content Group) visibility
    index:          Option<usize>
    page_number:    Option<u32>
    level:          Option<String>
    mcid:           Option<i64>     // structure tree marker ID
}
```

`symbol_ends` enables character-level bounding box queries:
```
symbol_start_coordinate(idx) → symbol_ends[idx-1] or bbox.left_x
symbol_end_coordinate(idx)   → symbol_ends[idx]   or bbox.right_x
```

### ImageChunk

```
ImageChunk {
    bbox:   BoundingBox
    index:  Option<u32>
    level:  Option<String>
}
```

Pixel data is extracted lazily at output time; the chunk carries only position.

### LineChunk — Path segment

```
LineChunk {
    bbox:               BoundingBox
    index:              Option<u32>
    level:              Option<String>
    start:              Vertex { x: f64, y: f64 }
    end:                Vertex { x: f64, y: f64 }
    width:              f64             // stroke width (points)
    is_horizontal_line: bool
    is_vertical_line:   bool
    is_square:          bool
}
```

Used by `table_detector` (Stages 3-4) to discover grid structures.

### LineArtChunk — Complex vector graphic

```
LineArtChunk {
    bbox:        BoundingBox
    index:       Option<u32>
    level:       Option<String>
    line_chunks: Vec<LineChunk>   // component segments
}
```

Produced when a path has ≥ 3 segments or non-orthogonal geometry.

---

## 04 · Text Grouping Types

**Source:** [`models/text.rs`](../crates/edgeparse-core/src/models/text.rs)

### TextLine

```
TextLine {
    bbox:                      BoundingBox
    index:                     Option<u32>
    level:                     Option<String>
    font_size:                 f64           // dominant font size
    base_line:                 f64           // Y of text baseline
    slant_degree:              f64
    is_hidden_text:            bool
    text_chunks:               Vec<TextChunk>
    is_line_start:             bool          // begins paragraph
    is_line_end:               bool          // ends paragraph
    is_list_line:              bool
    connected_line_art_label:  Option<LineArtChunk>  // bullet marker
}
```

`TextLine.value()` reconstructs text with word-space inference:
```
needs_space(prev, curr):
  gap = curr.bbox.left_x - prev.bbox.right_x
  return gap > prev.font_size * TEXT_LINE_SPACE_RATIO  (0.17)
```

### TextBlock

```
TextBlock {
    bbox:        BoundingBox
    index:       Option<u32>
    level:       Option<String>
    text_lines:  Vec<TextLine>
    ...          (alignment, column, font aggregates)
}
```

### TextColumn

```
TextColumn {
    text_blocks: Vec<TextBlock>
}
```

`SemanticTextNode.value()` concatenates columns with `"\n"` separator.

---

## 05 · Semantic Node Hierarchy

**Source:** [`models/semantic.rs`](../crates/edgeparse-core/src/models/semantic.rs)

### SemanticTextNode (base)

```
SemanticTextNode {
    bbox:                  BoundingBox
    index:                 Option<u32>
    level:                 Option<String>
    semantic_type:         SemanticType     ← see §08 below
    correct_semantic_score:Option<f64>      // classifier confidence
    columns:               Vec<TextColumn>
    font_weight:           Option<f64>
    font_size:             Option<f64>
    text_color:            Option<Vec<f64>> // [R,G,B] or [K] or [C,M,Y,K]
    italic_angle:          Option<f64>
    font_name:             Option<String>
    text_format:           Option<TextFormat>
    max_font_size:         Option<f64>
    background_color:      Option<Vec<f64>>
    is_hidden_text:        bool
}
```

### Derived types

```
SemanticParagraph {
    base:          SemanticTextNode
    enclosed_top:  bool                // boxed at top
    enclosed_bottom: bool              // boxed at bottom
    indentation:   i32
}

SemanticHeading {
    base:          SemanticParagraph
    heading_level: Option<u32>         // 1-6; None until Stage 12
}

SemanticNumberHeading {
    base:          SemanticHeading     // "1.2.3 Title"
}

SemanticCaption {
    base:             SemanticTextNode
    linked_content_id: Option<u64>    // index of linked Figure/Table
}

SemanticHeaderOrFooter {
    bbox:          BoundingBox
    index:         Option<u32>
    level:         Option<String>
    semantic_type: SemanticType        // Header or Footer
    contents:      Vec<ContentElement>
}

SemanticFigure {
    bbox:          BoundingBox
    index:         Option<u32>
    level:         Option<String>
    semantic_type: SemanticType
    images:        Vec<ImageChunk>
    line_arts:     Vec<LineArtChunk>
}

SemanticTable {
    bbox:         BoundingBox
    index:        Option<u32>
    level:        Option<String>
    semantic_type:SemanticType
    table_border: TableBorder
}

SemanticFormula {
    bbox:  BoundingBox
    index: Option<u32>
    level: Option<String>
    latex: String
}

SemanticPicture {
    bbox:        BoundingBox
    index:       Option<u32>
    level:       Option<String>
    image_index: u32
    description: String
}
```

---

## 06 · Table Types

**Source:** [`models/table.rs`](../crates/edgeparse-core/src/models/table.rs)

```
TableBorder {
    bbox:             BoundingBox
    index:            Option<u32>
    level:            Option<String>
    x_coordinates:    Vec<f64>         // N+1 column boundary X positions
    x_widths:         Vec<f64>         // border line widths at each X
    y_coordinates:    Vec<f64>         // M+1 row boundary Y positions
    y_widths:         Vec<f64>         // border line widths at each Y
    rows:             Vec<TableBorderRow>
    num_rows:         usize            // M
    num_columns:      usize            // N
    is_bad_table:     bool
    is_table_transformer: bool
    previous_table:   Option<Box<TableBorder>>  // cross-page chain
    next_table:       Option<Box<TableBorder>>  // cross-page chain
}

TableBorderRow {
    bbox:           BoundingBox
    index:          Option<u32>
    level:          Option<String>
    row_number:     usize              // 0-based
    cells:          Vec<TableBorderCell>
    semantic_type:  Option<SemanticType>  // TableHeaders/TableBody/TableFooter
}

TableBorderCell {
    bbox:            BoundingBox
    index:           Option<u32>
    level:           Option<String>
    row_number:      usize             // 0-based
    col_number:      usize             // 0-based
    row_span:        usize
    col_span:        usize
    text_chunks:     Vec<TextChunk>    // assigned by Stage 4b
    semantic_type:   Option<SemanticType>  // TableHeader / TableCell
}
```

**Grid visualisation:**
```
x_coordinates: [50, 200, 350, 500]  (4 values = 3 columns)
y_coordinates: [700, 680, 660, 640] (4 values = 3 rows)

     50   200   350   500
700   ┼─────┼─────┼─────┼
      │ 0,0 │ 0,1 │ 0,2 │
680   ┼─────┼─────┼─────┼
      │ 1,0 │ 1,1 │ 1,2 │
660   ┼─────┼─────┼─────┼
      │ 2,0 │ 2,1 │ 2,2 │
640   ┼─────┼─────┼─────┼
```

---

## 07 · ContentElement Enum (Unified)

**Source:** [`models/content.rs`](../crates/edgeparse-core/src/models/content.rs)

```rust
pub enum ContentElement {
    // Raw (early pipeline)
    TextChunk(TextChunk),
    Image(ImageChunk),
    Line(LineChunk),
    LineArt(LineArtChunk),
    TableBorder(TableBorder),

    // Grouped (mid pipeline)
    TextLine(TextLine),
    TextBlock(TextBlock),
    List(PDFList),

    // Semantic (late pipeline)
    Paragraph(SemanticParagraph),
    Heading(SemanticHeading),
    NumberHeading(SemanticNumberHeading),
    Caption(SemanticCaption),
    HeaderFooter(SemanticHeaderOrFooter),
    Figure(SemanticFigure),
    Formula(SemanticFormula),
    Picture(SemanticPicture),
    Table(SemanticTable),
}
```

The enum implements:
- `bbox()` → `&BoundingBox` (dispatches to inner type)
- `index()` → `Option<u32>`
- `page_number()` → `Option<u32>`
- `set_index(u32)` → used by Stage 13

Early stages produce `TextChunk`/`Line`/`Image`. Late stages produce `Heading`/`Paragraph`/`Table`. The enum spans the full lifecycle in one type.

---

## 08 · SemanticType Enum

**Source:** [`models/enums.rs`](../crates/edgeparse-core/src/models/enums.rs)

```
Document, Div, Paragraph, Span, Table, TableHeaders, TableFooter,
TableBody, TableRow, TableHeader, TableCell, Form, Link, Annot,
Caption, List, ListLabel, ListBody, ListItem, TableOfContent,
TableOfContentItem, Figure, NumberHeading, Heading, Title,
BlockQuote, Note, Header, Footer, Code, Part
```

`is_ignored_standard_type()` returns `true` for `Div`, `Span`, `Form`, `Link`, `Annot` — these are structural PDF tags useful for parsing but suppressed from output.

---

## 09 · PdfDocument (Root Output Type)

**Source:** [`models/document.rs`](../crates/edgeparse-core/src/models/document.rs)

```
PdfDocument {
    file_name:         String
    number_of_pages:   u32
    author:            Option<String>
    title:             Option<String>
    creation_date:     Option<String>
    modification_date: Option<String>
    producer:          Option<String>
    creator:           Option<String>
    subject:           Option<String>
    keywords:          Option<String>
    kids:              Vec<ContentElement>  // all semantic content, reading order
}
```

`metadata_pairs()` returns a `Vec<(&str, &str)>` for rendering — only non-empty fields are included.

---

## 10 · Tagged PDF — McidMap

**Source:** [`tagged/struct_tree.rs`](../crates/edgeparse-core/src/tagged/struct_tree.rs)

For PDFs with accessibility tags (`/StructTreeRoot`), the structure tree is parsed into:

```
McidMap = HashMap<(page_number: u32, mcid: i64), McidTagInfo>

McidTagInfo {
    role:          &'static str    // "heading", "paragraph", "table", ...
    heading_level: Option<u32>    // 1-6 for H/H1-H6 structure nodes
    struct_type:   String          // raw tag, e.g. "H2", "P", "Table"
}
```

`build_mcid_map()` walks the structure tree, resolves the `RoleMap` (custom tag aliases), and populates the map. Stage 12 (`heading_detector`) consults this map for authoritative heading classification when available.

---

## 11 · Page Geometry — PageInfo

**Source:** [`pdf/page_info.rs`](../crates/edgeparse-core/src/pdf/page_info.rs)

```
PageInfo {
    index:       usize         // 0-based position in Vec<PageInfo>
    page_number: u32           // 1-based PDF page number
    media_box:   BoundingBox   // full physical page (PDF /MediaBox)
    crop_box:    BoundingBox   // visible area (/CropBox; defaults to MediaBox)
    rotation:    i64           // 0 | 90 | 180 | 270 degrees
    width:       f64           // media_box.width()
    height:      f64           // media_box.height()
}
```

Used by:
- Stage 2 (`content_filter`) — filter elements outside `crop_box`
- Stage 8 (`header_footer`) — compute header/footer Y thresholds
- Stage 5b (`column_detector`) — page width for column ratio
- Stage 18 (`reading_order`) — XY-Cut page region boundary

---

## Cross-Reference

| Topic | Document |
|-------|---------|
| How types are produced | [02-pipeline.md](02-pipeline.md) |
| Where types originate (parsing) | [04-pdf-extraction.md](04-pdf-extraction.md) |
| How types are serialised | [05-output-formats.md](05-output-formats.md) |
