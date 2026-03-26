# EdgeParse

**Fastest PDF extraction engine. Rust-native. Zero GPU, zero JVM, zero OCR models.**

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![crates.io](https://img.shields.io/crates/v/edgeparse-cli.svg)](https://crates.io/crates/edgeparse-cli)
[![PyPI](https://img.shields.io/pypi/v/edgeparse.svg)](https://pypi.org/project/edgeparse/)
[![npm](https://img.shields.io/npm/v/edgeparse.svg)](https://www.npmjs.com/package/edgeparse)

Extract Markdown, JSON (with bounding boxes), and HTML from any born-digital PDF
— deterministically, in milliseconds, on CPU.

- **How accurate is it?** — **0.787 overall** on the latest `opendataloader.org` PDF-to-Markdown benchmark, with the best score in every reported metric: reading order, tables, headings, paragraphs, text quality, table detection, and speed. [Benchmark details](#benchmark)
- **How fast?** — **0.064 s/doc** on the 200-document benchmark corpus on Apple M4 Max. Faster than OpenDataLoader, Docling, PyMuPDF4LLM, MarkItDown, and LiteParse.
- **Does it need a GPU or Java?** — No. No JVM, no GPU, no OCR models, no Python runtime for the CLI. Single ~15 MB binary.
- **RAG / LLM pipelines?** — Yes. Outputs structured Markdown for chunking, JSON with bounding boxes for citations, preserves reading order across multi-column layouts. [See integration examples](#rag--llm-integration)

Available as a **Rust library**, **CLI binary**, **Python package**, **Node.js package**, and **WebAssembly module** for in-browser PDF parsing.

---

## Table of Contents

- [Get Started in 30 Seconds](#get-started-in-30-seconds)
- [What Problems Does This Solve?](#what-problems-does-this-solve)
- [Release Channels](#release-channels)
- [Benchmark](#benchmark)
- [Capability Matrix](#capability-matrix)
- [Installation](#installation)
- [Python SDK](#python-sdk)
- [Node.js SDK](#nodejs-sdk)
- [WebAssembly SDK](#webassembly-sdk)
- [CLI Reference](#cli-reference)
- [Output Formats](#output-formats)
- [RAG / LLM Integration](#rag--llm-integration)
- [Agent Skill](#agent-skill)
- [Architecture](#architecture)
- [FAQ](#faq)
- [Tutorials](#tutorials)
- [Documentation](#documentation)
- [Project Layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

---

## Get Started in 30 Seconds

**Python** (Python 3.9+):

```bash
pip install edgeparse
```

```python
import edgeparse

# Markdown — ready for LLM context or RAG chunking
md = edgeparse.convert("report.pdf", format="markdown")

# JSON with bounding boxes — for citations and element-level control
data = edgeparse.convert("report.pdf", format="json")
```

**CLI** (Rust 1.85+):

```bash
cargo install edgeparse-cli
edgeparse report.pdf --format markdown --output-dir output/
```

**Node.js** (Node 18+):

```bash
npm install edgeparse
```

```js
import { convert } from 'edgeparse';
const md = convert('report.pdf', { format: 'markdown' });
```

---

## Release Channels

Tagged releases publish every supported distribution target through GitHub Actions:

| Channel | Artifact | Install / Pull |
|---------|----------|----------------|
| Rust crates | `pdf-cos`, `edgeparse-core`, `edgeparse-cli` | `cargo install edgeparse-cli` |
| Python SDK | `edgeparse` wheels + sdist | `pip install edgeparse` |
| Node.js SDK | `edgeparse` + 5 platform addons | `npm install edgeparse` |
| WebAssembly SDK | `edgeparse-wasm` | `npm install edgeparse-wasm` |
| CLI binaries | GitHub Release archives for macOS, Linux, Windows | [GitHub Releases](https://github.com/raphaelmansuy/edgeparse/releases) |
| Homebrew | `raphaelmansuy/edgeparse` tap | `brew tap raphaelmansuy/edgeparse && brew install edgeparse` |
| Containers | GHCR + Docker Hub multi-arch images | `docker pull ghcr.io/raphaelmansuy/edgeparse:0.2.1` |

Release automation and registry details: [docs/07-cicd-publishing.md](docs/07-cicd-publishing.md)

---

## What Problems Does This Solve?

| Problem | EdgeParse Solution | Status |
|---------|--------------------|--------|
| PDF text loses reading order in multi-column layouts | XY-Cut++ algorithm preserves correct reading sequence across columns, sidebars, and mixed layouts | ✅ Shipped |
| Table extraction is broken (merged cells, borderless tables) | Ruling-line table detection + borderless cluster method; `--table-method cluster` for complex cases | ✅ Shipped |
| OCR/ML tools add 500 MB+ of dependencies to a simple PDF pipeline | Zero GPU, zero OCR models, zero JVM — single 15 MB binary, pure Rust | ✅ Shipped |
| Heading hierarchy is lost (all text looks the same) | Font-metric + geometry-based heading classifier; MHS score 0.553 on the current benchmark | ✅ Shipped |
| PDFs can carry hidden prompt injection payloads | AI safety filters: hidden text, off-page content, tiny-text, invisible OCG layers detected and stripped | ✅ Shipped |
| Need bounding boxes to cite sources in RAG answers | Every element (`paragraph`, `heading`, `table`, `image`) has `[left, bottom, right, top]` coordinates in PDF points | ✅ Shipped |
| In-browser PDF parsing uploads data to a server | WebAssembly build — full Rust engine in the browser, PDF data never leaves the device | ✅ Shipped |

---

## Benchmark

Evaluated on **200 real-world PDFs** — academic papers, financial reports, multi-column layouts, complex tables, mixed-language documents — running on Apple M4 Max.

### Current comparison set

| Engine | NID ↑ | TEDS ↑ | MHS ↑ | PBF ↑ | TQS ↑ | TD F1 ↑ | Speed ↓ | Overall ↑ |
|--------|------:|-------:|------:|------:|------:|--------:|--------:|----------:|
| **EdgeParse** | **0.889** | **0.596** | **0.553** | **0.559** | **0.920** | **0.901** | **0.064 s/doc** | **0.787** |
| OpenDataLoader | 0.873 | 0.326 | 0.442 | 0.544 | 0.916 | 0.636 | 0.094 s/doc | 0.733 |
| Docling | 0.867 | 0.540 | 0.438 | 0.530 | 0.908 | 0.891 | 0.768 s/doc | 0.745 |
| PyMuPDF4LLM | 0.852 | 0.323 | 0.407 | 0.538 | 0.888 | 0.744 | 0.439 s/doc | 0.710 |
| EdgeParse (pre-frontier baseline) | 0.859 | 0.493 | 0.500 | 0.482 | 0.891 | 0.849 | 0.232 s/doc | 0.751 |
| MarkItDown | 0.808 | 0.193 | 0.001 | 0.362 | 0.861 | 0.558 | 0.149 s/doc | 0.564 |
| LiteParse | 0.815 | 0.000 | 0.001 | 0.383 | 0.887 | N/A | 0.196 s/doc | 0.564 |

EdgeParse now leads the entire comparison set on every reported benchmark metric, including speed. Relative to the previous EdgeParse baseline, the current pipeline increases reading-order accuracy, table structure similarity, paragraph boundaries, text quality, table-detection F1, and overall score while cutting latency from `0.232` to `0.064 s/doc`.

**When to choose what:**

| Use case | Recommendation |
|----------|---------------|
| Born-digital PDFs, latency matters, production deployment | **EdgeParse** — best accuracy/speed, zero dependencies |
| Complex scanned tables, GPU available, batch offline | Consider Docling or MinerU |
| Scanned documents requiring full OCR | Dedicated OCR pipeline |

### Metrics

| Metric | What it measures |
|--------|-----------------|
| **NID** | Reading order accuracy — normalised index distance |
| **TEDS** | Table structure accuracy — tree-edit distance vs. ground truth |
| **MHS** | Heading hierarchy accuracy |
| **PBF** | Paragraph boundary F1 |
| **TQS** | Text quality score |
| **TD F1** | Table detection F1 |
| **Overall** | Normalized aggregate benchmark score |
| **Speed** | Wall-clock seconds per document (full pipeline, 200 docs, parallel) |

### Running the benchmark

```bash
cargo build --release
cd benchmark
uv sync
uv run python run.py          # EdgeParse on all 200 docs
uv run python compare_all.py  # Compare against 9 engines
```

Results → `benchmark/prediction/edgeparse/` · HTML reports → `benchmark/reports/`

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

## Capability Matrix

| Capability | Available | Notes |
|-----------|-----------|-------|
| **Extraction** | | |
| Text with correct reading order | ✅ | XY-Cut++ across columns and sidebars |
| Bounding boxes for every element | ✅ | `[left, bottom, right, top]` in PDF points |
| Table extraction — ruling-line borders | ✅ | Default mode |
| Table extraction — borderless/cluster | ✅ | `--table-method cluster` |
| Heading hierarchy detection | ✅ | Numbered + unnumbered, all levels |
| List detection (numbered, bulleted, nested) | ✅ | |
| Image extraction with coordinates | ✅ | PNG or JPEG, embedded or external |
| Header / footer / watermark filtering | ✅ | |
| Multi-column layout support | ✅ | |
| CMap / ToUnicode font decoding | ✅ | Handles non-standard encodings |
| Tagged PDF structure tree | ✅ | `--use-struct-tree` preserves author intent |
| **Safety** | | |
| Hidden text filtering | ✅ | Prompt injection protection |
| Off-page content filtering | ✅ | |
| Tiny-text / invisible OCG layer filtering | ✅ | |
| PII sanitization | ✅ | `--sanitize` flag |
| **Output** | | |
| Markdown (GFM tables) | ✅ | |
| JSON with bounding boxes | ✅ | |
| HTML5 | ✅ | |
| Plain text (reading order preserved) | ✅ | |
| **SDKs** | | |
| Python (PyO3 native extension) | ✅ | Python 3.9+, pre-built wheels |
| Node.js (NAPI-RS native addon) | ✅ | Node 18+, pre-built addons |
| WebAssembly | ✅ | In-browser, no server required |
| Rust library | ✅ | `edgeparse-core` crate |
| CLI binary | ✅ | `edgeparse-cli` on crates.io |
| **Runtime** | | |
| GPU required | ❌ No | CPU only |
| JVM required | ❌ No | Pure Rust |
| OCR models required | ❌ No | Born-digital PDFs only |
| Parallel processing | ✅ | Rayon per-page parallelism |
| Deterministic / reproducible | ✅ | Same input → same output, always |

---

## Installation

### Python

```bash
pip install edgeparse
```

Python 3.9+. Pre-built wheels for macOS (arm64, x64), Linux (x64, arm64), Windows (x64).

### CLI

```bash
cargo install edgeparse-cli
```

Requires [Rust 1.85+](https://rustup.rs/).

### Rust library

```toml
[dependencies]
edgeparse-core = "0.2.1"
```

Docs: [docs.rs/edgeparse-core](https://docs.rs/edgeparse-core) · [docs.rs/edgeparse-cli](https://docs.rs/edgeparse-cli)

### Node.js

```bash
npm install edgeparse
```

Node 18+. Pre-built native addons for macOS (arm64, x64), Linux (x64, arm64), Windows (x64).

### Build from source

```bash
git clone https://github.com/raphaelmansuy/edgeparse.git
cd edgeparse
cargo build --release
# Binary: target/release/edgeparse
```

### System requirements

- macOS 12+, Linux (glibc 2.17+), or Windows 10+
- ~15 MB binary (stripped release build)
- No Java, no Python (for the CLI), no GPU

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

### `edgeparse.convert_file()`

```python
def convert_file(
    input_path: str | Path,
    output_dir: str | Path = "output",
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
) -> str: ...  # returns output file path
```

### Examples

```python
import edgeparse

# Basic Markdown extraction
md = edgeparse.convert("report.pdf", format="markdown")

# JSON with bounding boxes
json_str = edgeparse.convert("report.pdf", format="json")

# Password-protected, specific pages, borderless tables
md = edgeparse.convert(
    "secure.pdf",
    format="markdown",
    pages="1,3,5-7",
    password="secret",
    reading_order="xycut",
    table_method="cluster",
)

# Write output file  
out_path = edgeparse.convert_file("report.pdf", output_dir="output/", format="markdown")
```

### CLI entry point (Python package)

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

// Markdown
const md = convert('report.pdf', { format: 'markdown' });

// JSON with bounding boxes
const json = convert('report.pdf', { format: 'json' });

// With options
const result = convert('report.pdf', {
  format: 'markdown',
  pages: '1-5',
  readingOrder: 'xycut',
  tableMethod: 'cluster',
});
```

### `ConvertOptions`

```ts
interface ConvertOptions {
  format?: 'markdown' | 'json' | 'html' | 'text';  // default: "markdown"
  pages?: string;         // e.g. "1,3,5-7"
  password?: string;
  readingOrder?: 'xycut' | 'off';        // default: "xycut"
  tableMethod?: 'default' | 'cluster';   // default: "default"
  imageOutput?: 'off' | 'embedded' | 'external';  // default: "off"
}
```

### CLI (npm)

```bash
npx edgeparse report.pdf -f markdown -o output.md
npx edgeparse report.pdf --format json --pages "1-5"
```

---

## WebAssembly SDK

EdgeParse compiles to WebAssembly — **client-side PDF extraction in any modern browser with no server, no uploads, and no backend infrastructure.**

- Same Rust engine, identical output to CLI/Python/Node
- PDF data never leaves the user's device (privacy by design)
- Works offline after initial WASM load (~4 MB cached)
- Zero infrastructure cost — static hosting only

### Quick start

```typescript
import init, { convert_to_string } from 'edgeparse-wasm';

await init();  // load WASM binary once

const bytes = new Uint8Array(await file.arrayBuffer());

const markdown = convert_to_string(bytes, 'markdown');
const json     = convert_to_string(bytes, 'json');
const html     = convert_to_string(bytes, 'html');
```

### API

| Function | Returns | Description |
|----------|---------|-------------|
| `convert(bytes, format?, pages?, readingOrder?, tableMethod?)` | JS object | Structured `PdfDocument` with pages, elements, bounding boxes |
| `convert_to_string(bytes, format?, pages?, readingOrder?, tableMethod?)` | `string` | Formatted output (Markdown, JSON, HTML, or text) |
| `version()` | `string` | EdgeParse version |

### Live demo

**[edgeparse.com/demo/](https://edgeparse.com/demo/)** — drag-and-drop any PDF, all processing runs locally in your browser.

### Build from source

```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
cd crates/edgeparse-wasm
wasm-pack build --target web --release
# Output: crates/edgeparse-wasm/pkg/
```

Full documentation: [docs/09-wasm-sdk.md](docs/09-wasm-sdk.md)

---

## CLI Reference

```
edgeparse [OPTIONS] <PDF_FILE>...
```

### Core options

| Flag | Default | Description |
|------|---------|-------------|
| `-o, --output-dir <DIR>` | — | Write output files to this directory |
| `-f, --format <FMT>` | `json` | Output format(s), comma-separated |
| `-p, --password <PW>` | — | Password for encrypted PDFs |
| `--pages <RANGE>` | — | Page range e.g. `"1,3,5-7"` |
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

Multiple formats: `--format markdown,json`

### Layout & extraction options

| Flag | Default | Description |
|------|---------|-------------|
| `--reading-order <ALGO>` | `xycut` | Reading order: `xycut` or `off` |
| `--table-method <METHOD>` | `default` | Table detection: `default` (ruling lines) or `cluster` (borderless) |
| `--keep-line-breaks` | false | Preserve original line breaks within paragraphs |
| `--use-struct-tree` | false | Use tagged PDF structure tree when available |
| `--include-header-footer` | false | Include headers and footers in output |
| `--sanitize` | false | Enable PII sanitization |
| `--replace-invalid-chars <CH>` | `" "` | Replacement for invalid Unicode characters |

### Image options

| Flag | Default | Description |
|------|---------|-------------|
| `--image-output <MODE>` | `external` | `off`, `embedded` (base64), or `external` (files) |
| `--image-format <FMT>` | `png` | `png` or `jpeg` |
| `--image-dir <DIR>` | — | Directory for extracted image files |

### Separator options

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

## Output Formats

| Format | Flag value | Best for |
|--------|-----------|----------|
| JSON with bounding boxes | `json` | RAG citations, element-level processing, source highlighting |
| Markdown (GFM) | `markdown` | LLM context windows, chunking pipelines, readable output |
| Markdown + HTML tables | `markdown-with-html` | Complex tables that don't render well in pure Markdown |
| Markdown + images | `markdown-with-images` | Documents where figures matter |
| HTML5 | `html` | Web display, accessibility pipelines |
| Plain text | `text` | Keyword search, simple NLP, legacy pipelines |

Combine formats: `--format markdown,json`

### JSON output example

```json
{
  "type": "heading",
  "id": 42,
  "level": "Title",
  "page number": 1,
  "bounding box": [72.0, 700.0, 540.0, 730.0],
  "heading level": 1,
  "font": "Helvetica-Bold",
  "font size": 24.0,
  "content": "Introduction"
}
```

| Field | Description |
|-------|-------------|
| `type` | Element type: `heading`, `paragraph`, `table`, `list`, `image`, `caption` |
| `id` | Unique identifier for cross-referencing |
| `page number` | 1-indexed page reference |
| `bounding box` | `[left, bottom, right, top]` in PDF points (72 pt = 1 inch) |
| `heading level` | Heading depth (1+) |
| `content` | Extracted text |

---

## RAG / LLM Integration

EdgeParse is designed for AI pipelines. Every element has a `bounding box` and `page number`, so you can cite exact sources in answers.

### Extract Markdown for chunking

```python
import edgeparse

md = edgeparse.convert("report.pdf", format="markdown")

# Feed directly into an LLM
response = llm.invoke(f"Summarize this document:\n\n{md}")
```

### Extract JSON for source citations

```python
import json, edgeparse

data = json.loads(edgeparse.convert("report.pdf", format="json"))

for element in data["kids"]:
    if element["type"] == "paragraph":
        # element["bounding box"] → highlight location in original PDF
        # element["page number"] → link back to source page
        print(f"p.{element['page number']}: {element['content'][:80]}")
```

### LangChain integration

EdgeParse has no official LangChain loader yet, but integrates trivially:

```python
from langchain.schema import Document
import edgeparse, json

def load_pdf(path: str) -> list[Document]:
    data = json.loads(edgeparse.convert(path, format="json"))
    docs = []
    for el in data["kids"]:
        if el["type"] in ("paragraph", "heading"):
            docs.append(Document(
                page_content=el["content"],
                metadata={
                    "source": path,
                    "page": el["page number"],
                    "bbox": el["bounding box"],
                    "type": el["type"],
                }
            ))
    return docs
```

### LlamaIndex integration

```python
from llama_index.core import Document
import edgeparse, json

def edgeparse_reader(path: str) -> list[Document]:
    data = json.loads(edgeparse.convert(path, format="json"))
    return [
        Document(
            text=el["content"],
            metadata={"page": el["page number"], "source": path}
        )
        for el in data["kids"]
        if el.get("content")
    ]
```

### Which output format for which use case?

| Use case | Recommended format | Why |
|----------|--------------------|-----|
| Feed PDF to LLM | `markdown` | Clean structure, fits in context window |
| RAG with source citations | `json` | Bounding boxes enable "click-to-source" UX |
| Semantic chunking by section | `markdown` | Headings make natural chunk boundaries |
| Element-level filtering | `json` | Filter by `type`, `page number`, `heading level` |
| Web display | `html` | Styled output with semantic elements |

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

See [docs/08-agent-skill.md](docs/08-agent-skill.md) for the full integration guide (LangChain, LlamaIndex, MCP, CrewAI patterns).

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

## FAQ

### What is the best PDF parser for RAG?

For RAG pipelines, you need a parser that preserves document structure, maintains correct reading order, and provides element coordinates for citations. EdgeParse outputs structured JSON with bounding boxes for every element, handles multi-column layouts with XY-Cut++, and runs locally on CPU without a GPU or JVM. On the current 200-document benchmark it leads the comparison set in both overall score (`0.787`) and latency (`0.064 s/doc`). [See RAG integration examples](#rag--llm-integration).

### How do I cite PDF sources in RAG answers?

Every element in JSON output includes a `bounding box` (`[left, bottom, right, top]` in PDF points, 72 pt = 1 inch) and `page number`. Map the source chunk back to its bounding box to highlight the exact location in the original PDF — enabling "click-to-source" UX. No other non-OCR open-source parser provides bounding boxes for every element by default.

### How do I extract tables from PDF?

EdgeParse detects tables using border (ruling-line) analysis by default. For complex or borderless tables, add `--table-method cluster` (CLI) or `table_method="cluster"` (Python). This uses a text-clustering algorithm to detect table structure without visible borders. On the current benchmark it reaches `0.596` TEDS and `0.901` table-detection F1, both best in the published comparison set.

### Does it work without sending data to the cloud?

Yes. EdgeParse runs 100% locally. No API calls, no data transmission — your documents never leave your environment. The WebAssembly SDK also runs entirely in the browser: the PDF is processed client-side and never uploaded to any server. Ideal for legal, healthcare, and financial documents.

### Does it handle multi-column layouts?

Yes. XY-Cut++ reading order analysis correctly sequences text across multi-column pages, sidebars, and mixed layouts. This works without any configuration change; it is the default reading order algorithm.

### Does it need a GPU or Java?

No. EdgeParse is a pure Rust implementation. It requires no JVM, no GPU, no OCR models, and no Python runtime for the CLI binary. The CLI binary is ~15 MB stripped. On Apple M4 Max, it processes the 200-document benchmark corpus in about `12.7` seconds total.

### How does it compare to Docling, MinerU, and Marker?

| vs. | EdgeParse advantage | Tradeoff |
|-----|---------------------|----------|
| OpenDataLoader | Faster (`0.064` vs `0.094 s/doc`) with stronger table structure and heading recovery | OpenDataLoader remains close on text quality and paragraph boundaries |
| IBM Docling | Faster (`0.064` vs `0.768 s/doc`) with better TEDS and overall score in the current benchmark snapshot | Docling remains a viable OCR-heavy fallback for scanned documents |
| Marker | Faster and materially better on every published metric in this benchmark family | Marker supports scanned PDFs via Surya OCR |
| PyMuPDF4LLM | Faster (`0.064` vs `0.439 s/doc`) with stronger tables, headings, and reading order | PyMuPDF4LLM is simpler if you only need lightweight text extraction |

### Does it support scanned PDFs?

Not directly — EdgeParse is built for born-digital PDFs with embedded fonts. It does not include an OCR engine. For scanned documents, use EdgeParse's hybrid backend option (`--hybrid docling-fast`) which routes complex pages to a Docling-Fast backend running locally.

### What does "deterministic" mean?

Same input PDF → same output, every time. No stochastic models, no floating-point non-determinism from ML inference. This makes EdgeParse safe to use in CI pipelines, regression tests, and compliance workflows where reproducibility is required.

### How do I chunk PDFs for semantic search?

Use `format="markdown"`. EdgeParse preserves heading hierarchy and table structure in Markdown output — headings make natural chunk boundaries for `RecursiveCharacterTextSplitter` (LangChain) or heading-based splitters. For element-level control, use `format="json"` and split on `heading level` boundaries or `page number` changes.

### Does the Python SDK run on Windows?

Yes. Pre-built wheels are available for Windows (x64). The Python package installs from PyPI with no compilation needed:
```bash
pip install edgeparse
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
│   ├── edgeparse-cli/       # CLI binary (clap, 25+ flags)
│   ├── edgeparse-wasm/      # WebAssembly build for browsers (wasm-bindgen)
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
## Star History

<a href="https://www.star-history.com/?repos=raphaelmansuy%2Fedgeparse&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/image?repos=raphaelmansuy/edgeparse&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/image?repos=raphaelmansuy/edgeparse&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/image?repos=raphaelmansuy/edgeparse&type=date&legend=top-left" />
 </picture>
</a>

## License

EdgeParse is licensed under the **Apache License 2.0**. See [LICENSE](LICENSE) for the full text.

The `crates/pdf-cos/` directory is a fork of [lopdf](https://github.com/J-F-Liu/lopdf) (MIT/Apache-2.0 dual-licensed).  
Benchmark PDF documents (`benchmark/pdfs/`) are sourced from publicly available documents and are used solely for evaluation purposes.
