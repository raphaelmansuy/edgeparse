# 09 — Rust Migration Guide

> **Cross-references**: [01-overview](01-overview.md) | [03-technical-architecture](03-technical-architecture.md) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) | [05-data-models](05-data-models.md) | [06-cli-interface](06-cli-interface.md) | [07-hybrid-mode](07-hybrid-mode.md) | [08-output-formats](08-output-formats.md)

---

## 1. Recommended Crate Inventory

### 1.1 Core Dependencies

| Crate | Version | Purpose | Replaces (Java) |
|-------|---------|---------|-----------------|
| `lopdf` | 0.39.0 | Low-level PDF object access, reading/writing | veraPDF `PDDocument`, `COSDocument` |
| `pdf` | 0.10.0 | Higher-level PDF reading, font decoding, content streams | veraPDF `GFSAPDFDocument`, `parseChunks` |
| `pdf-extract` | 0.10.0 | Text extraction from PDF pages | veraPDF text chunk extraction |
| `clap` | 4.6.0 | CLI argument parsing with derive macros | Apache Commons CLI |
| `serde` | 1.x | Serialization/deserialization framework | Jackson annotations |
| `serde_json` | 1.0.149 | JSON output generation | Jackson `ObjectMapper` |
| `reqwest` | 0.13.2 | Async HTTP client (hybrid mode) | OkHttp3 |
| `regex` | 1.12.3 | Pattern matching (PII, list labels, numbering) | `java.util.regex` |
| `image` | 0.25.10 | Image decoding/encoding (PNG, JPEG, BMP) | `javax.imageio`, AWT |
| `rayon` | 1.11.0 | Data-parallel document processing | Manual thread management |
| `thiserror` | 2.0.18 | Derive `Error` for library error types | Custom exception hierarchy |
| `anyhow` | 1.x | Error context in application (CLI) code | Exception wrapping |
| `printpdf` | 0.9.1 | Annotated PDF output generation | Apache PDFBox `PDAnnotation` |
| `tokio` | 1.x | Async runtime for HTTP (hybrid mode) | OkHttp async dispatch |
| `log` | 0.4.x | Logging facade | `java.util.logging` |
| `env_logger` | 0.11.x | Console logger impl | JUL ConsoleHandler |
| `base64` | 0.22.x | Base64 encoding for embedded images | `java.util.Base64` |
| `unicode-normalization` | 0.1.x | Unicode NFC normalization | Java `Normalizer` |
| `ordered-float` | 4.x | `f64` keys in BTreeMap (for sorting) | Direct `Double` comparison |
| `indexmap` | 2.x | Insertion-order-preserving maps | `LinkedHashMap` |
| `tiny-skia` | 0.11.x | Page rasterization (contrast ratio calc) | AWT `BufferedImage` rendering |

### 1.2 Crate Selection Rationale

#### PDF Parsing: `lopdf` + `pdf` Dual Strategy

The Java code uses veraPDF which provides:
1. Low-level PDF object access (dictionaries, streams, xrefs)
2. Content stream parsing (text operators: Tj, TJ, Tm, etc.)
3. Font decoding (CMap, ToUnicode mapping)
4. Image extraction (inline/XObject images)
5. Page geometry (MediaBox, CropBox)

No single Rust crate replicates all veraPDF features. The recommended approach:

```
+-- lopdf (0.39.0) -------+     +-- pdf (0.10.0) ---------+
|  PDF object access       |     |  Content stream parsing  |
|  Page tree traversal     |     |  Font decoding + CMap    |
|  Dictionary read/write   |     |  Text position tracking  |
|  Stream decompression    |     |  Type-safe PDF types     |
|  Annotation creation     |     |                          |
|  Encrypted PDF support   |     |                          |
+--------------------------+     +--------------------------+
            |                                |
            v                                v
+-- Custom Extraction Layer --------------------------------+
|  ExtractedPage {                                          |
|    text_chunks: Vec<TextChunk>,                           |
|    image_chunks: Vec<ImageChunk>,                         |
|    line_chunks: Vec<LineChunk>,                           |
|    line_art_chunks: Vec<LineArtChunk>,                    |
|    table_borders: TableBordersCollection,                 |
|    page_bbox: BoundingBox,                                |
|  }                                                        |
+-----------------------------------------------------------+
```

**Why not `pdf-extract` alone?** It provides text extraction but lacks:
- Bounding box tracking per character/word (critical for this project)
- Line segment extraction (needed for table border detection)
- Image extraction with position data
- Font metadata (size, weight, color) per text span

**Why not `lopdf` alone?** It handles PDF object trees but lacks:
- Content stream operator interpretation
- Font CMap / ToUnicode decoding
- Built-in text extraction with position tracking

The `pdf` crate provides strongly-typed PDF structures and content stream parsing.
Use `lopdf` for I/O and annotation writing. Use `pdf` crate for content parsing.

#### HTTP: `reqwest` with `tokio`

Hybrid mode requires HTTP multipart POST. `reqwest` 0.13.2 uses `rustls` by
default (no OpenSSL dependency), supports:
- `multipart::Form` for file upload
- JSON response deserialization via serde
- Connection pooling (`Client` is `Clone + Send + Sync`)
- Configurable timeouts

Since hybrid calls are I/O-bound, use a small `tokio` runtime confined to the
hybrid module. The main pipeline stays synchronous:

```rust
// In hybrid/client.rs
pub fn send_pdf(client: &reqwest::Client, url: &str, pdf_bytes: &[u8])
    -> Result<DoclingResponse, HybridError>
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let form = reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::bytes(pdf_bytes.to_vec())
                .file_name("document.pdf")
                .mime_str("application/pdf")?);
        let resp = client.post(url)
            .multipart(form)
            .timeout(Duration::from_millis(config.timeout_ms))
            .send()
            .await?;
        resp.json::<DoclingResponse>().await.map_err(Into::into)
    })
}
```

#### Parallelism: `rayon` for Batch Processing

The CLI accepts directories. Each PDF is independent. Use rayon's `par_iter`:

```rust
use rayon::prelude::*;

let results: Vec<ProcessResult> = pdf_paths
    .par_iter()
    .map(|path| process_single_pdf(path, &config, &http_client))
    .collect();

let exit_code = if results.iter().any(|r| r.is_err()) { 1 } else { 0 };
```

Within a single document, stages are sequential (each depends on prior output).

---

## 2. Workspace Layout

### 2.1 Cargo Workspace Structure

