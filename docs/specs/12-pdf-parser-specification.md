# Spec 12 — Pure-Rust PDF Parser (`edgeparse-parser`)

> **Specification version**: 1.0  
> **Status**: Draft  
> **Scope**: Replace `lopdf` with a purpose-built, modular, pure-Rust PDF parser  
> **Cross-references**: [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) | [05-data-models](05-data-models.md) | [11-verapdf-replacement-analysis](11-verapdf-replacement-analysis.md)

---

## 1. Motivation

### 1.1 Why Replace `lopdf`?

The current Rust implementation depends on `lopdf 0.39.0` for all PDF object access. While lopdf provides adequate low-level primitives, it has critical limitations:

| Problem | Impact |
|---------|--------|
| No structure tree interpretation | 155-heading gap vs Java across PDF/UA test suite |
| No font decoding built-in | 1,090 lines of custom font code in `font.rs` |
| No marked content tracking | Cannot link text chunks to structure elements (MCID) |
| Opaque `Object` enum | Pattern-matching boilerplate throughout 13 source files (108 `lopdf::` references) |
| No role map resolution | Cannot normalize custom structure types to standard PDF tags |
| No content stream typing | Raw string operator names instead of typed enum variants |

### 1.2 Design Goals

1. **Zero external C/C++ dependencies** — pure Rust, no FFI bridges
2. **Modular architecture** — each concern in its own module with a clean public API
3. **Structure tree first-class** — read `StructTreeRoot`, walk `StructElem` trees, resolve role maps, link MCIDs
4. **Typed content stream** — enum-based operators, not string matching
5. **Font subsystem** — CMap/ToUnicode/encoding built into the parser, not layered on top
6. **Drop-in replacement** — expose an adapter layer so existing `pdf/*.rs` modules can migrate incrementally
7. **Performance** — zero-copy where possible, lazy stream decompression, parallel page extraction

### 1.3 Non-Goals

- PDF writing/modification (will remain in a separate `metadata_writer` module using lopdf until a later phase)
- PDF rendering to bitmaps
- PDF/A or PDF/UA validation (that's the pipeline's job)
- JavaScript execution or multimedia handling

---

## 2. Architecture Overview

```
edgeparse-parser (new crate)
├── raw/                  # Layer 0: Binary PDF parsing
│   ├── lexer.rs          # Tokenizer: PDF tokens from byte stream
│   ├── xref.rs           # Cross-reference table/stream parser
│   ├── parser.rs         # Object parser: tokens → PdfObject tree
│   └── decrypt.rs        # PDF decryption (RC4, AES-128, AES-256)
│
├── object/               # Layer 1: Typed PDF object model
│   ├── types.rs          # PdfObject enum, Dictionary, Array, Stream, Reference
│   ├── resolver.rs       # Object dereference & indirect object resolution
│   └── document.rs       # PdfDocument: object store + trailer + catalog access
│
├── page/                 # Layer 2: Page-level access
│   ├── tree.rs           # Page tree traversal (inheritable attributes)
│   ├── geometry.rs       # MediaBox, CropBox, Rotation, coordinate transforms
│   └── resources.rs      # Font/XObject/ExtGState/ColorSpace resource resolution
│
├── content/              # Layer 3: Content stream interpretation
│   ├── operator.rs       # Typed enum for all PDF operators (80+ variants)
│   ├── decoder.rs        # Content stream bytes → Vec<Operation>
│   ├── interpreter.rs    # Graphics state machine (CTM, text state, color)
│   └── marked.rs         # BMC/BDC/EMC tracking → MCID extraction
│
├── font/                 # Layer 4: Font subsystem
│   ├── types.rs          # Font, CIDFont, Type0Font, FontDescriptor
│   ├── cmap.rs           # CMap parser (ToUnicode, predefined CMaps)
│   ├── encoding.rs       # WinAnsi, MacRoman, PDFDoc, Differences arrays
│   ├── metrics.rs        # Glyph widths, standard 14 font metrics
│   ├── type1.rs          # Type1 font program encoding extraction
│   ├── truetype.rs       # TrueType/OpenType cmap table reading (via ttf-parser)
│   └── glyph_names.rs    # Adobe Glyph List + TeX CM extras → Unicode
│
├── structure/            # Layer 5: Structure tree (Tagged PDF)
│   ├── tree.rs           # StructTreeRoot parsing, StructElem walking
│   ├── role_map.rs       # RoleMap resolution (custom tags → standard types)
│   ├── mcid_map.rs       # MCID → StructElem lookup table construction
│   └── classify.rs       # Standard structure type → semantic role classification
│
├── stream/               # Layer 6: Stream decompression
│   ├── filter.rs         # FlateDecode, LZWDecode, ASCII85Decode, ASCIIHexDecode
│   ├── predictor.rs      # PNG/TIFF predictor unfiltering
│   └── image.rs          # DCTDecode (JPEG), JPXDecode (JPEG2000), CCITTFaxDecode
│
└── lib.rs                # Public API surface
```

### 2.1 Crate Topology

```
edgeparse-parser          ← new crate (this spec)
  └── dependencies: flate2, ttf-parser, thiserror, log
  └── dev-dependencies: pretty_assertions

edgeparse-core            ← existing crate
  └── removes: lopdf dependency
  └── adds: edgeparse-parser dependency
  └── pdf/*.rs modules migrate to use edgeparse-parser types
```

---

## 3. Layer 0 — Binary PDF Parsing (`raw/`)

### 3.1 Lexer (`raw/lexer.rs`)

The lexer converts a byte stream into a sequence of PDF tokens.

**Token types:**

