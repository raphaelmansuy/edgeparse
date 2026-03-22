# 03 — Technical Architecture

> **Cross-references**: [01-overview](01-overview.md) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) | [05-data-models](05-data-models.md) | [09-rust-migration-guide](09-rust-migration-guide.md)

---

## 1. System Architecture

### 1.1 High-Level Component Diagram

```
+=========================================================================+
|                         USER INTERFACE LAYER                             |
|-------------------------------------------------------------------------|
|                                                                         |
|  CLI Binary (Rust)            Python Wrapper         Node.js Wrapper    |
|  +------------------+        +------------------+  +------------------+ |
|  | Arg Parser       |        | subprocess call  |  | child_process    | |
|  | File Traversal   |        | to Rust binary   |  | to Rust binary   | |
|  | Config Builder   |        | Same CLI args    |  | Same CLI args    | |
|  | --export-options |        | Generated code   |  | Generated code   | |
|  +--------+---------+        +--------+---------+  +--------+---------+ |
|           |                           |                      |          |
+===========|===========================|======================|==========+
            |                           |                      |
            v                           v                      v
+=========================================================================+
|                          CORE ENGINE LAYER                               |
|-------------------------------------------------------------------------|
|                                                                         |
|  +-------------------------------------------------------------------+ |
|  |                    DocumentProcessor                               | |
|  |                                                                    | |
|  |  1. PDF Loading -----> 2. Content Filtering ----->                 | |
|  |  3. Table Detection -> 4. Text Line Grouping -->                   | |
|  |  5. Header/Footer ---> 6. List Detection -------->                 | |
|  |  7. Paragraphs ------> 8. Headings -------------->                 | |
|  |  9. Captions --------> 10. Cross-page Linking -->                  | |
|  |  11. Level Assignment > 12. Reading Order -------->                | |
|  |  13. Content Safety --> 14. Output Generation                      | |
|  +-------------------------------------------------------------------+ |
|                                                                         |
|  +-------------------+  +-------------------+  +--------------------+  |
|  | TaggedDocProcessor|  | HybridDocProcessor|  | Std DocProcessor   |  |
|  | (struct tree path)|  | (triage + merge)  |  | (default path)    |  |
|  +-------------------+  +-------------------+  +--------------------+  |
|                                                                         |
+=========================================================================+
            |                    |                         |
            v                    v                         v
+=========================================================================+
|                        OUTPUT GENERATION LAYER                           |
|-------------------------------------------------------------------------|
|                                                                         |
|  JsonWriter    MarkdownGen   HtmlGenerator   TextGenerator   PdfWriter  |
|  (Jackson-     (pipe-delim   (HTML5,         (plain text,    (annotated |
|   style,       tables,       semantic        tab-separated   PDF with   |
|   custom       #-headings,   tags,           tables)         bounding   |
|   serializers) formulas)     MathJax)                        boxes)     |
|                                                                         |
+=========================================================================+
            |
            v
+=========================================================================+
|                         HYBRID CLIENT LAYER                              |
|-------------------------------------------------------------------------|
|                                                                         |
|  TriageProcessor -----> HybridClient (trait) -----> SchemaTransformer   |
|  (page classifier)      |                          (response mapper)    |
|                          +-- DoclingFastClient                          |
|                          +-- HancomClient                               |
|                                                                         |
+=========================================================================+
            |                                                  ^
            v (HTTP)                                           |
+=========================================================================+
|                    EXTERNAL BACKEND (Python)                              |
|-------------------------------------------------------------------------|
|  hybrid_server.py (FastAPI)                                              |
|  +-- Docling DocumentConverter                                           |
|  +-- EasyOCR (optional)                                                  |
|  +-- SmolVLM (optional, picture descriptions)                            |
+=========================================================================+
```

### 1.2 Processing Paths

```
                        +-- use_struct_tree=true ---> TaggedDocumentProcessor
                        |                            (structure tree walk)
                        |
PDF Input ---> Config --+-- hybrid != "off" --------> HybridDocumentProcessor
                        |                            (triage + split + merge)
                        |
                        +-- default ----------------> DocumentProcessor
                                                     (20-stage pipeline)
```

---

## 2. Module Decomposition

### 2.1 Module Boundary Diagram

