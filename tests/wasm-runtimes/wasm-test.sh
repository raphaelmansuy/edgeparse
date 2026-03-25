#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
#  EdgeParse WASM/RISC-V Integration Test Manager
# ═══════════════════════════════════════════════════════════════════════════════
# Greg's AI coding buddy:
#   One script to rule them all. Builds, runs, and manages Docker-based
#   integration tests for edgeparse across multiple WASM runtimes and RISC-V.
#
#   All Docker artifacts are prefixed with EDGEPARSE_PREFIX (default: "edgeparse")
#   to avoid collisions. Override via environment:
#     EDGEPARSE_PREFIX=myproject ./wasm-test.sh build all
#
# Usage:
#   ./wasm-test.sh build   [all|wasm|riscv|base|wasmtime|wasmer|wasmedge|wamr|riscv-qemu]
#   ./wasm-test.sh test    [all|wasmtime|wasmer|wasmedge|wamr|riscv-qemu]
#   ./wasm-test.sh status
#   ./wasm-test.sh log     <runtime>
#   ./wasm-test.sh rmi     [all|<image>]
#   ./wasm-test.sh clean
#   ./wasm-test.sh help
# ═══════════════════════════════════════════════════════════════════════════════
set -euo pipefail

# ── Config ────────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BUILD_DIR="${SCRIPT_DIR}/.build"

# Docker artifact prefix — override with EDGEPARSE_PREFIX env var
PREFIX="${EDGEPARSE_PREFIX:-edgeparse}"

# Image names — WASM
IMG_BUILD_WASM="${PREFIX}-wasi-build"
IMG_BUILD_WASIX="${PREFIX}-wasix-build"
IMG_BASE="${PREFIX}-wasi-base"
IMG_WASMTIME="${PREFIX}-wasi-wasmtime"
IMG_WASMER="${PREFIX}-wasi-wasmer"
IMG_WASMEDGE="${PREFIX}-wasi-wasmedge"
IMG_WAMR="${PREFIX}-wasi-wamr"
IMG_WASIX="${PREFIX}-wasi-wasix"

# Image names — RISC-V
IMG_BUILD_RISCV="${PREFIX}-riscv-build"
IMG_RISCV_QEMU="${PREFIX}-riscv-qemu"
IMG_SPIKE="${PREFIX}-riscv-spike"
IMG_LIBRISCV="${PREFIX}-riscv-libriscv"
IMG_RVVM="${PREFIX}-riscv-rvvm"
IMG_CKB_VM="${PREFIX}-riscv-ckb-vm"

ALL_WASM_RUNTIMES="wasmtime wasmer wasmedge wamr wasix"
ALL_RISCV_RUNTIMES="riscv-qemu spike libriscv rvvm ckb-vm"
ALL_RUNNERS="${ALL_WASM_RUNTIMES} ${ALL_RISCV_RUNTIMES}"
ALL_IMAGES="${IMG_BUILD_WASM} ${IMG_BUILD_WASIX} ${IMG_BUILD_RISCV} ${IMG_BASE} ${IMG_WASMTIME} ${IMG_WASMER} ${IMG_WASMEDGE} ${IMG_WAMR} ${IMG_WASIX} ${IMG_RISCV_QEMU} ${IMG_SPIKE} ${IMG_LIBRISCV} ${IMG_RVVM} ${IMG_CKB_VM}"

# ── Colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

log()  { echo -e "${BOLD}${GREEN} >>>${RESET} $*"; }
warn() { echo -e "${BOLD}${YELLOW} !!${RESET} $*"; }
err()  { echo -e "${BOLD}${RED} ✖${RESET} $*" >&2; }
dim()  { echo -e "${DIM}$*${RESET}"; }

# ── Helpers ───────────────────────────────────────────────────────────────────
image_name_for() {
    case "$1" in
        wasm)        echo "${IMG_BUILD_WASM}" ;;
        wasix)       echo "${IMG_WASIX}" ;;
        riscv)       echo "${IMG_BUILD_RISCV}" ;;
        base)        echo "${IMG_BASE}" ;;
        wasmtime)    echo "${IMG_WASMTIME}" ;;
        wasmer)      echo "${IMG_WASMER}" ;;
        wasmedge)    echo "${IMG_WASMEDGE}" ;;
        wamr)        echo "${IMG_WAMR}" ;;
        riscv-qemu)  echo "${IMG_RISCV_QEMU}" ;;
        spike)       echo "${IMG_SPIKE}" ;;
        libriscv)    echo "${IMG_LIBRISCV}" ;;
        rvvm)        echo "${IMG_RVVM}" ;;
        ckb-vm)      echo "${IMG_CKB_VM}" ;;
        *)           err "Unknown target: $1"; exit 1 ;;
    esac
}