```rust
pub enum Token<'a> {
    /// `true` or `false`
    Boolean(bool),
    /// Integer value (i64)
    Integer(i64),
    /// Real number (f64)
    Real(f64),
    /// Literal string `(...)` — raw bytes, not yet decoded
    LiteralString(&'a [u8]),
    /// Hex string `<...>` — raw hex chars, not yet decoded
    HexString(&'a [u8]),
    /// Name object `/SomeName`
    Name(&'a [u8]),
    /// Array start `[`
    ArrayStart,
    /// Array end `]`
    ArrayEnd,
    /// Dictionary start `<<`
    DictStart,
    /// Dictionary end `>>`
    DictEnd,
    /// Stream keyword
    StreamBegin,
    /// endstream keyword
    StreamEnd,
    /// `obj` keyword (preceded by `<gen> <num>`)
    Obj,
    /// `endobj` keyword
    EndObj,
    /// `R` keyword (indirect reference, preceded by `<gen> <num>`)
    Reference,
    /// `null`
    Null,
    /// Cross-reference keyword `xref`
    Xref,
    /// Trailer keyword `trailer`
    Trailer,
    /// `startxref`
    StartXref,
    /// PDF comment `%...`
    Comment(&'a [u8]),
    /// End of file
    Eof,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| L-01 | Zero-copy: tokens borrow from the input `&[u8]` slice |
| L-02 | Handle nested parentheses in literal strings: `(a (b) c)` |
| L-03 | Handle escape sequences in literal strings: `\n`, `\r`, `\t`, `\\`, `\(`, `\)`, octal `\ddd` |
| L-04 | Handle hex strings with whitespace: `<48 65 6C 6C 6F>` |
| L-05 | Handle `#xx` hex escapes in name objects: `/A#20B` → `A B` |
| L-06 | Skip comments (`%` to end of line) |
| L-07 | Handle PDF header: `%PDF-1.x` or `%PDF-2.0` |
| L-08 | Support reading backwards from EOF to find `startxref` |

### 3.2 Cross-Reference Parser (`raw/xref.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| X-01 | Parse traditional xref table format (`xref\n0 6\n0000000000 65535 f\n...`) |
| X-02 | Parse cross-reference streams (PDF 1.5+ — stream with `/Type /XRef`) |
| X-03 | Handle hybrid xref (table + stream fallback) |
| X-04 | Follow `/Prev` links to build complete xref from incremental updates |
| X-05 | Handle linearized PDFs (first-page xref) |
| X-06 | Repair broken xref by scanning for `\d+ \d+ obj` patterns (fallback) |

**Output type:**

```rust
pub struct XrefTable {
    /// Map from ObjectId → (byte_offset, generation)
    entries: HashMap<ObjectId, XrefEntry>,
}

pub enum XrefEntry {
    /// Object at byte offset in file
    InUse { offset: u64, generation: u16 },
    /// Object inside an object stream
    Compressed { stream_obj: u32, index: u16 },
    /// Freed object
    Free { next_free: u32, generation: u16 },
}
```

### 3.3 Object Parser (`raw/parser.rs`)

Parses raw bytes at a given offset into a `PdfObject`.

**Requirements:**

| Req | Description |
|-----|-------------|
| P-01 | Parse all 8 basic object types: Boolean, Integer, Real, String, Name, Array, Dictionary, Stream |
| P-02 | Parse indirect objects: `<num> <gen> obj ... endobj` |
| P-03 | Parse indirect references: `<num> <gen> R` |
| P-04 | Parse object streams (`/Type /ObjStm`) — decompress, then parse contained objects |
| P-05 | Handle streams: read `stream` keyword, use `/Length` to determine data extent |
| P-06 | Lazy parsing: only parse an object when first accessed |
| P-07 | Handle PDF string encodings: PDFDocEncoding, UTF-16BE (BOM `\xFE\xFF`), UTF-8 (BOM `\xEF\xBB\xBF`) |

### 3.4 Decryption (`raw/decrypt.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| D-01 | Detect encryption from `/Encrypt` dictionary in trailer |
| D-02 | Support Standard Security Handler (`/Filter /Standard`) |
| D-03 | Revision 2/3/4: RC4 encryption (40-128 bit) |
| D-04 | Revision 5/6: AES-256 encryption (PDF 2.0) |
| D-05 | Auto-decrypt with empty password |
| D-06 | Accept user-provided password |
| D-07 | Per-object decryption using object number + generation as key |
| D-08 | Stream vs string decryption distinction |

**Note:** Encryption is a Phase 2 feature. Phase 1 handles unencrypted PDFs and empty-password decryption via the existing `encryption.rs` adapter.

---

## 4. Layer 1 — Typed Object Model (`object/`)

### 4.1 Core Types (`object/types.rs`)

```rust
/// PDF object identifier: (object number, generation number)
pub type ObjectId = (u32, u16);

/// Core PDF object types (ISO 32000-2:2020 §7.3)
#[derive(Debug, Clone)]
pub enum PdfObject {
    Null,
    Boolean(bool),
    Integer(i64),
    Real(f64),
    Name(Vec<u8>),
    String(PdfString),
    Array(Vec<PdfObject>),
    Dictionary(PdfDict),
    Stream(PdfStream),
    Reference(ObjectId),
}

/// PDF string with encoding awareness
#[derive(Debug, Clone)]
pub struct PdfString {
    /// Raw bytes
    pub bytes: Vec<u8>,
    /// Original format (literal or hex)
    pub format: StringFormat,
}

#[derive(Debug, Clone, Copy)]
pub enum StringFormat {
    Literal,
    Hex,
}

/// PDF dictionary — ordered map of Name → PdfObject
#[derive(Debug, Clone)]
pub struct PdfDict {
    entries: Vec<(Vec<u8>, PdfObject)>,
}

/// PDF stream — dictionary + raw data
#[derive(Debug, Clone)]
pub struct PdfStream {
    pub dict: PdfDict,
    /// Raw (possibly compressed) data
    pub data: Vec<u8>,
}
```

