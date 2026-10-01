"""PDF parser using Docling (IBM Research)."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    from docling.document_converter import DocumentConverter

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    converter = DocumentConverter()

    for pdf_path in document_paths:
        try:
            result = converter.convert(str(pdf_path))
            md_text = result.document.export_to_markdown()
            (output_dir / f"{pdf_path.stem}.md").write_text(md_text, encoding="utf-8")
        except Exception as exc:
            logger.error("Docling failed on %s: %s", pdf_path.name, exc)
