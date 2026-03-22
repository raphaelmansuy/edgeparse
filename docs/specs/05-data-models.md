# 05 — Data Models

> **Cross-references**: [03-technical-architecture](03-technical-architecture.md) | [04-pdf-parsing-pipeline](04-pdf-parsing-pipeline.md) | [08-output-formats](08-output-formats.md)

---

## 1. Type Hierarchy Overview

```
IObject (trait)
├── BaseObject
│   ├── InfoChunk (IChunk)
│   │   ├── TextInfoChunk
│   │   │   ├── TextChunk       ← Atomic text fragment
│   │   │   ├── TextLine        ← Horizontal group of TextChunks
│   │   │   ├── TextBlock       ← Vertical group of TextLines
│   │   │   ├── TextColumn      ← Vertical group of TextBlocks
│   │   │   ├── TableToken      ← TextChunk assigned to table cell
│   │   │   ├── TableRow        ← Row in Table
│   │   │   ├── TableCell       ← Cell in TableRow
│   │   │   └── ListElement
│   │   │       ├── ListLabel   ← Label part of list item
│   │   │       └── ListBody    ← Body part of list item
│   │   ├── ImageChunk          ← Image bounding box
│   │   ├── LineChunk           ← Line segment (for borders)
│   │   ├── LineArtChunk        ← Vector graphic (bullet, etc.)
│   │   └── Vertex              ← Point with radius
│   ├── SemanticNode (INode)
│   │   ├── SemanticTextNode
│   │   │   ├── SemanticParagraph
│   │   │   │   ├── SemanticHeading
│   │   │   │   └── SemanticNumberHeading
│   │   │   ├── SemanticCaption
│   │   │   ├── SemanticSpan
│   │   │   ├── SemanticPart
│   │   │   └── SemanticList
│   │   ├── SemanticHeaderOrFooter
│   │   ├── SemanticFigure
│   │   ├── SemanticTable
│   │   ├── SemanticGroupingNode
│   │   ├── SemanticDocument
│   │   └── SemanticAnnot
│   ├── SemanticFormula          ← LaTeX formula (custom)
│   ├── SemanticPicture          ← Described image (custom)
│   └── TableBorder              ← Grid-based table structure
│       ├── TableBorderRow
│       └── TableBorderCell
├── PDFList                      ← Ordered/unordered list
└── ListItem                     ← Entry in PDFList
```

---

## 2. Core Trait: IObject

All elements implement this trait. In Rust, use a trait + struct pattern.

```rust
/// Core trait for all positioned PDF elements
pub trait PdfObject {
    fn page_number(&self) -> Option<u32>;
    fn set_page_number(&mut self, page: Option<u32>);
    fn last_page_number(&self) -> Option<u32>;
    fn set_last_page_number(&mut self, page: Option<u32>);
    fn bounding_box(&self) -> &BoundingBox;
    fn bounding_box_mut(&mut self) -> &mut BoundingBox;
    fn index(&self) -> Option<u32>;
    fn set_index(&mut self, idx: Option<u32>);
    fn level(&self) -> Option<&str>;
    fn set_level(&mut self, level: Option<String>);

    // Convenience (default impls)
    fn left_x(&self) -> f64   { self.bounding_box().left_x }
    fn right_x(&self) -> f64  { self.bounding_box().right_x }
    fn bottom_y(&self) -> f64 { self.bounding_box().bottom_y }
    fn top_y(&self) -> f64    { self.bounding_box().top_y }
    fn width(&self) -> f64    { self.right_x() - self.left_x() }
    fn height(&self) -> f64   { self.top_y() - self.bottom_y() }
    fn center_x(&self) -> f64 { (self.left_x() + self.right_x()) / 2.0 }
    fn center_y(&self) -> f64 { (self.bottom_y() + self.top_y()) / 2.0 }
}
```

---

## 3. Geometry

### 3.1 BoundingBox

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BoundingBox {
    pub page_number: Option<u32>,
    pub last_page_number: Option<u32>,
    pub left_x: f64,
    pub bottom_y: f64,
    pub right_x: f64,
    pub top_y: f64,
}

const BBOX_EPSILON: f64 = 0.0001;

impl BoundingBox {
    pub fn new(page: Option<u32>, left_x: f64, bottom_y: f64,
               right_x: f64, top_y: f64) -> Self;
    pub fn empty() -> Self;

