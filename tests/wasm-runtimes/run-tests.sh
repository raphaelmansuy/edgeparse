#!/usr/bin/env bash
# ─── EdgeParse WASM/RISC-V Integration Test Runner ─────────────────────────
# Greg's AI coding buddy:
#   This script runs INSIDE the Docker container. It exercises edgeparse
#   against test PDFs using whichever runtime the container provides.
#
#   Usage (automatic via ENTRYPOINT):
#     run-tests.sh wasmtime    # test with wasmtime runtime
#     run-tests.sh wasmer      # test with wasmer runtime
#     run-tests.sh wasmedge    # test with wasmedge runtime
#     run-tests.sh wamr        # test with wamr/iwasm runtime
#     run-tests.sh riscv-qemu  # test with QEMU RISC-V emulation
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

# ── Terminal colours (because we're civilized) ──────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

RUNTIME="${1:-unknown}"
PASS=0
FAIL=0
SKIP=0
TOTAL=0

log_header() {
    echo -e "\n${BOLD}${CYAN}═══════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}${CYAN}  EdgeParse Integration Tests — ${RUNTIME}${RESET}"
    echo -e "${BOLD}${CYAN}═══════════════════════════════════════════════════════════${RESET}\n"
}

log_test() {
    TOTAL=$((TOTAL + 1))
    echo -e "${DIM}[${TOTAL}]${RESET} ${BOLD}$1${RESET}"
}

log_pass() {
    PASS=$((PASS + 1))
    echo -e "  ${GREEN}PASS${RESET} $1"
}

log_fail() {
    FAIL=$((FAIL + 1))
    echo -e "  ${RED}FAIL${RESET} $1"
}

log_skip() {
    SKIP=$((SKIP + 1))
    echo -e "  ${YELLOW}SKIP${RESET} $1"
}

log_summary() {
    echo -e "\n${BOLD}───────────────────────────────────────────────────────────${RESET}"
    echo -e "${BOLD}Results:${RESET} ${GREEN}${PASS} passed${RESET}, ${RED}${FAIL} failed${RESET}, ${YELLOW}${SKIP} skipped${RESET} / ${TOTAL} total"
    echo -e "${BOLD}Runtime:${RESET} ${RUNTIME}"
    echo -e "${BOLD}───────────────────────────────────────────────────────────${RESET}"
}

# ── Build the runtime command ───────────────────────────────────────────────
# Each runtime has slightly different CLI syntax. We normalize here.
build_run_cmd() {
    local wasm_or_bin="$1"
    shift
    # NOTE: Do NOT use "--" to separate runtime flags from wasm args.
    # Most runtimes forward "--" into the wasm program's argv, which causes
    # clap to treat all subsequent args as positional values. Instead, place
    # wasm program args directly after the .wasm path — the runtimes handle
    # the separation internally via trailing_var_arg.
    case "${RUNTIME}" in
        wasmtime)
            # wasmtime: trailing args after .wasm are passed to the program;
            # do NOT use "--" as wasmtime forwards it into argv
            echo "wasmtime run --dir / ${wasm_or_bin} $*"
            ;;
        wasmer)
            # wasmer v7+: uses --volume (--dir is deprecated);
            # requires "--" to separate wasmer flags from wasm args
            echo "wasmer run --volume /test:/test ${wasm_or_bin} -- $*"
            ;;
        wasmedge)
            # wasmedge: --dir guest_path:host_path
            echo "wasmedge --dir /:/  ${wasm_or_bin} $*"
            ;;
        wamr)
            # iwasm: --dir=path preopens a directory
            echo "iwasm --dir=/ ${wasm_or_bin} $*"
            ;;
        wasix)
            # WASIX on Wasmer: same as wasmer but with WASIX binary
            echo "wasmer run --volume /test:/test ${wasm_or_bin} -- $*"
            ;;
        riscv-qemu)
            # RISC-V: direct execution under QEMU user-mode (native binary)
            echo "qemu-riscv64 ${wasm_or_bin} $*"
            ;;
        spike)
            # Spike + pk: RISC-V ISA reference simulator with proxy kernel
            echo "spike pk ${wasm_or_bin} $*"
            ;;
        libriscv)
            # libriscv rvlinux: fastest RISC-V sandbox, Linux syscall emulation
            # rvlinux intercepts -f (fuel) and -h (help), so use -- to separate
            echo "rvlinux ${wasm_or_bin} -- $*"
            ;;
        rvvm)
            # RVVM: tracing JIT RISC-V emulator, userland mode
            echo "rvvm-userland ${wasm_or_bin} $*"
            ;;
        ckb-vm)
            # CKB-VM: blockchain RISC-V VM (limited syscall support)
            echo "ckb-debugger --bin ${wasm_or_bin} -- $*"
            ;;
        *)
            echo "echo 'Unknown runtime: ${RUNTIME}' && false"
            ;;
    esac
}