```
opendataloader-pdf-rust/
+-- Cargo.toml                          # Workspace root
+-- crates/
|   +-- opendataloader-pdf-core/        # Library crate
|   |   +-- Cargo.toml
|   |   +-- src/
|   |       +-- lib.rs                  # Public API surface
|   |       +-- pdf/
|   |       |   +-- mod.rs
|   |       |   +-- loader.rs           # PDF document loading
|   |       |   +-- extractor.rs        # Content stream parsing
|   |       |   +-- font_decoder.rs     # CMap/ToUnicode decoding
|   |       |   +-- image_extractor.rs  # Image XObject extraction
|   |       |   +-- line_extractor.rs   # Path op -> LineChunk
|   |       |   +-- struct_tree.rs      # Structure tree parsing
|   |       |   +-- page_geometry.rs    # MediaBox, CropBox, rotation
|   |       |
|   |       +-- model/
|   |       |   +-- mod.rs
|   |       |   +-- bbox.rs            # BoundingBox, MultiBoundingBox
|   |       |   +-- types.rs           # ContentElement enum
|   |       |   +-- text.rs            # TextChunk, TextLine, TextBlock
|   |       |   +-- semantic.rs        # SemanticType, SemanticNode types
|   |       |   +-- table.rs           # Table, TableBorder, Row, Cell
|   |       |   +-- list.rs            # PDFList, ListItem
|   |       |   +-- image.rs           # ImageChunk
|   |       |   +-- line.rs            # LineChunk, LineArtChunk
|   |       |   +-- document.rs        # Document metadata envelope
|   |       |   +-- config.rs          # Config, FilterConfig, HybridConfig
|   |       |
|   |       +-- pipeline/
|   |       |   +-- mod.rs             # DocumentProcessor orchestrator
|   |       |   +-- context.rs         # ProcessingContext (replaces statics)
|   |       |   +-- content_filter.rs  # 11-step content filter
|   |       |   +-- hidden_text.rs     # Contrast ratio hidden text detector
|   |       |   +-- text_line.rs       # TextChunk -> TextLine grouping
|   |       |   +-- table_cluster.rs   # Cluster-based table detection
|   |       |   +-- table_border.rs    # Border-based table detection
|   |       |   +-- table_special.rs   # Korean special table patterns
|   |       |   +-- header_footer.rs   # Cross-page header/footer detection
|   |       |   +-- list.rs           # Two-pass list detection
|   |       |   +-- paragraph.rs       # 9-pass paragraph merging
|   |       |   +-- heading.rs         # Font statistics heading detection
|   |       |   +-- caption.rs         # Caption-to-element linking
|   |       |   +-- cross_page.rs      # Cross-page table/list linking
|   |       |   +-- level.rs           # Heading level + nesting assignment
|   |       |   +-- reading_order.rs   # XY-Cut++ algorithm
|   |       |   +-- sanitizer.rs       # PII regex sanitization
|   |       |   +-- tagged.rs          # TaggedDocumentProcessor
|   |       |
|   |       +-- hybrid/
|   |       |   +-- mod.rs             # HybridDocumentProcessor
|   |       |   +-- triage.rs          # TriageProcessor (6-signal chain)
|   |       |   +-- client.rs          # HybridClient trait
|   |       |   +-- docling.rs         # DoclingFastServerClient
|   |       |   +-- hancom.rs          # HancomClient
|   |       |   +-- transform.rs       # DoclingSchemaTransformer
|   |       |
|   |       +-- output/
|   |       |   +-- mod.rs
|   |       |   +-- json.rs            # JSON writer (serde custom serializers)
|   |       |   +-- markdown.rs        # Markdown generator
|   |       |   +-- markdown_html.rs   # Markdown with HTML tables
|   |       |   +-- html.rs            # HTML5 semantic output
|   |       |   +-- text.rs            # Plain text output
|   |       |   +-- pdf_annotator.rs   # Annotated PDF writer
|   |       |   +-- image_export.rs    # Image extraction + base64
|   |       |
|   |       +-- utils/
|   |           +-- mod.rs
|   |           +-- statistics.rs      # ModeWeightStatistics
|   |           +-- merge.rs           # Chunk merging utilities
|   |           +-- numbering.rs       # Numbering/bullet detection
|   |           +-- sanitize_rules.rs  # PII regex definitions
|   |           +-- text_style.rs      # TextStyle comparisons
|   |           +-- geometry.rs        # Geometric overlap calculations
|   |
|   +-- opendataloader-pdf-cli/        # Binary crate
|       +-- Cargo.toml
|       +-- src/
|           +-- main.rs               # Entry point + exit codes
|           +-- cli.rs                # clap derive definitions
|           +-- traversal.rs          # File/dir walking + extension filter
|           +-- export_options.rs     # --export-options JSON output
|
+-- options.json                       # Single source of truth
+-- schema.json                        # JSON output schema
+-- scripts/
|   +-- generate-options.mjs           # Code gen for Node/Python wrappers
|   +-- generate-schema.mjs            # Doc gen from schema.json
+-- python/                            # Python wrapper (unchanged)
+-- node/                              # Node.js wrapper (unchanged)
+-- tests/
    +-- benchmark/                     # Benchmark suite
    +-- fixtures/                      # Test PDF files
    +-- integration/                   # Integration tests
```

### 2.2 Cargo.toml (Workspace)

```toml
[workspace]
resolver = "2"
members = [
    "crates/opendataloader-pdf-core",
    "crates/opendataloader-pdf-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1.0"
regex = "1.12"
log = "0.4"
thiserror = "2.0"
anyhow = "1"
```

### 2.3 Core Crate Cargo.toml

```toml
[package]
name = "opendataloader-pdf-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
regex = { workspace = true }
log = { workspace = true }
thiserror = { workspace = true }

lopdf = { version = "0.39", features = ["nom_parser"] }
pdf = "0.10"
reqwest = { version = "0.13", features = ["json", "multipart"] }
tokio = { version = "1", features = ["rt", "macros"] }
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
printpdf = "0.9"
base64 = "0.22"
unicode-normalization = "0.1"
ordered-float = "4"
indexmap = { version = "2", features = ["serde"] }
tiny-skia = "0.11"
```

### 2.4 CLI Crate Cargo.toml

```toml
[package]
name = "opendataloader-pdf"
version.workspace = true
edition.workspace = true

[[bin]]
name = "opendataloader-pdf"
path = "src/main.rs"

[dependencies]
opendataloader-pdf-core = { path = "../opendataloader-pdf-core" }
clap = { version = "4.6", features = ["derive"] }
anyhow = { workspace = true }
log = { workspace = true }
env_logger = "0.11"
rayon = "1.11"
serde_json = { workspace = true }
```

---

## 3. veraPDF Reimplementation Strategy

### 3.1 Overview

The Java codebase depends on ~50 veraPDF types across 9 categories (see
[05-data-models § Type Hierarchy](05-data-models.md)). The veraPDF dependency must
be fully replaced because:

1. veraPDF is Java-only (no Rust bindings possible)
2. It provides both PDF parsing AND layout analysis types
3. The project uses ~30% of veraPDF's capabilities

### 3.2 Category-by-Category Migration

```
+===================================================================+
|  CATEGORY         | STRATEGY             | EFFORT   | PRIORITY   |
|===================================================================|
|  Geometry          | Rewrite from scratch  | Small    | P0 (first)|
|  (BoundingBox,     | Pure math, no deps    |          |            |
|   MultiBoundingBox) |                      |          |            |
|-------------------------------------------------------------------|
|  Enums             | Rewrite from scratch  | Small    | P0         |
|  (SemanticType,    | Direct translation    |          |            |
|   TextAlignment)   |                       |          |            |
|-------------------------------------------------------------------|
|  Content chunks    | Rewrite from scratch  | Medium   | P0         |
|  (TextChunk,       | Struct definitions    |          |            |
|   ImageChunk, etc) |                       |          |            |
|-------------------------------------------------------------------|
|  Text grouping     | Rewrite from scratch  | Medium   | P1         |
|  (TextLine,        | Depends on TextChunk  |          |            |
|   TextBlock)       |                       |          |            |
|-------------------------------------------------------------------|
|  Semantic nodes    | Rewrite from scratch  | Medium   | P1         |
|  (SemanticParagraph| Trait + enum pattern  |          |            |
|   SemanticHeading) |                       |          |            |
|-------------------------------------------------------------------|
|  Tables            | Rewrite from scratch  | Large    | P1         |
|  (TableBorder,     | Complex grid math     |          |            |
|   Row, Cell)       |                       |          |            |
|-------------------------------------------------------------------|
|  Lists             | Rewrite from scratch  | Medium   | P1         |
|  (PDFList,         | Recursive structure   |          |            |
|   ListItem)        |                       |          |            |
|-------------------------------------------------------------------|
|  Utilities         | Rewrite from scratch  | Large    | P2         |
|  (merge, caption,  | Algorithm translation |          |            |
|   numbering utils) |                       |          |            |
|-------------------------------------------------------------------|
|  PDF parsing       | lopdf + pdf crates    | V.Large  | P0 (first)|
|  (GFSAPDFDocument, | Custom extraction     |          |            |
|   ChunkParser)     | layer on top          |          |            |
+===================================================================+
```

### 3.3 PDF Content Extraction (The Hardest Part)

This is the most complex reimplementation because veraPDF's `parseChunks()` method
does extensive work that Rust crates only partially cover.

#### What `parseChunks()` Does

