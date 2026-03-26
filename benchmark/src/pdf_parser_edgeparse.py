"""PDF parser using local edgeparse Rust build."""

import subprocess
import sys
from pathlib import Path
from typing import List


def _find_edgeparse_binary() -> Path:
    """Find the locally built edgeparse Rust binary.

    In the standalone edgeparse repo the layout is:
        <repo-root>/
            benchmark/src/pdf_parser_edgeparse.py   ← this file
            target/release/edgeparse                 ← the binary
    """
    # benchmark/src/ → benchmark/ → repo-root/
    repo_root = Path(__file__).parent.parent.parent.resolve()
    binary = repo_root / "target" / "release" / "edgeparse"
    if not binary.exists():
        raise FileNotFoundError(
            f"edgeparse binary not found at {binary}. "
            "Run: cargo build --release"
        )
    return binary


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to Markdown using the local edgeparse Rust binary."""
    binary = _find_edgeparse_binary()
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    command = [
        str(binary),
        *[str(pdf_path) for pdf_path in document_paths],
        "--output-dir", str(output_dir),
        "--format", "markdown",
        "--table-method", "cluster",
        "--image-output", "off",
        "--quiet",
    ]

    env = dict(**__import__("os").environ)
    env["EDGEPARSE_RASTER_TABLE_OCR"] = "off"

    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        env=env,
    )

    if result.returncode != 0:
        print("Error converting PDFs with edgeparse:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
