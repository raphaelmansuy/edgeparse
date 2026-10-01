"""PDF parser using Unstructured (hi_res strategy).

Install: pip install "unstructured[pdf]"
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    """Convert PDFs to Markdown using Unstructured hi_res partitioning."""
    from unstructured.partition.pdf import partition_pdf

    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    for pdf_path in document_paths:
        try:
            elements = partition_pdf(filename=str(pdf_path), strategy="hi_res")
            lines = []
            for el in elements:
                category = getattr(el, "category", "") or type(el).__name__
                text = (getattr(el, "text", None) or str(el)).strip()
                if not text:
                    continue
                if category in {"Title", "Header"}:
                    lines.append(f"# {text}")
                elif category == "Table":
                    lines.append(text)
                else:
                    lines.append(text)
                lines.append("")
            out_file = output_dir / f"{pdf_path.stem}.md"
            out_file.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
        except Exception as exc:
            logger.error("Unstructured (hi_res) failed on %s: %s", pdf_path.name, exc)
