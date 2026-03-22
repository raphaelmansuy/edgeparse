# EdgeParse — 20-Stage Processing Pipeline

> Every stage listed here exists as a real function called in
> [`orchestrator.rs`](../crates/edgeparse-core/src/pipeline/orchestrator.rs).
> Stage numbers follow the orchestrator source comments.

---

## Pipeline Overview

```
       PDF bytes
           │
           ▼
     ┌──────────────────────────────────────────────────────────────────┐
     │  PRE-PIPELINE: PDF Loading & Chunk Extraction                    │
     │                                                                  │
     │  loader::load_pdf()          → RawPdfDocument                   │
     │  page_info::extract_page_info() → Vec<PageInfo>                 │
     │  chunk_parser::extract_page_chunks() → PageChunks per page      │
     └──────────────────────────────────────────────────────────────────┘
           │
           ▼  Vec<ContentElement> per page  (text + image + line chunks)
     ┌──────────────────────────────────────────────────────────────────┐
     │  LAYER 1: Safety & Filtering                                     │
     │                                                                  │
     │  Stage 0b  Page Range Filtering      page_range::filter_pages() │
     │  Stage 1b  Watermark Removal         watermark_detector         │
     │  Stage 2   Content Filtering         content_filter             │
     │  Stage 2b  Replace U+FFFD            replace_fffd_in_element()  │
     └──────────────────────────────────────────────────────────────────┘
           │
           ▼  Cleaned TextChunk / Image / Line elements
     ┌──────────────────────────────────────────────────────────────────┐
     │  LAYER 2: Table Detection                                        │
     │                                                                  │
     │  Stage 3-4  Border Table Detection   table_detector             │
     │  Stage 4b   Content → Table Cells    table_content_assigner     │
     │  Stage 4b2  Filter Empty Tables      table_detector             │
     │  Stage 4c   Boxed Heading Promoter   boxed_heading_promoter     │
     │  Stage 4d   Pre-Cluster Table Release table_detector            │
     └──────────────────────────────────────────────────────────────────┘
           │
           ▼  TableBorder elements + free TextChunks
     ┌──────────────────────────────────────────────────────────────────┐
     │  LAYER 3: Grouping                                               │
     │                                                                  │
     │  Stage 5b   Column Detection         column_detector            │
     │  Stage 6    TextChunk → TextLine     text_line_grouper          │
     │  Stage 6b   Re-run Column Detection  column_detector            │
     │  Stage 6.5  List Detection Pass 1    list_detector              │
     │  Stage 7    TextLine → TextBlock     text_block_grouper         │
     │  Stage 7b   Cluster Table Detection  cluster_table_detector     │
     │  Stage 7b2  Suspicious Table Filter  table_detector             │
     └──────────────────────────────────────────────────────────────────┘
           │
           ▼  TextBlock / TextLine / TableBorder / List
     ┌──────────────────────────────────────────────────────────────────┐
     │  LAYER 4: Semantic Classification                                │
     │                                                                  │
     │  Stage 8    Header/Footer Detection  header_footer (cross-page) │
     │  Stage 9    List Detection Pass 1    list_detector              │
     │  Stage 10   Paragraph Detection      paragraph_detector         │
     │  Stage 10b  Figure Detection         figure_detector            │
     │  Stage 12   Heading Detection        heading_detector           │
     └──────────────────────────────────────────────────────────────────┘
           │
           ▼  Heading / Paragraph / Figure / List / Table / HeaderFooter
     ┌──────────────────────────────────────────────────────────────────┐
     │  LAYER 5: Linking & Ordering                                     │
     │                                                                  │
     │  Stage 18-pre  Reading Order (pre-pass)  reading_order          │
     │  Stage 11      List Detection Pass 2     list_pass2             │
     │  Stage 11b     Common-prefix Lists       list_pass2             │
     │  Stage 13      ID Assignment             id_assignment          │
     │  Stage 14      Caption Linking           caption_linker         │
     │  Stage 14b     Footnote Detection        footnote_detector      │
     │  Stage 14c     TOC Detection             toc_detector           │
     │  Stage 15      Cross-Page Table Linking  cross_page_linker      │
     │  Stage 17      Nesting Level Assignment  nesting_level          │
     │  Stage 18      Final Reading Order       reading_order          │
     │  Stage 19      Content Sanitization      content_sanitizer      │
     └──────────────────────────────────────────────────────────────────┘
           │
           ▼  PipelineState.pages (fully classified, ordered)
     ┌──────────────────────────────────────────────────────────────────┐
     │  POST-PIPELINE: PdfDocument Assembly  (lib.rs#L103)             │
     └──────────────────────────────────────────────────────────────────┘
```

