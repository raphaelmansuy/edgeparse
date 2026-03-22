# 04 — PDF Parsing Pipeline

> **Cross-references**: [03-technical-architecture](03-technical-architecture.md) | [05-data-models](05-data-models.md) | [07-hybrid-mode](07-hybrid-mode.md)

---

## Pipeline Overview

```
Stage  Name                        Input                    Output
-----  --------------------------  -----------------------  -------------------------
  1    PDF Loading                 PDF bytes                Raw page contents
  2    Content Filtering           Raw chunks               Clean chunks
  3    Table Detection (cluster)   Clean chunks             + TableBorder objects
  4    Table Border Matching       Chunks + borders         Chunks assigned to cells
  5    Line Chunk Removal          Mixed chunks             No LineChunks
  6    Text Line Grouping          TextChunks               TextLines
  7    Special Table Detection     TextLines                + Korean tables
  8    Header/Footer Detection     Cross-page content       + SemanticHeaderOrFooter
  9    List Detection (Pass 1)     TextLines                + PDFList objects
 10    Paragraph Detection         TextLines/TextBlocks     SemanticParagraphs
 11    List Detection (Pass 2)     Paragraphs               + More PDFList objects
 12    Heading Detection           Paragraphs               + SemanticHeadings
 13    ID Assignment               All elements             Elements with IDs
 14    Caption Linking             Headings + images/tables + SemanticCaptions
 15    Cross-Page Linking          Lists, tables            Linked lists/tables
 16    Heading Level Assignment    All headings             Headings with H1-H6
 17    Nesting Level Assignment    All elements             Elements with levels
 18    Reading Order Sorting       Ordered elements         XY-Cut++ sorted elements
 19    Content Sanitization        Text elements            Sanitized text
 20    Output Generation           Final elements           Files on disk
```

---

## Stage 1: PDF Loading

### 1.1 Document Loading

```
Input:  file_path: &str, password: Option<&str>
Output: PdfDocument { pages, metadata, table_borders_from_lines }
```

**Algorithm**:
1. Open PDF file as byte stream
2. If password provided, attempt decryption
3. Parse PDF cross-reference table and object tree
4. Extract document metadata (author, title, creation/modification dates)
5. For each page:
   a. Parse page content stream(s)
   b. Apply current transformation matrix (CTM) to all coordinates
   c. Extract text chunks with font resolution
   d. Extract image objects with bounding boxes
   e. Extract line drawing operations (paths)
   f. Classify line operations as LineChunk or LineArtChunk

### 1.2 Text Chunk Extraction

For each text operation in the content stream:
1. Resolve font from resource dictionary
2. Decode character codes to Unicode using font's ToUnicode CMap or encoding
3. Calculate glyph widths and positions
4. Apply text matrix and CTM for final coordinates
5. Create TextChunk with:
   - `value`: decoded Unicode string
   - `font_name`: resolved font name
   - `font_size`: effective size after matrix transform
   - `font_weight`: from font descriptor (or inferred from name)
   - `text_color`: from current graphics state (fill color)
   - `bounding_box`: [leftX, bottomY, rightX, topY] in page coordinates
   - `base_line`: Y coordinate of text baseline
   - `is_white_space`: true if entirely whitespace
   - `contrast_ratio`: initially None (computed later)

### 1.3 Line Segment Extraction

For each path operation:
1. Parse move-to, line-to, curve-to, close-path operations
2. Classify straight lines as:
   - **LineChunk**: `width > MIN_LINE_WIDTH` and approximately horizontal/vertical
   - **LineArtChunk**: more complex vector graphics (bullets, decorations)
3. Feed line segments into `LinesPreprocessingConsumer` for table border detection

### 1.4 Table Border Pre-Detection

The `LinesPreprocessingConsumer` analyzes line segments to find table borders:
1. Group horizontal and vertical lines
2. Find intersecting line pairs forming grid cells
3. Create `TableBorder` objects with row/column structure
4. Store in `TableBordersCollection` indexed by page

### 1.5 Image Extraction

For each image XObject:
1. Extract image dimensions and color space
2. Calculate bounding box from CTM
3. Create `ImageChunk` with bounding box
4. Actual image data extraction deferred to output generation

