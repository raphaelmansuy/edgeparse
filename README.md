# EdgeParse

**High-performance PDF-to-structured-data extraction engine, written in Rust.**

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/badge/build-passing-brightgreen.svg)]()

EdgeParse converts any digital PDF into Markdown, JSON (with bounding boxes), HTML, or plain text — deterministically, without a JVM, without a GPU, and with best-in-class accuracy on the 200-document benchmark suite included in this repository.

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [CLI Reference](#cli-reference)
- [Architecture](#architecture)
- [Benchmark](#benchmark)
- [Project Layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

---

## Features

| Feature | Status |
|---------|--------|
| Text extraction with correct reading order (XY-Cut++) | ✅ |
| Bounding boxes for every element | ✅ |
| Heading hierarchy detection | ✅ |
| Table extraction (simple and complex/borderless via cluster method) | ✅ |
| List detection (numbered, bulleted, nested) | ✅ |
| Image extraction with coordinates | ✅ |
| Header / footer / watermark filtering | ✅ |
| AI safety filters (hidden text, off-page, tiny-text, invisible layers) | ✅ |
| Multi-column layout support | ✅ |
| CMap / ToUnicode font decoding | ✅ |
| Markdown, JSON, HTML, plain-text output | ✅ |
| Zero JVM dependency | ✅ |
| Deterministic, reproducible output | ✅ |
| Parallel processing via Rayon | ✅ |

---

## Quick Start

### 1. Build

```bash
git clone https://github.com/<your-org>/edgeparse.git
cd edgeparse
cargo build --release
```

The binary is placed at `target/release/edgeparse`.

### 2. Convert a PDF

```bash
# Convert a single PDF to Markdown (stdout)
./target/release/edgeparse examples/pdf/lorem.pdf

# Convert to a specific output directory
./target/release/edgeparse examples/pdf/1901.03003.pdf \
    --output-dir output/ \
    --format markdown

# Convert multiple files at once
./target/release/edgeparse examples/pdf/*.pdf --output-dir output/
```

### 3. Get JSON with bounding boxes

```bash
./target/release/edgeparse examples/pdf/lorem.pdf \
    --format json \
    --output-dir output/
```

---

## Installation

### From source (recommended)

Requires [Rust 1.85+](https://rustup.rs/).

```bash
cargo build --release
# Optionally install to PATH:
cargo install --path crates/edgeparse-cli
```

### System requirements

- macOS 12+, Linux (glibc 2.31+), or Windows 10+
- No Java, no Python, no GPU required
- ~15 MB binary (stripped release build)

---

## CLI Reference

```
edgeparse [OPTIONS] <PDF_FILE>...

Arguments:
  <PDF_FILE>...    One or more PDF files to convert

Options:
  --output-dir <DIR>        Write output files here (default: stdout / current dir)
  --format <FORMAT>         Output format: markdown | json | html | text  [default: markdown]
  --table-method <METHOD>   Table extraction method: simple | cluster  [default: cluster]
  --image-output <MODE>     Image handling: off | embed | extract  [default: off]
  --quiet                   Suppress progress output
  -h, --help                Print help
  -V, --version             Print version
```

### Table Methods

| Method | Description |
|--------|-------------|
| `simple` | Uses ruling lines / borders — fast, works on well-structured tables |
| `cluster` | Grid clustering algorithm — handles borderless and complex tables |

### Output Formats

| Format | Description |
|--------|-------------|
| `markdown` | Standard Markdown with GFM tables |
| `json` | Structured JSON with bounding boxes, element types, page numbers |
| `html` | Full HTML5 document with semantic elements |
| `text` | Plain UTF-8 text, reading order preserved |

---

## Architecture

```
edgeparse/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── edgeparse-cli/        # CLI binary (clap argument parsing, entry point)
│   ├── edgeparse-core/       # Core library
│   │   └── src/
│   │       ├── pdf/        # PDF parsing: page tree, content streams, fonts
│   │       ├── models/     # Data types: Element, BoundingBox, Page, Document
│   │       ├── pipeline/   # Processing stages: text merge, heading detection,
│   │       │               # reading order (XY-Cut++), table detection, lists
│   │       ├── output/     # Renderers: Markdown, JSON, HTML, text
│   │       ├── api/        # Public API surface
│   │       ├── hybrid/     # Optional hybrid-mode HTTP client
│   │       ├── tagged/     # Tagged-PDF structure tree reader
│   │       └── utils/      # Sanitiser, statistics, helpers
│   └── pdf-cos/            # Low-level PDF object model (fork of lopdf 0.39)
```

### Processing pipeline

```
PDF bytes
    │
    ▼
pdf-cos (lopdf fork)          ← COS-level: xref, object graph, streams
    │
    ▼
edgeparse-core::pdf             ← Page tree traversal, content stream parsing,
    │                            font/CMap decoding, image extraction
    ▼
edgeparse-core::pipeline        ← Text chunk merging → line assembly →
    │                            heading detection → XY-Cut++ reading order →
    │                            table cluster detection → list labelling →
    │                            AI-safety filtering
    ▼
edgeparse-core::output          ← Render to Markdown / JSON / HTML / text
```

---

## Benchmark

The `benchmark/` directory contains a full evaluation suite against 200 real-world PDFs (academic papers, multi-column documents, scanned pages) with ground-truth Markdown and element annotations.

### Metrics

| Metric | Description |
|--------|-------------|
| **NID** | Normalised Index Distance — reading order accuracy |
| **TEDS** | Tree-Edit-Distance-based Similarity — table structure accuracy |
| **MHS** | Markdown Heading Similarity — heading hierarchy accuracy |
| **Table Detection F1** | Precision/recall of table presence detection |
| **Speed** | Seconds per document (on benchmark hardware) |

### Running the benchmark

**Prerequisites**: Python 3.11+, [uv](https://docs.astral.sh/uv/)

```bash
# 1. Build edgeparse first
cargo build --release

# 2. Set up Python environment
cd benchmark
uv sync

# 3. Run all 200 documents
uv run python run.py

# 4. Run a single document
uv run python run.py --doc-id 01030000000042

# 5. Check regression thresholds (CI mode)
uv run python run.py --check-regression
```

Results are written to `benchmark/prediction/edgeparse/`.

### Threshold file

`benchmark/thresholds.json` defines minimum acceptable scores:

```json
{
  "nid": 0.85,
  "teds": 0.40,
  "mhs": 0.55,
  "table_detection_f1": 0.55,
  "elapsed_per_doc": 2.0
}
```

### Java comparison (optional)

The benchmark supports side-by-side comparison with the Java-based
[OpenDataLoader PDF](https://github.com/opendataloader-project/opendataloader-pdf) engine.
To enable it:

1. `pip install opendataloader-pdf`
2. Uncomment the `opendataloader` lines in `benchmark/src/engine_registry.py`
3. Run: `uv run python run.py --engine opendataloader`

---

## Project Layout

```
edgeparse/
├── LICENSE                  # Apache 2.0
├── CONTRIBUTING.md
├── README.md
├── Cargo.toml               # Rust workspace
├── Cargo.lock
│
├── crates/                  # Rust source
│   ├── edgeparse-cli/
│   ├── edgeparse-core/
│   └── pdf-cos/
│
├── benchmark/               # Evaluation suite
│   ├── run.py               # Benchmark runner
│   ├── pyproject.toml
│   ├── thresholds.json      # Regression thresholds
│   ├── pdfs/                # 200 benchmark PDFs
│   ├── ground-truth/        # Reference Markdown + element annotations
│   └── src/                 # Python evaluators
│       ├── evaluator.py
│       ├── evaluator_table.py
│       ├── evaluator_heading_level.py
│       ├── evaluator_reading_order.py
│       ├── evaluator_table_detection.py
│       ├── engine_registry.py
│       ├── pdf_parser.py
│       └── pdf_parser_edgeparse.py
│
├── docs/                    # Documentation
│   ├── specs/               # Technical specifications (01–12)
│   ├── benchmark/           # Benchmark metric explanations
│   └── internals/           # Java-to-Rust migration notes
│
├── examples/
│   └── pdf/                 # Sample PDFs for quick testing
│       ├── lorem.pdf
│       ├── 1901.03003.pdf   # Academic paper (multi-column)
│       ├── 2408.02509v1.pdf # Another academic paper
│       └── chinese_scan.pdf # CJK + scan example
│
├── benches/                 # Rust micro-benchmarks (criterion)
└── tests/
    └── fixtures/            # Rust integration test fixtures
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short:

1. Fork, branch from `main`
2. `cargo fmt && cargo clippy -- -D warnings`
3. Run the benchmark to check for regressions
4. Open a PR

---

## License

EdgeParse is licensed under the **Apache License 2.0**. See [LICENSE](LICENSE) for the full text.

The `crates/pdf-cos/` directory is a fork of [lopdf](https://github.com/J-F-Liu/lopdf) (also MIT/Apache-2.0).  
Benchmark PDF documents (`benchmark/pdfs/`) are sourced from publicly available documents and are used solely for evaluation purposes.