---

## Stage-by-Stage Reference

### Stage 0b — Page Range Filtering

**Source:** [`utils/page_range.rs`](../crates/edgeparse-core/src/utils/page_range.rs)
**Called from:** [`orchestrator.rs`](../crates/edgeparse-core/src/pipeline/orchestrator.rs) (line ~140)

```
Input:  config.pages = Some("1,3,5-7")
Action: parse_page_range() → BTreeSet<usize>
        filter_pages(pages, &selected) → keep only selected pages
Output: state.pages shrunk to selected pages
```

**Trigger:** Only runs when `config.pages` is `Some`. Supports comma-separated numbers and ranges: `"1,3-5,7"`.

---

### Stage 1b — Watermark Removal

**Source:** [`pipeline/stages/watermark_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/watermark_detector.rs)
**Parallelism:** Sequential (uses `&mut state.pages` across all pages)

```
Input:  Vec<ContentElement>  (all page elements)
Action: Detect repeated or low-confidence text across pages
        Mark or remove watermark candidates
Output: Same Vec with watermark elements removed/ignored
```

Watermarks typically appear as: large text at center, very low contrast, or repeated identically across all pages.

---

### Stage 2 — Content Filtering

**Source:** [`pipeline/stages/content_filter.rs`](../crates/edgeparse-core/src/pipeline/stages/content_filter.rs)
**Parallelism:** `par_map_pages_indexed` (one filter run per page, parallel)
**Config:** [`api/filter.rs`](../crates/edgeparse-core/src/api/filter.rs)

```
FilterConfig flags applied:
  filter_hidden_text  → drop chunks where contrast_ratio < threshold
  filter_out_of_page  → drop chunks outside CropBox
  filter_tiny_text    → drop chunks below minimum height
  filter_hidden_ocg   → drop chunks where ocg_visible == false
```

Page geometry (`CropBox`) is provided by `PageInfo` resolved via `state.page_info.get(page_idx)`.

---

### Stage 2b — Replace U+FFFD

**Source:** [`orchestrator.rs` — `replace_fffd_in_element()`](../crates/edgeparse-core/src/pipeline/orchestrator.rs)
**Parallelism:** `par_map_pages`

Replaces Unicode replacement characters (`\u{FFFD}`) in `TextChunk.value` with `config.replace_invalid_chars` (default: `" "`). Only acts on `ContentElement::TextChunk` variants (other types don't exist yet at this stage).

---

### Stages 3–4 — Border Table Detection

**Source:** [`pipeline/stages/table_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/table_detector.rs)
**Parallelism:** `par_map_pages`

```
Input:  Vec<ContentElement>  containing LineChunk elements
Action: Group collinear horizontal/vertical line segments into grid cells
        Build TableBorder{x_coordinates, y_coordinates, rows[cells]}
Output: Vec<ContentElement>  with LineChunk → TableBorder promotions
```

**Algorithm sketch:**
```
1. Collect all LineChunk elements
2. Cluster horizontal lines by Y-coordinate (± epsilon)
3. Cluster vertical lines by X-coordinate (± epsilon)
4. Build grid from intersections → (N rows × M cols)
5. Create TableBorder with x_coordinates, y_coordinates
6. Remove the constituent LineChunks from page
```

**Key data type:** [`models/table.rs — TableBorder`](../crates/edgeparse-core/src/models/table.rs)

---

### Stage 4b — Content Assignment to Table Cells

**Source:** [`pipeline/stages/table_content_assigner.rs`](../crates/edgeparse-core/src/pipeline/stages/table_content_assigner.rs)
**Parallelism:** `par_map_pages`

```
Input:  Page with TableBorder + free TextChunk/Image elements
Action: For each TableBorder, for each cell:
          compute intersection(chunk.bbox, cell.bbox) ≥ MIN_CELL_CONTENT_INTERSECTION_PERCENT
          assign matching chunks to cell.text_chunks
Output: TableBorder.rows[i].cells[j].text_chunks populated
        Assigned elements removed from free pool
```

