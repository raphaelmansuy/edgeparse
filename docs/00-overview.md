# EdgeParse — Project Overview

> **"Code is law"** — every statement here is traceable to a source file.

EdgeParse is a high-performance PDF-to-structured-data extraction engine written in Rust.
It converts raw PDF byte streams into semantically-rich, machine-readable output (JSON, Markdown, HTML, plain text) using a deterministic 20-stage processing pipeline.

---

## Repository Layout

```
edgepdf/
├── Cargo.toml                          ← Workspace root: 5 crates
│
├── crates/
│   ├── pdf-cos/                        ← Low-level PDF object model (fork of lopdf 0.39.0)
│   ├── edgeparse-core/                 ← The extraction engine (all business logic)
│   │   ├── src/
│   │   │   ├── lib.rs                  ← Public API: convert()
│   │   │   ├── api/                    ← Config, filter, batch
│   │   │   ├── models/                 ← Type system: BoundingBox → ContentElement
│   │   │   ├── pdf/                    ← PDF loading & chunk extraction
│   │   │   ├── pipeline/               ← 20-stage orchestrator + stages
│   │   │   ├── output/                 ← Renderers: JSON, MD, HTML, text
│   │   │   ├── tagged/                 ← Tagged-PDF structure tree
│   │   │   └── utils/                  ← XY-Cut, layout analysis, sanitizer
│   │   └── tests/
│   ├── edgeparse-cli/                  ← Binary: edgeparse <input> [opts]
│   ├── edgeparse-python/               ← PyO3 extension: edgeparse.convert()
│   └── edgeparse-node/                 ← NAPI-RS extension: edgeparse.convert()
│
├── sdks/
│   ├── python/                         ← Python package (wraps edgeparse-python)
│   └── node/                           ← Node.js package (wraps edgeparse-node)
│
├── benchmark/                          ← Accuracy benchmark suite (Python)
├── benches/                            ← Rust micro-benchmarks (criterion)
└── tests/                              ← Integration test fixtures
```

**Workspace definition:** [`Cargo.toml`](../Cargo.toml)

---

## Ten-Second Mental Model

```
                      ┌────────────────────────────────────────────────────────┐
  PDF File            │                   edgeparse-core                        │
  ─────────           │                                                          │
   bytes ──▶ pdf-cos  │  chunk       pipeline        output                    │
             (lopdf)  │  parser ──▶ orchestrator ──▶ renderers ──▶ String/File │
                      │  (Stage 0)  (Stages 1–19)   (Stage 20)                 │
                      └────────────────────────────────────────────────────────┘
                                          │
                              ┌───────────┼───────────┐
                              │           │           │
                           CLI        Python SDK   Node.js SDK
```

The engine is pure Rust and has **zero Python/Node runtime dependencies at parse time**.
The Python and Node.js SDKs are thin FFI wrappers — all logic lives in `edgeparse-core`.

---

## Public Contracts

| Layer | Entry point | Return type |
|-------|------------|-------------|
| Rust API | [`edgeparse_core::convert()`](../crates/edgeparse-core/src/lib.rs#L42) | `Result<PdfDocument, EdgePdfError>` |
| CLI | [`edgeparse-cli/src/main.rs`](../crates/edgeparse-cli/src/main.rs) | Exit code + files on disk |
| Python | [`edgeparse_core.convert()`](../crates/edgeparse-python/src/lib.rs#L31) | `str` |
| Node.js | [`convert()`](../crates/edgeparse-node/src/lib.rs#L22) | `string` |

---

## Key Design Decisions

| Decision | Rationale | Code location |
|----------|-----------|---------------|
| Single-pass chunk parser | Avoids multiple content-stream decodes | [`chunk_parser.rs`](../crates/edgeparse-core/src/pdf/chunk_parser.rs) |
| Rayon parallel per-page | Pages are independent; saturates CPU cores | [`parallel.rs`](../crates/edgeparse-core/src/pipeline/parallel.rs) |
| Mutable `PipelineState` | Avoids cloning large page vectors between stages | [`orchestrator.rs`](../crates/edgeparse-core/src/pipeline/orchestrator.rs) |
| `ContentElement` enum | Single type models all abstraction levels (raw→semantic) | [`content.rs`](../crates/edgeparse-core/src/models/content.rs) |
| pdf-cos (lopdf fork) | Custom encryption/CMaps without upstream release dependency | [`crates/pdf-cos/`](../crates/pdf-cos/) |
| XY-Cut++ reading order | Handles multi-column layouts without ML dependency | [`xycut.rs`](../crates/edgeparse-core/src/utils/xycut.rs) |

---

## Error Architecture

```
EdgePdfError  (lib.rs)
├── LoadError(String)          ← pdf::loader fails to open/decrypt
├── PipelineError{stage, msg}  ← any stage returns Err
├── OutputError(String)        ← renderer write failure
├── IoError(std::io::Error)    ← file I/O (#[from])
├── ConfigError(String)        ← invalid config values
└── LopdfError(String)         ← pdf-cos object errors (#[from] lopdf::Error)
```

**Source:** [`crates/edgeparse-core/src/lib.rs#L124`](../crates/edgeparse-core/src/lib.rs#L124)

---

## Cross-Reference to Other Docs

| Document | What you'll learn |
|----------|------------------|
| [01-architecture.md](01-architecture.md) | Crate dependency graph, module structure, threading model |
| [02-pipeline.md](02-pipeline.md) | All 20 stages with inputs/outputs and code pointers |
| [03-data-model.md](03-data-model.md) | Full type hierarchy from `BoundingBox` to `PdfDocument` |
| [04-pdf-extraction.md](04-pdf-extraction.md) | Content stream parsing, font resolution, graphics state |
| [05-output-formats.md](05-output-formats.md) | Renderers: JSON, Markdown, HTML, text |
| [06-sdk-integration.md](06-sdk-integration.md) | CLI flags, Python API, Node.js API |
