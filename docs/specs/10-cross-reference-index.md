# 10 — Cross-Reference Index

> Master index linking every concept, algorithm, constant, data type, and CLI
> option across all specification documents.

---

## A. Algorithms & Processing Stages

| Algorithm / Stage | Primary Spec | Related Specs |
|-------------------|-------------|---------------|
| **XY-Cut++ reading order** | [04 Stage 18](04-pdf-parsing-pipeline.md) | [02 §7.1](02-functional-spec.md), [03 §3.1](03-technical-architecture.md), [09 §5.5](09-rust-migration-guide.md) |
| **Content filtering** (11 sub-steps) | [04 Stage 2](04-pdf-parsing-pipeline.md) | [02 §5](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Cluster table detection** | [04 Stage 3](04-pdf-parsing-pipeline.md) | [02 §6.1](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Table border matching** | [04 Stage 4](04-pdf-parsing-pipeline.md) | [02 §6](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Line chunk removal** | [04 Stage 5](04-pdf-parsing-pipeline.md) | [03 §3.1](03-technical-architecture.md) |
| **Text line grouping** | [04 Stage 6](04-pdf-parsing-pipeline.md) | [03 §3.1](03-technical-architecture.md), [09 §4.2](09-rust-migration-guide.md) |
| **Same-line probability** | [04 §6.3](04-pdf-parsing-pipeline.md) | [03 §4.2](03-technical-architecture.md) |
| **Special table detection** (Korean) | [04 Stage 7](04-pdf-parsing-pipeline.md) | [02 §16.2](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Header/footer detection** | [04 Stage 8](04-pdf-parsing-pipeline.md) | [02 §9](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **List detection** (two-pass) | [04 Stage 9, 11](04-pdf-parsing-pipeline.md) | [02 §10](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Paragraph detection** (9 passes) | [04 Stage 10](04-pdf-parsing-pipeline.md) | [02 §13.2](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Heading detection** (probability) | [04 Stage 12](04-pdf-parsing-pipeline.md) | [02 §8](02-functional-spec.md), [03 §3.1](03-technical-architecture.md), [09 §6.5](09-rust-migration-guide.md) |
| **Caption linking** | [04 Stage 14](04-pdf-parsing-pipeline.md) | [02 §11](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Cross-page linking** | [04 Stage 15](04-pdf-parsing-pipeline.md) | [02 §6.3](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Heading level assignment** | [04 Stage 16](04-pdf-parsing-pipeline.md) | [02 §8.2](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| **Nesting level assignment** | [04 Stage 17](04-pdf-parsing-pipeline.md) | [03 §3.1](03-technical-architecture.md) |
| **PII sanitization** (10 regex) | [04 Stage 19](04-pdf-parsing-pipeline.md) | [02 §5.3](02-functional-spec.md), [09 §8.2](09-rust-migration-guide.md) |
| **Hidden text detection** (contrast) | [04 §2.2.9](04-pdf-parsing-pipeline.md) | [02 §5.1](02-functional-spec.md), [09 §3.4](09-rust-migration-guide.md) |
| **ModeWeightStatistics** | [04 Stage 12](04-pdf-parsing-pipeline.md) | [09 §6.5](09-rust-migration-guide.md) |
| **Triage processor** (6-signal) | [07 §2](07-hybrid-mode.md) | [03 §1.2](03-technical-architecture.md), [09 §5.7](09-rust-migration-guide.md) |
| **Schema transformation** (Docling→internal) | [07 §4](07-hybrid-mode.md) | [09 §5.7](09-rust-migration-guide.md) |
| **Tagged PDF walk** (structure tree) | [04 Stage 1](04-pdf-parsing-pipeline.md) | [02 §12](02-functional-spec.md), [03 §1.2](03-technical-architecture.md), [09 §6.6](09-rust-migration-guide.md) |
| **PDF content stream parsing** | [04 §1.2](04-pdf-parsing-pipeline.md) | [09 §3.3](09-rust-migration-guide.md) |
| **Font decoding** (CMap, ToUnicode) | [04 §1.2](04-pdf-parsing-pipeline.md) | [09 §3.3](09-rust-migration-guide.md) |

---

## B. Constants & Thresholds

### B.1 Content Filter Constants (Spec 04 §2)

| Constant | Value | Used In |
|----------|-------|---------|
| `MIN_TEXT_INTERSECTION_PERCENT` | 0.5 | [04 §2.2.1](04-pdf-parsing-pipeline.md) — duplicate text removal |
| `MAX_TOP_DECORATION_IMAGE_EPSILON` | 0.3 | [04 §2.2.2](04-pdf-parsing-pipeline.md) — decoration image detection |
| `MAX_BOTTOM_DECORATION_IMAGE_EPSILON` | 0.1 | [04 §2.2.2](04-pdf-parsing-pipeline.md) — decoration image detection |
| `MAX_LEFT_DECORATION_IMAGE_EPSILON` | 0.1 | [04 §2.2.2](04-pdf-parsing-pipeline.md) — decoration image detection |
| `MAX_RIGHT_DECORATION_IMAGE_EPSILON` | 1.5 | [04 §2.2.2](04-pdf-parsing-pipeline.md) — decoration image detection |
| `TEXT_MIN_HEIGHT` | 1.0 pt | [04 §2.2.3](04-pdf-parsing-pipeline.md) — tiny text filter |
| `NEIGHBORS_TEXT_CHUNKS_EPSILON` | 0.1 | [04 §2.2.5](04-pdf-parsing-pipeline.md) — close text merge |
| `MIN_CONTRAST_RATIO` | 1.2 | [04 §2.2.9](04-pdf-parsing-pipeline.md) — hidden text detection |

### B.2 Table Detection Constants (Spec 04 §3–4)

| Constant | Value | Used In |
|----------|-------|---------|
| `Y_DIFFERENCE_EPSILON` | 0.1 | [04 §3.3](04-pdf-parsing-pipeline.md) — cluster same-row tolerance |
| `X_DIFFERENCE_EPSILON` | 3.0 | [04 §3.3](04-pdf-parsing-pipeline.md) — cluster column gap |
| `MAX_NESTED_TABLE_DEPTH` | 10 | [04 §4.1](04-pdf-parsing-pipeline.md) — recursion limit |
| `NEIGHBOUR_TABLE_EPSILON` | 0.2 | [04 §4.3](04-pdf-parsing-pipeline.md) — cross-page width match |

### B.3 Text Line Constants (Spec 04 §6)

| Constant | Value | Used In |
|----------|-------|---------|
| `ONE_LINE_PROBABILITY` | 0.75 | [04 §6.1](04-pdf-parsing-pipeline.md), [03 §4.2](03-technical-architecture.md) — same-line threshold |
| `LIST_LABEL_HEIGHT_EPSILON` | (contextual) | [04 §6.2](04-pdf-parsing-pipeline.md) — line-art vs text height |
| `FONT_SIZE_EPSILON` | 0.01 | [04 §6.3](04-pdf-parsing-pipeline.md), [09 §6.4](09-rust-migration-guide.md) — font size comparison |

### B.4 List Detection Constants (Spec 04 §9)

| Constant | Value | Used In |
|----------|-------|---------|
| `LIST_ITEM_X_INTERVAL_RATIO` | 0.3 | [04 §9.3](04-pdf-parsing-pipeline.md) — indentation gap |
| `LIST_ITEM_PROBABILITY` | 0.7 | [04 §9.3](04-pdf-parsing-pipeline.md) — merge probability |
| `LIST_ITEM_BASELINE_DIFFERENCE` | 1.2 | [04 §9.3](04-pdf-parsing-pipeline.md) — baseline alignment |

### B.5 Paragraph Constants (Spec 04 Stage 10)

| Constant | Value | Used In |
|----------|-------|---------|
| `DIFFERENT_LINES_PROBABILITY` / `MERGE_THRESHOLD` | 0.75 | [04 Stage 10](04-pdf-parsing-pipeline.md) — paragraph merge |

### B.6 Heading Constants (Spec 04 Stage 12)

| Constant | Value | Used In |
|----------|-------|---------|
| `HEADING_PROBABILITY` | 0.75 | [04 Stage 12](04-pdf-parsing-pipeline.md), [02 §8.1](02-functional-spec.md), [03 §3.1](03-technical-architecture.md) |
| `font_size_dominant_min` | 10.0 | [04 Stage 12](04-pdf-parsing-pipeline.md) — body text range |
| `font_size_dominant_max` | 13.0 | [04 Stage 12](04-pdf-parsing-pipeline.md) — body text range |
| `font_size_heading_min` | 10.0 | [04 Stage 12](04-pdf-parsing-pipeline.md) — heading candidate range |
| `font_size_heading_max` | 32.0 | [04 Stage 12](04-pdf-parsing-pipeline.md) — heading candidate range |

### B.7 Caption Constants (Spec 04 Stage 14)

| Constant | Value | Used In |
|----------|-------|---------|
| `CAPTION_PROBABILITY` | 0.75 | [04 Stage 14](04-pdf-parsing-pipeline.md), [02 §11](02-functional-spec.md) |

### B.8 XY-Cut++ Constants (Spec 04 Stage 18)

| Constant | Value | Used In |
|----------|-------|---------|
| `BETA` | 2.0 | [04 Stage 18](04-pdf-parsing-pipeline.md) — cut scoring |
| `DENSITY_THRESHOLD` | 0.9 | [04 Stage 18](04-pdf-parsing-pipeline.md) — block density |

### B.9 Hybrid / Triage Constants (Spec 07 §2)

| Constant | Value | Used In |
|----------|-------|---------|
| `DEFAULT_LINE_RATIO_THRESHOLD` | 0.3 | [07 §2.4](07-hybrid-mode.md) — signal 5 |
| `DEFAULT_ALIGNED_LINE_GROUPS_THRESHOLD` | 5 | [07 §2.4](07-hybrid-mode.md) — signal 6 (disabled) |
| `BASELINE_EPSILON` | 0.1 | [07 §2.4](07-hybrid-mode.md) — same-baseline tolerance |
| `MIN_LINE_COUNT_FOR_TABLE` | 8 | [07 §2.4](07-hybrid-mode.md) — border line count |
| `MIN_GRID_LINES` | 3 | [07 §2.4](07-hybrid-mode.md) — grid detection |
| `MIN_ROW_SEPARATOR_PATTERN` | 5 | [07 §2.4](07-hybrid-mode.md) — row separator pattern |
| `MIN_LINE_ART_FOR_TABLE` | 8 | [07 §2.4](07-hybrid-mode.md) — LineArt chunk count |
| `MIN_ALIGNED_SHORT_LINES` | 2 | [07 §2.4](07-hybrid-mode.md) — matching short lines |
| `MIN_CONSECUTIVE_PATTERNS` | 2 | [07 §2.4](07-hybrid-mode.md) — consecutive suspicious pairs |
| `MIN_LARGE_IMAGE_RATIO` | 0.11 | [07 §2.4](07-hybrid-mode.md) — 11% of page area |
| `MIN_IMAGE_ASPECT_RATIO` | 1.75 | [07 §2.4](07-hybrid-mode.md) — width/height for large image |
| `HIGH_PATTERN_COUNT_THRESHOLD` | 30 | [07 §2.4](07-hybrid-mode.md) — skip consecutive check |
| `MIN_TABLE_PATTERNS` | 3 | [07 §2.4](07-hybrid-mode.md) — min text pattern count |
| `MIN_PATTERN_DENSITY` | 0.10 | [07 §2.4](07-hybrid-mode.md) — density threshold |
| `MIN_PATTERNS_FOR_DENSITY` | 2 | [07 §2.4](07-hybrid-mode.md) — min for density path |
| `MULTI_COLUMN_X_SHIFT_RATIO` | 2.0 | [07 §2.4](07-hybrid-mode.md) — multi-column filter |

---

## C. Data Types

### C.1 Core Geometry

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `BoundingBox` | [05 §3.1](05-data-models.md) | [04 §1.6](04-pdf-parsing-pipeline.md), [08 §1.4](08-output-formats.md), [09 §4.1](09-rust-migration-guide.md), [09 §8.4](09-rust-migration-guide.md), [09 §11.2](09-rust-migration-guide.md) |
| `MultiBoundingBox` | [05 §3.2](05-data-models.md) | [04 §4](04-pdf-parsing-pipeline.md) |
| `Vertex` | [05 §3.3](05-data-models.md) | [05 §4.7](05-data-models.md) |

### C.2 Content Elements (Chunks)

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `TextChunk` | [05 §4.1](05-data-models.md) | [04 §1.2](04-pdf-parsing-pipeline.md), [04 §2](04-pdf-parsing-pipeline.md), [04 §6](04-pdf-parsing-pipeline.md), [09 §3.3](09-rust-migration-guide.md) |
| `TextLine` | [05 §4.2](05-data-models.md) | [04 §6](04-pdf-parsing-pipeline.md), [04 §9](04-pdf-parsing-pipeline.md) |
| `TextBlock` | [05 §4.3](05-data-models.md) | [04 §6](04-pdf-parsing-pipeline.md) |
| `TextColumn` | [05 §4.4](05-data-models.md) | [04 §6](04-pdf-parsing-pipeline.md) |
| `ImageChunk` | [05 §4.5](05-data-models.md) | [04 §1.5](04-pdf-parsing-pipeline.md), [08 §6](08-output-formats.md), [09 §3.3](09-rust-migration-guide.md) |
| `LineChunk` | [05 §4.6](05-data-models.md) | [04 §1.3](04-pdf-parsing-pipeline.md), [04 §5](04-pdf-parsing-pipeline.md), [07 §2.2](07-hybrid-mode.md) |
| `LineArtChunk` | [05 §4.7](05-data-models.md) | [04 §1.3](04-pdf-parsing-pipeline.md), [07 §2.2](07-hybrid-mode.md) |
| `LinesCollection` | [05 §4.8](05-data-models.md) | [09 §4.3](09-rust-migration-guide.md) |

### C.3 Semantic Nodes

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `SemanticType` (30 variants) | [05 §5.1](05-data-models.md) | [04 §13](04-pdf-parsing-pipeline.md), [08 §1.3](08-output-formats.md), [09 §4.1](09-rust-migration-guide.md) |
| `SemanticTextNode` | [05 §5.2](05-data-models.md) | [04 §10](04-pdf-parsing-pipeline.md), [04 §12](04-pdf-parsing-pipeline.md) |
| `SemanticParagraph` | [05 §5.3](05-data-models.md) | [04 §10](04-pdf-parsing-pipeline.md), [08 §2.1](08-output-formats.md) |
| `SemanticHeading` | [05 §5.4](05-data-models.md) | [04 §12](04-pdf-parsing-pipeline.md), [08 §2.1](08-output-formats.md) |
| `SemanticNumberHeading` | [05 §5.5](05-data-models.md) | [04 §12](04-pdf-parsing-pipeline.md) |
| `SemanticCaption` | [05 §5.6](05-data-models.md) | [04 §14](04-pdf-parsing-pipeline.md), [08 §2.1](08-output-formats.md) |
| `SemanticHeaderOrFooter` | [05 §5.7](05-data-models.md) | [04 §8](04-pdf-parsing-pipeline.md), [08 §2.1](08-output-formats.md) |
| `SemanticFigure` | [05 §5.8](05-data-models.md) | [08 §2.1](08-output-formats.md) |
| `SemanticTable` | [05 §5.9](05-data-models.md) | [04 §4](04-pdf-parsing-pipeline.md), [08 §1.7](08-output-formats.md) |
| `SemanticFormula` | [05 §5.10](05-data-models.md) | [08 §1.10](08-output-formats.md) |
| `SemanticPicture` | [05 §5.11](05-data-models.md) | [08 §2.5](08-output-formats.md) |

### C.4 Table Types

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `TableBorder` | [05 §5.9](05-data-models.md) | [04 §4](04-pdf-parsing-pipeline.md), [08 §1.7](08-output-formats.md) |
| `TableBorderRow` | [05 §5.9](05-data-models.md) | [04 §4.2](04-pdf-parsing-pipeline.md), [08 §1.7](08-output-formats.md) |
| `TableBorderCell` | [05 §5.9](05-data-models.md) | [04 §4.2](04-pdf-parsing-pipeline.md), [08 §1.7](08-output-formats.md) |
| `TableBordersCollection` | [05 §4.8](05-data-models.md) | [04 §1.4](04-pdf-parsing-pipeline.md), [09 §4.3](09-rust-migration-guide.md) |

### C.5 List Types

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `PDFList` | [05 §5](05-data-models.md) | [04 §9](04-pdf-parsing-pipeline.md), [08 §1.8](08-output-formats.md) |
| `ListItem` | [05 §5](05-data-models.md) | [04 §9.3](04-pdf-parsing-pipeline.md), [08 §1.8](08-output-formats.md) |

### C.6 Configuration Types

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `Config` | [03 §6.1](03-technical-architecture.md) | [06 §9.2](06-cli-interface.md), [09 §4.3](09-rust-migration-guide.md), [09 §4.5](09-rust-migration-guide.md) |
| `FilterConfig` | [03 §6.1](03-technical-architecture.md) | [04 §2](04-pdf-parsing-pipeline.md), [06 §2.2](06-cli-interface.md) |
| `HybridConfig` | [03 §6.1](03-technical-architecture.md) | [07 §1](07-hybrid-mode.md), [06 §2.2](06-cli-interface.md) |
| `CliArgs` | [06 §9.1](06-cli-interface.md) | [09 §4.5](09-rust-migration-guide.md) |

### C.7 Error Types

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `OpendataLoaderError` | [03 §6](03-technical-architecture.md) | [09 §4.4](09-rust-migration-guide.md) |
| `PdfError` | [03 §6](03-technical-architecture.md) | [09 §4.4](09-rust-migration-guide.md) |
| `HybridError` | [03 §6](03-technical-architecture.md) | [09 §4.4](09-rust-migration-guide.md) |
| `ConfigError` | [09 §4.4](09-rust-migration-guide.md) | [06 §8](06-cli-interface.md) |
| `OutputError` | [09 §4.4](09-rust-migration-guide.md) | [08](08-output-formats.md) |

### C.8 Pipeline Types

| Type | Defined In | Referenced In |
|------|-----------|---------------|
| `ProcessingContext` | [03 §4.2](03-technical-architecture.md) | [09 §4.3](09-rust-migration-guide.md) |
| `PageContent` | [09 §4.2](09-rust-migration-guide.md) | [03 §4.2](03-technical-architecture.md) |
| `ContentElement` (enum) | [05 §1](05-data-models.md) | [09 §4.1](09-rust-migration-guide.md) |
| `ElementMeta` | [09 §4.1](09-rust-migration-guide.md) | [05 §2](05-data-models.md) |
| `ModeWeightStatistics` | [04 §12](04-pdf-parsing-pipeline.md) | [09 §6.5](09-rust-migration-guide.md) |
| `TriageDecision` | [07 §2.5](07-hybrid-mode.md) | [09 §4.3](09-rust-migration-guide.md) |
| `TriageSignals` | [07 §2.5](07-hybrid-mode.md) | [07 §2.1](07-hybrid-mode.md) |

---

## D. CLI Options

All 24 options defined in [06 §2.1](06-cli-interface.md). Cross-references:

| Option | Type | Affects |
|--------|------|---------|
| `--input` / `-i` | String | [06 §8.1](06-cli-interface.md), [09 §4.5](09-rust-migration-guide.md) |
| `--output` / `-o` | String | [06 §8.2](06-cli-interface.md), [08 §7](08-output-formats.md) |
| `--format` / `-f` | Enum[] | [06 §2.2](06-cli-interface.md), [08](08-output-formats.md) all sections |
| `--password` / `-p` | String | [02 §2.1](02-functional-spec.md), [04 §1.1](04-pdf-parsing-pipeline.md) |
| `--pages` | Range | [02 §2.2](02-functional-spec.md), [06 §2.2](06-cli-interface.md) |
| `--table-method` | Enum | [02 §6.1](02-functional-spec.md), [04 §3](04-pdf-parsing-pipeline.md) |
| `--reading-order` | Enum | [02 §7](02-functional-spec.md), [04 §18](04-pdf-parsing-pipeline.md) |
| `--use-struct-tree` | Bool | [02 §12](02-functional-spec.md), [04 §1](04-pdf-parsing-pipeline.md), [09 §6.6](09-rust-migration-guide.md) |
| `--keep-line-breaks` | Bool | [02 §13](02-functional-spec.md), [04 §10](04-pdf-parsing-pipeline.md) |
| `--include-header-footer` | Bool | [02 §9.2](02-functional-spec.md), [04 §8](04-pdf-parsing-pipeline.md) |
| `--image-output` | Enum | [02 §3.3](02-functional-spec.md), [08 §6](08-output-formats.md) |
| `--image-format` | Enum | [08 §6.1](08-output-formats.md), [09 §6.7](09-rust-migration-guide.md) |
| `--image-dir` | String | [08 §6.4](08-output-formats.md) |
| `--content-safety-off` | Enum[] | [02 §5.1](02-functional-spec.md), [04 §2](04-pdf-parsing-pipeline.md) |
| `--filter-sensitive-data` | Bool | [02 §5.3](02-functional-spec.md), [04 §19](04-pdf-parsing-pipeline.md) |
| `--replace-invalid-chars` | String | [02 §13.1](02-functional-spec.md), [04 §2.2.10](04-pdf-parsing-pipeline.md) |
| `--hybrid` | Enum | [07 §1](07-hybrid-mode.md), [06 §2.2](06-cli-interface.md) |
| `--hybrid-mode` | Enum | [07 §5](07-hybrid-mode.md), [06 §2.2](06-cli-interface.md) |
| `--hybrid-url` | String | [07 §3](07-hybrid-mode.md), [06 §8.3](06-cli-interface.md) |
| `--hybrid-timeout` | u64 | [07 §8](07-hybrid-mode.md), [06 §2.2](06-cli-interface.md) |
| `--hybrid-fallback` | Bool | [07 §5.4](07-hybrid-mode.md) |
| `--md-page-separator` | String | [08 §2.4](08-output-formats.md), [06 §2.2](06-cli-interface.md) |
| `--text-page-separator` | String | [08 §4.4](08-output-formats.md) |
| `--html-page-separator` | String | [08 §3](08-output-formats.md) |
| `--export-options` | Bool | [06 §6](06-cli-interface.md), [09 §5.2](09-rust-migration-guide.md) |

---

## E. Output Formats

| Format | Spec | Key Sections |
|--------|------|-------------|
| **JSON** | [08 §1](08-output-formats.md) | Envelope §1.1, Field names §1.2, Element types §1.3, Tables §1.7, Lists §1.8, Images §1.9, Formulas §1.10 |
| **Markdown** | [08 §2](08-output-formats.md) | Rendering rules §2.1, Pipe tables §2.2, HTML tables §2.3, Pictures §2.5 |
| **HTML** | [08 §3](08-output-formats.md) | Wrapper §3.1, Elements §3.2, Escaping §3.3, Tables §3.4 |
| **Text** | [08 §4](08-output-formats.md) | Elements §4.1, Indentation §4.2, Tables §4.3, Separators §4.4 |
| **Annotated PDF** | [08 §5](08-output-formats.md) | Overlay §5.1, Annotations §5.2, Colors §5.3, Layers §5.4, Tooltips §5.5 |
| **Image extraction** | [08 §6](08-output-formats.md) | Flow §6.1, Naming §6.2, Base64 §6.3, Directory §6.4 |
| JSON serialization (Rust) | [09 §4.6](09-rust-migration-guide.md) | Serde config, field name mapping, f64 precision |

---

## F. Hybrid Mode Components

| Component | Spec | Related |
|-----------|------|---------|
| **TriageProcessor** | [07 §2](07-hybrid-mode.md) | [09 §5.7](09-rust-migration-guide.md) — Rust port |
| Signal 1: Border table lines | [07 §2.1](07-hybrid-mode.md) | [04 §1.3](04-pdf-parsing-pipeline.md) — line extraction |
| Signal 2: Vector table patterns | [07 §2.2](07-hybrid-mode.md) | [05 §4.6](05-data-models.md) — LineChunk |
| Signal 3: Text table patterns | [07 §2.3](07-hybrid-mode.md) | [04 §6](04-pdf-parsing-pipeline.md) — text line grouping |
| Signal 4: Large images | [07 §2.1](07-hybrid-mode.md) | [05 §4.5](05-data-models.md) — ImageChunk |
| Signal 5: Line ratio | [07 §2.1](07-hybrid-mode.md) | — |
| Signal 6: Aligned line groups (disabled) | [07 §2.1](07-hybrid-mode.md) | — |
| **DoclingFastServerClient** | [07 §3.1](07-hybrid-mode.md) | [09 §1.2](09-rust-migration-guide.md) — reqwest |
| **HancomClient** | [07 §3.2](07-hybrid-mode.md) | [09 §5.7](09-rust-migration-guide.md) — Rust port |
| **DoclingSchemaTransformer** | [07 §4](07-hybrid-mode.md) | [09 §5.7](09-rust-migration-guide.md) — Rust port |
| **Python FastAPI server** | [07 §6](07-hybrid-mode.md) | — (not part of Rust rewrite) |
| **Coordinate conversion** | [07 §4.1](07-hybrid-mode.md) | [09 §6.1](09-rust-migration-guide.md) |
| **Client factory** | [07 §7](07-hybrid-mode.md) | [09 §5.7](09-rust-migration-guide.md) |
| **Fallback logic** | [07 §5.4](07-hybrid-mode.md) | [09 §5.7](09-rust-migration-guide.md) |

---

## G. Rust Crate Inventory

| Crate | Version | Primary Spec | Purpose |
|-------|---------|-------------|---------|
| `lopdf` | 0.39.0 | [09 §1.1](09-rust-migration-guide.md) | PDF object access, I/O |
| `pdf` | 0.10.0 | [09 §1.2](09-rust-migration-guide.md) | Content stream parsing, fonts |
| `pdf-extract` | 0.10.0 | [09 §1.1](09-rust-migration-guide.md) | Text extraction (reference only) |
| `clap` | 4.6.0 | [09 §4.5](09-rust-migration-guide.md) | CLI argument parsing |
| `serde` | 1.x | [09 §2.3](09-rust-migration-guide.md) | Serialization framework |
| `serde_json` | 1.0.149 | [09 §4.6](09-rust-migration-guide.md) | JSON output |
| `reqwest` | 0.13.2 | [09 §1.2](09-rust-migration-guide.md) | HTTP client (hybrid mode) |
| `tokio` | 1.x | [09 §2.4](09-rust-migration-guide.md) | Async runtime |
| `regex` | 1.12.3 | [09 §8.2](09-rust-migration-guide.md) | Pattern matching (PII, lists) |
| `image` | 0.25.10 | [09 §6.7](09-rust-migration-guide.md) | Image decode/encode |
| `rayon` | 1.11.0 | [09 §8.3](09-rust-migration-guide.md) | Parallel batch processing |
| `thiserror` | 2.0.18 | [09 §4.4](09-rust-migration-guide.md) | Error derive macro |
| `anyhow` | 1.x | [09 §2.4](09-rust-migration-guide.md) | CLI error context |
| `printpdf` | 0.9.1 | [09 §1.1](09-rust-migration-guide.md) | Annotated PDF output |
| `tiny-skia` | 0.11.x | [09 §3.4](09-rust-migration-guide.md) | Page rasterization |
| `ordered-float` | 4.x | [09 §6.4](09-rust-migration-guide.md) | f64 in map keys |
| `indexmap` | 2.x | [09 §6.3](09-rust-migration-guide.md) | Insertion-order maps |
| `base64` | 0.22.x | [09 §2.3](09-rust-migration-guide.md) | Image base64 encoding |
| `unicode-normalization` | 0.1.x | [09 §6.2](09-rust-migration-guide.md) | Unicode NFC |
| `log` + `env_logger` | 0.4.x / 0.11.x | [09 §2.3](09-rust-migration-guide.md) | Logging |

---

## H. Processing Paths

| Path | Entry Condition | Spec | Stages |
|------|----------------|------|--------|
| **Normal** (Java heuristic) | Default; no `--use-struct-tree`, no `--hybrid` | [03 §1.2](03-technical-architecture.md), [04](04-pdf-parsing-pipeline.md) | All 20 stages |
| **Tagged** (structure tree) | `--use-struct-tree` and PDF has `/StructTreeRoot` | [03 §1.2](03-technical-architecture.md), [02 §12](02-functional-spec.md), [09 §6.6](09-rust-migration-guide.md) | Structure tree walk |
| **Hybrid** (triage + backend) | `--hybrid docling-fast` or `--hybrid hancom` | [03 §1.2](03-technical-architecture.md), [07](07-hybrid-mode.md) | Triage → split → Java path / backend → merge |

---

## I. Exit Codes

| Code | Meaning | Spec |
|------|---------|------|
| 0 | All files processed successfully | [06 §3](06-cli-interface.md) |
| 1 | One or more files failed | [06 §3](06-cli-interface.md) |
| 2 | Invalid arguments / configuration | [06 §3](06-cli-interface.md) |

---

## J. JSON Field Name Mapping

Canonical field names using space-separated convention (see [08 §1.2](08-output-formats.md)):

| Internal Name | JSON Field | Spec |
|---------------|-----------|------|
| `file_name` | `"file name"` | [08 §1.1](08-output-formats.md) |
| `creation_date` | `"creation date"` | [08 §1.1](08-output-formats.md) |
| `modification_date` | `"modification date"` | [08 §1.1](08-output-formats.md) |
| `page_count` | `"page count"` | [08 §1.1](08-output-formats.md) |
| `page_number` | `"page number"` | [08 §1.4](08-output-formats.md) |
| `bounding_box` | `"bounding box"` | [08 §1.4](08-output-formats.md) |
| `text_format` | `"text format"` | [08 §1.5](08-output-formats.md) |
| `linked_content_id` | `"linked content id"` | [08 §1.5](08-output-formats.md) |
| `row_span` | `"row span"` | [08 §1.7](08-output-formats.md) |
| `col_span` | `"col span"` | [08 §1.7](08-output-formats.md) |
| `nesting_level` | `"nesting level"` | [08 §1.4](08-output-formats.md) |

---

## K. Annotated PDF Color Scheme

| Color | RGB | Element Type | Spec |
|-------|-----|-------------|------|
| Blue | (0, 0, 255) | Paragraph | [08 §5.3](08-output-formats.md) |
| Red | (255, 0, 0) | Heading | [08 §5.3](08-output-formats.md) |
| Green | (0, 128, 0) | Table | [08 §5.3](08-output-formats.md) |
| Orange | (255, 165, 0) | List | [08 §5.3](08-output-formats.md) |
| Purple | (128, 0, 128) | Figure / Image | [08 §5.3](08-output-formats.md) |
| Cyan | (0, 255, 255) | Caption | [08 §5.3](08-output-formats.md) |
| Gray | (128, 128, 128) | Header / Footer | [08 §5.3](08-output-formats.md) |

---

## L. Document Map (Reading Order)

| # | Document | Focus | Priority |
|---|----------|-------|----------|
| 01 | [Project Overview](01-overview.md) | Mission, scope, terminology | Read first |
| 02 | [Functional Spec](02-functional-spec.md) | Use cases, features, requirements | What to build |
| 03 | [Technical Architecture](03-technical-architecture.md) | Modules, data flow, dependencies | How it fits together |
| 04 | [PDF Parsing Pipeline](04-pdf-parsing-pipeline.md) | 20-stage algorithm reference | Core implementation guide |
| 05 | [Data Models](05-data-models.md) | Type hierarchy, struct definitions | All data structures |
| 06 | [CLI Interface](06-cli-interface.md) | 24 options, validation, code gen | User-facing interface |
| 07 | [Hybrid Mode](07-hybrid-mode.md) | Triage, backends, merge logic | AI backend integration |
| 08 | [Output Formats](08-output-formats.md) | JSON/MD/HTML/Text/PDF rendering | Output layer |
| 09 | [Rust Migration Guide](09-rust-migration-guide.md) | Crates, patterns, phased plan | How to build in Rust |
| 10 | [Cross-Reference Index](10-cross-reference-index.md) | This document | Master lookup |

---

## M. Benchmark Metrics

| Metric | Full Name | Measures | Threshold File |
|--------|-----------|----------|----------------|
| **NID** | Normalized Inverse Displacement | Reading order quality | [thresholds.json](../tests/benchmark/thresholds.json) |
| **TEDS** | Tree Edit Distance Similarity | Table structure accuracy | [thresholds.json](../tests/benchmark/thresholds.json) |
| **MHS** | Mean Heading Similarity | Heading detection quality | [thresholds.json](../tests/benchmark/thresholds.json) |
| **Table F1** | Table Detection F1 Score | Table finding accuracy | [thresholds.json](../tests/benchmark/thresholds.json) |
| **Speed** | Seconds per page | Processing performance | [thresholds.json](../tests/benchmark/thresholds.json) |

Referenced in: [02 §14](02-functional-spec.md), [09 §7.4](09-rust-migration-guide.md), [01 §2](01-overview.md)

---

## N. Code Generation Pipeline

```
options.json ──► generate-options.mjs ──► Python: .generated.py
                                     ──► Node.js: .generated.ts
                                     ──► MDX docs: .generated.mdx
```

| Artifact | Source | Spec |
|----------|--------|------|
| `options.json` | Hand-maintained (24 options) | [06 §7](06-cli-interface.md) |
| `generate-options.mjs` | Code generator script | [06 §7](06-cli-interface.md) |
| Python `.generated.py` | Auto-generated from options.json | [06 §7](06-cli-interface.md), [09 §9.1](09-rust-migration-guide.md) |
| Node.js `.generated.ts` | Auto-generated from options.json | [06 §7](06-cli-interface.md), [09 §9.2](09-rust-migration-guide.md) |
| MDX docs | Auto-generated from options.json | [06 §7](06-cli-interface.md) |
| `schema.json` | JSON output schema | [08 §1](08-output-formats.md) |

**Important**: After changing CLI options in Java/Rust, run `npm run sync` to regenerate all wrappers ([CLAUDE.md](../CLAUDE.md)).
