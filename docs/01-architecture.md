# EdgeParse — System Architecture

> All diagrams reflect actual source code. Every box maps to a real Rust module.

---

## 01 · Crate Dependency Graph

```
       ┌─────────────────┐
       │   edgeparse-cli  │  (binary)
       └────────┬────────┘
                │ depends on
       ┌────────▼────────────────────────────┐
       │        edgeparse-core               │  (library)
       │                                     │
       │  api/  models/  pdf/  pipeline/     │
       │  output/  tagged/  utils/           │
       └────────┬────────────────────────────┘
                │ depends on
       ┌────────▼────────┐   ┌──────────────────────┐
       │    pdf-cos       │   │  external crates      │
       │  (lopdf 0.39.0  │   │  rayon, serde,        │
       │   fork/rename)  │   │  clap, image,         │
       └─────────────────┘   │  regex, tokio, etc.   │
                             └──────────────────────┘

       ┌──────────────────────┐    ┌──────────────────────┐
       │  edgeparse-python    │    │   edgeparse-node     │
       │  (PyO3 FFI wrapper)  │    │   (NAPI-RS wrapper)  │
       └──────────┬───────────┘    └──────────┬───────────┘
                  └──────────────────────────▼
                              edgeparse-core
```

**Workspace root:** [`Cargo.toml`](../Cargo.toml)
**pdf-cos alias:** declared as `lopdf = { package = "pdf-cos", path = "crates/pdf-cos" }` so all existing `use lopdf::…` code compiles unchanged.

---

## 02 · Module Map (edgeparse-core)

```
crates/edgeparse-core/src/
│
├── lib.rs                    ← Top-level convert() entry point
│
├── api/
│   ├── mod.rs
│   ├── config.rs             ← ProcessingConfig (24 fields), enums
│   ├── filter.rs             ← FilterConfig — hidden text, off-page, tiny
│   ├── config_loader.rs      ← Optional JSON config loading
│   └── batch.rs              ← BatchResult, process_batch()
│
├── models/
│   ├── mod.rs
│   ├── bbox.rs               ← BoundingBox  (geometry primitive)
│   ├── chunks.rs             ← TextChunk, ImageChunk, LineChunk, LineArtChunk
│   ├── text.rs               ← TextLine, TextBlock, TextColumn
│   ├── table.rs              ← TableBorder, TableBorderRow, TableBorderCell
│   ├── list.rs               ← PDFList, PDFListItem
│   ├── semantic.rs           ← SemanticParagraph, SemanticHeading, etc.
│   ├── document.rs           ← PdfDocument (root output type)
│   ├── content.rs            ← ContentElement enum (unified)
│   └── enums.rs              ← SemanticType, TextFormat, TextType, ...
│
├── pdf/
│   ├── mod.rs
│   ├── loader.rs             ← load_pdf() → RawPdfDocument
│   ├── chunk_parser.rs       ← Single-pass content stream walker → PageChunks
│   ├── text_extractor.rs     ← Text-only extractor (legacy path, still used)
│   ├── line_extractor.rs     ← Line/path geometry extraction
│   ├── image_extractor.rs    ← XObject / inline image extraction
│   ├── font.rs               ← Font resolution, CMap decoding, glyph → Unicode
│   ├── graphics_state.rs     ← CTM, color state, text state stack
│   ├── page_info.rs          ← PageInfo: MediaBox, CropBox, Rotation
│   ├── encryption.rs         ← Password-based decryption
│   ├── annotation_extractor.rs
│   ├── annotation_enrichment.rs
│   ├── bookmark_extractor.rs
│   ├── hyperlink_extractor.rs
│   ├── form_extractor.rs
│   ├── metadata_writer.rs
│   └── raster_table_ocr.rs   ← Image-based table border recovery
│
├── pipeline/
│   ├── mod.rs
│   ├── orchestrator.rs       ← run_pipeline() — sequences all 20 stages
│   ├── parallel.rs           ← par_map_pages() / par_map_pages_indexed()
│   ├── logging.rs
│   ├── error_recovery.rs
│   └── stages/
│       ├── mod.rs
│       ├── watermark_detector.rs
│       ├── content_filter.rs
│       ├── content_sanitizer.rs
│       ├── table_detector.rs
│       ├── table_content_assigner.rs
│       ├── cluster_table_detector.rs
│       ├── boxed_heading_promoter.rs
│       ├── column_detector.rs
│       ├── text_line_grouper.rs
│       ├── text_block_grouper.rs
│       ├── header_footer.rs
│       ├── list_detector.rs
│       ├── list_pass2.rs
│       ├── paragraph_detector.rs
│       ├── figure_detector.rs
│       ├── heading_detector.rs
│       ├── id_assignment.rs
│       ├── caption_linker.rs
│       ├── footnote_detector.rs
│       ├── footnote_linker.rs
│       ├── toc_detector.rs
│       ├── cross_page_linker.rs
│       ├── nesting_level.rs
│       ├── reading_order.rs
│       └── output_builder.rs
│
├── output/
│   ├── mod.rs
│   ├── json.rs               ← Canonical JSON format
│   ├── legacy_json.rs        ← Legacy/compatibility JSON
│   ├── markdown.rs           ← Markdown (with special-case handling)
│   ├── html.rs               ← HTML5
│   ├── text.rs               ← Plain text
│   ├── csv.rs                ← CSV (tables)
│   ├── docx.rs               ← DOCX (stub)
│   └── toc_builder.rs        ← TOC extraction helper
│
├── tagged/
│   ├── mod.rs
│   ├── struct_tree.rs        ← extract_struct_tree(), build_mcid_map()
│   └── processor.rs          ← TaggedProcessor
│
└── utils/
    ├── mod.rs
    ├── xycut.rs              ← XY-Cut++ reading order algorithm
    ├── layout_analysis.rs    ← Column geometry analysis
    ├── font_metrics_cache.rs
    ├── image_dedup.rs
    ├── language_detector.rs
    ├── page_range.rs         ← parse_page_range(), filter_pages()
    ├── sanitizer.rs          ← PII removal
    ├── statistics.rs
    ├── text_normalizer.rs
    ├── xref_index.rs
    └── diff.rs
```