# ── Determine binary path ──────────────────────────────────────────────────
case "${RUNTIME}" in
    riscv-qemu|spike|libriscv|rvvm|ckb-vm)
        BINARY="/test/edgeparse-riscv64"
        ;;
    *)
        BINARY="/test/edgeparse.wasm"
        ;;
esac

# ── Pre-flight checks ──────────────────────────────────────────────────────
log_header

echo -e "${DIM}Binary: ${BINARY}${RESET}"
echo -e "${DIM}Size:   $(du -h "${BINARY}" 2>/dev/null | cut -f1 || echo 'N/A')${RESET}"
echo -e "${DIM}Type:   $(file "${BINARY}" 2>/dev/null || echo 'N/A')${RESET}"

if [ ! -f "${BINARY}" ]; then
    echo -e "\n${RED}ERROR: Binary not found at ${BINARY}${RESET}"
    exit 1
fi

if [ ! -f "/test/fixtures/sample.pdf" ]; then
    echo -e "\n${RED}ERROR: Test fixture not found at /test/fixtures/sample.pdf${RESET}"
    exit 1
fi

mkdir -p /test/output

# ─────────────────────────────────────────────────────────────────────────────
# TEST 1: --help flag works
# ─────────────────────────────────────────────────────────────────────────────
log_test "CLI --help flag"
run_cmd=$(build_run_cmd "${BINARY}" "--help")
if eval "${run_cmd}" > /test/output/help.txt 2>&1; then
    if grep -qi "edgeparse" /test/output/help.txt; then
        log_pass "Help output contains 'edgeparse'"
    else
        log_fail "Help output doesn't mention 'edgeparse'"
    fi
else
    log_fail "Exit code non-zero: ${run_cmd}"
fi

# ─────────────────────────────────────────────────────────────────────────────
# TEST 2: --version flag works
# ─────────────────────────────────────────────────────────────────────────────
log_test "CLI --version flag"
run_cmd=$(build_run_cmd "${BINARY}" "--version")
if eval "${run_cmd}" > /test/output/version.txt 2>&1; then
    if grep -qE '[0-9]+\.[0-9]+\.[0-9]+' /test/output/version.txt; then
        version=$(cat /test/output/version.txt)
        log_pass "Version: ${version}"
    else
        log_fail "Version output doesn't match semver pattern"
    fi
else
    log_fail "Exit code non-zero"
fi

# ─────────────────────────────────────────────────────────────────────────────
# TEST 3: Convert sample PDF to JSON
# ─────────────────────────────────────────────────────────────────────────────
log_test "Convert sample.pdf → JSON"
run_cmd=$(build_run_cmd "${BINARY}" "-f json -o /test/output -q /test/fixtures/sample.pdf")
if eval "${run_cmd}" > /test/output/json_stdout.txt 2>&1; then
    if [ -f "/test/output/sample.json" ]; then
        json_size=$(wc -c < /test/output/sample.json)
        if [ "${json_size}" -gt 10 ]; then
            log_pass "JSON output: ${json_size} bytes"
        else
            log_fail "JSON output too small: ${json_size} bytes"
        fi
    else
        log_fail "Expected /test/output/sample.json not created"
    fi