    // Geometry operations
    pub fn union(&self, other: &BoundingBox) -> BoundingBox;
    pub fn normalize(&mut self);  // ensure left < right, bottom < top
    pub fn overlaps(&self, other: &BoundingBox) -> bool;
    pub fn contains(&self, other: &BoundingBox) -> bool;
    pub fn weakly_contains(&self, other: &BoundingBox) -> bool;
    pub fn area(&self) -> f64;
    pub fn is_empty(&self) -> bool;

    // Intersection analysis
    pub fn intersection_percent(&self, other: &BoundingBox) -> f64;
    pub fn vertical_intersection_percent(&self, other: &BoundingBox) -> f64;
    pub fn vertical_gap(&self, other: &BoundingBox) -> f64;
    pub fn horizontal_gap(&self, other: &BoundingBox) -> f64;
    pub fn are_horizontal_overlapping(&self, other: &BoundingBox) -> bool;
    pub fn are_vertical_overlapping(&self, other: &BoundingBox) -> bool;

    // Transform
    pub fn scale(&mut self, factor: f64);
    pub fn translate(&mut self, dx: f64, dy: f64);
    pub fn is_one_page(&self) -> bool;
    pub fn is_multi_page(&self) -> bool;
}
```

### 3.2 MultiBoundingBox

```rust
pub struct MultiBoundingBox {
    pub outer: BoundingBox,
    pub inner: Vec<BoundingBox>,
}
```

### 3.3 Vertex

```rust
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
}
```

---

## 4. Content Elements (Chunks)

### 4.1 TextChunk

The atomic unit of text extraction. One font run in the PDF content stream.

```rust
pub struct TextChunk {
    // PdfObject fields
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,

    // TextInfoChunk fields
    pub font_size: f64,
    pub base_line: f64,
    pub slant_degree: f64,
    pub is_hidden_text: bool,

    // TextChunk-specific fields
    pub value: String,
    pub font_name: String,
    pub font_weight: f64,          // 100.0 - 900.0
    pub italic_angle: f64,
    pub font_color: [f64; 3],      // RGB, 0.0-1.0 each
    pub contrast_ratio: f64,       // against background
    pub has_special_style: bool,
    pub has_special_background: bool,
    pub background_color: Option<[f64; 3]>,
    pub is_underlined_text: bool,
    pub text_format: TextFormat,   // Normal, Superscript, Subscript
    pub symbol_ends: Vec<f64>,     // X-coordinate of each glyph end
}
```

**Key methods**:
- `is_white_space_chunk()` — entire value is whitespace
- `compress_spaces()` — collapse consecutive spaces
- `average_symbol_width()` — width / character_count
- `text_length()` — number of characters
- `symbol_start_coordinate(idx)` / `symbol_end_coordinate(idx)` — glyph X positions
- `sub_chunk(start_idx, end_idx)` — extract sub-range of characters as new TextChunk

### 4.2 TextLine

A horizontal group of TextChunks that share a baseline.

```rust
pub struct TextLine {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub font_size: f64,
    pub base_line: f64,
    pub slant_degree: f64,
    pub is_hidden_text: bool,

    pub text_chunks: Vec<TextChunk>,
    pub is_line_start: bool,
    pub is_line_end: bool,
    pub is_list_line: bool,
    pub connected_line_art_label: Option<LineArtChunk>,
}
```

### 4.3 TextBlock

A vertical group of TextLines forming a text block (partial paragraph).

```rust
pub struct TextBlock {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub font_size: f64,
    pub base_line: f64,
    pub slant_degree: f64,
    pub is_hidden_text: bool,

    pub text_lines: Vec<TextLine>,
    pub has_start_line: bool,       // block starts with a new paragraph
    pub has_end_line: bool,         // block ends a paragraph
    pub text_alignment: Option<TextAlignment>,
}
```

### 4.4 TextColumn

A vertical group of TextBlocks.

```rust
pub struct TextColumn {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub font_size: f64,
    pub base_line: f64,
    pub slant_degree: f64,
    pub is_hidden_text: bool,

    pub text_blocks: Vec<TextBlock>,
}
```

### 4.5 ImageChunk

Bounding box only — actual pixel data extracted at output time.

```rust
pub struct ImageChunk {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
}
```

### 4.6 LineChunk

A straight line segment, used for table border detection.

```rust
pub struct LineChunk {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,

    pub start: Vertex,
    pub end: Vertex,
    pub width: f64,
    pub is_horizontal_line: bool,
    pub is_vertical_line: bool,
    pub is_square: bool,
}