```
PDF Page Content Stream
    |
    v
+-- Font Resolution ----------------------------------------+
|  1. Read /Resources /Font dictionary from page            |
|  2. Enumerate all font objects (Type1, TrueType, CIDFont) |
|  3. Parse /Encoding, /ToUnicode CMap, /Differences array  |
|  4. Build codepoint → Unicode mapping per font            |
+-----------------------------------------------------------+
    |
    v
+-- Text Operator Processing --------------------------------+
|  Operators: BT/ET, Tf, Tm, Td, TD, T*, Tj, TJ, ', "      |
|  For each text-showing operator:                           |
|  1. Decode bytes to Unicode using font CMap                |
|  2. Calculate glyph widths from font metrics               |
|  3. Transform glyph positions through text matrix + CTM    |
|  4. Emit TextChunk with:                                   |
|     - text: String (Unicode)                               |
|     - bbox: BoundingBox (page coordinates)                 |
|     - font_name: String                                    |
|     - font_size: f64                                       |
|     - font_weight: f64 (estimated from font name or OS/2)  |
|     - font_color: (r, g, b)                                |
|     - text_format: TextFormat (bold/italic/neither)        |
+------------------------------------------------------------+
    |
    v
+-- Graphics State Tracking ----------------------------------+
|  Operators: q/Q, cm, gs                                     |
|  Track current transformation matrix (CTM stack)            |
|  Track fill/stroke color (g, rg, k, cs, sc, scn)           |
|  Track line width, dash pattern                             |
+-------------------------------------------------------------+
    |
    v
+-- Path/Line Extraction -------------------------------------+
|  Operators: m, l, re, c, h, S, f, B                        |
|  For each stroked/filled path:                              |
|  1. Classify as horizontal line, vertical line, or shape    |
|  2. Emit LineChunk (h-line/v-line) or LineArtChunk (other)  |
|  3. Detect table border rectangles from 're' operations     |
+-------------------------------------------------------------+
    |
    v
+-- Image Extraction -----------------------------------------+
|  Operators: Do (XObject reference), BI/ID/EI (inline)       |
|  For each image:                                            |
|  1. Read XObject dictionary (Width, Height, BitsPerComponent)|
|  2. Decode image data (DCT, Flate, CCITT, JBIG2)           |
|  3. Apply color space conversion (DeviceRGB, CMYK, ICCBased)|
|  4. Emit ImageChunk with bbox from CTM                      |
+-------------------------------------------------------------+
```

#### Rust Implementation Approach

```
                     lopdf::Document
                           |
                           v
              +-- PageExtractor::extract(page_num) --+
              |                                       |
              |  1. Get page dict via lopdf            |
              |  2. Get /MediaBox, /CropBox            |
              |  3. Get /Resources /Font               |
              |  4. Read content stream bytes           |
              |                                        |
              +---+------------------------------------+
                  |
                  v
    +-- ContentStreamParser (custom) --+
    |                                   |
    |  Tokenize content stream into     |
    |  PDF operators + operands         |
    |  (re-use pdf crate's parser       |
    |   or write minimal tokenizer)     |
    |                                   |
    +---+-------------------------------+
        |
        v
    +-- OperatorInterpreter (custom) --+
    |                                   |
    |  Walk operator sequence:          |
    |  - Track graphics state stack     |
    |  - Track text state (Tm, font)    |
    |  - Decode text → Unicode          |
    |  - Calculate positions via CTM    |
    |  - Emit TextChunk/ImageChunk/etc  |
    |                                   |
    |  Key types:                       |
    |  - GraphicsState { ctm, color,    |
    |      line_width, dash }           |
    |  - TextState { font, size, Tm,    |
    |      Tc, Tw, Th, Tl, Trise }     |
    |  - FontCache { name → FontInfo }  |
    |                                   |
    +-----------------------------------+
```

**Font Decoding Complexity**: This is the single hardest sub-problem. PDF fonts
can use:
- Simple encodings (WinAnsi, MacRoman)
- Differences arrays (custom glyph remapping)
- ToUnicode CMap (stream with bfchar/bfrange entries)
- CIDFont with CID-to-GID mapping
- Identity-H/V encoding with embedded CMap

The `pdf` crate (v0.10.0) has partial font support. Plan to use it for the common
cases and implement custom CMap parsing for edge cases.

### 3.4 Contrast Ratio Calculation (Hidden Text Detection)

Java uses AWT `BufferedImage` rendering to detect hidden text (text color too close
to background). The Rust equivalent:

```
+-- tiny-skia based approach -------------------------------+
|                                                            |
|  1. Render page region behind text chunk                   |
|     - Use tiny-skia to rasterize path/fill operations      |
|     - Render at reduced resolution (72 DPI is sufficient)  |
|                                                            |
|  2. Sample background color at text bbox center            |
|     - Average pixel values in small region                 |
|                                                            |
|  3. Calculate WCAG contrast ratio:                         |
|     L1 = relative_luminance(text_color)                    |
|     L2 = relative_luminance(background_color)              |
|     ratio = (max(L1,L2) + 0.05) / (min(L1,L2) + 0.05)    |
|                                                            |
|  4. If ratio < MIN_CONTRAST_RATIO (1.2):                   |
|     Mark text as hidden                                    |
|                                                            |
+------------------------------------------------------------+
```

**Alternative**: If full-page rendering is too expensive, use a simplified approach:
- Track fill/stroke colors from the graphics state at each text position
- Compare text color against the most recent background fill
- This avoids full rasterization but may miss overlapping elements

The simplified approach is recommended for initial implementation. Port the full
rasterization approach only if the simplified method produces false positives in
benchmarks.

---

## 4. Architecture Patterns

### 4.1 ContentElement: Enum vs Trait Pattern

Java uses an inheritance hierarchy (`IObject` → `IChunk` → `TextChunk`, etc.).
In Rust, use an **enum with shared fields**:

```rust
/// All content elements share a bounding box and ordering
pub struct ElementMeta {
    pub bbox: BoundingBox,
    pub page_number: u32,
    pub reading_order: Option<u32>,
    pub nesting_level: u32,
}

/// The core content element enum
/// See: 05-data-models § ContentElement Enum
pub enum ContentElement {
    TextChunk(ElementMeta, TextChunkData),
    TextLine(ElementMeta, TextLineData),
    TextBlock(ElementMeta, TextBlockData),
    TextColumn(ElementMeta, TextColumnData),
    ImageChunk(ElementMeta, ImageChunkData),
    LineChunk(ElementMeta, LineChunkData),
    LineArtChunk(ElementMeta, LineArtChunkData),
    Paragraph(ElementMeta, ParagraphData),
    Heading(ElementMeta, HeadingData),
    NumberHeading(ElementMeta, NumberHeadingData),
    Caption(ElementMeta, CaptionData),
    HeaderOrFooter(ElementMeta, HeaderOrFooterData),
    Figure(ElementMeta, FigureData),
    Table(ElementMeta, TableData),
    Formula(ElementMeta, FormulaData),
    Picture(ElementMeta, PictureData),
    List(ElementMeta, ListData),
}

impl ContentElement {
    /// Access shared metadata regardless of variant
    pub fn meta(&self) -> &ElementMeta {
        match self {
            ContentElement::TextChunk(m, _) => m,
            ContentElement::TextLine(m, _) => m,
            // ... all variants
        }
    }

    pub fn meta_mut(&mut self) -> &mut ElementMeta {
        match self {
            ContentElement::TextChunk(m, _) => m,
            ContentElement::TextLine(m, _) => m,
            // ... all variants
        }
    }

    pub fn semantic_type(&self) -> SemanticType {
        match self {
            ContentElement::TextChunk(..) => SemanticType::TextChunk,
            ContentElement::Paragraph(..) => SemanticType::Paragraph,
            ContentElement::Heading(..) => SemanticType::Heading,
            // ... all variants
        }
    }
}
```

**Why enum over trait objects?**
- Size known at compile time (no `Box<dyn>` indirection)
- Pattern matching enables exhaustive case handling
- Better cache locality when stored in `Vec<ContentElement>`
- Serialization is straightforward via serde `#[serde(tag = "type")]`

### 4.2 Processing Pipeline Pattern

