"""PDF parser using local edgeparse with real Docling Fast hybrid routing.

Requires:
  - ``cargo build --release`` (hybrid feature enabled on the CLI)
  - Local hybrid server: ``opendataloader-pdf-hybrid --port 5002``

Environment:
  DOCLING_URL / HYBRID_URL — backend URL (default http://localhost:5002)
  HYBRID_TIMEOUT — timeout in milliseconds (default 600000)
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import List


DEFAULT_URL = "http://127.0.0.1:5002"


def _find_edgeparse_binary() -> Path:
    """Reuse the shared resolver from the deterministic EdgeParse adapter."""
    import pdf_parser_edgeparse as edgeparse

    return edgeparse._find_edgeparse_binary()


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs with EdgeParse hybrid triage → Docling Fast for complex pages."""
    binary = _find_edgeparse_binary()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    backend_url = os.environ.get("DOCLING_URL") or os.environ.get("HYBRID_URL", DEFAULT_URL)
    timeout_ms = os.environ.get("HYBRID_TIMEOUT", "600000")

    command = [
        str(binary),
        *[str(pdf_path) for pdf_path in document_paths],
        "--output-dir",
        str(output_dir),
        "--format",
        "markdown",
        "--table-method",
        "cluster",
        "--image-output",
        "off",
        "--hybrid",
        "docling-fast",
        "--hybrid-mode",
        "auto",
        "--hybrid-url",
        backend_url,
        "--hybrid-timeout",
        timeout_ms,
        "--hybrid-fallback",
        "--quiet",
    ]

    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        print("Error converting PDFs with edgeparse hybrid:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