**`PdfDict` API requirements:**

| Req | Description |
|-----|-------------|
| OD-01 | `get(&self, key: &[u8]) -> Option<&PdfObject>` |
| OD-02 | `get_name(&self, key: &[u8]) -> Option<&[u8]>` — shorthand for Name extraction |
| OD-03 | `get_i64(&self, key: &[u8]) -> Option<i64>` — Integer or Real→i64 |
| OD-04 | `get_f64(&self, key: &[u8]) -> Option<f64>` — Integer→f64 or Real |
| OD-05 | `get_str(&self, key: &[u8]) -> Option<String>` — String→decoded UTF-8 |
| OD-06 | `get_bool(&self, key: &[u8]) -> Option<bool>` |
| OD-07 | `get_array(&self, key: &[u8]) -> Option<&[PdfObject]>` |
| OD-08 | `get_dict(&self, key: &[u8]) -> Option<&PdfDict>` |
| OD-09 | `get_reference(&self, key: &[u8]) -> Option<ObjectId>` |
| OD-10 | Iteration: `iter(&self) -> impl Iterator<Item = (&[u8], &PdfObject)>` |

**`PdfObject` convenience methods:**

| Method | Returns |
|--------|---------|
| `as_i64()` | `Option<i64>` |
| `as_f64()` | `Option<f64>` |
| `as_bool()` | `Option<bool>` |
| `as_name()` | `Option<&[u8]>` |
| `as_str()` | `Option<String>` |
| `as_array()` | `Option<&[PdfObject]>` |
| `as_dict()` | `Option<&PdfDict>` |
| `as_stream()` | `Option<&PdfStream>` |
| `as_reference()` | `Option<ObjectId>` |
| `is_null()` | `bool` |

### 4.2 Object Resolver (`object/resolver.rs`)

The resolver dereferences indirect references, following chains of references.

```rust
pub struct ObjectResolver {
    /// Raw file data (memory-mapped or loaded)
    data: Vec<u8>,
    /// Cross-reference table
    xref: XrefTable,
    /// Parsed object cache
    cache: HashMap<ObjectId, PdfObject>,
    /// Decryption handler (if encrypted)
    decrypt: Option<DecryptHandler>,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| R-01 | `resolve(&self, obj: &PdfObject) -> PdfObject` — if Reference, dereference; otherwise return as-is |
| R-02 | `get(&self, id: ObjectId) -> Result<&PdfObject>` — fetch and cache an object by ID |
| R-03 | `get_dict(&self, id: ObjectId) -> Result<&PdfDict>` — fetch + type check |
| R-04 | Cycle detection: track visited IDs during resolution, return error on cycle |
| R-05 | Lazy decompression: stream data decompressed only on first `PdfStream::decoded_data()` call |
| R-06 | Object stream unpacking: decompress `/Type /ObjStm` and parse contained objects |

### 4.3 Document (`object/document.rs`)

```rust
pub struct PdfDocument {
    resolver: ObjectResolver,
    trailer: PdfDict,
    version: PdfVersion,
}
```

**Public API:**

| Method | Description |
|--------|-------------|
| `open(path: &Path) -> Result<PdfDocument>` | Load from file path |
| `open_with_password(path, pwd) -> Result<PdfDocument>` | Load with decryption password |
| `from_bytes(data: Vec<u8>) -> Result<PdfDocument>` | Load from in-memory bytes |
| `version() -> PdfVersion` | PDF version (1.0–2.0) |
| `trailer() -> &PdfDict` | Trailer dictionary |
| `catalog() -> Result<&PdfDict>` | Document catalog (`/Root`) |
| `page_count() -> u32` | Number of pages |
| `page_ids() -> Vec<(u32, ObjectId)>` | Ordered (page_number, page_object_id) pairs |
| `resolve(&self, obj: &PdfObject) -> PdfObject` | Dereference an object |
| `get_object(&self, id: ObjectId) -> Result<&PdfObject>` | Get object by ID |
| `get_dict(&self, id: ObjectId) -> Result<&PdfDict>` | Get dictionary by ID |

---

## 5. Layer 2 — Page Access (`page/`)

### 5.1 Page Tree (`page/tree.rs`)

Traverses the `/Pages` tree to enumerate all pages with inherited attributes.

**Requirements:**

| Req | Description |
|-----|-------------|
| PT-01 | Walk `/Pages` → `/Kids` → `/Page` recursively |
| PT-02 | Inherit `/MediaBox`, `/CropBox`, `/Rotate`, `/Resources` from parent nodes |
| PT-03 | Return pages sorted by document order (not object ID order) |
| PT-04 | Handle deep nesting (balanced page trees in large documents) |

### 5.2 Page Geometry (`page/geometry.rs`)

```rust
pub struct PageGeometry {
    pub page_number: u32,
    pub media_box: Rect,
    pub crop_box: Rect,
    pub bleed_box: Option<Rect>,
    pub trim_box: Option<Rect>,
    pub art_box: Option<Rect>,
    pub rotation: i32,   // 0, 90, 180, 270
    pub width: f64,      // effective width after rotation
    pub height: f64,     // effective height after rotation
}

