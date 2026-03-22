# 06 — CLI Interface

> **Cross-references**: [02-functional-spec](02-functional-spec.md) | [03-technical-architecture](03-technical-architecture.md) | [05-data-models](05-data-models.md)

---

## 1. Invocation

```
opendataloader-pdf [OPTIONS] <FILE|DIRECTORY>...

Arguments:
  <FILE|DIRECTORY>...    One or more PDF files or directories to process.
                         Directories are scanned recursively for *.pdf files.
```

When invoked via the Java JAR:
```
java -jar opendataloader-pdf.jar [OPTIONS] <FILE|DIRECTORY>...
```

---

## 2. Options Reference

### 2.1 Full Options Table

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `--output-dir` | `-o` | string | input dir | Output directory for generated files |
| `--password` | `-p` | string | none | Password for encrypted PDFs |
| `--format` | `-f` | string | `json` | Comma-separated output formats |
| `--quiet` | `-q` | flag | false | Suppress console logging |
| `--content-safety-off` | — | string | none | Disable safety filters (comma-separated) |
| `--sanitize` | — | flag | false | Enable PII sanitization |
| `--keep-line-breaks` | — | flag | false | Preserve original line breaks |
| `--replace-invalid-chars` | — | string | `" "` | Replacement for unrecognized characters |
| `--use-struct-tree` | — | flag | false | Use PDF structure tree |
| `--table-method` | — | string | `default` | Table detection algorithm |
| `--reading-order` | — | string | `xycut` | Reading order algorithm |
| `--markdown-page-separator` | — | string | none | Page separator in Markdown |
| `--text-page-separator` | — | string | none | Page separator in text |
| `--html-page-separator` | — | string | none | Page separator in HTML |
| `--image-output` | — | string | `external` | Image output mode |
| `--image-format` | — | string | `png` | Image format |
| `--image-dir` | — | string | none | Image output directory |
| `--pages` | — | string | all | Page selection (e.g., `1,3,5-7`) |
| `--include-header-footer` | — | flag | false | Include headers/footers |
| `--hybrid` | — | string | `off` | Hybrid backend selection |
| `--hybrid-mode` | — | string | `auto` | Hybrid triage mode |
| `--hybrid-url` | — | string | backend default | Backend URL override |
| `--hybrid-timeout` | — | string | `30000` | Backend timeout (ms) |
| `--hybrid-fallback` | — | flag | false | Enable Java fallback on error |
| `--export-options` | — | flag | — | Print options metadata as JSON |
| `--version` | — | flag | — | Print version and exit |
| `--help` | `-h` | flag | — | Print help and exit |

### 2.2 Option Details

#### `--format` Values
| Value | Generated File(s) | Extension |
|-------|-------------------|-----------|
| `json` | JSON output | `.json` |
| `text` | Plain text | `.txt` |
| `html` | Semantic HTML | `.html` |
| `markdown` | CommonMark | `.md` |
| `markdown-with-html` | Markdown + HTML tags | `.md` |
| `markdown-with-images` | Markdown + embedded images | `.md` |
| `pdf` | Annotated PDF | `.annotated.pdf` |

Multiple values: `--format json,markdown,html`

#### `--content-safety-off` Values
| Value | Effect |
|-------|--------|
| `all` | Disable ALL safety filters |
| `hidden-text` | Show text with low contrast ratio |
| `off-page` | Include content outside CropBox |
| `tiny` | Include text < 1pt height |
| `hidden-ocg` | Include hidden Optional Content Groups |

Multiple values: `--content-safety-off hidden-text,off-page`

#### `--table-method` Values
| Value | Description |
|-------|-------------|
| `default` | Border-based detection only (line intersections) |
| `cluster` | Border-based + statistical column clustering |

#### `--reading-order` Values
| Value | Description |
|-------|-------------|
| `off` | Content in PDF stream order |
| `xycut` | XY-Cut++ algorithm (see [04-pipeline, Stage 18](04-pdf-parsing-pipeline.md)) |

#### `--image-output` Values
| Value | Description |
|-------|-------------|
| `off` | No image extraction |
| `embedded` | Base64 data URIs in output |
| `external` | Separate image files with references |

#### `--hybrid` Values
| Value | Description |
|-------|-------------|
| `off` | Java-only processing |
| `docling-fast` | Docling-based backend server |
| `hancom` | Hancom Cloud API |
| `azure` | Azure Document Intelligence |
| `google` | Google Document AI |

#### `--hybrid-mode` Values
| Value | Description |
|-------|-------------|
| `auto` | Triage each page (Java vs backend decision) |
| `full` | Send all pages to backend (required for enrichments) |

#### `--pages` Syntax
```
"1"         → page 1 only
"1,3,5"     → pages 1, 3, and 5
"1-5"       → pages 1 through 5
"1-5,8,10-12" → pages 1-5, 8, and 10-12
```
Pages are 1-indexed. Out-of-range pages are silently ignored.

#### Page Separator Placeholder
All `--*-page-separator` options support `%page-number%`:
```
--markdown-page-separator "---\n<!-- Page %page-number% -->\n---"
```

---

## 3. Exit Codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | SUCCESS | All files processed successfully |
| 1 | PARTIAL_FAILURE | Some files failed, at least one succeeded |
| 2 | TOTAL_FAILURE | All files failed to process |

