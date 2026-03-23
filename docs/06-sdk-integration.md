# 06 — SDK Integration

> Code is law. Every statement in this document is traced to a source file.

This document covers all three integration surfaces for EdgeParse:

1. **Rust CLI** — `crates/edgeparse-cli/src/main.rs`
2. **Python SDK** — `sdks/python/edgeparse/` + `crates/edgeparse-python/src/lib.rs`
3. **Node.js SDK** — `sdks/node/src/` + `crates/edgeparse-node/src/lib.rs`
4. **Batch API** — `crates/edgeparse-core/src/api/batch.rs`

---

## Integration surfaces overview

```
┌─────────────────────────────────────────────────────────┐
│                     edgeparse-core                      │
│  convert(path, config) → PdfDocument                    │
└──────────────────────────┬──────────────────────────────┘
                           │
           ┌───────────────┼────────────────┐
           │               │                │
    ┌──────▼──────┐ ┌──────▼──────┐ ┌──────▼──────┐
    │  edgeparse  │ │  edgeparse  │ │  edgeparse  │
    │    -cli     │ │   -python   │ │    -node    │
    │  (Rust bin) │ │ (PyO3 ext.) │ │(NAPI-RS add)│
    └──────┬──────┘ └──────┬──────┘ └──────┬──────┘
           │               │                │
      CLI flags       sdks/python/     sdks/node/
      (clap)          (maturin wrap)   (TS wrapper)
```

All three surfaces call the same `edgeparse_core::convert()` function and the
same output renderers. Parameter names differ by convention (Rust: `snake_case`,
Python: `snake_case`, TypeScript: `camelCase`) but map to identical
`ProcessingConfig` fields.

---

## 1. Rust CLI

**Source:** [`crates/edgeparse-cli/src/main.rs`](../crates/edgeparse-cli/src/main.rs)  
**Binary name:** `edgeparse`  
**Argument parser:** `clap` 4 (derive macro)

### Build & install

```bash
# Build
cargo build --release
# Binary: target/release/edgeparse

# Install to PATH
cargo install --path crates/edgeparse-cli
```

### Basic usage

```bash
edgeparse [OPTIONS] <PDF_FILE>...
```

At least one `<PDF_FILE>` is required. Multiple files are processed in sequence.

### Complete flag reference

All flags are defined in the `Cli` struct in `main.rs`. The mapping to
`ProcessingConfig` fields is performed by `build_config()`.

#### Core flags

| Flag | Type | Default | `ProcessingConfig` field | Description |
|------|------|---------|--------------------------|-------------|
| `<PDF_FILE>...` | Vec\<PathBuf\> | — | — | Input PDF files (required) |
| `-o, --output-dir <DIR>` | Option\<String\> | — | `output_dir` | Output directory; if omitted, writes next to input |
| `-f, --format <FMT>` | Option\<String\> | `json` | `formats` | Comma-separated output formats (see below) |
| `-p, --password <PW>` | Option\<String\> | — | `password` | Password for encrypted PDFs |
| `--pages <RANGE>` | Option\<String\> | — | `pages` | Page range e.g. `"1,3,5-7"` |
| `-q, --quiet` | bool | false | `quiet` | Suppress log output |

#### Format values

`--format` accepts a comma-separated list. When omitted the default is `json`.

| Value | `OutputFormat` variant | Description |
|-------|------------------------|-------------|
| `json` | `Json` | Structured JSON with bounding boxes (default) |
| `markdown` | `Markdown` | Standard Markdown with GFM tables |
| `markdown-with-html` | `MarkdownWithHtml` | Markdown with HTML table fallback |
| `markdown-with-images` | `MarkdownWithImages` | Markdown with image references |
| `html` | `Html` | Full HTML5 document |
| `text` | `Text` | Plain UTF-8 text |
| `pdf` | `Pdf` | *(not yet implemented, silently skipped)* |

Multiple formats: `--format markdown,json`

#### Layout & extraction flags

| Flag | Type | Default | `ProcessingConfig` field | Description |
|------|------|---------|--------------------------|-------------|
| `--reading-order <ALGO>` | String | `xycut` | `reading_order` | `xycut` or `off` |
| `--table-method <METHOD>` | String | `default` | `table_method` | `default` (ruling lines) or `cluster` (borderless) |
| `--keep-line-breaks` | bool | false | `keep_line_breaks` | Preserve original line breaks |
| `--use-struct-tree` | bool | false | `use_struct_tree` | Use tagged PDF structure tree |
| `--include-header-footer` | bool | false | `include_header_footer` | Include headers/footers in output |
| `--sanitize` | bool | false | `sanitize` | Enable PII sanitisation |
| `--replace-invalid-chars <CH>` | String | `" "` | `replace_invalid_chars` | Replacement for invalid Unicode |

