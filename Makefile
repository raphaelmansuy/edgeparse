# ==============================================================================
#  EdgeParse — Makefile
#
#  Usage:  make <target>              (default: help)
#          make bench-doc DOC=01030000000042
#          make run PDF=examples/pdf/lorem.pdf
# ==============================================================================

.DEFAULT_GOAL := help
.PHONY: help \
        build build-debug check fmt fmt-check lint test ci \
        install uninstall \
        bench bench-setup bench-regression bench-doc \
        bench-odl-setup bench-odl bench-compare bench-compare-report \
        bench-compare-all bench-compare-fast bench-install-tools bench-engines \
        bench-download-models bench-download-marker bench-download-mineru \
        run demo \
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
	@printf "\n$(BOLD)EdgeParse$(RESET) — High-performance PDF-to-structured-data engine (Rust)\n"
	@printf "$(DIM)https://github.com/opendataloader-project/edgeparse$(RESET)\n\n"
	@printf "$(BOLD)Usage$(RESET)\n"
	@printf "  make $(CYAN)<target>$(RESET)\n"
	@printf "  make bench-doc $(CYAN)DOC=01030000000042$(RESET)\n"
	@printf "  make run       $(CYAN)PDF=examples/pdf/lorem.pdf$(RESET)\n\n"
	@printf "$(BOLD)Targets$(RESET)\n"
	@awk 'BEGIN {FS = ":.*##"; section=""} \
	     /^## / { printf "\n  $(BOLD)%s$(RESET)\n", substr($$0, 4); next } \
	     /^[a-zA-Z_-]+:.*##/ { printf "    $(CYAN)%-22s$(RESET) %s\n", $$1, $$2 }' \
	     $(MAKEFILE_LIST)
	@printf "\n"

# ══════════════════════════════════════════════════════════════════════════════
## Rust build
# ══════════════════════════════════════════════════════════════════════════════

build: ## Build optimised release binary  →  target/release/edgeparse
	$(call log,cargo build --release)
	@cargo build --release
	$(call ok,Binary ready: $(BINARY))

build-debug: ## Build debug binary  →  target/debug/edgeparse
	$(call log,cargo build)
	@cargo build

check: ## Compile-check all crates without producing binaries (fast)
	$(call log,cargo check)
	@cargo check

# ══════════════════════════════════════════════════════════════════════════════
## Code quality
# ══════════════════════════════════════════════════════════════════════════════

fmt: ## Auto-format all Rust source files
	$(call log,cargo fmt)
	@cargo fmt

fmt-check: ## Verify formatting without applying changes (CI gate)
	$(call log,cargo fmt --check)
	@cargo fmt --check

lint: ## Run Clippy — all warnings promoted to errors
	$(call log,cargo clippy -- -D warnings)
	@cargo clippy -- -D warnings

test: ## Run all Rust unit and integration tests
	$(call log,cargo test)
	@cargo test

ci: fmt-check lint test ## Full CI gate: fmt-check → lint → test
	$(call ok,All CI checks passed)

# ══════════════════════════════════════════════════════════════════════════════
## Install / uninstall
# ══════════════════════════════════════════════════════════════════════════════

install: build ## Install edgeparse to ~/.cargo/bin
	$(call log,cargo install --path crates/edgeparse-cli)
	@cargo install --path crates/edgeparse-cli
	$(call ok,Installed: $$(which edgeparse))

uninstall: ## Remove edgeparse from ~/.cargo/bin
	$(call warn,Removing edgeparse from ~/.cargo/bin ...)
	@cargo uninstall edgeparse-cli || true

# ══════════════════════════════════════════════════════════════════════════════
## Benchmark
# ══════════════════════════════════════════════════════════════════════════════

