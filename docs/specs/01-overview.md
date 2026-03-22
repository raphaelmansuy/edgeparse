# 01 — Project Overview & Mission Statement

> **Specification version**: 1.0  
> **Scope**: Full rewrite of OpenDataLoader PDF in Rust  
> **Cross-references**: [02-functional-spec](02-functional-spec.md) | [03-technical-architecture](03-technical-architecture.md) | [10-cross-reference-index](10-cross-reference-index.md)

---

## 1. What Is OpenDataLoader PDF?

OpenDataLoader PDF is a **PDF-to-structured-data extraction engine** that converts
any PDF (digital, scanned, tagged) into Markdown, JSON (with bounding boxes), HTML,
plain text, and annotated PDF output. It is also the foundation for a future
**PDF accessibility auto-tagging** pipeline (untagged PDF → Tagged PDF).

### 1.1 Key Differentiators

| # | Differentiator | Detail |
|---|----------------|--------|
| 1 | Deterministic local mode | Rule-based, no GPU, 0.05 s/page on CPU |
| 2 | Bounding boxes for every element | Left/bottom/right/top in PDF points (72 pt = 1 inch) |
| 3 | XY-Cut++ reading order | Based on arXiv:2504.10258, handles multi-column |
| 4 | AI safety filters | Hidden-text, off-page, tiny-text, invisible-layer filtering |
| 5 | Hybrid AI mode | Routes complex pages to backend (docling-fast, Hancom) |
| 6 | Tagged PDF support | Reads existing structure tree for correct semantics |
| 7 | #1 benchmark accuracy | Overall 0.90 in hybrid, 0.93 table accuracy (TEDS) |
| 8 | Multi-SDK | Java core + Python wrapper + Node.js wrapper |
| 9 | Auto-tagging (upcoming) | First open-source end-to-end untagged → Tagged PDF |

### 1.2 Current Architecture (Java)

```
+-------------------------------------------------------------------------+
|                        MONOREPO STRUCTURE                                |
|-------------------------------------------------------------------------|
|                                                                         |
|  options.json  <-- single source of truth for all CLI options            |
|       |                                                                 |
|       +---> generate-options.mjs ---> Node.js .generated.ts             |
|       |                          ---> Python  .generated.py             |
|       |                          ---> MDX docs                          |
|       |                                                                 |
|  schema.json   <-- JSON output schema                                   |
|       |                                                                 |
|       +---> generate-schema.mjs ---> json-schema.mdx                    |
|                                                                         |
|  java/                                                                  |
|  +-- opendataloader-pdf-core/   <-- Core library (76 Java files)        |
|  |   +-- api/                   Config, FilterConfig, OpenDataLoaderPDF |
|  |   +-- containers/            StaticLayoutContainers                  |
|  |   +-- entities/              SemanticFormula, SemanticPicture        |
|  |   +-- processors/            16 processing stages + XY-Cut++        |
|  |   +-- hybrid/                Triage, clients, schema transformers   |
|  |   +-- json/                  JSON serialription + 17 serializers    |
|  |   +-- markdown/              Markdown + Markdown-HTML generators    |
|  |   +-- html/                  Full HTML5 generator                   |
|  |   +-- pdf/                   Annotated PDF writer                   |
|  |   +-- text/                  Plain text generator                   |
|  |   +-- utils/                 Sanitizer, statistics, level info      |
|  |                                                                      |
|  +-- opendataloader-pdf-cli/    <-- CLI (2 Java files, Apache Commons) |
|      +-- CLIMain.java           Entry point, file/dir traversal        |
|      +-- CLIOptions.java        24 options, config construction        |
|                                                                         |
|  python/opendataloader-pdf/                                             |
|  +-- runner.py                  subprocess(java -jar ...)              |
|  +-- wrapper.py                 convert(), run(), main()               |
|  +-- hybrid_server.py           FastAPI + Docling backend              |
|  +-- *_generated.py             Auto-generated from options.json       |
|                                                                         |
|  node/opendataloader-pdf/                                               |
|  +-- src/index.ts               convert(), executeJar()                |
|  +-- src/cli.ts                 Commander.js CLI                       |
|  +-- src/*_generated.ts         Auto-generated from options.json       |
|                                                                         |
|  tests/benchmark/               200-doc quality benchmark suite        |
|  +-- run.py                     NID, TEDS, MHS, Table F1, Speed       |
+-------------------------------------------------------------------------+
```

---

## 2. Mission: Rust Rewrite

### 2.1 Why Rust?

| Concern | Java | Rust |
|---------|------|------|
| Startup latency | ~500ms JVM warmup per invocation | Near-zero startup |
| Memory | GC pauses, 100–300 MB heap | Zero-cost abstractions, ~10–50 MB |
| Distribution | Requires JRE 11+ installed | Single static binary |
| Wrapper overhead | subprocess spawn per call | FFI or direct binary |
| Concurrency | Thread pools + GC pressure | Fearless concurrency, no GC |
| PDF low-level access | Via veraPDF (WCAG library) | Via `lopdf`, `pdf-rs`, custom |

