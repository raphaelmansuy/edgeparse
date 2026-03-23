# EdgeParse Tutorials

Step-by-step guides for every interface EdgeParse provides.

---

## Guides

| # | Title | What you'll learn |
|---|-------|-------------------|
| [01](01-cli.md) | **CLI Quick-Start** | Install the binary, convert PDFs, use every flag |
| [02](02-python-sdk.md) | **Python SDK** | `pip install edgeparse`, convert to every format, batch files |
| [03](03-nodejs-sdk.md) | **Node.js SDK** | `npm install edgeparse`, TypeScript + CJS usage, CLI via npx |
| [04](04-rust-library.md) | **Rust Library** | Add `edgeparse-core` to your crate, use the full API |
| [05](05-output-formats.md) | **Output Formats Deep-Dive** | JSON schema, Markdown flavours, HTML structure, plain text |

---

## Prerequisites

| Interface | Requirement |
|-----------|-------------|
| CLI (pre-built) | macOS 12+, Linux glibc 2.31+, or Windows 10+ |
| CLI (from source) | [Rust 1.85+](https://rustup.rs/) |
| Python SDK | Python 3.9+ |
| Node.js SDK | Node.js 18+ |
| Rust library | Rust 1.85+ |

---

## Sample PDFs

All tutorials use files already in this repository:

```
examples/pdf/
├── lorem.pdf           # Simple single-page document — good for quick tests
├── 1901.03003.pdf      # Multi-column academic paper with tables
├── 2408.02509v1.pdf    # Dense research paper
└── chinese_scan.pdf    # CJK content

tests/fixtures/
└── sample.pdf          # Minimal fixture used in automated tests
```

Use any PDF you have — just replace the path in the examples.

---

## Cross-references

- For flag/option reference, see the [README CLI Reference](../README.md#cli-reference).
- For architecture details, see [docs/01-architecture.md](../docs/01-architecture.md).
- For output format schemas, see [docs/05-output-formats.md](../docs/05-output-formats.md).
- For CI/CD setup, see [docs/07-cicd-publishing.md](../docs/07-cicd-publishing.md).