```
+-- opendataloader-pdf (binary crate) ----+
|                                          |
|  main.rs          CLI entry point        |
|  cli.rs           Argument parsing       |
|  config.rs        Config + FilterConfig  |
|  traversal.rs     File/dir traversal     |
|                                          |
+------------------------------------------+
           |
           | depends on
           v
+-- opendataloader-pdf-core (library crate) -------+
|                                                    |
|  +-- pdf/                                          |
|  |   loader.rs       PDF document loading          |
|  |   parser.rs       Text/image/line extraction    |
|  |   struct_tree.rs  Structure tree parsing         |
|  |                                                  |
|  +-- model/                                         |
|  |   types.rs        ContentElement enum + structs  |
|  |   bbox.rs         BoundingBox, MultiBoundingBox  |
|  |   document.rs     Document metadata              |
|  |   semantic.rs     SemanticType enum              |
|  |   alignment.rs    TextAlignment enum             |
|  |                                                  |
|  +-- pipeline/                                      |
|  |   mod.rs          DocumentProcessor orchestrator |
|  |   filter.rs       ContentFilterProcessor         |
|  |   hidden_text.rs  HiddenTextProcessor            |
|  |   text_proc.rs    TextProcessor utilities        |
|  |   text_line.rs    TextLineProcessor              |
|  |   table_border.rs TableBorderProcessor           |
|  |   table_cluster.rs ClusterTableProcessor         |
|  |   table_special.rs SpecialTableProcessor         |
|  |   header_footer.rs HeaderFooterProcessor         |
|  |   list.rs         ListProcessor                  |
|  |   paragraph.rs    ParagraphProcessor             |
|  |   heading.rs      HeadingProcessor               |
|  |   caption.rs      CaptionProcessor               |
|  |   level.rs        LevelProcessor                 |
|  |   reading_order.rs XYCutPlusPlusSorter           |
|  |   sanitizer.rs    ContentSanitizer               |
|  |   tagged.rs       TaggedDocumentProcessor        |
|  |                                                  |
|  +-- hybrid/                                        |
|  |   mod.rs          HybridDocumentProcessor        |
|  |   triage.rs       TriageProcessor                |
|  |   client.rs       HybridClient trait             |
|  |   docling.rs      DoclingFastServerClient        |
|  |   hancom.rs       HancomClient                   |
|  |   transform.rs    Schema transformers            |
|  |   config.rs       HybridConfig                   |
|  |                                                  |
|  +-- output/                                        |
|  |   json.rs         JSON writer (serde-based)      |
|  |   markdown.rs     Markdown generator             |
|  |   markdown_html.rs Markdown+HTML variant          |
|  |   html.rs         HTML5 generator                |
|  |   text.rs         Plain text generator           |
|  |   pdf_writer.rs   Annotated PDF writer           |
|  |   images.rs       Image extraction utilities     |
|  |                                                  |
|  +-- utils/                                         |
|      statistics.rs   Font/weight statistics         |
|      merge.rs        Chunk merge utilities          |
|      numbering.rs    Numbering pattern detection    |
|      sanitize_rules.rs PII regex rules              |
|                                                     |
+-----------------------------------------------------+
```

### 2.2 Rust Crate Structure

```
opendataloader-pdf/
+-- Cargo.toml              (workspace)
+-- crates/
    +-- opendataloader-pdf-core/
    |   +-- Cargo.toml
    |   +-- src/
    |       +-- lib.rs
    |       +-- pdf/         (PDF loading & raw extraction)
    |       +-- model/       (data types)
    |       +-- pipeline/    (processing stages)
    |       +-- hybrid/      (client-side hybrid)
    |       +-- output/      (formatters)
    |       +-- utils/       (shared utilities)
    |
    +-- opendataloader-pdf-cli/
        +-- Cargo.toml
        +-- src/
            +-- main.rs
            +-- cli.rs
            +-- config.rs
            +-- traversal.rs
```

---

## 3. Data Flow Architecture

### 3.1 Complete Data Flow

