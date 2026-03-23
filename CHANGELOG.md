# Changelog

All notable changes to EdgeParse are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and
this project adheres to [Semantic Versioning](https://semver.org/).

---

## [0.1.1] — 2026-03-23

### Fixed
- Zero Clippy warnings across all crates
- Corrected `.gitignore` to exclude build artifacts cleanly

### Added
- Comprehensive Rust doc comments on public API surface
- Step-by-step tutorials for CLI, Python SDK, Node.js SDK, Rust library, and output formats
- CI/CD publishing guide (`docs/07-cicd-publishing.md`)

### Changed
- Bumped version to `0.1.1` in all crates and SDK manifests

---

## [0.1.0] — 2026-03-22

### Added
- **Core extraction engine** (`edgeparse-core`): Rust-native PDF-to-structured-data pipeline — no ML, no Java, no GPU
- **Python SDK** (`edgeparse`): PyO3-based bindings, available on PyPI
- **Node.js SDK** (`edgeparse`): NAPI-RS bindings, available on npm
- **CLI binary** (`edgeparse-cli`): Zero-dependency binary for all major platforms
- **Rust library** (`edgeparse-core`): First-class crate published to crates.io
- Reading-order reconstruction for multi-column and sidebar layouts
- Ruling-line and borderless table detection with cell-span merging
- Heading and paragraph classification
- AI safety filters (PII scrubbing, content flags)
- Tagged PDF support (PDF/UA accessibility structure)
- Output formats: JSON (full schema), Markdown, HTML, plain text
- Benchmark suite comparing EdgeParse against Docling, Marker, pymupdf4llm, MinerU, MarkItDown, and LiteParse
- Docker image for containerised deployment
- High-level technical documentation: overview, architecture, pipeline, data model, extraction, output formats, SDK integration
- GitHub Actions CI workflows for Rust, Python, Node.js, and Docker
- Renamed Node.js package from `@edgeparse/pdf` → `edgeparse`

---

## Links

- [GitHub Releases](https://github.com/raphaelmansuy/edgeparse/releases)
- [crates.io — edgeparse-core](https://crates.io/crates/edgeparse-core)
- [PyPI — edgeparse](https://pypi.org/project/edgeparse/)
- [npm — edgeparse](https://www.npmjs.com/package/edgeparse)