```rust
/// Each processing stage is a function that mutates ProcessingContext
type ProcessingStage = fn(&mut ProcessingContext) -> Result<(), PipelineError>;

pub struct ProcessingContext {
    pub config: Config,
    pub pdf_document: PdfDocument,           // Loaded PDF reference
    pub pages: BTreeMap<u32, PageContent>,    // Per-page content
    pub table_borders: TableBordersCollection,
    pub heading_styles: Vec<TextStyle>,       // Detected heading styles
    pub statistics: DocumentStatistics,
}

pub struct PageContent {
    pub page_number: u32,
    pub page_bbox: BoundingBox,
    pub elements: Vec<ContentElement>,
}

/// The main pipeline orchestrator
/// See: 04-pdf-parsing-pipeline for full stage definitions
pub fn process_document(ctx: &mut ProcessingContext) -> Result<(), PipelineError> {
    let stages: &[(&str, ProcessingStage)] = &[
        ("content_filter",    content_filter::run),
        ("table_cluster",     table_cluster::run),
        ("table_border",      table_border::run),
        ("line_removal",      line_removal::run),
        ("text_line_group",   text_line::run),
        ("special_table",     table_special::run),
        ("header_footer",     header_footer::run),
        ("list_pass1",        list::run_pass1),
        ("paragraph",         paragraph::run),
        ("list_pass2",        list::run_pass2),
        ("heading",           heading::run),
        ("id_assignment",     id_assignment::run),
        ("caption",           caption::run),
        ("cross_page",        cross_page::run),
        ("heading_level",     level::assign_heading_levels),
        ("nesting_level",     level::assign_nesting_levels),
        ("reading_order",     reading_order::run),
        ("sanitizer",         sanitizer::run),
    ];

    for (name, stage) in stages {
        log::debug!("Running stage: {}", name);
        stage(ctx)?;
    }
    Ok(())
}
```

### 4.3 ProcessingContext (Replacing Static Globals)

Java uses `StaticContainers`, `StaticLayoutContainers`, `StaticResources`, and
`StaticStorages` — global mutable state shared across processors. This is the
#1 architectural anti-pattern to fix.

```rust
/// Replaces Java's StaticContainers + StaticLayoutContainers
/// + StaticResources + StaticStorages
pub struct ProcessingContext {
    // ---- Configuration (immutable after construction) ----
    pub config: Config,

    // ---- PDF Document (read-only after loading) ----
    pub pdf: PdfDocument,

    // ---- Per-page mutable state ----
    /// Page contents indexed by 1-based page number
    pub pages: BTreeMap<u32, PageContent>,

    // ---- Cross-page mutable state ----
    /// Table borders detected during PDF loading
    pub table_borders: TableBordersCollection,

    /// Lines (h-lines, v-lines) extracted during loading
    pub lines_collection: LinesCollection,

    /// Heading styles detected across all pages
    pub heading_styles: Vec<TextStyle>,

    /// Font statistics for heading detection
    pub font_statistics: Option<ModeWeightStatistics>,

    /// Text style statistics for heading detection
    pub text_style_config: Option<TextNodeStatisticsConfig>,

    // ---- Hybrid mode state ----
    /// HTTP client for backend communication (shared, cheaply cloneable)
    pub hybrid_client: Option<Arc<dyn HybridClient>>,

    /// Triage decisions per page
    pub triage_decisions: BTreeMap<u32, TriageDecision>,
}

impl ProcessingContext {
    pub fn new(config: Config, pdf: PdfDocument) -> Self {
        Self {
            config,
            pdf,
            pages: BTreeMap::new(),
            table_borders: TableBordersCollection::default(),
            lines_collection: LinesCollection::default(),
            heading_styles: Vec::new(),
            font_statistics: None,
            text_style_config: None,
            hybrid_client: None,
            triage_decisions: BTreeMap::new(),
        }
    }

    /// Clear all mutable state for reuse (batch processing)
    pub fn reset(&mut self, pdf: PdfDocument) {
        self.pdf = pdf;
        self.pages.clear();
        self.table_borders = TableBordersCollection::default();
        self.lines_collection = LinesCollection::default();
        self.heading_styles.clear();
        self.font_statistics = None;
        self.text_style_config = None;
        self.triage_decisions.clear();
    }
}
```

### 4.4 Error Handling Architecture

```rust
use thiserror::Error;

/// Library errors (used in opendataloader-pdf-core)
/// See: 03-technical-architecture § Error Architecture
#[derive(Error, Debug)]
pub enum OpendataLoaderError {
    #[error("PDF loading error: {0}")]
    Pdf(#[from] PdfError),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Hybrid backend error: {0}")]
    Hybrid(#[from] HybridError),

    #[error("Output generation error: {0}")]
    Output(#[from] OutputError),
}

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("Failed to open PDF file: {0}")]
    OpenFailed(String),

    #[error("PDF is password-protected")]
    PasswordRequired,

    #[error("Invalid password")]
    InvalidPassword,

    #[error("Failed to parse page {page}: {reason}")]
    PageParseFailed { page: u32, reason: String },

    #[error("Unsupported PDF feature: {0}")]
    UnsupportedFeature(String),
}

#[derive(Error, Debug)]
pub enum HybridError {
    #[error("Backend health check failed: {url}")]
    HealthCheckFailed { url: String },

    #[error("Request to backend failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Failed to parse backend response: {0}")]
    ResponseParseError(String),

    #[error("Partial success: {failed_pages:?} pages failed")]
    PartialSuccess { failed_pages: Vec<u32> },
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Input file not found: {0}")]
    InputNotFound(String),

    #[error("Output directory not writable: {0}")]
    OutputNotWritable(String),

    #[error("Invalid page range: {0}")]
    InvalidPageRange(String),

    #[error("Hybrid backend requires a URL (--hybrid-url)")]
    HybridUrlRequired,
}

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("Failed to write {format} output: {reason}")]
    WriteFailed { format: String, reason: String },
}
```

### 4.5 CLI with clap Derive

```rust
use clap::Parser;

/// OpenDataLoader PDF — extract structured data from PDF
/// See: 06-cli-interface for full option reference
#[derive(Parser, Debug)]
#[command(name = "opendataloader-pdf", version, about)]
pub struct CliArgs {
    /// Input PDF file or directory
    #[arg(short, long)]
    pub input: String,

    /// Output directory (default: same as input)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Output format(s): json, text, html, pdf, md, md:html, md:image
    #[arg(short, long, value_delimiter = ',', default_value = "md")]
    pub format: Vec<String>,

    /// PDF password for encrypted files
    #[arg(short, long)]
    pub password: Option<String>,

    /// Page range: "1-5,8,11-13"
    #[arg(long)]
    pub pages: Option<String>,

    /// Table detection method: default, cluster
    #[arg(long, default_value = "default")]
    pub table_method: String,

    /// Reading order algorithm: off, xy-cut
    #[arg(long, default_value = "xy-cut")]
    pub reading_order: String,

    /// Use PDF structure tree if available
    #[arg(long, default_value = "false")]
    pub use_struct_tree: bool,

    /// Keep hard line breaks within paragraphs
    #[arg(long, default_value = "false")]
    pub keep_line_breaks: bool,

    /// Include headers and footers in output
    #[arg(long, default_value = "false")]
    pub include_header_footer: bool,

    /// Image output mode: off, embedded, external
    #[arg(long, default_value = "off")]
    pub image_output: String,

    /// Image format: png, jpeg
    #[arg(long, default_value = "png")]
    pub image_format: String,

    /// Directory for external images
    #[arg(long)]
    pub image_dir: Option<String>,

    /// Disable content safety filter(s): hidden-text, out-of-page, tiny-text, hidden-layer
    #[arg(long, value_delimiter = ',')]
    pub content_safety_off: Vec<String>,

    /// Enable PII/sensitive data filtering
    #[arg(long, default_value = "false")]
    pub filter_sensitive_data: bool,

    /// Character replacement for invalid/undefined chars
    #[arg(long, default_value = "")]
    pub replace_invalid_chars: String,

    /// Hybrid backend: off, docling-fast, hancom
    #[arg(long, default_value = "off")]
    pub hybrid: String,

    /// Hybrid mode: auto, full
    #[arg(long, default_value = "auto")]
    pub hybrid_mode: String,

    /// Hybrid backend URL
    #[arg(long)]
    pub hybrid_url: Option<String>,

    /// Hybrid request timeout (ms)
    #[arg(long, default_value = "30000")]
    pub hybrid_timeout: u64,

    /// Fallback to local processing on hybrid error
    #[arg(long, default_value = "false")]
    pub hybrid_fallback: bool,

    /// Markdown page separator
    #[arg(long)]
    pub md_page_separator: Option<String>,

    /// Text page separator
    #[arg(long)]
    pub text_page_separator: Option<String>,

    /// HTML page separator
    #[arg(long)]
    pub html_page_separator: Option<String>,

    /// Export CLI options as JSON and exit
    #[arg(long, default_value = "false")]
    pub export_options: bool,
}
```