### 1.6 Coordinate System

```
PDF coordinate system (origin at bottom-left):

(0, pageHeight)              (pageWidth, pageHeight)
       +----------------------------+
       |                            |
       |     Content Area           |
       |                            |
       |  topY  +--------+         |
       |        | Element |         |
       |  bottomY+--------+         |
       |  leftX          rightX     |
       |                            |
       +----------------------------+
(0, 0)                    (pageWidth, 0)

BoundingBox = [leftX, bottomY, rightX, topY]
- 72 points = 1 inch
- Origin at bottom-left corner
- Y increases upward
- Width  = rightX - leftX
- Height = topY - bottomY
```

---

## Stage 2: Content Filtering

### 2.1 Overview

```
Input:  Map<page, Vec<ContentElement>>
Output: Map<page, Vec<ContentElement>>  (filtered)
```

### 2.2 Sub-stages (applied in order)

#### 2.2.1 Remove Duplicate Text Chunks

Two text chunks are duplicates if:
- Same text value
- Bounding box intersection > 50% (`MIN_TEXT_INTERSECTION_PERCENT = 0.5`)
Keep the one with more text / better quality.

#### 2.2.2 Remove Text Decoration Images

Images that appear to be text decorations (underlines, strikethroughs):
- Image top aligned with text chunk bottom (within `MAX_TOP_DECORATION_IMAGE_EPSILON = 0.3`)
- Image bottom close to text bottom (within `MAX_BOTTOM_DECORATION_IMAGE_EPSILON = 0.1`)
- Image left close to text left (within `MAX_LEFT_DECORATION_IMAGE_EPSILON = 0.1`)
- Image right extends beyond text (within `MAX_RIGHT_DECORATION_IMAGE_EPSILON = 1.5`)

#### 2.2.3 Filter Tiny Text

Remove text chunks with `height <= TEXT_MIN_HEIGHT (1.0 point)`.

Controlled by: `filter_config.filter_tiny_text`

#### 2.2.4 Filter Off-Page Content

Remove content outside CropBox (or MediaBox if no CropBox).

Controlled by: `filter_config.filter_out_of_page`

#### 2.2.5 Merge Close Text Chunks

Merge adjacent text chunks with:
- Same font, size, color, weight
- Same baseline (within `NEIGHBORS_TEXT_CHUNKS_EPSILON = 0.1`)
- Horizontally adjacent

#### 2.2.6 Trim Whitespace

Remove leading/trailing whitespace from each text chunk's value.

#### 2.2.7 Compress Consecutive Spaces

Replace runs of multiple spaces with single space within each chunk.

#### 2.2.8 Split Text Chunks by Whitespace

Split text chunks at internal whitespace boundaries to create fine-grained positioning.

#### 2.2.9 Hidden Text Detection

```
For each text chunk:
    contrast_ratio = calculate_contrast(text_color, background_color)
    if contrast_ratio < MIN_CONTRAST_RATIO (1.2):
        if filter_hidden_text:
            remove chunk
        else:
            mark chunk.hidden_text = true
```

**Contrast calculation**: Requires rendering the page to determine actual background color at the text position. The renderer samples the pixel region behind the text.

Controlled by: `filter_config.filter_hidden_text`

#### 2.2.10 Replace Undefined Characters

Replace Unicode replacement character (U+FFFD) and other unrecognized characters with `config.replace_invalid_chars` (default: space).

#### 2.2.11 Remove Backgrounds

Detect and remove large rectangular objects that appear to be page backgrounds:
- Width > 50% page width AND height > 10% page height
- OR width > 10% page width AND height > 50% page height

---

## Stage 3: Table Detection (Cluster Method)

### 3.1 Activation

Only runs when `config.table_method == TableMethod::Cluster`.

### 3.2 Algorithm

```
Input:  Map<page, Vec<ContentElement>>
Output: Additional TableBorder objects in TableBordersCollection

For each page:
  1. Collect all TextChunk objects
  2. Split chunks by internal whitespace to get atomic tokens
  3. Create TableToken(textChunk, semanticTextNode) pairs
  4. Feed tokens into ClusterTableConsumer
  5. ClusterTableConsumer groups tokens by:
     a. Vertical alignment (same row detection)
     b. Horizontal alignment (column boundary detection)
     c. Statistical clustering of X-coordinates for column positions
  6. For each detected table:
     a. Create TableBorder from cluster Table
     b. Check: does NOT intersect with existing table borders
        (intersection < TABLE_INTERSECTION_PERCENT = 0.01)
     c. If valid, add to TableBordersCollection
```