image_exists() {
    docker image inspect "$1" &>/dev/null
}

ensure_build_dir() {
    mkdir -p "${BUILD_DIR}"
}

# ── Build Commands ────────────────────────────────────────────────────────────
build_wasm() {
    log "Building WASM binary (wasm32-wasip1)..."
    ensure_build_dir
    docker build \
        -f "${SCRIPT_DIR}/Dockerfile.build.wasm" \
        -t "${IMG_BUILD_WASM}" \
        "${REPO_ROOT}"

    log "Extracting edgeparse.wasm..."
    local cid
    cid=$(docker create "${IMG_BUILD_WASM}" /bin/true 2>/dev/null || docker create "${IMG_BUILD_WASM}" true)
    docker cp "${cid}:/out/edgeparse.wasm" "${BUILD_DIR}/edgeparse.wasm"
    docker rm "${cid}" > /dev/null
    log "WASM binary: ${BUILD_DIR}/edgeparse.wasm ($(du -h "${BUILD_DIR}/edgeparse.wasm" | cut -f1))"
}

build_riscv() {
    log "Building RISC-V binaries (riscv64gc-unknown-linux-gnu)..."
    ensure_build_dir
    docker build \
        -f "${SCRIPT_DIR}/Dockerfile.build.riscv" \
        -t "${IMG_BUILD_RISCV}" \
        "${REPO_ROOT}"

    log "Extracting RISC-V binaries (dynamic + static)..."
    local cid
    cid=$(docker create "${IMG_BUILD_RISCV}" /bin/true 2>/dev/null || docker create "${IMG_BUILD_RISCV}" true)
    docker cp "${cid}:/out/edgeparse" "${BUILD_DIR}/edgeparse-riscv64"
    docker cp "${cid}:/out/edgeparse-static" "${BUILD_DIR}/edgeparse-riscv64-static"
    docker rm "${cid}" > /dev/null
    log "RISC-V dynamic: ${BUILD_DIR}/edgeparse-riscv64 ($(du -h "${BUILD_DIR}/edgeparse-riscv64" | cut -f1))"
    log "RISC-V static:  ${BUILD_DIR}/edgeparse-riscv64-static ($(du -h "${BUILD_DIR}/edgeparse-riscv64-static" | cut -f1))"
}

build_wasix() {
    log "Building WASIX binary (wasm32-wasmer-wasi)..."
    ensure_build_dir
    docker build \
        -f "${SCRIPT_DIR}/Dockerfile.build.wasix" \
        -t "${IMG_BUILD_WASIX}" \
        "${REPO_ROOT}"

    log "Extracting edgeparse-wasix.wasm..."
    local cid
    cid=$(docker create "${IMG_BUILD_WASIX}" /bin/true 2>/dev/null || docker create "${IMG_BUILD_WASIX}" true)
    docker cp "${cid}:/out/edgeparse-wasix.wasm" "${BUILD_DIR}/edgeparse-wasix.wasm"
    docker rm "${cid}" > /dev/null
    log "WASIX binary: ${BUILD_DIR}/edgeparse-wasix.wasm ($(du -h "${BUILD_DIR}/edgeparse-wasix.wasm" | cut -f1))"
}

build_base() {
    log "Building shared runner base image..."
    docker build \
        -f "${SCRIPT_DIR}/Dockerfile.runner.base" \
        -t "${IMG_BASE}" \
        "${REPO_ROOT}"
}

build_runner() {
    local runtime="$1"
    local img
    img=$(image_name_for "${runtime}")

    # Ensure prerequisites based on runtime type
    case "${runtime}" in
        wasmtime|wasmer|wasmedge|wamr)
            [ -f "${BUILD_DIR}/edgeparse.wasm" ] || build_wasm
            image_exists "${IMG_BASE}" || build_base
            ;;
        wasix)
            # WASIX runner uses standard WASI binary (backward compat)
            [ -f "${BUILD_DIR}/edgeparse.wasm" ] || build_wasm
            image_exists "${IMG_BASE}" || build_base
            ;;
        riscv-qemu)
            [ -f "${BUILD_DIR}/edgeparse-riscv64" ] || build_riscv
            ;;
        spike|libriscv|rvvm|ckb-vm)
            [ -f "${BUILD_DIR}/edgeparse-riscv64-static" ] || build_riscv
            ;;
    esac

    log "Building ${runtime} runner image..."
    docker build \
        -f "${SCRIPT_DIR}/Dockerfile.runner.${runtime}" \
        -t "${img}" \
        "${REPO_ROOT}"
}

