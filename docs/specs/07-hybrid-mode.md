# 07 — Hybrid Mode

> **Cross-references**: [03-technical-architecture](03-technical-architecture.md) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) | [05-data-models](05-data-models.md)

---

## 1. Architecture Overview

```
                           ┌─────────────────────────┐
                           │     CLI / API Entry      │
                           └────────────┬─────────────┘
                                        │
                           ┌────────────v─────────────┐
                           │  Content Filter (all pages) │
                           └────────────┬─────────────┘
                                        │
                    ┌───────────────────┬┘
                    │                   │
             mode=auto            mode=full
                    │                   │
           ┌────────v─────────┐   All pages → BACKEND
           │  TriageProcessor  │        │
           │  (per page)       │        │
           └─────┬────┬───────┘        │
                 │    │                 │
            JAVA │    │ BACKEND         │
                 │    │                 │
        ┌────────v┐  ┌v────────────────v┐
        │ Java    │  │  HTTP Client      │
        │ Pipeline│  │  (OkHttp)         │
        │ (§4)    │  │  POST /v1/convert │
        └────┬────┘  └────────┬──────────┘
             │                │
             │        ┌───────v──────────┐
             │        │ Schema Transform │
             │        │ (Docling/Hancom) │
             │        └───────┬──────────┘
             │                │
        ┌────v────────────────v────┐
        │   Merge Results by Page  │
        └────────────┬─────────────┘
                     │
        ┌────────────v─────────────┐
        │  Cross-Page Post-Process │
        │  - Header/Footer detect  │
        │  - List linking          │
        │  - Table linking         │
        │  - Heading levels        │
        │  - Nesting levels        │
        └──────────────────────────┘
```

---

## 2. Triage Processor

### 2.1 Signal Priority Chain

Signals are evaluated in order. **First match wins**.

| Priority | Signal Name | Condition | Decision | Confidence |
|----------|------------|-----------|----------|------------|
| 1 | `hasTableBorder` | `TableBorder` exists for this page (from line pre-processing) | JAVA | 1.0 |
| 2 | `hasVectorTableSignal` | Any of 5 sub-signals (see §2.2) | BACKEND | 0.95 |
| 3 | `hasTextTablePattern` | Text-based table pattern (see §2.3) | BACKEND | 0.9 |
| 3.5 | `hasLargeImage` | `largeImageRatio ≥ 0.11` AND `aspectRatio ≥ 1.75` | BACKEND | 0.85 |
| 4 | ~~`hasSuspiciousPattern`~~ | Same-baseline chunks with gap > 3× height | ~~BACKEND~~ | ~~0.85~~ |
| 5 | `lineToTextRatio` | `lineChunkCount / totalContent > 0.3` | BACKEND | 0.8 |
| 6 | ~~`alignedLineGroups`~~ | `≥ 5` groups of baseline-aligned text | ~~BACKEND~~ | ~~0.7~~ |
| Default | No signal triggered | — | JAVA | 0.9 |

**Disabled signals**: Signals 4 and 6 are disabled based on experiments:
- Signal 4: Caused 19 false positives (28.4%) in Experiment 003
- Signal 6: Caused 12 false positives, 0 new true positives in Experiment 004D

### 2.2 Vector Table Sub-Signals (Signal 2)

Any one of these triggers BACKEND:

| Sub-Signal | Condition |
|------------|-----------|
| `hasGridLines` | `horizontalLineCount ≥ 3` AND `verticalLineCount ≥ 3` |
| `hasTableBorderLines` | `horizontalLineCount + verticalLineCount ≥ 8` |
| `lineArtCount` | `lineArtCount ≥ 8` |
| `hasRowSeparatorPattern` | ≥ 5 alternating line/text patterns (start with line) |
| `hasAlignedShortLines` | ≥ 2 lines with same length (within 5%) and aligned left edges |

### 2.3 Text Table Pattern (Signal 3)

```
hasTextTablePattern = 
    (hasConsecutivePatterns || tablePatternCount >= 30)
    AND (tablePatternCount >= 3 || (patternDensity >= 0.10 AND tablePatternCount >= 2))
```

Where:
- `tablePatternCount`: Number of same-baseline text chunk pairs with gap > `3.0 × textHeight`
- `patternDensity`: `tablePatternCount / textChunkCount`
- `hasConsecutivePatterns`: `maxConsecutiveStreak ≥ 2` (adjacent suspicious lines)