pub struct Rect {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}
```

### 5.3 Resources (`page/resources.rs`)

Resolves the `/Resources` dictionary for a page, handling inheritance.

**Requirements:**

| Req | Description |
|-----|-------------|
| PR-01 | Resolve `/Font` sub-dictionary → map of font names to font objects |
| PR-02 | Resolve `/XObject` sub-dictionary → map of XObject names to object IDs |
| PR-03 | Resolve `/ExtGState` sub-dictionary → graphics state parameter dictionaries |
| PR-04 | Resolve `/ColorSpace` sub-dictionary |
| PR-05 | Handle resource inheritance from parent `/Pages` nodes |

---

## 6. Layer 3 — Content Stream Interpretation (`content/`)

### 6.1 Typed Operators (`content/operator.rs`)

Replace string-based operator matching with a typed enum. This eliminates the `match op.operator.as_str()` pattern used 40+ times in the current codebase.

```rust
/// All PDF content stream operators (ISO 32000-2 §8/9)
#[derive(Debug, Clone)]
pub enum Op {
    // --- Graphics state ---
    SaveState,                                    // q
    RestoreState,                                 // Q
    ConcatMatrix { a: f64, b: f64, c: f64, d: f64, e: f64, f: f64 }, // cm
    SetLineWidth(f64),                            // w
    SetLineCap(i32),                              // J
    SetLineJoin(i32),                             // j
    SetMiterLimit(f64),                           // M
    SetDashPattern { array: Vec<f64>, phase: f64 }, // d
    SetFlatness(f64),                             // i
    SetGraphicsState(Vec<u8>),                    // gs (name)

    // --- Path construction ---
    MoveTo { x: f64, y: f64 },                   // m
    LineTo { x: f64, y: f64 },                   // l
    CurveTo { x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64 }, // c
    CurveToV { x2: f64, y2: f64, x3: f64, y3: f64 }, // v
    CurveToY { x1: f64, y1: f64, x3: f64, y3: f64 }, // y
    ClosePath,                                    // h
    Rectangle { x: f64, y: f64, w: f64, h: f64 }, // re

    // --- Path painting ---
    Stroke,                                       // S
    CloseAndStroke,                                // s
    Fill,                                         // f
    FillEvenOdd,                                  // f*
    FillAndStroke,                                // B
    FillAndStrokeEvenOdd,                         // B*
    CloseFillAndStroke,                            // b
    CloseFillAndStrokeEvenOdd,                     // b*
    EndPath,                                      // n

    // --- Clipping ---
    ClipNonZero,                                  // W
    ClipEvenOdd,                                  // W*

    // --- Text objects ---
    BeginText,                                    // BT
    EndText,                                      // ET

    // --- Text state ---
    SetCharSpacing(f64),                          // Tc
    SetWordSpacing(f64),                          // Tw
    SetHorizontalScaling(f64),                    // Tz
    SetLeading(f64),                              // TL
    SetFont { name: Vec<u8>, size: f64 },         // Tf
    SetRenderMode(i32),                           // Tr
    SetRise(f64),                                 // Ts

    // --- Text positioning ---
    MoveTextPos { tx: f64, ty: f64 },             // Td
    MoveTextPosSetLeading { tx: f64, ty: f64 },   // TD
    SetTextMatrix { a: f64, b: f64, c: f64, d: f64, e: f64, f: f64 }, // Tm
    NextLine,                                     // T*

    // --- Text showing ---
    ShowText(Vec<u8>),                            // Tj
    ShowTextAdjusted(Vec<TextArrayItem>),          // TJ
    NextLineShowText(Vec<u8>),                    // '
    SetSpacingNextLineShowText { aw: f64, ac: f64, text: Vec<u8> }, // "

    // --- Color ---
    SetStrokeGray(f64),                           // G
    SetFillGray(f64),                             // g
    SetStrokeRgb { r: f64, g: f64, b: f64 },     // RG
    SetFillRgb { r: f64, g: f64, b: f64 },       // rg
    SetStrokeCmyk { c: f64, m: f64, y: f64, k: f64 }, // K
    SetFillCmyk { c: f64, m: f64, y: f64, k: f64 },   // k
    SetStrokeColorSpace(Vec<u8>),                 // CS
    SetFillColorSpace(Vec<u8>),                   // cs
    SetStrokeColor(Vec<f64>),                     // SC / SCN
    SetFillColor(Vec<f64>),                       // sc / scn

    // --- XObject ---
    PaintXObject(Vec<u8>),                        // Do

    // --- Inline image ---
    InlineImage { dict: PdfDict, data: Vec<u8> }, // BI/ID/EI

    // --- Marked content ---
    BeginMarkedContent(Vec<u8>),                  // BMC
    BeginMarkedContentWithProps { tag: Vec<u8>, properties: MarkedContentProps }, // BDC
    EndMarkedContent,                              // EMC
    DefineMarkedContentPoint(Vec<u8>),             // MP
    DefineMarkedContentPointWithProps { tag: Vec<u8>, properties: MarkedContentProps }, // DP

    // --- Compatibility ---
    BeginCompat,                                  // BX
    EndCompat,                                    // EX

    /// Unknown/unsupported operator
    Unknown { operator: String, operands: Vec<PdfObject> },
}

/// Item in a TJ array
#[derive(Debug, Clone)]
pub enum TextArrayItem {
    /// Text string
    Text(Vec<u8>),
    /// Numeric adjustment (thousandths of text space unit)
    Adjustment(f64),
}