#### Image flags

| Flag | Type | Default | `ProcessingConfig` field | Description |
|------|------|---------|--------------------------|-------------|
| `--image-output <MODE>` | String | `external` | `image_output` | `off`, `embedded` (base64), `external` (files) |
| `--image-format <FMT>` | String | `png` | `image_format` | `png` or `jpeg` |
| `--image-dir <DIR>` | Option\<String\> | — | `image_dir` | Directory for extracted image files |

#### Page separator flags

| Flag | Type | Default | `ProcessingConfig` field |
|------|------|---------|--------------------------|
| `--markdown-page-separator <STR>` | Option\<String\> | — | `markdown_page_separator` |
| `--text-page-separator <STR>` | Option\<String\> | — | `text_page_separator` |
| `--html-page-separator <STR>` | Option\<String\> | — | `html_page_separator` |

#### Content safety flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--content-safety-off <FLAGS>` | Option\<String\> | — | Disable filters: `all`, `hidden-text`, `off-page`, `tiny`, `hidden-ocg` |

Parsed by `FilterConfig::apply_safety_off()` in `crates/edgeparse-core/src/api/filter.rs`.

#### Hybrid backend flags

| Flag | Type | Default | `ProcessingConfig` field | Description |
|------|------|---------|--------------------------|-------------|
| `--hybrid <BACKEND>` | String | `off` | `hybrid` | `off` or `docling-fast` |
| `--hybrid-mode <MODE>` | String | `auto` | `hybrid_mode` | `auto` or `full` |
| `--hybrid-url <URL>` | Option\<String\> | — | `hybrid_url` | Hybrid service endpoint |
| `--hybrid-timeout <MS>` | u64 | `30000` | `hybrid_timeout` | Timeout in milliseconds |
| `--hybrid-fallback` | bool | false | `hybrid_fallback` | Fall back on hybrid error |

### `build_config()` mapping

`build_config()` in `main.rs` maps every `Cli` field to a `ProcessingConfig`.
The only non-trivial mappings are:

```
cli.format  →  formats: Vec<OutputFormat>   (split on ',', parsed per-token)
cli.table_method  →  "cluster" ⇒ Cluster, else Default
cli.reading_order →  "off" ⇒ Off, else XyCut
cli.image_output  →  "off" ⇒ Off | "embedded" ⇒ Embedded | else External
cli.image_format  →  "jpeg" ⇒ Jpeg | else Png
cli.hybrid        →  "docling-fast" ⇒ DoclingFast | else Off
cli.hybrid_mode   →  "full" ⇒ Full | else Auto
```

### Output file naming

`write_outputs()` writes one file per format:

```
<output_dir>/<stem>.<ext>
```

Where `<stem>` is the input filename without extension and `<ext>` is:

| Format | Extension |
|--------|-----------|
| `json` | `.json` |
| `markdown*` | `.md` |
| `html` | `.html` |
| `text` | `.txt` |

---

## 2. Python SDK

**Package name:** `edgeparse`  
**Requires:** Python 3.9+  
**Source (pure Python):** [`sdks/python/edgeparse/`](../sdks/python/edgeparse/)  
**Source (Rust extension):** [`crates/edgeparse-python/src/lib.rs`](../crates/edgeparse-python/src/lib.rs)  
**Build tool:** maturin ≥ 1.7  
**Module layout:**

```
sdks/python/
├── pyproject.toml              # maturin build config, project metadata
└── edgeparse/
    ├── __init__.py             # Public API: convert(), convert_file()
    ├── _types.py               # String literal constants (FORMATS, etc.)
    ├── cli.py                  # argparse CLI entry point
    └── _edgeparse.so           # Native extension (built by maturin)
```

### Installation

```bash
pip install edgeparse

# Or build from source:
cd sdks/python
pip install maturin
maturin develop --release
```

### `edgeparse.convert()`

Defined in [`sdks/python/edgeparse/__init__.py`](../sdks/python/edgeparse/__init__.py).  
Delegates to `_edgeparse.convert()` (PyO3 native, defined in `crates/edgeparse-python/src/lib.rs`).

