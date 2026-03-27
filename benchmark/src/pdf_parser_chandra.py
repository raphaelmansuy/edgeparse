"""PDF parser using Chandra OCR (chandra-ocr CLI).

Chandra is an OCR model that converts PDFs/images to structured Markdown/HTML/JSON
while preserving layout.

Install:
  pip install chandra-ocr

Runtime note:
  By default the Chandra CLI uses `--method vllm` which requires a local vLLM
  server (see Chandra docs: `chandra_vllm`). For local inference you can use
  `--method hf` but that requires the `chandra-ocr[hf]` extra (torch).

This adapter writes one Markdown file per input PDF into the benchmark output
directory, matching the evaluator's expected layout.
"""

from __future__ import annotations

import logging
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import List

logger = logging.getLogger(__name__)


def _find_chandra() -> str:
    found = shutil.which("chandra")
    if found:
        return found
    raise RuntimeError("chandra CLI not found. Install with: pip install chandra-ocr")


def _method() -> str:
    # Chandra supports: hf | vllm
    return os.environ.get("CHANDRA_METHOD", "vllm")


def _copy_outputs(document_paths: List[Path], chandra_out: Path, output_dir: Path) -> None:
    for pdf in document_paths:
        stem = pdf.stem
        produced = chandra_out / stem / f"{stem}.md"
        if not produced.exists():
            logger.error("Chandra did not produce markdown for %s (missing %s)", pdf.name, produced)
            continue
        dest = output_dir / f"{stem}.md"
        try:
            dest.write_text(produced.read_text(encoding="utf-8"), encoding="utf-8")
        except Exception as exc:
            logger.error("Failed to copy Chandra output for %s: %s", pdf.name, exc)


def to_markdown(document_paths: List[Path], input_path: Path, output_dir: Path):
    """Convert PDFs to Markdown using the Chandra CLI."""
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    try:
        chandra_bin = _find_chandra()
    except RuntimeError as exc:
        logger.error("Cannot run Chandra: %s", exc)
        return

    method = _method()

    with tempfile.TemporaryDirectory(prefix="edgeparse-bench-chandra-") as tmp:
        tmp_dir = Path(tmp)
        chandra_out = tmp_dir / "out"
        chandra_out.mkdir(parents=True, exist_ok=True)

        # Run Chandra once for the full batch to avoid re-loading the model per PDF.
        # If the benchmark passes a subset of documents, stage them into a temp dir.
        staged_input = tmp_dir / "in"
        staged_input.mkdir(parents=True, exist_ok=True)
        if len(document_paths) == 1 and Path(input_path).is_file():
            run_input = Path(input_path)
        else:
            for p in document_paths:
                # Copy (not symlink) to keep it portable across filesystems.
                shutil.copy2(p, staged_input / p.name)
            run_input = staged_input

        cmd = [
            chandra_bin,
            str(run_input),
            str(chandra_out),
            "--method",
            method,
            "--no-html",
            "--no-images",
        ]

        try:
            r = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=3600,
                env=os.environ.copy(),
            )
        except subprocess.TimeoutExpired:
            logger.error("Chandra timed out")
            return

        if r.returncode != 0:
            logger.error("Chandra failed: %s", (r.stderr or r.stdout)[-800:])
            return

        _copy_outputs(document_paths, chandra_out, output_dir)

