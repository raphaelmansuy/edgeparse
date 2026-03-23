# 07 — CI/CD Publishing Pipeline

This document describes how automated publishing works for all edgeparse distribution targets, what secrets must be configured, and how to trigger a release. Read this before cutting your first release.

---

## Overview

All publishing is driven by four independent GitHub Actions workflows, each triggered by pushing a version tag:

```
git tag v0.2.0 && git push --tags
    │
    ├──► release-rust.yml    ──► crates.io  (pdf-cos, edgeparse-core, edgeparse-cli)
    ├──► release-python.yml  ──► PyPI       (edgeparse wheels × 5 platforms + sdist)
    ├──► release-node.yml    ──► npm        (edgeparse + 5 platform packages)
    └──► release-docker.yml  ──► GHCR + Docker Hub  (linux/amd64, linux/arm64)
```

A shared `ci.yml` runs on every push and pull request covering Rust build + test, Python wheel build + test, and Node.js build + test.

---

## Published Artifacts

| Registry | Package | URL |
|----------|---------|-----|
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
| Docker Hub | `raphaelmansuy/edgeparse` | https://hub.docker.com/r/raphaelmansuy/edgeparse |
| GHCR | `ghcr.io/raphaelmansuy/edgeparse` | https://github.com/raphaelmansuy/edgeparse/pkgs/container/edgeparse |

---

## Required Secrets and Environments

### GitHub Repository Secrets

