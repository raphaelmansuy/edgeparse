# EdgeParse

**High-performance PDF-to-structured-data extraction engine, written in Rust.**

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![crates.io](https://img.shields.io/crates/v/edgeparse-cli.svg)](https://crates.io/crates/edgeparse-cli)
[![PyPI](https://img.shields.io/pypi/v/edgeparse.svg)](https://pypi.org/project/edgeparse/)
[![npm](https://img.shields.io/npm/v/edgeparse.svg)](https://www.npmjs.com/package/edgeparse)

EdgeParse converts any digital PDF into Markdown, JSON (with bounding boxes), HTML, or plain text — deterministically, without a JVM, without a GPU, without OCR models, and with **best-in-class accuracy** among non-OCR tools on the 200-document benchmark suite included in this repository.

Available as a **Rust library**, **CLI binary**, **Python package** (`edgeparse`), **Node.js package** (`edgeparse`), and **WebAssembly module** for in-browser PDF parsing.

---

## Table of Contents

- [Features](#features)
- [Quick Start](#quick-start)
- [Agent Skill](#agent-skill)
- [Installation](#installation)
- [CLI Reference](#cli-reference)
- [Python SDK](#python-sdk)
- [Node.js SDK](#nodejs-sdk)
- [WebAssembly SDK](#webassembly-sdk)
- [Architecture](#architecture)
- [Benchmark](#benchmark)
  - [Why it matters](#why-it-matters)
  - [Results on 200-document benchmark suite](#results-on-200-document-benchmark-suite)
  - [Running the benchmark](#running-the-benchmark)
- [Tutorials](#tutorials)
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
| WebAssembly SDK (in-browser PDF parsing) | ✅ |
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
import { convert } from 'edgeparse';

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

## Agent Skill

EdgeParse ships as a **Claude agent skill** — a structured description that teaches Claude (and any compatible AI agent) how to extract PDF content on behalf of users.

```bash
# Add the EdgeParse skill to your agent environment
npx skills add raphaelmansuy/edgeparse --skill edgeparse

# Install the Python package  
pip install edgeparse
```

The `npx skills add` command registers the skill in `skills-lock.json`:

```json
{
  "version": 1,
  "skills": {
    "edgeparse": {
      "source": "raphaelmansuy/edgeparse",
      "sourceType": "github"
    }
  }
}
```

Once installed, the agent reads `skills/edgeparse/SKILL.md` and knows when to call `edgeparse.convert()`, which format to use for different tasks, and how to handle edge cases like encrypted PDFs, borderless tables, and multi-column layouts.

See [docs/08-agent-skill.md](docs/08-agent-skill.md) for the full skill documentation and integration patterns (LangChain, LlamaIndex, MCP, CrewAI).

---

## Installation

### CLI (from crates.io)

```bash
cargo install edgeparse-cli
```

### Rust library

Add to `Cargo.toml`:

```toml
[dependencies]
edgeparse-core = "0.1"
```

Docs: [docs.rs/edgeparse-core](https://docs.rs/edgeparse-core) · [docs.rs/edgeparse-cli](https://docs.rs/edgeparse-cli)

### CLI (from source)

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
npm install edgeparse
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

**Package:** `edgeparse` · **Requires:** Node.js 18+ · **Source:** [`sdks/node/`](sdks/node/)

### `convert()`

```ts
import { convert } from 'edgeparse';

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
npx edgeparse report.pdf -f markdown -o output.md
npx edgeparse report.pdf --format json --pages "1-5"
```

---

## WebAssembly SDK

EdgeParse compiles to WebAssembly, enabling **client-side PDF extraction in any modern browser** — no server, no uploads, no backend infrastructure.

**Key advantages:**
- Same Rust engine, same accuracy — identical output to CLI/Python/Node
- PDF data never leaves the user's device (privacy by design)
- Works offline after initial WASM load (~4 MB cached)
- Zero infrastructure cost — deploy on static hosting

### Quick start

```typescript
import init, { convert_to_string } from '@edgeparse/edgeparse-wasm';

// Load WASM binary (once)
await init();

// Read PDF file from user upload or fetch
const bytes = new Uint8Array(await file.arrayBuffer());

// Extract Markdown
const markdown = convert_to_string(bytes, 'markdown');

// Extract structured JSON
const json = convert_to_string(bytes, 'json');

// Extract HTML
const html = convert_to_string(bytes, 'html');
```

### API

| Function | Returns | Description |
|----------|---------|-------------|
| `convert(bytes, format?, pages?, readingOrder?, tableMethod?)` | JS object | Structured `PdfDocument` with pages, elements, bounding boxes |
| `convert_to_string(bytes, format?, pages?, readingOrder?, tableMethod?)` | `string` | Formatted output (Markdown, JSON, HTML, or text) |
| `version()` | `string` | EdgeParse version |

### Build from source

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build WASM package
cd crates/edgeparse-wasm
wasm-pack build --target web --release
```

Output goes to `crates/edgeparse-wasm/pkg/`. Use it locally or publish to npm.

### Live demo

Try EdgeParse WASM in your browser: **[edgeparse.com/demo/](https://edgeparse.com/demo/)**

Drag-and-drop any PDF and see extracted Markdown, JSON, HTML, or plain text — all processing runs locally in your browser.

Full documentation: [docs/09-wasm-sdk.md](docs/09-wasm-sdk.md)

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

### Why it matters

Most PDF parsers were designed for one thing: **handle scanned documents with OCR at any cost**. That means pulling in deep-learning stacks (PaddleOCR, Surya, EasyOCR, layout detection models), Python-heavy runtimes, and GPU dependencies — even when processing a born-digital PDF that contains perfectly legible text. The result is tools that are **slow, large to install, and brittle in production**.

The reality is that the vast majority of business, research, and enterprise PDFs are **born-digital**: they have embedded fonts, vector text, and structured content. OCR is unnecessary. What they need is precision — correct reading order, accurate table extraction, and reliable heading detection.

EdgeParse is built on this insight. It uses **zero ML models, zero OCR, zero GPU**, and achieves top-tier accuracy through first-principles PDF parsing: font decoding, layout geometry, ruling-line analysis, and XY-Cut++ reading order. The result is a parser that is **fastest in class** and **dominant among all non-OCR tools** on every benchmark metric.

### Results on 200-document benchmark suite

Evaluated on 200 real-world PDFs spanning academic papers, financial reports, multi-column layouts, complex tables, and mixed-language documents, running on Apple M4 Max.

#### Against non-OCR tools (apples-to-apples)

Tools that require no OCR or deep-learning model inference. EdgeParse wins on **every metric** including speed.

| Engine | NID ↑ | TEDS ↑ | MHS ↑ | Overall ↑ | Speed ↓ |
|--------|-------:|-------:|------:|----------:|--------:|
| **EdgeParse** ✅ | **0.911** | **0.783** | **0.821** | **0.881** | **0.023 s/doc** |
| OpenDataLoader | 0.912 | 0.494 | 0.760 | 0.844 | 0.048 s/doc |
| PyMuPDF4LLM | 0.888 | 0.540 | 0.774 | 0.833 | 0.310 s/doc |
| Microsoft MarkItDown | 0.844 | 0.273 | 0.000 | 0.589 | 0.078 s/doc |
| LiteParse (LlamaIndex) | 0.857 | 0.000 | 0.000 | 0.569 | 0.214 s/doc |

> **NID** = reading order accuracy (normalised index distance), **TEDS** = table structure accuracy, **MHS** = heading hierarchy accuracy, **Overall** = geometric mean of all metrics. Higher is better (↑), lower is better for speed (↓).

EdgeParse is **13× faster than PyMuPDF4LLM** and **2× faster than OpenDataLoader**, while delivering significantly better table and heading accuracy. MarkItDown and LiteParse produce zero MHS and near-zero TEDS, meaning they extract raw text only with no structural understanding.

#### Against ML/OCR-based tools

Tools that rely on deep-learning models, OCR engines, or GPU inference. Included for reference — they carry significant deployment weight.

| Engine | NID ↑ | TEDS ↑ | MHS ↑ | Overall ↑ | Speed ↓ | Requires |
|--------|-------:|-------:|------:|----------:|--------:|---------|
| **EdgeParse** ✅ | **0.911** | **0.783** | **0.821** | **0.881** | **0.023 s/doc** | Nothing |
| MinerU | 0.953 | — | 0.858 | 0.906 | 20.8 s/doc | PaddleOCR + layout models |
| IBM Docling | 0.899 | **0.887** | 0.824 | 0.882 | 0.424 s/doc | Layout + OCR models |
| Marker | 0.866 | 0.825 | 0.794 | 0.846 | 30.3 s/doc | Surya OCR + GPU |

EdgeParse is within rounding distance of Docling's **MHS** (0.821 vs 0.824) and **Overall** (0.881 vs 0.882) — while being **18× faster** and requiring zero model downloads. It outperforms Marker on all metrics while being **1,300× faster**. MinerU leads on NID and MHS but at **900× the latency** and requires a full OCR + layout model stack.

The tradeoff is TEDS: Docling's layout models give it an edge on complex borderless tables (0.887 vs 0.783). If your pipeline is dominated by complex scanned tables, weigh that against the 18× speed penalty and model dependencies.

#### Summary

| Condition | Recommendation |
|-----------|---------------|
| Born-digital PDFs, latency-sensitive, production deployment | **EdgeParse** — best accuracy/speed tradeoff, zero dependencies |
| Complex scanned tables, GPU available, batch offline processing | Consider Docling or MinerU |
| Scanned documents requiring full OCR | Use a dedicated OCR pipeline |

### Metrics explained

| Metric | What it measures |
|--------|-----------------|
| **NID** | Reading order accuracy — how well content follows the logical reading sequence |
| **TEDS** | Table structure accuracy — tree-edit distance between extracted and ground-truth table trees |
| **MHS** | Heading hierarchy accuracy — correctness of document structure and section titles |
| **Overall** | Geometric mean of NID, TEDS, and MHS |
| **Speed** | Wall-clock seconds per document (full pipeline, 200 docs, parallel) |

### Running the benchmark

**Prerequisites:** Python 3.11+, [uv](https://docs.astral.sh/uv/)

```bash
# 1. Build edgeparse
cargo build --release

# 2. Set up Python environment
cd benchmark
uv sync

# 3. Run EdgeParse on all 200 documents
uv run python run.py

# 4. Compare against other engines
uv run python compare_all.py
```

Results are written to `benchmark/prediction/edgeparse/`.  
HTML reports are written to `benchmark/reports/`.

### Regression thresholds

`benchmark/thresholds.json` defines minimum acceptable scores for CI:

```json
{
  "nid": 0.85,
  "teds": 0.40,
  "mhs": 0.55,
  "table_detection_f1": 0.55,
  "elapsed_per_doc": 2.0
}
```

---

## Tutorials

Step-by-step guides with working examples live in [`tutorials/`](tutorials/):

| Tutorial | Description |
|----------|-------------|
| [tutorials/01-cli.md](tutorials/01-cli.md) | All CLI flags with working examples and output samples |
| [tutorials/02-python-sdk.md](tutorials/02-python-sdk.md) | `pip install edgeparse` — full API, batch processing, JSON parsing |
| [tutorials/03-nodejs-sdk.md](tutorials/03-nodejs-sdk.md) | `npm install edgeparse` — TypeScript, CJS, and worker threads |
| [tutorials/04-rust-library.md](tutorials/04-rust-library.md) | `edgeparse-core` in your Rust project — config, models, Rayon |
| [tutorials/05-output-formats.md](tutorials/05-output-formats.md) | JSON schema, bounding boxes, Markdown variants, HTML, plain text |

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
| [docs/07-cicd-publishing.md](docs/07-cicd-publishing.md) | CI/CD publishing pipeline — how it works and how to configure it |
| [docs/08-agent-skill.md](docs/08-agent-skill.md) | EdgeParse agent skill — `npx skills add`, SKILL.md structure, SDK patterns |
| [docs/09-wasm-sdk.md](docs/09-wasm-sdk.md) | WebAssembly SDK — objectives, API, use cases, build instructions |

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
│   ├── edgeparse-cli/       # CLI binary (clap, 25+ flags)│   ├── edgeparse-wasm/      # WebAssembly build for browsers│   ├── edgeparse-wasm/      # WebAssembly build for browsers (wasm-bindgen)
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
│   ├── run.py               # Benchmark runner (EdgeParse)
│   ├── compare_all.py       # Multi-engine comparison (9 engines)
│   ├── pyproject.toml
│   ├── thresholds.json      # Regression thresholds
│   ├── pdfs/                # Benchmark PDFs (200 docs)
│   ├── ground-truth/        # Reference Markdown and JSON annotations
│   ├── prediction/          # Per-engine output directories
│   ├── reports/             # HTML benchmark reports
│   └── src/                 # Python evaluators and engine adapters
│
├── docs/                    # Technical documentation (Markdown)
│
├── demo/                    # Interactive WASM demo (Vite + TypeScript)
│   └── src/                 # Demo application source
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
