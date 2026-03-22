"""PDF parser using PyMuPDF4LLM (Artifex).

Install: pip install pymupdf4llm
"""

import logging
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to Markdown using pymupdf4llm."""
    import pymupdf4llm

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    for pdf_path in document_paths:
        try:
            md_text = pymupdf4llm.to_markdown(str(pdf_path))
            out_file = output_dir / f"{pdf_path.stem}.md"
            out_file.write_text(md_text, encoding="utf-8")
        except Exception as exc:
            logger.error("PyMuPDF4LLM failed on %s: %s", pdf_path.name, exc)
