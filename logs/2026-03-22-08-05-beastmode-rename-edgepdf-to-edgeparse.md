# Task Log: Rename edgepdf → edgeparse

## Actions
- Scanned 104+ files containing "edgepdf" references across codebase
- Bulk-replaced all content: `edgepdf` → `edgeparse`, `EdgePDF` → `EdgeParse`, `EDGEPDF` → `EDGEPARSE`
- Fixed Makefile and Dockerfiles missed by initial extension-based grep filter
- Renamed 4 crate directories: `edgeparse-core`, `edgeparse-cli`, `edgeparse-python`, `edgeparse-node`
- Renamed Python SDK package directory: `sdks/python/edgeparse/`
- Renamed benchmark files: `pdf_parser_edgeparse.py`, `compare_edgeparse_ground_truth.py`, `prediction/edgeparse/`
- 154 files modified/renamed total

## Decisions
- Used `edgeparse` (not `egdeparse` from user typo) as the new name
- Kept benchmark HTML/JSON report files updated for consistency
- Did not rename the top-level repo directory (that's a git remote concern)

## Verification
- `cargo check` — all 3 default crates pass (core, cli, pdf-cos)
- `cargo check -p edgeparse-node` — passes
- `cargo check -p edgeparse-python` — passes
- `cargo test -p edgeparse-core --lib` — 407 passed, 0 failed
- CLI binary: `./target/release/edgeparse --version` → `edgeparse 0.1.0`
- CLI functional: `./target/release/edgeparse examples/pdf/lorem.pdf -f text` works correctly
- Zero remaining references to `edgepdf` or `EdgePDF` in source files

## Lessons
- `grep --include=` misses files without extensions (Makefile, Dockerfile); always check these separately