cmd_build() {
    local target="${1:-all}"
    case "${target}" in
        all)
            build_wasm
            build_wasix
            build_riscv
            build_base
            for rt in ${ALL_RUNNERS}; do
                build_runner "${rt}"
            done
            ;;
        wasm)       build_wasm ;;
        wasix-bin)  build_wasix ;;
        riscv)      build_riscv ;;
        base)       build_base ;;
        wasmtime|wasmer|wasmedge|wamr|wasix|riscv-qemu|spike|libriscv|rvvm|ckb-vm)
            build_runner "${target}"
            ;;
        *)
            err "Unknown build target: ${target}"
            echo "Valid: all wasm wasix-bin riscv base wasmtime wasmer wasmedge wamr wasix riscv-qemu spike libriscv rvvm ckb-vm"
            exit 1
            ;;
    esac
}

# ── Test Commands ─────────────────────────────────────────────────────────────
run_test() {
    local runtime="$1"
    local img
    img=$(image_name_for "${runtime}")

    if ! image_exists "${img}"; then
        warn "Image ${img} not found, building first..."
        build_runner "${runtime}"
    fi

    log "Testing with ${runtime}..."
    echo ""
    docker run --rm \
        --name "${PREFIX}-test-${runtime}" \
        "${img}" \
        "${runtime}"
    local exit_code=$?
    echo ""
    return ${exit_code}
}

cmd_test() {
    local target="${1:-all}"
    local failed=0
    local passed=0
    local targets

    if [ "${target}" = "all" ]; then
        targets="${ALL_RUNNERS}"
    else
        targets="${target}"
    fi

    for rt in ${targets}; do
        if run_test "${rt}"; then
            passed=$((passed + 1))
        else
            failed=$((failed + 1))
            warn "${rt}: SOME TESTS FAILED"
        fi
    done

    echo ""
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}  Overall: ${GREEN}${passed} runtimes passed${RESET}, ${RED}${failed} runtimes failed${RESET}"
    echo -e "${BOLD}═══════════════════════════════════════════════════════════${RESET}"

    [ "${failed}" -eq 0 ]
}

# ── Status ────────────────────────────────────────────────────────────────────
cmd_status() {
    echo -e "${BOLD}EdgeParse WASM/RISC-V Test Infrastructure${RESET}\n"

    echo -e "${BOLD}Docker Images:${RESET}"
    for img in ${ALL_IMAGES}; do
        if image_exists "${img}"; then
            local size
            size=$(docker image inspect "${img}" --format='{{.Size}}' 2>/dev/null | numfmt --to=iec 2>/dev/null || echo "?")
            echo -e "  ${GREEN}●${RESET} ${img} (${size})"
        else
            echo -e "  ${DIM}○ ${img} (not built)${RESET}"
        fi
    done

    echo -e "\n${BOLD}Build Artifacts:${RESET}"
    if [ -f "${BUILD_DIR}/edgeparse.wasm" ]; then
        echo -e "  ${GREEN}●${RESET} edgeparse.wasm ($(du -h "${BUILD_DIR}/edgeparse.wasm" | cut -f1))"
    else
        echo -e "  ${DIM}○ edgeparse.wasm (not built)${RESET}"
    fi
    if [ -f "${BUILD_DIR}/edgeparse-riscv64" ]; then
        echo -e "  ${GREEN}●${RESET} edgeparse-riscv64 ($(du -h "${BUILD_DIR}/edgeparse-riscv64" | cut -f1))"
    else
        echo -e "  ${DIM}○ edgeparse-riscv64 (not built)${RESET}"
    fi

    echo -e "\n${BOLD}Running Containers:${RESET}"
    local running
    running=$(docker ps --filter "name=${PREFIX}-test-" --format '{{.Names}} ({{.Status}})' 2>/dev/null)
    if [ -n "${running}" ]; then
        echo "${running}" | while read -r line; do
            echo -e "  ${CYAN}▶${RESET} ${line}"
        done
    else
        echo -e "  ${DIM}(none)${RESET}"
    fi
}

# ── Log ───────────────────────────────────────────────────────────────────────
cmd_log() {
    local runtime="${1:?Usage: wasm-test.sh log <runtime>}"
    local container="${PREFIX}-test-${runtime}"
    docker logs "${container}" 2>&1 || err "No logs for ${container}"
}

