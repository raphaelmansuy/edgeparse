# 07 — CI/CD Publishing Pipeline

This document describes the release path for every EdgeParse distribution asset:
Rust crates, Python wheels, Node.js packages, the WASM SDK, CLI archives,
Homebrew, and Docker images.

---

## Overview

Publishing is driven by six GitHub Actions workflows, all triggered by pushing a
semantic version tag:

```bash
git tag v0.2.2
git push origin v0.2.2
```

```text
vX.Y.Z tag
  ├─ release-rust.yml    -> crates.io         (pdf-cos, edgeparse-core, edgeparse-cli)
  ├─ release-python.yml  -> PyPI              (edgeparse wheels + sdist)
  ├─ release-node.yml    -> npm               (edgeparse + 5 platform packages)
  ├─ release-wasm.yml    -> GitHub Releases   (edgeparse-wasm tarball)
  ├─ release-cli.yml     -> GitHub Releases   (5 CLI archives) + Homebrew tap
  └─ release-docker.yml  -> GHCR + Docker Hub (linux/amd64, linux/arm64)
```

Shared verification happens in `ci.yml` on pushes and pull requests:

- Rust build, test, clippy, and fmt checks
- Python wheel build and SDK tests
- Node.js addon build and SDK tests
- WASM target compilation check
- Docker image smoke build
- Cargo audit and cargo-deny

---

## Published Artifacts

| Channel | Artifact | Registry / Location |
|---------|----------|---------------------|
| crates.io | `pdf-cos` | https://crates.io/crates/pdf-cos |
| crates.io | `edgeparse-core` | https://crates.io/crates/edgeparse-core |
| crates.io | `edgeparse-cli` | https://crates.io/crates/edgeparse-cli |
| PyPI | `edgeparse` | https://pypi.org/project/edgeparse/ |
| npm | `edgeparse` | https://www.npmjs.com/package/edgeparse |
| npm | `edgeparse-darwin-arm64` | https://www.npmjs.com/package/edgeparse-darwin-arm64 |
| npm | `edgeparse-darwin-x64` | https://www.npmjs.com/package/edgeparse-darwin-x64 |
| npm | `edgeparse-linux-arm64-gnu` | https://www.npmjs.com/package/edgeparse-linux-arm64-gnu |
| npm | `edgeparse-linux-x64-gnu` | https://www.npmjs.com/package/edgeparse-linux-x64-gnu |
| npm | `edgeparse-win32-x64-msvc` | https://www.npmjs.com/package/edgeparse-win32-x64-msvc |
| GitHub Releases | CLI archives + WASM npm tarball | https://github.com/raphaelmansuy/edgeparse/releases |
| Homebrew | `raphaelmansuy/edgeparse` tap | https://github.com/raphaelmansuy/homebrew-edgeparse |
| GHCR | `ghcr.io/raphaelmansuy/edgeparse` | https://github.com/raphaelmansuy/edgeparse/pkgs/container/edgeparse |
| Docker Hub | `rmansuy/edgeparse` | https://hub.docker.com/r/rmansuy/edgeparse |

### CLI Release Targets

Each GitHub Release includes:

| Archive | Platform |
|---------|----------|
| `edgeparse-X.Y.Z-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `edgeparse-X.Y.Z-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `edgeparse-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 (`glibc >= 2.17`) |
| `edgeparse-X.Y.Z-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64 (`glibc >= 2.17`) |
| `edgeparse-X.Y.Z-x86_64-pc-windows-gnu.zip` | Windows x86_64 |

### Python Wheel Coverage

| Platform | Python versions |
|----------|-----------------|
| Linux x86_64 | cp310, cp311, cp312, cp313 |
| Linux ARM64 | cp310, cp311, cp312, cp313 |
| macOS Intel | cp310, cp311, cp312, cp313 |
| macOS Apple Silicon | cp310, cp311, cp312, cp313 |
| Windows x86_64 | cp310, cp311, cp312, cp313 |
| Source distribution | sdist |

---

## Secrets and Environments

### Repository secrets

| Secret | Used by | Purpose |
|--------|---------|---------|
| `CARGO_REGISTRY_TOKEN` | `release-rust.yml` | Publish crates to crates.io |
| `NPM_TOKEN` | `release-node.yml` | Publish Node.js packages to npm |
| `DOCKERHUB_TOKEN` | `release-docker.yml` | Push Docker images to Docker Hub |
| `HOMEBREW_TAP_TOKEN` | `release-cli.yml` | Push `edgeparse.rb` to the Homebrew tap |

### GitHub environments

| Environment | Used by | Notes |
|-------------|---------|-------|
| `npm` | `release-node.yml`, `release-wasm.yml` | Optional protection rules for npm release jobs |
| `pypi` | `release-python.yml` | Required for PyPI Trusted Publishing |

### External setup

- crates.io: create a token with `publish-new` and `publish-update`
- npm: use a Classic Automation token so the main package and platform packages
  can publish from CI
- PyPI: configure Trusted Publishing for `release-python.yml` in environment
  `pypi`
- Docker Hub: create a read/write access token for account `rmansuy`
- Homebrew tap: create a PAT with `contents: write` on
  `raphaelmansuy/homebrew-edgeparse`