| Secret name | What it is | Where to create it |
|-------------|-----------|-------------------|
| `CARGO_REGISTRY_TOKEN` | crates.io API token with `publish-new` + `publish-update` scope | [crates.io → Account Settings → API Tokens](https://crates.io/settings/tokens) |
| `NPM_TOKEN` | npm Granular Access Token with read+write to all `edgeparse*` packages | [npmjs.com → Account → Access Tokens](https://www.npmjs.com/settings/~/tokens) |
| `DOCKERHUB_TOKEN` | Docker Hub personal access token (read+write) | [hub.docker.com → Account Settings → Security](https://hub.docker.com/settings/security) |

> **PyPI does not require a secret** — it uses GitHub OIDC Trusted Publishing. See the PyPI section below.

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

**Required for:** `release-rust.yml`

1. Sign in to [crates.io](https://crates.io) as the account that owns the packages.
2. Go to **Account Settings → API Tokens → New Token**.
   - Token name: `edgeparse-github-actions`
   - Scope: `publish-new` + `publish-update` (do **not** grant `yank`)
3. Copy the token — shown only once.
4. Add to GitHub: secret name `CARGO_REGISTRY_TOKEN`.

**Verify locally:**
```bash
CARGO_REGISTRY_TOKEN=<token> cargo publish -p edgeparse-core --dry-run
```

**Publish order matters.** `pdf-cos` is a local dependency of `edgeparse-core`, and `edgeparse-core` is a dependency of `edgeparse-cli`. The workflow publishes in order with 30-second waits for the crates.io index to propagate:
1. `pdf-cos` (internal lopdf fork)
2. `edgeparse-core`
3. `edgeparse-cli`

**Reserved file gotcha.** If `pdf-cos` was extracted from a `.crate` archive, it may contain `.cargo_vcs_info.json`. This file is reserved by cargo and will block publish. The `exclude` field in `crates/pdf-cos/Cargo.toml` handles this automatically:
```toml
exclude = [".cargo_vcs_info.json", ".cargo-ok", "Cargo.toml.orig"]
```

---

### 2. PyPI — Trusted Publisher (OIDC, no token)

**Required for:** `release-python.yml`

PyPI Trusted Publishing authenticates via GitHub's OIDC — no long-lived token needed.

**Steps (one-time, before first release):**

1. Sign in to [pypi.org](https://pypi.org).
2. Go to [Manage → Publishing](https://pypi.org/manage/account/publishing/) → **Add a new pending publisher**.
   - PyPI Project Name: `edgeparse`
   - GitHub Owner: `raphaelmansuy`
   - Repository name: `edgeparse`
   - Workflow filename: `release-python.yml`
   - Environment name: `pypi`
3. In GitHub, create the `pypi` **Environment** (Settings → Environments → New environment). No secrets required in the environment.

The `release-python.yml` workflow uses `pypa/gh-action-pypi-publish@release/v1` with `id-token: write` permission, which exchanges the GitHub OIDC token for a temporary PyPI upload credential.

**Verify locally (dry-run):**
```bash
cd sdks/python
pip install maturin twine
maturin build --release --out dist/
twine check dist/*.whl
```

---

### 3. npm — Access Token

**Required for:** `release-node.yml`

The npm package is `edgeparse` (unscoped, no organization required). Platform-specific packages (`edgeparse-darwin-arm64`, etc.) are also unscoped.

**Steps (one-time):**

1. Sign in to [npmjs.com](https://www.npmjs.com) as the publisher account.
2. Go to **Account → Access Tokens → Generate New Token → Granular Access Token**.
   - Token name: `edgeparse-github-actions`
   - Expiration: 365 days (set a calendar reminder to rotate!)
   - Packages and scopes: **Read and write** — all packages belonging to this account (or select specific package names)
   - Organizations: None needed (unscoped packages)
3. Copy the token.
4. Add to GitHub: secret name `NPM_TOKEN`.

> **Token rotation:** npm Granular Access Tokens expire. Create a new token before the old one expires, update the GitHub secret, then revoke the old token. Check the expiry date in [npmjs.com → Access Tokens](https://www.npmjs.com/settings/~/tokens).

**Verify locally:**
```bash
# Check that token works
NODE_AUTH_TOKEN=<token> npm whoami
# should print the npm username

# Dry-run pack
cd sdks/node && npm pack --dry-run
```

**Publishing order for Node.js:**

1. First, all 5 platform-specific packages are published (CI downloads the `.node` artifacts built on each platform runner).
2. Then the main `edgeparse` package is published (it lists the platform packages as `optionalDependencies`).

This order is important: if the main package is published first, npm users will get warnings about missing optional dependencies.

---

### 4. Docker Hub — Access Token

**Required for:** `release-docker.yml`

1. Sign in to [hub.docker.com](https://hub.docker.com) as `raphaelmansuy`.
2. Create a public repository: **Repositories → Create Repository** → `raphaelmansuy/edgeparse`, Public.
3. Create an Access Token: **Account Settings → Security → Access Tokens → New Access Token**
   - Description: `edgeparse-github-actions`
   - Access: Read & Write
4. Add to GitHub: secret name `DOCKERHUB_TOKEN`.

The Docker Hub username is hardcoded as `raphaelmansuy` in the workflow — no username secret is needed. GHCR authentication uses `GITHUB_TOKEN` (auto-provisioned).

---

## How to Cut a Release

```bash
# 1. Bump the version in the workspace Cargo.toml
#    [workspace.package]
#    version = "0.2.0"

# 2. Also bump the Node.js package versions
cd sdks/node
# Update package.json and npm/*/package.json to new version

# 3. Also bump the Python package version in sdks/python/pyproject.toml

# 4. Commit and push
git add -A
git commit -m "chore: bump version to 0.2.0"
git push origin main

# 5. Tag and push — this triggers all four release workflows
git tag v0.2.0
git push origin v0.2.0
```

The tag format must match `v[0-9]+.[0-9]+.[0-9]+` (no pre-release suffixes like `-rc1` — the workflows only match that pattern). The Rust workflow verifies that the tag version matches `edgeparse-core`'s version in Cargo.toml and fails fast if they diverge.

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
| Publish edgeparse-cli | CLI binary |
| GitHub Release | Created with generated release notes |

### `release-python.yml` — PyPI

**Triggers:** `v*.*.*` tag push

| Job | Detail |
|-----|--------|
| `build-wheels` | Matrix: 5 platforms (ubuntu x86_64, ubuntu arm64, macos x86_64, macos arm64, windows x86_64). Uses `maturin-action@v1` with `sccache`. |
| `build-sdist` | Source distribution via `maturin sdist` |
| `publish-pypi` | Downloads all wheel artifacts, publishes via `pypa/gh-action-pypi-publish` using OIDC. Gated by `environment: pypi`. |

### `release-node.yml` — npm

**Triggers:** `v*.*.*` tag push

| Job | Detail |
|-----|--------|
| `build-native` | Matrix: 5 platforms. Builds the `.node` NAPI-RS addon via `cargo build --release`. For aarch64 Linux, uses `cross` for cross-compilation. |
| `publish-npm` | Downloads all 5 `.node` artifacts → syncs version in all package.json files → `npm run build:ts` → publishes 5 platform packages → publishes main `edgeparse` package. Gated by `environment: npm`. |

### `release-docker.yml` — Container registries

**Triggers:** `v*.*.*` tag push

Builds a multi-arch image (`linux/amd64` + `linux/arm64`) using `docker buildx` and pushes to both Docker Hub (`raphaelmansuy/edgeparse`) and GHCR (`ghcr.io/raphaelmansuy/edgeparse`).

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

All `path = "..."` dependencies published to crates.io must also include `version = "x.y.z"`. Example:
```toml
edgeparse-core = { path = "../edgeparse-core", version = "0.1.0" }
```

### npm: E401 Unauthorized

The `NPM_TOKEN` secret is expired or invalid. Generate a new Granular Access Token at [npmjs.com/settings/~/tokens](https://www.npmjs.com/settings/~/tokens) and update the GitHub secret.

### npm: "Scope not found"

The package uses the unscoped name `edgeparse` — no npm organization is needed. If you see scope errors, verify `package.json` has `"name": "edgeparse"` (not `"@someorg/edgeparse"`).

### PyPI: "File already exists"

Like crates.io, PyPI does not allow overwriting a version. Bump the version in `sdks/python/pyproject.toml`.

### PyPI OIDC: "Token request failed"

The Trusted Publisher configuration on PyPI must exactly match the GitHub owner, repo name, workflow filename, and environment name. Recheck all four fields at [pypi.org/manage/account/publishing](https://pypi.org/manage/account/publishing/).

### npm: WebAuthn 2FA during manual publish

If publishing manually with `npm publish` and your account has WebAuthn 2FA enabled, npm will display a browser auth URL:
```
Authenticate your account at:
https://www.npmjs.com/auth/cli/<uuid>
Press ENTER to open in the browser...
```
Open this URL in a browser, complete the WebAuthn challenge, then press Enter in the terminal to complete the publish.

For automated CI, use a Granular Access Token (not an interactive session) — tokens bypass 2FA for CI operations.

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Unscoped `edgeparse` npm package instead of `@edgeparse/pdf` | No npm organization required; avoids the friction of creating and maintaining an org. All platform packages (`edgeparse-darwin-arm64`, etc.) are also unscoped. |
| PyPI OIDC instead of API token | No long-lived secret; OIDC tokens expire in minutes and are scoped to each workflow run. |
| `pdf-cos` published separately before `edgeparse-core` | `edgeparse-core` depends on `pdf-cos = "0.39.0"` from crates.io. Publishing `pdf-cos` first ensures the index is available when `edgeparse-core` is validated. |
| 30-second wait steps between crates | crates.io index updates are eventually consistent. 30 seconds is sufficient for the dependency to appear in the index before the dependent crate is validated. |
| `environment: npm` on publish-npm job | Enables GitHub Environment protection rules (optional reviewers, deployment history). Mirrors the `environment: pypi` pattern used for Python. |
