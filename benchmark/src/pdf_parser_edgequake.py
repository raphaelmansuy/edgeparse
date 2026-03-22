"""PDF parser using edgequake-pdf2md (pdf2md CLI).

Uses Vision Language Models (default: gpt-4.1-nano via OpenAI) to convert PDFs
to Markdown by rasterising each page and sending images to the VLM API.

Install: cargo install edgequake-pdf2md
Requires: OPENAI_API_KEY (or other provider API key)
"""

import logging
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)

_MODEL = "gpt-4.1-nano"
_PROVIDER = "openai"


def _find_pdf2md() -> str:
    """Return path to the pdf2md CLI binary."""
    # Check ~/.cargo/bin first (typical cargo install location)
    cargo_bin = Path.home() / ".cargo" / "bin" / "pdf2md"
    if cargo_bin.exists():
        return str(cargo_bin)
    found = shutil.which("pdf2md")
    if found:
        return found
    raise RuntimeError(
        "pdf2md not found. Install with: cargo install edgequake-pdf2md"
    )


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to Markdown using edgequake-pdf2md (VLM-based)."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    try:
        pdf2md_bin = _find_pdf2md()
    except RuntimeError as exc:
        logger.error("Cannot run edgequake-pdf2md: %s", exc)
        return

    for pdf_path in document_paths:
        out_file = output_dir / f"{pdf_path.stem}.md"
        cmd = [
            pdf2md_bin,
            str(pdf_path),
            "--model", _MODEL,
            "--provider", _PROVIDER,
            "--output", str(out_file),
        ]
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=600,
                env=os.environ.copy(),
            )
            if result.returncode != 0:
                logger.error(
                    "edgequake-pdf2md failed on %s: %s",
                    pdf_path.name,
                    result.stderr[-400:],
                )
        except subprocess.TimeoutExpired:
            logger.error("edgequake-pdf2md timed out on %s", pdf_path.name)
        except Exception as exc:
            logger.error("edgequake-pdf2md error on %s: %s", pdf_path.name, exc)