```python
def convert(
    input_path: str | Path,
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
    reading_order: str = "xycut",
    table_method: str = "default",
    image_output: str = "off",
) -> str
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `input_path` | `str \| Path` | — | Path to the PDF file |
| `format` | `str` | `"markdown"` | Output format: `"markdown"`, `"json"`, `"html"`, `"text"` |
| `pages` | `str \| None` | `None` | Page range e.g. `"1,3,5-7"` |
| `password` | `str \| None` | `None` | Password for encrypted PDFs |
| `reading_order` | `str` | `"xycut"` | `"xycut"` or `"off"` |
| `table_method` | `str` | `"default"` | `"default"` or `"cluster"` |
| `image_output` | `str` | `"off"` | `"off"`, `"embedded"`, or `"external"` |

**Returns:** The extracted content as a `str`.

**Raises:** `RuntimeError` (mapped from `EdgePdfError` via PyO3) on parse failure.

**Example:**

```python
import edgeparse

# Markdown extraction
md = edgeparse.convert("report.pdf")

# JSON with bounding boxes
json_str = edgeparse.convert("report.pdf", format="json")

# Specific pages, cluster table detection
md = edgeparse.convert(
    "report.pdf",
    format="markdown",
    pages="1-10",
    table_method="cluster",
)

# Password-protected PDF
md = edgeparse.convert("secure.pdf", password="hunter2")
```

### `edgeparse.convert_file()`

Defined in [`sdks/python/edgeparse/__init__.py`](../sdks/python/edgeparse/__init__.py).  
Delegates to `_edgeparse.convert_file()` (PyO3 native).

```python
def convert_file(
    input_path: str | Path,
    output_dir: str | Path = "output",
    *,
    format: str = "markdown",
    pages: str | None = None,
    password: str | None = None,
) -> str
```

**Parameters:** Same as `convert()` plus `output_dir`. Note that `reading_order`,
`table_method`, and `image_output` are not exposed here (use `convert()` for
full control).

**Returns:** Path to the created output file as a `str`.

**Example:**

```python
out_path = edgeparse.convert_file(
    "report.pdf",
    output_dir="extracted/",
    format="markdown",
)
print(f"Wrote {out_path}")
```

### PyO3 extension internals

**Source:** [`crates/edgeparse-python/src/lib.rs`](../crates/edgeparse-python/src/lib.rs)

The PyO3 layer is thin — it maps Python string arguments to `ProcessingConfig`
enum fields and calls `edgeparse_core::convert()`:

```
#[pyfunction]
fn convert(input_path, format, pages, password,
           reading_order, table_method, image_output)
  → PyResult<String>
```

Enum mapping:

```
format "markdown" → OutputFormat::Markdown
format "json"     → OutputFormat::Json
format "html"     → OutputFormat::Html
format "text"     → OutputFormat::Text

reading_order "xycut" → ReadingOrder::XyCut
reading_order "off"   → ReadingOrder::Off

table_method "cluster" → TableMethod::Cluster
table_method "default" → TableMethod::Default

image_output "embedded" → ImageOutput::Embedded
image_output "external" → ImageOutput::External
image_output "off"      → ImageOutput::Off
```

### Python CLI

**Source:** [`sdks/python/edgeparse/cli.py`](../sdks/python/edgeparse/cli.py)  
**Entry point:** `edgeparse` console script (registered in `pyproject.toml`)

```bash
edgeparse <input.pdf> [<input.pdf> ...] [-o OUTPUT_DIR] [-f FORMAT]
          [--pages RANGE] [-p PASSWORD]
```

| Flag | Default | Description |
|------|---------|-------------|
| `input` (positional) | — | One or more PDF files |
| `-o, --output-dir` | `output` | Output directory |
| `-f, --format` | `markdown` | Output format |
| `--pages` | — | Page range |
| `-p, --password` | — | PDF password |

Note: The Python CLI calls `convert_file()` internally (not `convert()`), so
`reading_order`, `table_method`, and `image_output` are not exposed as CLI
flags in the Python wrapper — use the Rust CLI for full flag coverage.

### Valid value constants

[`sdks/python/edgeparse/_types.py`](../sdks/python/edgeparse/_types.py) exports
string literal tuples:

```python
FORMATS        = ("markdown", "json", "html", "text")
READING_ORDERS = ("xycut", "off")
TABLE_METHODS  = ("default", "cluster")
IMAGE_OUTPUTS  = ("off", "embedded", "external")
```

---

## 3. Node.js SDK

**Package name:** `edgeparse`  
**Requires:** Node.js ≥ 18  
**Source (TypeScript wrapper):** [`sdks/node/src/`](../sdks/node/src/)  
**Source (Rust addon):** [`crates/edgeparse-node/src/lib.rs`](../crates/edgeparse-node/src/lib.rs)  
**Build tool:** NAPI-RS + tsup  

### Package structure

```
sdks/node/
├── package.json               # edgeparse, optionalDependencies per platform
├── tsconfig.json
├── src/
│   ├── index.ts               # convert(), version() — public API
│   ├── types.ts               # ConvertOptions interface
│   └── cli.ts                 # node:util parseArgs CLI
├── npm/
│   ├── darwin-arm64/          # platform package stubs
│   ├── darwin-x64/
│   ├── linux-arm64-gnu/
│   ├── linux-x64-gnu/
│   └── win32-x64-msvc/
└── tests/
    └── convert.test.ts        # vitest tests
