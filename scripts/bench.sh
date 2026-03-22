#!/usr/bin/env bash
# scripts/bench.sh — build edgeparse and run the full benchmark
# Usage:
#   ./scripts/bench.sh                      # default: run edgeparse benchmark
#   ./scripts/bench.sh --engine edgeparse     # explicit engine
#   ./scripts/bench.sh --html report.html   # generate HTML report
#   ./scripts/bench.sh compare              # run multi-engine comparison
#   ./scripts/bench.sh compare --all        # compare all installed engines
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOLD='\033[1m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
DIM='\033[2m'
RESET='\033[0m'

_log()  { printf "${BOLD}${GREEN} ▶${RESET} ${GREEN}%s${RESET}\n" "$1"; }
_ok()   { printf "${BOLD}${GREEN} ✓${RESET} %s\n" "$1"; }
_warn() { printf "${BOLD}${YELLOW} ⚠${RESET} ${YELLOW}%s${RESET}\n" "$1"; }
_err()  { printf "${BOLD}${RED} ✖${RESET} ${RED}%s${RESET}\n" "$1"; }

# Check uv is installed
if ! command -v uv &>/dev/null; then
  _err "'uv' is not installed. Install it with: curl -Ls https://astral.sh/uv/install.sh | sh"
  exit 1
fi

# Sub-command routing
CMD="${1:-bench}"
case "$CMD" in
  compare|compare-all)
    shift
    _log "Building edgeparse (release)..."
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1 | tail -1
    _ok "Binary ready"

    cd "$REPO_ROOT/benchmark"
    uv sync --quiet
    _log "Running multi-engine comparison..."
    uv run python compare_all.py "$@"
    ;;

  list|engines)
    cd "$REPO_ROOT/benchmark"
    uv sync --quiet
    uv run python compare_all.py --list
    ;;

  install)
    shift
    cd "$REPO_ROOT/benchmark"
    TOOLS="${1:-pymupdf4llm,markitdown}"
    _log "Installing engines: $TOOLS"
    uv sync --quiet
    uv run python compare_all.py --engines "$TOOLS" --install --no-run
    ;;

  bench|run|"")
    shift 2>/dev/null || true
    _log "Building edgeparse (release)..."
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1 | tail -1
    _ok "Binary ready"

    cd "$REPO_ROOT/benchmark"
    uv sync --quiet
    _log "Running benchmark..."
    uv run python run.py "$@"
    ;;

  help|--help|-h)
    printf "\n${BOLD}EdgeParse Benchmark${RESET}\n\n"
    printf "  ${CYAN}bench.sh${RESET}                          Run EdgeParse benchmark (default)\n"
    printf "  ${CYAN}bench.sh compare --all${RESET}            Compare all installed engines\n"
    printf "  ${CYAN}bench.sh compare --engines e1,e2${RESET}  Compare specific engines\n"
    printf "  ${CYAN}bench.sh list${RESET}                     List available engines\n"
    printf "  ${CYAN}bench.sh install pymupdf4llm${RESET}      Install comparison engines\n"
    printf "  ${CYAN}bench.sh --html report.html${RESET}       Generate HTML report\n\n"
    ;;

  *)
    # Pass everything through to run.py
    _log "Building edgeparse (release)..."
    cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1 | tail -1
    _ok "Binary ready"

    cd "$REPO_ROOT/benchmark"
    uv sync --quiet
    _log "Running benchmark..."
    uv run python run.py "$@"
    ;;
esac
