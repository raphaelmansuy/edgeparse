# 08 — EdgeParse Agent Skill

EdgeParse ships as a **Claude agent skill** — a single installable unit that teaches Claude how to extract structured content from PDFs on behalf of users and autonomous agents.

Install once via `npx skills add`, and every Claude agent session in your project gains the ability to read, parse, and reason about PDF documents.

---

## What is a skill?

A skill is a structured Markdown file (`SKILL.md`) your AI agent pre-loads to gain domain-specific knowledge and tool-use patterns. The `npx skills add` command fetches the skill from GitHub and registers it in your project's `skills-lock.json`.

When Claude sees a PDF-related task, it reads the EdgeParse skill and knows:
- How to install and call `edgeparse.convert()`
- Which output format to use for different tasks (Markdown for LLMs, JSON for bounding boxes)
- How to handle tables, multi-column layouts, and encrypted files
- Recommended patterns for RAG pipelines, agent tools, and MCP servers

---

## Install

### Via `npx skills add` (recommended)

```bash
npx skills add raphaelmansuy/edgeparse --skill edgeparse
```

This adds an entry to your project's `skills-lock.json`:

```json
{
  "version": 1,
  "skills": {
    "edgeparse": {
      "source": "raphaelmansuy/edgeparse",
      "sourceType": "github"
    }
  }
}
```

### Manual install

Copy `skills/edgeparse/` from the repository into your `.agents/skills/` directory:

```bash
cp -r skills/edgeparse/ .agents/skills/edgeparse/
```

Or reference it in `skills-lock.json` manually as shown above.

---

## Python package

The skill requires the `edgeparse` Python package:

```bash
pip install edgeparse
```

Wheels are available for macOS (arm64, x86_64), Linux (x86_64, arm64), and Windows (x86_64). Python 3.9+ required.

---

## Node.js package

```bash
npm install edgeparse
```

Node.js 18+ required.

---

## Skill anatomy

```
skills/edgeparse/
├── SKILL.md                  ← loaded into agent context when skill triggers
└── references/
    ├── api.md                ← full Python + Node.js API reference (loaded on demand)
    └── patterns.md           ← LangChain, LlamaIndex, MCP, CrewAI patterns
```

The skill uses three-level loading:
1. **Skill description** (always in context) — ~50 words describing when to trigger
2. **SKILL.md body** (loaded when skill triggers) — quick start, core API, common patterns
3. **references/** (loaded on demand) — full API reference and framework-specific patterns

---

## Usage examples

### Basic extraction

```python
import edgeparse

# Markdown — best for LLM context
markdown = edgeparse.convert("report.pdf", format="markdown")

# JSON — with bounding boxes, types, reading order
import json
doc = json.loads(edgeparse.convert("report.pdf", format="json"))
```

### With Claude

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
        "content": f"Summarize the key findings:\n\n{doc}"
    }]
)
print(response.content[0].text)
```

### As an MCP tool

```python
# In your MCP server
@server.call_tool()
async def call_tool(name: str, arguments: dict):
    if name == "read_pdf":
        result = edgeparse.convert(
            arguments["path"],
            format=arguments.get("format", "markdown"),
        )
        return [types.TextContent(type="text", text=result)]
```

For more patterns — LangChain, LlamaIndex, CrewAI, OpenAI function calling, async batch — see `skills/edgeparse/references/patterns.md`.

---

## Skill source

The skill files live in this repository at `skills/edgeparse/`.

| File | Purpose |
|------|---------|
| `skills/edgeparse/SKILL.md` | Main skill — quick start, API, patterns |
| `skills/edgeparse/references/api.md` | Full API reference |
| `skills/edgeparse/references/patterns.md` | Framework integration patterns |

---

## What the skill teaches Claude

When the EdgeParse skill is active, Claude knows to:

1. **Choose the right format** — `markdown` for LLM context, `json` for bounding-box workflows
2. **Handle edge cases** — encrypted PDFs, specific page ranges, borderless tables (`table_method="cluster"`)
3. **Chunk for RAG** — use element boundaries from JSON output as natural chunk boundaries
4. **Integrate with frameworks** — LangChain, LlamaIndex, MCP, CrewAI, OpenAI function calling
5. **Process in batch** — async executor pattern for high-throughput pipelines

---

## Troubleshooting

**`ModuleNotFoundError: No module named 'edgeparse'`**

Install the package: `pip install edgeparse`

**`FileNotFoundError`**

Check that the path to the PDF is correct and accessible from the current working directory.

**`ValueError: ...`**

Common causes:
- Corrupt or password-protected PDF (use `password=` parameter)
- Invalid `pages` range (format must be `"1-5"` or `"1,3,7"`)
- Invalid `format` value

**Tables not extracted correctly**

Try `table_method="cluster"` for borderless tables in financial reports, presentations, and printed documents.