// Constants
const BUTT_CAP_STYLE: u8 = 0;
const ROUND_CAP_STYLE: u8 = 1;
const PROJECTING_SQUARE_CAP_STYLE: u8 = 2;
```

### 4.7 LineArtChunk

A collection of line segments forming a vector graphic (bullet, decoration).

```rust
pub struct LineArtChunk {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,

    pub line_chunks: Vec<LineChunk>,
}

const LINE_ART_SIZE_EPSILON: f64 = 1.0; // size comparison tolerance
```

### 4.8 LinesCollection

Per-page collection of line segments, used during table border detection.

```rust
pub struct LinesCollection {
    /// page_number → sorted set of horizontal lines
    pub horizontal_lines: HashMap<u32, BTreeSet<LineChunk>>,
    /// page_number → sorted set of vertical lines
    pub vertical_lines: HashMap<u32, BTreeSet<LineChunk>>,
    /// page_number → sorted set of square-like lines
    pub squares: HashMap<u32, BTreeSet<LineChunk>>,
}
```

---

## 5. Semantic Nodes

### 5.1 SemanticType Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticType {
    Document,
    Div,
    Paragraph,
    Span,
    Table,
    TableHeaders,
    TableFooter,
    TableBody,
    TableRow,
    TableHeader,
    TableCell,
    Form,
    Link,
    Annot,
    Caption,
    List,
    ListLabel,
    ListBody,
    ListItem,
    TableOfContent,
    TableOfContentItem,
    Figure,
    NumberHeading,
    Heading,
    Title,
    BlockQuote,
    Note,
    Header,
    Footer,
    Code,
    Part,
}

impl SemanticType {
    pub fn is_ignored_standard_type(&self) -> bool;
}
```

### 5.2 SemanticTextNode

Base for all text-bearing semantic elements.

```rust
pub struct SemanticTextNode {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub semantic_type: SemanticType,
    pub correct_semantic_score: Option<f64>,

    pub columns: Vec<TextColumn>,
    pub font_weight: Option<f64>,
    pub font_size: Option<f64>,
    pub text_color: Option<[f64; 3]>,
    pub italic_angle: Option<f64>,
    pub font_name: Option<String>,
    pub text_format: Option<TextFormat>,
    pub max_font_size: Option<f64>,
    pub background_color: Option<[f64; 3]>,
    pub is_hidden_text: bool,
}

impl SemanticTextNode {
    pub fn value(&self) -> String;  // concatenated text of all lines
    pub fn lines_number(&self) -> usize;
    pub fn columns_number(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn is_space_node(&self) -> bool;
    pub fn starts_with_arabic_number(&self) -> bool;
    pub fn has_full_lines(&self) -> bool;
}
```

### 5.3 SemanticParagraph

```rust
pub struct SemanticParagraph {
    pub base: SemanticTextNode,
    pub enclosed_top: bool,
    pub enclosed_bottom: bool,
    pub indentation: i32,
}
```

### 5.4 SemanticHeading

```rust
pub struct SemanticHeading {
    pub base: SemanticParagraph,
    pub heading_level: Option<u32>,  // 1-6
}
```

### 5.5 SemanticNumberHeading

```rust
pub struct SemanticNumberHeading {
    pub base: SemanticHeading,
    // No additional fields — distinguishes numbered headings
    // e.g., "1.2.3 Budget Overview"
}
```

### 5.6 SemanticCaption

```rust
pub struct SemanticCaption {
    pub base: SemanticTextNode,
    pub linked_content_id: Option<u64>,  // ID of the image or table
}
```

### 5.7 SemanticHeaderOrFooter

```rust
pub struct SemanticHeaderOrFooter {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub semantic_type: SemanticType, // Header or Footer
    pub contents: Vec<ContentElement>,
}
```

### 5.8 SemanticFigure

```rust
pub struct SemanticFigure {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub semantic_type: SemanticType,
    pub images: Vec<ImageChunk>,
    pub line_arts: Vec<LineArtChunk>,
}
```

### 5.9 SemanticTable

```rust
pub struct SemanticTable {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub semantic_type: SemanticType,
    pub table_border: TableBorder,
}
```

### 5.10 SemanticFormula (Custom)

```rust
pub struct SemanticFormula {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub latex: String,  // LaTeX representation from enrichment
}
```

### 5.11 SemanticPicture (Custom)