### 4.6 JSON Serialization Strategy

The Java code uses custom serializers with Jackson. In Rust, use serde with
`#[serde(rename = "...")]` to match the space-separated field name convention:

```rust
/// See: 08-output-formats § JSON Output for complete field mapping
#[derive(Serialize)]
pub struct JsonDocument {
    #[serde(rename = "file name")]
    pub file_name: String,

    #[serde(rename = "creation date")]
    pub creation_date: Option<String>,

    #[serde(rename = "modification date")]
    pub modification_date: Option<String>,

    #[serde(rename = "page count")]
    pub page_count: u32,

    pub elements: Vec<JsonElement>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum JsonElement {
    #[serde(rename = "paragraph")]
    Paragraph(JsonParagraphData),
    #[serde(rename = "heading")]
    Heading(JsonHeadingData),
    #[serde(rename = "table")]
    Table(JsonTableData),
    #[serde(rename = "list")]
    List(JsonListData),
    #[serde(rename = "image")]
    Image(JsonImageData),
    #[serde(rename = "formula")]
    Formula(JsonFormulaData),
    #[serde(rename = "picture")]
    Picture(JsonPictureData),
    #[serde(rename = "caption")]
    Caption(JsonCaptionData),
    #[serde(rename = "header")]
    Header(JsonHeaderFooterData),
    #[serde(rename = "footer")]
    Footer(JsonHeaderFooterData),
    #[serde(rename = "figure")]
    Figure(JsonFigureData),
}

/// Shared fields for all text-bearing elements
#[derive(Serialize)]
pub struct JsonParagraphData {
    pub content: String,

    #[serde(rename = "page number")]
    pub page_number: u32,

    #[serde(rename = "bounding box")]
    pub bounding_box: JsonBoundingBox,

    #[serde(rename = "text format")]
    pub text_format: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
}

#[derive(Serialize)]
pub struct JsonBoundingBox {
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub top: f64,
}

// Custom f64 serialization to limit decimal places
impl JsonBoundingBox {
    pub fn from_bbox(bbox: &BoundingBox) -> Self {
        Self {
            left: round3(bbox.left),
            bottom: round3(bbox.bottom),
            right: round3(bbox.right),
            top: round3(bbox.top),
        }
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
```

---

## 5. Migration Phase Plan

### 5.1 Phase Overview

```
Phase 0: Foundation     Phase 1: Core Types    Phase 2: PDF Parsing
(2 weeks)               (2 weeks)              (4-6 weeks)
+------------------+    +------------------+   +------------------+
| Workspace setup  |    | BoundingBox      |   | Content stream   |
| Cargo.toml files |    | All enums        |   | parser           |
| Error types      |    | ContentElement   |   | Font decoder     |
| Config structs   |    | All data structs |   | Text extraction  |
| CLI skeleton     |    | ProcessingContext|   | Image extraction |
| Logging          |    | Serde impls      |   | Line extraction  |
+----+-------------+    +----+-------------+   +----+-------------+
     |                       |                      |
     v                       v                      v

Phase 3: Pipeline       Phase 4: Output        Phase 5: Hybrid
(4-6 weeks)             (2-3 weeks)            (2-3 weeks)
+------------------+    +------------------+   +------------------+
| ContentFilter    |    | JSON writer      |   | TriageProcessor  |
| TableCluster     |    | Markdown gen     |   | Docling client   |
| TableBorder      |    | HTML generator   |   | Hancom client    |
| TextLineProc     |    | Text generator   |   | Schema transform |
| HeaderFooter     |    | Annotated PDF    |   | Merge logic      |
| ListDetection    |    | Image export     |   | Fallback handling|
| ParagraphDetect  |    +------------------+   +------------------+
| HeadingDetect    |
| ReadingOrder     |    Phase 6: Integration & Benchmark
| Sanitizer        |    (2-3 weeks)
| Tagged PDF       |    +------------------+
+------------------+    | Python wrapper   |
                        | Node.js wrapper  |
                        | Benchmark port   |
                        | Threshold tests  |
                        | CI pipeline      |
                        +------------------+
```

### 5.2 Phase 0 — Foundation

**Goal**: Compilable workspace with CLI that accepts all 24 options and prints help.

- [ ] Create workspace `Cargo.toml`
- [ ] Create `opendataloader-pdf-core` crate with `lib.rs`
- [ ] Create `opendataloader-pdf-cli` crate with `main.rs`
- [ ] Define `CliArgs` struct with all 24 clap options
- [ ] Define `Config`, `FilterConfig`, `HybridConfig` structs
- [ ] Implement `CliArgs → Config` conversion with validation
- [ ] Define error hierarchy (`OpendataLoaderError`, `PdfError`, etc.)
- [ ] Set up logging with `env_logger`
- [ ] Implement `--export-options` (read `options.json`, print JSON)
- [ ] Implement directory traversal for batch processing
- [ ] Write unit tests for config validation

**Exit criteria**: `cargo run -- --help` prints all options; `--export-options` outputs valid JSON matching `options.json`.

### 5.3 Phase 1 — Core Types

**Goal**: All data model structs compile and serialize correctly.

- [ ] Implement `BoundingBox` with all geometry methods (see [05-data-models](05-data-models.md))
- [ ] Implement `MultiBoundingBox`
- [ ] Implement all enums (`SemanticType`, `TextAlignment`, `TextFormat`, etc.)
- [ ] Implement `TextChunk`, `TextLine`, `TextBlock`, `TextColumn` structs
- [ ] Implement `ImageChunk`, `LineChunk`, `LineArtChunk` structs
- [ ] Implement `ContentElement` enum with `meta()` accessor
- [ ] Implement semantic node types (`ParagraphData`, `HeadingData`, etc.)
- [ ] Implement table types (`TableBorder`, `TableBorderRow`, `TableBorderCell`)
- [ ] Implement list types (`PDFList`, `ListItem`)
- [ ] Implement `ProcessingContext` struct
- [ ] Implement `PageContent` struct
- [ ] Add serde `Serialize` impls with correct JSON field names
- [ ] Write round-trip serialization tests against `schema.json`

**Exit criteria**: All types compile; JSON serialization matches expected format from [08-output-formats](08-output-formats.md).

### 5.4 Phase 2 — PDF Parsing

**Goal**: Load any PDF and extract raw content (TextChunk, ImageChunk, LineChunk) with bounding boxes.

- [ ] Implement `PdfDocument` wrapper over `lopdf::Document`
- [ ] Extract page geometry (MediaBox, CropBox, rotation)
- [ ] Parse content stream operators (use `pdf` crate)
- [ ] Implement `GraphicsState` stack tracking
- [ ] Implement `TextState` tracking (Tm, Td, font, size, color)
- [ ] Implement simple font encoding (WinAnsi, MacRoman)
- [ ] Implement ToUnicode CMap parsing
- [ ] Implement CIDFont / Identity-H encoding
- [ ] Calculate text bounding boxes from glyph widths + CTM
- [ ] Extract font weight from font descriptor / name heuristics
- [ ] Extract font color from graphics state
- [ ] Implement path operation parsing (m, l, re, c → LineChunk)
- [ ] Classify paths: horizontal line, vertical line, rectangle, other
- [ ] Implement image XObject extraction with bbox
- [ ] Handle page rotation (0°, 90°, 180°, 270°)
- [ ] Implement table border detection from `re` operations
- [ ] Handle encrypted PDFs (password support via `lopdf`)
- [ ] Test against 20+ diverse PDF files from benchmark suite

**Exit criteria**: Running against benchmark PDFs produces `Vec<ContentElement>` per page with correct bounding boxes for text, images, and lines.

### 5.5 Phase 3 — Processing Pipeline

**Goal**: Full pipeline (stages 2–19) transforms raw content into semantic elements.

Each stage maps directly to a stage in [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md):

