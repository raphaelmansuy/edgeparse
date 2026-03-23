# ==============================================================================
#  EdgeParse — Makefile
#
#  Quick start:
#    make build                          # release binary
#    make run PDF=examples/pdf/lorem.pdf # convert a PDF to Markdown
#    make ci                             # full CI gate (fmt + lint + test)
#    make bench                          # benchmark EdgeParse alone (200 PDFs)
#    make bench-non-ocr                  # compare vs non-OCR tools  →  HTML
#    make bench-ocr                      # compare vs OCR/ML tools   →  HTML
#    make bench-ocr OCR_ENGINES=docling  # partial OCR comparison
# ==============================================================================

.DEFAULT_GOAL := help
.PHONY: help \
        build build-debug check fmt fmt-check lint test ci \
        install uninstall \
        bench bench-setup bench-check bench-doc \
        bench-engines bench-non-ocr bench-ocr bench-compare-all bench-report \
        run demo \
        publish-rust publish-rust-dry publish-python publish-python-dry \
        publish-node publish-node-dry publish-all \
        clean clean-bench clean-all

# ── Colours ────────────────────────────────────────────────────────────────────
BOLD   := \033[1m
GREEN  := \033[0;32m
CYAN   := \033[0;36m
YELLOW := \033[0;33m
RED    := \033[0;31m
DIM    := \033[2m
RESET  := \033[0m

# ── Paths ──────────────────────────────────────────────────────────────────────
BINARY    := target/release/edgeparse
BENCH_DIR := benchmark
EXAMPLES  := examples/pdf

# ── Benchmark engine configuration ────────────────────────────────────────────
# Override OCR_ENGINES to run a subset:  make bench-ocr OCR_ENGINES=docling
# Available OCR engines: docling, marker, mineru
OCR_ENGINES ?= docling,marker,mineru

# ── Helper macros ──────────────────────────────────────────────────────────────
define log
  @printf "$(BOLD)$(GREEN) ▶$(RESET) $(GREEN)$(1)$(RESET)\n"
endef
define ok
  @printf "$(BOLD)$(GREEN) ✓$(RESET) $(1)\n"
endef
define warn
  @printf "$(BOLD)$(YELLOW) ⚠$(RESET) $(YELLOW)$(1)$(RESET)\n"
endef
define err
  @printf "$(BOLD)$(RED) ✖$(RESET) $(RED)$(1)$(RESET)\n"
endef

# ══════════════════════════════════════════════════════════════════════════════
#  HELP
# ══════════════════════════════════════════════════════════════════════════════
help: ## Show this help screen
	@printf "\n$(BOLD)EdgeParse$(RESET) — High-performance PDF-to-Markdown engine (Rust)\n"
	@printf "$(DIM)https://github.com/raphaelmansuy/edgeparse$(RESET)\n\n"
	@printf "$(BOLD)Usage$(RESET)\n"
	@printf "  make $(CYAN)<target>$(RESET)\n"
	@printf "  make run           $(CYAN)PDF=examples/pdf/lorem.pdf$(RESET)\n"
	@printf "  make bench-doc     $(CYAN)DOC=01030000000042$(RESET)\n"
	@printf "  make bench-ocr     $(CYAN)OCR_ENGINES=docling$(RESET)          # partial OCR run\n\n"
	@printf "$(BOLD)Targets$(RESET)\n"
	@awk 'BEGIN {FS = ":.*##"; section=""} \
	     /^## / { printf "\n  $(BOLD)%s$(RESET)\n", substr($$0, 4); next } \
	     /^[a-zA-Z_-]+:.*##/ { printf "    $(CYAN)%-26s$(RESET) %s\n", $$1, $$2 }' \
	     $(MAKEFILE_LIST)
	@printf "\n"

# ══════════════════════════════════════════════════════════════════════════════
## Build
# ══════════════════════════════════════════════════════════════════════════════

build: ## Build optimised release binary  →  target/release/edgeparse
	$(call log,cargo build --release)
	@cargo build --release
	$(call ok,Binary ready: $(BINARY))

build-debug: ## Build debug binary  →  target/debug/edgeparse
	$(call log,cargo build)
	@cargo build

check: ## Fast compile-check (no binary produced)
	$(call log,cargo check)
	@cargo check

# ══════════════════════════════════════════════════════════════════════════════
## Code quality
# ══════════════════════════════════════════════════════════════════════════════

fmt: ## Auto-format publishable crates (edgeparse-core, edgeparse-cli)
	$(call log,cargo fmt -p edgeparse-core -p edgeparse-cli)
	@cargo fmt -p edgeparse-core -p edgeparse-cli

fmt-check: ## Verify formatting without changes — publishable crates (CI gate)
	$(call log,cargo fmt -p edgeparse-core -p edgeparse-cli -- --check)
	@cargo fmt -p edgeparse-core -p edgeparse-cli -- --check

lint: ## Run Clippy on publishable crates — warnings promoted to errors
	$(call log,cargo clippy -p edgeparse-core -p edgeparse-cli -- -D warnings)
	@cargo clippy -p edgeparse-core -p edgeparse-cli -- -D warnings

test: ## Run all unit and integration tests
	$(call log,cargo test)
	@cargo test

ci: fmt-check lint test ## Full CI gate: fmt-check → lint → test
	$(call ok,All CI checks passed)

# ══════════════════════════════════════════════════════════════════════════════
## Install
# ══════════════════════════════════════════════════════════════════════════════

install: build ## Install edgeparse to ~/.cargo/bin
	$(call log,cargo install --path crates/edgeparse-cli)
	@cargo install --path crates/edgeparse-cli
	$(call ok,Installed: $$(which edgeparse))

uninstall: ## Remove edgeparse from ~/.cargo/bin
	$(call warn,Removing edgeparse from ~/.cargo/bin ...)
	@cargo uninstall edgeparse-cli || true

# ══════════════════════════════════════════════════════════════════════════════
## Convert
# ══════════════════════════════════════════════════════════════════════════════

run: build ## Convert one PDF to Markdown  →  make run PDF=examples/pdf/lorem.pdf
ifndef PDF
	$(call err,PDF is required.  Usage:  make run PDF=examples/pdf/lorem.pdf)
	@exit 1
endif
	$(call log,Converting: $(PDF))
	@./$(BINARY) "$(PDF)"

demo: build ## Convert all bundled example PDFs  →  output in /tmp/edgeparse-demo/
	$(call log,Converting example PDFs  →  /tmp/edgeparse-demo/)
	@mkdir -p /tmp/edgeparse-demo
	@./$(BINARY) $(EXAMPLES)/*.pdf --output-dir /tmp/edgeparse-demo/ --format markdown
	$(call ok,Output written to /tmp/edgeparse-demo/:)
	@ls -1 /tmp/edgeparse-demo/

# ══════════════════════════════════════════════════════════════════════════════
## Benchmark — EdgeParse Accuracy
# ══════════════════════════════════════════════════════════════════════════════

bench-setup: ## Install Python benchmark dependencies  (requires: uv)
	@command -v uv >/dev/null 2>&1 || { \
	  $(call err,uv not found — install: curl -LsSf https://astral.sh/uv/install.sh | sh); \
	  exit 1; }
	$(call log,uv sync  [$(BENCH_DIR)/])
	@cd $(BENCH_DIR) && uv sync --quiet
	$(call ok,Benchmark environment ready)

bench: build bench-setup ## Run EdgeParse benchmark alone against ground truth  (200 PDFs → HTML)
	$(call log,Running EdgeParse benchmark  —  200 documents ...)
	@cd $(BENCH_DIR) && uv run python run.py

bench-check: build bench-setup ## Run benchmark + fail if scores drop below thresholds  (CI gate)
	$(call log,Running benchmark with regression check ...)
	@cd $(BENCH_DIR) && uv run python run.py --check-regression

bench-doc: build bench-setup ## Benchmark a single document  →  make bench-doc DOC=01030000000042
ifndef DOC
	$(call err,DOC is required.  Usage:  make bench-doc DOC=01030000000042)
	@exit 1
endif
	$(call log,Benchmarking document: $(DOC))
	@cd $(BENCH_DIR) && uv run python run.py --doc-id $(DOC)

## Benchmark — Non-OCR Comparison (fast, no ML models)

bench-non-ocr: build bench-setup ## EdgeParse vs non-OCR tools: OpenDataLoader, PyMuPDF4LLM, MarkItDown, LiteParse  →  HTML
	$(call log,Non-OCR comparison  —  EdgeParse + OpenDataLoader + PyMuPDF4LLM + MarkItDown + LiteParse)
	@cd $(BENCH_DIR) && uv run python compare_all.py \
		--group non-ocr --install \
		--title "EdgeParse vs Non-OCR Tools"

## Benchmark — OCR / ML Comparison (model-heavy)

bench-ocr: build bench-setup ## EdgeParse vs OCR/ML tools  →  make bench-ocr  or  make bench-ocr OCR_ENGINES=docling
	$(call log,OCR/ML comparison  —  EdgeParse + $(OCR_ENGINES))
	@cd $(BENCH_DIR) && uv run python compare_all.py \
		--engines edgeparse,$(OCR_ENGINES) --install \
		--title "EdgeParse vs OCR / ML Tools ($(OCR_ENGINES))"

## Benchmark — Combined & Utilities

bench-compare-all: build bench-setup ## Compare EdgeParse against ALL engines (non-OCR + OCR)  →  HTML
	$(call log,Full comparison  —  all engines ...)
	@cd $(BENCH_DIR) && uv run python compare_all.py \
		--group all --install \
		--title "EdgeParse Full Benchmark — All Engines"

bench-engines: bench-setup ## List all engines with their install status
	@cd $(BENCH_DIR) && uv run python compare_all.py --list

bench-report: bench-setup ## Regenerate HTML report from existing results  (no re-run)
	$(call log,Generating HTML report from existing results ...)
	@cd $(BENCH_DIR) && uv run python compare_all.py --group all --no-run
	$(call ok,Report saved to $(BENCH_DIR)/reports/benchmark-latest.html)

# ══════════════════════════════════════════════════════════════════════════════
## Publish
# ══════════════════════════════════════════════════════════════════════════════

# ── Rust / crates.io ──────────────────────────────────────────────────────────
publish-rust-dry: ## Dry-run: verify edgeparse-core + edgeparse-cli can be published
	$(call log,cargo publish --dry-run  [edgeparse-core])
	@cargo publish -p edgeparse-core --dry-run
	$(call log,cargo publish --dry-run  [edgeparse-cli])
	@cargo publish -p edgeparse-cli --dry-run
	$(call ok,Rust dry-run passed — ready for crates.io)

publish-rust: ## Publish edgeparse-core then edgeparse-cli to crates.io
	$(call log,Publishing edgeparse-core to crates.io ...)
	@cargo publish -p edgeparse-core
	$(call log,Waiting 30 s for crates.io index to propagate ...)
	@sleep 30
	$(call log,Publishing edgeparse-cli to crates.io ...)
	@cargo publish -p edgeparse-cli
	$(call ok,Rust crates published)

# ── Python / PyPI ─────────────────────────────────────────────────────────────
publish-python-dry: ## Dry-run: build Python wheel and validate with twine
	$(call log,maturin build  [sdks/python/])
	@cd sdks/python && maturin build --release --out dist/
	$(call log,twine check)
	@twine check sdks/python/dist/*.whl
	$(call ok,Python dry-run passed — wheel is valid)

publish-python: ## Build Python manylinux wheel and upload to PyPI
	$(call log,maturin publish  [sdks/python/])
	@cd sdks/python && maturin publish
	$(call ok,Python SDK published to PyPI)

# ── Node.js / npm ─────────────────────────────────────────────────────────────
publish-node-dry: ## Dry-run: build TypeScript and show what would be published
	$(call log,npm pack --dry-run  [sdks/node/])
	@cd sdks/node && npm install --ignore-scripts
	@cd sdks/node && npm run build:ts
	@cd sdks/node && npm pack --dry-run
	$(call ok,Node.js dry-run passed — package looks good)

publish-node: ## Build and publish @edgeparse/pdf to npm
	$(call log,Building Node.js SDK ...)
	@cd sdks/node && npm install --ignore-scripts
	@cd sdks/node && npm run build:ts
	$(call log,npm publish  [@edgeparse/pdf])
	@cd sdks/node && npm publish --access public
	$(call ok,Node.js SDK published to npm)

# ── Combined ──────────────────────────────────────────────────────────────────
publish-all: publish-rust publish-python publish-node ## Publish Rust + Python + Node.js SDKs in sequence
	$(call ok,All SDKs published)

# ══════════════════════════════════════════════════════════════════════════════
## Clean
# ══════════════════════════════════════════════════════════════════════════════

clean: ## Remove Rust build artefacts  (target/)
	$(call warn,Removing target/ ...)
	@cargo clean

clean-bench: ## Remove benchmark prediction outputs  (benchmark/prediction/)
	$(call warn,Removing $(BENCH_DIR)/prediction/ ...)
	@rm -rf $(BENCH_DIR)/prediction/

clean-all: clean clean-bench ## Remove all build artefacts and benchmark outputs
	$(call ok,All clean)