bench-setup: ## Install Python benchmark dependencies  (requires: uv)
	@command -v uv >/dev/null 2>&1 || { \
	  $(call err,uv not found. Install: curl -Ls https://astral.sh/uv/install.sh | sh); \
	  exit 1; }
	$(call log,uv sync  [$(BENCH_DIR)/])
	@cd $(BENCH_DIR) && uv sync --quiet
	$(call ok,Benchmark environment ready)

bench: build bench-setup ## Build + run full benchmark  (200 PDFs, all metrics)
	$(call log,Running full benchmark  —  200 documents ...)
	@cd $(BENCH_DIR) && uv run python run.py

bench-regression: build bench-setup ## Build + run benchmark + fail if below thresholds
	$(call log,Running benchmark with regression check ...)
	@cd $(BENCH_DIR) && uv run python run.py --check-regression

bench-doc: build bench-setup ## Benchmark a single document  →  make bench-doc DOC=01030000000042
ifndef DOC
	$(call err,DOC is required.  Usage:  make bench-doc DOC=01030000000042)
	@exit 1
endif
	$(call log,Benchmarking document: $(DOC))
	@cd $(BENCH_DIR) && uv run python run.py --doc-id $(DOC)

## Comparison (EdgeParse vs OpenDataLoader)

bench-odl-setup: bench-setup ## Install opendataloader-pdf for comparison  (requires: Java 11+)
	@command -v java >/dev/null 2>&1 || { \
	  $(call err,Java not found. Install JDK 11+ from https://adoptium.net/); \
	  exit 1; }
	$(call log,Installing opendataloader-pdf  [uv sync --extra opendataloader])
	@cd $(BENCH_DIR) && uv sync --quiet --extra opendataloader
	$(call ok,opendataloader-pdf ready  —  java version: $$(java -version 2>&1 | head -1))

bench-odl: build bench-odl-setup ## Run full benchmark using OpenDataLoader (Java/published)
	$(call log,Running opendataloader benchmark  —  200 documents ...)
	@cd $(BENCH_DIR) && uv run python run.py --engine opendataloader

bench-compare: build bench-odl-setup ## Build + run both engines, then show side-by-side comparison
	$(call log,Running edgeparse benchmark  —  200 documents ...)
	@cd $(BENCH_DIR) && uv run python run.py --engine edgeparse
	$(call log,Running opendataloader benchmark  —  200 documents ...)
	@cd $(BENCH_DIR) && uv run python run.py --engine opendataloader
	$(call log,Generating comparison report ...)
	@cd $(BENCH_DIR) && uv run python compare.py --no-run

bench-compare-report: bench-setup ## Show comparison from existing results  (no re-run)
	@cd $(BENCH_DIR) && uv run python compare.py --no-run

## Multi-engine comparison (EdgeParse vs third-party tools)

bench-engines: bench-setup ## List all known engines and their install status
	@cd $(BENCH_DIR) && uv run python compare_all.py --list

bench-install-tools: bench-setup ## Install comparison tools  →  make bench-install-tools TOOLS=pymupdf4llm,markitdown
	$(call log,Installing benchmark tools: $(or $(TOOLS),pymupdf4llm$(,)markitdown))
	@cd $(BENCH_DIR) && uv run python compare_all.py --engines $(or $(TOOLS),pymupdf4llm,markitdown) --install --no-run

bench-download-models: bench-download-marker bench-download-mineru ## Download ML models for Marker + MinerU (pipeline, CPU-friendly)
	@echo " ✓ All ML models ready"

bench-download-marker: bench-setup ## Create Marker isolated venv + download surya models (~500 MB)
	$(call log,Setting up isolated Marker venv ...)
	@uv venv $(BENCH_DIR)/.venvs/marker --quiet 2>/dev/null || true
	@uv pip install marker-pdf --python $(BENCH_DIR)/.venvs/marker/bin/python -q
	$(call log,Downloading Marker (surya) models from datalab.to ...)
	@PYTORCH_ENABLE_MPS_FALLBACK=1 $(BENCH_DIR)/.venvs/marker/bin/python -c \
		"print('Downloading Marker (surya) models — this may take a few minutes...'); \
		 from marker.models import create_model_dict; create_model_dict(); \
		 print('✓ Marker models ready (~/.cache/datalab/models/)')"

bench-download-mineru: bench-setup ## Create MinerU isolated venv + download pipeline models (~2 GB)
	$(call log,Setting up isolated MinerU venv ...)
	@uv venv $(BENCH_DIR)/.venvs/mineru --quiet 2>/dev/null || true
	@uv pip install "mineru[all]" --python $(BENCH_DIR)/.venvs/mineru/bin/python -q
	$(call log,Downloading MinerU pipeline models from HuggingFace ...)
	@$(BENCH_DIR)/.venvs/mineru/bin/mineru-models-download \
		--source huggingface --model_type pipeline

bench-compare-all: build bench-setup ## Compare EdgeParse against ALL engines: opendataloader, docling, marker, mineru, pymupdf4llm, markitdown, edgequake
	$(call log,Running full multi-engine comparison — all 8 engines ...)
	@cd $(BENCH_DIR) && uv run python compare_all.py --all --install

bench-compare-fast: build bench-setup ## Quick comparison: EdgeParse + pymupdf4llm + markitdown  (installs missing engines)
	$(call log,Running fast comparison  —  lightweight engines only ...)
	@cd $(BENCH_DIR) && uv run python compare_all.py --engines edgeparse,pymupdf4llm,markitdown --install

# ══════════════════════════════════════════════════════════════════════════════
## Quick demos
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
	$(call ok,Output files:)
	@ls -1 /tmp/edgeparse-demo/

# ══════════════════════════════════════════════════════════════════════════════
## Clean up
# ══════════════════════════════════════════════════════════════════════════════

clean: ## Remove Rust build artefacts  (target/)
	$(call warn,Removing target/ ...)
	@cargo clean

clean-bench: ## Remove benchmark prediction outputs  (benchmark/prediction/)
	$(call warn,Removing $(BENCH_DIR)/prediction/ ...)
	@rm -rf $(BENCH_DIR)/prediction/

clean-all: clean clean-bench ## Remove all build artefacts and benchmark outputs
	$(call ok,All clean)