```rust
pub struct SemanticPicture {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub image_index: u32,
    pub description: String,  // from enrichment (SmolVLM, etc.)
}
```

---

## 6. Table Structures

### 6.1 TableBorder

Grid-based table structure defined by row/column coordinates.

```rust
pub struct TableBorder {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,

    /// X-coordinates of column boundaries (N+1 values for N columns)
    pub x_coordinates: Vec<f64>,
    /// Widths of column boundary lines
    pub x_widths: Vec<f64>,
    /// Y-coordinates of row boundaries (M+1 values for M rows)
    pub y_coordinates: Vec<f64>,
    /// Widths of row boundary lines
    pub y_widths: Vec<f64>,

    pub rows: Vec<TableBorderRow>,
    pub num_rows: usize,
    pub num_columns: usize,
    pub is_bad_table: bool,
    pub is_table_transformer: bool,

    /// Cross-page linking
    pub previous_table: Option<Box<TableBorder>>,
    pub next_table: Option<Box<TableBorder>>,
}

const TABLE_BORDER_EPSILON: f64 = 0.5;
const MIN_CELL_CONTENT_INTERSECTION_PERCENT: f64 = 0.01;
```

### 6.2 TableBorderRow

```rust
pub struct TableBorderRow {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub row_number: usize,
    pub cells: Vec<TableBorderCell>,
    pub semantic_type: Option<SemanticType>,
}
```

### 6.3 TableBorderCell

```rust
pub struct TableBorderCell {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,
    pub row_number: usize,
    pub col_number: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub content: Vec<TableToken>,
    pub contents: Vec<ContentElement>,   // After sub-pipeline processing
    pub semantic_type: Option<SemanticType>,
}
```

### 6.4 Table (Cluster Method)

Used during cluster-based table detection.

```rust
pub struct Table {
    pub bbox: BoundingBox,
    pub id: Option<u64>,
    pub rows: Vec<TableRow>,
    pub validation_score: Option<f64>,
    pub table_border: Option<TableBorder>,
}

pub struct TableRow {
    pub bbox: BoundingBox,
    pub id: Option<u64>,
    pub cells: Vec<TableCell>,
    pub semantic_type: Option<SemanticType>,
}

pub struct TableCell {
    pub bbox: BoundingBox,
    pub content: Vec<TableTokenRow>,
    pub semantic_type: Option<SemanticType>,
}

pub struct TableToken {
    pub base: TextChunk,
    pub token_type: TableTokenType,
}

#[derive(Debug, Clone, Copy)]
pub enum TableTokenType {
    Image,
    Text,
    Table,
}

pub type TableTokenRow = Vec<TableToken>;
```

### 6.5 TableBordersCollection

Global collection of detected table borders, indexed by page.

```rust
pub struct TableBordersCollection {
    /// page_index → sorted set of TableBorder on that page
    pub table_borders: Vec<BTreeSet<TableBorder>>,
}

impl TableBordersCollection {
    pub fn new(num_pages: usize) -> Self;
    pub fn add(&mut self, page: usize, border: TableBorder);
    pub fn get_page(&self, page: usize) -> &BTreeSet<TableBorder>;
    pub fn get_cell(&self, page: usize, bbox: &BoundingBox) -> Option<&TableBorderCell>;
}
```

---

## 7. List Structures

### 7.1 PDFList

```rust
pub struct PDFList {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,

    pub list_items: Vec<ListItem>,
    pub numbering_style: Option<String>,
    pub common_prefix: Option<String>,

    /// Cross-page linking
    pub previous_list_id: Option<u64>,
    pub next_list_id: Option<u64>,
}
```

### 7.2 ListItem

```rust
pub struct ListItem {
    pub bbox: BoundingBox,
    pub index: Option<u32>,
    pub level: Option<String>,

    pub label: ListLabel,
    pub body: ListBody,
    pub label_length: usize,
    pub contents: Vec<ContentElement>,
    pub semantic_type: Option<SemanticType>,
}
```

### 7.3 ListLabel / ListBody

```rust
pub struct ListLabel {
    pub bbox: BoundingBox,
    pub content: Vec<TableTokenRow>,
    pub semantic_type: Option<SemanticType>,
}

pub struct ListBody {
    pub bbox: BoundingBox,
    pub content: Vec<TableTokenRow>,
    pub semantic_type: Option<SemanticType>,
}
```

### 7.4 ListInterval

Used during list detection to track numbering sequences.

