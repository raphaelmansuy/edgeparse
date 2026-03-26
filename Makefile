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
        publish-node publish-node-dry publish-wasm publish-wasm-dry \
        publish-cli publish-cli-dry \
        publish-brew publish-brew-dry \
        wasm-build wasm-check wasm-size wasm-clean \
        publish-all \
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
BINARY      := target/release/edgeparse
BENCH_DIR   := benchmark
EXAMPLES    := examples/pdf
RELEASE_DIR := target/release-dist

# ── Version (read from workspace Cargo.toml) ──────────────────────────────────
VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

# ── Homebrew tap repository ───────────────────────────────────────────────────
# Override to point at a fork:  make publish-brew BREW_TAP_REPO=your-org/homebrew-edgeparse
BREW_TAP_REPO ?= raphaelmansuy/homebrew-edgeparse

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
publish-rust-dry: ## Dry-run: verify pdf-cos + edgeparse-core publish cleanly and edgeparse-cli packages cleanly
	$(call log,cargo publish --dry-run  [pdf-cos])
	@cargo publish -p pdf-cos --dry-run --allow-dirty
	$(call log,cargo publish --dry-run  [edgeparse-core])
	@cargo publish -p edgeparse-core --dry-run --allow-dirty
	$(call log,cargo package --allow-dirty  [edgeparse-cli])
	@OUTPUT=$$(cargo package -p edgeparse-cli --allow-dirty 2>&1) && echo "$$OUTPUT" || { \
	  echo "$$OUTPUT"; \
	  if echo "$$OUTPUT" | grep -q 'location searched: crates.io index' \
	    && echo "$$OUTPUT" | grep -q 'required by package `edgeparse-cli'; then \
	    printf "$(BOLD)$(YELLOW) ⚠$(RESET) $(YELLOW)edgeparse-cli package dry-run requires edgeparse-core $(VERSION) to already exist on crates.io; the tagged CI release handles that publish order.$(RESET)\n"; \
	  else \
	    exit 1; \
	  fi; \
	}
	$(call ok,Rust dry-run passed — ready for crates.io)

publish-rust: ## Publish pdf-cos, edgeparse-core, then edgeparse-cli to crates.io
	$(call log,Publishing pdf-cos to crates.io ...)
	@cargo publish -p pdf-cos
	$(call log,Waiting 30 s for crates.io index to propagate ...)
	@sleep 30
	$(call log,Publishing edgeparse-core to crates.io ...)
	@cargo publish -p edgeparse-core
	$(call log,Waiting 30 s for crates.io index to propagate ...)
	@sleep 30
	$(call log,Publishing edgeparse-cli to crates.io ...)
	@cargo publish -p edgeparse-cli
	$(call ok,Rust crates published)

# ── Python / PyPI — all architectures ────────────────────────────────────────
# Linux   : manylinux Docker container (requires Docker)
# macOS   : native cross-compilation via rustup targets
# Windows : zig cross-linker (pip install "maturin[zig]" + brew install zig)
PYTHON_LINUX_TARGETS  := x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
PYTHON_DARWIN_TARGETS := x86_64-apple-darwin aarch64-apple-darwin
PYTHON_WIN_TARGETS    := x86_64-pc-windows-gnu
# Python versions to target for Linux cross-compilation wheels:
PYTHON_VERSIONS       := python3.10 python3.11 python3.12 python3.13