---

## Release Checklist

1. Ensure the working tree is clean.
2. Update versioned manifests:
   - root `Cargo.toml`
   - `crates/edgeparse-cli/Cargo.toml`
   - `sdks/node/package.json`
   - `sdks/node/package-lock.json`
   - `sdks/node/npm/*/package.json`
   - `crates/edgeparse-wasm/pkg/package.json`
3. Update release notes:
   - `CHANGELOG.md`
   - `README.md`
   - this document when the release surface changes
4. Run local release-prep verification.
5. Push the release branch and open a PR.
6. Merge the PR.
7. Tag the merge commit and push the tag.
8. Watch all six release workflows complete.

---

## Local Verification

Run the checks that correspond to shipped assets before tagging:

```bash
cargo test
cargo check -p edgeparse-wasm --target wasm32-unknown-unknown
docker build -f docker/Dockerfile .

cd sdks/node
npm ci
cargo build --manifest-path ../../crates/edgeparse-node/Cargo.toml --release
# Copy the host-specific addon into the matching local package before testing.
# Example shown here for Apple Silicon:
cp ../../target/release/libedgeparse_node.dylib npm/darwin-arm64/edgeparse-node.darwin-arm64.node
npm install --no-save file:./npm/darwin-arm64
npm run build:ts
npm test
cd ../..

cd benchmark
uv run python run.py --check-regression
cd ..
```

Optional dry runs:

```bash
make publish-rust-dry
make publish-python-dry
make publish-node-dry
make publish-wasm-dry
make publish-cli-dry
make publish-brew-dry
```

---

## Tag Release Flow

```bash
# 1. Commit and push the release-prep branch
git add -A
git commit -m "chore: prepare 0.2.2 release"
git push origin <branch>

# 2. Open and merge the PR
gh pr create --base main --head <branch>
gh pr merge <pr-number> --merge --delete-branch=false

# 3. Tag the merge commit on main
git checkout main
git pull --ff-only origin main
git tag v0.2.2
git push origin v0.2.2
```

The tag must match `v[0-9]+.[0-9]+.[0-9]+`. The Rust and WASM release
workflows verify that the tag version matches the workspace version and fail
fast on mismatches.

---

## Workflow Reference

### `ci.yml`

| Job | Coverage |
|-----|----------|
| `rust` | `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt --check` |
| `python` | Build wheel with maturin and run SDK tests |
| `node` | Build native addon, compile TypeScript, run SDK tests |
| `wasm` | `cargo check -p edgeparse-wasm --target wasm32-unknown-unknown` |
| `docker` | `docker build -f docker/Dockerfile .` |
| `security` | `cargo audit` and `cargo deny check` |

### `release-rust.yml`

- Verifies tag/version consistency
- Publishes `pdf-cos`, then `edgeparse-core`, then `edgeparse-cli`
- Waits for crates.io index propagation between dependent crates
- Creates or updates the GitHub Release notes

### `release-python.yml`

- Builds wheel artifacts for Linux, macOS, and Windows
- Builds an sdist
- Publishes to PyPI via OIDC Trusted Publishing

### `release-node.yml`

- Builds native `.node` binaries for five targets
- Syncs the package version from the tag
- Publishes five platform packages and the main `edgeparse` package
- Treats "already published" as idempotent rather than fatal

### `release-wasm.yml`

- Builds the browser-targeted WASM package with `wasm-pack`
- Syncs the npm package version from the tag
- npm publication is currently disabled
- Uploads the generated npm tarball to the GitHub Release

### `release-cli.yml`

- Builds five CLI archives
- Uploads them to the GitHub Release
- Regenerates and pushes the Homebrew formula

### `release-docker.yml`

- Builds and pushes a multi-arch container image
- Publishes to GHCR and Docker Hub
- Generates provenance and SBOM metadata
- Runs a Trivy vulnerability scan

---

## Local Publish Helpers

The Makefile mirrors the registry release flow for manual publishing:

```bash
make publish-rust
make publish-python
make publish-node
make publish-wasm
make publish-cli
make publish-brew
make publish-all
```

`make publish-all` covers crates, Python, Node.js, the WASM SDK, CLI archives,
and Homebrew. Docker publishing remains CI-driven through `release-docker.yml`.

---

## Troubleshooting

### crates.io rejects a publish because the version already exists

Crates.io versions are immutable. Bump the version and retag.

### npm publish fails on platform packages

Use a Classic Automation token for `NPM_TOKEN`. Granular tokens often miss one
or more package names and produce `E403 Forbidden`.

### PyPI publish fails with `invalid-publisher`

The PyPI Trusted Publisher entry must match:

- project: `edgeparse`
- owner: `raphaelmansuy`
- repository: `edgeparse`
- workflow: `release-python.yml`
- environment: `pypi`

### The GitHub Release exists but some assets are missing

Re-run the specific workflow. `release-cli.yml` and `release-wasm.yml` upload
assets with `--clobber`, so the release can be repaired without retagging.

### Local Linux cross-builds fail on macOS

Use `cargo-zigbuild` plus `zig`. The release workflows already do this for the
Linux ARM64 and Windows targets.
