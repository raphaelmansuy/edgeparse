# 07 — CI/CD Publishing Pipeline

This document describes how automated publishing works for all edgeparse distribution targets, what secrets must be configured, and how to trigger a release. Read this before cutting your first release.

---

## Overview

Publishing is driven by five independent GitHub Actions workflows, each triggered by pushing a version tag:

```
git tag v0.2.0 && git push --tags
    │
    ├──► release-rust.yml    ──► crates.io  (pdf-cos, edgeparse-core, edgeparse-cli)
    ├──► release-python.yml  ──► PyPI       (edgeparse wheels × 9 platform-Python combos + sdist)
    ├──► release-node.yml    ──► npm        (edgeparse + 5 platform packages)
    ├──► release-cli.yml     ──► GitHub Release (5 arch binaries) + Homebrew tap
    └──► release-docker.yml  ──► GHCR + Docker Hub  (linux/amd64, linux/arm64)
```

A shared `ci.yml` runs on every push and pull request covering Rust build + test, Python wheel build + test, and Node.js build + test.

You can also publish everything locally without CI:

```bash
# Publish all: crates.io + PyPI + npm + GitHub Release + Homebrew tap
make publish-all
```

---

## Published Artifacts

| Registry | Package / Location | URL |
|----------|-------------------|-----|
| crates.io | `pdf-cos` | https://crates.io/crates/pdf-cos |
| crates.io | `edgeparse-core` | https://crates.io/crates/edgeparse-core |
| crates.io | `edgeparse-cli` | https://crates.io/crates/edgeparse-cli |
| PyPI | `edgeparse` | https://pypi.org/project/edgeparse/ |
| npm | `edgeparse` | https://www.npmjs.com/package/edgeparse |
| npm | `edgeparse-darwin-arm64` | https://www.npmjs.com/package/edgeparse-darwin-arm64 |
| npm | `edgeparse-darwin-x64` | https://www.npmjs.com/package/edgeparse-darwin-x64 |
| npm | `edgeparse-linux-x64-gnu` | https://www.npmjs.com/package/edgeparse-linux-x64-gnu |
| npm | `edgeparse-linux-arm64-gnu` | https://www.npmjs.com/package/edgeparse-linux-arm64-gnu |
| npm | `edgeparse-win32-x64-msvc` | https://www.npmjs.com/package/edgeparse-win32-x64-msvc |
| GitHub Releases | CLI binaries (5 archs) | https://github.com/raphaelmansuy/edgeparse/releases |
| Homebrew tap | `raphaelmansuy/edgeparse` | https://github.com/raphaelmansuy/homebrew-edgeparse |
| Docker Hub | `rmansuy/edgeparse` | https://hub.docker.com/r/rmansuy/edgeparse |
| GHCR | `ghcr.io/raphaelmansuy/edgeparse` | https://github.com/raphaelmansuy/edgeparse/pkgs/container/edgeparse |

### CLI Binary Targets (GitHub Release)

Each GitHub Release includes ready-to-run binaries for:

| Archive | Platform |
|---------|----------|
| `edgeparse-X.Y.Z-aarch64-apple-darwin.tar.gz` | macOS Apple Silicon |
| `edgeparse-X.Y.Z-x86_64-apple-darwin.tar.gz` | macOS Intel |
| `edgeparse-X.Y.Z-x86_64-unknown-linux-gnu.tar.gz` | Linux x86_64 (glibc ≥ 2.17) |
| `edgeparse-X.Y.Z-aarch64-unknown-linux-gnu.tar.gz` | Linux ARM64 (glibc ≥ 2.17) |
| `edgeparse-X.Y.Z-x86_64-pc-windows-gnu.zip` | Windows x86_64 |

### Python Wheel Coverage (PyPI)

| Platform | Python versions |
|----------|----------------|
| Linux x86_64 (manylinux2014) | cp310, cp311, cp312, cp313 |
| Linux ARM64 (manylinux2014) | cp310, cp311, cp312, cp313 |
| macOS Apple Silicon | cp310, cp311, cp312, cp313 |
| macOS Intel | cp310, cp311, cp312, cp313 |
| Windows x86_64 | cp312 |
| Source distribution | — |

### Homebrew Installation

```bash
brew tap raphaelmansuy/edgeparse
brew install edgeparse
```