### 3.3 Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `Y_DIFFERENCE_EPSILON` | 0.1 | Same-row vertical tolerance |
| `X_DIFFERENCE_EPSILON` | 3.0 | Minimum horizontal gap for column |
| `TABLE_INTERSECTION_PERCENT` | 0.01 | Max overlap with existing table border |

---

## Stage 4: Table Border Matching

### 4.1 Algorithm

```
For each page:
  For each TableBorder on this page:
    For each content element on the page:
      cell = tableBorder.getCell(element.bbox)
      if cell exists:
        if element is TextChunk:
          if element spans multiple cells:
            split element at cell boundaries
            assign parts to respective cells
          else:
            add element to cell.contents
        else:
          add element to cell.contents
```

### 4.2 Cell Content Processing

After assigning content to cells, each cell's contents undergo the **full sub-pipeline**:
1. Text line grouping
2. List detection
3. Paragraph detection
4. Heading detection
5. Caption linking

This is recursive, with a **depth limit of 10** (`MAX_NESTED_TABLE_DEPTH = 10`) to prevent stack overflow from deeply nested tables.

### 4.3 Cross-Page Table Linking

```
checkNeighborTables():
  For each pair of adjacent tables (table_a on page N, table_b on page N+1):
    if table_a.num_columns == table_b.num_columns
       AND abs(table_a.width - table_b.width) < NEIGHBOUR_TABLE_EPSILON (0.2)
       AND column_widths_match(table_a, table_b):
      table_a.next_table_id = table_b.id
      table_b.previous_table_id = table_a.id
```

---

## Stage 5: Line Chunk Removal

After table detection is complete, all `LineChunk` objects are removed from page contents. They were only needed for table border detection.

---

## Stage 6: Text Line Grouping

### 6.1 Algorithm

```
Input:  Vec<ContentElement> per page (TextChunks, images, etc.)
Output: TextChunks replaced with TextLines

For each page:
  current_line = new TextLine()
  prev_chunk = None
  
  For each element in page contents:
    if element is TextChunk:
      if prev_chunk exists:
        prob = count_one_line_probability(prev_chunk, element)
        same_hidden = prev_chunk.hidden == element.hidden
        if prob >= ONE_LINE_PROBABILITY (0.75) AND same_hidden:
          current_line.add(element)
        else:
          emit current_line
          current_line = new TextLine(element)
      else:
        current_line.add(element)
      prev_chunk = element
    else if element is TableBorder:
      emit current_line  // force line break at table
      emit element
      current_line = new TextLine()
      prev_chunk = None
    else:
      emit element  // images, lineArt pass through
  
  emit current_line  // last line
```

### 6.2 Within-Line Processing

After grouping, within each TextLine:
1. **Sort chunks by leftX** (left-to-right order)
2. **Insert space chunks** between adjacent chunks when:
   ```
   gap = chunk[i+1].leftX - chunk[i].rightX
   if gap > fontSize * TEXT_LINE_SPACE_RATIO:
     insert WhitespaceChunk at gap position
   ```
3. **Link LineArt bullets**: If a `LineArtChunk` appears immediately before a TextLine:
   ```
   if lineArt.rightX <= textLine.leftX
      AND lineArt.height < LIST_LABEL_HEIGHT_EPSILON * textLine.height:
     textLine.connectedLineArtLabel = lineArt
   ```

### 6.3 Same-Line Probability

`count_one_line_probability(chunk_a, chunk_b)` evaluates:
- Baseline alignment (within font size tolerance)
- Vertical overlap between bounding boxes
- Font size similarity
- No large vertical gap

Returns float in [0.0, 1.0].

---

## Stage 7: Special Table Detection (Korean)

### 7.1 Pattern

Detects Korean government document format:
```
수신  :  <recipient>
경유  :  <via>
제목  :  <subject>
```

