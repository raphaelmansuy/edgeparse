# EdgeParse Integration Patterns

Common patterns for integrating EdgeParse into AI agent frameworks, RAG pipelines, and production workflows.

---

## LangChain

### PDF loader

Use EdgeParse as a custom document loader:

```python
from langchain.schema import Document
import edgeparse, json

def load_pdf_with_edgeparse(path: str) -> list[Document]:
    """Load a PDF and return LangChain Documents with metadata."""
    raw = edgeparse.convert(path, format="json")
    doc = json.loads(raw)

    documents = []
    for el in doc["elements"]:
        if el["type"] not in ("header", "footer"):
            documents.append(Document(
                page_content=el["text"],
                metadata={
                    "source":   path,
                    "page":     el["page_number"],
                    "type":     el["type"],
                    "bbox":     el["bounding_box"],
                }
            ))
    return documents

# Use with any LangChain retriever
docs = load_pdf_with_edgeparse("report.pdf")
```

### RAG chain

```python
from langchain.vectorstores import Chroma
from langchain.embeddings import OpenAIEmbeddings
from langchain.chains import RetrievalQA
from langchain_anthropic import ChatAnthropic

docs = load_pdf_with_edgeparse("report.pdf")

vectorstore = Chroma.from_documents(docs, OpenAIEmbeddings())
retriever = vectorstore.as_retriever(search_kwargs={"k": 5})

chain = RetrievalQA.from_chain_type(
    llm=ChatAnthropic(model="claude-opus-4-5"),
    retriever=retriever,
)
result = chain.run("What are the key financial metrics?")
```

---

## LlamaIndex

### SimpleDirectoryReader replacement

```python
from llama_index.core import VectorStoreIndex
from llama_index.core.schema import TextNode
import edgeparse, json

def edgeparse_nodes(paths: list[str]) -> list[TextNode]:
    nodes = []
    for path in paths:
        raw = edgeparse.convert(path, format="json")
        doc = json.loads(raw)
        for el in doc["elements"]:
            if el["text"].strip():
                nodes.append(TextNode(
                    text=el["text"],
                    metadata={"source": path, "page": el["page_number"], "type": el["type"]},
                ))
    return nodes

nodes = edgeparse_nodes(["report.pdf", "paper.pdf"])
index = VectorStoreIndex(nodes)
query_engine = index.as_query_engine()
response = query_engine.query("Summarize the methodology section")
print(response)
```

---

## MCP (Model Context Protocol) tool

Register EdgeParse as an MCP tool so Claude Desktop can call it:

```python
from mcp.server.models import InitializationOptions
from mcp.server import NotificationOptions, Server
from mcp import types
import edgeparse

server = Server("edgeparse-mcp")

@server.list_tools()
async def list_tools() -> list[types.Tool]:
    return [types.Tool(
        name="read_pdf",
        description="Extract structured text from a PDF file. Returns Markdown.",
        inputSchema={
            "type": "object",
            "properties": {
                "path":   {"type": "string", "description": "Path to the PDF file"},
                "pages":  {"type": "string", "description": "Optional page range, e.g. '1-5'"},
                "format": {"type": "string", "enum": ["markdown", "json", "text"], "default": "markdown"},
            },
            "required": ["path"],
        },
    )]

@server.call_tool()
async def call_tool(name: str, arguments: dict) -> list[types.TextContent]:
    if name == "read_pdf":
        try:
            result = edgeparse.convert(
                arguments["path"],
                format=arguments.get("format", "markdown"),
                pages=arguments.get("pages"),
            )
            return [types.TextContent(type="text", text=result)]
        except Exception as e:
            return [types.TextContent(type="text", text=f"Error: {e}")]
    raise ValueError(f"Unknown tool: {name}")
```

---

## CrewAI agent tool

```python
from crewai_tools import BaseTool
import edgeparse

class PDFExtractorTool(BaseTool):
    name: str = "PDF Extractor"
    description: str = (
        "Extract text, tables, and structure from a PDF file. "
        "Input: file path. Output: Markdown text."
    )

    def _run(self, pdf_path: str, pages: str = None) -> str:
        try:
            return edgeparse.convert(
                pdf_path,
                format="markdown",
                pages=pages,
                table_method="cluster",
            )
        except Exception as e:
            return f"Failed to extract PDF: {e}"

# In your crew
from crewai import Agent

analyst = Agent(
    role="Document Analyst",
    goal="Extract and analyze information from PDF documents",
    tools=[PDFExtractorTool()],
)
```