```rust
pub struct ListInterval {
    pub list_indexes: Vec<usize>,
    pub list_item_infos: Vec<ListItemInfo>,
    pub numbering_style: Option<String>,
    pub number_of_columns: Option<usize>,
}

pub struct ListItemInfo {
    pub label_text: String,
    pub sequence_value: i64,  // numeric value for ordering
}
```

---

## 8. Unified Content Element

All page content is stored as a flat `Vec<ContentElement>` per page.

```rust
#[derive(Debug)]
pub enum ContentElement {
    TextChunk(TextChunk),
    TextLine(TextLine),
    TextBlock(TextBlock),
    Image(ImageChunk),
    Line(LineChunk),
    LineArt(LineArtChunk),
    TableBorder(TableBorder),
    List(PDFList),
    Paragraph(SemanticParagraph),
    Heading(SemanticHeading),
    NumberHeading(SemanticNumberHeading),
    Caption(SemanticCaption),
    HeaderFooter(SemanticHeaderOrFooter),
    Figure(SemanticFigure),
    Formula(SemanticFormula),
    Picture(SemanticPicture),
}

impl ContentElement {
    pub fn bbox(&self) -> &BoundingBox;
    pub fn index(&self) -> Option<u32>;
    pub fn page_number(&self) -> Option<u32>;
}
```

---

## 9. Enums

### 9.1 Text Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat {
    Normal,
    Superscript,
    Subscript,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextType {
    Regular,
    Large,
    Logo,
}
```

### 9.2 Processing Layer Enum

Tracks which processing layer added/modified an element.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfLayer {
    Content,
    TableCells,
    ListItems,
    TableContent,
    ListContent,
    TextBlockContent,
    HeaderAndFooterContent,
}
```