Multi-column filter: Skip chunks where `xShift > MULTI_COLUMN_X_SHIFT_RATIO (2.0) × avgWidth`.

### 2.4 Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_LINE_RATIO_THRESHOLD` | 0.3 | Signal 5 |
| `DEFAULT_ALIGNED_LINE_GROUPS_THRESHOLD` | 5 | Signal 6 (disabled) |
| `DEFAULT_GRID_GAP_MULTIPLIER` | 3.0 | Gap width multiplier |
| `BASELINE_EPSILON` | 0.1 | Same-baseline tolerance |
| `MIN_LINE_COUNT_FOR_TABLE` | 8 | Border line count |
| `MIN_GRID_LINES` | 3 | Grid detection |
| `MIN_ROW_SEPARATOR_PATTERN` | 5 | Row separator pattern |
| `MIN_LINE_ART_FOR_TABLE` | 8 | LineArt chunk count |
| `LINE_LENGTH_TOLERANCE` | 0.05 | Aligned short lines (5%) |
| `MIN_ALIGNED_SHORT_LINES` | 2 | Min matching short lines |
| `MIN_CONSECUTIVE_PATTERNS` | 2 | Consecutive suspicious pairs |
| `MIN_LARGE_IMAGE_RATIO` | 0.11 | 11% of page area |
| `MIN_IMAGE_ASPECT_RATIO` | 1.75 | Width/height for large image |
| `HIGH_PATTERN_COUNT_THRESHOLD` | 30 | Skip consecutive check |
| `MIN_TABLE_PATTERNS` | 3 | Min text pattern count |
| `MIN_PATTERN_DENSITY` | 0.10 | Density threshold |
| `MIN_PATTERNS_FOR_DENSITY` | 2 | Min for density path |
| `MULTI_COLUMN_X_SHIFT_RATIO` | 2.0 | Multi-column filter |
| `X_DIFFERENCE_EPSILON` | 1.5 | Gap detection tolerance |

### 2.5 TriageSignals Data Structure

```rust
pub struct TriageSignals {
    pub line_chunk_count: usize,
    pub text_chunk_count: usize,
    pub line_to_text_ratio: f64,
    pub aligned_line_groups: usize,
    pub has_table_border: bool,
    pub has_suspicious_pattern: bool,
    pub horizontal_line_count: usize,
    pub vertical_line_count: usize,
    pub line_art_count: usize,
    pub has_grid_lines: bool,
    pub has_table_border_lines: bool,
    pub has_row_separator_pattern: bool,
    pub has_aligned_short_lines: bool,
    pub table_pattern_count: usize,
    pub max_consecutive_streak: usize,
    pub pattern_density: f64,
    pub has_consecutive_patterns: bool,
    pub large_image_ratio: f64,
    pub large_image_aspect_ratio: f64,
}

pub struct TriageResult {
    pub decision: TriageDecision,
    pub confidence: f64,
    pub winning_signal: String,
    pub signals: TriageSignals,
}
```

---

## 3. HTTP Protocol

### 3.1 Docling Backend

#### Health Check
```
GET /health
Timeout: 3000ms (connect + read)
Expected: HTTP 200
```

#### Conversion
```
POST /v1/convert/file
Content-Type: multipart/form-data

Form fields:
  files:       <pdf_bytes>  (filename="document.pdf", type=application/pdf)
  page_ranges: "1-5"        (optional, format: "min-max", 1-indexed)

Timeout: configurable (default 30000ms)
```

#### Response Format
```json
{
  "status": "success" | "partial_success" | "failure",
  "document": {
    "json_content": {
      "schema_name": "DoclingDocument",
      "version": "1.0",
      "name": "document.pdf",
      "texts": [...],
      "tables": [...],
      "pictures": [...],
      "pages": {
        "1": { "size": {"width": 612.0, "height": 792.0} },
        "2": { ... }
      }
    }
  },
  "processing_time": 2.34,
  "errors": [],
  "failed_pages": [3, 7]
}
```

### 3.2 Hancom Backend

#### 3-Step Workflow

**Step 1: Upload**
```
POST /v1/dl/files/upload
Content-Type: multipart/form-data
Form field: file = <pdf_bytes>

Response: {"codeNum": 0, "code": "file.upload.success", "data": {"fileId": "abc123"}}
```