### 7.2 Algorithm

```
For each page:
  Scan consecutive TextLines for Korean pattern:
    if line starts with (수신|경유|제목):
      Add to special table candidates
  
  If >= 2 consecutive matches:
    Create 2-column TableBorder:
      Column 1: label (수신, 경유, 제목)
      Column 2: value after ":"
    Single-column rows span both columns
```

---

## Stage 8: Header/Footer Detection

### 8.1 Algorithm (Cross-Page)

```
Input:  Map<page, Vec<ContentElement>> for ALL pages
Output: SemanticHeaderOrFooter elements added, original content removed

// Header detection
For position_index = 0, 1, 2, ...:
  candidates = []
  For page_num in 1..total_pages:
    element_a = pages[page_num].sorted_contents[position_index]  // top elements
    element_b = pages[page_num + 1].sorted_contents[position_index]
    
    if are_possible_header_footer(element_a, element_b):
      candidates.add((page_num, element_a))
    else:
      break  // first mismatch stops scanning this position
  
  // Also check 2-page style (comparing page N and N+2)
  For page_num in 1..total_pages:
    element_a = pages[page_num].sorted_contents[position_index]
    element_c = pages[page_num + 2].sorted_contents[position_index]  // skip one page
    if are_possible_header_footer(element_a, element_c):
      candidates.add(...)
  
  if candidates span enough pages:
    For each candidate:
      if element.topY > page_height * 2/3:  // top 1/3 of page
        wrap in SemanticHeaderOrFooter(HEADER)
        remove from page contents

// Footer detection: same but scanning from END of sorted contents
// and checking element.bottomY < page_height * 1/3  // bottom 1/3
```

### 8.2 Matching Criteria

`are_possible_header_footer(a, b)`:
1. Both are SemanticTextNode:
   - Overlapping bbox (excluding page number)
   - Close font size
   - EITHER same text value OR sequential numbering
2. Both are TextLine:
   - Convert to SemanticTextNode, then compare
3. Otherwise:
   - Same bounding box (excluding page)

### 8.3 Numbering Sequence Detection

Detect increments of 1 or 2 in:
- Arabic numerals: 1, 2, 3 / 1, 3, 5
- Roman numerals: I, II, III / i, ii, iii
- Korean character sequences: 가, 나, 다
- Alphabetic: a, b, c / A, B, C

---

## Stage 9: List Detection (Pass 1 — TextLine Level)

### 9.1 Label Pattern Detection

```
Supported label patterns:
  Arabic:     "1.", "2.", "3."  or  "1)", "2)", "3)"
  Korean:     "가.", "나.", "다." (consonant sequence)
              "제1장", "제2조", "제3절" (chapter/article/section)
  Roman:      "I.", "II.", "III." or "i.", "ii.", "iii."
  Circled:    "①", "②", "③"
  Bullet:     "•", "◦", "▪", "►", "-"
  Korean붙임: "붙임 N." prefix
```

### 9.2 List Construction Algorithm

```
For each page:
  For each TextLine:
    label = detect_list_label(textLine.value)
    if label exists:
      if current_list exists AND isTwoListItemsOfOneList(prev_label, label):
        current_list.add(ListItem(textLine))
      else:
        emit current_list
        current_list = new PDFList(numbering_style, ListItem(textLine))
      prev_label = label
    else:
      if current_list exists AND isListItemLine(textLine, current_list):
        current_list.lastItem.addBodyLine(textLine)
      else:
        emit current_list
        emit textLine
        current_list = None
```

### 9.3 List Item Body Assignment

`isListItemLine(textLine, currentList)`:
```
merge_probability = mergeLeadingProbability(lastLine, textLine) 
baseline_diff = abs(textLine.baseY - lastLine.baseY) / fontSize
x_gap = abs(textLine.leftX - lastItem.leftX)
max_gap = fontSize * LIST_ITEM_X_INTERVAL_RATIO (0.3)

return merge_probability > LIST_ITEM_PROBABILITY (0.7)
   AND baseline_diff < LIST_ITEM_BASELINE_DIFFERENCE (1.2)
   AND x_gap <= max_gap
   AND NOT isLabeledLine(textLine)
   AND NOT isListLine(textLine)
```

