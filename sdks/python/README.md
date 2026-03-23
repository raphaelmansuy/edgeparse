# edgeparse

High-performance PDF-to-structured-data extraction for Python — powered by a Rust engine via PyO3.

## Install

```bash
pip install edgeparse
```

Pre-built wheels are available for **macOS**, **Linux** (x86_64, arm64), and **Windows** (x64).
No system dependencies or compilation required.

## Quick start

```python
import edgeparse

# Convert a PDF to Markdown
result = edgeparse.convert("document.pdf")
print(result.markdown)

# Convert with options
result = edgeparse.convert(
    "document.pdf",
    format="markdown",      # "markdown" | "json" | "html"
    extract_images=False,
    page_range=None,        # None = all pages, or [0, 5] for pages 1–6
)
```

## CLI

```bash
edgeparse document.pdf                     # → Markdown on stdout
edgeparse document.pdf --format json       # → JSON
edgeparse /path/to/dir/ --output-dir out/  # batch convert
```

## Performance

`edgeparse` consistently leads open benchmarks for PDF-to-Markdown extraction quality across 200-document test suites.

## Links

- [GitHub](https://github.com/raphaelmansuy/edgeparse)
- [crates.io (Rust)](https://crates.io/crates/edgeparse-core)
- [npm (@edgeparse/pdf)](https://www.npmjs.com/package/@edgeparse/pdf)