```
PDF File (bytes)
    |
    v
+-- PDF Loader -------------------------------------------+
|  Parse PDF structure (pages, fonts, content streams)    |
|  Extract raw content per page:                          |
|    - TextChunk[]  (text with font/position)             |
|    - ImageChunk[] (images with bbox)                    |
|    - LineChunk[]  (horizontal/vertical rules)           |
|    - LineArtChunk[] (vector graphics)                   |
|  Detect table borders from line segments                |
+----------------------------+----------------------------+
                             |
                             v
                   Map<PageNum, Vec<ContentElement>>
                             |
                             v
+-- Content Filter -------------------------------------------+
|  For each page:                                             |
|  1. Remove duplicate text chunks (>50% bbox overlap)        |
|  2. Remove text-decoration images                           |
|  3. Filter tiny text (height <= 1pt)                        |
|  4. Filter off-page content (outside CropBox)               |
|  5. Merge close text chunks (same style, adjacent)          |
|  6. Trim whitespace                                         |
|  7. Compress consecutive spaces                             |
|  8. Split text chunks by internal whitespace                |
|  9. Detect hidden text (contrast < 1.2)                     |
|  10. Replace undefined characters                           |
|  11. Remove background objects                              |
+----------------------------+--------------------------------+
                             |
                             v
+-- Table Detection ------------------------------------------+
|  Cluster method: Split text → cluster → TableBorder         |
|  Border method: Match content to pre-detected borders       |
|  Special: Korean government table patterns                  |
|  Remove LineChunk objects after table processing             |
+----------------------------+--------------------------------+
                             |
                             v
+-- Text Line Grouping ---------------------------------------+
|  Merge TextChunks into TextLines                            |
|  Criteria: same-line probability >= 0.75                    |
|  Sort chunks within line by leftX                           |
|  Insert space chunks at word boundaries                     |
|  Link LineArtChunk bullets to TextLines                     |
+----------------------------+--------------------------------+
                             |
                             v
+-- Header/Footer Detection (cross-page) ---------------------+
|  Compare element positions across adjacent pages            |
|  Match by: text similarity, numbering sequence, bbox        |
|  Filter: top 1/3 (headers), bottom 1/3 (footers)           |
|  Support 2-page alternating styles                          |
|  Wrap matched content in SemanticHeaderOrFooter             |
+----------------------------+--------------------------------+
                             |
                             v
+-- List Detection (Pass 1: TextLine level) ------------------+
|  Scan for label patterns (numbers, bullets, Korean, etc.)   |
|  Group consecutive labels into PDFList structures           |
|  Assign body content to list items                          |
+----------------------------+--------------------------------+
                             |
                             v
+-- Paragraph Detection --------------------------------------+
|  Multi-pass merging of TextLines into paragraphs:           |
|  1. Justify alignment blocks                                |
|  2. Justify first/last line detection                       |
|  3. Left alignment (strict, then relaxed)                   |
|  4. Left block first lines                                  |
|  5. Two-line paragraphs                                     |
|  6. Center alignment                                        |
|  7. Right alignment                                         |
|  8. Fallback same-style merge                               |
+----------------------------+--------------------------------+
                             |
                             v
+-- List Detection (Pass 2: Paragraph level) -----------------+
|  Detect lists formed by SemanticTextNode sequences          |
+----------------------------+--------------------------------+
                             |
                             v
+-- Heading Detection ----------------------------------------+
|  Calculate heading probability per text node:               |
|  base + font_size_rarity + font_weight_rarity + bullet      |
|  Threshold: >= 0.75                                         |
+----------------------------+--------------------------------+
                             |
                             v
+-- Caption Linking ------------------------------------------+
|  Link text nodes to adjacent tables/images                  |
|  Probability threshold: >= 0.75                             |
+----------------------------+--------------------------------+
                             |
                             v
+-- Cross-Page Linking (lists, tables) -----------------------+
|  Merge lists with continuous numbering across pages         |
|  Link tables with same column structure                     |
+----------------------------+--------------------------------+
                             |
                             v
+-- Heading Level Assignment ---------------------------------+
|  Group headings by TextStyle (font+size+weight+color)       |
|  Sort groups by visual prominence                           |
|  Assign H1, H2, H3... to distinct groups                   |
+----------------------------+--------------------------------+
                             |
                             v
+-- Level/Nesting Assignment ---------------------------------+
|  Stack-based nesting: headings, lists, tables               |
|  First H1 = "Doctitle", others = "Subtitle"                |
|  Connected elements inherit parent level                    |
+----------------------------+--------------------------------+
                             |
                             v
+-- Reading Order Sorting ------------------------------------+
|  XY-Cut++ per page:                                         |
|  1. Pre-mask cross-layout (wide) elements                   |
|  2. Compute density ratio                                   |
|  3. Recursive projection-based segmentation                 |
|  4. Merge cross-layout elements by Y-position               |
+----------------------------+--------------------------------+
                             |
                             v
+-- Content Sanitization (optional) --------------------------+
|  Regex-based PII replacement                                |
|  TextLine-level pattern matching and chunk splitting        |
+----------------------------+--------------------------------+
                             |
                             v
+-- Output Generation ----------------------------------------+
|  JSON: Document metadata + elements tree with serializers   |
|  Markdown: Pipe tables, # headings, - lists, $$ formulas   |
|  HTML: Semantic HTML5 with CSS, MathJax formulas            |
|  Text: Plain text with tab tables, indent lists             |
|  PDF: Annotated overlay with bounding box rectangles        |
+---------+----------+----------+----------+----------+-------+
          |          |          |          |          |
          v          v          v          v          v
       .json       .md       .html      .txt    _annotated.pdf
```

