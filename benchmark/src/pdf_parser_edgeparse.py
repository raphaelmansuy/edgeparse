"""PDF parser using local edgeparse Rust build."""

import subprocess
import sys
from pathlib import Path
from typing import List


def _find_edgeparse_binary() -> Path:
    """Find the locally built edgeparse Rust binary.

    Checks, in order:
      1. ``<repo>/target/release/edgeparse`` (default or symlink)
      2. ``cargo metadata`` target_directory (custom CARGO_TARGET_DIR)
      3. ``EDGEPARSE_BIN`` environment override
    """
    import os
    import json
    import subprocess

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
    env["EDGEPARSE_RASTER_TABLE_OCR"] = __import__("os").environ.get(
        "EDGEPARSE_RASTER_TABLE_OCR", "on"
    )

    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        env=env,
    )

    if result.returncode != 0:
        print("Error converting PDFs with edgeparse:", file=sys.stderr)
        print(result.stderr, file=sys.stderr)