### 9.3 Configuration Enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingOrder {
    Off,
    XyCut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableMethod {
    Default,   // border-based only
    Cluster,   // border + cluster detection
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOutput {
    Off,
    Embedded,   // base64 inline
    External,   // separate files
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Markdown,
    MarkdownWithHtml,
    MarkdownWithImages,
    Html,
    Text,
    Pdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridBackend {
    Off,
    Docling,
    Hancom,
    Azure,
    Google,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HybridMode {
    Auto,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriageDecision {
    Java,
    Backend,
}
```

---

## 10. Configuration Structs

### 10.1 Config

```rust
pub struct Config {
    // Input
    pub password: Option<String>,
    pub pages: Option<String>,       // "1-5,8,10-12"

    // Output formats
    pub generate_json: bool,         // default: true
    pub generate_markdown: bool,     // default: false
    pub generate_html: bool,         // default: false
    pub generate_text: bool,         // default: false
    pub generate_pdf: bool,          // default: false
    pub use_html_in_markdown: bool,  // default: false
    pub add_image_to_markdown: bool, // default: false

    // Text processing
    pub keep_line_breaks: bool,      // default: false
    pub replace_invalid_chars: String, // default: " "
    pub use_struct_tree: bool,       // default: false

    // Table
    pub table_method: TableMethod,   // default: Default

    // Reading order
    pub reading_order: ReadingOrder, // default: XyCut

    // Page separators
    pub markdown_page_separator: String, // default: ""
    pub text_page_separator: String,     // default: ""
    pub html_page_separator: String,     // default: ""

    // Images
    pub image_output: ImageOutput,   // default: External
    pub image_format: ImageFormat,   // default: Png
    pub image_dir: Option<String>,

    // Output
    pub output_folder: Option<String>,
    pub include_header_footer: bool, // default: false

    // Filtering
    pub filter_config: FilterConfig,

    // Hybrid
    pub hybrid: HybridBackend,       // default: Off
    pub hybrid_config: HybridConfig,

    // Internal cache
    cached_page_numbers: Option<Vec<u32>>,
}
```

### 10.2 FilterConfig

```rust
pub struct FilterConfig {
    pub filter_hidden_text: bool,     // default: true
    pub filter_out_of_page: bool,     // default: true
    pub filter_tiny_text: bool,       // default: true
    pub filter_hidden_ocg: bool,      // default: true
    pub filter_sensitive_data: bool,  // default: false
    pub filter_rules: Vec<SanitizationRule>,  // 10 default rules
}
```

### 10.3 HybridConfig

```rust
pub struct HybridConfig {
    pub url: Option<String>,
    pub timeout_ms: u32,          // default: 30000
    pub fallback_to_java: bool,   // default: false
    pub max_concurrent_requests: u32, // default: 4
    pub mode: HybridMode,        // default: Auto
}

// Default URLs
const DOCLING_DEFAULT_URL: &str = "http://localhost:5001";
const HANCOM_DEFAULT_URL: &str = "https://dataloader.cloud.hancom.com/studio-lite/api";
const DEFAULT_TIMEOUT_MS: u32 = 30000;
const DEFAULT_MAX_CONCURRENT_REQUESTS: u32 = 4;
```

### 10.4 SanitizationRule

```rust
pub struct SanitizationRule {
    pub pattern: Regex,
    pub replacement: String,
}
```

---

## 11. Statistics and Detection Support

### 11.1 ModeWeightStatistics

Used for heading detection — tracks font size and weight distributions.

```rust
pub struct ModeWeightStatistics {
    /// value (int = font_size * 100) → frequency count
    histogram: HashMap<i32, usize>,
}

impl ModeWeightStatistics {
    pub fn add(&mut self, value: f64);

    /// Most frequent value in [min, max]
    pub fn mode(&self, min: f64, max: f64) -> Option<f64>;

    /// Rarity-based boost score for heading detection
    /// Returns rank position normalized to [0, 1]
    pub fn get_boost(&self, score: f64, score_min: f64, score_max: f64) -> f64;
}
```

### 11.2 TextNodeStatisticsConfig

```rust
pub struct TextNodeStatisticsConfig {
    pub font_size_dominant_min: f64,   // 10.0
    pub font_size_dominant_max: f64,   // 13.0
    pub font_size_heading_min: f64,    // 10.0
    pub font_size_heading_max: f64,    // 32.0
    pub font_size_rarity_boost: f64,   //  0.5

    pub font_weight_dominant_min: f64, // 395.0
    pub font_weight_dominant_max: f64, // 405.0
    pub font_weight_heading_min: f64,  // 400.0
    pub font_weight_heading_max: f64,  // 900.0
    pub font_weight_rarity_boost: f64, //   0.3
}
```

### 11.3 TextStyle

Used for heading level assignment — groups headings by visual style.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextStyle {
    /// Primary sort: larger first (descending)
    pub font_size: OrderedFloat<f64>,
    /// Secondary sort: bolder first (descending)
    pub font_weight: OrderedFloat<f64>,
    /// Tertiary sort: alphabetical
    pub font_name: String,
    /// Quaternary sort: numeric tiebreaker
    pub text_color: [OrderedFloat<f64>; 3],
}
```

---

## 12. Processing Context

Replacement for Java static containers — owns all mutable state during processing.

```rust
pub struct ProcessingContext {
    // Per-page content
    pub pages: Vec<PageContent>,

    // Table detection results
    pub table_borders: TableBordersCollection,

    // Cross-page tracking
    pub all_headings: Vec<HeadingRef>,
    pub all_lists: Vec<ListRef>,

    // Statistics (built incrementally)
    pub font_size_stats: ModeWeightStatistics,
    pub font_weight_stats: ModeWeightStatistics,

    // ID counter
    pub next_id: u32,

    // Configuration
    pub config: Config,
}

pub struct PageContent {
    pub page_number: u32,
    pub width: f64,
    pub height: f64,
    pub crop_box: BoundingBox,
    pub elements: Vec<ContentElement>,
}
```

---

## 13. Output Schema (JSON)

See [schema.json](../schema.json) for the full JSON Schema.

The JSON output uses different field names than the internal Rust field names:

```
Internal Field              JSON Field Name
--------------------------  ------------------
content_element.type        "type"
paragraph                   "paragraph"
heading                     "heading"
heading.heading_level       "level"
caption                     "caption"
caption.linked_content_id   "linkedContentId"
table                       "table"
table.rows                  "rows"
table_cell.row_span         "rowSpan"
table_cell.col_span         "colSpan"
text_block                  "textBlock"
list                        "list"
list.list_items             "items"
list_item.label             "label"
list_item.body              "body"
image                       "image"
header_footer               "headerFooter"
header_footer.type          "headerFooterType"
bbox.left_x                 "leftX"
bbox.bottom_y               "bottomY"
bbox.right_x                "rightX"
bbox.top_y                  "topY"
text_properties.font_name   "fontName"
text_properties.font_size   "fontSize"
text_properties.font_weight "fontWeight"
text_properties.font_color  "fontColor"
text_properties.text_format "textFormat"
```

For full JSON Schema structure, refer to [08-output-formats](08-output-formats.md).