publish-python-dry: ## Dry-run: build all-arch Python wheels and validate with twine
	$(call log,Building all-arch Python wheels [dry-run] ...)
	@rm -rf sdks/python/dist && mkdir -p sdks/python/dist
	@rustup target add $(PYTHON_LINUX_TARGETS) $(PYTHON_DARWIN_TARGETS) 2>/dev/null || true
	@for t in $(PYTHON_LINUX_TARGETS); do \
	  for pyver in $(PYTHON_VERSIONS); do \
	    which $$pyver >/dev/null 2>&1 || continue; \
	    printf " $(CYAN)→$(RESET) maturin build --target $$t --zig --compatibility manylinux2014 -i $$pyver\n"; \
	    (cd sdks/python && maturin build --release --target $$t --zig --compatibility manylinux2014 -i $$pyver --out dist/) \
	      || printf "$(YELLOW)  ⚠ Linux $$t ($$pyver) skipped$(RESET)\n"; \
	  done; \
	done
	@for t in $(PYTHON_DARWIN_TARGETS); do \
	  printf " $(CYAN)→$(RESET) maturin build --target $$t\n"; \
	  (cd sdks/python && maturin build --release --target $$t --out dist/) || exit 1; \
	done
	@for t in $(PYTHON_WIN_TARGETS); do \
	  printf " $(CYAN)→$(RESET) maturin build --target $$t --zig\n"; \
	  for pyver in $(PYTHON_VERSIONS); do \
	    which $$pyver >/dev/null 2>&1 || continue; \
	    (cd sdks/python && maturin build --release --target $$t --zig -i $$pyver --out dist/) \
	      || printf "$(YELLOW)  ⚠ Windows zig build skipped ($$pyver)$(RESET)\n"; \
	  done; \
	done
	$(call log,twine check — validating all wheels ...)
	@twine check sdks/python/dist/*.whl
	$(call ok,Python dry-run passed — all wheels valid)

publish-python: ## Build all-arch Python wheels (Linux · macOS · Windows) and upload to PyPI
	$(call log,Building all-arch Python wheels ...)
	@rm -rf sdks/python/dist && mkdir -p sdks/python/dist
	@rustup target add $(PYTHON_LINUX_TARGETS) $(PYTHON_DARWIN_TARGETS) 2>/dev/null || true
	@for t in $(PYTHON_LINUX_TARGETS); do \
	  for pyver in $(PYTHON_VERSIONS); do \
	    which $$pyver >/dev/null 2>&1 || continue; \
	    printf " $(CYAN)→$(RESET) maturin build --target $$t --zig --compatibility manylinux2014 -i $$pyver\n"; \
	    (cd sdks/python && maturin build --release --target $$t --zig --compatibility manylinux2014 -i $$pyver --out dist/) \
	      || printf "$(YELLOW)  ⚠ Linux $$t ($$pyver) skipped$(RESET)\n"; \
	  done; \
	done
	@for t in $(PYTHON_DARWIN_TARGETS); do \
	  printf " $(CYAN)→$(RESET) maturin build --target $$t\n"; \
	  (cd sdks/python && maturin build --release --target $$t --out dist/) || exit 1; \
	done
	@for t in $(PYTHON_WIN_TARGETS); do \
	  printf " $(CYAN)→$(RESET) maturin build --target $$t --zig\n"; \
	  for pyver in $(PYTHON_VERSIONS); do \
	    which $$pyver >/dev/null 2>&1 || continue; \
	    (cd sdks/python && maturin build --release --target $$t --zig -i $$pyver --out dist/) \
	      || printf "$(YELLOW)  ⚠ Windows zig build skipped ($$pyver)$(RESET)\n"; \
	  done; \
	done
	$(call log,Building source distribution ...)
	@cd sdks/python && maturin sdist --out dist/
	$(call log,Uploading all wheels + sdist to PyPI ...)
	@twine upload sdks/python/dist/* --username __token__ --password "$$PYPI_PASSWORD"
	$(call ok,Python SDK published to PyPI — all architectures)

# ── Node.js / npm — all architectures ────────────────────────────────────────
# macOS   : cargo (native) after rustup target add
# Linux   : cargo zigbuild (zig as cross-linker, no Docker needed).
# Windows : cargo zigbuild; skipped gracefully if cross-compilation fails.
publish-node-dry: ## Dry-run: build all-arch Node.js .node binaries + show what npm would publish
	$(call log,Installing Rust cross-compilation targets ...)
	@rustup target add aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu 2>/dev/null || true
	$(call log,Building macOS ARM64 ...)
	@cargo build --release --target aarch64-apple-darwin -p edgeparse-node
	@mkdir -p sdks/node/npm/darwin-arm64
	@cp target/aarch64-apple-darwin/release/libedgeparse_node.dylib \
	   sdks/node/npm/darwin-arm64/edgeparse-node.darwin-arm64.node
	$(call log,Building macOS x86_64 ...)
	@cargo build --release --target x86_64-apple-darwin -p edgeparse-node
	@mkdir -p sdks/node/npm/darwin-x64
	@cp target/x86_64-apple-darwin/release/libedgeparse_node.dylib \
	   sdks/node/npm/darwin-x64/edgeparse-node.darwin-x64.node
	$(call log,Building Linux x86_64 [zigbuild] ...)
	@{ cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17 -p edgeparse-node \
	  && mkdir -p sdks/node/npm/linux-x64-gnu \
	  && cp target/x86_64-unknown-linux-gnu/release/libedgeparse_node.so \
	       sdks/node/npm/linux-x64-gnu/edgeparse-node.linux-x64-gnu.node; } \
	  || printf "$(YELLOW)  ⚠ Linux x86_64 node skipped$(RESET)\n"
	$(call log,Building Linux ARM64 [zigbuild] ...)
	@{ cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.17 -p edgeparse-node \
	  && mkdir -p sdks/node/npm/linux-arm64-gnu \
	  && cp target/aarch64-unknown-linux-gnu/release/libedgeparse_node.so \
	       sdks/node/npm/linux-arm64-gnu/edgeparse-node.linux-arm64-gnu.node; } \
	  || printf "$(YELLOW)  ⚠ Linux ARM64 node skipped$(RESET)\n"
	$(call log,Building Windows x86_64 [zigbuild — optional] ...)
	@{ rustup target add x86_64-pc-windows-gnu 2>/dev/null || true; \
	   cargo zigbuild --release --target x86_64-pc-windows-gnu -p edgeparse-node \
	  && mkdir -p sdks/node/npm/win32-x64-msvc \
	  && cp target/x86_64-pc-windows-gnu/release/edgeparse_node.dll \
	       sdks/node/npm/win32-x64-msvc/edgeparse-node.win32-x64-msvc.node; } \
	  || printf "$(YELLOW)  ⚠ Windows target skipped$(RESET)\n"
	$(call log,Building TypeScript ...)
	@cd sdks/node && npm install --ignore-scripts && npm run build:ts
	$(call log,npm pack --dry-run [platform packages] ...)
	@for dir in sdks/node/npm/*/; do \
	  [ -f "$$dir/package.json" ] && (cd "$$dir" && npm pack --dry-run) || true; \
	done
	$(call log,npm pack --dry-run [main package] ...)
	@cd sdks/node && npm pack --dry-run
	$(call ok,Node.js dry-run passed — all architectures)

