"""Engine registry for the official OpenDataLoader board harness.

Runnable board competitors (this harness):
  edgeparse, edgeparse_hybrid, opendataloader, opendataloader_hybrid,
  docling, pymupdf4llm, markitdown, unstructured

Not run (GPL/AGPL/commercial): marker, mineru, nutrient — recorded as skipped.
"""

from __future__ import annotations

from typing import Callable, Dict, List

import pdf_parser_edgeparse as edgeparse

EngineHandler = Callable[..., None]

# Engines we attempt to score on this board
BOARD_ENGINES: List[str] = [
    "edgeparse",
    "edgeparse_hybrid",
    "opendataloader",
    "opendataloader_hybrid",
    "docling",
    "pymupdf4llm",
    "markitdown",
    "unstructured",
]

# Explicitly out of scope for this harness
SKIPPED_ENGINES: Dict[str, str] = {
    "marker": "GPL — not run",
    "mineru": "AGPL — not run",
    "nutrient": "commercial — not run",
}

ENGINES: Dict[str, str] = {
    "edgeparse": "local-rust",
}

ENGINE_DISPATCH: Dict[str, EngineHandler] = {
    "edgeparse": edgeparse.to_markdown,
}

ENGINE_META: Dict[str, tuple] = {
    "edgeparse": ("EdgeParse", None, "Local Rust binary (--table-method cluster)"),
    "edgeparse_hybrid": (
        "EdgeParse [hybrid]",
        None,
        "Local Rust + Docling Fast hybrid (127.0.0.1:5002)",
    ),
    "opendataloader": ("OpenDataLoader", "opendataloader-pdf", "Published ODL CLI"),
    "opendataloader_hybrid": (
        "OpenDataLoader [hybrid]",
        "opendataloader-pdf",
        "ODL + Docling Fast hybrid",
    ),
    "docling": ("Docling", "docling", "IBM Docling"),
    "pymupdf4llm": ("PyMuPDF4LLM", "pymupdf4llm", "PyMuPDF for LLM/RAG"),
    "markitdown": ("MarkItDown", "markitdown[all]", "Microsoft MarkItDown"),
    "unstructured": ("Unstructured", "unstructured[pdf]", "Unstructured fast strategy"),
}


def _try_register(name: str, module_name: str, version_label: str = "installed") -> None:
    try:
        mod = __import__(module_name)
        ENGINES[name] = version_label
        ENGINE_DISPATCH[name] = mod.to_markdown
    except Exception:
        pass


_try_register("edgeparse_hybrid", "pdf_parser_edgeparse_hybrid", "local-rust")
_try_register("opendataloader", "pdf_parser_opendataloader", "published")
_try_register("opendataloader_hybrid", "pdf_parser_opendataloader_hybrid", "local-hybrid")
_try_register("docling", "pdf_parser_docling", "installed")
_try_register("pymupdf4llm", "pdf_parser_pymupdf4llm", "installed")
_try_register("markitdown", "pdf_parser_markitdown", "installed")
_try_register("unstructured", "pdf_parser_unstructured", "installed")


def available_engines() -> List[str]:
    return sorted(ENGINES.keys())


def display_name(engine: str) -> str:
    meta = ENGINE_META.get(engine)
    return meta[0] if meta else engine
