# EdgeParse WASM/RISC-V Integration Tests

> Greg's AI coding buddy: Testing edgeparse across every WASM runtime
> and CPU architecture we can get our hands on — all inside Docker,
> because reproducibility is not optional.

## Architecture

```
tests/wasm-runtimes/
├── wasm-test.sh                    # Management script (build/test/status/clean)
├── run-tests.sh                    # Test runner (executes inside containers)
│
├── Dockerfile.build.wasm           # Builds edgeparse → wasm32-wasip1
├── Dockerfile.build.wasix          # Builds edgeparse → wasm32-wasmer-wasi (WASIX)
├── Dockerfile.build.riscv          # Cross-compiles edgeparse → riscv64gc (dynamic + static)
│
├── Dockerfile.runner.base          # Shared Ubuntu base for WASM runners
├── Dockerfile.runner.wasmtime      # Wasmtime (Bytecode Alliance reference)
├── Dockerfile.runner.wasmer        # Wasmer (WASIX superpowers)
├── Dockerfile.runner.wasmedge      # WasmEdge (CNCF, cloud-native)
├── Dockerfile.runner.wamr          # WAMR/iwasm (embedded, 6 exec modes)
├── Dockerfile.runner.wasix         # WASIX on Wasmer (POSIX extensions)
│
├── Dockerfile.runner.riscv-qemu    # RISC-V QEMU user-mode emulation
├── Dockerfile.runner.spike         # Spike — official RISC-V ISA reference sim
├── Dockerfile.runner.libriscv      # libriscv — fastest RISC-V sandbox (3ns calls)
├── Dockerfile.runner.rvvm          # RVVM — tracing JIT (experimental)
├── Dockerfile.runner.ckb-vm        # CKB-VM — blockchain RISC-V VM (experimental)
│
└── .build/                         # Build artifacts (gitignored)
    ├── edgeparse.wasm              # WASI Preview 1 binary
    ├── edgeparse-riscv64           # RISC-V ELF (dynamic)
    └── edgeparse-riscv64-static    # RISC-V ELF (static, for VM sandboxes)
```

## Quick Start

```bash
# Build everything and run all tests
./tests/wasm-runtimes/wasm-test.sh build all
./tests/wasm-runtimes/wasm-test.sh test all

# Or test a single runtime
./tests/wasm-runtimes/wasm-test.sh build wasmtime
./tests/wasm-runtimes/wasm-test.sh test wasmtime

# Check what's built
./tests/wasm-runtimes/wasm-test.sh status

# Interactive debugging shell inside a runtime container
./tests/wasm-runtimes/wasm-test.sh run wasmer

# Clean up everything
./tests/wasm-runtimes/wasm-test.sh clean
```

## How It Works

### Build Phase

1. **`Dockerfile.build.wasm`** compiles edgeparse-cli for `wasm32-wasip1` using
   `--no-default-features` (disables rayon/image/zip — those need native threads).
   The result is a ~2-4 MB `.wasm` binary that any WASI Preview 1 runtime can execute.

2. **`Dockerfile.build.riscv`** cross-compiles for `riscv64gc-unknown-linux-gnu` using
   the Debian cross-toolchain (`gcc-riscv64-linux-gnu`). The result is a standard
   ELF binary targeting the RV64GC ISA.

Both build Dockerfiles use the same layer-caching strategy as the production
`docker/Dockerfile`: copy Cargo manifests first, warm the dep cache with a dummy
build, then copy real source.

### Runner Phase

All WASM runner Dockerfiles inherit from `edgeparse-wasi-base` (Ubuntu 24.04 with
curl, ca-certificates, test fixtures, and the test script). Each adds its runtime:

| Runtime    | Install Method       | Target               | Notes                          |
|------------|----------------------|----------------------|--------------------------------|
| Wasmtime   | Official installer   | WASI p1 + p2         | Bytecode Alliance reference    |
| Wasmer     | Official installer   | WASI p1 + WASIX      | Only runtime with WASIX        |
| WasmEdge   | Release tarball      | WASI p1 (+ P2 WIP)   | CNCF, AOT support              |
| WAMR       | Built from source    | WASI p1              | ~100KB, 6 execution modes      |
| WASIX      | Wasmer (WASIX mode)  | WASI p1 (compat)     | Tests POSIX runtime compat     |
| QEMU       | apt package          | riscv64gc ELF        | User-mode emulation            |
| Spike      | Built from source    | riscv64gc ELF        | Official RISC-V ISA reference  |
| libriscv   | Built from source    | riscv64gc ELF        | Fastest sandbox (~3ns calls)   |
| RVVM       | Built from source    | riscv64gc ELF        | Tracing JIT (experimental)     |
| CKB-VM     | cargo install        | riscv64gc ELF        | Blockchain VM (experimental)   |

### Test Phase

`run-tests.sh` runs inside each container and exercises:

1. `--help` flag works
2. `--version` returns semver
3. PDF → JSON conversion
4. PDF → Markdown conversion
5. PDF → Text conversion (with content sanity check)
6. PDF → HTML conversion
7. Error handling for non-existent files

Each runtime has slightly different CLI syntax for preopening directories;
`run-tests.sh` normalizes this via a `build_run_cmd()` function.

## Docker Image Naming

All images are prefixed with `edgeparse` (configurable via `EDGEPARSE_PREFIX`):

* `edgeparse-wasi-build` — WASM build environment
* `edgeparse-riscv-build` — RISC-V cross-compilation environment
* `edgeparse-wasi-base` — Shared runner base
* `edgeparse-wasi-wasmtime` — Wasmtime runner
* `edgeparse-wasi-wasmer` — Wasmer runner
* `edgeparse-wasi-wasmedge` — WasmEdge runner
* `edgeparse-wasi-wamr` — WAMR runner
* `edgeparse-riscv-qemu` — RISC-V QEMU runner

## CI/CD Integration

The management script is CI-friendly — no interactive prompts, proper exit codes,
and the `test` command returns non-zero if any runtime fails. Add to GitHub Actions:

```yaml
- name: WASM Runtime Integration Tests
  run: |
    ./tests/wasm-runtimes/wasm-test.sh build all
    ./tests/wasm-runtimes/wasm-test.sh test all
```

## Extending

To add a new WASM runtime:

1. Create `Dockerfile.runner.<name>` — inherit `FROM edgeparse-wasi-base`
2. Install the runtime and add it to `PATH`
3. `COPY tests/wasm-runtimes/.build/edgeparse.wasm /test/edgeparse.wasm`
4. Set `CMD ["<name>"]`
5. Add a case to `build_run_cmd()` in `run-tests.sh`
6. Add `<name>` to `ALL_RUNTIMES` in `wasm-test.sh`

## WASI vs WASIX vs Native

| Feature       | WASI (wasip1) | WASIX       | Native CLI |
|---------------|---------------|-------------|------------|
| File I/O      | Preopened     | Full POSIX  | Full       |
| Parallelism   | No (no rayon) | Threads     | Full rayon |
| Image extract | No            | Possible    | Full       |
| PDF parsing   | Full          | Full        | Full       |
| Binary size   | ~2-4 MB       | ~4-6 MB     | ~15 MB     |

The WASI build disables `native` features (rayon, image, zip) since WASI Preview 1
doesn't support threads. Core PDF parsing and text extraction work identically.