### 9.4 List Validation

`isCorrectList(list)`: Reject sequences of decimal numbers (e.g., `1.0, 2.5, 3.1`) — these are data values, not list items.

---

## Stage 10: Paragraph Detection

### 10.1 Multi-Pass Merging

```
Input:  TextLines and TextBlocks on each page
Output: SemanticParagraph objects

MERGE_THRESHOLD = DIFFERENT_LINES_PROBABILITY = 0.75
```

Each pass attempts to merge TextLines/TextBlocks with specific alignment patterns:

#### Pass 1: Justify Alignment Merging
```
For consecutive blocks where:
  alignment(block_a, block_b) == JUSTIFY
  AND same text size
  AND mergeLeadingProbability >= 0.75
→ Merge into single TextBlock
```

#### Pass 2: Justify First/Last Line Detection
```
If single-line block followed by JUSTIFY block:
  AND first line is RIGHT-aligned or spans wider
→ Merge as first line of justified paragraph

If JUSTIFY block followed by single-line LEFT-aligned block:
→ Merge as last line of justified paragraph
```

#### Pass 3: Left Alignment (Strict)
```
For consecutive LEFT-aligned blocks with:
  Same text style (font size within 0.1, font weight within 0.1)
  AND mergeLeadingProbability >= 0.75
→ Merge
```

#### Pass 4: Left Alignment (Relaxed)
```
Same as Pass 3 but WITHOUT style check
```

#### Pass 5: Left Block First Lines
```
Single-line block before LEFT-aligned block:
  AND overlapping X range
→ Merge as first line
```

#### Pass 6: Two-Line Paragraphs
```
Two single-line blocks where:
  First line spans wider than second line
→ Merge
```

#### Pass 7: Center Alignment
```
For consecutive CENTER-aligned blocks:
  AND mergeLeadingProbability >= 0.75
→ Merge
```

#### Pass 8: Right Alignment
```
For consecutive RIGHT-aligned blocks:
  AND mergeLeadingProbability >= 0.75
→ Merge
```

#### Pass 9: Fallback
```
For consecutive blocks with:
  Same style AND same size
  AND overlapping X range
  AND no explicit alignment set
  AND mergeLeadingProbability >= 0.75
→ Merge
```

### 10.2 SemanticParagraph Creation

After merging, each final TextBlock is wrapped in a `SemanticParagraph` node:
```
SemanticParagraph {
  columns: [TextColumn { blocks: [TextBlock { lines: [TextLine] }] }]
  bounding_box: union of all contained lines
  page_number: from first line
  semantic_type: PARAGRAPH
}
```

---

## Stage 11: List Detection (Pass 2 — Paragraph Level)

After paragraphs are formed, a second pass detects lists composed of SemanticTextNode sequences:
1. Scan SemanticTextNode objects for list label patterns
2. Use same matching logic as Pass 1
3. Convert matching paragraph sequences into PDFList objects

---

## Stage 12: Heading Detection

### 12.1 Probability Score Calculation

```
For each SemanticTextNode on each page:
  // Base probability from node context
  base = heading_probability(node, prev_node, next_node)
  
  // Font size rarity boost
  size_boost = 0.0
  if font_size NOT in dominant range [10.0, 13.0]:
    if font_size in heading range [10.0, 32.0]:
      size_boost = mode_weight_statistics.get_boost(font_size) * 0.5
  
  // Font weight rarity boost  
  weight_boost = 0.0
  if font_weight NOT in dominant range [395.0, 405.0]:
    if font_weight in heading range [400.0, 900.0]:
      weight_boost = mode_weight_statistics.get_boost(font_weight) * 0.3
  
  // Bulleted paragraph bonus
  bullet_bonus = if is_bulleted_paragraph(node) { 0.1 } else { 0.0 }
  
  total = base + size_boost + weight_boost + bullet_bonus
  
  if total >= HEADING_PROBABILITY (0.75) AND NOT is_list_item(node):
    mark as heading
    convert to SemanticHeading
    add to headings collection
```

### 12.2 Text Style Statistics