**Step 2: Get Visual Info**
```
GET /v1/dl/files/{fileId}/visualinfo?engine=pdf_ai_dl&dlaMode=ENABLED&ocrMode=FORCE

Response: Hancom-specific JSON (mapped via HancomSchemaTransformer)
```

**Step 3: Cleanup** (always, even on failure)
```
DELETE /v1/dl/files/{fileId}

Response: ignored
```

#### Default URL
```
https://dataloader.cloud.hancom.com/studio-lite/api
```

#### Health Check
```
HEAD <base_url>
Any HTTP response (including 401, 403) = server reachable
```

---

## 4. Schema Transformation

### 4.1 Docling → Internal Model

#### Text Elements

| Docling Label | → Internal Type | Notes |
|---------------|-----------------|-------|
| `text` | `SemanticParagraph` | Default font_size = 12.0 |
| `section_header` | `SemanticHeading` | Level from `meta.level` (default 1) |
| `caption` | `SemanticParagraph` | Treated as paragraph |
| `footnote` | `SemanticParagraph` | Treated as paragraph |
| `list_item` | `SemanticParagraph` | Treated as paragraph |
| `formula` | `SemanticFormula` | LaTeX text from content |
| `page_header` | **Filtered** | Furniture — skipped |
| `page_footer` | **Filtered** | Furniture — skipped |

#### Tables

```
Docling table → TableBorder:
  1. Read data.grid → determine num_rows × num_columns
  2. Build cell map from data.table_cells, keyed by "row,col"
  3. For each cell:
     - Extract: start_row_offset_idx, start_col_offset_idx, row_span, col_span, text
     - Compute bbox by evenly dividing table bbox:
         col_width = table_width / num_cols
         row_height = table_height / num_rows
  4. Cell text → SemanticParagraph → cell.contents
```

#### Pictures

```
Docling picture → SemanticPicture:
  - index: incrementing counter (reset per transform() call)
  - description: annotations[kind="description"].text (optional)
  - bbox: from prov[0].bbox
```

#### Coordinate Conversion

```
if coord_origin == "TOPLEFT":
    top = page_height - docling_t
    bottom = page_height - docling_b
elif coord_origin == "BOTTOMLEFT":
    // direct mapping
    left = docling_l
    bottom = docling_b
    right = docling_r
    top = docling_t
```

#### Page Sort Order

Per page, elements sorted by:
1. Top Y descending (with 5.0pt tolerance for same-line grouping)
2. Left X ascending

---

## 5. Processing Flow

### 5.1 HybridDocumentProcessor Phases

| Phase | Description |
|-------|-------------|
| 0 | Check backend availability (`checkAvailability()`) — fail fast if unreachable |
| 1 | Content filter all pages (`ContentFilterProcessor`) |
| 2 | Triage: if `mode=full` → all BACKEND (confidence 1.0); if `mode=auto` → per-page triage |
| 3 | Partition pages into `java_pages` and `backend_pages` sets |
| 4 | Process Java pages through full pipeline (see §5.2) |
| 5 | Process backend pages via HTTP client + schema transformer |
| 6 | Merge results by page number |
| 7 | Cross-page post-processing (header/footer, list/table linking, heading levels, nesting) |

### 5.2 Java Path Per-Page Pipeline

```
1. TableBorderProcessor.processTableBorders()
2. Filter out LineChunk objects
3. TextLineProcessor.processTextLines()
4. SpecialTableProcessor.detectSpecialTables()
5. ParagraphProcessor.processContent()
6. ListProcessor.processContent()
7. HeadingProcessor.processContent()
8. DocumentProcessor.setIDs()
9. CaptionProcessor.processContent()
```

### 5.3 Backend Path

```
1. Read PDF file bytes
2. Build HybridRequest (JSON format only)
3. Send to backend
4. Parse response, extract failed_pages
5. Transform via DoclingSchemaTransformer or HancomSchemaTransformer
6. Apply replaceUndefinedCharacters()
7. documentProcessor.setIDs()
```

### 5.4 Fallback Logic

```
                    Backend Request
                         │
                    ┌────v────┐
              ┌─────┤ Result? ├─────┐
              │     └─────────┘     │
         Full Failure         Partial Success
              │                     │
     ┌────────v────────┐   ┌───────v────────┐
     │fallback enabled?│   │fallback enabled?│
     └──┬──────────┬───┘   └──┬──────────┬──┘
       Yes        No         Yes        No
        │          │          │          │
  Reprocess   IOException  Reprocess   Log warning,
  ALL pages   (propagate)  FAILED      skip failed
  via Java               pages only    pages
```

