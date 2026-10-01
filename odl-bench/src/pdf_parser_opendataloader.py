"""PDF parser using the published opendataloader-pdf package."""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path
from typing import List


def _find_odl_command() -> str:
    cmd = shutil.which("opendataloader-pdf")
    if cmd is None:
        raise RuntimeError(
            "opendataloader-pdf command not found in PATH.\n"
            "Install with: cd odl-bench && uv sync --extra opendataloader\n"
            "Requires Java 11+."
        )
    return cmd


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    cmd = _find_odl_command()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    command = [
        cmd,
        *[str(p) for p in document_paths],
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

    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        print("Error converting PDFs with opendataloader-pdf:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
