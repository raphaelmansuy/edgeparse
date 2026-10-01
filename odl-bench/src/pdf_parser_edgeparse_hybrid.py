"""PDF parser using local edgeparse with Docling Fast hybrid routing.

Requires ``opendataloader-pdf-hybrid`` listening on ``127.0.0.1:5002``.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path
from typing import List

import pdf_parser_edgeparse as edgeparse

DEFAULT_URL = "http://127.0.0.1:5002"


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    """Convert PDFs with EdgeParse hybrid triage → Docling Fast for complex pages."""
    binary = edgeparse._find_edgeparse_binary()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    backend_url = os.environ.get("DOCLING_URL") or os.environ.get("HYBRID_URL", DEFAULT_URL)
    timeout_ms = os.environ.get("HYBRID_TIMEOUT", "600000")

    # Canonical DoclingDocument.export_to_markdown for hybrid JSON responses.
    repo_root = Path(__file__).resolve().parents[2]
    hybrid_python = repo_root / "benchmark" / ".venvs" / "hybrid" / "bin" / "python"
    env = os.environ.copy()
    if hybrid_python.exists():
        env["EDGEPARSE_DOCLING_PYTHON"] = str(hybrid_python)

    command_base = [
        str(binary),
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

    for pdf_path in document_paths:
        command = [command_base[0], str(pdf_path), *command_base[1:]]
        result = subprocess.run(command, capture_output=True, text=True, env=env)
        if result.returncode != 0:
            print(f"Error converting {pdf_path.name} with edgeparse hybrid:", file=sys.stderr)
            print(result.stderr, file=sys.stderr)
