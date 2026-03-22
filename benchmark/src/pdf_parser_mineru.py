"""PDF parser using MinerU (OpenDataLab).

Install: pip install mineru[all]

Model pre-download (pipeline backend, CPU-friendly):
    uv run --with 'mineru[all]' mineru-models-download \
        --source huggingface --model_type pipeline
"""

import logging
import subprocess
import sys
import shutil
import tempfile
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def _find_mineru() -> str:
    """Return path to the mineru CLI entry point.

    MinerU uses click CLI scripts, not a ``__main__`` module, so
    ``python -m mineru`` fails.  We look next to the current interpreter
    first (works inside ``uv run --with 'mineru[all]'``), then PATH.
    """
    candidate = Path(sys.executable).parent / "mineru"
    if candidate.exists():
        return str(candidate)
    found = shutil.which("mineru")
    if found:
        return found
    raise RuntimeError(
        "mineru not found. Install with: pip install mineru[all]"
    )


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to Markdown using the MinerU CLI (pipeline backend)."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    try:
        mineru_bin = _find_mineru()
    except RuntimeError as exc:
        logger.error("Cannot run MinerU: %s", exc)
        return

    for pdf_path in document_paths:
        try:
            with tempfile.TemporaryDirectory() as tmp_dir:
                cmd = [
                    mineru_bin,
                    "-p", str(pdf_path),
                    "-o", tmp_dir,
                    "-b", "pipeline",  # CPU-friendly; vlm requires GPU
                ]
                result = subprocess.run(
                    cmd, capture_output=True, text=True, timeout=600,
                )
                if result.returncode != 0:
                    logger.error("MinerU failed on %s: %s", pdf_path.name, result.stderr[-400:])
                    continue

                # MinerU outputs in auto/ subfolder with .md extension
                for md_file in Path(tmp_dir).rglob("*.md"):
                    out_file = output_dir / f"{pdf_path.stem}.md"
                    shutil.copy2(str(md_file), str(out_file))
                    break
        except subprocess.TimeoutExpired:
            logger.error("MinerU timed out on %s", pdf_path.name)
        except Exception as exc:
            logger.error("MinerU failed on %s: %s", pdf_path.name, exc)