- [ ] Content filter (11 sub-steps, Stage 2)
- [ ] Cluster table detection (Stage 3, Y_DIFF_EPSILON=0.1, X_DIFF_EPSILON=3.0)
- [ ] Table border matching (Stage 4, MAX_NESTED_DEPTH=10)
- [ ] Line removal (Stage 5)
- [ ] Text line grouping (Stage 6, ONE_LINE_PROBABILITY=0.75)
- [ ] Special table detection (Stage 7, Korean patterns)
- [ ] Header/footer detection (Stage 8, cross-page comparison)
- [ ] List detection pass 1 (Stage 9, label regex patterns)
- [ ] Paragraph detection (Stage 10, 9 merge passes)
- [ ] List detection pass 2 (Stage 11)
- [ ] Heading detection (Stage 12, probability formula ≥ 0.75)
- [ ] ID assignment (Stage 13)
- [ ] Caption linking (Stage 14, probability ≥ 0.75)
- [ ] Cross-page linking (Stage 15)
- [ ] Heading level assignment (Stage 16)
- [ ] Nesting level assignment (Stage 17)
- [ ] XY-Cut++ reading order (Stage 18, beta=2.0, density=0.9)
- [ ] Content sanitization (Stage 19, 10 regex rules)

For each stage:
1. Port the algorithm from Java pseudocode in spec 04
2. Use exact same constants/thresholds
3. Write unit test with synthetic input
4. Integration test with real PDF

**Exit criteria**: Pipeline produces correct semantic elements for all benchmark PDFs matching Java output within acceptable tolerance.

### 5.6 Phase 4 — Output Generation

**Goal**: Generate all 5 output formats matching Java behavior.

Port from [08-output-formats](08-output-formats.md):

