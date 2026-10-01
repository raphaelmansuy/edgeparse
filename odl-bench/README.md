# odl-bench — Official OpenDataLoader board harness

Separate scoreboard that matches the public
[opendataloader-bench](https://github.com/opendataloader-project/opendataloader-bench)
formula:

```
overall = mean(NID, TEDS, MHS)   # null metrics omitted per document
```

This harness does **not** fold text quality, paragraph F1, or table-detection F1
into overall. The EdgeParse-native harness under [`../benchmark/`](../benchmark/)
is unchanged and uses a different overall.

## Setup

```bash
# Clone upstream corpus (Git LFS) once
git clone https://github.com/opendataloader-project/opendataloader-bench.git \
  odl-bench/.upstream/opendataloader-bench
cd odl-bench/.upstream/opendataloader-bench && git lfs pull && cd ../../..

# Symlinks (created by scaffolding; recreate if needed)
ln -sfn .upstream/opendataloader-bench/ground-truth/markdown odl-bench/ground-truth
ln -sfn .upstream/opendataloader-bench/pdfs odl-bench/pdfs

cd odl-bench
uv sync
# Optional competitors:
uv sync --extra opendataloader
uv sync --extra compare-light
uv sync --extra docling
uv sync --extra unstructured
```

Pinned upstream commit: see [`UPSTREAM.txt`](UPSTREAM.txt).

Metric modules under `src/evaluator_*.py` are vendored Apache-2.0 from upstream
so scores match their code.

## Run

```bash
# Build EdgeParse first
cargo build --release -p edgeparse-cli

# Deterministic EdgeParse on all 200 docs
uv run python run.py --engine edgeparse

# Hybrid (requires opendataloader-pdf-hybrid on 127.0.0.1:5002)
uv run python run.py --engine edgeparse_hybrid

# Competitors
uv run python run.py --engine opendataloader
uv run python run.py --engine opendataloader_hybrid
uv run python run.py --engine pymupdf4llm
uv run python run.py --engine markitdown
uv run python run.py --engine docling
uv run python run.py --engine unstructured

# Refresh scores without re-parsing
uv run python run.py --engine edgeparse --skip-parse

# Board report
uv run python compare.py
```

Outputs:

- `prediction/<engine>/evaluation.json`
- `reports/board.json`

## Engines

| Engine | Notes |
|--------|--------|
| `edgeparse` | Local `target/release/edgeparse`, `--table-method cluster` |
| `edgeparse_hybrid` | Same + `--hybrid docling-fast --hybrid-fallback` |
| `edgeparse_wasm` | Node `wasm-pack` pkg; prefer `npm run bench:wasm-sdk` (product path via `@edgeparse/web` Node adapter + PP-OCR), or legacy `scripts/run_wasm_bench.mjs`; then `--skip-parse`. CI gate: `npm run gate:wasm-sdk` (TEDS ≥ 0.776, overall ≥ 0.854). |
| `opendataloader` | Published CLI |
| `opendataloader_hybrid` | ODL + Docling Fast |
| `docling` / `pymupdf4llm` / `markitdown` / `unstructured` | Pip engines |

**Not run:** Marker (GPL), MinerU (AGPL), Nutrient (commercial). A missing row
is not a win — only engines that finish all 200 docs count as finished
competitors.

## Board numbers

Formula: `overall = mean(NID, TEDS, MHS)` on upstream commit `7af1d8f` (200 docs).

| Engine | Overall | NID | TEDS | MHS | Status |
|--------|---------|-----|------|-----|--------|
| **EdgeParse [hybrid]** | **0.900** | **0.917** | **0.928** | **0.836** | finished |
| Docling | 0.891 | 0.905 | 0.925 | 0.827 | finished |
| OpenDataLoader [hybrid] | 0.877 | 0.915 | 0.676 | 0.822 | finished |
| PyMuPDF4LLM | 0.873 | 0.916 | 0.803 | 0.772 | finished |
| EdgeParse | 0.857 | 0.894 | 0.773 | 0.763 | finished |
| EdgeParse [wasm] | 0.854 | 0.891 | 0.776 | 0.761 | finished |
| OpenDataLoader | 0.842 | 0.912 | 0.483 | 0.757 | finished |
| MarkItDown | 0.589 | 0.844 | 0.273 | 0.000 | finished |
| Unstructured | 0.086 | 0.093 | 0.000 | 0.054 | partial (20/200) |

Not run: Marker (GPL), MinerU (AGPL), Nutrient (commercial).

`EdgeParse [wasm]` is scored via the **product path** first
(`@edgeparse/web` Node adapter + optional PP-OCRv6 host):

```bash
make wasm-build-node
cd odl-bench && npm i
npm run bench:wasm-sdk          # scripts/run_wasm_sdk_bench.mjs
uv run python -c "import sys; sys.path.insert(0,'src'); from evaluator import run; run('ground-truth','prediction','evaluation.json',target_engine='edgeparse_wasm')"
npm run gate:wasm-sdk           # TEDS ≥ 0.776, overall ≥ 0.854
```

Legacy in-process path (ocrs feature):

```bash
wasm-pack build crates/edgeparse-wasm --target nodejs --out-dir pkg-node --release --features ocr-ocrs
cd odl-bench && npm i
node scripts/run_wasm_bench.mjs
```

Committed metric snapshots: [`reports/`](reports/).

### Progress vs starting point

| | Overall | NID | TEDS | MHS |
|--|---------|-----|------|-----|
| Published EdgeParse (board) | 0.837 | 0.894 | 0.717 | 0.706 |
| This harness baseline | 0.838 | 0.895 | 0.717 | 0.707 |
| EdgeParse after TOC/MHS geometry | 0.848 | 0.896 | 0.717 | 0.744 |
| EdgeParse + raster OCR / form tables | 0.859 | 0.898 | 0.759 | 0.765 |
| EdgeParse + projection lattice / OCR lattice | 0.857 | 0.894 | 0.773 | 0.763 |
| EdgeParse WASM (pre: no heading/inmem OCR) | 0.854 | 0.894 | 0.734 | 0.763 |
| EdgeParse WASM (heading + XObject + ocrs) | 0.856 | 0.892 | 0.771 | 0.765 |
| **EdgeParse WASM (PP-OCRv6_small host + ocrs)** | **0.854** | **0.891** | **0.776** | **0.761** |
| EdgeParse hybrid (pre-win) | 0.885 | 0.899 | 0.922 | 0.796 |
| **EdgeParse hybrid (leader every metric)** | **0.900** | **0.917** | **0.928** | **0.836** |

**Leader among finished engines on every metric** (NID, TEDS, MHS, and overall vs Docling / PyMuPDF4LLM / ODL).

### Method (first principles)

- **Hybrid merge:** replace triaged pages in reading order (never append duplicates). Prefer full backend markdown when information-complete (≥85% non-whitespace mass vs local); image routes keep local when backend is shorter.
- **Hybrid triage (geometry + PDF structure):** tables/`TableBorder`, H∩V ruling lattices, orphan column grids (adaptive 1D clustering), multi-column block layout, large image/figure coverage, PDF `/Outlines` + typographic heading-inventory deficit, title fragmentation.
- **Table lattice pick:** among local / Docling Fast / DocumentConverter, choose by effective column arity (discount empty spacer columns) then filled-cell count; image-coverage routes always invite DocumentConverter (Fast OCR under-fills).
- **Docling Fast JSON → Markdown:** canonical `DoclingDocument.export_to_markdown` (body `$ref` order); Rust body-walk fallback. Unique temp paths per conversion.
- **Local raster OCR:** Tesseract/RapidOCR on figure-embedded grids; ruled lattices recovered before chart/photo skips (worksheet rasters were false-positive bar charts).
- **Projection lattice:** 1D x-center / baseline histograms recover sparse numeric grids (MACRS rates, etc.); skipped on image pages so OCR tables are not rebuilt from spilled tokens.
- **Blank-form tables:** stub-label worksheets with empty body cells recovered via projection (header columns + left stubs).
- **TOC / headings:** one `#` title + plain TOC entries; demote figure/table caption prefixes; outline + typography promotion (≥1.15× body mode / bold above body weight).
- **TEDS spans:** HTML emission for `col_span`/`row_span`.

## Relationship to `benchmark/`

| | `benchmark/` | `odl-bench/` |
|--|--------------|--------------|
| Overall | Includes text quality (+ more) | mean(NID, TEDS, MHS) only |
| Ground truth | Local / shared copy | Symlink to upstream clone |
| Purpose | EdgeParse regression + product score | Match public leaderboard |