---

## Required Secrets and Environments

### GitHub Repository Secrets

| Secret name | What it is | Where to create it |
|-------------|-----------|-------------------|
| `CARGO_REGISTRY_TOKEN` | crates.io API token with `publish-new` + `publish-update` scope | [crates.io → Account Settings → API Tokens](https://crates.io/settings/tokens) |
| `NPM_TOKEN` | npm Granular Access Token with read+write to all `edgeparse*` packages | [npmjs.com → Account → Access Tokens](https://www.npmjs.com/settings/~/tokens) |
| `DOCKERHUB_TOKEN` | Docker Hub personal access token (read+write) | [hub.docker.com → Account Settings → Security](https://hub.docker.com/settings/security) |
| `HOMEBREW_TAP_TOKEN` | GitHub PAT with `contents: write` access to `raphaelmansuy/homebrew-edgeparse` | [github.com → Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens) |

> **PyPI does not require a secret** — it uses GitHub OIDC Trusted Publishing. See the PyPI section below.
>
> **Local publishing** uses `PYPI_PASSWORD` (a PyPI API token) in your shell environment, not OIDC. See the _Local publishing_ section.

Set secrets at: **GitHub repo → Settings → Secrets and variables → Actions → New repository secret**

### GitHub Environments

Two environments gate publish jobs with optional protection rules:

| Environment | Used by | Secrets scoped here |
|-------------|---------|-------------------|
| `pypi` | `release-python.yml` (publish-pypi job) | None — uses OIDC |
| `npm` | `release-node.yml` (publish-npm job) | `NPM_TOKEN` (optional, can also be repo-level) |

Create environments at: **GitHub repo → Settings → Environments → New environment**

---

## One-Time Setup Procedures

### 1. crates.io — API Token

**Required for:** `release-rust.yml` and `make publish-rust`

1. Sign in to [crates.io](https://crates.io) as the account that owns the packages.
2. Go to **Account Settings → API Tokens → New Token**.
   - Token name: `edgeparse-github-actions`
   - Scope: `publish-new` + `publish-update` (do **not** grant `yank`)
3. Copy the token — shown only once.
4. Add to GitHub: secret name `CARGO_REGISTRY_TOKEN`.

For **local publishing**, export the token:

```bash
export CARGO_REGISTRY_TOKEN=<token>
make publish-rust
```

**Verify locally:**

```bash
CARGO_REGISTRY_TOKEN=<token> cargo publish -p edgeparse-core --dry-run
```

**Publish order matters.** `pdf-cos` is a local dependency of `edgeparse-core`, and `edgeparse-core` is a dependency of `edgeparse-cli`. The workflow publishes in order with 30-second waits for the crates.io index to propagate:
1. `pdf-cos` (internal lopdf fork)
2. `edgeparse-core`
3. `edgeparse-cli`

**Reserved file gotcha.** If `pdf-cos` was extracted from a `.crate` archive, it may contain `.cargo_vcs_info.json`. The `exclude` field in `crates/pdf-cos/Cargo.toml` handles this:
```toml
exclude = [".cargo_vcs_info.json", ".cargo-ok", "Cargo.toml.orig"]
```

---

### 2. PyPI — Trusted Publisher (OIDC, no token)

**Required for:** `release-python.yml` — CI publishes via OIDC (no long-lived secret).

**Steps (one-time, before first release):**

1. Sign in to [pypi.org](https://pypi.org).
2. Go to [Manage → Publishing](https://pypi.org/manage/account/publishing/) → **Add a new pending publisher**.
   - PyPI Project Name: `edgeparse`
   - GitHub Owner: `raphaelmansuy`
   - Repository name: `edgeparse`
   - Workflow filename: `release-python.yml`
   - Environment name: `pypi`
3. In GitHub, create the `pypi` **Environment** (Settings → Environments → New environment). No secrets required.

The `release-python.yml` workflow uses `pypa/gh-action-pypi-publish@release/v1` with `id-token: write` permission.

**Local publishing** uses a PyPI API token instead of OIDC:

```bash
# Create a PyPI API token at https://pypi.org/manage/account/token/
export PYPI_PASSWORD=pypi-<your-api-token>
make publish-python
```

The Makefile uses `--username __token__ --password "$PYPI_PASSWORD"` — the literal string `__token__` is required when authenticating with an API token.

**Verify wheels locally (dry-run):**

```bash
make publish-python-dry
```

---

### 3. npm — Access Token

**Required for:** `release-node.yml`

The npm package is `edgeparse` (unscoped). Platform-specific packages (`edgeparse-darwin-arm64`, etc.) are also unscoped.

**Steps (one-time):**

1. Sign in to [npmjs.com](https://www.npmjs.com) as the publisher account.
2. Go to **Account → Access Tokens → Generate New Token → Granular Access Token**.
   - Token name: `edgeparse-github-actions`
   - Expiration: 365 days (set a calendar reminder to rotate!)
   - Packages and scopes: **Read and write** — all packages belonging to this account
3. Copy the token.
4. Add to GitHub: secret name `NPM_TOKEN`.

> **Token rotation:** npm Granular Access Tokens expire. Rotate before expiry at [npmjs.com → Access Tokens](https://www.npmjs.com/settings/~/tokens).

**Verify locally:**

```bash
NODE_AUTH_TOKEN=<token> npm whoami
cd sdks/node && npm pack --dry-run
# Or via Makefile:
make publish-node-dry
```

---

### 4. GitHub CLI Binary Release + Homebrew Tap

**Required for:** `release-cli.yml`

`release-cli.yml` builds CLI binaries for all 5 target platforms and attaches them to the GitHub Release. It then generates and pushes the Homebrew formula to the tap repository.

**One-time setup — Homebrew tap repository:**

The formula tap lives at **https://github.com/raphaelmansuy/homebrew-edgeparse** (already created).

**Create `HOMEBREW_TAP_TOKEN`:**

1. Go to [github.com → Settings → Developer settings → Personal access tokens → Fine-grained tokens](https://github.com/settings/tokens?type=beta).
2. Create a new token:
   - Token name: `homebrew-tap-push`
   - Repository access: **Only select repositories** → `raphaelmansuy/homebrew-edgeparse`
   - Permissions: **Contents → Read and write**
3. Add to GitHub repo secrets: name `HOMEBREW_TAP_TOKEN`.

**Local publish (no CI needed):**

```bash
# 1. Build CLI binaries for all archs and attach to GitHub Release
make publish-cli

# 2. Generate Homebrew formula and push to tap
make publish-brew
```

Prerequisites: `cargo-zigbuild` + `zig` for Linux/Windows cross-compilation:
```bash
cargo install cargo-zigbuild
brew install zig
```

---

### 5. Docker Hub — Access Token

**Required for:** `release-docker.yml`

1. Sign in to [hub.docker.com](https://hub.docker.com) as `rmansuy`.
2. Create a public repository: **Repositories → Create Repository** → `raphaelmansuy/edgeparse`, Public.
3. Create an Access Token: **Account Settings → Security → Access Tokens → New Access Token**
   - Description: `edgeparse-github-actions`
   - Access: Read & Write
4. Add to GitHub: secret name `DOCKERHUB_TOKEN`.

The Docker Hub username is `rmansuy`. GHCR uses `GITHUB_TOKEN` automatically.

---

## How to Cut a Release

### Option A — Automated (tag push triggers CI)

```bash
# 1. Bump the version in the workspace Cargo.toml
#    [workspace.package]
#    version = "0.2.0"

# 2. Bump Node.js package versions
cd sdks/node
# Update package.json and npm/*/package.json to new version

# 3. Bump Python version
# Update sdks/python/pyproject.toml

# 4. Commit and push
git add -A
git commit -m "chore: bump version to 0.2.0"
git push origin main

# 5. Tag and push — triggers all five release workflows
git tag v0.2.0
git push origin v0.2.0
```

The tag format must match `v[0-9]+.[0-9]+.[0-9]+`. The Rust workflow verifies the tag version matches `edgeparse-core`'s Cargo.toml version and fails fast if they diverge.

### Option B — Local publishing (Makefile, no CI)

```bash
# Set credentials in environment
export CARGO_REGISTRY_TOKEN=<crates-io-token>
export PYPI_PASSWORD=pypi-<api-token>        # note: --username __token__ is used automatically
export NPM_TOKEN=<npm-granular-access-token>

# Full publish: crates + PyPI + npm + CLI binaries + Homebrew
make publish-all

# Or target-by-target:
make publish-rust      # → crates.io
make publish-python    # → PyPI
make publish-node      # → npm
make publish-cli       # → GitHub Release (binaries)
make publish-brew      # → Homebrew tap (run after publish-cli)
```

Dry-run any target first:
```bash
make publish-rust-dry
make publish-python-dry
make publish-node-dry
make publish-cli-dry
make publish-brew-dry
```

---

## Workflow Reference

### `ci.yml` — Continuous Integration

**Triggers:** Every push to `main`, every PR targeting `main`

| Job | What it does |
|-----|-------------|
| `rust` | `cargo build + test + clippy + fmt` on ubuntu, macos, windows |
| `python` | `maturin develop --release + pytest tests/` |
| `node` | `npm ci + cargo build + npm run build + npm test` |
| `security` | `cargo audit + cargo deny check` |

### `release-rust.yml` — crates.io

**Triggers:** `v*.*.*` tag push

| Step | Detail |
|------|--------|
| Version check | Tag version must match `edgeparse-core` Cargo.toml version |
| CHANGELOG | `git-cliff` generates release notes from conventional commits |
| Publish pdf-cos | Internal lopdf fork — must be published before edgeparse-core |
| Wait 30s | crates.io index propagation delay |
| Publish edgeparse-core | Core library |
| Wait 30s | Index propagation |
| Publish edgeparse-cli | CLI crate (binary) |
| GitHub Release | Created with generated release notes |

### `release-python.yml` — PyPI

**Triggers:** `v*.*.*` tag push

| Job | Detail |
|-----|--------|
| `build-wheels` | Matrix: 5 platforms. Linux builds all cp310–cp313 automatically via manylinux2014. macOS installs Python 3.10–3.13 and passes `-i python3.10 python3.11 python3.12 python3.13` to maturin. Windows builds cp312 only. Uses `maturin-action@v1` with `sccache`. |
| `build-sdist` | Source distribution via `maturin sdist` |
| `publish-pypi` | Downloads all wheel artifacts, publishes via `pypa/gh-action-pypi-publish` using OIDC. Gated by `environment: pypi`. |

### `release-node.yml` — npm

**Triggers:** `v*.*.*` tag push

| Job | Detail |
|-----|--------|
| `build-native` | Matrix: 5 platforms. macOS and Windows: native `cargo build`. Linux ARM64: `cargo-zigbuild` with glibc 2.17 floor (no Docker needed). |
| `publish-npm` | Downloads 5 `.node` artifacts → syncs version in all package.json → `npm run build:ts` → publishes 5 platform packages → publishes main `edgeparse` package. Gated by `environment: npm`. |

### `release-cli.yml` — GitHub Release binaries + Homebrew

**Triggers:** `v*.*.*` tag push

| Job | Detail |
|-----|--------|
| `build-cli` | Matrix: 5 platforms. macOS: native `cargo build`. Linux ARM64 + Windows: `cargo-zigbuild` targeting glibc 2.17. Each job uploads its artifact. |
| `attach-release` | Downloads all 5 artifacts, creates GitHub Release if not already present (release-rust.yml may have created it first), uploads tarballs/zips with `--clobber`. |
| `homebrew` | Downloads CLI artifacts, runs `scripts/gen-formula.sh` to compute SHA256s locally, commits and pushes updated formula to `raphaelmansuy/homebrew-edgeparse`. Requires `HOMEBREW_TAP_TOKEN` secret. |

### `release-docker.yml` — Container registries

**Triggers:** `v*.*.*` tag push

Builds a multi-arch image (`linux/amd64` + `linux/arm64`) using `docker buildx` and pushes to both Docker Hub (`raphaelmansuy/edgeparse`) and GHCR (`ghcr.io/raphaelmansuy/edgeparse`). Runs a Trivy HIGH/CRITICAL vulnerability scan after push.

---

## Troubleshooting

### crates.io: "crate version already exists"

You cannot overwrite a version on crates.io. Bump the version and re-tag.

### crates.io: "reserved file name .cargo_vcs_info.json"

Ensure `crates/pdf-cos/Cargo.toml` has the `exclude` field:
```toml
exclude = [".cargo_vcs_info.json", ".cargo-ok", "Cargo.toml.orig"]
```

### crates.io: "dependency X does not specify a version"

All `path = "..."` dependencies published to crates.io must also include `version = "x.y.z"`:
```toml
edgeparse-core = { path = "../edgeparse-core", version = "0.1.0" }
```

### npm: E401 Unauthorized

The `NPM_TOKEN` secret is expired or invalid. Generate a new Granular Access Token at [npmjs.com/settings/~/tokens](https://www.npmjs.com/settings/~/tokens) and update the GitHub secret.

### npm: "Scope not found"

The package uses the unscoped name `edgeparse`. If you see scope errors, verify `package.json` has `"name": "edgeparse"` (not `"@someorg/edgeparse"`).

### PyPI: "File already exists"

Like crates.io, PyPI does not allow overwriting a version. Bump the version in `sdks/python/pyproject.toml`.

### PyPI OIDC: "Token request failed"

The Trusted Publisher configuration on PyPI must exactly match the GitHub owner, repo name, workflow filename (`release-python.yml`), and environment name (`pypi`). Recheck all four fields at [pypi.org/manage/account/publishing](https://pypi.org/manage/account/publishing/).

### PyPI local: "403 Forbidden" with API token

Use `--username __token__` (the literal string `__token__`) when authenticating with an API token. The Makefile handles this automatically via `publish-python`. If running `twine` manually:
```bash
twine upload dist/*.whl --username __token__ --password "$PYPI_PASSWORD"
```

### Linux CLI / Node.js build fails (cross-compilation)

`cross v0.2.5` fails on macOS ARM64 when targeting Linux (it tries to install a Linux-runnable toolchain). Use `cargo-zigbuild` instead — it requires no Docker and works on macOS ARM64:

```bash
cargo install cargo-zigbuild
brew install zig
cargo zigbuild --release --target aarch64-unknown-linux-gnu.2.17 -p edgeparse-cli
```

### GitHub Release: CLI binary not attached

`release-cli.yml` runs in parallel with `release-rust.yml`. If it runs first, it creates the Release with placeholder notes; `release-rust.yml` will then update the release body with CHANGELOG content. Both upload with `--clobber`, so re-running either workflow is safe.

### Homebrew formula: wrong SHA256

The `release-cli.yml` `homebrew` job generates SHA256 from the locally-built artifacts before they are uploaded. If you push the formula manually with `make publish-brew`, run `make publish-cli` first so the artifacts are in `target/release-dist/`.

### npm: WebAuthn 2FA during manual publish

For automated CI, use a Granular Access Token — tokens bypass 2FA. For manual publishing, open the auth URL in a browser, complete the WebAuthn challenge, then press Enter.

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| `cargo-zigbuild` instead of `cross` for Linux ARM64 cross-compilation | `cross v0.2.5` cannot build Linux targets on a macOS ARM64 host (the runner or local machine). `cargo-zigbuild` uses zig as a linker, requires no Docker daemon, and works identically on macOS and Linux CI runners. |
| Unscoped `edgeparse` npm package | No npm organization required; avoids org overhead. All platform packages (`edgeparse-darwin-arm64`, etc.) are also unscoped. |
| PyPI OIDC for CI, API token for local Makefile | OIDC tokens expire in minutes and are scoped per run — ideal for CI. The Makefile uses a long-lived API token for convenient local publishing. |
| `pdf-cos` published separately before `edgeparse-core` | `edgeparse-core` depends on `pdf-cos` from crates.io. Publishing first ensures the index is available when the dependent crate is validated. |
| 30-second wait steps between crates | crates.io index updates are eventually consistent. 30 seconds is sufficient for the dependency to appear before the dependent crate is validated. |
| `environment: npm` / `environment: pypi` on publish jobs | Enables GitHub Environment protection rules (optional reviewers, deployment history). |
| Separate `release-cli.yml` for binaries + Homebrew | CLI binary publishing is unrelated to crates.io and has its own lifecycle. Decoupling avoids blocking the Rust publish on cross-compilation. The Homebrew formula depends on the CLI artifacts, so it lives in the same workflow. |
| `HOMEBREW_TAP_TOKEN` instead of `GITHUB_TOKEN` for tap push | `GITHUB_TOKEN` only has write access to the current repository. A dedicated PAT scoped to `raphaelmansuy/homebrew-edgeparse` is required to push the formula. |
