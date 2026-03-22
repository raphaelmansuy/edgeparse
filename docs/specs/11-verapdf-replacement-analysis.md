# Spec 11 — veraPDF Replacement Analysis for Rust Migration

## Executive Summary

This document provides a comprehensive analysis of Rust crate alternatives to replace all veraPDF Java dependencies used by opendataloader-pdf. The analysis covers 12 candidate crates, maps them against 10 capability categories required by the 20-stage processing pipeline, identifies 6 critical roadblocks, and recommends a phased migration strategy.

**Key Finding**: A pure-Rust replacement is achievable for ~85% of veraPDF's functionality using existing crates. The remaining ~15% (structure tree walking, image extraction with positions) requires either custom implementation (~1,300 lines) or selective use of `pdfium-render` as an FFI bridge.

---

## Table of Contents

- [1. Scope of veraPDF Usage](#1-scope-of-verapdf-usage)
- [2. Crate Inventory and Evaluation](#2-crate-inventory-and-evaluation)
- [3. Capability Gap Analysis](#3-capability-gap-analysis)
- [4. Roadblocks and Mitigations](#4-roadblocks-and-mitigations)
- [5. Recommended Strategy](#5-recommended-strategy)
- [6. Dependency Graph](#6-dependency-graph)
- [7. Risk Matrix](#7-risk-matrix)
- [8. Decision Log](#8-decision-log)

---

## 1. Scope of veraPDF Usage

### 1.1 Maven Dependencies

opendataloader-pdf depends on two veraPDF artifacts:

| Artifact | Version Range | Role |
|---|---|---|
| `validation-model` | `[1.31.0, 1.32.0-RC)` | PDF parsing, content stream processing, object model |
| `wcag-validation` | `[1.31.0, 1.32.0-RC)` | WCAG accessibility validation, structure tree walking |

### 1.2 Type Usage Summary

The Java codebase uses **~50 veraPDF types** across 5 categories:

| Category | Types Used | Pipeline Stages Affected |
|---|---|---|
| Core Interfaces | `IObject`, `INode`, `ITree`, `BaseObject`, `IChunk` | All stages |
| Content Chunks | `TextChunk`, `TextLine`, `TextBlock`, `TextColumn`, `ImageChunk`, `LineChunk`, `LineArtChunk` | Stages 1–6 |
| Semantic Nodes | `SemanticTextNode`, `SemanticParagraph`, `SemanticHeading`, `SemanticHeaderOrFooter`, `SemanticCaption`, `SemanticFigure`, `SemanticSpan` | Stages 9–14 |
| Table Types | `TableBorder`, `TableBorderRow`, `TableBorderCell`, `TableBordersCollection`, `Table`, `TableToken`, `TableChecker` | Stages 7–8 |
| List Types | `PDFList`, `ListItem` | Stage 15 |

### 1.3 Critical Operations veraPDF Performs

1. **Content stream parsing** — Interprets PDF operators (Tj, TJ, Tm, Td, cm, etc.) to extract text with positions
2. **Font decoding** — Handles Type1, TrueType, CIDFont, Type0 with CMap/ToUnicode/encoding tables
3. **Per-character bounding boxes** — Computes precise character positions via transformation matrices (Tsm × Tm × CTM)
4. **Image extraction** — Extracts XObject images with position/dimension data
5. **Line/path extraction** — Captures line segments and paths for table border detection
6. **Structure tree walking** — Traverses /StructTreeRoot for tagged PDF semantics
7. **Page geometry** — MediaBox, CropBox, rotation, coordinate transforms
8. **Color space handling** — DeviceRGB, DeviceCMYK, DeviceGray, ICCBased, Separation, CalGray/CalRGB/Lab

---

## 2. Crate Inventory and Evaluation

### 2.1 PDF Document Parsing

#### `lopdf` 0.39.0 — Low-Level PDF Manipulation

| Attribute | Value |
|---|---|
| **Downloads** | 5,000,000+ |
| **License** | MIT |
| **SLoC** | 16,000 |
| **MSRV** | 1.85 |
| **Maintenance** | Active |

**Capabilities**:
- ✅ PDF object access (Object, Dictionary, Stream, Array, Reference)
- ✅ Cross-reference table and object stream parsing
- ✅ Content stream decoding (`Content` → `Vec<Operation>`)
- ✅ Stream decompression (FlateDecode, LZWDecode, ASCII85Decode)
- ✅ PDF encryption/decryption (RC4, AES, up to PDF 2.0 — empty password auto-decrypt)
- ✅ Document modification and saving
- ✅ PDF merging
- ❌ No font decoding or text extraction
- ❌ No per-character positioning
- ❌ No structure tree interpretation

**Role in migration**: Foundation layer for low-level PDF object access.

---

#### `pdf` (pdf-rs) 0.10.0 — Typed PDF Reader

| Attribute | Value |
|---|---|
| **Downloads** | 487,000 |
| **License** | MIT |
| **SLoC** | 11,000 |
| **Stars** | 1,600 (GitHub) |
| **Maintenance** | Active (updated 16 days ago, 37 contributors) |

**Capabilities**:
- ✅ Typed content stream parsing: `Op` enum with **45 variants** covering all PDF content stream operators
- ✅ `Content` struct representing parsed content streams
- ✅ Font module: `Font`, `CIDFont`, `Type0Font`, `FontDescriptor`, `ToUnicodeMap`, `Widths`
- ✅ Matrix/Point/ViewRect geometry types
- ✅ Color types (RGB, CMYK, generic)
- ✅ `FormXObject` for XObject form handling
- ✅ `ImageXObject` for inline images
- ✅ `parse_ops()` and `serialize_ops()` for content stream round-tripping
- ❌ No text extraction with positions (that's handled by pdf-extract)

**Key types mapping to veraPDF**:

| pdf-rs Type | veraPDF Equivalent | Notes |
|---|---|---|
| `Op::TextDraw` | Tj operator in parseChunks | Direct mapping |
| `Op::TextDrawAdjusted` | TJ operator | Direct mapping |
| `Op::SetTextMatrix` | Tm operator | Direct mapping |
| `Op::Transform` | cm operator | Direct mapping |
| `Op::BeginMarkedContent` | BMC/BDC | Tagged PDF markers |
| `Op::XObject` | Do operator | Image/form XObjects |
| `Op::InlineImage` | BI/ID/EI operators | Inline images |
| `Op::MoveTo/LineTo/CurveTo` | Path operators | Line segment data |
| `Font`, `CIDFont` | Font handling | ToUnicode support |

**Role in migration**: Alternative to pdf-extract for typed content stream access. Can be used for higher-level PDF object operations.

---

#### `pdf-extract` 0.10.0 — Text Extraction with Positions

| Attribute | Value |
|---|---|
| **Downloads** | 883,000 |
| **License** | MIT |
| **SLoC** | 2,416 lines (single lib.rs) |
| **Foundation** | Built on `lopdf` |
| **Maintenance** | Active (last commit: 1 month ago) |

**This is the most critical crate for the migration.** It implements a near-complete PDF content stream interpreter in Rust.

**Capabilities** (verified by source code analysis):

| Feature | Implementation Detail |
|---|---|
| **Content stream operators** | BT/ET, Tj, TJ, Td, TD, T*, Tm, Tf, Tc, Tw, Tz, TL, Ts, cm, q, Q, gs, Do, BMC/BDC/EMC |
| **Font types** | `PdfSimpleFont` (Type1, TrueType), `PdfType3Font`, `PdfCIDFont` (Type0/CID) |
| **CMap/encoding** | `adobe_cmap_parser` for CMap, `type1_encoding_parser` for Type1, `cff_parser` for CFF |
| **Unicode mapping** | `glyphnames` crate for glyph name → Unicode, ToUnicode CMap streams |
| **Standard encodings** | WinAnsi, MacRoman, PDFDocEncoding, built-in core font metrics |
| **Transform chain** | Full Tsm × Tm × CTM computation per character |
| **Per-char output** | `OutputDev::output_character(&trm, width, spacing, font_size, &char_string)` |
| **Path operations** | `PathOp::MoveTo`, `LineTo`, `CurveTo`, `Close`, `Rect` |
| **Stroke/Fill** | `OutputDev::stroke/fill(ctm, colorspace, color, path)` |
| **Graphics state** | Full q/Q stack, CTM, SMask, line_width |
| **Color spaces** | DeviceRGB/CMYK/Gray, Pattern, Separation, ICCBased, CalGray/CalRGB/Lab |
| **XObject forms** | Recursive `process_stream()` for form XObjects via Do operator |
| **Core font metrics** | Built-in width tables for 14 standard PDF fonts |
| **Encryption** | Via lopdf's decryption (empty password auto, user password explicit) |

**OutputDev trait** — the key integration point:
```rust
pub trait OutputDev {
    fn begin_page(&mut self, page_num: u32, media_box: &MediaBox, art_box: Option<ArtBox>);
    fn end_page(&mut self);
    fn output_character(&mut self, trm: &Transform, width: f64, spacing: f64, 
                        font_size: f64, char: &str);
    fn begin_word(&mut self);
    fn end_word(&mut self);
    fn end_line(&mut self);
    fn stroke(&mut self, ctm: &Transform, colorspace: &ColorSpace, 
              color: &[f64], path: &Path);
    fn fill(&mut self, ctm: &Transform, colorspace: &ColorSpace, 
            color: &[f64], path: &Path);
}
```

**Critical gaps**:
- ❌ No image XObject extraction (Do operator only recurses into form XObjects, ignores image XObjects)
- ❌ No structure tree walking
- ❌ No text grouping into lines/blocks/columns (individual characters only)
- ❌ No font ascent/descent metrics for bounding box height computation

**Role in migration**: Primary content stream interpreter. Fork and extend for image extraction and bounding box computation.

---

### 2.2 PDF via FFI Bridge

#### `pdfium-render` 0.8.37 — Pdfium Wrapper (Google Chromium)

| Attribute | Value |
|---|---|
| **Downloads** | 802,000 |
| **License** | MIT / Apache-2.0 |
| **SLoC** | 229,000 |
| **Pdfium** | C++ library (Google Chromium) |
| **Maintenance** | Very active (37 releases in 0.8.x) |

**Capabilities**:

| Feature | API | Notes |
|---|---|---|
| **Text with positions** | `PdfPageTextChar` | `loose_bounds()`, `tight_bounds()`, `unicode_char()`, `font_size()`, `is_generated()`, `is_hyphen()` |
| **Image extraction** | `PdfPageImageObject` | `get_raw_image_data()`, `width()`, `height()`, bounds via transforms |
| **Page objects** | `PdfPageObjects` | Text, Image, Path, Form, Shading object introspection |
| **Structure tree** | `FPDF_StructTree_*`, `FPDF_StructElement_*` | Tagged PDF support |
| **Annotations** | `PdfPageAnnotation*` | Full annotation support |
| **Form fields** | `PdfFormField*` | Reading and filling |
| **Bookmarks** | `PdfBookmark` | Navigation tree |
| **Rendering** | `PdfPage::render()` | Pages to bitmaps |
| **Document creation** | `PdfDocument` | Create new PDFs |
| **Thread safety** | Mutex-based | All calls serialized |
| **WASM** | Supported | With external Pdfium WASM module |

**Critical advantages over pure-Rust crates**:
1. `PdfPageTextChar` provides **bounding boxes directly** — no manual computation needed
2. `PdfPageImageObject` provides **image data with position** — complete image extraction
3. Structure tree API via `FPDF_StructTree_*` — tagged PDF support out of the box

**Critical disadvantages**:
1. **Requires external Pdfium binary** (~20–30 MB, platform-specific)
2. **Not pure Rust** — C++ FFI dependency
3. **All calls serialized through mutex** — no parallel page processing within a single Pdfium instance
4. **229K SLoC** — large dependency surface area

**Role in migration**: Fallback/complement for capabilities that pure-Rust crates lack (structure tree, image extraction).

---

#### `mupdf` 0.6.0 — MuPDF Binding

| Attribute | Value |
|---|---|
| **License** | **AGPL-3.0** |
| **Status** | "Working in progress" |

**❌ ELIMINATED**: AGPL-3.0 license is incompatible with the project's MIT/Apache-2.0 licensing model. The "working in progress" status also indicates immaturity.

---

### 2.3 Font Processing

#### `ttf-parser` 0.25.1 — Zero-Allocation Font Parser

| Attribute | Value |
|---|---|
| **Downloads** | 56,488,000+ |
| **License** | MIT / Apache-2.0 |
| **SLoC** | 19,000 |
| **Dependencies** | Zero |
| **Unsafe** | Zero |
| **Heap allocs** | Zero |

**Capabilities**:
- ✅ TrueType, OpenType, AAT font parsing
- ✅ `cmap` table: glyph index ↔ codepoint mapping (formats 0, 2, 4, 6, 10, 12, 13, 14)
- ✅ CFF and CFF2 table support (outline extraction)
- ✅ `hmtx`/`vmtx`: horizontal/vertical glyph metrics (advance width, side bearing)
- ✅ `OS/2` table: sTypoAscender, sTypoDescender, sCapHeight, xHeight
- ✅ `head` table: unitsPerEm, fontBBox
- ✅ `name` table: family name, style
- ✅ `glyf` table: glyph outlines
- ✅ Variable fonts (fvar, gvar, HVAR, VVAR)
- ✅ GPOS/GSUB tables (positioning, substitution)
- ✅ `no_std` compatible, WASM compatible

**Role in migration**: Parse embedded TrueType/OpenType font programs to extract precise glyph metrics (ascent, descent, advance width) for bounding box computation. Supplements pdf-extract's font handling.

---

#### `skrifa` 0.40.0 — Google Fonts Glyph Scaler

| Attribute | Value |
|---|---|
| **Downloads** | 7,100,000+ |
| **License** | MIT / Apache-2.0 |
| **SLoC** | 28,000 |
| **Foundation** | Built on `read-fonts` |
| **Unsafe** | `#![forbid(unsafe_code)]` |

**Capabilities**:
- ✅ Global font metrics with variation support
- ✅ Per-glyph metrics with variation support (advance width, LSB)
- ✅ Codepoint → glyph ID mapping (including Unicode variation sequences)
- ✅ glyf, CFF, CFF2 outline extraction with hinting
- ✅ COLRv0/v1 color font support
- ✅ Part of Google's `fontations` project (actively maintained)

**Role in migration**: Higher-level alternative to `ttf-parser` for glyph metrics. More feature-rich but heavier.

---

#### `read-fonts` 0.37.0 — Google Fonts OpenType Reader

| Attribute | Value |
|---|---|
| **Downloads** | 7,893,000+ |
| **License** | MIT / Apache-2.0 |
| **SLoC** | 68,000 |
| **Unsafe** | `#![forbid(unsafe_code)]` |

**Role in migration**: Low-level foundation for `skrifa`. Direct use unlikely — prefer `skrifa` for high-level API or `ttf-parser` for minimal dependency.

---

### 2.4 Supporting Crates

#### `adobe_cmap_parser` 0.4.1

| Attribute | Value |
|---|---|
| **Downloads** | 1,921,000+ |
| **License** | MIT |
| **SLoC** | 302 |

Parses Adobe CMap files for CID font encoding. Already used by `pdf-extract`. Provides `get_unicode_map()` and `get_byte_mapping()`.

---

#### `subsetter` 0.2.3

| Attribute | Value |
|---|---|
| **Downloads** | 1,164,000+ |
| **License** | MIT / Apache-2.0 |
| **SLoC** | 3,600 |
| **Dependencies** | 1 (rustc-hash) |

OpenType font subsetter for PDF embedding. Part of Typst ecosystem. Relevant for output generation, not parsing.

---

#### `image` 0.25.10

| Attribute | Value |
|---|---|
| **Downloads** | 107,000,000+ |
| **License** | MIT / Apache-2.0 |
| **SLoC** | 27,000 |

Standard Rust image processing library. Handles JPEG, PNG, TIFF, BMP, WebP, etc. Required for decoding extracted image data from PDF streams.

---

#### `euclid` — 2D Geometry

Used by `pdf-extract` for `Transform2D` (2D affine transformation matrices). Provides the `vec2()`, `Transform2D::row_major()`, `pre_transform()`, `post_transform()`, `create_translation()` operations needed for the PDF text positioning chain.

---

## 3. Capability Gap Analysis

### 3.1 Coverage Matrix

| # | Capability | veraPDF Types | Pure Rust Coverage | FFI Coverage | Gap Level |
|---|---|---|---|---|---|
| C1 | PDF object access | IObject, BaseObject | lopdf ✅ | pdfium ✅ | **None** |
| C2 | Content stream parsing | parseChunks() | pdf-extract ✅ | pdfium ✅ | **None** |
| C3 | Font decoding | CMap, ToUnicode, encodings | pdf-extract ✅ (3 font types, CMap, Type1, CFF) | pdfium ✅ | **None** |
| C4 | Per-char positions | TextChunk.getBoundingBox() | pdf-extract ✅ (transforms) | pdfium ✅ (PdfPageTextChar) | **Low** (see §4.3) |
| C5 | Image extraction | ImageChunk + position | lopdf ⚠️ (manual) | pdfium ✅ | **Medium** |
| C6 | Line/path extraction | LineChunk, LineArtChunk | pdf-extract ✅ (PathOp + stroke/fill) | pdfium ✅ | **None** |
| C7 | Structure tree | INode, ITree | lopdf ⚠️ (manual walk) | pdfium ✅ (FPDF_StructTree_*) | **High** |
| C8 | Page geometry | MediaBox, CropBox, rotation | lopdf ✅, pdf-extract ✅ | pdfium ✅ | **None** |
| C9 | Text grouping | TextLine, TextBlock, TextColumn | None (custom algo) | None (custom algo) | **Expected** |
| C10 | Table detection | TableBorder, Table, etc. | None (custom algo) | None (custom algo) | **Expected** |

### 3.2 Gap Detail

**No Gap (C1, C2, C3, C6, C8)**: These capabilities are fully covered by existing Rust crates with mature, battle-tested implementations.

**Low Gap (C4)**: pdf-extract provides per-character transform matrices (`trm`) that encode position, but computing a full bounding box requires font ascent/descent metrics. See Roadblock §4.3.

**Medium Gap (C5)**: Image extraction requires custom code to intercept the Do operator for Image XObjects (currently pdf-extract only handles Form XObjects). See Roadblock §4.2.

**High Gap (C7)**: No pure-Rust crate provides structure tree traversal. This is the single largest gap. See Roadblock §4.1.

**Expected (C9, C10)**: Text grouping and table detection are application-specific algorithms that no library provides. These must be reimplemented regardless of the PDF parsing library used. The Java implementations serve as specifications.

---

## 4. Roadblocks and Mitigations

### 4.1 ROADBLOCK 1: Structure Tree Walking (Tagged PDF Path)

**Severity**: 🔴 HIGH  
**Pipeline Impact**: Stage 11 (TaggedDocumentProcessor), Stage 12 (SemanticClassification)  
**Description**: The Tagged PDF processing path requires walking the PDF structure tree (/StructTreeRoot → /K → /S, /C, /Pg, etc.). No pure-Rust crate exposes a structure tree API.

**Technical Detail**:
The Java code uses `INode.getChildren()`, `INode.getSemanticType()`, and `ITree.getRoot()` to traverse the structure tree. The structure tree follows a well-defined hierarchy specified in PDF 1.7 (ISO 32000-1:2008, §14.7):

```
/StructTreeRoot (dictionary)
  ├── /K → structure element(s) (array or dict)
  │     ├── /S → structure type (name: Document, Part, Sect, H1-H6, P, Table, TR, TD, etc.)
  │     ├── /C → class map (name or array)
  │     ├── /Pg → page reference
  │     ├── /K → children (array of elements, MCIDs, or object references)
  │     │     ├── integer → marked content ID (MCID)
  │     │     ├── dictionary → child structure element
  │     │     └── stream → content stream reference
  │     ├── /A → attribute objects
  │     └── /Alt → alternate text
  └── /ParentTree → number tree mapping MCIDs to structure elements
```

**Mitigation Options**:

| Option | Effort | Risk | Recommendation |
|---|---|---|---|
| **A. Custom walker on lopdf** | ~800 lines | Medium (edge cases in malformed PDFs) | ✅ **Recommended for Phase 2** |
| **B. Use pdfium-render** | ~50 lines | Low (proven API) | ✅ Acceptable fallback |
| **C. Contribute to pdf-rs** | Unknown | High (upstream acceptance) | ❌ Not recommended |

**Option A Implementation Outline**:
```rust
struct StructureTree {
    root: StructureElement,
}

struct StructureElement {
    struct_type: String,          // /S value
    children: Vec<StructChild>,   // /K children
    page: Option<ObjectId>,       // /Pg reference
    attributes: Vec<Attribute>,   // /A values
    alt_text: Option<String>,     // /Alt
    actual_text: Option<String>,  // /ActualText
    mcids: Vec<i64>,             // Marked content IDs
}

enum StructChild {
    Element(StructureElement),
    MarkedContent(i64),           // MCID integer
    ObjectReference(ObjectId),
}

impl StructureTree {
    fn from_document(doc: &lopdf::Document) -> Result<Self, Error> {
        let catalog = doc.catalog()?;
        let struct_tree_root = catalog.get(b"StructTreeRoot")?;
        // Recursive traversal of /K arrays
        // ...
    }
}
```

---

### 4.2 ROADBLOCK 2: Image XObject Extraction with Position Data

**Severity**: 🟡 MEDIUM  
**Pipeline Impact**: Stage 3 (ImageExtraction), output generation  
**Description**: pdf-extract processes the `Do` operator but only recurses into Form XObjects, ignoring Image XObjects. Image data and position must be extracted separately.

**Technical Detail**:
In the Java code, `ImageChunk` captures:
- `getBoundingBox()` — position from CTM at the time of the Do operator
- Color space, bits per component, width, height
- Decoded image bytes (after filter decompression)

The position of an image XObject is determined by the CTM at the time the `Do` operator is executed. The image occupies a 1×1 unit square in user space, transformed by the CTM.

**Mitigation**:

```rust
// Extension to pdf-extract's Processor::process_stream()
"Do" => {
    let xobject: &Dictionary = get(&doc, resources, b"XObject");
    let name = operation.operands[0].as_name().unwrap();
    let obj = doc.get_object(xobject.get(name)?.as_reference()?)?;
    
    match obj.as_stream() {
        Ok(stream) => {
            let subtype = stream.dict.get(b"Subtype")?.as_name()?;
            match subtype {
                b"Form" => { /* existing recursive processing */ }
                b"Image" => {
                    let width = stream.dict.get(b"Width")?.as_i64()?;
                    let height = stream.dict.get(b"Height")?.as_i64()?;
                    let cs = stream.dict.get(b"ColorSpace");
                    let bpc = stream.dict.get(b"BitsPerComponent")?.as_i64()?;
                    let data = get_contents(stream); // decompress
                    
                    // Image occupies [0,0]→[1,1] in user space
                    // CTM transforms this to page coordinates
                    output.image(&gs.ctm, width, height, cs, bpc, &data)?;
                }
                _ => {}
            }
        }
        Err(_) => {}
    }
}
```

**Effort**: ~300–500 lines. Also requires adding an `image()` callback to the `OutputDev` trait.  
**Risk**: Low. The PDF specification for image XObjects is straightforward. The main complexity is in handling filter chains (FlateDecode, DCTDecode, CCITTFaxDecode, JBIG2Decode, JPXDecode).

- `FlateDecode` → `flate2` crate (widely used, reliable)
- `DCTDecode` → already JPEG data, pass through to `image` crate
- `CCITTFaxDecode` → `fax` crate or manual implementation
- `JBIG2Decode` → No pure-Rust implementation; rare in modern PDFs
- `JPXDecode` → JPEG 2000; `jpeg2k` crate (limited maturity)

---

### 4.3 ROADBLOCK 3: Font Metrics for Bounding Box Height

**Severity**: 🟢 LOW–MEDIUM  
**Pipeline Impact**: Stage 1 (TextExtraction), all subsequent text processing  
**Description**: pdf-extract provides per-character position (x, y) and width via transforms, but computing bounding box *height* requires font ascent/descent metrics.

**Technical Detail**:
The transform `trm` gives the baseline position. To compute a full bounding box:
```
bbox.x = trm.m31 (translated x)
bbox.y = trm.m32 (translated y) 
bbox.width = char_width × font_size (from trm transform)
bbox.height = (ascent - descent) × font_size (need font metrics)
```

**Mitigation**:

| Source | Availability | Precision |
|---|---|---|
| `/FontDescriptor` → `/Ascent`, `/Descent` | Always present (PDF spec requirement) | Good (±5%) |
| Embedded font program via `ttf-parser` → `OS/2.sTypoAscender` | Only when font is embedded | Excellent |
| Core font metrics (14 standard fonts) | Always available | Exact |
| Fallback heuristic: `ascent ≈ 0.8 × fontSize` | Always available | Rough (±15%) |

**Implementation**: ~100–200 lines. Read `/Ascent` and `/Descent` from the font descriptor dictionary via lopdf. For embedded fonts, optionally parse with `ttf-parser` for precise metrics.

---

### 4.4 ROADBLOCK 4: CMap/ToUnicode Edge Cases

**Severity**: 🟢 LOW  
**Pipeline Impact**: Stage 1 (TextExtraction)  
**Description**: Complex CJK fonts may use multi-byte CMap encodings, vertical writing modes, or supplementary Unicode planes.

**Current coverage in pdf-extract**:
- ✅ Identity-H and Identity-V encodings
- ✅ Multi-byte CID codes (1–4 byte variable width)
- ✅ ToUnicode CMap streams (beginbfchar, beginbfrange)
- ✅ Adobe CMap files via `adobe_cmap_parser`
- ✅ Type1C (CFF) encoding via `cff_parser`
- ✅ Standard encodings (WinAnsi, MacRoman, MacExpert)
- ⚠️ Surrogate pair filtering (D800–DFFF range handled but deserves testing)

**Mitigation**: Extensive testing with CJK-heavy PDFs. The existing `pdf-extract` implementation covers the vast majority of real-world cases. Edge cases can be addressed incrementally.

---

### 4.5 ROADBLOCK 5: Encrypted PDF Handling

**Severity**: 🟢 LOW  
**Pipeline Impact**: Document loading  
**Description**: lopdf supports PDF decryption (Standard Security Handler, RC4, AES-128, AES-256) but defaults to empty password. User-supplied passwords require explicit API call.

**Mitigation**: Trivially handled. lopdf's `Document::load()` + `doc.decrypt(password)` covers all standard encryption methods. The CLI already accepts a password option.

---

### 4.6 ROADBLOCK 6: Performance — Parallel Page Processing

**Severity**: 🟡 MEDIUM  
**Pipeline Impact**: Batch processing throughput  
**Description**: If using `pdfium-render`, all Pdfium calls are serialized through a mutex. This prevents parallel page processing within a single document.

**Mitigation Options**:

| Option | Description | Impact |
|---|---|---|
| **A. Pure-Rust path** | Use pdf-extract/lopdf (no mutex) | Full `rayon` parallelism across pages |
| **B. Process-level parallelism** | Spawn separate processes per document | High memory, but avoids mutex |
| **C. Multiple Pdfium instances** | One per thread (if supported) | Complex lifetime management |

**Recommendation**: The pure-Rust path (Option A) naturally supports `rayon::par_iter()` for parallel processing across pages since lopdf and pdf-extract use standard Rust ownership. This is the primary reason to prefer the pure-Rust approach.

---

## 5. Recommended Strategy

### 5.1 Progressive Pure-Rust Migration (Recommended)

```
Phase 1: Core Content Parsing (~70% coverage)
├── pdf-extract (fork) → content stream + fonts + text positions + paths
├── lopdf → PDF document loading, object access
├── euclid → 2D transforms
├── adobe_cmap_parser → CMap parsing
├── image → image format decoding
└── ttf-parser → embedded font metrics

Phase 2: Structure Tree (+15% coverage)  
└── Custom walker on lopdf → /StructTreeRoot traversal (~800 lines)

Phase 3: Image Extraction (+10% coverage)
└── pdf-extract extension → image XObject extraction with CTM (~500 lines)

Phase 4: Validation & Optimization
├── rayon → parallel page processing
├── Benchmark against Java baseline
└── Edge case testing (CJK, encrypted, malformed PDFs)

Phase 5: Optional Pdfium Fallback
└── pdfium-render → opt-in alternative for complex PDFs
```

### 5.2 Alternative: Pdfium-First Strategy

If development speed is prioritized over pure-Rust goals:

```
Primary: pdfium-render
├── Text extraction → PdfPageTextChar (char-level bounds)
├── Image extraction → PdfPageImageObject
├── Structure tree → FPDF_StructTree_* / FPDF_StructElement_*
├── Path/lines → PdfPagePathObject
└── Page geometry → PdfPage

Secondary: lopdf
└── Low-level object access for edge cases

Trade-off: Requires distributing Pdfium binary (~20-30 MB per platform)
          All calls serialized (no rayon parallelism within Pdfium)
```

### 5.3 Decision Framework

| Criterion | Pure-Rust (§5.1) | Pdfium-First (§5.2) |
|---|---|---|
| **Time to first working prototype** | 6–8 weeks | 2–3 weeks |
| **Distribution size** | ~5 MB binary | ~25–35 MB (+ Pdfium) |
| **Cross-compilation** | Trivial (cargo build) | Complex (need Pdfium for each target) |
| **WASM deployment** | Native support | Possible but complex |
| **Parallel processing** | Full rayon support | Serialized through mutex |
| **Maintenance burden** | ~1,300 lines custom code | Dependent on Pdfium releases |
| **Edge case coverage** | Good (with testing) | Excellent (Chromium-tested) |
| **License compatibility** | MIT/Apache-2.0 throughout | MIT/Apache-2.0 (Pdfium: BSD-3-Clause) |

---

## 6. Dependency Graph

### 6.1 Pure-Rust Strategy Dependency Tree

```
opendataloader-pdf-rs
├── lopdf 0.39.0 .................. PDF document loading, object access
│   ├── flate2 ................... FlateDecode decompression
│   └── aes / sha2 .............. PDF encryption/decryption
├── pdf-extract 0.10.0 (fork) .... Content stream interpreter
│   ├── lopdf .................... (shared)
│   ├── adobe_cmap_parser 0.4.1 .. CMap parsing for CID fonts
│   ├── type1_encoding_parser .... Type1 font encoding
│   ├── cff_parser ............... CFF font parsing
│   ├── glyphnames ............... Glyph name → Unicode
│   ├── euclid ................... 2D transforms
│   └── encoding_rs .............. Text encoding
├── ttf-parser 0.25.1 ............ Embedded font metrics (ascent/descent)
├── image 0.25.10 ................ Image format encoding/decoding
├── serde + serde_json ........... JSON output format
├── rayon ....................... Parallel page processing
├── reqwest + tokio .............. HTTP client (Hybrid mode)
├── clap ........................ CLI argument parsing
└── [custom modules]
    ├── struct_tree (~800 lines) . Structure tree walker
    └── image_extract (~500 lines) Image XObject extraction
```

### 6.2 Crate Compatibility Matrix

| Crate | MSRV | License | no_std | WASM | Unsafe |
|---|---|---|---|---|---|
| lopdf 0.39.0 | 1.85 | MIT | ❌ | ❌ | Some |
| pdf-extract 0.10.0 | 1.60+ | MIT | ❌ | ❌ | Minimal |
| ttf-parser 0.25.1 | 1.59 | MIT/Apache-2.0 | ✅ | ✅ | ❌ |
| skrifa 0.40.0 | 1.85 | MIT/Apache-2.0 | ❌ | ✅ | ❌ |
| adobe_cmap_parser 0.4.1 | 1.40+ | MIT | ❌ | ❌ | Minimal |
| image 0.25.10 | 1.80 | MIT/Apache-2.0 | ❌ | ⚠️ | Minimal |
| subsetter 0.2.3 | 1.82 | MIT/Apache-2.0 | ❌ | ✅ | ❌ |
| euclid | 1.56+ | MIT/Apache-2.0 | ✅ | ✅ | ❌ |

**MSRV constraint**: lopdf 0.39.0 requires Rust 1.85. This sets the project-wide MSRV.

---

## 7. Risk Matrix

| Risk | Probability | Impact | Mitigation | Residual Risk |
|---|---|---|---|---|
| Structure tree edge cases in malformed PDFs | Medium | High | Extensive test suite with PDF/UA reference PDFs | Low |
| CJK font encoding failures | Low | Medium | Test with CJK corpus; fallback to raw bytes | Low |
| Image filter unsupported (JBIG2, JPX) | Low | Low | Log warning, skip unsupported images | Negligible |
| lopdf breaking changes | Low | Medium | Pin version, fork if needed | Low |
| pdf-extract API instability | Medium | Medium | Fork and maintain internally | Low |
| Pdfium binary distribution (if using §5.2) | N/A (pure Rust) | N/A | Not applicable in recommended strategy | N/A |
| Performance regression vs. Java | Low | Medium | Benchmark at each phase; Rust is typically faster | Low |
| Parallel processing contention | Low | Low | lopdf is thread-safe for read operations | Negligible |

---

## 8. Decision Log

| # | Decision | Rationale | Date |
|---|---|---|---|
| D1 | Eliminate `mupdf` crate | AGPL-3.0 license incompatible with MIT/Apache-2.0 project | 2025-07-17 |
| D2 | Use `pdf-extract` as primary content stream interpreter | 2,416-line implementation covering all PDF text operators, font types, and coordinate transforms; built on lopdf | 2025-07-17 |
| D3 | Prefer `ttf-parser` over `skrifa` for font metrics | Zero dependencies, zero allocations, zero unsafe; sufficient for ascent/descent extraction; lighter than skrifa (19K vs 28K+68K SLoC) | 2025-07-17 |
| D4 | Recommend pure-Rust strategy over Pdfium-first | Eliminates C++ dependency, enables rayon parallelism, simplifies distribution, reduces binary size by ~20 MB | 2025-07-17 |
| D5 | Fork `pdf-extract` rather than contribute upstream | Need to add image extraction callback, bounding box computation, and potentially break API — faster to fork than negotiate upstream | 2025-07-17 |
| D6 | Retain `pdfium-render` as optional Phase 5 fallback | Provides battle-tested structure tree and image extraction for edge cases; can be feature-gated | 2025-07-17 |
| D7 | Use `lopdf` for structure tree walking (custom code) | ~800 lines of well-specified code vs. adding Pdfium dependency just for structure tree | 2025-07-17 |
| D8 | Set project MSRV to 1.85 | Driven by lopdf 0.39.0 requirement; all other crates compatible | 2025-07-17 |

---

## Appendix A: veraPDF Type → Rust Crate Mapping

| veraPDF Type | Rust Replacement | Source |
|---|---|---|
| `IObject` | Custom `PdfObject` trait | Project-defined |
| `INode` | Custom `StructureElement` | Built on lopdf |
| `ITree` | Custom `StructureTree` | Built on lopdf |
| `BaseObject` | Custom struct with `BoundingBox` | Project-defined |
| `IChunk` | Custom `ContentChunk` enum | Project-defined |
| `TextChunk` | `pdf-extract` OutputDev + custom struct | pdf-extract fork |
| `TextLine` | Custom grouping algorithm | Project-defined |
| `TextBlock` | Custom grouping algorithm | Project-defined |
| `TextColumn` | Custom grouping algorithm | Project-defined |
| `ImageChunk` | Custom struct from Do/Image handler | pdf-extract extension |
| `LineChunk` | `PathOp::LineTo` + CTM + stroke callback | pdf-extract |
| `LineArtChunk` | `PathOp::CurveTo` + fill callback | pdf-extract |
| `SemanticTextNode` | Custom enum | Project-defined |
| `SemanticParagraph` | Custom struct | Project-defined |
| `SemanticHeading` | Custom struct with level | Project-defined |
| `SemanticCaption` | Custom struct | Project-defined |
| `SemanticFigure` | Custom struct | Project-defined |
| `TableBorder` | Custom struct from line segments | Project-defined |
| `TableBorderRow` | Custom struct | Project-defined |
| `TableBorderCell` | Custom struct | Project-defined |
| `Table` | Custom struct | Project-defined |
| `PDFList` | Custom struct | Project-defined |
| `ListItem` | Custom struct | Project-defined |
| `BoundingBox` | `euclid::Rect<f64>` or custom | euclid / Project |
| `Transform` | `euclid::Transform2D<f64>` | euclid |
| `MediaBox` | `pdf-extract::MediaBox` | pdf-extract |
| Font handling | `PdfSimpleFont`, `PdfCIDFont`, `PdfType3Font` | pdf-extract |
| CMap parsing | `adobe_cmap_parser` | Crate |
| Glyph names | `glyphnames` | Crate |
| Type1 encoding | `type1_encoding_parser` | Crate |
| CFF parsing | `cff_parser` | Crate |

---

## Appendix B: Test Strategy for Migration Validation

### B.1 Equivalence Test Suite

For each veraPDF capability being replaced, create a test that:
1. Processes the same PDF with both Java (veraPDF) and Rust (replacement crate)
2. Compares output character-by-character (text content)
3. Compares bounding boxes within ±1pt tolerance
4. Validates all images are extracted with correct dimensions

### B.2 PDF Corpus

| Category | Source | Count | Purpose |
|---|---|---|---|
| Standard | Project `samples/pdf/` | ~10 | Basic functionality |
| PDF/UA Reference | `pdfua-1-reference-suite-1-1/` | ~50 | Tagged PDF / structure tree |
| Benchmark | `tests/benchmark/pdfs/` | ~20 | Performance regression |
| CJK | To be sourced | ~10 | Font encoding edge cases |
| Encrypted | To be created | ~5 | Decryption handling |
| Malformed | To be sourced | ~10 | Error handling robustness |

### B.3 Benchmark Targets

The Rust implementation should meet or exceed the Java baseline on all benchmark metrics:
- **NID** (reading order): ≥ current thresholds
- **TEDS** (table structure): ≥ current thresholds
- **MHS** (heading structure): ≥ current thresholds
- **Table Detection F1**: ≥ current thresholds
- **Speed**: ≥ 2× improvement target (Rust advantage)

---

*End of Spec 11*
