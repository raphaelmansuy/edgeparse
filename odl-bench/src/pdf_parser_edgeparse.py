"""PDF parser using local edgeparse Rust build (official board adapter)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import List


def _find_edgeparse_binary() -> Path:
    """Find the locally built edgeparse Rust binary.

    Checks, in order:
      1. ``EDGEPARSE_BIN`` environment override
      2. ``<repo>/target/release/edgeparse``
      3. ``cargo metadata`` target_directory (custom CARGO_TARGET_DIR)
    """
    if env := os.environ.get("EDGEPARSE_BIN"):
        path = Path(env)
        if path.exists():
            return path

    repo_root = Path(__file__).parent.parent.parent.resolve()
    candidates = [repo_root / "target" / "release" / "edgeparse"]

    try:
        meta = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=str(repo_root),
            capture_output=True,
            text=True,
            check=False,
        )
        if meta.returncode == 0:
            target_dir = Path(json.loads(meta.stdout)["target_directory"])
            candidates.append(target_dir / "release" / "edgeparse")
    except Exception:
        pass

    for binary in candidates:
        if binary.exists():
            return binary

    raise FileNotFoundError(
        "edgeparse binary not found. Tried: "
        + ", ".join(str(c) for c in candidates)
        + ". Run: cargo build --release -p edgeparse-cli"
    )


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    """Convert PDFs to Markdown using the local edgeparse Rust binary."""
    binary = _find_edgeparse_binary()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    env = dict(**os.environ)
    # Raster OCR recovers figure-embedded tables (TEDS) via classical
    # projection + Tesseract — required for local to lead Docling on image grids.
    env["EDGEPARSE_RASTER_TABLE_OCR"] = os.environ.get("EDGEPARSE_RASTER_TABLE_OCR", "on")

    # One PDF per invocation so a slow OCR page cannot kill the whole corpus parse.
    for pdf_path in document_paths:
        command = [
            str(binary),
            str(pdf_path),
            "--output-dir",
            str(output_dir),
            "--format",
            "markdown",
            "--table-method",
            "cluster",
            "--image-output",
            "off",
            "--quiet",
        ]
        result = subprocess.run(command, capture_output=True, text=True, env=env)
        if result.returncode != 0:
            print(f"Error converting {pdf_path.name} with edgeparse:", file=sys.stderr)
            print(result.stderr, file=sys.stderr)