---

## 6. Python Hybrid Server

### 6.1 FastAPI Endpoints

```
GET  /health              → {"status": "ok"}
POST /v1/convert/file     → conversion response
```

### 6.2 Server CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--host` | `0.0.0.0` | Bind address |
| `--port` | `5001` or `5002` | Listen port |
| `--force-ocr` | false | Force full-page OCR |
| `--ocr-lang` | `["en"]` | EasyOCR languages (comma-separated) |
| `--enrich-formula` | false | Enable formula enrichment |
| `--enrich-picture-description` | false | Enable SmolVLM picture description |
| `--picture-description-prompt` | (built-in) | Custom VLM prompt |

### 6.3 Docling SDK Configuration

```python
PdfPipelineOptions(
    do_ocr=True,
    do_table_structure=True,
    ocr_options=EasyOcrOptions(
        force_full_page_ocr=force_ocr,
        lang=ocr_lang_list,
    ),
    table_structure_options=TableStructureOptions(
        mode=TableFormerMode.ACCURATE,
    ),
    do_formula_enrichment=enrich_formula,
    do_picture_description=enrich_picture_description,
    generate_picture_images=enrich_picture_description,
    picture_description_options=PictureDescriptionVlmOptions(
        repo_id="HuggingFaceTB/SmolVLM-256M-Instruct",
        prompt="Describe what you see in this image...",
        generation_config={"max_new_tokens": 300, "do_sample": False},
    ),
)
```

### 6.4 Enrichment Requirements

| Feature | Requires |
|---------|----------|
| Formula enrichment | `--hybrid-mode full` on client side |
| Picture description | `--hybrid-mode full` on client side |
| EasyOCR | Server-side only, no client flag needed |

**Important**: Enrichments only run on the backend. If `--hybrid-mode auto`, enriched results are only available for pages triaged to BACKEND.

### 6.5 Max File Size

```python
MAX_FILE_SIZE = 100 * 1024 * 1024  # 100 MB
```

### 6.6 Unicode Sanitization

```python
def sanitize_unicode(obj):
    """Recursively replace lone surrogates (U+D800–U+DFFF) and null chars with U+FFFD"""
    pattern = re.compile(r'[\ud800-\udfff\x00]')
    # Applied recursively to all strings in response dict
```

---

## 7. Client Factory

```rust
pub struct HybridClientFactory {
    clients: HashMap<String, Box<dyn HybridClient>>,
}

impl HybridClientFactory {
    pub fn get_or_create(&mut self, backend: &str, config: &HybridConfig)
        -> Result<&dyn HybridClient, HybridError>;
    pub fn shutdown(&mut self);
}
```

Supported backends:
| Backend | Client | Default URL |
|---------|--------|-------------|
| `docling-fast` | `DoclingFastServerClient` | `http://localhost:5002` |
| `hancom` | `HancomClient` | `https://dataloader.cloud.hancom.com/studio-lite/api` |
| `azure` | — | Not yet implemented |
| `google` | — | Not yet implemented |

---

## 8. Connection Management

OkHttp-based (Java). For Rust, use `reqwest`:

```rust
pub struct DoclingClient {
    client: reqwest::Client,
    base_url: String,
    timeout: Duration,
}

impl DoclingClient {
    pub fn new(url: &str, timeout_ms: u32) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(timeout_ms as u64))
            .build()
            .unwrap();
        Self { client, base_url: url.to_string(), timeout: Duration::from_millis(timeout_ms as u64) }
    }

    pub async fn check_availability(&self) -> Result<(), HybridError> {
        let resp = self.client
            .get(format!("{}/health", self.base_url))
            .timeout(Duration::from_millis(3000))
            .send().await?;
        if resp.status().is_success() { Ok(()) }
        else { Err(HybridError::Unavailable) }
    }

    pub async fn convert(&self, request: HybridRequest) -> Result<HybridResponse, HybridError> {
        let mut form = reqwest::multipart::Form::new()
            .part("files", reqwest::multipart::Part::bytes(request.pdf_bytes)
                .file_name("document.pdf")
                .mime_str("application/pdf")?);

        if let Some(range) = request.page_range_str() {
            form = form.text("page_ranges", range);
        }

        let resp = self.client
            .post(format!("{}/v1/convert/file", self.base_url))
            .multipart(form)
            .send().await?;

        parse_response(resp).await
    }
}
```