publish-node: ## Build all-arch Node.js .node binaries and publish all platform + main packages to npm
	$(call log,Installing Rust cross-compilation targets ...)
	@rustup target add aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu 2>/dev/null || true
	$(call log,Building macOS ARM64 ...)
	@cargo build --release --target aarch64-apple-darwin -p edgeparse-node
	@mkdir -p sdks/node/npm/darwin-arm64
	@cp target/aarch64-apple-darwin/release/libedgeparse_node.dylib \
	   sdks/node/npm/darwin-arm64/edgeparse-node.darwin-arm64.node
	$(call log,Building macOS x86_64 ...)
	@cargo build --release --target x86_64-apple-darwin -p edgeparse-node
	@mkdir -p sdks/node/npm/darwin-x64
	@cp target/x86_64-apple-darwin/release/libedgeparse_node.dylib \
	   sdks/node/npm/darwin-x64/edgeparse-node.darwin-x64.node
	$(call log,Building Linux x86_64 [zigbuild] ...)
	@{ cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17 -p edgeparse-node \
	  && mkdir -p sdks/node/npm/linux-x64-gnu \
	  && cp target/x86_64-unknown-linux-gnu/release/libedgeparse_node.so \
	       sdks/node/npm/linux-x64-gnu/edgeparse-node.linux-x64-gnu.node; } \
	  || printf "$(YELLOW)  ⚠ Linux x86_64 node skipped$(RESET)\n"
	$(call log,Building Linux ARM64 [zigbuild] ...)
	@{ cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.17 -p edgeparse-node \
	  && mkdir -p sdks/node/npm/linux-arm64-gnu \
	  && cp target/aarch64-unknown-linux-gnu/release/libedgeparse_node.so \
	       sdks/node/npm/linux-arm64-gnu/edgeparse-node.linux-arm64-gnu.node; } \
	  || printf "$(YELLOW)  ⚠ Linux ARM64 node skipped$(RESET)\n"
	$(call log,Building Windows x86_64 [zigbuild — optional] ...)
	@{ rustup target add x86_64-pc-windows-gnu 2>/dev/null || true; \
	   cargo zigbuild --release --target x86_64-pc-windows-gnu -p edgeparse-node \
	  && mkdir -p sdks/node/npm/win32-x64-msvc \
	  && cp target/x86_64-pc-windows-gnu/release/edgeparse_node.dll \
	       sdks/node/npm/win32-x64-msvc/edgeparse-node.win32-x64-msvc.node; } \
	  || printf "$(YELLOW)  ⚠ Windows target skipped$(RESET)\n"
	$(call log,Building TypeScript ...)
	@cd sdks/node && npm install --ignore-scripts && npm run build:ts
	$(call log,Publishing platform packages to npm ...)
	@for dir in sdks/node/npm/*/; do \
	  [ -f "$$dir/package.json" ] \
	    && (cd "$$dir" && npm publish --access public \
	         && printf "$(GREEN)  ✓$(RESET) published $$dir\n") \
	    || printf "$(YELLOW)  ⚠$(RESET) skip $$dir (no package.json)\n"; \
	done
	$(call log,Publishing main edgeparse package to npm ...)
	@cd sdks/node && npm publish --access public
	$(call ok,Node.js SDK published to npm — all architectures)

# ── CLI binaries + GitHub Release — all architectures ─────────────────────────
# Builds the `edgeparse` CLI binary for every platform, packages each into a
# tarball or zip, and attaches them to a GitHub Release.
#
# Prerequisites:
#   macOS native:   cargo + rustup   (already present)
#   Linux/Windows:  cargo install cargo-zigbuild  +  brew install zig
#   GitHub release: gh CLI authenticated
#
publish-cli-dry: ## Dry-run: show all CLI artifacts that would be built
	$(call log,CLI build plan — v$(VERSION))
	@printf "  $(CYAN)aarch64-apple-darwin$(RESET)         → edgeparse-$(VERSION)-aarch64-apple-darwin.tar.gz\n"
	@printf "  $(CYAN)x86_64-apple-darwin$(RESET)          → edgeparse-$(VERSION)-x86_64-apple-darwin.tar.gz\n"
	@printf "  $(CYAN)x86_64-unknown-linux-gnu$(RESET)     → edgeparse-$(VERSION)-x86_64-unknown-linux-gnu.tar.gz\n"
	@printf "  $(CYAN)aarch64-unknown-linux-gnu$(RESET)    → edgeparse-$(VERSION)-aarch64-unknown-linux-gnu.tar.gz\n"
	@printf "  $(CYAN)x86_64-pc-windows-gnu$(RESET)        → edgeparse-$(VERSION)-x86_64-pc-windows-gnu.zip   (skipped if cargo-zigbuild unavailable)\n"
	$(call ok,CLI dry-run — artifacts would be uploaded to GitHub Release v$(VERSION))

publish-cli: ## Build all-arch CLI binaries and attach to GitHub Release v$(VERSION)
	$(call log,Building CLI v$(VERSION) for all architectures ...)
	@rm -rf $(RELEASE_DIR) && mkdir -p $(RELEASE_DIR)
	@rustup target add aarch64-apple-darwin x86_64-apple-darwin 2>/dev/null || true
	$(call log,Building macOS ARM64 ...)
	@cargo build --release --target aarch64-apple-darwin -p edgeparse-cli
	@cp target/aarch64-apple-darwin/release/edgeparse $(RELEASE_DIR)/edgeparse
	@tar -czf $(RELEASE_DIR)/edgeparse-$(VERSION)-aarch64-apple-darwin.tar.gz \
	   -C $(RELEASE_DIR) edgeparse \
	   -C $(CURDIR) README.md LICENSE
	$(call log,Building macOS x86_64 ...)
	@cargo build --release --target x86_64-apple-darwin -p edgeparse-cli
	@cp target/x86_64-apple-darwin/release/edgeparse $(RELEASE_DIR)/edgeparse
	@tar -czf $(RELEASE_DIR)/edgeparse-$(VERSION)-x86_64-apple-darwin.tar.gz \
	   -C $(RELEASE_DIR) edgeparse \
	   -C $(CURDIR) README.md LICENSE
	$(call log,Building Linux x86_64 [zigbuild] ...)
	@rustup target add x86_64-unknown-linux-gnu 2>/dev/null || true
	@{ cargo zigbuild --release --target x86_64-unknown-linux-gnu.2.17 -p edgeparse-cli \
	  && cp target/x86_64-unknown-linux-gnu/release/edgeparse $(RELEASE_DIR)/edgeparse \
	  && tar -czf $(RELEASE_DIR)/edgeparse-$(VERSION)-x86_64-unknown-linux-gnu.tar.gz \
	     -C $(RELEASE_DIR) edgeparse \
	     -C $(CURDIR) README.md LICENSE; } \
	  || printf "$(YELLOW)  ⚠ Linux x86_64 skipped (cargo-zigbuild unavailable)$(RESET)\n"
	$(call log,Building Linux ARM64 [zigbuild] ...)
	@rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
	@{ cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.17 -p edgeparse-cli \
	  && cp target/aarch64-unknown-linux-gnu/release/edgeparse $(RELEASE_DIR)/edgeparse \
	  && tar -czf $(RELEASE_DIR)/edgeparse-$(VERSION)-aarch64-unknown-linux-gnu.tar.gz \
	     -C $(RELEASE_DIR) edgeparse \
	     -C $(CURDIR) README.md LICENSE; } \
	  || printf "$(YELLOW)  ⚠ Linux ARM64 skipped (cargo-zigbuild unavailable)$(RESET)\n"
	$(call log,Building Windows x86_64 [zigbuild] ...)
	@rustup target add x86_64-pc-windows-gnu 2>/dev/null || true
	@{ cargo zigbuild --release --target x86_64-pc-windows-gnu -p edgeparse-cli \
	  && cp target/x86_64-pc-windows-gnu/release/edgeparse.exe $(RELEASE_DIR)/ \
	  && cd $(RELEASE_DIR) \
	  && zip -q edgeparse-$(VERSION)-x86_64-pc-windows-gnu.zip edgeparse.exe; } \
	  || printf "$(YELLOW)  ⚠ Windows target skipped (cargo-zigbuild unavailable)$(RESET)\n"
	@rm -f $(RELEASE_DIR)/edgeparse $(RELEASE_DIR)/edgeparse.exe
	$(call log,Creating / updating GitHub Release v$(VERSION) ...)
	@gh release view v$(VERSION) --repo raphaelmansuy/edgeparse >/dev/null 2>&1 \
	  || gh release create v$(VERSION) \
	       --repo raphaelmansuy/edgeparse \
	       --title "v$(VERSION)" \
	       --notes "Release v$(VERSION)"
	$(call log,Uploading CLI artifacts ...)
	@for f in $(RELEASE_DIR)/edgeparse-$(VERSION)-*.tar.gz \
	           $(RELEASE_DIR)/edgeparse-$(VERSION)-*.zip; do \
	  [ -f "$$f" ] || continue; \
	  gh release upload v$(VERSION) "$$f" \
	    --repo raphaelmansuy/edgeparse --clobber \
	    && printf "$(GREEN)  ✓$(RESET) uploaded $$f\n"; \
	done
	$(call ok,CLI binaries v$(VERSION) attached to GitHub Release)

# ── Homebrew tap — formula generation + push ──────────────────────────────────
# Generates Formula/edgeparse.rb by downloading the macOS release assets
# from the GitHub Release and computing their SHA-256, then pushes the
# formula to the tap repo (BREW_TAP_REPO).  Run AFTER publish-cli.
#
# Prerequisites:  gh CLI authenticated, git, curl, shasum
#
publish-brew-dry: ## Dry-run: generate Homebrew formula from published release assets (no push)
	$(call log,Generating Homebrew formula [dry-run] v$(VERSION) ...)
	@mkdir -p Formula
	@bash scripts/gen-formula.sh $(VERSION) Formula/edgeparse.rb
	@printf "$(GREEN)  ✓$(RESET) Formula/edgeparse.rb written (dry-run — not pushed)\n"
	@cat Formula/edgeparse.rb

publish-brew: ## Generate Homebrew formula and push to $(BREW_TAP_REPO)
	$(call log,Generating Homebrew formula v$(VERSION) ...)
	@mkdir -p Formula
	@bash scripts/gen-formula.sh $(VERSION) Formula/edgeparse.rb
	$(call log,Pushing formula to $(BREW_TAP_REPO) ...)
	@TAPDIR=$$(mktemp -d); \
	 git clone "https://github.com/$(BREW_TAP_REPO).git" "$$TAPDIR" 2>&1; \
	 mkdir -p "$$TAPDIR/Formula"; \
	 cp Formula/edgeparse.rb "$$TAPDIR/Formula/edgeparse.rb"; \
	 cd "$$TAPDIR" \
	   && git config user.email "actions@github.com" \
	   && git config user.name "EdgeParse Release Bot" \
	   && git add Formula/edgeparse.rb \
	   && git commit -m "edgeparse $(VERSION)" \
	   && git push origin HEAD; \
	 rm -rf "$$TAPDIR"
	$(call ok,Homebrew formula v$(VERSION) pushed to $(BREW_TAP_REPO))

# ── WASM / npm ────────────────────────────────────────────────────────────────
publish-wasm-dry: ## Dry-run: build the WASM package and preview the npm tarball
	$(call log,Building WebAssembly package [dry-run] ...)
	@command -v wasm-pack >/dev/null 2>&1 || { \
	  $(call err,wasm-pack not found — install: cargo install wasm-pack); \
	  exit 1; }
	@cd crates/edgeparse-wasm && wasm-pack build --target web --release
	@node -e "const fs=require('fs');const p='crates/edgeparse-wasm/pkg/package.json';const pkg=JSON.parse(fs.readFileSync(p,'utf8'));pkg.name='@edgeparse/edgeparse-wasm';pkg.version='$(VERSION)';fs.writeFileSync(p,JSON.stringify(pkg,null,2)+'\n');"
	@cd crates/edgeparse-wasm/pkg && npm pack --dry-run
	$(call ok,WASM dry-run passed — ready for npm)

publish-wasm: ## Build and publish the WASM npm package (@edgeparse/edgeparse-wasm)
ifndef NPM_TOKEN
	$(call err,NPM_TOKEN is required.  Usage:  NPM_TOKEN=<token> make publish-wasm)
	@exit 1
endif
	$(call log,Publishing @edgeparse/edgeparse-wasm to npm ...)
	@command -v wasm-pack >/dev/null 2>&1 || { \
	  $(call err,wasm-pack not found — install: cargo install wasm-pack); \
	  exit 1; }
	@printf "//registry.npmjs.org/:_authToken=%s\n" "$(NPM_TOKEN)" > ~/.npmrc
	@cd crates/edgeparse-wasm && wasm-pack build --target web --release
	@node -e "const fs=require('fs');const p='crates/edgeparse-wasm/pkg/package.json';const pkg=JSON.parse(fs.readFileSync(p,'utf8'));pkg.name='@edgeparse/edgeparse-wasm';pkg.version='$(VERSION)';fs.writeFileSync(p,JSON.stringify(pkg,null,2)+'\n');"
	@cd crates/edgeparse-wasm/pkg && npm publish --access public
	@rm -f ~/.npmrc
	$(call ok,WASM package published to npm)

# ── Combined ──────────────────────────────────────────────────────────────────
publish-all: publish-rust publish-python publish-node publish-wasm publish-cli publish-brew ## Publish everything: crates + Python + Node + WASM + CLI + Homebrew
	$(call ok,All publish targets completed)

# ══════════════════════════════════════════════════════════════════════════════
## WASM
# ══════════════════════════════════════════════════════════════════════════════

WASM_CRATE := crates/edgeparse-wasm

wasm-build: ## Build WASM package (release, --target web)
	$(call log,Building WASM package...)
	@cd $(WASM_CRATE) && wasm-pack build --target web --release --scope edgeparse
	$(call ok,WASM package built → $(WASM_CRATE)/pkg/)

wasm-check: ## Check WASM compilation (fast, no codegen)
	$(call log,Checking WASM compilation...)
	@cargo check --target wasm32-unknown-unknown -p edgeparse-wasm
	$(call ok,WASM check passed)

wasm-size: wasm-build ## Show WASM binary size
	@echo "Raw WASM size:"
	@du -h $(WASM_CRATE)/pkg/edgeparse_wasm_bg.wasm

wasm-clean: ## Remove WASM build artefacts (pkg/)
	$(call warn,Removing $(WASM_CRATE)/pkg/ ...)
	@rm -rf $(WASM_CRATE)/pkg/

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
