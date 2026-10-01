"""PDF to Markdown conversion runner for the official board harness."""

from __future__ import annotations

import argparse
import json
import logging
import time
from pathlib import Path
from typing import List, Optional

import cpuinfo

from engine_registry import ENGINES, ENGINE_DISPATCH

DEFAULT_INPUT_DIR = "pdfs"


def process_markdown(
    engine_name: str,
    input_dir_name: str,
    doc_id: Optional[str] = None,
    doc_ids: Optional[List[str]] = None,
) -> None:
    """Run PDF-to-Markdown conversion for a single engine."""
    project_root = Path(__file__).parent.parent.resolve()

    if engine_name not in ENGINES:
        raise ValueError(
            f"Engine '{engine_name}' is not available. "
            f"Registered: {sorted(ENGINES.keys())}"
        )

    engine_version = ENGINES[engine_name]
    input_dir = Path(input_dir_name).resolve()
    output_dir = project_root / "prediction" / engine_name / "markdown"
    output_dir.mkdir(parents=True, exist_ok=True)

    if doc_id and doc_ids:
        raise ValueError("Use either doc_id or doc_ids, not both.")

    if doc_id:
        candidate_path = input_dir / f"{doc_id.strip()}.pdf"
        if not candidate_path.exists():
            raise FileNotFoundError(f"'{doc_id.strip()}.pdf' not found in {input_dir}.")
        document_paths = [candidate_path]
        input_path = candidate_path
    elif doc_ids:
        document_paths = []
        for did in doc_ids:
            candidate_path = input_dir / f"{did.strip()}.pdf"
            if not candidate_path.exists():
                raise FileNotFoundError(f"'{did.strip()}.pdf' not found in {input_dir}.")
            document_paths.append(candidate_path)
        input_path = input_dir
    else:
        document_paths = sorted(input_dir.glob("*.pdf"))
        input_path = input_dir
        if not document_paths:
            raise FileNotFoundError(f"No PDFs found in {input_dir}.")

    document_count = len(document_paths)
    logging.info(
        "Processing %d PDFs with %s %s...", document_count, engine_name, engine_version
    )

    start_time = time.time()
    to_markdown_func = ENGINE_DISPATCH.get(engine_name)
    if not to_markdown_func:
        raise ValueError(f"Unknown engine: {engine_name}")
    to_markdown_func(document_paths, input_path, output_dir)
    total_elapsed = time.time() - start_time

    # Count pages from produced markdown filenames (1 PDF = N pages unknown here);
    # store elapsed_per_doc; compare.py derives s/page from page counts when available.
    produced = list(output_dir.glob("*.md"))
    page_count = _estimate_page_count(document_paths)

    summary_data = {
        "engine_name": engine_name,
        "engine_version": engine_version,
        "processor": cpuinfo.get_cpu_info().get("brand_raw", "unknown"),
        "document_count": document_count,
        "markdown_count": len(produced),
        "page_count": page_count,
        "total_elapsed": total_elapsed,
        "elapsed_per_doc": total_elapsed / document_count if document_count else 0,
        "elapsed_per_page": total_elapsed / page_count if page_count else None,
        "date": time.strftime("%Y-%m-%d"),
        "formula": "official-odl-board",
    }

    summary_file_path = output_dir.parent / "summary.json"
    with open(summary_file_path, "w", encoding="utf-8") as f:
        json.dump(summary_data, f, indent=4)
    logging.info("Summary saved to %s", summary_file_path)


def _estimate_page_count(document_paths: List[Path]) -> int:
    """Best-effort page count via pypdf / PyMuPDF if available; else document count."""
    total = 0
    try:
        import fitz  # type: ignore

        for path in document_paths:
            with fitz.open(path) as doc:
                total += doc.page_count
        return total
    except Exception:
        pass
    try:
        from pypdf import PdfReader  # type: ignore

        for path in document_paths:
            total += len(PdfReader(str(path)).pages)
        return total
    except Exception:
        pass
    return len(document_paths)


def _parse_args(argv: Optional[List[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Parse PDFs for odl-bench.")
    parser.add_argument("--input-dir", type=str, default=DEFAULT_INPUT_DIR)
    parser.add_argument(
        "--engine",
        type=str,
        default=None,
        choices=list(ENGINES.keys()) or None,
    )
    parser.add_argument("--doc-id", type=str, default=None)
    return parser.parse_args(argv)


def main(argv: Optional[List[str]] = None) -> None:
    logging.basicConfig(level=logging.INFO)
    args = _parse_args(argv)
    engines = [args.engine] if args.engine else list(ENGINES.keys())
    for engine in engines:
        process_markdown(engine, args.input_dir, doc_id=args.doc_id)


if __name__ == "__main__":
    main()