`ModeWeightStatistics` tracks distributions:
```
histogram: HashMap<i32, usize>  // value → frequency (value = font_size * 100)

mode(min, max) -> i32:
  Most frequent value within [min, max]

get_boost(score) -> f64:
  sorted = values > mode AND within [score_min, score_max]
  rank = position of score in sorted
  return (rank + 1) / sorted.len()
```

### 12.3 `TextNodeStatisticsConfig` Defaults

```
font_size_dominant_min:  10.0
font_size_dominant_max:  13.0
font_size_heading_min:   10.0
font_size_heading_max:   32.0
font_size_rarity_boost:   0.5

font_weight_dominant_min: 395.0
font_weight_dominant_max: 405.0
font_weight_heading_min:  400.0
font_weight_heading_max:  900.0
font_weight_rarity_boost:  0.3
```

---

## Stage 13: ID Assignment

```
id_counter = 0
For each page in order:
  For each element in page contents:
    element.id = id_counter
    id_counter += 1
    // Recurse into table cells, list items, etc.
```

IDs are sequential integers starting from 0, assigned in reading order.

---

## Stage 14: Caption Linking

### 14.1 Algorithm

```
For each page:
  prev_text_node = None
  prev_image_or_table = None
  
  For each element in page contents:
    if element is SemanticTextNode (not heading, not list):
      if prev_image_or_table exists:
        // Check if this text is a caption for the previous image/table
        prob = caption_probability(element, prev_image_or_table)
        
      // Also check if this is a caption for the NEXT image/table
      if next_element is image/table:
        next_prob = caption_probability(element, next_element)
      
      best = max(prob, next_prob)
      if best >= CAPTION_PROBABILITY (0.75):
        convert element to SemanticCaption
        caption.linked_content_id = target.id
      
      prev_text_node = element
    
    else if element is ImageChunk or TableBorder:
      // Check if prev_text_node is a caption for this
      if prev_text_node exists:
        prob = caption_probability(prev_text_node, element)
        if prob >= 0.75:
          convert prev_text_node to SemanticCaption
          caption.linked_content_id = element.id
      
      prev_image_or_table = element
```

### 14.2 Caption Probability Factors

```
caption_probability(text_node, target):
  - Vertical proximity (within CAPTION_VERTICAL_OFFSET_RATIO * fontSize)
  - Horizontal overlap (within CAPTION_HORIZONTAL_OFFSET_RATIO * fontSize)
  - Text not contained within image bbox
  - Target is not a "subtle" image (aspect_ratio >= SUBTLE_IMAGE_RATIO_THRESHOLD = 0.01)
  - Text content analysis (starts with "Figure", "Table", "Fig.", etc.)
```

---

## Stage 15: Cross-Page Linking

### 15.1 List Linking

```
checkNeighborLists():
  For each pair of lists (list_a on page N, list_b on page N+1):
    if lists form continuous numbering sequence:
      list_a.next_list_id = list_b.id
      list_b.previous_list_id = list_a.id
    
    // Middle content absorption
    if single text node between list_a.last and list_b.first:
      absorb text node into list_a.lastItem.body
```

### 15.2 Table Linking

See Stage 4.3 above.

---

## Stage 16: Heading Level Assignment

```
detectHeadingsLevels():
  // Group all headings by visual style
  style_groups: HashMap<TextStyle, Vec<&SemanticHeading>>
  
  For each heading:
    style = TextStyle {
      font_name: heading.font,
      font_size: heading.font_size,
      font_weight: heading.font_weight,
      text_color: heading.color,
    }
    style_groups[style].push(heading)
  
  // Sort styles by visual prominence
  // TextStyle implements Ord:
  //   1. Larger font_size first
  //   2. Bolder font_weight first
  //   3. Font name alphabetically
  //   4. Color as tiebreaker
  
  sorted_styles = style_groups.keys().sorted()
  
  // Assign levels
  For (level, style) in sorted_styles.enumerate():
    For heading in style_groups[style]:
      heading.heading_level = level + 1  // 1-indexed
```

---

## Stage 17: Nesting Level Assignment