**Constant:** `MIN_CELL_CONTENT_INTERSECTION_PERCENT = 0.01` in [`models/table.rs`](../crates/edgeparse-core/src/models/table.rs#L14)

---

### Stage 4b2 — Filter Empty Tables (Chart Grid FPs)

**Source:** [`table_detector::filter_empty_tables()`](../crates/edgeparse-core/src/pipeline/stages/table_detector.rs)

Removes `TableBorder` elements where most cells are empty — these are typically chart grid lines rendered as bordered rectangles, not actual data tables.

---

### Stage 4c — Boxed Heading Promoter

**Source:** [`pipeline/stages/boxed_heading_promoter.rs`](../crates/edgeparse-core/src/pipeline/stages/boxed_heading_promoter.rs)
**Parallelism:** `par_map_pages`

Single-cell tables containing short heading-like text are dissolved back into free `TextChunk` elements so `heading_detector` can classify them properly.

```
TableBorder(1 row × 1 col, short text) → TextChunk (released)
```

---

### Stage 4d — Pre-Cluster Table Release

**Source:** [`table_detector::release_pre_cluster_tables()`](../crates/edgeparse-core/src/pipeline/stages/table_detector.rs)

Releases page-wide single-cell pseudo-tables (tables that span the full width of a page and likely result from layout artefacts) back into the free text flow before the cluster detector runs.

---

### Stage 5b — Column Detection

**Source:** [`pipeline/stages/column_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/column_detector.rs)
**Parallelism:** Operates on `&mut state.pages`, returns `Vec<Option<ColumnLayout>>`

```
Input:  Vec<ContentElement> per page
Action: Analyse X-coordinate distribution of elements
        Detect vertical gap zones → column boundaries
Output: ColumnLayout per page (passed to Stage 6)
```

Column layouts are used by `text_line_grouper` to prevent grouping chunks across column boundaries.

---

### Stage 6 — TextChunk → TextLine Grouping

**Source:** [`pipeline/stages/text_line_grouper.rs`](../crates/edgeparse-core/src/pipeline/stages/text_line_grouper.rs)
**Parallelism:** `par_map_pages_indexed`

```
Input:  Vec<ContentElement::TextChunk>
Action: Group chunks that share the same baseline (± slant tolerance)
        Within column boundaries
        Insert space between chunks when gap > fontSize * 0.17
Output: Vec<ContentElement::TextLine>
```

**Key type:** [`models/text.rs — TextLine`](../crates/edgeparse-core/src/models/text.rs)

`TextLine.value()` reconstructs text by calling `needs_space(prev, curr)` to re-insert word spaces from bounding box gaps.

---

### Stage 6b — Re-run Column Detection on TextLines

Second pass of column detection, now operating on formed `TextLine` elements for more stable geometry.

---

### Stage 6.5 — List Detection Pass 1 (TextLine Level)

**Source:** [`pipeline/stages/list_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/list_detector.rs)
**Parallelism:** `par_map_pages`

Detects list patterns at the `TextLine` level **before** block grouping. Catches bibliography entries (`[N]` bracket notation) and other list patterns that might be merged by Stage 7.

---

### Stage 7 — TextLine → TextBlock Grouping

**Source:** [`pipeline/stages/text_block_grouper.rs`](../crates/edgeparse-core/src/pipeline/stages/text_block_grouper.rs)
**Parallelism:** `par_map_pages`

```
Input:  Vec<ContentElement::TextLine>
Action: Group consecutive TextLines that:
        - share similar X-extent (left margin, right margin)
        - have consistent line spacing
        - belong to the same column
Output: Vec<ContentElement::TextBlock>
```

**Key type:** [`models/text.rs — TextBlock`](../crates/edgeparse-core/src/models/text.rs)

---

### Stage 7b — Cluster (Borderless) Table Detection

**Source:** [`pipeline/stages/cluster_table_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/cluster_table_detector.rs)
**Parallelism:** `par_map_pages`

Detects tables that have **no visible borders** by clustering `TextBlock` elements with regular X/Y spacing patterns into a `TableBorder` grid. Only active when `config.table_method == TableMethod::Cluster`.

---

### Stage 7b2 — Suspicious Table Filter

Rejects table-shaped layout artefacts from both border and cluster detectors, releasing their text back into the page flow.

---

### Stage 8 — Header / Footer Detection

**Source:** [`pipeline/stages/header_footer.rs`](../crates/edgeparse-core/src/pipeline/stages/header_footer.rs)
**Parallelism:** Sequential (cross-page comparison)

```
Input:  All pages + median page_height
Action: Elements in top/bottom N% of page height that repeat across pages →
        classify as Header or Footer
Output: ContentElement::TextBlock → ContentElement::HeaderFooter
```

Uses `page_height` (median from `state.page_info`) for threshold calculation.

---

### Stage 9 — List Detection Pass 1 (Block Level)

Second application of `list_detector::detect_lists`. Catches numbered list patterns in `TextBlock` elements that the block grouper may have split.

---

### Stage 10 — Paragraph Detection

**Source:** [`pipeline/stages/paragraph_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/paragraph_detector.rs)
**Parallelism:** `par_map_pages`

```
Input:  TextBlock elements
Action: Classify TextBlock → SemanticParagraph
        Assign indentation level
        Set enclosed_top / enclosed_bottom flags
Output: ContentElement::Paragraph
```

**Key type:** [`models/semantic.rs — SemanticParagraph`](../crates/edgeparse-core/src/models/semantic.rs#L89)

---

### Stage 10b — Figure Detection

**Source:** [`pipeline/stages/figure_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/figure_detector.rs)
**Parallelism:** `par_map_pages`

```
Input:  Image and LineArt elements
Action: Group nearby ImageChunk/LineArtChunk into SemanticFigure
Output: ContentElement::Figure
```

---

### Stage 12 — Heading Detection

**Source:** [`pipeline/stages/heading_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/heading_detector.rs)
**Parallelism:** Sequential (uses `mcid_map` for tagged PDFs, cross-page font analysis)

```
Input:  SemanticParagraph elements + optional McidMap
Signals used:
  A. Structure tree tag (McidMap): H/H1-H6 → direct classification
  B. Font size relative to body text (dominant page font)
  C. Font weight (bold) / italic angle
  D. Location on page (near top = more likely heading)
  E. Text length (short, no terminal punctuation)
  F. Numeric prefix pattern ("1.2.3 Section Title")
Output: ContentElement::Heading or ContentElement::NumberHeading
        with heading_level: Option<u32>  (1-6)
```

**McidMap key:** `(page_number: u32, mcid: i64)` → `McidTagInfo{role, heading_level, struct_type}`
**Source:** [`tagged/struct_tree.rs`](../crates/edgeparse-core/src/tagged/struct_tree.rs)

---

### Stage 18-pre — Reading Order Pre-pass

First application of XY-Cut++ sorting — runs before List Pass 2 so elements are in correct reading order for sequential list detection.

**Source:** [`pipeline/stages/reading_order.rs`](../crates/edgeparse-core/src/pipeline/stages/reading_order.rs)
**Algorithm:** [`utils/xycut.rs`](../crates/edgeparse-core/src/utils/xycut.rs)

---

### Stage 11 — List Detection Pass 2 (Paragraph Level)

**Source:** [`pipeline/stages/list_pass2.rs`](../crates/edgeparse-core/src/pipeline/stages/list_pass2.rs)
**Parallelism:** `par_map_pages`

Detects list patterns in classified `Paragraph` elements. Works on body text that contains bullet indicators, dash-prefixes, or numbered items.

---

### Stage 11b — Document-Level Common-Prefix Lists

**Source:** [`list_pass2::detect_common_prefix_lists_document()`](../crates/edgeparse-core/src/pipeline/stages/list_pass2.rs)
**Parallelism:** Sequential (operates across all pages)

Identifies patterns like "Figure N …" or "Table N …" that repeat across the document and promotes them to `List` elements.

---

### Stage 13 — ID Assignment

**Source:** [`pipeline/stages/id_assignment.rs`](../crates/edgeparse-core/src/pipeline/stages/id_assignment.rs)
**Parallelism:** Sequential (global counter across all pages)

Assigns a monotonically increasing `index: u32` to every `ContentElement`. Used by renderers and the cross-page linker.

```
Input:  state.pages — unindexed elements
Action: Traverse all pages in order, call elem.set_index(counter++)
Output: All elements have unique index values
```

---

### Stage 14 — Caption Linking

**Source:** [`pipeline/stages/caption_linker.rs`](../crates/edgeparse-core/src/pipeline/stages/caption_linker.rs)

Links `Caption` elements to the nearest preceding `Figure` or `TableBorder` using spatial proximity and text prefix patterns ("Figure N", "Table N", etc.).

Sets `SemanticCaption.linked_content_id` to the index of the linked element.

---

### Stage 14b — Footnote Detection

**Source:** [`pipeline/stages/footnote_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/footnote_detector.rs)

Detects footnotes by:
- Position at bottom of page
- Small font size relative to body text
- Numeric or symbolic prefix (1, *, †)

---

### Stage 14c — TOC Detection

**Source:** [`pipeline/stages/toc_detector.rs`](../crates/edgeparse-core/src/pipeline/stages/toc_detector.rs)

Detects Table of Contents sections using leader dot patterns, right-aligned page numbers, and section-number prefixes. Promotes matching elements to `SemanticType::TableOfContent`.

---

### Stage 15 — Cross-Page Table Linking

**Source:** [`pipeline/stages/cross_page_linker.rs`](../crates/edgeparse-core/src/pipeline/stages/cross_page_linker.rs)
**Parallelism:** Sequential

Links `TableBorder` elements that span across page boundaries. Sets `TableBorder.previous_table` / `next_table` Box pointers. `BoundingBox.last_page_number` is updated to reflect the true extent.

---

### Stage 17 — Nesting Level Assignment

**Source:** [`pipeline/stages/nesting_level.rs`](../crates/edgeparse-core/src/pipeline/stages/nesting_level.rs)

Assigns `level: Option<String>` to each element based on its position in the semantic hierarchy (heading level, list nesting, etc.).

---

### Stage 18 — Final Reading Order Sorting

**Source:** [`pipeline/stages/reading_order.rs`](../crates/edgeparse-core/src/pipeline/stages/reading_order.rs)
**Algorithm:** [`utils/xycut.rs — xycut_sort()`](../crates/edgeparse-core/src/utils/xycut.rs#L24)

```
XY-Cut++ Algorithm:
1. Find largest horizontal gap → candidate split_y
2. Find largest vertical gap  → candidate split_x
3. Prefer vertical (column) split when:
   - Both gaps exist AND
   - ≥ 2 elements on each side of vertical gap AND
   - < 55% of elements span the full width (not full-width headings)
4. Split by whichever wins
5. Recurse on each partition
6. If no split found: sort by quantized Y bucket (4 pt) then left_x
```

The 4-point Y-bucket prevents column-order reversal when elements in adjacent columns have slightly different Y coordinates due to PDF rounding.

---

### Stage 19 — Content Sanitization

**Source:** [`pipeline/stages/content_sanitizer.rs`](../crates/edgeparse-core/src/pipeline/stages/content_sanitizer.rs)
**Config:** `config.sanitize: bool`

When `sanitize = true`, applies PII removal patterns (email addresses, phone numbers, SSNs, etc.) using regex-based replacement.

**Source regexes:** [`utils/sanitizer.rs`](../crates/edgeparse-core/src/utils/sanitizer.rs)

---

## Execution Timeline Visualisation

```
Time ──────────────────────────────────────────────────────────────▶

Stage 0b   [─]  (filter by page range — very fast)
Stage 1b   [─────]  (scalar pass, cross-page)
Stage 2    [══════════════]  (parallel, per page)
Stage 2b   [══════]  (parallel, per page)
Stage 3-4  [══════════════════]  (parallel, table geometry)
Stage 4b   [══════════════]  (parallel)
Stage 4b2  [══════]  (parallel)
Stage 4c   [══════]  (parallel)
Stage 4d   [══════]  (parallel)
Stage 5b   [──────────]  (scalar, layout analysis)
Stage 6    [══════════════════]  (parallel, line grouping)
Stage 6b   [──────────]  (scalar)
Stage 6.5  [══════════]  (parallel)
Stage 7    [══════════════════]  (parallel, block grouping)
Stage 7b   [══════════════]  (parallel, cluster tables)
Stage 7b2  [══════]  (parallel)
Stage 8    [──────────]  (scalar, cross-page)
Stage 9    [══════════]  (parallel)
Stage 10   [══════════════]  (parallel)
Stage 10b  [══════]  (parallel)
Stage 12   [──────────────]  (scalar, cross-page+tagged)
Stage 18p  [──────────]  (scalar, pre-pass sort)
Stage 11   [══════════]  (parallel)
Stage 11b  [──────]  (scalar)
Stage 13   [──────]  (scalar, global counter)
Stage 14   [──────]  (scalar)
Stage 14b  [══════]  (parallel)
Stage 14c  [══════]  (parallel)
Stage 15   [──────]  (scalar, cross-page)
Stage 17   [══════]  (parallel)
Stage 18   [──────────]  (scalar, final sort)
Stage 19   [══════════]  (parallel, optional)

══ = par_map_pages (Rayon parallel)
── = sequential (cross-page or ordering constraint)
```

---

## Cross-Reference

| Topic | Document |
|-------|---------|
| Architecture & modules | [01-architecture.md](01-architecture.md) |
| Data types used in stages | [03-data-model.md](03-data-model.md) |
| PDF chunk extraction (pre-pipeline) | [04-pdf-extraction.md](04-pdf-extraction.md) |
| Reading order (XY-Cut++) detail | [03-data-model.md#xycut](03-data-model.md) |
| Output after pipeline | [05-output-formats.md](05-output-formats.md) |