---

## 03 · Threading Model

```
                              Main thread
                                  │
                    ┌─────────────▼──────────────┐
                    │   run_pipeline(state)        │
                    │   orchestrator.rs            │
                    └─────────────┬───────────────┘
                                  │
                 ┌────────────────┼────────────────┐
                 │                │                │
         PAGE-PARALLEL STAGES     │      CROSS-PAGE STAGES
         (rayon thread pool)      │      (single-threaded)
                 │                │                │
    par_map_pages() calls         │   header_footer::detect_headers_footers()
                                  │   heading_detector::detect_headings()
    table_detector        ← CPU   │   cross_page_linker::link_cross_page_tables()
    text_line_grouper     ← CPU   │   reading_order::sort_reading_order()
    text_block_grouper    ← CPU   │   list_pass2::detect_common_prefix_lists_document()
    list_detector         ← CPU   │   id_assignment::assign_ids()
    paragraph_detector    ← CPU   │
    figure_detector       ← CPU   │
    content_sanitizer     ← CPU   │
```

**Key:** `par_map_pages` in [`parallel.rs`](../crates/edgeparse-core/src/pipeline/parallel.rs#L24) uses Rayon's work-stealing thread pool.
Cross-page stages operate on `&mut state.pages` sequentially because they compare elements across page boundaries.

---

## 04 · Data Flow (End-to-End)

```
PDF File (bytes)
    │
    ▼ pdf::loader::load_pdf()         [loader.rs]
RawPdfDocument
  ├── document: lopdf::Document
  └── metadata: PdfMetadata
    │
    ▼ pdf::page_info::extract_page_info()  [page_info.rs]
Vec<PageInfo>  (MediaBox, CropBox, Rotation per page)
    │
    ▼ pdf::chunk_parser::extract_page_chunks()  [chunk_parser.rs]
PageChunks (per page)
  ├── text_chunks:    Vec<TextChunk>    (font run atoms)
  ├── image_chunks:   Vec<ImageChunk>   (XObject / inline)
  ├── line_chunks:    Vec<LineChunk>    (path geometry)
  └── line_art_chunks: Vec<LineArtChunk> (compound paths)
    │
    ▼  Merge into ContentElement variants
Vec<ContentElement>  per page  →  PipelineState.pages
    │
    ▼ pipeline::orchestrator::run_pipeline()  [orchestrator.rs]
  Stage 0b ─ Page Range Filter
  Stage 1b ─ Watermark Removal
  Stage 2  ─ Content Filter (hidden, off-page, tiny, OCG)
  Stage 2b ─ Replace U+FFFD
  ...
  Stage 18 ─ Reading Order (XY-Cut++)
  Stage 19 ─ Content Sanitization
                │
                ▼
PipelineState.pages  (semantic ContentElements)
    │
    ▼ Build PdfDocument                    [lib.rs#L103]
PdfDocument
  ├── file_name, number_of_pages, metadata
  └── kids: Vec<ContentElement>  (all pages flattened, reading order)
    │
    ▼ output::{json,markdown,html,text}::to_*()
String  (serialised output)
```

---

## 05 · Memory Layout

All page content lives as `Vec<ContentElement>` (a Rust enum — no heap-boxing per element).
`PipelineState` holds a `Vec<Vec<ContentElement>>` — one inner Vec per page.
Stages that work per-page use `std::mem::take` to avoid double-borrow violations:

```rust
// parallel.rs — par_map_pages pattern
let results: Vec<PageContent> = std::mem::take(pages)
    .into_par_iter()
    .map(|page| op(page))
    .collect();
*pages = results;
```

**Source:** [`parallel.rs#L27`](../crates/edgeparse-core/src/pipeline/parallel.rs#L27)

---

## 06 · Configuration Surface

`ProcessingConfig` has 24 fields — see full definition at [`config.rs`](../crates/edgeparse-core/src/api/config.rs):

```
ProcessingConfig
├── output_dir         : Option<String>
├── password           : Option<String>
├── formats            : Vec<OutputFormat>  {Json,Text,Html,Pdf,Markdown,...}
├── quiet              : bool
├── filter_config      : FilterConfig       ← content safety
├── sanitize           : bool               ← PII removal
├── keep_line_breaks   : bool
├── replace_invalid_chars : String          ← default " "
├── use_struct_tree    : bool
├── table_method       : TableMethod        {Default, Cluster}
├── reading_order      : ReadingOrder       {Off, XyCut}
├── markdown_page_separator : Option<String>
├── text_page_separator     : Option<String>
├── html_page_separator     : Option<String>
├── image_output       : ImageOutput        {Off, Embedded, External}
├── image_format       : ImageFormat        {Png, Jpeg}
├── image_dir          : Option<String>
├── pages              : Option<String>     ← "1,3,5-7"
├── include_header_footer : bool
├── hybrid             : HybridBackend      {Off, DoclingFast}
├── hybrid_mode        : HybridMode         {Auto, Full}
├── hybrid_url         : Option<String>
├── hybrid_timeout     : u64                ← ms, default 30000
└── hybrid_fallback    : bool
```

`FilterConfig` (content safety):
```
FilterConfig
├── filter_hidden_text   : bool  (low contrast ratio)
├── filter_out_of_page   : bool  (outside CropBox)
├── filter_tiny_text     : bool  (below min height)
└── filter_hidden_ocg    : bool  (invisible OCG layers)
```
**Source:** [`filter.rs`](../crates/edgeparse-core/src/api/filter.rs)

---

## Cross-Reference

| Topic | Document |
|-------|---------|
| Full pipeline stage sequence | [02-pipeline.md](02-pipeline.md) |
| Type hierarchy | [03-data-model.md](03-data-model.md) |
| PDF chunk parsing internals | [04-pdf-extraction.md](04-pdf-extraction.md) |
| Output renderers | [05-output-formats.md](05-output-formats.md) |
| CLI / SDK APIs | [06-sdk-integration.md](06-sdk-integration.md) |
