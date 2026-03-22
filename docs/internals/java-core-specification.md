# OpenDataLoader PDF — Java Core Module Specification

> Comprehensive specification of `java/opendataloader-pdf-core/src/main/java/` for rewriting the codebase in Rust.
>
> **Copyright**: 2025-2026 Hancom Inc., Apache License 2.0
> **PDF Engine**: veraPDF library (`org.verapdf.*`)
> **JSON**: Jackson (`com.fasterxml.jackson`)
> **HTTP**: OkHttp (`okhttp3.*`)
> **Tagged PDF Output**: Apache PDFBox (`org.apache.pdfbox.*`)

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Package Structure](#2-package-structure)
3. [Content Model & Type Hierarchy](#3-content-model--type-hierarchy)
4. [Processing Pipeline](#4-processing-pipeline)
5. [Package: `api` — Public API & Configuration](#5-package-api)
6. [Package: `containers` — Thread-Local State](#6-package-containers)
7. [Package: `entities` — Custom Types](#7-package-entities)
8. [Package: `processors` — Core Processing Pipeline](#8-package-processors)
9. [Package: `processors.readingorder` — XY-Cut++ Algorithm](#9-package-readingorder)
10. [Package: `hybrid` — AI Backend Integration](#10-package-hybrid)
11. [Package: `markdown` — Markdown Output](#11-package-markdown)
12. [Package: `html` — HTML Output](#12-package-html)
13. [Package: `json` — JSON Output](#13-package-json)
14. [Package: `pdf` — Annotated PDF Output](#14-package-pdf)
15. [Package: `text` — Plain Text Output](#15-package-text)
16. [Package: `utils` — Utility Classes](#16-package-utils)
17. [Key Constants & Thresholds](#17-key-constants--thresholds)
18. [External Dependencies from veraPDF](#18-external-dependencies-from-verapdf)
19. [Rust Rewrite Considerations](#19-rust-rewrite-considerations)

---

## 1. Architecture Overview

OpenDataLoader PDF converts PDF documents into structured output formats (Markdown, HTML, JSON, plain text, annotated PDF). The architecture follows a **pipeline pattern**:

```
PDF File
  ↓
[veraPDF Parser] → PDDocument, parseChunks(), LinesPreprocessingConsumer
  ↓
[Content Filtering] → remove tiny/hidden/duplicate text, merge close chunks
  ↓
[Table Detection] → border-based (LineChunks) + cluster-based (spatial)
  ↓
[Text Processing] → group TextChunks → TextLines, paragraphs, headings
  ↓
[List Detection] → label pattern matching (bullets, numbers, Korean)
  ↓
[Header/Footer Detection] → repeating content across pages
  ↓
[Caption Linking] → associate captions with images/tables
  ↓
[Level Assignment] → hierarchical nesting (headings, lists, tables)
  ↓
[Reading Order Sorting] → XY-Cut++ algorithm
  ↓
[Content Safety] → regex-based PII sanitization
  ↓
[Output Generation] → Markdown, HTML, JSON, Text, Annotated PDF
```

**Three processing paths exist:**
1. **Normal (Java-only)**: Full pipeline above
2. **Tagged PDF (struct tree)**: Walks PDF structure tree, maps tags to IObject types
3. **Hybrid**: Routes pages to Java or external AI backend based on triage signals

---

## 2. Package Structure

```
org.opendataloader.pdf/
├── api/                           # Public API (3 files)
│   ├── OpenDataLoaderPDF.java     # Main entry point
│   ├── Config.java                # Central configuration (~650 lines)
│   └── FilterConfig.java          # Content safety / PII filter config
├── containers/                    # Thread-local state (1 file)
│   └── StaticLayoutContainers.java
├── entities/                      # Custom content types (2 files)
│   ├── SemanticFormula.java       # LaTeX formula
│   └── SemanticPicture.java       # Picture with AI description
├── processors/                    # Core pipeline (16 files)
│   ├── DocumentProcessor.java     # Main orchestrator (~330 lines)
│   ├── ContentFilterProcessor.java
│   ├── TextProcessor.java
│   ├── TextLineProcessor.java
│   ├── ParagraphProcessor.java    # ~500 lines
│   ├── HeadingProcessor.java      # ~200 lines
│   ├── HeaderFooterProcessor.java # ~300 lines
│   ├── ListProcessor.java         # ~500 lines
│   ├── TableBorderProcessor.java  # ~280 lines
│   ├── ClusterTableProcessor.java # ~100 lines
│   ├── AbstractTableProcessor.java
│   ├── SpecialTableProcessor.java # Korean-style tables
│   ├── CaptionProcessor.java
│   ├── HiddenTextProcessor.java
│   ├── LevelProcessor.java
│   ├── TaggedDocumentProcessor.java # ~330 lines
│   └── readingorder/
│       └── XYCutPlusPlusSorter.java # ~550 lines
├── hybrid/                        # AI backend integration (10 files)
│   ├── HybridClient.java         # Interface + DTOs
│   ├── HybridClientFactory.java  # Factory with caching
│   ├── HybridConfig.java         # URL, timeout, mode config
│   ├── HybridSchemaTransformer.java # Interface
│   ├── HybridDocumentProcessor.java # ~400 lines
│   ├── DoclingFastServerClient.java # ~308 lines
│   ├── DoclingSchemaTransformer.java # ~592 lines
│   ├── HancomClient.java          # ~305 lines
│   ├── HancomSchemaTransformer.java # ~532 lines
│   ├── TriageProcessor.java       # ~1123 lines
│   └── TriageLogger.java          # ~225 lines
├── markdown/                      # Markdown output (4 files)
│   ├── MarkdownGenerator.java     # ~350 lines
│   ├── MarkdownHTMLGenerator.java # Subclass for HTML tables
│   ├── MarkdownGeneratorFactory.java
│   └── MarkdownSyntax.java       # Constants
├── html/                          # HTML output (3 files)
│   ├── HtmlGenerator.java        # ~470 lines
│   ├── HtmlGeneratorFactory.java
│   └── HtmlSyntax.java           # Constants
├── json/                          # JSON output (3 + 18 serializers)
│   ├── JsonWriter.java           # ~100 lines
│   ├── JsonName.java             # Field name constants
│   ├── ObjectMapperHolder.java   # Jackson ObjectMapper + modules
│   └── serializers/              # 18 Jackson serializers
│       ├── SerializerUtil.java   # Shared write helpers
│       ├── TableSerializer.java
│       ├── TableRowSerializer.java
│       ├── TableCellSerializer.java
│       ├── HeadingSerializer.java
│       ├── ParagraphSerializer.java
│       ├── CaptionSerializer.java
│       ├── ListSerializer.java
│       ├── ListItemSerializer.java
│       ├── ImageSerializer.java
│       ├── PictureSerializer.java
│       ├── FormulaSerializer.java
│       ├── HeaderFooterSerializer.java
│       ├── SemanticTextNodeSerializer.java
│       ├── TextChunkSerializer.java
│       ├── TextLineSerializer.java
│       ├── LineChunkSerializer.java
│       └── DoubleSerializer.java
├── pdf/                           # Annotated PDF output (2 files)
│   ├── PDFWriter.java            # ~350 lines (Apache PDFBox)
│   └── PDFLayer.java             # Enum: CONTENT, TABLE_CELLS, etc.
├── text/                          # Plain text output (1 file)
│   └── TextGenerator.java        # ~250 lines
└── utils/                         # Utilities (8 + 5 files)
    ├── ContentSanitizer.java      # PII regex redaction (~300 lines)
    ├── SanitizationRule.java      # Pattern + replacement
    ├── ImagesUtils.java           # Image extraction
    ├── Base64ImageUtils.java      # Base64 data URI conversion
    ├── BulletedParagraphUtils.java # Bullet/label detection (~200 lines)
    ├── ModeWeightStatistics.java  # Font mode/rarity analysis
    ├── TextNodeStatistics.java    # Heading scoring statistics
    ├── TextNodeStatisticsConfig.java # Scoring thresholds
    └── levels/                    # Hierarchical level types
        ├── LevelInfo.java         # Base class
        ├── ListLevelInfo.java
        ├── TableLevelInfo.java
        ├── LineArtBulletParagraphLevelInfo.java
        └── TextBulletParagraphLevelInfo.java
```

**Total: ~75 Java source files**

---

## 3. Content Model & Type Hierarchy

All content elements implement the `IObject` interface from veraPDF. The hierarchy:

```
IObject (interface) — has: BoundingBox, pageNumber, recognizedStructureId, level
├── BaseObject (abstract) — basic IObject impl
│   ├── SemanticFormula            ← CUSTOM: LaTeX formula
│   └── SemanticPicture            ← CUSTOM: image with description
├── INode (interface) — adds: SemanticType, children
│   ├── SemanticTextNode           — text with TextColumns/TextBlocks/TextLines
│   │   ├── SemanticParagraph
│   │   ├── SemanticHeading        — adds: headingLevel (int)
│   │   ├── SemanticCaption        — adds: linkedContentId (Long)
│   │   └── SemanticSpan
│   ├── SemanticHeaderOrFooter     — contains: List<IObject> contents
│   └── SemanticFigure
├── TextChunk                      — single text run: value, fontName, fontSize, fontWeight, baseLine, textColor
├── TextLine                       — ordered list of TextChunks, optional connectedLineArtLabel
├── TextBlock                      — list of TextLines, firstLineIndent
├── TextColumn                     — list of TextBlocks
├── ImageChunk                     — image: index, BoundingBox
├── LineChunk                      — vector line segment
├── LineArtChunk                   — complex vector graphic
├── TableBorder                    — table: rows[], columns, previousTableId, nextTableId, isTextBlock
│   ├── TableBorderRow             — cells[]
│   └── TableBorderCell            — rowNumber, colNumber, rowSpan, colSpan, contents[]
├── PDFList                        — list items, previousListId, nextListId, commonPrefix, numberingStyle
│   └── ListItem                   — text lines, contents[]
└── ITree (interface)              — structure tree (for tagged PDFs)
```

### SemanticType Enum (from veraPDF)
```
HEADING, PARAGRAPH, LIST, LIST_ITEM, TABLE, TABLE_ROW, TABLE_CELL,
TABLE_HEADER, TABLE_BODY, TABLE_FOOTER, TABLE_HEADERS,
TABLE_OF_CONTENT, CAPTION, HEADER, FOOTER, TITLE, NUMBER_HEADING,
FIGURE, PART, SPAN
```

### BoundingBox
- Fields: `leftX`, `bottomY`, `rightX`, `topY`, `pageNumber`, `lastPageNumber`
- Coordinate system: **BOTTOMLEFT origin** (PDF standard)
- Subclass: `MultiBoundingBox` for multi-page elements

---

## 4. Processing Pipeline

### Main Entry Point
```
OpenDataLoaderPDF.processFile(inputPdfName, config)
  → DocumentProcessor.processFile(inputPdfName, config)
```

### DocumentProcessor.processFile() Flow

```
1. preprocessing(inputFile, config)
   ├── StaticContainers.init(config)         // veraPDF globals
   ├── StaticLayoutContainers.clearContainers()
   ├── PDDocument = veraPDF.open(file)       // GFSAPDFDocument
   ├── document.parseChunks()               // Extract TextChunks, ImageChunks, LineChunks
   └── LinesPreprocessingConsumer.findTableBorders()  // Detect table borders from lines

2. calculateDocumentInfo()                   // author, title, dates
3. getValidPageNumbers(config)               // Parse page ranges "1,3,5-7"
4. processDocument() — one of three paths:
   A. useStructTree → TaggedDocumentProcessor
   B. hybrid != "off" → HybridDocumentProcessor
   C. else → normalProcessDocument()

5. sortContents()
   └── if readingOrder=="xycut" → XYCutPlusPlusSorter.sort()

6. sanitizeContents()
   └── ContentSanitizer.sanitizeContents()

7. generateOutputs()
   ├── ImagesUtils.write()          // Extract images to files
   ├── PDFWriter.updatePDF()        // Annotated PDF
   ├── JsonWriter.writeToJson()     // JSON output
   ├── MarkdownGenerator.writeToMarkdown()  // Markdown
   ├── HtmlGenerator.writeToHtml()  // HTML
   └── TextGenerator.writeToText()  // Plain text
```

### Normal Processing Pipeline (per page)

```
ContentFilterProcessor.getFilteredContents()
  ├── removeSameTextChunks()
  ├── removeTextDecorationImages()
  ├── filterTinyText()
  ├── filterOutOfPage()
  ├── mergeCloseTextChunks()
  ├── trimWhiteSpaces()
  ├── filterConsecutiveSpaces()
  ├── splitByWhiteSpaces()
  ├── findHiddenText()
  ├── replaceUndefinedChars()
  └── processBackgrounds()

ClusterTableProcessor (optional, if tableMethod=="cluster")
TableBorderProcessor.processTableBorders()
  └── Recursive cell processing: TextLineProcessor → ListProcessor → ParagraphProcessor → HeadingProcessor → CaptionProcessor

TextLineProcessor.processTextLines()
SpecialTableProcessor

--- cross-page ---
HeaderFooterProcessor
ListProcessor.processLists()

--- per-page again ---
ParagraphProcessor.processParagraphs()
ListProcessor.processListsFromTextNodes()
HeadingProcessor.processHeadings()
setIDs()
CaptionProcessor.processCaptions()

--- cross-page ---
checkNeighborLists()
checkNeighborTables()
detectHeadingsLevels()
LevelProcessor.detectLevels()
```

---

## 5. Package: `api`

### OpenDataLoaderPDF.java
- **Static** `processFile(String inputPdfName, Config config)` → delegates to `DocumentProcessor.processFile()`
- **Static** `shutdown()` → `HybridClientFactory.shutdown()` — closes cached HTTP clients

### Config.java (~650 lines)
Central configuration bean. Key fields:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `password` | String | `""` | PDF password |
| `isGenerateMarkdown` | boolean | `true` | Generate .md output |
| `isGenerateHtml` | boolean | `false` | Generate .html output |
| `isGenerateJSON` | boolean | `false` | Generate .json output |
| `isGeneratePDF` | boolean | `false` | Generate annotated .pdf |
| `isGenerateText` | boolean | `false` | Generate .txt output |
| `keepLineBreaks` | boolean | `false` | Preserve line breaks in text |
| `useStructTree` | boolean | `false` | Use PDF structure tree |
| `useHTMLInMarkdown` | boolean | `false` | Use HTML table tags in markdown |
| `addImageToMarkdown` | boolean | `false` | Include images in markdown |
| `replaceInvalidChars` | boolean | `true` | Replace undefined chars |
| `outputFolder` | String | `"."` | Output directory |
| `tableMethod` | String | `"default"` | `"default"` or `"cluster"` |
| `readingOrder` | String | `"off"` | `"off"` or `"xycut"` |
| `pages` | String | `null` | Page range: `"1,3,5-7"` |
| `imageOutput` | String | `"off"` | `"off"`, `"embedded"`, `"external"` |
| `imageFormat` | String | `"png"` | `"png"` or `"jpeg"` |
| `imageDir` | String | `null` | Custom image directory |
| `hybrid` | String | `"off"` | `"off"`, `"docling"`, `"docling-fast"`, `"hancom"`, `"azure"`, `"google"` |
| `hybridConfig` | HybridConfig | `null` | Hybrid backend config |
| `includeHeaderFooter` | boolean | `false` | Include headers/footers in output |
| `markdownPageSeparator` | String | `""` | Between-page separator (supports `%page-number%`) |
| `textPageSeparator` | String | `""` | Same for text output |
| `htmlPageSeparator` | String | `""` | Same for HTML output |

**Page range parsing**: `parsePageRanges(String pages)` → `Set<Integer>` (0-indexed internally)
- Formats: `"1"`, `"1,3,5"`, `"1-7"`, `"1,3,5-7"`

**Constants**:
- `PAGE_NUMBER_STRING = "%page-number%"`
- `IMAGE_FORMAT_PNG = "png"`, `IMAGE_FORMAT_JPEG = "jpeg"`

### FilterConfig.java
Content safety configuration:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `filterHiddenText` | boolean | `true` | Remove hidden text |
| `filterOutOfPage` | boolean | `true` | Remove out-of-bounds text |
| `filterTinyText` | boolean | `true` | Remove tiny text (height ≤ 1) |
| `filterHiddenOCG` | boolean | `true` | Filter hidden optional content groups |
| `filterSensitiveData` | boolean | `false` | Enable PII redaction |
| `filterRules` | List\<SanitizationRule\> | *defaults* | Regex rules for PII |

**Default PII patterns**: email, phone, passport, credit card, bank account, IPv4, IPv6, MAC address, IMEI, URL.

---

## 6. Package: `containers`

### StaticLayoutContainers.java
Thread-local state holder using `ThreadLocal<T>` for:

| Field | Type | Description |
|-------|------|-------------|
| `currentContentId` | int | Auto-incrementing element ID |
| `headings` | List\<SemanticHeading\> | Collected headings for level assignment |
| `imageIndex` | int | Auto-incrementing image counter |
| `isUseStructTree` | boolean | Structure tree mode flag |
| `contrastRatioConsumer` | ContrastRatioConsumer | For image extraction (lazy init) |
| `imagesDirectory` | String | Path for extracted images |
| `embedImages` | boolean | Base64 embed mode |
| `imageFormat` | String | "png" or "jpeg" |

Key methods:
- `incrementContentId()` → `int`
- `incrementImageIndex()` → `int`
- `getContrastRatioConsumer(pdfPath, password, ...)` → lazy-creates the consumer
- `clearContainers()` — resets all thread-local state

---

## 7. Package: `entities`

### SemanticFormula.java
```java
class SemanticFormula extends BaseObject {
    String latex;           // LaTeX representation
    getLatex() → String     // Returns "" if null
}
```

### SemanticPicture.java
```java
class SemanticPicture extends BaseObject {
    int index;              // Sequential picture index
    String description;     // AI-generated alt text (from hybrid backend)
    getPictureIndex() → int
    getDescription() → String  // Returns "" if null
    hasDescription() → boolean
}
```

---

## 8. Package: `processors`

### DocumentProcessor.java (~330 lines)
Main orchestrator. Key static methods:

- `processFile(String inputPdfName, Config config)` — full pipeline entry
- `preprocessing(File, Config)` — initializes veraPDF, parses PDF chunks
- `normalProcessDocument(Config, Set<Integer>)` — standard pipeline
- `processDocument(Config, Set<Integer>)` — routes to struct tree / hybrid / normal
- `sortContents()` — applies reading order sort
- `sanitizeContents()` — applies PII sanitization
- `generateOutputs()` — dispatches to output generators
- `getPageBoundingBox(int pageNumber)` → `BoundingBox` — page dimensions

### ContentFilterProcessor.java
`getFilteredContents(List<IObject>, Config)` → `List<IObject>`

Pipeline steps:
1. **removeSameTextChunks**: Remove duplicates (same value, overlapping bbox)
2. **removeTextDecorationImages**: Remove images inline with text
3. **filterTinyText**: Remove chunks with height ≤ 1pt
4. **filterOutOfPage**: Remove content outside page bounds
5. **mergeCloseTextChunks**: Merge adjacent chunks with same style/baseline
6. **trimWhiteSpaces**: Trim leading/trailing spaces from chunks
7. **filterConsecutiveSpaces**: Collapse multiple spaces
8. **splitByWhiteSpaces**: Split chunks containing internal whitespace
9. **findHiddenText**: Detect low-contrast text (ratio < 1.2)
10. **replaceUndefinedChars**: Replace unmappable characters
11. **processBackgrounds**: Detect background LineArtChunks (>50% width+>10% height or vice versa)

Constants:
- `MIN_TEXT_INTERSECTION_PERCENT = 0.5`
- `TEXT_MIN_HEIGHT = 1`
- `MIN_CONTRAST_RATIO = 1.2` (in HiddenTextProcessor)

### TextProcessor.java
Static text manipulation utilities:
- `replaceUndefinedCharacters(TextChunk)` — replaces unmappable chars
- `filterTinyText(List<IObject>)` — removes height ≤ 1
- `trimTextChunksWhiteSpaces(List<IObject>)` — trims whitespace
- `mergeCloseTextChunks(List<IObject>)` — merges same-style neighbors
- `removeSameTextChunks(List<IObject>)` — deduplicates
- `removeTextDecorationImages(List<IObject>)` — removes inline decoration images

### TextLineProcessor.java
Groups `TextChunk`s into `TextLine`s:
- Uses **one-line-probability** threshold: `0.75`
- Inserts space `TextChunk`s between gaps
- Links `TextLine`s with preceding `LineArtChunk` bullets
- Method: `processTextLines(List<IObject>)` → modifies list in-place, replacing TextChunks with TextLines

### ParagraphProcessor.java (~500 lines)
Multi-pass paragraph detection from TextLines:

```
processParagraphs() sequence:
1. detectParagraphsWithJustifyAlignments()
2. detectFirstAndLastLinesOfParagraphsWithJustifyAlignments()
3. detectParagraphsWithLeftAlignments(checkStyle=true)
4. detectParagraphsWithLeftAlignments(checkStyle=false)
5. detectFirstLinesOfParagraphWithLeftAlignments()
6. detectTwoLinesParagraphs()
7. detectParagraphsWithCenterAlignments()
8. detectParagraphsWithRightAlignments()
9. processOtherLines() — remaining TextLines become single-line paragraphs
```

Key constant: `DIFFERENT_LINES_PROBABILITY = 0.75`

Creates `SemanticParagraph` from grouped `TextBlock`s.

### HeadingProcessor.java (~200 lines)
Heading detection using font statistics:

**Algorithm**:
1. Build `TextNodeStatistics` from all text nodes (font sizes and weights)
2. For each `SemanticParagraph`, compute heading probability:
   - `NodeUtils.headingProbability()` (from veraPDF)
   - `fontSizeRarityBoost()` — how rare is this font size vs. document mode
   - `fontWeightRarityBoost()` — how rare is this font weight
   - `bulletedParagraph boost` — +0.1 if bulleted
3. If probability ≥ `HEADING_PROBABILITY` (0.75), convert to `SemanticHeading`
4. Can detect headings inside list items

**Level assignment** (`detectHeadingsLevels()`):
- Group headings by `TextStyle` (font name + size + weight)
- Sort groups by font size (descending)
- Assign levels 1, 2, 3, ... N

### HeaderFooterProcessor.java (~300 lines)
Detects repeating content at page top/bottom:

**Algorithm**:
1. Compare text content across consecutive pages and 2-page-skip pages
2. Match criteria: bounding box overlap, font size match, OR list label pattern
3. List label patterns: AlfaLetters, Korean, Roman, Arabic numbering
4. Position constraint: headers in top 1/3, footers in bottom 1/3 of page
5. Processes detected content through sub-pipeline: paragraph → list → heading → caption

### ListProcessor.java (~500 lines)
Two detection approaches:

**Approach 1 — `processLists()` (from TextLines)**:
- Uses `TextListInterval` with multiple label detection algorithms
- `ListLabelsUtils` from veraPDF for pattern matching
- Groups consecutive labeled lines into `PDFList`

**Approach 2 — `processListsFromTextNodes()` (from SemanticTextNodes)**:
- Detects lists from semantic text node labels
- Uses `BulletedParagraphUtils` for label classification

**Cross-page merging**: `checkNeighborLists()` — links adjacent lists across pages via `previousListId` / `nextListId`

**Korean-specific**: `ATTACHMENTS_PATTERN` for "붙임" prefix pattern

Constants:
- `LIST_ITEM_PROBABILITY = 0.7`
- `LIST_ITEM_BASELINE_DIFFERENCE = 1.2`

### TableBorderProcessor.java (~280 lines)
Processes border-detected tables from `LinesPreprocessingConsumer`:

**Algorithm**:
1. Get `TableBordersCollection` from `StaticContainers`
2. Map content (TextChunks, LineArtChunks) to `TableBorderCell`s by bounding box overlap
3. Handle split text chunks spanning cells
4. Recursively process cell contents (nested tables up to `MAX_NESTED_TABLE_DEPTH = 10`)

**Cell content processing pipeline**:
```
TableBorderProcessor → TextLineProcessor → ListProcessor → 
ParagraphProcessor → ListProcessor → HeadingProcessor → CaptionProcessor
```

**Cross-page linking**: `checkNeighborTables()` — links tables with matching column structure via `previousTableId` / `nextTableId`

### ClusterTableProcessor.java (~100 lines)
Extends `AbstractTableProcessor`. Detects **borderless tables** via spatial clustering:
- Uses `ClusterTableConsumer` from veraPDF
- Splits text chunks by whitespace before clustering
- Activates when `tableMethod == "cluster"` in config

### AbstractTableProcessor.java
Base class. `getPagesWithPossibleTables()`:
- Identifies suspicious pages: TextChunks that overlap vertically or have large horizontal gaps
- Constants: `Y_DIFFERENCE_EPSILON = 0.1`, `X_DIFFERENCE_EPSILON = 3`

### SpecialTableProcessor.java
Korean-style table detection:
- Pattern: `(수신|경유|제목).*`
- Creates 2-column `TableBorder`, splitting matching TextLines on ":" character

### CaptionProcessor.java
Links captions to images/tables:
- Iterates content sequentially, tracks last text node and current image
- Computes caption probability using `CaptionUtils.imageCaptionProbability()` from veraPDF
- Threshold: `CAPTION_PROBABILITY = 0.75`
- Creates `SemanticCaption` with `linkedContentId`
- Filters subtle images (aspect ratio < 0.01)

### HiddenTextProcessor.java
Uses `ContrastRatioConsumer` from veraPDF:
- Minimum contrast ratio: `MIN_CONTRAST_RATIO = 1.2`
- Either filters out or marks text as hidden (`isHiddenText = true`)

### LevelProcessor.java
Assigns hierarchical levels using a stack:

```
Level types:
- ListLevelInfo      → from PDFList
- TableLevelInfo     → from TableBorder
- LineArtBulletParagraphLevelInfo → from graphical bullet
- TextBulletParagraphLevelInfo    → from text bullet

Special levels:
- "Doctitle" → first H1 heading
- "Subtitle" → subsequent headings
```

### TaggedDocumentProcessor.java (~330 lines)
Processes tagged PDFs using the PDF structure tree:
- Walks `ITree` recursively
- Maps `SemanticType`s: CAPTION, HEADING, LIST, NUMBER_HEADING, PARAGRAPH, TABLE, TITLE
- Builds table structure from TH/TD cells with `rowSpan`/`colSpan`
- Collects artifacts separately for header/footer processing

---

## 9. Package: `readingorder`

### XYCutPlusPlusSorter.java (~550 lines)
Implements XY-Cut++ reading order algorithm (arXiv:2504.10258).

**Four Phases**:

1. **Pre-mask cross-layout elements**
   - Elements wider than `beta * maxWidth` with ≥ 2 vertical overlaps
   - Temporarily removed before segmentation

2. **Compute density ratio for adaptive axis selection**
   - X-density vs Y-density determines initial cut direction
   - Threshold: `DEFAULT_DENSITY_THRESHOLD = 0.9`

3. **Recursive segmentation**
   - Project elements onto X and Y axes
   - Find largest gap in each projection
   - Split at gap midpoint if gap ≥ `MIN_GAP_THRESHOLD` (5.0 pts)
   - Recursive until single elements or no valid gaps

4. **Merge cross-layout elements**
   - Re-insert pre-masked elements by Y position

Constants:
- `DEFAULT_BETA = 2.0`
- `DEFAULT_DENSITY_THRESHOLD = 0.9`
- `MIN_GAP_THRESHOLD = 5.0` (pts)

---

## 10. Package: `hybrid`

### HybridClient.java (Interface + DTOs)

```java
interface HybridClient {
    void checkAvailability() throws IOException;
    HybridResponse convert(HybridRequest request) throws IOException;
    CompletableFuture<HybridResponse> convertAsync(HybridRequest request);
    default void close() {}
}

class HybridRequest {
    byte[] pdfBytes;              // PDF file bytes
    List<Integer> pageNumbers;    // 1-indexed page numbers
    Set<OutputFormat> outputFormats; // JSON, MARKDOWN, HTML
}

class HybridResponse {
    String markdown;
    JsonNode json;
    Map<Integer, JsonNode> pageContents; // Per-page JSON
    List<Integer> failedPages;
}

enum OutputFormat { JSON, MARKDOWN, HTML }
```

### HybridClientFactory.java
Factory with `ConcurrentHashMap` cache:
- `"docling-fast"` → `DoclingFastServerClient`
- `"hancom"` → `HancomClient`
- `"azure"`, `"google"` → **not yet implemented**
- `shutdown()` — closes all cached clients

### HybridConfig.java
| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `url` | String | *per backend* | Server URL |
| `timeoutMs` | int | 30000 | HTTP timeout |
| `fallbackToJava` | boolean | `false` | Fallback on backend failure |
| `maxConcurrentRequests` | int | 4 | Concurrency limit |
| `mode` | String | `"auto"` | `"auto"` or `"full"` |

Default URLs:
- `docling-fast` → `http://localhost:5002`
- `hancom` → `https://dataloader.cloud.hancom.com/studio-lite/api`

### HybridDocumentProcessor.java (~400 lines)
Routes pages to Java or AI backend:

```
Flow:
1. checkAvailability()           — health check backend
2. filterAllPages()              — ContentFilterProcessor on all pages
3. triageAllPages()              — classify pages (or skip if mode="full")
4. Split pages by decision       — JAVA vs BACKEND
5. processJavaPath()             — standard pipeline for JAVA pages
6. processBackendPath()          — send PDF bytes, transform response
7. mergeResults()                — combine Java + backend results
8. postProcess()                 — header/footer, captions, levels
```

Backend path: Reads PDF bytes → `HybridClient.convert()` → `HybridSchemaTransformer.transform()` → `List<List<IObject>>`

Supports **fallback to Java** on backend failure when `fallbackToJava = true`.

### DoclingFastServerClient.java (~308 lines)
HTTP client for docling-fast-server FastAPI:
- Endpoint: `POST /v1/convert/file` (multipart/form-data)
- Health: `GET /health`
- Uses OkHttp
- Supports page range parameter: `page_ranges`
- Parses `DoclingDocument` JSON response format

### HancomClient.java (~305 lines)
HTTP client for Hancom Document AI:
- 3-step workflow: Upload → VisualInfo → Delete
- Upload: `POST /v1/dl/files/upload`
- VisualInfo: `GET /v1/dl/files/{fileId}/visualinfo?engine=pdf_ai_dl&dlaMode=ENABLED&ocrMode=FORCE`
- Delete: `DELETE /v1/dl/files/{fileId}`
- Always cleans up (file deletion) even on failure

### DoclingSchemaTransformer.java (~592 lines)
Transforms Docling `DoclingDocument` JSON to IObject hierarchy:

**Schema mapping**:
| Docling Label | → IObject Type |
|---------------|----------------|
| `text` | `SemanticParagraph` |
| `section_header` | `SemanticHeading` |
| `formula` | `SemanticFormula` |
| `picture` | `SemanticPicture` |
| `caption` | `SemanticParagraph` (with caption text) |
| `list_item` | `SemanticParagraph` |
| `page_header`, `page_footer` | Filtered out |
| `table` | `TableBorder` with rows/cells |

**Coordinate handling**:
- Detects origin: TOPLEFT or BOTTOMLEFT (from `CoordOrigin` enum)
- Converts TOPLEFT Y: `pageHeight - topY` → `bottomY`
- Table bbox distributed evenly across cells when cell coords not available

**Picture descriptions**: Extracted from `annotations` array where type is `"description"`

### HancomSchemaTransformer.java (~532 lines)
Transforms Hancom VisualInfoDto JSON to IObject hierarchy:

**Schema mapping**:
| Hancom Type | → IObject Type |
|-------------|----------------|
| `PARAGRAPH` | `SemanticParagraph` |
| `HEADING` | `SemanticHeading` |
| `TABLE` | `TableBorder` |
| `FIGURE` | `SemanticPicture` |
| `FORMULA` | `SemanticFormula` |
| `LIST_ITEM` | `SemanticParagraph` |
| `PAGE_HEADER`, `PAGE_FOOTER` | Filtered out |

**Coordinate conversion**: TOPLEFT (left, top, width, height) → BOTTOMLEFT (left, bottom, right, top)

### TriageProcessor.java (~1123 lines)
Page classification for hybrid routing.

**Decision enum**: `JAVA` | `BACKEND`

**Signal priority (in classifyPage)**:
1. **TableBorder presence** → BACKEND (confidence 1.0) — most reliable
2. **Vector graphics table signal** → BACKEND (0.95) — grid lines, borders, line art
3. **Text-based table patterns** → BACKEND (0.9) — consecutive alignment patterns
4. **Large image detection** → BACKEND (0.85) — image ≥ 11% of page area + aspect ratio ≥ 1.75
5. ~~Suspicious patterns~~ (disabled) — too many false positives
6. **High LineChunk ratio** → BACKEND (0.8) — ratio exceeds threshold
7. ~~Grid pattern detection~~ (disabled) — too many false positives
8. **Default** → JAVA (confidence 0.9)

**TriageSignals** tracks:
- `lineChunkCount`, `textChunkCount`, `lineToTextRatio`
- `alignedLineGroups`
- `hasTableBorder`
- `horizontalLineCount`, `verticalLineCount`, `lineArtCount`
- `hasGridLines`, `hasTableBorderLines`, `hasRowSeparatorPattern`, `hasAlignedShortLines`
- `tablePatternCount`, `maxConsecutiveStreak`, `patternDensity`, `hasConsecutivePatterns`
- `largeImageRatio`, `largeImageAspectRatio`

**Signal extraction** uses `SignalAccumulator` for single-pass analysis of page content.

**Detection constants**:
| Constant | Value | Description |
|----------|-------|-------------|
| `MIN_LINE_COUNT_FOR_TABLE` | 8 | Min lines for table detection |
| `MIN_GRID_LINES` | 3 | Min horizontal+vertical lines for grid |
| `MIN_ROW_SEPARATOR_PATTERN` | 5 | Min row separator patterns |
| `DEFAULT_ALIGNED_LINE_GROUPS_THRESHOLD` | 5 | Min aligned groups |
| `MIN_LARGE_IMAGE_RATIO` | 0.11 | Min image/page area ratio |
| `MIN_IMAGE_ASPECT_RATIO` | 1.75 | Min width/height ratio for large image |
| `MIN_PATTERN_DENSITY` | 0.10 | Min table pattern density |
| `MIN_CONSECUTIVE_PATTERNS` | 3 | Min consecutive suspicious chunks |
| `MIN_TABLE_PATTERNS` | 3 | Min total table patterns |
| `HIGH_PATTERN_COUNT_THRESHOLD` | 8 | High pattern count bypass |
| `MIN_ALIGNED_SHORT_LINES` | 4 | Min aligned short lines |
| `LINE_LENGTH_TOLERANCE` | 0.15 | Tolerance for line matching |

---

## 11. Package: `markdown`

### MarkdownGenerator.java (~350 lines)
Implements `Closeable`. Writes markdown output to file.

**Element handling**:
| IObject Type | Markdown Output |
|-------------|-----------------|
| `SemanticHeading` | `# ` / `## ` / `### ` (1-6 levels) |
| `SemanticParagraph` | Plain text + double newline |
| `TableBorder` | `\|...\|` pipe table syntax |
| `PDFList` | `- ` prefix per item |
| `ImageChunk` | `![figureN](path)` |
| `SemanticFormula` | `$$...$$` block |
| `SemanticPicture` | `![figureN](path)` + description block |
| `SemanticHeaderOrFooter` | Recursive write (if included) |

**Table handling**: Tracks nesting level. Nested tables within cells use HTML fallback.

**Image modes**: Embedded (Base64 data URI) or External (relative path).

**Page separator**: Replaces `%page-number%` with actual page number.

### MarkdownHTMLGenerator.java
Extends `MarkdownGenerator`. Overrides `writeTable()` to use HTML `<table>` tags with `colspan`/`rowspan` support instead of pipe tables.

### MarkdownGeneratorFactory.java
Factory: returns `MarkdownHTMLGenerator` if `config.isUseHTMLInMarkdown()`, else `MarkdownGenerator`.

### MarkdownSyntax.java
Constants:
```
TABLE_COLUMN_SEPARATOR = "|"
TABLE_HEADER_SEPARATOR = "---"
HEADING_LEVEL = "#"
LIST_ITEM = "-"
IMAGES_DIRECTORY_SUFFIX = "_images"
IMAGE_FILE_NAME_FORMAT = "%s%simageFile%d.%s"
IMAGE_FORMAT = "![%s](%s)"
MATH_BLOCK_START = "$$"
MATH_BLOCK_END = "$$"
```
Also includes HTML table tag constants for `MarkdownHTMLGenerator`.

---

## 12. Package: `html`

### HtmlGenerator.java (~470 lines)
Implements `Closeable`. Generates full HTML document.

**Structure**: `<!DOCTYPE html><html><head>...</head><body>...</body></html>`

**Element handling**:
| IObject Type | HTML Output |
|-------------|-------------|
| `SemanticHeading` | `<h1>` - `<h6>` |
| `SemanticParagraph` | `<p>` with optional `&nbsp;` indent |
| `TableBorder` | `<table border="1">` with `<th>`/`<td>`, `colspan`/`rowspan` |
| `PDFList` | `<ul><li>` |
| `ImageChunk` | `<img src="..." alt="figureN">` |
| `SemanticFormula` | `<div class="math-display">\\[...\\]</div>` |
| `SemanticPicture` | `<figure><img><figcaption></figure>` |
| `SemanticTextNode` | `<figcaption>` (for captions) |

Tracks table nesting level. Escapes HTML attributes (`&amp;`, `&quot;`, `&lt;`, `&gt;`).

### HtmlSyntax.java
HTML tag constants: `<table>`, `<tr>`, `<td>`, `<th>`, `<ul>`, `<li>`, `<p>`, `<figure>`, `<figcaption>`, `<div class="math-display">`, `<br>`, etc.

---

## 13. Package: `json`

### JsonWriter.java (~100 lines)
Writes JSON using Jackson `JsonGenerator`. 

**Document structure**:
```json
{
  "file name": "test.pdf",
  "number of pages": 5,
  "author": "...",
  "title": "...",
  "creation date": "...",
  "modification date": "...",
  "kids": [
    // Content objects serialized as POJO
  ]
}
```

### ObjectMapperHolder.java
Singleton `ObjectMapper` with custom serializer module. Registers serializers for:
- `TextChunk`, `TextLine`, `ImageChunk`, `LineChunk`
- `TableBorder`, `TableBorderRow`, `TableBorderCell`
- `PDFList`, `ListItem`
- `SemanticTextNode`, `SemanticHeading`, `SemanticCaption`
- `SemanticHeaderOrFooter`, `SemanticFormula`, `SemanticPicture`
- `Double` (custom formatting)

Note: `ParagraphSerializer` is commented out — paragraphs use `SemanticTextNodeSerializer`.

### JsonName.java
Field name constants for JSON output:
```
"type", "id", "level", "page number", "bounding box", "content",
"font", "font size", "text color", "hidden text",
"heading level", "heading", "paragraph", "table", "table row",
"table cell", "list", "list item", "image", "line", "text block",
"number of rows", "number of columns", "row number", "column number",
"row span", "column span", "rows", "cells", "kids", "list items",
"number of list items", "previous list id", "next list id",
"previous table id", "next table id", "numbering style",
"source", "data", "format", "formula", "description"
```

### SerializerUtil.java
Shared JSON serialization helpers:
- `writeEssentialInfo(generator, object, type)` — writes type, id, level, page number, bounding box
- `writeTextInfo(generator, textNode)` — writes font, font size, text color, content, hidden text flag

### Serializer Pattern
All serializers extend `StdSerializer<T>` and follow this pattern:
```java
writeStartObject()
SerializerUtil.writeEssentialInfo(gen, obj, TYPE_NAME)
// type-specific fields
writeEndObject()
```

**Image/Picture serializers** handle embedded (Base64 `data` field) vs external (`source` field) modes.

---

## 14. Package: `pdf`

### PDFWriter.java (~350 lines)
Generates annotated PDF with colored bounding box overlays using **Apache PDFBox**:
- Each element type gets a color (heading=blue, table=magenta, list=green, paragraph=cyan, image=red, caption=yellow)
- Uses `PDAnnotationSquare` with 0.4 opacity
- Supports PDF Optional Content Groups (OCG layers): CONTENT, TABLE_CELLS, LIST_ITEMS, TABLE_CONTENT, LIST_CONTENT, TEXT_BLOCK_CONTENT, HEADER_AND_FOOTER_CONTENT
- Output filename: `{original}_annotated.pdf`
- Handles multi-page bounding boxes via `MultiBoundingBox`

### PDFLayer.java
Enum with values: `CONTENT`, `TABLE_CELLS`, `LIST_ITEMS`, `TABLE_CONTENT`, `LIST_CONTENT`, `TEXT_BLOCK_CONTENT`, `HEADER_AND_FOOTER_CONTENT`

---

## 15. Package: `text`

### TextGenerator.java (~250 lines)
Plain text output generator. Implements `Closeable`.
- Handles heading, paragraph, list (with indentation), table (tab-separated columns)
- Skips images and non-text content
- Uses `INDENT = "  "` (2 spaces) per nesting level
- Sanitizes null characters → spaces
- Compacts whitespace in table cells
- Respects `includeHeaderFooter` config

---

## 16. Package: `utils`

### ContentSanitizer.java (~300 lines)
PII regex-based content sanitization:
- Walks all content types recursively (text nodes, lists, tables, headers/footers)
- Applies `SanitizationRule` patterns to `TextLine` values
- Replaces matched patterns in-place at `TextChunk` level
- Handles overlapping replacements (priority by position, then length)
- Updates bounding boxes for replacement chunks

### SanitizationRule.java
```java
class SanitizationRule {
    Pattern pattern;     // Compiled regex
    String replacement;  // Replacement text
}
```

### ImagesUtils.java
Image extraction from PDF:
- Creates images directory
- Walks all content types recursively to find `ImageChunk` / `SemanticPicture`
- Uses `ContrastRatioConsumer.getPageSubImage(BoundingBox)` to extract image regions
- Writes to file using `ImageIO.write(BufferedImage, format, file)`
- Auto-increments image index (`StaticLayoutContainers.incrementImageIndex()`)

### Base64ImageUtils.java
- `toDataUri(File imageFile, String format)` → `"data:image/png;base64,..."`
- `MAX_EMBEDDED_IMAGE_SIZE = 10MB` — larger images skipped
- `getMimeType(format)` → "image/png" or "image/jpeg"

### BulletedParagraphUtils.java (~200 lines)
Comprehensive bullet/label detection:

**Bullet characters** (~300 Unicode symbols): `•‣※⁃★○◆▶◇...`

**Regex patterns** (static initialization block):
- Arabic: `^\d+[.\])>].*`, `^\(\d+\).*`, `^<\d+>.*`, `^\[\d+\].*`, `^{\d+}.*`
- Korean consonants: `^[ㄱㄴㄷ...][.\])>].*`
- Korean syllables: `^[가나다라...][.\-)>].*`
- Korean chapters: `^(제\d+[장조절]).*`
- Unicode encircled: `①-⑳`, `⑴-⒇`, `⒈-⒛`, `⒜-⒵`, `Ⓐ-Ⓩ`, `ⓐ-ⓩ`, `❶-❿`, `➀-➉`, `➊-➓`
- Korean encircled: `㉮-㉻`
- Roman numerals: `Ⅰ-Ⅻ`, `ⅰ-ⅻ`

Methods:
- `isBulletedParagraph(SemanticTextNode)` → `boolean`
- `isBulletedLine(TextLine)` → `boolean`
- `isLabeledLine(TextLine)` → `boolean` — checks characters, LineArt labels, regex patterns
- `isBulletedLineArtParagraph(SemanticTextNode)` → `boolean` — graphical bullet
- `getLabelRegex(SemanticTextNode)` → `String` — returns matching pattern
- `getLabel(SemanticTextNode)` → `String` — first character

### ModeWeightStatistics.java
Statistical analysis for font properties:
- Tracks frequency of scores (font size or weight)
- Computes **mode** (most frequent value within bounds)
- `getBoost(score)` → `double` — rarity boost (0.0 to 1.0) based on score's position among values above mode
- Used by `TextNodeStatistics` for heading detection

### TextNodeStatistics.java
Heading scoring using font statistics:
- Contains `ModeWeightStatistics` for font size and font weight
- `addTextNode(SemanticTextNode)` — accumulates font data
- `fontSizeRarityBoost(node)` → `double` — boost based on font size rarity × 0.5
- `fontWeightRarityBoost(node)` → `double` — boost based on weight rarity × 0.3

### TextNodeStatisticsConfig.java
Tunable thresholds for heading scoring:

| Field | Default | Description |
|-------|---------|-------------|
| `fontSizeDominantMin` | 10.0 | Min font size for "dominant" range |
| `fontSizeDominantMax` | 13.0 | Max font size for "dominant" range |
| `fontSizeHeadingMin` | 10.0 | Min font size for heading candidates |
| `fontSizeHeadingMax` | 32.0 | Max font size for heading candidates |
| `fontSizeRarityBoost` | 0.5 | Max boost from font size rarity |
| `fontWeightDominantMin` | 395.0 | Min weight for "dominant" range |
| `fontWeightDominantMax` | 405.0 | Max weight for "dominant" range |
| `fontWeightHeadingMin` | 400.0 | Min weight for heading candidates |
| `fontWeightHeadingMax` | 900.0 | Max weight for heading candidates |
| `fontWeightRarityBoost` | 0.3 | Max boost from font weight rarity |

### Level Info Classes (utils/levels/)

**LevelInfo** (base):
- Fields: `left`, `right` (x-coordinates)
- Static: `areSameLevelsInfos()` — compares two level infos for nesting
- Static: `checkBoundingBoxes()` — validates x-overlap with `X_GAP_MULTIPLIER = 0.3`

**ListLevelInfo**: `commonPrefix`, `numberingStyle`, `maxFontSize`
**TableLevelInfo**: Marker class (no extra fields)
**LineArtBulletParagraphLevelInfo**: `bullet` (LineArtChunk), `maxFontSize`
**TextBulletParagraphLevelInfo**: `label`, `labelRegex`, `maxFontSize`

---

## 17. Key Constants & Thresholds

### Content Filtering
| Constant | Value | Location |
|----------|-------|----------|
| `TEXT_MIN_HEIGHT` | 1 | TextProcessor |
| `MIN_TEXT_INTERSECTION_PERCENT` | 0.5 | TextProcessor |
| `MIN_CONTRAST_RATIO` | 1.2 | HiddenTextProcessor |
| `MAX_EMBEDDED_IMAGE_SIZE` | 10MB | Base64ImageUtils |

### Paragraph & Heading Detection
| Constant | Value | Location |
|----------|-------|----------|
| `DIFFERENT_LINES_PROBABILITY` | 0.75 | ParagraphProcessor |
| `HEADING_PROBABILITY` | 0.75 | HeadingProcessor |
| `ONE_LINE_PROBABILITY` | 0.75 | TextLineProcessor |
| `CAPTION_PROBABILITY` | 0.75 | CaptionProcessor |
| `LIST_ITEM_PROBABILITY` | 0.7 | ListProcessor |
| `LIST_ITEM_BASELINE_DIFFERENCE` | 1.2 | ListProcessor |

### Table Detection
| Constant | Value | Location |
|----------|-------|----------|
| `Y_DIFFERENCE_EPSILON` | 0.1 | AbstractTableProcessor |
| `X_DIFFERENCE_EPSILON` | 3 | AbstractTableProcessor |
| `MAX_NESTED_TABLE_DEPTH` | 10 | TableBorderProcessor |

### Reading Order
| Constant | Value | Location |
|----------|-------|----------|
| `DEFAULT_BETA` | 2.0 | XYCutPlusPlusSorter |
| `DEFAULT_DENSITY_THRESHOLD` | 0.9 | XYCutPlusPlusSorter |
| `MIN_GAP_THRESHOLD` | 5.0 pts | XYCutPlusPlusSorter |

### Triage
| Constant | Value | Location |
|----------|-------|----------|
| `DEFAULT_LINE_RATIO_THRESHOLD` | 0.3 | TriageProcessor |
| `MIN_LARGE_IMAGE_RATIO` | 0.11 | TriageProcessor |
| `MIN_IMAGE_ASPECT_RATIO` | 1.75 | TriageProcessor |
| `MIN_GRID_LINES` | 3 | TriageProcessor |
| `MIN_LINE_COUNT_FOR_TABLE` | 8 | TriageProcessor |
| `MIN_PATTERN_DENSITY` | 0.10 | TriageProcessor |
| `BASELINE_EPSILON` | 0.3 | TriageProcessor |
| `X_DIFFERENCE_EPSILON` | 3.0 | TriageProcessor |
| `MULTI_COLUMN_X_SHIFT_RATIO` | 0.5 | TriageProcessor |

### Font Statistics
| Constant | Value | Location |
|----------|-------|----------|
| `fontSizeDominantMin` | 10.0 | TextNodeStatisticsConfig |
| `fontSizeDominantMax` | 13.0 | TextNodeStatisticsConfig |
| `fontSizeRarityBoost` | 0.5 | TextNodeStatisticsConfig |
| `fontWeightRarityBoost` | 0.3 | TextNodeStatisticsConfig |
| `X_GAP_MULTIPLIER` | 0.3 | LevelInfo |

---

## 18. External Dependencies from veraPDF

These types/interfaces must be reimplemented or wrapped in Rust:

### Core Interfaces
- `IObject` — base content interface (bbox, pageNumber, id, level)
- `INode` — extends IObject with SemanticType and children
- `ITree` — structure tree interface

### Content Types (from veraPDF WCAG algorithms)
- `BaseObject`, `SemanticTextNode`, `SemanticParagraph`, `SemanticHeading`, `SemanticCaption`, `SemanticSpan`, `SemanticHeaderOrFooter`, `SemanticFigure`
- `TextChunk` — text with font info, baseline, bounding box, character positions
- `TextLine` — ordered TextChunks + connectedLineArtLabel
- `TextBlock` — TextLines + firstLineIndent
- `TextColumn` — TextBlocks
- `ImageChunk`, `LineChunk`, `LineArtChunk`
- `TableBorder`, `TableBorderRow`, `TableBorderCell`
- `PDFList`, `ListItem`
- `BoundingBox`, `MultiBoundingBox`

### Geometry
- `BoundingBox` — leftX, bottomY, rightX, topY, pageNumber, lastPageNumber
- Methods: `getWidth()`, `getHeight()`, `move()`, `getBoundingBox(pageNumber)`

### Algorithms (from veraPDF)
- `LinesPreprocessingConsumer.findTableBorders()` — line → table border detection
- `ClusterTableConsumer` — spatial clustering for borderless tables
- `ContrastRatioConsumer` — text contrast analysis, image extraction
- `NodeUtils` — heading probability, close number comparison
- `CaptionUtils` — caption probability
- `ListLabelsUtils` — list label pattern matching
- `TextChunkUtils`, `ChunksMergeUtils` — text merging utilities
- `TableBordersCollection` — collection of detected table borders per page
- `TextListInterval` — list detection algorithm
- `TextStyle` — font name + size + weight triple

### State Management (from veraPDF)
- `StaticContainers` — global document state, TableBordersCollection, PDDocument
- `StaticResources` — PDF resources

### PDF Parser
- `GFSAPDFDocument` (veraPDF) — PDF document parser
- `PDDocument.parseChunks()` — extracts TextChunks, ImageChunks, LineChunks

---

## 19. Rust Rewrite Considerations

### Architecture Decisions
1. **Replace ThreadLocal state** with explicit context objects passed through the pipeline
2. **Replace veraPDF** with a Rust PDF parsing library (e.g., `pdf-rs`, `lopdf`, or custom)
3. **Replace Jackson** with `serde` for JSON serialization
4. **Replace OkHttp** with `reqwest` for HTTP clients
5. **Replace Apache PDFBox** with a Rust PDF writing library for annotated output

### Critical Algorithms to Port
1. **XY-Cut++ reading order** (XYCutPlusPlusSorter) — self-contained algorithm
2. **Table border detection** — currently delegated to veraPDF's `LinesPreprocessingConsumer`
3. **Borderless table detection** — veraPDF's `ClusterTableConsumer`
4. **Paragraph detection** — multi-pass alignment analysis (ParagraphProcessor)
5. **Heading detection** — font statistics + rarity scoring
6. **List detection** — label pattern matching with extensive unicode support
7. **Header/footer detection** — cross-page repetition analysis
8. **Content triage** — signal extraction and decision logic

### Content Model for Rust
```rust
enum ContentElement {
    Paragraph(ParagraphData),
    Heading(HeadingData),        // level: u8
    Table(TableData),            // rows, cols, cells with spans
    List(ListData),              // items with contents
    Image(ImageData),            // index, bbox
    Picture(PictureData),        // index, description (from AI)
    Formula(FormulaData),        // latex: String
    Caption(CaptionData),        // linked_content_id
    HeaderFooter(HeaderFooterData), // contents: Vec<ContentElement>
    LineChunk(LineData),
    LineArt(LineArtData),
}
```

### Configuration Mapping
The `Config` and `FilterConfig` classes map directly to Rust structs. The page range parser is a simple state machine.

### Thread Safety
The current Java code uses `ThreadLocal` heavily (StaticContainers, StaticLayoutContainers). In Rust, use:
- Explicit `ProcessingContext` struct passed through pipeline
- Or `Arc<Mutex<State>>` for shared state (less preferred)

### Output Format Notes
- JSON schema is defined by the serializers — each element has `type`, `id`, `level`, `page number`, `bounding box`
- Markdown supports both pipe tables and HTML table fallback
- HTML uses semantic tags (`<figure>`, `<figcaption>`, `<div class="math-display">`)
- Annotated PDF uses PDFBox Optional Content Groups — will need equivalent in Rust PDF library
