"""PDF parser using local edgeparse Rust build with hybrid-enabled OCR recovery."""

import subprocess
import sys
from pathlib import Path
from typing import List


def _find_edgeparse_binary() -> Path:
    """Find the locally built edgeparse Rust binary."""
    repo_root = Path(__file__).parent.parent.parent.resolve()
    binary = repo_root / "target" / "release" / "edgeparse"
    if not binary.exists():
        raise FileNotFoundError(
            f"edgeparse binary not found at {binary}. "
            "Run: cargo build --release"
        )
    return binary


def to_markdown(document_paths: List[Path], _input_path, output_dir: Path):
    """Convert PDFs to Markdown using EdgeParse with hybrid-gated OCR enabled."""
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
        "--hybrid", "docling-fast",
        "--quiet",
    ]

    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        print("Error converting PDFs with edgeparse hybrid OCR:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