```

Platform packages (`edgeparse-{platform}`) are loaded at runtime by
`loadNative()` in `index.ts` using `process.platform`/`process.arch` as the
lookup key.

### Installation

```bash
npm install edgeparse
# or
yarn add edgeparse
# or
pnpm add edgeparse
```

The correct platform native addon is automatically selected via
`optionalDependencies` in `package.json`.

### `convert()`

Defined in [`sdks/node/src/index.ts`](../sdks/node/src/index.ts):

```ts
import { convert } from 'edgeparse';

function convert(inputPath: string, options?: ConvertOptions): string
```

**Returns:** The extracted content as a `string`.

**Throws:** `Error` on unsupported platform or parse failure.

### `ConvertOptions`

Defined in [`sdks/node/src/types.ts`](../sdks/node/src/types.ts):

```ts
interface ConvertOptions {
  /** Output format: "markdown" | "json" | "html" | "text". Default: "markdown". */
  format?: string;
  /** Page range string, e.g. "1,3,5-7". */
  pages?: string;
  /** Password for encrypted PDFs. */
  password?: string;
  /** Reading order algorithm: "xycut" (default) or "off". */
  readingOrder?: string;
  /** Table detection method: "default" or "cluster". */
  tableMethod?: string;
  /** Image output mode: "off" (default), "embedded", or "external". */
  imageOutput?: string;
}
```

Note the `camelCase` naming convention (`readingOrder`, `tableMethod`,
`imageOutput`) compared to the Python/Rust `snake_case`. `index.ts` maps these
by writing an intermediate object with `snake_case` keys before passing to the
native addon:

```ts
n.convert(inputPath, options ? {
  format:        options.format,
  pages:         options.pages,
  password:      options.password,
  reading_order: options.readingOrder,
  table_method:  options.tableMethod,
  image_output:  options.imageOutput,
} : undefined);
```

### `version()`

```ts
import { version } from 'edgeparse';

function version(): string
```

Returns the edgeparse version string from the native addon.

### Example usage

```ts
import { convert } from 'edgeparse';

// Markdown (default format)
const md = convert('report.pdf');

// JSON with bounding boxes
const json = convert('report.pdf', { format: 'json' });

// Cluster table detection, specific pages
const result = convert('report.pdf', {
  format: 'markdown',
  pages: '1-5',
  tableMethod: 'cluster',
  readingOrder: 'xycut',
});

// Password-protected
const secure = convert('secure.pdf', { password: 'hunter2' });
```

### Node.js CLI

**Source:** [`sdks/node/src/cli.ts`](../sdks/node/src/cli.ts)  
**Entry point:** `edgeparse` binary (registered in `package.json` → `bin.edgeparse`)

```bash
npx edgeparse [options] <input.pdf>
# or after install:
edgeparse [options] <input.pdf>
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--format` | `-f` | `markdown` | Output format |
| `--pages` | `-p` | — | Page range |
| `--password` | — | — | PDF password |
| `--reading-order` | — | `xycut` | Reading order algorithm |
| `--table-method` | — | `default` | Table detection method |
| `--image-output` | — | `off` | Image output mode |
| `--output` | `-o` | stdout | Output file path |
| `--version` | `-v` | — | Print version and exit |
| `--help` | `-h` | — | Print help and exit |

Uses `node:util.parseArgs`. Output goes to stdout by default; use `-o <path>`
to write to a file.

### NAPI-RS extension internals

**Source:** [`crates/edgeparse-node/src/lib.rs`](../crates/edgeparse-node/src/lib.rs)

```rust
#[napi]
pub struct ConvertOptions {
  pub format: Option<String>,
  pub pages: Option<String>,
  pub password: Option<String>,
  pub reading_order: Option<String>,
  pub table_method: Option<String>,
  pub image_output: Option<String>,
}

#[napi]
pub fn convert(input_path: String, options: Option<ConvertOptions>)
  -> napi::Result<String>
