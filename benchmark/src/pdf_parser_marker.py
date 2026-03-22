"""PDF parser using Marker (VikParuchuri/marker).

Install: pip install marker-pdf
"""

import logging
import os
import subprocess
import sys
import tempfile
import shutil
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def _find_marker_single() -> str:
    """Return path to the marker_single CLI entry point.

    Marker uses click CLI scripts, not a ``__main__`` module, so
    ``python -m marker`` fails.  We look next to the current interpreter
    first (works inside ``uv run --with marker-pdf``), then fall back to
    the system PATH.
    """
    # Same bin/ directory as the running interpreter (works in uv isolated env)
    candidate = Path(sys.executable).parent / "marker_single"
    if candidate.exists():
        return str(candidate)
    found = shutil.which("marker_single")
    if found:
        return found
    raise RuntimeError(
        "marker_single not found. Install with: pip install marker-pdf"
    )


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to Markdown using the marker_single CLI."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    try:
        marker_bin = _find_marker_single()
    except RuntimeError as exc:
        logger.error("Cannot run Marker: %s", exc)
        return

    for pdf_path in document_paths:
        try:
            with tempfile.TemporaryDirectory() as tmp_dir:
                # Force CPU to avoid Apple MPS AcceleratorError in Surya's
                # unpack_qkv_with_mask. MPS has a known issue where large
                # image sequences (>2048 patches) cause index-out-of-bounds
                # in the vision encoder attention, and this cannot be reliably
                # fixed via FOUNDATION_CHUNK_SIZE alone (the chunk granularity
                # is per-page, so a single large page still overflows).
                env = {
                    **os.environ,
                    "TORCH_DEVICE": "cpu",
                    "DETECTOR_IMAGE_CHUNK_HEIGHT": "700",
                }
                cmd = [
                    marker_bin,
                    str(pdf_path),
                    "--output_format", "markdown",
                    "--output_dir", tmp_dir,
                    "--disable_image_extraction",
                ]
                result = subprocess.run(
                    cmd, capture_output=True, text=True, timeout=600, env=env,
                )
                if result.returncode != 0:
                    logger.error("Marker failed on %s: %s", pdf_path.name, result.stderr[-400:])
                    continue

                # Marker writes output in a subdirectory named after the file
                for md_file in Path(tmp_dir).rglob("*.md"):
                    out_file = output_dir / f"{pdf_path.stem}.md"
                    shutil.copy2(str(md_file), str(out_file))
                    break
        except subprocess.TimeoutExpired:
            logger.error("Marker timed out on %s", pdf_path.name)
        except Exception as exc:
            logger.error("Marker failed on %s: %s", pdf_path.name, exc)