# ── Cleanup ───────────────────────────────────────────────────────────────────
cmd_rmi() {
    local target="${1:-all}"
    if [ "${target}" = "all" ]; then
        log "Removing all edgeparse test images..."
        for img in ${ALL_IMAGES}; do
            docker rmi -f "${img}" 2>/dev/null && dim "  removed ${img}" || true
        done
    else
        local img
        img=$(image_name_for "${target}")
        docker rmi -f "${img}" 2>/dev/null && dim "  removed ${img}" || warn "Image ${img} not found"
    fi
}

cmd_clean() {
    log "Cleaning build artifacts and images..."

    # Stop and remove any running test containers
    docker ps -q --filter "name=${PREFIX}-test-" 2>/dev/null | xargs -r docker rm -f 2>/dev/null || true

    # Remove images
    cmd_rmi all

    # Remove build directory
    rm -rf "${BUILD_DIR}"

    log "Clean complete."
}

# ── Run (interactive shell in container) ──────────────────────────────────────
cmd_run() {
    local runtime="${1:?Usage: wasm-test.sh run <runtime>}"
    local img
    img=$(image_name_for "${runtime}")

    if ! image_exists "${img}"; then
        warn "Image ${img} not found, building first..."
        build_runner "${runtime}"
    fi

    log "Launching interactive shell in ${runtime} container..."
    docker run --rm -it \
        --name "${PREFIX}-run-${runtime}" \
        --entrypoint /bin/bash \
        "${img}"
}

# ── Help ──────────────────────────────────────────────────────────────────────
cmd_help() {
    cat <<'BANNER'
    ┌─────────────────────────────────────────────────────────┐
    │  EdgeParse WASM/RISC-V Integration Test Manager         │
    │  Greg's AI coding buddy reporting for duty! o7          │
    └─────────────────────────────────────────────────────────┘
BANNER
    echo ""
    echo -e "${BOLD}Usage:${RESET} $(basename "$0") <command> [target]"
    echo ""
    echo -e "${BOLD}Commands:${RESET}"
    echo "  build   [target]  Build Docker images and binaries"
    echo "  test    [target]  Run integration tests"
    echo "  status            Show image/container status"
    echo "  run     <runtime> Launch interactive shell in container"
    echo "  log     <runtime> Show container logs"
    echo "  rmi     [target]  Remove Docker images"
    echo "  clean             Remove everything (images + artifacts)"
    echo "  help              This help screen"
    echo ""
    echo -e "${BOLD}Build Targets:${RESET}"
    echo "  all         Build everything (default)"
    echo "  wasm        Build WASM binary only (wasm32-wasip1)"
    echo "  riscv       Build RISC-V binary only (riscv64gc)"
    echo "  base        Build shared runner base image"
    echo "  wasmtime    Build Wasmtime runner"
    echo "  wasmer      Build Wasmer runner"
    echo "  wasmedge    Build WasmEdge runner"
    echo "  wamr        Build WAMR/iwasm runner"
    echo "  riscv-qemu  Build RISC-V QEMU runner"
    echo ""
    echo -e "${BOLD}Test Targets:${RESET}"
    echo "  all         Test all runtimes (default)"
    echo "  wasmtime    Test with Wasmtime"
    echo "  wasmer      Test with Wasmer"
    echo "  wasmedge    Test with WasmEdge"
    echo "  wamr        Test with WAMR/iwasm"
    echo "  riscv-qemu  Test with RISC-V QEMU"
    echo ""
    echo -e "${BOLD}Environment:${RESET}"
    echo "  EDGEPARSE_PREFIX  Docker artifact prefix (default: edgeparse)"
    echo ""
    echo -e "${BOLD}Examples:${RESET}"
    echo "  $(basename "$0") build all        # build everything"
    echo "  $(basename "$0") test wasmtime    # test with wasmtime only"
    echo "  $(basename "$0") test all         # test all runtimes"
    echo "  $(basename "$0") run wasmer       # interactive shell in wasmer"
    echo "  $(basename "$0") status           # check what's built"
    echo "  $(basename "$0") clean            # nuke everything"
}

# ── Main Dispatch ─────────────────────────────────────────────────────────────
main() {
    local cmd="${1:-help}"
    shift || true

    case "${cmd}" in
        build)   cmd_build "$@" ;;
        test)    cmd_test "$@" ;;
        status)  cmd_status ;;
        run)     cmd_run "$@" ;;
        log)     cmd_log "$@" ;;
        rmi)     cmd_rmi "$@" ;;
        clean)   cmd_clean ;;
        help|-h|--help)
            cmd_help
            ;;
        *)
            err "Unknown command: ${cmd}"
            cmd_help
            exit 1
            ;;
    esac
}

main "$@"
