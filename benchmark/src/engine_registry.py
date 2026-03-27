"""Centralised definitions for available PDF parsing engines.

Engines:
  * ``edgeparse``      — Rust binary built from this repository (always available)
  * ``edgeparse_hybrid`` — EdgeParse with hybrid-gated local OCR recovery
  * ``opendataloader`` — Published Java/Python package (opendataloader-pdf ≥ 2.0)
  * ``opendataloader_hybrid_docling_fast`` — OpenDataLoader hybrid with Docling Fast backend
  * ``opendataloader_hybrid_hancom``       — OpenDataLoader hybrid with Hancom backend
  * ``pymupdf4llm``    — PyMuPDF4LLM (pip install pymupdf4llm)
  * ``markitdown``     — Microsoft MarkItDown (pip install markitdown[all])
  * ``liteparse``      — LlamaIndex LiteParse (@llamaindex/liteparse, Node.js CLI)
  * ``chandra``        — Chandra OCR 2 (pip install chandra-ocr)    [OCR/ML]
  * ``docling``        — IBM Research Docling (pip install docling)  [OCR/ML]
  * ``marker``         — Marker PDF (pip install marker-pdf)         [OCR/ML, isolated venv]
  * ``mineru``         — MinerU/OpenDataLab (pip install mineru[all])[OCR/ML, isolated venv]

External engines are registered automatically when their packages are installed.

Engine groups (for benchmark segmentation):
  NON_OCR_ENGINES — no ML models, no GPU; pure text/geometry extraction
  HYBRID_ENGINES  — mixed local + backend routing for complex pages
  OCR_ENGINES     — require deep-learning models; GPU optional but recommended
"""

from __future__ import annotations

from typing import Callable, Dict, List

import pdf_parser_edgeparse as edgeparse

EngineHandler = Callable[..., None]

# ── Engine group constants ────────────────────────────────────────────────────
# EdgeParse is the baseline and appears in both groups.
NON_OCR_ENGINES: List[str] = [
    "edgeparse",
    "opendataloader",
    "pymupdf4llm",
    "markitdown",
    "liteparse",
]

HYBRID_ENGINES: List[str] = [
    "edgeparse",
    "edgeparse_hybrid",
    "opendataloader_hybrid_docling_fast",
    "opendataloader_hybrid_hancom",
]

OCR_ENGINES: List[str] = [
    "edgeparse",
    "chandra",
    "docling",
    "marker",
    "mineru",
]

# Engine name → version/source label
ENGINES: Dict[str, str] = {
    "edgeparse": "local-rust",
    "edgeparse_hybrid": "local-rust",
}

# Engine name → to_markdown callable
ENGINE_DISPATCH: Dict[str, EngineHandler] = {
    "edgeparse": edgeparse.to_markdown,
}

# Engine display metadata: name → (display_name, pip_package, description)
ENGINE_META: Dict[str, tuple] = {
    "edgeparse":      ("EdgeParse",                            None,                    "Rust PDF engine (this repo)"),
    "edgeparse_hybrid": ("EdgeParse [hybrid OCR]",             None,                    "EdgeParse with hybrid-gated local OCR recovery"),
    "opendataloader": ("OpenDataLoader",                       "opendataloader-pdf",    "Java/Python PDF engine"),
    "opendataloader_hybrid_docling_fast": ("OpenDataLoader [hybrid/docling-fast]", None, "OpenDataLoader hybrid with Docling Fast backend"),
    "opendataloader_hybrid_hancom":       ("OpenDataLoader [hybrid/hancom]",       None, "OpenDataLoader hybrid with Hancom backend"),
    "pymupdf4llm":    ("PyMuPDF4LLM",                         "pymupdf4llm",           "PyMuPDF for LLM/RAG"),
    "markitdown":     ("MarkItDown",                           "markitdown[all]",       "Microsoft multi-format converter"),
    "liteparse":      ("LiteParse",                            "@llamaindex/liteparse", "LlamaIndex local PDF parser"),
    # OCR / ML engines
    "chandra":        ("Chandra OCR",                          "chandra-ocr",           "Chandra OCR model (CLI) [OCR/ML]"),
    "docling":        ("Docling",                              "docling",               "IBM Research document parser [OCR/ML]"),
    "marker":         ("Marker",                               "marker-pdf",            "Marker PDF — Surya OCR [isolated venv]"),
    "mineru":         ("MinerU",                               "mineru[all]",           "OpenDataLab PDF extractor [isolated venv]"),
}

# ── Auto-register external engines ───────────────────────────────────────────

def _try_register(name: str, module_name: str, version_label: str = "installed"):
    """Attempt to import and register an engine module."""
    try:
        mod = __import__(module_name)
        ENGINES[name] = version_label
        ENGINE_DISPATCH[name] = mod.to_markdown
    except Exception:
        pass

_try_register("opendataloader", "pdf_parser_opendataloader", "published")
_try_register("edgeparse_hybrid", "pdf_parser_edgeparse_hybrid", "local-rust")
_try_register("opendataloader_hybrid_docling_fast", "pdf_parser_opendataloader_hybrid_docling_fast", "local-hybrid")
_try_register("opendataloader_hybrid_hancom", "pdf_parser_opendataloader_hybrid_hancom", "local-hybrid")
_try_register("chandra",        "pdf_parser_chandra",        "installed")
_try_register("docling",        "pdf_parser_docling",        "installed")
_try_register("pymupdf4llm",    "pdf_parser_pymupdf4llm",    "installed")
_try_register("markitdown",     "pdf_parser_markitdown",     "installed")
_try_register("liteparse",      "pdf_parser_liteparse",      "installed")
# marker and mineru run in isolated venvs — not auto-registered here


def available_engines() -> list:
    """Return sorted list of engine names that are currently available."""
    return sorted(ENGINES.keys())


def display_name(engine: str) -> str:
    """Return the human-friendly display name for an engine."""
    meta = ENGINE_META.get(engine)
    return meta[0] if meta else engine
