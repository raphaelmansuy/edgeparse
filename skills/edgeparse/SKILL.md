---
name: edgeparse
description: Extract structured content from any PDF for AI agents, RAG pipelines, and Copilot Skills. Use this skill whenever the user wants to read, analyze, or reason about a PDF document; needs to feed document content to an LLM; mentions PDF extraction, parsing, or conversion; wants tables, headings, or bounding boxes from a PDF; is building a RAG pipeline; or asks an agent to process a document. Install with: pip install edgeparse
license: Apache-2.0
metadata:
  authors: "EdgeParse Contributors"
  version: "0.1.0"
  package: "edgeparse"
  install_python: "pip install edgeparse"
  install_node: "npm install edgeparse"
  source: "raphaelmansuy/edgeparse"
---

# EdgeParse Skill

Enables AI agents to extract clean, structured content from any PDF — headings, tables, paragraphs, lists, bounding boxes — deterministically, without ML dependencies or GPU requirements.

**Install:** `pip install edgeparse` · **Node.js:** `npm install edgeparse`  
**Speed:** ~0.023 s/doc (Apple M4 Max, 200-doc benchmark)

---

## When to reach for this skill

Activate when the workflow involves:
- Reading or analyzing a PDF document on behalf of a user
- Building a RAG pipeline that ingests PDFs
- Feeding PDF content to an LLM for summarization, Q&A, or synthesis
- Extracting tables from financial reports, research papers, or invoices
- Processing a batch of documents for indexing or search
- An agent tool that must "open" a PDF and return its contents

---

## Quick start

```python
import edgeparse

# Convert any PDF to Markdown — best for LLM context windows
text = edgeparse.convert("report.pdf", format="markdown")

# Convert to JSON with bounding boxes and full structure
import json
doc = json.loads(edgeparse.convert("report.pdf", format="json"))

# Plain text (fast, minimal)
plain = edgeparse.convert("report.pdf", format="text")
```

The `format` parameter controls output:
| Value | Best for |
|-------|----------|
| `"markdown"` | LLM context — headings, tables, lists in Markdown |
| `"json"` | Bounding boxes, citations, structured element metadata |
| `"html"` | Web rendering, semantic HTML5 |
| `"text"` | Simple full-text search, minimal output |

---

## Core API

### `edgeparse.convert()`

```python
result: str = edgeparse.convert(
    input_path,             # str or Path — required
    format="markdown",      # output format (see table above)
    pages=None,             # e.g. "1-5" or "1,3,7-10" — specific pages only
    password=None,          # for password-protected PDFs
    reading_order="xycut",  # "xycut" (spatial sort, default) or "off"
    table_method="default", # "default" (ruling-line) or "cluster" (borderless)
    image_output="off",     # "off", "embedded" (base64), "external" (files)
)
```

Returns the extracted content as a **string**. Raises `FileNotFoundError` for missing files and `ValueError` for corrupt PDFs or bad options.

### `edgeparse.convert_file()`

```python
out_path: str = edgeparse.convert_file(
    input_path,
    output_dir="output",    # write output file to this directory
    format="markdown",
    pages=None,
    password=None,
)
```

Writes the output file and returns its path.

---

## Common patterns

### Feed a PDF to an LLM

```python
import edgeparse
import anthropic

doc = edgeparse.convert("report.pdf", format="markdown")

client = anthropic.Anthropic()
response = client.messages.create(
    model="claude-opus-4-5",
    max_tokens=4096,
    messages=[{
        "role": "user",
        "content": f"Analyze this document and summarize the key findings:\n\n{doc}"
    }]
)
print(response.content[0].text)
```

### RAG pipeline — chunk with metadata

```python
import edgeparse, json

raw = edgeparse.convert("paper.pdf", format="json")
doc = json.loads(raw)

chunks = []
for el in doc["elements"]:
    if el["type"] in ("paragraph", "heading", "table"):
        chunks.append({
            "text": el["text"],
            "metadata": {
                "page":    el["page_number"],
                "type":    el["type"],
                "bbox":    el["bounding_box"],   # for citation highlights
                "order":   el["reading_order"],
            }
        })

# Now embed chunks["text"] and store chunks["metadata"] in your vector store
```

### Batch processing

```python
import edgeparse
from pathlib import Path

results = {}
for pdf in Path("documents/").glob("*.pdf"):
    try:
        results[pdf.name] = edgeparse.convert(str(pdf), format="markdown")
    except Exception as e:
        results[pdf.name] = f"ERROR: {e}"
```

### Extract specific pages only

```python
# Pages 1–5
text = edgeparse.convert("report.pdf", format="markdown", pages="1-5")

# Non-contiguous pages
text = edgeparse.convert("report.pdf", format="markdown", pages="1,3,7-10")
```

### Borderless table extraction

Many financial reports and invoices use tables without ruling lines.
Use `table_method="cluster"` to handle them:

```python
text = edgeparse.convert(
    "earnings.pdf",
    format="markdown",
    table_method="cluster"   # spatial clustering for borderless tables
)
```

### Password-protected PDF

```python
text = edgeparse.convert("secure.pdf", format="markdown", password="mypassword")
```

---

## Node.js usage

```js
import { convert } from 'edgeparse';

const markdown = convert('report.pdf', { format: 'markdown' });
const json     = convert('report.pdf', { format: 'json' });

// With options
const result = convert('report.pdf', {
    format:       'markdown',
    pages:        '1-5',
    readingOrder: 'xycut',
    tableMethod:  'cluster',
});
```

---

## JSON output schema

When `format="json"`, the output is a JSON string with shape:

```json
{
  "page_count": 10,
  "title": "Document Title",
  "elements": [
    {
      "type": "heading",
      "level": 1,
      "text": "Introduction",
      "page_number": 1,
      "reading_order": 0,
      "bounding_box": { "x0": 72, "y0": 144, "x1": 540, "y1": 180 }
    },
    {
      "type": "table",
      "text": "| Col A | Col B |\n|-------|-------|\n| val1  | val2  |",
      "page_number": 2,
      "bounding_box": { "x0": 72, "y0": 200, "x1": 540, "y1": 350 }
    },
    {
      "type": "paragraph",
      "text": "This is body text...",
      "page_number": 1,
      "reading_order": 2,
      "bounding_box": { "x0": 72, "y0": 190, "x1": 540, "y1": 220 }
    }
  ]
}
```

Element `type` values: `heading`, `paragraph`, `table`, `list`, `list_item`, `figure`, `caption`, `header`, `footer`.

---

## Error handling

```python
import edgeparse

try:
    text = edgeparse.convert("report.pdf", format="markdown")
except FileNotFoundError:
    # PDF file not found — check the path
    pass
except ValueError as e:
    # Invalid format, corrupt PDF, wrong password, or bad page range
    print(f"Extraction failed: {e}")
```

---

## For more detail

Read these reference files when the SKILL.md body isn't enough:
- `references/api.md` — complete Python + Node.js API with all parameters and types
- `references/patterns.md` — LangChain, LlamaIndex, MCP tool, CrewAI, and async batch patterns