---

## OpenAI function calling

```python
import openai
import edgeparse, json

tools = [{
    "type": "function",
    "function": {
        "name": "read_pdf",
        "description": "Extract text and tables from a PDF file",
        "parameters": {
            "type": "object",
            "properties": {
                "path":   {"type": "string"},
                "format": {"type": "string", "enum": ["markdown", "json", "text"]},
                "pages":  {"type": "string"},
            },
            "required": ["path"],
        },
    }
}]

def handle_tool_call(tool_name, args):
    if tool_name == "read_pdf":
        return edgeparse.convert(
            args["path"],
            format=args.get("format", "markdown"),
            pages=args.get("pages"),
        )

client = openai.OpenAI()
messages = [{"role": "user", "content": "Summarize the report at /tmp/report.pdf"}]

response = client.chat.completions.create(model="gpt-4o", messages=messages, tools=tools)

if response.choices[0].message.tool_calls:
    call = response.choices[0].message.tool_calls[0]
    result = handle_tool_call(call.function.name, json.loads(call.function.arguments))
    # Continue conversation with tool result...
```

---

## Async batch processing

For high-throughput pipelines, use async:

```python
import asyncio
import edgeparse
from pathlib import Path

async def extract_pdf(path: str, semaphore: asyncio.Semaphore) -> tuple[str, str]:
    async with semaphore:
        # EdgeParse is CPU-bound; use executor to avoid blocking the event loop
        loop = asyncio.get_event_loop()
        result = await loop.run_in_executor(
            None, lambda: edgeparse.convert(path, format="markdown")
        )
        return path, result

async def batch_extract(pdf_dir: str, max_concurrent: int = 8) -> dict[str, str]:
    paths = [str(p) for p in Path(pdf_dir).glob("*.pdf")]
    semaphore = asyncio.Semaphore(max_concurrent)
    tasks = [extract_pdf(p, semaphore) for p in paths]
    results = await asyncio.gather(*tasks, return_exceptions=True)
    return {
        path: result if isinstance(result, str) else f"ERROR: {result}"
        for path, result in results
    }

# Run
results = asyncio.run(batch_extract("documents/"))
```

---

## Chunking strategy for RAG

EdgeParse element boundaries make natural chunk boundaries:

```python
import edgeparse, json

def chunk_for_rag(
    pdf_path: str,
    max_chunk_tokens: int = 512,
    overlap_elements: int = 1,
) -> list[dict]:
    raw = edgeparse.convert(pdf_path, format="json")
    doc = json.loads(raw)

    chunks = []
    current_chunk = []
    current_len = 0

    for el in doc["elements"]:
        if el["type"] in ("header", "footer"):
            continue

        el_len = len(el["text"].split())

        # Heading = natural chunk boundary
        if el["type"] == "heading" and current_chunk:
            chunks.append({
                "text": "\n\n".join(e["text"] for e in current_chunk),
                "page_start": current_chunk[0]["page_number"],
                "page_end": current_chunk[-1]["page_number"],
            })
            current_chunk = current_chunk[-overlap_elements:]  # overlap
            current_len = sum(len(e["text"].split()) for e in current_chunk)

        # Size-based split
        if current_len + el_len > max_chunk_tokens and current_chunk:
            chunks.append({
                "text": "\n\n".join(e["text"] for e in current_chunk),
                "page_start": current_chunk[0]["page_number"],
                "page_end": current_chunk[-1]["page_number"],
            })
            current_chunk = current_chunk[-overlap_elements:]
            current_len = sum(len(e["text"].split()) for e in current_chunk)

        current_chunk.append(el)
        current_len += el_len

    if current_chunk:
        chunks.append({
            "text": "\n\n".join(e["text"] for e in current_chunk),
            "page_start": current_chunk[0]["page_number"],
            "page_end": current_chunk[-1]["page_number"],
        })

    return chunks
```