---

## 4. Error Output

Errors are written to stderr in the format:
```
ERROR: <message>
```

When `--quiet` is NOT set, progress is logged to stderr:
```
Processing: document.pdf
  Page 1/10...
  Page 2/10...
  ...
  Output: /path/to/document.json
```

---

## 5. Batch Processing

When a directory is provided as input:
```
opendataloader-pdf --format json /path/to/pdfs/
```

**Behavior**:
1. Recursively scan directory for `*.pdf` files (case-insensitive)
2. Process each file independently
3. Output files maintain relative directory structure under `--output-dir`
4. Exit code reflects aggregate result (see §3)

**File naming**:
```
Input:  /input/sub/document.pdf
Output: /output/sub/document.json
        /output/sub/document.md
        /output/sub/images/document_img_1.png
```

---

## 6. `--export-options` Output

Prints a JSON array describing all CLI options to stdout, then exits.

```json
[
  {
    "name": "output-dir",
    "shortName": "o",
    "type": "string",
    "required": false,
    "default": null,
    "description": "Directory where output files are written. Default: input file directory"
  },
  ...
]
```

This is used by the code generation pipeline to produce wrapper bindings (Node.js, Python).

---

## 7. Code Generation Pipeline

The `options.json` file at repo root is the **single source of truth** for all CLI options. A code generation script (`scripts/generate-options.mjs`) reads it and produces:

```
options.json
    |
    v
scripts/generate-options.mjs
    |
    +---> node/opendataloader-pdf/src/_generated/options.ts    (TypeScript types)
    +---> python/opendataloader-pdf/src/opendataloader_pdf/_generated/options.py  (kwargs)
    +---> content/docs/_generated/cli-options-reference.mdx    (docs)
```

**Rust rewrite**: This pipeline must be preserved. The Rust CLI should:
1. Parse options using `clap` (derive API)
2. Maintain `--export-options` to output the same JSON format
3. Keep `options.json` as the human-readable reference
4. Continue generating wrapper bindings from `options.json`

---

## 8. Validation Rules

### 8.1 Input Validation

| Rule | Behavior |
|------|----------|
| No input files/dirs | Print help + exit code 2 |
| File not found | Error to stderr, skip file |
| Not a PDF | Error to stderr, skip file |
| Encrypted without password | Error to stderr, skip file |
| Invalid `--pages` syntax | Error to stderr, exit code 2 |
| Invalid `--format` value | Error to stderr, exit code 2 |
| Invalid `--table-method` | Error to stderr, exit code 2 |

### 8.2 Output Directory Rules

| Scenario | Behavior |
|----------|----------|
| `--output-dir` specified | Create if not exists, write there |
| `--output-dir` not specified | Write to same directory as input file |
| `--image-dir` specified | Create if not exists, store images there |
| `--image-dir` not specified | Store images in `<output-dir>/images/` |

### 8.3 Hybrid Validation

| Rule | Behavior |
|------|----------|
| `--hybrid` without `--hybrid-url` | Use backend-specific default URL |
| `--hybrid-url` without `--hybrid` | Ignored (hybrid stays off) |
| `--hybrid-mode full` without `--hybrid` | Ignored |
| `--hybrid-timeout` not numeric | Error to stderr, exit code 2 |
| Backend unreachable + `--hybrid-fallback` | Fall back to Java-only |
| Backend unreachable, no fallback | Error to stderr, skip file |

---

## 9. Rust CLI Implementation Notes

### 9.1 Recommended Crate: clap

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "opendataloader-pdf")]
#[command(about = "Extract text, tables, and images from PDF documents")]
pub struct Cli {
    /// PDF files or directories to process
    #[arg(required = true)]
    pub input: Vec<PathBuf>,

    /// Output directory
    #[arg(short = 'o', long)]
    pub output_dir: Option<PathBuf>,

    /// Password for encrypted PDFs
    #[arg(short = 'p', long)]
    pub password: Option<String>,

    /// Output formats (comma-separated)
    #[arg(short = 'f', long, value_delimiter = ',')]
    pub format: Option<Vec<String>>,

    /// Suppress console output
    #[arg(short = 'q', long)]
    pub quiet: bool,

    // ... remaining options per table above
}
```

### 9.2 Conversion to Config

```rust
impl Cli {
    pub fn to_config(&self) -> Result<Config, CliError> {
        let mut config = Config::default();

        if let Some(ref dir) = self.output_dir {
            config.output_folder = Some(dir.to_string_lossy().into());
        }

        if let Some(ref formats) = self.format {
            for f in formats {
                match f.as_str() {
                    "json" => config.generate_json = true,
                    "markdown" => config.generate_markdown = true,
                    "html" => config.generate_html = true,
                    "text" => config.generate_text = true,
                    "pdf" => config.generate_pdf = true,
                    "markdown-with-html" => {
                        config.generate_markdown = true;
                        config.use_html_in_markdown = true;
                    }
                    "markdown-with-images" => {
                        config.generate_markdown = true;
                        config.add_image_to_markdown = true;
                    }
                    _ => return Err(CliError::InvalidFormat(f.clone())),
                }
            }
        }

        // ... remaining option mapping
        Ok(config)
    }
}
```
