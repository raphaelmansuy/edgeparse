"""PDF parser using published opendataloader-pdf with hybrid docling-fast backend.

Requirements:
  - Java 11+
  - ``pip install "opendataloader-pdf[hybrid]"``
  - Hybrid server running: ``opendataloader-pdf-hybrid --port 5002``

Environment:
  DOCLING_URL / HYBRID_URL — backend URL (default http://localhost:5002)
  HYBRID_TIMEOUT — request timeout in milliseconds (default 600000)
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import List


DEFAULT_URL = "http://127.0.0.1:5002"


def _find_odl_command() -> str:
    cmd = shutil.which("opendataloader-pdf")
    if cmd is None:
        raise RuntimeError(
            "opendataloader-pdf command not found in PATH.\n"
            "Install with: pip install \"opendataloader-pdf[hybrid]\"\n"
            "Requires Java 11+. Check with: java -version"
        )
    return cmd


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    """Convert PDFs to Markdown via published CLI + local Docling Fast hybrid."""
    cmd = _find_odl_command()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    backend_url = os.environ.get("DOCLING_URL") or os.environ.get("HYBRID_URL", DEFAULT_URL)
    timeout_ms = os.environ.get("HYBRID_TIMEOUT", "600000")

    command = [
        cmd,
        *[str(p) for p in document_paths],
        "--output-dir",
        str(output_dir),
        "--format",
        "markdown",
        "--image-output",
        "off",
        "--quiet",
        "--hybrid",
        "docling-fast",
        "--hybrid-url",
        backend_url,
        "--hybrid-timeout",
        timeout_ms,
        "--hybrid-fallback",
    ]

    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        print("Error converting PDFs with opendataloader-pdf hybrid:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