### 2.2 Scope of Rewrite

The Rust rewrite must reproduce **all** of the following:

| Component | Spec Document |
|-----------|---------------|
| CLI interface (24 options, exit codes, batch processing) | [06-cli-interface](06-cli-interface.md) |
| PDF parsing pipeline (16 processing stages) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) |
| Content data model (IObject hierarchy, 20+ types) | [05-data-models](05-data-models.md) |
| Output formatters (JSON, Markdown, HTML, text, PDF) | [08-output-formats](08-output-formats.md) |
| Hybrid mode client (triage, HTTP, schema transform) | [07-hybrid-mode](07-hybrid-mode.md) |
| All algorithms (XY-Cut++, table detection, etc.) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) |
| AI safety / content sanitization | [02-functional-spec](02-functional-spec.md#9) |
| options.json compatibility | [06-cli-interface](06-cli-interface.md) |
| schema.json output compatibility | [08-output-formats](08-output-formats.md) |

### 2.3 Out of Scope (for initial rewrite)

- Python hybrid server (stays in Python — Docling is Python-native)
- Benchmark suite (stays in Python)
- Code generation scripts (stays in Node.js/mjs)
- Documentation site
- PDF/UA enterprise features
- Accessibility studio

### 2.4 Compatibility Requirements

1. **CLI compatibility**: Identical option names, types, defaults, exit codes
2. **JSON output compatibility**: Identical schema (field names, types, structure)
3. **Markdown output compatibility**: Same formatting rules
4. **HTML output compatibility**: Same structure and tags
5. **Benchmark score parity**: NID ≥ 0.85, TEDS ≥ 0.40, MHS ≥ 0.55, F1 ≥ 0.55

---

## 3. Document Map

```
specs/
+-- 01-overview.md .................... THIS FILE — mission, scope, orientation
+-- 02-functional-spec.md ............. User-facing features and behaviors
+-- 03-technical-architecture.md ...... System architecture, module boundaries
+-- 04-pdf-parsing-pipeline.md ........ 20-stage processing pipeline in detail
+-- 05-data-models.md ................. Complete type hierarchy and data structures
+-- 06-cli-interface.md ............... CLI options, validation, batch processing
+-- 07-hybrid-mode.md ................. Triage algorithm, HTTP protocol, backends
+-- 08-output-formats.md .............. JSON, Markdown, HTML, text, PDF formatters
+-- 09-rust-migration-guide.md ........ Crate selection, architecture for Rust
+-- 10-cross-reference-index.md ....... Master index linking all specs together
```

### Reading Order

For implementers:
1. Start with **01-overview** (this file) for orientation
2. Read **02-functional-spec** for user-facing requirements
3. Read **03-technical-architecture** for system design
4. Read **05-data-models** for the complete type system
5. Read **04-pdf-parsing-pipeline** for the core algorithm
6. Read **06-cli-interface** through **08-output-formats** for interface details
7. Read **09-rust-migration-guide** for Rust-specific recommendations
8. Use **10-cross-reference-index** as an ongoing reference

---

## 4. Terminology

| Term | Definition |
|------|-----------|
| **IObject** | Root interface of all content elements in the document model |
| **TextChunk** | Atomic text fragment with font, size, color, bounding box |
| **TextLine** | Horizontal sequence of TextChunks on the same baseline |
| **TextBlock** | Group of TextLines forming a visual block |
| **SemanticTextNode** | Text block with semantic meaning (paragraph, heading, caption) |
| **TableBorder** | Detected table with rows, columns, and cells |
| **PDFList** | Detected ordered or unordered list with items |
| **BoundingBox** | Rectangle [leftX, bottomY, rightX, topY] in PDF points |
| **XY-Cut++** | Reading order algorithm based on recursive projection cuts |
| **Triage** | Hybrid mode page classification (JAVA vs BACKEND routing) |
| **TEDS** | Tree Edit Distance-based Similarity (table accuracy metric) |
| **NID** | Normalized Information Distance (reading order metric) |
| **MHS** | Markdown Hierarchical Similarity (heading structure metric) |
| **Tagged PDF** | PDF with structure tree defining semantic elements |
| **Structure tree** | PDF internal tree of tagged content (headings, paragraphs, etc.) |
| **veraPDF** | Java PDF validation library used as the parsing foundation |
| **Docling** | IBM's document understanding AI used as hybrid backend |

---

## 5. Version History

| Date | Version | Author | Changes |
|------|---------|--------|---------|
| 2026-03-18 | 1.0 | Spec Generation | Initial specification for Rust rewrite |
