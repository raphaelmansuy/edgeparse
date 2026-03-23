# Task Log: 2025-07-12 — SDK Publishing Infrastructure (continued)

## Actions
- Resumed from Phase 3 (Node.js SDK) where `index.ts` was just created
- Created `sdks/node/src/cli.ts` — Node.js CLI entry point with argparse
- Created 5 platform sub-packages under `sdks/node/npm/` (linux-x64-gnu, linux-arm64-gnu, darwin-x64, darwin-arm64, win32-x64-msvc)
- Created `sdks/node/tests/convert.test.ts` — vitest tests with graceful skip when native addon unavailable
- Created `docker/Dockerfile` — multi-stage build (rust:1.85-slim-bookworm → distroless/cc-debian12:nonroot)
- Created `docker/Dockerfile.dev` — development Dockerfile
- Created `docker/.dockerignore`
- Created 5 GitHub Actions workflows: `ci.yml`, `release-rust.yml`, `release-python.yml`, `release-node.yml`, `release-docker.yml`
- Fixed CI workflow to use `cargo build` (default members) instead of `cargo build --workspace` (which breaks on cdylib crates)
- Updated `.gitignore` (added Node.js, .so, .node, removed /mission/ exclusion)
- Updated `mission/plan.md` with all checkboxes marked
- Committed 50 files on `feat/sdk-and-publish` branch

## Decisions
- Used `cargo build` (default-members only) in CI since Python/Node cdylib crates need maturin/napi-cli
- Node.js tests use `skipIf(!edgeparse)` to gracefully handle missing native addon (CI can't build .node in TS tests)

## Verification Results
- `cargo build` (default members): OK
- `cargo test -p edgeparse-core --lib`: 407 passed
- `cargo check -p edgeparse-node`: OK  
- Python tests: 13/13 passed
- YAML syntax validation: 5/5 OK

## Next Steps
- Push branch and create PR
- Docker build verification (requires Docker runtime)
- Set up repository secrets for publishing (CARGO_REGISTRY_TOKEN, NPM_TOKEN, DOCKERHUB_TOKEN)
- Configure PyPI Trusted Publishing (OIDC)