---

## 4. State Management

### 4.1 Current Java Architecture (Static Containers — Anti-pattern)

The Java codebase uses **global static containers** (`StaticContainers`, `StaticLayoutContainers`, `StaticResources`, `StaticStorages`) to share state between processing stages. This is a known anti-pattern that:

- Prevents parallelism (single-threaded processing per document)
- Requires explicit clearing between documents
- Makes testing difficult
- Creates hidden coupling between processors

### 4.2 Recommended Rust Architecture (Owned State)

```
ProcessingContext {
    document: PdfDocument,        // Loaded PDF
    config: Config,               // User configuration
    page_contents: HashMap<u32, Vec<ContentElement>>,  // Per-page elements
    table_borders: TableBordersCollection,             // Detected tables
    headings: Vec<HeadingRef>,                         // Detected headings
    contrast_consumer: Option<ContrastRatioCalculator>, // For hidden text
}
```

Each `ProcessingContext` is created per document and passed by mutable reference through the pipeline. No global state.

### 4.3 Concurrency Model

```
                    +--- Document 1 ---> Pipeline ---> Output
                    |
Main Thread --------+--- Document 2 ---> Pipeline ---> Output
(file traversal)    |
                    +--- Document 3 ---> Pipeline ---> Output
```

Each document can be processed independently in parallel (no shared state between documents). Within a single document, processing is sequential (stages depend on previous stage output).

---

## 5. External Dependencies (Java → Rust Mapping)

### 5.1 PDF Parsing Layer

| Java (veraPDF) | Rust Equivalent | Notes |
|----------------|----------------|-------|
| `PDDocument` | `lopdf::Document` or `pdf` crate | Low-level PDF object access |
| `GFSAPDFDocument` | Custom wrapper | Text/image/line extraction |
| `parseChunks()` | Custom page content parser | Must handle font decoding, coordinate transforms |
| `LinesPreprocessingConsumer` | Custom line segment extractor | Find table borders from path operations |
| `ContrastRatioConsumer` | Custom renderer | Page rasterization for contrast calculation |
| `ClusterTableConsumer` | Custom implementation | Text alignment clustering |

### 5.2 Utility Libraries

| Java Library | Rust Equivalent | Purpose |
|-------------|----------------|---------|
| Apache Commons CLI | `clap` | CLI argument parsing |
| Jackson | `serde` + `serde_json` | JSON serialization |
| OkHttp | `reqwest` | HTTP client (hybrid mode) |
| java.util.logging | `log` + `env_logger` | Logging |
| java.util.regex | `regex` crate | PII pattern matching |
| java.awt (rendering) | `tiny-skia` or `resvg` | Page rasterization for contrast |

### 5.3 veraPDF Types to Reimplement

| Category | Types | Effort |
|----------|-------|--------|
| Geometry | `BoundingBox`, `MultiBoundingBox` | Small |
| Content chunks | `TextChunk`, `ImageChunk`, `LineChunk`, `LineArtChunk` | Medium |
| Text grouping | `TextLine`, `TextBlock`, `TextColumn` | Medium |
| Semantic nodes | `SemanticTextNode`, `SemanticParagraph`, `SemanticHeading`, `SemanticCaption`, `SemanticHeaderOrFooter`, `SemanticFigure` | Medium |
| Tables | `TableBorder`, `TableBorderRow`, `TableBorderCell`, `TableBordersCollection` | Large |
| Lists | `PDFList`, `ListItem`, `ListInterval`, `ListItemInfo` | Medium |
| Utilities | `ChunksMergeUtils`, `NodeUtils`, `CaptionUtils`, `ListLabelsUtils`, `BulletedParagraphUtils` | Large |
| Enums | `SemanticType`, `TextAlignment`, `ContentType` | Small |
| PDF parsing | `GFSAPDFDocument`, chunk parsing, font decoding | **Very Large** |