/// Properties for marked content (BDC/DP)
#[derive(Debug, Clone)]
pub enum MarkedContentProps {
    /// Inline property dictionary
    Dict(PdfDict),
    /// Reference to a properties dictionary in page resources
    Name(Vec<u8>),
}
```

### 6.2 Content Decoder (`content/decoder.rs`)

Transforms raw content stream bytes into `Vec<Op>`.

**Requirements:**

| Req | Description |
|-----|-------------|
| CD-01 | Parse all operators listed in the `Op` enum above |
| CD-02 | Handle inline images (BI/ID/EI) — detect data boundary correctly |
| CD-03 | Handle interleaved operands and operators |
| CD-04 | Graceful recovery from malformed operators (skip to next valid operator) |
| CD-05 | Performance: decode 1 MB content stream in < 5 ms |

### 6.3 Content Interpreter (`content/interpreter.rs`)

Maintains the graphics state machine and dispatches operations to an output handler.

```rust
pub trait ContentVisitor {
    /// Called for each positioned text run
    fn text(&mut self, text: &[u8], state: &GraphicsState);
    /// Called for each TJ adjustment
    fn text_adjustment(&mut self, displacement: f64, state: &GraphicsState);
    /// Called for path stroke
    fn stroke(&mut self, path: &Path, state: &GraphicsState);
    /// Called for path fill
    fn fill(&mut self, path: &Path, state: &GraphicsState);
    /// Called for XObject references
    fn xobject(&mut self, name: &[u8], state: &GraphicsState);
    /// Called for inline images
    fn inline_image(&mut self, dict: &PdfDict, data: &[u8], state: &GraphicsState);
    /// Called when marked content begins
    fn begin_marked_content(&mut self, tag: &[u8], mcid: Option<i64>, state: &GraphicsState);
    /// Called when marked content ends
    fn end_marked_content(&mut self, state: &GraphicsState);
}
```

**`GraphicsState` exposed to visitors:**

```rust
pub struct GraphicsState {
    pub ctm: Matrix,
    pub text_matrix: Matrix,
    pub text_line_matrix: Matrix,
    pub font_name: Vec<u8>,
    pub font_size: f64,
    pub char_spacing: f64,
    pub word_spacing: f64,
    pub horizontal_scaling: f64,
    pub leading: f64,
    pub rise: f64,
    pub render_mode: i32,
    pub fill_color: Color,
    pub stroke_color: Color,
    pub line_width: f64,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| CI-01 | Maintain full q/Q graphics state stack |
| CI-02 | Track CTM through `cm` operators |
| CI-03 | Track text matrix through Td, TD, Tm, T* |
| CI-04 | Compute effective text position: Tsm × Tm × CTM |
| CI-05 | Track MCID from BDC properties dictionary (`/MCID` key) |
| CI-06 | Recursively process Form XObjects (Do operator with form XObject) |
| CI-07 | Apply form XObject's `/Matrix` to CTM before processing |

### 6.4 Marked Content Tracker (`content/marked.rs`)

Tracks the current marked content stack during content stream interpretation.

```rust
pub struct MarkedContentTracker {
    /// Stack of active marked content tags
    stack: Vec<MarkedContentEntry>,
}

pub struct MarkedContentEntry {
    pub tag: Vec<u8>,
    pub mcid: Option<i64>,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| MC-01 | Push on BMC/BDC, pop on EMC |
| MC-02 | Extract MCID from BDC property dictionary |
| MC-03 | Provide `current_mcid() -> Option<i64>` for the text extractor to tag chunks |
| MC-04 | Handle nested marked content (MCID comes from innermost BDC with `/MCID`) |

---

## 7. Layer 4 — Font Subsystem (`font/`)

### 7.1 Font Types (`font/types.rs`)

```rust
pub struct PdfFont {
    pub base_font: String,
    pub subtype: FontSubtype,
    pub encoding: FontEncoding,
    pub to_unicode: Option<CMapTable>,
    pub widths: GlyphWidths,
    pub descriptor: Option<FontDescriptor>,
    pub is_vertical: bool,
}

pub enum FontSubtype {
    Type1,
    TrueType,
    Type3,
    Type0 { descendant: Box<CIDFontInfo> },
    MMType1,
    CIDFontType0,
    CIDFontType2,
}

pub struct CIDFontInfo {
    pub subtype: FontSubtype,
    pub cid_to_gid: Option<Vec<u16>>,
    pub default_width: f64,
    pub widths: GlyphWidths,
    pub descriptor: Option<FontDescriptor>,
}

pub struct FontDescriptor {
    pub ascent: f64,
    pub descent: f64,
    pub cap_height: f64,
    pub flags: u32,
    pub italic_angle: f64,
    pub font_weight: f64,
    pub font_bbox: [f64; 4],
}
```

### 7.2 CMap Parser (`font/cmap.rs`)

Parses ToUnicode CMaps and predefined CID CMaps.

**Requirements:**

| Req | Description |
|-----|-------------|
| CM-01 | Parse `beginbfchar` / `endbfchar` single-char mappings |
| CM-02 | Parse `beginbfrange` / `endbfrange` range mappings |
| CM-03 | Parse `begincodespacerange` / `endcodespacerange` |
| CM-04 | Handle multi-byte CID ranges |
| CM-05 | Handle UTF-16BE encoded Unicode targets |
| CM-06 | Handle array-form range targets: `<0001> <0003> [<0041> <0042> <0043>]` |
| CM-07 | Handle predefined CMap names (Identity-H, Identity-V, etc.) |

### 7.3 Encoding Resolution (`font/encoding.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| FE-01 | Built-in WinAnsiEncoding table (256 entries → Unicode) |
| FE-02 | Built-in MacRomanEncoding table |
| FE-03 | Built-in PDFDocEncoding table |
| FE-04 | Built-in StandardEncoding table |
| FE-05 | Built-in MacExpertEncoding table |
| FE-06 | `/Differences` array processing — sparse overrides on top of base encoding |
| FE-07 | Glyph name → Unicode via Adobe Glyph List |
| FE-08 | Glyph name → Unicode via TeX Computer Modern extensions |

### 7.4 Glyph Width Metrics (`font/metrics.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| FM-01 | `/Widths` array for simple fonts (FirstChar/LastChar indexed) |
| FM-02 | `/W` array for CIDFonts (CID-keyed width entries) |
| FM-03 | `/DW` (default width) for CIDFonts |
| FM-04 | Built-in width tables for the 14 standard PDF fonts |
| FM-05 | `/MissingWidth` from FontDescriptor as fallback |

### 7.5 Type1 Font Program (`font/type1.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| FT1-01 | Extract `/Encoding` array from Type1 font program (PFB/PFA) |
| FT1-02 | Decrypt Type1 eexec section (Lenient IV=4 decryption) |
| FT1-03 | Parse charstring names for glyph mapping |

### 7.6 TrueType/OpenType (`font/truetype.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| FTT-01 | Read `cmap` table via `ttf-parser` for glyph ID → Unicode |
| FTT-02 | Read `hmtx` table for glyph advance widths |
| FTT-03 | Read `name` table for font name/family |
| FTT-04 | Read `OS/2` table for weight class, ascent/descent |
| FTT-05 | Handle font programs embedded in `/FontFile2` (TrueType) and `/FontFile3` (CFF/OpenType) |

### 7.7 Adobe Glyph List (`font/glyph_names.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| GA-01 | Load Adobe Glyph List (4,300 entries) at compile time via `include_str!` |
| GA-02 | Parse `uniXXXX` name convention: `uni0041` → `A` |
| GA-03 | Parse `uXXXX` / `uXXXXX` name convention |
| GA-04 | TeX Computer Modern glyph name extensions (CMSY, CMEX, CMMI, CMR extras) |

---

## 8. Layer 5 — Structure Tree (`structure/`)

This is the **critical new capability** that lopdf lacks entirely.

### 8.1 Tree Parser (`structure/tree.rs`)

```rust
pub struct StructureTree {
    pub root: StructNode,
    /// Role map: custom type → standard type
    pub role_map: HashMap<Vec<u8>, Vec<u8>>,
}

pub struct StructNode {
    /// Structure type as raw name bytes (e.g., b"H1", b"P", b"Table")
    pub struct_type: Vec<u8>,
    /// Resolved standard type after role map application
    pub standard_type: StandardStructType,
    /// Actual text (/ActualText)
    pub actual_text: Option<String>,
    /// Alternative text (/Alt)
    pub alt_text: Option<String>,
    /// Language (/Lang)
    pub lang: Option<String>,
    /// Page reference
    pub page_id: Option<ObjectId>,
    /// Children: MCIDs or nested StructElems
    pub children: Vec<StructChild>,
}

pub enum StructChild {
    /// Marked content reference
    Mcr(MarkedContentRef),
    /// Nested structure element
    Element(StructNode),
    /// Object reference (/OBJR)
    ObjRef(ObjectId),
}

pub struct MarkedContentRef {
    /// Marked content ID
    pub mcid: i64,
    /// Page the content is on (from /Pg or inherited)
    pub page_id: Option<ObjectId>,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| ST-01 | Read `/StructTreeRoot` from document catalog |
| ST-02 | Parse `/K` entry: integer MCID, dictionary (MCR or StructElem), or array of those |
| ST-03 | Distinguish MCR dictionaries (`/Type /MCR` with `/MCID` and `/Pg`) from StructElem dictionaries (`/S` key) |
| ST-04 | Recurse through all StructElem children |
| ST-05 | Cycle detection: track visited ObjectIds |
| ST-06 | Inherit `/Pg` (page reference) from parent StructElem when child lacks it |
| ST-07 | Handle `/K` being a single integer (leaf MCID), single dict, or array |

### 8.2 Role Map Resolution (`structure/role_map.rs`)

```rust
pub fn resolve_role(
    raw_type: &[u8],
    role_map: &HashMap<Vec<u8>, Vec<u8>>,
) -> Vec<u8>
```

**Requirements:**

| Req | Description |
|-----|-------------|
| RM-01 | Read `/RoleMap` dictionary from `StructTreeRoot` |
| RM-02 | Resolve custom types to standard types: e.g., `"Title"` → `"H1"` |
| RM-03 | Follow chains: if mapped value is itself mapped, resolve transitively (max depth: 10) |
| RM-04 | Return original type if not in role map (already standard) |

### 8.3 MCID Lookup Table (`structure/mcid_map.rs`)

This is the key integration with the text extraction pipeline.

```rust
pub struct McidMap {
    /// (page_object_id, mcid) → semantic info about the structure element
    entries: HashMap<(ObjectId, i64), McidInfo>,
}

pub struct McidInfo {
    /// Resolved standard structure type
    pub struct_type: StandardStructType,
    /// Heading level (1-6) if heading, None otherwise
    pub heading_level: Option<u8>,
    /// Actual text override
    pub actual_text: Option<String>,
    /// Language
    pub lang: Option<String>,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| MM-01 | Build from the parsed `StructureTree` by walking all leaves |
| MM-02 | Map `(page_id, mcid)` → structure element info |
| MM-03 | For heading types (H, H1-H6), set `heading_level` appropriately |
| MM-04 | For H (unnumbered heading), infer level from nesting depth or default to 1 |

### 8.4 Standard Type Classification (`structure/classify.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardStructType {
    Document,
    Part, Art, Sect, Div,
    P,
    H, H1, H2, H3, H4, H5, H6,
    L, LI, Lbl, LBody,
    Table, TR, TH, TD, THead, TBody, TFoot, Caption,
    Figure,
    Formula,
    Form,
    Span,
    Link,
    Note,
    Reference,
    BibEntry,
    Code,
    BlockQuote,
    TOC, TOCI,
    Index,
    NonStruct,
    Ruby, RB, RT, RP, Warichu, WT, WP,
    Unknown,
}
```

**Requirements:**

| Req | Description |
|-----|-------------|
| SC-01 | Map raw name bytes to `StandardStructType` enum |
| SC-02 | Apply role map first, then classify the resolved type |
| SC-03 | Case-sensitive matching (PDF names are case-sensitive) |
| SC-04 | Return `Unknown` for unrecognized types |

---

## 9. Layer 6 — Stream Decompression (`stream/`)

### 9.1 Filter Chain (`stream/filter.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| SF-01 | `FlateDecode` — zlib/deflate decompression (via `flate2` crate) |
| SF-02 | `LZWDecode` — LZW decompression |
| SF-03 | `ASCII85Decode` — ASCII base-85 decoding |
| SF-04 | `ASCIIHexDecode` — hex string decoding |
| SF-05 | `RunLengthDecode` — PackBits-style run-length decoding |
| SF-06 | Chained filters: apply in order specified by `/Filter` array |
| SF-07 | Handle both single filter (Name) and filter array (Array) in stream dict |

### 9.2 Predictor Unfiltering (`stream/predictor.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| SP-01 | TIFF Predictor 2 (horizontal differencing) |
| SP-02 | PNG predictors: None (0), Sub (1), Up (2), Average (3), Paeth (4) |
| SP-03 | Optimum PNG predictor (per-row selector byte) |
| SP-04 | Read `/DecodeParms` for Columns, Colors, BitsPerComponent, Predictor |

### 9.3 Image Stream Handling (`stream/image.rs`)

**Requirements:**

| Req | Description |
|-----|-------------|
| SI-01 | `DCTDecode` — JPEG passthrough (return raw JPEG data) |
| SI-02 | `JPXDecode` — JPEG2000 passthrough |
| SI-03 | `CCITTFaxDecode` — Group 3/4 fax data passthrough (for downstream processing) |
| SI-04 | `JBIG2Decode` — JBIG2 passthrough |

**Note:** Image data is passed through as-is for the image extractor. Actual decoding to pixels happens in the pipeline's image processing stage.

---

## 10. Migration Strategy

### 10.1 Phased Approach

| Phase | Scope | Files Affected | Risk |
|-------|-------|----------------|------|
| **Phase 1** | New crate skeleton + object model + xref + lexer | New crate only | Low |
| **Phase 2** | Content stream decoder + typed operators | New crate only | Low |
| **Phase 3** | Font subsystem (port from `font.rs`) | New crate + `font.rs` | Medium |
| **Phase 4** | Structure tree + MCID map | New crate + `tagged/` | Medium |
| **Phase 5** | Stream decompression | New crate | Low |
| **Phase 6** | Adapter layer — make `edgeparse-core` use `edgeparse-parser` | `pdf/*.rs`, `lib.rs` | High |
| **Phase 7** | Remove `lopdf` dependency | `Cargo.toml`, all `pdf/*.rs` | High |

### 10.2 Adapter Pattern for Incremental Migration

During Phase 6, create an adapter module that exposes lopdf-compatible types:

```rust
// pdf/compat.rs — temporary adapter during migration
pub use edgeparse_parser::object::types::PdfObject;
pub use edgeparse_parser::object::types::PdfDict as Dictionary;
pub use edgeparse_parser::object::types::PdfStream as Stream;
pub use edgeparse_parser::object::types::ObjectId;
pub use edgeparse_parser::object::document::PdfDocument as Document;
```

Each `pdf/*.rs` module is migrated one at a time:
1. Change imports from `lopdf::*` to `crate::pdf::compat::*`
2. Fix type mismatches (minimal since the API is designed to be similar)
3. Run tests for that module
4. Proceed to next module

### 10.3 Files to Migrate (by lopdf reference count)

| File | lopdf refs | Migration complexity |
|------|------------|---------------------|
| `font.rs` | 31 | **High** — port to new font subsystem |
| `form_extractor.rs` | 11 | Medium |
| `annotation_extractor.rs` | 10 | Medium |
| `text_extractor.rs` | 10 | Medium — rewrite to use `ContentVisitor` |
| `encryption.rs` | 8 | Low — thin wrapper |
| `hyperlink_extractor.rs` | 7 | Medium |
| `metadata_writer.rs` | 7 | **Deferred** — writing stays on lopdf for now |
| `line_extractor.rs` | 5 | Medium — rewrite to use `ContentVisitor` |
| `image_extractor.rs` | 5 | Medium |
| `loader.rs` | 5 | Low — replace `Document::load` with `PdfDocument::open` |
| `lib.rs` | 5 | Low — change import + error type |
| `bookmark_extractor.rs` | 2 | Low |
| `page_info.rs` | 2 | Low |

---

## 11. Testing Strategy

### 11.1 Unit Tests per Layer

| Layer | Test approach | Min coverage |
|-------|--------------|-------------|
| Lexer | Token-by-token verification from crafted byte sequences | 95% |
| Xref | Parse real xref sections extracted from test PDFs | 90% |
| Object parser | Round-trip: parse → serialize → parse | 90% |
| Content decoder | Verify Op enum variants from known content streams | 95% |
| Font CMap | Parse known ToUnicode maps, verify char→unicode | 95% |
| Structure tree | Parse tagged PDFs, verify tree shape and MCID mapping | 90% |
| Stream filters | Decompress known compressed data, compare to expected | 95% |

### 11.2 Integration Tests

| Test | Description |
|------|-------------|
| **Smoke test** | Load each of the 13 available test PDFs, extract text, compare to current lopdf-based output |
| **Heading parity** | Run heading detection on PDF/UA suite, compare heading counts to Java output |
| **Round-trip** | Load → extract text → verify character count matches lopdf-based extraction |
| **Encrypted PDF** | Load password-protected test PDFs |
| **Large PDF** | Performance test on 100+ page documents |

### 11.3 Regression Gate

The existing 332 `cargo test` tests **must all pass** after migration. Any heading count changes must be improvements (closer to Java), not regressions.

---

## 12. Dependencies

### 12.1 Required Dependencies

| Crate | Version | Purpose | License |
|-------|---------|---------|---------|
| `flate2` | 1.x | FlateDecode (zlib decompression) | MIT/Apache-2.0 |
| `ttf-parser` | 0.25 | TrueType/OpenType font table reading | MIT/Apache-2.0 |
| `thiserror` | 2.x | Error type derivation | MIT/Apache-2.0 |
| `log` | 0.4 | Logging | MIT/Apache-2.0 |

### 12.2 Optional Dependencies

| Crate | Purpose | When needed |
|-------|---------|-------------|
| `aes` + `cbc` | AES decryption (PDF 2.0 encryption) | Phase 2 (encryption) |
| `md5` | MD5 hash for PDF encryption key derivation | Phase 2 |
| `rc4` | RC4 stream cipher (older PDF encryption) | Phase 2 |
| `sha2` | SHA-256 for PDF 2.0 encryption | Phase 2 |

### 12.3 No Longer Required After Migration

| Crate | Current role | Removed when |
|-------|-------------|-------------|
| `lopdf` | PDF object access, content stream decoding | Phase 7 |
| `pdf-extract` | Not used directly (was considered) | N/A |

---

## 13. Performance Requirements

| Metric | Target | Measurement |
|--------|--------|-------------|
| Cold start (load + xref parse) | < 50 ms for 100-page PDF | `criterion` bench |
| Per-page text extraction | < 5 ms/page | `criterion` bench |
| Memory usage | < 2× PDF file size | RSS measurement |
| Content stream decoding | > 200 MB/s throughput | `criterion` bench |

---

## 14. Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid PDF: {0}")]
    InvalidPdf(String),