```

All fields are `Option<String>` — missing fields fall back to
`ProcessingConfig` defaults.

---

## 4. Batch API (`edgeparse-core`)

**Source:** [`crates/edgeparse-core/src/api/batch.rs`](../crates/edgeparse-core/src/api/batch.rs)

The batch API is a Rust-only library API (not yet exposed through the Python or
Node.js SDKs). It provides progress-tracking bulk processing over a list of
PDF files.

### Key types

#### `BatchRequest`

```rust
pub struct BatchRequest {
    pub files: Vec<PathBuf>,
    pub config: ProcessingConfig,
    pub output_dir: Option<PathBuf>,
}
```

Builder pattern:

```rust
let req = BatchRequest::new(files, config)
    .with_output_dir(PathBuf::from("output/"));
```

#### `BatchFileResult`

```rust
pub struct BatchFileResult {
    pub input_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
    pub duration: Duration,
    pub page_count: Option<u32>,
}
```

#### `BatchResult`

```rust
pub struct BatchResult {
    pub files: Vec<BatchFileResult>,
    pub total_duration: Duration,
}
```

Helper methods:

| Method | Returns | Description |
|--------|---------|-------------|
| `success_count()` | `usize` | Number of successfully processed files |
| `failure_count()` | `usize` | Number of failed files |
| `total_count()` | `usize` | Total file count |
| `avg_duration()` | `Duration` | Average per-file processing time |
| `summary()` | `String` | Human-readable result summary |

### `process_batch()`

```rust
pub fn process_batch<F>(
    request: &BatchRequest,
    process_fn: F,
) -> BatchResult
where
    F: FnMut(&Path, &ProcessingConfig) -> Result<u32, String>
```

Calls `process_fn` for each file in `request.files` sequentially,
collecting per-file results and total elapsed time.

**Example (Rust):**

```rust
use edgeparse_core::{convert, api::{batch::*, config::ProcessingConfig}};
use std::path::PathBuf;

let files = collect_pdf_files(Path::new("docs/"))?;
let config = ProcessingConfig::default();
let req = BatchRequest::new(files, config);

let result = process_batch(&req, |path, cfg| {
    let doc = convert(path, cfg).map_err(|e| e.to_string())?;
    Ok(doc.number_of_pages)
});

println!("{}", result.summary());
for f in &result.files {
    if !f.success {
        eprintln!("{}: {}", f.input_path.display(), f.error.as_deref().unwrap_or("unknown"));
    }
}
```

### File collection helpers

```rust
// Non-recursive — PDF files in a single directory
pub fn collect_pdf_files(dir: &Path) -> Result<Vec<PathBuf>, EdgePdfError>

// Recursive — PDF files in all subdirectories
pub fn collect_pdf_files_recursive(dir: &Path) -> Result<Vec<PathBuf>, EdgePdfError>
```

Both return paths sorted alphabetically.

---

## 5. Parameter compatibility matrix

| Parameter | Rust CLI flag | Python kwarg | Node.js `ConvertOptions` |
|-----------|--------------|--------------|--------------------------|
| Output format | `--format` | `format=` | `format` |
| Page range | `--pages` | `pages=` | `pages` |
| Password | `--password` | `password=` | `password` |
| Reading order | `--reading-order` | `reading_order=` | `readingOrder` |
| Table method | `--table-method` | `table_method=` | `tableMethod` |
| Image output | `--image-output` | `image_output=` | `imageOutput` |
| Output dir | `--output-dir` | *(convert_file only)* | *(not exposed)* |
| Sanitize | `--sanitize` | *(not exposed)* | *(not exposed)* |
| Struct tree | `--use-struct-tree` | *(not exposed)* | *(not exposed)* |
| Hybrid backend | `--hybrid` | *(not exposed)* | *(not exposed)* |
| Content safety | `--content-safety-off` | *(not exposed)* | *(not exposed)* |

The Python and Node.js SDKs expose the six most-used parameters. Advanced
options (hybrid, safety filters, PII sanitisation, tagged PDF) are only
available via the Rust CLI or by calling `edgeparse_core` directly.

---

## 6. Default values comparison

| Parameter | Rust CLI default | Python SDK default | Node.js SDK default |
|-----------|-----------------|-------------------|---------------------|
| `format` | `json` | `markdown` | `markdown` |
| `reading_order` | `xycut` | `xycut` | `xycut` |
| `table_method` | `default` | `default` | `default` |
| `image_output` | `external` | `off` | `off` |

> The Rust CLI defaults `format` to `json` (per `build_config` in `main.rs`
> when `--format` is omitted) and `image_output` to `external`. Both SDKs
> default to `markdown` and `off` respectively — optimised for in-memory
> programmatic use where callers usually want the full text and no image files.