→ Full type hierarchy: [05-data-models](05-data-models.md)

---

## 6. Configuration Architecture

### 6.1 Config Flow

```
CLI Args (strings)
    |
    v
+-- Argument Parser (clap) ----+
|  Validate types and values   |
|  Build Config struct         |
+------------------------------+
    |
    v
Config {
    // Output
    output_folder: Option<PathBuf>,
    formats: HashSet<OutputFormat>,
    
    // PDF
    password: Option<String>,
    pages: Option<Vec<u32>>,
    
    // Processing
    use_struct_tree: bool,
    table_method: TableMethod,
    reading_order: ReadingOrder,
    keep_line_breaks: bool,
    replace_invalid_chars: String,
    include_header_footer: bool,
    
    // Images
    image_output: ImageOutput,
    image_format: ImageFormat,
    image_dir: Option<PathBuf>,
    
    // Content safety
    filter_config: FilterConfig,
    
    // Separators
    markdown_page_separator: Option<String>,
    text_page_separator: Option<String>,
    html_page_separator: Option<String>,
    
    // Hybrid
    hybrid_config: HybridConfig,
}

FilterConfig {
    filter_hidden_text: bool,      // default: true
    filter_out_of_page: bool,      // default: true
    filter_tiny_text: bool,        // default: true
    filter_hidden_ocg: bool,       // default: true
    filter_sensitive_data: bool,   // default: false
    sanitization_rules: Vec<SanitizationRule>,
}

HybridConfig {
    backend: HybridBackend,        // Off, DoclingFast, Hancom
    mode: HybridMode,             // Auto, Full
    url: Option<String>,
    timeout_ms: u64,              // default: 30000
    fallback_to_java: bool,       // default: false
}
```

### 6.2 Enum Definitions

```rust
enum OutputFormat { Json, Text, Html, Pdf, Markdown, MarkdownWithHtml, MarkdownWithImages }
enum TableMethod { Default, Cluster }
enum ReadingOrder { Off, XyCut }
enum ImageOutput { Off, Embedded, External }
enum ImageFormat { Png, Jpeg }
enum HybridBackend { Off, DoclingFast, Hancom }
enum HybridMode { Auto, Full }
```

---

## 7. Error Architecture

### 7.1 Error Hierarchy

```
OpendataLoaderError
+-- PdfError           (PDF loading/parsing failures)
+-- ConfigError        (invalid configuration)
+-- IoError            (file system errors)
+-- HybridError        (backend communication errors)
    +-- HealthCheckFailed
    +-- RequestFailed
    +-- ResponseParseError
    +-- PartialSuccess { failed_pages: Vec<u32> }
+-- OutputError        (output generation failures)
```

### 7.2 Error Propagation

```
main()
  |
  +-- parse_args() -> Result<Config, ConfigError>  [exit 2 on error]
  |
  +-- for each path:
      |
      +-- process_file() -> Result<(), OpendataLoaderError>
          |                 [log error, mark failure, continue]
          |
          +-- load_pdf() -> Result<PdfDocument, PdfError>
          +-- run_pipeline() -> Result<ProcessedDocument, PdfError>
          +-- generate_outputs() -> Result<(), OutputError>
  |
  +-- exit(if any_failed { 1 } else { 0 })
```

---

## 8. Thread Safety Model

### 8.1 Document-Level Parallelism

```
                                    +-- Thread 1: process(doc1)
                                    |
CLI -> file_list -> ThreadPool -----+-- Thread 2: process(doc2)
                                    |
                                    +-- Thread 3: process(doc3)
```

Each document processing is fully independent. No shared mutable state between documents.

### 8.2 Hybrid Client Sharing

The hybrid HTTP client should be shared across threads:
- `reqwest::Client` is `Clone + Send + Sync`
- Connection pooling handles concurrent requests
- Client created once, shared via `Arc<HybridClient>`

### 8.3 Within-Document Sequential

Within a single document, all processing stages run sequentially. The pipeline modifies `ProcessingContext` in place.