else
    log_fail "Conversion failed (exit=$?)"
    cat /test/output/json_stdout.txt 2>/dev/null || true
fi

# ─────────────────────────────────────────────────────────────────────────────
# TEST 4: Convert sample PDF to Markdown
# ─────────────────────────────────────────────────────────────────────────────
log_test "Convert sample.pdf → Markdown"
# Clean previous output
rm -f /test/output/sample.md
run_cmd=$(build_run_cmd "${BINARY}" "-f markdown -o /test/output -q /test/fixtures/sample.pdf")
if eval "${run_cmd}" > /test/output/md_stdout.txt 2>&1; then
    if [ -f "/test/output/sample.md" ]; then
        md_size=$(wc -c < /test/output/sample.md)
        if [ "${md_size}" -gt 5 ]; then
            log_pass "Markdown output: ${md_size} bytes"
        else
            log_fail "Markdown output too small: ${md_size} bytes"
        fi
    else
        log_fail "Expected /test/output/sample.md not created"
    fi
else
    log_fail "Conversion failed (exit=$?)"
    cat /test/output/md_stdout.txt 2>/dev/null || true
fi

# ─────────────────────────────────────────────────────────────────────────────
# TEST 5: Convert sample PDF to plain text
# ─────────────────────────────────────────────────────────────────────────────
log_test "Convert sample.pdf → Text"
rm -f /test/output/sample.txt
run_cmd=$(build_run_cmd "${BINARY}" "-f text -o /test/output -q /test/fixtures/sample.pdf")
if eval "${run_cmd}" > /test/output/txt_stdout.txt 2>&1; then
    if [ -f "/test/output/sample.txt" ]; then
        txt_size=$(wc -c < /test/output/sample.txt)
        if [ "${txt_size}" -gt 5 ]; then
            log_pass "Text output: ${txt_size} bytes"
            # Bonus: check content looks like our test PDF
            if grep -qi "Hello\|EdgePDF\|test" /test/output/sample.txt 2>/dev/null; then
                log_pass "Content sanity check — found expected text"
            else
                log_fail "Content sanity check — expected text not found"
            fi
        else
            log_fail "Text output too small: ${txt_size} bytes"
        fi
    else
        log_fail "Expected /test/output/sample.txt not created"
    fi
else
    log_fail "Conversion failed (exit=$?)"
    cat /test/output/txt_stdout.txt 2>/dev/null || true
fi

# ─────────────────────────────────────────────────────────────────────────────
# TEST 6: Convert to HTML
# ─────────────────────────────────────────────────────────────────────────────
log_test "Convert sample.pdf → HTML"
rm -f /test/output/sample.html
run_cmd=$(build_run_cmd "${BINARY}" "-f html -o /test/output -q /test/fixtures/sample.pdf")
if eval "${run_cmd}" > /test/output/html_stdout.txt 2>&1; then
    if [ -f "/test/output/sample.html" ]; then
        html_size=$(wc -c < /test/output/sample.html)
        log_pass "HTML output: ${html_size} bytes"
    else
        log_fail "Expected /test/output/sample.html not created"
    fi
else
    log_fail "Conversion failed (exit=$?)"
fi

# ─────────────────────────────────────────────────────────────────────────────
# TEST 7: Bad input (non-existent file) returns error
# ─────────────────────────────────────────────────────────────────────────────
log_test "Error handling: non-existent file"
run_cmd=$(build_run_cmd "${BINARY}" "-q /test/fixtures/does_not_exist.pdf")
if eval "${run_cmd}" > /test/output/err_stdout.txt 2>&1; then
    log_fail "Should have returned non-zero exit code for missing file"
else
    log_pass "Correctly returned error for non-existent file"
fi

# ── Summary ─────────────────────────────────────────────────────────────────
log_summary

if [ "${FAIL}" -gt 0 ]; then
    exit 1
fi
exit 0
