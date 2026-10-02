# Changelog

All notable changes to EdgeParse are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) and
this project adheres to [Semantic Versioning](https://semver.org/).

---

## [0.3.2] — 2026-10-02

### Fixed
- **Type 3 / Skia font widths** — `/Widths` and `/FontBBox` are normalized through `/FontMatrix` into per-mille text space, restoring word boundaries on Chrome/Skia PDFs (previously ~90% of spaces were lost)
- **Type 3 font metadata** — fall back to `FontDescriptor/FontName` and prefer `/FontWeight` over StemV so variable-font Skia faces are not all marked bold
- **Header/footer swallowing** — tighter margin zones, max edge depth, and no absorption of images/figures into running headers (top-of-page charts stay in the body)
- **Raster OCR routing** — classify charts/photos/UI before table OCR so bar charts are not turned into junk markdown tables; shared `ImageRegionClassifier` + optional `ocr_budget_ms`
- **WASM JSON** — `convert_to_string("json")` / `finishAll` emit the same compact legacy schema as the CLI (was multi-MB pretty internal model)
- **Content-stream `/ActualText`** — BDC/EMC spans honor ISO 32000-1 §14.9.4; ActualText replaces glyph/ToUnicode text for extraction
- **TJ word gaps** — large negative TJ adjustments insert explicit spaces via named `SpaceThreshold` (TeX/InDesign)
- **CID / Identity-H** — layered Unicode fallback (ToUnicode → Differences/AGL → embedded TTF cmap); ASCII-range CID fallback without WinAnsi mojibake
- **Encrypted PDFs** — Standard Security Handler decrypt via `--password` / `ProcessingConfig.password` (pdf-cos revisions 2–6)
- **OCG visibility** — catalog `/OCProperties` default ON/OFF wired to `TextChunk.ocg_visible` (content filter drops hidden layers)

### Added
- `pdf::font_type3` / `pdf::font_style` — DRY Type 3 metric normalization and shared name/weight helpers
- `pdf::font_type3_ocr` — CharProc → Unicode OCR cache seam (`GlyphOcrProvider`)
- `pdf::image_region` — `ImageRegionClassifier` trait, heuristic classifier, `OcrBudget`, `TableStructureModel` + `ClassifyingRegionRouter` for TableFormer/SLANet plug-in
- `pdf::image_codecs` — DCT/Flate/CCITT decode for OCR; optional `codecs-jbig2` / `codecs-jpx` features
- `pdf::ocg` — Optional Content Group default visibility from catalog
- `pdf::pdf_string` — UTF-16BE / PDFDocEncoding string decode for ActualText
- Auto-enable structure tree when `/StructTreeRoot` is present; PDF 2.0 tags (`Title`, `Aside`, `FENote`, `Strong`, `Em`, `Hn`)
- `benchmark/scripts/assess_pdf.py` — shared native/WASM quality harness for Skia-style PDFs
- `crates/edgeparse-wasm/scripts/smoke-native-wasm.mjs` — native↔WASM markdown parity smoke test
- **OCR on/off** — `ParseOptions.enableOcr` / WASM `rasterTableOcr` / Python `raster_table_ocr` / Node `rasterTableOcr`; demo toolbar OCR toggle re-parses without recreating the client
- **Docs** — OCR Models guide (tiers, OPFS/Cache, Hugging Face + CSP, self-host / offline); npm optionalDeps synced to release version

### Changed
- Raster table recovery is gated through one helper that respects `ocr_budget_ms`
- Site / Web SDK docs: OPFS+Cache (not IndexedDB); CSP `connect-src` includes `huggingface.co`
- Benchmark MHS floor lowered to **0.47** (caption demotion / heading precision); harness board refreshed for 0.3.2

---

## [0.3.1] — 2026-10-02

### Fixed
- **Skia/Chrome Pattern fills** — tiling Pattern paints resolve to `ImageChunk`s; PatternType-2 shadings and soft-circle AA (thousands of 1×1 filled rects) no longer explode into Line spam that broke `finishAll` JSON
- **Filled paths vs strokes** — fills emit Lines only for thin geometric strips (table rules); equidimensional fills are decoration
- **OCR routing** — `RasterCandidateKind::TextBlocks` for full-bleed / image-only / Pattern-fill pages; table OCR reserved for ruled lattices
- **Overpaint text** — identical glyphs redrawn at the same origin (faux-bold) are deduped before line grouping
- **Heading false positives** — body mode uses character-mass weighting; near-body bold diagram labels are not outline nodes when a size hierarchy exists
- **Title / checkbox markdown** — Info.Title only when corroborated; Unicode/`[x]`/`[ ]` task items (not Latin `X`)

### Added
- Pattern-image fixture + WASM/Playwright e2e coverage for form-style Pattern fills
- `ImagePaintSource` on image chunks; quality warning for raster-heavy pages

---

## [0.3.0] — 2026-10-01

### Added
- **Two-phase `ParseSession`** (WASM) — open → OCR candidates → finish, so browsers can OCR image tables without blocking the UI thread
- **`@edgeparse/web`** — observable Web SDK with model manager (IndexedDB + SHA-256), consent hooks, progress events, and WebGPU/WASM OCR workers
- **Real PP-OCRv6 pipeline** in the Web SDK — detect → recognize → CTC decode, models pinned with sha256 from Hugging Face `snowfluke/ppu-paddle-ocr-models` (tiny/small/medium + dictionaries)
- **In-memory raster OCR** for Image XObjects (no Poppler/pdftoppm required on the WASM path)
- **Host OCR callback** on wasm32 for PP-OCR / custom engines, with ocrs fallback when enabled
- **odl-bench SDK gate** — `npm run gate:wasm-sdk` (TEDS ≥ 0.776, overall ≥ 0.854) and score snapshots under `odl-bench/reports/`
- **Demo** — accessible OCR consent dialog, quality badge (`full` / `degraded`), live model download progress, retry wiring
- **Docs** — Web SDK quick-start and API reference; both benchmark boards labeled (official odl-bench + EdgeParse harness)

### Changed
- WASM package build is `--target web` for the browser demo/site; Node bench uses separate `pkg-node/` (gitignored)
- Site copy: born-digital PDFs need no ML stack; optional in-browser OCR for image tables
- CI: `deploy-site.yml` builds wasm + `@edgeparse/web` before the demo; `release-wasm.yml` fails closed on placeholder model hashes

### Fixed
- Stale committed Node-target `pkg/` lacking `ParseSession` (rebuild required for browser)
- OCR stub that returned empty word lists — replaced with real PP-OCR

---

## [0.2.5] — 2026-04-14

### Fixed
- **Homepage URL updated to `https://www.edgeparse.com`** across all published packages (`edgeparse-wasm`, Node.js SDK platform packages, Python SDK) and crate metadata — previously some packages showed `edgeparse.elitizon.com` or stale GitHub URLs
- **`release-wasm.yml` metadata** — `pkg.homepage` corrected to `https://www.edgeparse.com` so all future WASM npm publishes carry the right URL

### Changed
- Workspace `Cargo.toml` now declares `homepage = "https://www.edgeparse.com"` inherited by all crates
- `sdks/node/package.json` and all five platform `package.json` files updated to hompage `https://www.edgeparse.com`
- `sdks/python/pyproject.toml` `Homepage` updated to `https://www.edgeparse.com`

---

## [0.2.4] — 2026-04-13

### Added
- **WASM npm publication** — `edgeparse-wasm` is now published to the npm public registry on every tagged release; jsDelivr and unpkg CDNs become available automatically
- **GitHub Packages secondary registry** — `@raphaelmansuy/edgeparse-wasm` is published to `npm.pkg.github.com` alongside the npm release, providing a GitHub-native install path for enterprise users
- **CDN quick-start** — `docs/09-wasm-sdk.md` now includes copy-pasteable `<script type="module">` examples for jsDelivr and unpkg (no build tool required)
- **Framework quick-starts** — Vite + React, Next.js App Router, Webpack 5, and Service Worker PWA examples added to the WASM SDK docs
- **`exports` field in `pkg/package.json`** — adds a proper ESM exports map for bundler interop

### Changed
- `release-wasm.yml` now publishes to npm (primary) and GitHub Packages (secondary) instead of skipping publication; both steps treat "already published" as non-fatal
- `release-wasm.yml` requires `packages: write` permission (for GitHub Packages) and existing `contents: write` (for GitHub Releases)
- `pkg/package.json` carries full metadata (keywords, exports, publishConfig) so it is ready to publish without CI patching the file from scratch
- `docs/09-wasm-sdk.md` consolidated installation and distribution section with all four channels (npm, jsDelivr, unpkg, GitHub Packages)
- `docs/07-cicd-publishing.md` updated with WASM npm rows in the artifacts table, `NPM_TOKEN` scope note, and step-by-step token setup instructions

### Fixed
- `INPUT_TAG_NAME` env variable now written to `$GITHUB_ENV` so downstream steps in `release-wasm.yml` can reference `${{ env.TAG_NAME }}` without re-reading inputs

---

## [0.2.3] — 2026-03-28

### Added
- Hybrid OCR false-positive guard: photo/matrix heuristics now prevent non-table images from being classified as tables in raster OCR mode
- Tie-aware benchmark ranking: identical scores across engines now share a rank rather than producing misleading orderings

### Changed
- Heading detector robustness improvements for edge-case font-size clustering
- Table cluster detector refined to reduce false positives on dense text regions
- Benchmark HTML and terminal reporters include PBF in verdict summary; hybrid engines show correct display names
- Site benchmark figures updated to 2026-03-28 snapshot: EdgeParse 0.7811 overall, 0.007 s/doc (83× faster than Docling, 49× faster than PyMuPDF4LLM, 2× faster than OpenDataLoader, TEDS 73% better than OpenDataLoader)

### Fixed
- Raster table OCR no longer triggers on photo-heavy or matrix-style pages that lack tabular structure
- Speed and quality claims on the website corrected to match current measured benchmark figures

---

## [0.2.2] — 2026-03-26

### Added
- Separate benchmark groups for `non-ocr` and `hybrid` runs so published comparisons can be reported more cleanly
- Shared benchmark snapshot data for the site so landing and docs stay aligned from one source

### Changed
- Bumped the workspace and published SDK manifests to `0.2.2`
- Rebuilt the checked-in WASM package used by the demo and site so browser deployments use the latest parser bundle
- Updated the site/demo release path to ship the refreshed WASM bundle and current benchmark documentation

### Fixed
- Disabled WASM npm publication while keeping the release tarball attached to GitHub Releases
- Corrected benchmark runner/docs grouping so hybrid engines no longer appear under the non-OCR bucket

---

## [0.2.1] — 2026-03-26

### Added
- Dedicated `release-wasm.yml` workflow to publish `edgeparse-wasm` on tagged releases and attach the npm tarball to the GitHub Release
- CI coverage for the WASM target and Docker image smoke builds so every shipped artifact is validated before release
- Release-channel documentation in the README covering crates, SDKs, CLI archives, Homebrew, and container images

### Changed
- Bumped the workspace and published SDK manifests to `0.2.1`
- Local release helpers now publish `pdf-cos` before `edgeparse-core`, matching the crates.io CI workflow
- `make publish-all` now includes the WASM SDK release path
- README benchmark results updated to the latest 200-document `opendataloader.org` comparison, where EdgeParse leads the published field on every reported metric

### Fixed
- Removed stale release documentation that still described five workflows and partial manual workarounds for older releases
- Updated install guidance to reflect Linux `glibc >= 2.17` compatibility for release binaries

---

## [0.2.0] — 2026-03-24

### Added
- **WASM SDK** (`edgeparse-wasm`): WebAssembly bindings for browser and edge-runtime deployments
- **WASM demo** (`demo/`): Interactive in-browser PDF extraction demo using the WASM SDK
- **Enterprise page** on the website (`/enterprise`) with pricing and contact CTA
- **Contact page** on the website (`/contact`) routed from all enterprise CTAs
- **Demo link** in the site header and landing page hero section
- **Elitizon partnership links** across the site
- Social-card meta tags, Open Graph images, and sitemap improvements for SEO

### Changed
- Cross-compilation for Windows CLI binary now uses `cargo zigbuild` correctly (no spurious glibc suffix on Windows targets)
- Trivy security-scan action pinned to `v0.35.0` (was `@master`) and uses the correct image tag (strips `v` prefix)
- `edgeparse-core` internal dependency version constraint updated to `0.2.0`

### Fixed
- CLI release workflow: Windows cross-compiled binary no longer received a Linux glibc suffix (`.2.17`) which could cause `cargo zigbuild` errors
- Docker release workflow: Trivy scan now pulls the correct image tag (`0.2.0`, not `v0.2.0`)

---

## [0.1.1] — 2026-03-23

### Fixed
- Zero Clippy warnings across all crates
- Corrected `.gitignore` to exclude build artifacts cleanly

### Added
- Comprehensive Rust doc comments on public API surface
- Step-by-step tutorials for CLI, Python SDK, Node.js SDK, Rust library, and output formats
- CI/CD publishing guide (`docs/07-cicd-publishing.md`)

### Changed
- Bumped version to `0.1.1` in all crates and SDK manifests

---

## [0.1.0] — 2026-03-22

### Added
- **Core extraction engine** (`edgeparse-core`): Rust-native PDF-to-structured-data pipeline — no ML, no Java, no GPU
- **Python SDK** (`edgeparse`): PyO3-based bindings, available on PyPI
- **Node.js SDK** (`edgeparse`): NAPI-RS bindings, available on npm
- **CLI binary** (`edgeparse-cli`): Zero-dependency binary for all major platforms
- **Rust library** (`edgeparse-core`): First-class crate published to crates.io
- Reading-order reconstruction for multi-column and sidebar layouts
- Ruling-line and borderless table detection with cell-span merging
- Heading and paragraph classification
- AI safety filters (PII scrubbing, content flags)
- Tagged PDF support (PDF/UA accessibility structure)
- Output formats: JSON (full schema), Markdown, HTML, plain text
- Benchmark suite comparing EdgeParse against Docling, Marker, pymupdf4llm, MinerU, MarkItDown, and LiteParse
- Docker image for containerised deployment
- High-level technical documentation: overview, architecture, pipeline, data model, extraction, output formats, SDK integration
- GitHub Actions CI workflows for Rust, Python, Node.js, and Docker
- Renamed Node.js package from `@edgeparse/pdf` → `edgeparse`

---

## Links

- [GitHub Releases](https://github.com/raphaelmansuy/edgeparse/releases)
- [crates.io — edgeparse-core](https://crates.io/crates/edgeparse-core)
- [PyPI — edgeparse](https://pypi.org/project/edgeparse/)
- [npm — edgeparse](https://www.npmjs.com/package/edgeparse)