- [ ] JSON writer with all field mappings (space-separated names)
- [ ] Markdown generator (pipe tables, # headings, - lists, $$ formulas)
- [ ] Markdown+HTML variant (HTML tables with colspan/rowspan)
- [ ] HTML5 generator (semantic tags, MathJax, CSS)
- [ ] Plain text generator (tab tables, indent lists)
- [ ] Annotated PDF generator (printpdf overlays, 7-color scheme, layers)
- [ ] Image extraction (external files + base64 embedded, max 10MB)
- [ ] Page separator support for Markdown/Text/HTML

**Exit criteria**: All output formats match Java output byte-for-byte for a reference set of 10 PDFs.

### 5.7 Phase 5 — Hybrid Mode

**Goal**: Full hybrid pipeline with triage, backend communication, and merge.

Port from [07-hybrid-mode](07-hybrid-mode.md):

- [ ] `TriageProcessor` with 6-signal priority chain and all 23 constants
- [ ] `HybridClient` trait definition
- [ ] `DoclingFastServerClient` (POST /v1/convert/file, multipart)
- [ ] `HancomClient` (3-step: upload → visualinfo → delete)
- [ ] `DoclingSchemaTransformer` (8 label mappings)
- [ ] Coordinate system conversion (TOPLEFT → BOTTOMLEFT)
- [ ] Merge logic (Java path + backend path → final output)
- [ ] Fallback handling (on backend error, retry with Java path)
- [ ] Health check before processing

**Exit criteria**: Hybrid mode produces output matching Java hybrid behavior against a running Docling server.

### 5.8 Phase 6 — Integration & Wrappers

**Goal**: End-to-end working system with benchmark validation.

- [ ] Update Python wrapper `runner.py` to call Rust binary
- [ ] Update Node.js wrapper `index.ts` to call Rust binary
- [ ] Verify `options.json` compatibility (same CLI interface)
- [ ] Port benchmark suite to run against Rust binary
- [ ] Verify all metrics meet `thresholds.json`:
  - NID ≥ threshold (reading order quality)
  - TEDS ≥ threshold (table structure accuracy)
  - MHS ≥ threshold (heading structure)
  - Table F1 ≥ threshold (table detection)
  - Speed ≤ threshold (processing time)
- [ ] Set up CI pipeline (cargo test, clippy, benchmark)
- [ ] Cross-platform testing (Linux, macOS, Windows)
- [ ] Binary distribution (cargo-dist or manual release)

**Exit criteria**: All benchmark metrics match or exceed Java baseline; Python and Node.js wrappers work unchanged (only binary path changes).

---

## 6. Critical Migration Details

### 6.1 Coordinate System Mapping

PDF uses bottom-left origin. The Java code stores coordinates as-is from veraPDF.

```
PDF Coordinate System (used in Java and Rust):

  top ─────────────────────┐
  │                         │
  │  y increases upward     │
  │                         │
  │  BoundingBox {          │
  │    left,                │
  │    bottom,              │
  │    right,               │
  │    top                  │
  │  }                      │
  │                         │
  bottom ──────────────────┘
  0,0                    right
       x increases rightward

Docling uses top-left origin (see 07-hybrid-mode § Coordinate Conversion):

  0,0 ────────────────────┐
  │                        │
  │  y increases downward  │
  │                        │
  bottom ─────────────────┘

Conversion:
  rust_bottom = page_height - docling_top
  rust_top    = page_height - docling_bottom
```

### 6.2 String Handling Differences

| Java | Rust | Notes |
|------|------|-------|
| `String` (UTF-16) | `String` (UTF-8) | PDF text may contain non-UTF-8 bytes |
| Mutable `StringBuilder` | `String::push_str` | Direct string building |
| `null` | `Option<String>` | No null strings in Rust |
| `String.isEmpty()` | `str.is_empty()` | Same semantics |
| `String.trim()` | `str.trim()` | Equivalent |
| `String.contains()` | `str.contains()` | Equivalent |
| `String.replaceAll(regex)` | `regex.replace_all()` | Use `regex` crate |
| `Character.isWhitespace()` | `char::is_whitespace()` | Equivalent |
| `String.format()` | `format!()` | Equivalent |

### 6.3 Collection Type Mapping

| Java | Rust | Notes |
|------|------|-------|
| `List<T>` | `Vec<T>` | Primary sequence type |
| `ArrayList<T>` | `Vec<T>` | Equivalent |
| `LinkedList<T>` | `VecDeque<T>` | Only where needed |
| `HashMap<K,V>` | `HashMap<K,V>` | Equivalent |
| `LinkedHashMap<K,V>` | `IndexMap<K,V>` | Insertion-order preserving |
| `TreeMap<K,V>` | `BTreeMap<K,V>` | Sorted by key |
| `HashSet<T>` | `HashSet<T>` | Equivalent |
| `TreeSet<T>` | `BTreeSet<T>` | Sorted |
| `Map.Entry<K,V>` | `(K, V)` tuple | In iterator context |
| `Collections.sort()` | `vec.sort_by()` | Custom comparator |
| `Stream.filter().map()` | `.iter().filter().map()` | Iterator chain |
| `Optional<T>` | `Option<T>` | Equivalent |

### 6.4 Numeric Precision

Java uses `double` ( IEEE 754 64-bit). Rust uses `f64` (same). Key considerations:

- BoundingBox coordinates: store as `f64`, serialize with `round3()` (3 decimal places)
- Epsilon comparisons: use `(a - b).abs() < EPSILON` pattern
- `f64` is not `Eq`/`Hash` in Rust — use `ordered_float::OrderedFloat<f64>` when needed as map keys or in sets
- Font size comparisons use `FONT_SIZE_EPSILON = 0.01` (see [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md))

### 6.5 Statistics and Font Analysis

The heading detection algorithm (see [04-pdf-parsing-pipeline § Heading Detection](04-pdf-parsing-pipeline.md)) uses `ModeWeightStatistics` to find the "normal" font. Rust implementation:

```rust
/// Tracks frequency of font properties across the document
/// Used to identify what the "normal" body text looks like
pub struct ModeWeightStatistics {
    /// Weighted frequency of each font size
    size_weights: BTreeMap<OrderedFloat<f64>, f64>,
    /// Weighted frequency of each font weight
    weight_weights: BTreeMap<OrderedFloat<f64>, f64>,
    /// Total character count
    total_chars: u64,
}

impl ModeWeightStatistics {
    pub fn new() -> Self { /* ... */ }

    /// Add a text node's contribution
    pub fn add(&mut self, size: f64, weight: f64, char_count: u64) {
        *self.size_weights
            .entry(OrderedFloat(size))
            .or_insert(0.0) += char_count as f64;
        *self.weight_weights
            .entry(OrderedFloat(weight))
            .or_insert(0.0) += char_count as f64;
        self.total_chars += char_count;
    }

    /// Returns the most common font size (the "mode")
    pub fn mode_size(&self) -> f64 {
        self.size_weights.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, _)| k.0)
            .unwrap_or(12.0)
    }

    /// Returns how "rare" a given font size is (0.0 = most common, 0.5 = max bonus)
    pub fn size_rarity(&self, size: f64) -> f64 {
        if self.total_chars == 0 { return 0.0; }
        let mode = self.mode_size();
        if (size - mode).abs() < 0.01 { return 0.0; }
        // Larger than mode → higher rarity → heading candidate
        if size > mode { 0.5_f64.min((size - mode) / mode * 0.5) } else { 0.0 }
    }

    /// Returns how "rare" a given font weight is (0.0 = common, 0.3 = max bonus)
    pub fn weight_rarity(&self, weight: f64) -> f64 {
        if self.total_chars == 0 { return 0.0; }
        let mode_weight = self.weight_weights.iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(k, _)| k.0)
            .unwrap_or(400.0);
        if (weight - mode_weight).abs() < 0.01 { return 0.0; }
        if weight > mode_weight { 0.3 } else { 0.0 }
    }
}
```

### 6.6 Tagged PDF Support

Structure tree parsing (for `--use-struct-tree` mode) requires:

```
PDF Structure Tree:
  /StructTreeRoot → /K (root element)
    → /K children (recursive)
      Each /S tag: Document, Part, Sect, H1-H6, P, Table, TR, TH, TD,
                   L, LI, Lbl, LBody, Figure, Formula, Span, etc.
      Each child has /K (content or more children)
      MCIDs link to page content via marked content

Rust implementation:
  1. Read /StructTreeRoot from catalog
  2. Walk /K children recursively
  3. For each leaf with MCID, find corresponding page content
  4. Map PDF tag → SemanticType
  5. Build ContentElement tree

Tag mapping:
  Document → skip (container)
  P        → Paragraph
  H, H1-H6 → Heading (level from tag)
  Table    → Table
  TR       → TableBorderRow
  TH, TD   → TableBorderCell
  L        → PDFList
  LI       → ListItem
  Lbl      → ListLabel
  LBody    → ListBody
  Figure   → Figure
  Formula  → Formula
  Span     → inline text (merge into parent)
```

### 6.7 Image Handling

```rust
use image::{DynamicImage, ImageFormat, ImageOutputFormat};

/// Extract image from PDF XObject and convert to target format
/// See: 08-output-formats § Image Extraction
pub fn extract_image(
    raw_bytes: &[u8],
    width: u32,
    height: u32,
    color_space: &ColorSpace,
    target_format: ImageFormat,
) -> Result<Vec<u8>, OutputError> {
    // 1. Decode raw image data based on color space
    let img = match color_space {
        ColorSpace::DeviceRGB => {
            DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(width, height, raw_bytes.to_vec())
                    .ok_or(OutputError::WriteFailed {
                        format: "image".into(),
                        reason: "invalid dimensions".into(),
                    })?
            )
        }
        ColorSpace::DeviceGray => {
            DynamicImage::ImageLuma8(
                image::GrayImage::from_raw(width, height, raw_bytes.to_vec())
                    .ok_or(OutputError::WriteFailed {
                        format: "image".into(),
                        reason: "invalid dimensions".into(),
                    })?
            )
        }
        ColorSpace::DeviceCMYK => {
            // Convert CMYK → RGB
            let rgb_bytes = cmyk_to_rgb(raw_bytes);
            DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(width, height, rgb_bytes)
                    .ok_or(OutputError::WriteFailed {
                        format: "image".into(),
                        reason: "invalid dimensions".into(),
                    })?
            )
        }
        _ => return Err(OutputError::WriteFailed {
            format: "image".into(),
            reason: format!("unsupported color space: {:?}", color_space),
        }),
    };

    // 2. Encode to target format
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), target_format)
        .map_err(|e| OutputError::WriteFailed {
            format: "image".into(),
            reason: e.to_string(),
        })?;
    Ok(buf)
}

/// Max embedded image size (base64): 10 MB
/// See: 08-output-formats § Image Extraction
const MAX_EMBEDDED_IMAGE_SIZE: usize = 10 * 1024 * 1024;
```

---

## 7. Testing Strategy

### 7.1 Test Pyramid

```
                                  /\
                                 /  \
                                / E2E\       10 tests
                               / (bench)\    Full pipeline
                              /___________\  vs Java output
                             /              \
                            / Integration    \  50 tests
                           / (real PDFs,      \ Each stage
                          / multi-stage)       \ individually
                         /_____________________\
                        /                        \
                       / Unit tests                \ 200+ tests
                      / (per function, per module)  \ Synthetic data
                     /______________________________\
```

### 7.2 Unit Testing Strategy

For each module, create corresponding test files:

```rust
// In pipeline/content_filter.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_removes_duplicate_text_chunks() {
        let mut ctx = test_context();
        // Add two overlapping TextChunks (>50% overlap)
        ctx.pages.get_mut(&1).unwrap().elements.push(
            make_text_chunk("hello", bbox(10.0, 10.0, 50.0, 20.0))
        );
        ctx.pages.get_mut(&1).unwrap().elements.push(
            make_text_chunk("hello", bbox(12.0, 10.0, 52.0, 20.0))
        );

        content_filter::run(&mut ctx).unwrap();

        assert_eq!(ctx.pages[&1].elements.len(), 1);
    }

    #[test]
    fn test_filters_tiny_text() {
        let mut ctx = test_context();
        ctx.pages.get_mut(&1).unwrap().elements.push(
            make_text_chunk("tiny", bbox(10.0, 10.0, 50.0, 10.5)) // height = 0.5 < 1.0
        );

        content_filter::run(&mut ctx).unwrap();

        assert!(ctx.pages[&1].elements.is_empty());
    }
}
```

### 7.3 Integration Testing Against Java Output

Create a reference set by running the Java version on benchmark PDFs:

```bash
# Generate reference outputs from Java version
for pdf in tests/benchmark/pdfs/*.pdf; do
    java -jar opendataloader-pdf.jar -i "$pdf" -o tests/fixtures/reference/ -f json
done
```

Then compare in Rust integration tests:

```rust
#[test]
fn test_lorem_pdf_matches_java_output() {
    let result = process_pdf("tests/fixtures/lorem.pdf", &default_config());
    let expected: JsonDocument = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/reference/lorem.json").unwrap()
    ).unwrap();

    assert_eq!(result.elements.len(), expected.elements.len());
    for (got, exp) in result.elements.iter().zip(expected.elements.iter()) {
        assert_eq!(got.element_type(), exp.element_type());
        assert_bbox_close(&got.bounding_box(), &exp.bounding_box(), 0.5);
    }
}

fn assert_bbox_close(a: &BoundingBox, b: &BoundingBox, tolerance: f64) {
    assert!((a.left - b.left).abs() < tolerance, "left: {} vs {}", a.left, b.left);
    assert!((a.bottom - b.bottom).abs() < tolerance, "bottom mismatch");
    assert!((a.right - b.right).abs() < tolerance, "right mismatch");
    assert!((a.top - b.top).abs() < tolerance, "top mismatch");
}
```

### 7.4 Benchmark Validation

Port the Python benchmark suite ([tests/benchmark/](../tests/benchmark/)) to
validate the Rust implementation:

```
Benchmark Metrics (must match or exceed thresholds.json):

  NID   — Normalized Inverse Displacement (reading order)
  TEDS  — Tree Edit Distance Similarity (table structure)
  MHS   — Mean Heading Similarity (heading detection)
  TDF1  — Table Detection F1 (table finding)
  Speed — Seconds per page (must be faster than Java)

Test flow:
  1. Run Rust binary on 200 benchmark PDFs
  2. Compare predictions/ against ground-truth/
  3. Calculate 5 metrics
  4. Assert all metrics ≥ thresholds
  5. Fail CI if any regression
```

---

## 8. Performance Optimization Opportunities

### 8.1 Zero-Copy PDF Parsing

Where veraPDF copies all text into Java Strings, Rust can use lifetime-bound
references for intermediate processing:

```rust
// During content stream parsing, reference PDF bytes directly
struct RawTextSpan<'a> {
    bytes: &'a [u8],          // Points into content stream
    font_id: &'a str,         // Points into /Resources dict
    transform: Matrix,
}

// Only allocate String when building final TextChunk
impl<'a> RawTextSpan<'a> {
    fn decode(&self, font_cache: &FontCache) -> String {
        font_cache.decode(self.font_id, self.bytes)
    }
}
```

### 8.2 SIMD-Accelerated Regex

The `regex` crate already uses SIMD (vectorized DFA) internally.
PII sanitization with 10 regex patterns benefits from `RegexSet`:

```rust
use regex::RegexSet;
use std::sync::LazyLock;

static PII_PATTERNS: LazyLock<RegexSet> = LazyLock::new(|| {
    RegexSet::new(&[
        r"\b\d{3}-\d{2}-\d{4}\b",           // SSN
        r"\b[A-Z]{2}\d{6,8}\b",             // Passport
        r"\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b", // Credit card
        // ... 7 more patterns from 04-pdf-parsing-pipeline § 20
    ]).unwrap()
});
```

### 8.3 Parallel Batch Processing

```rust
use rayon::prelude::*;

// Process multiple PDFs in parallel
let results: Vec<Result<(), OpendataLoaderError>> = pdf_paths
    .par_iter()
    .map(|path| {
        let pdf = load_pdf(path, &config)?;
        let mut ctx = ProcessingContext::new(config.clone(), pdf);
        process_document(&mut ctx)?;
        generate_outputs(&ctx)?;
        Ok(())
    })
    .collect();
```

### 8.4 Memory Pool for BoundingBox Operations

Since `BoundingBox` is created/destroyed millions of times during processing:

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingBox {
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub top: f64,
}
```

Using `Copy` instead of `Clone` avoids heap allocation. All BoundingBox
operations return new values by copy (16 bytes on stack).

### 8.5 Lazy Font Decoding

Don't decode all fonts upfront — decode on first use:

```rust
use std::collections::HashMap;
use std::cell::RefCell;

pub struct FontCache {
    raw_fonts: HashMap<String, RawFontData>,
    decoded: RefCell<HashMap<String, DecodedFont>>,
}

impl FontCache {
    pub fn decode(&self, font_id: &str, bytes: &[u8]) -> String {
        let mut decoded = self.decoded.borrow_mut();
        let font = decoded.entry(font_id.to_string()).or_insert_with(|| {
            self.raw_fonts[font_id].decode()
        });
        font.map_bytes_to_string(bytes)
    }
}
```

---

## 9. Wrapper Compatibility

### 9.1 Python Wrapper Changes

The Python wrapper (`python/opendataloader-pdf/src/opendataloader_pdf/runner.py`)
currently spawns `java -jar opendataloader-pdf.jar`. After migration:

```python
# Before (Java):
cmd = ["java", "-jar", jar_path, "-i", input_path, ...]

# After (Rust):
cmd = [binary_path, "-i", input_path, ...]
```

The CLI interface is identical. Only the binary name and invocation method change.
The `hatch_build.py` hook should bundle the compiled Rust binary instead of the JAR.

### 9.2 Node.js Wrapper Changes

Similarly `node/opendataloader-pdf/src/index.ts` currently calls:

```typescript
// Before:
const args = ['java', '-jar', jarPath, ...cliArgs];

// After:
const args = [binaryPath, ...cliArgs];
```

The exact same CLI arguments are used. All generated code from `options.json`
remains valid.

### 9.3 Code Generation Pipeline

The `options.json` → code generation pipeline is language-agnostic:

```
options.json  ──► generate-options.mjs ──► Python .generated.py
                                      ──► Node.js .generated.ts
                                      ──► MDX docs .generated.mdx
```

This pipeline requires **no changes** for the Rust migration. The generated
code wraps CLI arguments, which are the same regardless of whether the backend
is Java or Rust.

After Rust migration, add `npm run sync` to also generate Rust constants if desired:

```rust
// Optionally auto-generated from options.json
pub const OPTIONS_JSON: &str = include_str!("../../../options.json");
```

---

## 10. Risk Assessment

### 10.1 High Risk Items

| Risk | Impact | Mitigation |
|------|--------|------------|
| Font decoding accuracy | Incorrect text extraction for CIDFonts, custom encodings | Test against full benchmark suite; fallback to raw bytes with replacement char |
| Table border detection parity | Missing tables in complex PDFs | Port exact algorithm from Java with same constants; compare bounding boxes |
| Reading order divergence | XY-Cut++ with floating point differences | Use ordered-float for deterministic sorting; validate NID metric |
| Annotated PDF quality | PDF viewer compatibility issues with overlays | Test in Adobe Reader, Chrome, Preview; use printpdf layers |

### 10.2 Medium Risk Items

| Risk | Impact | Mitigation |
|------|--------|------------|
| Performance regression | Slower than Java for some operations | Profile early; benchmark suite catches regressions |
| Memory usage for large PDFs | OOM on 1000+ page PDFs | Use streaming page-by-page processing; limit image cache |
| Hybrid mode compatibility | HTTP protocol mismatch with Python server | Integration test against running server; mock server for unit tests |
| Cross-platform builds | Windows/macOS/Linux differences | CI matrix testing; avoid platform-specific code |

### 10.3 Low Risk Items

| Risk | Impact | Mitigation |
|------|--------|------------|
| CLI compatibility | Option name mismatch | Generated from same options.json |
| JSON output format | Field name differences | Unit test against schema.json |
| Wrapper compatibility | Python/Node.js breakage | Only binary path changes |

---

## 11. Quick Reference: Java → Rust Cheat Sheet

### 11.1 Common Patterns

```
Java:                                    Rust:
─────────────────────────────────────    ─────────────────────────────────────
for (T item : list) { ... }             for item in &list { ... }
list.stream().filter(p).collect()        list.iter().filter(p).collect()
list.add(item)                           list.push(item)
list.get(i)                              list.get(i) → Option<&T>
list.size()                              list.len()
list.isEmpty()                           list.is_empty()
map.put(k, v)                            map.insert(k, v)
map.get(k)                               map.get(&k) → Option<&V>
map.containsKey(k)                       map.contains_key(&k)
map.getOrDefault(k, d)                   map.get(&k).unwrap_or(&d)
obj == null                              obj.is_none()
obj != null                              obj.is_some()
obj.toString()                           format!("{}", obj) or obj.to_string()
Math.abs(x)                              x.abs()
Math.max(a, b)                           a.max(b)  or  f64::max(a, b)
Math.min(a, b)                           a.min(b)  or  f64::min(a, b)
Math.round(x)                            x.round()
String.format("%s", x)                   format!("{}", x)
instanceof                               matches!(val, Variant(..))
(Type) cast                              if let Variant(data) = val { ... }
try { ... } catch (E e) { ... }          match result { Ok(v) => ..., Err(e) => ... }
throw new Exception(msg)                 return Err(Error::new(msg))
```

### 11.2 veraPDF Method → Rust Function Mapping

```
veraPDF:                                 Rust equivalent:
──────────────────────────────┐         ──────────────────────────────────────
bbox.getLeftX()               │         bbox.left
bbox.getRightX()              │         bbox.right
bbox.getBottomY()             │         bbox.bottom
bbox.getTopY()                │         bbox.top
bbox.getWidth()               │         bbox.width()   → right - left
bbox.getHeight()              │         bbox.height()  → top - bottom
bbox.contains(other)          │         bbox.contains(&other)
bbox.overlaps(other, min%)    │         bbox.overlap_percent(&other) >= min
chunk.getValue()              │         chunk.text.as_str()
chunk.getFontName()           │         chunk.font_name.as_str()
chunk.getFontSize()           │         chunk.font_size
chunk.getFontWeight()         │         chunk.font_weight
chunk.getFontColor()          │         chunk.font_color
node.getSemanticType()        │         element.semantic_type()
node.getChildren()            │         element.children() → &[ContentElement]
node.getTextValue()           │         element.text_content()
```