```
detectLevels():
  level_stack: Vec<LevelInfo> = []
  
  For each page:
    For each element:
      match element:
        SemanticHeading:
          // First H1 → "Doctitle", others → "Subtitle"
          if heading.level == 1:
            if is_first_h1: element.level = "Doctitle"
            else: element.level = "Subtitle"
          pop stack to appropriate depth
          push HeadingLevelInfo
          
        PDFList:
          if list.is_connected (has previous_list_id):
            inherit level from predecessor
          else:
            element.level = current_depth + 1
          push ListLevelInfo
          
        TableBorder:
          if table.is_connected:
            inherit level from predecessor
          else:
            element.level = current_depth + 1
          push TableLevelInfo
          
        BulletedParagraph (text with lineArt label):
          element.level = current_depth + 1
          push BulletLevelInfo
          
        _:
          element.level = current_depth
```

---

## Stage 18: Reading Order Sorting (XY-Cut++)

### 18.1 Full Algorithm

```
                    Input: Vec<ContentElement>
                              |
                    Phase 1: Pre-mask
                              |
              +-------------------------------+
              | Identify cross-layout elements |
              | width >= beta * maxWidth (2.0) |
              | AND overlaps >= 2 others       |
              +-------------------------------+
                              |
                    Phase 2: Density
                              |
              +-------------------------------+
              | density = sum(areas) /        |
              |           bbox_of_all.area()  |
              | prefer_horizontal =           |
              |   density > 0.9               |
              +-------------------------------+
                              |
                    Phase 3: Recursive Cut
                              |
              +-------------------------------+
              | recursiveSegment(elements,    |
              |   preferHorizontalFirst)      |
              +-------------------------------+
                              |
                    Phase 4: Merge
                              |
              +-------------------------------+
              | Insert cross-layout elements  |
              | back at Y-sorted positions    |
              +-------------------------------+
                              |
                    Output: Vec<ContentElement>
                           (reading order)
```

### 18.2 Recursive Segmentation

```
recursiveSegment(objects, preferHorizontalFirst):
  if objects.len() <= 1:
    return objects  // base case
  
  // Find best cuts via projection profiles
  h_gap = findBestHorizontalCut(objects)  // largest Y-gap
  v_gap = findBestVerticalCut(objects)    // largest X-gap
  
  if h_gap < MIN_GAP_THRESHOLD (5.0) AND v_gap < MIN_GAP_THRESHOLD:
    return sortByYThenX(objects)  // no valid cut
  
  // Choose cut
  if preferHorizontalFirst:
    if h_gap >= MIN_GAP_THRESHOLD:
      use horizontal cut
    else:
      use vertical cut
  else:
    use cut with larger gap
  
  // Split
  (group_above, group_below) = splitByCut(objects, cut_position)
  
  // Recurse
  sorted_above = recursiveSegment(group_above, preferHorizontalFirst)
  sorted_below = recursiveSegment(group_below, preferHorizontalFirst)
  
  return concat(sorted_above, sorted_below)
```

### 18.3 Projection Profile Cut Finding

```
findBestHorizontalCut(objects):
  // Project all objects onto Y-axis
  // Find the largest empty gap in the projection
  
  y_intervals = objects.map(|o| (o.bottomY, o.topY)).sorted_by_bottom()
  
  max_gap = 0.0
  max_gap_position = 0.0
  
  For consecutive intervals [a, b]:
    gap = b.bottomY - a.topY
    if gap > max_gap:
      max_gap = gap
      max_gap_position = a.topY + gap / 2
  
  return (max_gap, max_gap_position)

findBestVerticalCut(objects):
  // Same but projecting onto X-axis
  x_intervals = objects.map(|o| (o.leftX, o.rightX)).sorted_by_left()
  // Find largest gap in X projection
```

### 18.4 Split by Cut

```
splitByHorizontalCut(objects, cut_y):
  above = objects.filter(|o| o.center_y() > cut_y)
  below = objects.filter(|o| o.center_y() <= cut_y)
  return (above, below)
  
// Analogous for vertical cut
```

### 18.5 Base Case Sort

```
sortByYThenX(objects):
  objects.sort_by(|a, b| {
    // Primary: top Y descending (higher on page first)
    let y_cmp = b.topY.partial_cmp(&a.topY);
    if y_cmp != Equal: return y_cmp
    // Secondary: left X ascending (left-to-right)
    a.leftX.partial_cmp(&b.leftX)
  })
```

