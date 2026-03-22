"""PDF parser using the published opendataloader-pdf package.

Install before use:
    make bench-odl-setup
  or manually:
    cd benchmark && uv sync --extra opendataloader

This uses the ``opendataloader-pdf`` CLI command installed by the
``opendataloader-pdf`` Python package.  All PDFs are passed in a single
subprocess invocation so only one JVM process is spawned (fast batch mode).

Requires: Java 11+. Check with: java -version
"""

from __future__ import annotations

import shutil
import subprocess
import sys
from pathlib import Path
from typing import List


def _find_odl_command() -> str:
    """Return the path to the opendataloader-pdf CLI command.

    Raises RuntimeError with a helpful install message if not found.
    """
    cmd = shutil.which("opendataloader-pdf")
    if cmd is None:
        raise RuntimeError(
            "opendataloader-pdf command not found in PATH.\n"
            "Install it with:\n"
            "    make bench-odl-setup\n"
            "  or:\n"
            "    cd benchmark && uv sync --extra opendataloader\n"
            "Requires Java 11+.  Check with: java -version"
        )
    return cmd


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path) -> None:
    """Convert PDFs to Markdown via the published opendataloader-pdf CLI.

    All PDFs are submitted in one subprocess call (single JVM startup cost).
    Uses identical settings to the edgeparse benchmark for a fair comparison:
    cluster table detection, no image output.
    """
    cmd = _find_odl_command()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    command = [
        cmd,
        *[str(p) for p in document_paths],
        "--output-dir", str(output_dir),
        "--format", "markdown",
        "--table-method", "cluster",
        "--image-output", "off",
        "--quiet",
    ]

    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print("Error converting PDFs with opendataloader-pdf:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
        # Do not raise — allow partial results to flow through evaluation
