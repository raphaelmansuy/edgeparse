# EdgeParse

**High-performance PDF-to-structured-data extraction engine, written in Rust.**

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)

EdgeParse converts any digital PDF into Markdown, JSON (with bounding boxes), HTML, or plain text — deterministically, without a JVM, without a GPU, and with best-in-class accuracy on the 200-document benchmark suite included in this repository.

Available as a **Rust library**, **CLI binary**, **Python package** (`edgeparse`), and **Node.js package** (`@edgeparse/pdf`).

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Installation](#installation)
- [CLI Reference](#cli-reference)
- [Python SDK](#python-sdk)
- [Node.js SDK](#nodejs-sdk)
- [Architecture](#architecture)
- [Benchmark](#benchmark)
- [Documentation](#documentation)
- [Project Layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

---

## Features

| Feature | Status |
|---------|--------|
| Text extraction with correct reading order (XY-Cut++) | ✅ |
| Bounding boxes for every element | ✅ |
| Heading hierarchy detection (numbered + unnumbered) | ✅ |
| Table extraction — ruling-line and borderless (cluster method) | ✅ |
| List detection (numbered, bulleted, nested) | ✅ |
| Image extraction with coordinates | ✅ |
| Header / footer / watermark filtering | ✅ |
| AI safety filters (hidden text, off-page, tiny-text, invisible OCG layers) | ✅ |
| Multi-column layout support | ✅ |
| CMap / ToUnicode font decoding | ✅ |
| Tagged PDF structure tree support | ✅ |
| Markdown, JSON, HTML, plain-text output | ✅ |
| Python SDK (PyO3 native extension) | ✅ |
| Node.js SDK (NAPI-RS native addon) | ✅ |
| Batch processing API | ✅ |
| Hybrid backend support (Docling-Fast) | ✅ |
| Zero JVM dependency | ✅ |
| Deterministic, reproducible output | ✅ |
| Parallel per-page processing via Rayon | ✅ |

---

## Quick Start

### CLI

```bash
git clone https://github.com/raphaelmansuy/edgeparse.git
cd edgeparse
cargo build --release
```

The binary is placed at `target/release/edgeparse`.

```bash
# Convert to JSON with bounding boxes (default)
./target/release/edgeparse examples/pdf/lorem.pdf --output-dir output/

# Convert to Markdown
./target/release/edgeparse examples/pdf/1901.03003.pdf \
    --format markdown --output-dir output/

# Convert multiple files, specific page range
./target/release/edgeparse examples/pdf/*.pdf \
    --format markdown --pages "1-5" --output-dir output/
```

### Python

```python
import edgeparse

# Convert to Markdown (returns a string)
md = edgeparse.convert("report.pdf", format="markdown")

# Convert to JSON
json_str = edgeparse.convert("report.pdf", format="json")

# Write output file directly
out_path = edgeparse.convert_file("report.pdf", output_dir="output/", format="markdown")

# Extract specific pages of a password-protected PDF
md = edgeparse.convert(
    "secure.pdf",
    format="markdown",
    pages="1,3,5-7",
    password="secret",
    reading_order="xycut",
    table_method="cluster",
)
```

### Node.js

```js
import { convert } from '@edgeparse/pdf';

// Convert to Markdown (returns a string)
const md = convert('report.pdf', { format: 'markdown' });

// Convert to JSON
const json = convert('report.pdf', { format: 'json' });

// With options
const result = convert('report.pdf', {
  format: 'markdown',
  pages: '1-5',
  readingOrder: 'xycut',
  tableMethod: 'cluster',
});
```

---

## Installation

### Rust CLI (from source)

Requires [Rust 1.85+](https://rustup.rs/).

```bash
cargo build --release
# Or install to PATH:
cargo install --path crates/edgeparse-cli
```

### Python

```bash
pip install edgeparse
```

Requires Python 3.9+. Pre-built wheels for macOS (arm64, x64), Linux (x64, arm64), and Windows (x64).

```bash
# Or build from source with maturin:
cd sdks/python
pip install maturin
maturin develop --release
```

### Node.js

```bash
npm install @edgeparse/pdf
```

Requires Node.js 18+. Pre-built native addons for macOS (arm64, x64), Linux (x64, arm64), and Windows (x64).

### System requirements

- macOS 12+, Linux (glibc 2.31+), or Windows 10+
- No Java, no Python (for the CLI), no GPU required
- ~15 MB binary (stripped release build)

---

## CLI Reference

```
edgeparse [OPTIONS] <PDF_FILE>...
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<PDF_FILE>...` | One or more PDF files to convert (required) |

### Core options

| Flag | Default | Description |
|------|---------|-------------|
| `-o, --output-dir <DIR>` | — | Write output files to this directory |
| `-f, --format <FMT>` | `json` | Output format(s), comma-separated (see below) |
| `-p, --password <PW>` | — | Password for encrypted PDFs |
| `--pages <RANGE>` | — | Page range, e.g. `"1,3,5-7"` |
| `-q, --quiet` | false | Suppress log output |

### Output format values

| Value | Description |
|-------|-------------|
| `json` | Structured JSON with bounding boxes and element types (default) |
| `markdown` | Standard Markdown with GFM tables |
| `markdown-with-html` | Markdown with HTML table fallback for complex tables |
| `markdown-with-images` | Markdown with embedded or linked images |
| `html` | Full HTML5 document with semantic elements |
| `text` | Plain UTF-8 text, reading order preserved |

Multiple formats can be combined: `--format markdown,json`

### Layout & extraction options

| Flag | Default | Description |
|------|---------|-------------|
| `--reading-order <ALGO>` | `xycut` | Reading order: `xycut` or `off` |
| `--table-method <METHOD>` | `default` | Table detection: `default` (ruling lines) or `cluster` (borderless) |
| `--keep-line-breaks` | false | Preserve original line breaks within paragraphs |
| `--use-struct-tree` | false | Use tagged PDF structure tree when available |
| `--include-header-footer` | false | Include headers and footers in output |
| `--sanitize` | false | Enable PII sanitization |
| `--replace-invalid-chars <CH>` | `" "` | Replacement string for invalid Unicode characters |

### Image options

| Flag | Default | Description |
|------|---------|-------------|
| `--image-output <MODE>` | `external` | `off`, `embedded` (base64), or `external` (files) |
| `--image-format <FMT>` | `png` | Image format: `png` or `jpeg` |
| `--image-dir <DIR>` | — | Directory for extracted image files |

### Output separator options

| Flag | Description |
|------|-------------|
| `--markdown-page-separator <STR>` | String inserted between Markdown pages |
| `--text-page-separator <STR>` | String inserted between plain-text pages |
| `--html-page-separator <STR>` | String inserted between HTML pages |

### Content safety options

| Flag | Default | Description |
|------|---------|-------------|
| `--content-safety-off <FLAGS>` | — | Disable safety filters: `all`, `hidden-text`, `off-page`, `tiny`, `hidden-ocg` |

### Hybrid backend options

| Flag | Default | Description |
|------|---------|-------------|
| `--hybrid <BACKEND>` | `off` | Hybrid backend: `off` or `docling-fast` |
| `--hybrid-mode <MODE>` | `auto` | Triage mode: `auto` or `full` |
| `--hybrid-url <URL>` | — | Hybrid backend service URL |
| `--hybrid-timeout <MS>` | `30000` | Timeout in milliseconds |
| `--hybrid-fallback` | false | Fall back to local extraction on hybrid error |

---

## Python SDK

**Package:** `edgeparse` · **Requires:** Python 3.9+ · **Source:** [`sdks/python/`](sdks/python/)

### `edgeparse.convert()`

```python
def convert(
    input_path: str | Path,
    *,
    format: str = "markdown",       # "markdown", "json", "html", "text"
    pages: str | None = None,        # e.g. "1,3,5-7"
    password: str | None = None,
    reading_order: str = "xycut",   # "xycut" or "off"
    table_method: str = "default",  # "default" or "cluster"
    image_output: str = "off",      # "off", "embedded", "external"
) -> str: ...
```

Returns the extracted content as a string.

### `edgeparse.convert_file()`

```python
def convert_file(
    input_path: str | Path,
    output_dir: str | Path = "output",
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
) -> str: ...
```

Writes the output file to `output_dir` and returns the output file path.

### CLI (Python package)

The Python package also installs an `edgeparse` CLI entry point:

```bash
edgeparse report.pdf -f markdown -o output/
edgeparse *.pdf --format json --output-dir out/ --pages "1-3"
```

---

## Node.js SDK

**Package:** `@edgeparse/pdf` · **Requires:** Node.js 18+ · **Source:** [`sdks/node/`](sdks/node/)

### `convert()`

```ts
import { convert } from '@edgeparse/pdf';

function convert(inputPath: string, options?: ConvertOptions): string
```

### `ConvertOptions`

```ts
interface ConvertOptions {
  format?: string;        // "markdown" | "json" | "html" | "text"  (default: "markdown")
  pages?: string;         // e.g. "1,3,5-7"
  password?: string;
  readingOrder?: string;  // "xycut" | "off"  (default: "xycut")
  tableMethod?: string;   // "default" | "cluster"  (default: "default")
  imageOutput?: string;   // "off" | "embedded" | "external"  (default: "off")
}
```

### CLI (Node.js package)

```bash
npx @edgeparse/pdf report.pdf -f markdown -o output.md
npx @edgeparse/pdf report.pdf --format json --pages "1-5"
```

---

## Architecture

### Crate structure

```
edgeparse/
├── crates/
│   ├── pdf-cos/            # Low-level PDF object model (fork of lopdf 0.39)
│   ├── edgeparse-core/     # Core extraction engine (~90 source files)
│   │   └── src/
│   │       ├── api/        # ProcessingConfig, FilterConfig, BatchResult
│   │       ├── pdf/        # Loader, content stream parser, font/CMap decoding
│   │       ├── models/     # ContentElement, BoundingBox, TextChunk, PdfDocument
│   │       ├── pipeline/   # 20-stage orchestrator + Rayon parallel helpers
│   │       ├── output/     # Renderers: JSON, Markdown, HTML, text, CSV
│   │       ├── tagged/     # Tagged-PDF structure tree → McidMap
│   │       └── utils/      # XY-Cut++ algorithm, sanitizer, layout analysis
│   ├── edgeparse-cli/      # CLI binary (clap derive, 25+ flags)
│   ├── edgeparse-python/   # PyO3 native extension
│   └── edgeparse-node/     # NAPI-RS native addon
└── sdks/
    ├── python/             # Python packaging (maturin, pyproject.toml)
    └── node/               # npm packaging (TypeScript wrapper, tsup)
```

### Processing pipeline (20 stages)

```
PDF file
    │
    ▼
pdf-cos                   ← xref parsing, object graph, encrypted streams
    │
    ▼
edgeparse-core::pdf       ← page tree, content stream operators, font decoding,
    │                       CMap/ToUnicode, image extraction, tagged PDF
    ▼
edgeparse-core::pipeline  ← 20 sequential/parallel stages:
    │  [page range] → [watermark] → [filter] → [table borders] →
    │  [cell assignment] → [boxed headings] → [column detection] →
    │  [TextLine assembly] → [TextBlock grouping] → [table clustering] →
    │  [header/footer] → [list detection] → [paragraph] → [figure] →
    │  [heading classification] → [XY-Cut++ reading order] →
    │  [list pass 2] → [caption/footnote/TOC] → [cross-page tables] →
    │  [element nesting] → [final reading order] → [sanitize]
    ▼
edgeparse-core::output    ← render to JSON / Markdown / HTML / text
```

Stages marked `par_map_pages` run in parallel via Rayon; cross-page stages run sequentially.

---

## Benchmark

The `benchmark/` directory contains a full evaluation suite against real-world PDFs — academic papers, multi-column layouts, tables, scanned pages — with ground-truth Markdown and element annotations.

### Metrics

| Metric | Description |
|--------|-------------|
| **NID** | Normalised Index Distance — reading order accuracy |
| **TEDS** | Tree-Edit-Distance-based Similarity — table structure accuracy |
| **MHS** | Markdown Heading Similarity — heading hierarchy accuracy |
| **Table Detection F1** | Precision / recall of table presence detection |
| **Speed** | Seconds per document |

### Running the benchmark

**Prerequisites:** Python 3.11+, [uv](https://docs.astral.sh/uv/)

```bash
# 1. Build edgeparse
cargo build --release

# 2. Set up Python environment
cd benchmark
uv sync

# 3. Run all documents
uv run python run.py

# 4. Run against a single engine
uv run python run.py --engine edgeparse

# 5. Compare engines
uv run python compare_all.py
```

Results are written to `benchmark/prediction/edgeparse/`.  
HTML reports are written to `benchmark/reports/`.

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

### Supported engines

The benchmark can compare multiple engines side by side:

| Engine | Notes |
|--------|-------|
| `edgeparse` | This project (default) |
| `docling` | IBM Docling |
| `marker` | VikParuchuri/marker |
| `markitdown` | Microsoft MarkItDown |
| `mineru` | MinerU |
| `pymupdf4llm` | PyMuPDF4LLM |
| `opendataloader` | OpenDataLoader PDF |
| `edgequake` | EdgeQuake service |

---

## Documentation

Technical documentation lives in [`docs/`](docs/):

| Document | Description |
|----------|-------------|
| [docs/00-overview.md](docs/00-overview.md) | Project overview, goals, and design philosophy |
| [docs/01-architecture.md](docs/01-architecture.md) | Crate structure, module map, data-flow diagram |
| [docs/02-pipeline.md](docs/02-pipeline.md) | All 20 pipeline stages with ASCII diagrams |
| [docs/03-data-model.md](docs/03-data-model.md) | Type hierarchy: `ContentElement`, `BoundingBox`, `PdfDocument` |
| [docs/04-pdf-extraction.md](docs/04-pdf-extraction.md) | PDF loader, chunk parser, font/CMap decoding |
| [docs/05-output-formats.md](docs/05-output-formats.md) | JSON schema, Markdown renderer, HTML/text/CSV output |
| [docs/06-sdk-integration.md](docs/06-sdk-integration.md) | CLI flag reference, Python SDK API, Node.js SDK API, Batch API |

---

## Project Layout

```
edgeparse/
├── LICENSE
├── CONTRIBUTING.md
├── README.md
├── Cargo.toml               # Rust workspace (5 members)
├── Cargo.lock
│
├── crates/
│   ├── pdf-cos/             # lopdf 0.39 fork — low-level PDF object model
│   ├── edgeparse-core/      # Core extraction engine (~90 source files)
│   ├── edgeparse-cli/       # CLI binary (clap, 25+ flags)
│   ├── edgeparse-python/    # PyO3 native Python extension
│   └── edgeparse-node/      # NAPI-RS native Node.js addon
│
├── sdks/
│   ├── python/              # Python wheel packaging (maturin + pyproject.toml)
│   │   └── edgeparse/       # Pure-Python wrapper + CLI entry point
│   └── node/                # npm packaging (TypeScript + tsup + vitest)
│       └── src/             # index.ts, types.ts, cli.ts
│
├── benchmark/               # Evaluation suite
│   ├── run.py               # Benchmark runner
│   ├── compare_all.py       # Multi-engine comparison
│   ├── pyproject.toml
│   ├── thresholds.json      # Regression thresholds
│   ├── pdfs/                # Benchmark PDFs
│   ├── ground-truth/        # Reference Markdown and JSON annotations
│   ├── prediction/          # Per-engine output directories
│   ├── reports/             # HTML benchmark reports
│   └── src/                 # Python evaluators and engine parsers
│
├── docs/                    # Technical documentation (Markdown)
│
├── examples/
│   └── pdf/                 # Sample PDFs for quick testing
│       ├── lorem.pdf
│       ├── 1901.03003.pdf   # Academic paper (multi-column)
│       ├── 2408.02509v1.pdf # Academic paper
│       └── chinese_scan.pdf # CJK + scan example
│
├── benches/                 # Rust micro-benchmarks (criterion)
├── docker/                  # Dockerfile and Dockerfile.dev
├── scripts/                 # bench.sh, publish-crates.sh
└── tests/
    └── fixtures/            # Rust integration test fixtures
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short:

1. Fork, branch from `main`
2. `cargo fmt && cargo clippy -- -D warnings`
3. Run the benchmark to check for regressions: `cd benchmark && uv run python run.py --engine edgeparse`
4. Open a PR

---

## License

EdgeParse is licensed under the **Apache License 2.0**. See [LICENSE](LICENSE) for the full text.

The `crates/pdf-cos/` directory is a fork of [lopdf](https://github.com/J-F-Liu/lopdf) (MIT/Apache-2.0 dual-licensed).  
Benchmark PDF documents (`benchmark/pdfs/`) are sourced from publicly available documents and are used solely for evaluation purposes.