### 18.6 Cross-Layout Element Identification

```
identifyCrossLayoutElements(objects):
  max_width = objects.iter().map(|o| o.width()).reduce(f64::max)
  threshold = DEFAULT_BETA (2.0) * max_width
  
  cross_layout = []
  For each object with width >= threshold:
    overlap_count = 0
    For each other_object:
      if horizontal_overlap_ratio(object, other) >= OVERLAP_THRESHOLD (0.1):
        overlap_count += 1
    if overlap_count >= MIN_OVERLAP_COUNT (2):
      cross_layout.push(object)
  
  return cross_layout
```

### 18.7 Constants Reference

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_BETA` | 2.0 | Cross-layout width multiplier |
| `DEFAULT_DENSITY_THRESHOLD` | 0.9 | Density ratio for axis preference |
| `OVERLAP_THRESHOLD` | 0.1 | Minimum horizontal overlap ratio |
| `MIN_OVERLAP_COUNT` | 2 | Min overlaps for cross-layout |
| `MIN_GAP_THRESHOLD` | 5.0 pts | Minimum gap to perform a cut |

---

## Stage 19: Content Sanitization

### 19.1 Activation

Only runs when `filter_config.filter_sensitive_data == true`.

### 19.2 Algorithm

```
sanitize_contents(contents):
  For each element:
    match element:
      SemanticTextNode | SemanticParagraph | SemanticHeading:
        for column in element.columns:
          for block in column.blocks:
            for line in block.lines:
              sanitize_text_line(line)
      
      PDFList:
        for item in list.items:
          sanitize_contents(item.contents)
      
      TableBorder:
        for row in table.rows:
          for cell in row.cells:
            sanitize_contents(cell.contents)
      
      SemanticHeaderOrFooter:
        sanitize_contents(element.contents)
```

### 19.3 Text Line Sanitization

```
sanitize_text_line(line):
  full_text = line.chunks.map(|c| c.value).join("")
  
  // Find all regex matches
  matches = []
  for rule in sanitization_rules:
    for m in rule.pattern.find_iter(full_text):
      matches.push(Match { start: m.start(), end: m.end(), 
                           replacement: rule.replacement })
  
  // Sort by start position, remove overlapping
  matches.sort_by(|a, b| a.start.cmp(&b.start))
  matches = remove_overlapping(matches)
  
  // Split original chunks at match boundaries
  // Create replacement chunks with placeholder text
  // Preserve font, size, color from original chunks
  new_chunks = apply_replacements(line.chunks, matches)
  line.chunks = new_chunks
```

### 19.4 Default Sanitization Rules

| Priority | Pattern | Replacement |
|----------|---------|-------------|
| 1 | Email addresses | `email@example.com` |
| 2 | Phone numbers (`+XX-XXXX-XXXX`) | `+00-0000-0000` |
| 3 | ID numbers (1-2 upper + 6-9 digits) | `AA0000000` |
| 4 | Credit cards (4×4 digits) | `0000-0000-0000-0000` |
| 5 | Long numbers (10-18 digits) | `0000000000000000` |
| 6 | IPv4 addresses | `0.0.0.0` |
| 7 | IPv6 addresses | `0.0.0.0::1` |
| 8 | MAC addresses | `00:00:00:00:00:00` |
| 9 | IMEI (15 digits) | `000000000000000` |
| 10 | URLs (`http(s)://...`) | `https://example.com` |

---

## Stage 20: Output Generation

See [08-output-formats](08-output-formats.md) for detailed format specifications.

```
generate_outputs(config, document_info, contents):
  if image_output != Off:
    extract_and_save_images(contents, config)
  
  if formats.contains(Json):
    write_json(document_info, contents, config)
  
  if formats.contains(Markdown):
    write_markdown(contents, config)
  
  if formats.contains(MarkdownWithHtml):
    write_markdown_html(contents, config)
  
  if formats.contains(MarkdownWithImages):
    write_markdown_images(contents, config)
  
  if formats.contains(Html):
    write_html(contents, config)
  
  if formats.contains(Text):
    write_text(contents, config)
  
  if formats.contains(Pdf):
    write_annotated_pdf(contents, config)
```