    #[error("Unsupported PDF version: {0}")]
    UnsupportedVersion(String),

    #[error("Object not found: {0:?}")]
    ObjectNotFound(ObjectId),

    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: &'static str, actual: String },

    #[error("Decompression error: {0}")]
    Decompression(String),

    #[error("Font error: {0}")]
    Font(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Xref error: {0}")]
    Xref(String),

    #[error("Cycle detected at object {0:?}")]
    Cycle(ObjectId),
}
```

**Integration with `EdgePdfError`:**

```rust
impl From<edgeparse_parser::ParseError> for EdgePdfError {
    fn from(e: edgeparse_parser::ParseError) -> Self {
        EdgePdfError::LopdfError(e.to_string()) // reuse existing variant, rename later
    }
}
```

---

## 15. Public API Summary

The crate exposes these top-level types and functions:

```rust
// Document loading
pub fn open(path: &Path) -> Result<PdfDocument, ParseError>;
pub fn open_with_password(path: &Path, password: &str) -> Result<PdfDocument, ParseError>;
pub fn from_bytes(data: Vec<u8>) -> Result<PdfDocument, ParseError>;

// Document access
pub struct PdfDocument { .. }
impl PdfDocument {
    pub fn page_count(&self) -> u32;
    pub fn page_ids(&self) -> Vec<(u32, ObjectId)>;
    pub fn catalog(&self) -> Result<&PdfDict, ParseError>;
    pub fn trailer(&self) -> &PdfDict;
    pub fn resolve(&self, obj: &PdfObject) -> PdfObject;
    pub fn get_object(&self, id: ObjectId) -> Result<&PdfObject, ParseError>;
}

// Page access
pub fn page_geometries(doc: &PdfDocument) -> Vec<PageGeometry>;
pub fn page_resources(doc: &PdfDocument, page_id: ObjectId) -> PageResources;

// Content stream
pub fn decode_content_stream(data: &[u8]) -> Result<Vec<Op>, ParseError>;
pub fn interpret_page(doc: &PdfDocument, page_id: ObjectId, visitor: &mut dyn ContentVisitor) -> Result<(), ParseError>;

// Font
pub fn resolve_page_fonts(doc: &PdfDocument, page_id: ObjectId) -> FontCache;

// Structure tree
pub fn extract_structure_tree(doc: &PdfDocument) -> Option<StructureTree>;
pub fn build_mcid_map(tree: &StructureTree) -> McidMap;
pub fn is_tagged(doc: &PdfDocument) -> bool;

// Stream decompression
pub fn decompress_stream(stream: &PdfStream) -> Result<Vec<u8>, ParseError>;
```

---

## 16. Open Questions

| # | Question | Impact | Current Assumption |
|---|----------|--------|-------------------|
| 1 | Should `edgeparse-parser` be a workspace crate or a separate published crate? | Package management | Workspace crate (`crates/edgeparse-parser/`) |
| 2 | Should we support PDF writing in the parser, or keep `metadata_writer.rs` on lopdf? | Scope | Keep writing on lopdf for now |
| 3 | Should we memory-map the PDF file for zero-copy lexing? | Performance | Yes, use `memmap2` for files > 10 MB |
| 4 | Should encryption support be optional (feature flag)? | Binary size | Yes, `encryption` feature flag |
| 5 | What is the minimum PDF version to support? | Compatibility | PDF 1.0 through 2.0 |

---

## 17. Acceptance Criteria

The parser is considered complete when:

1. **All 332 existing `cargo test` tests pass** without lopdf in the dependency tree (except `metadata_writer.rs`)
2. **Heading counts match or exceed current Rust output** on all 13 test PDFs
3. **Structure tree extraction works** on all 10 PDF/UA reference suite PDFs
4. **MCID map correctly links** text chunks to structure elements on tagged PDFs
5. **Performance is within 2×** of current lopdf-based implementation on the benchmark suite
6. **No external C/C++ dependencies** (pure Rust + `flate2` which uses Rust miniz_oxide by default)
