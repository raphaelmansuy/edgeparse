"""PDF parser using MarkItDown (Microsoft)."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    from markitdown import MarkItDown

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    md = MarkItDown()

    for pdf_path in document_paths:
        try:
            result = md.convert(str(pdf_path))
            (output_dir / f"{pdf_path.stem}.md").write_text(
                result.text_content, encoding="utf-8"
            )
        except Exception as exc:
            logger.error("MarkItDown failed on %s: %s", pdf_path.name, exc)
