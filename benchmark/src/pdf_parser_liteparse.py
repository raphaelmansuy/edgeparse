"""PDF parser using LiteParse (LlamaIndex).

Install: npm i -g @llamaindex/liteparse

LiteParse is a fast local PDF parser built on PDF.js that uses spatial text
projection.  It outputs plain text (no structural Markdown), making it a
useful baseline for pure-text reading-order comparisons.
"""

import logging
import shutil
import subprocess
import sys
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def _find_lit() -> str:
    """Return path to the ``lit`` CLI binary."""
    found = shutil.which("lit")
    if found:
        return found
    raise RuntimeError(
        "lit (LiteParse) not found. Install with: npm i -g @llamaindex/liteparse"
    )


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to text using LiteParse (saved as .md for evaluation)."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    try:
        lit_bin = _find_lit()
    except RuntimeError as exc:
        logger.error("Cannot run LiteParse: %s", exc)
        return

    for pdf_path in document_paths:
        out_file = output_dir / f"{pdf_path.stem}.md"
        cmd = [
            lit_bin,
            "parse",
            str(pdf_path),
            "--no-ocr",
            "-q",
            "-o", str(out_file),
        ]
        try:
            result = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=120,
            )
            if result.returncode != 0:
                logger.error(
                    "LiteParse failed on %s: %s",
                    pdf_path.name,
                    result.stderr[-400:] if result.stderr else "(no stderr)",
                )
        except subprocess.TimeoutExpired:
            logger.error("LiteParse timed out on %s", pdf_path.name)
        except Exception as exc:
            logger.error("LiteParse error on %s: %s", pdf_path.name, exc)
