# Contributing to EdgeParse

Thank you for your interest in contributing to EdgeParse! This document explains how to get started.

## Code of Conduct

By participating in this project, you agree to be respectful and constructive. We follow the [Contributor Covenant](https://www.contributor-covenant.org/) Code of Conduct.

## How to Contribute

### Reporting Bugs

1. Search [existing issues](../../issues) to avoid duplicates.
2. Open a new issue using the **Bug Report** template.
3. Include the EdgeParse version, OS, Rust toolchain version, and a minimal PDF reproducer if possible.

### Suggesting Features

Open an issue with the **Feature Request** template. Describe the use case, expected behaviour, and any alternative approaches you considered.

### Submitting Pull Requests

1. Fork the repository and create a branch from `main`.
2. Run `cargo fmt` and `cargo clippy -- -D warnings` before committing.
3. Add or update tests for any changed behaviour.
4. Run the full benchmark to check for regressions (`./scripts/bench.sh`).
5. Open a PR against `main` with a clear description of the change.

## Development Setup

### Prerequisites

| Tool | Minimum version |
|------|----------------|
| Rust | 1.85 (stable) |
| Python | 3.11 |
| [uv](https://docs.astral.sh/uv/) | latest |

### Build

```bash
# Clone
git clone https://github.com/<your-org>/edgeparse.git
cd edgeparse

# Build release binary
cargo build --release

# Run unit tests
cargo test
```

### Running the Benchmark

```bash
cd benchmark

# Install Python deps
uv sync

# Run against all 200 PDFs
uv run python run.py

# Run for a single document
uv run python run.py --doc-id 01030000000001

# Check threshold regressions
uv run python run.py --check-regression
```

## Coding Guidelines

- All public APIs must have doc comments.
- Keep functions small and focused; prefer composition over inheritance.
- Avoid unsafe code except where absolutely necessary, with a safety comment.
- Use `thiserror` for library errors and `anyhow` in CLI/application code.

## Licence

By contributing, you agree that your contributions will be licensed under the Apache License 2.0.
