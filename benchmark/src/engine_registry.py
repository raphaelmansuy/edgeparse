"""Centralised definitions for available PDF parsing engines.

Engines:
  * ``edgeparse``        — Rust binary built from this repository (always available)
  * ``opendataloader`` — Published Java/Python package (opendataloader-pdf ≥ 2.0)
  * ``docling``        — IBM Research Docling (pip install docling)
  * ``marker``         — Marker PDF (pip install marker-pdf)
  * ``mineru``         — MinerU/OpenDataLab (pip install mineru[all])
  * ``pymupdf4llm``    — PyMuPDF4LLM (pip install pymupdf4llm)
  * ``markitdown``     — Microsoft MarkItDown (pip install markitdown[all])

External engines are registered automatically when their packages are installed.
"""

from __future__ import annotations

from typing import Callable, Dict

import pdf_parser_edgeparse as edgeparse

EngineHandler = Callable[..., None]

# Engine name → version/source label
ENGINES: Dict[str, str] = {
    "edgeparse": "local-rust",
}

# Engine name → to_markdown callable
ENGINE_DISPATCH: Dict[str, EngineHandler] = {
    "edgeparse": edgeparse.to_markdown,
}

# Engine display metadata: name → (display_name, pip_package, description)
ENGINE_META: Dict[str, tuple] = {
    "edgeparse":        ("EdgeParse",        None,                "Rust PDF engine (this repo)"),
    "opendataloader": ("OpenDataLoader", "opendataloader-pdf","Java/Python PDF engine"),
    "docling":        ("Docling",        "docling",           "IBM Research document parser"),
    # "marker":         ("Marker",         "marker-pdf",        "ML-based PDF-to-Markdown"),
    # "mineru":         ("MinerU",         "mineru[all]",       "OpenDataLab PDF extractor"),
    "pymupdf4llm":    ("PyMuPDF4LLM",   "pymupdf4llm",       "PyMuPDF for LLM/RAG"),
    "markitdown":     ("MarkItDown",     "markitdown[all]",   "Microsoft multi-format converter"),
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
_try_register("docling",        "pdf_parser_docling",        "installed")


_try_register("pymupdf4llm",    "pdf_parser_pymupdf4llm",   "installed")
_try_register("markitdown",     "pdf_parser_markitdown",     "installed")



def available_engines() -> list:
    """Return sorted list of engine names that are currently available."""
    return sorted(ENGINES.keys())


def display_name(engine: str) -> str:
    """Return the human-friendly display name for an engine."""
    meta = ENGINE_META.get(engine)
    return meta[0] if meta else engine
